use ashpd::zbus;
use std::{
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    sync::Mutex,
    time::Duration,
};
use tokio::sync::oneshot;

#[derive(serde::Deserialize)]
pub struct Window {
    pub title: String,
    pub app: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

struct Reply(Mutex<Option<oneshot::Sender<String>>>);

#[zbus::interface(name = "com.olledyberg.Koett.PinContext", crate = "ashpd::zbus")]
impl Reply {
    fn window(&self, value: String) {
        if let Ok(mut sender) = self.0.lock()
            && let Some(sender) = sender.take()
        {
            let _ = sender.send(value);
        }
    }
}

struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join("pin.js"));
        let _ = std::fs::remove_dir(&self.0);
    }
}

pub async fn at_point(point: (f64, f64)) -> Option<Window> {
    if !point.0.is_finite() || !point.1.is_finite() {
        return None;
    }
    let (sender, receiver) = oneshot::channel();
    let connection = zbus::connection::Builder::session()
        .ok()?
        .serve_at("/Pin", Reply(Mutex::new(Some(sender))))
        .ok()?
        .build()
        .await
        .ok()?;
    let bus = zbus::Proxy::new(
        &connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )
    .await
    .ok()?;
    // Avoid writing a script on desktops without KWin.
    let dbus = zbus::fdo::DBusProxy::new(&connection).await.ok()?;
    if !dbus
        .name_has_owner("org.kde.KWin".try_into().ok()?)
        .await
        .ok()?
    {
        return None;
    }
    let name = format!("koett-pin-{}", uuid::Uuid::new_v4());
    let directory = directories::BaseDirs::new()?.cache_dir().join(&name);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .ok()?;
    let scratch = Scratch(directory);
    let path = scratch.0.join("pin.js");
    let destination = serde_json::to_string(connection.unique_name()?.as_str()).ok()?;
    let script = format!(
        r#"
const windows = workspace.windowAt({{x:{}, y:{}}}, -1);
let result = null;
for (const w of windows) {{
    if (String(w.resourceClass).toLowerCase().includes("koett")) continue;
    if (w.dock || w.desktopWindow) break;
    const r = w.clientGeometry;
    result = {{title:w.caption, app:String(w.resourceClass), x:r.x, y:r.y, width:r.width, height:r.height}};
    break;
}}
callDBus({}, "/Pin", "com.olledyberg.Koett.PinContext", "Window", JSON.stringify(result));
"#,
        point.0, point.1, destination
    );
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .ok()?
        .write_all(script.as_bytes())
        .ok()?;
    let loaded: zbus::Result<i32> = tokio::time::timeout(
        Duration::from_millis(150),
        bus.call("loadScript", &(path.to_str()?, &name)),
    )
    .await
    .ok()?;
    let id = loaded.ok()?;
    if id < 0 {
        return None;
    }
    let result = async {
        let path = format!("/Scripting/Script{id}");
        let runner = zbus::Proxy::new(&connection, "org.kde.KWin", path, "org.kde.kwin.Script")
            .await
            .ok()?;
        runner.call::<_, _, ()>("run", &()).await.ok()?;
        let raw = receiver.await.ok()?;
        serde_json::from_str::<Option<Window>>(&raw).ok().flatten()
    };
    let result = tokio::time::timeout(Duration::from_millis(200), result)
        .await
        .ok()
        .flatten();
    let _: Result<zbus::Result<bool>, _> = tokio::time::timeout(
        Duration::from_millis(100),
        bus.call("unloadScript", &(&name,)),
    )
    .await;
    result
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires an isolated KWin desktop and fixture window"]
    fn private_kwin_geometry() {
        assert!(
            std::env::var("AGENT_LAB_RUNTIME")
                .is_ok_and(|path| path.contains("/agent-lab/koett-row3-sol"))
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let window = runtime
            .block_on(super::at_point((500.0, 500.0)))
            .expect("fixture at point");
        println!(
            "window title={} app={} rect={},{},{},{}",
            window.title, window.app, window.x, window.y, window.width, window.height
        );
    }
}
