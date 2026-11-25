use super::records::JobError;
use super::*;
use crate::meeting::{Artifact, MeetingView};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiskState {
    Active {
        id: MeetingId,
        manifest: Option<Manifest>,
    },
    Recoverable {
        manifest: Manifest,
        reason: RecoveryReason,
        seal: Option<Seal>,
    },
    Complete {
        manifest: Manifest,
        receipt: Receipt,
        seal: Option<Seal>,
        projections: Projections,
        audio_retained: bool,
    },
    Unreadable {
        id: MeetingId,
        issue: String,
    },
}

impl DiskState {
    pub(crate) fn is_recoverable(&self) -> bool {
        match self {
            Self::Recoverable { .. } => true,
            Self::Complete {
                receipt,
                audio_retained,
                ..
            } => *audio_retained && receipt.publication.fallback,
            _ => false,
        }
    }

    pub(crate) fn view(self, root: &Path) -> MeetingView {
        match self {
            Self::Active { id, manifest } => MeetingView::Active(manifest.map_or(
                SessionInfo {
                    id,
                    started_at_ms: 0,
                },
                |manifest| manifest.info,
            )),
            Self::Recoverable {
                manifest, reason, ..
            } => MeetingView::Recoverable(manifest.info, reason),
            Self::Complete {
                manifest,
                receipt,
                projections,
                audio_retained,
                ..
            } => {
                let directory = root.join(manifest.info.id.to_string());
                let publication = receipt.publication;
                MeetingView::Complete(
                    manifest.info,
                    Artifact {
                        markdown: directory.join(TRANSCRIPT),
                        assets: publication
                            .assets
                            .into_iter()
                            .map(|path| directory.join(path))
                            .collect(),
                        backends: publication.backends,
                        fallback: publication.fallback,
                        audio_retained,
                    },
                    projections,
                )
            }
            Self::Unreadable { id, issue } => MeetingView::Unreadable { id, issue },
        }
    }

    pub(in crate::meeting) fn needs_replay(&self) -> bool {
        matches!(self, Self::Complete { manifest, receipt, projections, audio_retained, .. }
            if projections.has_pending()
                || (*audio_retained && !manifest.keep_audio && !receipt.publication.fallback))
    }

    pub(crate) fn duration(&self) -> Option<Offset> {
        match self {
            Self::Recoverable { seal, .. } | Self::Complete { seal, .. } => {
                seal.as_ref().map(|seal| seal.cutoff)
            }
            _ => None,
        }
    }

    pub(crate) fn id(&self) -> MeetingId {
        match self {
            Self::Recoverable { manifest, .. } | Self::Complete { manifest, .. } => {
                manifest.info.id
            }
            Self::Active { id, .. } | Self::Unreadable { id, .. } => *id,
        }
    }

    fn started_at_ms(&self) -> Option<u64> {
        match self {
            Self::Recoverable { manifest, .. } | Self::Complete { manifest, .. } => {
                Some(manifest.info.started_at_ms)
            }
            Self::Active { manifest, .. } => manifest
                .as_ref()
                .map(|manifest| manifest.info.started_at_ms),
            Self::Unreadable { .. } => None,
        }
    }
}

pub(crate) fn scan(root: &Path, id: MeetingId) -> io::Result<Option<DiskState>> {
    let directory = root.join(id.to_string());
    if !directory.try_exists()? {
        return Ok(None);
    }
    Ok(Some(inspect(&directory, id).unwrap_or_else(|error| {
        DiskState::Unreadable {
            id,
            issue: error.to_string(),
        }
    })))
}

pub(crate) fn scan_all(root: &Path) -> io::Result<Vec<DiskState>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut states = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        if let Some(id) = name.to_str().and_then(MeetingId::parse)
            && name == id.to_string().as_str()
            && let Some(state) = scan(root, id)?
        {
            states.push(state);
        }
    }
    states.sort_unstable_by_key(|state| (std::cmp::Reverse(state.started_at_ms()), state.id()));
    Ok(states)
}

fn inspect(directory: &Path, id: MeetingId) -> io::Result<DiskState> {
    let lock = match File::open(directory.join(LOCK)) {
        Ok(lock) => Some(lock),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if let Some(lock) = lock {
        match lock.try_lock_shared() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Ok(DiskState::Active {
                    id,
                    manifest: read_manifest(directory, id).ok(),
                });
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
    let manifest = read_manifest(directory, id)?;
    Ok(inspect_unlocked(directory, manifest))
}

pub(super) fn inspect_unlocked(directory: &Path, manifest: Manifest) -> DiskState {
    let seal = read_json::<Seal>(&directory.join(SEAL)).ok();
    if let Ok(receipt) = read_json::<Receipt>(&directory.join(RECEIPT))
        && valid_transcript(directory, &receipt).unwrap_or(false)
    {
        let projections = read_json::<ProjectionFile>(&directory.join(PROJECTIONS))
            .ok()
            .filter(|file| file.transcript_sha256 == receipt.transcript_sha256)
            .map_or_else(|| Projections::pending(&manifest), |file| file.projections);
        let audio_retained = ["you.journal", "other.journal"]
            .iter()
            .any(|name| directory.join("audio").join(name).is_file());
        return DiskState::Complete {
            manifest,
            receipt,
            seal,
            projections,
            audio_retained,
        };
    }
    let reason = if seal.is_some() {
        recovery_reason(directory)
    } else {
        RecoveryReason::Interrupted
    };
    DiskState::Recoverable {
        manifest,
        reason,
        seal,
    }
}

fn recovery_reason(directory: &Path) -> RecoveryReason {
    if let Ok(job) = read_json::<Job>(&directory.join(JOB)) {
        return match job.last_error {
            JobError::Decode(issue) => RecoveryReason::DecodeFailed(issue),
            JobError::Publish(issue) => RecoveryReason::PublishFailed(issue),
        };
    }
    match read_json::<Intent>(&directory.join(INTENT)).map(|intent| intent.goal) {
        Ok(CloseGoal::Hold) => RecoveryReason::HeldByUser,
        Ok(CloseGoal::Interrupted(cause)) => cause.recovery_reason(),
        _ => RecoveryReason::Interrupted,
    }
}
