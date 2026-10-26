use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use sherpa_onnx::{LinearResampler, VoiceActivityDetector};

use super::{
    DIRECT_DECODE_LIMIT, MODEL_SAMPLE_RATE, ParakeetTranscriber, Transcriber, Transcript,
    VAD_WINDOW_SIZE,
};
use crate::audio::AudioRecording;

pub struct BackgroundSession {
    completed: Receiver<Result<Option<String>, String>>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for BackgroundSession {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

pub struct TranscriptionWorker {
    commands: Sender<WorkerCommand>,
    worker: Option<JoinHandle<()>>,
}

enum WorkerCommand {
    Record {
        sample_rate: i32,
        audio_chunks: Receiver<Vec<f32>>,
        cancelled: Arc<AtomicBool>,
        completed: Sender<Result<Option<String>, String>>,
    },
    Transcribe {
        audio: AudioRecording,
        completed: Sender<Result<Transcript, String>>,
    },
    Quit,
}

impl TranscriptionWorker {
    pub fn start(transcriber: ParakeetTranscriber) -> Result<Self, String> {
        let (commands, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("koett-transcription".to_string())
            .spawn(move || run_transcription_worker(transcriber, receiver))
            .map_err(|error| format!("could not start transcription: {error}"))?;
        Ok(Self {
            commands,
            worker: Some(worker),
        })
    }

    pub fn record(
        &self,
        sample_rate: i32,
        audio_chunks: Receiver<Vec<f32>>,
    ) -> Result<BackgroundSession, String> {
        if sample_rate < 1 {
            return Err("audio sample rate must be positive".to_string());
        }
        let (completed, result) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.commands
            .send(WorkerCommand::Record {
                sample_rate,
                audio_chunks,
                cancelled: cancelled.clone(),
                completed,
            })
            .map_err(|_| "the transcription worker stopped unexpectedly".to_string())?;
        Ok(BackgroundSession {
            completed: result,
            cancelled,
        })
    }

    pub fn finish(
        &self,
        session: BackgroundSession,
        audio: AudioRecording,
    ) -> Result<Transcript, String> {
        let started = Instant::now();
        match session
            .completed
            .recv()
            .map_err(|_| "background transcription stopped unexpectedly".to_string())?
        {
            Ok(Some(text)) => Ok(Transcript {
                text,
                audio_duration: audio.duration(),
                transcription: started.elapsed(),
            }),
            Ok(None) => self.transcribe(audio),
            Err(error) => {
                eprintln!("background_transcription_fallback={error}");
                self.transcribe(audio)
            }
        }
    }

    fn transcribe(&self, audio: AudioRecording) -> Result<Transcript, String> {
        let (completed, result) = mpsc::channel();
        self.commands
            .send(WorkerCommand::Transcribe { audio, completed })
            .map_err(|_| "the transcription worker stopped unexpectedly".to_string())?;
        result
            .recv()
            .map_err(|_| "transcription stopped unexpectedly".to_string())?
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.commands.send(WorkerCommand::Quit);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for TranscriptionWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run_transcription_worker(
    mut transcriber: ParakeetTranscriber,
    commands: Receiver<WorkerCommand>,
) {
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Record {
                sample_rate,
                audio_chunks,
                cancelled,
                completed,
            } => {
                let result =
                    transcriber.transcribe_while_recording(sample_rate, audio_chunks, &cancelled);
                let _ = completed.send(result);
            }
            WorkerCommand::Transcribe { audio, completed } => {
                let result = transcriber.transcribe(&audio);
                let _ = completed.send(result);
            }
            WorkerCommand::Quit => break,
        }
    }
}

impl ParakeetTranscriber {
    pub(super) fn transcribe_while_recording(
        &self,
        input_sample_rate: i32,
        audio_chunks: Receiver<Vec<f32>>,
        cancelled: &AtomicBool,
    ) -> Result<Option<String>, String> {
        let Some(vad) = &self.vad else {
            for _ in audio_chunks {}
            return Ok(None);
        };

        let mut input_samples = 0_usize;
        let mut initial_audio = Vec::new();
        let mut resampler = None;
        let mut model_samples = Vec::new();
        let mut model_offset = 0_usize;
        let mut parts = Vec::new();
        let mut is_long = false;

        for chunk in audio_chunks {
            if cancelled.load(Ordering::Acquire) {
                return Err("background transcription was cancelled".to_string());
            }
            input_samples = input_samples.saturating_add(chunk.len());
            if !is_long {
                initial_audio.extend(chunk);
                if input_samples as f64 / input_sample_rate as f64
                    <= DIRECT_DECODE_LIMIT.as_secs_f64()
                {
                    continue;
                }
                is_long = true;
                vad.reset();
                if input_sample_rate != MODEL_SAMPLE_RATE {
                    resampler = Some(
                        LinearResampler::create(input_sample_rate, MODEL_SAMPLE_RATE).ok_or_else(
                            || "sherpa-onnx could not create the live audio resampler".to_string(),
                        )?,
                    );
                }
                if let Some(resampler) = &resampler {
                    model_samples.extend(resampler.resample(&initial_audio, false));
                    initial_audio.clear();
                } else {
                    model_samples.append(&mut initial_audio);
                }
            } else if let Some(resampler) = &resampler {
                model_samples.extend(resampler.resample(&chunk, false));
            } else {
                model_samples.extend(chunk);
            }
            accept_vad_windows(vad, &mut model_samples, &mut model_offset);
            self.decode_ready_vad_segments(&mut parts)?;
        }

        if cancelled.load(Ordering::Acquire) {
            return Err("background transcription was cancelled".to_string());
        }
        if !is_long {
            return Ok(None);
        }
        if let Some(resampler) = &resampler {
            model_samples.extend(resampler.resample(&[], true));
        }
        accept_vad_windows(vad, &mut model_samples, &mut model_offset);
        let remainder = &model_samples[model_offset..];
        if !remainder.is_empty() {
            let mut final_window = [0.0_f32; VAD_WINDOW_SIZE];
            final_window[..remainder.len()].copy_from_slice(remainder);
            vad.accept_waveform(&final_window);
        }
        vad.flush();
        self.decode_ready_vad_segments(&mut parts)?;
        Ok(Some(parts.join(" ")))
    }

    fn decode_ready_vad_segments(&self, parts: &mut Vec<String>) -> Result<(), String> {
        let vad = self
            .vad
            .as_ref()
            .ok_or_else(|| "voice detection is unavailable".to_string())?;
        while let Some(segment) = vad.front() {
            vad.pop();
            let (text, _) = self.decode_bounded(segment.samples())?;
            if !text.is_empty() {
                parts.push(text);
            }
        }
        Ok(())
    }
}

fn accept_vad_windows(vad: &VoiceActivityDetector, samples: &mut Vec<f32>, offset: &mut usize) {
    while *offset + VAD_WINDOW_SIZE <= samples.len() {
        vad.accept_waveform(&samples[*offset..*offset + VAD_WINDOW_SIZE]);
        *offset += VAD_WINDOW_SIZE;
    }
    if *offset >= VAD_WINDOW_SIZE * 16 {
        samples.drain(..*offset);
        *offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;

    use super::BackgroundSession;

    #[test]
    fn dropping_a_session_cancels_background_work() {
        let (_, completed) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let session = BackgroundSession {
            completed,
            cancelled: cancelled.clone(),
        };

        drop(session);

        assert!(cancelled.load(Ordering::Acquire));
    }
}
