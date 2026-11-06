//! Recording start and stop cues, matching the Mac: a high tick when capture has
//! begun and a low tone when it ends. They are synthesized here because the Mac uses
//! macOS system sounds, which Koett can not ship. Playback runs on its own short-lived
//! thread, so a slow or missing output device never delays recording or transcription.
use crate::state::AppStatus;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, I24, SampleFormat, SizedSample, U24};
use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Cue {
    Start,
    Stop,
}

/// Which cue a status change plays. Cancel returns straight to Ready and stays
/// silent, as on the Mac; losing the microphone ends in Error and plays the stop.
pub(super) fn for_change(old: &AppStatus, new: &AppStatus) -> Option<Cue> {
    match (old, new) {
        (AppStatus::Recording, AppStatus::Recording) => None,
        (_, AppStatus::Recording) => Some(Cue::Start),
        (AppStatus::Recording, AppStatus::Transcribing | AppStatus::Error(_)) => Some(Cue::Stop),
        _ => None,
    }
}

/// Mono samples for a cue at a sample rate.
fn tone(cue: Cue, rate: u32) -> Vec<f32> {
    // (seconds, fundamental Hz, decay seconds, attack seconds, peak)
    let (length, pitch, decay, attack, peak) = match cue {
        Cue::Start => (0.14, 2093.0, 0.035, 0.002, 0.35),
        Cue::Stop => (0.26, 196.0, 0.07, 0.005, 0.45),
    };
    let rate = rate as f32;
    let count = (length * rate) as usize;
    let fade = 0.02 * rate;
    (0..count)
        .map(|index| {
            let time = index as f32 / rate;
            let wave = (TAU * pitch * time).sin() + 0.35 * (TAU * pitch * 2.0 * time).sin();
            let rise = (time / attack).min(1.0);
            let tail = ((count - index) as f32 / fade).min(1.0);
            peak / 1.35 * wave * (-time / decay).exp() * rise * tail
        })
        .collect()
}

pub(super) fn play(cue: Cue) {
    let spawned = std::thread::Builder::new()
        .name("koett-cue".into())
        .spawn(move || {
            if let Err(error) = play_now(cue) {
                eprintln!("Recording sound unavailable: {error}");
            }
        });
    if let Err(error) = spawned {
        eprintln!("Recording sound unavailable: {error}");
    }
}

fn play_now(cue: Cue) -> Result<(), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no output device")?;
    let config = device
        .default_output_config()
        .map_err(|error| error.to_string())?;
    let samples = Arc::new(tone(cue, config.sample_rate()));
    let length = samples.len() as f32 / config.sample_rate() as f32;
    let format = config.sample_format();
    let config = config.config();
    let stream = match format {
        SampleFormat::F32 => stream::<f32>(&device, &config, samples),
        SampleFormat::F64 => stream::<f64>(&device, &config, samples),
        SampleFormat::I16 => stream::<i16>(&device, &config, samples),
        SampleFormat::I24 => stream::<I24>(&device, &config, samples),
        SampleFormat::I32 => stream::<i32>(&device, &config, samples),
        SampleFormat::U8 => stream::<u8>(&device, &config, samples),
        SampleFormat::U16 => stream::<u16>(&device, &config, samples),
        SampleFormat::U24 => stream::<U24>(&device, &config, samples),
        SampleFormat::U32 => stream::<u32>(&device, &config, samples),
        other => return Err(format!("unsupported output format {other}")),
    }?;
    stream.play().map_err(|error| error.to_string())?;
    // Long enough for the device buffer to drain before the stream closes.
    std::thread::sleep(Duration::from_secs_f32(length) + Duration::from_millis(250));
    Ok(())
}

fn stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Vec<f32>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels).max(1);
    let position = AtomicUsize::new(0);
    device
        .build_output_stream(
            *config,
            move |output: &mut [T], _| {
                for frame in output.chunks_mut(channels) {
                    let index = position.fetch_add(1, Ordering::Relaxed);
                    let value = samples.get(index).copied().unwrap_or(0.0);
                    frame.fill(T::from_sample(value));
                }
            },
            |error| eprintln!("Recording sound stream failed: {error}"),
            None,
        )
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_follow_recording_like_the_mac() {
        use AppStatus::{Error, Ready, Recording, Starting, Transcribing};
        assert_eq!(for_change(&Ready, &Recording), Some(Cue::Start));
        assert_eq!(for_change(&Starting, &Recording), Some(Cue::Start));
        assert_eq!(for_change(&Recording, &Transcribing), Some(Cue::Stop));
        assert_eq!(
            for_change(&Recording, &Error("microphone lost".into())),
            Some(Cue::Stop)
        );
        assert_eq!(for_change(&Recording, &Ready), None, "cancel is silent");
        assert_eq!(
            for_change(&Recording, &Recording),
            None,
            "a repeated status"
        );
        assert_eq!(for_change(&Transcribing, &Ready), None);
        assert_eq!(for_change(&Ready, &Transcribing), None);
    }

    #[test]
    fn tones_are_short_quiet_and_free_of_clicks() {
        for (cue, longest) in [(Cue::Start, 0.15), (Cue::Stop, 0.27)] {
            for rate in [44_100, 48_000] {
                let samples = tone(cue, rate);
                assert!((samples.len() as f32) < longest * rate as f32);
                let peak = samples.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
                assert!(peak > 0.15 && peak <= 0.5, "{cue:?} peak {peak}");
                assert!(samples[0].abs() < 0.01, "starts at silence");
                assert!(samples.last().unwrap().abs() < 0.01, "ends at silence");
            }
        }
    }

    #[test]
    fn the_start_is_high_and_the_stop_is_low() {
        let crossings = |samples: &[f32], rate: f32| {
            let flips = samples.windows(2).filter(|w| w[0] * w[1] < 0.0).count() as f32;
            flips / (samples.len() as f32 / rate)
        };
        let high = crossings(&tone(Cue::Start, 48_000), 48_000.0);
        let low = crossings(&tone(Cue::Stop, 48_000), 48_000.0);
        assert!(high > low * 4.0, "{high} vs {low}");
    }
}
