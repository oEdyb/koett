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
    active: std::sync::Mutex<Option<Arc<AtomicBool>>>,
}

enum WorkerCommand {
    Record {
        offline: bool,
        sample_rate: i32,
        audio_chunks: Receiver<Vec<f32>>,
        cancelled: Arc<AtomicBool>,
        completed: Sender<Result<Option<String>, String>>,
    },
    Transcribe {
        audio: Arc<AudioRecording>,
        completed: Sender<Result<Transcript, String>>,
    },
    Media {
        path: std::path::PathBuf,
        cancelled: Arc<AtomicBool>,
        completed: Sender<Result<String, String>>,
    },
    Quit,
}

impl TranscriptionWorker {
    pub fn start(transcriber: ParakeetTranscriber) -> Result<Self, String> {
        Self::start_with_nemotron(transcriber, None)
    }

    pub fn start_with_nemotron(
        transcriber: ParakeetTranscriber,
        nemotron: Option<super::NemotronTranscriber>,
    ) -> Result<Self, String> {
        let (commands, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("koett-transcription".to_string())
            .spawn(move || run_transcription_worker(transcriber, nemotron, receiver))
            .map_err(|error| format!("could not start transcription: {error}"))?;
        Ok(Self {
            commands,
            worker: Some(worker),
            active: std::sync::Mutex::new(None),
        })
    }

    pub fn record(
        &self,
        sample_rate: i32,
        audio_chunks: Receiver<Vec<f32>>,
    ) -> Result<BackgroundSession, String> {
        self.record_mode(sample_rate, audio_chunks, false)
    }

    pub fn record_offline(
        &self,
        sample_rate: i32,
        audio_chunks: Receiver<Vec<f32>>,
    ) -> Result<BackgroundSession, String> {
        self.record_mode(sample_rate, audio_chunks, true)
    }

    fn record_mode(
        &self,
        sample_rate: i32,
        audio_chunks: Receiver<Vec<f32>>,
        offline: bool,
    ) -> Result<BackgroundSession, String> {
        if sample_rate < 1 {
            return Err("audio sample rate must be positive".to_string());
        }
        let (completed, result) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        *self
            .active
            .lock()
            .map_err(|_| "transcription cancellation unavailable")? = Some(cancelled.clone());
        self.commands
            .send(WorkerCommand::Record {
                offline,
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
            .unwrap_or_else(|_| Err("background transcription stopped unexpectedly".to_string()))
        {
            Ok(Some(text)) => Ok(Transcript {
                raw: None,
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

    pub fn transcribe(&self, audio: AudioRecording) -> Result<Transcript, String> {
        let audio = Arc::new(audio);
        let result = (|| {
            let (completed, result) = mpsc::channel();
            self.commands
                .send(WorkerCommand::Transcribe {
                    audio: audio.clone(),
                    completed,
                })
                .map_err(|_| "the transcription worker stopped unexpectedly".to_string())?;
            result
                .recv()
                .map_err(|_| "transcription stopped unexpectedly".to_string())?
        })();
        result.map_err(|error| match super::recovery::retain(&audio) {
            Ok(path) => format!("{error}. Recording saved: {}", path.display()),
            Err(retention) => format!("{error}. Audio recovery failed: {retention}"),
        })
    }

    pub fn media_request(
        &self,
        path: std::path::PathBuf,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Receiver<Result<String, String>>, String> {
        let (completed, reply) = mpsc::channel();
        self.commands
            .send(WorkerCommand::Media {
                path,
                cancelled,
                completed,
            })
            .map_err(|_| "transcription worker stopped".to_string())?;
        Ok(reply)
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if let Ok(active) = self.active.lock()
            && let Some(active) = active.as_ref()
        {
            active.store(true, Ordering::Release);
        }
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
    nemotron: Option<super::NemotronTranscriber>,
    commands: Receiver<WorkerCommand>,
) {
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Record {
                offline,
                sample_rate,
                audio_chunks,
                cancelled,
                completed,
            } => {
                let result = match nemotron.as_ref().filter(|_| !offline) {
                    Some(engine) => engine.record(sample_rate, audio_chunks, &cancelled),
                    None => transcriber.transcribe_while_recording(
                        sample_rate,
                        audio_chunks,
                        &cancelled,
                    ),
                };
                let _ = completed.send(result);
            }
            WorkerCommand::Transcribe { audio, completed } => {
                let result = transcriber.transcribe(&audio);
                let _ = completed.send(result);
            }
            WorkerCommand::Media {
                path,
                cancelled,
                completed,
            } => {
                let result = transcribe_media_pcm(&mut transcriber, &path, &cancelled);
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
            while !cancelled.load(Ordering::Acquire) {
                match audio_chunks.recv_timeout(std::time::Duration::from_millis(25)) {
                    Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            return Ok(None);
        };

        let mut input_samples = 0_usize;
        let mut initial_audio = Vec::new();
        let mut resampler = None;
        let mut model_samples = Vec::new();
        let mut model_offset = 0_usize;
        let mut parts = Vec::new();
        let mut is_long = false;

        loop {
            let chunk = match audio_chunks.recv_timeout(std::time::Duration::from_millis(25)) {
                Ok(chunk) => chunk,
                Err(mpsc::RecvTimeoutError::Timeout) if !cancelled.load(Ordering::Acquire) => {
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            };
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

fn transcribe_media_pcm(
    transcriber: &mut ParakeetTranscriber,
    path: &std::path::Path,
    cancelled: &AtomicBool,
) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if size == 0 || size > 16000 * 2 * 10800 || size % 2 != 0 {
        return Err("invalid or oversized media audio".into());
    }
    let mut bytes = vec![0; 16000 * 2 * 30];
    let mut parts = Vec::new();
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err("Media cancelled".into());
        }
        let mut count = 0;
        while count < bytes.len() {
            let n = file.read(&mut bytes[count..]).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            count += n;
        }
        if count == 0 {
            break;
        }
        let samples = bytes[..count]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
            .collect();
        parts.push(
            transcriber
                .transcribe(&AudioRecording::new(16000, samples)?)?
                .text,
        );
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("Media cancelled".into());
    }
    let text = parts.join(" ");
    if text.trim().is_empty() {
        return Err("No speech found in media".into());
    }
    Ok(text)
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

#[cfg(test)]
mod nemotron_recovery_tests {
    use super::*;

    #[test]
    #[ignore = "requires public v2 model and isolated recovery directory"]
    fn stopped_backup_preserves_failed_recording() {
        let data = std::env::var("XDG_DATA_HOME").unwrap();
        assert!(data.starts_with("/tmp/koett-nemotron-"));
        let directory = std::path::PathBuf::from(std::env::var("KOETT_PARAKEET_EVAL").unwrap());
        let mut worker =
            TranscriptionWorker::start(ParakeetTranscriber::load(&directory, 2).unwrap()).unwrap();
        worker.shutdown();
        let (sender, completed) = mpsc::channel();
        sender
            .send(Err("injected finalization error".into()))
            .unwrap();
        let session = BackgroundSession {
            completed,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let audio = AudioRecording::read_wav(&directory.join("test_wavs/0.wav")).unwrap();
        let error = worker.finish(session, audio).err().unwrap();
        assert!(error.contains("Recording saved:"), "{error}");
        let path = error.split("Recording saved: ").nth(1).unwrap();
        assert!(
            !AudioRecording::read_wav(std::path::Path::new(path))
                .unwrap()
                .samples
                .is_empty()
        );
    }

    #[test]
    #[ignore = "requires public Parakeet v2 evaluation model"]
    fn public_stream_failure_uses_warm_parakeet_once() {
        let directory = std::path::PathBuf::from(std::env::var("KOETT_PARAKEET_EVAL").unwrap());
        let audio = AudioRecording::read_wav(&directory.join("test_wavs/0.wav")).unwrap();
        let mut transcriber = ParakeetTranscriber::load(&directory, 2).unwrap();
        let expected = transcriber.transcribe(&audio).unwrap().text;
        let worker = TranscriptionWorker::start(transcriber).unwrap();
        for error in ["injected stream error", "injected finalization error"] {
            let (sender, completed) = mpsc::channel();
            sender.send(Err(error.to_string())).unwrap();
            let session = BackgroundSession {
                completed,
                cancelled: Arc::new(AtomicBool::new(false)),
            };
            let audio = AudioRecording::read_wav(&directory.join("test_wavs/0.wav")).unwrap();
            assert_eq!(worker.finish(session, audio).unwrap().text, expected);
        }
        let (sender, receiver) = mpsc::channel();
        let session = worker.record(16000, receiver).unwrap();
        let started = Instant::now();
        worker.stop();
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        drop(session);
        drop(sender);
    }
}
