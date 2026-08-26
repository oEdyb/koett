use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Settings {
    pub shortcut: String,
    pub start_at_login: bool,
    pub model_directory: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: "Ctrl+Shift+Space".to_string(),
            start_at_login: true,
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
        fs::write(path, data)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("could not protect {}: {error}", path.display()))?;
        }
        Ok(())
    }

    pub fn model_directory(&self) -> Result<PathBuf, String> {
        match &self.model_directory {
            Some(path) => Ok(path.clone()),
            None => paths::default_model_directory(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::Settings;

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
            model_directory: Some("custom-model".into()),
        };

        settings.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path).unwrap(), settings);

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\n  \"shortcut\""));
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
