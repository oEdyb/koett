use super::Configuration;
use base64::Engine;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
const ENDPOINT: &str = "wss://api.cartesia.ai/tts/websocket?cartesia_version=2026-03-01";
const MODEL: &str = "sonic-3.5-2026-05-04";
const RATE: usize = 44_100;
const QUEUE_LIMIT: usize = RATE * 3;
type Queue = Arc<Mutex<VecDeque<f32>>>;
pub fn run(
    config: Configuration,
    cancel: Arc<AtomicBool>,
    queue: Queue,
    rx: tokio::sync::mpsc::Receiver<Option<String>>,
    notice: std::sync::mpsc::Sender<String>,
) -> Result<(), String> {
    let voice = config.voice_id()?;
    let key = super::super::secret("cartesia", None, &cancel)?;
    if cancel.load(Ordering::Acquire) {
        return Ok(());
    }
    #[cfg(test)]
    let endpoint_override =
        std::env::var("KOETT_SPEECH_FIXTURE_URL").unwrap_or_else(|_| ENDPOINT.into());
    #[cfg(test)]
    let endpoint = endpoint_override.as_str();
    #[cfg(test)]
    assert!(endpoint == ENDPOINT || endpoint.starts_with("wss://localhost:"));
    #[cfg(not(test))]
    let endpoint = ENDPOINT;
    connection(voice, key, endpoint, cancel, queue, rx, Some(notice))
}
fn connection(
    voice: &str,
    key: String,
    endpoint: &str,
    cancel: Arc<AtomicBool>,
    queue: Queue,
    mut rx: tokio::sync::mpsc::Receiver<Option<String>>,
    notice: Option<std::sync::mpsc::Sender<String>>,
) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Cannot start speech runtime")?;
    runtime.block_on(async {
        let operation = async {
            let mut request = endpoint.into_client_request().map_err(|_| "Invalid speech endpoint")?;
            let mut header: tokio_tungstenite::tungstenite::http::HeaderValue = key.parse().map_err(|_| "Invalid speech key")?;
            header.set_sensitive(true);
            request.headers_mut().insert("X-API-Key", header);
            request.headers_mut().insert("Cartesia-Version", "2026-03-01".parse().map_err(|_| "Invalid speech version")?);
            let limits = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default().max_message_size(Some(512 * 1024)).max_frame_size(Some(512 * 1024));
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(10), tokio_tungstenite::connect_async_with_config(request, Some(limits), false)).await.map_err(|_| "Speech connection timed out")?.map_err(|_| "Speech connection failed")?;
            let context = uuid::Uuid::new_v4().to_string();
            let mut stream = None;
            let playback_failed = Arc::new(AtomicBool::new(false));
            let mut frames = 0;
            let mut sent = false;
            let mut input_done = false;
            let mut last = tokio::time::Instant::now();
            let mut tick = tokio::time::interval(Duration::from_millis(100));
            loop {
                tokio::select! {
                    text = rx.recv(), if !input_done => {
                        let final_input = !matches!(text, Some(Some(_)));
                        if final_input && !sent { break; }
                        let transcript = text.flatten().unwrap_or_default();
                        let payload = serde_json::json!({"model_id":MODEL,"transcript":transcript,"voice":{"mode":"id","id":voice},"language":"en","context_id":context,"output_format":{"container":"raw","encoding":"pcm_f32le","sample_rate":RATE},"continue":!final_input});
                        tokio::time::timeout(Duration::from_secs(5),socket.send(Message::Text(payload.to_string().into()))).await.map_err(|_| "Speech send timed out")?.map_err(|_| "Speech send failed")?;
                        sent=true; input_done=final_input; last=tokio::time::Instant::now();
                    }
                    message = socket.next() => {
                        let message=message.ok_or("Speech socket closed")?.map_err(|_| "Speech receive failed")?;
                        if let Message::Text(text) = message {
                            let event: serde_json::Value = serde_json::from_str(&text).map_err(|_| "Invalid speech event")?;
                            if event["context_id"].as_str()!=Some(context.as_str()) { return Err("Unexpected speech context".into()); }
                            match event["type"].as_str() {
                                Some("chunk") => {
                                    if !sent { return Err("Unexpected speech audio".into()); }
                                    let samples=decode(&event)?;
                                    frames+=samples.len();
                                    if frames>RATE*60 { return Err("Speech duration exceeds limit".into()); }
                                    if stream.is_none() { stream=Some(player(queue.clone(),cancel.clone(),playback_failed.clone())?); if let Some(notice) = &notice { let _ = notice.send("Speaking…".into()); } }
                                    let deadline=tokio::time::Instant::now()+Duration::from_secs(5);
                                    for chunk in samples.chunks(RATE/10) {
                                        loop {
                                            let inserted={let mut q=queue.lock().map_err(|_| "Speech queue unavailable")?; if q.len()+chunk.len()<=QUEUE_LIMIT { q.extend(chunk); true } else { false }};
                                            if inserted { break; }
                                            if tokio::time::Instant::now()>deadline { return Err("Speech playback stalled".into()); }
                                            tokio::time::sleep(Duration::from_millis(10)).await;
                                        }
                                    }
                                    last=tokio::time::Instant::now();
                                }
                                Some("done") if input_done => { break; }
                                Some("error") => return Err("Speech provider failed".into()),
                                _ => return Err("Unexpected speech event".into()),
                            }
                        } else if matches!(message, Message::Close(_)) { return Err("Speech socket closed".into()); }
                    }
                    _ = tick.tick() => {
                        if playback_failed.load(Ordering::Acquire) { return Err("Speech playback failed".into()); }
                        if last.elapsed()>Duration::from_secs(if sent {15} else {120}) { return Err("Speech stream timed out".into()); }
                    }
                }
            }
            let _=tokio::time::timeout(Duration::from_secs(1),socket.close(None)).await;
            let deadline=tokio::time::Instant::now()+Duration::from_secs(4);
            while !queue.lock().map_err(|_| "Speech queue unavailable")?.is_empty() {
                if tokio::time::Instant::now()>deadline { return Err("Speech drain timed out".into()); }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(stream);
            if let Some(notice) = &notice { let _ = notice.send(String::new()); }
            Ok(())
        };
        tokio::select! {
            result=operation=>result,
            _=async { while !cancel.load(Ordering::Acquire) { tokio::time::sleep(Duration::from_millis(10)).await; } }=>Ok(()),
        }
    })
}
fn decode(event: &serde_json::Value) -> Result<Vec<f32>, String> {
    if event
        .get("sample_rate")
        .is_some_and(|v| v.as_u64() != Some(RATE as u64))
    {
        return Err("Invalid speech sample rate".into());
    }
    let encoded = event["data"].as_str().ok_or("Missing speech audio")?;
    if encoded.len() > 256 * 1024 {
        return Err("Speech chunk exceeds limit".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "Invalid speech audio encoding")?;
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return Err("Invalid speech audio length".into());
    }
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| {
            let sample = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            if sample.is_finite() && sample.abs() <= 1.0 {
                Ok(sample)
            } else {
                Err("Invalid speech sample".into())
            }
        })
        .collect()
}
fn player(
    queue: Queue,
    cancel: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("No speech output device")?;
    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: RATE as u32,
        buffer_size: cpal::BufferSize::Fixed(441),
    };
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _| {
                data.fill(0.0);
                if let Ok(mut q) = queue.lock() {
                    if cancel.load(Ordering::Acquire) {
                        q.clear();
                        return;
                    }
                    for sample in data {
                        *sample = q.pop_front().unwrap_or(0.0);
                    }
                }
            },
            move |error| {
                let _ = error;
                failed.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|_| "Cannot open speech output")?;
    stream.play().map_err(|_| "Cannot play speech output")?;
    Ok(stream)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_trace_is_compiled_out() {
        assert!(log::STATIC_MAX_LEVEL <= log::LevelFilter::Warn);
    }
    #[test]
    fn rejects_invalid_pcm() {
        for bytes in [
            vec![0],
            f32::NAN.to_le_bytes().to_vec(),
            2f32.to_le_bytes().to_vec(),
        ] {
            assert!(decode(&serde_json::json!({"data":base64::engine::general_purpose::STANDARD.encode(bytes)})).is_err());
        }
    }
}

#[cfg(test)]
mod fixture {
    use super::*;
    #[test]
    #[ignore = "requires private koett-speech lab and local TLS fixture"]
    fn private_wss_playback() {
        assert!(
            std::env::var("XDG_RUNTIME_DIR")
                .unwrap()
                .ends_with("/agent-lab/koett-speech")
        );
        assert_eq!(std::env::var("PULSE_SINK").unwrap(), "agent_lab_audio");
        let endpoint = std::env::var("KOETT_SPEECH_FIXTURE_URL").unwrap();
        assert!(endpoint.starts_with("wss://localhost:"));
        let secret_cancel = AtomicBool::new(false);
        super::super::super::secret("cartesia", Some("private-fixture-only"), &secret_cancel)
            .unwrap();
        let key = super::super::super::secret("cartesia", None, &secret_cancel).unwrap();
        assert_eq!(key, "private-fixture-only");
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let observed = queue.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        let worker = std::thread::spawn(move || {
            connection(
                super::super::VOICES[0].1,
                key,
                &endpoint,
                cancel,
                queue,
                rx,
                None,
            )
        });
        std::thread::sleep(Duration::from_millis(500));
        let _ = tx.blocking_send(Some("Hello private fixture.".into()));
        let _ = tx.blocking_send(None);
        let mut peak_queue = 0;
        for _ in 0..40 {
            let length = observed.lock().unwrap().len();
            assert!(length <= QUEUE_LIMIT);
            peak_queue = peak_queue.max(length);
            std::thread::sleep(Duration::from_millis(5));
        }
        let timeout_mode = std::env::var("KOETT_SPEECH_TIMEOUT_MODE").ok();
        if !matches!(timeout_mode.as_deref(), Some("handshake" | "first_audio")) {
            assert!(peak_queue > 0);
        }
        if std::env::var_os("KOETT_SPEECH_CANCEL").is_some() {
            let started = std::time::Instant::now();
            stop.store(true, Ordering::Release);
            worker.join().unwrap().unwrap();
            assert!(started.elapsed() < Duration::from_millis(150));
        } else {
            let result = worker.join().unwrap();
            if timeout_mode.is_some() {
                assert!(result.is_err());
            } else {
                result.unwrap();
            }
        }
    }
}
