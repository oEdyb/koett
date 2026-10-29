use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, I24, Sample, SampleFormat, SizedSample, U24};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapProd, HeapRb};
use sherpa_onnx::Wave;

#[cfg(target_os = "linux")]
pub(crate) mod preview;

const MAX_RECORDING_DURATION: Duration = Duration::from_secs(10 * 60);

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
}

impl AudioLevels {
    fn new() -> Self {
        Self {
            rms: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
            peak: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
        }
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
    #[cfg(target_os = "linux")]
    _capture_priority: preview::RecordingPriority,
    stream: Option<cpal::Stream>,
    collector: Option<thread::JoinHandle<Vec<f32>>>,
    stop_collector: Arc<AtomicBool>,
    sample_rate: i32,
    started: Instant,
    maximum_duration: Duration,
    overflowed: Arc<AtomicBool>,
    stream_error: Arc<Mutex<Option<String>>>,
    levels: AudioLevels,
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
        let (sender, receiver) = mpsc::sync_channel(64);
        let recorder = Self::start_inner(MAX_RECORDING_DURATION, Some(sender), microphone)?;
        Ok((recorder, receiver))
    }

    fn start_inner(
        maximum_duration: Duration,
        audio_chunks: Option<SyncSender<Vec<f32>>>,
        microphone: Option<&str>,
    ) -> Result<Self, String> {
        if maximum_duration.is_zero() || maximum_duration > MAX_RECORDING_DURATION {
            return Err("maximum recording duration must be between 0 and 600 seconds".to_string());
        }

        #[cfg(target_os = "linux")]
        let capture_priority = preview::RecordingPriority::acquire()?;
        let host = cpal::default_host();
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
        let config = supported.config();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate as usize;
        let capacity = sample_rate * 2;
        let (producer, consumer) = HeapRb::<f32>::new(capacity).split();
        let overflowed = Arc::new(AtomicBool::new(false));
        let stream_error = Arc::new(Mutex::new(None));
        let levels = AudioLevels::new();
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

        let stream = match supported.sample_format() {
            SampleFormat::I8 => build_stream::<i8>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::I16 => build_stream::<i16>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::I24 => build_stream::<I24>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::I32 => build_stream::<i32>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::I64 => build_stream::<i64>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::U8 => build_stream::<u8>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::U16 => build_stream::<u16>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::U24 => build_stream::<U24>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::U32 => build_stream::<u32>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::U64 => build_stream::<u64>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::F32 => build_stream::<f32>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            SampleFormat::F64 => build_stream::<f64>(
                &device,
                &config,
                channels,
                producer,
                overflowed.clone(),
                stream_error.clone(),
                levels.clone(),
            ),
            format => return Err(format!("unsupported microphone sample format: {format}")),
        }?;

        stream
            .play()
            .map_err(|error| format!("could not start the microphone: {error}"))?;
        let collector_stop = stop_collector.clone();
        let collector_overflow = overflowed.clone();
        let collector = thread::Builder::new()
            .name("koett-audio-collector".to_string())
            .spawn(move || {
                collect_samples(consumer, collector_stop, audio_chunks, collector_overflow)
            })
            .map_err(|error| format!("could not start the microphone collector: {error}"))?;

        Ok(Self {
            #[cfg(target_os = "linux")]
            _capture_priority: capture_priority,
            stream: Some(stream),
            collector: Some(collector),
            stop_collector,
            sample_rate: sample_rate as i32,
            started: Instant::now(),
            maximum_duration,
            overflowed,
            stream_error,
            levels,
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
        self.stream.take();
        self.stop_collector.store(true, Ordering::Release);
        let samples = self
            .collector
            .take()
            .expect("a live recorder always has a collector")
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
        } else if self.started.elapsed() > self.maximum_duration + Duration::from_millis(250) {
            Some("Recording exceeded the ten-minute limit".into())
        } else {
            None
        };

        let recording = AudioRecording::new(self.sample_rate, samples)?;
        let (rms, peak) = recording.levels();
        eprintln!("microphone_rms={rms:.6} microphone_peak={peak:.6}");
        Ok((recording, warning))
    }
}

impl Drop for MicrophoneRecorder {
    fn drop(&mut self) {
        self.stream.take();
        self.stop_collector.store(true, Ordering::Release);
        if let Some(collector) = self.collector.take() {
            let _ = collector.join();
        }
    }
}

pub fn capture_default_microphone(duration: Duration) -> Result<AudioRecording, String> {
    if duration.is_zero() || duration > MAX_RECORDING_DURATION {
        return Err("microphone duration must be between 0 and 600 seconds".to_string());
    }
    let recorder = MicrophoneRecorder::start()?;
    thread::sleep(duration);
    recorder.finish()
}

fn collect_samples(
    mut consumer: ringbuf::HeapCons<f32>,
    stop: Arc<AtomicBool>,
    mut audio_chunks: Option<SyncSender<Vec<f32>>>,
    overflowed: Arc<AtomicBool>,
) -> Vec<f32> {
    let mut samples = Vec::new();
    let mut chunk = [0.0_f32; 2_048];
    loop {
        let count = consumer.pop_slice(&mut chunk);
        samples.extend_from_slice(&chunk[..count]);
        if count > 0
            && let Some(sender) = &audio_chunks
            && sender.try_send(chunk[..count].to_vec()).is_err()
        {
            overflowed.store(true, Ordering::Release);
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

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut producer: HeapProd<f32>,
    overflowed: Arc<AtomicBool>,
    stream_error: Arc<Mutex<Option<String>>>,
    levels: AudioLevels,
) -> Result<cpal::Stream, String>
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            *config,
            move |data: &[T], _| {
                measure_input(data, channels, &levels, |sample| {
                    if producer.try_push(sample).is_err() {
                        overflowed.store(true, Ordering::Relaxed);
                    }
                });
            },
            move |error| {
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
                    if let Ok(mut stored) = stream_error.lock()
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

fn measure_input<T>(data: &[T], channels: usize, levels: &AudioLevels, mut consume: impl FnMut(f32))
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
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
    let Some(selected) = selected else {
        return Ok(None);
    };
    let host = cpal::default_host();
    let id = selected
        .parse::<cpal::DeviceId>()
        .map_err(|e| format!("Invalid microphone ID: {e}"))?;
    let devices = input_devices()?;
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

pub fn input_devices() -> Result<Vec<InputDevice>, String> {
    cpal::default_host()
        .input_devices()
        .map_err(|e| e.to_string())?
        .map(|device| {
            Ok(InputDevice {
                id: device.id().map_err(|e| e.to_string())?.to_string(),
                label: device.description().map_err(|e| e.to_string())?.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    use ringbuf::HeapRb;
    use ringbuf::traits::{Producer, Split};

    use super::{AudioRecording, collect_samples, is_recoverable_stream_error};

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
    fn streaming_queue_overflow_preserves_captured_audio() {
        let (mut producer, consumer) = HeapRb::<f32>::new(4).split();
        producer.push_slice(&[0.1, 0.2, 0.3]);
        let (sender, _receiver) = mpsc::sync_channel(0);
        let overflow = Arc::new(AtomicBool::new(false));
        let audio = collect_samples(
            consumer,
            Arc::new(AtomicBool::new(true)),
            Some(sender),
            overflow.clone(),
        );
        assert_eq!(audio, vec![0.1, 0.2, 0.3]);
        assert!(overflow.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn collector_keeps_recovery_audio_and_forwards_the_same_samples() {
        let (mut producer, consumer) = HeapRb::<f32>::new(4).split();
        producer.push_slice(&[0.1, 0.2, 0.3]);
        let stop = Arc::new(AtomicBool::new(true));
        let (sender, receiver) = mpsc::sync_channel(64);

        let recorded = collect_samples(
            consumer,
            stop,
            Some(sender),
            Arc::new(AtomicBool::new(false)),
        );
        let forwarded = receiver.recv().unwrap();

        assert_eq!(
            (recorded, forwarded),
            (vec![0.1, 0.2, 0.3], vec![0.1, 0.2, 0.3])
        );
    }
}
