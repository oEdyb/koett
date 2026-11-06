use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, I24, Sample, SampleFormat, SizedSample, U24};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapProd, HeapRb};
use sherpa_onnx::Wave;

#[cfg(target_os = "linux")]
pub(crate) mod inputs;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) mod preview;

#[cfg(any(target_os = "windows", test))]
mod fixture;
#[cfg(target_os = "windows")]
pub mod windows_capture;

/// The longest choice in Settings. Each recording's own limit comes from there.
pub(crate) const MAX_RECORDING_DURATION: Duration = Duration::from_secs(120 * 60);

pub struct AudioRecording {
    pub sample_rate: i32,
    pub samples: Vec<f32>,
}

impl AudioRecording {
    pub fn read_wav(path: &Path) -> Result<Self, String> {
        let path = path
            .to_str()
            .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))?;
        let wave = Wave::read(path)
            .ok_or_else(|| "could not read the WAV file; use mono 16-bit PCM".to_string())?;
        Self::new(wave.sample_rate(), wave.samples().to_vec())
    }

    pub fn new(sample_rate: i32, samples: Vec<f32>) -> Result<Self, String> {
        if sample_rate < 1 {
            return Err("audio sample rate must be positive".to_string());
        }
        if samples.is_empty() {
            return Err("the recording contains no audio".to_string());
        }
        Ok(Self {
            sample_rate,
            samples,
        })
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.samples.len() as f64 / self.sample_rate as f64)
    }

    pub fn levels(&self) -> (f32, f32) {
        let square_sum = self
            .samples
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>();
        let rms = (square_sum / self.samples.len() as f32).sqrt();
        let peak = self
            .samples
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0, f32::max);
        (rms, peak)
    }
}

#[derive(Clone)]
pub struct AudioLevels {
    rms: Arc<AtomicU32>,
    peak: Arc<AtomicU32>,
    paused: Arc<AtomicBool>,
    #[cfg(target_os = "windows")]
    captured_frames: Arc<AtomicU64>,
    #[cfg(target_os = "windows")]
    sample_rate: Arc<AtomicU32>,
}

impl AudioLevels {
    pub(crate) fn new() -> Self {
        Self {
            rms: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
            peak: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
            paused: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "windows")]
            captured_frames: Arc::new(AtomicU64::new(0)),
            #[cfg(target_os = "windows")]
            sample_rate: Arc::new(AtomicU32::new(0)),
        }
    }

    /// The accepted audio timeline, excluding pauses and time after a fixture ends.
    #[cfg(target_os = "windows")]
    pub fn recorded_ms(&self) -> u64 {
        let rate = self.sample_rate.load(Ordering::Relaxed);
        self.captured_frames
            .load(Ordering::Relaxed)
            .saturating_mul(1000)
            .checked_div(u64::from(rate))
            .unwrap_or(0)
    }

    /// While paused the capture callback discards input: nothing is recorded,
    /// transcribed or metered. The stream stays open so resuming is instant.
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Release);
        if paused {
            self.update(0.0, 0.0);
        }
    }

    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    pub fn current(&self) -> (f32, f32) {
        (
            f32::from_bits(self.rms.load(Ordering::Relaxed)),
            f32::from_bits(self.peak.load(Ordering::Relaxed)),
        )
    }

    fn update(&self, rms: f32, peak: f32) {
        self.rms.store(rms.to_bits(), Ordering::Relaxed);
        self.peak.store(peak.to_bits(), Ordering::Relaxed);
    }
}

pub struct MicrophoneRecorder {
    #[cfg(target_os = "windows")]
    process: Option<windows_capture::Process>,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    _capture_priority: preview::RecordingPriority,
    stream: Option<cpal::Stream>,
    collector: Option<thread::JoinHandle<Vec<f32>>>,
    stop_collector: Arc<AtomicBool>,
    sample_rate: i32,
    overflowed: Arc<AtomicBool>,
    stream_error: Arc<Mutex<Option<String>>>,
    levels: AudioLevels,
    #[cfg(target_os = "linux")]
    live: Option<LiveInput>,
    #[cfg(target_os = "linux")]
    watcher: Option<(Arc<AtomicBool>, thread::JoinHandle<()>)>,
}

/// What a capture stream writes into. A replacement stream during a recording
/// writes to the same place, and only the active generation's samples count.
#[derive(Clone)]
struct InputSink {
    producer: Arc<Mutex<HeapProd<f32>>>,
    overflowed: Arc<AtomicBool>,
    stream_error: Arc<Mutex<Option<String>>>,
    levels: AudioLevels,
    generation: Arc<AtomicU32>,
    /// Milliseconds since `origin` of the last audio callback from the active stream.
    last_audio: Arc<AtomicU64>,
    origin: std::time::Instant,
}

impl InputSink {
    fn elapsed_ms(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }
}

/// What one stream has shown so far.
#[derive(Clone, Default)]
struct StreamProbe {
    alive: Arc<AtomicBool>,
    /// Any sample that isn't exact digital zero, ever, on this stream.
    signal: Arc<AtomicBool>,
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct LiveInput {
    sink: InputSink,
    probe: StreamProbe,
    sample_rate: u32,
}

impl MicrophoneRecorder {
    pub fn start() -> Result<Self, String> {
        Self::start_with_limit(MAX_RECORDING_DURATION)
    }

    pub fn start_with_limit(maximum_duration: Duration) -> Result<Self, String> {
        Self::start_inner(maximum_duration, None, None)
    }

    pub fn start_streaming() -> Result<(Self, Receiver<Vec<f32>>), String> {
        Self::start_streaming_with_device(None)
    }

    pub fn start_streaming_with_device(
        microphone: Option<&str>,
    ) -> Result<(Self, Receiver<Vec<f32>>), String> {
        #[cfg(target_os = "windows")]
        return windows_capture::start(microphone);
        #[cfg(not(target_os = "windows"))]
        {
            let (sender, receiver) = mpsc::channel();
            let recorder = Self::start_inner(MAX_RECORDING_DURATION, Some(sender), microphone)?;
            Ok((recorder, receiver))
        }
    }

    fn start_inner(
        maximum_duration: Duration,
        audio_chunks: Option<Sender<Vec<f32>>>,
        microphone: Option<&str>,
    ) -> Result<Self, String> {
        if maximum_duration.is_zero() || maximum_duration > MAX_RECORDING_DURATION {
            return Err(
                "maximum recording duration must be between 0 and 7200 seconds".to_string(),
            );
        }

        #[cfg(any(target_os = "linux", target_os = "windows"))]
        let capture_priority = preview::RecordingPriority::acquire()?;
        let host = audio_host();
        let device = match microphone {
            None => host
                .default_input_device()
                .ok_or("No default microphone is available")?,
            Some(name) => host
                .device_by_id(
                    &name
                        .parse::<cpal::DeviceId>()
                        .map_err(|e| format!("Invalid microphone ID: {e}"))?,
                )
                .ok_or_else(|| {
                    format!("Microphone unavailable: {name}. Choose another input in Settings")
                })?,
        };
        let supported = device
            .default_input_config()
            .map_err(|error| format!("could not read the microphone format: {error}"))?;
        let mut config = supported.config();
        request_short_buffer(&device, &mut config);
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate as usize;
        let capacity = sample_rate * 2;
        let (producer, consumer) = HeapRb::<f32>::new(capacity).split();
        let overflowed = Arc::new(AtomicBool::new(false));
        let stream_error = Arc::new(Mutex::new(None));
        let levels = AudioLevels::new();
        let sink = InputSink {
            producer: Arc::new(Mutex::new(producer)),
            overflowed: overflowed.clone(),
            stream_error: stream_error.clone(),
            levels: levels.clone(),
            generation: Arc::new(AtomicU32::new(0)),
            last_audio: Arc::new(AtomicU64::new(0)),
            origin: std::time::Instant::now(),
        };
        #[cfg(target_os = "windows")]
        levels
            .sample_rate
            .store(config.sample_rate, Ordering::Relaxed);
        let stop_collector = Arc::new(AtomicBool::new(false));

        eprintln!(
            "microphone={} sample_rate={} channels={} format={}",
            device
                .description()
                .map(|description| description.to_string())
                .unwrap_or_else(|_| "default".to_string()),
            sample_rate,
            channels,
            supported.sample_format()
        );

        let probe = StreamProbe::default();
        let stream = open_stream(
            &device,
            &config,
            supported.sample_format(),
            sink.clone(),
            0,
            probe.clone(),
        )?;

        stream
            .play()
            .map_err(|error| format!("could not start the microphone: {error}"))?;
        let collector_stop = stop_collector.clone();
        let collector = thread::Builder::new()
            .name("koett-audio-collector".to_string())
            .spawn(move || collect_samples(consumer, collector_stop, audio_chunks))
            .map_err(|error| format!("could not start the microphone collector: {error}"))?;
        if std::env::var_os("KOETT_CAPTURE_TIMINGS").is_some() {
            eprintln!("capture_ready=true");
        }

        Ok(Self {
            #[cfg(target_os = "windows")]
            process: None,
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            _capture_priority: capture_priority,
            stream: Some(stream),
            collector: Some(collector),
            stop_collector,
            sample_rate: sample_rate as i32,
            overflowed,
            stream_error,
            levels,
            #[cfg(target_os = "linux")]
            live: Some(LiveInput {
                sink,
                probe,
                sample_rate: sample_rate as u32,
            }),
            #[cfg(target_os = "linux")]
            watcher: None,
        })
    }

    pub fn levels(&self) -> AudioLevels {
        self.levels.clone()
    }

    pub fn sample_rate(&self) -> i32 {
        self.sample_rate
    }

    pub fn finish(self) -> Result<AudioRecording, String> {
        let (audio, warning) = self.finish_with_warning()?;
        match warning {
            Some(error) => Err(error),
            None => Ok(audio),
        }
    }

    pub fn finish_with_warning(mut self) -> Result<(AudioRecording, Option<String>), String> {
        #[cfg(target_os = "linux")]
        {
            self.stop_watcher();
            self.close_stream();
        }
        self.stream.take();
        self.stop_collector.store(true, Ordering::Release);
        #[cfg(target_os = "windows")]
        self.process.take();
        let collector = self
            .collector
            .take()
            .expect("a live recorder always has a collector");
        #[cfg(target_os = "windows")]
        let samples = windows_capture::join(collector)?;
        #[cfg(not(target_os = "windows"))]
        let samples = collector
            .join()
            .map_err(|_| "the microphone collector stopped unexpectedly".to_string())?;

        let stream_error = self
            .stream_error
            .lock()
            .map_err(|_| "the microphone error state is unavailable".to_string())?
            .clone();
        let warning = if let Some(error) = stream_error {
            Some(format!(
                "Microphone disconnected; recording is incomplete: {error}"
            ))
        } else if self.overflowed.load(Ordering::Relaxed) {
            Some("Audio buffer overflowed; recording may be incomplete".into())
        } else {
            None
        };

        let recording = AudioRecording::new(self.sample_rate, samples)?;
        let (rms, peak) = recording.levels();
        eprintln!("microphone_rms={rms:.6} microphone_peak={peak:.6}");
        Ok((recording, warning))
    }
}

/// A dropped Pulse client leaves its connection open: the library's reactor sleeps with
/// no timeout and nothing wakes it, so each host costs one socket and one thread until
/// the server hits its client limit and refuses every app on the desktop. So the process
/// keeps one host. It is replaced only when the server has dropped it, which also ends
/// the old reactor.
#[cfg(target_os = "linux")]
static SHARED_HOST: Mutex<Option<Arc<cpal::Host>>> = Mutex::new(None);
/// When connecting last failed. A stuck server would otherwise cost a socket per try.
#[cfg(target_os = "linux")]
static HOST_FAILED: Mutex<Option<std::time::Instant>> = Mutex::new(None);
#[cfg(target_os = "linux")]
const HOST_RETRY: Duration = Duration::from_secs(5);

/// The audio host: the shared sound server connection when there is one, else ALSA.
#[cfg(target_os = "linux")]
pub(crate) fn audio_host() -> Arc<cpal::Host> {
    let mut shared = SHARED_HOST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(host) = shared.as_ref() {
        return host.clone();
    }
    let mut failed = HOST_FAILED.lock().unwrap_or_else(|e| e.into_inner());
    if failed.is_some_and(|at| at.elapsed() < HOST_RETRY)
        && let Ok(host) = cpal::host_from_id(cpal::HostId::Alsa)
    {
        return Arc::new(host);
    }
    let host = Arc::new(cpal::default_host());
    if host.id() == cpal::HostId::PulseAudio {
        *shared = Some(host.clone());
        *failed = None;
    } else {
        *failed = Some(std::time::Instant::now());
    }
    host
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn audio_host() -> Arc<cpal::Host> {
    Arc::new(cpal::default_host())
}

/// The server dropped the connection, so the next use makes a new one.
#[cfg(target_os = "linux")]
fn forget_audio_host() {
    *SHARED_HOST.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// A running recorder, its audio, and a notice if it is not on the chosen mic.
#[cfg(target_os = "linux")]
pub type StartedInput = (MicrophoneRecorder, Receiver<Vec<f32>>, Option<String>);

/// With no audio callback for this long the stream has died. Silence still calls
/// back with quiet samples, so a thinking pause can never trigger this.
#[cfg(target_os = "linux")]
const STREAM_STALL: u64 = 2500;
#[cfg(target_os = "linux")]
const PROBE_WAIT: Duration = Duration::from_millis(1500);
/// A stream that has never produced one non-zero sample for this long has no real
/// input behind it. Only "never": a noise gate outputs exact zero during a thinking
/// pause, so a stream that has shown signal once is never called silent.
#[cfg(target_os = "linux")]
const SILENT_AFTER: u64 = 3000;

#[cfg(target_os = "linux")]
impl MicrophoneRecorder {
    /// Starts on the chosen mic, or the first that works, and keeps a working mic for
    /// the whole recording. `notify` tells the user about a switch during it. Returns
    /// a notice if capture did not start on the chosen mic. The saved choice is left
    /// alone, so that mic is used again as soon as it works.
    pub fn start_resilient(
        selected: Option<&str>,
        notify: impl Fn(String) + Send + 'static,
    ) -> Result<StartedInput, String> {
        let chosen = inputs::effective_choice(selected);
        let ((mut recorder, receiver), current, notice) =
            inputs::start_first_working(selected, |candidate: Option<String>| {
                let (sender, receiver) = mpsc::channel();
                Self::start_inner(MAX_RECORDING_DURATION, Some(sender), candidate.as_deref())
                    .map(|recorder| (recorder, receiver))
            })?;
        if let Some(live) = recorder.live.clone() {
            let stop = Arc::new(AtomicBool::new(false));
            let flag = stop.clone();
            let handle = thread::Builder::new()
                .name("koett-mic-watch".into())
                .spawn(move || watch_input(live, chosen, current, flag, notify))
                .map_err(|error| format!("could not watch the microphone: {error}"))?;
            recorder.watcher = Some((stop, handle));
        }
        Ok((recorder, receiver, notice))
    }

    /// Stops the watcher without waiting: it may be probing a stuck server, and
    /// stopping a recording must never wait on that. No stream writes after this.
    fn stop_watcher(&mut self) {
        if let Some((stop, _)) = self.watcher.take() {
            stop.store(true, Ordering::Release);
        }
        if let Some(live) = &self.live {
            live.sink.generation.store(u32::MAX, Ordering::Release);
        }
    }

    /// Closes the stream, without letting a stuck server hold up the engine.
    fn close_stream(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = inputs::bounded(Duration::from_secs(1), move || drop(stream));
        }
    }
}

/// Opens a stream on `id` at the recording's sample rate, for a mid-recording switch.
#[cfg(target_os = "linux")]
fn open_replacement(
    id: Option<&str>,
    live: &LiveInput,
    generation: u32,
    probe: StreamProbe,
) -> Result<cpal::Stream, String> {
    let host = audio_host();
    let device = match id {
        None => host.default_input_device(),
        Some(id) => host.device_by_id(&id.parse::<cpal::DeviceId>().map_err(|e| e.to_string())?),
    }
    .ok_or("Microphone unavailable")?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let mut config = supported.config();
    config.sample_rate = live.sample_rate;
    request_short_buffer(&device, &mut config);
    let stream = open_stream(
        &device,
        &config,
        supported.sample_format(),
        live.sink.clone(),
        generation,
        probe,
    )?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

/// Replaces the stream if it errors, goes quiet at the callback level, or its source
/// disappears. Runs on its own thread, so the engine never waits on any of this.
#[cfg(target_os = "linux")]
fn watch_input(
    live: LiveInput,
    chosen: Option<String>,
    mut current: Option<String>,
    stop: Arc<AtomicBool>,
    notify: impl Fn(String),
) {
    // Keep the replacement alive here. The original stays with the recorder.
    let mut replacement: Option<cpal::Stream> = None;
    let mut generation = 0;
    let mut ticks = 0_u32;
    let sink = &live.sink;
    let mut active = live.probe.clone();
    let mut listening_since = sink.elapsed_ms();
    let mut gave_up = false;
    let mut silence_reported = false;
    let checking = Arc::new(AtomicBool::new(false));
    while !stop.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(200));
        ticks = ticks.wrapping_add(1);
        let now = sink.elapsed_ms();
        if sink.levels.paused() {
            // Paused audio is discarded, so it says nothing about the mic.
            listening_since = now;
        }
        let errored = sink.stream_error.lock().is_ok_and(|error| error.is_some());
        let stalled = now.saturating_sub(sink.last_audio.load(Ordering::Acquire)) > STREAM_STALL;
        // Listing goes over the server's socket, so check for a gone source only now and then.
        // A check stuck on a frozen server is left alone rather than joined by another.
        let gone = ticks.is_multiple_of(5)
            && current.as_deref().is_some_and(|id| {
                if checking.swap(true, Ordering::AcqRel) {
                    return false;
                }
                let (id, checking) = (id.to_string(), checking.clone());
                inputs::bounded(Duration::from_secs(1), move || {
                    let listed =
                        input_devices().map(|list| list.iter().any(|device| device.id == id));
                    checking.store(false, Ordering::Release);
                    listed
                })
                .is_some_and(|listed| listed.is_ok_and(|listed| !listed))
            });
        // Working on the chosen mic again: the next problem is a new one worth saying.
        if current == chosen && active.signal.load(Ordering::Acquire) {
            inputs::settle_situation();
        }
        let failed = errored || stalled || gone;
        let silent = !failed
            && !silence_reported
            && !active.signal.load(Ordering::Acquire)
            && now.saturating_sub(listening_since) > SILENT_AFTER;
        if !(failed || silent) {
            continue;
        }
        let (order, devices) = inputs::replacements(current.as_deref());
        let mut switched = false;
        for candidate in order {
            if stop.load(Ordering::Acquire) {
                return;
            }
            let next = generation + 1;
            let probe = StreamProbe::default();
            let opened = {
                let (live, probe, id) = (live.clone(), probe.clone(), candidate.clone());
                inputs::bounded(Duration::from_millis(2500), move || {
                    open_replacement(id.as_deref(), &live, next, probe)
                })
            };
            let Some(Ok(stream)) = opened else { continue };
            // Replacing a silent mic needs proof of signal, or it could pick another
            // silent one. Replacing a lost mic only needs audio to flow.
            let proven = |probe: &StreamProbe| {
                if silent {
                    probe.signal.load(Ordering::Acquire)
                } else {
                    probe.alive.load(Ordering::Acquire)
                }
            };
            let started = std::time::Instant::now();
            while !proven(&probe) && started.elapsed() < PROBE_WAIT {
                thread::sleep(Duration::from_millis(20));
            }
            if !proven(&probe) {
                eprintln!(
                    "microphone_candidate_rejected id={candidate:?} at_ms={}",
                    sink.elapsed_ms()
                );
                continue;
            }
            generation = next;
            sink.generation.store(next, Ordering::Release);
            sink.last_audio.store(sink.elapsed_ms(), Ordering::Release);
            listening_since = sink.elapsed_ms();
            if let Ok(mut error) = sink.stream_error.lock() {
                *error = None;
            }
            replacement = Some(stream);
            active = probe;
            eprintln!(
                "microphone_switched generation={next} silent={silent} id={candidate:?} at_ms={}",
                sink.elapsed_ms()
            );
            if silent {
                inputs::remember_silent(current.as_deref());
            }
            if inputs::announce(chosen.as_deref(), candidate.as_deref()) {
                notify(if silent {
                    inputs::silent_notice(candidate.as_deref(), &devices)
                } else {
                    inputs::lost_notice(candidate.as_deref(), &devices)
                });
            }
            current = candidate;
            gave_up = false;
            silence_reported = false;
            switched = true;
            break;
        }
        if switched {
            continue;
        }
        if silent {
            // Nothing better: keep recording and say so once.
            silence_reported = true;
            eprintln!("microphone_silent no replacement");
            if inputs::announce(chosen.as_deref(), Some(inputs::NOTHING_BETTER)) {
                notify("This mic is silent".into());
            }
        } else {
            // Nothing works: the recording keeps what it has and says so at the end.
            if !gave_up {
                gave_up = true;
                if let Ok(mut error) = sink.stream_error.lock()
                    && error.is_none()
                {
                    *error = Some("no audio from the microphone".into());
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    }
    drop(replacement);
}

impl Drop for MicrophoneRecorder {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            self.stop_watcher();
            self.close_stream();
        }
        self.stream.take();
        self.stop_collector.store(true, Ordering::Release);
        #[cfg(target_os = "windows")]
        self.process.take();
        if let Some(collector) = self.collector.take() {
            #[cfg(target_os = "windows")]
            let _ = windows_capture::join(collector);
            #[cfg(not(target_os = "windows"))]
            let _ = collector.join();
        }
    }
}

pub fn capture_default_microphone(duration: Duration) -> Result<AudioRecording, String> {
    if duration.is_zero() || duration > MAX_RECORDING_DURATION {
        return Err("microphone duration must be between 0 and 7200 seconds".to_string());
    }
    let recorder = MicrophoneRecorder::start()?;
    thread::sleep(duration);
    recorder.finish()
}

fn collect_samples(
    mut consumer: ringbuf::HeapCons<f32>,
    stop: Arc<AtomicBool>,
    mut audio_chunks: Option<Sender<Vec<f32>>>,
) -> Vec<f32> {
    let mut samples = Vec::new();
    #[cfg(target_os = "windows")]
    let forward_only = windows_capture::helper_requested();
    let mut chunk = [0.0_f32; 2_048];
    loop {
        let count = consumer.pop_slice(&mut chunk);
        #[cfg(target_os = "windows")]
        if !forward_only {
            samples.extend_from_slice(&chunk[..count]);
        }
        #[cfg(not(target_os = "windows"))]
        samples.extend_from_slice(&chunk[..count]);
        if count > 0
            && let Some(sender) = &audio_chunks
            && sender.send(chunk[..count].to_vec()).is_err()
        {
            audio_chunks = None;
        }
        if stop.load(Ordering::Acquire) && consumer.is_empty() {
            return samples;
        }
        if count == 0 {
            thread::sleep(Duration::from_millis(2));
        }
    }
}

/// A sound server otherwise delivers capture in chunks of up to about 2 s, and
/// stopping would lose the last chunk: the end of what was said.
fn request_short_buffer(device: &cpal::Device, config: &mut cpal::StreamConfig) {
    #[cfg(target_os = "linux")]
    if device
        .id()
        .is_ok_and(|id| id.to_string().starts_with("pulseaudio:"))
    {
        config.buffer_size = cpal::BufferSize::Fixed(config.sample_rate / 25);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (device, config);
}

fn open_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    sink: InputSink,
    generation: u32,
    probe: StreamProbe,
) -> Result<cpal::Stream, String> {
    let go = (device, config, sink, generation, probe);
    match format {
        SampleFormat::I8 => build_stream::<i8>(go),
        SampleFormat::I16 => build_stream::<i16>(go),
        SampleFormat::I24 => build_stream::<I24>(go),
        SampleFormat::I32 => build_stream::<i32>(go),
        SampleFormat::I64 => build_stream::<i64>(go),
        SampleFormat::U8 => build_stream::<u8>(go),
        SampleFormat::U16 => build_stream::<u16>(go),
        SampleFormat::U24 => build_stream::<U24>(go),
        SampleFormat::U32 => build_stream::<u32>(go),
        SampleFormat::U64 => build_stream::<u64>(go),
        SampleFormat::F32 => build_stream::<f32>(go),
        SampleFormat::F64 => build_stream::<f64>(go),
        format => Err(format!("unsupported microphone sample format: {format}")),
    }
}

fn build_stream<T>(
    (device, config, sink, generation, probe): (
        &cpal::Device,
        &cpal::StreamConfig,
        InputSink,
        u32,
        StreamProbe,
    ),
) -> Result<cpal::Stream, String>
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels).max(1);
    let errors = sink.clone();
    device
        .build_input_stream(
            *config,
            move |data: &[T], _| {
                probe.alive.store(true, Ordering::Release);
                if !sink.levels.paused()
                    && !probe.signal.load(Ordering::Relaxed)
                    && has_signal(data)
                {
                    probe.signal.store(true, Ordering::Release);
                }
                if sink.generation.load(Ordering::Acquire) != generation {
                    return;
                }
                sink.last_audio.store(sink.elapsed_ms(), Ordering::Release);
                // Only a stream switch could hold this lock, and that happens off this path.
                let Ok(mut producer) = sink.producer.try_lock() else {
                    return;
                };
                measure_input(data, channels, &sink.levels, |sample| {
                    if producer.try_push(sample).is_err() {
                        sink.overflowed.store(true, Ordering::Relaxed);
                    }
                });
            },
            move |error| {
                if errors.generation.load(Ordering::Acquire) != generation {
                    return;
                }
                if is_recoverable_stream_error(error.kind()) {
                    eprintln!(
                        "microphone_stream_notice kind={:?} error={error}",
                        error.kind()
                    );
                } else {
                    eprintln!(
                        "microphone_stream_error kind={:?} error={error}",
                        error.kind()
                    );
                    if let Ok(mut stored) = errors.stream_error.lock()
                        && stored.is_none()
                    {
                        *stored = Some(error.to_string());
                    }
                }
            },
            None,
        )
        .map_err(|error| format!("could not create the microphone stream: {error}"))
}

/// Whether any sample differs from exact digital zero. Even a quiet room's noise
/// floor passes this; only a source with nothing behind it does not.
fn has_signal<T>(data: &[T]) -> bool
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
    data.iter().any(|sample| f32::from_sample(*sample) != 0.0)
}

fn measure_input<T>(data: &[T], channels: usize, levels: &AudioLevels, mut consume: impl FnMut(f32))
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
    if levels.paused() {
        return;
    }
    let mut square_sum = 0.0_f32;
    let mut peak = 0.0_f32;
    let mut frame_count = 0_usize;
    for frame in data.chunks_exact(channels) {
        let sample = frame.iter().copied().map(f32::from_sample).sum::<f32>() / channels as f32;
        square_sum += sample * sample;
        peak = peak.max(sample.abs());
        frame_count += 1;
        consume(sample);
    }
    if frame_count > 0 {
        #[cfg(target_os = "windows")]
        levels
            .captured_frames
            .fetch_add(frame_count as u64, Ordering::Relaxed);
        levels.update((square_sum / frame_count as f32).sqrt(), peak);
    }
}

fn is_recoverable_stream_error(kind: cpal::ErrorKind) -> bool {
    matches!(
        kind,
        cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied | cpal::ErrorKind::Xrun
    )
}

#[derive(Clone, Debug)]
pub struct InputDevice {
    pub id: String,
    pub label: String,
}

fn replacement_input<'a>(devices: &'a [InputDevice], default: Option<&str>) -> Option<&'a str> {
    devices
        .iter()
        .find(|device| {
            let name = device.label.to_lowercase();
            name.contains("built-in") || name.contains("builtin") || name.contains("internal")
        })
        .or_else(|| {
            devices
                .iter()
                .find(|device| Some(device.id.as_str()) == default)
        })
        .or_else(|| devices.first())
        .map(|device| device.id.as_str())
}

pub fn resolve_recording_input(selected: Option<&str>) -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    return windows_capture::resolve(selected);
    #[cfg(not(target_os = "windows"))]
    resolve_recording_input_native(selected)
}

fn resolve_recording_input_native(selected: Option<&str>) -> Result<Option<String>, String> {
    let Some(selected) = selected else {
        return Ok(None);
    };
    let host = audio_host();
    let id = selected
        .parse::<cpal::DeviceId>()
        .map_err(|e| format!("Invalid microphone ID: {e}"))?;
    let devices = input_devices_native()?;
    if devices.iter().any(|device| device.id == id.to_string()) {
        return Ok(Some(selected.into()));
    }
    let default = host
        .default_input_device()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string());
    replacement_input(&devices, default.as_deref())
        .map(|id| Some(id.into()))
        .ok_or_else(|| "No microphone is available. Connect an input and retry".into())
}

fn list_devices(host: &cpal::Host) -> Result<Vec<InputDevice>, String> {
    host.input_devices()
        .map_err(|e| e.to_string())?
        .map(|device| {
            Ok(InputDevice {
                id: device.id().map_err(|e| e.to_string())?.to_string(),
                label: device.description().map_err(|e| e.to_string())?.to_string(),
            })
        })
        .collect()
}

pub fn input_devices() -> Result<Vec<InputDevice>, String> {
    #[cfg(target_os = "windows")]
    return windows_capture::devices();
    #[cfg(not(target_os = "windows"))]
    input_devices_native()
}

fn input_devices_native() -> Result<Vec<InputDevice>, String> {
    let host = audio_host();
    let listed = list_devices(&host);
    #[cfg(target_os = "linux")]
    let listed = match listed {
        Err(_) if host.id() == cpal::HostId::PulseAudio => {
            forget_audio_host();
            list_devices(&audio_host())
        }
        listed => listed,
    };
    let all = listed?;
    #[cfg(target_os = "linux")]
    return Ok(inputs::microphones(all));
    #[cfg(not(target_os = "linux"))]
    Ok(all)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    use ringbuf::HeapRb;
    use ringbuf::traits::{Producer, Split};

    use super::{AudioRecording, collect_samples, is_recoverable_stream_error};

    #[cfg(target_os = "windows")]
    #[test]
    fn drawing_clock_counts_audio_frames_and_excludes_pause() {
        let levels = super::AudioLevels::new();
        assert_eq!(levels.recorded_ms(), 0);
        levels.sample_rate.store(16_000, super::Ordering::Relaxed);
        super::measure_input(&[0.0_f32; 320], 2, &levels, |_| {});
        assert_eq!(levels.recorded_ms(), 10);
        levels.set_paused(true);
        super::measure_input(&[0.0_f32; 320], 2, &levels, |_| panic!("paused"));
        assert_eq!(levels.recorded_ms(), 10);
        levels.set_paused(false);
        super::measure_input(&[0.0_f32; 80], 1, &levels, |_| {});
        assert_eq!(levels.clone().recorded_ms(), 15);
    }

    #[test]
    fn only_exact_zero_counts_as_no_signal() {
        assert!(!super::has_signal(&[0.0_f32; 8]));
        assert!(!super::has_signal(&[0_i16; 8]));
        assert!(!super::has_signal(&[128_u8; 8]), "unsigned zero");
        assert!(
            super::has_signal(&[0.0_f32, 0.000_001, 0.0]),
            "a noise floor"
        );
        assert!(super::has_signal(&[0_i16, 1, 0]), "one quantization step");
        assert!(super::has_signal(&[129_u8]));
    }

    #[test]
    fn live_levels_convert_integer_stereo_and_reset_for_silence() {
        let levels = super::AudioLevels::new();
        let mut recorded = Vec::new();
        super::measure_input(&[16384_i16, 16384, -16384, -16384], 2, &levels, |sample| {
            recorded.push(sample)
        });
        assert_eq!(levels.current(), (0.5, 0.5));
        assert_eq!(recorded, vec![0.5, -0.5]);
        super::measure_input(&[0.0_f32; 4], 2, &levels, |_| {});
        assert_eq!(levels.current(), (0.0, 0.0));
        super::measure_input(&[128_u8; 4], 2, &levels, |_| {});
        assert_eq!(levels.current(), (0.0, 0.0));
        super::measure_input(&[16384_i16, 16384], 2, &levels, |_| {});
        levels.set_paused(true);
        assert_eq!(levels.current(), (0.0, 0.0));
        let mut consumed = 0;
        super::measure_input(&[16384_i16, 16384], 2, &levels, |_| consumed += 1);
        assert_eq!((consumed, levels.current()), (0, (0.0, 0.0)));
        levels.set_paused(false);
        super::measure_input(&[16384_i16, 16384], 2, &levels, |_| consumed += 1);
        assert_eq!((consumed, levels.current()), (1, (0.5, 0.5)));
    }

    #[test]
    fn missing_input_chooses_builtin_then_default_then_first() {
        let devices = vec![
            super::InputDevice {
                id: "usb".into(),
                label: "USB microphone".into(),
            },
            super::InputDevice {
                id: "internal".into(),
                label: "Built-in microphone".into(),
            },
        ];
        assert_eq!(
            super::replacement_input(&devices, Some("usb")),
            Some("internal")
        );
        assert_eq!(
            super::replacement_input(&devices[..1], Some("usb")),
            Some("usb")
        );
        assert_eq!(
            super::replacement_input(&devices[..1], Some("gone")),
            Some("usb")
        );
        assert_eq!(super::replacement_input(&[], Some("gone")), None);
    }

    #[test]
    fn duration_uses_sample_count_and_rate() {
        let audio = AudioRecording::new(16_000, vec![0.0; 8_000]).unwrap();
        assert_eq!(audio.duration().as_secs_f64(), 0.5);
    }

    #[test]
    fn empty_recording_is_rejected() {
        assert!(AudioRecording::new(16_000, Vec::new()).is_err());
    }

    #[test]
    fn levels_report_rms_and_peak() {
        let audio = AudioRecording::new(16_000, vec![-0.5, 0.5]).unwrap();
        assert_eq!(audio.levels(), (0.5, 0.5));
    }

    #[test]
    fn recoverable_stream_notices_do_not_discard_recording() {
        assert!(is_recoverable_stream_error(cpal::ErrorKind::DeviceChanged));
        assert!(is_recoverable_stream_error(cpal::ErrorKind::RealtimeDenied));
        assert!(is_recoverable_stream_error(cpal::ErrorKind::Xrun));
        assert!(!is_recoverable_stream_error(
            cpal::ErrorKind::StreamInvalidated
        ));
    }

    #[test]
    fn slow_live_decoder_loses_no_chunks() {
        let input = vec![0.25; 2_048 * 100];
        let (mut producer, consumer) = HeapRb::<f32>::new(input.len()).split();
        producer.push_slice(&input);
        let (sender, receiver) = mpsc::channel();
        let recorded = collect_samples(consumer, Arc::new(AtomicBool::new(true)), Some(sender));
        let forwarded: Vec<f32> = receiver.try_iter().flatten().collect();
        assert_eq!(recorded, input);
        assert_eq!(forwarded, input);
    }

    #[test]
    fn collector_keeps_recovery_audio_and_forwards_the_same_samples() {
        let (mut producer, consumer) = HeapRb::<f32>::new(4).split();
        producer.push_slice(&[0.1, 0.2, 0.3]);
        let stop = Arc::new(AtomicBool::new(true));
        let (sender, receiver) = mpsc::channel();

        let recorded = collect_samples(consumer, stop, Some(sender));
        let forwarded = receiver.recv().unwrap();

        assert_eq!(
            (recorded, forwarded),
            (vec![0.1, 0.2, 0.3], vec![0.1, 0.2, 0.3])
        );
    }
}
