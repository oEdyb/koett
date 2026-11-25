use super::*;
use std::time::Duration;

fn token(reply: &Reply) -> Option<StopKey> {
    match reply {
        Reply::View(MeetingView::ConfirmStop { key, .. }) => Some(*key),
        _ => None,
    }
}

struct Session {
    info: SessionInfo,
    zero: Instant,
    attempt: Attempt,
}

impl Session {
    fn new() -> Self {
        Self {
            info: SessionInfo {
                id: MeetingId::new(),
                started_at_ms: 123,
            },
            zero: Instant::now(),
            attempt: Attempt(2),
        }
    }

    fn start(&self) -> StartAdmission {
        StartAdmission::New {
            info: self.info.clone(),
            attempt: self.attempt,
            plan: SessionPlan,
        }
    }

    fn opened(&self) -> Event {
        Event::Opened(
            self.attempt,
            Ok(OpenedCapture {
                capture: LiveCapture,
                epoch: MeetingEpoch(self.zero),
                health: CaptureHealth::default(),
            }),
        )
    }

    fn request(&self, active: Active, request: AdmittedRequest) -> Transition {
        next(
            active,
            Event::Request(request, self.zero + Duration::from_micros(1_250_099)),
        )
    }

    fn recording(&self) -> Active {
        let (opening, _, _) = self.request(Active::Idle, AdmittedRequest::Start(self.start()));
        next(opening, self.opened()).0
    }

    fn stopped(&self) -> (Active, StopKey) {
        let (stopped, _, reply) =
            self.request(self.recording(), AdmittedRequest::Stop(self.info.id));
        let Some(key) = token(&reply) else {
            panic!("expected stop token, got {reply:?}");
        };
        (stopped, key)
    }

    fn closing(&self, hold: bool) -> Active {
        let (stopped, key) = self.stopped();
        let request = if hold {
            AdmittedRequest::Cancel(key)
        } else {
            AdmittedRequest::Finish(key)
        };
        let (committing, _, _) = self.request(stopped, request);
        next(
            committing,
            Event::IntentWritten(self.attempt, IntentWrite::Committed),
        )
        .0
    }

    fn processing(&self) -> Active {
        next(
            self.closing(false),
            Event::Closed(self.attempt, Ok(SealedBundle)),
        )
        .0
    }

    fn recover(&self, backend: Option<Backend>) -> AdmittedRequest {
        AdmittedRequest::Recover(Ok(RecoveryJob {
            info: self.info.clone(),
            attempt: self.attempt,
            input: JobInput {
                source: JobSource::Recover {
                    bundle: RecoverableBundle,
                    backend,
                },
            },
        }))
    }

    fn status(&self, active: Active) -> Reply {
        self.request(
            active,
            AdmittedRequest::Status {
                id: None,
                disk_view: None,
            },
        )
        .2
    }
}

#[test]
fn start_and_shortcut_reserve_the_engine_then_reply_recording() {
    let session = Session::new();
    for request in [
        AdmittedRequest::Start(session.start()),
        AdmittedRequest::Shortcut(session.start()),
    ] {
        let (opening, effects, reply) = session.request(Active::Idle, request);
        assert!(opening.reserves_engine());
        assert_eq!(opening.view(), None);
        assert_eq!(effects, vec![Effect::Open(session.attempt, SessionPlan)]);
        assert_eq!(reply, Reply::None);

        let (recording, effects, reply) = next(opening, session.opened());
        let expected_view = MeetingView::Recording {
            info: session.info.clone(),
            health: CaptureHealth::default(),
        };
        assert!(recording.reserves_engine());
        assert_eq!(reply, Reply::View(expected_view.clone()));
        assert_eq!(
            effects,
            vec![
                Effect::Notify(MeetingUpdate::Sketch(SketchScope::Open(MeetingMarkScope {
                    id: session.info.id,
                    epoch: MeetingEpoch(session.zero),
                }))),
                Effect::NotifyView(expected_view),
            ]
        );
    }
}

#[test]
fn start_failure_releases_the_engine_and_reports_the_specific_code() {
    let session = Session::new();
    for (error, expected_code) in [
        (
            OpenError::CaptureUnavailable("no monitor".into()),
            "capture_unavailable",
        ),
        (OpenError::DiskLow("space".into()), "disk_low"),
    ] {
        let (opening, _, _) =
            session.request(Active::Idle, AdmittedRequest::Start(session.start()));
        let (active, effects, reply) = next(opening, Event::Opened(session.attempt, Err(error)));
        assert!(!active.reserves_engine());
        assert!(effects.is_empty());
        assert_eq!(session.status(active), Reply::View(MeetingView::Idle));
        let Reply::Error(error) = reply else {
            panic!("expected error, got {reply:?}")
        };
        assert_eq!(error.code(), expected_code);
    }
}

#[test]
fn start_of_existing_id_replays_without_opening_capture() {
    let session = Session::new();
    let on_disk = MeetingView::Recoverable(session.info.clone(), RecoveryReason::Interrupted);
    let (active, effects, reply) =
        session.request(Active::Idle, AdmittedRequest::Replay(on_disk.clone()));
    assert_eq!(reply, Reply::View(on_disk));
    assert!(effects.is_empty());
    assert_eq!(session.status(active), Reply::View(MeetingView::Idle));

    let expected = session.recording().view().unwrap();
    let (active, effects, reply) = session.request(
        session.recording(),
        AdmittedRequest::Replay(expected.clone()),
    );
    assert_eq!(reply, Reply::View(expected.clone()));
    assert!(effects.is_empty());
    assert_eq!(session.status(active), Reply::View(expected));
}

#[test]
fn start_when_engine_is_not_ready_is_busy() {
    let session = Session::new();
    for (active, request) in [
        (Active::Idle, AdmittedRequest::Start(StartAdmission::Busy)),
        (
            Active::Idle,
            AdmittedRequest::Shortcut(StartAdmission::Busy),
        ),
        (
            session.processing(),
            AdmittedRequest::Start(session.start()),
        ),
        (
            session.processing(),
            AdmittedRequest::Shortcut(session.start()),
        ),
        (session.recording(), AdmittedRequest::Start(session.start())),
    ] {
        let previous = active.view().unwrap();
        let (active, effects, reply) = session.request(active, request);
        assert_eq!(reply, Reply::Error(ReplyError::EngineBusy));
        assert!(effects.is_empty());
        assert_eq!(session.status(active), Reply::View(previous));
    }
}

#[test]
fn stop_and_shortcut_open_confirmation_without_touching_capture() {
    let session = Session::new();
    let key = StopKey {
        id: session.info.id,
        generation: 1,
    };
    for request in [
        AdmittedRequest::Stop(session.info.id),
        AdmittedRequest::Shortcut(StartAdmission::Busy),
    ] {
        let (active, effects, reply) = session.request(session.recording(), request);
        let view = MeetingView::ConfirmStop {
            info: session.info.clone(),
            health: CaptureHealth::default(),
            key,
        };
        assert!(active.reserves_engine());
        assert_eq!(token(&reply), Some(key));
        assert_eq!(effects, vec![Effect::NotifyView(view.clone())]);
        assert_eq!(session.status(active), Reply::View(view));
    }
}

#[test]
fn repeated_stop_keeps_the_current_token() {
    let session = Session::new();
    let (active, key) = session.stopped();
    let view = active.view().unwrap();
    let (active, effects, reply) = session.request(active, AdmittedRequest::Stop(session.info.id));
    assert_eq!(token(&reply), Some(key));
    assert_eq!(reply, Reply::View(view.clone()));
    assert!(effects.is_empty());
    assert_eq!(session.status(active), Reply::View(view));
}

#[test]
fn shortcut_in_confirmation_changes_nothing() {
    let session = Session::new();
    let (active, key) = session.stopped();
    let view = active.view().unwrap();
    let (active, effects, reply) =
        session.request(active, AdmittedRequest::Shortcut(StartAdmission::Busy));
    assert_eq!(token(&reply), Some(key));
    assert!(effects.is_empty());
    assert_eq!(session.status(active), Reply::View(view));
}

#[test]
fn resume_keeps_capture_running() {
    let session = Session::new();
    let (active, key) = session.stopped();
    let (active, effects, reply) = session.request(active, AdmittedRequest::Resume(key));
    let expected = session.recording().view().unwrap();
    assert_eq!(reply, Reply::View(expected.clone()));
    assert_eq!(effects, vec![Effect::NotifyView(expected.clone())]);
    assert_eq!(session.status(active), Reply::View(expected));
}

#[test]
fn stop_resume_stop_rejects_finish_with_the_first_token() {
    let session = Session::new();
    let (active, first) = session.stopped();
    let (active, _, _) = session.request(active, AdmittedRequest::Resume(first));
    let (active, _, reply) = session.request(active, AdmittedRequest::Stop(session.info.id));
    assert_eq!(first.generation, 1);
    let current = StopKey {
        generation: 2,
        ..first
    };
    assert_eq!(token(&reply), Some(current));
    let (active, effects, reply) = session.request(active, AdmittedRequest::Finish(first));
    assert_eq!(reply, Reply::Error(ReplyError::StaleStopToken));
    assert!(effects.is_empty());
    assert!(
        matches!(session.status(active), Reply::View(MeetingView::ConfirmStop { key, .. }) if key == current)
    );
}

#[test]
fn finish_and_cancel_commit_the_admission_cutoff_before_draining() {
    let session = Session::new();
    for goal in [CloseGoal::Finish, CloseGoal::Hold] {
        let (active, key) = session.stopped();
        let request = if goal == CloseGoal::Finish {
            AdmittedRequest::Finish(key)
        } else {
            AdmittedRequest::Cancel(key)
        };
        let (committing, effects, reply) = session.request(active, request);
        let spec = CloseSpec {
            cutoff: Offset(20_001),
            goal: goal.clone(),
        };
        assert!(committing.reserves_engine());
        assert_eq!(committing.view(), None);
        assert_eq!(
            effects,
            vec![Effect::PersistIntent(spec.clone(), IntentPolicy::Required)]
        );
        assert_eq!(reply, Reply::None);

        let (closing, effects, reply) = next(
            committing,
            Event::IntentWritten(session.attempt, IntentWrite::Committed),
        );
        assert!(closing.reserves_engine());
        let view = MeetingView::Closing {
            info: session.info.clone(),
            goal,
        };
        assert_eq!(reply, Reply::View(view.clone()));
        assert_eq!(
            effects,
            vec![
                Effect::Notify(MeetingUpdate::Sketch(SketchScope::Finishing(
                    session.info.id,
                    spec.cutoff
                ))),
                Effect::NotifyView(view),
                Effect::Drain(LiveCapture, spec, DrainMode::Worker),
            ]
        );
    }
}

#[test]
fn rejected_intent_restores_confirmation_and_can_drain_the_original_capture() {
    let session = Session::new();
    let (stopped, key) = session.stopped();
    let health = CaptureHealth {
        disk_low: true,
        ..CaptureHealth::default()
    };
    let (stopped, _, _) = next(stopped, Event::Health(session.attempt, health));
    let previous = stopped.view().unwrap();
    let (committing, _, _) = session.request(stopped, AdmittedRequest::Finish(key));
    let (active, effects, reply) = next(
        committing,
        Event::IntentWritten(session.attempt, IntentWrite::Rejected("read-only".into())),
    );
    assert_eq!(
        reply,
        Reply::Error(ReplyError::IntentWriteFailed("read-only".into()))
    );
    assert!(effects.is_empty());
    assert_eq!(active.view(), Some(previous));

    let (committing, _, _) = session.request(active, AdmittedRequest::Finish(key));
    let (_, effects, _) = next(
        committing,
        Event::IntentWritten(session.attempt, IntentWrite::Committed),
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, Effect::Drain(LiveCapture, _, _)))
            .count(),
        1
    );
}

#[test]
fn indeterminate_intent_drains_as_interrupted() {
    let session = Session::new();
    let (stopped, key) = session.stopped();
    let (committing, _, _) = session.request(stopped, AdmittedRequest::Finish(key));
    let goal = CloseGoal::Interrupted(InterruptCause::IntentUncertain("sync failed".into()));
    let (closing, effects, reply) = next(
        committing,
        Event::IntentWritten(
            session.attempt,
            IntentWrite::Indeterminate("sync failed".into()),
        ),
    );
    assert_eq!(
        reply,
        Reply::View(MeetingView::Closing {
            info: session.info,
            goal: goal.clone()
        })
    );
    assert!(closing.reserves_engine());
    assert_eq!(
        effects.last(),
        Some(&Effect::Drain(
            LiveCapture,
            CloseSpec {
                cutoff: Offset(20_001),
                goal
            },
            DrainMode::Worker
        ))
    );
}

#[test]
fn health_notifies_without_changing_the_recording_or_confirmation_phase() {
    let session = Session::new();
    let (stopped, key) = session.stopped();
    let health = CaptureHealth {
        tracks: Tracks {
            you: SourceHealth::Receiving,
            other: SourceHealth::Reconnecting,
        },
        disk_low: true,
    };
    for (active, view) in [
        (
            session.recording(),
            MeetingView::Recording {
                info: session.info.clone(),
                health,
            },
        ),
        (
            stopped,
            MeetingView::ConfirmStop {
                info: session.info.clone(),
                health,
                key,
            },
        ),
    ] {
        let (active, effects, reply) = next(active, Event::Health(session.attempt, health));
        assert_eq!(effects, vec![Effect::NotifyView(view.clone())]);
        assert_eq!(reply, Reply::None);
        assert_eq!(session.status(active), Reply::View(view));
    }
}

#[test]
fn capture_fault_persists_interruption_at_the_fault_offset_and_drains() {
    let session = Session::new();
    let spec = CloseSpec {
        cutoff: Offset(17),
        goal: CloseGoal::Interrupted(InterruptCause::CaptureFault("writer failure".into())),
    };
    for active in [session.recording(), session.stopped().0] {
        let (closing, effects, reply) = next(
            active,
            Event::CaptureFault(session.attempt, spec.cutoff, "writer failure".into()),
        );
        assert_eq!(
            effects.first(),
            Some(&Effect::PersistIntent(
                spec.clone(),
                IntentPolicy::BestEffort
            ))
        );
        assert_eq!(
            effects.last(),
            Some(&Effect::Drain(LiveCapture, spec.clone(), DrainMode::Worker))
        );
        assert_eq!(reply, Reply::None);
        assert_eq!(
            session.status(closing),
            Reply::View(MeetingView::Closing {
                info: session.info.clone(),
                goal: spec.goal.clone()
            })
        );
    }
}

#[test]
fn finished_seal_starts_processing_and_releases_the_engine() {
    let session = Session::new();
    let (active, effects, reply) = next(
        session.closing(false),
        Event::Closed(session.attempt, Ok(SealedBundle)),
    );
    assert!(!active.reserves_engine());
    assert_eq!(reply, Reply::None);
    let view = MeetingView::Processing {
        info: session.info.clone(),
        done: 0.0,
    };
    assert_eq!(
        effects,
        vec![
            Effect::Notify(MeetingUpdate::Sketch(SketchScope::Closed)),
            Effect::NotifyView(view.clone()),
            Effect::Process(JobInput {
                source: JobSource::Finished(SealedBundle)
            }),
        ]
    );
    assert_eq!(session.status(active), Reply::View(view));
}

#[test]
fn held_or_interrupted_seals_and_seal_failures_return_idle_without_processing() {
    let session = Session::new();
    let (interrupted, _, _) = next(
        session.recording(),
        Event::CaptureFault(session.attempt, Offset(3), "fault".into()),
    );
    for (active, result) in [
        (session.closing(true), Ok(SealedBundle)),
        (interrupted, Ok(SealedBundle)),
        (
            session.closing(false),
            Err((RecoverableBundle, "seal failure".into())),
        ),
    ] {
        let (active, effects, _) = next(active, Event::Closed(session.attempt, result));
        assert_eq!(session.status(active), Reply::View(MeetingView::Idle));
        assert!(effects.contains(&Effect::NotifyView(MeetingView::Idle)));
        assert!(
            effects
                .iter()
                .all(|effect| !matches!(effect, Effect::Process(_)))
        );
    }
}

#[test]
fn recover_launches_an_admitted_bundle_and_passes_through_backend_override() {
    let session = Session::new();
    for backend in [None, Some(Backend::Gpu)] {
        let view = MeetingView::Processing {
            info: session.info.clone(),
            done: 0.0,
        };
        let (active, effects, reply) = session.request(Active::Idle, session.recover(backend));
        assert!(!active.reserves_engine());
        assert_eq!(reply, Reply::View(view.clone()));
        assert!(effects.contains(&Effect::Process(JobInput {
            source: JobSource::Recover {
                bundle: RecoverableBundle,
                backend
            }
        })));
        assert_eq!(session.status(active), Reply::View(view));
    }
}

#[test]
fn successful_processing_returns_idle_and_notifies_history() {
    let session = Session::new();
    let (active, effects, _) = next(
        session.processing(),
        Event::Processed(session.attempt, Ok(Published)),
    );
    assert_eq!(
        effects,
        vec![
            Effect::Notify(MeetingUpdate::HistoryChanged),
            Effect::NotifyView(MeetingView::Idle)
        ]
    );
    assert_eq!(session.status(active), Reply::View(MeetingView::Idle));
}

#[test]
fn failed_processing_returns_idle_and_reports_recovery_issue() {
    let session = Session::new();
    let (active, effects, _) = next(
        session.processing(),
        Event::Processed(
            session.attempt,
            Err((
                RecoverableBundle,
                RecoveryReason::DecodeFailed("decoder exit".into()),
            )),
        ),
    );
    assert_eq!(
        effects,
        vec![
            Effect::Notify(MeetingUpdate::Issue("decoder exit".into())),
            Effect::NotifyView(MeetingView::Idle)
        ]
    );
    assert_eq!(session.status(active), Reply::View(MeetingView::Idle));
}

#[test]
fn shutdown_drains_live_capture_inline_as_interrupted() {
    let session = Session::new();
    for active in [session.recording(), session.stopped().0] {
        let (closing, effects, _) = next(
            active,
            Event::Shutdown(session.zero + Duration::from_millis(42)),
        );
        let spec = CloseSpec {
            cutoff: Offset(672),
            goal: CloseGoal::Interrupted(InterruptCause::Shutdown),
        };
        assert_eq!(
            effects.first(),
            Some(&Effect::PersistIntent(
                spec.clone(),
                IntentPolicy::BestEffort
            ))
        );
        assert_eq!(
            effects.last(),
            Some(&Effect::Drain(LiveCapture, spec, DrainMode::Inline))
        );
        let (active, _, _) = next(closing, Event::Closed(session.attempt, Ok(SealedBundle)));
        assert_eq!(session.status(active), Reply::View(MeetingView::Idle));
    }
}

#[test]
fn shutdown_cancels_processing() {
    let session = Session::new();
    let (active, effects, _) = next(session.processing(), Event::Shutdown(session.zero));
    assert_eq!(effects, vec![Effect::CancelJob(session.attempt)]);
    assert_eq!(session.status(active), Reply::View(MeetingView::Idle));
}

#[test]
fn wrong_state_and_stale_id_do_not_change_the_active_meeting() {
    let session = Session::new();
    let other_id = MeetingId::new();
    let (active, effects, reply) =
        session.request(session.recording(), AdmittedRequest::Stop(other_id));
    assert_eq!(reply, Reply::Error(ReplyError::WrongState));
    assert!(effects.is_empty());
    assert_eq!(active.view(), session.recording().view());
    let (active, key) = session.stopped();
    let previous = active.view().unwrap();
    let wrong = StopKey {
        id: other_id,
        ..key
    };
    let (active, effects, reply) = session.request(active, AdmittedRequest::Cancel(wrong));
    assert_eq!(reply, Reply::Error(ReplyError::StaleStopToken));
    assert!(effects.is_empty());
    assert_eq!(session.status(active), Reply::View(previous));
}

#[test]
fn late_worker_completions_for_old_attempts_have_no_effect() {
    let session = Session::new();
    let old = Attempt(1);
    let (opening, _, _) = session.request(Active::Idle, AdmittedRequest::Start(session.start()));
    let (committing, key) = session.stopped();
    let (committing, _, _) = session.request(committing, AdmittedRequest::Finish(key));
    for (active, event) in [
        (
            opening,
            Event::Opened(old, Err(OpenError::DiskLow("late".into()))),
        ),
        (
            session.recording(),
            Event::Health(
                old,
                CaptureHealth {
                    disk_low: true,
                    ..CaptureHealth::default()
                },
            ),
        ),
        (
            session.recording(),
            Event::CaptureFault(old, Offset(0), "late".into()),
        ),
        (
            committing,
            Event::IntentWritten(old, IntentWrite::Committed),
        ),
        (session.closing(false), Event::Closed(old, Ok(SealedBundle))),
        (session.processing(), Event::Processed(old, Ok(Published))),
    ] {
        let previous_view = active.view();
        let reserved = active.reserves_engine();
        let (active, effects, reply) = next(active, event);
        assert!(effects.is_empty());
        assert_eq!(reply, Reply::None);
        assert_eq!(active.view(), previous_view);
        assert_eq!(active.reserves_engine(), reserved);
    }
}

#[test]
fn status_resolves_live_or_disk_views_and_reports_missing_ids() {
    let session = Session::new();
    let other = Session::new();
    let disk_view = MeetingView::Recoverable(other.info.clone(), RecoveryReason::HeldByUser);
    for (id, from_disk, expected) in [
        (
            session.info.id,
            None,
            Reply::View(session.recording().view().unwrap()),
        ),
        (
            other.info.id,
            Some(disk_view.clone()),
            Reply::View(disk_view),
        ),
        (other.info.id, None, Reply::Error(ReplyError::NotFound)),
    ] {
        let (_, effects, reply) = session.request(
            session.recording(),
            AdmittedRequest::Status {
                id: Some(id),
                disk_view: from_disk,
            },
        );
        assert!(effects.is_empty());
        assert_eq!(reply, expected);
    }
}

#[test]
fn interrupt_causes_map_to_their_recovery_reasons() {
    assert_eq!(
        InterruptCause::Shutdown.recovery_reason(),
        RecoveryReason::Interrupted
    );
    assert_eq!(
        InterruptCause::IntentUncertain("sync failed".into()).recovery_reason(),
        RecoveryReason::Interrupted
    );
    assert_eq!(
        InterruptCause::CaptureFault("disk low".into()).recovery_reason(),
        RecoveryReason::CaptureFailed("disk low".into())
    );
}
