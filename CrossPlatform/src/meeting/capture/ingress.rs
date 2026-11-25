use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use super::SourceEpoch;

const RING_SECONDS: usize = 8;
const MAX_STAMPS: usize = 8_192;

#[derive(Clone, Copy)]
pub(super) struct Stamp {
    pub(super) frames_pushed_total: u64,
    pub(super) at: Instant,
    pub(super) dropped: u64,
    pub(super) epoch: SourceEpoch,
}

pub(crate) struct TrackIngress {
    pcm: HeapProd<f32>,
    stamps: HeapProd<Stamp>,
    epoch: SourceEpoch,
    frames_pushed_total: u64,
    dropped: Arc<AtomicU64>,
}

impl TrackIngress {
    pub(super) fn pair(sample_rate: u32, epoch: SourceEpoch) -> (Self, TrackDrain) {
        let frames = sample_rate as usize * RING_SECONDS;
        let (pcm, pcm_rx) = HeapRb::new(frames).split();
        let (stamps, stamp_rx) = HeapRb::new(frames.min(MAX_STAMPS)).split();
        let dropped = Arc::new(AtomicU64::new(0));
        (
            Self {
                pcm,
                stamps,
                epoch,
                frames_pushed_total: 0,
                dropped: dropped.clone(),
            },
            TrackDrain {
                pcm: pcm_rx,
                stamps: stamp_rx,
                frames_popped_total: 0,
                dropped,
            },
        )
    }

    pub(crate) fn begin_epoch(&mut self, epoch: SourceEpoch) {
        self.epoch = epoch;
    }

    pub(crate) fn push(&mut self, samples: &[f32], at: Instant) -> bool {
        if !self.stamps.read_is_held() {
            return false;
        }
        if samples.is_empty() {
            return true;
        }
        if self.stamps.is_full() || self.pcm.vacant_len() < samples.len() {
            self.dropped
                .fetch_add(samples.len() as u64, Ordering::Relaxed);
            return false;
        }

        // The stamp publishes the block only after all its PCM is visible.
        self.frames_pushed_total += self.pcm.push_slice(samples) as u64;
        let stamp = Stamp {
            frames_pushed_total: self.frames_pushed_total,
            at,
            dropped: self.dropped_frames(),
            epoch: self.epoch,
        };
        self.stamps.try_push(stamp).is_ok()
    }

    pub(crate) fn dropped_frames(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

pub(super) struct TrackDrain {
    pcm: HeapCons<f32>,
    stamps: HeapCons<Stamp>,
    frames_popped_total: u64,
    dropped: Arc<AtomicU64>,
}

impl TrackDrain {
    pub(super) fn pop(&mut self, samples: &mut Vec<f32>) -> Option<Stamp> {
        let stamp = self.stamps.try_pop()?;
        let len = (stamp.frames_pushed_total - self.frames_popped_total) as usize;
        samples.resize(len, 0.0);
        let popped = self.pcm.pop_slice(samples);
        debug_assert_eq!(popped, len);
        self.frames_popped_total = stamp.frames_pushed_total;
        Some(stamp)
    }

    pub(super) fn pending_blocks(&self) -> usize {
        self.stamps.occupied_len()
    }

    pub(super) fn dropped_frames(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}
