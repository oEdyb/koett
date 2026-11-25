//! The pill while a meeting records: its clock, with Stop on hover, and once a
//! stop is asked for, one row that finishes, keeps the audio without a
//! transcript, or continues (`meeting_confirm` decides what a click sends).
use super::super::meeting_confirm::Choice;
use crate::meeting::{
    CaptureHealth, MeetingId, MeetingRequest, MeetingView, SourceHealth, StopKey,
};

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

    pub(super) fn key(&self) -> Option<StopKey> {
        match self {
            Self::Confirm { key, .. } => Some(*key),
            Self::Recording { .. } => None,
        }
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

/// The confirm row's words, left to right. Continue sits where Stop was.
pub(super) fn label(choice: Choice) -> &'static str {
    match choice {
        Choice::Finish => "Finish",
        Choice::NoTranscript => "No transcript",
        Choice::Continue => "Continue",
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

        assert_eq!(pill.stop(), Some(MeetingRequest::Stop(info.id)));
        assert_eq!(pill.key(), None);
    }

    #[test]
    fn the_confirm_row_answers_the_current_stop_token() {
        let (pill, key) = confirm(3);

        assert!(pill.confirming());
        assert_eq!(pill.stop(), None);
        assert_eq!(pill.key(), Some(key));
        assert_eq!(
            Choice::ROW.map(label),
            ["Finish", "No transcript", "Continue"]
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
