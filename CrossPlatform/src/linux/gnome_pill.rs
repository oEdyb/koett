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
