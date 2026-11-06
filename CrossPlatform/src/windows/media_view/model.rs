#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::windows) struct Completion {
    pub text: String,
    pub warning: Option<String>,
    pub saved: bool,
}
#[derive(Default, Clone)]
pub(super) struct Model {
    pub ready: bool,
    pub active: bool,
    pub result: String,
    pub message: String,
    pub revision: u64,
    success: bool,
    warning: Option<String>,
    unsaved: bool,
    copied: bool,
}
impl Model {
    pub fn available(&mut self, ready: bool) {
        if self.ready != ready {
            self.ready = ready;
            self.changed();
        }
    }
    pub fn request(&mut self) -> bool {
        if !self.ready || self.active || self.needs_copy() {
            return false;
        }
        self.active = true;
        self.success = false;
        self.warning = None;
        self.unsaved = false;
        self.copied = false;
        self.message = "Preparing media…".into();
        self.changed();
        true
    }
    pub fn status(&mut self, ready: bool) -> Option<Result<Completion, String>> {
        let outcome = if ready && self.active {
            self.active = false;
            Some(if self.success {
                Ok(Completion {
                    text: self.result.clone(),
                    warning: self.warning.clone(),
                    saved: !self.unsaved,
                })
            } else {
                Err(self.message.clone())
            })
        } else {
            None
        };
        if self.ready != ready || outcome.is_some() {
            self.ready = ready;
            self.changed();
        }
        outcome
    }
    pub fn notice(&mut self, value: &str) {
        let value = value
            .strip_prefix(crate::runtime::MEDIA_ERROR)
            .unwrap_or(value);
        if let Some(error) = value.strip_prefix("@media-rejected:") {
            self.active = false;
            self.message = error.into();
            self.changed();
        } else if let Some(text) = value.strip_prefix("@media-result:") {
            self.result = text.into();
            self.copied = false;
            self.message = self.warning.clone().unwrap_or_else(|| "Saved".into());
            self.success = true;
            self.changed();
        } else if self.active {
            if value.starts_with("Media saved in transcript recovery")
                || value.starts_with("Media saved in History")
                || value.starts_with("Media storage failed:")
            {
                self.warning = Some(if value.starts_with("Media storage failed:") {
                    "Not saved · Copy before quitting".into()
                } else {
                    value.into()
                });
                self.unsaved = value.starts_with("Media storage failed:");
            }
            let message = self.warning.as_deref().unwrap_or(value).to_owned();
            self.local_notice(&message);
        }
    }
    pub fn needs_copy(&self) -> bool {
        self.unsaved && !self.copied && !self.result.is_empty()
    }
    pub fn copied(&mut self) {
        self.copied = true;
        self.local_notice("Copied");
    }
    pub fn discard(&mut self) {
        if !self.active {
            self.result.clear();
            self.unsaved = false;
            self.warning = None;
            self.success = false;
            self.copied = false;
            self.local_notice("Discarded");
        }
    }
    pub fn local_notice(&mut self, value: &str) {
        self.message = value.into();
        self.changed();
    }
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successful_media_finishes_once_and_keeps_its_result() {
        let mut m = Model::default();
        m.status(true);
        assert!(m.request());
        assert!(!m.request());
        m.status(false);
        m.notice("@media-result:Raw words");
        assert_eq!(
            m.status(true),
            Some(Ok(Completion {
                text: "Raw words".into(),
                warning: None,
                saved: true
            }))
        );
        assert_eq!(m.status(true), None);
        assert!(!m.active);
        assert_eq!(m.result, "Raw words");
    }
    #[test]
    fn invalid_source_returns_to_ready_and_can_retry() {
        let mut m = Model::default();
        m.status(true);
        m.request();
        m.notice("Enter an HTTP(S) URL");
        assert_eq!(m.status(true), Some(Err("Enter an HTTP(S) URL".into())));
        assert!(m.request());
    }
    #[test]
    fn rejected_and_cancelled_work_do_not_erase_last_media() {
        let mut m = Model::default();
        m.status(true);
        m.request();
        m.notice("@media-result:Keep this");
        m.status(true);
        m.request();
        m.notice("@media-rejected:Koett is busy");
        assert!(!m.active);
        assert_eq!(m.result, "Keep this");
        assert!(m.request());
        m.status(false);
        m.notice("Media cancelled");
        assert_eq!(m.status(true), Some(Err("Media cancelled".into())));
        assert_eq!(m.result, "Keep this");
    }
    #[test]
    fn recovery_and_unsaved_warnings_survive_successful_asr() {
        for (message, saved) in [
            ("Media saved in transcript recovery", true),
            ("Media storage failed: blocked. Copy before quitting", false),
            ("Media saved in History · Media file unavailable", true),
        ] {
            let mut m = Model::default();
            m.status(true);
            m.request();
            m.status(false);
            m.notice(message);
            m.notice("@media-result:Keep raw words");
            let completed = m.status(true).unwrap().unwrap();
            assert_eq!(completed.text, "Keep raw words");
            let expected = if saved {
                message
            } else {
                "Not saved · Copy before quitting"
            };
            assert_eq!(completed.warning.as_deref(), Some(expected));
            assert_eq!(completed.saved, saved);
            assert_eq!(m.message, expected);
            assert_eq!(m.result, "Keep raw words");
        }
    }
    #[test]
    fn unsaved_result_requires_copy_or_explicit_discard_before_replacement() {
        for discard in [false, true] {
            let mut m = Model::default();
            m.status(true);
            m.request();
            m.notice("Media storage failed: blocked");
            m.notice("@media-result:Keep me");
            m.status(true);
            assert!(m.needs_copy());
            assert!(!m.request());
            if discard {
                m.discard();
                assert!(m.result.is_empty());
            } else {
                m.copied();
                assert_eq!(m.result, "Keep me");
            }
            assert!(!m.needs_copy());
            assert!(m.request());
        }
    }
    #[test]
    fn unrelated_dictation_notices_cannot_replace_media() {
        let mut m = Model::default();
        m.notice("Microphone unavailable");
        assert!(m.message.is_empty());
        m.status(false);
        assert!(!m.request());
    }
}
