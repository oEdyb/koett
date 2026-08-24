use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sherpa_onnx::{
    LinearResampler, OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig,
    VadModelConfig, VoiceActivityDetector,
};

use crate::audio::AudioRecording;
use crate::model::VAD_FILE;

const MODEL_SAMPLE_RATE: i32 = 16_000;
const DIRECT_DECODE_LIMIT: Duration = Duration::from_secs(20);
const VAD_WINDOW_SIZE: usize = 512;
const MAX_DECODE_SAMPLES: usize = MODEL_SAMPLE_RATE as usize * 20;
const DECODE_OVERLAP_SAMPLES: usize = MODEL_SAMPLE_RATE as usize * 2;
const MAX_OVERLAP_WORDS: usize = 20;

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
    vad: Option<VoiceActivityDetector>,
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
        let vad_path = model_directory.join(VAD_FILE);
        let vad = if vad_path.is_file() {
            let mut vad_config = VadModelConfig::default();
            vad_config.silero_vad.model = Some(path_string(&vad_path)?);
            vad_config.silero_vad.threshold = 0.5;
            vad_config.silero_vad.min_silence_duration = 0.5;
            vad_config.silero_vad.min_speech_duration = 0.25;
            vad_config.silero_vad.max_speech_duration = 20.0;
            vad_config.silero_vad.window_size = VAD_WINDOW_SIZE as i32;
            vad_config.sample_rate = MODEL_SAMPLE_RATE;
            vad_config.num_threads = 1;
            vad_config.provider = Some("cpu".to_string());
            Some(
                VoiceActivityDetector::create(&vad_config, 60.0)
                    .ok_or_else(|| "sherpa-onnx could not load voice detection".to_string())?,
            )
        } else {
            None
        };

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
    duration > DIRECT_DECODE_LIMIT
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

        let mut parts = Vec::new();
        let mut vad_segments = 0;
        let mut decode_chunks = 0;
        if let Some(vad) = &self.vad {
            vad.reset();
            let mut chunks = samples.chunks_exact(VAD_WINDOW_SIZE);
            for chunk in &mut chunks {
                vad.accept_waveform(chunk);
            }
            let remainder = chunks.remainder();
            if !remainder.is_empty() {
                let mut final_window = [0.0_f32; VAD_WINDOW_SIZE];
                final_window[..remainder.len()].copy_from_slice(remainder);
                vad.accept_waveform(&final_window);
            }
            vad.flush();

            while let Some(segment) = vad.front() {
                vad.pop();
                vad_segments += 1;
                let (text, chunks) = self.decode_bounded(segment.samples())?;
                decode_chunks += chunks;
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        } else {
            let (text, chunks) = self.decode_bounded(samples)?;
            decode_chunks = chunks;
            if !text.is_empty() {
                parts.push(text);
            }
        }
        eprintln!(
            "transcription_vad={} transcription_segments={vad_segments} transcription_decode_chunks={decode_chunks}",
            self.vad.is_some()
        );

        Ok(Transcript {
            text: parts.join(" "),
            audio_duration,
            transcription: started.elapsed(),
        })
    }

    fn decode_bounded(&self, samples: &[f32]) -> Result<(String, usize), String> {
        let mut text = String::new();
        let ranges = bounded_decode_ranges(samples.len());
        for range in &ranges {
            let stream = self.recognizer.create_stream();
            stream.accept_waveform(MODEL_SAMPLE_RATE, &samples[range.clone()]);
            self.recognizer.decode(&stream);
            let result = stream
                .get_result()
                .ok_or_else(|| "sherpa-onnx returned no transcript".to_string())?;
            merge_overlapping_text(&mut text, &result.text);
        }
        Ok((text, ranges.len()))
    }
}

fn bounded_decode_ranges(sample_count: usize) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < sample_count {
        let end = (start + MAX_DECODE_SAMPLES).min(sample_count);
        ranges.push(start..end);
        if end == sample_count {
            break;
        }
        start = end - DECODE_OVERLAP_SAMPLES;
    }
    ranges
}

fn merge_overlapping_text(existing: &mut String, next: &str) {
    let next = next.trim();
    if next.is_empty() {
        return;
    }
    if existing.is_empty() {
        existing.push_str(next);
        return;
    }

    let existing_words = existing.split_whitespace().collect::<Vec<_>>();
    let next_words = next.split_whitespace().collect::<Vec<_>>();
    if let Some((existing_overlap, next_overlap)) =
        overlapping_word_counts(&existing_words, &next_words)
    {
        let mut merged = existing_words[..existing_words.len() - existing_overlap]
            .iter()
            .map(|word| (*word).to_string())
            .collect::<Vec<_>>();
        merged.extend(shortest_common_words(
            &existing_words[existing_words.len() - existing_overlap..],
            &next_words[..next_overlap],
        ));
        merged.extend(
            next_words[next_overlap..]
                .iter()
                .map(|word| (*word).to_string()),
        );
        *existing = merged.join(" ");
    } else {
        existing.push(' ');
        existing.push_str(next);
    }
}

fn overlapping_word_counts(existing: &[&str], next: &[&str]) -> Option<(usize, usize)> {
    let existing = existing
        .iter()
        .rev()
        .take(MAX_OVERLAP_WORDS)
        .rev()
        .map(|word| normalize_word(word))
        .collect::<Vec<_>>();
    let next = next
        .iter()
        .take(MAX_OVERLAP_WORDS)
        .map(|word| normalize_word(word))
        .collect::<Vec<_>>();
    let mut best = None;
    for existing_count in 2..=existing.len() {
        for next_count in 2..=next.len() {
            let distance = word_distance(
                &existing[existing.len() - existing_count..],
                &next[..next_count],
            );
            let compared = existing_count.max(next_count);
            if distance * 3 > compared {
                continue;
            }
            let common = compared - distance;
            let score = common as isize * 10
                - distance as isize * 3
                - existing_count.abs_diff(next_count) as isize;
            let candidate = (
                score,
                common,
                usize::MAX - distance,
                existing_count,
                next_count,
            );
            if best.is_none_or(|current| candidate > current) {
                best = Some(candidate);
            }
        }
    }
    best.map(|(_, _, _, existing_count, next_count)| (existing_count, next_count))
}

fn shortest_common_words(left: &[&str], right: &[&str]) -> Vec<String> {
    let left_normalized = left
        .iter()
        .map(|word| normalize_word(word))
        .collect::<Vec<_>>();
    let right_normalized = right
        .iter()
        .map(|word| normalize_word(word))
        .collect::<Vec<_>>();
    let mut shared = vec![vec![0_usize; right.len() + 1]; left.len() + 1];
    for left_index in (0..left.len()).rev() {
        for right_index in (0..right.len()).rev() {
            shared[left_index][right_index] =
                if left_normalized[left_index] == right_normalized[right_index] {
                    shared[left_index + 1][right_index + 1] + 1
                } else {
                    shared[left_index + 1][right_index].max(shared[left_index][right_index + 1])
                };
        }
    }

    let mut merged = Vec::new();
    let (mut left_index, mut right_index) = (0, 0);
    while left_index < left.len() && right_index < right.len() {
        if left_normalized[left_index] == right_normalized[right_index] {
            merged.push(left[left_index].to_string());
            left_index += 1;
            right_index += 1;
        } else if shared[left_index + 1][right_index] >= shared[left_index][right_index + 1] {
            merged.push(left[left_index].to_string());
            left_index += 1;
        } else {
            merged.push(right[right_index].to_string());
            right_index += 1;
        }
    }
    merged.extend(left[left_index..].iter().map(|word| (*word).to_string()));
    merged.extend(right[right_index..].iter().map(|word| (*word).to_string()));
    merged
}

fn word_distance(left: &[String], right: &[String]) -> usize {
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    for (left_index, left_word) in left.iter().enumerate() {
        let mut current = vec![left_index + 1; right.len() + 1];
        for (right_index, right_word) in right.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_word != right_word);
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            current[right_index + 1] = substitution.min(insertion).min(deletion);
        }
        previous = current;
    }
    previous[right.len()]
}

fn normalize_word(word: &str) -> String {
    word.chars()
        .filter(|character| character.is_alphanumeric() || *character == '\'')
        .flat_map(char::to_lowercase)
        .collect()
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

    use super::{
        DECODE_OVERLAP_SAMPLES, MAX_DECODE_SAMPLES, Transcript, bounded_decode_ranges,
        merge_overlapping_text, needs_segmentation,
    };

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
        assert!(!needs_segmentation(Duration::from_secs(20)));
        assert!(needs_segmentation(Duration::from_secs(21)));
    }

    #[test]
    fn decoder_chunks_have_a_hard_limit_and_overlap() {
        let ranges = bounded_decode_ranges(MAX_DECODE_SAMPLES * 2);

        assert!(ranges.iter().all(|range| range.len() <= MAX_DECODE_SAMPLES));
        assert_eq!(ranges[0], 0..MAX_DECODE_SAMPLES);
        assert_eq!(ranges[1].start, ranges[0].end - DECODE_OVERLAP_SAMPLES);
        assert_eq!(ranges.last().unwrap().end, MAX_DECODE_SAMPLES * 2);
    }

    #[test]
    fn overlapping_text_is_not_duplicated() {
        let mut text = "One quiet sentence, with three shared words.".to_string();

        merge_overlapping_text(&mut text, "three shared words. Then the next thought.");

        assert_eq!(
            text,
            "One quiet sentence, with three shared words. Then the next thought."
        );
    }

    #[test]
    fn overlapping_text_tolerates_a_small_recognition_change() {
        let mut text = "I don't wish to see it anymore, observed Phoebe, turning away.".to_string();

        merge_overlapping_text(
            &mut text,
            "I don't wish to see it any more, observed Phoebe, turning away. The next thought.",
        );

        assert_eq!(
            text,
            "I don't wish to see it anymore, any more, observed Phoebe, turning away. The next thought."
        );
    }

    #[test]
    fn uncertain_overlap_preserves_inserted_words() {
        let mut text = "alpha beta gamma".to_string();

        merge_overlapping_text(&mut text, "alpha beta important gamma new");

        assert_eq!(text, "alpha beta important gamma new");
    }

    #[test]
    fn unmatched_text_is_never_dropped() {
        let mut text = "First chunk.".to_string();

        merge_overlapping_text(&mut text, "Different second chunk.");

        assert_eq!(text, "First chunk. Different second chunk.");
    }
}
