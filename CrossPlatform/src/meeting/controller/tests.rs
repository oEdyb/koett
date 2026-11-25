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
        recoverable: Some(0),
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
