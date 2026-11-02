use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub fn run(
    command: &mut Command,
    input: Vec<u8>,
    cancelled: &AtomicBool,
    timeout: Duration,
    limit: usize,
    mut output: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        return Err("Ask cancelled".into());
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Ask dependency unavailable".to_string())?;
    let mut stdin = child.stdin.take().ok_or("Ask input unavailable")?;
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let mut stdout = child.stdout.take().ok_or("Ask output unavailable")?;
    let fd = stdout.as_raw_fd();
    let started = Instant::now();
    let result = (|| {
        // Only this owned pipe is changed, never a host descriptor.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err("Ask pipe setup failed".into());
        }
        let mut total = 0;
        let mut buffer = [0; 8192];
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("Ask cancelled".into());
            }
            if started.elapsed() >= timeout {
                return Err("Ask timed out".into());
            }
            match stdout.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    total += count;
                    if total > limit {
                        return Err("Ask response exceeds limit".into());
                    }
                    output(&buffer[..count])?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => return Err("Ask response interrupted".into()),
            }
        }
        let status = loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("Ask cancelled".into());
            }
            if started.elapsed() >= timeout {
                return Err("Ask timed out".into());
            }
            if let Some(status) = child.try_wait().map_err(|_| "Ask worker failed")? {
                break status;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        if !status.success() {
            return Err("Ask request failed; check permissions, provider and key".into());
        }
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    let _ = writer.join();
    result
}
