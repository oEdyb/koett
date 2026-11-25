use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::super::{Offset, SAMPLE_RATE};
use super::GAP_THRESHOLD;

const WINDOW: u64 = SAMPLE_RATE as u64 * 2;
const MAX_STEER: i64 = 2;

pub(super) fn session_offset(zero: Instant, at: Instant) -> Offset {
    let nanos = at.saturating_duration_since(zero).as_nanos();
    Offset((nanos * u128::from(SAMPLE_RATE) / Duration::from_secs(1).as_nanos()) as u64)
}

pub(super) struct TrackClock {
    anchor: Offset,
    minimums: VecDeque<(Offset, i64)>,
    lag: i64,
    correction: i64,
    written: u64,
}

impl TrackClock {
    pub(super) fn new(anchor: Offset, stamp: Offset) -> Self {
        Self {
            anchor,
            minimums: VecDeque::from([(stamp, 0)]),
            lag: 0,
            correction: 0,
            written: 0,
        }
    }

    pub(super) fn observe(&mut self, stamp: Offset, nominal_frames: u64) -> Option<Offset> {
        let nominal = self.anchor.0 + nominal_frames;
        let lag = (stamp.0 as i64).saturating_sub(nominal as i64);
        while self
            .minimums
            .front()
            .is_some_and(|&(at, _)| stamp.0.saturating_sub(at.0) > WINDOW)
        {
            self.minimums.pop_front();
        }
        while self.minimums.back().is_some_and(|&(_, old)| old >= lag) {
            self.minimums.pop_back();
        }
        self.minimums.push_back((stamp, lag));
        let (_, minimum) = self.minimums[0];
        if minimum.saturating_sub(self.lag) > GAP_THRESHOLD as i64 {
            return Some(Offset(nominal.saturating_add_signed(minimum)));
        }
        self.lag = minimum;
        None
    }

    pub(super) fn place(&mut self, frames: usize) -> Offset {
        if self.written != 0 {
            self.correction += (self.lag - self.correction).clamp(-MAX_STEER, MAX_STEER);
        }
        let first = (self.anchor.0 + self.written).saturating_add_signed(self.correction);
        self.written += frames as u64;
        Offset(first)
    }
}
