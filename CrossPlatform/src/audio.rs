use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, I24, Sample, SampleFormat, SizedSample, U24};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapProd, HeapRb};
use sherpa_onnx::Wave;

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
        if maximum_duration.is_zero() || maximum_duration > MAX_RECORDING_DURATION {
            return Err("maximum recording duration must be between 0 and 600 seconds".to_string());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "no default microphone is available".to_string())?;
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
        let collector_stop = stop_collector.clone();
        let collector = thread::spawn(move || collect_samples(consumer, collector_stop));

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

        Ok(Self {
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

    pub fn finish(mut self) -> Result<AudioRecording, String> {
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
        if let Some(error) = stream_error {
            return Err(format!(
                "the microphone stream failed; the recording is incomplete: {error}"
            ));
        }
        if self.overflowed.load(Ordering::Relaxed) {
            return Err("microphone buffer overflowed; the recording is incomplete".to_string());
        }
        if self.started.elapsed() > self.maximum_duration + Duration::from_millis(250) {
            return Err("the recording exceeded the ten-minute limit".to_string());
        }

        let recording = AudioRecording::new(self.sample_rate, samples)?;
        let (rms, peak) = recording.levels();
        eprintln!("microphone_rms={rms:.6} microphone_peak={peak:.6}");
        Ok(recording)
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

fn collect_samples(mut consumer: ringbuf::HeapCons<f32>, stop: Arc<AtomicBool>) -> Vec<f32> {
    let mut samples = Vec::new();
    let mut chunk = [0.0_f32; 2_048];
    loop {
        let count = consumer.pop_slice(&mut chunk);
        samples.extend_from_slice(&chunk[..count]);
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
                let mut square_sum = 0.0_f32;
                let mut peak = 0.0_f32;
                let mut frame_count = 0_usize;
                for frame in data.chunks_exact(channels) {
                    let sample =
                        frame.iter().copied().map(f32::from_sample).sum::<f32>() / channels as f32;
                    square_sum += sample * sample;
                    peak = peak.max(sample.abs());
                    frame_count += 1;
                    if producer.try_push(sample).is_err() {
                        overflowed.store(true, Ordering::Relaxed);
                    }
                }
                if frame_count > 0 {
                    levels.update((square_sum / frame_count as f32).sqrt(), peak);
                }
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

fn is_recoverable_stream_error(kind: cpal::ErrorKind) -> bool {
    matches!(
        kind,
        cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied | cpal::ErrorKind::Xrun
    )
}

#[cfg(test)]
mod tests {
    use super::{AudioRecording, is_recoverable_stream_error};

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
}
