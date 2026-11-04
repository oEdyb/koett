use std::time::{Duration, Instant};

use super::sketch::keys::{Keyboards, Modifiers};

#[derive(Default)]
struct Gate {
    released: bool,
}

impl Gate {
    fn press(&mut self, keys: Modifiers) -> bool {
        if !keys.escape {
            self.released = true;
            return false;
        }
        let cancel = self.released && !keys.shift && !keys.control && !keys.alt && !keys.meta;
        self.released = false;
        cancel
    }
}

#[derive(Default)]
pub(super) struct EscapeMonitor {
    active: bool,
    keyboards: Option<Keyboards>,
    refreshed: Option<Instant>,
    gate: Gate,
}

impl EscapeMonitor {
    pub fn poll(&mut self, recording: bool) -> bool {
        if !recording {
            *self = Self::default();
            return false;
        }
        if !self.active {
            self.active = true;
            self.keyboards = Keyboards::open().ok();
            self.refreshed = Some(Instant::now());
        }
        if self
            .refreshed
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(1))
        {
            if let Some(keyboards) = &mut self.keyboards {
                keyboards.refresh();
            } else {
                self.keyboards = Keyboards::open().ok();
            }
            self.refreshed = Some(Instant::now());
        }
        let Some(keyboards) = &mut self.keyboards else {
            return false;
        };
        let keys = keyboards.modifiers();
        if keyboards.is_empty() {
            self.gate = Gate::default();
            return false;
        }
        self.gate.press(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_release_and_ignores_repeat_and_modified_escape() {
        let mut gate = Gate::default();
        let escape = Modifiers {
            escape: true,
            ..Default::default()
        };
        assert!(!gate.press(escape));
        assert!(!gate.press(Modifiers::default()));
        assert!(!gate.press(Modifiers {
            control: true,
            ..escape
        }));
        assert!(!gate.press(escape));
        assert!(!gate.press(Modifiers::default()));
        assert!(gate.press(escape));
        assert!(!gate.press(escape));
    }

    #[test]
    fn leaving_recording_closes_devices_and_disarms() {
        let mut monitor = EscapeMonitor {
            active: true,
            gate: Gate { released: true },
            ..Default::default()
        };
        assert!(!monitor.poll(false));
        assert!(!monitor.active);
        assert!(monitor.keyboards.is_none());
        assert!(!monitor.gate.press(Modifiers {
            escape: true,
            ..Default::default()
        }));
    }
}
