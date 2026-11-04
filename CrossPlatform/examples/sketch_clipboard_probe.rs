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
    feedback::copy_transcript(&gdk::Display::default().unwrap().clipboard(), text).unwrap();
    println!("Ready");
    glib::MainLoop::new(None, false).run();
}

#[cfg(not(target_os = "linux"))]
fn main() {}
