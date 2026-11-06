mod preview_proxy;
use super::*;
use crate::windows::formatting_job::Job;
pub(super) use preview_proxy::Capture as Preview;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::{io::AsRawHandle, process::CommandExt};
use std::process::{Child, ChildStdout, Command, Stdio};

const OPEN_TIMEOUT: Duration = Duration::from_secs(5);
const HELPER: &str = "--windows-microphone-worker";

pub(crate) struct Process {
    child: Child,
    _job: Job,
    _lease: Arc<ReaderLease>,
}
impl Process {
    fn stop(&mut self) {
        let _ = self.child.kill();
        if unsafe {
            windows::Win32::System::Threading::WaitForSingleObject(
                windows::Win32::Foundation::HANDLE(self.child.as_raw_handle()),
                250,
            )
        } == windows::Win32::Foundation::WAIT_OBJECT_0
        {
            let _ = self.child.wait();
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.stop();
    }
}
static READERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct ReaderLease(&'static std::sync::atomic::AtomicUsize);
impl ReaderLease {
    fn take() -> Result<Self, String> {
        Self::take_from(&READERS)
    }
    fn take_from(counter: &'static std::sync::atomic::AtomicUsize) -> Result<Self, String> {
        counter
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 4).then_some(n + 1)
            })
            .map_err(|_| {
                "Windows microphone workers are still closing. Retry after the device recovers."
                    .to_string()
            })?;
        Ok(Self(counter))
    }
}
impl Drop for ReaderLease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
pub(super) fn join<T>(worker: thread::JoinHandle<T>) -> Result<T, String> {
    let result = unsafe {
        windows::Win32::System::Threading::WaitForSingleObject(
            windows::Win32::Foundation::HANDLE(worker.as_raw_handle()),
            250,
        )
    };
    if result != windows::Win32::Foundation::WAIT_OBJECT_0 {
        return Err(
            "Microphone worker did not close in time; retry after the device recovers".into(),
        );
    }
    worker
        .join()
        .map_err(|_| "The microphone worker stopped unexpectedly".into())
}

pub(super) fn finish_samples(
    worker: thread::JoinHandle<Vec<f32>>,
    retained: Option<Arc<Mutex<Vec<f32>>>>,
) -> Result<(Vec<f32>, Option<String>), String> {
    let result = join(worker);
    if let Some(retained) = retained {
        let samples =
            std::mem::take(&mut *retained.lock().unwrap_or_else(|error| error.into_inner()));
        let warning = result
            .err()
            .map(|error| format!("{error}; captured audio retained"));
        return Ok((samples, warning));
    }
    result.map(|samples| (samples, None))
}

pub fn helper_requested() -> bool {
    std::env::args().nth(1).as_deref() == Some(HELPER)
}

fn spawn(mode: &str, microphone: Option<&str>) -> Result<Process, String> {
    let lease = Arc::new(ReaderLease::take()?);
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
    let mut process = Process {
        child,
        _job: job,
        _lease: lease,
    };
    process
        ._job
        .attach(&process.child)
        .map_err(|error| format!("Could not supervise microphone worker: {error}"))?;
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
    let lease = Arc::clone(&process._lease);
    let (tx, rx) = mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let _lease = lease;
        let mut reader = BufReader::new(output);
        let result = json_line::<Result<T, String>>(&mut reader).and_then(|v| v);
        let _ = tx.send(result.map(|value| (value, reader)));
    });
    match rx.recv_timeout(timeout) {
        Ok(result) => {
            let _ = join(reader);
            result
        }
        Err(_) => {
            process.stop();
            let _ = join(reader);
            Err("Microphone timed out. Try again.".into())
        }
    }
}

pub(super) fn devices() -> Result<Vec<InputDevice>, String> {
    let mut process = spawn("devices", None)?;
    let (devices, _) = ready::<Vec<(String, String)>>(&mut process)?;
    Ok(devices
        .into_iter()
        .map(|(id, label)| InputDevice { id, label })
        .collect())
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
    start_proxy(microphone, "stream")
}
pub(crate) fn start_wav_proxy(
    path: &Path,
) -> Result<(MicrophoneRecorder, Receiver<Vec<f32>>), String> {
    start_proxy(
        Some(path.to_str().ok_or("WAV fixture path is not UTF-8")?),
        "fixture",
    )
}
fn start_proxy(
    microphone: Option<&str>,
    mode: &str,
) -> Result<(MicrophoneRecorder, Receiver<Vec<f32>>), String> {
    let priority = preview::RecordingPriority::acquire()?;
    let mut process = spawn(mode, microphone)?;
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
    let lease = Arc::clone(&process._lease);
    let finite = mode == "fixture";
    let retained_samples = Arc::new(Mutex::new(Vec::new()));
    let retained = Arc::clone(&retained_samples);
    let collector = thread::spawn(move || {
        let _lease = lease;
        let mut captured = 0usize;
        loop {
            let chunk = read_chunk(&mut output);
            let chunk = match chunk {
                Ok(Some(chunk)) => chunk,
                Ok(None) => {
                    if !finite
                        && !worker_stop.load(Ordering::Acquire)
                        && let Ok(mut stored) = worker_error.lock()
                    {
                        *stored = Some("Microphone ended before Stop".into());
                    }
                    break;
                }
                Err(error) => {
                    if !worker_stop.load(Ordering::Acquire)
                        && let Ok(mut stored) = worker_error.lock()
                    {
                        *stored = Some(error);
                    }
                    break;
                }
            };
            if recording_limit_exceeded(captured, chunk.len(), rate) {
                if let Ok(mut stored) = worker_error.lock() {
                    *stored = Some("Recording reached its maximum duration".into());
                }
                break;
            }
            let mut accepted = Vec::with_capacity(chunk.len());
            measure_input(&chunk, 1, &worker_levels, |sample| accepted.push(sample));
            if !accepted.is_empty() {
                captured = captured.saturating_add(accepted.len());
                retained
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .extend_from_slice(&accepted);
                if send.send(accepted).is_err() {
                    break;
                }
            }
        }
        worker_levels.update(0.0, 0.0);
        Vec::new()
    });
    Ok((
        MicrophoneRecorder {
            process: Some(process),
            retained_samples: Some(retained_samples),
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

fn recording_limit_exceeded(captured: usize, incoming: usize, sample_rate: u32) -> bool {
    captured.saturating_add(incoming) as u64
        > u64::from(sample_rate) * MAX_RECORDING_DURATION.as_secs()
}

fn read_chunk(reader: &mut impl Read) -> Result<Option<Vec<f32>>, String> {
    let mut count = [0; 4];
    reader.read_exact(&mut count).map_err(|e| e.to_string())?;
    let count = u32::from_le_bytes(count) as usize;
    if count == u32::MAX as usize {
        return Ok(None);
    }
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
    Ok(Some(samples))
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
    if mode == "devices" {
        let devices = input_devices_native().map(|devices| {
            devices
                .into_iter()
                .map(|device| (device.id, device.label))
                .collect::<Vec<_>>()
        });
        writeln!(
            output,
            "{}",
            serde_json::to_string(&devices).map_err(|e| e.to_string())?
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }
    if mode == "preview" {
        return preview_proxy::serve(&mut output, microphone.as_deref());
    }
    let opened = match mode.as_str() {
        "stream" => {
            let (send, receive) = mpsc::channel();
            MicrophoneRecorder::start_inner(
                MAX_RECORDING_DURATION,
                Some(send),
                microphone.as_deref(),
            )
            .map(|recorder| (recorder, receive))
        }
        "fixture" => microphone
            .as_deref()
            .ok_or_else(|| "Missing WAV fixture".to_string())
            .and_then(|path| MicrophoneRecorder::start_wav_fixture(Path::new(path))),
        _ => return Err("Unknown microphone mode".into()),
    };
    let (recorder, receive) = match opened {
        Ok(opened) => opened,
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
        let mut packet = Vec::with_capacity(4 + chunk.len() * 4);
        packet.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        for sample in chunk {
            packet.extend_from_slice(&sample.to_le_bytes());
        }
        output.write_all(&packet).map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
    }
    output
        .write_all(&u32::MAX.to_le_bytes())
        .map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    drop(recorder);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standalone_capture_keeps_audio_when_collector_exit_times_out() {
        let captured = vec![0.25; 16_000];
        let (release, wait) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = wait.recv();
            Vec::new()
        });
        let recorder = MicrophoneRecorder {
            process: None,
            retained_samples: Some(Arc::new(Mutex::new(captured.clone()))),
            _capture_priority: preview::RecordingPriority::acquire().unwrap(),
            stream: None,
            collector: Some(worker),
            stop_collector: Arc::new(AtomicBool::new(false)),
            sample_rate: 16_000,
            overflowed: Arc::new(AtomicBool::new(false)),
            stream_error: Arc::new(Mutex::new(None)),
            levels: AudioLevels::new(),
        };
        let recording = recorder.finish().unwrap();
        release.send(()).unwrap();
        assert_eq!(recording.samples, captured);
    }
    #[test]
    fn slow_collector_exit_keeps_all_captured_samples() {
        let samples = Arc::new(Mutex::new(vec![0.25, -0.5, 0.75]));
        let (release, wait) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = wait.recv();
            Vec::new()
        });
        let (audio, warning) = finish_samples(worker, Some(samples)).unwrap();
        release.send(()).unwrap();
        assert_eq!(audio, vec![0.25, -0.5, 0.75]);
        assert!(warning.unwrap().contains("captured audio retained"));
    }
    #[test]
    fn recording_limit_accepts_an_hour_and_rejects_past_the_shared_maximum() {
        assert!(!recording_limit_exceeded(60 * 60 * 16_000, 320, 16_000));
        let maximum = MAX_RECORDING_DURATION.as_secs() as usize * 16_000;
        assert!(!recording_limit_exceeded(maximum - 320, 320, 16_000));
        assert!(recording_limit_exceeded(maximum, 1, 16_000));
        assert!(recording_limit_exceeded(usize::MAX, 1, 16_000));
    }
    #[test]
    fn stalled_reader_cleanup_has_a_deadline() {
        let (release, wait) = mpsc::channel();
        let (finished, done) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = wait.recv();
            let _ = finished.send(());
        });
        let start = std::time::Instant::now();
        assert!(join(worker).is_err());
        assert!(start.elapsed() < Duration::from_secs(3));
        release.send(()).unwrap();
        done.recv_timeout(Duration::from_secs(1)).unwrap();
    }
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
            let mut process = Process {
                child,
                _job: job,
                _lease: Arc::new(ReaderLease::take().unwrap()),
            };
            if script.starts_with("Start-Sleep") {
                let start = std::time::Instant::now();
                assert!(
                    ready_with_timeout::<u32>(&mut process, Duration::from_millis(50)).is_err()
                );
                assert!(start.elapsed() < Duration::from_secs(3));
                assert!(process.child.try_wait().unwrap().is_some());
            } else {
                // A cold PowerShell on a busy runner can take longer than the open limit.
                let opened = ready_with_timeout::<u32>(&mut process, Duration::from_secs(60));
                assert_eq!(opened.unwrap().0, 16_000);
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
        assert_eq!(read_chunk(&mut bytes.as_slice()).unwrap(), Some(vec![0.25]));
        assert!(
            read_chunk(&mut &u32::MAX.to_le_bytes()[..])
                .unwrap()
                .is_none()
        );
        assert!(json_line::<u32>(&mut &b"123"[..]).is_err());
    }
    #[test]
    fn stalled_readers_keep_the_spawn_budget_until_they_exit() {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let mut releases = Vec::new();
        for _ in 0..4 {
            let process_lease = Arc::new(ReaderLease::take_from(&COUNTER).unwrap());
            let reader_lease = Arc::clone(&process_lease);
            let (release, wait) = mpsc::channel();
            let reader = thread::spawn(move || {
                let _lease = reader_lease;
                let _ = wait.recv();
            });
            drop(process_lease);
            releases.push((release, reader));
        }
        assert!(ReaderLease::take_from(&COUNTER).is_err());
        let (release, reader) = releases.pop().unwrap();
        release.send(()).unwrap();
        reader.join().unwrap();
        let retry = ReaderLease::take_from(&COUNTER).unwrap();
        assert!(ReaderLease::take_from(&COUNTER).is_err());
        drop(retry);
        for (release, reader) in releases {
            release.send(()).unwrap();
            reader.join().unwrap();
        }
        assert_eq!(COUNTER.load(Ordering::Acquire), 0);
    }
}
