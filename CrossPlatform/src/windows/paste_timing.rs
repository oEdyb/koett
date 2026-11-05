use std::time::{Duration, Instant};

pub(super) fn text_ready(started: Instant, target_read: Option<Instant>, now: Instant) -> bool {
    target_read.is_some_and(|at| now.saturating_duration_since(at) >= Duration::from_millis(200))
        || now.saturating_duration_since(started) >= Duration::from_millis(1500)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }
}
