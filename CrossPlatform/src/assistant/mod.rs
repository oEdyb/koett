mod process;
pub mod screen;
pub mod speech;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
use std::time::Duration;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub enabled: bool,
    pub provider: String,
    pub endpoint: String,
    pub model: String,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "groq".into(),
            endpoint: "https://api.groq.com/openai/v1/chat/completions".into(),
            model: "qwen/qwen3.6-27b".into(),
        }
    }
}
impl Configuration {
    pub fn validate(&self) -> Result<(), String> {
        let url = url::Url::parse(&self.endpoint).map_err(|_| "Invalid Ask endpoint")?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err("Ask requires an HTTPS endpoint without credentials or fragment".into());
        }
        if !matches!(self.provider.as_str(), "groq" | "openRouter" | "custom")
            || self.model.trim().is_empty()
            || self.model.len() > 512
        {
            return Err("Invalid Ask provider or model".into());
        }
        Ok(())
    }
    pub fn load() -> Result<Self, String> {
        let path = crate::paths::settings_file()?.with_file_name("assistant.json");
        match std::fs::read(path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|_| "Invalid Ask configuration".into())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err("Cannot read Ask configuration".into()),
        }
    }
    pub fn save(&self) -> Result<(), String> {
        self.validate()?;
        let path = crate::paths::settings_file()?.with_file_name("assistant.json");
        std::fs::create_dir_all(path.parent().ok_or("Ask configuration directory missing")?)
            .map_err(|_| "Cannot create Ask configuration directory")?;
        std::fs::write(
            path,
            serde_json::to_vec_pretty(self).map_err(|_| "Cannot encode Ask configuration")?,
        )
        .map_err(|_| "Cannot save Ask configuration".into())
    }
}

pub fn secret(
    provider: &str,
    value: Option<&str>,
    cancelled: &AtomicBool,
) -> Result<String, String> {
    if !matches!(provider, "groq" | "openRouter" | "custom" | "cartesia") {
        return Err("Invalid Ask provider".into());
    }
    if value
        .is_some_and(|s| s.trim().is_empty() || s.len() > 8192 || s.chars().any(char::is_control))
    {
        return Err("Invalid API key".into());
    }
    let mut command = Command::new("/usr/bin/secret-tool");
    if value.is_some() {
        command.args(["store", "--label=Koett Ask"]);
    } else {
        command.arg("lookup");
    }
    command.args([
        "service",
        if provider == "cartesia" {
            "com.olledyberg.Koett.Speech"
        } else {
            "com.olledyberg.Koett.Ask"
        },
        "provider",
        provider,
    ]);
    let mut bytes = Vec::new();
    process::run(
        &mut command,
        value.unwrap_or("").as_bytes().to_vec(),
        cancelled,
        Duration::from_secs(15),
        8192,
        |chunk| {
            bytes.extend_from_slice(chunk);
            Ok(())
        },
    )
    .map_err(|_| {
        "Secret Service unavailable, locked, or key missing; no plaintext fallback".to_string()
    })?;
    let key = String::from_utf8(bytes).map_err(|_| "Invalid stored API key")?;
    if value.is_none() && (key.trim().is_empty() || key.trim().chars().any(char::is_control)) {
        return Err("Ask API key missing or invalid".into());
    }
    Ok(key.trim().into())
}

pub fn request(question: &str, image: &[u8], config: &Configuration) -> Value {
    let mut body = json!({"model": config.model, "temperature": 0.4, "stream": true,
        "messages": [{"role": "system", "content": include_str!("prompt.txt")}, {"role": "user", "content": [
            {"type": "text", "text": question}, {"type": "image_url", "image_url": {"url": format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(image))}}]}]});
    if config.provider == "groq" {
        body["reasoning_effort"] = json!("none");
    }
    body
}

#[derive(Default)]
pub struct Events {
    pending: Vec<u8>,
    pub done: bool,
    received: bool,
    total: usize,
}
impl Events {
    pub fn feed(&mut self, bytes: &[u8], mut delta: impl FnMut(String)) -> Result<(), String> {
        self.total += bytes.len();
        if self.total > 2 * 1024 * 1024 {
            return Err("Ask response exceeds limit".into());
        }
        self.pending.extend_from_slice(bytes);
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            if end >= 256 * 1024 {
                return Err("Ask event exceeds limit".into());
            }
            let line: Vec<_> = self.pending.drain(..=end).collect();
            let line = std::str::from_utf8(&line)
                .map_err(|_| "Invalid Ask response encoding")?
                .trim();
            if self.done || !line.starts_with("data:") {
                continue;
            }
            let data = line[5..].trim();
            if data == "[DONE]" {
                if !self.received {
                    return Err("Ask returned an empty answer".into());
                }
                self.done = true;
                continue;
            }
            let value: Value =
                serde_json::from_str(data).map_err(|_| "Invalid Ask stream event")?;
            if value.get("error").is_some() {
                return Err("Ask provider reported an error".into());
            }
            if let Some(text) = value["choices"][0]["delta"]["content"].as_str() {
                self.received |= !text.trim().is_empty();
                delta(text.to_owned());
            }
        }
        if self.pending.len() > 256 * 1024 {
            return Err("Ask event exceeds limit".into());
        }
        Ok(())
    }
    pub fn finish(&self) -> Result<(), String> {
        if self.done {
            Ok(())
        } else {
            Err("Ask response ended early; partial answer retained".into())
        }
    }
}

pub enum Update {
    Delta(String),
    Finished(Result<(), String>),
}
pub struct Answer {
    pub updates: Receiver<Update>,
    cancelled: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Answer {
    pub fn start(
        question: String,
        image: Vec<u8>,
        config: Configuration,
        cancelled: Arc<AtomicBool>,
    ) -> Self {
        let cancel = cancelled.clone();
        let (tx, updates) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = (|| {
                if cancel.load(Ordering::Acquire) {
                    return Err("Ask cancelled".into());
                }
                config.validate()?;
                if !config.enabled {
                    return Err("Enable Ask before sending a question".into());
                }
                let key = secret(&config.provider, None, &cancel)?;
                let escape = |s: &str| {
                    s.replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r")
                };
                let mut input = format!(
                    "url = \"{}\"\nheader = \"Authorization: Bearer {}\"\nheader = \"Content-Type: application/json\"\ndata-binary = \"{}\"\n",
                    escape(&config.endpoint),
                    escape(&key),
                    escape(&request(&question, &image, &config).to_string())
                );
                if config.provider == "openRouter" {
                    input.push_str("header = \"X-OpenRouter-Title: Koett\"\n");
                }
                let mut events = Events::default();
                process::run(
                    Command::new("/usr/bin/curl").args([
                        "--disable",
                        "--config",
                        "-",
                        "--silent",
                        "--fail",
                        "--no-buffer",
                        "--proto",
                        "=https",
                        "--connect-timeout",
                        "15",
                        "--max-time",
                        "60",
                    ]),
                    input.into_bytes(),
                    &cancel,
                    Duration::from_secs(65),
                    2 * 1024 * 1024,
                    |bytes| {
                        events.feed(bytes, |s| {
                            let _ = tx.send(Update::Delta(s));
                        })
                    },
                )?;
                events.finish()
            })();
            let _ = tx.send(Update::Finished(result));
        });
        Self {
            updates,
            cancelled,
            worker: Some(worker),
        }
    }
}
impl Drop for Answer {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests;

static STATUS: std::sync::OnceLock<std::sync::RwLock<Value>> = std::sync::OnceLock::new();
pub fn status() -> Value {
    STATUS
        .get_or_init(|| std::sync::RwLock::new(json!({"phase": "Idle", "screen": false})))
        .read()
        .map(|v| v.clone())
        .unwrap_or(json!({"error": "Ask status unavailable"}))
}
pub fn update_status(value: &Value) {
    if let Ok(mut current) = STATUS
        .get_or_init(|| std::sync::RwLock::new(json!({"phase": "Idle", "screen": false})))
        .write()
    {
        if value["reset"] == true {
            *current = json!({"phase": "Listening…", "screen": false});
        }
        for field in ["phase", "screen", "error", "warning", "speech_notice"] {
            if let Some(next) = value.get(field) {
                current[field] = next.clone();
            }
        }
        if value.get("error").is_some() {
            current["phase"] = json!("Error");
        }
    }
}
