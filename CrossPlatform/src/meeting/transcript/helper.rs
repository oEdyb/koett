use std::collections::BTreeMap;
use std::fs::{self, DirBuilder, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::annotation::TimedWord;
use crate::linux::child::ChildGroup;

use super::{Channel, DecodedWindow, Window, WindowId, check_cancelled, validate_words};
use crate::meeting::{MeetingId, bundle::Bundle};

const SCRIPT: &str = include_str!("../asr_helper.py");
const TURBO: &str = "whisper-large-v3-turbo";
const SWEDISH: &str = "kb-whisper-large";
const READY_TIMEOUT: Duration = Duration::from_secs(180);
const IO_INTERVAL: Duration = Duration::from_millis(25);
const MAX_REPLY: usize = 1024 * 1024;
const STDERR_TAIL_BYTES: u64 = 8 * 1024;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) struct Helper {
    stream: BufReader<UnixStream>,
    child: ChildGroup,
    directory: Scratch,
}

#[derive(Serialize)]
pub(crate) struct Check {
    device: String,
    models: BTreeMap<String, String>,
    vram_mb: u64,
}

#[derive(Deserialize)]
struct Ready {
    ready: bool,
    device: String,
    models: BTreeMap<String, String>,
    vram_mb: Option<u64>,
}

enum Mode {
    Decode,
    Check,
    Fetch,
}

#[derive(Serialize)]
struct Input {
    id: String,
    pcm: PathBuf,
    speech_samples: u64,
}

#[derive(Deserialize)]
struct Output {
    id: String,
    #[serde(flatten)]
    result: WindowResult,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WindowResult {
    Error {
        error: String,
    },
    Words {
        lang: String,
        lang_p: f64,
        backend: String,
        words: Vec<(String, f32, f32)>,
    },
}

pub(crate) fn asr_check(python: &Path, fetch: bool) -> Result<Check, String> {
    let path = std::env::temp_dir().join(format!("koett-asr-{}", MeetingId::new()));
    DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .map_err(|error| error.to_string())?;
    let mode = if fetch { Mode::Fetch } else { Mode::Check };
    let (_helper, ready) = Helper::spawn(Scratch(path), python, mode, &AtomicBool::new(false))?;
    Ok(Check {
        device: ready.device,
        models: ready.models,
        vram_mb: ready
            .vram_mb
            .ok_or("Could not measure GPU memory with nvidia-smi")?,
    })
}

impl Helper {
    pub(super) fn start(
        bundle: &Bundle,
        python: &Path,
        cancelled: &AtomicBool,
    ) -> Result<Self, String> {
        let directory = bundle
            .create_job_directory()
            .map_err(|error| error.to_string())?;
        Self::spawn(Scratch(directory), python, Mode::Decode, cancelled).map(|(helper, _)| helper)
    }

    fn spawn(
        directory: Scratch,
        python: &Path,
        mode: Mode,
        cancelled: &AtomicBool,
    ) -> Result<(Self, Ready), String> {
        check_cancelled(cancelled)?;
        let (script, log) = stage(&directory.0).map_err(|error| error.to_string())?;
        let (parent, remote) = UnixStream::pair().map_err(|error| error.to_string())?;
        let stdin = remote.try_clone().map_err(|error| error.to_string())?;
        parent
            .set_read_timeout(Some(IO_INTERVAL))
            .map_err(|error| error.to_string())?;
        parent
            .set_write_timeout(Some(IO_INTERVAL))
            .map_err(|error| error.to_string())?;
        let mut command = Command::new(python);
        command
            .arg(script)
            .stdin(OwnedFd::from(stdin))
            .stdout(OwnedFd::from(remote))
            .stderr(Stdio::from(log));
        match mode {
            Mode::Decode => {
                command.env("HF_HUB_OFFLINE", "1");
            }
            Mode::Check => {
                command.arg("--check").env("HF_HUB_OFFLINE", "1");
            }
            Mode::Fetch => {
                command.arg("--fetch");
            }
        }
        let child = ChildGroup::spawn_parent_bound(&mut command)
            .map_err(|error| format!("Could not start meeting ASR helper: {error}"))?;
        drop(command);
        let mut helper = Self {
            stream: BufReader::new(parent),
            child,
            directory,
        };
        match helper.read_ready(cancelled) {
            Ok(ready) => Ok((helper, ready)),
            Err(error) => Err(helper.with_stderr(error)),
        }
    }

    fn read_ready(&mut self, cancelled: &AtomicBool) -> Result<Ready, String> {
        let response = self.receive(Instant::now() + READY_TIMEOUT, cancelled)?;
        if let Some(error) = response.get("error").and_then(Value::as_str) {
            return Err(error.into());
        }
        let ready: Ready = serde_json::from_value(response)
            .map_err(|error| format!("Invalid meeting ASR readiness reply: {error}"))?;
        if !ready.ready
            || ready.device != "cuda"
            || [TURBO, SWEDISH]
                .iter()
                .any(|name| ready.models.get(*name).is_none_or(String::is_empty))
        {
            return Err("Meeting ASR helper did not load both CUDA models".into());
        }
        Ok(ready)
    }

    pub(super) fn with_stderr(self, error: String) -> String {
        let Self {
            child, directory, ..
        } = self;
        drop(child);
        match stderr_tail(&directory.0) {
            Ok(tail) if !tail.trim().is_empty() => format!("{error}\n{}", tail.trim_end()),
            _ => error,
        }
    }

    pub(super) fn decode_prefix(
        &mut self,
        windows: &[Window],
        cancelled: &AtomicBool,
        budget: Duration,
    ) -> (Vec<DecodedWindow>, Option<String>) {
        let mut decoded = Vec::with_capacity(windows.len());
        let result = self.batch(windows, cancelled, Instant::now() + budget, &mut decoded);
        (decoded, result.err())
    }

    fn batch(
        &mut self,
        windows: &[Window],
        cancelled: &AtomicBool,
        deadline: Instant,
        decoded: &mut Vec<DecodedWindow>,
    ) -> Result<(), String> {
        let inputs = windows
            .iter()
            .map(|window| {
                check_deadline(deadline, cancelled)?;
                let id = wire_id(window.id);
                let pcm = self.directory.0.join(format!("{id}.s16"));
                let bytes: Vec<_> = window
                    .samples
                    .iter()
                    .flat_map(|sample| ((sample * 32_768.0).round() as i16).to_le_bytes())
                    .collect();
                fs::write(&pcm, bytes).map_err(|error| error.to_string())?;
                let speech_samples = window.core.end.0 - window.core.first.0;
                Ok(Input {
                    id,
                    pcm,
                    speech_samples,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut line = serde_json::to_vec(&json!({"op": "decode", "windows": inputs}))
            .map_err(|error| error.to_string())?;
        line.push(b'\n');
        self.send(&line, deadline, cancelled)?;
        for (input, window) in inputs.iter().zip(windows) {
            let output: Output = serde_json::from_value(self.receive(deadline, cancelled)?)
                .map_err(|error| format!("Invalid meeting ASR window reply: {error}"))?;
            if input.id != output.id {
                return Err(format!(
                    "Expected ASR window {}, received {}",
                    input.id, output.id
                ));
            }
            let result = output.result.into_decoded(window)?;
            decoded.push(result);
        }
        for input in inputs {
            fs::remove_file(input.pcm).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn send(
        &mut self,
        mut bytes: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), String> {
        while !bytes.is_empty() {
            check_deadline(deadline, cancelled)?;
            match self.stream.get_mut().write(bytes) {
                Ok(0) => return Err("Meeting ASR helper stopped reading requests".into()),
                Ok(written) => bytes = &bytes[written..],
                Err(error) if retry(&error) => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(())
    }

    fn receive(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<Value, String> {
        let mut line = Vec::new();
        loop {
            check_deadline(deadline, cancelled)?;
            let available = match self.stream.fill_buf() {
                Ok([]) => return Err("Meeting ASR helper closed its output".into()),
                Ok(bytes) => bytes,
                Err(error) if retry(&error) => continue,
                Err(error) => return Err(error.to_string()),
            };
            let end = available.iter().position(|byte| *byte == b'\n');
            let size = end.map_or(available.len(), |index| index + 1);
            if line.len() + size > MAX_REPLY {
                return Err("Meeting ASR reply exceeds the size limit".into());
            }
            line.extend_from_slice(&available[..size]);
            self.stream.consume(size);
            if end.is_some() {
                check_deadline(deadline, cancelled)?;
                return serde_json::from_slice(&line)
                    .map_err(|error| format!("Invalid meeting ASR reply: {error}"));
            }
        }
    }
}

impl WindowResult {
    fn into_decoded(self, window: &Window) -> Result<DecodedWindow, String> {
        let (lang, lang_p, backend, words) = match self {
            Self::Error { error } => return Err(error),
            Self::Words {
                lang,
                lang_p,
                backend,
                words,
            } => (lang, lang_p, backend, words),
        };
        if !matches!(
            (lang.as_str(), backend.as_str()),
            ("sv", SWEDISH) | ("en", TURBO)
        ) || !(0.0..=1.0).contains(&lang_p)
        {
            return Err(format!("Invalid language routing for {:?}", window.id));
        }
        let decoded = DecodedWindow {
            id: window.id,
            words: words
                .into_iter()
                .map(|(text, start, end)| TimedWord { text, start, end })
                .collect(),
            lang,
            backend,
        };
        validate_words(window, &decoded)?;
        Ok(decoded)
    }
}

fn stage(directory: &Path) -> io::Result<(PathBuf, File)> {
    let script = directory.join("asr_helper.py");
    fs::write(&script, SCRIPT)?;
    Ok((script, File::create(directory.join("helper.log"))?))
}

fn stderr_tail(directory: &Path) -> io::Result<String> {
    let mut log = File::open(directory.join("helper.log"))?;
    let start = log.metadata()?.len().saturating_sub(STDERR_TAIL_BYTES);
    log.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    log.take(STDERR_TAIL_BYTES).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn wire_id(id: WindowId) -> String {
    let channel = match id.channel {
        Channel::You => "you",
        Channel::Other => "other",
    };
    format!("{channel}-{:06}", id.sequence)
}

fn retry(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

fn check_deadline(deadline: Instant, cancelled: &AtomicBool) -> Result<(), String> {
    check_cancelled(cancelled)?;
    if Instant::now() >= deadline {
        return Err("Meeting ASR helper timed out".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
