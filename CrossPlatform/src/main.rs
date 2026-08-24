use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use koett_engine::audio::{AudioRecording, capture_default_microphone};
use koett_engine::model::{self, ModelProgress};
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
    if env::args().nth(1).as_deref() == Some("--self-test") {
        if env::args().count() != 2 {
            return Err("--self-test does not take arguments".to_string());
        }
        return run_self_test();
    }
    if env::args().nth(1).as_deref() == Some("--long-self-test") {
        let values = env::args().collect::<Vec<_>>();
        if values.len() != 3 {
            return Err("--long-self-test requires one duration in seconds".to_string());
        }
        let seconds = values[2]
            .parse::<u64>()
            .map_err(|_| "long self-test duration must be a whole number".to_string())?;
        if !(60..=420).contains(&seconds) {
            return Err("long self-test duration must be between 60 and 420 seconds".to_string());
        }
        return run_long_self_test(Duration::from_secs(seconds));
    }

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

fn run_self_test() -> Result<(), String> {
    let (sample, mut transcriber) = load_self_test()?;
    let result = transcriber.transcribe(&sample)?;
    check_self_test_text(&result.text)?;
    print_result("official-model-sample", result);
    eprintln!("self_test=passed");
    Ok(())
}

fn run_long_self_test(duration: Duration) -> Result<(), String> {
    let (sample, mut transcriber) = load_self_test()?;
    let target_samples = sample.sample_rate as usize * duration.as_secs() as usize;
    let samples = sample
        .samples
        .iter()
        .copied()
        .cycle()
        .take(target_samples)
        .collect();
    let expected_repetitions = target_samples / sample.samples.len();
    let audio = AudioRecording::new(sample.sample_rate, samples)?;
    let result = transcriber.transcribe(&audio)?;
    check_self_test_text(&result.text)?;
    let recognized_repetitions = result
        .text
        .to_ascii_lowercase()
        .matches("old portrait")
        .count();
    if recognized_repetitions < expected_repetitions {
        return Err(format!(
            "long self-test recognized {recognized_repetitions} of {expected_repetitions} repeated samples"
        ));
    }
    eprintln!(
        "long_self_test=passed audio_seconds={:.3} transcribe_ms={:.1} realtime_factor={:.5} recognized_repetitions={recognized_repetitions}",
        result.audio_duration.as_secs_f64(),
        result.transcription.as_secs_f64() * 1_000.0,
        result.realtime_factor()
    );
    Ok(())
}

fn load_self_test() -> Result<(AudioRecording, ParakeetTranscriber), String> {
    let cancelled = AtomicBool::new(false);
    let mut last_percent = None;
    let model_directory = model::ensure_default_model(&cancelled, |progress| match progress {
        ModelProgress::Downloading { received, total } => {
            let percent = received.saturating_mul(100) / total;
            if last_percent != Some(percent) {
                eprintln!("model_download_percent={percent}");
                last_percent = Some(percent);
            }
        }
        ModelProgress::Installing => eprintln!("model_installing=true"),
        ModelProgress::Ready => eprintln!("model_ready=true"),
    })?;
    let sample = model_directory.join("test_wavs").join("0.wav");
    let audio = AudioRecording::read_wav(&sample)?;
    let transcriber = ParakeetTranscriber::load(&model_directory, 2)?;
    Ok((audio, transcriber))
}

fn check_self_test_text(text: &str) -> Result<(), String> {
    let normalized = text.to_ascii_lowercase();
    if !normalized.contains("phoebe") || !normalized.contains("old portrait") {
        return Err(format!(
            "the official model sample produced unexpected text: {text}"
        ));
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
            "usage: {} MODEL_DIRECTORY WAV_FILE [WAV_FILE ...]\n       {} MODEL_DIRECTORY --mic SECONDS\n       {} --self-test\n       {} --long-self-test SECONDS",
            env::args()
                .next()
                .unwrap_or_else(|| "koett-engine".to_string()),
            env::args()
                .next()
                .unwrap_or_else(|| "koett-engine".to_string()),
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
