//! How a meeting's start and length read in the tray and History.
use crate::meeting::catalog::Listed;
use crate::meeting::{Offset, SAMPLE_RATE};

/// `2026-11-25 14:05 · 52 min`, or the start alone when the length is unknown.
pub(super) fn when(meeting: &Listed) -> String {
    let start = local_minute(meeting.info.started_at_ms).join(" ");
    match meeting.duration {
        Some(duration) => format!("{start} · {}", length(duration)),
        None => start,
    }
}

/// Date and time in the local zone, `["2026-11-25", "14:05"]`.
pub(super) fn local_minute(ms: u64) -> [String; 2] {
    i64::try_from(ms / 1000)
        .ok()
        .filter(|_| ms != 0)
        .and_then(|seconds| time::OffsetDateTime::from_unix_timestamp(seconds).ok())
        .map(|date| {
            let date = date
                .to_offset(time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC));
            [
                format!(
                    "{:04}-{:02}-{:02}",
                    date.year(),
                    u8::from(date.month()),
                    date.day()
                ),
                format!("{:02}:{:02}", date.hour(), date.minute()),
            ]
        })
        .unwrap_or_else(|| ["Unknown date".into(), String::new()])
}

/// `45 s`, `52 min`, `1 h 5 min`.
pub(super) fn length(offset: Offset) -> String {
    let seconds = offset.0 / u64::from(SAMPLE_RATE);
    let (hours, minutes) = (seconds / 3600, seconds % 3600 / 60);
    match (hours, minutes) {
        (0, 0) => format!("{seconds} s"),
        (0, minutes) => format!("{minutes} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(seconds: u64) -> Offset {
        Offset(seconds * u64::from(SAMPLE_RATE))
    }

    #[test]
    fn lengths_read_in_the_largest_whole_units() {
        assert_eq!(length(seconds(0)), "0 s");
        assert_eq!(length(seconds(45)), "45 s");
        assert_eq!(length(seconds(60)), "1 min");
        assert_eq!(length(seconds(3_130)), "52 min");
        assert_eq!(length(seconds(3_600)), "1 h");
        assert_eq!(length(seconds(3_929)), "1 h 5 min");
        assert_eq!(length(seconds(7_200)), "2 h");
    }

    #[test]
    fn an_unknown_start_says_so() {
        assert_eq!(local_minute(0), ["Unknown date".to_string(), String::new()]);
    }
}
