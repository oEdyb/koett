use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};
use windows::{
    Foundation::TypedEventHandler,
    Media::Control::*,
    Win32::System::Com::*,
    core::{Error, Result},
};
type Session = GlobalSystemMediaTransportControlsSession;
type Manager = GlobalSystemMediaTransportControlsSessionManager;
type Playback = GlobalSystemMediaTransportControlsSessionPlaybackStatus;
const WORK_WAIT: Duration = Duration::from_millis(400);
const ACK_WAIT: Duration = Duration::from_millis(550);
macro_rules! complete {
    ($operation:expr, $deadline:expr) => {{
        if Instant::now() >= $deadline {
            return Err(Error::new(
                windows::Win32::Foundation::E_ABORT,
                "Media control timed out",
            ));
        }
        let operation = $operation?;
        while operation.Status()?.0 == 0 {
            if Instant::now() >= $deadline {
                let _ = operation.Cancel();
                return Err(Error::new(
                    windows::Win32::Foundation::E_ABORT,
                    "Media control timed out",
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        operation.GetResults()?
    }};
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
    reply: mpsc::SyncSender<std::result::Result<(), String>>,
}
pub(crate) struct MediaController {
    commands: mpsc::SyncSender<Command>,
}
impl MediaController {
    pub(crate) fn new() -> std::result::Result<Self, String> {
        let (commands, receiver) = mpsc::sync_channel::<Command>(4);
        std::thread::Builder::new()
            .name("koett-media-control".into())
            .spawn(move || {
                let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() };
                let mut manager = None;
                let mut claim = None;
                while let Ok(command) = receiver.recv() {
                    let result = initialized.clone().and_then(|()| {
                        if Instant::now() >= command.deadline {
                            if matches!(command.action, Action::Resume | Action::Shutdown) {
                                claim = None;
                            }
                            return Err(Error::new(
                                windows::Win32::Foundation::E_ABORT,
                                "Media request expired",
                            ));
                        }
                        match command.action {
                            Action::Pause if claim.is_none() => {
                                pause(&mut manager, &mut claim, command.deadline)
                            }
                            Action::Pause => Ok(()),
                            Action::Resume | Action::Shutdown => {
                                resume(claim.take(), command.deadline)
                            }
                        }
                    });
                    let _ = command.reply.send(result.map_err(|e| e.to_string()));
                    if matches!(command.action, Action::Shutdown) {
                        break;
                    }
                }
                drop(claim);
                drop(manager);
                if initialized.is_ok() {
                    unsafe {
                        CoUninitialize();
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { commands })
    }
    fn request(&self, action: Action) -> std::result::Result<(), String> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.commands
            .try_send(Command {
                action,
                deadline: Instant::now() + WORK_WAIT,
                reply,
            })
            .map_err(|e| format!("Media worker unavailable: {e}"))?;
        receiver
            .recv_timeout(ACK_WAIT)
            .map_err(|_| "Media control timed out; dictation continues".to_string())?
    }
    pub(crate) fn pause(&self) -> std::result::Result<(), String> {
        self.request(Action::Pause)
    }
    pub(crate) fn resume(&self) -> std::result::Result<(), String> {
        self.request(Action::Resume)
    }
}
impl Drop for MediaController {
    fn drop(&mut self) {
        let _ = self.request(Action::Shutdown);
    }
}
struct Claim {
    session: Session,
    manager: Manager,
    valid: Arc<AtomicBool>,
    armed: Arc<AtomicBool>,
    successful: bool,
    position: i64,
    playback: Option<i64>,
    timeline: Option<i64>,
    properties: Option<i64>,
    current: Option<i64>,
}
impl Claim {
    fn new(session: Session, manager: Manager) -> Result<Self> {
        let mut claim = Self {
            session,
            manager,
            valid: Arc::new(AtomicBool::new(true)),
            armed: Arc::new(AtomicBool::new(false)),
            successful: false,
            position: 0,
            playback: None,
            timeline: None,
            properties: None,
            current: None,
        };
        let valid = claim.valid.clone();
        let session = claim.session.clone();
        let armed = claim.armed.clone();
        claim.playback =
            Some(claim.session.PlaybackInfoChanged(&TypedEventHandler::<
                Session,
                PlaybackInfoChangedEventArgs,
            >::new(move |_, _| {
                if armed.load(Ordering::Acquire)
                    && session
                        .GetPlaybackInfo()
                        .and_then(|info| info.PlaybackStatus())
                        .ok()
                        != Some(Playback::Paused)
                {
                    valid.store(false, Ordering::Release);
                }
                Ok(())
            }))?);
        let valid = claim.valid.clone();
        let armed = claim.armed.clone();
        claim.timeline = Some(
            claim
                .session
                .TimelinePropertiesChanged(&TypedEventHandler::<
                    Session,
                    TimelinePropertiesChangedEventArgs,
                >::new(move |_, _| {
                    if armed.load(Ordering::Acquire) {
                        valid.store(false, Ordering::Release);
                    }
                    Ok(())
                }))?,
        );
        let valid = claim.valid.clone();
        claim.properties =
            Some(claim.session.MediaPropertiesChanged(&TypedEventHandler::<
                Session,
                MediaPropertiesChangedEventArgs,
            >::new(move |_, _| {
                valid.store(false, Ordering::Release);
                Ok(())
            }))?);
        let valid = claim.valid.clone();
        let session = claim.session.clone();
        let manager = claim.manager.clone();
        claim.current =
            Some(claim.manager.CurrentSessionChanged(&TypedEventHandler::<
                Manager,
                CurrentSessionChangedEventArgs,
            >::new(move |_, _| {
                if manager.GetCurrentSession().ok().as_ref() != Some(&session) {
                    valid.store(false, Ordering::Release);
                }
                Ok(())
            }))?);
        Ok(claim)
    }
}
impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(t) = self.playback {
            let _ = self.session.RemovePlaybackInfoChanged(t);
        }
        if let Some(t) = self.timeline {
            let _ = self.session.RemoveTimelinePropertiesChanged(t);
        }
        if let Some(t) = self.properties {
            let _ = self.session.RemoveMediaPropertiesChanged(t);
        }
        if let Some(t) = self.current {
            let _ = self.manager.RemoveCurrentSessionChanged(t);
        }
    }
}
fn pause(
    manager: &mut Option<Manager>,
    claim: &mut Option<Claim>,
    deadline: Instant,
) -> Result<()> {
    if manager.is_none() {
        *manager = Some(complete!(Manager::RequestAsync(), deadline));
    }
    let manager = manager.as_ref().expect("manager prepared");
    let session = match manager.GetCurrentSession() {
        Ok(session) => session,
        Err(error) if error.code() == windows::Win32::Foundation::E_POINTER => return Ok(()),
        Err(error) => return Err(error),
    };
    let info = session.GetPlaybackInfo()?;
    if info.PlaybackStatus()? != Playback::Playing {
        return Ok(());
    }
    if !info.Controls()?.IsPauseEnabled()? {
        return Err(Error::new(
            windows::Win32::Foundation::E_NOTIMPL,
            "This player cannot pause",
        ));
    }
    *claim = Some(Claim::new(session, manager.clone())?);
    let owned = claim.as_mut().expect("claim prepared");
    owned.successful = complete!(owned.session.TryPauseAsync(), deadline);
    if !owned.successful {
        return Err(Error::new(
            windows::Win32::Foundation::E_FAIL,
            "Player refused to pause",
        ));
    }
    while owned.session.GetPlaybackInfo()?.PlaybackStatus()? != Playback::Paused {
        if Instant::now() >= deadline {
            return Err(Error::new(
                windows::Win32::Foundation::E_ABORT,
                "Player did not confirm pause",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    owned.position = owned.session.GetTimelineProperties()?.Position()?.Duration;
    owned.armed.store(true, Ordering::Release);
    Ok(())
}
fn resume(claim: Option<Claim>, deadline: Instant) -> Result<()> {
    let Some(claim) = claim else { return Ok(()) };
    if !claim.successful
        || !claim.valid.load(Ordering::Acquire)
        || claim.manager.GetCurrentSession().ok().as_ref() != Some(&claim.session)
    {
        return Ok(());
    }
    let info = claim.session.GetPlaybackInfo()?;
    if info.PlaybackStatus()? != Playback::Paused
        || claim.session.GetTimelineProperties()?.Position()?.Duration != claim.position
    {
        return Ok(());
    }
    if !info.Controls()?.IsPlayEnabled()? {
        return Err(Error::new(
            windows::Win32::Foundation::E_NOTIMPL,
            "This player cannot resume",
        ));
    }
    if !claim.valid.load(Ordering::Acquire) {
        return Ok(());
    }
    if !complete!(claim.session.TryPlayAsync(), deadline) {
        return Err(Error::new(
            windows::Win32::Foundation::E_FAIL,
            "Player refused to resume",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn current() -> (Manager, Session) {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        }
        let deadline = Instant::now() + WORK_WAIT;
        let manager =
            (|| -> Result<Manager> { Ok(complete!(Manager::RequestAsync(), deadline)) })().unwrap();
        let session = manager
            .GetCurrentSession()
            .expect("Start the controlled Edge audio fixture first");
        (manager, session)
    }
    fn status(session: &Session, expected: Playback) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let actual = session.GetPlaybackInfo().unwrap().PlaybackStatus().unwrap();
            if actual == expected {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "Expected {expected:?}, got {actual:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn action(session: &Session, play: bool) {
        let deadline = Instant::now() + WORK_WAIT;
        (|| -> Result<()> {
            assert!(complete!(
                if play {
                    session.TryPlayAsync()
                } else {
                    session.TryPauseAsync()
                },
                deadline
            ));
            Ok(())
        })()
        .unwrap();
        status(
            session,
            if play {
                Playback::Playing
            } else {
                Playback::Paused
            },
        );
    }
    #[test]
    fn caller_never_waits_indefinitely_for_a_stalled_worker() {
        let (commands, _receiver) = mpsc::sync_channel(4);
        let controller = MediaController { commands };
        let started = Instant::now();
        assert!(controller.pause().unwrap_err().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(_receiver);
    }
    #[test]
    #[ignore = "requires the isolated interactive Edge media fixture"]
    fn native_player_pause_resumes_once() {
        let (_manager, session) = current();
        action(&session, true);
        let controller = MediaController::new().unwrap();
        controller.pause().unwrap();
        status(&session, Playback::Paused);
        controller.resume().unwrap();
        status(&session, Playback::Playing);
        action(&session, false);
        controller.resume().unwrap();
        status(&session, Playback::Paused);
    }
    #[test]
    #[ignore = "requires the isolated interactive Edge media fixture"]
    fn an_already_paused_player_is_never_resumed() {
        let (_manager, session) = current();
        action(&session, false);
        let controller = MediaController::new().unwrap();
        controller.pause().unwrap();
        controller.resume().unwrap();
        status(&session, Playback::Paused);
    }
    #[test]
    #[ignore = "requires the isolated interactive Edge media fixture"]
    fn user_playback_changes_revoke_resume() {
        let (_manager, session) = current();
        action(&session, true);
        let controller = MediaController::new().unwrap();
        controller.pause().unwrap();
        status(&session, Playback::Paused);
        action(&session, true);
        std::thread::sleep(Duration::from_millis(100));
        action(&session, false);
        std::thread::sleep(Duration::from_millis(100));
        controller.resume().unwrap();
        status(&session, Playback::Paused);
    }
    #[test]
    #[ignore = "requires the isolated interactive Edge media fixture"]
    fn user_seek_revokes_resume() {
        let (_manager, session) = current();
        action(&session, true);
        let controller = MediaController::new().unwrap();
        controller.pause().unwrap();
        status(&session, Playback::Paused);
        let position = session
            .GetTimelineProperties()
            .unwrap()
            .Position()
            .unwrap()
            .Duration;
        let destination = if position > 20_000_000 { 0 } else { 30_000_000 };
        let deadline = Instant::now() + WORK_WAIT;
        (|| -> Result<()> {
            assert!(complete!(
                session.TryChangePlaybackPositionAsync(destination),
                deadline
            ));
            Ok(())
        })()
        .unwrap();
        std::thread::sleep(Duration::from_millis(150));
        controller.resume().unwrap();
        status(&session, Playback::Paused);
    }
}
