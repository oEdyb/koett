use super::{Settings, TrayAction};
#[cfg(feature = "ask")]
use gtk4::{self as gtk, prelude::*};
use std::sync::{Arc, mpsc};

#[derive(Clone, Debug)]
pub(super) struct Reply(pub Arc<mpsc::Sender<serde_json::Value>>);
impl PartialEq for Reply {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Reply {}

pub(super) fn request(sender: &mpsc::Sender<TrayAction>, command: String) -> serde_json::Value {
    let (tx, rx) = mpsc::channel();
    if sender
        .send(TrayAction::Menu(command, Reply(Arc::new(tx))))
        .is_err()
    {
        return error("Koett is stopping");
    }
    rx.recv_timeout(std::time::Duration::from_secs(4))
        .unwrap_or_else(|_| error("No response; inspect menu-status before retrying"))
}

pub(super) fn error(message: &str) -> serde_json::Value {
    serde_json::json!({"ok":false,"error":{"code":"menu_action_failed","message":message,"fix":"Wait for work to finish, then retry; use menu-status to inspect preferences."}})
}

pub(super) fn operation(command: &str, busy: bool) -> serde_json::Value {
    let result = (|| {
        if command == "menu-status" {
            let settings = Settings::load()?;
            return Ok(
                serde_json::json!({"ok":true,"busy":busy,"desired_model":settings.selected_model,"active_engine":ACTIVE.read().map_err(|e| e.to_string())?.clone(),"microphone":settings.microphone,"microphones":input_status(),"recording_mode":settings.recording_mode,"start_at_login":settings.start_at_login,"login_registered":login_registered(),"formatting":settings.formatting,"paste_shortcut":settings.paste_shortcut,"image_paste_shortcuts":settings.image_paste_shortcuts,"ctc_dictionary_bias":crate::transcription::ctc_dictionary_bias_status(),"version":env!("CARGO_PKG_VERSION")}),
            );
        }
        if busy {
            return Err("Finish recording or processing before changing preferences".into());
        }
        if let Some(rule) = command.strip_prefix("menu-image-paste ") {
            let (app, chord) = rule
                .rsplit_once(' ')
                .ok_or("Use menu-image-paste APP_ID ctrl_v|ctrl_shift_v|default")?;
            let parsed = super::image_paste::parse(&format!(
                "{app} = {}",
                match chord {
                    "ctrl_shift_v" => "Ctrl+Shift+V",
                    "ctrl_v" | "default" => "Ctrl+V",
                    _ => return Err("Choose ctrl_v, ctrl_shift_v or default".into()),
                }
            ))?;
            Settings::update(|settings| {
                if chord == "default" {
                    settings.image_paste_shortcuts.remove(app.trim());
                } else {
                    settings.image_paste_shortcuts.extend(parsed);
                }
            })?;
            return Ok(serde_json::json!({"ok":true}));
        }
        if let Some(mode) = command.strip_prefix("menu-mode ") {
            let mode = match mode {
                "toggle" => crate::settings::RecordingMode::Toggle,
                "hold" => crate::settings::RecordingMode::Hold,
                _ => return Err("Choose toggle or hold".into()),
            };
            Settings::update(|settings| settings.recording_mode = mode)?;
        } else if let Some(id) = command.strip_prefix("menu-microphone ") {
            let microphone = if id == "default" {
                None
            } else {
                if !crate::audio::input_devices()?.iter().any(|d| d.id == id) {
                    return Err("Microphone unavailable; refresh Settings > Dictation".into());
                }
                Some(id.into())
            };
            Settings::update(|settings| settings.microphone = microphone)?;
        } else if let Some(path) = command.strip_prefix("menu-dictionary-import ") {
            return import_dictionary(std::path::Path::new(path));
        } else if let Some(value) = command.strip_prefix("menu-login ") {
            let enabled = match value {
                "on" => true,
                "off" => false,
                _ => return Err("Choose on or off".into()),
            };
            let mut edit = Settings::edit()?;
            let previous = login_registered();
            super::set_start_at_login(enabled)?;
            if let Err(error) = {
                edit.settings.start_at_login = enabled;
                edit.commit()
            } {
                let _ = super::set_start_at_login(previous);
                return Err(error);
            }
            return Ok(serde_json::json!({"ok":true}));
        } else {
            return Err("Unknown menu command; use menu-help".into());
        }
        Ok(serde_json::json!({"ok":true}))
    })();
    result.unwrap_or_else(|message: String| error(&message))
}

fn import_dictionary(path: &std::path::Path) -> Result<serde_json::Value, String> {
    if !path.is_absolute() {
        return Err("Give the dictionary file as an absolute path".into());
    }
    let imported = crate::dictionary::read_import(path)?;
    let mut edit = Settings::edit()?;
    edit.settings.dictionary = crate::dictionary::merge(&edit.settings.dictionary, imported)?;
    let entries = edit.settings.dictionary.len();
    edit.commit()?;
    Ok(serde_json::json!({"ok":true,"entries":entries}))
}

pub(super) fn login_registered() -> bool {
    directories::BaseDirs::new().is_some_and(|dirs| {
        dirs.config_dir()
            .join("autostart")
            .join(format!("{}.desktop", super::APP_ID))
            .is_file()
    })
}

pub(super) fn formatting_patch(
    current: crate::formatting::Formatting,
    value: &str,
) -> Result<crate::formatting::Formatting, String> {
    let mut json = serde_json::to_value(current).map_err(|e| e.to_string())?;
    match value {
        "raw" | "s1-mini" => json["enabled"] = (value == "s1-mini").into(),
        _ => {
            let (field, value) = value
                .split_once(' ')
                .ok_or("Choose raw, s1-mini, style VALUE, structure VALUE or context VALUE")?;
            if !matches!(field, "style" | "structure" | "context") {
                return Err("Unknown formatting field".into());
            }
            json[field] = value.into();
        }
    }
    serde_json::from_value(json).map_err(|e| format!("Invalid formatting choice: {e}"))
}

#[cfg(feature = "ask")]
pub(super) fn ask_state() -> (&'static str, bool) {
    let status = crate::assistant::status();
    match status["phase"].as_str().unwrap_or("") {
        "Listening…" => ("Stop Asking", true),
        "Transcribing…" | "Answering…" => ("Interrupt and Ask", true),
        _ => ("Ask", false),
    }
}

fn input_status() -> serde_json::Value {
    match INPUTS.read() {
        Ok(inputs) => match &*inputs {
            Ok(inputs) => {
                serde_json::json!({"ok":true,"devices":inputs.iter().map(|d| serde_json::json!({"id":d.id,"label":d.label})).collect::<Vec<_>>()})
            }
            Err(message) => serde_json::json!({"ok":false,"message":message}),
        },
        Err(_) => serde_json::json!({"ok":false,"message":"Microphones unavailable"}),
    }
}

pub(super) static ACTIVE: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

#[cfg(feature = "ask")]
pub(super) fn attach(application: &gtk::Application) {
    let application = application.clone();
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        let (_, active) = ask_state();
        for (name, enabled) in [
            (
                "ask",
                active || !super::QUIT_BUSY.load(std::sync::atomic::Ordering::Acquire),
            ),
            ("cancel-ask", active),
        ] {
            if let Some(action) = application
                .lookup_action(name)
                .and_then(|a| a.downcast::<gtk::gio::SimpleAction>().ok())
            {
                action.set_enabled(enabled);
            }
        }
        gtk::glib::ControlFlow::Continue
    });
}

pub(super) static INPUTS: std::sync::LazyLock<
    std::sync::RwLock<Result<Vec<crate::audio::InputDevice>, String>>,
> = std::sync::LazyLock::new(|| std::sync::RwLock::new(Err("Loading microphones…".into())));

pub(super) fn start_inputs() {
    std::thread::spawn(|| {
        loop {
            let devices = crate::audio::input_devices();
            if let Ok(mut inputs) = INPUTS.write() {
                *inputs = devices;
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dictionary_import_needs_an_absolute_path() {
        let error = import_dictionary(std::path::Path::new("seed.txt")).unwrap_err();
        assert!(error.contains("absolute"));
    }
    #[test]
    fn pending_capture_rejects_model_and_formatting_changes() {
        let _guard = super::super::runtime_test_guard();
        let (engine, received, _) = crate::runtime::EngineHandle::controlled();
        let mut app = super::super::LinuxEngine::new(engine);
        app.status = crate::state::AppStatus::Ready;
        app.command(crate::runtime::EngineCommand::Start, || None);
        assert!(received.try_recv().is_ok());
        app.command(
            crate::runtime::EngineCommand::SwitchModel(crate::model::LocalModel::Nemotron),
            || None,
        );
        app.command(
            crate::runtime::EngineCommand::Formatting(Default::default()),
            || None,
        );
        assert!(received.try_recv().is_err());
        assert!(app.quit_busy());
    }
    #[test]
    fn formatting_patches_preserve_other_controls_and_reject_invalid_values() {
        let current = crate::formatting::Formatting::default();
        let patched = formatting_patch(current, "context email").unwrap();
        assert_eq!(patched.context, crate::formatting::Context::Email);
        assert_eq!(patched.style, current.style);
        assert_eq!(patched.structure, current.structure);
        assert_eq!(patched.enabled, current.enabled);
        assert!(formatting_patch(current, "style invented").is_err());
        assert!(formatting_patch(current, "unknown value").is_err());
    }

    #[test]
    fn every_catalog_model_is_selectable_through_the_agent_command() {
        let _guard = super::super::runtime_test_guard();
        for model in crate::model::LocalModel::ALL {
            let (engine, received, _) = crate::runtime::EngineHandle::controlled();
            let mut app = super::super::LinuxEngine::new(engine);
            app.status = crate::state::AppStatus::Ready;
            assert_eq!(
                app.menu_command(&format!("menu-model {}", model.id()), None)["status"],
                "change_requested"
            );
            assert!(
                matches!(received.try_recv().unwrap(), crate::runtime::EngineCommand::SwitchModel(selected) if selected == model)
            );
        }
    }

    #[test]
    fn model_command_dispatches_valid_choice_and_rejects_busy_or_invalid_requests() {
        let _guard = super::super::runtime_test_guard();
        let (engine, received, _) = crate::runtime::EngineHandle::controlled();
        let mut app = super::super::LinuxEngine::new(engine);
        app.status = crate::state::AppStatus::Ready;
        assert_eq!(app.menu_command("menu-model invented", None)["ok"], false);
        assert!(received.try_recv().is_err());
        assert_eq!(
            app.menu_command("menu-model parakeet_v2", None)["status"],
            "change_requested"
        );
        assert!(matches!(
            received.try_recv().unwrap(),
            crate::runtime::EngineCommand::SwitchModel(crate::model::LocalModel::ParakeetV2)
        ));
        assert!(app.quit_busy());
        assert_eq!(app.menu_command("menu-model compact", None)["ok"], false);
        assert!(received.try_recv().is_err());
    }

    #[test]
    fn formatting_request_blocks_a_stale_second_patch_before_worker_completion() {
        let _guard = super::super::runtime_test_guard();
        let (engine, received, _) = crate::runtime::EngineHandle::controlled();
        let mut app = super::super::LinuxEngine::new(engine);
        app.status = crate::state::AppStatus::Ready;
        app.command(
            crate::runtime::EngineCommand::Formatting(Default::default()),
            || None,
        );
        assert!(matches!(
            received.try_recv().unwrap(),
            crate::runtime::EngineCommand::Formatting(_)
        ));
        assert_eq!(
            app.menu_command("menu-format context email", None)["ok"],
            false
        );
        assert!(received.try_recv().is_err());
    }

    #[test]
    fn menu_errors_offer_a_fix_and_never_claim_success() {
        let value = error("Read-only preferences");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["message"], "Read-only preferences");
        assert!(
            value["error"]["fix"]
                .as_str()
                .unwrap()
                .contains("menu-status")
        );
    }
}
