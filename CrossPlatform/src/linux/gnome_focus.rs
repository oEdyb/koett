use ashpd::zbus;
use std::collections::HashMap;
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_millis(250);
const LIMIT: usize = 4096;

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
struct Identities(HashMap<(String, u32), u32>);
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
        let key = (owner, window.id);
        let token = if let Some(token) = self.0.get(&key) {
            *token
        } else {
            if self.0.len() >= LIMIT {
                return None;
            }
            let token = u32::try_from(self.0.len() + 1).ok()?;
            self.0.insert(key, token);
            token
        };
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
    fn distinct_windows_and_shell_restarts_never_reuse_tokens() {
        let mut ids = Identities::default();
        let first = ids.resolve(":1.1".into(), &window(1)).unwrap();
        assert_eq!(ids.resolve(":1.1".into(), &window(1)), Some(first.clone()));
        assert_ne!(ids.resolve(":1.1".into(), &window(2)).unwrap().0, first.0);
        assert_ne!(ids.resolve(":1.2".into(), &window(1)).unwrap().0, first.0);
    }
    #[test]
    fn invalid_own_unknown_and_exhausted_identities_fail_closed() {
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
        for id in 1..=LIMIT as u32 {
            assert!(ids.resolve(":1.1".into(), &window(id)).is_some());
        }
        assert!(
            ids.resolve(":1.1".into(), &window(LIMIT as u32 + 1))
                .is_none()
        );
    }
}
