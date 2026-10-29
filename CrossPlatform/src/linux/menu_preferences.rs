use super::{Settings, TrayAction};
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
                serde_json::json!({"ok":true,"busy":busy,"desired_model":settings.selected_model,"active_engine":ACTIVE.read().map_err(|e| e.to_string())?.clone(),"microphone":settings.microphone,"microphones":input_status(),"recording_mode":settings.recording_mode,"start_at_login":settings.start_at_login,"login_registered":login_registered(),"formatting":settings.formatting,"version":env!("CARGO_PKG_VERSION")}),
            );
        }
        if busy {
            return Err("Finish recording or processing before changing preferences".into());
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
                    return Err("Microphone unavailable; refresh Settings > Audio".into());
                }
                Some(id.into())
            };
            Settings::update(|settings| settings.microphone = microphone)?;
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

pub(super) fn attach(application: &gtk::Application, sender: mpsc::Sender<TrayAction>) {
    for (name, target) in [
        ("menu-model", "models"),
        ("menu-format", "formatting"),
        ("menu-about", "about"),
    ] {
        let action = gtk::gio::SimpleAction::new_stateful(
            name,
            Some(gtk::glib::VariantTy::STRING),
            &"".to_variant(),
        );
        let action = if name == "menu-about" {
            gtk::gio::SimpleAction::new(name, None)
        } else {
            action
        };
        let sender = sender.clone();
        action.connect_activate(move |_, value| {
            let sender = sender.clone();
            if target == "models" {
                let selection = value.and_then(|value| value.str()).map(str::to_owned);
                std::thread::spawn(move || {
                    let opened = request(&sender, "menu-open models".into());
                    if opened["ok"] == true
                        && let Some(selection) = selection
                    {
                        let _ = request(&sender, format!("menu-model {selection}"));
                    }
                });
                return;
            }
            let command = if target == "formatting" {
                format!(
                    "menu-format {}",
                    value.and_then(|v| v.str()).unwrap_or("raw")
                )
            } else {
                format!("menu-open {target}")
            };
            std::thread::spawn(move || {
                let _ = request(&sender, command);
            });
        });
        application.add_action(&action);
    }
    let application = application.clone();
    let mut actions = Vec::new();
    for (name, state) in [
        ("menu-mode", "toggle".to_variant()),
        ("menu-login", false.to_variant()),
    ] {
        let parameter = if name == "menu-mode" {
            Some(gtk::glib::VariantTy::STRING)
        } else {
            None
        };
        let action = gtk::gio::SimpleAction::new_stateful(name, parameter, &state);
        let sender = sender.clone();
        action.connect_activate(move |action, value| {
            let command = if action.name() == "menu-mode" {
                format!("menu-mode {}", value.and_then(|v| v.str()).unwrap_or(""))
            } else {
                format!(
                    "menu-login {}",
                    if action
                        .state()
                        .and_then(|v| v.get::<bool>())
                        .unwrap_or(false)
                    {
                        "off"
                    } else {
                        "on"
                    }
                )
            };
            let sender = sender.clone();
            std::thread::spawn(move || {
                let _ = request(&sender, command);
            });
        });
        application.add_action(&action);
        actions.push(action);
    }
    let microphone = gtk::gio::SimpleAction::new_stateful(
        "menu-microphone",
        Some(gtk::glib::VariantTy::STRING),
        &"default".to_variant(),
    );
    microphone.connect_activate(move |_, value| {
        if let Some(value) = value.and_then(|v| v.str()) {
            let sender = sender.clone();
            let command = format!("menu-microphone {value}");
            std::thread::spawn(move || {
                let _ = request(&sender, command);
            });
        }
    });
    application.add_action(&microphone);
    actions.push(microphone);
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        if let Ok(settings) = Settings::load() {
            for (name, value) in [
                (
                    "menu-model",
                    serde_json::to_value(settings.selected_model)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string(),
                ),
                (
                    "menu-format",
                    if settings.formatting.enabled {
                        "s1-mini"
                    } else {
                        "raw"
                    }
                    .to_string(),
                ),
            ] {
                if let Some(action) = application
                    .lookup_action(name)
                    .and_then(|a| a.downcast::<gtk::gio::SimpleAction>().ok())
                {
                    action.set_state(&value.to_variant());
                    action
                        .set_enabled(!super::QUIT_BUSY.load(std::sync::atomic::Ordering::Acquire));
                }
            }

            actions[0].set_state(
                &serde_json::to_value(settings.recording_mode)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_variant(),
            );
            actions[1].set_state(&login_registered().to_variant());
            actions[2].set_state(
                &settings
                    .microphone
                    .as_deref()
                    .unwrap_or("default")
                    .to_variant(),
            );
        }
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
        for action in &actions {
            action.set_enabled(!super::QUIT_BUSY.load(std::sync::atomic::Ordering::Acquire));
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

pub(super) fn tray(sender: &mpsc::Sender<TrayAction>) -> Vec<ksni::MenuItem<super::LinuxTray>> {
    use ksni::menu::{CheckmarkItem, RadioGroup, RadioItem, StandardItem, SubMenu};
    let Ok(settings) = Settings::load() else {
        return vec![
            StandardItem {
                label: "Preferences unavailable".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
        ];
    };
    let enabled = !super::QUIT_BUSY.load(std::sync::atomic::Ordering::Acquire);
    let sender = sender.clone();
    let send = move |command: String| {
        let sender = sender.clone();
        std::thread::spawn(move || {
            let _ = request(&sender, command);
        });
    };
    let mode = RadioGroup {
        selected: usize::from(settings.recording_mode == crate::settings::RecordingMode::Hold),
        options: ["Toggle", "Hold"]
            .into_iter()
            .map(|label| RadioItem {
                label: label.into(),
                enabled,
                ..Default::default()
            })
            .collect(),
        select: Box::new(move |_: &mut super::LinuxTray, index| {
            send(format!(
                "menu-mode {}",
                if index == 0 { "toggle" } else { "hold" }
            ))
        }),
    };
    let mut ids = vec![None];
    let mut labels = vec!["System default".to_string()];
    if let Ok(inputs) = INPUTS.read() {
        match &*inputs {
            Ok(inputs) => {
                for input in inputs {
                    ids.push(Some(input.id.clone()));
                    labels.push(input.label.clone());
                }
            }
            Err(message) => labels[0] = format!("System default · {message}"),
        }
    }
    if !ids.contains(&settings.microphone) {
        ids.push(settings.microphone.clone());
        labels.push("Saved microphone unavailable".into());
    }
    let selected = ids
        .iter()
        .position(|id| id == &settings.microphone)
        .unwrap_or(0);
    let microphone = RadioGroup {
        selected,
        options: labels
            .into_iter()
            .map(|label| RadioItem {
                label,
                enabled,
                ..Default::default()
            })
            .collect(),
        select: Box::new(move |tray: &mut super::LinuxTray, index| {
            if let Some(id) = ids.get(index) {
                let sender = tray.action_sender.clone();
                let command = format!("menu-microphone {}", id.as_deref().unwrap_or("default"));
                std::thread::spawn(move || {
                    let _ = request(&sender, command);
                });
            }
        }),
    };
    let model = RadioGroup {
        selected: match settings.selected_model {
            crate::model::LocalModel::Compact => 0,
            crate::model::LocalModel::ParakeetV2 => 1,
            crate::model::LocalModel::ParakeetV3 => 2,
            crate::model::LocalModel::Nemotron => 3,
        },
        options: crate::model::LocalModel::ALL
            .into_iter()
            .map(|m| RadioItem {
                label: m.label().into(),
                enabled,
                ..Default::default()
            })
            .collect(),
        select: Box::new(|tray: &mut super::LinuxTray, index| {
            let sender = tray.action_sender.clone();
            std::thread::spawn(move || {
                let opened = request(&sender, "menu-open models".into());
                if opened["ok"] == true
                    && let Some(selection) = crate::model::LocalModel::ALL.get(index)
                {
                    let _ = request(&sender, format!("menu-model {}", selection.id()));
                }
            });
        }),
    };
    let formatting = RadioGroup {
        selected: usize::from(settings.formatting.enabled),
        options: ["Raw", "S1-mini"]
            .into_iter()
            .map(|label| RadioItem {
                label: label.into(),
                enabled,
                ..Default::default()
            })
            .collect(),
        select: Box::new(|tray: &mut super::LinuxTray, index| {
            let sender = tray.action_sender.clone();
            std::thread::spawn(move || {
                let _ = request(
                    &sender,
                    format!("menu-format {}", if index == 0 { "raw" } else { "s1-mini" }),
                );
            });
        }),
    };
    let ask_items = [
        ("Ask", super::TrayAction::Ask),
        ("Cancel Ask", super::TrayAction::CancelAsk),
        ("Ask settings…", super::TrayAction::OpenSettings),
    ]
    .into_iter()
    .map(|(label, action)| {
        StandardItem {
            label: if matches!(action, super::TrayAction::Ask) {
                ask_state().0.into()
            } else {
                label.into()
            },
            enabled: if matches!(action, super::TrayAction::CancelAsk) {
                ask_state().1
            } else {
                true
            },
            activate: Box::new(move |tray: &mut super::LinuxTray| {
                let _ = tray.action_sender.send(action.clone());
            }),
            ..Default::default()
        }
        .into()
    })
    .collect();
    vec![
        SubMenu {
            label: "Ask".into(),
            submenu: ask_items,
            ..Default::default()
        }
        .into(),
        SubMenu {
            label: format!("Model: {}…", active_label()),
            submenu: vec![model.into()],
            ..Default::default()
        }
        .into(),
        SubMenu {
            label: "Formatting…".into(),
            submenu: vec![
                formatting.into(),
                StandardItem {
                    label: "Formatting controls…".into(),
                    activate: Box::new(|tray: &mut super::LinuxTray| {
                        let sender = tray.action_sender.clone();
                        std::thread::spawn(move || {
                            let _ = request(&sender, "menu-open formatting".into());
                        });
                    }),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into(),
        StandardItem {
            label: format!("About Koett {}", env!("CARGO_PKG_VERSION")),
            activate: Box::new(|tray: &mut super::LinuxTray| {
                let sender = tray.action_sender.clone();
                std::thread::spawn(move || {
                    let _ = request(&sender, "menu-open about".into());
                });
            }),
            ..Default::default()
        }
        .into(),
        SubMenu {
            label: "Recording mode".into(),
            submenu: vec![mode.into()],
            ..Default::default()
        }
        .into(),
        SubMenu {
            label: "Microphone".into(),
            submenu: vec![microphone.into()],
            ..Default::default()
        }
        .into(),
        CheckmarkItem {
            label: "Start at login".into(),
            checked: login_registered(),
            enabled,
            activate: Box::new(move |tray: &mut super::LinuxTray| {
                let sender = tray.action_sender.clone();
                let command = format!(
                    "menu-login {}",
                    if login_registered() { "off" } else { "on" }
                );
                std::thread::spawn(move || {
                    let _ = request(&sender, command);
                });
            }),
            ..Default::default()
        }
        .into(),
    ]
}

pub(super) fn active_label() -> String {
    ACTIVE
        .read()
        .map(|active| {
            {
                if active.contains("nemotron") {
                    "Nemotron 560 ms (Test)"
                } else if active.contains("600m") {
                    "Parakeet v2"
                } else if active.contains("110m") {
                    "Parakeet 110M"
                } else if active.is_empty() {
                    "Loading…"
                } else {
                    "Custom model"
                }
            }
            .to_string()
        })
        .unwrap_or_else(|_| "Unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_capture_rejects_model_and_formatting_changes() {
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
