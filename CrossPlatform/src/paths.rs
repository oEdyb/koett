use std::path::PathBuf;

use directories::ProjectDirs;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "DybergCo";
const APPLICATION: &str = "Koett";

fn project_directories() -> Result<ProjectDirs, String> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        .ok_or_else(|| "could not find the current user's data directory".to_string())
}

pub fn settings_file() -> Result<PathBuf, String> {
    Ok(project_directories()?.config_dir().join("settings.json"))
}

pub fn history_file() -> Result<PathBuf, String> {
    Ok(project_directories()?.data_dir().join("Transcripts.md"))
}

pub fn default_model_directory() -> Result<PathBuf, String> {
    if let Some(directory) = std::env::var_os("KOETT_MODEL_DIR") {
        return Ok(PathBuf::from(directory));
    }

    let executable = std::env::current_exe()
        .map_err(|error| format!("could not find the Koett executable: {error}"))?;
    let parent = executable
        .parent()
        .ok_or_else(|| "the Koett executable has no parent directory".to_string())?;
    let bundled = parent.join("model");
    if bundled.is_dir() {
        return Ok(bundled);
    }

    Ok(project_directories()?.data_dir().join("model"))
}

#[cfg(test)]
mod tests {
    use super::{history_file, settings_file};

    #[test]
    fn settings_and_history_use_platform_directories() {
        let settings = settings_file().unwrap();
        let history = history_file().unwrap();

        assert_eq!(settings.file_name().unwrap(), "settings.json");
        assert_eq!(history.file_name().unwrap(), "Transcripts.md");
        assert!(settings.parent().is_some());
        assert!(history.parent().is_some());
    }
}
