//! Draw-key state straight from keyboard devices, so the sketch overlay never
//! needs keyboard focus and typing keeps working while recording.
//!
//! Privacy boundary: Koett never reads key events. While a recording with
//! drawing is active it samples each keyboard's held-key state (`EVIOCGKEY`).
//! The kernel only reports all keys at once, so a sample briefly holds which
//! keys are down; Koett uses only the Shift/Ctrl/Alt/Super bits, stores
//! nothing, and wipes the sample with volatile writes right after each poll.
//! Devices are opened only for that recording and closed after it.
use crate::settings::DrawKey;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

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

/// Clears a key-state sample so it does not outlive the poll.
fn wipe(bits: &mut [u8]) {
    for byte in bits.iter_mut() {
        // SAFETY: a valid, aligned, exclusively borrowed byte.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
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

/// Whether sysfs' `capabilities/key` (hex words, most significant first)
/// reports a Shift key, which is how keyboards are told apart from mice and
/// buttons without opening them.
fn reports_shift(capabilities: &str) -> bool {
    let words: Vec<&str> = capabilities.split_whitespace().collect();
    let word_bits = usize::BITS as usize;
    words
        .iter()
        .rev()
        .nth(KEY_LEFTSHIFT / word_bits)
        .and_then(|word| usize::from_str_radix(word, 16).ok())
        .is_some_and(|word| word & (1 << (KEY_LEFTSHIFT % word_bits)) != 0)
}

/// Open keyboards for one recording, refreshed for hot-plug.
pub struct Keyboards {
    devices: PathBuf,
    classes: PathBuf,
    open: Vec<(String, File)>,
    unreadable: Vec<String>,
}

impl Keyboards {
    pub fn open() -> Result<Self, Access> {
        Self::open_in(Path::new("/dev/input"), Path::new("/sys/class/input"))
    }

    fn open_in(devices: &Path, classes: &Path) -> Result<Self, Access> {
        let mut keyboards = Self {
            devices: devices.to_path_buf(),
            classes: classes.to_path_buf(),
            open: Vec::new(),
            unreadable: Vec::new(),
        };
        keyboards.refresh();
        match (keyboards.open.is_empty(), keyboards.unreadable.is_empty()) {
            (false, _) => Ok(keyboards),
            (true, false) => Err(Access::Denied),
            (true, true) => Err(Access::NoKeyboards),
        }
    }

    /// Opens newly attached keyboards and forgets removed ones.
    pub fn refresh(&mut self) {
        let mut present = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.classes) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let keyboard = name.starts_with("event")
                    && std::fs::read_to_string(entry.path().join("device/capabilities/key"))
                        .is_ok_and(|capabilities| reports_shift(&capabilities));
                if keyboard {
                    present.push(name);
                }
            }
        }
        self.open.retain(|(name, _)| present.contains(name));
        self.unreadable.clear();
        for name in present {
            if self.open.iter().any(|(open, _)| *open == name) {
                continue;
            }
            match std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(self.devices.join(&name))
            {
                Ok(file) => self.open.push((name, file)),
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    self.unreadable.push(name);
                }
                Err(_) => {}
            }
        }
    }

    /// Keyboards present that this user cannot open.
    pub fn unreadable(&self) -> usize {
        self.unreadable.len()
    }

    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// Current modifiers across all keyboards. A handle that no longer
    /// answers (unplugged device) is dropped.
    pub fn modifiers(&mut self) -> Modifiers {
        let mut held = Modifiers::default();
        self.open.retain(|(_, file)| {
            let mut bits = [0_u8; KEY_BYTES];
            // SAFETY: EVIOCGKEY writes at most KEY_BYTES into the live buffer.
            let read = unsafe {
                libc::ioctl(file.as_raw_fd(), eviocg(0x18, KEY_BYTES), bits.as_mut_ptr())
            };
            if read >= 0 {
                held = held.or(Modifiers::from_bits(&bits));
            }
            wipe(&mut bits);
            read >= 0
        });
        held
    }
}

/// Shown in Settings. Creates the read-only `koett-keyboard` group, adds the
/// current user, installs a udev rule adding a read-only ACL for that group on keyboard devices (owner
/// and the existing `input` group stay unchanged), and applies it. Group
/// membership takes effect after logging in again.
pub const SETUP_COMMAND: &str = "sudo groupadd -f koett-keyboard && sudo usermod -aG koett-keyboard \"$USER\" && echo 'SUBSYSTEM==\"input\", KERNEL==\"event*\", ENV{ID_INPUT_KEYBOARD}==\"1\", RUN+=\"/usr/bin/setfacl -m g:koett-keyboard:r $devnode\"' | sudo tee /etc/udev/rules.d/70-koett-keyboard.rules && sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=input --action=change";

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn bits(keys: &[usize]) -> [u8; KEY_BYTES] {
        let mut bits = [0_u8; KEY_BYTES];
        for key in keys {
            bits[key / 8] |= 1 << (key % 8);
        }
        bits
    }

    /// A fake /dev/input + /sys/class/input pair; device nodes are plain files.
    struct Fake(PathBuf);

    impl Fake {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("koett-keys-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("dev")).unwrap();
            std::fs::create_dir_all(root.join("sys")).unwrap();
            Self(root)
        }

        fn add(&self, name: &str, capabilities: &str, readable: bool) {
            let class = self.0.join("sys").join(name).join("device/capabilities");
            std::fs::create_dir_all(&class).unwrap();
            std::fs::write(class.join("key"), capabilities).unwrap();
            let node = self.0.join("dev").join(name);
            std::fs::write(&node, b"").unwrap();
            let mode = if readable { 0o600 } else { 0o000 };
            std::fs::set_permissions(&node, std::fs::Permissions::from_mode(mode)).unwrap();
        }

        fn remove(&self, name: &str) {
            std::fs::remove_dir_all(self.0.join("sys").join(name)).unwrap();
        }

        fn open(&self) -> Result<Keyboards, Access> {
            Keyboards::open_in(&self.0.join("dev"), &self.0.join("sys"))
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A full keyboard's capability words (Shift is bit 42 of the last word).
    const KEYBOARD: &str = "1000000000007 ff9f207ac14057ff febeffdfffefffff fffffffffffffffe";
    const MOUSE: &str = "1f0000 0 0 0 0";

    fn running_as_root() -> bool {
        // SAFETY: geteuid has no preconditions.
        unsafe { libc::geteuid() == 0 }
    }

    #[test]
    fn ioctl_numbers_match_linux_headers() {
        // EVIOCGKEY(96) and EVIOCGBIT(EV_KEY, 96) from linux/input.h.
        assert_eq!(eviocg(0x18, KEY_BYTES), 0x8060_4518);
        assert_eq!(eviocg(0x21, KEY_BYTES), 0x8060_4521);
    }

    #[test]
    fn only_modifier_bits_are_interpreted_and_samples_are_wiped() {
        let mut sample = bits(&[KEY_RIGHTSHIFT, 30, 31, 200]);
        let held = Modifiers::from_bits(&sample);
        assert_eq!(
            held,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            }
        );
        wipe(&mut sample);
        assert!(sample.iter().all(|byte| *byte == 0));
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
    fn sysfs_capabilities_tell_keyboards_from_mice() {
        assert!(reports_shift(KEYBOARD));
        assert!(!reports_shift(MOUSE));
        assert!(!reports_shift("0"));
        assert!(!reports_shift(""));
    }

    #[test]
    fn hot_plugged_keyboards_are_opened_and_removed_ones_forgotten() {
        let fake = Fake::new("hotplug");
        fake.add("event1", KEYBOARD, true);
        fake.add("event2", MOUSE, true);
        let mut keyboards = fake.open().unwrap();
        assert_eq!(keyboards.open.len(), 1, "the mouse is not opened");
        fake.add("event7", KEYBOARD, true);
        keyboards.refresh();
        assert_eq!(keyboards.open.len(), 2);
        fake.remove("event1");
        keyboards.refresh();
        let names: Vec<&str> = keyboards
            .open
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(names, ["event7"]);
    }

    #[test]
    fn handles_that_stop_answering_are_dropped() {
        let fake = Fake::new("dead");
        fake.add("event3", KEYBOARD, true);
        let mut keyboards = fake.open().unwrap();
        // A plain file rejects EVIOCGKEY like an unplugged device.
        assert_eq!(keyboards.modifiers(), Modifiers::default());
        assert!(keyboards.is_empty());
    }

    #[test]
    fn unreadable_keyboards_are_reported_not_ignored() {
        if running_as_root() {
            return;
        }
        let fake = Fake::new("partial");
        fake.add("event4", KEYBOARD, false);
        assert_eq!(fake.open().err(), Some(Access::Denied));
        fake.add("event5", KEYBOARD, true);
        let keyboards = fake.open().unwrap();
        assert_eq!(keyboards.unreadable(), 1);
    }

    #[test]
    fn no_keyboards_is_distinct_from_denied() {
        let fake = Fake::new("none");
        fake.add("event6", MOUSE, true);
        assert_eq!(fake.open().err(), Some(Access::NoKeyboards));
    }
}
