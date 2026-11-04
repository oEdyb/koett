//! Captured sketches handed from the overlay to delivery, scoped to one
//! recording. Every recording start (with drawing on or off) begins a new
//! scope, so frames from a cancelled, failed or earlier recording, and captures
//! that finish late, can never be pasted with a later dictation.
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// A captured sketch: start seconds on the captured-audio clock and its PNG.
pub struct Keyframe {
    /// Creation order; the contact sheet, bullets and pill all number by it.
    pub order: u64,
    pub start: f32,
    pub png: Vec<u8>,
    /// What a pin read at the click; `None` for sketches.
    pub context: Option<crate::pin_context::PinContext>,
}

struct Frames {
    recording: u64,
    open: bool,
    frames: Vec<Keyframe>,
}

static FRAMES: Mutex<Frames> = Mutex::new(Frames {
    recording: 0,
    open: false,
    frames: Vec::new(),
});
static CLOSED: Condvar = Condvar::new();

fn frames() -> MutexGuard<'static, Frames> {
    FRAMES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts a new scope for a recording and drops anything older.
pub fn begin_recording() -> u64 {
    let mut state = frames();
    state.recording += 1;
    state.open = false;
    state.frames.clear();
    CLOSED.notify_all();
    state.recording
}

/// Drops the current recording's frames (cancelled or failed recording) and
/// closes its scope, so its late captures are rejected too.
pub fn discard_recording() {
    let mut state = frames();
    state.recording += 1;
    state.open = false;
    state.frames.clear();
    CLOSED.notify_all();
}

/// Marks the overlay for `recording` as still capturing (`true`) or done.
pub fn set_open(recording: u64, open: bool) {
    let mut state = frames();
    if state.recording == recording {
        state.open = open;
        CLOSED.notify_all();
    }
}

/// Stores a frame; returns false and drops it when its recording is over.
pub fn push(recording: u64, frame: Keyframe) -> bool {
    let mut state = frames();
    let current = state.recording == recording;
    if current {
        state.frames.push(frame);
    }
    current
}

/// An empty ASR result may still have drawings awaiting delivery.
pub fn has_frames_or_captures() -> bool {
    let state = frames();
    state.open || !state.frames.is_empty()
}

/// Waits (bounded) for the overlay to finish capturing, then takes the current
/// recording's keyframes in time order. Empty when nothing was drawn.
pub fn take(timeout: Duration) -> Vec<Keyframe> {
    let (mut state, _) = CLOSED
        .wait_timeout_while(frames(), timeout, |state| state.open)
        .unwrap_or_else(PoisonError::into_inner);
    let mut taken = std::mem::take(&mut state.frames);
    // Creation order, not audio time: sketches made during one pause share an
    // audio start, and their captures can finish in either order.
    taken.sort_by_key(|frame| frame.order);
    taken
}

#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(start: f32) -> Keyframe {
        Keyframe {
            order: (start * 10.0) as u64,
            start,
            png: vec![start as u8],
            context: None,
        }
    }

    fn starts(frames: &[Keyframe]) -> Vec<f32> {
        frames.iter().map(|frame| frame.start).collect()
    }

    #[test]
    fn frames_return_in_time_order_without_waiting_once_closed() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let recording = begin_recording();
        set_open(recording, true);
        assert!(push(recording, frame(2.0)));
        assert!(push(recording, frame(1.0)));
        set_open(recording, false);
        let started = std::time::Instant::now();
        assert_eq!(starts(&take(Duration::from_secs(5))), [1.0, 2.0]);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(take(Duration::ZERO).is_empty());
    }

    #[test]
    fn cancelled_frames_never_reach_a_later_dictation() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let cancelled = begin_recording();
        assert!(push(cancelled, frame(1.0)));
        discard_recording();
        assert!(
            !push(cancelled, frame(2.0)),
            "late result of a discarded session"
        );
        // Drawing turned off: the next recording opens no overlay.
        begin_recording();
        assert!(take(Duration::ZERO).is_empty());
    }

    #[test]
    fn a_new_recording_drops_frames_left_by_a_failed_one() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let failed = begin_recording();
        assert!(push(failed, frame(1.0)));
        begin_recording();
        assert!(take(Duration::ZERO).is_empty());
    }

    #[test]
    fn late_captures_from_an_earlier_recording_are_rejected() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let earlier = begin_recording();
        let current = begin_recording();
        assert!(!push(earlier, frame(1.0)));
        set_open(earlier, true);
        assert!(
            take(Duration::ZERO).is_empty(),
            "stale overlay cannot block delivery"
        );
        assert!(push(current, frame(3.0)));
        assert_eq!(starts(&take(Duration::ZERO)), [3.0]);
    }

    #[test]
    fn sketches_drawn_during_one_pause_keep_their_creation_order() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let recording = begin_recording();
        let second = Keyframe {
            order: 2,
            start: 5.0,
            png: vec![2],
            context: None,
        };
        let first = Keyframe {
            order: 1,
            start: 5.0,
            png: vec![1],
            context: None,
        };
        assert!(push(recording, second));
        assert!(push(recording, first));
        let taken = take(Duration::ZERO);
        assert_eq!(
            taken.iter().map(|frame| frame.png[0]).collect::<Vec<_>>(),
            [1, 2]
        );
    }
}
