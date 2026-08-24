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
    BindShortcutsOptions, ConfigureShortcutsOptions, GlobalShortcuts, ListShortcutsOptions,
    NewShortcut,
};
use ashpd::desktop::remote_desktop::{
    DeviceType, KeyState, NotifyKeyboardKeysymOptions, RemoteDesktop, SelectDevicesOptions,
};
use ashpd::desktop::{CreateSessionOptions, PersistMode, Session};
use futures_util::StreamExt;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use ksni::blocking::{Handle as TrayHandle, TrayMethods as _};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt as _};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use crate::history;
use crate::model::ModelProgress;
use crate::runtime::{self, EngineCommand, EngineHandle, EngineUpdate};
use crate::settings::Settings;
use crate::shortcut::Shortcut;
use crate::state::AppStatus;

const APP_ID: &str = "com.olledyberg.Koett";
const SHORTCUT_ID: &str = "toggle-recording";

#[derive(Clone, Copy, Debug)]
enum TrayAction {
    Toggle,
    OpenTranscripts,
    OpenSettings,
    ConfigureShortcut,
    ToggleStartAtLogin,
    Quit,
}

#[derive(Debug)]
struct LinuxTray {
    action_sender: Sender<TrayAction>,
    status: String,
    shortcut: String,
    start_at_login: bool,
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
        let _ = self.action_sender.send(TrayAction::Toggle);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};

        vec![
            StandardItem {
                label: self.status.clone(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format!("Shortcut: {}", self.shortcut),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Start or stop recording".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::Toggle);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Change shortcut…".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::ConfigureShortcut);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Settings".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Transcripts".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::OpenTranscripts);
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Start at login".to_string(),
                checked: self.start_at_login,
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_sender.send(TrayAction::ToggleStartAtLogin);
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
        ]
    }
}

pub fn run() -> Result<(), String> {
    let _instance = single_instance()?;
    let settings = Settings::load()?;
    if settings.start_at_login {
        set_start_at_login(true)?;
    }
    let (action_sender, action_receiver) = mpsc::channel();
    let tray = LinuxTray {
        action_sender,
        status: "Starting…".to_string(),
        shortcut: settings.shortcut.clone(),
        start_at_login: settings.start_at_login,
    }
    .assume_sni_available(true)
    .spawn()
    .ok();
    let session_type = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let use_x11 = session_type == "x11"
        || (session_type != "wayland"
            && std::env::var_os("WAYLAND_DISPLAY").is_none()
            && std::env::var_os("DISPLAY").is_some());
    let result = if use_x11 {
        run_x11(settings, action_receiver, tray.as_ref())
    } else {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("could not start the Linux event loop: {error}"))?
            .block_on(run_wayland(settings, action_receiver, tray.as_ref()))
    };
    if let Some(tray) = tray {
        tray.shutdown().wait();
    }
    result
}

pub fn show_fatal_error(error: &str) {
    eprintln!("Koett: {error}");
    let message = format!("Koett could not start.\n\n{error}");
    let dialogs: [(&str, &[&str]); 3] = [
        ("zenity", &["--error", "--title=Koett", "--text"]),
        ("kdialog", &["--title", "Koett", "--error"]),
        ("notify-send", &["--urgency=critical", "Koett"]),
    ];
    for (program, arguments) in dialogs {
        if Command::new(program)
            .args(arguments)
            .arg(&message)
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
    tray: Option<&TrayHandle<LinuxTray>>,
) -> Result<(), String> {
    let mut shortcut = HotKey::from_str(&settings.shortcut)
        .map_err(|error| format!("invalid shortcut {}: {error}", settings.shortcut))?;
    let hotkeys = GlobalHotKeyManager::new()
        .map_err(|error| format!("could not start X11 shortcuts: {error}"))?;
    let mut shortcut_registered = hotkeys.register(shortcut).is_ok();
    if !shortcut_registered {
        update_tray_status(
            tray,
            "Error: shortcut unavailable — open settings to change it",
        );
    }
    let mut output = X11Output::new()?;
    let engine = runtime::start(settings.clone());
    let mut app = LinuxEngine::new(engine);
    let mut last_settings_check = Instant::now();

    loop {
        while let Ok(action) = actions.try_recv() {
            match action {
                TrayAction::Toggle => app.toggle(|| output.focused_window()),
                TrayAction::OpenTranscripts => open_transcripts()?,
                TrayAction::OpenSettings | TrayAction::ConfigureShortcut => {
                    settings.save()?;
                    open_path(&crate::paths::settings_file()?)?;
                }
                TrayAction::ToggleStartAtLogin => {
                    settings.start_at_login = !settings.start_at_login;
                    set_start_at_login(settings.start_at_login)?;
                    settings.save()?;
                    update_tray_settings(tray, &settings.shortcut, settings.start_at_login);
                }
                TrayAction::Quit => {
                    app.stop();
                    return Ok(());
                }
            }
        }
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if shortcut_registered
                && event.id == shortcut.id()
                && event.state == HotKeyState::Pressed
            {
                app.toggle(|| output.focused_window());
            }
        }
        app.poll(
            |model, text, target| {
                let delivered = output.deliver(text, target);
                let saved =
                    history::append_transcript_resilient(model, text).and_then(|fallback| {
                        fallback.map_or(Ok(()), |path| {
                            Err(format!(
                                "the transcript was saved to the fallback file {}",
                                path.display()
                            ))
                        })
                    });
                delivered.and(saved)
            },
            |status| update_tray_status(tray, status),
        );
        if last_settings_check.elapsed() >= Duration::from_millis(500) {
            last_settings_check = Instant::now();
            if let Ok(updated) = Settings::load()
                && updated.shortcut != settings.shortcut
            {
                match HotKey::from_str(&updated.shortcut) {
                    Ok(next) if hotkeys.register(next).is_ok() => {
                        if shortcut_registered {
                            let _ = hotkeys.unregister(shortcut);
                        }
                        shortcut = next;
                        shortcut_registered = true;
                        settings.shortcut = updated.shortcut;
                        update_tray_settings(tray, &settings.shortcut, settings.start_at_login);
                    }
                    Ok(_) => update_tray_status(tray, "Error: shortcut unavailable"),
                    Err(_) => update_tray_status(tray, "Error: invalid shortcut"),
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

async fn run_wayland(
    mut settings: Settings,
    actions: Receiver<TrayAction>,
    tray: Option<&TrayHandle<LinuxTray>>,
) -> Result<(), String> {
    let shortcut = Shortcut::from_str(&settings.shortcut)?;
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
    let mut shortcut_label = listed
        .shortcuts()
        .iter()
        .find(|existing| existing.id() == SHORTCUT_ID)
        .map(|existing| existing.trigger_description().to_string());
    if shortcut_label.is_none() {
        let requested = NewShortcut::new(SHORTCUT_ID, "Start or stop Koett dictation")
            .preferred_trigger(shortcut.portal_trigger().as_str());
        let bound = shortcuts
            .bind_shortcuts(
                &shortcut_session,
                &[requested],
                None,
                BindShortcutsOptions::default(),
            )
            .await
            .and_then(|request| request.response())
            .map_err(|error| format!("could not bind the Koett shortcut: {error}"))?;
        if !bound
            .shortcuts()
            .iter()
            .any(|existing| existing.id() == SHORTCUT_ID)
        {
            return Err("the desktop did not bind the Koett shortcut".to_string());
        }
        shortcut_label = bound
            .shortcuts()
            .iter()
            .find(|existing| existing.id() == SHORTCUT_ID)
            .map(|existing| existing.trigger_description().to_string());
    }
    update_tray_settings(
        tray,
        shortcut_label.as_deref().unwrap_or(&settings.shortcut),
        settings.start_at_login,
    );

    let output = WaylandOutput::new(connection).await?;
    let engine = runtime::start(settings.clone());
    let mut app = LinuxEngine::new(engine);
    let mut activations = shortcuts
        .receive_activated()
        .await
        .map_err(|error| format!("could not listen for Koett shortcuts: {error}"))?;
    let mut shortcut_changes = shortcuts
        .receive_shortcuts_changed()
        .await
        .map_err(|error| format!("could not watch Koett shortcut changes: {error}"))?;
    let mut timer = tokio::time::interval(Duration::from_millis(10));

    loop {
        tokio::select! {
            activation = activations.next() => {
                let Some(activation) = activation else {
                    return Err("the desktop shortcut session closed".to_string());
                };
                if activation.shortcut_id() == SHORTCUT_ID {
                    app.toggle(|| None);
                }
            }
            change = shortcut_changes.next() => {
                let Some(change) = change else {
                    return Err("the desktop shortcut settings session closed".to_string());
                };
                if let Some(changed) = change
                    .shortcuts()
                    .iter()
                    .find(|changed| changed.id() == SHORTCUT_ID)
                {
                    shortcut_label = Some(changed.trigger_description().to_string());
                    update_tray_settings(
                        tray,
                        shortcut_label.as_deref().unwrap_or(&settings.shortcut),
                        settings.start_at_login,
                    );
                }
            }
            _ = timer.tick() => {
                while let Ok(action) = actions.try_recv() {
                    match action {
                        TrayAction::Toggle => app.toggle(|| None),
                        TrayAction::OpenTranscripts => open_transcripts()?,
                        TrayAction::OpenSettings => {
                            settings.save()?;
                            open_path(&crate::paths::settings_file()?)?;
                        }
                        TrayAction::ConfigureShortcut => {
                            if shortcuts.version() < 2 {
                                update_tray_status(
                                    tray,
                                    "Error: this desktop cannot edit portal shortcuts",
                                );
                            } else {
                                shortcuts
                                    .configure_shortcuts(
                                        &shortcut_session,
                                        None,
                                        ConfigureShortcutsOptions::default(),
                                    )
                                    .await
                                    .map_err(|error| format!("could not open shortcut settings: {error}"))?;
                            }
                        }
                        TrayAction::ToggleStartAtLogin => {
                            settings.start_at_login = !settings.start_at_login;
                            set_start_at_login(settings.start_at_login)?;
                            settings.save()?;
                            update_tray_settings(
                                tray,
                                shortcut_label.as_deref().unwrap_or(&settings.shortcut),
                                settings.start_at_login,
                            );
                        }
                        TrayAction::Quit => {
                            app.stop();
                            return Ok(());
                        }
                    }
                }
                let mut completed = Vec::new();
                app.poll(
                    |model, text, _| {
                        completed.push((model.to_string(), text.to_string()));
                        Ok(())
                    },
                    |status| update_tray_status(tray, status),
                );
                for (model, text) in completed {
                    let mut error = None;
                    if let Err(delivery_error) = output.deliver(&text).await {
                        error = Some(delivery_error);
                    }
                    match history::append_transcript_resilient(&model, &text) {
                        Ok(Some(path)) => {
                            error = Some(format!(
                                "the transcript was saved to the fallback file {}",
                                path.display()
                            ));
                        }
                        Ok(None) => {}
                        Err(save_error) => error = Some(save_error),
                    }
                    if let Some(error) = error {
                        update_tray_status(tray, format!("Error: {error}").as_str());
                    }
                }
            }
        }
    }
}

struct LinuxEngine {
    engine: EngineHandle,
    status: AppStatus,
    paste_target: Option<u32>,
}

impl LinuxEngine {
    fn new(engine: EngineHandle) -> Self {
        Self {
            engine,
            status: AppStatus::Starting,
            paste_target: None,
        }
    }

    fn toggle(&mut self, focused_window: impl FnOnce() -> Option<u32>) {
        if self.status != AppStatus::Ready
            && self.status != AppStatus::Recording
            && !matches!(self.status, AppStatus::Error(_))
        {
            return;
        }
        if self.status == AppStatus::Recording {
            self.paste_target = focused_window();
        }
        let _ = self.engine.commands.send(EngineCommand::Toggle);
    }

    fn stop(self) {
        self.engine.stop();
    }

    fn poll(
        &mut self,
        mut deliver: impl FnMut(&str, &str, Option<u32>) -> Result<(), String>,
        mut status_changed: impl FnMut(&str),
    ) {
        let mut interaction_error = None;
        while let Ok(update) = self.engine.updates.try_recv() {
            match update {
                EngineUpdate::Status(status) => {
                    self.status = status;
                    status_changed(status_text(&self.status).as_str());
                }
                EngineUpdate::TranscriptReady { model, transcript } => {
                    if let Err(error) = deliver(&model, &transcript.text, self.paste_target.take())
                    {
                        eprintln!("Koett: {error}");
                        interaction_error = Some(error);
                    }
                }
                EngineUpdate::ModelProgress(ModelProgress::Downloading { received, total }) => {
                    let percent = received.saturating_mul(100) / total.max(1);
                    status_changed(format!("Downloading model… {percent}%").as_str());
                }
                EngineUpdate::ModelProgress(ModelProgress::Installing) => {
                    status_changed("Installing model…");
                }
                EngineUpdate::ModelProgress(ModelProgress::Ready) => {
                    status_changed("Loading model…");
                }
                EngineUpdate::RecordingStarted(_) => {}
            }
        }
        if let Some(error) = interaction_error {
            self.status = AppStatus::Error(error);
            status_changed(status_text(&self.status).as_str());
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

fn update_tray_status(tray: Option<&TrayHandle<LinuxTray>>, status: &str) {
    if let Some(tray) = tray {
        let _ = tray.update(|tray| tray.status = status.to_string());
    }
}

fn update_tray_settings(
    tray: Option<&TrayHandle<LinuxTray>>,
    shortcut: &str,
    start_at_login: bool,
) {
    if let Some(tray) = tray {
        let _ = tray.update(|tray| {
            tray.shortcut = shortcut.to_string();
            tray.start_at_login = start_at_login;
        });
    }
}

fn open_transcripts() -> Result<(), String> {
    let path = crate::paths::history_file()?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("could not protect {}: {error}", path.display()))?;
    open_path(&path)
}

fn open_path(path: &std::path::Path) -> Result<(), String> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not open {}: {error}", path.display()))
}

struct X11Output {
    clipboard: arboard::Clipboard,
    connection: RustConnection,
    control_keycode: u8,
    v_keycode: u8,
}

impl X11Output {
    fn new() -> Result<Self, String> {
        let clipboard = arboard::Clipboard::new()
            .map_err(|error| format!("could not open the X11 clipboard: {error}"))?;
        let (connection, _) =
            x11rb::connect(None).map_err(|error| format!("could not connect to X11: {error}"))?;
        let control_keycode = find_keycode(&connection, xkeysym::key::Control_L)?;
        let v_keycode = find_keycode(&connection, xkeysym::key::v)?;
        Ok(Self {
            clipboard,
            connection,
            control_keycode,
            v_keycode,
        })
    }

    fn focused_window(&self) -> Option<u32> {
        self.connection
            .get_input_focus()
            .ok()?
            .reply()
            .ok()
            .map(|reply| reply.focus)
    }

    fn deliver(&mut self, text: &str, target: Option<u32>) -> Result<(), String> {
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
        if target.is_some() && target == self.focused_window() {
            self.paste()?;
        }
        Ok(())
    }

    fn paste(&self) -> Result<(), String> {
        let result = (|| {
            self.send_key(xproto::KEY_PRESS_EVENT, self.control_keycode)?;
            self.send_key(xproto::KEY_PRESS_EVENT, self.v_keycode)
        })();
        let _ = self.send_key(xproto::KEY_RELEASE_EVENT, self.v_keycode);
        let _ = self.send_key(xproto::KEY_RELEASE_EVENT, self.control_keycode);
        self.connection
            .flush()
            .map_err(|error| format!("could not flush the X11 paste: {error}"))?;
        result
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

    async fn deliver(&self, text: &str) -> Result<(), String> {
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
        self.send_keysym(xkeysym::key::Control_L as i32, KeyState::Pressed)
            .await?;
        let result = self
            .send_keysym(xkeysym::key::v as i32, KeyState::Pressed)
            .await;
        let _ = self
            .send_keysym(xkeysym::key::v as i32, KeyState::Released)
            .await;
        let _ = self
            .send_keysym(xkeysym::key::Control_L as i32, KeyState::Released)
            .await;
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
        "[Desktop Entry]\nType=Application\nName=Koett\nExec=\"{}\"\nTryExec={}\nTerminal=false\n",
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
