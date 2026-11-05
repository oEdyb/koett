use std::{env, ffi::OsString, path::PathBuf, process::ExitCode, sync::atomic::AtomicBool};

use koett_engine::{
    audio::AudioRecording,
    model::{self, LocalModel},
    transcription::{ParakeetTranscriber, Transcriber},
};

struct Arguments {
    selection: LocalModel,
    offline: bool,
    output: PathBuf,
    input: PathBuf,
}

impl Arguments {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let offline = args.first().is_some_and(|arg| arg == "--offline");
        let args = if offline { &args[1..] } else { args };
        if args.len() != 5 || args[0] != "--model-version" || args[2] != "--results-json" {
            return Err("usage: koett-transcribe-engine [--offline] --model-version compact|v2|v3 --results-json OUTPUT WAV".into());
        }
        let selection = match args[1].to_str() {
            Some("compact") => LocalModel::Compact,
            Some("v2") => LocalModel::ParakeetV2,
            Some("v3") => LocalModel::ParakeetV3,
            _ => return Err("model must be compact, v2 or v3".into()),
        };
        let output = PathBuf::from(&args[3]);
        let input = PathBuf::from(&args[4]);
        if output == input
            || (output.exists() && output.canonicalize().ok() == input.canonicalize().ok())
        {
            return Err("Result output must differ from the input WAV".into());
        }
        Ok(Self {
            selection,
            offline,
            output,
            input,
        })
    }

    fn model_directory(&self) -> Result<PathBuf, String> {
        if self.offline {
            let directory = self.selection.directory()?;
            model::validate_local_model(self.selection, &directory)
                .map_err(|error| format!("Offline model is unavailable or invalid: {error}"))?;
            Ok(directory)
        } else {
            model::ensure_model(self.selection, &AtomicBool::new(false), |progress| {
                eprintln!("{progress:?}");
            })
        }
    }
}

fn run() -> Result<(), String> {
    let args = Arguments::parse(&env::args_os().skip(1).collect::<Vec<_>>())?;
    let audio = AudioRecording::read_wav(&args.input)?;
    if audio.sample_rate != 16000 || audio.duration().as_secs_f64() > 10800.0 {
        return Err("expected 16 kHz mono WAV, at most three hours".into());
    }
    let directory = args.model_directory()?;
    let mut engine = ParakeetTranscriber::load(&directory, 4)?;
    let result = engine.transcribe(&audio)?;
    let output = serde_json::json!([{
        "text": result.text,
        "audioSeconds": result.audio_duration.as_secs_f64(),
        "engineMilliseconds": result.transcription.as_secs_f64() * 1000.0
    }]);
    std::fs::write(args.output, output.to_string())
        .map_err(|error| format!("could not write result: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn legacy_model_calls_keep_their_selection_and_paths() {
        for (name, selection) in [
            ("v2", LocalModel::ParakeetV2),
            ("v3", LocalModel::ParakeetV3),
        ] {
            let options = Arguments::parse(&args(&[
                "--model-version",
                name,
                "--results-json",
                "result å.json",
                "speech ö.wav",
            ]))
            .unwrap();
            assert_eq!(options.selection, selection);
            assert!(!options.offline);
            assert_eq!(options.output, PathBuf::from("result å.json"));
            assert_eq!(options.input, PathBuf::from("speech ö.wav"));
        }
    }

    #[test]
    fn offline_compact_is_explicit_and_does_not_change_legacy_defaults() {
        let options = Arguments::parse(&args(&[
            "--offline",
            "--model-version",
            "compact",
            "--results-json",
            "result.json",
            "speech.wav",
        ]))
        .unwrap();
        assert_eq!(options.selection, LocalModel::Compact);
        assert!(options.offline);
    }

    #[test]
    fn malformed_arguments_and_input_overwrite_are_rejected() {
        for values in [
            vec![],
            vec!["--offline"],
            vec![
                "--model-version",
                "nemotron",
                "--results-json",
                "r.json",
                "s.wav",
            ],
            vec![
                "--model-version",
                "v2",
                "--results-json",
                "same.wav",
                "same.wav",
            ],
            vec![
                "--model-version",
                "v2",
                "--results-json",
                "r.json",
                "s.wav",
                "--offline",
            ],
        ] {
            assert!(Arguments::parse(&args(&values)).is_err());
        }
    }
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
