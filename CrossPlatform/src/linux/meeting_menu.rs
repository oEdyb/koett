//! Tray items for the one foreground meeting, from its latest view.
use crate::meeting::{MeetingId, MeetingRequest, MeetingView};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MeetingAction {
    /// A fresh id per click, so a replayed menu never reopens an old meeting.
    Start,
    Send(MeetingRequest),
}

impl MeetingAction {
    pub(super) fn request(self) -> MeetingRequest {
        match self {
            Self::Start => MeetingRequest::Start(MeetingId::new()),
            Self::Send(request) => request,
        }
    }
}

/// One tray line; `None` is a disabled status line.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct MeetingItem {
    pub(super) label: String,
    pub(super) action: Option<MeetingAction>,
}

fn item(label: &str, action: Option<MeetingAction>) -> MeetingItem {
    MeetingItem {
        label: label.into(),
        action,
    }
}

pub(super) fn items(view: &MeetingView, recoverable: usize) -> Vec<MeetingItem> {
    use MeetingAction::Send;
    let mut items = match view {
        MeetingView::Recording { info, .. } => {
            vec![item(
                "Stop meeting…",
                Some(Send(MeetingRequest::Stop(info.id))),
            )]
        }
        MeetingView::ConfirmStop { key, .. } => vec![
            item("Finish meeting", Some(Send(MeetingRequest::Finish(*key)))),
            item(
                "Finish without transcript",
                Some(Send(MeetingRequest::Cancel(*key))),
            ),
            item("Continue meeting", Some(Send(MeetingRequest::Resume(*key)))),
        ],
        MeetingView::Closing { .. } => vec![item("Saving meeting…", None)],
        MeetingView::Processing { .. } => vec![item("Transcribing meeting…", None)],
        _ => vec![item("Start meeting", Some(MeetingAction::Start))],
    };
    match recoverable {
        0 => {}
        1 => items.push(item("1 meeting to recover", None)),
        count => items.push(item(&format!("{count} meetings to recover"), None)),
    }
    items
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
    use crate::meeting::{CaptureHealth, CloseGoal, SessionInfo, StopKey};

    fn info() -> SessionInfo {
        SessionInfo {
            id: MeetingId::new(),
            started_at_ms: 0,
        }
    }

    fn labels(items: &[MeetingItem]) -> Vec<(&str, bool)> {
        items
            .iter()
            .map(|item| (item.label.as_str(), item.action.is_some()))
            .collect()
    }

    #[test]
    fn idle_starts_a_new_meeting_on_every_click() {
        let items = items(&MeetingView::Idle, 0);
        assert_eq!(labels(&items), [("Start meeting", true)]);
        let action = items[0].action.unwrap();
        let (MeetingRequest::Start(first), MeetingRequest::Start(second)) =
            (action.request(), action.request())
        else {
            panic!("Start meeting must start a meeting");
        };
        assert_ne!(first, second);
    }

    #[test]
    fn stopping_a_recording_meeting_only_asks_to_confirm() {
        let info = info();
        let view = MeetingView::Recording {
            info: info.clone(),
            health: CaptureHealth::default(),
        };
        let items = items(&view, 0);
        assert_eq!(labels(&items), [("Stop meeting…", true)]);
        assert_eq!(
            items[0].action.unwrap().request(),
            MeetingRequest::Stop(info.id)
        );
        assert!(live(&view));
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
        let items = items(&view, 0);
        let requests: Vec<_> = items
            .iter()
            .map(|item| (item.label.as_str(), item.action.unwrap().request()))
            .collect();
        assert_eq!(
            requests,
            [
                ("Finish meeting", MeetingRequest::Finish(key)),
                ("Finish without transcript", MeetingRequest::Cancel(key)),
                ("Continue meeting", MeetingRequest::Resume(key)),
            ]
        );
        assert!(live(&view));
    }

    #[test]
    fn closing_and_processing_show_one_disabled_line() {
        let closing = MeetingView::Closing {
            info: info(),
            goal: CloseGoal::Finish,
        };
        let processing = MeetingView::Processing {
            info: info(),
            done: 0.5,
        };
        assert_eq!(labels(&items(&closing, 0)), [("Saving meeting…", false)]);
        assert_eq!(
            labels(&items(&processing, 0)),
            [("Transcribing meeting…", false)]
        );
        assert!(!live(&closing) && !live(&processing));
    }

    #[test]
    fn a_recoverable_count_adds_one_line() {
        assert_eq!(
            labels(&items(&MeetingView::Idle, 1)),
            [("Start meeting", true), ("1 meeting to recover", false)]
        );
        assert_eq!(
            labels(&items(&MeetingView::Idle, 3)),
            [("Start meeting", true), ("3 meetings to recover", false)]
        );
    }
}
