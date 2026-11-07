#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Hidden,
    Recording,
    Transcribing,
    Done(String),
    /// Not pasted, but on the clipboard: one line, then it fades like Done.
    Copied,
    NotPasted {
        saved: bool,
    },
    Error {
        message: String,
        rescue: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PaintKey {
    revision: u64,
    hovered: bool,
    paused: bool,
    copied: bool,
    drawing: bool,
    media_result: bool,
    repeat_paste: bool,
    sticky_result: bool,
}

pub struct State {
    pub phase: Phase,
    pub hovered: bool,
    pub paused: bool,
    pub copied: bool,
    pub drawing: bool,
    pub media_result: bool,
    pub repeat_paste: bool,
    pub sticky_result: bool,
    notice: Option<(String, u64)>,
    changed: u64,
    revision: u64,
    left_hover: Option<u64>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            phase: Phase::Hidden,
            hovered: false,
            paused: false,
            copied: false,
            drawing: false,
            media_result: false,
            repeat_paste: false,
            sticky_result: false,
            notice: None,
            changed: 0,
            revision: 0,
            left_hover: None,
        }
    }
}
impl State {
    pub fn set(&mut self, phase: Phase, now: u64) {
        self.phase = phase;
        self.revision = self.revision.wrapping_add(1);
        self.changed = now;
        self.copied = false;
        self.drawing = false;
        self.media_result = false;
        self.repeat_paste = false;
        self.sticky_result = false;
        self.notice = None;
        self.left_hover = None;
        if self.phase == Phase::Recording {
            self.paused = false;
        }
    }
    /// A short line in place of the strip while recording.
    pub fn show_notice(&mut self, text: &str, now: u64) {
        self.notice = Some((text.to_string(), now + 5000));
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn notice(&self, now: u64) -> Option<&str> {
        self.notice
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(text, _)| text.as_str())
    }
    pub fn hover(&mut self, value: bool, now: u64) {
        if self.hovered && !value {
            self.left_hover = Some(now);
        }
        self.hovered = value;
    }
    pub fn copy(&mut self, now: u64) {
        self.copied = true;
        self.revision = self.revision.wrapping_add(1);
        self.changed = now;
    }
    pub fn opacity(&self, now: u64, reduced: bool) -> f32 {
        let timeout = match self.phase {
            Phase::Hidden => return 0.0,
            Phase::Done(_) if self.copied => Some(self.changed + 1000),
            Phase::Done(_) if self.hovered || self.sticky_result => None,
            Phase::Done(_) => Some(self.left_hover.map(|t| t + 800).unwrap_or(
                self.changed
                    + if self.media_result || self.repeat_paste {
                        10000
                    } else {
                        1750
                    },
            )),
            Phase::Copied if self.hovered => None,
            Phase::Copied => Some(
                self.left_hover
                    .map(|t| t + 800)
                    .unwrap_or(self.changed + 1750),
            ),
            Phase::Error { rescue: false, .. } => Some(self.changed + 5000),
            _ => None,
        };
        match timeout {
            Some(end) if now >= end && reduced => 0.0,
            Some(end) if now >= end => (1.0 - (now - end) as f32 / 250.0).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }
    pub fn morph(&self, now: u64, reduced: bool) -> f32 {
        if reduced {
            1.0
        } else {
            let t = ((now - self.changed) as f32 / 240.0).clamp(0.0, 1.0);
            1.0 - (1.0 - t).powi(3)
        }
    }
    pub fn paint_key(&self) -> PaintKey {
        PaintKey {
            revision: self.revision,
            hovered: self.hovered,
            paused: self.paused,
            copied: self.copied,
            drawing: self.drawing,
            media_result: self.media_result,
            repeat_paste: self.repeat_paste,
            sticky_result: self.sticky_result,
        }
    }
    pub fn interactive(&self) -> bool {
        self.hovered
            || matches!(
                self.phase,
                Phase::Done(_) | Phase::Copied | Phase::NotPasted { .. } | Phase::Error { .. }
            )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_recording_notice_shows_for_five_seconds_and_ends_with_the_recording() {
        let mut s = State::default();
        s.set(Phase::Recording, 0);
        assert_eq!(s.notice(100), None);
        s.show_notice("1 minute left", 100);
        assert_eq!(s.notice(5099), Some("1 minute left"));
        assert_eq!(s.notice(5100), None);
        s.show_notice("1 minute left", 6000);
        s.set(Phase::Transcribing, 6100);
        assert_eq!(s.notice(6200), None);
    }
    #[test]
    fn paint_key_changes_for_controls_without_a_phase_transition() {
        let mut s = State::default();
        s.set(Phase::Recording, 0);
        let recording = s.paint_key();
        s.hover(true, 10);
        assert!(s.paint_key() != recording);
        let hovered = s.paint_key();
        s.hover(true, 20);
        assert!(s.paint_key() == hovered);
        s.paused = true;
        assert!(s.paint_key() != hovered);
        let paused = s.paint_key();
        s.drawing = true;
        assert!(s.paint_key() != paused);
        let drawing = s.paint_key();
        s.copy(30);
        assert!(s.paint_key() != drawing);
    }
    #[test]
    fn transitions_within_one_clock_tick_still_refresh_the_pill() {
        let mut s = State::default();
        s.set(Phase::Recording, 0);
        let recording = s.paint_key();
        s.set(Phase::Transcribing, 0);
        assert!(s.paint_key() != recording);
        s.set(Phase::Done("first".into()), 0);
        let first = s.paint_key();
        s.set(Phase::Done("second".into()), 0);
        assert!(s.paint_key() != first);
    }
    #[test]
    fn done_leaves_at_two_seconds_including_fade() {
        let mut s = State::default();
        s.set(Phase::Done("ok".into()), 0);
        assert_eq!(s.opacity(1750, false), 1.0);
        assert_eq!(s.opacity(2000, false), 0.0);
    }
    #[test]
    fn hover_holds_then_leaves_after_800ms() {
        let mut s = State::default();
        s.set(Phase::Done("ok".into()), 0);
        s.hover(true, 100);
        assert_eq!(s.opacity(9000, false), 1.0);
        s.hover(false, 9000);
        assert_eq!(s.opacity(9800, false), 1.0);
        assert_eq!(s.opacity(10050, false), 0.0);
    }
    #[test]
    fn rescue_never_times_out() {
        let mut s = State::default();
        s.set(Phase::NotPasted { saved: true }, 0);
        assert_eq!(s.opacity(90000, true), 1.0);
        s.set(
            Phase::Error {
                message: "saved".into(),
                rescue: true,
            },
            0,
        );
        assert_eq!(s.opacity(90000, false), 1.0);
    }
    #[test]
    fn unsaved_media_remains_visible_until_copied() {
        let mut s = State::default();
        s.set(Phase::Done("Not saved".into()), 0);
        s.media_result = true;
        s.sticky_result = true;
        assert_eq!(s.opacity(3600000, true), 1.0);
        s.copy(3600000);
        assert_eq!(s.opacity(3601000, true), 0.0);
    }
    #[test]
    fn copy_leaves_after_one_second_with_reduced_motion() {
        let mut s = State::default();
        s.set(Phase::Done("ok".into()), 0);
        s.hover(true, 10);
        s.copy(20);
        assert_eq!(s.opacity(1019, true), 1.0);
        assert_eq!(s.opacity(1020, true), 0.0);
    }
}

/// `copied`: a paste that didn't happen left the text on the clipboard, checked by
/// reading it back. Only then does a failed paste go away by itself.
pub fn delivery_result(
    text: String,
    pasted: bool,
    copied: bool,
    saved: bool,
    history_warning: bool,
) -> Phase {
    if !pasted && copied && saved && !history_warning {
        Phase::Copied
    } else if !pasted {
        Phase::NotPasted { saved }
    } else if !saved || history_warning {
        Phase::Error {
            message: "History failed · Copy from tray".into(),
            rescue: true,
        }
    } else {
        Phase::Done(text)
    }
}
#[cfg(test)]
mod delivery_tests {
    use super::*;
    #[test]
    fn delivery_failure_keeps_actions_for_every_storage_outcome() {
        for saved in [false, true] {
            for warning in [false, true] {
                assert!(
                    matches!(delivery_result("text".into(), false, false, saved, warning), Phase::NotPasted { saved: actual } if actual == saved)
                );
            }
        }
    }
    #[test]
    fn a_failed_paste_that_copied_fades_unless_history_also_failed() {
        assert_eq!(
            delivery_result("text".into(), false, true, true, false),
            Phase::Copied
        );
        assert_eq!(
            delivery_result("text".into(), false, true, false, false),
            Phase::NotPasted { saved: false }
        );
        assert_eq!(
            delivery_result("text".into(), false, true, true, true),
            Phase::NotPasted { saved: true }
        );
        let mut copied = State::default();
        copied.set(Phase::Copied, 0);
        assert_eq!(copied.opacity(1000, true), 1.0);
        assert_eq!(copied.opacity(1750, true), 0.0, "Done's timing");
        copied.set(Phase::Copied, 0);
        copied.hover(true, 100);
        assert_eq!(copied.opacity(60_000, true), 1.0, "hover keeps it");
    }

    #[test]
    fn successful_delivery_still_surfaces_storage_failure() {
        assert!(matches!(
            delivery_result("text".into(), true, false, true, false),
            Phase::Done(_)
        ));
        assert!(matches!(
            delivery_result("text".into(), true, false, true, true),
            Phase::Error { rescue: true, .. }
        ));
        assert!(matches!(
            delivery_result("text".into(), true, false, false, true),
            Phase::Error { rescue: true, .. }
        ));
    }
}
