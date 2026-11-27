use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use super::SourceEpoch;

const RING_SECONDS: usize = 8;
const MAX_STAMPS: usize = 8_192;
const INGRESS_CLOSED: u64 = 1 << 63;
/// A track that delivered nothing for this long reads as silent.
const LEVEL_STALE: Duration = Duration::from_millis(250);
const NEVER: u64 = u64::MAX;

#[derive(Clone, Copy)]
pub(super) struct Stamp {
    pub(super) frames_pushed_total: u64,
    pub(super) at: Instant,
    pub(super) dropped: u64,
    pub(super) epoch: SourceEpoch,
}

/// How loud one meeting track is now, for the pill. The capture thread stores
/// the RMS of each block it hands over; a track that stops delivering reads as
/// silence within [`LEVEL_STALE`].
#[derive(Clone)]
pub(crate) struct TrackLevel(Arc<LevelCell>);

struct LevelCell {
    zero: Instant,
    rms: AtomicU32,
    at_us: AtomicU64,
}

impl TrackLevel {
    pub(crate) fn new() -> Self {
        Self(Arc::new(LevelCell {
            zero: Instant::now(),
            rms: AtomicU32::new(0),
            at_us: AtomicU64::new(NEVER),
        }))
    }

    fn record(&self, samples: &[f32], at: Instant) {
        if samples.is_empty() {
            return;
        }
        let energy = samples.iter().map(|sample| sample * sample).sum::<f32>();
        let rms = (energy / samples.len() as f32).sqrt();
        let since = at.saturating_duration_since(self.0.zero).as_micros();
        self.0.rms.store(rms.to_bits(), Ordering::Relaxed);
        self.0
            .at_us
            .store(u64::try_from(since).unwrap_or(NEVER - 1), Ordering::Release);
    }

    /// The latest block's RMS, or 0 when nothing arrived within `LEVEL_STALE`.
    pub(crate) fn rms(&self, now: Instant) -> f32 {
        let at_us = self.0.at_us.load(Ordering::Acquire);
        if at_us == NEVER {
            return 0.0;
        }
        let at = self.0.zero + Duration::from_micros(at_us);
        if now.saturating_duration_since(at) > LEVEL_STALE {
            0.0
        } else {
            f32::from_bits(self.0.rms.load(Ordering::Relaxed))
        }
    }
}

impl Default for TrackLevel {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for TrackLevel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for TrackLevel {}

impl std::fmt::Debug for TrackLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackLevel").finish_non_exhaustive()
    }
}

pub(crate) struct TrackIngress {
    pcm: HeapProd<f32>,
    stamps: HeapProd<Stamp>,
    epoch: SourceEpoch,
    published_frames: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    level: TrackLevel,
}

impl TrackIngress {
    pub(super) fn pair(sample_rate: u32, epoch: SourceEpoch) -> (Self, TrackDrain) {
        let frames = sample_rate as usize * RING_SECONDS;
        let (pcm, pcm_rx) = HeapRb::new(frames).split();
        let (stamps, stamp_rx) = HeapRb::new(frames.min(MAX_STAMPS)).split();
        let published_frames = Arc::new(AtomicU64::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        (
            Self {
                pcm,
                stamps,
                epoch,
                published_frames: published_frames.clone(),
                dropped: dropped.clone(),
                level: TrackLevel::new(),
            },
            TrackDrain {
                pcm: pcm_rx,
                stamps: stamp_rx,
                frames_popped_total: 0,
                published_frames,
                dropped,
            },
        )
    }

    pub(crate) fn begin_epoch(&mut self, epoch: SourceEpoch) {
        self.epoch = epoch;
    }

    /// The live level of what this ingress receives.
    pub(crate) fn level(&self) -> TrackLevel {
        self.level.clone()
    }

    pub(crate) fn push(&mut self, samples: &[f32], at: Instant) -> bool {
        let published = self.published_frames.load(Ordering::Relaxed);
        if published & INGRESS_CLOSED != 0 {
            return false;
        }
        if samples.is_empty() {
            return true;
        }
        self.level.record(samples, at);
        if self.stamps.is_full() || self.pcm.vacant_len() < samples.len() {
            self.dropped
                .fetch_add(samples.len() as u64, Ordering::Relaxed);
            return false;
        }

        let frames_pushed_total = published + self.pcm.push_slice(samples) as u64;
        let stamp = Stamp {
            frames_pushed_total,
            at,
            dropped: self.dropped_frames(),
            epoch: self.epoch,
        };
        // A block becomes visible to the drain only if it wins the race with close.
        self.stamps.try_push(stamp).is_ok()
            && self
                .published_frames
                .compare_exchange(
                    published,
                    frames_pushed_total,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_ok()
    }

    pub(crate) fn dropped_frames(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

pub(super) struct TrackDrain {
    pcm: HeapCons<f32>,
    stamps: HeapCons<Stamp>,
    frames_popped_total: u64,
    published_frames: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
}

impl TrackDrain {
    pub(super) fn pop(&mut self, samples: &mut Vec<f32>) -> Option<Stamp> {
        let published = self.published_frames.load(Ordering::Acquire) & !INGRESS_CLOSED;
        if self.frames_popped_total == published {
            return None;
        }
        let stamp = self.stamps.try_pop()?;
        let len = (stamp.frames_pushed_total - self.frames_popped_total) as usize;
        samples.resize(len, 0.0);
        let popped = self.pcm.pop_slice(samples);
        debug_assert_eq!(popped, len);
        self.frames_popped_total = stamp.frames_pushed_total;
        Some(stamp)
    }

    pub(super) fn close(&self) {
        self.published_frames
            .fetch_or(INGRESS_CLOSED, Ordering::AcqRel);
    }

    pub(super) fn dropped_frames(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl Drop for TrackDrain {
    fn drop(&mut self) {
        self.close();
    }
}
