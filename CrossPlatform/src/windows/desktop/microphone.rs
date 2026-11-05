use super::*;
use crate::audio::{InputDevice, input_devices, preview};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(super) const INPUT: usize = 30;
pub(super) const REFRESH: usize = 31;
pub(super) const PREVIEW: usize = 32;
enum Command {
    Refresh,
    Preview(Option<String>),
    Stop,
}
#[derive(Clone, Default)]
struct Status {
    active: bool,
    message: String,
}
struct Monitor {
    commands: mpsc::Sender<Command>,
    devices: mpsc::Receiver<Result<Vec<InputDevice>, String>>,
    status: Arc<Mutex<Status>>,
}
impl Monitor {
    fn new() -> Self {
        let (commands, receive) = mpsc::channel();
        let (send_devices, devices) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let shared = status.clone();
        std::thread::spawn(move || {
            let mut capture: Option<preview::Capture> = None;
            let publish = |active, message| {
                if let Ok(mut state) = shared.lock() {
                    *state = Status { active, message };
                }
            };
            loop {
                match receive.recv_timeout(Duration::from_millis(100)) {
                    Ok(Command::Refresh) => {
                        capture = None;
                        publish(false, String::new());
                        if send_devices.send(input_devices()).is_err() {
                            break;
                        }
                    }
                    Ok(Command::Stop) => {
                        capture = None;
                        publish(false, String::new());
                    }
                    Ok(Command::Preview(id)) => {
                        capture = None;
                        match preview::Capture::start(id.as_deref(), || true) {
                            Ok(Some(started)) => {
                                capture = Some(started);
                            }
                            Ok(None) => publish(false, "Microphone is in use by dictation".into()),
                            Err(error) => publish(false, error),
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                if preview::recording() && capture.is_some() {
                    capture = None;
                    publish(false, "Microphone is in use by dictation".into());
                }
                if let Some(current) = &capture {
                    match current.levels() {
                        Ok((rms, _)) => publish(
                            true,
                            format!("Input level: {:.0}%", rms.clamp(0.0, 1.0) * 100.0),
                        ),
                        Err(error) => {
                            capture = None;
                            publish(false, error);
                        }
                    }
                }
            }
        });
        Self {
            commands,
            devices,
            status,
        }
    }
    fn status(&self) -> Status {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| Status {
                active: false,
                message: "Microphone preview unavailable".into(),
            })
    }
}
fn choices(devices: Vec<InputDevice>, selected: Option<&str>) -> Vec<(Option<String>, String)> {
    let mut rows = vec![(None, "System default".into())];
    rows.extend(devices.into_iter().map(|d| (Some(d.id), d.label)));
    if let Some(id) = selected
        && !rows.iter().any(|(key, _)| key.as_deref() == Some(id))
    {
        rows.push((
            Some(id.to_string()),
            "Selected microphone unavailable".into(),
        ));
    }
    rows
}
pub(super) struct Controls {
    input: HWND,
    button: HWND,
    meter: HWND,
    choices: Vec<(Option<String>, String)>,
    monitor: Monitor,
    displayed: String,
}
impl Controls {
    pub fn new(window: &mut Window, settings: &Settings) -> Result<Self, String> {
        window.control(
            w!("STATIC"),
            "Microphone",
            Default::default(),
            0,
            [24, 96, 200, 24],
        )?;
        let input = window.control(
            w!("COMBOBOX"),
            "",
            WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            INPUT,
            [24, 124, 360, 220],
        )?;
        window.control(
            w!("BUTTON"),
            "Refresh",
            WS_TABSTOP,
            REFRESH,
            [400, 122, 112, 32],
        )?;
        let button = window.control(
            w!("BUTTON"),
            "Preview",
            WS_TABSTOP,
            PREVIEW,
            [24, 168, 120, 32],
        )?;
        let meter = window.control(w!("STATIC"), "", Default::default(), 0, [160, 172, 352, 48])?;
        let monitor = Monitor::new();
        let mut controls = Self {
            input,
            button,
            meter,
            choices: vec![],
            monitor,
            displayed: String::new(),
        };
        controls.populate(vec![], settings.microphone.as_deref());
        let _ = controls.monitor.commands.send(Command::Refresh);
        Ok(controls)
    }
    fn populate(&mut self, devices: Vec<InputDevice>, selected: Option<&str>) {
        self.choices = choices(devices, selected);
        unsafe {
            SendMessageW(self.input, CB_RESETCONTENT, None, None);
        }
        for (_, label) in &self.choices {
            let label = super::super::wide(label);
            unsafe {
                SendMessageW(
                    self.input,
                    CB_ADDSTRING,
                    None,
                    Some(LPARAM(label.as_ptr() as isize)),
                );
            }
        }
        let index = self
            .choices
            .iter()
            .position(|(id, _)| id.as_deref() == selected)
            .unwrap_or(0);
        unsafe {
            SendMessageW(self.input, CB_SETCURSEL, Some(WPARAM(index)), None);
        }
    }
    pub fn selected(&self) -> Result<Option<String>, String> {
        let index = unsafe { SendMessageW(self.input, CB_GETCURSEL, None, None) }.0;
        usize::try_from(index)
            .ok()
            .and_then(|i| self.choices.get(i))
            .map(|(id, _)| id.clone())
            .ok_or_else(|| "Choose a microphone before saving".into())
    }
    pub fn command(&mut self, id: usize, notification: u16) -> bool {
        match id {
            REFRESH => {
                let _ = self.monitor.commands.send(Command::Refresh);
            }
            INPUT if notification == CBN_SELCHANGE as u16 => {
                let _ = self.monitor.commands.send(Command::Stop);
            }
            PREVIEW => {
                let command = if self.monitor.status().active {
                    Command::Stop
                } else {
                    match self.selected() {
                        Ok(id) => Command::Preview(id),
                        Err(error) => {
                            set(self.meter, &error);
                            return true;
                        }
                    }
                };
                let _ = self.monitor.commands.send(command);
            }
            _ => return false,
        }
        true
    }
    pub fn poll(&mut self) {
        while let Ok(result) = self.monitor.devices.try_recv() {
            match result {
                Ok(devices) => {
                    if let Ok(selected) = self.selected() {
                        self.populate(devices, selected.as_deref());
                    }
                }
                Err(error) => {
                    if let Ok(mut status) = self.monitor.status.lock() {
                        status.message = error;
                    }
                }
            }
        }
        let status = self.monitor.status();
        if status.message != self.displayed {
            set(self.meter, &status.message);
            self.displayed = status.message;
        }
        let label = if status.active {
            "Stop preview"
        } else {
            "Preview"
        };
        if read(self.button) != label {
            set(self.button, label);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_refresh_preserves_ids_and_unavailable_selection() {
        let rows = choices(
            vec![InputDevice {
                id: "stable-id".into(),
                label: "USB mic".into(),
            }],
            Some("missing-id"),
        );
        assert_eq!(rows[0], (None, "System default".into()));
        assert_eq!(rows[1], (Some("stable-id".into()), "USB mic".into()));
        assert_eq!(rows[2].0.as_deref(), Some("missing-id"));
        assert_eq!(choices(vec![], None).len(), 1);
    }
}
