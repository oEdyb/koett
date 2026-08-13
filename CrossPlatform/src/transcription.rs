use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

use crate::audio::AudioRecording;

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

        Ok(Self {
            recognizer,
            model_load: started.elapsed(),
        })
    }

    pub fn model_load(&self) -> Duration {
        self.model_load
    }
}

impl Transcriber for ParakeetTranscriber {
    fn transcribe(&mut self, audio: &AudioRecording) -> Result<Transcript, String> {
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

    use super::Transcript;

    #[test]
    fn realtime_factor_uses_transcription_over_audio() {
        let transcript = Transcript {
            text: String::new(),
            audio_duration: Duration::from_secs(10),
            transcription: Duration::from_secs(1),
        };

        assert_eq!(transcript.realtime_factor(), 0.1);
    }
}
