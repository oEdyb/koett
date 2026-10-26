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
