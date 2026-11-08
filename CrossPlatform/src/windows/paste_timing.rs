use std::time::{Duration, Instant};

pub(super) fn text_ready(started: Instant, target_read: Option<Instant>, now: Instant) -> bool {
    match target_read {
        Some(at) => now.saturating_duration_since(at.max(started)) >= Duration::from_millis(200),
        None => now.saturating_duration_since(started) >= Duration::from_millis(1500),
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum TextStage {
    Waiting,
    Ready,
    Unconfirmed,
}

pub(super) fn text_stage(
    started: Instant,
    target_read: Option<Instant>,
    now: Instant,
) -> TextStage {
    match target_read {
        Some(at)
            if now.saturating_duration_since(at.max(started)) >= Duration::from_millis(300) =>
        {
            TextStage::Ready
        }
        None if now.saturating_duration_since(started) >= Duration::from_millis(1500) => {
            TextStage::Unconfirmed
        }
        _ => TextStage::Waiting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_a_confirmed_read_can_advance_to_the_image() {
        let start = Instant::now();
        let late = start + Duration::from_millis(1490);
        assert_eq!(
            text_stage(start, Some(late), late + Duration::from_millis(299)),
            TextStage::Waiting
        );
        assert_eq!(
            text_stage(start, Some(late), late + Duration::from_millis(300)),
            TextStage::Ready
        );
        assert_eq!(
            text_stage(start, None, start + Duration::from_millis(1499)),
            TextStage::Waiting
        );
        assert_eq!(
            text_stage(start, None, start + Duration::from_millis(1500)),
            TextStage::Unconfirmed
        );
        assert_eq!(
            text_stage(start, None, start + Duration::from_secs(10)),
            TextStage::Unconfirmed
        );
        assert_eq!(
            text_stage(late, Some(start), late + Duration::from_millis(299)),
            TextStage::Waiting
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
