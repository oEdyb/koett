use koett_engine::{
    audio::AudioRecording,
    dictionary::{self, DictionaryEntry},
};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};
use std::{collections::BTreeMap, fs, path::Path, time::Instant};

fn recognizer(
    model: &Path,
    vocab: &Path,
    paths: Option<i32>,
    shared_context: bool,
) -> Result<OfflineRecognizer, String> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.tokens = Some(model.join("tokens.txt").display().to_string());
    config.model_config.num_threads = 2;
    config.model_config.provider = Some("cpu".into());
    config.model_config.model_type = Some("nemo_transducer".into());
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(model.join("encoder.int8.onnx").display().to_string()),
        decoder: Some(model.join("decoder.int8.onnx").display().to_string()),
        joiner: Some(model.join("joiner.int8.onnx").display().to_string()),
    };
    if let Some(paths) = paths {
        config.decoding_method = Some(
            if shared_context {
                "koett_context"
            } else {
                "modified_beam_search"
            }
            .into(),
        );
        config.max_active_paths = paths;
        config.hotwords_score = 2.0;
        config.model_config.modeling_unit = Some("bpe".into());
        config.model_config.bpe_vocab = Some(vocab.display().to_string());
    }
    OfflineRecognizer::create(&config).ok_or_else(|| "recognizer creation failed".into())
}

fn decode(
    engine: &OfflineRecognizer,
    audio: &AudioRecording,
    hotwords: &str,
    dual: bool,
    dictionary: &[DictionaryEntry],
) -> Result<(String, Option<String>), String> {
    let stream = if hotwords.is_empty() {
        engine.create_stream()
    } else {
        engine.create_stream_with_hotwords(hotwords)
    };
    stream.accept_waveform(audio.sample_rate, &audio.samples);
    let plain = dual.then(|| {
        let stream = engine.create_stream();
        stream.accept_waveform(audio.sample_rate, &audio.samples);
        stream
    });
    match &plain {
        Some(plain) => engine.decode_multiple_streams(&[plain, &stream]),
        None => engine.decode(&stream),
    }
    let biased = stream.get_result().ok_or("no result")?.text;
    match plain {
        Some(plain) => {
            let plain = plain.get_result().ok_or("no plain result")?.text;
            Ok((
                dictionary::adopt_terms(&plain, &biased, dictionary),
                Some(plain),
            ))
        }
        None => Ok((biased, None)),
    }
}

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
        return Err("MODEL CORPUS CLIPS OUTPUT".into());
    }
    let model = Path::new(&args[0]);
    let corpus: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&args[1]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let dictionary: Vec<_> = corpus["dictionary"]
        .as_array()
        .ok_or("dictionary")?
        .iter()
        .filter_map(|x| x.as_str())
        .map(|x| DictionaryEntry::new(x, &[]))
        .collect();
    let hotwords = dictionary
        .iter()
        .map(|x| x.term.as_str())
        .collect::<Vec<_>>()
        .join("/");
    let out = Path::new(&args[3]);
    let parent = out.parent().ok_or("output folder")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    fs::write(out.with_extension("meta.json"), serde_json::json!({"process_nice":process_nice(),"mode":"shared_encoder","header_sha256":sherpa_onnx::SHARED_CONTEXT_SHA256,"threads":2,"loadavg_start":fs::read_to_string("/proc/loadavg").ok()}).to_string()).map_err(|e|e.to_string())?;
    let vocab = parent.join("vocabulary.txt");
    let tokens = fs::read_to_string(model.join("tokens.txt")).map_err(|e| e.to_string())?;
    let vocabulary = tokens
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|piece| format!("{piece}\t-1.0\n"))
        .collect::<String>();
    fs::write(&vocab, vocabulary).map_err(|e| e.to_string())?;
    let engines = [
        recognizer(model, &vocab, None, false)?,
        recognizer(model, &vocab, Some(4), false)?,
        recognizer(model, &vocab, Some(4), true)?,
    ];
    let mut files: Vec<_> = fs::read_dir(&args[2])
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    files.sort();
    if files.len() != 210 {
        return Err(format!(
            "expected 210 public synthetic corpus clips; found {}",
            files.len()
        ));
    }
    let sample = AudioRecording::read_wav(&files[0])?;
    for engine in &engines {
        decode(engine, &sample, "", false, &dictionary)?;
    }
    let second = AudioRecording::read_wav(files.last().ok_or("second clip")?)?;
    let independent = |engine: &OfflineRecognizer| -> Result<(String, String), String> {
        let plain = engine.create_stream();
        plain.accept_waveform(sample.sample_rate, &sample.samples);
        let biased = engine.create_stream_with_hotwords(&hotwords);
        biased.accept_waveform(second.sample_rate, &second.samples);
        engine.decode_multiple_streams(&[&plain, &biased]);
        Ok((
            plain.get_result().ok_or("plain result")?.text,
            biased.get_result().ok_or("biased result")?.text,
        ))
    };
    if independent(&engines[1])? != independent(&engines[2])? {
        return Err("different-input fallback changed legacy decoder results".into());
    }
    let mut records = BTreeMap::new();
    for (i, file) in files.iter().enumerate() {
        let audio = AudioRecording::read_wav(file)?;
        let mut record = serde_json::Map::new();
        // Rotate the inference order per clip to spread changing host load.
        for step in 0..4 {
            let method = (i + step) % 4;
            let (name, index, hot, dual) = match method {
                0 => ("raw", 0, "", false),
                1 => ("dual4", 1, hotwords.as_str(), true),
                2 => ("context", 2, hotwords.as_str(), true),
                _ => ("empty_context", 2, "", false),
            };
            let start = Instant::now();
            let (text, plain) = decode(&engines[index], &audio, hot, dual, &dictionary)?;
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            let plain = if name == "empty_context" {
                Some(text.clone())
            } else {
                plain
            };
            if name == "raw" {
                let start = Instant::now();
                let spelling = dictionary::apply(&text, &dictionary);
                record.insert("cheap".into(), serde_json::json!({"text":spelling,"ms":elapsed+start.elapsed().as_secs_f64()*1000.0}));
            }
            let text = if name == "raw" {
                text
            } else {
                dictionary::apply(&text, &dictionary)
            };
            let elapsed = if name == "raw" {
                elapsed
            } else {
                start.elapsed().as_secs_f64() * 1000.0
            };
            record.insert(
                name.into(),
                serde_json::json!({"text":text,"ms":elapsed,"plain":plain}),
            );
        }
        if record["context"]["plain"] != record["raw"]["text"]
            || record["empty_context"]["plain"] != record["raw"]["text"]
        {
            return Err(format!(
                "shared plain stream differs from greedy: {}",
                file.display()
            ));
        }
        records.insert(
            file.file_stem()
                .ok_or("filename")?
                .to_string_lossy()
                .to_string(),
            record,
        );
        if (i + 1) % 10 == 0 {
            eprintln!("clips={}", i + 1);
        }
        fs::write(
            out,
            serde_json::to_string_pretty(&records).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
