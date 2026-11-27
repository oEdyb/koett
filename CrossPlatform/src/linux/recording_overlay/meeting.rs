//! The pill while a meeting records: its clock, with Stop on hover, and once a
//! stop is asked for, one row that finishes, keeps the audio without a
//! transcript, or continues (`meeting_confirm` decides what a click sends).
//! Finish and No transcript hand the meeting to the tray through Saving.
use super::super::meeting_confirm::Choice;
use crate::meeting::{
    CaptureHealth, CloseGoal, MeetingId, MeetingRequest, MeetingView, SourceHealth, StopKey, Tracks,
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
    Saving {
        started_at_ms: u64,
    },
}

impl MeetingPill {
    /// A live meeting shows in the pill, and a closing one that keeps its audio
    /// says Saving there until any dictation wants the pill. Transcribing
    /// shows in the tray.
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
            MeetingView::Closing {
                info,
                goal: CloseGoal::Finish | CloseGoal::Hold,
            } => Some(Self::Saving {
                started_at_ms: info.started_at_ms,
            }),
            _ => None,
        }
    }

    pub(super) fn saving(&self) -> bool {
        matches!(self, Self::Saving { .. })
    }

    fn health(&self) -> Option<&CaptureHealth> {
        match self {
            Self::Recording { health, .. } | Self::Confirm { health, .. } => Some(health),
            Self::Saving { .. } => None,
        }
    }

    /// The sides that are reconnecting, drawn flat in the strip.
    pub(super) fn lost(&self) -> Tracks<bool> {
        self.health()
            .map(|health| Tracks {
                you: health.tracks.you == SourceHealth::Reconnecting,
                other: health.tracks.other == SourceHealth::Reconnecting,
            })
            .unwrap_or_default()
    }

    pub(super) fn confirming(&self) -> bool {
        matches!(self, Self::Confirm { .. })
    }

    pub(super) fn key(&self) -> Option<StopKey> {
        match self {
            Self::Confirm { key, .. } => Some(*key),
            Self::Recording { .. } | Self::Saving { .. } => None,
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
        let (Self::Recording { started_at_ms, .. }
        | Self::Confirm { started_at_ms, .. }
        | Self::Saving { started_at_ms }) = self;
        now_ms.saturating_sub(*started_at_ms)
    }

    /// The notice above the pill when audio or disk needs attention.
    pub(super) fn issue(&self) -> Option<&'static str> {
        let health = self.health()?;
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
            Self::Confirm { .. } | Self::Saving { .. } => None,
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
                goal: CloseGoal::Interrupted(crate::meeting::InterruptCause::Shutdown),
            },
            MeetingView::Processing { info, done: 0.0 },
        ] {
            assert_eq!(MeetingPill::from_view(&view), None);
        }
    }

    #[test]
    fn finish_and_no_transcript_hand_off_through_saving() {
        for goal in [CloseGoal::Finish, CloseGoal::Hold] {
            let view = MeetingView::Closing { info: info(), goal };
            let pill = MeetingPill::from_view(&view).unwrap();
            assert!(pill.saving() && !pill.confirming());
            assert_eq!(pill.elapsed_ms(61_000), 60_000);
            assert_eq!((pill.stop(), pill.key(), pill.issue()), (None, None, None));
            assert_eq!(pill.lost(), Tracks::default());
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
            assert_eq!(
                pill.lost(),
                Tracks {
                    you: health.tracks.you == Reconnecting,
                    other: health.tracks.other == Reconnecting,
                }
            );
        }
    }
}
