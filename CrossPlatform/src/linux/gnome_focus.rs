use ashpd::zbus;
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_millis(250);

pub fn desktop() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|value| {
        value
            .split(':')
            .any(|part| part.eq_ignore_ascii_case("gnome"))
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Window {
    protocol: u32,
    id: u32,
    pid: u32,
    app: String,
}

#[derive(Default)]
struct Identities(super::focus_tokens::FocusTokens<(String, u32)>);
impl Identities {
    fn resolve(&mut self, owner: String, raw: &str) -> Option<(u32, String)> {
        let window: Window = serde_json::from_str(raw).ok()?;
        if window.protocol != 1
            || window.id == 0
            || window.pid == 0
            || window.pid == std::process::id()
            || window.app.is_empty()
            || window.app.len() > 256
            || window.app.eq_ignore_ascii_case(super::APP_ID)
        {
            return None;
        }
        let token = self.0.token((owner, window.id))?;
        Some((token, window.app))
    }
}

struct Request {
    expires: Instant,
    reply: mpsc::Sender<Option<(u32, String)>>,
}
static WORKER: OnceLock<Option<mpsc::SyncSender<Request>>> = OnceLock::new();

pub fn focused() -> Option<(u32, String)> {
    let sender = WORKER
        .get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel::<Request>(1);
            std::thread::Builder::new()
                .name("koett-gnome-focus".into())
                .spawn(move || {
                    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    else {
                        return;
                    };
                    let mut identities = Identities::default();
                    while let Ok(request) = rx.recv() {
                        if Instant::now() >= request.expires {
                            continue;
                        }
                        let result = runtime
                            .block_on(query())
                            .and_then(|(owner, raw)| identities.resolve(owner, &raw));
                        if Instant::now() < request.expires {
                            let _ = request.reply.send(result);
                        }
                    }
                })
                .ok()?;
            Some(tx)
        })
        .as_ref()?;
    let (tx, rx) = mpsc::channel();
    sender
        .try_send(Request {
            expires: Instant::now() + WAIT,
            reply: tx,
        })
        .ok()?;
    rx.recv_timeout(WAIT).ok().flatten()
}

async fn query() -> Option<(String, String)> {
    tokio::time::timeout(WAIT, async {
        let connection = zbus::Connection::session().await.ok()?;
        let dbus = zbus::fdo::DBusProxy::new(&connection).await.ok()?;
        let owner = dbus
            .get_name_owner("org.gnome.Shell".try_into().ok()?)
            .await
            .ok()?
            .to_string();
        let proxy = zbus::Proxy::new(
            &connection,
            owner.as_str(),
            "/org/gnome/Shell/Extensions/Koett",
            "com.olledyberg.Koett.Gnome1",
        )
        .await
        .ok()?;
        let raw: String = proxy.call("Focus", &()).await.ok()?;
        Some((owner, raw))
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn window(id: u32) -> String {
        serde_json::json!({"protocol":1,"id":id,"pid":1,"app":"org.example.Editor"}).to_string()
    }
    #[test]
    #[ignore = "requires the named private GNOME desktop and native windows"]
    fn native_gnome_focus() {
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
        let directory = std::path::PathBuf::from(std::env::var("KOETT_GNOME_PROBE_DIR").unwrap());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let connection = runtime.block_on(zbus::Connection::session()).unwrap();
        runtime
            .block_on(connection.request_name(super::super::APP_ID))
            .unwrap();
        std::fs::write(directory.join("ready"), "ready").unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut last = String::new();
        while Instant::now() < deadline {
            let command = std::fs::read_to_string(directory.join("command")).unwrap_or_default();
            if command == "stop" {
                return;
            }
            if !command.is_empty() && command != last {
                let started = Instant::now();
                let value = focused();
                let result = serde_json::json!({"case":command,"focus":value,"elapsed_ms":started.elapsed().as_millis()});
                std::fs::write(directory.join("result.json"), result.to_string()).unwrap();
                last = command;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("private GNOME controller did not finish");
    }
    #[test]
    fn distinct_windows_and_shell_restarts_never_reuse_tokens() {
        let mut ids = Identities::default();
        let first = ids.resolve(":1.1".into(), &window(1)).unwrap();
        assert_eq!(ids.resolve(":1.1".into(), &window(1)), Some(first.clone()));
        assert_ne!(ids.resolve(":1.1".into(), &window(2)).unwrap().0, first.0);
        assert_ne!(ids.resolve(":1.2".into(), &window(1)).unwrap().0, first.0);
    }
    #[test]
    fn invalid_own_and_unknown_identities_fail_closed() {
        let mut ids = Identities::default();
        for raw in [
            "null".into(),
            window(0),
            serde_json::json!({"protocol":2,"id":1,"pid":1,"app":"editor"}).to_string(),
            serde_json::json!({"protocol":1,"id":1,"pid":std::process::id(),"app":"editor"})
                .to_string(),
            serde_json::json!({"protocol":1,"id":1,"pid":1,"app":super::super::APP_ID}).to_string(),
        ] {
            assert!(ids.resolve(":1.1".into(), &raw).is_none());
        }
    }
}
