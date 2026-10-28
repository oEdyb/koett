use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CaptureWarning {
    Clipping,
}

impl CaptureWarning {
    pub fn text(self) -> &'static str {
        match self {
            Self::Clipping => "Input too loud",
        }
    }
}

#[derive(Default)]
pub(super) struct CaptureFeedback {
    candidate: Option<(CaptureWarning, Duration)>,
}

impl CaptureFeedback {
    pub fn update(&mut self, now: Duration, rms: f32, peak: f32) -> Option<CaptureWarning> {
        let candidate = if !rms.is_finite() || !peak.is_finite() {
            None
        } else if peak >= 0.995 {
            Some(CaptureWarning::Clipping)
        } else {
            None
        };
        let Some(candidate) = candidate else {
            self.candidate = None;
            return None;
        };
        let since = match self.candidate {
            Some((previous, since)) if previous == candidate => since,
            _ => now,
        };
        self.candidate = Some((candidate, since));
        let delay = Duration::from_millis(600);
        (now.saturating_sub(since) >= delay).then_some(candidate)
    }
}

pub(super) const RECOVERY_DURATION: Duration = Duration::from_secs(20);

pub(super) fn copy_transcript(
    clipboard: &gtk4::gdk::Clipboard,
    text: &str,
) -> Result<(), gtk4::glib::BoolError> {
    use gtk4::prelude::*;
    let content = gtk4::gdk::ContentProvider::for_value(&text.to_value());
    clipboard.set_content(Some(&content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_never_warns_and_only_sustained_loud_input_does() {
        let mut state = CaptureFeedback::default();
        assert_eq!(state.update(Duration::ZERO, 0.0, 0.0), None);
        assert_eq!(state.update(Duration::from_secs(4), 0.0, 0.0), None);
        assert_eq!(state.update(Duration::from_secs(5), 0.0, 0.0), None);
        assert_eq!(state.update(Duration::from_secs(60), 0.0, 0.0), None);
        assert_eq!(state.update(Duration::from_secs(600), 0.0, 0.0), None);
        let mut state = CaptureFeedback::default();
        assert_eq!(state.update(Duration::from_secs(6), 0.05, 0.3), None);
        assert_eq!(state.update(Duration::from_secs(7), 0.8, 1.0), None);
        assert_eq!(state.update(Duration::from_millis(7599), 0.8, 1.0), None);
        assert_eq!(
            state.update(Duration::from_millis(7600), 0.8, 1.0),
            Some(CaptureWarning::Clipping)
        );
        assert_eq!(state.update(Duration::from_secs(8), 0.05, 0.3), None);
    }

    #[test]
    fn interrupted_levels_and_new_capture_restart_debounce() {
        let mut state = CaptureFeedback::default();
        state.update(Duration::ZERO, 0.0, 0.0);
        state.update(Duration::from_secs(4), 0.02, 0.1);
        assert_eq!(state.update(Duration::from_secs(5), 0.0, 0.0), None);
        assert_eq!(state.update(Duration::from_secs(9), 0.0, 0.0), None);
        assert_eq!(state.update(Duration::from_secs(10), f32::NAN, 1.0), None);
        let mut fresh = CaptureFeedback::default();
        assert_eq!(fresh.update(Duration::from_secs(20), 0.0, 0.0), None);
        assert_eq!(fresh.update(Duration::from_secs(21), 0.7, 1.0), None);
    }
}
