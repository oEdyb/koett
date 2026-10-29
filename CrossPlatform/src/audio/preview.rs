use super::{AudioLevels, is_recoverable_stream_error, measure_input};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, I24, Sample, SampleFormat, SizedSample, U24};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

static RECORDERS: AtomicUsize = AtomicUsize::new(0);
static PREVIEW: Mutex<()> = Mutex::new(());

pub(crate) struct RecordingPriority;

impl RecordingPriority {
    pub(crate) fn acquire() -> Result<Self, String> {
        RECORDERS.fetch_add(1, Ordering::SeqCst);
        let priority = Self;
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match PREVIEW.try_lock() {
                Ok(guard) => {
                    drop(guard);
                    return Ok(priority);
                }
                Err(TryLockError::Poisoned(_)) => {
                    return Err("Microphone preview failed. Close Settings and retry.".into());
                }
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => {
                    return Err("Microphone preview is still closing. Retry recording.".into());
                }
            }
        }
    }
}

impl Drop for RecordingPriority {
    fn drop(&mut self) {
        RECORDERS.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(crate) fn recording() -> bool {
    RECORDERS.load(Ordering::SeqCst) != 0
}

pub(crate) struct Capture {
    // Drop the stream before releasing ownership to a waiting recorder.
    _stream: cpal::Stream,
    _ownership: MutexGuard<'static, ()>,
    levels: AudioLevels,
    error: Arc<Mutex<Option<String>>>,
    last_callback: Arc<Mutex<Instant>>,
}

impl Capture {
    pub(crate) fn start(
        microphone: Option<&str>,
        current: impl Fn() -> bool,
    ) -> Result<Option<Self>, String> {
        let ownership = match PREVIEW.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Poisoned(_)) => return Err("Microphone preview unavailable".into()),
        };
        if recording() || !current() {
            return Ok(None);
        }
        let host = cpal::default_host();
        let device = match microphone {
            None => host
                .default_input_device()
                .ok_or("No default microphone available")?,
            Some(id) => host
                .device_by_id(
                    &id.parse::<cpal::DeviceId>()
                        .map_err(|_| "Selected microphone unavailable")?,
                )
                .ok_or("Selected microphone unavailable")?,
        };
        let supported = device
            .default_input_config()
            .map_err(|error| format!("Cannot read microphone format: {error}"))?;
        let config = supported.config();
        let levels = AudioLevels::new();
        let error = Arc::new(Mutex::new(None));
        let last_callback = Arc::new(Mutex::new(Instant::now()));
        macro_rules! build {
            ($sample:ty) => {
                build_levels::<$sample>(
                    &device,
                    config,
                    levels.clone(),
                    error.clone(),
                    last_callback.clone(),
                )
            };
        }
        let stream = match supported.sample_format() {
            SampleFormat::I8 => build!(i8),
            SampleFormat::I16 => build!(i16),
            SampleFormat::I24 => build!(I24),
            SampleFormat::I32 => build!(i32),
            SampleFormat::I64 => build!(i64),
            SampleFormat::U8 => build!(u8),
            SampleFormat::U16 => build!(u16),
            SampleFormat::U24 => build!(U24),
            SampleFormat::U32 => build!(u32),
            SampleFormat::U64 => build!(u64),
            SampleFormat::F32 => build!(f32),
            SampleFormat::F64 => build!(f64),
            format => return Err(format!("Unsupported microphone format: {format}")),
        }
        .map_err(|error| format!("Cannot open microphone: {error}"))?;
        if recording() || !current() {
            return Ok(None);
        }
        stream
            .play()
            .map_err(|error| format!("Cannot start microphone: {error}"))?;
        Ok(Some(Self {
            _stream: stream,
            _ownership: ownership,
            levels,
            error,
            last_callback,
        }))
    }

    pub(crate) fn levels(&self) -> Result<(f32, f32), String> {
        if let Some(error) = self
            .error
            .lock()
            .map_err(|_| "Microphone preview failed")?
            .as_ref()
        {
            return Err(format!("Microphone disconnected: {error}"));
        }
        if self
            .last_callback
            .lock()
            .map_err(|_| "Microphone preview failed")?
            .elapsed()
            > Duration::from_millis(500)
        {
            return Ok((0.0, 0.0));
        }
        Ok(self.levels.current())
    }
}

fn build_levels<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    levels: AudioLevels,
    error: Arc<Mutex<Option<String>>>,
    last_callback: Arc<Mutex<Instant>>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            measure_input(data, usize::from(config.channels), &levels, |_| {});
            if let Ok(mut last) = last_callback.lock() {
                *last = Instant::now();
            }
        },
        move |failure| {
            if !is_recoverable_stream_error(failure.kind())
                && let Ok(mut stored) = error.lock()
            {
                *stored = Some(failure.to_string());
            }
        },
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the private koett-last-recovery agent-lab"]
    fn missing_selected_preview_never_uses_recording_fallback() {
        assert!(
            std::env::var("AGENT_LAB_RUNTIME")
                .unwrap()
                .ends_with("/koett-last-recovery")
        );
        let result = Capture::start(Some("alsa:nonexistent-private-fixture-device"), || true);
        assert!(matches!(result, Err(error) if error == "Selected microphone unavailable"));
        assert!(!recording());
    }

    #[test]
    fn recording_waits_for_preview_release_and_blocks_preview_until_dropped() {
        let ownership = PREVIEW.lock().unwrap();
        let worker = std::thread::spawn(RecordingPriority::acquire);
        let deadline = Instant::now() + Duration::from_secs(1);
        while !recording() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(recording());
        drop(ownership);
        let priority = worker.join().unwrap().unwrap();
        assert!(Capture::start(None, || true).unwrap().is_none());
        drop(priority);
        assert!(!recording());
        let ownership = PREVIEW.lock().unwrap();
        let started = Instant::now();
        assert!(RecordingPriority::acquire().is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(!recording());
        drop(ownership);
    }
}
