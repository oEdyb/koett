use std::{ops::Range, path::Path};

use sherpa_onnx::VoiceActivityDetector;

use super::{MAX_DECODE_SAMPLES, MODEL_SAMPLE_RATE, ParakeetTranscriber, VAD_WINDOW_SIZE};
use crate::annotation::TimedWord;

pub(crate) const MAX_SPEECH_FRAMES: usize = MAX_DECODE_SAMPLES;
const DETECTOR_BUFFER_SECONDS: f32 = 24.0;
const REBASE_FRAMES: u64 = MODEL_SAMPLE_RATE as u64 * 3600;

pub(crate) struct StreamingVad {
    vad: VoiceActivityDetector,
    tail: Vec<f32>,
    received: u64,
    origin: u64,
    active_since: Option<u64>,
}

impl StreamingVad {
    pub(crate) fn load(directory: &Path) -> Result<Self, String> {
        let vad = super::load_vad_with_buffer(directory, DETECTOR_BUFFER_SECONDS)?
            .ok_or_else(|| "Silero voice detection model is missing".to_string())?;
        Ok(Self {
            vad,
            tail: Vec::with_capacity(VAD_WINDOW_SIZE),
            received: 0,
            origin: 0,
            active_since: None,
        })
    }

    pub(crate) fn accept(&mut self, mut samples: &[f32], mut emit: impl FnMut(Range<u64>)) {
        while !samples.is_empty() {
            let count = samples.len().min(VAD_WINDOW_SIZE - self.tail.len());
            self.tail.extend_from_slice(&samples[..count]);
            self.received += count as u64;
            samples = &samples[count..];
            if self.tail.len() < VAD_WINDOW_SIZE {
                break;
            }
            self.vad.accept_waveform(&self.tail);
            self.tail.clear();
            if self.vad.detected() {
                let since = *self.active_since.get_or_insert(self.received);
                // Sherpa raises its silence threshold at 20 seconds but can keep detecting speech.
                if self.received - since >= MAX_SPEECH_FRAMES as u64 {
                    self.vad.flush();
                    self.active_since = None;
                }
            } else {
                self.active_since = None;
            }
            self.drain(&mut emit);
            if !self.vad.detected() && self.received - self.origin >= REBASE_FRAMES {
                self.vad.reset();
                self.origin = self.received;
            }
        }
    }

    pub(crate) fn finish(&mut self, mut emit: impl FnMut(Range<u64>)) {
        if !self.tail.is_empty() {
            self.tail.resize(VAD_WINDOW_SIZE, 0.0);
            self.vad.accept_waveform(&self.tail);
        }
        self.vad.flush();
        self.drain(&mut emit);
        self.vad.reset();
        self.tail.clear();
        self.received = 0;
        self.origin = 0;
        self.active_since = None;
    }

    fn drain(&self, emit: &mut impl FnMut(Range<u64>)) {
        while let Some(segment) = self.vad.front() {
            self.vad.pop();
            let first = self.origin + u64::try_from(segment.start()).unwrap_or(0);
            let end = (first + segment.samples().len() as u64).min(self.received);
            if first < end {
                emit(first..end);
            }
        }
    }
}

impl ParakeetTranscriber {
    pub(crate) fn decode_window(&self, samples: &[f32]) -> Result<Vec<TimedWord>, String> {
        const MAX_WINDOW_FRAMES: usize = MAX_SPEECH_FRAMES + MODEL_SAMPLE_RATE as usize * 6 / 10;
        if samples.is_empty() || samples.len() > MAX_WINDOW_FRAMES {
            return Err("Invalid meeting decode window length".into());
        }
        self.decode_timed_text(MODEL_SAMPLE_RATE, samples, 0.0)
            .map(|(_, words)| words)
    }
}
