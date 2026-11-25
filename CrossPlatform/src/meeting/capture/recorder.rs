use std::io::{self, Write};
use std::time::Instant;

use sherpa_onnx::LinearResampler;

use super::super::{Offset, SAMPLE_RATE, Span};
use super::clock::{TrackClock, session_offset};
use super::ingress::Stamp;
use super::journal::{GapCause, JournalWriter};
use super::{PCM_FRAMES, SourceEpoch};

struct Segment {
    epoch: SourceEpoch,
    clock: TrackClock,
    input_frames: u64,
}

pub(super) struct TrackRecorder<W> {
    journal: JournalWriter<W>,
    session_zero: Instant,
    sample_rate: u32,
    resampler: Option<LinearResampler>,
    segment: Option<Segment>,
    pending: Vec<f32>,
    end: Offset,
    dropped: u64,
}

impl<W: Write> TrackRecorder<W> {
    pub(super) fn new(
        journal: JournalWriter<W>,
        session_zero: Instant,
        sample_rate: u32,
    ) -> io::Result<Self> {
        let rate = i32::try_from(sample_rate)
            .ok()
            .filter(|rate| *rate > 0 && rate.checked_mul(8).is_some())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid capture sample rate")
            })?;
        let resampler = if sample_rate == SAMPLE_RATE {
            None
        } else {
            Some(
                LinearResampler::create(rate, SAMPLE_RATE as i32).ok_or_else(|| {
                    io::Error::other("Could not create the meeting audio resampler")
                })?,
            )
        };
        Ok(Self {
            journal,
            session_zero,
            sample_rate,
            resampler,
            segment: None,
            pending: Vec::with_capacity(PCM_FRAMES),
            end: Offset(0),
            dropped: 0,
        })
    }

    pub(super) fn push(&mut self, stamp: Stamp, samples: &[f32]) -> io::Result<()> {
        let at = session_offset(self.session_zero, stamp.at);
        let lost = stamp.dropped - self.dropped;
        let missing = self.lost_frames(lost);
        let rate = self.sample_rate;
        match &mut self.segment {
            None => {
                self.end = Offset(at.0.saturating_sub(missing));
                self.write_gap(at, GapCause::Overrun)?;
                self.start_segment(at, at, stamp.epoch, true)?;
            }
            Some(segment) if segment.epoch != stamp.epoch => {
                self.flush_segment()?;
                let resume = at.max(Offset(self.end.0 + missing));
                self.write_gap(resume, GapCause::SourceRestart)?;
                self.start_segment(resume, at, stamp.epoch, true)?;
            }
            Some(_) if lost > 0 => {
                self.flush_segment()?;
                let resume = Offset(self.end.0 + missing);
                self.write_gap(resume, GapCause::Overrun)?;
                self.start_segment(resume, at, stamp.epoch, false)?;
            }
            Some(segment) => {
                let nominal_frames = scale_frames(segment.input_frames, rate);
                if let Some(resume) = segment.clock.observe(at, nominal_frames) {
                    self.flush_segment()?;
                    self.write_gap(resume, GapCause::ClockJump)?;
                    self.start_segment(self.end, at, stamp.epoch, false)?;
                }
            }
        }
        self.dropped = stamp.dropped;
        let Some(segment) = &mut self.segment else {
            return Err(io::Error::other("Missing meeting audio segment"));
        };
        segment.input_frames += samples.len() as u64;
        if let Some(resampler) = &self.resampler {
            self.append(&resampler.resample(samples, false))
        } else {
            self.append(samples)
        }
    }

    pub(super) fn finish(&mut self, dropped: u64) -> io::Result<()> {
        self.flush_segment()?;
        let missing = self.lost_frames(dropped.saturating_sub(self.dropped));
        self.write_gap(Offset(self.end.0 + missing), GapCause::Overrun)
    }

    pub(super) fn sink(&self) -> &W {
        self.journal.sink()
    }

    fn start_segment(
        &mut self,
        anchor: Offset,
        at: Offset,
        epoch: SourceEpoch,
        write_epoch: bool,
    ) -> io::Result<()> {
        if write_epoch {
            self.journal.epoch(anchor, epoch)?;
        }
        self.segment = Some(Segment {
            epoch,
            clock: TrackClock::new(anchor, at),
            input_frames: 0,
        });
        Ok(())
    }

    fn append(&mut self, mut samples: &[f32]) -> io::Result<()> {
        while !samples.is_empty() {
            let take = samples.len().min(PCM_FRAMES - self.pending.len());
            let (head, rest) = samples.split_at(take);
            self.pending.extend_from_slice(head);
            samples = rest;
            if self.pending.len() == PCM_FRAMES {
                self.write_pending()?;
            }
        }
        Ok(())
    }

    fn flush_segment(&mut self) -> io::Result<()> {
        if self.segment.is_some()
            && let Some(resampler) = &self.resampler
        {
            let tail = resampler.resample(&[], true);
            resampler.reset();
            self.append(&tail)?;
        }
        self.write_pending()
    }

    fn write_pending(&mut self) -> io::Result<()> {
        if !self.pending.is_empty()
            && let Some(segment) = &mut self.segment
        {
            let first = segment.clock.place(self.pending.len());
            self.journal.pcm(first, &self.pending)?;
            self.end = Offset(first.0 + self.pending.len() as u64);
            self.pending.clear();
        }
        Ok(())
    }

    fn write_gap(&mut self, end: Offset, cause: GapCause) -> io::Result<()> {
        if self.end < end {
            self.journal.gap(
                Span {
                    first: self.end,
                    end,
                },
                cause,
            )?;
            self.end = end;
        }
        Ok(())
    }

    fn lost_frames(&self, dropped: u64) -> u64 {
        if dropped == 0 {
            0
        } else {
            scale_frames(dropped, self.sample_rate).max(1)
        }
    }
}

fn scale_frames(input_frames: u64, sample_rate: u32) -> u64 {
    (u128::from(input_frames) * u128::from(SAMPLE_RATE) / u128::from(sample_rate)) as u64
}
