mod lead;
mod transport;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub const VOICES: [(&str, &str); 5] = [
    ("Katie", "f786b574-daa5-4673-aa0c-cbe3e8534c02"),
    ("Skylar", "db6b0ed5-d5d3-463d-ae85-518a07d3c2b4"),
    ("Jameson", "a5136bf9-224c-4d76-b823-52bd5efcffcc"),
    ("Gemma", "62ae83ad-4f6a-430b-af41-a9bede9286ca"),
    ("Archie", "ef191366-f52f-447a-a398-ed8c0f2943a1"),
];
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub enabled: bool,
    pub voice: String,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            enabled: false,
            voice: "Katie".into(),
        }
    }
}
impl Configuration {
    pub fn load() -> Result<Self, String> {
        let path = crate::paths::settings_file()?.with_file_name("speech.json");
        match std::fs::read(path) {
            Ok(b) => serde_json::from_slice(&b).map_err(|_| "Invalid speech configuration".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err("Cannot read speech configuration".into()),
        }
    }
    pub fn save(&self) -> Result<(), String> {
        self.voice_id()?;
        let path = crate::paths::settings_file()?.with_file_name("speech.json");
        std::fs::create_dir_all(path.parent().ok_or("Missing settings directory")?)
            .map_err(|_| "Cannot create settings directory")?;
        std::fs::write(
            path,
            serde_json::to_vec(self).map_err(|_| "Cannot encode speech settings")?,
        )
        .map_err(|_| "Cannot save speech settings".into())
    }
    pub fn voice_id(&self) -> Result<&'static str, String> {
        VOICES
            .iter()
            .find(|(name, _)| *name == self.voice)
            .map(|(_, id)| *id)
            .ok_or("Unknown speech voice".into())
    }
}
pub struct Speech {
    cancelled: Arc<AtomicBool>,
    audio: Arc<Mutex<std::collections::VecDeque<f32>>>,
    tx: tokio::sync::mpsc::Sender<Option<String>>,
    worker: Option<std::thread::JoinHandle<()>>,
    lead: lead::Lead,
    pub notices: mpsc::Receiver<String>,
    notice: mpsc::Sender<String>,
}
impl Speech {
    pub fn prepare(config: Configuration) -> Option<Self> {
        if !config.enabled {
            return None;
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let audio = Arc::new(Mutex::new(std::collections::VecDeque::new()));
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        let (notice, notices) = mpsc::channel();
        let stop = cancelled.clone();
        let queue = audio.clone();
        let failure_notice = notice.clone();
        let worker = std::thread::spawn(move || {
            let result = transport::run(
                config,
                stop.clone(),
                queue.clone(),
                rx,
                failure_notice.clone(),
            );
            if result.is_err() && !stop.load(Ordering::Acquire) {
                let _ =
                    failure_notice.send("Spoken reply unavailable; text answer continues".into());
            }
            if let Ok(mut q) = queue.lock() {
                q.clear();
            }
        });
        Some(Self {
            cancelled,
            audio,
            tx,
            worker: Some(worker),
            lead: lead::Lead::default(),
            notices,
            notice,
        })
    }
    pub fn append(&mut self, text: &str) {
        for sentence in self.lead.append(text, false) {
            if let Err(error) = self.tx.try_send(Some(sentence)) {
                if matches!(error, tokio::sync::mpsc::error::TrySendError::Full(_)) {
                    let _ = self
                        .notice
                        .send("Spoken reply stopped; speech queue is full".into());
                }
                self.stop();
                break;
            }
        }
    }
    pub fn finish(&mut self) {
        for sentence in self.lead.append("", true) {
            if self.tx.try_send(Some(sentence)).is_err() {
                self.stop();
                return;
            }
        }
        if self.tx.try_send(None).is_err() {
            self.stop();
        }
    }
    pub fn stop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Ok(mut q) = self.audio.lock() {
            q.clear();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Speech {
    fn drop(&mut self) {
        self.stop();
    }
}
