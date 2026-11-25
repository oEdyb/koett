use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::clock::session_offset;
use super::disk::{CaptureFault, CaptureFaultReason, CaptureNotice, DISK_WARNING, DiskGuard};
use super::ingress::{TrackDrain, TrackIngress};
use super::journal::JournalWriter;
use super::recorder::TrackRecorder;
use super::{super::Offset, SourceEpoch};

const SYNC_INTERVAL: Duration = Duration::from_secs(2);
const IDLE_WAIT: Duration = Duration::from_millis(5);

pub(crate) struct TrackConfig {
    pub(crate) path: PathBuf,
    pub(crate) session_zero: Instant,
    pub(crate) sample_rate: u32,
    pub(crate) epoch: SourceEpoch,
}

pub(crate) struct TrackWriter {
    session_zero: Instant,
    stop: Sender<()>,
    worker: JoinHandle<Result<(), CaptureFault>>,
}

impl TrackWriter {
    pub(crate) fn start(
        config: TrackConfig,
        disk_space: impl FnMut(&Path) -> io::Result<u64> + Send + 'static,
        on_notice: impl FnMut(CaptureNotice) + Send + 'static,
    ) -> Result<(Self, TrackIngress), CaptureFault> {
        Self::start_with_clock(config, disk_space, on_notice, Instant::now)
    }

    pub(super) fn start_with_clock(
        config: TrackConfig,
        mut disk_space: impl FnMut(&Path) -> io::Result<u64> + Send + 'static,
        mut on_notice: impl FnMut(CaptureNotice) + Send + 'static,
        now: impl Fn() -> Instant + Send + 'static,
    ) -> Result<(Self, TrackIngress), CaptureFault> {
        let TrackConfig {
            path,
            session_zero,
            sample_rate,
            epoch,
        } = config;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let open_fault = |error| CaptureFault::io(session_offset(session_zero, now()), error);
        let free_bytes = disk_space(parent).map_err(open_fault)?;
        if free_bytes < DISK_WARNING {
            return Err(CaptureFault {
                at: session_offset(session_zero, now()),
                reason: CaptureFaultReason::DiskLow { free_bytes },
            });
        }

        let journal =
            JournalWriter::new(File::create_new(&path).map_err(open_fault)?).map_err(open_fault)?;
        let recorder =
            TrackRecorder::new(journal, session_zero, sample_rate).map_err(open_fault)?;
        recorder.sink().sync_data().map_err(open_fault)?;
        File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(open_fault)?;
        let parent = parent.to_path_buf();
        let (ingress, drain) = TrackIngress::pair(sample_rate, epoch);
        let (stop, stop_rx) = mpsc::channel();
        let started = now();
        let worker = thread::Builder::new()
            .name("meeting-audio-writer".into())
            .spawn(move || {
                let mut writer = WriterWorker {
                    recorder,
                    drain,
                    session_zero,
                    disk: DiskGuard::new(started),
                    last_sync: started,
                    samples: Vec::new(),
                };
                let mut notify = |health| on_notice(CaptureNotice::Health(health));
                let result = writer.run(stop_rx, &now, || disk_space(&parent), &mut notify);
                if let Err(fault) = &result {
                    on_notice(CaptureNotice::Fault(fault.clone()));
                    let _ = writer.recorder.sink().sync_data();
                }
                result
            })
            .map_err(|error| {
                CaptureFault::io(session_offset(session_zero, Instant::now()), error)
            })?;
        Ok((
            Self {
                session_zero,
                stop,
                worker,
            },
            ingress,
        ))
    }

    pub(crate) fn finish(self) -> Result<(), CaptureFault> {
        drop(self.stop);
        self.worker.join().map_err(|_| CaptureFault {
            at: session_offset(self.session_zero, Instant::now()),
            reason: CaptureFaultReason::WorkerPanicked,
        })?
    }
}

struct WriterWorker {
    recorder: TrackRecorder<File>,
    drain: TrackDrain,
    session_zero: Instant,
    disk: DiskGuard,
    last_sync: Instant,
    samples: Vec<f32>,
}

impl WriterWorker {
    fn run(
        &mut self,
        stop: Receiver<()>,
        now: impl Fn() -> Instant,
        mut disk_space: impl FnMut() -> io::Result<u64>,
        mut health_changed: impl FnMut(super::disk::CaptureHealth),
    ) -> Result<(), CaptureFault> {
        while let Err(TryRecvError::Empty) = stop.try_recv() {
            let tick = now();
            let at = session_offset(self.session_zero, tick);
            if let Some(health) = self.disk.poll(tick, at, &mut disk_space)? {
                health_changed(health);
            }
            if tick.duration_since(self.last_sync) >= SYNC_INTERVAL {
                self.recorder
                    .sink()
                    .sync_data()
                    .map_err(|error| CaptureFault::io(at, error))?;
                self.last_sync = tick;
            }
            if !self.drain_one(at)?
                && !matches!(stop.recv_timeout(IDLE_WAIT), Err(RecvTimeoutError::Timeout))
            {
                break;
            }
        }
        let at = session_offset(self.session_zero, now());
        for _ in 0..self.drain.pending_blocks() {
            self.drain_one(at)?;
        }
        self.recorder
            .finish(self.drain.dropped_frames())
            .map_err(|error| CaptureFault::io(at, error))?;
        self.recorder
            .sink()
            .sync_data()
            .map_err(|error| CaptureFault::io(at, error))
    }

    fn drain_one(&mut self, at: Offset) -> Result<bool, CaptureFault> {
        let Some(stamp) = self.drain.pop(&mut self.samples) else {
            return Ok(false);
        };
        self.recorder
            .push(stamp, &self.samples)
            .map_err(|error| CaptureFault::io(at, error))?;
        Ok(true)
    }
}
