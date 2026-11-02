//! Draw-key state straight from keyboard devices, so the sketch overlay never
//! needs keyboard focus and typing keeps working while recording.
//!
//! Privacy boundary: Koett never reads key events. While a recording with
//! drawing is active it asks the kernel for each keyboard's current key-state
//! bitmap (`EVIOCGKEY`), keeps only the six modifier bits and zeroes the rest
//! at once. Devices are opened only for that recording and closed after it.
use crate::settings::DrawKey;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;

const EV_KEY: u32 = 0x01;
const KEY_LEFTCTRL: usize = 29;
const KEY_LEFTSHIFT: usize = 42;
const KEY_RIGHTSHIFT: usize = 54;
const KEY_LEFTALT: usize = 56;
const KEY_RIGHTCTRL: usize = 97;
const KEY_RIGHTALT: usize = 100;
const KEY_LEFTMETA: usize = 125;
const KEY_RIGHTMETA: usize = 126;
/// Bytes for KEY_MAX (0x2ff) + 1 bits.
const KEY_BYTES: usize = 0x300 / 8;

/// Linux `_IOC(_IOC_READ, 'E', nr, size)`.
const fn eviocg(nr: u32, size: usize) -> libc::c_ulong {
    ((2 << 30) | ((size as u32) << 16) | ((b'E' as u32) << 8) | nr) as libc::c_ulong
}

fn bit(bits: &[u8], index: usize) -> bool {
    bits.get(index / 8)
        .is_some_and(|byte| byte & (1 << (index % 8)) != 0)
}

/// Modifiers currently held on any keyboard.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    fn from_bits(bits: &[u8]) -> Self {
        Self {
            shift: bit(bits, KEY_LEFTSHIFT) || bit(bits, KEY_RIGHTSHIFT),
            control: bit(bits, KEY_LEFTCTRL) || bit(bits, KEY_RIGHTCTRL),
            alt: bit(bits, KEY_LEFTALT) || bit(bits, KEY_RIGHTALT),
            meta: bit(bits, KEY_LEFTMETA) || bit(bits, KEY_RIGHTMETA),
        }
    }

    fn or(self, other: Self) -> Self {
        Self {
            shift: self.shift || other.shift,
            control: self.control || other.control,
            alt: self.alt || other.alt,
            meta: self.meta || other.meta,
        }
    }

    /// The draw key alone is held: chords such as Ctrl+Shift+Space (the
    /// default dictation shortcut) never start a sketch.
    pub fn draws(self, key: DrawKey) -> bool {
        let (held, others) = match key {
            DrawKey::Shift => (self.shift, self.control || self.alt || self.meta),
            DrawKey::Control => (self.control, self.shift || self.alt || self.meta),
            DrawKey::Alt => (self.alt, self.shift || self.control || self.meta),
        };
        held && !others
    }
}

/// Why keyboard devices cannot be read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    /// Keyboards exist but this user may not open them.
    Denied,
    /// No keyboard devices were found.
    NoKeyboards,
}

/// Open keyboards for one recording.
pub struct Keyboards(Vec<File>);

impl Keyboards {
    pub fn open() -> Result<Self, Access> {
        Self::open_in(Path::new("/dev/input"))
    }

    fn open_in(directory: &Path) -> Result<Self, Access> {
        let mut keyboards = Vec::new();
        let mut denied = false;
        let entries = std::fs::read_dir(directory).map_err(|_| Access::NoKeyboards)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("event"))
            {
                continue;
            }
            match std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(&path)
            {
                Ok(file) if has_shift(&file) => keyboards.push(file),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    denied = true;
                }
                Err(_) => {}
            }
        }
        match (keyboards.is_empty(), denied) {
            (false, _) => Ok(Self(keyboards)),
            (true, true) => Err(Access::Denied),
            (true, false) => Err(Access::NoKeyboards),
        }
    }

    /// Current modifiers across all keyboards; no other key state is kept.
    pub fn modifiers(&self) -> Modifiers {
        self.0.iter().fold(Modifiers::default(), |held, file| {
            let mut bits = [0_u8; KEY_BYTES];
            // SAFETY: EVIOCGKEY writes at most KEY_BYTES into the live buffer.
            let read = unsafe {
                libc::ioctl(file.as_raw_fd(), eviocg(0x18, KEY_BYTES), bits.as_mut_ptr())
            };
            let current = if read < 0 {
                Modifiers::default()
            } else {
                Modifiers::from_bits(&bits)
            };
            bits.fill(0);
            held.or(current)
        })
    }
}

use std::os::unix::fs::OpenOptionsExt as _;

/// Whether a device reports Shift keys (keyboards, not mice or buttons).
fn has_shift(file: &File) -> bool {
    let mut bits = [0_u8; KEY_BYTES];
    // SAFETY: EVIOCGBIT(EV_KEY) writes at most KEY_BYTES into the buffer.
    let read = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            eviocg(0x20 + EV_KEY, KEY_BYTES),
            bits.as_mut_ptr(),
        )
    };
    read > 0 && bit(&bits, KEY_LEFTSHIFT)
}

/// Shown in Settings: grants the active seat user read access to keyboards.
pub const SETUP_COMMAND: &str = "echo 'SUBSYSTEM==\"input\", KERNEL==\"event*\", ENV{ID_INPUT_KEYBOARD}==\"1\", TAG+=\"uaccess\"' | sudo tee /etc/udev/rules.d/70-koett-keyboard.rules && sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=input --action=change";

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(keys: &[usize]) -> [u8; KEY_BYTES] {
        let mut bits = [0_u8; KEY_BYTES];
        for key in keys {
            bits[key / 8] |= 1 << (key % 8);
        }
        bits
    }

    #[test]
    fn ioctl_numbers_match_linux_headers() {
        // EVIOCGKEY(96) and EVIOCGBIT(EV_KEY, 96) from linux/input.h.
        assert_eq!(eviocg(0x18, KEY_BYTES), 0x8060_4518);
        assert_eq!(eviocg(0x21, KEY_BYTES), 0x8060_4521);
    }

    #[test]
    fn only_modifier_bits_are_interpreted() {
        let held = Modifiers::from_bits(&bits(&[KEY_RIGHTSHIFT, 30, 31, 200]));
        assert_eq!(
            held,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            }
        );
    }

    #[test]
    fn the_draw_key_alone_draws_but_chords_do_not() {
        let shift = Modifiers::from_bits(&bits(&[KEY_LEFTSHIFT]));
        assert!(shift.draws(DrawKey::Shift));
        assert!(!shift.draws(DrawKey::Control));
        let chord = Modifiers::from_bits(&bits(&[KEY_LEFTSHIFT, KEY_LEFTCTRL]));
        assert!(
            !chord.draws(DrawKey::Shift),
            "Ctrl+Shift+Space starts dictation"
        );
        let alt = Modifiers::from_bits(&bits(&[KEY_RIGHTALT]));
        assert!(alt.draws(DrawKey::Alt));
        assert!(!Modifiers::default().draws(DrawKey::Shift));
    }

    #[test]
    fn missing_directory_reports_no_keyboards() {
        let missing = std::env::temp_dir().join("koett-no-input-devices");
        assert_eq!(
            Keyboards::open_in(&missing).err(),
            Some(Access::NoKeyboards)
        );
    }
}
