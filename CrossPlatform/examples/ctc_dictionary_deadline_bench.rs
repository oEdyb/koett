use koett_engine::{
    audio::AudioRecording,
    dictionary::{self, DictionaryEntry},
    transcription::{ParakeetTranscriber, Transcriber, ctc_dictionary_bias_status},
};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};
fn ready() -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ctc_dictionary_bias_status().contains("cached worker ready") {
        if Instant::now() > deadline || ctc_dictionary_bias_status().contains("failed") {
            return Err(ctc_dictionary_bias_status().into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("CTC_MODEL CORPUS CLIPS OUTPUT".into());
    }
    let corpus: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[1]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let dictionary: Vec<_> = corpus["dictionary"]
        .as_array()
        .ok_or("dictionary")?
        .iter()
        .filter_map(|t| t.as_str())
        .map(|t| DictionaryEntry::new(t, &[]))
        .collect();
    let mut engine =
        ParakeetTranscriber::load_with_dictionary(Path::new(&args[0]), 2, &dictionary)?;
    let sample = AudioRecording::read_wav(&Path::new(&args[0]).join("test_wavs/0.wav"))?;
    let start = Instant::now();
    let cold = engine.transcribe(&sample)?;
    let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
    ready()?;
    let mut files: Vec<_> = std::fs::read_dir(&args[2])
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "wav"))
        .collect();
    files.sort();
    let mut records = BTreeMap::new();
    for path in files {
        let audio = AudioRecording::read_wav(&path)?;
        let start = Instant::now();
        let result = engine.transcribe(&audio)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        records.insert(path.file_stem().ok_or("name")?.to_string_lossy().to_string(),serde_json::json!({"text":result.text,"with_spelling":dictionary::apply(&result.text,&dictionary),"elapsed_ms":elapsed}));
    }
    ready()?;
    let mut samples = sample.samples.clone();
    samples.extend_from_slice(&sample.samples);
    samples.extend_from_slice(&sample.samples[..sample.samples.len() / 2]);
    let long = AudioRecording::new(sample.sample_rate, samples)?;
    engine.set_dictionary(&[])?;
    let start = Instant::now();
    let raw = engine.transcribe(&long)?;
    let raw_ms = start.elapsed().as_secs_f64() * 1000.0;
    engine.set_dictionary(&dictionary)?;
    let start = Instant::now();
    let bounded = engine.transcribe(&long)?;
    let bounded_ms = start.elapsed().as_secs_f64() * 1000.0;
    let busy_status = ctc_dictionary_bias_status();
    let start = Instant::now();
    let overlap = engine.transcribe(&sample)?;
    let overlap_ms = start.elapsed().as_secs_f64() * 1000.0;
    let output = serde_json::json!({"records":records,"cold_ms":cold_ms,"cold_text":cold.text,"probe":{"duration_seconds":long.duration().as_secs_f64(),"raw_ms":raw_ms,"bounded_ms":bounded_ms,"unchanged_raw":bounded.text==raw.text,"status_after":busy_status,"overlap_ms":overlap_ms,"overlap_text":overlap.text}});
    std::fs::write(
        &args[3],
        serde_json::to_string_pretty(&output).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
