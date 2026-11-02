use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static EDIT_LOCK: Mutex<()> = Mutex::new(());
const ACTIVE_SHORTCUT_COUNT: usize = if cfg!(target_os = "linux") { 3 } else { 1 };

pub struct SettingsEdit {
    pub settings: Settings,
    path: PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl SettingsEdit {
    /// Persist without releasing the edit lock, including when persistence fails.
    pub fn commit(&self) -> Result<(), String> {
        self.settings.save_unlocked(&self.path)
    }
}

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    #[default]
    Toggle,
    Hold,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PasteShortcut {
    #[default]
    CtrlV,
    CtrlShiftV,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Settings {
    pub formatting: crate::formatting::Formatting,
    pub shortcut: String,
    pub ask_shortcut: String,
    pub media_shortcut: String,
    pub start_at_login: bool,
    pub microphone: Option<String>,
    pub recording_mode: RecordingMode,
    pub replacements: Vec<crate::replacements::Replacement>,
    pub pause_media: bool,
    pub paste_shortcut: PasteShortcut,
    pub selected_model: crate::model::LocalModel,
    pub model_directory: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            formatting: crate::formatting::Formatting::default(),
            shortcut: "Ctrl+Shift+Space".to_string(),
            ask_shortcut: "Ctrl+Alt+Space".to_string(),
            media_shortcut: "Ctrl+Shift+T".to_string(),
            start_at_login: true,
            microphone: None,
            recording_mode: RecordingMode::Toggle,
            replacements: Vec::new(),
            pause_media: false,
            paste_shortcut: PasteShortcut::default(),
            selected_model: crate::model::LocalModel::default(),
            model_directory: None,
        }
    }
}

impl Settings {
    pub fn edit() -> Result<SettingsEdit, String> {
        Self::edit_from(&paths::settings_file()?)
    }

    pub fn edit_from(path: &Path) -> Result<SettingsEdit, String> {
        let guard = EDIT_LOCK.lock().map_err(|error| error.to_string())?;
        Ok(SettingsEdit {
            settings: Self::load_from(path)?,
            path: path.to_path_buf(),
            _guard: guard,
        })
    }

    pub fn update(patch: impl FnOnce(&mut Self)) -> Result<(), String> {
        let mut edit = Self::edit()?;
        patch(&mut edit.settings);
        edit.commit()
    }

    pub fn load() -> Result<Self, String> {
        Self::load_from(&paths::settings_file()?)
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        match fs::read(path) {
            Ok(data) => {
                let value: serde_json::Value = serde_json::from_slice(&data)
                    .map_err(|error| format!("could not read {}: {error}", path.display()))?;
                let mut settings: Self = serde_json::from_value(value.clone())
                    .map_err(|error| format!("could not read {}: {error}", path.display()))?;
                for action in ["ask_shortcut", "media_shortcut"] {
                    if value.get(action).is_some() {
                        continue;
                    }
                    let candidate = if action == "ask_shortcut" {
                        &settings.ask_shortcut
                    } else {
                        &settings.media_shortcut
                    };
                    let parsed = candidate.parse::<crate::shortcut::Shortcut>().ok();
                    let other = if action == "ask_shortcut" {
                        &settings.media_shortcut
                    } else {
                        &settings.ask_shortcut
                    };
                    if parsed.is_some()
                        && [&settings.shortcut, other].iter().any(|binding| {
                            binding.parse::<crate::shortcut::Shortcut>().ok() == parsed
                        })
                    {
                        if action == "ask_shortcut" {
                            settings.ask_shortcut.clear();
                        } else {
                            settings.media_shortcut.clear();
                        }
                    }
                }
                Ok(settings)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("could not read {}: {error}", path.display())),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&paths::settings_file()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let _guard = EDIT_LOCK.lock().map_err(|error| error.to_string())?;
        self.save_unlocked(path)
    }

    fn save_unlocked(&self, path: &Path) -> Result<(), String> {
        self.validate_shortcuts()?;
        let parent = path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        let mut normalized = self.clone();
        for value in [
            &mut normalized.shortcut,
            &mut normalized.ask_shortcut,
            &mut normalized.media_shortcut,
        ]
        .into_iter()
        .take(ACTIVE_SHORTCUT_COUNT)
        {
            if !value.is_empty() {
                *value = value.parse::<crate::shortcut::Shortcut>()?.to_string();
            }
        }
        let data = serde_json::to_vec_pretty(&normalized)
            .map_err(|error| format!("could not encode settings: {error}"))?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let temporary = parent.join(format!(".settings-{}-{nonce}.part", std::process::id()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("could not prepare settings: {error}"))?;
        let result = (|| {
            use std::io::Write as _;
            file.write_all(&data)
                .map_err(|error| format!("could not write settings: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("could not sync settings: {error}"))?;
            drop(file);
            fs::rename(&temporary, path)
                .map_err(|error| format!("could not save {}: {error}", path.display()))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        Ok(())
    }

    pub fn merge_editor(self, opened: &Self, current: Self) -> Self {
        let mut merged = current;
        if self.shortcut != opened.shortcut {
            merged.shortcut = self.shortcut;
        }
        if self.ask_shortcut != opened.ask_shortcut {
            merged.ask_shortcut = self.ask_shortcut;
        }
        if self.media_shortcut != opened.media_shortcut {
            merged.media_shortcut = self.media_shortcut;
        }
        if self.microphone != opened.microphone {
            merged.microphone = self.microphone;
        }
        if self.recording_mode != opened.recording_mode {
            merged.recording_mode = self.recording_mode;
        }
        if self.start_at_login != opened.start_at_login {
            merged.start_at_login = self.start_at_login;
        }
        if self.replacements != opened.replacements {
            merged.replacements = self.replacements;
        }
        if self.pause_media != opened.pause_media {
            merged.pause_media = self.pause_media;
        }
        if self.paste_shortcut != opened.paste_shortcut {
            merged.paste_shortcut = self.paste_shortcut;
        }
        merged
    }

    pub fn validate_shortcuts(&self) -> Result<(), String> {
        let mut seen = Vec::new();
        for (name, value) in [
            ("Dictation", &self.shortcut),
            ("Ask", &self.ask_shortcut),
            ("Media", &self.media_shortcut),
        ]
        .into_iter()
        .take(ACTIVE_SHORTCUT_COUNT)
        {
            if value.is_empty() {
                continue;
            }
            let chord: crate::shortcut::Shortcut =
                value.parse().map_err(|error| format!("{name}: {error}"))?;
            if let Some((other, _)) = seen.iter().find(|(_, previous)| previous == &chord) {
                return Err(format!(
                    "{name} conflicts with {other}. Choose separate shortcuts."
                ));
            }
            seen.push((name, chord));
        }
        Ok(())
    }

    pub fn model_directory(&self) -> Result<PathBuf, String> {
        match &self.model_directory {
            Some(path) => Ok(path.clone()),
            None => self.selected_model.directory(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{PasteShortcut, Settings};

    fn temporary_file(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("koett-{name}-{nonce}.json"))
    }

    #[test]
    fn concurrent_owned_field_edits_keep_both_changes() {
        use std::sync::mpsc;
        use std::time::Duration;

        let path = temporary_file("concurrent-settings");
        Settings::default().save_to(&path).unwrap();
        let mut first = Settings::edit_from(&path).unwrap();
        first.settings.recording_mode = super::RecordingMode::Hold;
        let (attempted, attempts) = mpsc::channel();
        let (finished, finishes) = mpsc::channel();
        let second_path = path.clone();
        let second = std::thread::spawn(move || {
            attempted.send(()).unwrap();
            let mut edit = Settings::edit_from(&second_path).unwrap();
            edit.settings.replacements = vec![crate::replacements::Replacement {
                phrase: "spoken".into(),
                replacement: "written".into(),
            }];
            edit.commit().unwrap();
            finished.send(()).unwrap();
        });
        attempts.recv_timeout(Duration::from_secs(5)).unwrap();
        first.commit().unwrap();
        // Commit keeps the lock so side-effect rollback can finish before another edit.
        assert!(super::EDIT_LOCK.try_lock().is_err());
        assert!(finishes.try_recv().is_err());
        drop(first);
        finishes.recv_timeout(Duration::from_secs(5)).unwrap();
        second.join().unwrap();
        let saved = Settings::load_from(&path).unwrap();
        assert_eq!(saved.recording_mode, super::RecordingMode::Hold);
        assert_eq!(saved.replacements[0].replacement, "written");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn failed_transaction_preserves_file_and_releases_lock() {
        let path = temporary_file("failed-transaction");
        Settings::default().save_to(&path).unwrap();
        let original = fs::read(&path).unwrap();
        {
            let mut edit = Settings::edit_from(&path).unwrap();
            edit.settings.shortcut = "invalid-shortcut".into();
            assert!(edit.commit().is_err());
            assert!(super::EDIT_LOCK.try_lock().is_err());
            assert_eq!(fs::read(&path).unwrap(), original);
        }
        let mut edit = Settings::edit_from(&path).unwrap();
        edit.settings.pause_media = true;
        edit.commit().unwrap();
        drop(edit);
        assert!(Settings::load_from(&path).unwrap().pause_media);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn missing_file_uses_small_public_defaults() {
        let path = temporary_file("missing-settings");
        let settings = Settings::load_from(&path).unwrap();

        assert_eq!(settings.shortcut, "Ctrl+Shift+Space");
        assert!(settings.start_at_login);
        assert!(settings.model_directory.is_none());
    }

    #[test]
    fn legacy_bindings_keep_working_when_new_defaults_would_collide() {
        for (chord, disabled) in [
            ("Control+Alt+Space", "ask_shortcut"),
            ("Ctrl+Shift+T", "media_shortcut"),
        ] {
            let path = temporary_file("legacy-shortcuts");
            fs::write(&path, serde_json::json!({"shortcut":chord}).to_string()).unwrap();
            let mut settings = Settings::load_from(&path).unwrap();
            assert_eq!(settings.shortcut, chord);
            settings.validate_shortcuts().unwrap();
            assert_eq!(serde_json::to_value(&settings).unwrap()[disabled], "");
            settings.pause_media = true;
            settings.save_to(&path).unwrap();
            let mut expected = settings;
            expected.shortcut = chord
                .parse::<crate::shortcut::Shortcut>()
                .unwrap()
                .to_string();
            assert_eq!(Settings::load_from(&path).unwrap(), expected);
            fs::remove_file(path).unwrap();
        }
        let path = temporary_file("explicit-shortcut-conflict");
        fs::write(
            &path,
            r#"{"shortcut":"Ctrl+Alt+Space","ask_shortcut":"Ctrl+Alt+Space"}"#,
        )
        .unwrap();
        assert_eq!(
            Settings::load_from(&path)
                .unwrap()
                .validate_shortcuts()
                .is_err(),
            cfg!(target_os = "linux")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn settings_round_trip_as_readable_json() {
        let path = temporary_file("settings-round-trip");
        let settings = Settings {
            shortcut: "Alt+Space".to_string(),
            start_at_login: false,
            selected_model: crate::model::LocalModel::ParakeetV2,
            paste_shortcut: PasteShortcut::CtrlShiftV,
            model_directory: Some("custom-model".into()),
            ..Settings::default()
        };

        settings.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path).unwrap(), settings);

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\n  \"shortcut\""));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn paste_shortcut_defaults_for_existing_settings_and_rejects_unknown_chords() {
        let settings: Settings = serde_json::from_str(r#"{"shortcut":"Alt+Space"}"#).unwrap();
        assert_eq!(settings.paste_shortcut, PasteShortcut::CtrlV);
        assert!(serde_json::from_str::<Settings>(r#"{"paste_shortcut":"shift_insert"}"#).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn failed_save_preserves_previous_preferences() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory = temporary_file("read-only-settings");
        fs::create_dir(&directory).unwrap();
        let path = directory.join("settings.json");
        let original = Settings::default();
        original.save_to(&path).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o500)).unwrap();
        let mut edit = Settings::edit_from(&path).unwrap();
        edit.settings.selected_model = crate::model::LocalModel::ParakeetV2;
        let result = edit.commit();
        drop(edit);
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(result.is_err());
        assert_eq!(Settings::load_from(&path).unwrap(), original);
        let mut edit = Settings::edit_from(&path).unwrap();
        edit.settings.pause_media = true;
        edit.commit().unwrap();
        drop(edit);
        assert!(Settings::load_from(&path).unwrap().pause_media);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn stale_editor_preserves_live_changes_and_applies_only_user_edits() {
        let opened = Settings::default();
        let current = Settings {
            shortcut: "Alt+K".into(),
            microphone: Some("replacement".into()),
            selected_model: crate::model::LocalModel::ParakeetV2,
            ..opened.clone()
        };
        let edited = Settings {
            pause_media: true,
            ..opened.clone()
        };
        let merged = edited.merge_editor(&opened, current.clone());
        assert_eq!(merged.shortcut, "Alt+K");
        assert_eq!(merged.microphone.as_deref(), Some("replacement"));
        assert_eq!(merged.selected_model, current.selected_model);
        assert!(merged.pause_media);
        let edited = Settings {
            shortcut: "Alt+L".into(),
            ..opened.clone()
        };
        assert_eq!(edited.merge_editor(&opened, current).shortcut, "Alt+L");
    }

    #[test]
    fn shortcuts_migrate_validate_and_merge_independently() {
        let opened: Settings =
            serde_json::from_str(r#"{"shortcut":"Alt+K","pause_media":true}"#).unwrap();
        assert_eq!(opened.ask_shortcut, "Ctrl+Alt+Space");
        assert_eq!(opened.media_shortcut, "Ctrl+Shift+T");
        assert!(opened.validate_shortcuts().is_ok());
        let mut edited = opened.clone();
        edited.ask_shortcut = "control+shift+t".into();
        if cfg!(target_os = "linux") {
            assert!(
                edited
                    .validate_shortcuts()
                    .unwrap_err()
                    .contains("conflicts")
            );
        } else {
            assert!(edited.validate_shortcuts().is_ok());
        }
        edited.ask_shortcut = String::new();
        assert!(edited.validate_shortcuts().is_ok());
        let mut current = opened.clone();
        current.media_shortcut = "Alt+M".into();
        current.pause_media = false;
        let merged = edited.merge_editor(&opened, current);
        assert!(merged.ask_shortcut.is_empty());
        assert_eq!(merged.media_shortcut, "Alt+M");
        assert!(!merged.pause_media);
        assert_eq!(merged.shortcut, "Alt+K");
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn inactive_auxiliary_shortcuts_do_not_reserve_dictation_keys() {
        let path = temporary_file("inactive-shortcuts");
        for chord in ["Ctrl+Alt+Space", "Ctrl+Shift+T"] {
            let settings = Settings {
                shortcut: chord.into(),
                ..Settings::default()
            };
            settings.save_to(&path).unwrap();
            assert_eq!(Settings::load_from(&path).unwrap().shortcut, chord);
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_json_is_not_silently_replaced() {
        let path = temporary_file("invalid-settings");
        fs::write(&path, b"not json").unwrap();

        assert!(Settings::load_from(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
