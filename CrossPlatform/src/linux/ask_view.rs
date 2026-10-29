#[path = "ask_position.rs"]
mod ask_position;
#[path = "ask_renderer.rs"]
mod ask_renderer;
use gtk4::{self as gtk, prelude::*};
use std::{cell::RefCell, rc::Rc};
pub struct AskView {
    window: gtk::ApplicationWindow,
    question: gtk::Label,
    screen: gtk::Label,
    phase: gtk::Label,
    warning: gtk::Label,
    speech: gtk::Label,
    capability: gtk::Label,
    error: gtk::Label,
    buffer: gtk::TextBuffer,
    renderer: RefCell<Option<ask_renderer::Renderer>>,
    surface: gtk::Stack,
    copy: gtk::Button,
    answer: Rc<RefCell<String>>,
}
impl AskView {
    pub fn new(
        app: &gtk::Application,
        actions: std::sync::mpsc::Sender<super::TrayAction>,
    ) -> Self {
        let height = gtk::gdk::Display::default()
            .and_then(|display| display.monitors().item(0))
            .and_then(|monitor| monitor.downcast::<gtk::gdk::Monitor>().ok())
            .map_or(580, |monitor| {
                (monitor.geometry().height() * 72 / 100).clamp(440, 660)
            });
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Koett Ask")
            .default_width(480)
            .default_height(height)
            .build();
        if let Err(message) = ask_position::configure(&window) {
            eprintln!("{message}");
        }
        let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
        for side in [
            gtk::PositionType::Left,
            gtk::PositionType::Right,
            gtk::PositionType::Top,
            gtk::PositionType::Bottom,
        ] {
            match side {
                gtk::PositionType::Left => column.set_margin_start(22),
                gtk::PositionType::Right => column.set_margin_end(22),
                gtk::PositionType::Top => column.set_margin_top(22),
                _ => column.set_margin_bottom(22),
            }
        }
        let title = gtk::Label::new(Some("Koett Ask"));
        title.add_css_class("title-3");
        column.append(&title);
        let label = || {
            let value = gtk::Label::new(None);
            value.set_wrap(true);
            value.set_xalign(0.0);
            value.set_selectable(true);
            value.set_visible(false);
            column.append(&value);
            value
        };
        let screen = label();
        screen.set_selectable(false);
        let question_caption = gtk::Label::new(Some("Question"));
        question_caption.set_xalign(0.0);
        column.append(&question_caption);
        let question = label();
        let capability = label();
        let warning = label();
        let speech = label();
        let error = label();
        let text = gtk::TextView::new();
        text.add_css_class("koett-ask-answer");
        text.set_editable(false);
        text.set_cursor_visible(false);
        text.set_wrap_mode(gtk::WrapMode::WordChar);
        let buffer = text.buffer();
        let scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .child(&text)
            .build();
        let surface = gtk::Stack::new();
        surface.set_vexpand(true);
        surface.add_named(&scroll, Some("plain"));
        column.append(&surface);
        let phase = gtk::Label::new(None);
        column.append(&phase);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let copy = gtk::Button::with_label("Copy answer");
        copy.set_sensitive(false);
        let answer = Rc::new(RefCell::new(String::new()));
        let copied = answer.clone();
        copy.connect_clicked(move |button| {
            button.clipboard().set_text(&copied.borrow());
        });
        buttons.append(&copy);
        let cancel = gtk::Button::with_label("Cancel");
        cancel.connect_clicked(move |_| {
            let _ = actions.send(super::TrayAction::CancelAsk);
        });
        buttons.append(&cancel);
        let close = gtk::Button::with_label("Close");
        let weak = window.downgrade();
        close.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.set_visible(false);
            }
        });
        buttons.append(&close);
        column.append(&buttons);
        window.set_child(Some(&column));
        window.set_hide_on_close(true);
        let keyboard = gtk::EventControllerKey::new();
        let weak = window.downgrade();
        keyboard.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                if let Some(window) = weak.upgrade() {
                    window.set_visible(false);
                }
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        });
        window.add_controller(keyboard);
        Self {
            window,
            question,
            screen,
            phase,
            warning,
            speech,
            capability,
            error,
            buffer,
            renderer: RefCell::new(None),
            surface,
            copy,
            answer,
        }
    }
    pub fn hide(&self) {
        self.window.set_visible(false);
    }
    pub fn update(&self, value: &serde_json::Value) {
        if value["reset"] == true {
            if self.renderer.borrow().is_none() {
                match ask_renderer::Renderer::new() {
                    Ok(renderer) => {
                        let surface = self.surface.clone();
                        let capability = self.capability.clone();
                        renderer
                            .widget
                            .connect_local("web-process-terminated", false, move |_| {
                                surface.set_visible_child_name("plain");
                                capability.set_text(
                                    "Rich rendering stopped; plain answer remains available.",
                                );
                                capability.set_visible(true);
                                None
                            });
                        self.surface.add_named(&renderer.widget, Some("rich"));
                        self.surface.set_visible_child_name("rich");
                        self.renderer.replace(Some(renderer));
                    }
                    Err(message) => {
                        self.capability.set_text(&message);
                        self.capability.set_visible(true);
                    }
                }
            }
            self.answer.borrow_mut().clear();
            self.buffer.set_text("");
            if let Some(renderer) = &*self.renderer.borrow() {
                renderer.update("");
            }
            self.question.set_text("");
            self.speech.set_text("");
            self.speech.set_visible(false);
            self.warning.set_text("");
            self.warning.set_visible(false);
            self.error.set_visible(false);
            self.question.set_visible(false);
            self.error.set_text("");
            self.copy.set_sensitive(false);
            self.screen.set_text(if value["screen"] == true {
                "Screen included"
            } else {
                ""
            });
            self.screen.set_visible(value["screen"] == true);
            self.window.present();
        }
        if value["screen"] == false {
            self.screen.set_text("");
            self.screen.set_visible(false);
        }
        if let Some(s) = value["question"].as_str() {
            self.question.set_text(s);
            self.question.set_visible(!s.is_empty());
        }
        if value["phase"] == "Cancelled" {
            self.speech.set_text("");
            self.speech.set_visible(false);
        }
        if let Some(s) = value["speech_notice"].as_str() {
            self.speech.set_text(s);
            self.speech.set_visible(!s.is_empty());
        }
        if let Some(s) = value["warning"].as_str() {
            self.warning.set_text(s);
            self.warning.set_visible(!s.is_empty());
        }
        if let Some(s) = value["error"].as_str() {
            self.error.set_text(s);
            self.error.set_visible(!s.is_empty());
            self.phase.set_text("Could not answer");
            if !self.window.is_visible() {
                self.window.set_visible(true);
            }
        }
        if let Some(s) = value["phase"].as_str() {
            self.phase.set_text(s);
        }
        if let Some(s) = value["delta"].as_str() {
            self.answer.borrow_mut().push_str(s);
            self.buffer.set_text(&self.answer.borrow());
            if let Some(renderer) = &*self.renderer.borrow() {
                renderer.update(&self.answer.borrow());
            }
            self.copy.set_sensitive(!self.answer.borrow().is_empty());
        }
    }
}

#[cfg(test)]
mod spoken_fixture {
    use super::*;
    use crate::assistant::{
        self,
        speech::{Configuration, Speech},
    };
    use std::{cell::Cell, sync::atomic::AtomicBool, time::Duration};
    #[test]
    #[ignore = "requires private speech WSS and desktop fixture"]
    fn native_spoken_answer() {
        assert!(
            std::env::var("XDG_RUNTIME_DIR")
                .unwrap()
                .rsplit_once("/agent-lab/")
                .is_some_and(|(_, lab)| matches!(lab, "koett-speech" | "koett-nemotron"))
        );
        let markers = std::path::PathBuf::from(std::env::var("KOETT_SPEECH_MARKERS").unwrap());
        assistant::secret(
            "cartesia",
            Some("private-fixture-only"),
            &AtomicBool::new(false),
        )
        .unwrap();
        gtk::init().unwrap();
        let app = gtk::Application::builder()
            .application_id("com.olledyberg.Koett.SpeechFixture")
            .build();
        app.register(None::<&gtk::gio::Cancellable>).unwrap();
        let (actions, _rx) = std::sync::mpsc::channel();
        let view = Rc::new(AskView::new(&app, actions));
        view.update(&serde_json::json!({"reset":true,"screen":true,"question":"Private speech fixture","phase":"Answering…"}));
        let speech = Rc::new(RefCell::new(
            Speech::prepare(Configuration {
                enabled: true,
                voice: "Katie".into(),
            })
            .unwrap(),
        ));
        let answer = "Hello private fixture. Read [the docs](https://example.com).\n\n```rust\nlet answer = 42;\n```";
        let delivered = Rc::new(Cell::new(false));
        let playing = Rc::new(Cell::new(false));
        let poll_view = view.clone();
        let poll_speech = speech.clone();
        let poll_playing = playing.clone();
        let poll_markers = markers.clone();
        gtk::glib::timeout_add_local(Duration::from_millis(20), move || {
            for notice in poll_speech.borrow().notices.try_iter() {
                if notice == "Speaking…" {
                    assert!(!poll_view.answer.borrow().is_empty());
                    poll_playing.set(true);
                    std::fs::write(poll_markers.join("playing"), b"ready").unwrap();
                }
                if notice == "Spoken reply unavailable; text answer continues" {
                    std::fs::write(poll_markers.join("error"), b"ready").unwrap();
                }
                poll_view.update(&serde_json::json!({"speech_notice":notice}));
            }
            gtk::glib::ControlFlow::Continue
        });
        let text_view = view.clone();
        let text_speech = speech.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(700), move || {
            text_view.update(&serde_json::json!({"delta":answer,"phase":"Done"}));
            text_speech.borrow_mut().append(answer);
            text_speech.borrow_mut().finish();
        });
        let text_view = view.clone();
        let copied = delivered.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(1500), move || {
            text_view.copy.emit_clicked();
            let clipboard = text_view.copy.clipboard();
            gtk::glib::MainContext::default().spawn_local(async move {
                assert_eq!(
                    clipboard.read_text_future().await.unwrap().as_deref(),
                    Some(answer)
                );
                copied.set(true);
            });
        });
        let cancel_speech = speech.clone();
        let cancel_view = view.clone();
        gtk::glib::timeout_add_local_once(Duration::from_secs(2), move || {
            cancel_speech.borrow_mut().stop();
            *cancel_speech.borrow_mut() = Speech::prepare(Configuration {
                enabled: true,
                voice: "Katie".into(),
            })
            .unwrap();
            let next = cancel_speech.clone();
            gtk::glib::timeout_add_local_once(Duration::from_millis(250), move || {
                next.borrow_mut().stop()
            });
            cancel_view.update(&serde_json::json!({"phase":"Cancelled"}));
            std::fs::write(markers.join("cancelled"), b"ready").unwrap();
        });
        let main_loop = gtk::glib::MainLoop::new(None, false);
        let done = main_loop.clone();
        gtk::glib::timeout_add_local_once(Duration::from_secs(4), move || done.quit());
        main_loop.run();
        assert_eq!(
            playing.get(),
            std::env::var_os("KOETT_SPEECH_ERROR").is_none()
        );
        assert!(delivered.get());
        assert_eq!(view.answer.borrow().as_str(), answer);
        speech.borrow_mut().stop();
        view.hide();
    }
}
