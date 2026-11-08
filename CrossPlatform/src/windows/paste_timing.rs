use std::time::{Duration, Instant};

pub(super) fn text_ready(started: Instant, target_read: Option<Instant>, now: Instant) -> bool {
    match target_read {
        Some(at) => now.saturating_duration_since(at.max(started)) >= Duration::from_millis(200),
        None => now.saturating_duration_since(started) >= Duration::from_millis(1500),
    }
}

pub(super) fn image_ready(started: Instant, target_read: Option<Instant>, now: Instant) -> bool {
    match target_read {
        Some(at) => now.saturating_duration_since(at.max(started)) >= Duration::from_millis(300),
        None => now.saturating_duration_since(started) >= Duration::from_millis(1500),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drawing_waits_for_text_read_and_composer_settle() {
        let start = Instant::now();
        let late = start + Duration::from_millis(1490);
        assert!(!image_ready(
            start,
            Some(late),
            late + Duration::from_millis(299)
        ));
        assert!(image_ready(
            start,
            Some(late),
            late + Duration::from_millis(300)
        ));
        assert!(!image_ready(
            start,
            None,
            start + Duration::from_millis(1499)
        ));
        assert!(image_ready(
            start,
            None,
            start + Duration::from_millis(1500)
        ));
        assert!(!image_ready(
            late,
            Some(start),
            late + Duration::from_millis(299)
        ));
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
