use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sherpa_onnx::{
    LinearResampler, OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig,
    VadModelConfig, VoiceActivityDetector,
};

use crate::audio::AudioRecording;
use crate::model::VAD_FILE;

const MODEL_SAMPLE_RATE: i32 = 16_000;
const LONG_RECORDING_THRESHOLD: Duration = Duration::from_secs(30);
const VAD_WINDOW_SIZE: usize = 512;

pub struct Transcript {
    pub text: String,
    pub audio_duration: Duration,
    pub transcription: Duration,
}

impl Transcript {
    pub fn realtime_factor(&self) -> f64 {
        self.transcription.as_secs_f64() / self.audio_duration.as_secs_f64()
    }
}

pub trait Transcriber {
    fn transcribe(&mut self, audio: &AudioRecording) -> Result<Transcript, String>;
}

pub struct ParakeetTranscriber {
    recognizer: OfflineRecognizer,
    vad: VoiceActivityDetector,
    model_load: Duration,
}

impl ParakeetTranscriber {
    pub fn load(model_directory: &Path, threads: i32) -> Result<Self, String> {
        let tokens = required_file(model_directory, "tokens.txt")?;

        let mut config = OfflineRecognizerConfig::default();
        config.model_config.tokens = Some(path_string(&tokens)?);
        config.model_config.num_threads = threads;
        config.model_config.provider = Some("cpu".to_string());

        let ctc_model = model_directory.join("model.int8.onnx");
        if ctc_model.is_file() {
            config.model_config.nemo_ctc.model = Some(path_string(&ctc_model)?);
        } else {
            config.model_config.transducer = OfflineTransducerModelConfig {
                encoder: Some(path_string(&required_file(
                    model_directory,
                    "encoder.int8.onnx",
                )?)?),
                decoder: Some(path_string(&required_file(
                    model_directory,
                    "decoder.int8.onnx",
                )?)?),
                joiner: Some(path_string(&required_file(
                    model_directory,
                    "joiner.int8.onnx",
                )?)?),
            };
            config.model_config.model_type = Some("nemo_transducer".to_string());
        }

        let started = Instant::now();
        let recognizer = OfflineRecognizer::create(&config)
            .ok_or_else(|| "sherpa-onnx could not load the model".to_string())?;
        let mut vad_config = VadModelConfig::default();
        vad_config.silero_vad.model =
            Some(path_string(&required_file(model_directory, VAD_FILE)?)?);
        vad_config.silero_vad.threshold = 0.25;
        vad_config.silero_vad.min_silence_duration = 0.5;
        vad_config.silero_vad.min_speech_duration = 0.5;
        vad_config.silero_vad.max_speech_duration = 10.0;
        vad_config.silero_vad.window_size = VAD_WINDOW_SIZE as i32;
        vad_config.sample_rate = MODEL_SAMPLE_RATE;
        vad_config.num_threads = 1;
        vad_config.provider = Some("cpu".to_string());
        let vad = VoiceActivityDetector::create(&vad_config, 60.0)
            .ok_or_else(|| "sherpa-onnx could not load voice detection".to_string())?;

        Ok(Self {
            recognizer,
            vad,
            model_load: started.elapsed(),
        })
    }

    pub fn model_load(&self) -> Duration {
        self.model_load
    }
}

impl Transcriber for ParakeetTranscriber {
    fn transcribe(&mut self, audio: &AudioRecording) -> Result<Transcript, String> {
        if needs_segmentation(audio.duration()) {
            self.transcribe_long(audio)
        } else {
            self.transcribe_short(audio)
        }
    }
}

fn needs_segmentation(duration: Duration) -> bool {
    duration > LONG_RECORDING_THRESHOLD
}

impl ParakeetTranscriber {
    fn transcribe_short(&self, audio: &AudioRecording) -> Result<Transcript, String> {
        let audio_duration = audio.duration();
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(audio.sample_rate, &audio.samples);

        let started = Instant::now();
        self.recognizer.decode(&stream);
        let transcription = started.elapsed();
        let result = stream
            .get_result()
            .ok_or_else(|| "sherpa-onnx returned no transcript".to_string())?;

        Ok(Transcript {
            text: result.text,
            audio_duration,
            transcription,
        })
    }

    fn transcribe_long(&self, audio: &AudioRecording) -> Result<Transcript, String> {
        let audio_duration = audio.duration();
        let started = Instant::now();
        let resampled;
        let samples = if audio.sample_rate == MODEL_SAMPLE_RATE {
            audio.samples.as_slice()
        } else {
            let resampler = LinearResampler::create(audio.sample_rate, MODEL_SAMPLE_RATE)
                .ok_or_else(|| "sherpa-onnx could not create the audio resampler".to_string())?;
            resampled = resampler.resample(&audio.samples, true);
            resampled.as_slice()
        };

        self.vad.reset();
        let mut chunks = samples.chunks_exact(VAD_WINDOW_SIZE);
        for chunk in &mut chunks {
            self.vad.accept_waveform(chunk);
        }
        let remainder = chunks.remainder();
        if !remainder.is_empty() {
            let mut final_window = [0.0_f32; VAD_WINDOW_SIZE];
            final_window[..remainder.len()].copy_from_slice(remainder);
            self.vad.accept_waveform(&final_window);
        }
        self.vad.flush();

        let mut parts = Vec::new();
        while let Some(segment) = self.vad.front() {
            self.vad.pop();
            let stream = self.recognizer.create_stream();
            stream.accept_waveform(MODEL_SAMPLE_RATE, segment.samples());
            self.recognizer.decode(&stream);
            let result = stream
                .get_result()
                .ok_or_else(|| "sherpa-onnx returned no transcript".to_string())?;
            let text = result.text.trim();
            if !text.is_empty() {
                parts.push(text.to_string());
            }
        }
        eprintln!("transcription_segments={}", parts.len());

        Ok(Transcript {
            text: parts.join(" "),
            audio_duration,
            transcription: started.elapsed(),
        })
    }
}

fn required_file(directory: &Path, name: &str) -> Result<PathBuf, String> {
    let path = directory.join(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("missing {}", path.display()))
    }
}

fn path_string(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Transcript, needs_segmentation};

    #[test]
    fn realtime_factor_uses_transcription_over_audio() {
        let transcript = Transcript {
            text: String::new(),
            audio_duration: Duration::from_secs(10),
            transcription: Duration::from_secs(1),
        };

        assert_eq!(transcript.realtime_factor(), 0.1);
    }

    #[test]
    fn only_long_recordings_use_segmentation() {
        assert!(!needs_segmentation(Duration::from_secs(30)));
        assert!(needs_segmentation(Duration::from_secs(31)));
    }
}
