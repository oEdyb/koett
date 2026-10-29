use std::fs;
use std::path::{Path, PathBuf};

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
    pub fn load() -> Result<Self, String> {
        Self::load_from(&paths::settings_file()?)
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        match fs::read(path) {
            Ok(data) => serde_json::from_slice(&data)
                .map_err(|error| format!("could not read {}: {error}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("could not read {}: {error}", path.display())),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&paths::settings_file()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        let data = serde_json::to_vec_pretty(self)
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
    fn missing_file_uses_small_public_defaults() {
        let path = temporary_file("missing-settings");
        let settings = Settings::load_from(&path).unwrap();

        assert_eq!(settings.shortcut, "Ctrl+Shift+Space");
        assert!(settings.start_at_login);
        assert!(settings.model_directory.is_none());
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
        let updated = Settings {
            selected_model: crate::model::LocalModel::ParakeetV2,
            ..original.clone()
        };
        let result = updated.save_to(&path);
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(result.is_err());
        assert_eq!(Settings::load_from(&path).unwrap(), original);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_json_is_not_silently_replaced() {
        let path = temporary_file("invalid-settings");
        fs::write(&path, b"not json").unwrap();

        assert!(Settings::load_from(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
