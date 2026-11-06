use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub use crate::media_source::validate_url;

pub struct Download {
    directory: PathBuf,
    pub pcm: PathBuf,
    pub title: String,
    pub source: String,
}
impl Drop for Download {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

pub struct Fetch {
    pub cancelled: Arc<AtomicBool>,
    pub reply: Receiver<Result<Download, String>>,
    worker: Option<JoinHandle<()>>,
}
impl Drop for Fetch {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Fetch {
    pub fn start(source: String, progress: impl Fn(&str) + Send + 'static) -> Result<Self, String> {
        if !Path::new(&source).is_absolute() {
            validate_url(&source)?;
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let (send, reply) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = send.send(fetch(source, &flag, progress));
        });
        Ok(Self {
            cancelled,
            reply,
            worker: Some(worker),
        })
    }
}

fn fetch(
    source: String,
    cancelled: &AtomicBool,
    progress: impl Fn(&str),
) -> Result<Download, String> {
    let directory = std::env::temp_dir().join(format!(
        "koett-media-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let mut download = Download {
        pcm: directory.join("audio.pcm"),
        directory,
        title: String::new(),
        source,
    };
    let started = Instant::now();
    let input = if Path::new(&download.source).is_absolute() {
        let path = PathBuf::from(&download.source);
        download.title = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        path
    } else {
        let input = download.directory.join("source.%(ext)s");
        let metadata = download.directory.join("metadata");
        let downloader = tool("yt-dlp.exe")?;
        progress("Fetching audio…");
        let mut command = Command::new(downloader);

        command.args(["--ignore-config", "--no-plugin-dirs", "--no-cache-dir", "--no-cookies", "--no-cookies-from-browser", "--no-playlist", "--playlist-items", "1", "--socket-timeout", "30", "--retries", "2", "--max-filesize", "2G", "--match-filters", "!is_live & !playlist_id & duration<=?10800", "--format", "bestaudio[protocol~=\"^(https?|m3u8_native|http_dash_segments)$\"]/best[protocol~=\"^(https?|m3u8_native|http_dash_segments)$\"]", "--downloader", "native", "--fixup", "never", "--abort-on-unavailable-fragments", "--fragment-retries", "2", "--no-progress", "--no-simulate", "--print", "after_move:{\"title\":%(title)j,\"path\":%(filepath)j}", "--output"]).arg(&input);
        command
            .arg("--ffmpeg-location")
            .arg(download.directory.join("disabled-ffmpeg"))
            .arg("--")
            .arg(&download.source);
        run(
            &mut command,
            &metadata,
            &download.directory,
            cancelled,
            Duration::from_secs(1800),
            2 * 1024 * 1024 * 1024 + 1024 * 1024,
        )?;
        if fs::metadata(&metadata).map_err(|e| e.to_string())?.len() > 65536 {
            return Err("Media metadata exceeds limit".into());
        }
        let (title, input) = downloaded_item(&metadata, &download.directory)?;
        download.title = title;
        input
    };
    progress("Inspecting media…");
    let probe = download.directory.join("probe.json");
    let mut command = Command::new(tool("ffprobe.exe")?);
    command
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file",
            "-show_entries",
            "format=duration,size:stream=codec_type",
            "-of",
            "json",
            "-i",
        ])
        .arg(&input);
    run(
        &mut command,
        &probe,
        &download.directory,
        cancelled,
        Duration::from_secs(30),
        2 * 1024 * 1024 * 1024 + 1024 * 1024,
    )?;
    if fs::metadata(&probe).map_err(|e| e.to_string())?.len() > 65536 {
        return Err("Media probe exceeds limit".into());
    }
    let info: serde_json::Value =
        serde_json::from_slice(&fs::read(&probe).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    validate_probe(&info)?;
    progress("Converting audio…");
    let mut command = Command::new(tool("ffmpeg.exe")?);

    command
        .args([
            "-nostdin",
            "-v",
            "error",
            "-protocol_whitelist",
            "file",
            "-format_whitelist",
            "mov,matroska,webm,mp3,wav,ogg,flac,aac,mpegts",
            "-i",
        ])
        .arg(input)
        .args([
            "-vn", "-ac", "1", "-ar", "16000", "-t", "10801", "-f", "s16le",
        ])
        .arg(&download.pcm);
    run(
        &mut command,
        &download.directory.join("conversion.log"),
        &download.directory,
        cancelled,
        Duration::from_secs(1800).saturating_sub(started.elapsed()),
        2 * 1024 * 1024 * 1024 + 350 * 1024 * 1024,
    )?;
    if fs::metadata(&download.pcm)
        .map_err(|e| e.to_string())?
        .len()
        > 16000 * 2 * 10800
    {
        return Err("Media exceeds three hours".into());
    }
    Ok(download)
}

fn validate_probe(info: &serde_json::Value) -> Result<(), String> {
    if !info["streams"]
        .as_array()
        .is_some_and(|streams| streams.iter().any(|stream| stream["codec_type"] == "audio"))
    {
        return Err("Media contains no audio stream".into());
    }
    if info["format"]["size"]
        .as_str()
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|size| size > 2 * 1024 * 1024 * 1024)
    {
        return Err("Media source exceeds 2 GiB".into());
    }
    let seconds = info["format"]["duration"]
        .as_str()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(f64::NAN);
    if !seconds.is_finite() || seconds <= 0.0 || seconds > 10800.0 {
        return Err("Media duration must be known and no longer than three hours".into());
    }
    Ok(())
}

fn downloaded_item(metadata: &Path, directory: &Path) -> Result<(String, PathBuf), String> {
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    #[derive(serde::Deserialize)]
    struct Item {
        title: String,
        path: PathBuf,
    }
    let item: Item =
        serde_json::from_str(&fs::read_to_string(metadata).map_err(|e| e.to_string())?)
            .map_err(|_| "No single supported media item was downloaded")?;
    let path = item.path.canonicalize().map_err(|e| e.to_string())?;
    if path.parent() != Some(directory.as_path())
        || !path.is_file()
        || fs::symlink_metadata(&item.path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        || !path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("source."))
    {
        return Err("Media downloader returned an invalid local file".into());
    }
    Ok((item.title, path))
}

fn tool(name: &str) -> Result<PathBuf, String> {
    let path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Executable directory unavailable")?
        .join("tools")
        .join(name);
    if !path.is_file() {
        return Err(format!("Media requires tools/{name} beside Koett.exe"));
    }
    Ok(path)
}

fn run(
    command: &mut Command,
    output: &Path,
    directory: &Path,
    cancelled: &AtomicBool,
    timeout: Duration,
    maximum: u64,
) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let log = File::create(output).map_err(|e| e.to_string())?;
    let job = crate::windows::formatting_job::Job::new()?;
    let child = command
        .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Media tool unavailable: {e}"))?;
    let (mut child, job) = job.supervise(child)?;
    let started = Instant::now();
    let result = loop {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) => break Err(error.to_string()),
        };
        let size: u64 = entries
            .filter_map(Result::ok)
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();
        if cancelled.load(Ordering::Acquire) {
            break Err("Media cancelled".into());
        }
        if started.elapsed() > timeout {
            break Err("Media timed out".into());
        }
        if size > maximum {
            break Err("Media exceeds the size limit".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err("Media tool failed: unsupported, blocked or login-required source".into())
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(e) => break Err(e.to_string()),
        }
    };
    drop(job);
    let _ = child.wait();
    result
}

pub fn save(title: &str, source: &str, model: &str, text: &str) -> Result<PathBuf, String> {
    let directory = crate::paths::history_file()?.with_file_name("Media Transcripts");
    let result = (|| {
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;

        let path = directory.join(format!(
            "Media-{}-{}.md",
            time::OffsetDateTime::now_utc().unix_timestamp_nanos(),
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let metadata = serde_json::json!({"title":title,"source":source,"model":model});
        file.write_all(format!("{metadata}\n\n{text}").as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(path)
    })();
    result.or_else(|_error: String| {
        match crate::history::save_media_recovery_status(
            &serde_json::json!({"media_title": title,"source":source,"model":model}).to_string(),
            text,
        )? {
            crate::history::MediaRecovery::File(path) => Ok(path),
            crate::history::MediaRecovery::History => crate::paths::history_database(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn probe_refuses_missing_audio_unknown_and_excessive_duration() {
        for duration in ["0", "-1", "NaN", "inf", "10801", "unknown"] {
            assert!(validate_probe(&serde_json::json!({"streams":[{"codec_type":"audio"}],"format":{"duration":duration}})).is_err());
        }
        for duration in ["0.1", "10800"] {
            assert!(validate_probe(&serde_json::json!({"streams":[{"codec_type":"audio"}],"format":{"duration":duration}})).is_ok());
        }
        assert!(
            validate_probe(
                &serde_json::json!({"streams":[{"codec_type":"video"}],"format":{"duration":"4"}})
            )
            .is_err()
        );
    }
    #[test]
    fn owned_tools_exit_cancel_and_timeout() {
        let directory = std::env::temp_dir().join(format!(
            "koett-media-test-{}",
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let flag = AtomicBool::new(false);
        assert!(
            run(
                Command::new("cmd.exe").args(["/d", "/c", "exit 0"]),
                &directory.join("log"),
                &directory,
                &flag,
                Duration::from_secs(3),
                4096
            )
            .is_ok()
        );
        assert!(
            run(
                Command::new("cmd.exe").args(["/d", "/c", "exit 3"]),
                &directory.join("log"),
                &directory,
                &flag,
                Duration::from_secs(3),
                4096
            )
            .is_err()
        );
        let started = Instant::now();
        assert_eq!(
            run(
                Command::new("cmd.exe").args(["/d", "/c", "ping -n 30 127.0.0.1 >nul"]),
                &directory.join("log"),
                &directory,
                &flag,
                Duration::from_millis(100),
                4096
            ),
            Err("Media timed out".into())
        );
        assert!(started.elapsed() < Duration::from_secs(3));
        flag.store(true, Ordering::Release);
        assert_eq!(
            run(
                Command::new("cmd.exe").args(["/d", "/c", "ping -n 30 127.0.0.1 >nul"]),
                &directory.join("log"),
                &directory,
                &flag,
                Duration::from_secs(3),
                4096
            ),
            Err("Media cancelled".into())
        );
        flag.store(false, Ordering::Release);
        fs::write(directory.join("oversized"), [0u8; 8192]).unwrap();
        assert_eq!(
            run(
                Command::new("cmd.exe").args(["/d", "/c", "exit 0"]),
                &directory.join("log"),
                &directory,
                &flag,
                Duration::from_secs(3),
                4096
            ),
            Err("Media exceeds the size limit".into())
        );
        drop(Download {
            pcm: directory.join("audio.pcm"),
            directory: directory.clone(),
            title: String::new(),
            source: String::new(),
        });
        assert!(!directory.exists());
    }
}
