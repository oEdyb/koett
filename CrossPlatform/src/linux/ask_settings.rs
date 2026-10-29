use gtk4::{self as gtk, prelude::*};
pub fn build(column: &gtk::Box) {
    let configuration = crate::assistant::Configuration::load().unwrap_or_default();
    let enabled = gtk::CheckButton::with_label("Enable cloud Ask with one screen image");
    enabled.set_active(configuration.enabled);
    column.append(&enabled);
    let provider = gtk::ComboBoxText::new();
    for (id, label) in [
        ("groq", "Groq"),
        ("openRouter", "OpenRouter"),
        ("custom", "Custom"),
    ] {
        provider.append(Some(id), label);
    }
    provider.set_active_id(Some(&configuration.provider));
    column.append(&provider);
    let field = |label: &str, value: &str| {
        let caption = gtk::Label::new(Some(label));
        caption.set_xalign(0.0);
        column.append(&caption);
        let entry = gtk::Entry::new();
        entry.set_text(value);
        column.append(&entry);
        entry
    };
    let model = field("Model", &configuration.model);
    let endpoint = field("HTTPS endpoint", &configuration.endpoint);
    provider.connect_changed({
        let model = model.clone();
        let endpoint = endpoint.clone();
        move |provider| {
            let (url, name) = match provider.active_id().as_deref() {
                Some("groq") => (
                    "https://api.groq.com/openai/v1/chat/completions",
                    "qwen/qwen3.6-27b",
                ),
                Some("openRouter") => (
                    "https://openrouter.ai/api/v1/chat/completions",
                    "~openai/gpt-latest",
                ),
                _ => ("https://api.openai.com/v1/chat/completions", ""),
            };
            model.set_text(name);
            endpoint.set_text(url);
        }
    });
    let key = gtk::PasswordEntry::new();
    key.set_show_peek_icon(true);
    let caption = gtk::Label::new(Some("API key"));
    caption.set_xalign(0.0);
    column.append(&caption);
    column.append(&key);
    let save = gtk::Button::with_label("Save Ask configuration");
    column.append(&save);
    let result = gtk::Label::new(None);
    result.set_wrap(true);
    column.append(&result);
    speech_controls(column);
    save.connect_clicked(move |save| {
        let config = crate::assistant::Configuration {
            enabled: enabled.is_active(),
            provider: provider
                .active_id()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            model: model.text().to_string(),
            endpoint: endpoint.text().to_string(),
        };
        if let Err(error) = config.validate() {
            result.set_text(&error);
            return;
        }
        let secret = key.text().to_string();
        key.set_text("");
        save.set_sensitive(false);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let status = (|| {
                if !secret.is_empty() {
                    crate::assistant::secret(
                        &config.provider,
                        Some(&secret),
                        &std::sync::atomic::AtomicBool::new(false),
                    )?;
                }
                config.save()
            })();
            let _ = tx.send(status);
        });
        let save = save.clone();
        let result = result.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            if let Ok(status) = rx.try_recv() {
                save.set_sensitive(true);
                result.set_text(match &status {
                    Ok(()) => "Saved",
                    Err(e) => e,
                });
                gtk::glib::ControlFlow::Break
            } else {
                gtk::glib::ControlFlow::Continue
            }
        });
    });
}

fn speech_controls(column: &gtk::Box) {
    let config = crate::assistant::speech::Configuration::load().unwrap_or_default();
    let enabled = gtk::CheckButton::with_label("Speak Replies");
    enabled.set_active(config.enabled);
    column.append(&enabled);
    let caption = gtk::Label::new(Some("Voice"));
    caption.set_xalign(0.0);
    column.append(&caption);
    let voice = gtk::ComboBoxText::new();
    for (name, _) in crate::assistant::speech::VOICES {
        voice.append(Some(name), name);
    }
    super::menu::set_accessible_label(voice.upcast_ref(), "Voice");
    voice.set_active_id(Some(&config.voice));
    column.append(&voice);
    let caption = gtk::Label::new(Some("Cartesia key"));
    caption.set_xalign(0.0);
    column.append(&caption);
    let key = gtk::PasswordEntry::new();
    super::menu::set_accessible_label(key.upcast_ref(), "Cartesia key");
    column.append(&key);
    let save = gtk::Button::with_label("Save speech configuration");
    column.append(&save);
    let result = gtk::Label::new(None);
    result.set_wrap(true);
    column.append(&result);
    save.connect_clicked(move |button| {
        let config = crate::assistant::speech::Configuration {
            enabled: enabled.is_active(),
            voice: voice.active_id().map(|s| s.to_string()).unwrap_or_default(),
        };
        let secret = key.text().to_string();
        key.set_text("");
        button.set_sensitive(false);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let outcome = (|| {
                if !secret.is_empty() {
                    crate::assistant::secret(
                        "cartesia",
                        Some(&secret),
                        &std::sync::atomic::AtomicBool::new(false),
                    )?;
                }
                config.save()
            })();
            let _ = tx.send(outcome);
        });
        let button = button.clone();
        let result = result.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            match rx.try_recv() {
                Ok(outcome) => {
                    button.set_sensitive(true);
                    result.set_text(match &outcome {
                        Ok(()) => "Saved",
                        Err(e) => e,
                    });
                    gtk::glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    button.set_sensitive(true);
                    result.set_text("Speech settings failed");
                    gtk::glib::ControlFlow::Break
                }
                _ => gtk::glib::ControlFlow::Continue,
            }
        });
    });
}

#[cfg(test)]
mod speech_fixture {
    use super::*;
    #[test]
    #[ignore = "requires private koett-speech desktop"]
    fn native_speech_settings() {
        assert!(
            std::env::var("XDG_RUNTIME_DIR")
                .unwrap()
                .ends_with("/agent-lab/koett-speech")
        );
        gtk::init().unwrap();
        let window = gtk::Window::builder()
            .title("Speech settings fixture")
            .default_width(420)
            .default_height(900)
            .build();
        let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
        column.set_margin_start(20);
        column.set_margin_end(20);
        column.set_margin_top(20);
        column.set_margin_bottom(20);
        build(&column);
        window.set_child(Some(&column));
        window.present();
        let main_loop = gtk::glib::MainLoop::new(None, false);
        let done = main_loop.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(25), move || done.quit());
        main_loop.run();
        window.close();
    }
}
