use crate::audio::preview::{self, Capture};
use gtk4::{self as gtk, prelude::*};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, PartialEq)]
struct Request {
    generation: u64,
    microphone: Option<String>,
}

#[derive(Clone)]
struct Reading {
    generation: u64,
    level: f64,
    message: String,
}

#[derive(Default)]
struct State {
    request: Option<Request>,
    reading: Option<Reading>,
    closed: bool,
    generation: u64,
}

impl State {
    fn select(&mut self, microphone: Option<String>) {
        self.generation += 1;
        let generation = self.generation;
        self.request = Some(Request {
            generation,
            microphone,
        });
        self.reading = None;
    }

    fn publish(&mut self, request: &Request, level: f64, message: &str) {
        if !self.closed && self.request.as_ref() == Some(request) {
            self.reading = Some(Reading {
                generation: request.generation,
                level,
                message: message.into(),
            });
        }
    }

    fn close(&mut self) {
        self.closed = true;
        self.request = None;
        self.reading = None;
    }
}

pub(super) fn attach(
    window: &gtk::Window,
    input: &gtk::ComboBoxText,
    layout: &gtk::Box,
    stack: &gtk::Stack,
) {
    let meter = gtk::LevelBar::for_interval(0.0, 1.0);
    meter.set_hexpand(true);
    let title = gtk::Label::new(Some("Input level"));
    title.set_xalign(0.0);
    title.set_mnemonic_widget(Some(&meter));
    layout.append(&title);
    layout.append(&meter);
    let message = gtk::Label::new(Some("Opening microphone…"));
    message.set_xalign(0.0);
    message.set_wrap(true);
    layout.append(&message);
    let state = Arc::new(Mutex::new(State::default()));
    let select = {
        let state = state.clone();
        let meter = meter.clone();
        let message = message.clone();
        let stack = stack.downgrade();
        move |input: &gtk::ComboBoxText| {
            meter.set_value(0.0);
            message.set_text("Opening microphone…");
            message.set_visible(true);
            if stack
                .upgrade()
                .and_then(|stack| stack.visible_child_name())
                .as_deref()
                != Some("audio")
            {
                return;
            }
            state.lock().expect("preview state poisoned").select(
                input
                    .active_id()
                    .filter(|id| !id.is_empty())
                    .map(|id| id.to_string()),
            );
        }
    };
    select(input);
    let select = std::rc::Rc::new(select);
    input.connect_changed({
        let select = select.clone();
        move |input| select(input)
    });
    stack.connect_visible_child_name_notify({
        let state = state.clone();
        let input = input.clone();
        let meter = meter.clone();
        move |stack| {
            if stack.visible_child_name().as_deref() == Some("audio") {
                select(&input);
            } else {
                let mut state = state.lock().expect("preview state poisoned");
                state.request = None;
                state.reading = None;
                meter.set_value(0.0);
            }
        }
    });
    window.connect_close_request({
        let state = state.clone();
        let meter = meter.clone();
        move |_| {
            state.lock().expect("preview state poisoned").close();
            meter.set_value(0.0);
            gtk::glib::Propagation::Proceed
        }
    });
    window.connect_unmap({
        let state = state.clone();
        let meter = meter.clone();
        move |_| {
            state.lock().expect("preview state poisoned").close();
            meter.set_value(0.0);
        }
    });
    window.connect_destroy({
        let state = state.clone();
        move |_| state.lock().expect("preview state poisoned").close()
    });
    match std::thread::Builder::new()
        .name("koett-input-preview".into())
        .spawn({
            let state = state.clone();
            move || run(state)
        }) {
        Ok(_) => {}
        Err(error) => {
            message.set_text(&format!("Cannot start microphone preview: {error}"));
            return;
        }
    }
    gtk::glib::timeout_add_local(Duration::from_millis(50), {
        let window = window.downgrade();
        move || {
            let mut state = state.lock().expect("preview state poisoned");
            if state.closed || window.upgrade().is_none() {
                state.close();
                return gtk::glib::ControlFlow::Break;
            }
            if preview::recording() {
                meter.set_value(0.0);
                message.set_text("Preview paused while recording");
                message.set_visible(true);
            } else if let Some(reading) = &state.reading
                && state
                    .request
                    .as_ref()
                    .is_some_and(|request| request.generation == reading.generation)
            {
                meter.set_value(reading.level);
                message.set_text(&reading.message);
                message.set_visible(!reading.message.is_empty());
            }
            gtk::glib::ControlFlow::Continue
        }
    });
}

fn run(state: Arc<Mutex<State>>) {
    let mut current = None;
    let mut capture = None;
    let mut failed = false;
    let mut feedback = super::feedback::CaptureFeedback::default();
    let mut started = std::time::Instant::now();
    loop {
        let request = {
            let state = state.lock().expect("preview state poisoned");
            if state.closed {
                break;
            }
            state.request.clone()
        };
        if current != request {
            capture.take();
            current = request;
            failed = false;
            feedback = super::feedback::CaptureFeedback::default();
            started = std::time::Instant::now();
        }
        let Some(request) = &current else {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        };
        let (level, message) = if preview::recording() {
            capture.take();
            failed = false;
            feedback = super::feedback::CaptureFeedback::default();
            started = std::time::Instant::now();
            (0.0, "Preview paused while recording".to_string())
        } else if failed {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        } else {
            if capture.is_none() {
                match Capture::start(request.microphone.as_deref(), || {
                    let state = state.lock().expect("preview state poisoned");
                    !state.closed && state.request.as_ref() == Some(request)
                }) {
                    Ok(opened) => capture = opened,
                    Err(error) => {
                        state
                            .lock()
                            .expect("preview state poisoned")
                            .publish(request, 0.0, &error);
                        failed = true;
                        continue;
                    }
                }
            }
            match capture.as_ref().map(Capture::levels) {
                Some(Ok((rms, peak))) => {
                    let warning = feedback.update(started.elapsed(), rms, peak);
                    (
                        f64::from((rms * 10.0).clamp(0.0, 1.0)),
                        warning.map_or(String::new(), |warning| warning.text().into()),
                    )
                }
                Some(Err(error)) => {
                    capture.take();
                    failed = true;
                    (0.0, error)
                }
                None => (0.0, "Opening microphone…".into()),
            }
        };
        state
            .lock()
            .expect("preview state poisoned")
            .publish(request, level, &message);
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_and_close_reject_late_levels_and_errors() {
        let mut state = State::default();
        state.select(Some("first".into()));
        let first = state.request.clone().unwrap();
        state.publish(&first, 0.7, "");
        assert_eq!(state.reading.as_ref().unwrap().level, 0.7);
        state.select(Some("second".into()));
        state.publish(&first, 0.9, "old error");
        assert!(state.reading.is_none());
        let second = state.request.clone().unwrap();
        state.publish(&second, 0.0, "new error");
        assert_eq!(state.reading.as_ref().unwrap().message, "new error");
        state.close();
        state.publish(&second, 1.0, "late error");
        assert!(state.reading.is_none());
        assert!(state.request.is_none());
    }
}
