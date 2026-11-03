//! Which window is under the pointer on niri. Its IPC leaves
//! `tile_pos_in_workspace_view` empty for tiled windows, so positions cannot
//! locate one; `PickWindow` asks the compositor instead. A pick finishes on the
//! next click, which the virtual pointer supplies, so the user sees nothing.
//!
//! The overlay blocks a pick while it accepts input, and without the overlay a
//! click that beats the pick lands in the app. So the click waits for proof the
//! pick is running: its pointer grab sends the overlay a pointer leave. Only
//! then does the overlay open its input region. See [`PickGate`].
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

/// How the pick and the overlay take turns; shared with the UI thread.
#[derive(Default)]
pub struct PickGate(AtomicU8);

impl PickGate {
    /// UI thread, before the reader starts: the overlay keeps accepting input.
    pub fn begin(&self) {
        self.0.store(WAITING, Ordering::SeqCst);
    }

    pub fn finish(&self) {
        self.0.store(IDLE, Ordering::SeqCst);
    }

    pub fn busy(&self) -> bool {
        self.0.load(Ordering::SeqCst) != IDLE
    }

    /// The overlay lost the pointer; a pick that is waiting takes this as its grab.
    pub fn pointer_left(&self) {
        let _ = self
            .0
            .compare_exchange(WAITING, GRABBED, Ordering::SeqCst, Ordering::SeqCst);
    }

    /// The pointer came back, so that leave was the user crossing monitors.
    pub fn pointer_entered(&self) {
        let _ = self
            .0
            .compare_exchange(GRABBED, WAITING, Ordering::SeqCst, Ordering::SeqCst);
    }

    /// Whether the overlay must let the click through to the compositor.
    pub fn wants_open(&self) -> bool {
        matches!(self.0.load(Ordering::SeqCst), GRABBED | OPEN)
    }

    /// The overlay has asked for an empty input region.
    pub fn opened(&self) {
        let _ = self
            .0
            .compare_exchange(GRABBED, OPEN, Ordering::SeqCst, Ordering::SeqCst);
    }

    fn is_open(&self) -> bool {
        self.0.load(Ordering::SeqCst) == OPEN
    }
}

const GRAB_DEADLINE: Duration = Duration::from_millis(200);
/// The empty input region reaches the compositor with the overlay's next frame.
const REGION_SETTLES: Duration = Duration::from_millis(40);
const ANSWER_DEADLINE: Duration = Duration::from_millis(150);
const POLL: Duration = Duration::from_millis(4);
const WINDOWS_TIMEOUT: Duration = Duration::from_millis(200);
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
    /// The window the user focused most recently.
    pub last_focused: bool,
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
        last_focused: false,
    })
}

fn parse_picked(reply: &str) -> Option<NiriWindow> {
    let reply: Value = serde_json::from_str(reply).ok()?;
    parse_window(reply.get("Ok")?.get("PickedWindow")?)
}

fn focus_time(window: &Value) -> Option<(u64, u32)> {
    let stamp = window.get("focus_timestamp")?;
    let nanos = stamp.get("nanos")?.as_u64()?;
    Some((stamp.get("secs")?.as_u64()?, u32::try_from(nanos).ok()?))
}

/// The window with the greatest focus timestamp, from a `Windows` reply.
fn parse_last_focused(reply: &str) -> Option<u64> {
    let reply: Value = serde_json::from_str(reply).ok()?;
    reply
        .get("Ok")?
        .get("Windows")?
        .as_array()?
        .iter()
        .filter_map(|window| Some((focus_time(window)?, window.get("id")?.as_u64()?)))
        .max_by_key(|(time, _)| *time)
        .map(|(_, id)| id)
}

fn request(socket: &Path, name: &str, timeout: Duration) -> Result<BufReader<UnixStream>, String> {
    let mut stream = UnixStream::connect(socket).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|error| error.to_string())?;
    stream
        .write_all(format!("\"{name}\"\n").as_bytes())
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

fn last_focused(socket: &Path) -> Option<u64> {
    let reply = read_reply(request(socket, "Windows", WINDOWS_TIMEOUT).ok()?).ok()?;
    parse_last_focused(&reply)
}

/// Clicks once the pick is proven to be running. When it never proves itself,
/// the overlay still accepts input, so the cleanup clicks can reach no app.
fn pick(socket: &Path, pointer: &mut VirtualPointer, gate: &PickGate) -> Option<String> {
    let reader = request(socket, "PickWindow", REQUEST_TIMEOUT).ok()?;
    let (sender, answer) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(read_reply(reader));
    });
    let started = Instant::now();
    while !gate.is_open() {
        if started.elapsed() >= GRAB_DEADLINE {
            eprintln!("Pin window lookup: niri did not start the pick");
            let _ = pointer.click();
            let _ = answer.recv_timeout(ANSWER_DEADLINE);
            let _ = pointer.click();
            return None;
        }
        std::thread::sleep(POLL);
    }
    std::thread::sleep(REGION_SETTLES);
    if let Err(error) = pointer.click() {
        eprintln!("Pin window lookup: {error}");
        return None;
    }
    answer.recv_timeout(ANSWER_DEADLINE).ok()?.ok()
}

/// The window under the pointer, or `None` outside niri, without virtual
/// pointer support, or over no window. Never starts a pick it could not finish.
pub fn window_at_pointer(gate: &PickGate) -> Option<NiriWindow> {
    let socket = std::env::var_os("NIRI_SOCKET")?;
    let socket = Path::new(&socket);
    let mut pointer = VirtualPointer::connect()
        .inspect_err(|error| eprintln!("Pin window lookup unavailable: {error}"))
        .ok()?;
    let reply = pick(socket, &mut pointer, gate);
    gate.finish();
    let mut window = parse_picked(&reply?)?;
    window.last_focused = last_focused(socket) == Some(window.id);
    Some(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_opens_only_after_the_pick_took_the_pointer() {
        let gate = PickGate::default();
        gate.begin();
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
                last_focused: false,
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

    #[test]
    fn last_focused_compares_seconds_then_nanoseconds() {
        let reply = r#"{"Ok":{"Windows":[
            {"id":1,"focus_timestamp":{"secs":5,"nanos":900}},
            {"id":2,"focus_timestamp":{"secs":6,"nanos":1}},
            {"id":3,"focus_timestamp":{"secs":6,"nanos":2}},
            {"id":4,"focus_timestamp":null}
        ]}}"#;
        assert_eq!(parse_last_focused(reply), Some(3));
    }

    #[test]
    fn no_focus_timestamps_mean_no_last_focused_window() {
        assert_eq!(parse_last_focused(r#"{"Ok":{"Windows":[{"id":1}]}}"#), None);
    }
}
