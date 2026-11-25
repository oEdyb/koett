//! The pill while a meeting records: its clock, with Stop on hover, and once a
//! stop is asked for, one row that finishes, keeps the audio without a
//! transcript, or continues. Finishing always carries the current stop token.
use crate::meeting::{
    CaptureHealth, MeetingId, MeetingRequest, MeetingView, SourceHealth, StopKey,
};
use std::time::{Duration, Instant};

/// A double-click on Stop must not land on the row that replaces it.
const ARM: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, PartialEq)]
pub(in crate::linux) enum MeetingPill {
    Recording {
        id: MeetingId,
        started_at_ms: u64,
        health: CaptureHealth,
    },
    Confirm {
        key: StopKey,
        started_at_ms: u64,
        health: CaptureHealth,
    },
}

impl MeetingPill {
    /// Only a live meeting shows in the pill; closing and transcribing leave it
    /// free for dictation and show in the tray.
    pub(in crate::linux) fn from_view(view: &MeetingView) -> Option<Self> {
        match view {
            MeetingView::Recording { info, health } => Some(Self::Recording {
                id: info.id,
                started_at_ms: info.started_at_ms,
                health: *health,
            }),
            MeetingView::ConfirmStop { info, health, key } => Some(Self::Confirm {
                key: *key,
                started_at_ms: info.started_at_ms,
                health: *health,
            }),
            _ => None,
        }
    }

    pub(super) fn confirming(&self) -> bool {
        matches!(self, Self::Confirm { .. })
    }

    pub(super) fn elapsed_now(&self) -> u64 {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| {
                u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
            });
        self.elapsed_ms(now_ms)
    }

    pub(super) fn elapsed_ms(&self, now_ms: u64) -> u64 {
        let (Self::Recording { started_at_ms, .. } | Self::Confirm { started_at_ms, .. }) = self;
        now_ms.saturating_sub(*started_at_ms)
    }

    /// The line under the pill when audio or disk needs attention.
    pub(super) fn issue(&self) -> Option<&'static str> {
        let (Self::Recording { health, .. } | Self::Confirm { health, .. }) = self;
        match (health.tracks.you, health.tracks.other) {
            (SourceHealth::Reconnecting, SourceHealth::Reconnecting) => Some("Audio reconnecting"),
            (SourceHealth::Reconnecting, _) => Some("Microphone reconnecting"),
            (_, SourceHealth::Reconnecting) => Some("System audio reconnecting"),
            _ if health.disk_low => Some("Disk almost full"),
            _ => None,
        }
    }

    /// Hover Stop asks for the confirm row; it never ends the meeting.
    pub(super) fn stop(&self) -> Option<MeetingRequest> {
        match self {
            Self::Recording { id, .. } => Some(MeetingRequest::Stop(*id)),
            Self::Confirm { .. } => None,
        }
    }
}

/// The confirm row, left to right. Continue sits where Stop was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Finish,
    NoTranscript,
    Continue,
}

impl Choice {
    pub(super) const ROW: [Self; 3] = [Self::Finish, Self::NoTranscript, Self::Continue];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Finish => "Finish",
            Self::NoTranscript => "No transcript",
            Self::Continue => "Continue",
        }
    }

    /// What a click sends, given the pill now and when its row appeared.
    pub(super) fn request(
        self,
        pill: Option<&MeetingPill>,
        shown: Instant,
        now: Instant,
    ) -> Option<MeetingRequest> {
        let Some(MeetingPill::Confirm { key, .. }) = pill else {
            return None;
        };
        let armed = now.saturating_duration_since(shown) >= ARM;
        match self {
            Self::Finish if armed => Some(MeetingRequest::Finish(*key)),
            Self::NoTranscript if armed => Some(MeetingRequest::Cancel(*key)),
            Self::Continue => Some(MeetingRequest::Resume(*key)),
            Self::Finish | Self::NoTranscript => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::{SessionInfo, Tracks};

    fn info() -> SessionInfo {
        SessionInfo {
            id: MeetingId::new(),
            started_at_ms: 1_000,
        }
    }

    fn confirm(generation: u64) -> (MeetingPill, StopKey) {
        let info = info();
        let key = StopKey {
            id: info.id,
            generation,
        };
        let view = MeetingView::ConfirmStop {
            info,
            health: CaptureHealth::default(),
            key,
        };
        (MeetingPill::from_view(&view).unwrap(), key)
    }

    #[test]
    fn only_a_live_meeting_takes_the_pill() {
        let info = info();
        let recording = MeetingView::Recording {
            info: info.clone(),
            health: CaptureHealth::default(),
        };
        let pill = MeetingPill::from_view(&recording).unwrap();
        assert!(!pill.confirming());
        assert_eq!(pill.elapsed_ms(62_500), 61_500);
        for view in [
            MeetingView::Idle,
            MeetingView::Closing {
                info: info.clone(),
                goal: crate::meeting::CloseGoal::Finish,
            },
            MeetingView::Processing { info, done: 0.0 },
        ] {
            assert_eq!(MeetingPill::from_view(&view), None);
        }
    }

    #[test]
    fn stop_from_the_pill_only_asks_to_confirm() {
        let info = info();
        let view = MeetingView::Recording {
            info: info.clone(),
            health: CaptureHealth::default(),
        };
        let pill = MeetingPill::from_view(&view).unwrap();
        let now = Instant::now();

        assert_eq!(pill.stop(), Some(MeetingRequest::Stop(info.id)));
        for choice in Choice::ROW {
            assert_eq!(choice.request(Some(&pill), now - ARM, now), None);
        }
    }

    #[test]
    fn each_choice_carries_the_current_stop_token() {
        let (pill, key) = confirm(3);
        let now = Instant::now();
        let shown = now - ARM;

        assert!(pill.confirming());
        assert_eq!(pill.stop(), None);
        assert_eq!(
            Choice::Finish.request(Some(&pill), shown, now),
            Some(MeetingRequest::Finish(key))
        );
        assert_eq!(
            Choice::NoTranscript.request(Some(&pill), shown, now),
            Some(MeetingRequest::Cancel(key))
        );
        assert_eq!(
            Choice::Continue.request(Some(&pill), shown, now),
            Some(MeetingRequest::Resume(key))
        );
        assert_eq!(Choice::Finish.request(None, shown, now), None);
    }

    #[test]
    fn a_double_click_on_stop_cannot_end_the_meeting() {
        let (pill, key) = confirm(1);
        let shown = Instant::now();
        let second_click = shown + Duration::from_millis(250);

        assert_eq!(
            Choice::Finish.request(Some(&pill), shown, second_click),
            None
        );
        assert_eq!(
            Choice::NoTranscript.request(Some(&pill), shown, second_click),
            None
        );
        assert_eq!(
            Choice::Continue.request(Some(&pill), shown, second_click),
            Some(MeetingRequest::Resume(key))
        );
    }

    #[test]
    fn a_lost_source_or_low_disk_is_said_under_the_pill() {
        let health = |you, other, disk_low| CaptureHealth {
            tracks: Tracks { you, other },
            disk_low,
        };
        use SourceHealth::{Receiving, Reconnecting};
        for (health, expected) in [
            (health(Receiving, Receiving, false), None),
            (
                health(Reconnecting, Receiving, false),
                Some("Microphone reconnecting"),
            ),
            (
                health(Receiving, Reconnecting, true),
                Some("System audio reconnecting"),
            ),
            (
                health(Reconnecting, Reconnecting, false),
                Some("Audio reconnecting"),
            ),
            (health(Receiving, Receiving, true), Some("Disk almost full")),
        ] {
            let pill = MeetingPill::Recording {
                id: MeetingId::new(),
                started_at_ms: 0,
                health,
            };
            assert_eq!(pill.issue(), expected);
        }
    }
}
