use ashpd::zbus::{self, Connection, Proxy, zvariant::OwnedObjectPath};
use futures_util::StreamExt;
use std::time::Duration;
type Object = (String, OwnedObjectPath);
const ROOT: &str = "/org/a11y/atspi/accessible/root";
async fn proxy<'a>(bus: &'a Connection, object: &'a Object) -> zbus::Result<Proxy<'a>> {
    Proxy::new(
        bus,
        object.0.as_str(),
        object.1.as_str(),
        "org.a11y.atspi.Accessible",
    )
    .await
}
async fn state(bus: &Connection, object: &Object, bit: u32) -> zbus::Result<bool> {
    let words: Vec<u32> = proxy(bus, object).await?.call("GetState", &()).await?;
    Ok(words
        .get((bit / 32) as usize)
        .is_some_and(|word| word & (1 << (bit % 32)) != 0))
}
async fn children(bus: &Connection, object: &Object) -> zbus::Result<Vec<Object>> {
    proxy(bus, object).await?.call("GetChildren", &()).await
}
async fn active_frame(bus: &Connection) -> Result<Object, String> {
    let root = (
        "org.a11y.atspi.Registry".into(),
        OwnedObjectPath::try_from(ROOT).map_err(|e| e.to_string())?,
    );
    let mut active = Vec::new();
    let apps = children(bus, &root).await.map_err(|e| e.to_string())?;
    if apps.len() > 64 {
        return Err("Accessibility application limit exceeded; enter a URL".into());
    }
    for app in apps {
        let frames = children(bus, &app).await.map_err(|e| e.to_string())?;
        if frames.len() > 32 {
            return Err("Accessibility window limit exceeded; enter a URL".into());
        }
        for frame in frames {
            if state(bus, &frame, 1).await.unwrap_or(false) {
                active.push(frame);
            }
        }
    }
    if active.len() != 1 {
        return Err("Focus one browser document, or enter its URL".into());
    }
    Ok(active.remove(0))
}
async fn discover() -> Result<String, String> {
    let session = Connection::session().await.map_err(|e| e.to_string())?;
    let accessibility = Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus")
        .await
        .map_err(|e| e.to_string())?;
    let address: String = accessibility
        .call("GetAddress", &())
        .await
        .map_err(|e| e.to_string())?;
    let bus = zbus::connection::Builder::address(address.as_str())
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path_namespace("/org/a11y/atspi/accessible")
        .map_err(|e| e.to_string())?
        .build();
    let mut changes = zbus::MessageStream::for_match_rule(rule, &bus, Some(64))
        .await
        .map_err(|e| e.to_string())?;
    let registry = Proxy::new(
        &bus,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/registry",
        "org.a11y.atspi.Registry",
    )
    .await
    .map_err(|e| e.to_string())?;
    for event in [
        "object:state-changed:focused",
        "object:state-changed:active",
        "window:activate",
        "window:deactivate",
        "document:",
    ] {
        registry
            .call::<_, _, ()>("RegisterEvent", &(event, Vec::<String>::new(), ""))
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::select! {
        biased;
        _ = async {
            while let Some(message) = changes.next().await {
                let Ok(message) = message else { break };
                let header = message.header();
                let interface = header.interface().map(|value| value.as_str()).unwrap_or_default();
                if matches!(interface, "org.a11y.atspi.Event.Window" | "org.a11y.atspi.Event.Document") {
                    break;
                }
                if interface == "org.a11y.atspi.Event.Object"
                    && header.member().is_some_and(|member| member.as_str() == "StateChanged")
                {
                    let body = message.body().deserialize::<(String, i32, i32, zbus::zvariant::OwnedValue, Object)>();
                    if body.is_err() || body.is_ok_and(|body| matches!(body.0.as_str(), "focused" | "active")) {
                        break;
                    }
                }
            }
        } => Err("Focus or document changed; invoke media again".into()),
        result = discover_frame(&bus) => result,
    }
}
async fn discover_frame(bus: &Connection) -> Result<String, String> {
    let frame = active_frame(bus).await?;
    let mut pending = vec![(frame.clone(), Vec::<Object>::new())];
    let mut focused = Vec::new();
    let mut visited = 0;
    while let Some((object, mut ancestors)) = pending.pop() {
        visited += 1;
        if visited > 512 {
            return Err("Focused document is too complex; enter its URL".into());
        }
        ancestors.push(object.clone());
        if state(bus, &object, 12).await.unwrap_or(false) {
            focused.push(ancestors.clone());
        }
        let descendants = children(bus, &object).await.map_err(|e| e.to_string())?;
        if descendants.len() > 128 || (ancestors.len() >= 32 && !descendants.is_empty()) {
            return Err("Focused document exceeds accessibility limits; enter its URL".into());
        }
        for child in descendants {
            pending.push((child, ancestors.clone()));
        }
    }
    if focused.len() != 1 {
        return Err("Browser document focus is unavailable; enter its URL".into());
    }
    let path = focused.remove(0);
    for object in path.iter().rev() {
        let role: u32 = proxy(bus, object)
            .await
            .map_err(|e| e.to_string())?
            .call("GetRole", &())
            .await
            .map_err(|e| e.to_string())?;
        if role != 95 {
            continue;
        }
        let interfaces: Vec<String> = proxy(bus, object)
            .await
            .map_err(|e| e.to_string())?
            .call("GetInterfaces", &())
            .await
            .map_err(|e| e.to_string())?;
        if !interfaces
            .iter()
            .any(|name| name == "org.a11y.atspi.Document")
        {
            continue;
        }
        let document = Proxy::new(
            bus,
            object.0.as_str(),
            object.1.as_str(),
            "org.a11y.atspi.Document",
        )
        .await
        .map_err(|e| e.to_string())?;
        let attributes: std::collections::HashMap<String, String> = document
            .call("GetAttributes", &())
            .await
            .map_err(|e| e.to_string())?;
        let urls: std::collections::HashSet<_> = attributes
            .iter()
            .filter(|(key, _)| {
                matches!(key.to_ascii_lowercase().as_str(), "docurl" | "uri" | "url")
            })
            .map(|(_, value)| value.clone())
            .collect();
        if urls.len() != 1 {
            return Err("Browser document URL is unavailable; enter its URL".into());
        }
        let url = urls.into_iter().next().ok_or("Missing document URL")?;
        crate::media_transcription::validate_url(&url)?;
        if active_frame(bus).await? != frame
            || !state(bus, path.last().ok_or("Missing focus")?, 12)
                .await
                .unwrap_or(false)
        {
            return Err("Focus changed; invoke media again".into());
        }
        let confirmed: std::collections::HashMap<String, String> = document
            .call("GetAttributes", &())
            .await
            .map_err(|e| e.to_string())?;
        if confirmed != attributes {
            return Err("Document changed; invoke media again".into());
        }
        return Ok(url);
    }
    Err("Focus inside a browser document, or enter its URL".into())
}
pub(super) fn current_url() -> Result<String, String> {
    std::thread::spawn(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(async {
                tokio::time::timeout(Duration::from_millis(1500), discover())
                    .await
                    .map_err(|_| "Browser accessibility timed out; enter its URL".to_string())?
            })
    })
    .join()
    .map_err(|_| "Browser accessibility unavailable; enter its URL".to_string())?
}
