//! Which window is under the pointer on niri. Its IPC leaves
//! `tile_pos_in_workspace_view` empty for tiled windows, so positions cannot
//! locate one; `PickWindow` asks the compositor instead. A pick finishes on the
//! next click, which the virtual pointer supplies, so the user sees nothing.
use super::wl_click::VirtualPointer;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Sent after this long from the request, in case an earlier click arrived
/// before the compositor had started the pick.
const CLICKS_AT: [Duration; 3] = [
    Duration::from_millis(25),
    Duration::from_millis(120),
    Duration::from_millis(240),
];
const PICK_DEADLINE: Duration = Duration::from_millis(500);
const WINDOWS_TIMEOUT: Duration = Duration::from_millis(200);

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

/// Clicks on schedule until the pick is answered. `None` when it never is.
fn pick(socket: &Path, pointer: &mut VirtualPointer) -> Option<String> {
    let reader = request(socket, "PickWindow", PICK_DEADLINE).ok()?;
    let started = Instant::now();
    let (sender, answer) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(read_reply(reader));
    });
    for click_at in CLICKS_AT {
        let wait = click_at.saturating_sub(started.elapsed());
        match answer.recv_timeout(wait) {
            Ok(reply) => return reply.ok(),
            Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Err(error) = pointer.click() {
            eprintln!("Pin window lookup: {error}");
        }
    }
    answer
        .recv_timeout(PICK_DEADLINE.saturating_sub(started.elapsed()))
        .ok()?
        .ok()
}

/// The window under the pointer, or `None` outside niri, without virtual
/// pointer support, or over no window. Never starts a pick it could not finish.
pub fn window_at_pointer() -> Option<NiriWindow> {
    let socket = std::env::var_os("NIRI_SOCKET")?;
    let socket = Path::new(&socket);
    let mut pointer = VirtualPointer::connect()
        .inspect_err(|error| eprintln!("Pin window lookup unavailable: {error}"))
        .ok()?;
    let mut window = parse_picked(&pick(socket, &mut pointer)?)?;
    window.last_focused = last_focused(socket) == Some(window.id);
    Some(window)
}

#[cfg(test)]
mod tests {
    use super::*;

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
