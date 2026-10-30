use super::process;
use std::os::{fd::AsRawFd, unix::process::CommandExt};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub fn capture(cancelled: &AtomicBool) -> Result<Vec<u8>, String> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        static RUNTIME: std::sync::OnceLock<Result<tokio::runtime::Runtime, String>> =
            std::sync::OnceLock::new();
        RUNTIME
            .get_or_init(|| {
                tokio::runtime::Runtime::new()
                    .map_err(|_| "Screen portal runtime unavailable".into())
            })
            .as_ref()
            .map_err(Clone::clone)?
            .block_on(portal(cancelled))
    } else {
        x11(cancelled)
    }
}
fn x11(cancelled: &AtomicBool) -> Result<Vec<u8>, String> {
    use x11rb::connection::Connection;
    let (connection, index) =
        x11rb::connect(None).map_err(|_| "X11 screen connection unavailable")?;
    let screen = &connection.setup().roots[index];
    let width = u32::from(screen.width_in_pixels);
    let height = u32::from(screen.height_in_pixels);
    if width == 0 || height == 0 {
        return Err("Invalid X11 screen dimensions".into());
    }
    let target = width.min(1600);
    let scaled = (u64::from(height) * u64::from(target) / u64::from(width)).max(1);
    let mut command = Command::new("/usr/bin/gst-launch-1.0");
    command.args([
        "-q",
        "ximagesrc",
        &format!("xid={}", screen.root),
        "num-buffers=1",
        "use-damage=false",
        "show-pointer=false",
        "!",
        "videoconvert",
        "!",
        "videoscale",
        "!",
        &format!("video/x-raw,width={target},height={scaled}"),
        "!",
        "jpegenc",
        "quality=68",
        "!",
        "fdsink",
        "fd=1",
    ]);
    let mut jpeg = Vec::new();
    process::run(
        &mut command,
        Vec::new(),
        cancelled,
        Duration::from_secs(15),
        8 * 1024 * 1024,
        |bytes| {
            jpeg.extend_from_slice(bytes);
            Ok(())
        },
    )?;
    if !jpeg.starts_with(&[0xff, 0xd8]) || !jpeg.ends_with(&[0xff, 0xd9]) {
        return Err("X11 screen did not produce a JPEG".into());
    }
    Ok(jpeg)
}
async fn portal(cancelled: &AtomicBool) -> Result<Vec<u8>, String> {
    use ashpd::desktop::{
        CreateSessionOptions, PersistMode,
        screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType},
    };
    let proxy = Screencast::new()
        .await
        .map_err(|_| "Screencast portal unavailable")?;
    let session = proxy
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|_| "Cannot create screen session")?;
    let operation = async {
        proxy
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_sources(Some(SourceType::Monitor.into()))
                    .set_multiple(false)
                    .set_cursor_mode(CursorMode::Hidden)
                    .set_persist_mode(PersistMode::DoNot),
            )
            .await
            .map_err(|_| "Cannot request screen permission")?
            .response()
            .map_err(|_| "Screen permission denied")?;
        let response = proxy
            .start(&session, None, Default::default())
            .await
            .map_err(|_| "Cannot start screen capture")?
            .response()
            .map_err(|_| "Screen permission denied")?;
        if response.streams().len() != 1 {
            return Err("Select exactly one screen".into());
        }
        let stream = &response.streams()[0];
        let (width, height) = stream.size().ok_or("Screen dimensions unavailable")?;
        if width <= 0 || height <= 0 {
            return Err("Invalid screen dimensions".into());
        }
        let target = width.min(1600);
        let scaled = (i64::from(height) * i64::from(target) / i64::from(width)).max(1);
        let fd = proxy
            .open_pipe_wire_remote(&session, Default::default())
            .await
            .map_err(|_| "Restricted screen connection unavailable")?;
        let raw = fd.as_raw_fd();
        let mut command = Command::new("/usr/bin/gst-launch-1.0");
        command.args([
            "-q",
            "pipewiresrc",
            "fd=100",
            &format!("path={}", stream.pipe_wire_node_id()),
            "num-buffers=1",
            "!",
            "videoconvert",
            "!",
            "videoscale",
            "!",
            &format!("video/x-raw,width={target},height={scaled}"),
            "!",
            "jpegenc",
            "quality=68",
            "!",
            "fdsink",
            "fd=1",
        ]);
        // The child receives only the portal's restricted connection at descriptor 100.
        unsafe {
            command.pre_exec(move || inherit_portal_fd(raw));
        }
        let mut jpeg = Vec::new();
        process::run(
            &mut command,
            Vec::new(),
            cancelled,
            Duration::from_secs(15),
            8 * 1024 * 1024,
            |bytes| {
                jpeg.extend_from_slice(bytes);
                Ok(())
            },
        )?;
        if !jpeg.starts_with(&[0xff, 0xd8]) || !jpeg.ends_with(&[0xff, 0xd9]) {
            return Err("Screen did not produce a JPEG".into());
        }
        Ok(jpeg)
    };
    let interruption = async {
        loop {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let result = tokio::select! {
        result = operation => result,
        _ = interruption => Err("Ask cancelled".into()),
        _ = tokio::time::sleep(Duration::from_secs(60)) => Err("Screen permission timed out".into()),
    };
    let _ = session.close().await;
    result
}

pub struct Capture {
    pub result: std::sync::mpsc::Receiver<Result<Vec<u8>, String>>,
    cancelled: std::sync::Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Default for Capture {
    fn default() -> Self {
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let cancel = cancelled.clone();
        let (sender, result) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _ = sender.send(capture(&cancel));
        });
        Self {
            result,
            cancelled,
            worker: Some(worker),
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn inherit_portal_fd(raw: i32) -> std::io::Result<()> {
    unsafe {
        if libc::dup2(raw, 100) < 0 || libc::fcntl(100, libc::F_SETFD, 0) < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod descriptor_tests {
    use std::os::{fd::AsRawFd, unix::process::CommandExt};
    #[test]
    fn restricted_fd_survives_exec_even_when_already_at_target() {
        let source = std::fs::File::open("/dev/null").unwrap();
        let raw = source.as_raw_fd();
        for already_at_target in [false, true] {
            let mut command = std::process::Command::new("/bin/sh");
            command.args(["-c", "test -e /proc/self/fd/100"]);
            unsafe {
                command.pre_exec(move || {
                    let fd = if already_at_target {
                        if libc::dup2(raw, 100) < 0
                            || libc::fcntl(100, libc::F_SETFD, libc::FD_CLOEXEC) < 0
                        {
                            return Err(std::io::Error::last_os_error());
                        }
                        100
                    } else {
                        raw
                    };
                    super::inherit_portal_fd(fd)
                });
            }
            assert!(command.status().unwrap().success());
        }
    }
}
