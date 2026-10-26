pub mod audio;
pub mod history;
pub mod model;
pub mod paths;
pub mod runtime;
pub mod settings;
pub mod shortcut;
pub mod state;
pub mod transcription;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;
