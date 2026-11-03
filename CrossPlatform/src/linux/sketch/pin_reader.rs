//! Reads a pin's context, once, when a pin is dropped: the window from niri,
//! then the page, the element under the pointer and the selection through
//! AT-SPI, then PRIMARY as the selection fallback. Every failure degrades to
//! the fields already known, so a pin on any window has at least app and title.
use super::niri_pick::{self, NiriWindow, PickGate};
use crate::pin_context::PinContext;
use ashpd::zbus::{
    self, Connection,
    zvariant::{OwnedObjectPath, OwnedValue, Type},
};
use serde::de::DeserializeOwned;
use std::collections::{HashMap, VecDeque};
use std::io::Read as _;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

type Object = (String, OwnedObjectPath);

const TOTAL: Duration = Duration::from_millis(800);
const ACCESSIBILITY: Duration = Duration::from_millis(500);
const PRIMARY: Duration = Duration::from_millis(300);
const PRIMARY_BYTES: u64 = 8 * 1024;
const MAX_APPS: usize = 64;
const MAX_FRAMES: usize = 32;
const MAX_NODES: usize = 512;
const MAX_CHILDREN: usize = 128;
const MAX_PARENTS: usize = 32;

const ROOT: &str = "/org/a11y/atspi/accessible/root";
const NULL_OBJECT: &str = "/org/a11y/atspi/null";
const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
const TEXT: &str = "org.a11y.atspi.Text";
const COMPONENT: &str = "org.a11y.atspi.Component";
const DOCUMENT: &str = "org.a11y.atspi.Document";
const HYPERLINK: &str = "org.a11y.atspi.Hyperlink";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

const ROLE_IMAGE: u32 = 27;
const ROLE_PASSWORD: u32 = 40;
const ROLE_LINK: u32 = 88;
const ROLE_DOCUMENT_WEB: u32 = 95;
const STATE_FOCUSED: u32 = 12;
const STATE_SHOWING: u32 = 25;
const COORDS_WINDOW: u32 = 1;

/// Blocking; call on a worker thread. `surface_point` is the click in the
/// overlay's monitor-local coordinates.
pub fn read(
    include_selection: bool,
    surface_point: (f64, f64),
    gate: &PickGate,
) -> Option<PinContext> {
    let deadline = Instant::now() + TOTAL;
    let window = niri_pick::window_at_pointer(gate)?;
    if is_own_window(&window.app_id, window.pid) {
        return None;
    }
    let app = app_name(&window.app_id);
    let mut context = PinContext {
        title: Some(page_title(&window.title, &app)),
        app: Some(app),
        ..PinContext::default()
    };
    let found = accessible(&window, include_selection, surface_point, deadline);
    context.url = found.url;
    context.link = found.link;
    context.image = found.image;
    if let Some(title) = found.title {
        context.title = Some(title);
    }
    context.selection = found.selection;
    if include_selection && context.selection.is_none() && window.last_focused && !found.password {
        context.selection = primary_selection(remaining(deadline).min(PRIMARY));
    }
    Some(context)
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn is_own_window(app_id: &str, pid: Option<i64>) -> bool {
    app_id.to_ascii_lowercase().contains("koett") || pid == Some(i64::from(std::process::id()))
}

/// `org.gnome.Nautilus` → `Nautilus`, `Google-chrome` → `Google Chrome`.
fn app_name(app_id: &str) -> String {
    app_id
        .rsplit('.')
        .next()
        .unwrap_or(app_id)
        .split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn page_title(raw: &str, app: &str) -> String {
    raw.strip_suffix(&format!(" - {app}"))
        .unwrap_or(raw)
        .to_owned()
}

fn web_url(text: &str) -> Option<String> {
    let parsed = url::Url::parse(text).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| text.to_owned())
}

fn document_url(attributes: &HashMap<String, String>) -> Option<String> {
    attributes
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("docurl"))
        .and_then(|(_, value)| web_url(value))
}

/// Chromium puts U+FFFC in a container's text for each embedded child.
fn selection_text(raw: &str) -> Option<String> {
    let text = raw.replace('\u{FFFC}', "");
    let meaningful = text.trim();
    (!meaningful.is_empty()).then(|| meaningful.to_owned())
}

/// GTK can publish a password entry's selection as its bullets.
fn only_bullets(text: &str) -> bool {
    text.chars()
        .all(|character| character.is_whitespace() || matches!(character, '•' | '●' | '*'))
}

fn window_point(surface: (f64, f64), tile: (f64, f64), offset: (f64, f64)) -> (i32, i32) {
    (
        (surface.0 - tile.0 - offset.0).round() as i32,
        (surface.1 - tile.1 - offset.1).round() as i32,
    )
}

fn has_state(words: &[u32], bit: u32) -> bool {
    words
        .get((bit / 32) as usize)
        .is_some_and(|word| word & (1 << (bit % 32)) != 0)
}

fn primary_selection(limit: Duration) -> Option<String> {
    let mut child = Command::new("wl-paste")
        .args(["--primary", "--no-newline", "--type", "text"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = stdout.take(PRIMARY_BYTES).read_to_end(&mut bytes);
        let _ = sender.send(read.map(|_| bytes));
    });
    let bytes = receiver.recv_timeout(limit);
    let _ = child.kill();
    let _ = child.wait();
    let text = String::from_utf8_lossy(&bytes.ok()?.ok()?).into_owned();
    (!text.trim().is_empty() && !only_bullets(&text)).then_some(text)
}

#[derive(Default)]
struct Found {
    url: Option<String>,
    title: Option<String>,
    link: Option<String>,
    image: Option<String>,
    selection: Option<String>,
    password: bool,
}

fn accessible(
    window: &NiriWindow,
    include_selection: bool,
    surface_point: (f64, f64),
    deadline: Instant,
) -> Found {
    let point = window
        .tile_pos
        .map(|tile| window_point(surface_point, tile, window.window_offset));
    let limit = remaining(deadline).min(ACCESSIBILITY);
    let mut found = Found::default();
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return found;
    };
    runtime.block_on(async {
        // Partial results stay in `found` when the budget runs out.
        let read = fill(&window.title, include_selection, point, &mut found);
        let _ = tokio::time::timeout(limit, read).await;
    });
    found
}

async fn fill(
    title: &str,
    include_selection: bool,
    point: Option<(i32, i32)>,
    found: &mut Found,
) -> zbus::Result<()> {
    let bus = accessibility_bus().await?;
    let Some(frame) = frame_titled(&bus, title).await? else {
        return Ok(());
    };
    let scan = scan_frame(&bus, &frame, include_selection, found).await;
    if let Some(point) = point {
        under_pointer(&bus, &frame, point, found).await;
    }
    if include_selection && !found.password {
        found.selection = read_selection(&bus, &scan).await;
    }
    Ok(())
}

async fn accessibility_bus() -> zbus::Result<Connection> {
    let session = Connection::session().await?;
    let reply = session
        .call_method(
            Some("org.a11y.Bus"),
            "/org/a11y/bus",
            Some("org.a11y.Bus"),
            "GetAddress",
            &(),
        )
        .await?;
    let address: String = reply.body().deserialize()?;
    zbus::connection::Builder::address(address.as_str())?
        .build()
        .await
}

async fn call<B, R>(
    bus: &Connection,
    object: &Object,
    interface: &str,
    method: &str,
    body: &B,
) -> zbus::Result<R>
where
    B: serde::Serialize + Type,
    R: DeserializeOwned + Type,
{
    let reply = bus
        .call_method(
            Some(object.0.as_str()),
            object.1.as_str(),
            Some(interface),
            method,
            body,
        )
        .await?;
    reply.body().deserialize()
}

async fn property(bus: &Connection, object: &Object, name: &str) -> zbus::Result<OwnedValue> {
    call(bus, object, PROPERTIES, "Get", &(ACCESSIBLE, name)).await
}

async fn name(bus: &Connection, object: &Object) -> Option<String> {
    String::try_from(property(bus, object, "Name").await.ok()?).ok()
}

async fn parent(bus: &Connection, object: &Object) -> Option<Object> {
    let value = property(bus, object, "Parent").await.ok()?;
    let parent = <(String, OwnedObjectPath)>::try_from(value).ok()?;
    (parent.1.as_str() != NULL_OBJECT).then_some(parent)
}

async fn children(bus: &Connection, object: &Object) -> zbus::Result<Vec<Object>> {
    call(bus, object, ACCESSIBLE, "GetChildren", &()).await
}

async fn role(bus: &Connection, object: &Object) -> zbus::Result<u32> {
    call(bus, object, ACCESSIBLE, "GetRole", &()).await
}

async fn implements(bus: &Connection, object: &Object, interface: &str) -> bool {
    call::<_, Vec<String>>(bus, object, ACCESSIBLE, "GetInterfaces", &())
        .await
        .is_ok_and(|names| names.iter().any(|name| name == interface))
}

/// niri reports xwayland-satellite's pid for X11 apps, so the frame is matched
/// by its exact title rather than by process.
async fn frame_titled(bus: &Connection, title: &str) -> zbus::Result<Option<Object>> {
    let root = (
        "org.a11y.atspi.Registry".to_owned(),
        OwnedObjectPath::try_from(ROOT)?,
    );
    for app in children(bus, &root).await?.into_iter().take(MAX_APPS) {
        let Ok(frames) = children(bus, &app).await else {
            continue;
        };
        for frame in frames.into_iter().take(MAX_FRAMES) {
            if name(bus, &frame).await.as_deref() == Some(title) {
                return Ok(Some(frame));
            }
        }
    }
    Ok(None)
}

#[derive(Default)]
struct Scan {
    focused: Option<Object>,
    showing: Vec<Object>,
}

/// Breadth-first over what is on screen, so hidden tabs are never visited.
async fn scan_frame(
    bus: &Connection,
    frame: &Object,
    include_selection: bool,
    found: &mut Found,
) -> Scan {
    let mut scan = Scan::default();
    let mut pending = VecDeque::from([frame.clone()]);
    let mut visited = 0;
    while let Some(object) = pending.pop_front() {
        visited += 1;
        if visited > MAX_NODES {
            break;
        }
        let Ok(state) = call::<_, Vec<u32>>(bus, &object, ACCESSIBLE, "GetState", &()).await else {
            continue;
        };
        if &object != frame && !has_state(&state, STATE_SHOWING) {
            continue;
        }
        let object_role = role(bus, &object).await.unwrap_or(0);
        let focused = has_state(&state, STATE_FOCUSED);
        if object_role == ROLE_PASSWORD && (focused || selected_text(bus, &object).await.is_some())
        {
            found.password = true;
        }
        if object_role == ROLE_DOCUMENT_WEB && found.url.is_none() {
            read_document(bus, &object, found).await;
        }
        if focused && scan.focused.is_none() {
            scan.focused = Some(object.clone());
        }
        if !include_selection && found.url.is_some() {
            break;
        }
        scan.showing.push(object.clone());
        if let Ok(descendants) = children(bus, &object).await {
            pending.extend(descendants.into_iter().take(MAX_CHILDREN));
        }
    }
    scan
}

async fn read_document(bus: &Connection, object: &Object, found: &mut Found) {
    if !implements(bus, object, DOCUMENT).await {
        return;
    }
    let attributes: zbus::Result<HashMap<String, String>> =
        call(bus, object, DOCUMENT, "GetAttributes", &()).await;
    let Some(url) = attributes.ok().as_ref().and_then(document_url) else {
        return;
    };
    found.url = Some(url);
    if let Some(title) = name(bus, object).await.filter(|title| !title.is_empty()) {
        found.title = Some(title);
    }
}

/// Needs window coordinates, which niri reports only for floating windows.
async fn under_pointer(bus: &Connection, frame: &Object, point: (i32, i32), found: &mut Found) {
    let hit: zbus::Result<Object> = call(
        bus,
        frame,
        COMPONENT,
        "GetAccessibleAtPoint",
        &(point.0, point.1, COORDS_WINDOW),
    )
    .await;
    let mut current = hit.ok().filter(|hit| hit.1.as_str() != NULL_OBJECT);
    for _ in 0..MAX_PARENTS {
        let Some(object) = current else {
            return;
        };
        match role(bus, &object).await {
            Ok(ROLE_LINK) if found.link.is_none() => {
                let uri: zbus::Result<String> =
                    call(bus, &object, HYPERLINK, "GetURI", &(0_i32,)).await;
                found.link = uri.ok().as_deref().and_then(web_url);
            }
            Ok(ROLE_IMAGE) if found.image.is_none() => {
                found.image = name(bus, &object).await.filter(|name| !name.is_empty());
            }
            Ok(ROLE_DOCUMENT_WEB) => return,
            _ => {}
        }
        if &object == frame {
            return;
        }
        current = parent(bus, &object).await;
    }
}

async fn selected_text(bus: &Connection, object: &Object) -> Option<String> {
    let count: i32 = call(bus, object, TEXT, "GetNSelections", &()).await.ok()?;
    if count < 1 {
        return None;
    }
    let (start, end): (i32, i32) = call(bus, object, TEXT, "GetSelection", &(0_i32,))
        .await
        .ok()?;
    if start >= end {
        return None;
    }
    let text: String = call(bus, object, TEXT, "GetText", &(start, end))
        .await
        .ok()?;
    selection_text(&text)
}

/// The focused element first. A page selection often has no focused text node,
/// so the longest selection among what is on screen is next.
async fn read_selection(bus: &Connection, scan: &Scan) -> Option<String> {
    if let Some(focused) = &scan.focused
        && let Some(text) = selected_text(bus, focused).await
    {
        return Some(text);
    }
    let mut longest: Option<String> = None;
    for object in &scan.showing {
        if let Some(text) = selected_text(bus, object).await
            && longest.as_ref().is_none_or(|best| text.len() > best.len())
        {
            longest = Some(text);
        }
    }
    longest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_names_read_like_apps() {
        assert_eq!(app_name("Google-chrome"), "Google Chrome");
        assert_eq!(app_name("org.gnome.Nautilus"), "Nautilus");
        assert_eq!(app_name("md.obsidian.Obsidian"), "Obsidian");
        assert_eq!(app_name("kitty"), "Kitty");
    }

    #[test]
    fn title_loses_only_a_trailing_app_name() {
        assert_eq!(
            page_title("Docs - Rust - Google Chrome", "Google Chrome"),
            "Docs - Rust"
        );
        assert_eq!(page_title("Notes", "Obsidian"), "Notes");
    }

    #[test]
    fn koett_windows_are_never_pinned() {
        assert!(is_own_window("com.olledyberg.Koett", None));
        assert!(is_own_window("x", Some(i64::from(std::process::id()))));
        assert!(!is_own_window("Google-chrome", Some(1)));
    }

    #[test]
    fn only_web_addresses_become_links() {
        assert!(web_url("https://example.com/a").is_some());
        assert!(web_url("http://example.com/").is_some());
        assert!(web_url("file:///home/olle/secret.txt").is_none());
        assert!(web_url("data:text/html,x").is_none());
        assert!(web_url("not a url").is_none());
    }

    #[test]
    fn document_url_ignores_key_case() {
        let attributes = HashMap::from([("DocURL".to_owned(), "https://a.example/".to_owned())]);
        assert_eq!(
            document_url(&attributes).as_deref(),
            Some("https://a.example/")
        );
        assert_eq!(document_url(&HashMap::new()), None);
    }

    #[test]
    fn selection_drops_embedded_object_markers() {
        assert_eq!(selection_text("\u{FFFC} hello ").as_deref(), Some("hello"));
        assert_eq!(selection_text("\u{FFFC}\u{FFFC}"), None);
    }

    #[test]
    fn obscured_password_text_is_rejected() {
        assert!(only_bullets("••••"));
        assert!(only_bullets("● ●"));
        assert!(only_bullets("***"));
        assert!(!only_bullets("a*b"));
    }

    #[test]
    fn window_point_subtracts_tile_and_offset() {
        assert_eq!(
            window_point((110.4, 60.0), (100.0, 40.0), (0.0, 10.0)),
            (10, 10)
        );
    }

    #[test]
    fn state_bits_span_both_words() {
        assert!(has_state(&[1 << STATE_FOCUSED, 0], STATE_FOCUSED));
        assert!(has_state(&[0, 1 << 1], 33));
        assert!(!has_state(&[0], 33));
    }
}
