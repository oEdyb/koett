use super::*;
use crate::windows::formatting_job::Job;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdout, Command, Stdio};

const OPEN_TIMEOUT: Duration = Duration::from_secs(5);
const HELPER: &str = "--windows-microphone-worker";

pub(crate) struct Process {
    child: Child,
    _job: Job,
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn helper_requested() -> bool {
    std::env::args().nth(1).as_deref() == Some(HELPER)
}

fn spawn(mode: &str, microphone: Option<&str>) -> Result<Process, String> {
    let mut executable = std::env::current_exe().map_err(|e| e.to_string())?;
    executable.set_file_name("koett.exe");
    let config = serde_json::to_string(&microphone).map_err(|e| e.to_string())?;
    if config.len() > 2048 {
        return Err("Microphone identifier exceeds its limit".into());
    }
    let job = Job::new()?;
    let mut command = Command::new(executable);
    if std::env::args().any(|arg| arg == "--microphone-open-timeout-lab") {
        command.env("KOETT_MIC_WORKER_STALL", "1");
    }
    let child = command
        .args([HELPER, mode])
        .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not start microphone worker: {e}"))?;
    let (child, job) = job.supervise(child)?;
    let mut process = Process { child, _job: job };
    let input = process
        .child
        .stdin
        .as_mut()
        .ok_or("Missing microphone input pipe")?;
    writeln!(input, "{}", config).map_err(|e| e.to_string())?;
    Ok(process)
}

fn json_line<T: serde::de::DeserializeOwned>(reader: &mut impl BufRead) -> Result<T, String> {
    let mut bytes = Vec::new();
    reader
        .take(65_536)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.last() != Some(&b'\n') {
        return Err("Microphone worker stopped before becoming ready".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid microphone response: {e}"))
}

fn ready<T: serde::de::DeserializeOwned + Send + 'static>(
    process: &mut Process,
) -> Result<(T, BufReader<ChildStdout>), String> {
    ready_with_timeout(process, OPEN_TIMEOUT)
}

fn ready_with_timeout<T: serde::de::DeserializeOwned + Send + 'static>(
    process: &mut Process,
    timeout: Duration,
) -> Result<(T, BufReader<ChildStdout>), String> {
    let output = process
        .child
        .stdout
        .take()
        .ok_or("Missing microphone output pipe")?;
    let (tx, rx) = mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let mut reader = BufReader::new(output);
        let result = json_line::<Result<T, String>>(&mut reader).and_then(|v| v);
        let _ = tx.send(result.map(|value| (value, reader)));
    });
    match rx.recv_timeout(timeout) {
        Ok(result) => {
            let _ = reader.join();
            result
        }
        Err(_) => {
            let _ = process.child.kill();
            let _ = process.child.wait();
            let _ = reader.join();
            Err("Microphone opening timed out after 5 seconds. Choose an input and retry.".into())
        }
    }
}

pub(super) fn resolve(selected: Option<&str>) -> Result<Option<String>, String> {
    if selected.is_none() {
        return Ok(None);
    }
    let mut process = spawn("resolve", selected)?;
    ready::<Option<String>>(&mut process).map(|(value, _)| value)
}

pub(super) fn start(
    microphone: Option<&str>,
) -> Result<(MicrophoneRecorder, Receiver<Vec<f32>>), String> {
    let priority = preview::RecordingPriority::acquire()?;
    let mut process = spawn("stream", microphone)?;
    let (rate, mut output) = ready::<u32>(&mut process)?;
    if !(8_000..=384_000).contains(&rate) {
        return Err("Microphone reported an unsupported sample rate".into());
    }
    let levels = AudioLevels::new();
    levels.sample_rate.store(rate, Ordering::Relaxed);
    let stop = Arc::new(AtomicBool::new(false));
    let stream_error = Arc::new(Mutex::new(None));
    let worker_levels = levels.clone();
    let worker_stop = stop.clone();
    let worker_error = stream_error.clone();
    let (send, receive) = mpsc::channel();
    let collector = thread::spawn(move || {
        let mut samples = Vec::new();
        loop {
            let chunk = read_chunk(&mut output);
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    if !worker_stop.load(Ordering::Acquire)
                        && let Ok(mut stored) = worker_error.lock()
                    {
                        *stored = Some(error);
                    }
                    break;
                }
            };
            if samples.len() + chunk.len() > rate as usize * 600 {
                if let Ok(mut stored) = worker_error.lock() {
                    *stored = Some("Recording reached its ten-minute limit".into());
                }
                break;
            }
            let mut accepted = Vec::with_capacity(chunk.len());
            measure_input(&chunk, 1, &worker_levels, |sample| accepted.push(sample));
            if !accepted.is_empty() {
                samples.extend_from_slice(&accepted);
                if send.send(accepted).is_err() {
                    break;
                }
            }
        }
        worker_levels.update(0.0, 0.0);
        samples
    });
    Ok((
        MicrophoneRecorder {
            process: Some(process),
            _capture_priority: priority,
            stream: None,
            collector: Some(collector),
            stop_collector: stop,
            sample_rate: rate as i32,
            overflowed: Arc::new(AtomicBool::new(false)),
            stream_error,
            levels,
        },
        receive,
    ))
}

fn read_chunk(reader: &mut impl Read) -> Result<Vec<f32>, String> {
    let mut count = [0; 4];
    reader.read_exact(&mut count).map_err(|e| e.to_string())?;
    let count = u32::from_le_bytes(count) as usize;
    if !(1..=2048).contains(&count) {
        return Err("Invalid microphone packet length".into());
    }
    let mut bytes = vec![0; count * 4];
    reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    let samples = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect::<Vec<_>>();
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err("Invalid microphone samples".into());
    }
    Ok(samples)
}

pub fn helper() -> Result<(), String> {
    let mode = std::env::args().nth(2).ok_or("Missing microphone mode")?;
    if std::env::var_os("KOETT_MIC_WORKER_STALL").is_some() {
        thread::sleep(Duration::from_secs(30));
    }
    let microphone: Option<String> = json_line(&mut std::io::stdin().lock())?;
    let mut output = std::io::stdout().lock();
    if mode == "resolve" {
        return writeln!(
            output,
            "{}",
            serde_json::to_string(&resolve_recording_input_native(microphone.as_deref()))
                .map_err(|e| e.to_string())?
        )
        .map_err(|e| e.to_string());
    }
    if mode != "stream" {
        return Err("Unknown microphone mode".into());
    }
    let (send, receive) = mpsc::channel();
    let opened =
        MicrophoneRecorder::start_inner(MAX_RECORDING_DURATION, Some(send), microphone.as_deref());
    let recorder = match opened {
        Ok(recorder) => recorder,
        Err(error) => {
            writeln!(
                output,
                "{}",
                serde_json::to_string(&Err::<u32, _>(error)).map_err(|e| e.to_string())?
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }
    };
    let first = receive.recv().map_err(|e| e.to_string())?;
    writeln!(
        output,
        "{}",
        serde_json::to_string(&Ok::<_, String>(recorder.sample_rate() as u32))
            .map_err(|e| e.to_string())?
    )
    .map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    for chunk in std::iter::once(first).chain(receive) {
        output
            .write_all(&(chunk.len() as u32).to_le_bytes())
            .map_err(|e| e.to_string())?;
        for sample in chunk {
            output
                .write_all(&sample.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
        output.flush().map_err(|e| e.to_string())?;
    }
    drop(recorder);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hung_open_is_terminated_and_a_later_open_can_respond() {
        for script in ["Start-Sleep -Seconds 30", "Write-Output '{\"Ok\":16000}'"] {
            let job = Job::new().unwrap();
            let child = Command::new("powershell.exe")
                .args(["-NoProfile", "-Command", script])
                .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let (child, job) = job.supervise(child).unwrap();
            let mut process = Process { child, _job: job };
            if script.starts_with("Start-Sleep") {
                let start = std::time::Instant::now();
                assert!(
                    ready_with_timeout::<u32>(&mut process, Duration::from_millis(50)).is_err()
                );
                assert!(start.elapsed() < Duration::from_secs(3));
                assert!(process.child.try_wait().unwrap().is_some());
            } else {
                assert_eq!(ready::<u32>(&mut process).unwrap().0, 16_000);
            }
        }
    }
    #[test]
    fn microphone_protocol_rejects_corrupt_and_unbounded_packets() {
        assert!(read_chunk(&mut &0_u32.to_le_bytes()[..]).is_err());
        assert!(read_chunk(&mut &2049_u32.to_le_bytes()[..]).is_err());
        let mut bytes = 1_u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&f32::NAN.to_le_bytes());
        assert!(read_chunk(&mut bytes.as_slice()).is_err());
        bytes[4..].copy_from_slice(&0.25_f32.to_le_bytes());
        assert_eq!(read_chunk(&mut bytes.as_slice()).unwrap(), [0.25]);
        assert!(json_line::<u32>(&mut &b"123"[..]).is_err());
    }
}
