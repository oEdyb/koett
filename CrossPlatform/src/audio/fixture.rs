use super::*;

#[cfg(target_os = "windows")]
impl MicrophoneRecorder {
    pub(crate) fn start_wav_fixture(path: &Path) -> Result<(Self, Receiver<Vec<f32>>), String> {
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 20 * 1024 * 1024 {
            return Err("WAV lab input exceeds the 20 MiB limit".into());
        }
        let audio = AudioRecording::read_wav(path)?;
        if audio.sample_rate != 16_000 || audio.duration() > MAX_RECORDING_DURATION {
            return Err("WAV lab input must be 16 kHz mono PCM, at most ten minutes".into());
        }
        let priority = preview::RecordingPriority::acquire()?;
        let levels = AudioLevels::new();
        let stop = Arc::new(AtomicBool::new(false));
        let (send, receive) = mpsc::channel();
        let worker_levels = levels.clone();
        let worker_stop = stop.clone();
        let collector = thread::spawn(move || {
            feed(
                audio,
                send,
                worker_stop,
                worker_levels,
                Duration::from_millis(20),
            )
        });
        Ok((
            Self {
                _capture_priority: priority,
                stream: None,
                collector: Some(collector),
                stop_collector: stop,
                sample_rate: 16_000,
                overflowed: Arc::new(AtomicBool::new(false)),
                stream_error: Arc::new(Mutex::new(None)),
                levels,
            },
            receive,
        ))
    }
}
fn feed(
    audio: AudioRecording,
    send: Sender<Vec<f32>>,
    stop: Arc<AtomicBool>,
    levels: AudioLevels,
    interval: Duration,
) -> Vec<f32> {
    let mut recorded = Vec::with_capacity(audio.samples.len());
    for chunk in audio.samples.chunks(320) {
        if stop.load(Ordering::Acquire) {
            break;
        }
        if !levels.paused() {
            measure_input(chunk, 1, &levels, |sample| recorded.push(sample));
            if send.send(chunk.to_vec()).is_err() {
                break;
            }
        }
        thread::sleep(interval);
    }
    levels.update(0.0, 0.0);
    recorded
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_streams_real_samples_once_in_order() {
        let samples: Vec<f32> = (0..700).map(|i| i as f32 / 1000.0).collect();
        let audio = AudioRecording::new(16_000, samples.clone()).unwrap();
        let (send, receive) = mpsc::channel();
        let recorded = feed(
            audio,
            send,
            Arc::new(AtomicBool::new(false)),
            AudioLevels::new(),
            Duration::ZERO,
        );
        assert_eq!(recorded, samples);
        assert_eq!(receive.into_iter().flatten().collect::<Vec<_>>(), samples);
    }
    #[test]
    fn paused_or_cancelled_fixture_keeps_and_sends_nothing() {
        for paused in [false, true] {
            let (send, receive) = mpsc::channel();
            let levels = AudioLevels::new();
            levels.set_paused(paused);
            let recorded = feed(
                AudioRecording::new(16_000, vec![0.5; 1000]).unwrap(),
                send,
                Arc::new(AtomicBool::new(!paused)),
                levels.clone(),
                Duration::ZERO,
            );
            assert!(recorded.is_empty());
            assert_eq!(receive.into_iter().count(), 0);
            assert_eq!(levels.current(), (0.0, 0.0));
        }
    }
}
