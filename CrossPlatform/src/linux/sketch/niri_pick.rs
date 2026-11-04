//! Which window is under the pointer on niri. Its IPC leaves
//! `tile_pos_in_workspace_view` empty for tiled windows, so positions cannot
//! locate one; `PickWindow` asks the compositor instead. A pick finishes on the
//! next click, which the virtual pointer supplies, so the user sees nothing.
//!
//! The overlay blocks a pick where it accepts input, and without the overlay a
//! click that beats the pick lands in the app. So the click waits for proof the
//! pick is running: its pointer grab sends the overlay a pointer leave. Only then
//! does the overlay open a small hole at the pin. A pointer that already moved on
//! is outside the hole, so the pick finds nothing rather than the wrong window,
//! and a real click anywhere else still lands on the overlay. See [`PickGate`].
use super::wl_click::VirtualPointer;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const IDLE: u8 = 0;
const WAITING: u8 = 1;
const GRABBED: u8 = 2;
const OPEN: u8 = 3;
/// The overlay is taking input back, or never gave it up.
const CLOSING: u8 = 4;
/// The whole overlay is committed: a click can only land on it.
const CLOSED: u8 = 5;

/// How the pick and the overlay take turns; shared with the UI thread.
#[derive(Default)]
pub struct PickGate {
    phase: AtomicU8,
}

impl PickGate {
    /// UI thread, before the reader starts: the overlay keeps accepting input.
    pub fn begin(&self) {
        self.phase.store(WAITING, Ordering::SeqCst);
    }

    pub fn finish(&self) {
        self.phase.store(IDLE, Ordering::SeqCst);
    }

    pub fn busy(&self) -> bool {
        self.phase.load(Ordering::SeqCst) != IDLE
    }

    /// The overlay lost the pointer; a pick that is waiting takes this as its grab.
    pub fn pointer_left(&self) {
        self.advance(WAITING, GRABBED);
    }

    /// The pointer came back, so that leave was the user crossing monitors.
    pub fn pointer_entered(&self) {
        self.advance(GRABBED, WAITING);
    }

    /// Whether the overlay must let the click through to the compositor.
    pub fn wants_open(&self) -> bool {
        matches!(self.phase.load(Ordering::SeqCst), GRABBED | OPEN)
    }

    /// The overlay's empty input region has been committed.
    pub fn opened(&self) {
        self.advance(GRABBED, OPEN);
    }

    fn advance(&self, from: u8, to: u8) -> bool {
        self.phase
            .compare_exchange(from, to, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    fn is_open(&self) -> bool {
        self.phase.load(Ordering::SeqCst) == OPEN
    }

    /// Gives up only while the pick is unproven. Once it is proven, the grab
    /// owns the next click, so the normal path stays safe.
    fn abandon(&self) -> bool {
        self.advance(WAITING, CLOSING)
    }

    /// The overlay must take input back.
    fn close(&self) {
        self.phase.store(CLOSING, Ordering::SeqCst);
    }

    pub fn wants_closed(&self) -> bool {
        self.phase.load(Ordering::SeqCst) == CLOSING
    }

    /// The overlay's whole input region has been committed.
    pub fn closed(&self) {
        self.advance(CLOSING, CLOSED);
    }

    fn wait_closed(&self) {
        let started = Instant::now();
        while self.phase.load(Ordering::SeqCst) != CLOSED && started.elapsed() < CLOSE_DEADLINE {
            std::thread::sleep(POLL);
        }
    }
}

/// Every path out of a pick frees the gate, a panicking one included.
struct FinishGate<'a>(&'a PickGate);

impl Drop for FinishGate<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}

const GRAB_DEADLINE: Duration = Duration::from_millis(400);
const OPEN_DEADLINE: Duration = Duration::from_millis(200);
/// The hole is committed; this covers the trip to the compositor.
const REGION_SETTLES: Duration = Duration::from_millis(10);
const ANSWER_DEADLINE: Duration = Duration::from_millis(150);
/// A cleanup click's events must reach the overlay while the gate is still busy.
const CLEANUP_SETTLES: Duration = Duration::from_millis(60);
const CLOSE_DEADLINE: Duration = Duration::from_millis(400);
const POLL: Duration = Duration::from_millis(4);
const IPC_TIMEOUT: Duration = Duration::from_millis(200);
const REQUEST_TIMEOUT: Duration = Duration::from_millis(600);

#[derive(Clone, Debug, PartialEq)]
pub struct NiriWindow {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    /// May belong to xwayland-satellite rather than the app.
    pub pid: Option<i64>,
    /// Set only for floating windows.
    pub tile_pos: Option<(f64, f64)>,
    pub window_offset: (f64, f64),
    pub geometry: Option<super::pick_geometry::PickGeometry>,
    /// No other window has this title, so a title match can only mean this one.
    pub title_unique: bool,
}

/// What keyboard focus was on before the pick's click moved it.
#[derive(Debug, PartialEq)]
enum Focus {
    Window(u64),
    Monitor(String),
}

impl Focus {
    fn restore_request(&self) -> String {
        match self {
            Self::Window(id) => {
                serde_json::json!({"Action": {"FocusWindow": {"id": id}}}).to_string()
            }
            Self::Monitor(output) => {
                serde_json::json!({"Action": {"FocusMonitor": {"output": output}}}).to_string()
            }
        }
    }
}

fn pair(value: &Value) -> Option<(f64, f64)> {
    Some((value.get(0)?.as_f64()?, value.get(1)?.as_f64()?))
}

fn parse_window(value: &Value) -> Option<NiriWindow> {
    let layout = value.get("layout");
    Some(NiriWindow {
        id: value.get("id")?.as_u64()?,
        title: value.get("title")?.as_str().unwrap_or_default().to_owned(),
        app_id: value.get("app_id")?.as_str().unwrap_or_default().to_owned(),
        pid: value.get("pid").and_then(Value::as_i64),
        tile_pos: layout
            .and_then(|layout| layout.get("tile_pos_in_workspace_view"))
            .and_then(pair),
        window_offset: layout
            .and_then(|layout| layout.get("window_offset_in_tile"))
            .and_then(pair)
            .unwrap_or_default(),
        geometry: value
            .get("koett_geometry_v1")
            .and_then(|value| serde_json::from_value(value.clone()).ok()),
        title_unique: false,
    })
}

pub(super) fn parse_picked(reply: &str) -> Option<NiriWindow> {
    let reply: Value = serde_json::from_str(reply).ok()?;
    parse_window(reply.get("Ok")?.get("PickedWindow")?)
}

/// While the overlay holds the keyboard, niri reports no focused window, so
/// focus means the focused workspace's active window.
fn parse_active_window(reply: &str) -> Option<u64> {
    let reply: Value = serde_json::from_str(reply).ok()?;
    reply
        .get("Ok")?
        .get("Workspaces")?
        .as_array()?
        .iter()
        .find(|workspace| workspace.get("is_focused").and_then(Value::as_bool) == Some(true))?
        .get("active_window_id")?
        .as_u64()
}

fn windows_titled(reply: &str, title: &str) -> Option<usize> {
    let reply: Value = serde_json::from_str(reply).ok()?;
    let windows = reply.get("Ok")?.get("Windows")?.as_array()?;
    Some(
        windows
            .iter()
            .filter(|window| window.get("title").and_then(Value::as_str) == Some(title))
            .count(),
    )
}

fn parse_focused_output(reply: &str) -> Option<String> {
    let reply: Value = serde_json::from_str(reply).ok()?;
    Some(
        reply
            .get("Ok")?
            .get("FocusedOutput")?
            .get("name")?
            .as_str()?
            .to_owned(),
    )
}

fn request(socket: &Path, line: &str, timeout: Duration) -> Result<BufReader<UnixStream>, String> {
    let mut stream = UnixStream::connect(socket).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|error| error.to_string())?;
    stream
        .write_all(format!("{line}\n").as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(BufReader::new(stream))
}

fn read_reply(mut reader: BufReader<UnixStream>) -> Result<String, String> {
    let mut reply = String::new();
    reader
        .read_line(&mut reply)
        .map_err(|error| error.to_string())?;
    Ok(reply)
}

fn ask(socket: &Path, line: &str) -> Option<String> {
    read_reply(request(socket, line, IPC_TIMEOUT).ok()?).ok()
}

fn current_focus(socket: &Path) -> Option<Focus> {
    if let Some(id) = parse_active_window(&ask(socket, "\"Workspaces\"")?) {
        return Some(Focus::Window(id));
    }
    parse_focused_output(&ask(socket, "\"FocusedOutput\"")?).map(Focus::Monitor)
}

/// Only for when the overlay accepts input: a repeated click lands there.
fn cleanup_click(pointer: &mut VirtualPointer) {
    let clicked = pointer.click().or_else(|first| {
        eprintln!("Pin window lookup: {first}");
        *pointer = VirtualPointer::connect()?;
        pointer.click()
    });
    if let Err(error) = clicked {
        eprintln!("Pin window lookup: {error}");
    }
}

/// Ends a pick left running, which would otherwise eat the next real click.
fn end_pick(pointer: &mut VirtualPointer, answer: &mpsc::Receiver<Result<String, String>>) {
    cleanup_click(pointer);
    if answer.recv_timeout(ANSWER_DEADLINE).is_err() {
        cleanup_click(pointer);
    }
}

/// Clicks once the pick is proven to be running. When it never proves itself,
/// the overlay still accepts input, so the cleanup clicks can reach no app.
/// Focus goes back the moment a click has landed, before waiting for the answer.
fn pick(
    socket: &Path,
    pointer: &mut VirtualPointer,
    gate: &PickGate,
    focus: &Focus,
) -> Option<String> {
    let restore = || {
        if ask(socket, &focus.restore_request()).is_none() {
            eprintln!("Pin window lookup: could not restore focus");
        }
    };
    let answer = start_pick(socket)?;
    let started = Instant::now();
    while !gate.is_open() {
        let waited = started.elapsed();
        if waited >= GRAB_DEADLINE + OPEN_DEADLINE {
            break;
        }
        if waited >= GRAB_DEADLINE && gate.abandon() {
            eprintln!("Pin window lookup: niri did not start the pick");
            end_pick(pointer, &answer);
            restore();
            std::thread::sleep(CLEANUP_SETTLES);
            return None;
        }
        std::thread::sleep(POLL);
    }
    std::thread::sleep(REGION_SETTLES);
    // A real press may already have ended the pick, and another click would then
    // reach the app. Its answer takes a moment to arrive, so wait for niri to
    // answer something newer first.
    let _ = ask(socket, "\"Version\"");
    std::thread::sleep(POLL);
    if let Ok(reply) = answer.try_recv() {
        restore();
        return reply.ok();
    }
    // Never repeated while the overlay is open: a click that finished the pick
    // but lost its acknowledgement would send the repeat into the app.
    // Never repeated while the hole is open: a click that finished the pick but
    // lost its acknowledgement would send the repeat into the app.
    let clicked = pointer.click();
    // The hole stays open for a moment, and a real press landing there would reach
    // the app, as the second click of a double click can. A second pick takes every
    // press until the hole has closed; a click on the whole overlay then ends it.
    let shield = start_pick(socket);
    gate.close();
    restore();
    if let Err(error) = &clicked {
        eprintln!("Pin window lookup: {error}");
    }
    let reply = answer.recv_timeout(ANSWER_DEADLINE);
    gate.wait_closed();
    if reply.is_err() && answer.try_recv().is_err() {
        eprintln!("Pin window lookup: the pick did not answer");
        end_pick(pointer, &answer);
    }
    if let Some(shield) = shield
        && shield.try_recv().is_err()
    {
        end_pick(pointer, &shield);
    }
    restore();
    std::thread::sleep(CLEANUP_SETTLES);
    reply.ok()?.ok()
}

fn start_pick(socket: &Path) -> Option<mpsc::Receiver<Result<String, String>>> {
    let reader = request(socket, "\"PickWindow\"", REQUEST_TIMEOUT).ok()?;
    let (sender, answer) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(read_reply(reader));
    });
    Some(answer)
}

/// The window under the pointer, or `None` outside niri, without virtual
/// pointer support, or over no window. Never starts a pick it could not finish,
/// and always hands focus back: niri focuses what a press lands on, even during
/// a pick, and the paste must reach the window being dictated into.
pub fn window_at_pointer(gate: &PickGate) -> Option<NiriWindow> {
    let _finish = FinishGate(gate);
    let socket = std::env::var_os("NIRI_SOCKET")?;
    let socket = Path::new(&socket);
    let mut pointer = VirtualPointer::connect()
        .inspect_err(|error| eprintln!("Pin window lookup unavailable: {error}"))
        .ok()?;
    let focus = current_focus(socket)?;
    let mut window =
        pick(socket, &mut pointer, gate, &focus).and_then(|reply| parse_picked(&reply))?;
    window.title_unique = ask(socket, "\"Windows\"")
        .is_some_and(|reply| windows_titled(&reply, &window.title) == Some(1));
    Some(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waiting() -> PickGate {
        let gate = PickGate::default();
        gate.begin();
        gate
    }

    #[test]
    fn the_gate_opens_only_after_the_pick_took_the_pointer() {
        let gate = waiting();
        assert!(gate.busy() && !gate.wants_open());
        gate.opened();
        assert!(!gate.wants_open(), "no leave yet");
        gate.pointer_left();
        assert!(gate.wants_open());
        gate.pointer_entered();
        assert!(!gate.wants_open(), "the user crossed monitors");
        gate.pointer_left();
        gate.opened();
        assert!(gate.is_open());
        gate.pointer_entered();
        assert!(gate.is_open(), "an open gate stays open until finished");
        gate.finish();
        assert!(!gate.busy());
    }

    #[test]
    fn an_idle_gate_ignores_pointer_crossings() {
        let gate = PickGate::default();
        gate.pointer_left();
        assert!(!gate.busy() && !gate.wants_open());
    }

    #[test]
    fn an_abandoned_pick_keeps_the_overlay_accepting_input() {
        let gate = waiting();
        assert!(gate.abandon());
        gate.pointer_left();
        assert!(gate.busy() && !gate.wants_open());
    }

    #[test]
    fn a_proven_pick_cannot_be_abandoned() {
        let gate = waiting();
        gate.pointer_left();
        assert!(!gate.abandon());
        assert!(gate.wants_open());
    }

    #[test]
    fn a_late_reader_cannot_free_the_next_pin() {
        let gate = PickGate::default();
        drop(FinishGate(&gate));
        gate.begin();
        assert!(gate.busy());
    }

    #[test]
    fn focus_goes_back_to_the_window_or_the_monitor() {
        assert_eq!(
            Focus::Window(7).restore_request(),
            r#"{"Action":{"FocusWindow":{"id":7}}}"#
        );
        assert_eq!(
            Focus::Monitor("DP-3".into()).restore_request(),
            r#"{"Action":{"FocusMonitor":{"output":"DP-3"}}}"#
        );
    }

    #[test]
    fn focus_is_the_focused_workspaces_active_window() {
        let reply = r#"{"Ok":{"Workspaces":[
            {"id":1,"is_focused":false,"active_window_id":9},
            {"id":2,"is_focused":true,"active_window_id":4}]}}"#;
        assert_eq!(parse_active_window(reply), Some(4));
        let empty = r#"{"Ok":{"Workspaces":[{"id":2,"is_focused":true,"active_window_id":null}]}}"#;
        assert_eq!(parse_active_window(empty), None);
    }

    #[test]
    fn a_shared_title_is_counted_across_all_windows() {
        let reply = r#"{"Ok":{"Windows":[{"title":"~"},{"title":"~"},{"title":"Docs"}]}}"#;
        assert_eq!(windows_titled(reply, "~"), Some(2));
        assert_eq!(windows_titled(reply, "Docs"), Some(1));
        assert_eq!(windows_titled("{}", "Docs"), None);
    }

    #[test]
    fn closing_takes_input_back_from_an_open_gate() {
        let gate = waiting();
        gate.pointer_left();
        gate.opened();
        gate.close();
        assert!(gate.busy() && !gate.wants_open() && gate.wants_closed());
        gate.closed();
        assert!(gate.busy() && !gate.wants_closed());
    }

    #[test]
    fn an_empty_workspace_falls_back_to_its_monitor() {
        assert_eq!(
            parse_focused_output(r#"{"Ok":{"FocusedOutput":{"name":"DP-3"}}}"#).as_deref(),
            Some("DP-3")
        );
        assert_eq!(
            parse_focused_output(r#"{"Ok":{"FocusedOutput":null}}"#),
            None
        );
    }

    const WINDOW: &str = r#"{
        "id": 100, "title": "New Tab - Google Chrome", "app_id": "Google-chrome",
        "pid": 2233, "workspace_id": 1, "is_focused": false, "is_floating": false,
        "is_urgent": false,
        "layout": {
            "pos_in_scrolling_layout": [3, 1], "tile_size": [1262.0, 1374.0],
            "window_size": [1262, 1374], "tile_pos_in_workspace_view": null,
            "window_offset_in_tile": [0.0, 0.0]
        },
        "focus_timestamp": {"secs": 288642, "nanos": 419489834}
    }"#;

    #[test]
    fn a_tiled_window_has_no_position() {
        let window = parse_picked(&format!(r#"{{"Ok":{{"PickedWindow":{WINDOW}}}}}"#)).unwrap();
        assert_eq!(
            window,
            NiriWindow {
                id: 100,
                title: "New Tab - Google Chrome".into(),
                app_id: "Google-chrome".into(),
                pid: Some(2233),
                tile_pos: None,
                window_offset: (0.0, 0.0),
                geometry: None,
                title_unique: false,
            }
        );
    }

    #[test]
    fn a_floating_window_has_its_position_and_offset() {
        let value: Value = serde_json::from_str(
            r#"{"id":1,"title":"t","app_id":"a","pid":null,"layout":{
                "tile_pos_in_workspace_view":[10.5,20.0],"window_offset_in_tile":[2.0,3.0]}}"#,
        )
        .unwrap();
        let window = parse_window(&value).unwrap();
        assert_eq!(window.tile_pos, Some((10.5, 20.0)));
        assert_eq!(window.window_offset, (2.0, 3.0));
        assert_eq!(window.pid, None);
    }

    #[test]
    fn a_window_may_have_no_title_or_app_id() {
        let value: Value =
            serde_json::from_str(r#"{"id":1,"title":null,"app_id":null,"pid":null}"#).unwrap();
        let window = parse_window(&value).unwrap();
        assert_eq!((window.title.as_str(), window.app_id.as_str()), ("", ""));
    }

    #[test]
    fn a_click_over_no_window_picks_nothing() {
        assert_eq!(parse_picked(r#"{"Ok":{"PickedWindow":null}}"#), None);
    }

    #[test]
    fn an_error_reply_picks_nothing() {
        assert_eq!(parse_picked(r#"{"Err":"nope"}"#), None);
        assert_eq!(parse_picked("not json"), None);
    }
}
