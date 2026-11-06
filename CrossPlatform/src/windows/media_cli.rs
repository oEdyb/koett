use crate::{
    media_transcription::Fetch,
    model::{self, LocalModel},
    transcription::{ParakeetTranscriber, TranscriptionWorker},
};
use std::{path::Path, sync::atomic::AtomicBool, time::Duration};
struct Options {
    offline: bool,
    selection: LocalModel,
    source: String,
}
impl Options {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let offline = arguments.first().is_some_and(|value| value == "--offline");
        let arguments = if offline { &arguments[1..] } else { arguments };
        let [flag, model, source] = arguments else {
            return Err(
                "usage: koett-engine media [--offline] --model compact|v2|v3 URL_OR_ABSOLUTE_FILE"
                    .into(),
            );
        };
        if flag != "--model" {
            return Err("Expected --model compact|v2|v3".into());
        }
        let selection = match model.as_str() {
            "compact" => LocalModel::Compact,
            "v2" => LocalModel::ParakeetV2,
            "v3" => LocalModel::ParakeetV3,
            _ => return Err("Model must be compact, v2 or v3".into()),
        };
        if !Path::new(source).is_absolute() {
            if offline {
                return Err("Offline media requires an absolute local file path".into());
            }
            crate::media_source::validate_url(source)?;
        }
        Ok(Self {
            offline,
            selection,
            source: source.clone(),
        })
    }
}
pub fn run(arguments: &[String]) -> Result<(), String> {
    if arguments == ["--help"] || arguments == ["help"] {
        println!(
            "{}",
            serde_json::json!({"ok":true,"usage":"koett-engine media [--offline] --model compact|v2|v3 URL_OR_ABSOLUTE_FILE","outputs":[],"warnings":[]})
        );
        return Ok(());
    }
    let options = Options::parse(arguments).map_err(|error| {
        super::automation::failure("invalid_arguments", "Run koett-engine media --help", error)
    })?;
    convert(options).map_err(|error|super::automation::failure("media_failed","Check the source, cached model and tools/ffmpeg.exe, tools/ffprobe.exe and tools/yt-dlp.exe beside the executable",error))
}
fn convert(options: Options) -> Result<(), String> {
    let directory = if options.offline {
        let directory = options.selection.directory()?;
        model::validate_local_model(options.selection, &directory)
            .map_err(|error| format!("Offline model is unavailable or invalid: {error}"))?;
        directory
    } else {
        model::ensure_model(options.selection, &AtomicBool::new(false), |progress| {
            eprintln!("{progress:?}")
        })?
    };
    let fetch = Fetch::start(options.source.clone(), |progress| eprintln!("{progress}"))?;
    let download = fetch
        .reply
        .recv_timeout(Duration::from_secs(1801))
        .map_err(|_| "Media conversion timed out".to_string())??;
    let worker = TranscriptionWorker::start(ParakeetTranscriber::load(&directory, 4)?)?;
    let result = worker
        .media_request(download.pcm.clone(), fetch.cancelled.clone())?
        .recv()
        .map_err(|_| "Media transcription stopped unexpectedly".to_string())?;
    let model = worker.media_model().to_owned();
    worker.stop();
    let text = result?;
    println!(
        "{}",
        serde_json::json!({"ok":true,"source":options.source,"title":download.title,"model":model,"text":text,"outputs":[],"warnings":[]})
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }
    #[test]
    fn offline_conversion_requires_a_local_source_and_explicit_cpu_model() {
        let parsed = Options::parse(&args(&[
            "--offline",
            "--model",
            "compact",
            r"C:\media\voice å.mp3",
        ]))
        .unwrap();
        assert!(parsed.offline);
        assert_eq!(parsed.selection, LocalModel::Compact);
        assert!(
            Options::parse(&args(&[
                "--offline",
                "--model",
                "compact",
                "https://example.com/video"
            ]))
            .is_err()
        );
        assert!(Options::parse(&args(&["--model", "nemotron", r"C:\media\voice.wav"])).is_err());
    }
    #[test]
    fn malformed_and_credential_sources_fail_before_model_or_tool_work() {
        for source in [
            "file:///C:/private",
            "https://u:p@example.com/v",
            "--exec=bad",
            "relative.wav",
        ] {
            assert!(Options::parse(&args(&["--model", "compact", source])).is_err());
        }
        assert!(
            Options::parse(&args(&["--model", "compact", "https://example.com/å.mp4"])).is_ok()
        );
        assert!(Options::parse(&args(&[])).is_err());
    }
}
