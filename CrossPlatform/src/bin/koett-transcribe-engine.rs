use std::{env, path::PathBuf, process::ExitCode, sync::atomic::AtomicBool};

use koett_engine::{
    audio::AudioRecording,
    model::{self, LocalModel},
    transcription::{ParakeetTranscriber, Transcriber},
};

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() != 5 || args[0] != "--model-version" || args[2] != "--results-json" {
        return Err(
            "usage: koett-transcribe-engine --model-version v2|v3 --results-json OUTPUT WAV".into(),
        );
    }
    let selection = match args[1].to_str() {
        Some("v2") => LocalModel::ParakeetV2,
        Some("v3") => LocalModel::ParakeetV3,
        _ => return Err("model must be v2 or v3".into()),
    };
    let audio = AudioRecording::read_wav(&PathBuf::from(&args[4]))?;
    if audio.sample_rate != 16000 || audio.duration().as_secs_f64() > 10800.0 {
        return Err("expected 16 kHz mono WAV, at most three hours".into());
    }
    let directory = model::ensure_model(selection, &AtomicBool::new(false), |progress| {
        eprintln!("{progress:?}");
    })?;
    let mut engine = ParakeetTranscriber::load(&directory, 4)?;
    let result = engine.transcribe(&audio)?;
    let output = serde_json::json!([{
        "text": result.text,
        "audioSeconds": result.audio_duration.as_secs_f64(),
        "engineMilliseconds": result.transcription.as_secs_f64() * 1000.0
    }]);
    std::fs::write(PathBuf::from(&args[3]), output.to_string())
        .map_err(|error| format!("could not write result: {error}"))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
