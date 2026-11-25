use super::*;
use std::sync::mpsc::{self, Receiver};

fn controller(
    active: Active,
    reserved: &'static AtomicBool,
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
        reserved,
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
    static RESERVED: AtomicBool = AtomicBool::new(false);
    let result = std::panic::catch_unwind(|| {
        let (mut controller, _commands, _updates) =
            controller(recording(Instant::now()), &RESERVED);
        controller.apply(Event::Health(Attempt(1), CaptureHealth::default()));
        assert!(RESERVED.load(Ordering::Acquire));
        panic!("fixture engine panic");
    });
    assert!(result.is_err());
    assert!(!RESERVED.load(Ordering::Acquire));
}
