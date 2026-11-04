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
#[path = "../src/linux/native_keyboard.rs"]
mod native_keyboard;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/output.rs"]
mod output;
#[cfg(target_os = "linux")]
mod sketch {
    pub(crate) use crate::keyboard_state as keys;
    pub(crate) use crate::kwin_context;
}
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/sketch/keys.rs"]
pub(crate) mod keyboard_state;
#[cfg(target_os = "linux")]
const APP_ID: &str = "com.olledyberg.Koett";
#[cfg(target_os = "linux")]
fn main() {
    use gtk4::{glib, prelude::*};
    use std::{
        cell::Cell,
        rc::Rc,
        sync::mpsc,
        time::{Duration, Instant},
    };
    let runtime = std::env::var("AGENT_LAB_RUNTIME").expect("private lab required");
    assert!(runtime.ends_with("/koett-paste-focus"));
    assert_eq!(std::env::var("XDG_RUNTIME_DIR").unwrap(), runtime);
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).unwrap().clone();
    let expected = args[2].parse::<u32>().unwrap();
    if mode == "plain" {
        let result = output::deliver(
            "Koett focus regression",
            Some(expected),
            settings::PasteShortcut::CtrlV,
        );
        println!(
            "{}",
            serde_json::json!({"ok":result.is_ok(),"error":result.err()})
        );
        return;
    }
    gtk4::init().unwrap();
    let window = gtk4::Window::builder()
        .title("Koett focus test overlay")
        .build();
    let layer = unsafe { libloading::Library::new("libgtk4-layer-shell.so.0") }.unwrap();
    type Init = unsafe extern "C" fn(*mut gtk4::ffi::GtkWindow);
    type Mode = unsafe extern "C" fn(*mut gtk4::ffi::GtkWindow, i32);
    let init = unsafe { *layer.get::<Init>(b"gtk_layer_init_for_window\0").unwrap() };
    let keyboard = unsafe { *layer.get::<Mode>(b"gtk_layer_set_keyboard_mode\0").unwrap() };
    unsafe {
        init(window.as_ptr());
        keyboard(window.as_ptr(), 1);
    }
    output::track_layer_keyboard(&window, true);
    window.present();
    let loop_ = glib::MainLoop::new(None, false);
    let run = loop_.clone();
    let started = Rc::new(Cell::new(false));
    let begin = Instant::now();
    let (tx, rx) = mpsc::channel();
    glib::timeout_add_local(Duration::from_millis(15), move || {
        if !started.get() && window.is_active() {
            started.set(true);
            let missing = output::focused_window().is_none();
            let captured = output::capture_target();
            println!(
                "{}",
                serde_json::json!({"overlay_active":true,"focused_missing":missing,"captured":captured,"expected":expected})
            );
            let tx = tx.clone();
            std::thread::spawn(move || {
                let result = output::deliver(
                    "Koett focus regression",
                    captured,
                    settings::PasteShortcut::CtrlV,
                );
                let _ = tx.send(result);
            });
            if mode == "release" {
                let released = window.clone();
                glib::timeout_add_local_once(Duration::from_millis(180), move || {
                    unsafe {
                        keyboard(released.as_ptr(), 0);
                    }
                    output::track_layer_keyboard(&released, false);
                    released.destroy();
                });
            }
        }
        if let Ok(result) = rx.try_recv() {
            println!(
                "{}",
                serde_json::json!({"ok":result.is_ok(),"error":result.err(),"elapsed_ms":begin.elapsed().as_millis()})
            );
            window.destroy();
            run.quit();
            return glib::ControlFlow::Break;
        }
        if begin.elapsed() > Duration::from_secs(4) {
            println!(
                "{}",
                serde_json::json!({"fixture_failed":"overlay never became active"})
            );
            window.destroy();
            run.quit();
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
    loop_.run();
    drop(layer);
}
#[cfg(not(target_os = "linux"))]
fn main() {}
