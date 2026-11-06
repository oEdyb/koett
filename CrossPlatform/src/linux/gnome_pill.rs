use ashpd::zbus;
use gtk::prelude::*;
use gtk4 as gtk;
use std::time::Duration;

fn query(
    method: &str,
    value: Option<String>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_millis(250), async {
            let connection = zbus::Connection::session().await?;
            let bus = zbus::fdo::DBusProxy::new(&connection).await?;
            let owner = bus.get_name_owner("org.gnome.Shell".try_into()?).await?;
            let proxy = zbus::Proxy::new(
                &connection,
                owner.as_str(),
                "/org/gnome/Shell/Extensions/Koett",
                "com.olledyberg.Koett.Gnome1",
            )
            .await?;
            let raw: String = match value {
                Some(value) => proxy.call(method, &(value,)).await?,
                None => proxy.call(method, &()).await?,
            };
            Ok::<_, Box<dyn std::error::Error>>(serde_json::from_str(&raw)?)
        })
        .await?
    })
}

pub fn available() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|value| {
        value
            .split(':')
            .any(|part| part.eq_ignore_ascii_case("gnome"))
    }) && query("PillStatus", None)
        .is_ok_and(|value| value["protocol"] == 1 && value["pill"] == true)
}

pub fn configure(
    window: &gtk::Window,
    monitor: Option<&gtk::gdk::Monitor>,
) -> Result<(), Box<dyn std::error::Error>> {
    let title = window
        .title()
        .filter(|title| title.starts_with("Koett pill "))
        .map(|title| title.to_string())
        .unwrap_or_else(|| format!("Koett pill {}", uuid::Uuid::new_v4()));
    window.set_title(Some(&title));
    let monitors = gtk::prelude::WidgetExt::display(window).monitors();
    let fallback = monitors.item(0).and_downcast::<gtk::gdk::Monitor>();
    let rect = monitor
        .or(fallback.as_ref())
        .ok_or("No monitor for GNOME recording pill")?
        .geometry();
    let reply = query("Pill", Some(serde_json::json!({"title":title,"monitor":[rect.x(),rect.y(),rect.width(),rect.height()]}).to_string()))?;
    if reply["ok"] != true {
        return Err("GNOME recording pill registration failed".into());
    }
    Ok(())
}

/// Registers a transparent window that covers one monitor for drawing.
pub fn cover(
    window: &gtk::Window,
    monitor: &gtk::gdk::Monitor,
) -> Result<(), Box<dyn std::error::Error>> {
    let title = format!("Koett sketch {}", uuid::Uuid::new_v4());
    window.set_title(Some(&title));
    let rect = monitor.geometry();
    window.set_default_size(rect.width(), rect.height());
    let reply = query(
        "Pill",
        Some(
            serde_json::json!({"title":title,"cover":true,
                "monitor":[rect.x(),rect.y(),rect.width(),rect.height()]})
            .to_string(),
        ),
    )?;
    if reply["ok"] != true {
        return Err("GNOME drawing surface registration failed".into());
    }
    Ok(())
}

/// The topmost other window at a desktop point, for a pin.
pub fn window_at(point: (f64, f64)) -> Option<super::sketch::GnomeWindow> {
    if !point.0.is_finite() || !point.1.is_finite() {
        return None;
    }
    let reply = query(
        "WindowAt",
        Some(serde_json::json!([point.0.round() as i64, point.1.round() as i64]).to_string()),
    )
    .ok()?;
    serde_json::from_value(reply).ok()
}

/// Whether drawing surfaces take the pointer. Sent in order from one thread, so
/// the UI never waits on the bus. Only the newest waiting value is sent.
pub fn take_pointer(take: bool) {
    use std::sync::{Mutex, OnceLock, mpsc};
    static SENDER: OnceLock<Mutex<mpsc::Sender<bool>>> = OnceLock::new();
    let sender = SENDER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<bool>();
        std::thread::spawn(move || {
            while let Ok(mut take) = receiver.recv() {
                while let Ok(newer) = receiver.try_recv() {
                    take = newer;
                }
                if let Err(error) =
                    query("Input", Some(serde_json::json!({"take":take}).to_string()))
                {
                    eprintln!("GNOME drawing input failed: {error}");
                }
            }
        });
        Mutex::new(sender)
    });
    if let Ok(sender) = sender.lock() {
        let _ = sender.send(take);
    }
}
