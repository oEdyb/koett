use koett_engine::{
    audio::AudioRecording,
    dictionary::{self, DictionaryEntry},
    transcription::{ParakeetTranscriber, Transcriber},
};
use std::{collections::BTreeMap, path::Path, time::Instant};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("CTC_MODEL V2_MODEL CORPUS_JSON CLIPS OUTPUT_JSON".into());
    }
    let corpus: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[2]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let dictionary: Vec<_> = corpus["dictionary"]
        .as_array()
        .ok_or("dictionary required")?
        .iter()
        .filter_map(|v| v.as_str())
        .map(|term| DictionaryEntry::new(term, &[]))
        .collect();
    let mut ctc = ParakeetTranscriber::load(Path::new(&args[0]), 4)?;
    let mut plain = ParakeetTranscriber::load(Path::new(&args[1]), 4)?;
    let mut biased =
        ParakeetTranscriber::load_with_dictionary(Path::new(&args[1]), 4, &dictionary)?;
    let mut results = BTreeMap::new();
    let mut files: Vec<_> = std::fs::read_dir(&args[3])
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "wav"))
        .collect();
    files.sort();
    for path in files {
        let audio = AudioRecording::read_wav(&path)?;
        let raw = ctc.transcribe(&audio)?;
        let start = Instant::now();
        let unbiased = plain.transcribe(&audio)?;
        let plain_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        let hot = biased.transcribe(&audio)?;
        let biased_ms = start.elapsed().as_secs_f64() * 1000.0;
        let record = serde_json::json!({"raw":raw.text,"spelling":dictionary::apply(&raw.text,&dictionary),"sidecar_plain":dictionary::adopt_terms(&raw.text,&unbiased.text,&dictionary),"sidecar_biased":dictionary::adopt_terms(&raw.text,&hot.text,&dictionary),"ctc_ms":raw.transcription.as_secs_f64()*1000.0,"plain_ms":plain_ms,"biased_ms":biased_ms,"audio_seconds":audio.duration().as_secs_f64()});
        results.insert(
            path.file_stem()
                .ok_or("clip name")?
                .to_string_lossy()
                .to_string(),
            record,
        );
    }
    std::fs::write(
        &args[4],
        serde_json::to_string_pretty(&results).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
