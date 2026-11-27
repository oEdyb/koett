mod clock;
pub(crate) mod disk;
mod ingress;
pub(crate) mod journal;
mod recorder;
pub(crate) mod writer;

#[cfg(test)]
mod tests;

use std::io;

use super::{Channel, SAMPLE_RATE};

pub(crate) use ingress::{TrackIngress, TrackLevel};

const PCM_FRAMES: usize = SAMPLE_RATE as usize / 2;
const GAP_THRESHOLD: u64 = SAMPLE_RATE as u64 * 300 / 1000;

pub(crate) trait CapturePlatform {
    type Source;

    fn arm(&mut self, channel: Channel, ingress: TrackIngress) -> io::Result<Self::Source>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Timing {
    Callback,
    Arrival,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourceEpoch {
    pub(crate) generation: u64,
    pub(crate) timing: Timing,
}
