use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, I24, Sample, SampleFormat, SizedSample, U24};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapProd, HeapRb};
use sherpa_onnx::Wave;

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

pub fn capture_default_microphone(duration: Duration) -> Result<AudioRecording, String> {
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
    let capacity = ((duration.as_secs_f64() + 1.0) * sample_rate as f64) as usize;
    let (producer, mut consumer) = HeapRb::<f32>::new(capacity).split();
    let overflowed = Arc::new(AtomicBool::new(false));
    let stream_failed = Arc::new(AtomicBool::new(false));

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
            stream_failed.clone(),
        ),
        SampleFormat::I16 => build_stream::<i16>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::I24 => build_stream::<I24>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::I32 => build_stream::<i32>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::I64 => build_stream::<i64>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::U8 => build_stream::<u8>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::U16 => build_stream::<u16>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::U24 => build_stream::<U24>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::U32 => build_stream::<u32>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::U64 => build_stream::<u64>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::F32 => build_stream::<f32>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        SampleFormat::F64 => build_stream::<f64>(
            &device,
            &config,
            channels,
            producer,
            overflowed.clone(),
            stream_failed.clone(),
        ),
        format => return Err(format!("unsupported microphone sample format: {format}")),
    }?;

    stream
        .play()
        .map_err(|error| format!("could not start the microphone: {error}"))?;
    thread::sleep(duration);
    drop(stream);

    if stream_failed.load(Ordering::Relaxed) {
        return Err("the microphone stream failed; the recording is incomplete".to_string());
    }
    if overflowed.load(Ordering::Relaxed) {
        return Err("microphone buffer overflowed; the recording is incomplete".to_string());
    }

    let mut samples = vec![0.0; consumer.occupied_len()];
    let count = consumer.pop_slice(&mut samples);
    samples.truncate(count);
    let recording = AudioRecording::new(sample_rate as i32, samples)?;
    let (rms, peak) = recording.levels();
    eprintln!("microphone_rms={rms:.6} microphone_peak={peak:.6}");
    Ok(recording)
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut producer: HeapProd<f32>,
    overflowed: Arc<AtomicBool>,
    stream_failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            *config,
            move |data: &[T], _| {
                let frames = data.chunks_exact(channels);
                let frame_count = frames.len();
                let pushed = producer.push_iter(frames.map(|frame| {
                    frame.iter().copied().map(f32::from_sample).sum::<f32>() / channels as f32
                }));
                if pushed != frame_count {
                    overflowed.store(true, Ordering::Relaxed);
                }
            },
            move |_| stream_failed.store(true, Ordering::Relaxed),
            None,
        )
        .map_err(|error| format!("could not create the microphone stream: {error}"))
}

#[cfg(test)]
mod tests {
    use super::AudioRecording;

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
}
