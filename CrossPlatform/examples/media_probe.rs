#[cfg(target_os = "linux")]
fn main() -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    if let Some(root) = std::env::var_os("KOETT_MEDIA_PROBE_PRIVATE_ROOT") {
        let root = std::path::PathBuf::from(root)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let temporary = std::env::temp_dir()
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if root.parent() != Some(temporary.as_path())
            || !root.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with("koett-private-media-probe-")
            })
            || std::env::var_os("XDG_DATA_HOME").as_deref() != Some(root.join("data").as_os_str())
        {
            return Err("Media probe requires its private temporary data directory".into());
        }
        return run_probe();
    }
    let root = std::env::temp_dir().join(format!(
        "koett-private-media-probe-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .map_err(|e| e.to_string())?;
    let mut child = std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    child
        .args(std::env::args_os().skip(1))
        .env("KOETT_MEDIA_PROBE_PRIVATE_ROOT", &root);
    for kind in ["DATA", "CONFIG", "CACHE", "STATE"] {
        child.env(format!("XDG_{kind}_HOME"), root.join(kind.to_lowercase()));
    }
    println!("private_probe_directory={}", root.display());
    if !child.status().map_err(|e| e.to_string())?.success() {
        return Err("Media probe failed; isolated evidence retained".into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_probe() -> Result<(), String> {
    use koett_engine::{
        media_transcription,
        runtime::{self, EngineCommand, EngineUpdate},
        settings::Settings,
        state::AppStatus,
    };
    use std::time::Duration;
    let args: Vec<String> = std::env::args().collect();
    if args[1] == "--reject" {
        let fetch =
            media_transcription::Fetch::start(args[2].clone(), |phase| println!("{phase}"))?;
        assert!(
            fetch
                .reply
                .recv_timeout(Duration::from_secs(15))
                .map_err(|e| e.to_string())?
                .is_err()
        );
        println!("redirect_to_local_file_rejected=passed");
        return Ok(());
    }
    let engine = runtime::start(Settings {
        model_directory: Some(args[1].clone().into()),
        pause_media: false,
        ..Settings::default()
    });
    loop {
        if matches!(
            engine
                .updates
                .recv_timeout(Duration::from_secs(120))
                .map_err(|e| e.to_string())?,
            EngineUpdate::Status(AppStatus::Ready)
        ) {
            break;
        }
    }
    for attempt in 0..2 {
        engine
            .commands
            .send(EngineCommand::Media(args[2].clone()))
            .map_err(|e| e.to_string())?;
        if attempt == 0 {
            engine
                .commands
                .send(EngineCommand::CancelMedia)
                .map_err(|e| e.to_string())?;
        }
        let mut text = None;
        loop {
            match engine
                .updates
                .recv_timeout(Duration::from_secs(120))
                .map_err(|e| e.to_string())?
            {
                EngineUpdate::Notice(message) => {
                    println!("{message}");
                    if let Some(value) = message.strip_prefix("@media-result:") {
                        text = Some(value.to_string());
                    }
                }
                EngineUpdate::Status(AppStatus::Ready) => break,
                _ => {}
            }
        }
        if attempt == 1 {
            let text = text.ok_or("Media produced no result")?;
            assert!(text.to_lowercase().contains("phoebe"), "{text}");
            let saved = media_transcription::save("Unicode åäö 🦀", &args[2], "fixture", &text)?;
            let content = std::fs::read_to_string(saved).map_err(|e| e.to_string())?;
            assert!(content.contains("Unicode åäö 🦀"));
            assert!(content.ends_with(&text));
            let primary = koett_engine::paths::history_file()?.with_file_name("Media Transcripts");
            let archived = primary.with_file_name("Fixture Media Saved");
            std::fs::rename(&primary, archived).map_err(|e| e.to_string())?;
            std::fs::write(&primary, "blocked fixture").map_err(|e| e.to_string())?;
            let fallback = media_transcription::save("Unicode åäö 🦀", &args[2], "fixture", &text)?;
            let recovered = std::fs::read_to_string(fallback).map_err(|e| e.to_string())?;
            assert!(recovered.contains(&text));
            assert!(recovered.contains("Unicode åäö 🦀"));
            assert!(recovered.contains(&args[2]));
            println!("real_ffmpeg_asr_cancel_warm_reuse_unicode_exact_save_fallback=passed");
        }
    }
    engine.stop();
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn main() {}
