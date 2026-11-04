#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../src/linux/feedback.rs"]
mod feedback;

#[cfg(target_os = "linux")]
fn main() {
    use gtk4::{gdk, glib, prelude::*};
    gtk4::init().unwrap();
    let args: Vec<String> = std::env::args().collect();
    let png = std::fs::read(&args[1]).unwrap();
    let text = "- #1 (0:00) “clipboard regression”";
    feedback::remember_sketch(text, Some(&png));
    if args.get(2).is_some_and(|arg| arg == "--explicit") {
        let runtime = std::env::var_os("AGENT_LAB_RUNTIME").expect("private lab required");
        assert_eq!(std::env::var_os("XDG_RUNTIME_DIR"), Some(runtime));
        assert_eq!(
            std::env::var("WAYLAND_DISPLAY").as_deref(),
            Ok("wayland-agent-lab")
        );
        let button = gtk4::Button::with_label("Offer clipboard");
        button.connect_clicked(move |button| {
            feedback::copy_transcript(&button.clipboard(), text).unwrap();
            println!("Ready");
        });
        let window = gtk4::Window::builder()
            .title("Koett clipboard probe")
            .child(&button)
            .build();
        window.present();
        button.grab_focus();
    } else {
        feedback::copy_transcript(&gdk::Display::default().unwrap().clipboard(), text).unwrap();
        println!("Ready");
    }
    glib::MainLoop::new(None, false).run();
}

#[cfg(not(target_os = "linux"))]
fn main() {}
