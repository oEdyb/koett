//! Paste delay and recall of the 110M dictionary sidecar on dictation-length audio.
//! Args: CTC_MODEL CLIPS CORPUS OUTPUT, with KOETT_CTC_BIAS_MODEL set to a v2 copy.
//! Builds dictations from corpus clips, one voice each, so terms are known.
use koett_engine::{
    audio::AudioRecording,
    dictionary::{self, DictionaryEntry},
    transcription::{ParakeetTranscriber, Transcriber, ctc_dictionary_bias_status},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn idle() -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ctc_dictionary_bias_status().contains("cached worker ready") {
        if Instant::now() > deadline {
            return Err(ctc_dictionary_bias_status().into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

struct Clip {
    samples: Vec<f32>,
    rate: i32,
    terms: Vec<String>,
}

fn clips(root: &Path, group: &[serde_json::Value], voice: &str) -> Result<Vec<Clip>, String> {
    group
        .iter()
        .map(|clip| {
            let id = clip["id"].as_str().ok_or("id")?;
            let audio = AudioRecording::read_wav(&root.join(format!("{id}-{voice}.wav")))?;
            Ok(Clip {
                samples: audio.samples.clone(),
                rate: audio.sample_rate,
                terms: clip["terms"]
                    .as_array()
                    .map(|terms| {
                        terms
                            .iter()
                            .filter_map(|term| term.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// Whole clips, starting at `offset`, until the dictation is at least `seconds` long.
fn dictation(
    all: &[Clip],
    offset: usize,
    seconds: usize,
) -> Result<(AudioRecording, Vec<String>), String> {
    let rate = all[0].rate;
    let mut samples = Vec::new();
    let mut terms = Vec::new();
    for clip in all.iter().cycle().skip(offset) {
        samples.extend_from_slice(&clip.samples);
        samples.extend(std::iter::repeat_n(0.0, rate as usize * 35 / 100));
        terms.extend(clip.terms.iter().cloned());
        if samples.len() >= seconds * rate as usize {
            break;
        }
    }
    Ok((AudioRecording::new(rate, samples)?, terms))
}

fn found(text: &str, terms: &[String]) -> usize {
    let mut rest = text.to_string();
    terms
        .iter()
        .filter(|term| match rest.find(term.as_str()) {
            Some(at) => {
                rest.replace_range(at..at + term.len(), "");
                true
            }
            None => false,
        })
        .count()
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [ctc, root, corpus, output] = args.as_slice() else {
        return Err("CTC_MODEL CLIPS CORPUS OUTPUT".into());
    };
    let corpus: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(corpus).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let entries: Vec<_> = corpus["dictionary"]
        .as_array()
        .ok_or("dictionary")?
        .iter()
        .filter_map(|term| term.as_str())
        .map(|term| DictionaryEntry::new(term, &[]))
        .collect();
    let words: Vec<String> = entries.iter().map(|entry| entry.term.clone()).collect();
    let root = Path::new(root);
    let mut raw = ParakeetTranscriber::load_with_dictionary(Path::new(ctc), 2, &[])?;
    let mut biased = ParakeetTranscriber::load_with_dictionary(Path::new(ctc), 2, &entries)?;
    let warm = AudioRecording::read_wav(&Path::new(ctc).join("test_wavs/0.wav"))?;
    raw.transcribe(&warm)?;
    biased.transcribe(&warm)?;
    idle()?;
    let mut rows = Vec::new();
    for seconds in [4, 6, 8, 10, 15, 20, 40, 60] {
        let mut added = Vec::new();
        let mut totals = Vec::new();
        let (mut expected, mut hit_raw, mut hit_biased, mut false_terms) = (0, 0, 0, 0);
        for (index, voice) in corpus["voices"]
            .as_array()
            .ok_or("voices")?
            .iter()
            .enumerate()
        {
            let voice = voice.as_str().ok_or("voice")?;
            let positive = clips(
                root,
                corpus["positive"].as_array().ok_or("positive")?,
                voice,
            )?;
            let negative = clips(
                root,
                corpus["negative"].as_array().ok_or("negative")?,
                voice,
            )?;
            for mix in 0..4 {
                let offset = index * 7 + mix * 9;
                let (audio, terms) = dictation(&positive, offset, seconds)?;
                let spell = |text: &str| dictionary::apply(text, &entries);
                idle()?;
                let started = Instant::now();
                let plain = raw.transcribe(&audio)?;
                let raw_ms = started.elapsed().as_secs_f64() * 1000.0;
                idle()?;
                let started = Instant::now();
                let bounded = biased.transcribe(&audio)?;
                let bounded_ms = started.elapsed().as_secs_f64() * 1000.0;
                added.push(bounded_ms - raw_ms);
                totals.push(bounded_ms);
                expected += terms.len();
                hit_raw += found(&spell(&plain.text), &terms);
                hit_biased += found(&spell(&bounded.text), &terms);
                idle()?;
                let (quiet, _) = dictation(&negative, offset, seconds)?;
                let bounded = biased.transcribe(&quiet)?;
                false_terms += found(&spell(&bounded.text), &words);
            }
        }
        added.sort_by(f64::total_cmp);
        totals.sort_by(f64::total_cmp);
        let row = serde_json::json!({
            "seconds": seconds,
            "dictations": added.len(),
            "added_ms_median": added[added.len() / 2].round(),
            "added_ms_worst": added[added.len() - 1].round(),
            "added_pct_of_length_median": (added[added.len() / 2] / (seconds as f64 * 10.0) * 10.0).round() / 10.0,
            "total_ms_median": totals[totals.len() / 2].round(),
            "terms": expected,
            "recall_raw": hit_raw,
            "recall_biased": hit_biased,
            "terms_in_negative_speech": false_terms,
        });
        println!("{row}");
        rows.push(row);
    }
    std::fs::write(
        output,
        serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
