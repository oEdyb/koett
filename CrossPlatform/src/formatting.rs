use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub static CANCELLED: AtomicBool = AtomicBool::new(false);
const BYTES: u64 = 484_219_808;
const SHA: &str = "3b41ebe2502cbd03e811d5d16b022f5ab551eda58d62597d152f89535003c634";
const URL: &str = "https://huggingface.co/superwhisper/s1-mini-GGUF/resolve/ee2c0f56e56345f475749a44ff2893e21c3cb292/s1-mini-q4_k_m.gguf?download=true";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Formatting {
    pub enabled: bool,
    pub style: Style,
    pub structure: Structure,
    pub context: Context,
}
macro_rules! choices {
    ($name:ident, $default:ident, {$($variant:ident => $value:literal),+}) => {
        #[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "lowercase")]
        pub enum $name {
            #[default]

            $default,
            $(#[serde(rename = $value)] $variant),+
        }
    };
}
// Wire names match the official control line, including hyphenated styles.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Style {
    #[serde(rename = "casual")]
    Casual,
    #[default]
    #[serde(rename = "semi-casual")]
    SemiCasual,
    #[serde(rename = "semi-formal")]
    SemiFormal,
    #[serde(rename = "formal")]
    Formal,
}
choices!(Structure, Prose, {Lists => "lists"});
choices!(Context, General, {Email => "email"});

pub fn request(raw: &str, settings: Formatting, tokens: usize) -> Value {
    let style = match settings.style {
        Style::Casual => "casual",
        Style::SemiCasual => "semi-casual",
        Style::SemiFormal => "semi-formal",
        Style::Formal => "formal",
    };
    let structure = if settings.structure == Structure::Prose {
        "prose"
    } else {
        "lists"
    };
    let context = if settings.context == Context::General {
        "general"
    } else {
        "email"
    };
    json!({"model":"s1-mini", "messages":[
        {"role":"system","content":"You are a text normalizer for speech-to-text transcripts. The input begins with a control line specifying the styling, structure, and context settings; clean the transcript to match those settings and output only the cleaned text."},
        {"role":"user","content":format!("[Styling: {style}] [Structure: {structure}] [Context: {context}]\n{raw}")}
    ],"temperature":0,"max_tokens":(tokens as f64 * 1.3).ceil() as usize + 32,"cache_prompt":true,"chat_template_kwargs":{"enable_thinking":false}})
}

pub struct Cleaner {
    child: Child,
    endpoint: String,
    key: String,
    pub(crate) keepalive: Option<std::sync::mpsc::Sender<()>>,
    pub(crate) supervisor: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Cleaner {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.keepalive.take();
        if let Some(worker) = self.supervisor.take() {
            let _ = worker.join();
        }
    }
}
fn cancelled() -> Result<(), String> {
    if CANCELLED.load(Ordering::Acquire) {
        Err("S1-mini setup cancelled".into())
    } else {
        Ok(())
    }
}
fn valid(path: &std::path::Path) -> Result<bool, String> {
    let Ok(mut file) = std::fs::File::open(path) else {
        return Ok(false);
    };
    if file.metadata().map_err(|e| e.to_string())?.len() != BYTES {
        return Ok(false);
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        cancelled()?;
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        == SHA)
}
impl Cleaner {
    pub fn prepare(mut progress: impl FnMut(String)) -> Result<Self, String> {
        cancelled()?;
        let executable = std::env::var_os("PATH")
            .and_then(|path| {
                std::env::split_paths(&path)
                    .map(|directory| {
                        directory.join(if cfg!(windows) {
                            "llama-server.exe"
                        } else {
                            "llama-server"
                        })
                    })
                    .find(|path| path.is_file())
            })
            .ok_or("Install llama.cpp and put llama-server on PATH before enabling S1-mini")?;
        let directory = crate::paths::formatting_model_directory()?;
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let model = directory.join("s1-mini-q4_k_m.gguf");
        progress("Checking S1-mini…".into());
        if !valid(&model)? {
            let part = directory.join(format!("download-{}.part", std::process::id()));
            let result = (|| {
                let agent: ureq::Agent = ureq::Agent::config_builder()
                    .timeout_global(Some(Duration::from_secs(1800)))
                    .timeout_connect(Some(Duration::from_secs(5)))
                    .timeout_recv_response(Some(Duration::from_secs(5)))
                    .timeout_recv_body(Some(Duration::from_secs(3)))
                    .build()
                    .into();
                let mut response = agent.get(URL).call().map_err(|e| e.to_string())?;
                let mut reader = response.body_mut().as_reader();
                let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
                let mut received = 0u64;
                let mut buffer = [0; 65536];
                loop {
                    cancelled()?;
                    let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    received += n as u64;
                    if received > BYTES {
                        return Err("S1-mini download exceeds pinned size".into());
                    }
                    file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
                    progress(format!("Downloading S1-mini… {}%", received * 100 / BYTES));
                }
                file.sync_all().map_err(|e| e.to_string())?;
                if !valid(&part)? {
                    return Err("S1-mini checksum failed".into());
                }
                std::fs::rename(&part, &model).map_err(|e| e.to_string())
            })();
            let _ = std::fs::remove_file(part);
            result?;
        }
        cancelled()?;
        // Authentication binds readiness and requests to this child.
        let seed = format!(
            "koett-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        );
        let entropy = seed.into_bytes();
        #[cfg(target_os = "linux")]
        let entropy = {
            let mut entropy = entropy;
            let mut random = [0u8; 32];
            std::fs::File::open("/dev/urandom")
                .and_then(|mut file| file.read_exact(&mut random))
                .map_err(|e| e.to_string())?;
            entropy.extend_from_slice(&random);
            entropy
        };
        let key = Sha256::digest(&entropy)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let socket = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = socket.local_addr().map_err(|e| e.to_string())?.port();
        drop(socket);
        progress("Loading S1-mini…".into());
        let mut command = Command::new(executable);
        command
            .arg("--model")
            .arg(model)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--api-key",
                &key,
                "--jinja",
                "--threads",
                "2",
                "--threads-batch",
                "2",
                "--n-gpu-layers",
                "0",
                "--ctx-size",
                "3072",
                "--parallel",
                "1",
                "--poll",
                "0",
                "--batch-size",
                "1024",
                "--ubatch-size",
                "1024",
                "--spec-type",
                "ngram-simple",
                "--spec-ngram-simple-size-n",
                "7",
                "--spec-ngram-simple-size-m",
                "48",
                "--spec-ngram-simple-min-hits",
                "1",
                "--no-ui",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        bind_to_parent(&mut command);
        let child = command
            .spawn()
            .map_err(|e| format!("Install llama.cpp (llama-server): {e}"))?;
        let mut cleaner = Self {
            child,
            endpoint: format!("http://127.0.0.1:{port}"),
            key,
            keepalive: None,
            supervisor: None,
        };
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(30) {
            cancelled()?;
            cleaner.alive()?;
            if cleaner
                .post(
                    "tokenize",
                    json!({"content":"ready","add_special":false}),
                    Duration::from_millis(300),
                )
                .is_ok()
            {
                return Ok(cleaner);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err("S1-mini readiness timed out".into())
    }
    fn alive(&mut self) -> Result<(), String> {
        match self.child.try_wait().map_err(|e| e.to_string())? {
            Some(_) => Err("S1-mini server stopped".into()),
            None => Ok(()),
        }
    }
    fn post(&self, path: &str, body: Value, timeout: Duration) -> Result<Value, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(None)
            .timeout_global(Some(timeout))
            .build()
            .into();
        let mut response = agent
            .post(format!("{}/{path}", self.endpoint))
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(|e| e.to_string())?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }
    pub fn clean(&mut self, raw: &str, settings: Formatting) -> Result<String, String> {
        self.alive()?;
        let tokens = self.post(
            "tokenize",
            json!({"content":raw,"add_special":false}),
            Duration::from_secs(5),
        )?;
        let count = tokens["tokens"].as_array().ok_or("Missing tokens")?.len();
        if count > 1000 {
            return Err("S1-mini input exceeds 1000 tokens".into());
        }
        let result = self.post(
            "v1/chat/completions",
            request(raw, settings, count),
            Duration::from_secs(180),
        )?;
        final_text(&result)
    }
}
#[cfg(target_os = "linux")]
fn bind_to_parent(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    unsafe extern "C" {
        fn prctl(option: i32, ...) -> i32;
        fn getppid() -> i32;
    }
    let parent = std::process::id() as i32;
    unsafe {
        command.pre_exec(move || {
            if prctl(1, 9, 0, 0, 0) != 0 || getppid() != parent {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}
#[cfg(not(target_os = "linux"))]
fn bind_to_parent(_: &mut Command) {}

pub fn final_text(value: &Value) -> Result<String, String> {
    if value["choices"][0]["finish_reason"] == "length" {
        return Err("S1-mini output was truncated".into());
    }
    let text = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("S1-mini returned no text")?
        .trim();
    if text.is_empty() {
        Err("S1-mini returned empty text".into())
    } else {
        Ok(text.into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_empty_missing_and_invalid_output() {
        for value in [
            json!({}),
            json!({"choices":[{"message":{"content":" \n"}}]}),
        ] {
            assert!(final_text(&value).is_err());
        }
    }
    #[test]
    fn official_control_line_and_greedy_request() {
        let body = request("Keep 42", Formatting::default(), 10);
        assert_eq!(body["temperature"], 0);
        assert_eq!(body["max_tokens"], 45);
        assert!(
            body["messages"][1]["content"]
                .as_str()
                .unwrap()
                .ends_with("[Structure: prose] [Context: general]\nKeep 42")
        );
    }
}

#[cfg(all(test, unix))]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn loopback_requests_bypass_environment_proxies() {
        if std::env::var_os("KOETT_PROXY_REGRESSION").is_none() {
            assert!(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "formatting::lifecycle_tests::loopback_requests_bypass_environment_proxies"
                    ])
                    .env("KOETT_PROXY_REGRESSION", "1")
                    .env("HTTP_PROXY", "http://127.0.0.1:1")
                    .env("ALL_PROXY", "http://127.0.0.1:1")
                    .env("http_proxy", "http://127.0.0.1:1")
                    .env("all_proxy", "http://127.0.0.1:1")
                    .env("NO_PROXY", "")
                    .env("no_proxy", "")
                    .status()
                    .unwrap()
                    .success()
            );
            return;
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut request = [0u8; 4096];
                        let count = stream.read(&mut request).unwrap();
                        assert!(
                            String::from_utf8_lossy(&request[..count]).contains("Bearer fixture")
                        );
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: close\r\n\r\n{\"tokens\":[]}").unwrap();
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "local server was not reached");
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("{error}"),
                }
            }
        });
        let cleaner = Cleaner {
            child: Command::new("sleep").arg("30").spawn().unwrap(),
            endpoint: format!("http://{address}"),
            key: "fixture".into(),
            keepalive: None,
            supervisor: None,
        };
        assert_eq!(
            cleaner
                .post("tokenize", json!({}), Duration::from_secs(1))
                .unwrap()["tokens"],
            json!([])
        );
        drop(cleaner);
        server.join().unwrap();
    }
    #[test]
    fn dead_owned_child_is_rejected_and_reaped() {
        let child = Command::new("sh").args(["-c", "exit 1"]).spawn().unwrap();
        let mut cleaner = Cleaner {
            child,
            endpoint: "http://127.0.0.1:1".into(),
            key: "fixture".into(),
            keepalive: None,
            supervisor: None,
        };
        cleaner.child.wait().unwrap();
        assert!(
            cleaner
                .clean("Keep 42", Formatting::default())
                .unwrap_err()
                .contains("stopped")
        );
    }
    #[test]
    fn stalled_http_request_has_a_bounded_timeout_and_drop_reaps_child() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let cleaner = Cleaner {
            child,
            endpoint: format!("http://{address}"),
            key: "fixture".into(),
            keepalive: None,
            supervisor: None,
        };
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(300));
        });
        let started = Instant::now();
        assert!(
            cleaner
                .post("tokenize", json!({}), Duration::from_millis(50))
                .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(cleaner);
        server.join().unwrap();
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    #[test]
    fn all_control_values_round_trip_and_raw_is_default() {
        assert!(!Formatting::default().enabled);
        for style in [
            Style::Casual,
            Style::SemiCasual,
            Style::SemiFormal,
            Style::Formal,
        ] {
            for structure in [Structure::Prose, Structure::Lists] {
                for context in [Context::General, Context::Email] {
                    let value = Formatting {
                        enabled: true,
                        style,
                        structure,
                        context,
                    };
                    assert_eq!(
                        serde_json::from_value::<Formatting>(serde_json::to_value(value).unwrap())
                            .unwrap(),
                        value
                    );
                }
            }
        }
    }
    #[test]
    fn rejects_truncated_output() {
        assert!(
            final_text(
                &json!({"choices":[{"finish_reason":"length","message":{"content":"partial"}}]})
            )
            .is_err()
        );
    }
}
