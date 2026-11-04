use std::collections::HashMap;
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_millis(200);
const LIMIT: usize = 4096;
const QUERY: &str = r#"
const w = workspace.activeWindow;
const result = w && !w.dock && !w.desktopWindow && !w.deleted
    ? {id:String(w.internalId), app:String(w.resourceClass), pid:w.pid} : null;
"#;

#[derive(serde::Deserialize)]
struct Window {
    id: String,
    app: String,
    pid: u32,
}

#[derive(Default)]
struct Identities {
    tokens: HashMap<(String, uuid::Uuid), u32>,
}
impl Identities {
    fn resolve(&mut self, owner: String, raw: &str) -> Option<(u32, String)> {
        let window: Window = serde_json::from_str(raw).ok()?;
        let id = uuid::Uuid::parse_str(&window.id).ok()?;
        if id.is_nil()
            || window.pid == 0
            || window.pid == std::process::id()
            || window.app.len() > 256
            || matches!(
                window.app.to_lowercase().as_str(),
                "koett" | "com.olledyberg.koett"
            )
        {
            return None;
        }
        let key = (owner, id);
        let token = if let Some(token) = self.tokens.get(&key) {
            *token
        } else {
            if self.tokens.len() >= LIMIT {
                return None;
            }
            let token = u32::try_from(self.tokens.len() + 1).ok()?;
            self.tokens.insert(key, token);
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

/// Each call asks KWin anew. Late/busy/failed checks never reuse cached focus.
pub fn focused() -> Option<(u32, String)> {
    let sender = WORKER
        .get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel::<Request>(1);
            std::thread::Builder::new()
                .name("koett-kwin-focus".into())
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
                            .block_on(super::sketch::kwin_context::query(QUERY))
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

#[cfg(test)]
mod tests {
    use super::*;
    fn window(id: uuid::Uuid) -> String {
        serde_json::json!({"id":id,"pid":1,"app":"org.example.Editor"}).to_string()
    }
    #[test]
    fn window_and_compositor_incarnations_never_share_tokens() {
        let mut ids = Identities::default();
        let a = window(uuid::Uuid::new_v4());
        let b = window(uuid::Uuid::new_v4());
        let first = ids.resolve(":1.1".into(), &a).unwrap();
        assert_eq!(ids.resolve(":1.1".into(), &a), Some(first.clone()));
        assert_ne!(ids.resolve(":1.1".into(), &b).unwrap().0, first.0);
        assert_ne!(ids.resolve(":1.2".into(), &a).unwrap().0, first.0);
        assert_eq!(first.1, "org.example.Editor");
    }
    #[test]
    fn invalid_own_and_exhausted_identities_fail_closed() {
        let mut ids = Identities::default();
        for raw in ["null".into(), window(uuid::Uuid::nil()), serde_json::json!({"id":uuid::Uuid::new_v4(),"pid":std::process::id(),"app":"custom-name"}).to_string(), serde_json::json!({"id":uuid::Uuid::new_v4(),"pid":1,"app":"com.olledyberg.Koett"}).to_string()] {
            assert!(ids.resolve(":1.1".into(), &raw).is_none());
        }
        for _ in 0..LIMIT {
            assert!(
                ids.resolve(":1.1".into(), &window(uuid::Uuid::new_v4()))
                    .is_some()
            );
        }
        assert!(
            ids.resolve(":1.1".into(), &window(uuid::Uuid::new_v4()))
                .is_none()
        );
    }
}
