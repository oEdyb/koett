use super::*;
use std::sync::mpsc::{self, Receiver};

fn controller(
    active: Active,
    activity: &'static AtomicU8,
) -> (
    MeetingController,
    Receiver<EngineCommand>,
    Receiver<EngineUpdate>,
) {
    let (commands, inbox) = mpsc::channel();
    let (updates, views) = mpsc::channel();
    let controller = MeetingController {
        root: Err("no fixture bundle".into()),
        active,
        attempt: 1,
        recoverable: Some(Vec::new()),
        boot: None,
        worker: None,
        commands,
        updates,
        activity,
        disk_health: Tracks::default(),
        disconnected: Tracks::default(),
    };
    (controller, inbox, views)
}

fn recording(zero: Instant) -> Active {
    Active::Recording(Live {
        info: SessionInfo {
            id: MeetingId::new(),
            started_at_ms: 123,
        },
        attempt: Attempt(1),
        capture: LiveCapture::Fixture(MarkSink::fixture()),
        epoch: MeetingEpoch(zero),
        health: CaptureHealth::default(),
        generation: 0,
    })
}

#[test]
fn engine_panic_releases_the_meeting_reservation() {
    static ACTIVITY: AtomicU8 = AtomicU8::new(MeetingActivity::Idle as u8);
    let result = std::panic::catch_unwind(|| {
        let (mut controller, _commands, _updates) =
            controller(recording(Instant::now()), &ACTIVITY);
        controller.apply(Event::Health(Attempt(1), CaptureHealth::default()));
        assert_eq!(
            ACTIVITY.load(Ordering::Acquire),
            MeetingActivity::Reserved as u8
        );
        panic!("fixture engine panic");
    });
    assert!(result.is_err());
    assert_eq!(
        ACTIVITY.load(Ordering::Acquire),
        MeetingActivity::Idle as u8
    );
}

#[test]
fn two_seconds_with_both_sources_lost_interrupts_without_processing() {
    static ACTIVITY: AtomicU8 = AtomicU8::new(MeetingActivity::Idle as u8);
    let zero = Instant::now();
    let at = |ms| zero + Duration::from_millis(ms);
    let source = |channel, health| MeetingEvent(Event::SourceHealth(Attempt(1), channel, health));
    let (mut controller, commands, updates) = controller(recording(zero), &ACTIVITY);

    controller.receive(source(Channel::You, SourceHealth::Reconnecting), at(0));
    controller.receive(
        source(Channel::Other, SourceHealth::Reconnecting),
        at(1_000),
    );
    controller.check_sources(at(2_999));
    controller.receive(source(Channel::You, SourceHealth::Receiving), at(2_999));
    controller.check_sources(at(3_000));

    controller.receive(source(Channel::You, SourceHealth::Reconnecting), at(4_000));
    controller.receive(source(Channel::You, SourceHealth::Reconnecting), at(5_000));
    controller.check_sources(at(5_999));
    assert!(matches!(
        controller.active.view(),
        Some(MeetingView::Recording { .. })
    ));

    controller.check_sources(at(6_000));
    assert!(matches!(
        controller.active.view(),
        Some(MeetingView::Closing {
            goal: CloseGoal::Interrupted(InterruptCause::CaptureFault(_)),
            ..
        })
    ));
    let EngineCommand::MeetingEvent(closed) =
        commands.recv_timeout(Duration::from_secs(2)).unwrap()
    else {
        panic!("expected drain completion");
    };
    controller.receive(closed, at(6_000));
    assert_eq!(controller.active.view(), Some(MeetingView::Idle));
    assert!(!updates.try_iter().any(|update| matches!(
        update,
        EngineUpdate::Meeting(MeetingUpdate::View(MeetingView::Processing { .. }, _))
    )));
}

#[test]
fn shutdown_waits_for_a_drain_with_a_pending_mark() {
    static ACTIVITY: AtomicU8 = AtomicU8::new(MeetingActivity::Idle as u8);
    let info = SessionInfo {
        id: MeetingId::new(),
        started_at_ms: 123,
    };
    let root = std::env::temp_dir().join(format!("koett-controller-{}", info.id));
    let mut bundle = Bundle::create(
        &root,
        Manifest {
            info: info.clone(),
            mic: None,
            backend_pref: Backend::Parakeet,
            keep_audio: true,
            export_dir: None,
        },
    )
    .unwrap();
    let (mut controller, commands, _updates) = controller(
        Active::Closing {
            info,
            attempt: Attempt(1),
            goal: CloseGoal::Hold,
        },
        &ACTIVITY,
    );
    let marks = bundle.start_marks().unwrap();
    let sink = marks.sink();
    let offset = Offset(100);
    assert!(sink.begin(1, offset));

    let post = controller.commands.clone();
    controller.worker = Some(Worker::Drain(
        Attempt(1),
        thread::spawn(move || {
            assert!(marks.close(offset).is_empty());
            emit(&post, Event::Closed(Attempt(1), Ok(SealedBundle::Fixture)));
        }),
    ));

    let directory = bundle.directory().to_owned();
    let (ready, started) = mpsc::sync_channel(0);
    let (done, completed) = mpsc::channel();
    let shutdown = thread::spawn(move || {
        ready.send(()).unwrap();
        controller.shutdown();
        done.send(bundle::marks::list_marks(&directory, None).unwrap())
            .unwrap();
    });
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    let early = completed.recv_timeout(Duration::from_millis(100));
    let submitted = sink.submit(1, offset, vec![1, 2, 3]);
    let drained = commands.recv_timeout(Duration::from_secs(2));
    shutdown.join().unwrap();
    drop(bundle);
    std::fs::remove_dir_all(root).unwrap();

    assert_eq!(early, Err(mpsc::RecvTimeoutError::Timeout));
    assert!(submitted);
    assert!(matches!(
        drained,
        Ok(EngineCommand::MeetingEvent(MeetingEvent(Event::Closed(
            _,
            Ok(_)
        ))))
    ));
    let committed = completed.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].offset, offset);
}

#[test]
fn a_shutdown_signal_mid_recording_drains_pending_marks_inline() {
    static ACTIVITY: AtomicU8 = AtomicU8::new(MeetingActivity::Idle as u8);
    let info = SessionInfo {
        id: MeetingId::new(),
        started_at_ms: 123,
    };
    let root = std::env::temp_dir().join(format!("koett-controller-{}", info.id));
    let mut bundle = Bundle::create(
        &root,
        Manifest {
            info: info.clone(),
            mic: None,
            backend_pref: Backend::Parakeet,
            keep_audio: true,
            export_dir: None,
        },
    )
    .unwrap();
    let marks = bundle.start_marks().unwrap();
    let sink = marks.sink();
    let directory = bundle.directory().to_owned();
    let zero = Instant::now().checked_sub(Duration::from_secs(10)).unwrap();
    let live = Live {
        info,
        attempt: Attempt(1),
        capture: LiveCapture::Silent(super::super::session::SilentCapture {
            marks,
            bundle: Arc::new(bundle),
        }),
        epoch: MeetingEpoch(zero),
        health: CaptureHealth::default(),
        generation: 0,
    };
    let (mut controller, _commands, _updates) = controller(Active::Recording(live), &ACTIVITY);
    let (captured, stalled) = (Offset(16_000), Offset(32_000));
    assert!(sink.begin(1, captured) && sink.begin(2, stalled));

    let late = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        sink.submit(1, captured, vec![1, 2, 3])
    });
    let signalled = Instant::now();
    controller.shutdown();
    let drained_for = signalled.elapsed();
    let read = |name| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(directory.join(name)).unwrap()).unwrap()
    };
    let (seal, intent) = (read("seal.json"), read("intent.json"));
    let committed = bundle::marks::list_marks(&directory, None).unwrap();
    std::fs::remove_dir_all(root).unwrap();

    assert!(late.join().unwrap());
    assert_eq!(controller.active.view(), Some(MeetingView::Idle));
    assert_eq!(
        (&intent["goal"], &intent["cause"]["kind"]),
        (&"interrupted".into(), &"shutdown".into())
    );
    assert_eq!(
        committed.iter().map(|mark| mark.offset).collect::<Vec<_>>(),
        [captured]
    );
    assert_eq!(
        seal["mark_issues"],
        serde_json::json!([{"id": 2, "issue": "mark drain timed out"}])
    );
    assert!(
        (Duration::from_secs(8)..Duration::from_secs(9)).contains(&drained_for),
        "{drained_for:?}"
    );
}

#[test]
fn a_rejected_desktop_request_answers_with_the_current_view() {
    static ACTIVITY: AtomicU8 = AtomicU8::new(MeetingActivity::Idle as u8);
    let (mut controller, _commands, updates) = controller(Active::Idle, &ACTIVITY);
    let recover = MeetingRequest::Recover {
        id: MeetingId::new(),
        backend: None,
    };

    controller.dispatch(MeetingCall::desktop(recover), true, &Settings::default());

    let updates: Vec<MeetingUpdate> = updates
        .try_iter()
        .filter_map(|update| match update {
            EngineUpdate::Meeting(update) => Some(update),
            _ => None,
        })
        .collect();
    assert!(
        matches!(
            updates.as_slice(),
            [
                MeetingUpdate::Issue(_),
                MeetingUpdate::View(MeetingView::Idle, _)
            ]
        ),
        "the control that sent the request waits for a view: {updates:?}"
    );
}
