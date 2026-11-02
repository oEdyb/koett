use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use sherpa_onnx::{
    LinearResampler, OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig,
    VadModelConfig, VoiceActivityDetector,
};

use crate::annotation::{TimedText, TimedWord, estimate_words, words_from_tokens};
use crate::audio::AudioRecording;
use crate::model::VAD_FILE;

mod background;
mod hotwords;
mod nemotron;
mod recovery;
pub use nemotron::NemotronTranscriber;

pub use background::{BackgroundSession, TranscriptionWorker};

static DECODED: AtomicU32 = AtomicU32::new(0);
static PLANNED: AtomicU32 = AtomicU32::new(0);

/// Decode units (VAD segments, or one whole short recording) finished and
/// planned for the dictation being finalized. `planned` stays 0 until the
/// final count is known, so callers never show an invented fraction.
pub fn decode_progress() -> (u32, u32) {
    (
        DECODED.load(Ordering::Acquire),
        PLANNED.load(Ordering::Acquire),
    )
}

fn progress_reset() {
    PLANNED.store(0, Ordering::Release);
    DECODED.store(0, Ordering::Release);
}

fn progress_plan(remaining: usize) {
    let remaining = u32::try_from(remaining).unwrap_or(u32::MAX);
    PLANNED.store(
        DECODED.load(Ordering::Acquire).saturating_add(remaining),
        Ordering::Release,
    );
}

fn progress_step() {
    DECODED.fetch_add(1, Ordering::AcqRel);
}

const MODEL_SAMPLE_RATE: i32 = 16_000;
const DIRECT_DECODE_LIMIT: Duration = Duration::from_secs(20);
const VAD_WINDOW_SIZE: usize = 512;
const MAX_DECODE_SAMPLES: usize = MODEL_SAMPLE_RATE as usize * 20;
const DECODE_OVERLAP_SAMPLES: usize = MODEL_SAMPLE_RATE as usize * 2;
const MAX_OVERLAP_WORDS: usize = 20;

pub struct Transcript {
    pub raw: Option<String>,
    pub text: String,
    /// Recognizer word times for `text` before replacements or cleanup.
    pub words: Vec<TimedWord>,
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
    model_directory: PathBuf,
    threads: i32,
    bias: Option<hotwords::Bias>,
}

impl ParakeetTranscriber {
    pub fn load(model_directory: &Path, threads: i32) -> Result<Self, String> {
        Self::load_with_dictionary(model_directory, threads, &[])
    }

    /// Load with Olle's dictionary. Transducer models bias decoding toward its terms.
    pub fn load_with_dictionary(
        model_directory: &Path,
        threads: i32,
        dictionary: &[crate::dictionary::DictionaryEntry],
    ) -> Result<Self, String> {
        let bias = hotwords::Bias::new(model_directory, dictionary)?;
        let started = Instant::now();
        let recognizer = Self::recognizer(model_directory, threads, bias.as_ref())?;
        let vad = load_vad(model_directory)?;
        Ok(Self {
            recognizer,
            vad,
            model_load: started.elapsed(),
            model_directory: model_directory.to_path_buf(),
            threads,
            bias,
        })
    }

    fn recognizer(
        model_directory: &Path,
        threads: i32,
        bias: Option<&hotwords::Bias>,
    ) -> Result<OfflineRecognizer, String> {
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
        if let Some(bias) = bias {
            bias.configure(&mut config)?;
        }
        OfflineRecognizer::create(&config)
            .ok_or_else(|| "sherpa-onnx could not load the model".to_string())
    }
}

fn load_vad(model_directory: &Path) -> Result<Option<VoiceActivityDetector>, String> {
    let vad_path = model_directory.join(VAD_FILE);
    Ok(if vad_path.is_file() {
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
    })
}

impl ParakeetTranscriber {
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
        progress_reset();
        progress_plan(1);
        let started = Instant::now();
        let (text, words) = self.decode_timed_text(audio.sample_rate, &audio.samples, 0.0)?;
        let transcription = started.elapsed();
        progress_step();

        Ok(Transcript {
            raw: None,
            text,
            words,
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

        let mut parts = TimedText::default();
        let mut vad_segments = 0;
        let mut decode_chunks = 0;
        if let Some(vad) = &self.vad {
            vad.reset();
            let (chunks, remainder) = samples.as_chunks::<VAD_WINDOW_SIZE>();
            for chunk in chunks {
                vad.accept_waveform(chunk);
            }
            if !remainder.is_empty() {
                let mut final_window = [0.0_f32; VAD_WINDOW_SIZE];
                final_window[..remainder.len()].copy_from_slice(remainder);
                vad.accept_waveform(&final_window);
            }
            vad.flush();

            let mut segments = Vec::new();
            while let Some(segment) = vad.front() {
                vad.pop();
                segments.push(segment);
            }
            progress_reset();
            progress_plan(segments.len());
            for segment in segments {
                vad_segments += 1;
                let offset = segment.start() as f32 / MODEL_SAMPLE_RATE as f32;
                let (text, words, chunks) = self.decode_timed(segment.samples(), offset)?;
                progress_step();
                decode_chunks += chunks;
                parts.push(&text, words);
            }
        } else {
            progress_reset();
            progress_plan(1);
            let (text, words, chunks) = self.decode_timed(samples, 0.0)?;
            progress_step();
            decode_chunks = chunks;
            parts.push(&text, words);
        }
        eprintln!(
            "transcription_vad={} transcription_segments={vad_segments} transcription_decode_chunks={decode_chunks}",
            self.vad.is_some()
        );

        Ok(Transcript {
            raw: None,
            text: parts.text,
            words: parts.words,
            audio_duration,
            transcription: started.elapsed(),
        })
    }

    #[cfg(test)]
    fn decode_bounded(&self, samples: &[f32]) -> Result<(String, usize), String> {
        self.decode_timed(samples, 0.0)
            .map(|(text, _, chunks)| (text, chunks))
    }

    /// Decodes model-rate samples starting `offset` seconds into the recording.
    fn decode_timed(
        &self,
        samples: &[f32],
        offset: f32,
    ) -> Result<(String, Vec<TimedWord>, usize), String> {
        let mut text = String::new();
        let mut words = None;
        let ranges = bounded_decode_ranges(samples.len());
        let duration = samples.len() as f32 / MODEL_SAMPLE_RATE as f32;
        for range in &ranges {
            let (decoded, timed) =
                self.decode_timed_text(MODEL_SAMPLE_RATE, &samples[range.clone()], offset)?;
            if ranges.len() == 1 {
                words = Some(timed);
            }
            merge_overlapping_text(&mut text, &decoded);
        }
        // Overlap merging drops tokens, so merged chunks get estimated times.
        // A single chunk keeps the recognizer's words; dictionary adoption can
        // change them, which the sketch linker's alignment absorbs.
        let words = words.unwrap_or_else(|| estimate_words(&text, offset, offset + duration));
        Ok((text, words, ranges.len()))
    }
}

/// Measured token times when they reproduce `text`, else an even estimate.
pub(crate) fn timed_words(
    text: &str,
    tokens: &[String],
    timestamps: Option<&[f32]>,
    offset: f32,
    duration: f32,
) -> Vec<TimedWord> {
    words_from_tokens(
        text,
        tokens,
        timestamps.unwrap_or_default(),
        offset,
        duration,
    )
    .unwrap_or_else(|| estimate_words(text, offset, offset + duration))
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
    use std::env;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::{
        DECODE_OVERLAP_SAMPLES, MAX_DECODE_SAMPLES, MODEL_SAMPLE_RATE, ParakeetTranscriber,
        Transcriber, Transcript, TranscriptionWorker, VAD_WINDOW_SIZE, bounded_decode_ranges,
        merge_overlapping_text, needs_segmentation,
    };

    #[test]
    fn decode_progress_counts_live_segments_and_plans_only_the_final_rest() {
        super::progress_reset();
        assert_eq!(super::decode_progress(), (0, 0));
        // Two segments decoded while still recording: total not known yet.
        super::progress_step();
        super::progress_step();
        assert_eq!(super::decode_progress(), (2, 0));
        // Stop: three closed segments remain.
        super::progress_plan(3);
        assert_eq!(super::decode_progress(), (2, 5));
        for _ in 0..3 {
            super::progress_step();
        }
        assert_eq!(super::decode_progress(), (5, 5));
        super::progress_reset();
        assert_eq!(super::decode_progress(), (0, 0));
    }
    use crate::audio::AudioRecording;

    #[test]
    #[ignore = "requires public Parakeet v2 evaluation model"]
    fn public_transcripts_carry_measured_word_times() {
        let directory = PathBuf::from(env::var("KOETT_PARAKEET_EVAL").unwrap());
        let audio = AudioRecording::read_wav(&directory.join("test_wavs/0.wav")).unwrap();
        let mut transcriber = ParakeetTranscriber::load(&directory, 2).unwrap();
        let short = transcriber.transcribe(&audio).unwrap();
        let texts = |transcript: &Transcript| {
            transcript
                .words
                .iter()
                .map(|word| word.text.clone())
                .collect::<Vec<_>>()
        };
        assert!(
            texts(&short)
                .iter()
                .map(String::as_str)
                .eq(short.text.split_whitespace())
        );
        // Speech starts 0.4 s in; an even estimate would start at zero.
        assert!(
            (short.words[0].start - 0.4).abs() < 0.05,
            "{:?}",
            short.words[0]
        );
        let mut repeated = audio.samples.clone();
        for _ in 0..3 {
            repeated.extend(vec![0.0; audio.sample_rate as usize]);
            repeated.extend(&audio.samples);
        }
        let long = transcriber
            .transcribe(&AudioRecording::new(audio.sample_rate, repeated).unwrap())
            .unwrap();
        assert!(
            texts(&long)
                .iter()
                .map(String::as_str)
                .eq(long.text.split_whitespace())
        );
        assert!(
            long.words
                .windows(2)
                .all(|pair| pair[0].start <= pair[1].start)
        );
        let last = long.words.last().unwrap();
        assert!(
            last.end > long.audio_duration.as_secs_f32() - 2.0,
            "{last:?}"
        );
    }

    #[test]
    fn realtime_factor_uses_transcription_over_audio() {
        let transcript = Transcript {
            raw: None,
            text: String::new(),
            words: Vec::new(),
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

    #[test]
    #[ignore = "requires KOETT_MODEL_DIR and measures the local machine"]
    fn background_segment_latency_probe() {
        let model_directory =
            PathBuf::from(env::var("KOETT_MODEL_DIR").expect("set KOETT_MODEL_DIR"));
        let mut transcriber = ParakeetTranscriber::load(&model_directory, 2).unwrap();
        let vad = transcriber
            .vad
            .as_ref()
            .expect("the model folder needs VAD");
        let sample =
            AudioRecording::read_wav(&model_directory.join("test_wavs").join("0.wav")).unwrap();
        assert_eq!(sample.sample_rate, MODEL_SAMPLE_RATE);

        let audio_seconds = 420_usize;
        let sample_count = MODEL_SAMPLE_RATE as usize * audio_seconds;
        let samples = sample
            .samples
            .iter()
            .copied()
            .cycle()
            .take(sample_count)
            .collect::<Vec<_>>();
        let expected_repetitions = sample_count / sample.samples.len();
        let mut worker_free_at = 0.0_f64;
        let mut decode_times = Vec::new();
        let mut parts = Vec::new();

        vad.reset();
        let (windows, remainder) = samples.as_chunks::<VAD_WINDOW_SIZE>();
        for (index, window) in windows.iter().enumerate() {
            vad.accept_waveform(window);
            let available_at =
                (index + 1) as f64 * VAD_WINDOW_SIZE as f64 / MODEL_SAMPLE_RATE as f64;
            drain_background_segments(
                &transcriber,
                available_at,
                &mut worker_free_at,
                &mut decode_times,
                &mut parts,
            );
        }
        if !remainder.is_empty() {
            let mut final_window = [0.0_f32; VAD_WINDOW_SIZE];
            final_window[..remainder.len()].copy_from_slice(remainder);
            vad.accept_waveform(&final_window);
        }
        vad.flush();
        drain_background_segments(
            &transcriber,
            audio_seconds as f64,
            &mut worker_free_at,
            &mut decode_times,
            &mut parts,
        );

        let text = parts.join(" ").to_ascii_lowercase();
        let recognized_starts = text.matches("phoebe").count();
        let recognized_ends = text.matches("old portrait").count();
        assert!(recognized_starts >= expected_repetitions);
        assert!(recognized_ends >= expected_repetitions);

        decode_times.sort_unstable();
        let total = decode_times.iter().sum::<Duration>();
        let maximum = decode_times.last().copied().unwrap_or_default();
        let p95_index = (decode_times.len() * 95).div_ceil(100).saturating_sub(1);
        let p95 = decode_times.get(p95_index).copied().unwrap_or_default();
        let post_stop_tail = (worker_free_at - audio_seconds as f64).max(0.0);
        eprintln!(
            "background_latency_probe=passed audio_seconds={audio_seconds} segments={} total_decode_ms={:.1} p95_decode_ms={:.1} max_decode_ms={:.1} simulated_post_stop_ms={:.1} recognized_starts={recognized_starts} recognized_ends={recognized_ends}",
            decode_times.len(),
            total.as_secs_f64() * 1_000.0,
            p95.as_secs_f64() * 1_000.0,
            maximum.as_secs_f64() * 1_000.0,
            post_stop_tail * 1_000.0,
        );

        let (sender, receiver) = mpsc::channel();
        for chunk in samples.chunks(2_048) {
            sender.send(chunk.to_vec()).unwrap();
        }
        drop(sender);
        let production_started = Instant::now();
        let production_text = transcriber
            .transcribe_while_recording(MODEL_SAMPLE_RATE, receiver, &AtomicBool::new(false))
            .unwrap()
            .expect("seven minutes must use background transcription")
            .text
            .to_ascii_lowercase();
        let production_elapsed = production_started.elapsed();
        let production_starts = production_text.matches("phoebe").count();
        let production_ends = production_text.matches("old portrait").count();
        assert!(production_starts >= expected_repetitions);
        assert!(production_ends >= expected_repetitions);
        eprintln!(
            "background_production_path=passed audio_seconds={audio_seconds} total_ms={:.1} recognized_starts={production_starts} recognized_ends={production_ends}",
            production_elapsed.as_secs_f64() * 1_000.0,
        );

        let mut forced_text = String::new();
        let mut forced_worker_free_at = 0.0_f64;
        let mut forced_decode_times = Vec::new();
        for range in bounded_decode_ranges(samples.len()) {
            let available_at = range.end as f64 / MODEL_SAMPLE_RATE as f64;
            let started = Instant::now();
            let (text, chunks) = transcriber.decode_bounded(&samples[range]).unwrap();
            let elapsed = started.elapsed();
            assert_eq!(chunks, 1);
            forced_worker_free_at = forced_worker_free_at.max(available_at) + elapsed.as_secs_f64();
            forced_decode_times.push(elapsed);
            merge_overlapping_text(&mut forced_text, &text);
        }
        let forced_text = forced_text.to_ascii_lowercase();
        let forced_starts = forced_text.matches("don't wish to see it").count();
        let forced_ends = forced_text.matches("old portrait").count();
        let minimum_forced_anchors = expected_repetitions.saturating_sub(1);
        assert!(
            forced_starts >= minimum_forced_anchors,
            "forced chunks kept {forced_starts} starts for {expected_repetitions} repetitions"
        );
        assert!(
            forced_ends >= minimum_forced_anchors,
            "forced chunks kept {forced_ends} ends for {expected_repetitions} repetitions"
        );

        forced_decode_times.sort_unstable();
        let forced_total = forced_decode_times.iter().sum::<Duration>();
        let forced_maximum = forced_decode_times.last().copied().unwrap_or_default();
        let forced_p95_index = (forced_decode_times.len() * 95)
            .div_ceil(100)
            .saturating_sub(1);
        let forced_p95 = forced_decode_times
            .get(forced_p95_index)
            .copied()
            .unwrap_or_default();
        let forced_post_stop_tail = (forced_worker_free_at - audio_seconds as f64).max(0.0);
        eprintln!(
            "background_forced_chunks=passed audio_seconds={audio_seconds} chunks={} total_decode_ms={:.1} p95_decode_ms={:.1} max_decode_ms={:.1} simulated_post_stop_ms={:.1} recognized_starts={forced_starts} recognized_ends={forced_ends}",
            forced_decode_times.len(),
            forced_total.as_secs_f64() * 1_000.0,
            forced_p95.as_secs_f64() * 1_000.0,
            forced_maximum.as_secs_f64() * 1_000.0,
            forced_post_stop_tail * 1_000.0,
        );

        let direct_started = Instant::now();
        let direct = transcriber.transcribe(&sample).unwrap();
        let direct_elapsed = direct_started.elapsed();
        let worker = TranscriptionWorker::start(transcriber).unwrap();
        let (sender, receiver) = mpsc::channel();
        let session = worker.record(sample.sample_rate, receiver).unwrap();
        for chunk in sample.samples.chunks(2_048) {
            sender.send(chunk.to_vec()).unwrap();
        }
        drop(sender);
        let worker_started = Instant::now();
        let through_worker = worker
            .finish(
                session,
                AudioRecording::new(sample.sample_rate, sample.samples.clone()).unwrap(),
            )
            .unwrap();
        let worker_elapsed = worker_started.elapsed();
        assert_eq!(through_worker.text, direct.text);
        eprintln!(
            "background_short_path=passed audio_seconds={:.3} direct_total_ms={:.1} worker_post_stop_ms={:.1}",
            sample.duration().as_secs_f64(),
            direct_elapsed.as_secs_f64() * 1_000.0,
            worker_elapsed.as_secs_f64() * 1_000.0,
        );
        worker.stop();
    }

    fn drain_background_segments(
        transcriber: &ParakeetTranscriber,
        available_at: f64,
        worker_free_at: &mut f64,
        decode_times: &mut Vec<Duration>,
        parts: &mut Vec<String>,
    ) {
        let vad = transcriber.vad.as_ref().unwrap();
        while let Some(segment) = vad.front() {
            vad.pop();
            let started = Instant::now();
            let (text, _) = transcriber.decode_bounded(segment.samples()).unwrap();
            let elapsed = started.elapsed();
            *worker_free_at = worker_free_at.max(available_at) + elapsed.as_secs_f64();
            decode_times.push(elapsed);
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }
}
