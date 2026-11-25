pub(crate) mod bundle;
pub(crate) mod capture;
pub(crate) mod catalog;
mod controller;
mod job;
mod session;
mod ticket;
mod transcript;

pub(crate) use controller::MeetingController;
pub(crate) use ticket::Ticket;
use transcript::absolute_mark_links;

use bundle::marks::MarkSink;
pub(crate) use bundle::records::Projections;
use bundle::records::{Manifest, Seal};
use session::CaptureResources;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use std::time::Instant;

use uuid::Uuid;

pub(crate) const SAMPLE_RATE: u32 = 16_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MeetingId(Uuid);

impl MeetingId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        Uuid::parse_str(text).ok().map(Self)
    }

    pub(crate) fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for MeetingId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_uuid().hyphenated().fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Offset(pub(crate) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) first: Offset,
    pub(crate) end: Offset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Channel {
    You,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Tracks<T> {
    pub(crate) you: T,
    pub(crate) other: T,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TrackSeal {
    pub(crate) valid_bytes: u64,
    pub(crate) captured_until: Offset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Backend {
    Gpu,
    Parakeet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CloseGoal {
    Finish,
    Hold,
    Interrupted(InterruptCause),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InterruptCause {
    Shutdown,
    CaptureFault(String),
    IntentUncertain(String),
}

impl InterruptCause {
    pub(crate) fn recovery_reason(&self) -> RecoveryReason {
        match self {
            Self::Shutdown | Self::IntentUncertain(_) => RecoveryReason::Interrupted,
            Self::CaptureFault(issue) => RecoveryReason::CaptureFailed(issue.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryReason {
    Interrupted,
    HeldByUser,
    CaptureFailed(String),
    DecodeFailed(String),
    PublishFailed(String),
    FallbackUsed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionInfo {
    pub(crate) id: MeetingId,
    pub(crate) started_at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StopKey {
    pub(crate) id: MeetingId,
    pub(crate) generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Attempt(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MeetingEpoch(Instant);

impl MeetingEpoch {
    fn offset(self, at: Instant) -> Offset {
        let elapsed = at.saturating_duration_since(self.0);
        let rate = u64::from(SAMPLE_RATE);
        let seconds = elapsed.as_secs().saturating_mul(rate);
        let fractional = u64::from(elapsed.subsec_nanos()) * rate / 1_000_000_000;
        Offset(seconds.saturating_add(fractional))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MeetingRequest {
    Start(MeetingId),
    Shortcut,
    Stop(MeetingId),
    Resume(StopKey),
    Finish(StopKey),
    Cancel(StopKey),
    Recover {
        id: MeetingId,
        backend: Option<Backend>,
    },
    Status(Option<MeetingId>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MeetingCall {
    request: MeetingRequest,
    reply: ReplyTarget,
}

impl MeetingCall {
    pub(crate) fn desktop(request: MeetingRequest) -> Self {
        Self {
            request,
            reply: ReplyTarget::Desktop,
        }
    }

    pub(crate) fn socket(request: MeetingRequest, ticket: Ticket, reply: Sender<Reply>) -> Self {
        Self {
            request,
            reply: ReplyTarget::Socket(ticket, reply),
        }
    }
    #[cfg(test)]
    pub(crate) fn test_reply(self, result: Reply) {
        let ReplyTarget::Socket(ticket, reply) = self.reply else {
            panic!("expected socket call")
        };
        assert!(ticket.claim(Instant::now()));
        reply.send(result).unwrap();
    }
}

#[derive(Clone, Debug)]
enum ReplyTarget {
    Desktop,
    Socket(Ticket, Sender<Reply>),
}

impl PartialEq for ReplyTarget {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Desktop, Self::Desktop) => true,
            (Self::Socket(ticket, _), Self::Socket(other, _)) => ticket == other,
            _ => false,
        }
    }
}

impl Eq for ReplyTarget {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MeetingView {
    Idle,
    Recording {
        info: SessionInfo,
        health: CaptureHealth,
    },
    ConfirmStop {
        info: SessionInfo,
        health: CaptureHealth,
        key: StopKey,
    },
    Closing {
        info: SessionInfo,
        goal: CloseGoal,
    },
    Processing {
        info: SessionInfo,
        done: f32,
    },
    Active(SessionInfo),
    Recoverable(SessionInfo, RecoveryReason),
    Complete(SessionInfo, Artifact, Projections),
    Unreadable {
        id: MeetingId,
        issue: String,
    },
}

#[derive(Debug, PartialEq)]
pub(crate) enum MeetingUpdate {
    /// The view, then the meetings `recover` would run on, newest first.
    View(MeetingView, Vec<catalog::Listed>),
    Sketch(SketchScope),
    HistoryChanged,
    Issue(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SketchScope {
    Closed,
    Open(MeetingMarkScope),
    Finishing(MeetingId, Offset),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MeetingMarkScope {
    id: MeetingId,
    epoch: MeetingEpoch,
    sink: MarkSink,
}

impl MeetingMarkScope {
    #[cfg(test)]
    pub(crate) fn writing_to(
        directory: &std::path::Path,
        zero: Instant,
    ) -> (Self, bundle::marks::MarkWriter) {
        let writer = bundle::marks::MarkWriter::start(directory).unwrap();
        let scope = Self {
            id: MeetingId::new(),
            epoch: MeetingEpoch(zero),
            sink: writer.sink(),
        };
        (scope, writer)
    }

    pub(crate) fn id(&self) -> MeetingId {
        self.id
    }
    pub(crate) fn offset(&self, at: Instant) -> Offset {
        self.epoch.offset(at)
    }
    pub(crate) fn begin(&self, id: u64, at: Instant) -> bool {
        self.sink.begin(id, self.offset(at))
    }
    pub(crate) fn submit(&self, id: u64, offset: Offset, png: Vec<u8>) -> bool {
        self.sink.submit(id, offset, png)
    }
    pub(crate) fn undo(&self, id: u64) -> bool {
        self.sink.undo(id)
    }
    pub(crate) fn miss(&self, id: u64, issue: String) -> bool {
        self.sink.miss(id, issue)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CaptureHealth {
    pub(crate) tracks: Tracks<SourceHealth>,
    pub(crate) disk_low: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SourceHealth {
    #[default]
    Receiving,
    Reconnecting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Artifact {
    pub(crate) markdown: PathBuf,
    pub(crate) assets: Vec<PathBuf>,
    pub(crate) backends: std::collections::BTreeMap<String, usize>,
    pub(crate) fallback: bool,
    pub(crate) audio_retained: bool,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum LiveCapture {
    Armed(Box<CaptureResources>),
    #[cfg(test)]
    Silent(session::SilentCapture),
    #[cfg(test)]
    Fixture(MarkSink),
}

impl LiveCapture {
    fn marks(&self) -> MarkSink {
        match self {
            Self::Armed(capture) => capture.marks.sink(),
            #[cfg(test)]
            Self::Silent(capture) => capture.marks.sink(),
            #[cfg(test)]
            Self::Fixture(sink) => sink.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SessionPlan {
    manifest: Manifest,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum SealedBundle {
    Locked {
        bundle: std::sync::Arc<bundle::Bundle>,
        seal: Seal,
    },
    #[cfg(test)]
    Fixture,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum RecoverableBundle {
    Locked(std::sync::Arc<bundle::Bundle>),
    #[cfg(test)]
    Fixture,
}

struct Published {
    artifact: Artifact,
    projections: Projections,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct JobInput {
    source: JobSource,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum JobSource {
    Finished(SealedBundle),
    Recover {
        bundle: RecoverableBundle,
        backend: Option<Backend>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum MeetingActivity {
    Idle,
    Reserved,
    Processing,
}

enum Active {
    Idle,
    Opening {
        info: SessionInfo,
        attempt: Attempt,
    },
    Recording(Live),
    ConfirmStop {
        live: Live,
        key: StopKey,
    },
    Committing {
        live: Live,
        key: StopKey,
        spec: CloseSpec,
    },
    Closing {
        info: SessionInfo,
        attempt: Attempt,
        goal: CloseGoal,
    },
    Processing {
        info: SessionInfo,
        attempt: Attempt,
    },
}

impl Active {
    fn reserves_engine(&self) -> bool {
        self.activity() == MeetingActivity::Reserved
    }

    fn activity(&self) -> MeetingActivity {
        match self {
            Self::Idle => MeetingActivity::Idle,
            Self::Processing { .. } => MeetingActivity::Processing,
            _ => MeetingActivity::Reserved,
        }
    }

    fn view(&self) -> Option<MeetingView> {
        Some(match self {
            Self::Idle => MeetingView::Idle,
            Self::Recording(live) => MeetingView::Recording {
                info: live.info.clone(),
                health: live.health,
            },
            Self::ConfirmStop { live, key } => MeetingView::ConfirmStop {
                info: live.info.clone(),
                health: live.health,
                key: *key,
            },
            Self::Closing { info, goal, .. } => MeetingView::Closing {
                info: info.clone(),
                goal: goal.clone(),
            },
            Self::Processing { info, .. } => MeetingView::Processing {
                info: info.clone(),
                done: 0.0,
            },
            Self::Opening { .. } | Self::Committing { .. } => return None,
        })
    }

    fn id(&self) -> Option<MeetingId> {
        match self {
            Self::Idle => None,
            Self::Opening { info, .. }
            | Self::Closing { info, .. }
            | Self::Processing { info, .. } => Some(info.id),
            Self::Recording(live)
            | Self::ConfirmStop { live, .. }
            | Self::Committing { live, .. } => Some(live.info.id),
        }
    }
}

struct Live {
    info: SessionInfo,
    attempt: Attempt,
    capture: LiveCapture,
    epoch: MeetingEpoch,
    health: CaptureHealth,
    generation: u64,
}

struct OpenedCapture {
    capture: LiveCapture,
    epoch: MeetingEpoch,
    health: CaptureHealth,
}

enum OpenError {
    CaptureUnavailable(String),
    DiskLow(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CloseSpec {
    cutoff: Offset,
    goal: CloseGoal,
}

enum IntentWrite {
    Committed,
    Rejected(String),
    Indeterminate(String),
}

enum AdmittedRequest {
    Start(StartAdmission),
    Replay(MeetingView),
    Shortcut(StartAdmission),
    Stop(MeetingId),
    Resume(StopKey),
    Finish(StopKey),
    Cancel(StopKey),
    Recover(Result<RecoveryJob, ReplyError>),
    Status {
        id: Option<MeetingId>,
        disk_view: Option<MeetingView>,
    },
}

enum StartAdmission {
    Busy,
    New {
        info: SessionInfo,
        plan: SessionPlan,
        attempt: Attempt,
    },
}

struct RecoveryJob {
    info: SessionInfo,
    attempt: Attempt,
    input: JobInput,
}

enum Event {
    Request(AdmittedRequest, Instant),
    Opened(Attempt, Result<OpenedCapture, OpenError>),
    IntentWritten(Attempt, IntentWrite),
    Health(Attempt, CaptureHealth),
    SourceHealth(Attempt, Channel, SourceHealth),
    DiskHealth(Attempt, Channel, capture::disk::CaptureHealth),
    BootScanned(Result<Vec<catalog::Listed>, String>),
    CaptureFault(Attempt, Offset, String),
    Closed(Attempt, Result<SealedBundle, (RecoverableBundle, String)>),
    Processed(
        Attempt,
        Result<Published, (RecoverableBundle, RecoveryReason)>,
    ),
    Shutdown(Instant),
}

pub(crate) struct MeetingEvent(Event);

#[derive(Debug, PartialEq, Eq)]
enum IntentPolicy {
    Required,
    BestEffort,
}

#[derive(Debug, PartialEq, Eq)]
enum DrainMode {
    Worker,
    Inline,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum Effect {
    Open(Attempt, SessionPlan),
    PersistIntent(CloseSpec, IntentPolicy),
    Drain(LiveCapture, CloseSpec, DrainMode),
    Process(JobInput),
    CancelJob(Attempt),
    NotifyView(MeetingView),
    Notify(MeetingUpdate),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Reply {
    None,
    View(MeetingView),
    Error(ReplyError),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReplyError {
    DaemonUnavailable,
    EngineBusy,
    OutcomeUnknown,
    NotFound,
    WrongState,
    StaleStopToken,
    CaptureUnavailable(String),
    DiskLow(String),
    IntentWriteFailed(String),
    ArtifactPending,
    HelperUnavailable,
    DiskUnreadable(String),
}

impl ReplyError {
    pub(crate) fn message(&self) -> &str {
        match self {
            Self::DaemonUnavailable => "Start Koett before running this command.",
            Self::EngineBusy => "Koett is busy. Try again when it is ready.",
            Self::OutcomeUnknown => {
                "The request was admitted. Check meeting status before retrying."
            }
            Self::NotFound => "No meeting with that ID was found.",
            Self::WrongState => "This command is unavailable in the current meeting state.",
            Self::StaleStopToken => "Use the token from the current stop confirmation.",
            Self::CaptureUnavailable(issue)
            | Self::DiskLow(issue)
            | Self::IntentWriteFailed(issue)
            | Self::DiskUnreadable(issue) => issue,
            Self::ArtifactPending => "This meeting does not have a committed transcript.",
            Self::HelperUnavailable => "The GPU meeting decoder is unavailable in this build.",
        }
    }

    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::DaemonUnavailable => "daemon_unavailable",
            Self::EngineBusy => "engine_busy",
            Self::OutcomeUnknown => "outcome_unknown",
            Self::NotFound => "not_found",
            Self::WrongState => "wrong_state",
            Self::StaleStopToken => "stale_stop_token",
            Self::CaptureUnavailable(_) => "capture_unavailable",
            Self::DiskLow(_) => "disk_low",
            Self::IntentWriteFailed(_) => "intent_write_failed",
            Self::ArtifactPending => "artifact_pending",
            Self::HelperUnavailable => "helper_unavailable",
            Self::DiskUnreadable(_) => "disk_unreadable",
        }
    }
}

type Transition = (Active, Vec<Effect>, Reply);

fn next(active: Active, event: Event) -> Transition {
    use AdmittedRequest as Request;

    match (active, event) {
        (active, Event::Request(Request::Start(StartAdmission::Busy), _))
        | (active @ Active::Idle, Event::Request(Request::Shortcut(StartAdmission::Busy), _)) => {
            (active, vec![], Reply::Error(ReplyError::EngineBusy))
        }
        (active, Event::Request(Request::Replay(view), _)) => (active, vec![], Reply::View(view)),
        (
            Active::Idle,
            Event::Request(
                Request::Start(StartAdmission::New {
                    info,
                    plan,
                    attempt,
                })
                | Request::Shortcut(StartAdmission::New {
                    info,
                    plan,
                    attempt,
                }),
                _,
            ),
        ) => (
            Active::Opening { info, attempt },
            vec![Effect::Open(attempt, plan)],
            Reply::None,
        ),
        (Active::Opening { info, attempt, .. }, Event::Opened(completed, result))
            if attempt == completed =>
        {
            match result {
                Ok(opened) => {
                    let scope = MeetingMarkScope {
                        id: info.id,
                        epoch: opened.epoch,
                        sink: opened.capture.marks(),
                    };
                    let live = Live {
                        info,
                        attempt,
                        capture: opened.capture,
                        epoch: opened.epoch,
                        health: opened.health,
                        generation: 0,
                    };
                    viewed(
                        Active::Recording(live),
                        vec![Effect::Notify(MeetingUpdate::Sketch(SketchScope::Open(
                            scope,
                        )))],
                    )
                }
                Err(error) => {
                    let error = match error {
                        OpenError::CaptureUnavailable(reason) => {
                            ReplyError::CaptureUnavailable(reason)
                        }
                        OpenError::DiskLow(reason) => ReplyError::DiskLow(reason),
                    };
                    (Active::Idle, vec![], Reply::Error(error))
                }
            }
        }
        (Active::Recording(live), Event::Request(request, _))
            if matches!(&request, Request::Shortcut(_))
                || matches!(&request, Request::Stop(id) if *id == live.info.id) =>
        {
            confirm_stop(live)
        }
        (active @ Active::ConfirmStop { key, .. }, Event::Request(request, _))
            if matches!(&request, Request::Shortcut(_))
                || matches!(&request, Request::Stop(id) if *id == key.id) =>
        {
            let reply = active.view().map_or(Reply::None, Reply::View);
            (active, vec![], reply)
        }
        (
            active,
            Event::Request(
                Request::Start(StartAdmission::New { .. })
                | Request::Shortcut(StartAdmission::New { .. }),
                _,
            ),
        ) => (active, vec![], Reply::Error(ReplyError::EngineBusy)),
        (Active::ConfirmStop { live, key }, Event::Request(Request::Resume(current), _))
            if key == current =>
        {
            viewed(Active::Recording(live), vec![])
        }
        (
            Active::ConfirmStop { live, key },
            Event::Request(
                request @ (Request::Finish(current) | Request::Cancel(current)),
                admitted,
            ),
        ) if key == current => {
            let spec = CloseSpec {
                cutoff: live.epoch.offset(admitted),
                goal: if matches!(request, Request::Finish(_)) {
                    CloseGoal::Finish
                } else {
                    CloseGoal::Hold
                },
            };
            let effect = Effect::PersistIntent(spec.clone(), IntentPolicy::Required);
            (
                Active::Committing { live, key, spec },
                vec![effect],
                Reply::None,
            )
        }
        (
            Active::Committing {
                live,
                key,
                mut spec,
            },
            Event::IntentWritten(attempt, result),
        ) if live.attempt == attempt => match result {
            IntentWrite::Committed => close(live, spec, DrainMode::Worker, vec![]),
            IntentWrite::Rejected(reason) => (
                Active::ConfirmStop { live, key },
                vec![],
                Reply::Error(ReplyError::IntentWriteFailed(reason)),
            ),
            IntentWrite::Indeterminate(reason) => {
                spec.goal = CloseGoal::Interrupted(InterruptCause::IntentUncertain(reason));
                close(live, spec, DrainMode::Worker, vec![])
            }
        },
        (Active::Recording(mut live), Event::Health(attempt, health))
            if live.attempt == attempt =>
        {
            live.health = health;
            notify_view(Active::Recording(live), vec![])
        }
        (Active::ConfirmStop { mut live, key }, Event::Health(attempt, health))
            if live.attempt == attempt =>
        {
            live.health = health;
            notify_view(Active::ConfirmStop { live, key }, vec![])
        }
        (
            Active::Recording(live) | Active::ConfirmStop { live, .. },
            Event::CaptureFault(attempt, cutoff, reason),
        ) if live.attempt == attempt => interrupt(
            live,
            cutoff,
            InterruptCause::CaptureFault(reason),
            DrainMode::Worker,
        ),
        (
            Active::Closing {
                info,
                attempt,
                goal,
            },
            Event::Closed(completed, result),
        ) if attempt == completed => {
            let effects = vec![Effect::Notify(MeetingUpdate::Sketch(SketchScope::Closed))];
            match result {
                Ok(bundle) if goal == CloseGoal::Finish => process(
                    info,
                    attempt,
                    JobInput {
                        source: JobSource::Finished(bundle),
                    },
                    effects,
                ),
                Ok(_) => notify_view(Active::Idle, effects),
                Err((_, issue)) => {
                    let mut effects = effects;
                    effects.push(Effect::Notify(MeetingUpdate::Issue(issue)));
                    notify_view(Active::Idle, effects)
                }
            }
        }
        (Active::Idle, Event::Request(Request::Recover(result), _)) => match result {
            Ok(job) => viewed(
                Active::Processing {
                    info: job.info,
                    attempt: job.attempt,
                },
                vec![Effect::Process(job.input)],
            ),
            Err(error) => (Active::Idle, vec![], Reply::Error(error)),
        },
        (Active::Processing { attempt, .. }, Event::Processed(completed, result))
            if attempt == completed =>
        {
            let update = match result {
                Ok(_) => MeetingUpdate::HistoryChanged,
                Err((_, reason)) => MeetingUpdate::Issue(recovery_issue(reason)),
            };
            notify_view(Active::Idle, vec![Effect::Notify(update)])
        }
        (Active::Recording(live) | Active::ConfirmStop { live, .. }, Event::Shutdown(at)) => {
            let cutoff = live.epoch.offset(at);
            interrupt(live, cutoff, InterruptCause::Shutdown, DrainMode::Inline)
        }
        (Active::Processing { attempt, .. }, Event::Shutdown(_)) => {
            (Active::Idle, vec![Effect::CancelJob(attempt)], Reply::None)
        }
        (active, Event::Request(Request::Status { id, disk_view }, _)) => {
            let view = if id.is_none() || id == active.id() {
                active.view()
            } else {
                disk_view
            };
            let reply = view.map_or(Reply::Error(ReplyError::NotFound), Reply::View);
            (active, vec![], reply)
        }
        (
            active,
            Event::Request(Request::Resume(key) | Request::Finish(key) | Request::Cancel(key), _),
        ) if active.id().is_some_and(|id| id != key.id)
            || matches!(active, Active::Recording(_) | Active::ConfirmStop { .. }) =>
        {
            (active, vec![], Reply::Error(ReplyError::StaleStopToken))
        }
        (active, Event::Request(_, _)) => (active, vec![], Reply::Error(ReplyError::WrongState)),
        (active, _) => (active, vec![], Reply::None),
    }
}

fn confirm_stop(mut live: Live) -> Transition {
    live.generation += 1;
    let key = StopKey {
        id: live.info.id,
        generation: live.generation,
    };
    viewed(Active::ConfirmStop { live, key }, vec![])
}

fn close(live: Live, spec: CloseSpec, mode: DrainMode, mut effects: Vec<Effect>) -> Transition {
    effects.push(Effect::Notify(MeetingUpdate::Sketch(
        SketchScope::Finishing(live.info.id, spec.cutoff),
    )));
    let active = Active::Closing {
        info: live.info,
        attempt: live.attempt,
        goal: spec.goal.clone(),
    };
    let drain = Effect::Drain(live.capture, spec, mode);
    let (active, mut effects, reply) = viewed(active, effects);
    effects.push(drain);
    (active, effects, reply)
}

fn interrupt(live: Live, cutoff: Offset, cause: InterruptCause, mode: DrainMode) -> Transition {
    let spec = CloseSpec {
        cutoff,
        goal: CloseGoal::Interrupted(cause),
    };
    let effect = Effect::PersistIntent(spec.clone(), IntentPolicy::BestEffort);
    let (active, effects, _) = close(live, spec, mode, vec![effect]);
    (active, effects, Reply::None)
}

fn process(info: SessionInfo, attempt: Attempt, job: JobInput, effects: Vec<Effect>) -> Transition {
    let (active, mut effects, reply) = notify_view(Active::Processing { info, attempt }, effects);
    effects.push(Effect::Process(job));
    (active, effects, reply)
}

fn viewed(active: Active, mut effects: Vec<Effect>) -> Transition {
    let reply = if let Some(view) = active.view() {
        effects.push(Effect::NotifyView(view.clone()));
        Reply::View(view)
    } else {
        Reply::None
    };
    (active, effects, reply)
}

fn notify_view(active: Active, mut effects: Vec<Effect>) -> Transition {
    if let Some(view) = active.view() {
        effects.push(Effect::NotifyView(view));
    }
    (active, effects, Reply::None)
}

fn recovery_issue(reason: RecoveryReason) -> String {
    match reason {
        RecoveryReason::Interrupted => "Meeting interrupted".into(),
        RecoveryReason::HeldByUser => "Meeting held".into(),
        RecoveryReason::FallbackUsed => "Meeting used the fallback decoder".into(),
        RecoveryReason::CaptureFailed(issue)
        | RecoveryReason::DecodeFailed(issue)
        | RecoveryReason::PublishFailed(issue) => issue,
    }
}

#[cfg(test)]
mod machine;
