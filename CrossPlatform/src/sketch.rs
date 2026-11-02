//! Sketch events shared between the draw-on-screen mode and the recording pill.
//!
//! The draw mode publishes one event per change; the pill drains them on its
//! UI thread and draws a numbered band per sketch. An event for an index that
//! was already seen replaces it (upsert), so a live sketch is republished with
//! a growing `end_ms` and finally with `live: false`.
//!
//! Times are milliseconds since the current dictation started, on the same
//! clock as the pill: use [`elapsed_ms`] when emitting. Events published while
//! no dictation is active are dropped. The registry is cleared on every start.
use std::sync::Mutex;
use std::time::Instant;

/// One sketch, as drawn on the pill's timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SketchEvent {
    /// 1-based number; the same number that is pasted with the sketch.
    pub index: u32,
    /// Milliseconds since the dictation started when drawing began.
    pub start_ms: u64,
    /// Latest drawing time. While `live`, the emitter keeps raising this.
    pub end_ms: u64,
    /// True while Shift is held and the sketch is still being drawn.
    pub live: bool,
}

struct Registry {
    started: Option<Instant>,
    pending: Vec<SketchEvent>,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    started: None,
    pending: Vec::new(),
});

const PENDING_LIMIT: usize = 4_096;

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Called by the UI when a dictation starts (`Some`) or ends (`None`).
pub fn set_recording_start(started: Option<Instant>) {
    let mut registry = registry();
    if registry.started != started {
        registry.pending.clear();
    }
    registry.started = started;
}

/// Milliseconds since the active dictation started, or `None` when idle.
pub fn elapsed_ms() -> Option<u64> {
    registry()
        .started
        .map(|started| u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
}

/// Publishes a sketch change. Returns false when no dictation is active or
/// the event is malformed (index 0 or end before start).
pub fn publish(event: SketchEvent) -> bool {
    if event.index == 0 || event.end_ms < event.start_ms {
        return false;
    }
    let mut registry = registry();
    if registry.started.is_none() || registry.pending.len() >= PENDING_LIMIT {
        return false;
    }
    registry.pending.push(event);
    true
}

/// Takes every event published since the last call, oldest first.
pub fn drain() -> Vec<SketchEvent> {
    std::mem::take(&mut registry().pending)
}

#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    fn event(index: u32, start_ms: u64, end_ms: u64, live: bool) -> SketchEvent {
        SketchEvent {
            index,
            start_ms,
            end_ms,
            live,
        }
    }

    #[test]
    fn events_flow_only_during_a_dictation_and_reset_on_start() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_recording_start(None);
        assert!(!publish(event(1, 0, 10, true)));
        assert_eq!(elapsed_ms(), None);
        set_recording_start(Some(Instant::now()));
        assert!(elapsed_ms().is_some());
        assert!(publish(event(1, 100, 200, true)));
        assert!(!publish(event(0, 100, 200, true)));
        assert!(!publish(event(2, 300, 200, false)));
        assert!(publish(event(1, 100, 400, false)));
        assert_eq!(
            drain(),
            vec![event(1, 100, 200, true), event(1, 100, 400, false)]
        );
        assert!(drain().is_empty());
        assert!(publish(event(2, 500, 600, true)));
        set_recording_start(Some(Instant::now() + std::time::Duration::from_secs(1)));
        assert!(drain().is_empty());
        set_recording_start(None);
    }
}
