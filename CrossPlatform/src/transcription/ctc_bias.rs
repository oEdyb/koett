use super::ParakeetTranscriber;
use crate::dictionary::{self, DictionaryEntry};
use std::{
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant},
};

pub const BUDGET: Duration = Duration::from_millis(250);
const LOADING: u8 = 0;
const READY: u8 = 1;
const BUSY: u8 = 2;
const FAILED: u8 = 3;
const MAX_SAMPLES: usize = 1_000_000;
static WORKER: OnceLock<Option<Worker>> = OnceLock::new();

struct Job {
    samples: Vec<f32>,
    sample_rate: i32,
    raw: String,
    dictionary: Vec<DictionaryEntry>,
    deadline: Instant,
    reply: mpsc::Sender<Option<String>>,
}
struct Worker {
    sender: SyncSender<Job>,
    state: Arc<AtomicU8>,
}

pub(super) struct Sidecar {
    dictionary: Vec<DictionaryEntry>,
}

pub fn status() -> &'static str {
    match WORKER.get() {
        None => "inactive; requires dictionary terms and a downloaded Parakeet v2 model",
        Some(None) => "unavailable; raw CTC fallback",
        Some(Some(worker)) => match worker.state.load(Ordering::Acquire) {
            LOADING => "loading; raw CTC fallback",
            READY => "cached worker ready; 110M dictionary requests have a 250 ms deadline",
            BUSY => "busy; raw CTC fallback for overlapping requests",
            _ => "failed; raw CTC fallback until restart",
        },
    }
}

impl Sidecar {
    pub fn new(dictionary: &[DictionaryEntry]) -> Option<Self> {
        if super::hotwords::hotwords(dictionary).is_empty() {
            return None;
        }
        let path = std::env::var_os("KOETT_CTC_BIAS_MODEL")
            .map(PathBuf::from)
            .or_else(|| crate::model::LocalModel::ParakeetV2.directory().ok())?;
        if WORKER.get().is_none() && !path.join("encoder.int8.onnx").is_file() {
            return None;
        }
        WORKER.get_or_init(|| Worker::start(path, dictionary.to_vec()));
        Some(Self {
            dictionary: dictionary.to_vec(),
        })
    }

    pub fn refine(&self, raw: &str, sample_rate: i32, samples: &[f32]) -> String {
        let deadline = Instant::now() + BUDGET;
        if raw.trim().is_empty() || samples.len() > MAX_SAMPLES || sample_rate <= 0 {
            return raw.into();
        }
        let Some(Some(worker)) = WORKER.get() else {
            return raw.into();
        };
        if worker
            .state
            .compare_exchange(READY, BUSY, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return raw.into();
        }
        let (reply, result) = mpsc::channel();
        let job = Job {
            samples: samples.to_vec(),
            sample_rate,
            raw: raw.into(),
            dictionary: self.dictionary.clone(),
            deadline,
            reply,
        };
        if worker.sender.try_send(job).is_err() {
            worker.state.store(FAILED, Ordering::Release);
            return raw.into();
        }
        wait(result, deadline).unwrap_or_else(|| raw.into())
    }
}

fn wait(result: mpsc::Receiver<Option<String>>, deadline: Instant) -> Option<String> {
    if Instant::now() >= deadline {
        return None;
    }
    let result = result
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .ok()
        .flatten();
    if Instant::now() < deadline {
        result
    } else {
        None
    }
}

impl Worker {
    fn start(path: PathBuf, dictionary: Vec<DictionaryEntry>) -> Option<Self> {
        let (sender, jobs) = mpsc::sync_channel::<Job>(1);
        let state = Arc::new(AtomicU8::new(LOADING));
        let progress = state.clone();
        std::thread::Builder::new()
            .name("koett-ctc-dictionary".into())
            .spawn(move || {
                struct FailedOnExit(Arc<AtomicU8>);
                impl Drop for FailedOnExit {
                    fn drop(&mut self) {
                        self.0.store(FAILED, Ordering::Release);
                    }
                }
                let _failure = FailedOnExit(progress.clone());
                if crate::model::validate_local_model(crate::model::LocalModel::ParakeetV2, &path)
                    .is_err()
                {
                    return;
                }
                let Ok(mut decoder) =
                    ParakeetTranscriber::load_with_dictionary(&path, 4, &dictionary)
                else {
                    return;
                };
                let mut current = dictionary;
                progress.store(READY, Ordering::Release);
                while let Ok(job) = jobs.recv() {
                    let result = (|| {
                        if Instant::now() >= job.deadline {
                            return None;
                        }
                        if current != job.dictionary {
                            decoder.set_dictionary(&job.dictionary).ok()?;
                            current = job.dictionary.clone();
                        }
                        let (candidate, _) = decoder
                            .decode_timed_text(job.sample_rate, &job.samples, 0.0)
                            .ok()?;
                        if Instant::now() >= job.deadline {
                            return None;
                        }
                        let adopted =
                            dictionary::adopt_terms(&job.raw, &candidate, &job.dictionary);
                        (Instant::now() < job.deadline).then_some(adopted)
                    })();
                    let _ = job.reply.send(result);
                    progress.store(READY, Ordering::Release);
                }
            })
            .ok()?;
        Some(Self { sender, state })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_failed_and_disconnected_results_cannot_replace_raw() {
        let (send, receive) = mpsc::channel();
        let started = Instant::now();
        assert_eq!(wait(receive, started + Duration::from_millis(10)), None);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(send.send(Some("late term".into())).is_err());
        let (send, receive) = mpsc::channel();
        send.send(Some("queued but expired".into())).unwrap();
        assert_eq!(wait(receive, Instant::now()), None);
        let (send, receive) = mpsc::channel();
        send.send(Some("on time".into())).unwrap();
        assert_eq!(
            wait(receive, Instant::now() + BUDGET),
            Some("on time".into())
        );
        let (send, receive) = mpsc::channel();
        send.send(None).unwrap();
        assert_eq!(wait(receive, Instant::now() + BUDGET), None);
        let (send, receive) = mpsc::channel();
        drop(send);
        assert_eq!(wait(receive, Instant::now() + BUDGET), None);
    }
}
