//! Signed automatic updates. Inert unless a build carries a feed, a public key and a
//! build number, which release builds don't yet. Checks in the background, downloads
//! quietly, and installs only when Koett is idle, through the same `install.py` a
//! person runs. The version it replaces stays on disk for `koett rollback-update`.
use crate::update_feed::{self, Feed};
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FEED: Option<&str> = option_env!("KOETT_UPDATE_FEED");
const PUBLIC_KEY: Option<&str> = option_env!("KOETT_UPDATE_PUBLIC_KEY");
const BUILD: Option<&str> = option_env!("KOETT_UPDATE_BUILD");
/// Lab builds only: allows plain HTTP to this machine.
const LOOPBACK: bool = option_env!("KOETT_UPDATE_LOOPBACK_TESTING").is_some();
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(60);
/// A laptop that started offline shouldn't wait a day to try again.
const RETRY_AFTER: Duration = Duration::from_secs(60 * 60);
const TICK: Duration = Duration::from_secs(1);
const FEED_LIMIT: Duration = Duration::from_secs(30);
const DOWNLOAD_LIMIT: Duration = Duration::from_secs(60 * 60);
const UNPACK_LIMIT: Duration = Duration::from_secs(120);
const PROBE_LIMIT: Duration = Duration::from_secs(30);
const INSTALL_LIMIT: Duration = Duration::from_secs(300);
/// Versions the updater keeps once it has installed some: the running one and one before.
const KEEP_VERSIONS: usize = 2;

struct Config {
    feed: &'static str,
    public_key: [u8; 32],
    build: u64,
}

fn parse_config(
    feed: Option<&'static str>,
    key: Option<&str>,
    build: Option<&str>,
    loopback: bool,
) -> Option<Config> {
    let feed = feed.filter(|feed| update_feed::accepts_url(feed, loopback))?;
    let public_key = STANDARD.decode(key?).ok()?.try_into().ok()?;
    let build = build?.parse().ok().filter(|build| *build > 0)?;
    Some(Config {
        feed,
        public_key,
        build,
    })
}

fn config() -> Option<&'static Config> {
    static CONFIG: OnceLock<Option<Config>> = OnceLock::new();
    CONFIG
        .get_or_init(|| parse_config(FEED, PUBLIC_KEY, BUILD, LOOPBACK))
        .as_ref()
}

/// `prefix` from `prefix/lib/koett/versions/ID/libexec/koett`, with that ID. Anything
/// else, such as a development build, isn't something this can replace.
fn install_location(executable: &Path) -> Option<(PathBuf, String)> {
    let version = executable.parent()?.parent()?;
    let versions = version.parent()?;
    if !versions.ends_with("lib/koett/versions") || !executable.ends_with("libexec/koett") {
        return None;
    }
    let prefix = versions.parent()?.parent()?.parent()?.to_path_buf();
    Some((prefix, version.file_name()?.to_str()?.to_owned()))
}

fn location() -> Option<(PathBuf, String)> {
    install_location(&std::env::current_exe().ok()?.canonicalize().ok()?).filter(|(prefix, _)| {
        writable(&prefix.join("bin")) && writable(&prefix.join("lib/koett/versions"))
    })
}

/// A system-wide install isn't this user's to replace, and shouldn't download daily to find out.
fn writable(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        // SAFETY: the pointer is a valid C string for the length of the call.
        .is_ok_and(|path| unsafe { libc::access(path.as_ptr(), libc::W_OK) } == 0)
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct State {
    last_check: u64,
    /// Rolled back from this build, so it isn't offered again.
    skip_build: u64,
    previous: Option<String>,
    /// Versions the updater installed, oldest first. Only these are ever removed.
    managed: Vec<String>,
}

fn state_file() -> Result<PathBuf, String> {
    Ok(crate::paths::updates_directory()?.join("state.json"))
}

fn load_state() -> State {
    state_file()
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_state(state: &State) -> Result<(), String> {
    let path = state_file()?;
    private_directory(path.parent().ok_or("no updates folder")?)?;
    let temporary = path.with_extension("json.new");
    std::fs::write(
        &temporary,
        serde_json::to_vec(state).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|error| error.to_string())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[derive(Clone, Default)]
struct Status {
    enabled: bool,
    state: &'static str,
    available_version: Option<String>,
    available_build: Option<u64>,
    error: Option<String>,
}

static STATUS: Mutex<Option<Status>> = Mutex::new(None);
static RESERVED: AtomicBool = AtomicBool::new(false);
static UI_BUSY: AtomicBool = AtomicBool::new(false);
static WINDOW_SHOWN: AtomicBool = AtomicBool::new(false);
static COMMANDS: OnceLock<Mutex<mpsc::Sender<Command>>> = OnceLock::new();
static GATE: Mutex<()> = Mutex::new(());
static UI: OnceLock<mpsc::Sender<super::window::Update>> = OnceLock::new();

fn change(apply: impl FnOnce(&mut Status)) {
    let mut status = STATUS.lock().unwrap_or_else(|error| error.into_inner());
    apply(status.get_or_insert_with(Status::default));
}

pub fn enabled() -> bool {
    config().is_some() && location().is_some()
}

/// Permission to start work, or None while the app is being replaced. Hold it until the
/// work has marked the app busy: `reserve` takes the same lock before it looks.
pub fn admit() -> Option<MutexGuard<'static, ()>> {
    let guard = GATE.lock().unwrap_or_else(|error| error.into_inner());
    if !RESERVED.load(Ordering::SeqCst) {
        return Some(guard);
    }
    if let Some(ui) = UI.get() {
        let _ = ui.send(super::window::Update::Status(
            "@notice:Koett is updating and will be right back".into(),
        ));
    }
    None
}

/// What the window can see and the engine can't: a result pill still up, a dialog open.
pub fn set_ui_state(busy: bool, window_shown: bool) {
    UI_BUSY.store(busy, Ordering::Relaxed);
    WINDOW_SHOWN.store(window_shown, Ordering::Relaxed);
}

fn idle() -> bool {
    !super::QUIT_BUSY.load(Ordering::SeqCst)
        && !UI_BUSY.load(Ordering::Relaxed)
        && crate::history::recovery::pending_count() == 0
}

enum Command {
    Check,
    Rollback(mpsc::Sender<serde_json::Value>),
}

fn agent(limit: Duration) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    ureq::Agent::config_builder()
        .https_only(!LOOPBACK)
        .max_redirects(5)
        // The whole body counts toward ureq's body limit, so only the global one applies.
        .timeout_global(Some(limit))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::Rustls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent()
}

/// The offer in the feed, if it's newer than this build and not rolled back from.
fn offered(feed: &Feed, current: u64, skip: u64) -> bool {
    feed.build > current && feed.build != skip
}

fn fetch_feed(config: &Config) -> Result<Feed, String> {
    let mut response = agent(FEED_LIMIT)
        .get(config.feed)
        .call()
        .map_err(|error| format!("Could not check for updates: {error}"))?;
    let mut raw = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(update_feed::MAX_FEED_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|error| format!("Could not check for updates: {error}"))?;
    if raw.len() as u64 > update_feed::MAX_FEED_BYTES {
        return Err("The update feed is too large".into());
    }
    Feed::verified(&raw, &config.public_key, LOOPBACK)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn file_matches(path: &Path, feed: &Feed) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    let mut total = 0;
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                digest.update(&buffer[..count]);
                total += count as u64;
            }
            Err(_) => return false,
        }
    }
    total == feed.length && hex(&digest.finalize()).eq_ignore_ascii_case(&feed.sha256)
}

/// Downloads to the side and only keeps bytes that match the signed length and checksum.
fn download(feed: &Feed) -> Result<PathBuf, String> {
    let folder = crate::paths::updates_directory()?;
    private_directory(&folder)?;
    let path = folder.join(format!("{}.tar.gz", feed.build));
    if file_matches(&path, feed) {
        return Ok(path);
    }
    // An offer can move on while one waits for an idle moment.
    if let Ok(entries) = std::fs::read_dir(&folder) {
        for stale in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if stale.extension().is_some_and(|extension| extension == "gz") {
                let _ = std::fs::remove_file(stale);
            }
        }
    }
    let partial = path.with_extension("part");
    let result = (|| {
        let mut response = agent(DOWNLOAD_LIMIT)
            .get(&feed.url)
            .call()
            .map_err(|error| format!("Could not download the update: {error}"))?;
        let mut file = std::fs::File::create(&partial).map_err(|error| error.to_string())?;
        let copied = std::io::copy(
            &mut response.body_mut().as_reader().take(feed.length + 1),
            &mut file,
        )
        .map_err(|error| format!("Could not download the update: {error}"))?;
        file.flush().map_err(|error| error.to_string())?;
        if copied != feed.length || !file_matches(&partial, feed) {
            return Err("The downloaded update does not match its signature".to_string());
        }
        std::fs::rename(&partial, &path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    result.map(|()| path)
}

/// A path list without the folders under `root`, or None if nothing is left.
fn without_root(list: &std::ffi::OsStr, root: &Path) -> Option<std::ffi::OsString> {
    let kept: Vec<_> = std::env::split_paths(list)
        .filter(|path| !path.starts_with(root))
        .collect();
    if kept.is_empty() {
        return None;
    }
    std::env::join_paths(kept).ok()
}

fn clean_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    // Set by the launcher for this version's runtime. The next version sets its own,
    // and this version's folders may be gone by the time that one's children look.
    for name in ["LD_LIBRARY_PATH", "GTK_DATA_PREFIX"] {
        command.env_remove(name);
    }
    let root = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.canonicalize().ok())
        .and_then(|executable| executable.parent()?.parent().map(Path::to_path_buf));
    if let Some(root) = root {
        for name in ["PATH", "XDG_DATA_DIRS"] {
            if let Some(value) = std::env::var_os(name) {
                match without_root(&value, &root) {
                    Some(kept) => command.env(name, kept),
                    None => command.env_remove(name),
                };
            }
        }
    }
    command
}

/// A step that hangs must not hold the reservation, or dictation stays blocked.
fn bounded(limit: Duration, program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut command = clean_command("timeout");
    command
        .args(["-k", "5", &limit.as_secs().to_string()])
        .arg(program);
    command
}

/// Unpacks, makes sure the package really is the signed build by asking it, and installs.
/// Returns the new version's ID. `install.py` swaps the launcher atomically at the end.
fn install(archive: &Path, feed: &Feed, prefix: &Path) -> Result<String, String> {
    let work = crate::paths::updates_directory()?.join(format!("install-{}", feed.build));
    let _ = std::fs::remove_dir_all(&work);
    private_directory(&work)?;
    let result = (|| {
        let unpacked = bounded(UNPACK_LIMIT, "tar")
            .arg("-xzf")
            .arg(archive)
            .arg("--no-same-owner")
            .arg("-C")
            .arg(&work)
            .status()
            .map_err(|error| format!("Could not unpack the update: {error}"))?;
        if !unpacked.success() {
            return Err("Could not unpack the update".to_string());
        }
        let package = std::fs::read_dir(&work)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.join("install.py").is_file())
            .ok_or("The update has no installer")?;
        let reported = bounded(PROBE_LIMIT, package.join("koett"))
            .arg("update-build")
            .output()
            .map_err(|error| format!("The update would not start: {error}"))?;
        let reported = String::from_utf8_lossy(&reported.stdout);
        if reported.trim() != feed.build.to_string() {
            return Err(format!(
                "The update reports build {} instead of {}",
                reported.trim(),
                feed.build
            ));
        }
        let installed = bounded(INSTALL_LIMIT, "python3")
            .arg(package.join("install.py"))
            .arg("--prefix")
            .arg(prefix)
            .output()
            .map_err(|error| format!("Could not run the installer: {error}"))?;
        if !installed.status.success() {
            return Err(format!(
                "The installer failed: {}",
                String::from_utf8_lossy(&installed.stderr)
                    .lines()
                    .last()
                    .unwrap_or_default()
            ));
        }
        let receipt: serde_json::Value = serde_json::from_slice(&installed.stdout)
            .map_err(|_| "The installer gave no receipt".to_string())?;
        let version = receipt["version"]
            .as_str()
            .ok_or("The installer gave no version")?;
        Path::new(version)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .ok_or_else(|| "The installer gave no version".to_string())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Keeps the running version and the one before; older ones this updater installed go.
fn removable(managed: &[String], keep: &[&str]) -> Vec<String> {
    let surplus = managed.len().saturating_sub(KEEP_VERSIONS);
    managed[..surplus]
        .iter()
        .filter(|id| !keep.contains(&id.as_str()))
        .cloned()
        .collect()
}

fn prune(prefix: &Path, state: &mut State, keep: &[&str]) {
    for id in removable(&state.managed, keep) {
        let version = prefix.join("lib/koett/versions").join(&id);
        if let Err(error) = std::fs::remove_dir_all(&version)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("Could not remove old version {id}: {error}");
            continue;
        }
        state.managed.retain(|kept| *kept != id);
    }
}

/// Starts the new version once this process has gone, then quits this one.
fn relaunch(prefix: &Path, actions: &mpsc::Sender<super::TrayAction>) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let mode = if WINDOW_SHOWN.load(Ordering::Relaxed) {
        "show"
    } else {
        "--background"
    };
    let mut waiter = clean_command("sh");
    waiter
        .args([
            "-c",
            // A parent that doesn't reap leaves a zombie, which `kill -0` calls alive.
            "i=0; while [ -r \"/proc/$1/status\" ] && ! grep -q '^State:[[:space:]]*Z' \"/proc/$1/status\" && [ \"$i\" -lt 150 ]; do sleep 0.2; i=$((i+1)); done; exec \"$2\" \"$3\"",
            "koett-update",
        ])
        .arg(std::process::id().to_string())
        .arg(prefix.join("bin/koett"))
        .arg(mode)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // SAFETY: setsid is async-signal-safe and touches no parent state.
    unsafe {
        waiter.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    waiter
        .spawn()
        .map_err(|error| format!("Could not restart Koett: {error}"))?;
    actions
        .send(super::TrayAction::Quit)
        .map_err(|_| "Could not restart Koett".to_string())
}

/// Takes the idle app, so nothing new can start while it's replaced.
fn reserve() -> bool {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    RESERVED.store(true, Ordering::SeqCst);
    if idle() {
        return true;
    }
    RESERVED.store(false, Ordering::SeqCst);
    false
}

fn point_launcher(prefix: &Path, version: &str) -> Result<(), String> {
    let target = prefix
        .join("lib/koett/versions")
        .join(version)
        .join("koett");
    if !target.is_file() {
        return Err("That version is no longer on disk".into());
    }
    let bin = prefix.join("bin");
    let link = bin.join(format!(".koett-rollback-{}", std::process::id()));
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&target, &link).map_err(|error| error.to_string())?;
    std::fs::rename(&link, bin.join("koett")).map_err(|error| error.to_string())
}

fn rollback(
    prefix: &Path,
    config: &Config,
    actions: &mpsc::Sender<super::TrayAction>,
) -> serde_json::Value {
    let failure = |code: &str, message: String| serde_json::json!({"ok":false,"error":{"code":code,"message":message}});
    let mut state = load_state();
    let Some(previous) = state.previous.clone() else {
        return failure(
            "no_previous_version",
            "There is no earlier version to go back to".into(),
        );
    };
    if !reserve() {
        return failure("busy", "Finish recording or processing first".into());
    }
    let result = point_launcher(prefix, &previous).and_then(|()| {
        state.skip_build = config.build;
        state.previous = None;
        save_state(&state)?;
        relaunch(prefix, actions)
    });
    match result {
        Ok(()) => serde_json::json!({"ok":true,"status":"rolling_back","version":previous}),
        Err(error) => {
            RESERVED.store(false, Ordering::SeqCst);
            failure("rollback_failed", error)
        }
    }
}

pub fn start(actions: mpsc::Sender<super::TrayAction>, ui: mpsc::Sender<super::window::Update>) {
    let Some(config) = config() else {
        return;
    };
    let Some((prefix, current)) = location() else {
        return;
    };
    let (sender, commands) = mpsc::channel();
    let _ = COMMANDS.set(Mutex::new(sender));
    let _ = UI.set(ui.clone());
    change(|status| {
        status.enabled = true;
        status.state = "idle";
    });
    let spawned = std::thread::Builder::new()
        .name("koett-updates".into())
        .spawn(move || run(config, &prefix, &current, &actions, &ui, &commands));
    if let Err(error) = spawned {
        change(|status| status.error = Some(error.to_string()));
    }
}

fn run(
    config: &Config,
    prefix: &Path,
    current: &str,
    actions: &mpsc::Sender<super::TrayAction>,
    ui: &mpsc::Sender<super::window::Update>,
    commands: &mpsc::Receiver<Command>,
) {
    let say = |text: &str| {
        let _ = ui.send(super::window::Update::Status(format!("@notice:{text}")));
    };
    // Wall-clock on purpose: suspended hours count, unlike an `Instant`.
    let mut next_check = now()
        + FIRST_CHECK_AFTER.as_secs().max(
            CHECK_EVERY
                .as_secs()
                .saturating_sub(now().saturating_sub(load_state().last_check)),
        );
    let mut ready = None::<(Feed, PathBuf)>;
    loop {
        let mut manual = false;
        match commands.recv_timeout(TICK) {
            Ok(Command::Check) => manual = true,
            Ok(Command::Rollback(reply)) => {
                let _ = reply.send(rollback(prefix, config, actions));
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if (manual || now() >= next_check) && ready.is_none() {
            change(|status| {
                status.state = "checking";
                status.error = None;
            });
            let state = load_state();
            let outcome = fetch_feed(config).and_then(|feed| {
                if !offered(&feed, config.build, state.skip_build) {
                    return Ok(None);
                }
                change(|status| {
                    status.state = "downloading";
                    status.available_version = Some(feed.version.clone());
                    status.available_build = Some(feed.build);
                });
                download(&feed).map(|path| Some((feed, path)))
            });
            next_check = now()
                + if outcome.is_ok() {
                    let mut state = load_state();
                    state.last_check = now();
                    let _ = save_state(&state);
                    CHECK_EVERY.as_secs()
                } else {
                    RETRY_AFTER.as_secs()
                };
            match outcome {
                Ok(Some(found)) => {
                    if manual {
                        say(&format!(
                            "Koett {} will install when Koett is idle",
                            found.0.version
                        ));
                    }
                    change(|status| status.state = "pending");
                    ready = Some(found);
                }
                Ok(None) => {
                    change(|status| {
                        status.state = "idle";
                        status.available_version = None;
                        status.available_build = None;
                    });
                    if manual {
                        say("Koett is up to date");
                    }
                }
                Err(error) => {
                    eprintln!("Update check failed: {error}");
                    if manual {
                        say(&error);
                    }
                    change(|status| {
                        status.state = "idle";
                        status.error = Some(error);
                    });
                }
            }
        } else if manual && ready.is_some() {
            say("The update will install when Koett is idle");
        }
        let Some((feed, archive)) = &ready else {
            continue;
        };
        if !reserve() {
            continue;
        }
        change(|status| status.state = "installing");
        let outcome = install(archive, feed, prefix).and_then(|new| {
            let mut state = load_state();
            state.previous = Some(current.to_owned());
            for id in [current, new.as_str()] {
                if !state.managed.iter().any(|known| known == id) {
                    state.managed.push(id.to_owned());
                }
            }
            prune(prefix, &mut state, &[current, new.as_str()]);
            save_state(&state)?;
            relaunch(prefix, actions)
        });
        let _ = std::fs::remove_file(archive);
        if let Err(error) = outcome {
            eprintln!("Update failed: {error}");
            say(&error);
            change(|status| {
                status.state = "idle";
                status.error = Some(error);
            });
            ready = None;
            RESERVED.store(false, Ordering::SeqCst);
            continue;
        }
        // Quitting takes it from here. If that is refused, don't stay blocked.
        std::thread::sleep(Duration::from_secs(30));
        RESERVED.store(false, Ordering::SeqCst);
        ready = None;
    }
}

/// Start a check now. False when updates are off in this build.
pub fn check_now() -> bool {
    COMMANDS.get().is_some_and(|commands| {
        commands
            .lock()
            .is_ok_and(|commands| commands.send(Command::Check).is_ok())
    })
}

/// `koett check-updates`, `update-status` and `rollback-update`.
pub fn control(command: &str) -> Option<serde_json::Value> {
    let disabled = || {
        serde_json::json!({"ok":false,"error":{"code":"updates_disabled",
            "message":"This build has no update feed"}})
    };
    match command {
        "update-status" => {
            let status = STATUS.lock().unwrap_or_else(|error| error.into_inner());
            let status = status.clone().unwrap_or_default();
            Some(serde_json::json!({
                "ok": true,
                "enabled": status.enabled,
                "state": if status.state.is_empty() { "disabled" } else { status.state },
                "build": config().map(|config| config.build),
                "version": env!("CARGO_PKG_VERSION"),
                "available_version": status.available_version,
                "available_build": status.available_build,
                "error": status.error,
                "rollback": load_state().previous,
            }))
        }
        "check-updates" => Some(if check_now() {
            serde_json::json!({"ok":true,"status":"checking"})
        } else {
            disabled()
        }),
        "rollback-update" => Some(match COMMANDS.get() {
            None => disabled(),
            Some(commands) => {
                let (reply, answer) = mpsc::channel();
                let sent = commands
                    .lock()
                    .is_ok_and(|commands| commands.send(Command::Rollback(reply)).is_ok());
                if sent {
                    answer
                        .recv_timeout(Duration::from_secs(10))
                        .unwrap_or_else(|_| {
                            serde_json::json!({"ok":false,"error":{"code":"busy",
                                "message":"The updater is busy; try again"}})
                        })
                } else {
                    disabled()
                }
            }
        }),
        _ => None,
    }
}

/// The build this binary was made as, read from a package before it's installed.
pub fn print_build() {
    println!("{}", BUILD.unwrap_or("0"));
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    const FEED_URL: Option<&str> = Some("https://example.com/feed.json");

    #[test]
    fn a_build_without_all_three_settings_is_inert() {
        assert!(parse_config(None, None, None, false).is_none());
        assert!(parse_config(FEED_URL, None, Some("3"), false).is_none());
        assert!(parse_config(FEED_URL, Some(KEY), None, false).is_none());
        assert!(parse_config(None, Some(KEY), Some("3"), false).is_none());
        assert!(parse_config(FEED_URL, Some(KEY), Some("3"), false).is_some());
    }

    #[test]
    fn bad_settings_are_inert_too() {
        assert!(parse_config(FEED_URL, Some("short"), Some("3"), false).is_none());
        assert!(parse_config(FEED_URL, Some(KEY), Some("0"), false).is_none());
        assert!(parse_config(FEED_URL, Some(KEY), Some("x"), false).is_none());
        let lab = Some("http://127.0.0.1:9/feed.json");
        assert!(parse_config(lab, Some(KEY), Some("3"), false).is_none());
        assert!(parse_config(lab, Some(KEY), Some("3"), true).is_some());
        assert!(parse_config(Some("http://example.com/f"), Some(KEY), Some("3"), true).is_none());
    }

    #[test]
    fn only_an_installed_version_tree_can_be_replaced() {
        assert_eq!(
            install_location(Path::new(
                "/home/a/.local/lib/koett/versions/abc/libexec/koett"
            )),
            Some((PathBuf::from("/home/a/.local"), "abc".to_owned()))
        );
        for path in [
            "/home/a/dev/koett/target/release/koett",
            "/usr/bin/koett",
            "/lib/koett/versions/abc/koett",
        ] {
            assert_eq!(install_location(Path::new(path)), None, "{path}");
        }
    }

    #[test]
    fn only_a_newer_build_that_was_not_rolled_back_is_offered() {
        let feed = |build| Feed {
            schema: 1,
            version: "1".into(),
            build,
            arch: "x".into(),
            url: "u".into(),
            length: 1,
            sha256: String::new(),
            signature: String::new(),
        };
        assert!(offered(&feed(8), 7, 0));
        assert!(!offered(&feed(7), 7, 0));
        assert!(!offered(&feed(6), 7, 0));
        assert!(!offered(&feed(8), 7, 8));
        assert!(offered(&feed(9), 7, 8));
    }

    #[test]
    fn old_versions_go_except_the_two_newest_and_any_in_use() {
        let ids = |list: &[&str]| list.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
        assert!(removable(&ids(&["a", "b"]), &[]).is_empty());
        assert_eq!(removable(&ids(&["a", "b", "c"]), &[]), ["a"]);
        assert!(removable(&ids(&["a", "b", "c"]), &["a"]).is_empty());
    }

    #[test]
    fn the_replaced_version_leaves_the_next_ones_paths() {
        let root = Path::new("/p/lib/koett/versions/old");
        let list = std::ffi::OsStr::new("/p/lib/koett/versions/old/runtime/bin:/usr/bin:/bin");
        assert_eq!(without_root(list, root).unwrap(), "/usr/bin:/bin");
        let only = std::ffi::OsStr::new("/p/lib/koett/versions/old/runtime/gtk/share");
        assert_eq!(without_root(only, root), None);
        // A sibling whose name merely starts the same stays.
        let sibling = std::ffi::OsStr::new("/p/lib/koett/versions/older/bin");
        assert_eq!(without_root(sibling, root).unwrap(), sibling);
    }

    #[test]
    fn a_hung_step_is_stopped() {
        let started = std::time::Instant::now();
        let status = bounded(Duration::from_secs(1), "sleep")
            .arg("30")
            .status()
            .unwrap();
        assert!(!status.success());
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_folder_this_user_cannot_write_is_not_replaceable() {
        assert!(writable(&std::env::temp_dir()));
        assert!(!writable(Path::new("/no/such/folder")));
    }

    #[test]
    fn a_failed_reservation_leaves_the_app_free() {
        super::super::QUIT_BUSY.store(true, Ordering::SeqCst);
        assert!(!reserve());
        assert!(admit().is_some());
        super::super::QUIT_BUSY.store(false, Ordering::SeqCst);
    }
}
