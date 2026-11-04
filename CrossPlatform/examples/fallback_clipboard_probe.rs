//! Private Wayland exercise of the production fallback delivery functions.
#[cfg(target_os = "linux")]
pub use koett_engine::settings;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/clipboard_owner.rs"]
mod clipboard_owner;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/feedback.rs"]
mod feedback;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/gnome_focus.rs"]
mod gnome_focus;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/image_paste.rs"]
mod image_paste;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/sketch/kwin_context.rs"]
pub(crate) mod kwin_context;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/kwin_focus.rs"]
mod kwin_focus;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/output.rs"]
mod output;
#[cfg(target_os = "linux")]
mod sketch {
    pub(crate) use crate::kwin_context;
}
#[cfg(target_os = "linux")]
const APP_ID: &str = "com.olledyberg.Koett";
#[cfg(target_os = "linux")]
fn main() {
    use std::{io::Read, path::Path, time::Duration};
    let runtime = std::env::var("AGENT_LAB_RUNTIME").expect("private lab required");
    assert_eq!(std::env::var("XDG_RUNTIME_DIR").unwrap(), runtime);
    assert!(Path::new(&runtime).parent().unwrap().ends_with("agent-lab"));
    assert_eq!(
        std::env::var("WAYLAND_DISPLAY").unwrap(),
        std::env::var("KOETT_TEST_NIRI_DISPLAY").unwrap()
    );
    if std::env::args().any(|arg| arg == "--focus-status") {
        let target = output::capture_target();
        let chord = image_paste::for_target(target).ok();
        println!(
            "{}",
            serde_json::json!({"target_available":target.is_some(),"image_chord":chord})
        );
        return;
    }
    let png = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    let text = "- #1 (0:00) “fallback clipboard regression”\nsecond line @AGENTS.md";
    if std::env::args().any(|arg| arg == "--clipboard-only") {
        clipboard_owner::start(clipboard_owner::Payload::image(&png)).unwrap();
        clipboard_owner::replace(clipboard_owner::Payload::text(text)).unwrap();
        clipboard_owner::replace(clipboard_owner::Payload::bundle(&png, text)).unwrap();
    } else {
        let target = output::capture_target();
        assert!(target.is_some());
        output::deliver_image(&png, target).unwrap();
        output::deliver_sketch_text(text, target, koett_engine::settings::PasteShortcut::CtrlV)
            .unwrap();
        std::thread::sleep(Duration::from_millis(700));
        clipboard_owner::replace(clipboard_owner::Payload::bundle(&png, text)).unwrap();
    }
    println!("Ready");
    let mut byte = [0];
    let _ = std::io::stdin().read(&mut byte);
    clipboard_owner::stop();
}
#[cfg(not(target_os = "linux"))]
fn main() {}
