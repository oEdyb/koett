#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Hidden,
    Recording,
    Transcribing,
    Done(String),
    NotPasted,
    Error { message: String, rescue: bool },
}

pub struct State {
    pub phase: Phase,
    pub hovered: bool,
    pub paused: bool,
    pub copied: bool,
    changed: u64,
    left_hover: Option<u64>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            phase: Phase::Hidden,
            hovered: false,
            paused: false,
            copied: false,
            changed: 0,
            left_hover: None,
        }
    }
}
impl State {
    pub fn set(&mut self, phase: Phase, now: u64) {
        self.phase = phase;
        self.changed = now;
        self.copied = false;
        self.left_hover = None;
        if self.phase == Phase::Recording {
            self.paused = false;
        }
    }
    pub fn hover(&mut self, value: bool, now: u64) {
        if self.hovered && !value {
            self.left_hover = Some(now);
        }
        self.hovered = value;
    }
    pub fn copy(&mut self, now: u64) {
        self.copied = true;
        self.changed = now;
    }
    pub fn opacity(&self, now: u64, reduced: bool) -> f32 {
        let timeout = match self.phase {
            Phase::Hidden => return 0.0,
            Phase::Done(_) if self.copied => Some(self.changed + 1000),
            Phase::Done(_) if self.hovered => None,
            Phase::Done(_) => Some(
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
    pub fn interactive(&self) -> bool {
        self.hovered
            || matches!(
                self.phase,
                Phase::Done(_) | Phase::NotPasted | Phase::Error { .. }
            )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
        s.set(Phase::NotPasted, 0);
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
    fn copy_leaves_after_one_second_with_reduced_motion() {
        let mut s = State::default();
        s.set(Phase::Done("ok".into()), 0);
        s.hover(true, 10);
        s.copy(20);
        assert_eq!(s.opacity(1019, true), 1.0);
        assert_eq!(s.opacity(1020, true), 0.0);
    }
}
