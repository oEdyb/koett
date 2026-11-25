use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::bundle::marks::MarkWriter;
use super::bundle::records::{Intent, MarkIssue};
use super::bundle::{Bundle, WriteError};
use super::capture::disk::{CaptureFault, CaptureFaultReason, CaptureNotice, free_disk_bytes};
use super::capture::journal::{TrackReader, has_record_bytes};
use super::capture::writer::{TrackConfig, TrackWriter};
use super::capture::{CapturePlatform, SourceEpoch, Timing};
use super::controller::emit;
use super::*;
use crate::linux::meeting_audio::{LinuxCapture, Source};
use crate::runtime::EngineCommand;

pub(super) struct CaptureResources {
    pub(super) marks: MarkWriter,
    bundle: Arc<Bundle>,
    tracks: Tracks<ArmedTrack>,
}

#[cfg(test)]
impl std::fmt::Debug for CaptureResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.bundle.fmt(f)
    }
}

#[cfg(test)]
impl PartialEq for CaptureResources {
    fn eq(&self, other: &Self) -> bool {
        self.bundle == other.bundle
    }
}

struct ArmedTrack {
    source: Source,
    writer: TrackWriter,
}

pub(super) fn open(
    root: &Path,
    attempt: Attempt,
    plan: SessionPlan,
    commands: &Sender<EngineCommand>,
) -> Result<OpenedCapture, OpenError> {
    let mut bundle = Bundle::create(root, plan.manifest).map_err(unavailable)?;
    let result = arm(attempt, &mut bundle, commands);
    match result {
        Ok((marks, tracks, epoch)) => Ok(OpenedCapture {
            capture: LiveCapture::Armed(Box::new(CaptureResources {
                marks,
                tracks,
                bundle: Arc::new(bundle),
            })),
            epoch,
            health: CaptureHealth::default(),
        }),
        Err(error) => {
            if let Err(issue) = bundle.remove_if_empty() {
                eprintln!("Could not clean up unused meeting: {issue}");
            }
            Err(error)
        }
    }
}

fn arm(
    attempt: Attempt,
    bundle: &mut Bundle,
    commands: &Sender<EngineCommand>,
) -> Result<(MarkWriter, Tracks<ArmedTrack>, MeetingEpoch), OpenError> {
    let directory = bundle.directory();
    let free = free_disk_bytes(directory).map_err(unavailable)?;
    if free < capture::disk::DISK_WARNING {
        return Err(OpenError::DiskLow(format!(
            "Meeting needs 1 GiB of free space ({free} bytes available)"
        )));
    }
    let post = commands.clone();
    let mut platform = LinuxCapture::new(
        bundle.manifest().mic.clone(),
        Arc::new(move |channel, health| {
            emit(&post, Event::SourceHealth(attempt, channel, health));
        }),
    )
    .map_err(unavailable)?;
    let epoch = MeetingEpoch(Instant::now());
    let marks = bundle.start_marks().map_err(unavailable)?;
    let mic_rate = platform.mic_rate();
    let other = arm_track(
        &mut platform,
        bundle,
        Channel::Other,
        SAMPLE_RATE,
        (attempt, epoch),
        commands,
    )?;
    match arm_track(
        &mut platform,
        bundle,
        Channel::You,
        mic_rate,
        (attempt, epoch),
        commands,
    ) {
        Ok(you) => Ok((marks, Tracks { you, other }, epoch)),
        Err(error) => {
            let ArmedTrack { source, writer } = other;
            drop(source);
            let _ = writer.finish();
            Err(error)
        }
    }
}

fn arm_track(
    platform: &mut LinuxCapture,
    bundle: &Bundle,
    channel: Channel,
    sample_rate: u32,
    (attempt, epoch): (Attempt, MeetingEpoch),
    commands: &Sender<EngineCommand>,
) -> Result<ArmedTrack, OpenError> {
    let commands = commands.clone();
    let (writer, ingress) = TrackWriter::start(
        TrackConfig {
            path: journal_path(bundle.directory(), channel),
            session_zero: epoch.0,
            sample_rate,
            epoch: SourceEpoch {
                generation: 0,
                timing: match channel {
                    Channel::You => Timing::Callback,
                    Channel::Other => Timing::Arrival,
                },
            },
        },
        free_disk_bytes,
        move |notice| {
            emit(
                &commands,
                match notice {
                    CaptureNotice::Health(status) => Event::DiskHealth(attempt, channel, status),
                    CaptureNotice::Fault(fault) => {
                        Event::CaptureFault(attempt, fault.at, fault.to_string())
                    }
                },
            )
        },
    )
    .map_err(writer_open_error)?;
    match platform.arm(channel, ingress) {
        Ok(source) => Ok(ArmedTrack { source, writer }),
        Err(error) => {
            let _ = writer.finish();
            Err(unavailable(error))
        }
    }
}

fn unavailable(error: io::Error) -> OpenError {
    OpenError::CaptureUnavailable(error.to_string())
}

fn writer_open_error(fault: CaptureFault) -> OpenError {
    match fault.reason {
        CaptureFaultReason::DiskLow { .. } => OpenError::DiskLow(fault.to_string()),
        _ => OpenError::CaptureUnavailable(fault.to_string()),
    }
}

pub(super) fn capture_bundle(capture: &LiveCapture) -> Option<&Arc<Bundle>> {
    match capture {
        LiveCapture::Armed(capture) => Some(&capture.bundle),
        #[cfg(test)]
        LiveCapture::Fixture(_) => None,
    }
}

pub(super) fn persist_intent(capture: &LiveCapture, intent: &Intent) -> IntentWrite {
    let Some(bundle) = capture_bundle(capture) else {
        return IntentWrite::Rejected("Meeting bundle is unavailable".into());
    };
    match bundle.write_intent(intent) {
        Ok(()) => IntentWrite::Committed,
        Err(error @ WriteError::BeforeRename(_)) => IntentWrite::Rejected(error.to_string()),
        Err(error @ WriteError::AfterRename(_)) => IntentWrite::Indeterminate(error.to_string()),
    }
}

pub(super) fn journal_path(directory: &Path, track: Channel) -> PathBuf {
    directory.join("audio").join(match track {
        Channel::You => "you.journal",
        Channel::Other => "other.journal",
    })
}

pub(super) fn drain(
    capture: LiveCapture,
    spec: CloseSpec,
) -> Result<SealedBundle, (RecoverableBundle, String)> {
    match capture {
        LiveCapture::Armed(capture) => drain_armed(*capture, spec),
        #[cfg(test)]
        LiveCapture::Fixture(_) => Ok(SealedBundle::Fixture),
    }
}

fn drain_armed(
    CaptureResources {
        marks,
        bundle,
        tracks,
    }: CaptureResources,
    spec: CloseSpec,
) -> Result<SealedBundle, (RecoverableBundle, String)> {
    let Tracks { you, other } = tracks;
    drop(you.source);
    drop(other.source);
    let you_result = you.writer.finish();
    let other_result = other.writer.finish();
    let mark_issues = marks.close(spec.cutoff);
    if matches!(
        &spec.goal,
        CloseGoal::Interrupted(InterruptCause::IntentUncertain(_))
    ) {
        let _ = bundle.write_intent(&Intent {
            goal: spec.goal.clone(),
            cutoff: spec.cutoff,
        });
    }
    let result = seal(&bundle, Some(spec.cutoff), mark_issues);
    match (result, &spec.goal) {
        (Ok(seal), CloseGoal::Interrupted(_)) => Ok(SealedBundle::Locked { bundle, seal }),
        (Ok(seal), _) => match you_result.and(other_result) {
            Ok(()) => Ok(SealedBundle::Locked { bundle, seal }),
            Err(error) => {
                let issue = error.to_string();
                let _ = bundle.write_intent(&Intent {
                    cutoff: spec.cutoff,
                    goal: CloseGoal::Interrupted(InterruptCause::CaptureFault(issue.clone())),
                });
                Err((RecoverableBundle::Locked(bundle), issue))
            }
        },
        (Err(error), _) => Err((RecoverableBundle::Locked(bundle), error.to_string())),
    }
}

pub(super) fn seal(
    bundle: &Bundle,
    cutoff: Option<Offset>,
    mark_issues: Vec<MarkIssue>,
) -> io::Result<Seal> {
    let max = cutoff.unwrap_or(Offset(u64::MAX));
    let read = |channel| match TrackReader::open(&journal_path(bundle.directory(), channel), max) {
        Ok(reader) => reader.seal(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(TrackSeal::default()),
        Err(error) => Err(error),
    };
    let tracks = Tracks {
        you: read(Channel::You)?,
        other: read(Channel::Other)?,
    };
    let seal = Seal {
        cutoff: cutoff.unwrap_or(tracks.you.captured_until.max(tracks.other.captured_until)),
        tracks,
        mark_issues,
    };
    bundle.write_seal(&seal)?;
    Ok(seal)
}

impl Bundle {
    fn remove_if_empty(self) -> io::Result<()> {
        let directory = self.directory();
        for channel in [Channel::You, Channel::Other] {
            if has_record_bytes(&journal_path(directory, channel))? {
                return Ok(());
            }
        }
        fs::remove_dir_all(directory)?;
        if let Some(parent) = directory.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    }
}
