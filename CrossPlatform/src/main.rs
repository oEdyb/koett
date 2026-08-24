use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use koett_engine::audio::{AudioRecording, capture_default_microphone};
use koett_engine::transcription::{ParakeetTranscriber, Transcriber};

struct Arguments {
    model_directory: PathBuf,
    input: Input,
    threads: i32,
}

enum Input {
    Microphone(Duration),
    Wavs(Vec<PathBuf>),
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    let mut transcriber = ParakeetTranscriber::load(&arguments.model_directory, arguments.threads)?;
    eprintln!(
        "model_load_ms={:.1}",
        transcriber.model_load().as_secs_f64() * 1_000.0
    );

    match arguments.input {
        Input::Microphone(duration) => {
            eprintln!("recording_seconds={:.1}", duration.as_secs_f64());
            let audio = capture_default_microphone(duration)?;
            print_result("microphone", transcriber.transcribe(&audio)?);
        }
        Input::Wavs(wavs) => {
            for wav in wavs {
                let audio = AudioRecording::read_wav(&wav)?;
                print_result(&wav.display().to_string(), transcriber.transcribe(&audio)?);
            }
        }
    }

    Ok(())
}

fn print_result(source: &str, result: koett_engine::transcription::Transcript) {
    println!("{}\t{}", source, result.text);
    eprintln!(
        "source={} audio_seconds={:.3} transcribe_ms={:.1} realtime_factor={:.5}",
        source,
        result.audio_duration.as_secs_f64(),
        result.transcription.as_secs_f64() * 1_000.0,
        result.realtime_factor()
    );
}

fn parse_arguments() -> Result<Arguments, String> {
    let values = env::args().skip(1).collect::<Vec<_>>();
    if values.len() < 2 {
        return Err(format!(
            "usage: {} MODEL_DIRECTORY WAV_FILE [WAV_FILE ...]\n       {} MODEL_DIRECTORY --mic SECONDS",
            env::args()
                .next()
                .unwrap_or_else(|| "koett-engine".to_string()),
            env::args()
                .next()
                .unwrap_or_else(|| "koett-engine".to_string())
        ));
    }

    let input = if values.get(1).map(String::as_str) == Some("--mic") {
        if values.len() != 3 {
            return Err("--mic requires one duration in seconds".to_string());
        }
        let seconds = values[2]
            .parse::<f64>()
            .map_err(|_| "microphone duration must be a positive number".to_string())?;
        if !seconds.is_finite() || seconds <= 0.0 || seconds > 600.0 {
            return Err("microphone duration must be between 0 and 600 seconds".to_string());
        }
        Input::Microphone(Duration::from_secs_f64(seconds))
    } else {
        Input::Wavs(values[1..].iter().map(PathBuf::from).collect())
    };

    Ok(Arguments {
        model_directory: PathBuf::from(&values[0]),
        input,
        threads: 2,
    })
}
