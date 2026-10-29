use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub fn validate_url(value: &str) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|_| "Enter an HTTP(S) URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || value.chars().any(char::is_control)
        || value.len() > 7000
        || !value.contains("://")
        || value.split_once("://").is_some_and(|(_, rest)| {
            rest.split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| authority.contains('@'))
        })
    {
        return Err("Enter an HTTP(S) URL without credentials".into());
    }
    Ok(())
}

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
        validate_url(&source)?;
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
    use std::os::unix::fs::DirBuilderExt;
    let directory = std::env::temp_dir().join(format!(
        "koett-media-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|e| e.to_string())?;
    let mut download = Download {
        pcm: directory.join("audio.pcm"),
        directory,
        title: String::new(),
        source,
    };
    let input = download.directory.join("source.%(ext)s");
    let metadata = download.directory.join("metadata");
    let tool = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("Home directory unavailable")?
        .join(".local/bin/yt-dlp");
    let started = Instant::now();
    progress("Fetching audio…");
    let mut command = Command::new(tool);
    bound_file_size(&mut command, 2 * 1024 * 1024 * 1024);
    command.args(["--ignore-config", "--no-cookies", "--no-cookies-from-browser", "--no-playlist", "--playlist-items", "1", "--socket-timeout", "30", "--retries", "2", "--max-filesize", "2G", "--match-filters", "!is_live & !playlist_id & duration<=?10800", "--format", "bestaudio[protocol~=\"^(https?|m3u8_native|http_dash_segments)$\"]/best[protocol~=\"^(https?|m3u8_native|http_dash_segments)$\"]", "--downloader", "native", "--fixup", "never", "--abort-on-unavailable-fragments", "--fragment-retries", "2", "--no-progress", "--no-simulate", "--print", "after_move:{\"title\":%(title)j,\"path\":%(filepath)j}", "--output"]).arg(&input);
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
    progress("Converting audio…");
    let mut command = Command::new("/usr/bin/ffmpeg");
    bound_file_size(&mut command, 16000 * 2 * 10800);
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

fn bound_file_size(command: &mut Command, bytes: u64) {
    use std::os::unix::process::CommandExt;
    // setrlimit is async-signal-safe; the child closure performs no allocation.
    unsafe {
        command.pre_exec(move || {
            let limit = libc::rlimit {
                rlim_cur: bytes,
                rlim_max: bytes,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn run(
    command: &mut Command,
    output: &Path,
    directory: &Path,
    cancelled: &AtomicBool,
    timeout: Duration,
    maximum: u64,
) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let log = File::create(output).map_err(|e| e.to_string())?;
    let mut child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Media tool unavailable: {e}"))?;
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
        if cancelled.load(Ordering::Acquire) || started.elapsed() > timeout || size > maximum {
            break Err("Media cancelled, timed out, or exceeded size limit".into());
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
    let _ = Command::new("/usr/bin/kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stderr(Stdio::null())
        .status();
    let _ = child.wait();
    result
}

pub fn save(title: &str, source: &str, model: &str, text: &str) -> Result<PathBuf, String> {
    let directory = crate::paths::history_file()?.with_file_name("Media Transcripts");
    let result = (|| {
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let path = directory.join(format!(
            "Media-{}-{}.md",
            time::OffsetDateTime::now_utc().unix_timestamp_nanos(),
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .create_new(true)
            .mode(0o600)
            .write(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let metadata = serde_json::json!({"title":title,"source":source,"model":model});
        file.write_all(format!("{metadata}\n\n{text}").as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        File::open(&directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(path)
    })();
    result.or_else(|_error: String| {
        crate::history::save_media_recovery(
            &serde_json::json!({"media_title": title,"source":source,"model":model}).to_string(),
            text,
        )
    })
}

#[cfg(test)]
mod tests {
    fn scratch() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "koett-media-test-{}",
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn owned_process_exit_timeout_and_cancel() {
        use super::*;
        let directory = scratch();
        let output = directory.join("log");
        let flag = AtomicBool::new(false);
        assert!(
            run(
                Command::new("/bin/sh").args(["-c", "exit 0"]),
                &output,
                &directory,
                &flag,
                Duration::from_secs(1),
                1024
            )
            .is_ok()
        );
        assert!(
            run(
                Command::new("/bin/sh").args(["-c", "exit 3"]),
                &output,
                &directory,
                &flag,
                Duration::from_secs(1),
                1024
            )
            .is_err()
        );
        let started = Instant::now();
        assert!(
            run(
                Command::new("/bin/sh").args(["-c", "sleep 30 & wait"]),
                &output,
                &directory,
                &flag,
                Duration::from_millis(80),
                1024
            )
            .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        flag.store(true, Ordering::Release);
        assert!(
            run(
                Command::new("/bin/sh").args(["-c", "sleep 30 & wait"]),
                &output,
                &directory,
                &flag,
                Duration::from_secs(5),
                1024
            )
            .is_err()
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn child_cannot_write_past_file_bound() {
        use super::*;
        let directory = scratch();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "head -c 2048 /dev/zero"]);
        bound_file_size(&mut command, 1024);
        let path = directory.join("bounded");
        assert!(
            run(
                &mut command,
                &path,
                &directory,
                &AtomicBool::new(false),
                Duration::from_secs(2),
                4096
            )
            .is_err()
        );
        assert!(fs::metadata(path).unwrap().len() <= 1024);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn final_filename_is_local_single_item_and_preserves_title() {
        use super::*;
        let directory = scratch().canonicalize().unwrap();
        let metadata = directory.join("metadata");
        let input = directory.join("source.mp4");
        fs::write(&input, b"fixture").unwrap();
        let title = "Title åäö 🦀\nexact";
        let item = serde_json::json!({"title":title,"path":input}).to_string();
        fs::write(&metadata, &item).unwrap();
        assert_eq!(
            downloaded_item(&metadata, &directory).unwrap(),
            (title.into(), input)
        );
        let alias = directory.with_extension("alias");
        std::os::unix::fs::symlink(&directory, &alias).unwrap();
        fs::write(
            &metadata,
            serde_json::json!({"title":title,"path":alias.join("source.mp4")}).to_string(),
        )
        .unwrap();
        assert_eq!(
            downloaded_item(&alias.join("metadata"), &alias).unwrap(),
            (title.into(), directory.join("source.mp4"))
        );
        fs::remove_file(alias).unwrap();
        fs::write(&metadata, format!("{item}\n{item}")).unwrap();
        assert!(downloaded_item(&metadata, &directory).is_err());
        let link = directory.join("source.link");
        std::os::unix::fs::symlink("/etc/hosts", &link).unwrap();
        fs::write(
            &metadata,
            serde_json::json!({"title":title,"path":link}).to_string(),
        )
        .unwrap();
        assert!(downloaded_item(&metadata, &directory).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn aggregate_fragments_exceed_limit_and_are_cleaned_up() {
        use super::*;
        let directory = scratch();
        let flag = AtomicBool::new(false);
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "head -c 700 /dev/zero > a; head -c 700 /dev/zero > b; sleep 30",
        ]);
        command.current_dir(&directory);
        let started = Instant::now();
        assert!(
            run(
                &mut command,
                &directory.join("log"),
                &directory,
                &flag,
                Duration::from_secs(5),
                1024
            )
            .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(Download {
            pcm: directory.join("audio.pcm"),
            directory: directory.clone(),
            title: String::new(),
            source: String::new(),
        });
        assert!(!directory.exists());
    }

    #[test]
    fn temporary_audio_is_removed_on_drop() {
        let directory = scratch();
        let pcm = directory.join("audio.pcm");
        std::fs::write(&pcm, [0, 0]).unwrap();
        drop(super::Download {
            directory: directory.clone(),
            pcm,
            source: "https://example.com".into(),
            title: "å 🦀".into(),
        });
        assert!(!directory.exists());
    }

    #[test]
    fn url_boundary() {
        for value in [
            "file:///etc/passwd",
            "--exec=bad",
            "https://u:p@example.com",
            "ftp://example.com",
            "https://example.com/\n",
        ] {
            assert!(super::validate_url(value).is_err());
        }
        for value in [
            "https://example.com/å?q=ö",
            "http://127.0.0.1:1234/test.wav",
        ] {
            assert!(super::validate_url(value).is_ok());
        }
    }
}
