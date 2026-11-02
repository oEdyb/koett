//! Optional MPRIS pause ownership. Call `pause` before capture, and `resume` on
//! capture stop, failure or cancellation, before recorder finalization / ASR.
//! Report errors as notices; they must never prevent dictation.
//!
//! Every call has a bounded wait and all D-Bus work runs on a private runtime.
//! Resume consumes ownership once, including on failure. PropertiesChanged,
//! Seeked and name-owner changes revoke ownership conservatively. A user Pause
//! which leaves the same state and emits no signal is indistinguishable from
//! Koett's pause. Noncompliant players which omit signals have the same limit.
//! MPRIS also offers no atomic compare-and-Play: an intervention concurrent with
//! the final state check can race Play.
//!
//! Protocol: https://specifications.freedesktop.org/mpris/latest/Player_Interface.html

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ashpd::zbus::zvariant::OwnedValue;
use ashpd::zbus::{self, Connection, MatchRule, MessageStream, Proxy, message::Type};
use futures_util::StreamExt;
use tokio::sync::{Mutex, mpsc as queue};

const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PREFIX: &str = "org.mpris.MediaPlayer2.";
const IPC_WAIT: Duration = Duration::from_millis(100);
const WORK_WAIT: Duration = Duration::from_millis(400);
const ACK_WAIT: Duration = Duration::from_millis(550);
type Properties = HashMap<String, OwnedValue>;
type Claims = Arc<Mutex<Vec<Claim>>>;

pub(crate) struct MediaController {
    commands: queue::Sender<Command>,
    stopped: AtomicBool,
}

#[derive(Clone, Copy)]
enum Action {
    Pause,
    Resume,
    Shutdown,
}

struct Command {
    action: Action,
    deadline: Instant,
    reply: mpsc::SyncSender<Result<(), String>>,
}

struct Claim {
    name: String,
    owner: String,
    properties: Properties,
    successful: bool,
    valid: bool,
    awaiting_pause: bool,
}

impl MediaController {
    /// Starts a dedicated worker, without connecting to or controlling players.
    pub(crate) fn new() -> Result<Self, String> {
        let (commands, receiver) = queue::channel(4);
        std::thread::Builder::new()
            .name("koett-media".into())
            .spawn(move || {
                match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime.block_on(worker(receiver)),
                    Err(error) => {
                        let mut receiver = receiver;
                        while let Some(command) = receiver.blocking_recv() {
                            let _ = command.reply.send(Err(error.to_string()));
                            if matches!(command.action, Action::Shutdown) {
                                break;
                            }
                        }
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            commands,
            stopped: AtomicBool::new(false),
        })
    }

    /// Idempotent until resume; waits at most 550ms for the pre-capture ack.
    pub(crate) fn pause(&self) -> Result<(), String> {
        self.request(Action::Pause)
    }

    /// Consume and conditionally resume only this capture's successful pauses.
    pub(crate) fn resume(&self) -> Result<(), String> {
        self.request(Action::Resume)
    }

    /// Best-effort resume and stop. Drop calls this too; repeated shutdown is safe.
    pub(crate) fn shutdown(&self) -> Result<(), String> {
        if self.stopped.swap(true, Ordering::AcqRel) || self.commands.is_closed() {
            return Ok(());
        }
        self.request(Action::Shutdown)
    }

    fn request(&self, action: Action) -> Result<(), String> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.commands
            .try_send(Command {
                action,
                deadline: Instant::now() + WORK_WAIT,
                reply,
            })
            .map_err(|error| format!("Media worker unavailable: {error}"))?;
        receiver
            .recv_timeout(ACK_WAIT)
            .map_err(|error| format!("Media worker acknowledgement: {error}"))?
    }
}

impl Drop for MediaController {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

async fn worker(mut commands: queue::Receiver<Command>) {
    let claims = Arc::new(Mutex::new(Vec::new()));
    let mut session = None;
    let mut active = false;
    while let Some(command) = commands.recv().await {
        let result = async {
            if matches!(command.action, Action::Pause) {
                if active {
                    return Ok(());
                }
                active = true;
                if session.is_none() {
                    let connection = ipc(Connection::session()).await?;
                    let rule = MatchRule::builder().msg_type(Type::Signal).build();
                    let stream =
                        ipc(MessageStream::for_match_rule(rule, &connection, Some(256))).await?;
                    let monitor = tokio::spawn(monitor(stream, claims.clone()));
                    session = Some((connection, monitor));
                }
                if let Some((connection, monitor)) = &session {
                    if monitor.is_finished() {
                        return Err("MPRIS observation ended".into());
                    }
                    pause_players(connection, &claims).await?;
                }
            } else {
                active = false;
                if let Some((connection, monitor)) = &session {
                    if monitor.is_finished() {
                        return Err("MPRIS observation ended".into());
                    }
                    resume_players(connection, &claims).await?;
                }
            }
            Ok(())
        };
        let remaining = command.deadline.saturating_duration_since(Instant::now());
        let result = tokio::time::timeout(remaining, result)
            .await
            .unwrap_or_else(|_| Err("Media operation exceeded 400ms budget".into()));
        if !matches!(command.action, Action::Pause) {
            claims.lock().await.clear();
        }
        let _ = command.reply.send(result);
        if matches!(command.action, Action::Shutdown) {
            break;
        }
    }
    // Sender disappearance is also a shutdown path. No unbounded thread join.
    if let Some((connection, monitor)) = session {
        if !monitor.is_finished() {
            let _ = tokio::time::timeout(WORK_WAIT, resume_players(&connection, &claims)).await;
        }
        monitor.abort();
    }
}

async fn ipc<T>(future: impl std::future::Future<Output = zbus::Result<T>>) -> Result<T, String> {
    tokio::time::timeout(IPC_WAIT, future)
        .await
        .map_err(|_| "MPRIS request timed out".to_string())?
        .map_err(|error| error.to_string())
}

async fn proxy<'a>(
    connection: &'a Connection,
    owner: &'a str,
    interface: &'a str,
) -> Result<Proxy<'a>, String> {
    ipc(Proxy::new(connection, owner, PATH, interface)).await
}

async fn bus(connection: &Connection) -> Result<Proxy<'_>, String> {
    ipc(Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    ))
    .await
}

async fn properties(connection: &Connection, owner: &str) -> Result<Properties, String> {
    let proxy = proxy(connection, owner, "org.freedesktop.DBus.Properties").await?;
    ipc(proxy.call("GetAll", &(PLAYER,))).await
}

fn status(properties: &Properties, expected: &str) -> bool {
    properties
        .get("PlaybackStatus")
        .and_then(|value| <&str>::try_from(value).ok())
        == Some(expected)
}

fn capability(properties: &Properties, name: &str) -> bool {
    properties
        .get(name)
        .and_then(|value| bool::try_from(value).ok())
        == Some(true)
}

fn fingerprint(mut properties: Properties) -> Properties {
    properties.remove("PlaybackStatus");
    properties.remove("Position");
    properties
}

async fn pause_players(connection: &Connection, claims: &Claims) -> Result<(), String> {
    let bus = bus(connection).await?;
    let names: Vec<String> = ipc(bus.call("ListNames", &())).await?;
    let mut errors = Vec::new();
    for name in names.into_iter().filter(|name| name.starts_with(PREFIX)) {
        if let Err(error) = pause_player(connection, &bus, claims, name).await {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

async fn pause_player(
    connection: &Connection,
    bus: &Proxy<'_>,
    claims: &Claims,
    name: String,
) -> Result<(), String> {
    let owner: String = ipc(bus.call("GetNameOwner", &(&name,))).await?;
    let properties = properties(connection, &owner).await?;
    if !status(&properties, "Playing") || !capability(&properties, "CanPause") {
        return Ok(());
    }
    let player = proxy(connection, &owner, PLAYER).await?;
    // Register before sending Pause so its signal cannot outrun the claim.
    claims.lock().await.push(Claim {
        name: name.clone(),
        owner: owner.clone(),
        properties: fingerprint(properties),
        successful: false,
        valid: true,
        awaiting_pause: true,
    });
    let result: Result<(), String> = ipc(player.call("Pause", &())).await;
    let mut claims = claims.lock().await;
    if let Some(claim) = claims.iter_mut().find(|claim| claim.name == name) {
        claim.successful = result.is_ok();
    }
    result
}

async fn resume_players(connection: &Connection, claims: &Claims) -> Result<(), String> {
    // Keep claims visible to the monitor during the final IPC checks. Remove
    // each one immediately before Play so even failed requests are never retried.
    let names: Vec<String> = claims
        .lock()
        .await
        .iter()
        .map(|claim| claim.name.clone())
        .collect();
    let bus = bus(connection).await?;
    let mut errors = Vec::new();
    for name in names {
        let result = resume_player(connection, &bus, claims, &name).await;
        claims.lock().await.retain(|claim| claim.name != name);
        if let Err(error) = result {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

async fn resume_player(
    connection: &Connection,
    bus: &Proxy<'_>,
    claims: &Claims,
    name: &str,
) -> Result<(), String> {
    let owner: String = ipc(bus.call("GetNameOwner", &(name,))).await?;
    let current = properties(connection, &owner).await?;
    if !status(&current, "Paused") || !capability(&current, "CanPlay") {
        return Ok(());
    }
    let current = fingerprint(current);
    let player = proxy(connection, &owner, PLAYER).await?;
    let mut claims = claims.lock().await;
    let Some(index) = claims.iter().position(|claim| claim.name == name) else {
        return Ok(());
    };
    let claim = claims.remove(index);
    let resume =
        claim.successful && claim.valid && claim.owner == owner && claim.properties == current;
    drop(claims);
    if resume {
        ipc(player.call::<_, _, ()>("Play", &())).await?;
    }
    Ok(())
}

async fn monitor(mut stream: MessageStream, claims: Claims) {
    while let Some(message) = stream.next().await {
        let Ok(message) = message else {
            break;
        };
        let header = message.header();
        let member = header.member().map(|member| member.as_str());
        let interface = header.interface().map(|interface| interface.as_str());
        let sender = header.sender().map(|sender| sender.as_str());
        let mut claims = claims.lock().await;
        if interface == Some("org.freedesktop.DBus") && member == Some("NameOwnerChanged") {
            if let Ok((name, _, _)) = message.body().deserialize::<(String, String, String)>() {
                for claim in claims
                    .iter_mut()
                    .filter(|claim| claim.name == name || claim.owner == name)
                {
                    claim.valid = false;
                }
            }
        } else if header.path().map(|path| path.as_str()) == Some(PATH) {
            for claim in claims
                .iter_mut()
                .filter(|claim| Some(claim.owner.as_str()) == sender)
            {
                if interface == Some(PLAYER) && member == Some("Seeked") {
                    claim.valid = false;
                } else if interface == Some("org.freedesktop.DBus.Properties")
                    && member == Some("PropertiesChanged")
                {
                    match message
                        .body()
                        .deserialize::<(String, Properties, Vec<String>)>()
                    {
                        Ok((interface, changed, invalidated)) if interface == PLAYER => {
                            let own_pause = claim.awaiting_pause
                                && changed.len() == 1
                                && status(&changed, "Paused")
                                && invalidated.is_empty();
                            claim.awaiting_pause = false;
                            if !own_pause {
                                claim.valid = false;
                            }
                        }
                        Err(_) => claim.valid = false,
                        _ => {}
                    }
                }
            }
        }
    }
    // Lost observation means we can no longer establish pause ownership.
    for claim in claims.lock().await.iter_mut() {
        claim.valid = false;
    }
}

#[cfg(test)]
#[path = "media/tests.rs"]
mod tests;
