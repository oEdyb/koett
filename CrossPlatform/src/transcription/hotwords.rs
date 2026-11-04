//! Bias Parakeet transducers toward dictionary terms.
//!
//! Each decode runs two streams in one batch: a plain one and one with the
//! terms as hotwords. Only the biased stream's dictionary words are kept (see
//! `dictionary::adopt_terms`), because the boost also changes casing of other
//! words. An empty dictionary keeps the original greedy decoder. CTC retains
//! its greedy primary stream and may use the bounded transducer sidecar.
//! Settings follow the pinned sherpa-onnx Parakeet TDT hotword example.

use std::fs;
use std::path::{Path, PathBuf};

use sherpa_onnx::OfflineRecognizerConfig;

use super::ParakeetTranscriber;
use crate::dictionary::{self, DictionaryEntry};

const HOTWORD_SCORE: f32 = 2.0;
const ACTIVE_PATHS: i32 = 4;

pub(super) struct Bias {
    hotwords: String,
    dictionary: Vec<DictionaryEntry>,
    vocabulary: PathBuf,
}

impl Bias {
    /// `None` when nothing can be biased: no usable term, or a CTC model.
    pub(super) fn new(
        model_directory: &Path,
        dictionary: &[DictionaryEntry],
    ) -> Result<Option<Self>, String> {
        let hotwords = hotwords(dictionary);
        if hotwords.is_empty() || model_directory.join("model.int8.onnx").is_file() {
            return Ok(None);
        }
        Ok(Some(Self {
            hotwords,
            dictionary: dictionary.to_vec(),
            vocabulary: write_vocabulary(model_directory)?,
        }))
    }

    pub(super) fn configure(&self, config: &mut OfflineRecognizerConfig) -> Result<(), String> {
        config.model_config.bpe_vocab = Some(super::path_string(&self.vocabulary)?);
        config.model_config.modeling_unit = Some("bpe".into());
        config.decoding_method = Some("modified_beam_search".into());
        config.max_active_paths = ACTIVE_PATHS;
        config.hotwords_score = HOTWORD_SCORE;
        Ok(())
    }
}

impl ParakeetTranscriber {
    /// Swap the dictionary; reloads the decoder only when biasing turns on or off.
    pub fn set_dictionary(&mut self, dictionary: &[DictionaryEntry]) -> Result<(), String> {
        let bias = Bias::new(&self.model_directory, dictionary)?;
        if bias.is_some() != self.bias.is_some() {
            self.recognizer = Self::recognizer(&self.model_directory, self.threads, bias.as_ref())?;
        }
        self.bias = bias;
        if self.model_directory.join("model.int8.onnx").is_file() {
            self.ctc_bias = super::ctc_bias::Sidecar::new(dictionary);
        }
        Ok(())
    }

    /// Text plus word times of the plain stream, `offset` seconds into the
    /// recording. Dictionary adoption may change some words of the text.
    pub(super) fn decode_timed_text(
        &self,
        sample_rate: i32,
        samples: &[f32],
        offset: f32,
    ) -> Result<(String, Vec<crate::annotation::TimedWord>), String> {
        let duration = samples.len() as f32 / sample_rate.max(1) as f32;
        let plain = self.recognizer.create_stream();
        plain.accept_waveform(sample_rate, samples);
        let biased = self.bias.as_ref().map(|bias| {
            let stream = self.recognizer.create_stream_with_hotwords(&bias.hotwords);
            stream.accept_waveform(sample_rate, samples);
            (bias, stream)
        });
        match &biased {
            None => self.recognizer.decode(&plain),
            Some((_, stream)) => self.recognizer.decode_multiple_streams(&[&plain, stream]),
        }
        let result = plain
            .get_result()
            .ok_or_else(|| "sherpa-onnx returned no transcript".to_string())?;
        let words = super::timed_words(
            &result.text,
            &result.tokens,
            result.timestamps.as_deref(),
            offset,
            duration,
        );
        let text = match &biased {
            None => result.text,
            Some((bias, stream)) => {
                dictionary::adopt_terms(&result.text, &text(stream)?, &bias.dictionary)
            }
        };
        let text = match &self.ctc_bias {
            Some(bias) => bias.refine(&text, sample_rate, samples),
            None => text,
        };
        Ok((text, words))
    }
}

fn text(stream: &sherpa_onnx::OfflineStream) -> Result<String, String> {
    stream
        .get_result()
        .map(|result| result.text)
        .ok_or_else(|| "sherpa-onnx returned no transcript".to_string())
}

/// sherpa-onnx separates per-stream hotwords with "/" and reads ":" as a score.
pub(super) fn hotwords(dictionary: &[DictionaryEntry]) -> String {
    dictionary
        .iter()
        .map(|entry| entry.term.trim())
        .filter(|term| !term.is_empty() && !term.contains(['/', ':']))
        .filter(|term| !term.chars().any(char::is_control))
        .collect::<Vec<_>>()
        .join("/")
}

/// Parakeet ships no bpe.vocab. Equal scores over tokens.txt give the
/// longest-match encoding the upstream example uses. It lives in Koett's cache,
/// never in the verified model directory.
fn write_vocabulary(model_directory: &Path) -> Result<PathBuf, String> {
    let tokens = fs::read_to_string(model_directory.join("tokens.txt"))
        .map_err(|error| format!("could not read the model tokens: {error}"))?;
    let vocabulary = vocabulary(&tokens);
    let name = model_directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("model");
    let path = crate::paths::hotword_vocabulary(name)?;
    if fs::read_to_string(&path).is_ok_and(|existing| existing == vocabulary) {
        return Ok(path);
    }
    let parent = path.parent().ok_or("hotword vocabulary has no folder")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let partial = path.with_extension(format!("{}.part", std::process::id()));
    fs::write(&partial, vocabulary)
        .and_then(|()| fs::rename(&partial, &path))
        .map_err(|error| format!("could not write the hotword vocabulary: {error}"))?;
    Ok(path)
}

fn vocabulary(tokens: &str) -> String {
    tokens
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|piece| format!("{piece}\t-1.0\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotwords_skip_decoder_syntax_and_join_terms() {
        let dictionary = [
            DictionaryEntry::new(" Koett ", &["cot"]),
            DictionaryEntry::new("Olle Dyberg", &[]),
            DictionaryEntry::new("C/C++", &[]),
            DictionaryEntry::new("ratio:2", &[]),
        ];
        assert_eq!(hotwords(&dictionary), "Koett/Olle Dyberg");
    }

    #[test]
    fn vocabulary_scores_every_token_equally() {
        assert_eq!(
            vocabulary("<unk> 0\n▁t 1\n\n<blk> 2\n"),
            "<unk>\t-1.0\n▁t\t-1.0\n<blk>\t-1.0\n"
        );
    }

    #[test]
    fn ctc_models_and_empty_dictionaries_are_not_biased() {
        let directory = std::env::temp_dir().join(format!("koett-ctc-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("model.int8.onnx"), b"").unwrap();
        let dictionary = [DictionaryEntry::new("Koett", &[])];
        assert!(Bias::new(&directory, &dictionary).unwrap().is_none());
        assert!(Bias::new(&directory, &[]).unwrap().is_none());
        fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use crate::audio::AudioRecording;
    use crate::transcription::Transcriber;

    #[test]
    #[ignore = "requires KOETT_MODEL_DIR pointing at Parakeet TDT v2"]
    fn dictionary_turns_on_and_off_without_reloading_the_transcriber() {
        let directory = PathBuf::from(std::env::var("KOETT_MODEL_DIR").unwrap());
        let audio = AudioRecording::read_wav(&directory.join("test_wavs/0.wav")).unwrap();
        let mut transcriber = ParakeetTranscriber::load(&directory, 2).unwrap();
        let plain = transcriber.transcribe(&audio).unwrap().text;
        assert!(!plain.contains("Phoebe"), "{plain}");

        transcriber
            .set_dictionary(&[DictionaryEntry::new("Phoebe", &[])])
            .unwrap();
        let biased = transcriber.transcribe(&audio).unwrap().text;
        assert!(biased.contains("Phoebe"), "{biased}");

        transcriber.set_dictionary(&[]).unwrap();
        assert_eq!(transcriber.transcribe(&audio).unwrap().text, plain);
    }
}
