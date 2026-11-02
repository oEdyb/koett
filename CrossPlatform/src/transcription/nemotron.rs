use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig};

use super::{path_string, required_file};

pub struct NemotronTranscriber {
    recognizer: OnlineRecognizer,
}

impl NemotronTranscriber {
    pub fn load(directory: &Path) -> Result<Self, String> {
        let mut config = OnlineRecognizerConfig::default();
        config.model_config.tokens = Some(path_string(&required_file(directory, "tokens.txt")?)?);
        config.model_config.transducer.encoder = Some(path_string(&required_file(
            directory,
            "encoder.int8.onnx",
        )?)?);
        config.model_config.transducer.decoder = Some(path_string(&required_file(
            directory,
            "decoder.int8.onnx",
        )?)?);
        config.model_config.transducer.joiner =
            Some(path_string(&required_file(directory, "joiner.int8.onnx")?)?);
        config.model_config.num_threads = 2;
        config.model_config.provider = Some("cpu".into());
        config.enable_endpoint = false;
        let recognizer =
            OnlineRecognizer::create(&config).ok_or("Could not load Nemotron streaming 560 ms")?;
        Ok(Self { recognizer })
    }

    pub(super) fn record(
        &self,
        sample_rate: i32,
        chunks: Receiver<Vec<f32>>,
        cancelled: &AtomicBool,
    ) -> Result<Option<String>, String> {
        let stream = self.recognizer.create_stream();
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("Streaming cancelled".into());
            }
            match chunks.recv_timeout(Duration::from_millis(25)) {
                Ok(samples) => {
                    stream.accept_waveform(sample_rate, &samples);
                    while self.recognizer.is_ready(&stream) {
                        if cancelled.load(Ordering::Acquire) {
                            return Err("Streaming cancelled".into());
                        }
                        self.recognizer.decode(&stream);
                    }
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        stream.accept_waveform(sample_rate, &vec![0.0; sample_rate as usize * 3 / 10]);
        stream.input_finished();
        while self.recognizer.is_ready(&stream) {
            if cancelled.load(Ordering::Acquire) {
                return Err("Streaming cancelled".into());
            }
            self.recognizer.decode(&stream);
        }
        self.recognizer
            .get_result(&stream)
            .map(|result| Some(result.text))
            .ok_or_else(|| "Nemotron returned no final result".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioRecording;
    use std::sync::mpsc;
    use std::time::Instant;

    #[test]
    #[ignore = "requires verified public Nemotron evaluation model"]
    fn public_stream_reset_probe() {
        let directory = std::path::PathBuf::from(std::env::var("KOETT_NEMOTRON_EVAL").unwrap());
        let started = Instant::now();
        let engine = NemotronTranscriber::load(&directory).unwrap();
        eprintln!("nemotron_load_ms={}", started.elapsed().as_millis());
        let audio = AudioRecording::read_wav(&directory.join("test_wavs/0.wav")).unwrap();
        let mut previous = None;
        for iteration in 0..3 {
            let (sender, receiver) = mpsc::channel();
            for chunk in audio.samples.chunks(3200) {
                sender.send(chunk.to_vec()).unwrap();
            }
            drop(sender);
            let started = Instant::now();
            let text = engine
                .record(audio.sample_rate, receiver, &AtomicBool::new(false))
                .unwrap()
                .unwrap();
            eprintln!(
                "iteration={iteration} decode_ms={} text={text}",
                started.elapsed().as_millis()
            );
            assert!(!text.is_empty());
            if let Some(previous) = previous.as_ref() {
                assert_eq!(&text, previous);
            }
            previous = Some(text);
        }
        let (sender, receiver) = mpsc::channel();
        for _ in 0..8 {
            for chunk in audio.samples.chunks(3200) {
                sender.send(chunk.to_vec()).unwrap();
            }
        }
        drop(sender);
        let started = Instant::now();
        let long = engine
            .record(audio.sample_rate, receiver, &AtomicBool::new(false))
            .unwrap()
            .unwrap();
        eprintln!(
            "long_decode_ms={} text={long}",
            started.elapsed().as_millis()
        );
        assert!(long.to_lowercase().matches("yellow lamps").count() >= 7);
        let (sender, receiver) = mpsc::channel();
        sender.send(audio.samples[..1600].to_vec()).unwrap();
        drop(sender);
        assert!(
            engine
                .record(audio.sample_rate, receiver, &AtomicBool::new(false))
                .is_ok()
        );
        let (_sender, receiver) = mpsc::channel();
        assert!(
            engine
                .record(audio.sample_rate, receiver, &AtomicBool::new(true))
                .is_err()
        );
        let (sender, receiver) = mpsc::channel();
        sender.send(vec![0.0; 16000]).unwrap();
        drop(sender);
        assert!(
            engine
                .record(16000, receiver, &AtomicBool::new(false))
                .unwrap()
                .unwrap()
                .is_empty()
        );
    }
}
