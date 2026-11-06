//! Reads one pin's context from a compositor-identified window or a unique
//! accessible frame with usable geometry. Missing metadata stays absent.
//! PRIMARY is never a selection fallback: it has no verified window owner.
use super::niri_pick::{self, NiriWindow, PickGate};
use super::video_time;
use crate::pin_context::PinContext;
use ashpd::zbus::{
    self, Connection,
    zvariant::{OwnedObjectPath, OwnedValue, Type},
};
use serde::de::DeserializeOwned;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

type Object = (String, OwnedObjectPath);

const TOTAL: Duration = Duration::from_millis(1300);
const ACCESSIBILITY: Duration = Duration::from_millis(1000);
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

const ROLE_SLIDER: u32 = 51;
const VALUE: &str = "org.a11y.atspi.Value";
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
    screen_point: (f64, f64),
    gate: &PickGate,
) -> Option<PinContext> {
    let deadline = Instant::now() + TOTAL;
    if std::env::var_os("NIRI_SOCKET").is_none() {
        gate.finish();
        return read_screen(include_selection, screen_point, deadline);
    }
    let window = niri_pick::window_at_pointer(gate)?;
    if is_own_window(&window.app_id, window.pid) {
        return None;
    }
    let app = app_name(&window.app_id);
    let chromium = is_chromium(&window.app_id);
    if private_title(&window.app_id, &window.title) || (chromium && !window.title_unique) {
        return Some(app_only(app));
    }
    let mut context = PinContext {
        title: Some(page_title(&window.title, &app)),
        app: Some(app),
        ..PinContext::default()
    };
    if !window.title_unique {
        return Some(context);
    }
    let found = accessible(
        &window,
        include_selection,
        surface_point,
        screen_point,
        deadline,
    );
    if chromium && !found.frame_confirmed {
        return context.app.map(app_only);
    }
    context.url = found.url;
    context.link = found.link;
    context.image = found.image;
    if let Some(title) = found.title {
        context.title = Some(title);
    }
    context.selection = found.selection;
    Some(context)
}

fn app_only(app: String) -> PinContext {
    PinContext {
        app: Some(app),
        ..PinContext::default()
    }
}

/// Google Chrome and Chromium. Their compositor title is the same in Incognito;
/// only the accessible frame's name carries the marker.
fn is_chromium(app: &str) -> bool {
    matches!(
        app.to_ascii_lowercase().replace(' ', "-").as_str(),
        "google-chrome"
            | "google-chrome-stable"
            | "google-chrome-beta"
            | "google-chrome-unstable"
            | "chromium"
            | "chromium-browser"
            | "org.chromium.chromium"
            | "com.google.chrome"
    )
}

fn is_firefox(app: &str) -> bool {
    let app = app.to_ascii_lowercase();
    matches!(
        app.as_str(),
        "firefox" | "firefox-esr" | "org.mozilla.firefox" | "mozilla-firefox" | "mozilla firefox"
    )
}

/// English window titles that say a window is private. Absence proves nothing:
/// other browsers and languages rely on the Include Page Details switch.
fn private_title(app: &str, title: &str) -> bool {
    (is_chromium(app) && title.ends_with(" (Incognito)"))
        || (is_firefox(app) && title.ends_with("Private Browsing"))
}

fn point_in_window(
    screen: (f64, f64),
    absolute: (i32, i32, i32, i32),
    local: (i32, i32, i32, i32),
) -> Option<(i32, i32)> {
    let (x, y, width, height) = absolute;
    if width <= 0
        || height <= 0
        || x == i32::MIN
        || y == i32::MIN
        || (x, y) == (local.0, local.1)
        || !screen.0.is_finite()
        || !screen.1.is_finite()
        || screen.0 < f64::from(x)
        || screen.1 < f64::from(y)
        || screen.0 >= f64::from(x) + f64::from(width)
        || screen.1 >= f64::from(y) + f64::from(height)
    {
        return None;
    }
    Some((
        (screen.0 - f64::from(x) + f64::from(local.0)).round() as i32,
        (screen.1 - f64::from(y) + f64::from(local.1)).round() as i32,
    ))
}

async fn screen_to_window(
    bus: &Connection,
    frame: &Object,
    screen: (f64, f64),
) -> Option<(i32, i32)> {
    let absolute = call(bus, frame, COMPONENT, "GetExtents", &(0_u32,))
        .await
        .ok()?;
    let local = call(bus, frame, COMPONENT, "GetExtents", &(COORDS_WINDOW,))
        .await
        .ok()?;
    point_in_window(screen, absolute, local)
}

fn read_screen(
    include_selection: bool,
    point: (f64, f64),
    deadline: Instant,
) -> Option<PinContext> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    let mut context = None;
    let mut found = Found::default();
    let mut chromium = false;
    let x11 = super::super::x11_surface::active();
    let gnome = !x11 && super::super::gnome_focus::desktop();
    let desktop_window = if x11 {
        super::super::x11_surface::window_at(point)
    } else if gnome {
        super::super::gnome_pill::window_at(point)
    } else {
        None
    };
    runtime.block_on(async {
        let geometry = if gnome || x11 {
            desktop_window
        } else {
            super::kwin_context::at_point(point).await
        };
        let read = async {
            let bus = accessibility_bus().await.ok()?;
            if let Some(window) = geometry {
                let local = (
                    (point.0 - window.x).round() as i32,
                    (point.1 - window.y).round() as i32,
                );
                if local.0 < 0
                    || local.1 < 0
                    || f64::from(local.0) >= window.width
                    || f64::from(local.1) >= window.height
                {
                    return None;
                }
                chromium = is_chromium(&window.app);
                if private_title(&window.app, &window.title) {
                    context = Some(app_only(app_name(&window.app)));
                    chromium = false;
                    return None;
                }
                context = Some(PinContext {
                    app: Some(app_name(&window.app)),
                    title: Some(window.title.clone()),
                    ..Default::default()
                });
                let frame = frame_titled(&bus, &window.title).await.ok()??;
                fill_frame(&bus, &frame, include_selection, Some(local), &mut found).await;
                found.frame_confirmed = true;
                return Some(());
            }
            let root = (
                "org.a11y.atspi.Registry".to_owned(),
                OwnedObjectPath::try_from(ROOT).ok()?,
            );
            let apps = children(&bus, &root).await.ok()?;
            if apps.len() > MAX_APPS {
                return None;
            }
            let mut candidates = Vec::new();
            for app in apps {
                let app_name = name(&bus, &app).await?;
                if is_own_window(&app_name, None) {
                    continue;
                }
                let frames = children(&bus, &app).await.ok()?;
                if frames.len() > MAX_FRAMES {
                    return None;
                }
                for frame in frames {
                    let state: Vec<u32> =
                        call(&bus, &frame, ACCESSIBLE, "GetState", &()).await.ok()?;
                    if !has_state(&state, STATE_SHOWING) {
                        continue;
                    }
                    if let Some(local) = screen_to_window(&bus, &frame, point).await {
                        candidates.push((
                            app_name.clone(),
                            name(&bus, &frame).await?,
                            frame,
                            local,
                        ));
                    }
                }
            }
            let (app, title, frame, local) = only(candidates)?;
            if private_title(&app, &title) {
                context = Some(app_only(app));
                return None;
            }
            chromium = is_chromium(&app);
            context = Some(PinContext {
                app: Some(app),
                title: Some(title.clone()),
                ..Default::default()
            });
            fill_frame(&bus, &frame, include_selection, Some(local), &mut found).await;
            if name(&bus, &frame).await.as_deref() != Some(&title) {
                found = Found::default();
            } else {
                found.frame_confirmed = true;
            }
            Some(())
        };
        let _ = tokio::time::timeout(remaining(deadline).min(ACCESSIBILITY), read).await;
    });
    let mut context = context?;
    if chromium && !found.frame_confirmed {
        return context.app.map(app_only);
    }
    context.url = found.url;
    context.link = found.link;
    context.image = found.image;
    context.selection = found.selection;
    if let Some(title) = found.title {
        context.title = Some(title);
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

/// Chromium calls it `URI`; other toolkits `DocURL` or `URL`.
fn document_url(attributes: &HashMap<String, String>) -> Option<String> {
    attributes
        .iter()
        .find(|(key, _)| matches!(key.to_ascii_lowercase().as_str(), "docurl" | "uri" | "url"))
        .and_then(|(_, value)| web_url(value))
}

/// Chromium puts U+FFFC in a container's text for each embedded child.
fn selection_text(raw: &str) -> Option<String> {
    let text = raw.replace('\u{FFFC}', "");
    let meaningful = text.trim();
    (!meaningful.is_empty()).then(|| meaningful.to_owned())
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

#[derive(Default)]
struct Found {
    url: Option<String>,
    title: Option<String>,
    link: Option<String>,
    image: Option<String>,
    selection: Option<String>,
    /// A password field is focused or has a selection: read no selection at all.
    password: bool,
    /// The accessible frame's name is exactly the window title.
    frame_confirmed: bool,
}

fn accessible(
    window: &NiriWindow,
    include_selection: bool,
    surface_point: (f64, f64),
    screen_point: (f64, f64),
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
        let read = fill(
            &window.title,
            include_selection,
            point,
            screen_point,
            window.geometry.as_ref(),
            &mut found,
        );
        let _ = tokio::time::timeout(limit, read).await;
    });
    found
}

async fn fill(
    title: &str,
    include_selection: bool,
    point: Option<(i32, i32)>,
    screen_point: (f64, f64),
    geometry: Option<&super::pick_geometry::PickGeometry>,
    found: &mut Found,
) -> zbus::Result<()> {
    let bus = accessibility_bus().await?;
    let Some(frame) = frame_titled(&bus, title).await? else {
        return Ok(());
    };
    let point = if let Some(geometry) = geometry {
        let rect: zbus::Result<(i32, i32, i32, i32)> =
            call(&bus, &frame, COMPONENT, "GetExtents", &(COORDS_WINDOW,)).await;
        rect.ok()
            .and_then(|rect| geometry.in_frame(screen_point, rect))
    } else {
        match point {
            Some(point) => Some(point),
            None => screen_to_window(&bus, &frame, screen_point).await,
        }
    };
    fill_frame(&bus, &frame, include_selection, point, found).await;
    if name(&bus, &frame).await.as_deref() != Some(title) {
        *found = Found::default();
    } else {
        found.frame_confirmed = true;
    }
    Ok(())
}

async fn fill_frame(
    bus: &Connection,
    frame: &Object,
    include_selection: bool,
    point: Option<(i32, i32)>,
    found: &mut Found,
) {
    let chain = if let Some(point) = point {
        hit_chain(bus, frame, point).await
    } else {
        Vec::new()
    };
    let mut scope = frame.clone();
    for object in &chain {
        if role(bus, object).await.ok() == Some(ROLE_DOCUMENT_WEB) {
            scope = object.clone();
            break;
        }
    }
    let scan = scan_frame(bus, &scope, include_selection, found).await;
    read_hits(bus, chain, found).await;
    if include_selection && !found.password {
        found.selection = read_selection(bus, &scan).await;
    }
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
/// by its exact title. Two windows with one title could be either, so neither
/// is used: no context beats another window's context.
async fn frame_titled(bus: &Connection, title: &str) -> zbus::Result<Option<Object>> {
    if title.is_empty() {
        return Ok(None);
    }
    let root = (
        "org.a11y.atspi.Registry".to_owned(),
        OwnedObjectPath::try_from(ROOT)?,
    );
    let mut matching = Vec::new();
    for app in children(bus, &root).await?.into_iter().take(MAX_APPS) {
        let Ok(frames) = children(bus, &app).await else {
            continue;
        };
        for frame in frames.into_iter().take(MAX_FRAMES) {
            if name(bus, &frame).await.as_deref() == Some(title) {
                matching.push(frame);
            }
        }
    }
    Ok(only(matching))
}

fn only<T>(mut items: Vec<T>) -> Option<T> {
    if items.len() == 1 { items.pop() } else { None }
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
    let mut video = None;
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
        if object_role == ROLE_PASSWORD {
            // Never `selected_text`: that would read the password itself.
            if focused || has_selection(bus, &object).await {
                found.password = true;
            }
            continue;
        }
        if object_role == ROLE_DOCUMENT_WEB {
            if found.url.is_none() {
                read_document(bus, &object, found).await;
            } else {
                continue;
            }
        }
        if focused && scan.focused.is_none() {
            scan.focused = Some(object.clone());
        }
        if object_role == ROLE_SLIDER
            && video.is_none()
            && found
                .url
                .as_deref()
                .is_some_and(video_time::is_youtube_video)
        {
            let attributes: zbus::Result<HashMap<String, String>> =
                call(bus, &object, ACCESSIBLE, "GetAttributes", &()).await;
            if attributes.as_ref().is_ok_and(video_time::is_player) {
                let value: zbus::Result<OwnedValue> =
                    call(bus, &object, PROPERTIES, "Get", &(VALUE, "CurrentValue")).await;
                video = value.ok().and_then(|value| f64::try_from(value).ok());
            }
        }
        if !include_selection
            && found.url.is_some()
            && !found
                .url
                .as_deref()
                .is_some_and(video_time::is_youtube_video)
        {
            break;
        }
        scan.showing.push(object.clone());
        if let Ok(descendants) = children(bus, &object).await {
            pending.extend(descendants.into_iter().take(MAX_CHILDREN));
        }
    }
    if let (Some(url), Some(seconds)) = (&found.url, video)
        && let Some(timed) = video_time::timestamp(url, seconds)
    {
        found.url = Some(timed);
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

/// Uses window-relative coordinates from a compositor or a validated provider.
async fn hit_chain(bus: &Connection, frame: &Object, point: (i32, i32)) -> Vec<Object> {
    let hit: zbus::Result<Object> = call(
        bus,
        frame,
        COMPONENT,
        "GetAccessibleAtPoint",
        &(point.0, point.1, COORDS_WINDOW),
    )
    .await;
    let mut current = hit.ok().filter(|hit| hit.1.as_str() != NULL_OBJECT);
    let mut descended = Vec::new();
    for _ in 0..MAX_PARENTS {
        let Some(object) = &current else {
            return Vec::new();
        };
        if descended.contains(object) {
            break;
        }
        descended.push(object.clone());
        let deeper: zbus::Result<Object> = call(
            bus,
            object,
            COMPONENT,
            "GetAccessibleAtPoint",
            &(point.0, point.1, COORDS_WINDOW),
        )
        .await;
        match deeper
            .ok()
            .filter(|next| next.1.as_str() != NULL_OBJECT && next != object)
        {
            Some(next) => current = Some(next),
            None => break,
        }
    }
    let mut chain = Vec::new();
    for _ in 0..MAX_PARENTS {
        let Some(object) = current else {
            return Vec::new();
        };
        if chain.contains(&object) {
            return Vec::new();
        }
        chain.push(object.clone());
        if &object == frame {
            break;
        }
        current = parent(bus, &object).await;
    }
    if chain.last() != Some(frame) {
        return Vec::new();
    }
    chain
}

async fn read_hits(bus: &Connection, chain: Vec<Object>, found: &mut Found) {
    for object in chain {
        match role(bus, &object).await {
            Ok(ROLE_PASSWORD) => {
                found.link = None;
                found.image = None;
                return;
            }
            Ok(ROLE_LINK) if found.link.is_none() => {
                let uri: zbus::Result<String> =
                    call(bus, &object, HYPERLINK, "GetURI", &(0_i32,)).await;
                found.link = uri.ok().as_deref().and_then(web_url);
            }
            Ok(ROLE_IMAGE) if found.image.is_none() => {
                let attributes: zbus::Result<HashMap<String, String>> =
                    call(bus, &object, ACCESSIBLE, "GetAttributes", &()).await;
                found.image = attributes
                    .ok()
                    .and_then(|attrs| attrs.get("src").and_then(|src| web_url(src)));
                if found.image.is_none() {
                    found.image = name(bus, &object).await.filter(|name| !name.is_empty());
                }
            }
            Ok(ROLE_DOCUMENT_WEB) => return,
            _ => {}
        }
    }
}

async fn has_selection(bus: &Connection, object: &Object) -> bool {
    call::<_, i32>(bus, object, TEXT, "GetNSelections", &())
        .await
        .is_ok_and(|count| count > 0)
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
    let end = end.min(start.saturating_add(crate::pin_context::SELECTION_LIMIT as i32));
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
    fn only_the_marker_on_the_right_browser_means_private() {
        for (app, title) in [
            ("google-chrome", "Bank - Google Chrome (Incognito)"),
            ("Google Chrome", "Bank - Google Chrome (Incognito)"),
            ("chromium", "Bank - Chromium (Incognito)"),
            ("firefox", "Bank — Mozilla Firefox Private Browsing"),
            (
                "org.mozilla.firefox",
                "Bank — Mozilla Firefox Private Browsing",
            ),
        ] {
            assert!(super::private_title(app, title), "{app} {title}");
        }
        for (app, title) in [
            ("google-chrome", "Bank - Google Chrome"),
            ("firefox", "Bank — Mozilla Firefox"),
            ("firefox", "Bank (Incognito) — Mozilla Firefox"),
            ("kitty", "notes (Incognito)"),
            ("kitty", "Private Browsing"),
            ("google-chrome", "Private Browsing"),
        ] {
            assert!(!super::private_title(app, title), "{app} {title}");
        }
    }

    #[test]
    #[ignore = "requires the named private niri geometry lab and real AT-SPI fixture"]
    fn native_niri_geometry_context() {
        let runtime = std::env::var("AGENT_LAB_RUNTIME").unwrap();
        assert!(runtime.ends_with("/koett-niri-geometry-sol"));
        assert_eq!(std::env::var("XDG_RUNTIME_DIR").unwrap(), runtime);
        let case_path = std::path::PathBuf::from(std::env::var("KOETT_NIRI_NATIVE_CASE").unwrap());
        let case: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&case_path).unwrap()).unwrap();
        let mut window = niri_pick::parse_picked(&case["reply"].to_string()).unwrap();
        window.title_unique = case["windows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["title"].as_str() == Some(&window.title))
            .count()
            == 1;
        let pin = (
            case["pin"][0].as_f64().unwrap(),
            case["pin"][1].as_f64().unwrap(),
        );
        let found = if window.title_unique {
            accessible(&window, false, pin, pin, Instant::now() + TOTAL)
        } else {
            Found::default()
        };
        let evidence = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let bus = accessibility_bus().await.ok()?;
                let frame = frame_titled(&bus, &window.title).await.ok()??;
                let rect: (i32, i32, i32, i32) =
                    call(&bus, &frame, COMPONENT, "GetExtents", &(COORDS_WINDOW,))
                        .await
                        .ok()?;
                Some((
                    rect,
                    window
                        .geometry
                        .as_ref()
                        .and_then(|geometry| geometry.in_frame(pin, rect)),
                ))
            });
        let result = serde_json::json!({"link":found.link,"image":found.image,"window":window.title,
            "point":window.geometry.as_ref().and_then(|geometry| geometry.point(pin)),
            "frame_bounds":evidence.map(|value|value.0),"frame_point":evidence.and_then(|value|value.1)});
        std::fs::write(case_path.with_extension("result.json"), result.to_string()).unwrap();
        assert_eq!(result["link"], case["expected_link"], "{result}");
        assert_eq!(result["image"], case["expected_image"], "{result}");
    }

    #[test]
    #[ignore = "requires isolated desktop with a YouTube browser fixture"]
    fn private_youtube_pin() {
        assert!(
            std::env::var("AGENT_LAB_RUNTIME")
                .is_ok_and(|path| path.contains("/agent-lab/koett-row3-sol"))
        );
        let context =
            read_screen(false, (700.0, 500.0), Instant::now() + TOTAL).expect("fixture context");
        println!("context={context:?}");
        assert!(
            context
                .url
                .as_deref()
                .is_some_and(|url| url.contains("youtube.com/watch?v=") && url.contains("t="))
        );
    }

    #[test]
    #[ignore = "requires isolated desktop with linked-image browser fixture"]
    fn private_element_pin() {
        assert!(
            std::env::var("AGENT_LAB_RUNTIME")
                .is_ok_and(|path| path.contains("/agent-lab/koett-row3-sol"))
        );
        let context =
            read_screen(false, (500.0, 250.0), Instant::now() + TOTAL).expect("fixture context");
        println!("context={context:?}");
        assert_eq!(
            context.link.as_deref(),
            Some("https://example.com/pin-target")
        );
        assert_eq!(context.image.as_deref(), Some("Coral square"));
    }

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
        assert!(web_url("file:///home/example/secret.txt").is_none());
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
        let chromium = HashMap::from([("URI".to_owned(), "https://b.example/".to_owned())]);
        assert_eq!(
            document_url(&chromium).as_deref(),
            Some("https://b.example/")
        );
    }

    #[test]
    fn selection_drops_embedded_object_markers() {
        assert_eq!(selection_text("\u{FFFC} hello ").as_deref(), Some("hello"));
        assert_eq!(selection_text("\u{FFFC}\u{FFFC}"), None);
    }

    #[test]
    fn a_title_shared_by_two_windows_matches_neither() {
        assert_eq!(only(vec![1]), Some(1));
        assert_eq!(only(vec![1, 2]), None);
        assert_eq!(only(Vec::<i32>::new()), None);
    }

    #[test]
    fn window_point_subtracts_tile_and_offset() {
        assert_eq!(
            window_point((110.4, 60.0), (100.0, 40.0), (0.0, 10.0)),
            (10, 10)
        );
    }

    #[test]
    fn global_coordinates_require_a_real_origin_and_containment() {
        assert_eq!(
            point_in_window((110.0, 230.0), (100, 200, 500, 400), (0, 0, 500, 400)),
            Some((10, 30))
        );
        assert_eq!(
            point_in_window((-390.0, 230.0), (-400, 200, 500, 400), (0, 0, 500, 400)),
            Some((10, 30))
        );
        assert_eq!(
            point_in_window((10.0, 30.0), (0, 0, 500, 400), (0, 0, 500, 400)),
            None
        );
        assert_eq!(
            point_in_window((900.0, 30.0), (100, 200, 500, 400), (0, 0, 500, 400)),
            None
        );
        assert_eq!(
            point_in_window(
                (100.0, 200.0),
                (i32::MIN, i32::MIN, 500, 400),
                (0, 0, 500, 400)
            ),
            None
        );
    }

    #[test]
    fn state_bits_span_both_words() {
        assert!(has_state(&[1 << STATE_FOCUSED, 0], STATE_FOCUSED));
        assert!(has_state(&[0, 1 << 1], 33));
        assert!(!has_state(&[0], 33));
    }
}
