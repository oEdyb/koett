use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::bundle::Bundle;
use super::bundle::records::Intent;
use super::bundle::scan::{DiskState, scan, scan_all};
use super::session::{capture_bundle, drain, open, persist_intent, process_job};
use super::*;
use crate::runtime::{EngineCommand, EngineUpdate};
use crate::settings::Settings;

pub(crate) struct MeetingController {
    root: Result<PathBuf, String>,
    active: Active,
    attempt: u64,
    recoverable: Option<usize>,
    boot: Option<JoinHandle<()>>,
    worker: Option<Worker>,
    commands: Sender<EngineCommand>,
    updates: Sender<EngineUpdate>,
    reserved: &'static AtomicBool,
    disk_health: Tracks<bool>,
    disconnected: Tracks<Option<Instant>>,
}

enum Worker {
    Drain(Attempt, JoinHandle<()>),
    Job(Attempt, JoinHandle<()>, Arc<AtomicBool>),
}

impl Worker {
    fn attempt(&self) -> Attempt {
        match self {
            Self::Drain(attempt, _) | Self::Job(attempt, ..) => *attempt,
        }
    }

    fn join(self) {
        let worker = match self {
            Self::Drain(_, worker) | Self::Job(_, worker, _) => worker,
        };
        let _ = worker.join();
    }
}

impl MeetingController {
    pub(crate) fn new(
        commands: Sender<EngineCommand>,
        updates: Sender<EngineUpdate>,
        reserved: &'static AtomicBool,
    ) -> Self {
        let root = crate::paths::meetings_directory();
        let scan_root = root.clone();
        let post = commands.clone();
        let boot = thread::spawn(move || {
            let count = scan_root.and_then(|root| recoverable_count(&root));
            emit(&post, Event::BootScanned(count));
        });
        Self {
            root,
            active: Active::Idle,
            attempt: 0,
            recoverable: None,
            boot: Some(boot),
            worker: None,
            commands,
            updates,
            reserved,
            disk_health: Tracks::default(),
            disconnected: Tracks::default(),
        }
    }

    pub(crate) fn dispatch(&mut self, call: MeetingCall, engine_ready: bool, settings: &Settings) {
        let at = Instant::now();
        if let ReplyTarget::Socket(ticket, _) = &call.reply
            && !ticket.claim(at)
        {
            return;
        }
        let opening = engine_ready
            && matches!(self.active, Active::Idle)
            && matches!(
                call.request,
                MeetingRequest::Start(_) | MeetingRequest::Shortcut
            );
        let admission = opening.then(crate::linux::admit_meeting_start);
        let engine_ready = engine_ready && !matches!(admission, Some(None));
        let previous = self.active.id();
        let request = match self.admit(call.request, engine_ready, settings) {
            Ok(request) => self.apply(Event::Request(request, at)),
            Err(error) => Reply::Error(error),
        };
        match call.reply {
            ReplyTarget::Socket(_, reply) => {
                let _ = reply.send(request);
            }
            ReplyTarget::Desktop => {
                if let Reply::Error(error) = request {
                    self.notify(MeetingUpdate::Issue(error.message().into()));
                }
            }
        }
        if previous.is_none() && self.active.id().is_some() {
            self.disconnected = Tracks::default();
            self.disk_health = Tracks::default();
        }
    }

    pub(crate) fn receive(&mut self, MeetingEvent(event): MeetingEvent) {
        let event = match event {
            Event::BootScanned(result) => {
                if let Some(boot) = self.boot.take() {
                    let _ = boot.join();
                }
                match result {
                    Ok(count) if self.recoverable.is_none() => {
                        self.recoverable = Some(count);
                        if let Some(view) = self.active.view() {
                            self.notify(MeetingUpdate::View(view, count));
                        }
                    }
                    Err(issue) => self.notify(MeetingUpdate::Issue(issue)),
                    _ => {}
                }
                return;
            }
            Event::SourceHealth(attempt, channel, status) => {
                let Some(mut health) = self.current_health(attempt) else {
                    return;
                };
                *track_mut(&mut health.tracks, channel) = status;
                let since = track_mut(&mut self.disconnected, channel);
                match status {
                    SourceHealth::Receiving => *since = None,
                    SourceHealth::Reconnecting => {
                        since.get_or_insert_with(Instant::now);
                    }
                }
                Event::Health(attempt, health)
            }
            Event::DiskHealth(attempt, channel, status) => {
                let Some(mut health) = self.current_health(attempt) else {
                    return;
                };
                *track_mut(&mut self.disk_health, channel) =
                    matches!(status, capture::disk::CaptureHealth::DiskLow { .. });
                health.disk_low = self.disk_health.you || self.disk_health.other;
                Event::Health(attempt, health)
            }
            event => event,
        };
        if let Event::Closed(attempt, _) | Event::Processed(attempt, _) = &event
            && self
                .worker
                .as_ref()
                .is_some_and(|worker| worker.attempt() == *attempt)
            && let Some(worker) = self.worker.take()
        {
            worker.join();
        }
        self.apply(event);
        self.check_sources();
    }

    pub(crate) fn check_sources(&mut self) {
        let Some(live) = self.live() else { return };
        let (Some(you), Some(other)) = (self.disconnected.you, self.disconnected.other) else {
            return;
        };
        // Allow both sources to attempt a reconnect before cutting off the journals.
        let now = Instant::now();
        if now.saturating_duration_since(you.max(other)) >= Duration::from_secs(2) {
            self.apply(Event::CaptureFault(
                live.attempt,
                live.epoch.offset(now),
                "Both meeting audio sources are unavailable".into(),
            ));
        }
    }

    pub(crate) fn reserves_engine(&self) -> bool {
        self.active.reserves_engine()
    }

    pub(crate) fn shutdown(&mut self) {
        self.apply(Event::Shutdown(Instant::now()));
        if let Some(worker) = self.worker.take() {
            worker.join();
        }
        if let Some(boot) = self.boot.take() {
            let _ = boot.join();
        }
        self.reserved.store(false, Ordering::Release);
    }

    fn live(&self) -> Option<&Live> {
        match &self.active {
            Active::Recording(live) | Active::ConfirmStop { live, .. } => Some(live),
            _ => None,
        }
    }

    fn current_health(&self, attempt: Attempt) -> Option<CaptureHealth> {
        self.live()
            .filter(|live| live.attempt == attempt)
            .map(|live| live.health)
    }

    fn root(&self) -> Result<&Path, ReplyError> {
        self.root
            .as_deref()
            .map_err(|error| ReplyError::DiskUnreadable(error.clone()))
    }

    fn admit(
        &mut self,
        request: MeetingRequest,
        engine_ready: bool,
        settings: &Settings,
    ) -> Result<AdmittedRequest, ReplyError> {
        use MeetingRequest as Request;
        Ok(match request {
            Request::Start(id) => {
                let replay = if self.active.id() == Some(id) {
                    self.active.view()
                } else {
                    self.disk_view(id)?
                };
                if let Some(view) = replay {
                    AdmittedRequest::Replay(view)
                } else {
                    AdmittedRequest::Start(self.admit_start(id, engine_ready, settings))
                }
            }
            Request::Shortcut => AdmittedRequest::Shortcut(self.admit_start(
                MeetingId::new(),
                engine_ready,
                settings,
            )),
            Request::Stop(id) => AdmittedRequest::Stop(id),
            Request::Resume(key) => AdmittedRequest::Resume(key),
            Request::Finish(key) => AdmittedRequest::Finish(key),
            Request::Cancel(key) => AdmittedRequest::Cancel(key),
            Request::Recover { id, backend } => {
                let job = if matches!(self.active, Active::Idle) {
                    self.recover(id, backend)
                } else {
                    Err(ReplyError::WrongState)
                };
                AdmittedRequest::Recover(job)
            }
            Request::Status(id) => AdmittedRequest::Status {
                id,
                disk_view: match id {
                    Some(id) if self.active.id() != Some(id) => self.disk_view(id)?,
                    _ => None,
                },
            },
        })
    }

    fn disk_view(&self, id: MeetingId) -> Result<Option<MeetingView>, ReplyError> {
        let root = self.root()?;
        let state =
            scan(root, id).map_err(|error| ReplyError::DiskUnreadable(error.to_string()))?;
        Ok(state.map(|state| state.view(root)))
    }

    fn admit_start(
        &mut self,
        id: MeetingId,
        engine_ready: bool,
        defaults: &Settings,
    ) -> StartAdmission {
        if !engine_ready || !matches!(self.active, Active::Idle) {
            return StartAdmission::Busy;
        }
        let info = SessionInfo {
            id,
            started_at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|span| u64::try_from(span.as_millis()).ok())
                .unwrap_or_default(),
        };
        let stored = Settings::load();
        let settings = stored.as_ref().unwrap_or(defaults);
        StartAdmission::New {
            info: info.clone(),
            attempt: self.new_attempt(),
            plan: SessionPlan {
                manifest: Manifest {
                    info,
                    mic: settings.microphone.clone(),
                    backend_pref: if settings.meeting.gpu_python.is_some() {
                        Backend::Gpu
                    } else {
                        Backend::Parakeet
                    },
                    keep_audio: settings.meeting.keep_audio,
                    export_dir: settings.meeting.export_directory.clone(),
                },
            },
        }
    }

    fn recover(
        &mut self,
        id: MeetingId,
        backend: Option<Backend>,
    ) -> Result<RecoveryJob, ReplyError> {
        let bundle = Bundle::open(self.root()?, id).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => ReplyError::NotFound,
            io::ErrorKind::WouldBlock => ReplyError::EngineBusy,
            _ => ReplyError::DiskUnreadable(error.to_string()),
        })?;
        match bundle.read_state() {
            DiskState::Recoverable { .. }
            | DiskState::Complete {
                audio_retained: true,
                ..
            } => {}
            _ => return Err(ReplyError::WrongState),
        }
        Ok(RecoveryJob {
            info: bundle.manifest().info.clone(),
            attempt: self.new_attempt(),
            input: JobInput {
                source: JobSource::Recover {
                    bundle: RecoverableBundle::Locked(Arc::new(bundle)),
                    backend,
                },
            },
        })
    }

    fn new_attempt(&mut self) -> Attempt {
        self.attempt += 1;
        Attempt(self.attempt)
    }

    fn apply(&mut self, event: Event) -> Reply {
        let mut bundle = self
            .live()
            .and_then(|live| capture_bundle(&live.capture))
            .cloned();
        let (active, effects, mut reply) =
            next(std::mem::replace(&mut self.active, Active::Idle), event);
        self.active = active;
        self.reserved
            .store(self.reserves_engine(), Ordering::Release);
        for effect in effects {
            let completion = match effect {
                Effect::Open(attempt, plan) => {
                    let result = self
                        .root()
                        .map_err(|error| OpenError::CaptureUnavailable(error.message().into()))
                        .and_then(|root| open(root, attempt, plan, &self.commands));

                    Some(Event::Opened(attempt, result))
                }
                Effect::PersistIntent(spec, policy) => {
                    let intent = Intent {
                        goal: spec.goal,
                        cutoff: spec.cutoff,
                    };
                    match (&self.active, policy) {
                        (Active::Committing { live, .. }, IntentPolicy::Required) => {
                            let result = persist_intent(&live.capture, &intent);
                            Some(Event::IntentWritten(live.attempt, result))
                        }
                        (_, IntentPolicy::BestEffort) => {
                            if let Some(bundle) = bundle.take()
                                && let Err(error) = bundle.write_intent(&intent)
                            {
                                self.notify(MeetingUpdate::Issue(error.to_string()));
                            }
                            None
                        }
                        _ => None,
                    }
                }
                Effect::Drain(capture, spec, mode) => {
                    drop(bundle.take());
                    if let Active::Closing { attempt, .. } = self.active {
                        match mode {
                            DrainMode::Inline => Some(Event::Closed(attempt, drain(capture, spec))),
                            DrainMode::Worker => {
                                let post = self.commands.clone();
                                let worker = thread::spawn(move || {
                                    emit(&post, Event::Closed(attempt, drain(capture, spec)))
                                });
                                self.worker = Some(Worker::Drain(attempt, worker));
                                None
                            }
                        }
                    } else {
                        None
                    }
                }
                Effect::Process(job) => {
                    if let Active::Processing { attempt, .. } = self.active {
                        let post = self.commands.clone();
                        let cancelled = Arc::new(AtomicBool::new(false));
                        let token = cancelled.clone();
                        let worker = thread::spawn(move || {
                            emit(&post, Event::Processed(attempt, process_job(job, &token)));
                        });
                        self.worker = Some(Worker::Job(attempt, worker, cancelled));
                    }
                    None
                }
                Effect::CancelJob(attempt) => {
                    if let Some(Worker::Job(active, _, cancelled)) = &self.worker
                        && *active == attempt
                    {
                        cancelled.store(true, Ordering::Release);
                    }
                    None
                }
                Effect::NotifyView(view) => {
                    if view == MeetingView::Idle {
                        self.refresh_count();
                    }
                    self.notify(MeetingUpdate::View(view, self.recoverable.unwrap_or(0)));
                    None
                }
                Effect::Notify(update) => {
                    self.notify(update);
                    None
                }
            };
            if let Some(completion) = completion {
                let result = self.apply(completion);
                if !matches!(result, Reply::None) {
                    reply = result;
                }
            }
        }
        reply
    }

    fn refresh_count(&mut self) {
        if let Ok(count) = self
            .root()
            .and_then(|root| recoverable_count(root).map_err(ReplyError::DiskUnreadable))
        {
            self.recoverable = Some(count);
        }
    }

    fn notify(&self, update: MeetingUpdate) {
        let _ = self.updates.send(EngineUpdate::Meeting(update));
    }
}

impl Drop for MeetingController {
    fn drop(&mut self) {
        self.reserved.store(false, Ordering::Release);
    }
}

pub(super) fn emit(post: &Sender<EngineCommand>, event: Event) {
    let _ = post.send(EngineCommand::MeetingEvent(MeetingEvent(event)));
}

fn track_mut<T>(tracks: &mut Tracks<T>, channel: Channel) -> &mut T {
    match channel {
        Channel::You => &mut tracks.you,
        Channel::Other => &mut tracks.other,
    }
}

fn recoverable_count(root: &Path) -> Result<usize, String> {
    scan_all(root)
        .map(|states| states.iter().filter(|state| state.is_recoverable()).count())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
