use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::process::{ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

use crate::audio;
use crate::meeting::capture::{CapturePlatform, SourceEpoch, Timing, TrackIngress};
use crate::meeting::{Channel, SAMPLE_RATE, SourceHealth};

use super::child::ChildGroup;

const RETRY_MIN: Duration = Duration::from_millis(250);
const RETRY_MAX: Duration = Duration::from_secs(2);
const SOURCE_POLL: Duration = Duration::from_millis(50);

type Health = Arc<dyn Fn(Channel, SourceHealth) + Send + Sync>;

pub(crate) struct LinuxCapture {
    mic: MicInput,
    microphone: Option<String>,
    health: Health,
}

impl LinuxCapture {
    pub(crate) fn new(microphone: Option<String>, health: Health) -> io::Result<Self> {
        Ok(Self {
            mic: MicInput::resolve(microphone.as_deref())?,
            microphone,
            health,
        })
    }

    pub(crate) fn mic_rate(&self) -> u32 {
        self.mic.config.sample_rate
    }
}

impl CapturePlatform for LinuxCapture {
    type Source = Source;

    fn arm(&mut self, channel: Channel, ingress: TrackIngress) -> io::Result<Source> {
        let health = self.health.clone();
        match channel {
            Channel::You => {
                let detached = Arc::new(AtomicBool::new(false));
                let (return_ingress, returned) = mpsc::channel();
                let stream = self.mic.start(
                    IngressLease {
                        ingress: Some(ingress),
                        returned: return_ingress.clone(),
                    },
                    detached.clone(),
                )?;
                let rate = self.mic_rate();
                let microphone = self.microphone.clone();
                Source::spawn("meeting-mic", move |stop| {
                    supervise_mic(
                        stream,
                        rate,
                        microphone,
                        detached,
                        returned,
                        return_ingress,
                        health,
                        stop,
                    )
                })
            }
            Channel::Other => {
                let monitor = Monitor::start()?;
                Source::spawn("meeting-monitor", move |stop| {
                    supervise_monitor(monitor, ingress, health, stop)
                })
            }
        }
    }
}

pub(crate) struct Source {
    stop: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl Source {
    fn spawn(name: &str, run: impl FnOnce(Receiver<()>) + Send + 'static) -> io::Result<Self> {
        let (stop, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name(name.into())
            .spawn(move || run(receiver))?;
        Ok(Self {
            stop: Some(stop),
            worker: Some(worker),
        })
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct MicInput {
    device: cpal::Device,
    config: cpal::StreamConfig,
    format: SampleFormat,
}

impl MicInput {
    fn resolve(selected: Option<&str>) -> io::Result<Self> {
        let selected = audio::resolve_recording_input(selected).map_err(io::Error::other)?;
        let host = cpal::default_host();
        let device = match selected {
            Some(id) => host.device_by_id(&id.parse().map_err(io::Error::other)?),
            None => host.default_input_device(),
        }
        .ok_or_else(|| io::Error::other("No recording microphone is available"))?;
        let config = device.default_input_config().map_err(io::Error::other)?;
        Ok(Self {
            device,
            config: config.config(),
            format: config.sample_format(),
        })
    }

    fn start(&self, ingress: IngressLease, detached: Arc<AtomicBool>) -> io::Result<cpal::Stream> {
        macro_rules! build {
            ($($format:ident => $ty:ty),+ $(,)?) => {
                match self.format {
                    $(SampleFormat::$format => self.build::<$ty>(ingress, detached),)+
                    format => Err(io::Error::other(format!("Unsupported microphone format: {format:?}"))),
                }
            };
        }
        let stream = build! {
            I8 => i8, I16 => i16, I24 => cpal::I24, I32 => i32, I64 => i64,
            U8 => u8, U16 => u16, U24 => cpal::U24, U32 => u32, U64 => u64,
            F32 => f32, F64 => f64,
        }?;
        stream.play().map_err(io::Error::other)?;
        Ok(stream)
    }

    fn build<T>(
        &self,
        mut ingress: IngressLease,
        detached: Arc<AtomicBool>,
    ) -> io::Result<cpal::Stream>
    where
        T: Sample + SizedSample,
        f32: FromSample<T>,
    {
        let channels = usize::from(self.config.channels);
        let mut pcm = vec![0.0; (self.config.sample_rate as usize).div_ceil(50)];
        self.device
            .build_input_stream(
                self.config,
                move |data: &[T], _| {
                    let at = Instant::now();
                    for block in data.chunks(pcm.len() * channels) {
                        let count = block.len() / channels;
                        for (dest, sample) in
                            pcm.iter_mut().zip(audio::mono_samples(block, channels))
                        {
                            *dest = sample;
                        }
                        ingress.push(&pcm[..count], at);
                    }
                },
                move |error| {
                    if !audio::is_recoverable_stream_error(error.kind()) {
                        detached.store(true, Ordering::Release);
                    }
                },
                None,
            )
            .map_err(io::Error::other)
    }
}

struct IngressLease {
    ingress: Option<TrackIngress>,
    returned: Sender<TrackIngress>,
}

impl IngressLease {
    fn push(&mut self, pcm: &[f32], at: Instant) {
        if let Some(ingress) = &mut self.ingress {
            ingress.push(pcm, at);
        }
    }
}

impl Drop for IngressLease {
    fn drop(&mut self) {
        if let Some(ingress) = self.ingress.take() {
            let _ = self.returned.send(ingress);
        }
    }
}

#[expect(clippy::too_many_arguments)]
fn supervise_mic(
    mut stream: cpal::Stream,
    rate: u32,
    microphone: Option<String>,
    detached: Arc<AtomicBool>,
    returned: Receiver<TrackIngress>,
    return_ingress: Sender<TrackIngress>,
    health: Health,
    stop: Receiver<()>,
) {
    let mut generation = 0;
    while matches!(
        stop.recv_timeout(SOURCE_POLL),
        Err(RecvTimeoutError::Timeout)
    ) {
        if !detached.swap(false, Ordering::AcqRel) {
            continue;
        }
        health(Channel::You, SourceHealth::Reconnecting);
        drop(stream);
        generation += 1;
        let mut retry = RETRY_MIN;
        stream = loop {
            if !matches!(stop.recv_timeout(retry), Err(RecvTimeoutError::Timeout)) {
                return;
            }
            if let Ok(input) = MicInput::resolve(microphone.as_deref())
                && input.config.sample_rate == rate
                && let Ok(mut ingress) = returned.recv()
            {
                ingress.begin_epoch(SourceEpoch {
                    generation,
                    timing: Timing::Callback,
                });
                let lease = IngressLease {
                    ingress: Some(ingress),
                    returned: return_ingress.clone(),
                };
                if let Ok(next) = input.start(lease, detached.clone()) {
                    health(Channel::You, SourceHealth::Receiving);
                    break next;
                }
            }
            retry = (retry * 2).min(RETRY_MAX);
        };
    }
}

struct Monitor {
    _group: ChildGroup,
    stdout: ChildStdout,
}

impl Monitor {
    fn start() -> io::Result<Self> {
        let mut group = ChildGroup::spawn_monitor(
            Command::new("pw-record")
                .args(["--raw", "--rate", "16000", "--channels", "1", "--format", "s16",
                    "--latency", "20ms", "-P",
                    r#"{ stream.capture.sink=true node.name=koett-meeting-other media.name="Koett meeting" }"#,
                    "-"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null()),
        ).map_err(|error| io::Error::new(error.kind(), format!("pw-record: {error}")))?;
        let stdout = group
            .take_stdout()
            .ok_or_else(|| io::Error::other("No pw-record output pipe"))?;
        Ok(Self {
            _group: group,
            stdout,
        })
    }

    fn read(&mut self, bytes: &mut [u8]) -> io::Result<Option<usize>> {
        let mut fd = libc::pollfd {
            fd: self.stdout.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut fd, 1, 50) };
        match ready {
            0 => Ok(None),
            -1 => {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
            _ => self.stdout.read(bytes).map(Some),
        }
    }
}

fn supervise_monitor(
    mut monitor: Monitor,
    mut ingress: TrackIngress,
    health: Health,
    stop: Receiver<()>,
) {
    let mut generation = 0;
    let mut retry = RETRY_MIN;
    loop {
        let mut bytes = [0; SAMPLE_RATE as usize / 25];
        let mut samples = [0.0; SAMPLE_RATE as usize / 50];
        let mut tail = 0;
        let mut receiving = generation == 0;
        while matches!(stop.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            let count = match monitor.read(&mut bytes[tail..]) {
                Ok(None) => continue,
                Ok(Some(0)) | Err(_) => break,
                Ok(Some(count)) => count + tail,
            };
            let at = Instant::now();
            let (pairs, rest) = bytes[..count].as_chunks::<2>();
            let frames = pairs.len();
            for (sample, &pair) in samples.iter_mut().zip(pairs) {
                *sample = f32::from(i16::from_le_bytes(pair)) / 32768.0;
            }
            tail = rest.len();
            if tail != 0 {
                bytes[0] = bytes[count - 1];
            }
            if frames > 0 {
                ingress.push(&samples[..frames], at);
                if !receiving {
                    health(Channel::Other, SourceHealth::Receiving);
                    receiving = true;
                }
                retry = RETRY_MIN;
            }
        }
        drop(monitor);
        health(Channel::Other, SourceHealth::Reconnecting);
        monitor = loop {
            if !matches!(stop.recv_timeout(retry), Err(RecvTimeoutError::Timeout)) {
                return;
            }
            retry = (retry * 2).min(RETRY_MAX);
            if let Ok(next) = Monitor::start() {
                generation += 1;
                ingress.begin_epoch(SourceEpoch {
                    generation,
                    timing: Timing::Arrival,
                });
                break next;
            }
        };
    }
}
