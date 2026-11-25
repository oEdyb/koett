//! The three ways out of a stop confirmation, shared by the pill and the tray.
//! A choice answers only the confirmation that was on screen when its press
//! began, and the two ending choices also need that confirmation to have shown
//! for `ARM` first. A double-click on Stop, or a press held while the meeting
//! resumes and stops again, never ends the meeting.
use crate::meeting::{MeetingRequest, MeetingView, StopKey};
use std::time::{Duration, Instant};

pub(super) const ARM: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Finish,
    NoTranscript,
    Continue,
}

/// The confirmation on screen: its stop key and when that key first showed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Shown {
    key: StopKey,
    since: Instant,
}

impl Shown {
    /// The confirmation shown once `key` is on screen at `now`. Its clock
    /// restarts only for a new key.
    pub(super) fn follow(
        previous: Option<Self>,
        key: Option<StopKey>,
        now: Instant,
    ) -> Option<Self> {
        let key = key?;
        Some(match previous {
            Some(shown) if shown.key == key => shown,
            _ => Self { key, since: now },
        })
    }

    pub(super) fn key(self) -> StopKey {
        self.key
    }
}

pub(super) fn stop_key(view: &MeetingView) -> Option<StopKey> {
    match view {
        MeetingView::ConfirmStop { key, .. } => Some(*key),
        _ => None,
    }
}

/// A press on a choice, or an activation without one: the stop key on screen
/// as it began, and when.
#[derive(Clone, Copy, Debug)]
pub(super) struct Press {
    pub(super) key: Option<StopKey>,
    pub(super) at: Instant,
}

impl Choice {
    pub(super) const ROW: [Self; 3] = [Self::Finish, Self::NoTranscript, Self::Continue];

    /// What `press` sends, given the confirmation on screen now.
    pub(super) fn request(self, press: Press, shown: Option<Shown>) -> Option<MeetingRequest> {
        let shown = shown.filter(|shown| press.key == Some(shown.key))?;
        let armed = press.at.saturating_duration_since(shown.since) >= ARM;
        match self {
            Self::Finish if armed => Some(MeetingRequest::Finish(shown.key)),
            Self::NoTranscript if armed => Some(MeetingRequest::Cancel(shown.key)),
            Self::Continue => Some(MeetingRequest::Resume(shown.key)),
            Self::Finish | Self::NoTranscript => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::MeetingId;

    fn keys() -> (StopKey, StopKey) {
        let id = MeetingId::new();
        (StopKey { id, generation: 1 }, StopKey { id, generation: 2 })
    }

    fn press(key: StopKey, at: Instant) -> Press {
        Press { key: Some(key), at }
    }

    #[test]
    fn each_choice_answers_the_confirmation_it_was_pressed_on() {
        let (key, _) = keys();
        let zero = Instant::now();
        let shown = Shown::follow(None, Some(key), zero);
        let at = zero + ARM;

        assert_eq!(
            Choice::Finish.request(press(key, at), shown),
            Some(MeetingRequest::Finish(key))
        );
        assert_eq!(
            Choice::NoTranscript.request(press(key, at), shown),
            Some(MeetingRequest::Cancel(key))
        );
        assert_eq!(
            Choice::Continue.request(press(key, at), shown),
            Some(MeetingRequest::Resume(key))
        );
        let pressed_while_recording = Press { key: None, at };
        for choice in Choice::ROW {
            assert_eq!(choice.request(pressed_while_recording, shown), None);
            assert_eq!(choice.request(press(key, at), None), None);
        }
    }

    #[test]
    fn a_double_click_on_stop_cannot_end_the_meeting() {
        let (key, _) = keys();
        let zero = Instant::now();
        let shown = Shown::follow(None, Some(key), zero);
        let second_click = press(key, zero + Duration::from_millis(250));

        assert_eq!(Choice::Finish.request(second_click, shown), None);
        assert_eq!(Choice::NoTranscript.request(second_click, shown), None);
        assert_eq!(
            Choice::Continue.request(second_click, shown),
            Some(MeetingRequest::Resume(key))
        );
    }

    #[test]
    fn a_press_held_while_the_meeting_resumes_and_stops_again_sends_nothing() {
        let (first, second) = keys();
        let zero = Instant::now();
        let shown = Shown::follow(None, Some(first), zero);
        let held = press(first, zero + Duration::from_millis(600));
        let shown = Shown::follow(shown, Some(second), zero + Duration::from_millis(700));

        for choice in Choice::ROW {
            assert_eq!(choice.request(held, shown), None);
        }
    }

    #[test]
    fn the_ending_choices_arm_again_for_each_new_key() {
        let (first, second) = keys();
        let zero = Instant::now();
        let shown = Shown::follow(None, Some(first), zero);
        let kept = Shown::follow(shown, Some(first), zero + Duration::from_secs(4));
        let renewed = Shown::follow(kept, Some(second), zero + Duration::from_secs(5));
        let early = press(second, zero + Duration::from_millis(5_100));
        let late = press(second, zero + Duration::from_secs(5) + ARM);

        assert_eq!(kept, shown);
        assert_eq!(renewed.map(Shown::key), Some(second));
        assert_eq!(Choice::Finish.request(early, renewed), None);
        assert_eq!(Choice::NoTranscript.request(early, renewed), None);
        assert_eq!(
            Choice::Continue.request(early, renewed),
            Some(MeetingRequest::Resume(second))
        );
        assert_eq!(
            Choice::Finish.request(late, renewed),
            Some(MeetingRequest::Finish(second))
        );
        assert_eq!(Shown::follow(renewed, None, zero), None);
    }
}
