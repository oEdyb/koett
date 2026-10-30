#[cfg(target_os = "linux")]
mod probe {
    use gtk4::{self as gtk, prelude::*};
    enum TrayAction {
        CancelAsk,
    }
    mod ask_view {
        include!("../src/linux/ask_view.rs");
    }
    pub fn run() {
        assert!(
            std::env::var("XDG_RUNTIME_DIR")
                .unwrap()
                .ends_with("/agent-lab/koett-ask-finish")
        );
        assert!(std::env::var_os("AGENT_LAB_GENERATION").is_some());
        let app = gtk::Application::builder()
            .application_id("com.olledyberg.Koett.PrivateAskProbe")
            .build();
        app.connect_activate(|app| {
            let (tx, _rx) = std::sync::mpsc::channel();
            let panel = std::rc::Rc::new(ask_view::AskView::new(app, tx));
            panel.hide();
            if std::env::args().any(|arg| arg == "--narrow") {
                for window in app.windows() { window.set_default_size(360, 440); }
            }
            panel.update(&serde_json::json!({"reset": true, "screen": true, "question": "Owned synthetic question", "phase": "Answering…"}));
            let mut count = 0;
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
                if count < 5 { panel.update(&serde_json::json!({"delta": (["**Synthetic** answer.\n\n", "```rust\nlet answer = 42;\n```\n\n", "| Item | Count |\n| --- | --- |\n| Test | 1 |\n\n", "$x^2$\n\n", "<script>document.body.textContent='UNSAFE'</script>"][count])})); count += 1; gtk::glib::ControlFlow::Continue }
                else { panel.update(&serde_json::json!({"error": "Synthetic disconnect; partial answer retained", "warning": "Synthetic storage warning"})); gtk::glib::ControlFlow::Break }
            });
        });
        app.run_with_args::<&str>(&[]);
    }
}
#[cfg(target_os = "linux")]
fn main() {
    probe::run();
}
#[cfg(not(target_os = "linux"))]
fn main() {}
