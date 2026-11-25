use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::meeting::bundle::scan::{scan, scan_all};
use crate::meeting::{
    Backend, CloseGoal, MeetingCall, MeetingId, MeetingRequest, MeetingView, RecoveryReason, Reply,
    ReplyError, SessionInfo, SourceHealth, StopKey, Ticket,
};

use super::TrayAction;

pub(super) const ROUTE: &[u8] = b"meeting ";
const MAX_REPLY: u64 = 64 * 1024;
const ADMISSION_WAIT: Duration = Duration::from_millis(350);
const REPLY_WAIT: Duration = Duration::from_secs(2);

enum Command {
    Daemon(MeetingRequest),
    List { recoverable: bool, limit: usize },
    Get(MeetingId),
    Unavailable,
}

fn parse(values: &[String]) -> Result<Command, &'static str> {
    use MeetingRequest as Request;
    let values: Vec<&str> = values.iter().map(String::as_str).collect();
    let id = |value| MeetingId::parse(value).ok_or("Meeting ID must be a UUID.");
    let key = |value, token: &str| -> Result<StopKey, &'static str> {
        let generation = token
            .parse::<u64>()
            .map_err(|_| "Stop token must be an integer.")?;
        Ok(StopKey {
            id: id(value)?,
            generation,
        })
    };
    Ok(Command::Daemon(match values.as_slice() {
        ["start"] => Request::Start(MeetingId::new()),
        ["start", "--id", value] => Request::Start(id(value)?),
        ["stop", value] => Request::Stop(id(value)?),
        ["resume", value, token] => Request::Resume(key(value, token)?),
        ["finish", value, token] => Request::Finish(key(value, token)?),
        ["cancel", value, token] => Request::Cancel(key(value, token)?),
        ["recover", value] => Request::Recover {
            id: id(value)?,
            backend: None,
        },
        ["recover", value, "--backend", backend] => Request::Recover {
            id: id(value)?,
            backend: Some(match *backend {
                "gpu" => Backend::Gpu,
                "parakeet" => Backend::Parakeet,
                _ => return Err("Choose gpu or parakeet."),
            }),
        },
        ["status"] => Request::Status(None),
        ["status", value] => Request::Status(Some(id(value)?)),
        ["toggle"] => Request::Shortcut,
        ["get", value] => return Ok(Command::Get(id(value)?)),
        ["list", options @ ..] => {
            let (recoverable, limit) = match options {
                [] => (false, 50),
                ["--recoverable"] => (true, 50),
                ["--limit", limit] => (false, parse_limit(limit)?),
                ["--limit", limit, "--recoverable"] | ["--recoverable", "--limit", limit] => {
                    (true, parse_limit(limit)?)
                }
                _ => return Err("Usage: koett meeting list [--recoverable] [--limit N]"),
            };
            return Ok(Command::List { recoverable, limit });
        }
        ["search", _] | ["asr-check"] | ["asr-check", "--fetch"] => {
            return Ok(Command::Unavailable);
        }
        ["search", _, "--limit", limit] => {
            parse_limit(limit)?;
            return Ok(Command::Unavailable);
        }
        _ => {
            return Err(
                "Usage: koett meeting start|stop|resume|finish|cancel|recover|status|list|get|toggle|search|asr-check",
            );
        }
    }))
}

fn parse_limit(limit: &str) -> Result<usize, &'static str> {
    limit
        .parse()
        .map_err(|_| "Limit must be a nonnegative integer.")
}

pub(super) fn run(values: &[String]) -> i32 {
    let command = match parse(values) {
        Ok(command) => command,
        Err(message) => {
            println!("{}", error("bad_usage", message));
            return 2;
        }
    };
    let reply = match command {
        Command::Daemon(request) => match connect() {
            Ok(stream) => request_socket(stream, values).unwrap_or_else(|error| failure(&error)),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    || error.kind() == io::ErrorKind::ConnectionRefused =>
            {
                match request {
                    MeetingRequest::Status(id) => offline_status(id),
                    _ => failure(&ReplyError::DaemonUnavailable),
                }
            }
            Err(error) => failure(&ReplyError::DiskUnreadable(error.to_string())),
        },
        command => {
            let daemon = !matches!(command, Command::Unavailable) && connect().is_ok();
            offline(command, daemon)
        }
    };
    println!("{reply}");
    i32::from(reply["ok"] != true)
}

fn connect() -> io::Result<UnixStream> {
    let path = super::control::path().map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
    UnixStream::connect(path)
}

fn request_socket(mut stream: UnixStream, values: &[String]) -> Result<Value, ReplyError> {
    let mut request = ROUTE.to_vec();
    serde_json::to_writer(&mut request, values).map_err(|_| ReplyError::OutcomeUnknown)?;
    if request.len() > super::control::MAX_REQUEST {
        return Ok(error(
            "request_too_large",
            "Meeting socket requests are limited to 8 KiB.",
        ));
    }
    stream
        .set_read_timeout(Some(REPLY_WAIT + Duration::from_secs(1)))
        .and_then(|()| stream.set_write_timeout(Some(ADMISSION_WAIT)))
        .and_then(|()| stream.write_all(&request))
        .and_then(|()| stream.shutdown(Shutdown::Write))
        .map_err(|_| ReplyError::OutcomeUnknown)?;
    read_reply(stream)
}

fn read_reply(stream: impl Read) -> Result<Value, ReplyError> {
    let mut bytes = Vec::new();
    stream
        .take(MAX_REPLY + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReplyError::OutcomeUnknown)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_REPLY {
        return Ok(error(
            "reply_too_large",
            "Meeting socket replies are limited to 64 KiB.",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|_| ReplyError::OutcomeUnknown)
}

pub(super) fn serve(stream: &mut UnixStream, bytes: &[u8], sender: &Sender<TrayAction>) {
    let _ = stream.set_write_timeout(Some(ADMISSION_WAIT));
    let reply = match serde_json::from_slice::<Vec<String>>(bytes) {
        Ok(values) => match parse(&values) {
            Ok(Command::Daemon(request)) => dispatch_socket(sender, request),
            Ok(command) => offline(command, true),
            Err(message) => error("bad_usage", message),
        },
        Err(_) => error(
            "bad_usage",
            "Send meeting arguments as a JSON string array.",
        ),
    };
    let body = bounded_reply(reply);
    let _ = stream.write_all(body.as_bytes());
}

fn bounded_reply(reply: Value) -> String {
    let body = reply.to_string();
    if body.len() as u64 <= MAX_REPLY {
        body
    } else {
        error(
            "reply_too_large",
            "Meeting socket replies are limited to 64 KiB.",
        )
        .to_string()
    }
}

fn dispatch_socket(sender: &Sender<TrayAction>, request: MeetingRequest) -> Value {
    let (reply, inbox) = mpsc::channel();
    let offered = Instant::now();
    let ticket = Ticket::new(offered + ADMISSION_WAIT);
    let call = MeetingCall::socket(request, ticket.clone(), reply);
    if sender.send(TrayAction::Meeting(call)).is_err() {
        return failure(&ReplyError::DaemonUnavailable);
    }
    let result = inbox.recv_timeout(ADMISSION_WAIT).or_else(|error| {
        let state = ticket.timeout();
        if state == ReplyError::OutcomeUnknown && error == mpsc::RecvTimeoutError::Timeout {
            inbox
                .recv_timeout(REPLY_WAIT.saturating_sub(offered.elapsed()))
                .map_err(|_| state)
        } else {
            Err(state)
        }
    });
    match result {
        Ok(Reply::View(view)) => success(view_data(view)),
        Ok(Reply::Error(error)) | Err(error) => failure(&error),
        Ok(Reply::None) => failure(&ReplyError::OutcomeUnknown),
    }
}

fn offline_status(id: Option<MeetingId>) -> Value {
    let result = match id {
        Some(id) => with_root(|root| Ok(view_data(find(root, id)?))),
        None => Ok(view_data(MeetingView::Idle)),
    };
    with_daemon(result, false)
}

fn offline(command: Command, daemon: bool) -> Value {
    match command {
        Command::List { recoverable, limit } => {
            with_daemon(with_root(|root| list(root, recoverable, limit)), daemon)
        }
        Command::Get(id) => with_daemon(with_root(|root| get(root, id)), daemon),
        Command::Unavailable => failure(&ReplyError::NotAvailable),
        Command::Daemon(_) => failure(&ReplyError::DaemonUnavailable),
    }
}

fn with_daemon(result: Result<Value, ReplyError>, daemon: bool) -> Value {
    match result {
        Ok(mut data) => {
            data["daemon"] = json!(daemon);
            success(data)
        }
        Err(error) => {
            let mut reply = failure(&error);
            reply["daemon"] = json!(daemon);
            reply
        }
    }
}

fn with_root(read: impl FnOnce(&Path) -> Result<Value, ReplyError>) -> Result<Value, ReplyError> {
    let root = crate::paths::meetings_directory().map_err(ReplyError::DiskUnreadable)?;
    read(&root)
}

fn list(root: &Path, recoverable: bool, limit: usize) -> Result<Value, ReplyError> {
    let states = scan_all(root).map_err(|error| ReplyError::DiskUnreadable(error.to_string()))?;
    let items: Vec<Value> = states
        .into_iter()
        .filter(|state| !recoverable || state.is_recoverable())
        .take(limit)
        .map(|state| {
            let duration_ms = state.duration().map(|offset| offset.0 / 16);
            let view = state.view(root);
            let reason = match &view {
                MeetingView::Recoverable(_, reason) => Some(reason_code(reason)),
                MeetingView::Complete(_, artifact, _) if artifact.fallback => {
                    Some(reason_code(&RecoveryReason::FallbackUsed))
                }
                _ => None,
            };
            let mut data = view_data(view);
            data["duration_ms"] = json!(duration_ms);
            data["reason"] = json!(reason);
            data
        })
        .collect();
    Ok(json!({"items": items}))
}

fn get(root: &Path, id: MeetingId) -> Result<Value, ReplyError> {
    match find(root, id)? {
        MeetingView::Complete(info, artifact, projections) => Ok(json!({
            "id": info.id.to_string(), "state": "complete",
            "markdown": artifact.markdown, "assets": artifact.assets,
            "backends": artifact.backends, "fallback": artifact.fallback,
            "audio_retained": artifact.audio_retained, "projections": projections,
        })),
        MeetingView::Unreadable { issue, .. } => Err(ReplyError::DiskUnreadable(issue)),
        _ => Err(ReplyError::ArtifactPending),
    }
}

fn find(root: &Path, id: MeetingId) -> Result<MeetingView, ReplyError> {
    scan(root, id)
        .map_err(|error| ReplyError::DiskUnreadable(error.to_string()))?
        .map(|state| state.view(root))
        .ok_or(ReplyError::NotFound)
}

fn success(data: Value) -> Value {
    json!({"version": 1, "ok": true, "data": data})
}

fn failure(failure: &ReplyError) -> Value {
    error(failure.code(), failure.message())
}

fn error(code: &str, message: &str) -> Value {
    json!({"version": 1, "ok": false, "error": {"code": code, "message": message}})
}

fn view_data(view: MeetingView) -> Value {
    fn session(info: SessionInfo, state: &str) -> Value {
        json!({"id": info.id.to_string(), "state": state, "started_at_ms": info.started_at_ms})
    }
    fn source(health: SourceHealth) -> &'static str {
        match health {
            SourceHealth::Receiving => "receiving",
            SourceHealth::Reconnecting => "reconnecting",
        }
    }
    match view {
        MeetingView::Idle => json!({"state": "idle"}),
        MeetingView::Recording { ref info, health }
        | MeetingView::ConfirmStop {
            ref info, health, ..
        } => {
            let key = match view {
                MeetingView::ConfirmStop { key, .. } => Some(key.generation.to_string()),
                _ => None,
            };
            let mut data = session(
                info.clone(),
                if key.is_some() {
                    "confirm_stop"
                } else {
                    "recording"
                },
            );
            data["capture"] =
                json!({"you": source(health.tracks.you), "other": source(health.tracks.other)});
            if health.disk_low {
                data["disk_low"] = json!(true);
            }
            if let Some(key) = key {
                data["stop_token"] = json!(key);
            }
            data
        }
        MeetingView::Closing { info, goal } => {
            let mut data = session(info, "closing");
            data["goal"] = json!(match goal {
                CloseGoal::Finish => "finish",
                CloseGoal::Hold => "hold",
                CloseGoal::Interrupted(_) => "interrupted",
            });
            data
        }
        MeetingView::Processing { info, done } => {
            let mut data = session(info, "processing");
            data["done"] = json!(done);
            data
        }
        MeetingView::Active(info) => session(info, "active"),
        MeetingView::Recoverable(info, reason) => {
            let mut data = session(info, "recoverable");
            data["reason"] = json!(reason_code(&reason));
            if let RecoveryReason::CaptureFailed(issue)
            | RecoveryReason::DecodeFailed(issue)
            | RecoveryReason::PublishFailed(issue) = reason
            {
                data["issue"] = json!(issue);
            }
            data
        }
        MeetingView::Complete(info, artifact, _) => {
            let mut data = session(info, "complete");
            data["markdown"] = json!(artifact.markdown);
            data
        }
        MeetingView::Unreadable { id, issue } => {
            json!({"id": id.to_string(), "state": "unreadable", "issue": issue})
        }
    }
}

fn reason_code(reason: &RecoveryReason) -> &'static str {
    match reason {
        RecoveryReason::Interrupted => "interrupted",
        RecoveryReason::HeldByUser => "held_by_user",
        RecoveryReason::CaptureFailed(_) => "capture_failed",
        RecoveryReason::DecodeFailed(_) => "decode_failed",
        RecoveryReason::PublishFailed(_) => "publish_failed",
        RecoveryReason::FallbackUsed => "fallback_used",
    }
}

#[cfg(test)]
mod tests;
