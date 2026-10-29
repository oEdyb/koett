use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use ashpd::desktop::clipboard::{Clipboard, SetSelectionOptions};
use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, GlobalShortcuts, ListShortcutsOptions, NewShortcut,
};
use ashpd::desktop::remote_desktop::{
    DeviceType, KeyState, NotifyKeyboardKeysymOptions, RemoteDesktop, SelectDevicesOptions,
};
use ashpd::desktop::{CreateSessionOptions, PersistMode, Session};
use futures_util::StreamExt;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use ksni::blocking::TrayMethods as _;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt as _};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use crate::history;
use crate::model::ModelProgress;
use crate::runtime::{self, EngineCommand, EngineHandle, EngineUpdate};
use crate::settings::{PasteShortcut, Settings};
use crate::shortcut::Shortcut;
use crate::state::{AppStatus, RecordingIntent, ShortcutLatch};

static QUIT_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

static SHORTCUT_EDITING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SHORTCUT_CAPTURE_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
static SHORTCUT_CAPTURE: std::sync::Mutex<Option<mpsc::SyncSender<String>>> =
    std::sync::Mutex::new(None);

fn begin_shortcut_capture() -> Result<(u64, Receiver<String>), String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    *SHORTCUT_CAPTURE.lock().map_err(|e| e.to_string())? = Some(sender);
    let generation =
        SHORTCUT_CAPTURE_GENERATION.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
    SHORTCUT_EDITING.store(true, std::sync::atomic::Ordering::Release);
    Ok((generation, receiver))
}

fn end_shortcut_capture() {
    SHORTCUT_CAPTURE_GENERATION.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    SHORTCUT_EDITING.store(false, std::sync::atomic::Ordering::Release);
    if let Ok(mut capture) = SHORTCUT_CAPTURE.lock() {
        capture.take();
    }
}

type ShortcutEdit = tokio::sync::oneshot::Sender<Result<(), String>>;
static SHORTCUT_COMMANDS: std::sync::Mutex<
    Option<tokio::sync::mpsc::UnboundedSender<ShortcutEdit>>,
> = std::sync::Mutex::new(None);

static SHORTCUT_STATUS: std::sync::LazyLock<RwLock<serde_json::Value>> =
    std::sync::LazyLock::new(|| {
        RwLock::new(serde_json::json!({"ok":true,"status":"starting","active":[]}))
    });
fn shortcut_status(value: serde_json::Value) {
    if let Ok(mut status) = SHORTCUT_STATUS.write() {
        *status = value;
    }
}

const APP_ID: &str = "com.olledyberg.Koett";
const SHORTCUT_ID: &str = "toggle-recording";

#[derive(Clone, Debug, PartialEq, Eq)]
enum TrayAction {
    Menu(String, menu_preferences::Reply),
    Ask,
    CancelAsk,
    AskStart,
    AskStop,
    OpenMedia,
    Media(String),
    CancelMedia,
    SwitchModel(crate::model::LocalModel),
    Formatting(crate::formatting::Formatting),
    Toggle,
    Start,
    Stop,
    OpenTranscripts,
    OpenSettings,
    Quit,
    QuitAnyway,
    Recovery(String),
}

enum TrayUpdate {
    PrepareAsk(Sender<()>),
    Status(String),
    Completed(String),
    DeliveryFailed,
    Levels(crate::audio::AudioLevels),
    Shortcut(String),
}

#[derive(Clone)]
struct TrayUpdates {
    sender: Sender<TrayUpdate>,
}

#[derive(Debug)]
struct LinuxTray {
    action_sender: Sender<TrayAction>,
    status: String,
}

impl LinuxTray {
    fn update_status(&mut self, status: String) {
        if !menu::is_control_message(&status) {
            self.status = status;
        }
    }
}

impl ksni::Tray for LinuxTray {
    fn id(&self) -> String {
        "koett".to_string()
    }

    fn title(&self) -> String {
        "Koett".to_string()
    }

    fn icon_name(&self) -> String {
        if self.status.starts_with("Recording") {
            "media-record-symbolic"
        } else if self.status.starts_with("Error") {
            "dialog-error-symbolic"
        } else {
            "audio-input-microphone-symbolic"
        }
        .to_string()
    }

    fn status(&self) -> ksni::Status {
        if self.status.starts_with("Error") {
            ksni::Status::NeedsAttention
        } else {
            ksni::Status::Active
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: self.icon_name(),
            title: "Koett".to_string(),
            description: self.status.clone(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.action_sender.send(TrayAction::OpenTranscripts);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};

        let recording = menu::recording_action(&self.status);

        let mut items = vec![
            StandardItem {
                label: self.status.clone(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: recording.label.to_string(),
                enabled: recording.enabled
                    && !(self.status == "Ready"
                        && QUIT_BUSY.load(std::sync::atomic::Ordering::Acquire)),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::Toggle);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Transcribe media…".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::OpenMedia);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "History".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::OpenTranscripts);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Copy last transcript".into(),
                enabled: history::recovery::last_available(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .action_sender
                        .send(TrayAction::Recovery("copy-last".into()));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Paste last transcript".into(),
                enabled: history::recovery::last_available(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .action_sender
                        .send(TrayAction::Recovery("paste-last".into()));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Copy unsaved transcripts".into(),
                enabled: true,
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .action_sender
                        .send(TrayAction::Recovery("copy-unsaved".into()));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Retry Save".into(),
                enabled: true,
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .action_sender
                        .send(TrayAction::Recovery("retry-unsaved".into()));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Media transcripts…".into(),
                enabled: true,
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .action_sender
                        .send(TrayAction::Recovery("media-transcripts".into()));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Failed transcripts…".into(),
                enabled: true,
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .action_sender
                        .send(TrayAction::Recovery("failed-transcripts".into()));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Failed recordings…".into(),
                enabled: true,
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .action_sender
                        .send(TrayAction::Recovery("failed-recordings".into()));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Recovery status".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .action_sender
                        .send(TrayAction::Recovery("recovery-status".into()));
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit Koett".to_string(),
                icon_name: "application-exit-symbolic".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ];
        let recovery = items.drain(8..14).collect();
        items.insert(
            7,
            ksni::menu::SubMenu {
                label: "Recovery".into(),
                submenu: recovery,
                ..Default::default()
            }
            .into(),
        );
        items.splice(3..3, menu_preferences::tray(&self.action_sender));
        items
    }
}

mod ask_settings;
mod ask_view;
mod control;
mod feedback;
mod formatting_view;
mod history_view;
mod input_preview;
pub(crate) mod media;
mod media_view;
mod menu;
mod menu_preferences;
mod models_view;
mod output;
mod recording_overlay;
mod recovery;
mod settings_view;
mod theme;
mod window;

pub fn run() -> Result<(), String> {
    if let Some(command) = std::env::args().nth(1).filter(|c| c.starts_with("menu-")) {
        if command == "menu-help" {
            println!(
                "{}",
                serde_json::json!({"ok":true,"commands":["menu-status","menu-model compact|parakeet_v2|nemotron","menu-mode toggle|hold","menu-microphone default|CPAL_ID","menu-login on|off", "menu-format raw|s1-mini", "menu-format style casual|semi-casual|semi-formal|formal", "menu-format structure prose|lists", "menu-format context general|email", "menu-open models|formatting|about|settings"]})
            );
        } else {
            let command = std::iter::once(command)
                .chain(std::env::args().skip(2))
                .collect::<Vec<_>>()
                .join(" ");
            let response = control::request(&command)?;
            println!("{response}");
            if serde_json::from_str::<serde_json::Value>(&response).map_err(|e| e.to_string())?["ok"]
                == false
            {
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("shortcuts-help") {
        println!(
            "{}",
            serde_json::json!({"ok":true,"commands":["shortcuts-status","shortcuts-edit","settings"],"editing":"X11: Settings > Shortcuts. Wayland: shortcuts-edit opens the live desktop session."})
        );
        return Ok(());
    }
    if let Some(command) = std::env::args()
        .nth(1)
        .filter(|c| matches!(c.as_str(), "shortcuts-status" | "shortcuts-edit"))
    {
        println!("{}", control::request(&command)?);
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("recovery-help") {
        println!(
            "{}",
            serde_json::json!({"commands":["copy-last","paste-last","copy-unsaved","retry-unsaved","recovery-status","media-transcripts","failed-transcripts","failed-recordings","quit"],"paste_last":"Copies to clipboard when external focus cannot be proven"})
        );
        return Ok(());
    }
    if std::env::args().nth(1).is_some_and(|c| {
        matches!(
            c.as_str(),
            "copy-last"
                | "paste-last"
                | "copy-unsaved"
                | "retry-unsaved"
                | "recovery-status"
                | "media-transcripts"
                | "failed-transcripts"
                | "failed-recordings"
                | "quit"
        )
    }) {
        let response = control::request(&std::env::args().nth(1).unwrap())?;
        println!("{response}");
        let value: serde_json::Value =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        if value["ok"] == false {
            std::process::exit(1);
        }
        return Ok(());
    }
    let mut command = std::env::args().nth(1).unwrap_or_else(|| "show".into());
    if command == "ask-status" {
        println!("{}", control::ask_status()?);
        return Ok(());
    }
    if command == "ask-help" {
        println!(
            "{}",
            serde_json::json!({"commands": ["ask", "ask-start", "ask-stop", "ask-cancel", "ask-status", "ask-configure < configuration.json", "ask-key PROVIDER < secret", "ask-key cartesia < secret", "ask-speech-configure < configuration.json", "ask-speech-status"], "desktop_shortcut": "Ctrl+Alt+Space; portal-managed on Wayland", "configuration": {"enabled": false, "provider": "groq|openRouter|custom", "model": "provider model identifier", "endpoint": "HTTPS chat completions URL"}})
        );
        return Ok(());
    }
    if command == "ask-speech-configure" {
        use std::io::Read;
        let mut input = String::new();
        std::io::stdin()
            .take(4097)
            .read_to_string(&mut input)
            .map_err(|_| "Cannot read speech configuration")?;
        if input.len() > 4096 {
            return Err("Speech configuration exceeds limit".into());
        }
        let config: crate::assistant::speech::Configuration =
            serde_json::from_str(&input).map_err(|_| "Invalid speech configuration")?;
        config.save()?;
        println!("{{\"ok\":true}}");
        return Ok(());
    }
    if command == "ask-speech-status" {
        println!(
            "{}",
            serde_json::json!({"ok":true,"configuration":crate::assistant::speech::Configuration::load()?,"voices":crate::assistant::speech::VOICES.map(|v|v.0)})
        );
        return Ok(());
    }
    if command == "ask-configure" {
        use std::io::Read;
        let mut input = String::new();
        std::io::stdin()
            .take(16385)
            .read_to_string(&mut input)
            .map_err(|_| "Cannot read configuration")?;
        if input.len() > 16384 {
            return Err("Configuration exceeds limit".into());
        }
        let configuration: crate::assistant::Configuration =
            serde_json::from_str(&input).map_err(|_| "Invalid Ask configuration")?;
        configuration.save()?;
        println!("{{\"ok\":true}}");
        return Ok(());
    }
    if command == "ask-key" {
        use std::io::Read;
        let provider = std::env::args()
            .nth(2)
            .ok_or("Usage: koett ask-key PROVIDER < secret")?;
        let mut key = String::new();
        std::io::stdin()
            .take(8193)
            .read_to_string(&mut key)
            .map_err(|_| "Cannot read key")?;
        crate::assistant::secret(
            &provider,
            Some(key.trim()),
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        println!("{{\"ok\":true}}");
        return Ok(());
    }
    if command == "media" {
        let source = std::env::args()
            .nth(2)
            .ok_or("Usage: koett media HTTP(S)_URL")?;
        if std::env::args().count() != 3 {
            return Err("Usage: koett media HTTP(S)_URL".into());
        }
        crate::media_transcription::validate_url(&source)?;
        command = format!("media {source}");
    }
    if control::send(&command).is_ok() {
        if command.starts_with("ask") {
            println!("{{\"ok\":true,\"accepted\":true}}");
        }
        return Ok(());
    }
    if !matches!(command.as_str(), "show" | "--background") {
        return Err("Koett is not running. Launch Koett first".into());
    }
    recording_overlay::prepare()?;
    let instance = single_instance()?;
    let settings = Settings::load()?;
    if let Err(error) = set_start_at_login(settings.start_at_login) {
        eprintln!("Launch at login: {error}");
    }
    let (actions, receiver) = mpsc::channel();
    let (ui, updates) = mpsc::channel();
    control::listen(actions.clone(), ui.clone())?;
    menu_preferences::start_inputs();
    let startup_ui = ui.clone();
    std::thread::spawn(move || match history::recovery::initialize_last() {
        Ok(()) => {
            if let Some(text) = history::recovery::last() {
                let _ = startup_ui.send(window::Update::Completed(text));
            }
        }
        Err(error) => {
            let _ = startup_ui.send(window::Update::Status(format!("@notice:{error}")));
        }
    });
    let worker_ui = ui.clone();
    let worker_actions = actions.clone();
    std::thread::spawn(move || {
        let _instance = instance;
        let tray = LinuxTray {
            action_sender: worker_actions,
            status: "Starting…".into(),
        }
        .spawn()
        .ok();
        let (sender, messages) = mpsc::channel();
        let tray_updates = TrayUpdates { sender };
        std::thread::spawn(move || {
            loop {
                let update = match messages.recv_timeout(Duration::from_millis(500)) {
                    Ok(update) => update,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(tray) = &tray {
                            let _ = tray.update(|_| {});
                        }
                        continue;
                    }
                    Err(_) => break,
                };
                match update {
                    TrayUpdate::Levels(levels) => {
                        let _ = worker_ui.send(window::Update::Levels(levels));
                    }
                    TrayUpdate::DeliveryFailed => {
                        let _ = worker_ui.send(window::Update::DeliveryFailed);
                    }
                    TrayUpdate::Completed(text) => {
                        let _ = worker_ui.send(window::Update::Completed(text));
                    }
                    TrayUpdate::PrepareAsk(ack) => {
                        let _ = worker_ui.send(window::Update::PrepareAsk(ack));
                    }
                    TrayUpdate::Status(status) => {
                        let _ = worker_ui.send(window::Update::Status(status.clone()));
                        if let Some(tray) = &tray {
                            let _ = tray.update(|t| t.update_status(status));
                        }
                    }
                    TrayUpdate::Shortcut(shortcut) => {
                        let _ = worker_ui.send(window::Update::Shortcut(shortcut.clone()));
                    }
                }
            }
        });
        let result = if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("x11") {
            run_x11(settings, receiver, Some(&tray_updates))
        } else {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())
                .and_then(|rt| rt.block_on(run_wayland(settings, receiver, Some(&tray_updates))))
        };
        if let Err(error) = result {
            let _ = ui.send(window::Update::Status(format!("Error: {error}")));
        }
    });
    window::run(actions, updates, command != "--background");
    Ok(())
}

pub fn show_fatal_error(error: &str) {
    eprintln!("Koett: {error}");
    let message = format!("Koett could not start.\n\n{error}");
    show_desktop_message(&message);
}

fn show_desktop_message(message: &str) {
    let dialogs: [(&str, &[&str]); 3] = [
        ("zenity", &["--error", "--title=Koett", "--text"]),
        ("kdialog", &["--title", "Koett", "--error"]),
        ("notify-send", &["--urgency=critical", "Koett"]),
    ];
    for (program, arguments) in dialogs {
        if Command::new(program)
            .args(arguments)
            .arg(message)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
        {
            break;
        }
    }
}

fn single_instance() -> Result<File, String> {
    let path = crate::paths::settings_file()?.with_file_name("koett.lock");
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;
    file.try_lock()
        .map_err(|_| "Koett is already running".to_string())?;
    Ok(file)
}

fn run_x11(
    mut settings: Settings,
    actions: Receiver<TrayAction>,
    tray: Option<&TrayUpdates>,
) -> Result<(), String> {
    settings.validate_shortcuts()?;
    let hotkeys = GlobalHotKeyManager::new()
        .map_err(|error| format!("could not start X11 shortcuts: {error}"))?;
    let mut held_keys = std::collections::HashSet::new();
    let mut blocked_keys = std::collections::HashSet::new();
    let mut bindings = Vec::new();
    for value in [
        &settings.shortcut,
        &settings.ask_shortcut,
        &settings.media_shortcut,
    ] {
        let key = if value.is_empty() {
            None
        } else {
            Some(HotKey::from_str(value).map_err(|error| error.to_string())?)
        };
        let registered = key.is_some_and(|key| hotkeys.register(key).is_ok());
        if key.is_some() && !registered {
            report_shortcut_failure(tray, "unavailable");
        }
        bindings.push((key, registered, ShortcutLatch::default()));
    }
    let missing: Vec<_> = ["dictation", "ask", "media"]
        .into_iter()
        .zip(&bindings)
        .filter(|(_, (key, registered, _))| key.is_some() && !registered)
        .map(|(name, _)| name)
        .collect();
    shortcut_status(
        serde_json::json!({"ok":missing.is_empty(),"backend":"x11","active":bindings.iter().map(|(key, registered, _)| if *registered { key.map(|k| k.to_string()) } else { None }).collect::<Vec<_>>(),"desired":[settings.shortcut,settings.ask_shortcut,settings.media_shortcut],"missing":missing,"error":if missing.is_empty() { serde_json::Value::Null } else { serde_json::json!({"code":"shortcut_registration_failed","message":"Some shortcuts are unavailable. Choose another key in Settings."}) }}),
    );
    let mut output = X11Output::new()?;
    let engine = runtime::start(settings.clone());
    let mut app = LinuxEngine::new(engine);
    let mut last_settings_check = Instant::now();
    loop {
        while let Ok(action) = actions.try_recv() {
            match action {
                TrayAction::Menu(command, reply) => {
                    let value = app.menu_command(&command, tray);
                    let _ = reply.0.send(value);
                }
                TrayAction::AskStart => app.command(EngineCommand::AskStart, || None),
                TrayAction::AskStop => app.command(EngineCommand::AskStop, || None),
                TrayAction::Ask => app.command(EngineCommand::Ask, || None),
                TrayAction::CancelAsk => app.command(EngineCommand::CancelAsk, || None),
                TrayAction::OpenMedia => update_tray_status(tray, "@media"),
                TrayAction::Media(source) => app.command(EngineCommand::Media(source), || None),
                TrayAction::CancelMedia => {
                    let _ = app.engine.commands.send(EngineCommand::CancelMedia);
                }
                TrayAction::Formatting(value) => {
                    app.command(EngineCommand::Formatting(value), || None)
                }
                TrayAction::SwitchModel(selection) => {
                    app.command(EngineCommand::SwitchModel(selection), || None)
                }
                TrayAction::Toggle => app.toggle(|| output.focused_window()),
                TrayAction::Start => app.command(EngineCommand::Start, || output.focused_window()),
                TrayAction::Stop => app.command(EngineCommand::Stop, || output.focused_window()),
                TrayAction::OpenTranscripts => update_tray_status(tray, "@show"),
                TrayAction::OpenSettings => {
                    update_tray_status(tray, "@settings");
                }
                TrayAction::Recovery(command) => match recovery::operation(&command) {
                    Err(error) => update_tray_status(tray, &format!("@notice:{error}")),
                    Ok(value) if command == "recovery-status" => update_tray_status(
                        tray,
                        &format!(
                            "@notice:Unsaved: {} · Last available: {}",
                            value["unsaved_count"], value["last_available"]
                        ),
                    ),
                    Ok(_) if command == "retry-unsaved" => {
                        update_tray_status(tray, "@refresh");
                        update_tray_status(tray, "@notice:Unsaved transcripts saved");
                    }
                    Ok(_) => {}
                },
                TrayAction::Quit | TrayAction::QuitAnyway => {
                    if app.quit_busy() {
                        update_tray_status(
                            tray,
                            "@notice:Finish recording or processing before quitting",
                        );
                        continue;
                    }
                    if action != TrayAction::QuitAnyway && history::recovery::retry().is_err() {
                        update_tray_status(tray, "@quit-protected");
                        continue;
                    }
                    app.stop();
                    update_tray_status(tray, "@quit");
                    return Ok(());
                }
            }
        }
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            let editing = SHORTCUT_EDITING.load(std::sync::atomic::Ordering::Acquire);
            if event.state == HotKeyState::Pressed {
                held_keys.insert(event.id);
            } else {
                held_keys.remove(&event.id);
            }
            if blocked_keys.contains(&event.id) {
                if event.state == HotKeyState::Released {
                    blocked_keys.remove(&event.id);
                }
                continue;
            }
            for (index, (key, registered, latch)) in bindings.iter_mut().enumerate() {
                if !*registered || !key.is_some_and(|key| key.id() == event.id) {
                    continue;
                }
                if event.state == HotKeyState::Pressed
                    && editing
                    && let Ok(capture) = SHORTCUT_CAPTURE.lock()
                    && let Some(capture) = capture.as_ref()
                {
                    let chord = match index {
                        0 => &settings.shortcut,
                        1 => &settings.ask_shortcut,
                        _ => &settings.media_shortcut,
                    };
                    let _ = capture.try_send(chord.clone());
                }
                let intent =
                    x11_shortcut_intent(latch, settings.recording_mode, event.state, editing);
                if let Some(intent) = intent {
                    match index {
                        0 => app.command(intent.into(), || output.focused_window()),
                        1 => app.command(ask_shortcut_command(intent), || None),
                        _ if event.state == HotKeyState::Pressed => {
                            update_tray_status(tray, "@media")
                        }
                        _ => {}
                    }
                }
            }
        }
        app.poll(
            |model, text, raw, target| {
                let loaded = Settings::load();
                let paste_shortcut = loaded.as_ref().unwrap_or(&settings).paste_shortcut;
                let (final_text, warning) = completed_text(text, loaded, &settings);
                if let Some(warning) = warning {
                    update_tray_status(tray, &format!("@notice:{warning}"));
                }
                cache_completed(tray, &final_text);
                let delivered = output.deliver(&final_text, target, paste_shortcut);
                report_delivery(tray, &delivered);
                let saved = history::append_transcript_with_raw_resilient(
                    model,
                    &final_text,
                    raw.or_else(|| (final_text != text).then_some(text)),
                )
                .and_then(|fallback| {
                    fallback.map_or(Ok(()), |path| {
                        Err(format!(
                            "the transcript was saved to the fallback file {}",
                            path.display()
                        ))
                    })
                });
                update_tray_status(tray, "@refresh");
                delivery_results(delivered, saved)
            },
            |status| update_tray_status(tray, status),
            |levels| {
                if let Some(tray) = tray {
                    let _ = tray.sender.send(TrayUpdate::Levels(levels));
                }
            },
            |ack| {
                if let Some(tray) = tray {
                    let _ = tray.sender.send(TrayUpdate::PrepareAsk(ack));
                } else {
                    let _ = ack.send(());
                }
            },
        );
        if last_settings_check.elapsed() >= Duration::from_millis(500) {
            last_settings_check = Instant::now();
            if let Ok(updated) = Settings::load() {
                settings.recording_mode = updated.recording_mode;
                settings.microphone = updated.microphone;
                settings.replacements = updated.replacements;
                settings.start_at_login = updated.start_at_login;
                settings.pause_media = updated.pause_media;
                settings.paste_shortcut = updated.paste_shortcut;
            }
            if let Ok(updated) = Settings::load()
                && updated.validate_shortcuts().is_ok()
            {
                let next_values = [
                    &updated.shortcut,
                    &updated.ask_shortcut,
                    &updated.media_shortcut,
                ];
                let old_values = [
                    settings.shortcut.clone(),
                    settings.ask_shortcut.clone(),
                    settings.media_shortcut.clone(),
                ];
                if next_values
                    .iter()
                    .zip(&old_values)
                    .any(|(next, old)| *next != old)
                {
                    let desired: Vec<Option<HotKey>> = next_values
                        .iter()
                        .map(|value| {
                            if value.is_empty() {
                                None
                            } else {
                                HotKey::from_str(value).ok()
                            }
                        })
                        .collect();
                    let old: Vec<HotKey> = bindings
                        .iter()
                        .filter_map(|(key, registered, _)| if *registered { *key } else { None })
                        .collect();
                    let mut added = Vec::new();
                    let mut failure = None;
                    for key in desired.iter().flatten().filter(|key| !old.contains(key)) {
                        match hotkeys.register(*key) {
                            Ok(()) => added.push(*key),
                            Err(error) => {
                                failure = Some(error.to_string());
                                break;
                            }
                        }
                    }
                    if let Some(error) = failure {
                        for key in added {
                            let _ = hotkeys.unregister(key);
                        }
                        if let Err(error) = Settings::update(|current| {
                            if current.shortcut == updated.shortcut
                                && current.ask_shortcut == updated.ask_shortcut
                                && current.media_shortcut == updated.media_shortcut
                            {
                                current.shortcut = old_values[0].clone();
                                current.ask_shortcut = old_values[1].clone();
                                current.media_shortcut = old_values[2].clone();
                            }
                        }) {
                            eprintln!("Shortcut rollback failed: {error}");
                        }
                        report_shortcut_failure(
                            tray,
                            &format!("rebind failed: {error}; previous bindings retained"),
                        );
                        shortcut_status(serde_json::json!({"ok":false,"backend":"x11",
                            "active":old_values,"desired":next_values,
                            "error":{"code":"shortcut_rebind_failed","message":format!("Rebind failed: {error}; previous bindings retained"),"fix":"Choose an available shortcut in Settings."}}));
                    } else {
                        blocked_keys.extend(held_keys.iter().copied());
                        for (index, (_, _, latch)) in bindings.iter_mut().enumerate() {
                            if let Some(intent) = latch.release() {
                                match index {
                                    0 => app.command(intent.into(), || output.focused_window()),
                                    1 => app.command(ask_shortcut_command(intent), || None),
                                    _ => (),
                                }
                            }
                        }
                        for key in old.iter().filter(|key| !desired.contains(&Some(**key))) {
                            let _ = hotkeys.unregister(*key);
                        }
                        for (index, key) in desired.into_iter().enumerate() {
                            bindings[index] = (key, key.is_some(), ShortcutLatch::default());
                        }
                        settings.shortcut = updated.shortcut;
                        settings.ask_shortcut = updated.ask_shortcut;
                        settings.media_shortcut = updated.media_shortcut;
                        shortcut_status(
                            serde_json::json!({"ok":true,"backend":"x11","active":[settings.shortcut,settings.ask_shortcut,settings.media_shortcut],"desired":[settings.shortcut,settings.ask_shortcut,settings.media_shortcut]}),
                        );
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(
            if app.status == AppStatus::Recording {
                16
            } else {
                50
            },
        ));
    }
}

fn x11_shortcut_intent(
    latch: &mut ShortcutLatch,
    mode: crate::settings::RecordingMode,
    state: HotKeyState,
    editing: bool,
) -> Option<RecordingIntent> {
    match state {
        HotKeyState::Pressed if !editing => latch.press(mode),
        HotKeyState::Released => latch.release(),
        _ => None,
    }
}

#[cfg(test)]
mod x11_editing_tests {
    use super::*;
    #[test]
    fn editing_blocks_new_presses_but_releases_existing_hold() {
        let mut latch = ShortcutLatch::default();
        let mode = crate::settings::RecordingMode::Hold;
        assert_eq!(
            x11_shortcut_intent(&mut latch, mode, HotKeyState::Pressed, false),
            Some(RecordingIntent::Start)
        );
        assert_eq!(
            x11_shortcut_intent(&mut latch, mode, HotKeyState::Released, true),
            Some(RecordingIntent::Stop)
        );
        assert_eq!(
            x11_shortcut_intent(&mut latch, mode, HotKeyState::Pressed, true),
            None
        );
        assert_eq!(
            x11_shortcut_intent(&mut latch, mode, HotKeyState::Released, true),
            None
        );
        assert_eq!(
            x11_shortcut_intent(&mut latch, mode, HotKeyState::Pressed, false),
            Some(RecordingIntent::Start)
        );
    }
}

fn shortcut_action(intent: RecordingIntent) -> TrayAction {
    match intent {
        RecordingIntent::Start => TrayAction::Start,
        RecordingIntent::Stop => TrayAction::Stop,
        RecordingIntent::Toggle => TrayAction::Toggle,
    }
}

fn ask_shortcut_command(intent: RecordingIntent) -> EngineCommand {
    match intent {
        RecordingIntent::Start => EngineCommand::AskStart,
        RecordingIntent::Stop => EngineCommand::AskStop,
        RecordingIntent::Toggle => EngineCommand::Ask,
    }
}

fn assigned_portal_bindings(
    triggers: &std::collections::BTreeMap<String, String>,
    enabled: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    triggers
        .iter()
        .filter(|(id, trigger)| enabled.contains(*id) && !trigger.trim().is_empty())
        .map(|(id, _)| id.clone())
        .collect()
}

#[cfg(test)]
mod portal_binding_tests {
    use super::*;

    #[test]
    fn returned_ids_without_assigned_keys_are_not_active() {
        let enabled = ["dictation", "ask", "media"].map(String::from).into();
        let mut triggers = [
            ("dictation".into(), "Ctrl+Shift+Space".into()),
            ("ask".into(), String::new()),
            ("media".into(), "  ".into()),
            ("disabled".into(), "Ctrl+Alt+X".into()),
        ]
        .into();
        assert_eq!(
            assigned_portal_bindings(&triggers, &enabled),
            ["dictation".into()].into()
        );
        triggers.insert("ask".into(), "Ctrl+Alt+Space".into());
        assert_eq!(
            assigned_portal_bindings(&triggers, &enabled),
            ["dictation".into(), "ask".into()].into()
        );
        triggers.insert("dictation".into(), String::new());
        assert_eq!(
            assigned_portal_bindings(&triggers, &enabled),
            ["ask".into()].into()
        );
    }
}

async fn shortcut_events(
    settings: Settings,
    actions: Sender<TrayAction>,
    updates: Option<TrayUpdates>,
) -> Result<(), String> {
    settings.validate_shortcuts()?;
    let connection = ashpd::zbus::Connection::session()
        .await
        .map_err(|error| format!("could not connect to the desktop portal: {error}"))?;
    let app_id = APP_ID
        .parse()
        .map_err(|error| format!("invalid Koett application ID: {error}"))?;
    ashpd::register_host_app_with_connection(connection.clone(), app_id)
        .await
        .map_err(|error| format!("could not register Koett with the desktop portal: {error}"))?;

    let shortcuts = GlobalShortcuts::with_connection(connection.clone())
        .await
        .map_err(|error| format!("global shortcuts are not available: {error}"))?;
    let shortcut_session = shortcuts
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|error| format!("could not create a shortcut session: {error}"))?;
    let listed = shortcuts
        .list_shortcuts(&shortcut_session, ListShortcutsOptions::default())
        .await
        .and_then(|request| request.response())
        .map_err(|error| format!("could not read the desktop shortcuts: {error}"))?;
    let mut triggers: std::collections::BTreeMap<String, String> = listed
        .shortcuts()
        .iter()
        .map(|s| (s.id().to_string(), s.trigger_description().to_string()))
        .collect();
    let mut requested = Vec::new();
    for (id, label, value) in [
        (SHORTCUT_ID, "Dictation", &settings.shortcut),
        ("ask", "Ask", &settings.ask_shortcut),
        ("media", "Transcribe media", &settings.media_shortcut),
    ] {
        if !value.is_empty() {
            requested.push(
                NewShortcut::new(id, label)
                    .preferred_trigger(Shortcut::from_str(value)?.portal_trigger().as_str()),
            );
        }
    }
    if !requested.is_empty() {
        let bound = shortcuts
            .bind_shortcuts(
                &shortcut_session,
                &requested,
                None,
                BindShortcutsOptions::default(),
            )
            .await
            .and_then(|request| request.response())
            .map_err(|error| format!("could not bind shortcuts: {error}"))?;
        triggers.extend(
            bound
                .shortcuts()
                .iter()
                .map(|s| (s.id().to_string(), s.trigger_description().to_string())),
        );
    }
    let enabled: std::collections::HashSet<String> = [
        (SHORTCUT_ID, &settings.shortcut),
        ("ask", &settings.ask_shortcut),
        ("media", &settings.media_shortcut),
    ]
    .into_iter()
    .filter(|(_, value)| !value.is_empty())
    .map(|(id, _)| id.to_string())
    .collect();
    let mut active = assigned_portal_bindings(&triggers, &enabled);
    let missing: Vec<_> = enabled.difference(&active).cloned().collect();
    if !missing.is_empty() {
        report_shortcut_failure(
            updates.as_ref(),
            &format!("missing bindings: {}", missing.join(", ")),
        );
    }
    shortcut_status(
        serde_json::json!({"ok":missing.is_empty(),"backend":"portal","active":active,"triggers":triggers,"missing":missing,"editing_supported":shortcuts.version() >= 2}),
    );
    let session_path = serde_json::to_value(&shortcut_session).map_err(|e| e.to_string())?;
    let mut closed = shortcut_session
        .receive_closed()
        .await
        .map_err(|e| e.to_string())?;
    let (commands, mut edits) = tokio::sync::mpsc::unbounded_channel::<ShortcutEdit>();
    *SHORTCUT_COMMANDS.lock().map_err(|e| e.to_string())? = Some(commands);
    let mut changes = shortcuts
        .receive_shortcuts_changed()
        .await
        .map_err(|error| error.to_string())?;
    let mut presses = shortcuts
        .receive_activated()
        .await
        .map_err(|e| e.to_string())?;
    let mut releases = shortcuts
        .receive_deactivated()
        .await
        .map_err(|e| e.to_string())?;
    let mut latch = ShortcutLatch::default();
    let mut ask_latch = ShortcutLatch::default();
    let mut media_pressed = false;
    loop {
        tokio::select! {
            _ = closed.next() => break,
            Some(reply) = edits.recv() => {
                let result = if shortcuts.version() < 2 {
                    Err("Desktop portal version 1 has no ConfigureShortcuts. Use desktop shortcut settings.".to_string())
                } else {
                    shortcuts.configure_shortcuts(&shortcut_session, None, Default::default()).await.map_err(|e| e.to_string())
                };
                let _ = reply.send(result);
            }
            event = presses.next() => {
                let Some(event) = event else { break; };
                if serde_json::to_value(event.session_handle()).ok().as_ref() != Some(&session_path) || !active.contains(event.shortcut_id()) { continue; }
                if event.shortcut_id() == "media" && !media_pressed { media_pressed = true; let _ = actions.send(TrayAction::OpenMedia); }
                if event.shortcut_id() == "ask" {
                    let mode = Settings::load().unwrap_or_else(|_| settings.clone()).recording_mode;
                    if let Some(intent) = ask_latch.press(mode) {
                        let action = match intent { RecordingIntent::Start => TrayAction::AskStart, RecordingIntent::Stop => TrayAction::AskStop, RecordingIntent::Toggle => TrayAction::Ask };
                        let _ = actions.send(action);
                    }
                }
                if event.shortcut_id() == SHORTCUT_ID {
                    let mode = Settings::load().unwrap_or_else(|_| settings.clone()).recording_mode;
                    if let Some(intent) = latch.press(mode) { let _ = actions.send(shortcut_action(intent)); }
                }
            }
            change = changes.next() => {
                let Some(change) = change else { break; };
                if serde_json::to_value(change.session_handle()).ok().as_ref() != Some(&session_path) { continue; }
                if ask_latch.release().is_some() { let _ = actions.send(TrayAction::AskStop); }
                if let Some(intent) = latch.release() { let _ = actions.send(shortcut_action(intent)); }
                media_pressed = false;
                triggers = change.shortcuts().iter().map(|s| (s.id().to_string(), s.trigger_description().to_string())).collect();
                active = assigned_portal_bindings(&triggers, &enabled);
                let missing: Vec<_> = enabled.difference(&active).cloned().collect();
                shortcut_status(serde_json::json!({"ok":missing.is_empty(),"backend":"portal","active":active,"triggers":triggers,"missing":missing,"editing_supported":shortcuts.version() >= 2}));
                if !missing.is_empty() { report_shortcut_failure(updates.as_ref(), &format!("missing bindings: {}", missing.join(", "))); }
                if let Some(shortcut) = change.shortcuts().iter().find(|shortcut| shortcut.id() == SHORTCUT_ID) {
                    if let Some(intent) = latch.release() { let _ = actions.send(shortcut_action(intent)); }
                    update_shortcut(updates.as_ref(), shortcut.trigger_description());
                }
            }
            event = releases.next() => {
                let Some(event) = event else { break; };
                if serde_json::to_value(event.session_handle()).ok().as_ref() != Some(&session_path) { continue; }
                if event.shortcut_id() == "media" { media_pressed = false; }
                if event.shortcut_id() == "ask" && ask_latch.release().is_some() { let _ = actions.send(TrayAction::AskStop); }
                if event.shortcut_id() == SHORTCUT_ID && let Some(intent) = latch.release() {
                    let _ = actions.send(shortcut_action(intent));
                }
            }
        }
    }
    if let Some(intent) = latch.release() {
        let _ = actions.send(shortcut_action(intent));
    }
    *SHORTCUT_COMMANDS.lock().map_err(|e| e.to_string())? = None;
    if ask_latch.release().is_some() {
        let _ = actions.send(TrayAction::AskStop);
    }
    Err("Shortcut session closed; use Koett start/stop controls".into())
}

async fn run_wayland(
    mut settings: Settings,
    actions: Receiver<TrayAction>,
    tray: Option<&TrayUpdates>,
) -> Result<(), String> {
    let (shortcut_sender, shortcut_receiver) = mpsc::channel();
    let shortcut_settings = settings.clone();
    let shortcut_updates = tray.cloned();
    let mut shortcut_task = Some(tokio::spawn(async move {
        shortcut_events(shortcut_settings, shortcut_sender, shortcut_updates).await
    }));
    let mut output_task = Some(tokio::spawn(async {
        let connection = ashpd::zbus::Connection::session()
            .await
            .map_err(|e| e.to_string())?;
        WaylandOutput::new(connection).await
    }));
    let mut output = None;
    let mut app = LinuxEngine::new(runtime::start(settings.clone()));
    let mut timer = tokio::time::interval(Duration::from_millis(30));
    loop {
        timer.tick().await;
        if shortcut_task
            .as_ref()
            .is_some_and(|task| task.is_finished())
            && let Some(task) = shortcut_task.take()
        {
            let error = match task.await {
                Ok(Err(error)) => error,
                Err(error) => error.to_string(),
                Ok(Ok(())) => "Shortcut session closed".into(),
            };
            update_shortcut(tray, "Compositor shortcut");
            update_tray_status(
                tray,
                "@notice:Global shortcut unavailable. Open Settings to set one up.",
            );
            shortcut_status(
                serde_json::json!({"ok":false,"backend":"portal","active":[],"error":{"code":"shortcut_session_failed","message":error,"fix":"Check desktop portal registration and permission, then restart Koett."}}),
            );
            eprintln!("Global shortcut unavailable: {error}");
        }
        if output_task.as_ref().is_some_and(|task| task.is_finished())
            && let Some(task) = output_task.take()
        {
            match task.await {
                Ok(Ok(ready)) => output = Some(ready),
                Ok(Err(error)) => {
                    eprintln!("Portal paste unavailable: {error}; using clipboard/virtual keyboard")
                }
                Err(error) => eprintln!("Portal task failed: {error}"),
            }
        }
        for action in actions.try_iter().chain(shortcut_receiver.try_iter()) {
            match action {
                TrayAction::Menu(command, reply) => {
                    let value = app.menu_command(&command, tray);
                    let _ = reply.0.send(value);
                }
                TrayAction::AskStart => app.command(EngineCommand::AskStart, || None),
                TrayAction::AskStop => app.command(EngineCommand::AskStop, || None),
                TrayAction::Ask => app.command(EngineCommand::Ask, || None),
                TrayAction::CancelAsk => app.command(EngineCommand::CancelAsk, || None),
                TrayAction::OpenMedia => update_tray_status(tray, "@media"),
                TrayAction::Media(source) => app.command(EngineCommand::Media(source), || None),
                TrayAction::CancelMedia => {
                    let _ = app.engine.commands.send(EngineCommand::CancelMedia);
                }
                TrayAction::Formatting(value) => {
                    app.command(EngineCommand::Formatting(value), || None)
                }
                TrayAction::SwitchModel(selection) => {
                    app.command(EngineCommand::SwitchModel(selection), || None)
                }
                TrayAction::Toggle => app.toggle(output::capture_target),
                TrayAction::Start => app.command(EngineCommand::Start, output::capture_target),
                TrayAction::Stop => app.command(EngineCommand::Stop, output::capture_target),
                TrayAction::OpenTranscripts => update_tray_status(tray, "@show"),
                TrayAction::OpenSettings => update_tray_status(tray, "@settings"),
                TrayAction::Recovery(command) => match recovery::operation(&command) {
                    Err(error) => update_tray_status(tray, &format!("@notice:{error}")),
                    Ok(value) if command == "recovery-status" => update_tray_status(
                        tray,
                        &format!(
                            "@notice:Unsaved: {} · Last available: {}",
                            value["unsaved_count"], value["last_available"]
                        ),
                    ),
                    Ok(_) if command == "retry-unsaved" => {
                        update_tray_status(tray, "@refresh");
                        update_tray_status(tray, "@notice:Unsaved transcripts saved");
                    }
                    Ok(_) => {}
                },
                TrayAction::Quit | TrayAction::QuitAnyway => {
                    if app.quit_busy() {
                        update_tray_status(
                            tray,
                            "@notice:Finish recording or processing before quitting",
                        );
                        continue;
                    }
                    if action != TrayAction::QuitAnyway && history::recovery::retry().is_err() {
                        update_tray_status(tray, "@quit-protected");
                        continue;
                    }
                    if let Some(task) = &shortcut_task {
                        task.abort();
                    }
                    app.stop();
                    update_tray_status(tray, "@quit");
                    return Ok(());
                }
            }
        }
        let mut completed = Vec::new();
        app.poll(
            |model, text, raw, target| {
                completed.push((
                    model.to_owned(),
                    text.to_owned(),
                    raw.map(str::to_owned),
                    target,
                ));
                Ok(())
            },
            |status| update_tray_status(tray, status),
            |levels| {
                if let Some(tray) = tray {
                    let _ = tray.sender.send(TrayUpdate::Levels(levels));
                }
            },
            |ack| {
                if let Some(tray) = tray {
                    let _ = tray.sender.send(TrayUpdate::PrepareAsk(ack));
                } else {
                    let _ = ack.send(());
                }
            },
        );
        for (model, formatted, original, target) in completed {
            let raw = original.as_deref().unwrap_or(&formatted);
            let loaded = Settings::load();
            let (text, warning) = completed_text(&formatted, loaded.clone(), &settings);
            if let Ok(updated) = loaded {
                settings = updated;
            }
            if let Some(warning) = warning {
                update_tray_status(tray, &format!("@notice:{warning}"));
            }
            cache_completed(tray, &text);
            let delivered = match &output {
                Some(output) => output.deliver(&text, target, settings.paste_shortcut).await,
                None => output::deliver(&text, target, settings.paste_shortcut),
            };
            report_delivery(tray, &delivered);
            let saved = history::append_transcript_with_raw_resilient(
                &model,
                &text,
                (text != raw).then_some(raw),
            );
            if let Err(error) = delivered {
                update_tray_status(tray, &format!("@notice:{error}"));
            }
            match saved {
                Err(error) => {
                    update_tray_status(tray, &format!("@notice:{error}. Use Copy last transcript"))
                }
                Ok(Some(path)) => {
                    update_tray_status(tray, &format!("@notice:Saved recovery: {}", path.display()))
                }
                Ok(None) => update_tray_status(tray, "@refresh"),
            }
        }
    }
}

fn capture_requested(status: &AppStatus, command: &EngineCommand) -> bool {
    *status == AppStatus::Ready
        && matches!(
            command,
            EngineCommand::Start
                | EngineCommand::Toggle
                | EngineCommand::Ask
                | EngineCommand::AskStart
        )
}

#[cfg(test)]
mod queued_capture_tests {
    use super::*;

    #[test]
    fn queued_capture_refuses_quit_before_any_worker_status() {
        for command in [EngineCommand::Start, EngineCommand::AskStart] {
            let is_ask = matches!(command, EngineCommand::AskStart);
            let (engine, received, updates) = EngineHandle::controlled();
            let mut app = LinuxEngine::new(engine);
            app.status = AppStatus::Ready;
            app.command(command, || None);
            assert!(app.quit_busy());
            assert!(received.try_recv().is_ok());
            assert!(app.engine.updates.try_recv().is_err());
            updates
                .send(if is_ask {
                    EngineUpdate::Ask(serde_json::json!({"phase":"Cancelled"}))
                } else {
                    EngineUpdate::Status(AppStatus::Ready)
                })
                .unwrap();
            app.poll(|_, _, _, _| Ok(()), |_| {}, |_| {}, |_| {});
            assert!(!app.quit_busy());
        }
    }

    #[test]
    fn capture_is_guarded_before_worker_status_arrives() {
        for command in [
            EngineCommand::Start,
            EngineCommand::Toggle,
            EngineCommand::Ask,
            EngineCommand::AskStart,
        ] {
            assert!(capture_requested(&AppStatus::Ready, &command));
            assert!(!capture_requested(&AppStatus::Recording, &command));
        }
        assert!(!capture_requested(&AppStatus::Ready, &EngineCommand::Stop));
    }
}

struct LinuxEngine {
    engine: EngineHandle,
    status: AppStatus,
    paste_target: Option<u32>,
    configuration_pending: bool,
    capture_pending: bool,
    capture_is_ask: bool,
}

impl LinuxEngine {
    fn new(engine: EngineHandle) -> Self {
        Self {
            engine,
            status: AppStatus::Starting,
            paste_target: None,
            configuration_pending: false,
            capture_pending: false,
            capture_is_ask: false,
        }
    }

    fn menu_command(&mut self, command: &str, tray: Option<&TrayUpdates>) -> serde_json::Value {
        let value = if let Some(selection) = command.strip_prefix("menu-model ") {
            if self.quit_busy() || self.status != AppStatus::Ready {
                menu_preferences::error("Finish recording or processing before changing models")
            } else {
                match serde_json::from_value::<crate::model::LocalModel>(selection.into()) {
                    Ok(selection) => {
                        self.command(EngineCommand::SwitchModel(selection), || None);
                        serde_json::json!({"ok":true,"status":"change_requested","fix":"Use menu-status to confirm completion"})
                    }
                    Err(_) => menu_preferences::error("Choose compact, parakeet_v2 or nemotron"),
                }
            }
        } else if let Some(output) = command.strip_prefix("menu-format ") {
            if self.quit_busy() || self.status != AppStatus::Ready {
                menu_preferences::error("Finish recording or processing before changing formatting")
            } else {
                match Settings::load() {
                    Ok(settings) => {
                        match menu_preferences::formatting_patch(settings.formatting, output) {
                            Ok(formatting) => {
                                self.command(EngineCommand::Formatting(formatting), || None);
                                serde_json::json!({"ok":true,"status":"change_requested","fix":"Use menu-status to confirm completion"})
                            }
                            Err(error) => menu_preferences::error(&error),
                        }
                    }
                    Err(error) => menu_preferences::error(&error),
                }
            }
        } else if let Some(target) = command.strip_prefix("menu-open ") {
            if matches!(target, "models" | "formatting" | "about" | "settings") {
                update_tray_status(tray, &format!("@{target}"));
                serde_json::json!({"ok":true,"status":"window_requested"})
            } else {
                menu_preferences::error("Choose models, formatting, about or settings")
            }
        } else {
            menu_preferences::operation(command, self.quit_busy())
        };
        if value["ok"] == false {
            update_tray_status(
                tray,
                &format!(
                    "@notice:{}",
                    value["error"]["message"]
                        .as_str()
                        .unwrap_or("Preferences unavailable")
                ),
            );
        }
        value
    }

    fn quit_busy(&self) -> bool {
        self.configuration_pending
            || self.capture_pending
            || matches!(
                self.status,
                AppStatus::Starting | AppStatus::Recording | AppStatus::Transcribing
            )
    }

    fn toggle(&mut self, focused_window: impl FnOnce() -> Option<u32>) {
        if self.status != AppStatus::Ready
            && self.status != AppStatus::Recording
            && !matches!(self.status, AppStatus::Error(_))
        {
            return;
        }
        self.command(EngineCommand::Toggle, focused_window);
    }

    fn command(&mut self, command: EngineCommand, focus: impl FnOnce() -> Option<u32>) {
        if matches!(command, EngineCommand::CancelAsk) {
            self.engine.cancel_ask();
            let _ = self.engine.commands.send(command);
            return;
        }
        let is_ask = matches!(
            command,
            EngineCommand::Ask | EngineCommand::AskStart | EngineCommand::AskStop
        );
        let is_dictation = matches!(
            command,
            EngineCommand::Toggle | EngineCommand::Start | EngineCommand::Stop
        );
        if (self.capture_pending || self.status == AppStatus::Recording)
            && (is_ask || is_dictation)
            && is_ask != self.capture_is_ask
        {
            return;
        }
        if matches!(
            command,
            EngineCommand::SwitchModel(_) | EngineCommand::Formatting(_)
        ) && (self.quit_busy() || self.status != AppStatus::Ready)
        {
            return;
        }

        if self.status != AppStatus::Ready
            && self.status != AppStatus::Recording
            && !matches!(self.status, AppStatus::Error(_))
        {
            return;
        }
        if matches!(self.status, AppStatus::Error(_)) && matches!(command, EngineCommand::Stop) {
            return;
        }
        if self.status == AppStatus::Ready {
            self.paste_target = focus();
        }
        if matches!(
            command,
            EngineCommand::SwitchModel(_) | EngineCommand::Formatting(_) | EngineCommand::Media(_)
        ) {
            self.configuration_pending = true;
            QUIT_BUSY.store(true, std::sync::atomic::Ordering::Release);
        }
        if capture_requested(&self.status, &command) {
            self.capture_is_ask = is_ask;
            self.capture_pending = true;
            QUIT_BUSY.store(true, std::sync::atomic::Ordering::Release);
        }
        if self.engine.commands.send(command).is_err() {
            match Settings::load() {
                Ok(settings) => {
                    self.engine = runtime::start(settings);
                    self.status = AppStatus::Starting;
                }
                Err(error) => self.status = AppStatus::Error(error),
            }
        }
    }

    fn stop(self) {
        self.engine.stop();
    }

    fn poll(
        &mut self,
        mut deliver: impl FnMut(&str, &str, Option<&str>, Option<u32>) -> Result<(), String>,
        mut status_changed: impl FnMut(&str),
        mut recording_started: impl FnMut(crate::audio::AudioLevels),
        mut prepare_ask: impl FnMut(Sender<()>),
    ) {
        let mut interaction_error = None;
        while let Ok(update) = self.engine.updates.try_recv() {
            match update {
                EngineUpdate::PrepareAsk(ack) => prepare_ask(ack),
                EngineUpdate::Ask(value) => {
                    if value.get("error").is_some()
                        || value.get("phase").and_then(serde_json::Value::as_str)
                            == Some("Cancelled")
                    {
                        self.capture_pending = false;
                        QUIT_BUSY.store(self.quit_busy(), std::sync::atomic::Ordering::Release);
                    }
                    crate::assistant::update_status(&value);
                    status_changed(&format!("@ask:{}", value));
                }
                EngineUpdate::FormattingBusy(busy) => {
                    self.configuration_pending = busy;
                    QUIT_BUSY.store(self.quit_busy(), std::sync::atomic::Ordering::Release);
                    status_changed(if busy { "@format-busy" } else { "@format-idle" })
                }
                EngineUpdate::ActiveModel(model) => {
                    if let Ok(mut active) = menu_preferences::ACTIVE.write() {
                        *active = model.clone();
                    }
                    status_changed(&format!("@model:{model}"));
                }
                EngineUpdate::Status(status) => {
                    self.configuration_pending = false;
                    self.status = status;
                    self.capture_pending = false;
                    QUIT_BUSY.store(
                        matches!(
                            self.status,
                            AppStatus::Starting | AppStatus::Recording | AppStatus::Transcribing
                        ),
                        std::sync::atomic::Ordering::Release,
                    );
                    status_changed(status_text(&self.status).as_str());
                }
                EngineUpdate::TranscriptReady { model, transcript } => {
                    if let Err(error) = deliver(
                        &model,
                        &transcript.text,
                        transcript.raw.as_deref(),
                        self.paste_target.take(),
                    ) {
                        eprintln!("Koett: {error}");
                        interaction_error = Some(error);
                    }
                }
                EngineUpdate::ModelProgress(ModelProgress::Downloading { received, total }) => {
                    let percent = received.saturating_mul(100) / total.max(1);
                    status_changed(format!("Downloading model… {percent}%").as_str());
                }
                EngineUpdate::ModelProgress(ModelProgress::Repairing) => {
                    status_changed("Repairing model cache…");
                }
                EngineUpdate::ModelProgress(ModelProgress::Installing) => {
                    status_changed("Installing model…");
                }
                EngineUpdate::ModelProgress(ModelProgress::Ready) => {
                    status_changed("Loading model…");
                }
                EngineUpdate::RecordingStarted(levels) => recording_started(levels),
                EngineUpdate::Notice(message) => status_changed(&format!("@notice:{message}")),
            }
        }
        if let Some(error) = interaction_error {
            status_changed(&format!("@notice:{error}. Use Copy last transcript"));
        }
    }
}

fn status_text(status: &AppStatus) -> String {
    match status {
        AppStatus::Starting => "Starting…".to_string(),
        AppStatus::Ready => "Ready".to_string(),
        AppStatus::Recording => "Recording…".to_string(),
        AppStatus::Transcribing => "Transcribing…".to_string(),
        AppStatus::Error(error) => format!("Error: {error}"),
    }
}

fn report_shortcut_failure(tray: Option<&TrayUpdates>, detail: &str) {
    update_tray_status(
        tray,
        &format!("@notice:Global shortcut {detail}. Open Settings to change it"),
    );
}

fn update_tray_status(tray: Option<&TrayUpdates>, status: &str) {
    if let Some(tray) = tray {
        let _ = tray.sender.send(TrayUpdate::Status(status.to_string()));
    }
}

fn update_shortcut(tray: Option<&TrayUpdates>, shortcut: &str) {
    if let Some(tray) = tray {
        let _ = tray.sender.send(TrayUpdate::Shortcut(shortcut.to_string()));
    }
}

struct X11Output {
    clipboard: arboard::Clipboard,
    connection: RustConnection,
}

impl X11Output {
    fn new() -> Result<Self, String> {
        let clipboard = arboard::Clipboard::new()
            .map_err(|error| format!("could not open the X11 clipboard: {error}"))?;
        let (connection, _) =
            x11rb::connect(None).map_err(|error| format!("could not connect to X11: {error}"))?;
        Ok(Self {
            clipboard,
            connection,
        })
    }

    fn focused_window(&self) -> Option<u32> {
        if output::WINDOW_ACTIVE.load(std::sync::atomic::Ordering::Acquire) {
            return None;
        }
        self.connection
            .get_input_focus()
            .ok()?
            .reply()
            .ok()
            .map(|reply| reply.focus)
    }

    fn deliver(
        &mut self,
        text: &str,
        target: Option<u32>,
        shortcut: PasteShortcut,
    ) -> Result<(), String> {
        self.clipboard
            .set_text(text)
            .map_err(|error| format!("could not copy the transcript: {error}"))?;
        let copied = self
            .clipboard
            .get_text()
            .map_err(|error| format!("could not verify the copied transcript: {error}"))?;
        if copied != text {
            return Err("the X11 clipboard did not preserve the transcript".to_string());
        }
        require_x11_target(target, self.focused_window())?;
        self.paste(shortcut)
    }

    fn paste(&self, shortcut: PasteShortcut) -> Result<(), String> {
        let keys = output::paste_keysyms(shortcut)
            .iter()
            .map(|keysym| find_keycode(&self.connection, *keysym))
            .collect::<Result<Vec<_>, _>>()?;
        let result = keys
            .iter()
            .try_for_each(|key| self.send_key(xproto::KEY_PRESS_EVENT, *key));
        let mut released = Ok(());
        for key in keys.iter().rev() {
            let next = self.send_key(xproto::KEY_RELEASE_EVENT, *key);
            released = released.and(next);
        }
        self.connection
            .flush()
            .map_err(|error| format!("could not flush the X11 paste: {error}"))?;
        result.and(released)
    }

    fn send_key(&self, event_type: u8, keycode: u8) -> Result<(), String> {
        self.connection
            .xtest_fake_input(event_type, keycode, x11rb::CURRENT_TIME, 0, 0, 0, 0)
            .map_err(|error| format!("could not request the X11 paste: {error}"))?
            .check()
            .map_err(|error| format!("could not send the X11 paste: {error}"))
    }
}

fn find_keycode(connection: &RustConnection, keysym: u32) -> Result<u8, String> {
    let setup = connection.setup();
    let count = setup.max_keycode - setup.min_keycode + 1;
    let mapping = connection
        .get_keyboard_mapping(setup.min_keycode, count)
        .map_err(|error| format!("could not request the X11 keyboard map: {error}"))?
        .reply()
        .map_err(|error| format!("could not read the X11 keyboard map: {error}"))?;
    mapping
        .keysyms
        .chunks(mapping.keysyms_per_keycode as usize)
        .position(|symbols| symbols.contains(&keysym))
        .map(|index| setup.min_keycode + index as u8)
        .ok_or_else(|| format!("the X11 keyboard map has no keysym {keysym:#x}"))
}

struct WaylandOutput {
    clipboard: Arc<Clipboard>,
    remote_desktop: RemoteDesktop,
    session: Session<RemoteDesktop>,
    current_text: Arc<RwLock<String>>,
}

impl WaylandOutput {
    async fn new(connection: ashpd::zbus::Connection) -> Result<Self, String> {
        let app_id = APP_ID
            .parse()
            .map_err(|error| format!("invalid Koett application ID: {error}"))?;
        ashpd::register_host_app_with_connection(connection.clone(), app_id)
            .await
            .map_err(|error| format!("could not identify Koett to the paste portal: {error}"))?;
        let remote_desktop = RemoteDesktop::with_connection(connection.clone())
            .await
            .map_err(|error| format!("automatic paste is unavailable: {error}"))?;
        if !remote_desktop
            .available_device_types()
            .await
            .map_err(|error| format!("could not read remote-desktop capabilities: {error}"))?
            .contains(DeviceType::Keyboard)
        {
            return Err("the desktop portal does not provide keyboard paste".to_string());
        }
        let clipboard = Arc::new(
            Clipboard::with_connection(connection)
                .await
                .map_err(|error| format!("the clipboard portal is unavailable: {error}"))?,
        );
        let session = remote_desktop
            .create_session(CreateSessionOptions::default())
            .await
            .map_err(|error| format!("could not create an automatic-paste session: {error}"))?;
        let restore_token = read_restore_token();
        remote_desktop
            .select_devices(
                &session,
                SelectDevicesOptions::default()
                    .set_devices(Some(DeviceType::Keyboard.into()))
                    .set_restore_token(restore_token.as_deref())
                    .set_persist_mode(PersistMode::ExplicitlyRevoked),
            )
            .await
            .and_then(|request| request.response())
            .map_err(|error| format!("keyboard paste permission was not granted: {error}"))?;
        clipboard
            .request(&session, Default::default())
            .await
            .map_err(|error| format!("clipboard permission was not granted: {error}"))?;
        let selected = remote_desktop
            .start(&session, None, Default::default())
            .await
            .and_then(|request| request.response())
            .map_err(|error| format!("automatic paste was not enabled: {error}"))?;
        if !selected.devices().contains(DeviceType::Keyboard) || !selected.is_clipboard_enabled() {
            return Err("the desktop did not enable keyboard and clipboard access".to_string());
        }
        if let Some(token) = selected.restore_token() {
            save_restore_token(token)?;
        }

        let current_text = Arc::new(RwLock::new(String::new()));
        let transfer_clipboard = clipboard.clone();
        let transfer_text = current_text.clone();
        tokio::spawn(async move {
            let Ok(transfers) = transfer_clipboard
                .receive_selection_transfer::<RemoteDesktop>()
                .await
            else {
                return;
            };
            futures_util::pin_mut!(transfers);
            while let Some((session, mime_type, serial)) = transfers.next().await {
                let supported =
                    mime_type == "text/plain" || mime_type == "text/plain;charset=utf-8";
                let text = transfer_text
                    .read()
                    .map(|text| text.clone())
                    .unwrap_or_default();
                let success = if supported {
                    match transfer_clipboard.selection_write(&session, serial).await {
                        Ok(fd) => {
                            let mut file = File::from(std::os::fd::OwnedFd::from(fd));
                            file.write_all(text.as_bytes()).is_ok()
                        }
                        Err(_) => false,
                    }
                } else {
                    false
                };
                let _ = transfer_clipboard
                    .selection_write_done(&session, serial, success)
                    .await;
            }
        });

        Ok(Self {
            clipboard,
            remote_desktop,
            session,
            current_text,
        })
    }

    async fn deliver(
        &self,
        text: &str,
        target: Option<u32>,
        shortcut: PasteShortcut,
    ) -> Result<(), String> {
        *self
            .current_text
            .write()
            .map_err(|_| "the clipboard text lock failed".to_string())? = text.to_string();
        let mime_types = ["text/plain;charset=utf-8", "text/plain"];
        self.clipboard
            .set_selection(
                &self.session,
                SetSelectionOptions::default().set_mime_types(&mime_types),
            )
            .await
            .map_err(|error| format!("could not copy the transcript: {error}"))?;
        if !output::target_unchanged(target) {
            return Err("Copied · paste into your target application".into());
        }
        let keys = output::paste_keysyms(shortcut);
        let mut result = Ok(());
        for key in keys {
            if let Err(error) = self.send_keysym(*key as i32, KeyState::Pressed).await {
                result = Err(error);
                break;
            }
        }
        for key in keys.iter().rev() {
            let released = self.send_keysym(*key as i32, KeyState::Released).await;
            result = result.and(released);
        }
        result
    }

    async fn send_keysym(&self, keysym: i32, state: KeyState) -> Result<(), String> {
        self.remote_desktop
            .notify_keyboard_keysym(
                &self.session,
                keysym,
                state,
                NotifyKeyboardKeysymOptions::default(),
            )
            .await
            .map_err(|error| format!("could not send automatic paste: {error}"))
    }
}

fn set_start_at_login(enabled: bool) -> Result<(), String> {
    let config = directories::BaseDirs::new()
        .ok_or_else(|| "could not find the Linux config directory".to_string())?
        .config_dir()
        .join("autostart");
    let file = config.join(format!("{APP_ID}.desktop"));
    if !enabled {
        return match std::fs::remove_file(&file) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("could not remove {}: {error}", file.display())),
        };
    }
    std::fs::create_dir_all(&config)
        .map_err(|error| format!("could not create {}: {error}", config.display()))?;
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not find the Koett executable: {error}"))?;
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=Koett\nExec=\"{}\" --background\nTryExec={}\nTerminal=false\n",
        executable.display(),
        executable.display()
    );
    std::fs::write(&file, desktop)
        .map_err(|error| format!("could not write {}: {error}", file.display()))
}

fn restore_token_file() -> Result<std::path::PathBuf, String> {
    let base = directories::BaseDirs::new()
        .ok_or_else(|| "could not find the Linux state directory".to_string())?;
    let directory = base
        .state_dir()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| base.config_dir().to_path_buf());
    Ok(directory.join("koett").join("remote-desktop-token"))
}

fn read_restore_token() -> Option<String> {
    let token = std::fs::read_to_string(restore_token_file().ok()?).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

fn save_restore_token(token: &str) -> Result<(), String> {
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

    let path = restore_token_file()?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not protect {}: {error}", parent.display()))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| format!("could not create {}: {error}", temporary.display()))?;
    file.write_all(token.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, &path).map_err(|error| {
        format!(
            "could not install {} as {}: {error}",
            temporary.display(),
            path.display()
        )
    })
}

async fn configure_shortcut() -> Result<(), String> {
    let (reply, response) = tokio::sync::oneshot::channel();
    SHORTCUT_COMMANDS
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .ok_or("No active desktop shortcut session. Restart Koett and retry.")?
        .send(reply)
        .map_err(|_| "Desktop shortcut session closed. Restart Koett.".to_string())?;
    response
        .await
        .map_err(|_| "Desktop shortcut session closed during editing".to_string())?
}

fn completed_text(
    raw: &str,
    loaded: Result<Settings, String>,
    last_known: &Settings,
) -> (String, Option<String>) {
    match loaded {
        Ok(settings) => (
            crate::replacements::apply(raw, &settings.replacements),
            None,
        ),
        Err(error) => (
            crate::replacements::apply(raw, &last_known.replacements),
            Some(format!(
                "Settings unavailable; using last valid replacements: {error}"
            )),
        ),
    }
}

fn cache_completed(tray: Option<&TrayUpdates>, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    if let Some(tray) = tray {
        let _ = tray.sender.send(TrayUpdate::Completed(text.to_owned()));
    }
}

fn require_x11_target(target: Option<u32>, focused: Option<u32>) -> Result<(), String> {
    if target.is_some() && target == focused {
        return Ok(());
    }
    Err(
        "Transcript copied; automatic paste skipped because the target changed or is unavailable"
            .into(),
    )
}

fn report_delivery(tray: Option<&TrayUpdates>, delivered: &Result<(), String>) {
    if delivered.is_err()
        && let Some(tray) = tray
    {
        let _ = tray.sender.send(TrayUpdate::DeliveryFailed);
    }
}

fn delivery_results(
    delivered: Result<(), String>,
    saved: Result<(), String>,
) -> Result<(), String> {
    match (delivered, saved) {
        (Err(delivery), Err(storage)) => Err(format!("{delivery}; {storage}")),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod delivery_tests {
    use super::*;
    #[test]
    fn x11_focus_guard_preserves_clipboard_only_recovery() {
        assert_eq!(require_x11_target(Some(42), Some(42)), Ok(()));
        for (target, focused) in [
            (Some(42), Some(99)),
            (Some(42), None),
            (None, Some(42)),
            (None, None),
        ] {
            let (sender, receiver) = mpsc::channel();
            let tray = TrayUpdates { sender };
            let delivered = require_x11_target(target, focused);
            assert!(delivered.is_err());
            report_delivery(Some(&tray), &delivered);
            assert!(matches!(
                receiver.try_recv(),
                Ok(TrayUpdate::DeliveryFailed)
            ));
        }
    }

    #[test]
    fn delivery_recovery_is_typed_and_independent_of_storage() {
        let (sender, receiver) = mpsc::channel();
        let tray = TrayUpdates { sender };
        report_delivery(Some(&tray), &Ok(()));
        assert!(receiver.try_recv().is_err());
        report_delivery(Some(&tray), &Err("guarded clipboard-only result".into()));
        assert!(matches!(
            receiver.try_recv().unwrap(),
            TrayUpdate::DeliveryFailed
        ));
        let failure = delivery_results(Ok(()), Err("storage failed".into()));
        assert_eq!(failure, Err("storage failed".into()));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn corrupt_settings_preserve_raw_text_and_warn() {
        let raw = "Keep Unicode: åäö 日本語";
        let (text, warning) = completed_text(raw, Err("invalid JSON".into()), &Settings::default());
        assert_eq!(text, raw);
        assert!(warning.unwrap().contains("invalid JSON"));
    }
    #[test]
    fn completed_text_remains_copyable_when_delivery_and_storage_fail() {
        let (sender, receiver) = mpsc::channel();
        let tray = TrayUpdates { sender };
        cache_completed(Some(&tray), "Recover this exact result");
        let error = delivery_results(
            Err("clipboard failed".into()),
            Err("history and fallback failed".into()),
        )
        .unwrap_err();
        assert!(
            error.contains("clipboard failed") && error.contains("history and fallback failed")
        );
        assert!(
            matches!(receiver.recv().unwrap(), TrayUpdate::Completed(text) if text == "Recover this exact result")
        );
    }
}

#[cfg(test)]
mod menu_tests {
    use super::*;
    use ksni::Tray;

    #[test]
    fn shortcut_failures_preserve_capture_actions_on_both_surfaces() {
        for (phase, label, enabled) in [
            ("Recording…", "Stop", true),
            ("Transcribing…", "Transcribing…", false),
        ] {
            for detail in ["unavailable", "invalid"] {
                let (sender, receiver) = mpsc::channel();
                let updates = TrayUpdates { sender };
                report_shortcut_failure(Some(&updates), detail);
                let TrayUpdate::Status(notice) = receiver.recv().unwrap() else {
                    panic!("missing shortcut notice");
                };
                assert!(notice.starts_with("@notice:"));
                let (action_sender, _) = mpsc::channel();
                let mut tray = LinuxTray {
                    action_sender,
                    status: phase.into(),
                };
                tray.update_status(notice);
                assert_eq!(tray.status, phase);
                let ksni::MenuItem::Standard(primary) = tray.menu().remove(2) else {
                    panic!("missing recording action");
                };
                assert_eq!((primary.label.as_str(), primary.enabled), (label, enabled));
                let window_action = menu::recording_action(&tray.status);
                assert_eq!(
                    (window_action.label, window_action.enabled),
                    (label, enabled)
                );
            }
        }
    }

    #[test]
    fn tray_navigation_and_capture_dispatch_are_distinct() {
        let (action_sender, receiver) = mpsc::channel();
        let mut tray = LinuxTray {
            action_sender,
            status: "Ready".into(),
        };
        tray.activate(0, 0);
        assert_eq!(receiver.recv().unwrap(), TrayAction::OpenTranscripts);
        for (label, expected) in [
            ("Record", TrayAction::Toggle),
            ("History", TrayAction::OpenTranscripts),
            ("Settings", TrayAction::OpenSettings),
            ("Quit Koett", TrayAction::Quit),
        ] {
            let item = tray
                .menu()
                .into_iter()
                .find_map(|item| match item {
                    ksni::MenuItem::Standard(item) if item.label == label => Some(item),
                    _ => None,
                })
                .unwrap();
            if label == "Record" {
                assert_eq!(
                    item.enabled,
                    !QUIT_BUSY.load(std::sync::atomic::Ordering::Acquire)
                );
            } else {
                assert!(item.enabled);
            }
            (item.activate)(&mut tray);
            assert_eq!(receiver.recv().unwrap(), expected);
        }
    }

    fn find_menu_action(
        items: Vec<ksni::MenuItem<LinuxTray>>,
        label: &str,
    ) -> Option<ksni::menu::StandardItem<LinuxTray>> {
        items.into_iter().find_map(|item| match item {
            ksni::MenuItem::Standard(item) if item.label == label => Some(item),
            ksni::MenuItem::SubMenu(item) => find_menu_action(item.submenu, label),
            _ => None,
        })
    }

    #[test]
    fn tray_recovery_and_folder_actions_dispatch_shared_operations() {
        let (action_sender, receiver) = mpsc::channel();
        let mut tray = LinuxTray {
            action_sender,
            status: "Ready".into(),
        };
        for (label, command) in [
            ("Copy last transcript", "copy-last"),
            ("Paste last transcript", "paste-last"),
            ("Copy unsaved transcripts", "copy-unsaved"),
            ("Retry Save", "retry-unsaved"),
            ("Media transcripts…", "media-transcripts"),
            ("Failed transcripts…", "failed-transcripts"),
            ("Failed recordings…", "failed-recordings"),
            ("Recovery status", "recovery-status"),
        ] {
            let item = find_menu_action(tray.menu(), label).unwrap();
            if matches!(command, "copy-last" | "paste-last") {
                assert_eq!(item.enabled, history::recovery::last_available());
            }
            (item.activate)(&mut tray);
            assert_eq!(
                receiver.recv().unwrap(),
                TrayAction::Recovery(command.into())
            );
        }
    }

    #[test]
    fn control_messages_preserve_tray_phase_and_busy_sensitivity() {
        let (action_sender, _) = mpsc::channel();
        let mut tray = LinuxTray {
            action_sender,
            status: "Transcribing…".into(),
        };
        for message in [
            "@notice:copy unavailable",
            "@refresh",
            "@show",
            "@settings",
            "@quit",
        ] {
            tray.update_status(message.into());
            assert_eq!(tray.status, "Transcribing…");
            assert_eq!(tray.tool_tip().description, "Transcribing…");
            let ksni::MenuItem::Standard(primary) = tray.menu().remove(2) else {
                panic!("missing primary recording action");
            };
            assert_eq!(primary.label, "Transcribing…");
            assert!(!primary.enabled);
        }
        tray.update_status("Ready".into());
        assert_eq!(tray.status, "Ready");
    }
}
