#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppStatus {
    Starting,
    Ready,
    Recording,
    Transcribing,
    Error(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppEvent {
    Prepared,
    Toggle,
    RecordingFinished,
    TranscriptionFinished,
    Failed,
    Recovered,
}

impl AppStatus {
    pub fn next(self, event: AppEvent, error: Option<String>) -> Result<Self, String> {
        match (self, event) {
            (Self::Starting, AppEvent::Prepared) => Ok(Self::Ready),
            (Self::Ready, AppEvent::Toggle) => Ok(Self::Recording),
            (Self::Recording, AppEvent::Toggle | AppEvent::RecordingFinished) => {
                Ok(Self::Transcribing)
            }
            (Self::Transcribing, AppEvent::TranscriptionFinished) => Ok(Self::Ready),
            (_, AppEvent::Failed) => Ok(Self::Error(
                error.unwrap_or_else(|| "Koett failed".to_string()),
            )),
            (Self::Error(_), AppEvent::Recovered) => Ok(Self::Ready),
            (status, event) => Err(format!("invalid Koett transition: {status:?} + {event:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppEvent, AppStatus};

    #[test]
    fn normal_toggle_session_has_one_explicit_path() {
        let status = AppStatus::Starting
            .next(AppEvent::Prepared, None)
            .unwrap()
            .next(AppEvent::Toggle, None)
            .unwrap()
            .next(AppEvent::Toggle, None)
            .unwrap()
            .next(AppEvent::TranscriptionFinished, None)
            .unwrap();

        assert_eq!(status, AppStatus::Ready);
    }

    #[test]
    fn invalid_second_start_is_rejected() {
        assert!(AppStatus::Recording.next(AppEvent::Prepared, None).is_err());
    }

    #[test]
    fn error_can_recover_to_ready() {
        let status = AppStatus::Transcribing
            .next(AppEvent::Failed, Some("ASR failed".to_string()))
            .unwrap();
        assert_eq!(status, AppStatus::Error("ASR failed".to_string()));
        assert_eq!(
            status.next(AppEvent::Recovered, None).unwrap(),
            AppStatus::Ready
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordingIntent {
    Start,
    Stop,
    Toggle,
}

#[derive(Default)]
pub struct ShortcutLatch {
    pressed: bool,
    holding: bool,
}

impl ShortcutLatch {
    pub fn press(&mut self, mode: crate::settings::RecordingMode) -> Option<RecordingIntent> {
        if self.pressed {
            return None;
        }
        self.pressed = true;
        self.holding = mode == crate::settings::RecordingMode::Hold;
        Some(if self.holding {
            RecordingIntent::Start
        } else {
            RecordingIntent::Toggle
        })
    }

    pub fn release(&mut self) -> Option<RecordingIntent> {
        self.pressed = false;
        if std::mem::take(&mut self.holding) {
            Some(RecordingIntent::Stop)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::{RecordingIntent, ShortcutLatch};
    use crate::settings::RecordingMode;

    #[test]
    fn repeated_hold_events_start_and_stop_once() {
        let mut latch = ShortcutLatch::default();
        assert_eq!(
            latch.press(RecordingMode::Hold),
            Some(RecordingIntent::Start)
        );
        assert_eq!(latch.press(RecordingMode::Hold), None);
        assert_eq!(latch.release(), Some(RecordingIntent::Stop));
        assert_eq!(latch.release(), None);
    }

    #[test]
    fn changing_mode_while_held_still_stops_the_original_capture() {
        let mut latch = ShortcutLatch::default();
        latch.press(RecordingMode::Hold);
        assert_eq!(latch.press(RecordingMode::Toggle), None);
        assert_eq!(latch.release(), Some(RecordingIntent::Stop));
        assert_eq!(
            latch.press(RecordingMode::Toggle),
            Some(RecordingIntent::Toggle)
        );
        assert_eq!(latch.release(), None);
    }

    #[test]
    fn toggle_ignores_repeat_until_release() {
        let mut latch = ShortcutLatch::default();
        assert_eq!(
            latch.press(RecordingMode::Toggle),
            Some(RecordingIntent::Toggle)
        );
        assert_eq!(latch.press(RecordingMode::Toggle), None);
        assert_eq!(latch.release(), None);
        assert_eq!(
            latch.press(RecordingMode::Toggle),
            Some(RecordingIntent::Toggle)
        );
    }
}
