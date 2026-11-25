use std::collections::VecDeque;
use std::ops::Range;

use super::{Window, WindowId, WordOwnership};
use crate::meeting::{Channel, Offset, SAMPLE_RATE, Span};
use crate::transcription::{MAX_SPEECH_FRAMES, StreamingVad};

const PADDING: u64 = SAMPLE_RATE as u64 * 3 / 10;
const LOOKBACK: usize = MAX_SPEECH_FRAMES + SAMPLE_RATE as usize * 2;

pub(super) struct WindowBuilder<'a> {
    vad: &'a mut StreamingVad,
    channel: Channel,
    next_id: u64,
    origin: Offset,
    received: u64,
    audio: VecDeque<f32>,
    pending: VecDeque<Range<u64>>,
    awaiting_neighbor: Option<Window>,
}

impl<'a> WindowBuilder<'a> {
    pub(super) fn new(vad: &'a mut StreamingVad, channel: Channel) -> Self {
        Self {
            vad,
            channel,
            next_id: 0,
            origin: Offset(0),
            received: 0,
            audio: VecDeque::with_capacity(LOOKBACK),
            pending: VecDeque::new(),
            awaiting_neighbor: None,
        }
    }

    pub(super) fn push(
        &mut self,
        samples: &[f32],
        mut emit: impl FnMut(Window) -> Result<(), String>,
    ) -> Result<(), String> {
        self.audio.extend(samples);
        self.received += samples.len() as u64;
        self.vad
            .accept(samples, |range| self.pending.push_back(range));
        self.emit_ready(false, &mut emit)?;
        let excess = self.audio.len().saturating_sub(LOOKBACK);
        self.audio.drain(..excess);
        Ok(())
    }

    pub(super) fn end_at(
        &mut self,
        next_origin: Offset,
        mut emit: impl FnMut(Window) -> Result<(), String>,
    ) -> Result<(), String> {
        self.vad.finish(|range| self.pending.push_back(range));
        self.emit_ready(true, &mut emit)?;
        if let Some(last) = self.awaiting_neighbor.take() {
            emit(last)?;
        }
        self.origin = next_origin;
        self.received = 0;
        self.audio.clear();
        Ok(())
    }

    fn emit_ready(
        &mut self,
        ending: bool,
        emit: &mut impl FnMut(Window) -> Result<(), String>,
    ) -> Result<(), String> {
        let retained_from = self.received - self.audio.len() as u64;
        while self.pending.front().is_some_and(|core| {
            ending || core.end.min(core.start + MAX_SPEECH_FRAMES as u64) + PADDING <= self.received
        }) {
            let Some(core) = self.pending.front_mut() else {
                break;
            };
            let first = core.start;
            let end = core.end.min(first + MAX_SPEECH_FRAMES as u64);
            core.start = end;
            if end == core.end {
                self.pending.pop_front();
            }
            let padded_first = first.saturating_sub(PADDING);
            let padded_end = (end + PADDING).min(self.received);
            if padded_first < retained_from {
                return Err("Silero emitted a segment outside the meeting audio buffer".into());
            }
            let samples = self
                .audio
                .range(
                    (padded_first - retained_from) as usize..(padded_end - retained_from) as usize,
                )
                .copied()
                .collect();
            let mut next = Window {
                id: WindowId {
                    channel: self.channel,
                    sequence: self.next_id,
                },
                first: Offset(self.origin.0 + padded_first),
                core: Span {
                    first: Offset(self.origin.0 + first),
                    end: Offset(self.origin.0 + end),
                },
                ownership: WordOwnership::default(),
                samples,
            };
            if let Some(mut earlier) = self.awaiting_neighbor.take() {
                Window::divide_padding(&mut earlier, &mut next);
                emit(earlier)?;
            }
            self.awaiting_neighbor = Some(next);
            self.next_id += 1;
        }
        Ok(())
    }
}
