use koett_engine::{
    audio::AudioRecording,
    dictionary,
    transcription::{ParakeetTranscriber, Transcriber},
};
use std::{collections::BTreeMap, fs, path::Path, time::Instant};

fn process_nice() -> Option<i32> {
    fs::read_to_string("/proc/self/stat")
        .ok()?
        .rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(16)?
        .parse()
        .ok()
}

fn main() -> Result<(), String> {
    if !sherpa_onnx::SHARED_CONTEXT_AVAILABLE {
        return Err("requires the reviewed shared-context native build".into());
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("MODEL DICTIONARY CLIP_DIRECTORY OUTPUT".into());
    }
    let entries =
        dictionary::parse_text(&fs::read_to_string(&args[1]).map_err(|e| e.to_string())?)?;
    if entries.is_empty() || Path::new(&args[0]).join("model.int8.onnx").exists() {
        return Err("requires a populated dictionary and a v2/v3 transducer model".into());
    }
    let mut engines = [
        ParakeetTranscriber::load(Path::new(&args[0]), 2)?,
        ParakeetTranscriber::load_with_dictionary(Path::new(&args[0]), 2, &entries)?,
        ParakeetTranscriber::load_with_acoustic_dictionary(Path::new(&args[0]), 2, &entries)?,
    ];
    if engines[1].dictionary_mode() != "shared_encoder" {
        return Err(
            "dictionary cache preparation failed; refusing to time spelling fallback".into(),
        );
    }
    let mut results = BTreeMap::new();
    for seconds in [5, 30, 120, 600] {
        let audio =
            AudioRecording::read_wav(&Path::new(&args[2]).join(format!("speech-{seconds}.wav")))?;
        let mut pairs = Vec::new();
        for repeat in 0..4 {
            let mut pair = serde_json::Map::new();
            for step in 0..3 {
                let index = (repeat + step) % 3;
                let load_start = fs::read_to_string("/proc/loadavg").ok();
                let start = Instant::now();
                let raw = engines[index].transcribe(&audio)?.text;
                let correction_start = Instant::now();
                let text = dictionary::apply(&raw, if index == 0 { &[] } else { &entries });
                pair.insert(match index { 0 => "empty", 1 => "seed", _ => "legacy" }.into(), serde_json::json!({"process_nice":process_nice(),"actual_dictionary_mode":engines[index].dictionary_mode(),"loadavg_start":load_start,"loadavg_end":fs::read_to_string("/proc/loadavg").ok(),"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"spelling_ms":correction_start.elapsed().as_secs_f64()*1000.0,"text":text,"mode":match index { 0 => "greedy", 1 => "shared_encoder", _ => "experimental_two_stream" },"header_sha256":sherpa_onnx::SHARED_CONTEXT_SHA256}));
            }
            pairs.push(pair);
        }
        results.insert(seconds, pairs);
        fs::write(
            &args[3],
            serde_json::to_string_pretty(&results).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        eprintln!("seconds={seconds} complete (first pair warm-up; three measured pairs)");
    }
    Ok(())
}
