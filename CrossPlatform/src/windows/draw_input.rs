use crate::settings::DrawKey;
use std::mem::size_of;
use std::time::{Duration, Instant};
use windows::Win32::{
    Foundation::{HWND, LPARAM},
    UI::Input::{KeyboardAndMouse::GetAsyncKeyState, *},
};

const ARM: Duration = Duration::from_millis(120);
const IDLE: Duration = Duration::from_secs(15);

#[derive(Default)]
struct Gate {
    seen_up: bool,
    spoiled: bool,
    since: Option<Instant>,
    activity: Option<Instant>,
}
impl Gate {
    fn sample(&mut self, held: bool, others: bool, now: Instant) -> bool {
        if !held {
            *self = Self {
                seen_up: true,
                ..Self::default()
            };
            return false;
        }
        if others
            || !self.seen_up
            || self
                .activity
                .is_some_and(|at| now.saturating_duration_since(at) >= IDLE)
        {
            self.spoiled = true;
        }
        if self.spoiled {
            self.since = None;
            return false;
        }
        let since = *self.since.get_or_insert(now);
        let active = now.saturating_duration_since(since) >= ARM;
        if active && self.activity.is_none() {
            self.activity = Some(now);
        }
        active
    }
}

pub(super) struct Monitor {
    key: DrawKey,
    known: bool,
    held: [bool; 2],
    gate: Gate,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct State {
    pub protect_clicks: bool,
    pub ink: bool,
    pub discard: bool,
}

impl Monitor {
    pub fn start(hwnd: HWND, key: DrawKey) -> Result<Self, String> {
        unsafe {
            RegisterRawInputDevices(
                &[RAWINPUTDEVICE {
                    usUsagePage: 1,
                    usUsage: 6,
                    dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                    hwndTarget: hwnd,
                }],
                size_of::<RAWINPUTDEVICE>() as u32,
            )
        }
        .map_err(|e| format!("Draw key unavailable: {e}"))?;
        let held = sides(key).map(down);
        let mut gate = Gate::default();
        gate.sample(held.iter().any(|v| *v), others(key), Instant::now());
        Ok(Self {
            key,
            known: true,
            held,
            gate,
        })
    }

    pub fn input(&mut self, parameter: LPARAM, now: Instant) {
        let mut raw = RAWINPUT::default();
        let mut length = size_of::<RAWINPUT>() as u32;
        let got = unsafe {
            GetRawInputData(
                HRAWINPUT(parameter.0 as *mut _),
                RID_INPUT,
                Some((&mut raw as *mut RAWINPUT).cast()),
                &mut length,
                size_of::<RAWINPUTHEADER>() as u32,
            )
        };
        let change = if got != u32::MAX
            && got as usize <= size_of::<RAWINPUT>()
            && got as usize >= size_of::<RAWINPUTHEADER>() + size_of::<RAWKEYBOARD>()
            && raw.header.dwType == RIM_TYPEKEYBOARD.0
            && !raw.header.hDevice.is_invalid()
        {
            let keyboard = unsafe { &raw.data.keyboard };
            let pressed = keyboard.Flags & 1 == 0;
            let side = side(self.key, keyboard.VKey, keyboard.MakeCode, keyboard.Flags);
            Some((side, pressed))
        } else {
            None
        };
        // Only modifier transitions and a yes/no for another key survive the read.
        unsafe { std::ptr::write_volatile(&mut raw, RAWINPUT::default()) };
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
        if let Some((side, pressed)) = change {
            if let Some(side) = side {
                self.held[side] = pressed;
                if !pressed && !self.held.iter().any(|v| *v) {
                    self.known = true;
                }
            }
            if !self.known {
                return;
            }
            self.gate.sample(
                self.held.iter().any(|v| *v),
                (side.is_none() && pressed) || others(self.key),
                now,
            );
        }
    }

    pub fn state(&mut self, now: Instant) -> State {
        if !self.known {
            return State::default();
        }
        let held = self.held.iter().any(|v| *v);
        let active = self.gate.sample(held, others(self.key), now);
        State {
            protect_clicks: !self.gate.spoiled && self.gate.since.is_some(),
            ink: active,
            discard: self.gate.spoiled,
        }
    }

    pub fn moved(&mut self, now: Instant) {
        self.gate.activity = Some(now);
    }

    pub fn device_changed(&mut self) {
        self.known = false;
        self.held = [false; 2];
        self.gate = Gate::default();
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        let result = unsafe {
            RegisterRawInputDevices(
                &[RAWINPUTDEVICE {
                    usUsagePage: 1,
                    usUsage: 6,
                    dwFlags: RIDEV_REMOVE,
                    hwndTarget: HWND::default(),
                }],
                size_of::<RAWINPUTDEVICE>() as u32,
            )
        };
        if let Err(e) = result {
            eprintln!("Could not release draw-key registration: {e}");
        }
    }
}

fn sides(key: DrawKey) -> [u16; 2] {
    match key {
        DrawKey::Shift => [0xa0, 0xa1],
        DrawKey::Control => [0xa2, 0xa3],
        DrawKey::Alt => [0xa4, 0xa5],
    }
}
fn down(key: u16) -> bool {
    unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
}
fn others(key: DrawKey) -> bool {
    let family = match key {
        DrawKey::Shift => 0x10,
        DrawKey::Control => 0x11,
        DrawKey::Alt => 0x12,
    };
    let sides = sides(key);
    (8..=254).any(|vk| vk != family && !sides.contains(&vk) && down(vk))
}
fn side(key: DrawKey, vk: u16, scan: u16, flags: u16) -> Option<usize> {
    let keys = sides(key);
    if vk == keys[0] {
        return Some(0);
    }
    if vk == keys[1] {
        return Some(1);
    }
    match (key, vk) {
        (DrawKey::Shift, 0x10) => match scan {
            0x2a => Some(0),
            0x36 => Some(1),
            _ => None,
        },
        (DrawKey::Control, 0x11) | (DrawKey::Alt, 0x12) => Some(usize::from(flags & 2 != 0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_hold_and_chord_cannot_become_ink_without_release() {
        let t = Instant::now();
        let mut g = Gate::default();
        assert!(!g.sample(true, false, t));
        assert!(!g.sample(true, false, t + ARM));
        g.sample(false, false, t + ARM);
        assert!(!g.sample(true, true, t + ARM));
        assert!(!g.sample(true, false, t + ARM * 2));
        g.sample(false, false, t + ARM * 2);
        assert!(!g.sample(true, false, t + ARM * 2));
        assert!(g.sample(true, false, t + ARM * 3));
        assert!(!g.sample(true, true, t + ARM * 3));
        assert!(!g.sample(true, false, t + ARM * 4));
    }
    #[test]
    fn raw_key_sides_and_idle_release_do_not_read_characters() {
        assert_eq!(side(DrawKey::Shift, 0x10, 0x36, 0), Some(1));
        assert_eq!(side(DrawKey::Control, 0x11, 0, 2), Some(1));
        assert_eq!(side(DrawKey::Shift, b'A' as u16, 0, 0), None);
        let t = Instant::now();
        let mut g = Gate::default();
        g.sample(false, false, t);
        g.sample(true, false, t);
        assert!(g.sample(true, false, t + ARM));
        assert!(!g.sample(true, false, t + ARM + IDLE));
    }
}
