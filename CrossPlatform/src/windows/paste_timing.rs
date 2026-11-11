use std::time::{Duration, Instant};

pub(super) fn text_ready(started: Instant, target_read: Option<Instant>, now: Instant) -> bool {
    match target_read {
        Some(at) => now.saturating_duration_since(at.max(started)) >= Duration::from_millis(200),
        None => now.saturating_duration_since(started) >= Duration::from_millis(1500),
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum DrawingStage {
    Waiting,
    Ready,
    Unconfirmed,
}

/// When the text may follow the drawing: a second after the target read the drawing, so it
/// has finished attaching it (T3 Code resets its editor when that ends, wiping text pasted
/// meanwhile), or after 1.5 s with no target read. A read by anyone, such as Chromium's
/// anonymous one, still gets its second before the text.
pub(super) fn drawing_stage(
    started: Instant,
    target_read: Option<Instant>,
    any_read: Option<Instant>,
    now: Instant,
) -> DrawingStage {
    let settled = |at: Instant| now.saturating_duration_since(at.max(started)) >= SETTLE;
    match target_read {
        Some(at) if settled(at) => DrawingStage::Ready,
        Some(_) => DrawingStage::Waiting,
        None if now.saturating_duration_since(started) < Duration::from_millis(1500) => {
            DrawingStage::Waiting
        }
        None if any_read.is_some_and(|at| !settled(at)) => DrawingStage::Waiting,
        None => DrawingStage::Unconfirmed,
    }
}

const SETTLE: Duration = Duration::from_millis(1000);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_text_follows_a_second_after_the_drawing_is_read() {
        let start = Instant::now();
        let late = start + Duration::from_millis(1490);
        let ms = |n| Duration::from_millis(n);
        assert_eq!(
            drawing_stage(start, Some(late), Some(late), late + ms(999)),
            DrawingStage::Waiting,
            "the target may still be attaching the drawing"
        );
        assert_eq!(
            drawing_stage(start, Some(late), Some(late), late + ms(1000)),
            DrawingStage::Ready
        );
        assert_eq!(
            drawing_stage(start, None, None, start + ms(1499)),
            DrawingStage::Waiting
        );
        assert_eq!(
            drawing_stage(start, None, None, start + ms(1500)),
            DrawingStage::Unconfirmed
        );
        assert_eq!(
            drawing_stage(late, Some(start), Some(start), late + ms(999)),
            DrawingStage::Waiting
        );
    }

    #[test]
    fn a_read_that_started_late_still_gets_its_settle() {
        let start = Instant::now();
        let ms = |n| Duration::from_millis(n);
        let slow = start + ms(1400);
        assert_eq!(
            drawing_stage(start, None, Some(slow), start + ms(1500)),
            DrawingStage::Waiting,
            "an anonymous read at 1.4 s may still be attaching at 1.5 s"
        );
        assert_eq!(
            drawing_stage(start, None, Some(slow), slow + ms(1000)),
            DrawingStage::Unconfirmed
        );
        assert_eq!(
            drawing_stage(start, None, Some(start + ms(5)), start + ms(1500)),
            DrawingStage::Unconfirmed,
            "a clipboard manager's read at the offer adds no wait"
        );
    }
    #[test]
    fn target_read_keeps_the_processing_gap_and_unknown_reader_keeps_its_bound() {
        let start = Instant::now();
        let read = start + Duration::from_millis(20);
        assert!(!text_ready(
            start,
            Some(read),
            read + Duration::from_millis(199)
        ));
        assert!(text_ready(
            start,
            Some(read),
            read + Duration::from_millis(200)
        ));
        assert!(!text_ready(
            start,
            None,
            start + Duration::from_millis(1499)
        ));
        assert!(text_ready(start, None, start + Duration::from_millis(1500)));
        assert!(!text_ready(start, Some(read), start));
        assert!(!text_ready(
            read,
            Some(start),
            read + Duration::from_millis(199)
        ));
        assert!(text_ready(
            read,
            Some(start),
            read + Duration::from_millis(200)
        ));
        let late = start + Duration::from_millis(1490);
        assert!(!text_ready(
            start,
            Some(late),
            start + Duration::from_millis(1500)
        ));
        assert!(text_ready(
            start,
            Some(late),
            late + Duration::from_millis(200)
        ));
    }
}
