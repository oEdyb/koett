//! Tray items for the one foreground meeting, from its latest view.
use super::TrayAction;
use super::meeting_confirm::{Choice, Press, Shown};
use crate::meeting::catalog::Listed;
use crate::meeting::{MeetingCall, MeetingId, MeetingRequest, MeetingView, StopKey};
use std::time::Instant;

/// The tray lists this many recoverable meetings; History holds the rest.
const RECOVER_ITEMS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MeetingAction {
    /// A fresh id per click, so a replayed menu never reopens an old meeting.
    Start,
    Stop(MeetingId),
    /// Answers only the confirmation this item was built for.
    Confirm(Choice, StopKey),
    Recover(MeetingId),
    ShowHistory,
}

impl MeetingAction {
    /// What a click at `at` sends, given the confirmation the tray shows now.
    pub(super) fn tray_action(self, shown: Option<Shown>, at: Instant) -> Option<TrayAction> {
        let request = match self {
            Self::ShowHistory => return Some(TrayAction::OpenTranscripts),
            Self::Start => MeetingRequest::Start(MeetingId::new()),
            Self::Stop(id) => MeetingRequest::Stop(id),
            Self::Confirm(choice, key) => choice.request(Press { key: Some(key), at }, shown)?,
            Self::Recover(id) => MeetingRequest::Recover { id, backend: None },
        };
        Some(TrayAction::Meeting(MeetingCall::desktop(request)))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum MeetingItem {
    /// One line; no action makes it a disabled status line.
    Line {
        label: String,
        action: Option<MeetingAction>,
    },
    Submenu {
        label: String,
        enabled: bool,
        items: Vec<MeetingItem>,
    },
}

fn line(label: &str, action: Option<MeetingAction>) -> MeetingItem {
    MeetingItem::Line {
        label: label.into(),
        action,
    }
}

pub(super) fn items(view: &MeetingView, recoverable: &[Listed]) -> Vec<MeetingItem> {
    use MeetingAction::Confirm;
    let mut items = match view {
        MeetingView::Recording { info, .. } => {
            vec![line("Stop meeting…", Some(MeetingAction::Stop(info.id)))]
        }
        MeetingView::ConfirmStop { key, .. } => vec![
            line("Finish meeting", Some(Confirm(Choice::Finish, *key))),
            line(
                "Finish without transcript",
                Some(Confirm(Choice::NoTranscript, *key)),
            ),
            line("Continue meeting", Some(Confirm(Choice::Continue, *key))),
        ],
        MeetingView::Closing { .. } => Vec::new(),
        MeetingView::Processing { .. } => vec![line("Transcribing meeting…", None)],
        _ => vec![line("Start meeting", Some(MeetingAction::Start))],
    };
    if !recoverable.is_empty() {
        let mut entries: Vec<_> = recoverable
            .iter()
            .take(RECOVER_ITEMS)
            .map(|meeting| {
                line(
                    &super::meeting_labels::when(meeting),
                    Some(MeetingAction::Recover(meeting.info.id)),
                )
            })
            .collect();
        if recoverable.len() > RECOVER_ITEMS {
            entries.push(line("More in History", Some(MeetingAction::ShowHistory)));
        }
        items.push(MeetingItem::Submenu {
            label: "Recover meeting".into(),
            enabled: matches!(view, MeetingView::Idle),
            items: entries,
        });
    }
    items
}

/// What the tray headline says instead of dictation's status.
pub(super) fn headline(view: &MeetingView) -> Option<&'static str> {
    match view {
        MeetingView::Recording { .. } | MeetingView::ConfirmStop { .. } => {
            Some("Recording meeting")
        }
        MeetingView::Closing { .. } => Some("Saving meeting…"),
        _ => None,
    }
}

/// Quit and media transcription are refused until the meeting is saved.
pub(super) fn holds_engine(view: &MeetingView) -> bool {
    matches!(
        view,
        MeetingView::Recording { .. }
            | MeetingView::ConfirmStop { .. }
            | MeetingView::Closing { .. }
    )
}

/// The tray shows the recording icon for a live meeting too.
pub(super) fn live(view: &MeetingView) -> bool {
    matches!(
        view,
        MeetingView::Recording { .. } | MeetingView::ConfirmStop { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::catalog::Listing;
    use crate::meeting::{CaptureHealth, CloseGoal, Offset, RecoveryReason, SessionInfo};

    fn info() -> SessionInfo {
        SessionInfo {
            id: MeetingId::new(),
            started_at_ms: 0,
        }
    }

    fn labels(items: &[MeetingItem]) -> Vec<(&str, bool)> {
        items
            .iter()
            .map(|item| match item {
                MeetingItem::Line { label, action } => (label.as_str(), action.is_some()),
                MeetingItem::Submenu { label, enabled, .. } => (label.as_str(), *enabled),
            })
            .collect()
    }

    fn action(item: &MeetingItem) -> MeetingAction {
        match item {
            MeetingItem::Line {
                action: Some(action),
                ..
            } => *action,
            item => panic!("{item:?} has no action"),
        }
    }

    fn meeting(call: MeetingRequest) -> Option<TrayAction> {
        Some(TrayAction::Meeting(MeetingCall::desktop(call)))
    }

    fn recoverable(count: usize) -> Vec<Listed> {
        (0..count)
            .map(|index| Listed {
                info: SessionInfo {
                    id: MeetingId::new(),
                    started_at_ms: 1_764_000_000_000 - index as u64 * 86_400_000,
                },
                duration: (index != 1).then_some(Offset(16_000 * 60 * (index as u64 + 1))),
                folder: std::path::PathBuf::from("/meetings"),
                state: Listing::NotTranscribed(RecoveryReason::HeldByUser),
                recoverable: true,
            })
            .collect()
    }

    #[test]
    fn idle_starts_a_new_meeting_on_every_click() {
        let items = items(&MeetingView::Idle, &[]);
        assert_eq!(labels(&items), [("Start meeting", true)]);
        let now = Instant::now();
        let (Some(TrayAction::Meeting(first)), Some(TrayAction::Meeting(second))) = (
            action(&items[0]).tray_action(None, now),
            action(&items[0]).tray_action(None, now),
        ) else {
            panic!("Start meeting must start a meeting");
        };
        assert_ne!(first, second);
        assert_eq!(headline(&MeetingView::Idle), None);
        assert!(!holds_engine(&MeetingView::Idle));
    }

    #[test]
    fn stopping_a_recording_meeting_only_asks_to_confirm() {
        let info = info();
        let view = MeetingView::Recording {
            info: info.clone(),
            health: CaptureHealth::default(),
        };
        let items = items(&view, &[]);
        assert_eq!(labels(&items), [("Stop meeting…", true)]);
        assert_eq!(
            action(&items[0]).tray_action(None, Instant::now()),
            meeting(MeetingRequest::Stop(info.id))
        );
        assert!(live(&view) && holds_engine(&view));
        assert_eq!(headline(&view), Some("Recording meeting"));
    }

    #[test]
    fn the_confirm_items_carry_the_current_stop_token() {
        let info = info();
        let key = StopKey {
            id: info.id,
            generation: 2,
        };
        let view = MeetingView::ConfirmStop {
            info,
            health: CaptureHealth::default(),
            key,
        };
        let items = items(&view, &[]);
        let shown = Shown::follow(None, Some(key), Instant::now());
        let armed = Instant::now() + super::super::meeting_confirm::ARM;
        let requests: Vec<_> = items
            .iter()
            .map(|item| match item {
                MeetingItem::Line { label, .. } => {
                    (label.as_str(), action(item).tray_action(shown, armed))
                }
                item => panic!("{item:?} is not a confirm choice"),
            })
            .collect();
        assert_eq!(
            requests,
            [
                ("Finish meeting", meeting(MeetingRequest::Finish(key))),
                (
                    "Finish without transcript",
                    meeting(MeetingRequest::Cancel(key))
                ),
                ("Continue meeting", meeting(MeetingRequest::Resume(key))),
            ]
        );
        assert!(live(&view) && holds_engine(&view));
        assert_eq!(headline(&view), Some("Recording meeting"));
    }

    #[test]
    fn closing_says_so_in_the_headline_and_processing_in_one_line() {
        let closing = MeetingView::Closing {
            info: info(),
            goal: CloseGoal::Finish,
        };
        let processing = MeetingView::Processing {
            info: info(),
            done: 0.5,
        };
        assert_eq!(labels(&items(&closing, &[])), []);
        assert_eq!(headline(&closing), Some("Saving meeting…"));
        assert!(holds_engine(&closing));
        assert_eq!(
            labels(&items(&processing, &[])),
            [("Transcribing meeting…", false)]
        );
        assert_eq!(headline(&processing), None);
        assert!(!holds_engine(&processing));
        assert!(!live(&closing) && !live(&processing));
    }

    #[test]
    fn each_recoverable_meeting_recovers_with_the_default_backend() {
        let meetings = recoverable(2);
        let items = items(&MeetingView::Idle, &meetings);
        assert_eq!(
            labels(&items),
            [("Start meeting", true), ("Recover meeting", true)]
        );
        let MeetingItem::Submenu { items: entries, .. } = &items[1] else {
            panic!("recoverable meetings sit in a submenu");
        };
        assert_eq!(entries.len(), 2);
        for (entry, listed) in entries.iter().zip(&meetings) {
            let MeetingItem::Line { label, .. } = entry else {
                panic!("one line per meeting");
            };
            assert_eq!(label, &super::super::meeting_labels::when(listed));
            assert_eq!(
                action(entry).tray_action(None, Instant::now()),
                meeting(MeetingRequest::Recover {
                    id: listed.info.id,
                    backend: None,
                })
            );
        }
        let MeetingItem::Line { label, .. } = &entries[0] else {
            unreachable!()
        };
        assert!(label.ends_with(" · 1 min"), "{label}");
        let MeetingItem::Line { label, .. } = &entries[1] else {
            unreachable!()
        };
        assert!(!label.contains('·'), "unknown length is left out: {label}");
    }

    #[test]
    fn the_tray_lists_five_newest_then_points_to_history() {
        let meetings = recoverable(7);
        let items = items(&MeetingView::Idle, &meetings);
        let MeetingItem::Submenu { label, items, .. } = &items[1] else {
            panic!("recoverable meetings sit in a submenu");
        };
        assert_eq!(label, "Recover meeting");
        assert_eq!(items.len(), 6);
        assert_eq!(
            action(&items[4]).tray_action(None, Instant::now()),
            meeting(MeetingRequest::Recover {
                id: meetings[4].info.id,
                backend: None,
            })
        );
        assert_eq!(labels(&items[5..]), [("More in History", true)]);
        assert_eq!(
            action(&items[5]).tray_action(None, Instant::now()),
            Some(TrayAction::OpenTranscripts)
        );
        let MeetingItem::Submenu { items: five, .. } =
            &self::items(&MeetingView::Idle, &meetings[..5])[1]
        else {
            panic!("recoverable meetings sit in a submenu");
        };
        assert_eq!(five.len(), 5, "History is offered only for a sixth meeting");
    }

    #[test]
    fn recovery_waits_while_a_meeting_is_running_or_transcribing() {
        let meetings = recoverable(1);
        for view in [
            MeetingView::Recording {
                info: info(),
                health: CaptureHealth::default(),
            },
            MeetingView::Closing {
                info: info(),
                goal: CloseGoal::Hold,
            },
            MeetingView::Processing {
                info: info(),
                done: 0.0,
            },
        ] {
            let items = items(&view, &meetings);
            assert_eq!(
                labels(&items).last(),
                Some(&("Recover meeting", false)),
                "{view:?}"
            );
        }
    }
}
