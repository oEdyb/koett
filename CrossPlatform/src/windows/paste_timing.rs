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
/// meanwhile), or after 1.5 s with no read at all.
pub(super) fn drawing_stage(
    started: Instant,
    target_read: Option<Instant>,
    now: Instant,
) -> DrawingStage {
    match target_read {
        Some(at)
            if now.saturating_duration_since(at.max(started)) >= Duration::from_millis(1000) =>
        {
            DrawingStage::Ready
        }
        None if now.saturating_duration_since(started) >= Duration::from_millis(1500) => {
            DrawingStage::Unconfirmed
        }
        _ => DrawingStage::Waiting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_text_follows_a_second_after_the_drawing_is_read() {
        let start = Instant::now();
        let late = start + Duration::from_millis(1490);
        assert_eq!(
            drawing_stage(start, Some(late), late + Duration::from_millis(999)),
            DrawingStage::Waiting,
            "the target may still be attaching the drawing"
        );
        assert_eq!(
            drawing_stage(start, Some(late), late + Duration::from_millis(1000)),
            DrawingStage::Ready
        );
        assert_eq!(
            drawing_stage(start, None, start + Duration::from_millis(1499)),
            DrawingStage::Waiting
        );
        assert_eq!(
            drawing_stage(start, None, start + Duration::from_millis(1500)),
            DrawingStage::Unconfirmed
        );
        assert_eq!(
            drawing_stage(late, Some(start), late + Duration::from_millis(999)),
            DrawingStage::Waiting
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
