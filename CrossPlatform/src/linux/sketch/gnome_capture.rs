use ashpd::zbus;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;

const MAX_PNG: usize = 32 * 1024 * 1024;
const WAIT: Duration = Duration::from_secs(5);

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    protocol: u32,
    ok: bool,
    png: Option<String>,
    error: Option<String>,
}

fn decode(raw: &str) -> Result<Vec<u8>, String> {
    if raw.len() > MAX_PNG.div_ceil(3) * 4 + 512 {
        return Err("GNOME screenshot is too large".into());
    }
    let reply: Reply = serde_json::from_str(raw).map_err(|_| "Invalid GNOME capture reply")?;
    if reply.protocol != 1 || !reply.ok {
        return Err(reply
            .error
            .filter(|error| error.len() <= 160)
            .unwrap_or_else(|| "GNOME capture unavailable".into()));
    }
    let encoded = reply.png.ok_or("Missing GNOME screenshot")?;
    let png = STANDARD
        .decode(encoded)
        .map_err(|_| "Invalid GNOME screenshot encoding")?;
    if png.len() > MAX_PNG || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Invalid GNOME screenshot".into());
    }
    let (width, height) = super::sheet::png_size(&png)?;
    if width <= 0 || height <= 0 || i64::from(width) * i64::from(height) > 32 * 1024 * 1024 {
        return Err("GNOME screenshot dimensions are too large".into());
    }
    gtk4::gdk::Texture::from_bytes(&gtk4::glib::Bytes::from(png.as_slice()))
        .map_err(|_| "Invalid GNOME screenshot data")?;
    Ok(png)
}

pub fn capture(rect: super::capture::Rect) -> Result<Vec<u8>, String> {
    static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
    static SLOTS: LazyLock<Arc<tokio::sync::Semaphore>> =
        LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(4)));
    let permit = SLOTS
        .clone()
        .try_acquire_owned()
        .map_err(|_| "GNOME capture busy; dictation was kept")?;
    let runtime = RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .max_blocking_threads(4)
                .enable_all()
                .build()
                .map_err(|_| "GNOME capture runtime unavailable".into())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    runtime.block_on(async {
        tokio::time::timeout(WAIT, async {
            let connection = zbus::Connection::session()
                .await
                .map_err(|_| "GNOME session bus unavailable")?;
            let bus = zbus::fdo::DBusProxy::new(&connection)
                .await
                .map_err(|_| "GNOME session bus unavailable")?;
            let owner = bus
                .get_name_owner(
                    "org.gnome.Shell"
                        .try_into()
                        .map_err(|_| "Invalid Shell service")?,
                )
                .await
                .map_err(|_| "GNOME Shell unavailable")?;
            let proxy = zbus::Proxy::new(
                &connection,
                owner.as_str(),
                "/org/gnome/Shell/Extensions/Koett",
                "com.olledyberg.Koett.Gnome1",
            )
            .await
            .map_err(|_| "GNOME capture extension unavailable")?;
            let request = serde_json::to_string(&rect).map_err(|_| "Invalid capture rectangle")?;
            let raw: String = proxy
                .call("Capture", &(request,))
                .await
                .map_err(|_| "Enable the Koett GNOME extension for drawing capture")?;
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                decode(&raw)
            })
            .await
            .map_err(|_| "GNOME screenshot decoding failed")?
        })
        .await
        .map_err(|_| "GNOME capture timed out; dictation was kept")?
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the named private GNOME capture lab and real compositor"]
    fn native_gnome_capture() {
        use gtk4::{self as gtk, prelude::*};
        use std::time::Instant;
        let lab = std::env::var("AGENT_LAB_RUNTIME").unwrap();
        assert!(lab.ends_with("/koett-gnome-sol"));
        assert_eq!(
            std::env::var("XDG_RUNTIME_DIR").unwrap(),
            format!("{lab}/gnome")
        );
        assert_eq!(
            std::env::var("WAYLAND_DISPLAY").unwrap(),
            "wayland-gnome-sol"
        );
        let directory = std::path::PathBuf::from(std::env::var("KOETT_CAPTURE_PROBE_DIR").unwrap());
        gtk::init().unwrap();
        let app = gtk::Application::builder()
            .application_id("com.olledyberg.Koett")
            .build();
        app.register(None::<&gtk::gio::Cancellable>).unwrap();
        let css = gtk::CssProvider::new();
        css.load_from_data(
            ".capture-pill {background:rgb(255,0,255);} .capture-ink {background:rgb(255,0,0);}",
        );
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let painted = std::rc::Rc::new(std::cell::Cell::new(0_u8));
        let windows = [
            (
                "Koett pill 11111111-1111-4111-8111-111111111111",
                "capture-pill",
            ),
            ("Koett sketch native capture fixture", "capture-ink"),
        ]
        .map(|(title, class)| {
            let window = gtk::ApplicationWindow::builder()
                .application(&app)
                .title(title)
                .decorated(false)
                .default_width(96)
                .default_height(96)
                .build();
            window.add_css_class(class);
            let area = gtk::DrawingArea::new();
            let blue = if class == "capture-pill" { 1.0 } else { 0.0 };
            let painted = painted.clone();
            let flag = if class == "capture-pill" { 1 } else { 2 };
            area.set_draw_func(move |_, context, _, _| {
                painted.set(painted.get() | flag);
                context.set_source_rgb(1.0, 0.0, blue);
                context.paint().unwrap();
            });
            window.set_child(Some(&area));
            window.present();
            window
        });
        let context = gtk::glib::MainContext::default();
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut last = String::new();
        let mut pending = None;
        while Instant::now() < deadline {
            while context.pending() {
                context.iteration(false);
            }
            if painted.get() == 3 && !directory.join("ready").exists() {
                std::fs::write(directory.join("ready"), "ready").unwrap();
            }
            let command = std::fs::read_to_string(directory.join("command")).unwrap_or_default();
            if command == "stop" {
                for window in windows {
                    window.destroy();
                }
                return;
            }
            if pending.is_none() && !command.is_empty() && command != last {
                let value: serde_json::Value = serde_json::from_str(&command).unwrap();
                let rect = serde_json::from_value(value["rect"].clone()).unwrap();
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let start = Instant::now();
                    let target = super::super::capture::Target {
                        connector: None,
                        monitor: rect,
                        desktop: rect,
                    };
                    let _ = tx.send((
                        super::super::capture::capture(&target),
                        start.elapsed().as_millis(),
                    ));
                });
                pending = Some((value["case"].as_str().unwrap().to_string(), rx));
                last = command;
            }
            if let Some((case, receiver)) = pending.as_ref()
                && let Ok((result, elapsed)) = receiver.try_recv()
            {
                let value = match result {
                    Ok(png) => {
                        std::fs::write(directory.join(format!("{case}.png")), &png).unwrap();
                        serde_json::json!({"case":case,"ok":true,"bytes":png.len(),"elapsed_ms":elapsed})
                    }
                    Err(error) => {
                        serde_json::json!({"case":case,"ok":false,"error":error,"elapsed_ms":elapsed})
                    }
                };
                std::fs::write(directory.join("result.json"), value.to_string()).unwrap();
                pending = None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("private capture controller did not finish");
    }

    #[test]
    fn complete_png_is_decoded_but_truncated_and_oversized_headers_fail() {
        let png = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/evidence/gnome-capture-20261104/capture.png"
        ));
        let reply = |png: &[u8]| {
            serde_json::json!({"protocol":1,"ok":true,"png":STANDARD.encode(png)}).to_string()
        };
        assert_eq!(decode(&reply(png)).unwrap(), png);
        assert!(decode(&reply(&png[..24])).is_err());
        let mut oversized = png.to_vec();
        oversized[16..20].copy_from_slice(&i32::MAX.to_be_bytes());
        assert!(decode(&reply(&oversized)).is_err());
    }

    #[test]
    fn invalid_missing_and_rejected_capture_replies_do_not_become_images() {
        for raw in [
            "null",
            r#"{"protocol":2,"ok":true,"png":"a"}"#,
            r#"{"protocol":1,"ok":false,"png":"a"}"#,
            r#"{"protocol":1,"ok":true}"#,
            r#"{"protocol":1,"ok":true,"png":"AAAA"}"#,
        ] {
            assert!(decode(raw).is_err(), "{raw}");
        }
        assert_eq!(
            decode(r#"{"protocol":1,"ok":false,"error":"locked"}"#),
            Err("locked".into())
        );
    }
}
