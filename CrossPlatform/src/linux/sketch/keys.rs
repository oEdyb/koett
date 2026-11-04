//! Draw-key state straight from keyboard devices, so the sketch overlay never
//! needs keyboard focus and typing keeps working while recording.
//!
//! Privacy boundary: Koett never reads key events. While recording is active it samples each keyboard's held-key state (`EVIOCGKEY`).
//! The kernel only reports all keys at once, so a sample briefly holds which
//! keys are down; Koett uses only the Escape/Shift/Ctrl/Alt/Super bits and whether any
//! other key is down, stores nothing, and wipes the sample with volatile writes
//! right after each poll. Devices are opened only for that recording.
//!
//! Only physical keyboards count. `EVIOCGKEY` reports a device's state even
//! when another process grabbed it, so a virtual device (uinput: ydotool, test
//! tools) could hold Shift invisibly to the desktop and start drawing, and a
//! gaming mouse's macro interface reports a whole keyboard.
use crate::settings::DrawKey;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const KEY_ESC: usize = 1;
const KEY_LEFTCTRL: usize = 29;
const KEY_LEFTSHIFT: usize = 42;
const KEY_RIGHTSHIFT: usize = 54;
const KEY_LEFTALT: usize = 56;
const KEY_RIGHTCTRL: usize = 97;
const KEY_RIGHTALT: usize = 100;
const KEY_LEFTMETA: usize = 125;
const KEY_RIGHTMETA: usize = 126;
const BTN_LEFT: usize = 0x110;
const REL_X: usize = 0;
const REL_Y: usize = 1;
const BUS_VIRTUAL: u16 = 0x06;
/// Where uinput devices live. Bluetooth keyboards sit under `misc/uhid` and stay.
const UINPUT_PARENT: &str = "/devices/virtual/input/";
/// A private lab names the one virtual keyboard it may use; never set on a desktop.
const TEST_KEYBOARD_ENV: &str = "KOETT_TEST_DRAW_KEYBOARD";
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

/// Escape and modifiers currently held on any keyboard.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub escape: bool,
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
    /// Any non-modifier key is down, as a yes/no; which key is never kept.
    pub other: bool,
}

impl Modifiers {
    fn from_bits(bits: &[u8]) -> Self {
        Self {
            escape: bit(bits, KEY_ESC),
            shift: bit(bits, KEY_LEFTSHIFT) || bit(bits, KEY_RIGHTSHIFT),
            control: bit(bits, KEY_LEFTCTRL) || bit(bits, KEY_RIGHTCTRL),
            alt: bit(bits, KEY_LEFTALT) || bit(bits, KEY_RIGHTALT),
            meta: bit(bits, KEY_LEFTMETA) || bit(bits, KEY_RIGHTMETA),
            other: bits.iter().enumerate().any(|(index, byte)| {
                let modifier_bits = [
                    KEY_LEFTCTRL,
                    KEY_LEFTSHIFT,
                    KEY_RIGHTSHIFT,
                    KEY_LEFTALT,
                    KEY_RIGHTCTRL,
                    KEY_RIGHTALT,
                    KEY_LEFTMETA,
                    KEY_RIGHTMETA,
                ]
                .iter()
                .filter(|key| *key / 8 == index)
                .fold(0_u8, |mask, key| mask | (1 << (key % 8)));
                byte & !modifier_bits != 0
            }),
        }
    }

    fn or(self, other: Self) -> Self {
        Self {
            escape: self.escape || other.escape,
            shift: self.shift || other.shift,
            control: self.control || other.control,
            alt: self.alt || other.alt,
            meta: self.meta || other.meta,
            other: self.other || other.other,
        }
    }

    /// Whether the draw key is down, and whether anything else is down with it.
    pub fn for_key(self, key: DrawKey) -> (bool, bool) {
        let (held, modifiers) = match key {
            DrawKey::Shift => (self.shift, self.control || self.alt || self.meta),
            DrawKey::Control => (self.control, self.shift || self.alt || self.meta),
            DrawKey::Alt => (self.alt, self.shift || self.control || self.meta),
        };
        (held, modifiers || self.other)
    }
}

/// A draw-key hold under this does nothing, so typing a capital letter neither
/// blocks clicks nor flashes the crosshair.
const DRAW_ARM: Duration = Duration::from_millis(120);
/// A hold with no pointer movement for this long gives the pointer back and
/// needs a real release first, so a stuck key cannot keep the screen covered.
const HOLD_IDLE: Duration = Duration::from_secs(15);

/// Decides from sampled key state whether a hold is a deliberate drawing hold.
/// It must start from a key seen up in this recording, stay free of any other
/// key, and last past [`DRAW_ARM`]. Releasing Ctrl from Ctrl+Shift+Space (the
/// default dictation shortcut) leaves Shift down, which must not start drawing.
#[derive(Debug, Default)]
pub struct DrawGate {
    seen_up: bool,
    spoiled: bool,
    down_since: Option<Instant>,
    active_at: Option<Instant>,
}

impl DrawGate {
    pub fn update(&mut self, held: bool, others: bool, now: Instant) -> bool {
        if !held {
            *self = Self {
                seen_up: true,
                ..Self::default()
            };
            return false;
        }
        if others || !self.seen_up || self.expired(now) {
            self.spoiled = true;
        }
        if self.spoiled {
            self.down_since = None;
            return false;
        }
        let since = *self.down_since.get_or_insert(now);
        let drawing = now.saturating_duration_since(since) >= DRAW_ARM;
        if drawing && self.active_at.is_none() {
            self.active_at = Some(now);
        }
        drawing
    }

    /// The pointer moved during the hold, so it is in use.
    pub fn moved(&mut self, now: Instant) {
        self.active_at = Some(now);
    }

    /// No pointer movement for [`HOLD_IDLE`] since the hold started drawing.
    pub fn expired(&self, now: Instant) -> bool {
        self.active_at
            .is_some_and(|at| now.saturating_duration_since(at) >= HOLD_IDLE)
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

/// Whether a sysfs capability bitmap (hex words, most significant first) has a
/// bit set.
fn capability(words: &str, index: usize) -> bool {
    let word_bits = usize::BITS as usize;
    words
        .split_whitespace()
        .rev()
        .nth(index / word_bits)
        .and_then(|word| usize::from_str_radix(word, 16).ok())
        .is_some_and(|word| word & (1 << (index % word_bits)) != 0)
}

fn reports_shift(capabilities: &str) -> bool {
    capability(capabilities, KEY_LEFTSHIFT)
}

fn read_capability(input: &Path, name: &str) -> String {
    std::fs::read_to_string(input.join("capabilities").join(name)).unwrap_or_default()
}

/// An input device with relative X/Y and a left button.
fn is_pointer(input: &Path) -> bool {
    let rel = read_capability(input, "rel");
    capability(&rel, REL_X)
        && capability(&rel, REL_Y)
        && capability(&read_capability(input, "key"), BTN_LEFT)
}

/// Every input device below a directory; sysfs links are not followed.
fn input_devices(directory: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let path = entry.path();
        if path.join("capabilities").is_dir() {
            found.push(path);
        } else {
            input_devices(&path, depth - 1, found);
        }
    }
}

/// The USB interface directory (`3-2:1.0`) above an input device.
fn usb_interface(device: &Path) -> Option<(&Path, &str)> {
    device.ancestors().find_map(|path| {
        let name = path.file_name()?.to_str()?;
        let (_, numbers) = name.split_once(':')?;
        let (config, interface) = numbers.split_once('.')?;
        let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
        (digits(config) && digits(interface)).then_some((path, name))
    })
}

/// A USB device whose first interface is a pointer with no keyboard is a mouse:
/// its other interfaces can report a whole keyboard for macro buttons. A
/// keyboard with a mouse interface of its own has a keyboard there too.
fn belongs_to_mouse(device: &Path) -> bool {
    let Some((interface, name)) = usb_interface(device) else {
        return false;
    };
    let (Some(usb), Some((prefix, _))) = (interface.parent(), name.rsplit_once('.')) else {
        return false;
    };
    let mut first = Vec::new();
    input_devices(&usb.join(format!("{prefix}.0")), 5, &mut first);
    first.iter().any(|input| is_pointer(input))
        && !first
            .iter()
            .any(|input| reports_shift(&read_capability(input, "key")))
}

/// Whether a class entry (`/sys/class/input/eventN`) is a physical keyboard.
/// `only` names the single test device a private lab allows instead.
fn is_physical_keyboard(class: &Path, only: Option<&str>) -> bool {
    let link = class.join("device");
    if !reports_shift(&read_capability(&link, "key")) {
        return false;
    }
    if let Some(only) = only {
        return std::fs::read_to_string(link.join("name")).is_ok_and(|name| name.trim() == only);
    }
    let bus = std::fs::read_to_string(link.join("id/bustype")).unwrap_or_default();
    let device = std::fs::canonicalize(&link).unwrap_or(link);
    u16::from_str_radix(bus.trim(), 16).ok() != Some(BUS_VIRTUAL)
        && !device.to_string_lossy().contains(UINPUT_PARENT)
        && !is_pointer(&device)
        && !belongs_to_mouse(&device)
}

/// Open keyboards for one recording, refreshed for hot-plug.
pub struct Keyboards {
    devices: PathBuf,
    classes: PathBuf,
    open: Vec<(String, File)>,
    unreadable: Vec<String>,
    only: Option<String>,
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
            only: std::env::var(TEST_KEYBOARD_ENV).ok(),
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
                if name.starts_with("event")
                    && is_physical_keyboard(&entry.path(), self.only.as_deref())
                {
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
    fn only_modifiers_and_a_yes_no_for_other_keys_are_interpreted() {
        let mut sample = bits(&[KEY_RIGHTSHIFT, 30, 31, 200]);
        let held = Modifiers::from_bits(&sample);
        assert_eq!(
            held,
            Modifiers {
                shift: true,
                other: true,
                ..Modifiers::default()
            }
        );
        assert!(!Modifiers::from_bits(&bits(&[KEY_RIGHTSHIFT])).other);
        wipe(&mut sample);
        assert!(sample.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn only_the_draw_key_alone_counts_as_a_clean_hold() {
        let shift = Modifiers::from_bits(&bits(&[KEY_LEFTSHIFT]));
        assert_eq!(shift.for_key(DrawKey::Shift), (true, false));
        assert!(!shift.for_key(DrawKey::Control).0);
        let chord = Modifiers::from_bits(&bits(&[KEY_LEFTSHIFT, KEY_LEFTCTRL]));
        assert_eq!(chord.for_key(DrawKey::Shift), (true, true));
        let typing = Modifiers::from_bits(&bits(&[KEY_LEFTSHIFT, 30]));
        assert_eq!(
            typing.for_key(DrawKey::Shift),
            (true, true),
            "Shift+A is typing"
        );
        let alt = Modifiers::from_bits(&bits(&[KEY_RIGHTALT]));
        assert_eq!(alt.for_key(DrawKey::Alt), (true, false));
    }

    /// Replays (held, others, milliseconds) samples; returns what the last did.
    fn replay(gate: &mut DrawGate, start: Instant, samples: &[(bool, bool, u64)]) -> bool {
        samples.iter().fold(false, |_, (held, others, at)| {
            gate.update(*held, *others, start + Duration::from_millis(*at))
        })
    }

    #[test]
    fn a_clean_hold_draws_and_a_capital_letter_tap_does_not() {
        let start = Instant::now();
        let mut gate = DrawGate::default();
        assert!(!replay(
            &mut gate,
            start,
            &[(false, false, 0), (true, false, 10), (true, false, 60)]
        ));
        assert!(
            !replay(&mut gate, start, &[(false, false, 70)]),
            "released within the arm time"
        );
        assert!(replay(
            &mut gate,
            start,
            &[(true, false, 100), (true, false, 100 + 120)]
        ));
    }

    #[test]
    fn releasing_ctrl_from_the_dictation_shortcut_never_starts_drawing() {
        // Ctrl+Shift+Space starts the recording; Ctrl comes up first, Shift stays.
        let start = Instant::now();
        let mut gate = DrawGate::default();
        let held_on = [
            (true, true, 0),
            (true, false, 50),
            (true, false, 500),
            (true, false, 5000),
        ];
        assert!(!replay(&mut gate, start, &held_on));
        assert!(!replay(
            &mut gate,
            start,
            &[(false, false, 5100), (true, false, 5200)]
        ));
        assert!(
            replay(&mut gate, start, &[(true, false, 5400)]),
            "a fresh hold draws"
        );
    }

    #[test]
    fn a_key_already_held_when_recording_starts_never_draws() {
        let start = Instant::now();
        let mut gate = DrawGate::default();
        assert!(!replay(
            &mut gate,
            start,
            &[(true, false, 0), (true, false, 3000)]
        ));
    }

    #[test]
    fn typing_during_a_hold_gives_the_pointer_back_until_release() {
        let start = Instant::now();
        let mut gate = DrawGate::default();
        assert!(replay(
            &mut gate,
            start,
            &[(false, false, 0), (true, false, 10), (true, false, 200)]
        ));
        assert!(!replay(
            &mut gate,
            start,
            &[(true, true, 210), (true, false, 300), (true, false, 900)]
        ));
    }

    #[test]
    fn a_stuck_key_gives_the_pointer_back_and_needs_a_real_release() {
        let start = Instant::now();
        let mut gate = DrawGate::default();
        assert!(replay(
            &mut gate,
            start,
            &[(false, false, 0), (true, false, 10), (true, false, 200)]
        ));
        let later = 200 + HOLD_IDLE.as_millis() as u64 - 1;
        gate.moved(start + Duration::from_millis(200));
        assert!(
            replay(&mut gate, start, &[(true, false, later)]),
            "still within the idle time"
        );
        let after = 200 + HOLD_IDLE.as_millis() as u64;
        assert!(!replay(
            &mut gate,
            start,
            &[(true, false, after), (true, false, after + 60_000)]
        ));
        assert!(!replay(
            &mut gate,
            start,
            &[
                (false, false, after + 60_001),
                (true, false, after + 60_002)
            ]
        ));
        assert!(replay(&mut gate, start, &[(true, false, after + 60_200)]));
    }

    /// One input device in a fake sysfs tree, linked from `sys/eventN/device`.
    fn device(
        root: &Path,
        event: &str,
        tree: &str,
        name: &str,
        bus: &str,
        key: &str,
        rel: &str,
    ) -> PathBuf {
        let path = root.join("tree").join(tree);
        std::fs::create_dir_all(path.join("capabilities")).unwrap();
        std::fs::create_dir_all(path.join("id")).unwrap();
        std::fs::write(path.join("capabilities/key"), key).unwrap();
        std::fs::write(path.join("capabilities/rel"), rel).unwrap();
        std::fs::write(path.join("name"), format!("{name}\n")).unwrap();
        std::fs::write(path.join("id/bustype"), format!("{bus}\n")).unwrap();
        let class = root.join("sys").join(event);
        std::fs::create_dir_all(&class).unwrap();
        std::os::unix::fs::symlink(&path, class.join("device")).unwrap();
        class
    }

    // Capability words copied from Olle's PC (2026-11-04).
    const FULL_KEYS: &str = "3f00733fff 0 0 483ffff17aff32d bfd4444600000000 1 130ff38b17d000 677bfad9415fed f1beffdff3cfffff fffffffffffffffe";
    const MOUSE_KEYS: &str = "1f0000 0 0 0 0";
    const POINTER_REL: &str = "903";
    const WHEEL_REL: &str = "1040";

    #[test]
    fn a_virtual_keyboard_is_never_read_as_a_draw_key() {
        let fake = Fake::new("virtual");
        let root = &fake.0;
        // ydotoold and the grabbed lab tool: uinput devices that report Shift.
        let ydotool = device(
            root,
            "event29",
            "devices/virtual/input/input29",
            "ydotoold virtual device",
            "0006",
            FULL_KEYS,
            "147",
        );
        let lab = device(
            root,
            "event30",
            "devices/virtual/input/input43",
            "Koett lab test shift (grabbed)",
            "0006",
            KEYBOARD,
            "0",
        );
        let usb_bus = device(
            root,
            "event31",
            "devices/virtual/input/input44",
            "uinput pretending USB",
            "0003",
            KEYBOARD,
            "0",
        );
        for class in [&ydotool, &lab, &usb_bus] {
            assert!(!is_physical_keyboard(class, None), "{class:?}");
        }
    }

    #[test]
    fn a_gaming_mouse_with_a_keyboard_interface_is_not_a_keyboard() {
        let fake = Fake::new("razer");
        let root = &fake.0;
        let usb = "devices/pci/usb6/6-4/6-4.4";
        device(
            root,
            "event9",
            &format!("{usb}/6-4.4:1.0/0003:1532:00C1.0001/input/input20"),
            "Razer Razer Viper V3 Pro",
            "0003",
            MOUSE_KEYS,
            POINTER_REL,
        );
        let keyboard = device(
            root,
            "event10",
            &format!("{usb}/6-4.4:1.1/0003:1532:00C1.0002/input/input21"),
            "Razer Razer Viper V3 Pro Keyboard",
            "0003",
            FULL_KEYS,
            WHEEL_REL,
        );
        let plain = device(
            root,
            "event13",
            &format!("{usb}/6-4.4:1.2/0003:1532:00C1.0003/input/input24"),
            "Razer Razer Viper V3 Pro",
            "0003",
            FULL_KEYS,
            "0",
        );
        assert!(!is_physical_keyboard(&keyboard, None));
        assert!(!is_physical_keyboard(&plain, None));
    }

    #[test]
    fn a_real_keyboard_with_its_own_mouse_interface_still_counts() {
        let fake = Fake::new("k100");
        let root = &fake.0;
        let usb = "devices/pci/usb3/3-2";
        let keyboard = device(
            root,
            "event2",
            &format!("{usb}/3-2:1.0/0003:1B1C:1B7D.0005/input/input5"),
            "Corsair CORSAIR K100 RGB Mechanical Gaming Keyboard",
            "0003",
            FULL_KEYS,
            WHEEL_REL,
        );
        device(
            root,
            "event3",
            &format!("{usb}/3-2:1.0/0003:1B1C:1B7D.0005/input/input6"),
            "Corsair CORSAIR K100 RGB Mechanical Gaming Keyboard Mouse",
            "0003",
            MOUSE_KEYS,
            POINTER_REL,
        );
        assert!(is_physical_keyboard(&keyboard, None));
    }

    #[test]
    fn built_in_and_bluetooth_keyboards_still_count() {
        let fake = Fake::new("laptop");
        let root = &fake.0;
        let laptop = device(
            root,
            "event0",
            "devices/platform/i8042/serio0/input/input0",
            "AT Translated Set 2 keyboard",
            "0011",
            KEYBOARD,
            "0",
        );
        assert!(is_physical_keyboard(&laptop, None));
        let bluetooth = device(
            root,
            "event1",
            "devices/virtual/misc/uhid/0005:05AC:0220.0007/input/input9",
            "Magic Keyboard",
            "0005",
            KEYBOARD,
            "0",
        );
        assert!(is_physical_keyboard(&bluetooth, None));
    }

    #[test]
    fn a_private_lab_can_name_the_one_virtual_keyboard_it_uses() {
        let fake = Fake::new("only");
        let root = &fake.0;
        let lab = device(
            root,
            "event30",
            "devices/virtual/input/input43",
            "Koett lab shift",
            "0006",
            KEYBOARD,
            "0",
        );
        let real = device(
            root,
            "event2",
            "devices/pci/usb3/3-2/3-2:1.0/x/input/input5",
            "Real keyboard",
            "0003",
            KEYBOARD,
            "0",
        );
        assert!(is_physical_keyboard(&lab, Some("Koett lab shift")));
        assert!(
            !is_physical_keyboard(&real, Some("Koett lab shift")),
            "the host keyboard stays out"
        );
        assert!(!is_physical_keyboard(&lab, Some("another name")));
    }

    #[test]
    #[ignore = "reads this machine's real input devices"]
    fn print_the_physical_keyboards_on_this_machine() {
        let classes = Path::new("/sys/class/input");
        let mut names: Vec<String> = std::fs::read_dir(classes)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("event"))
            .filter_map(|entry| {
                let path = entry.path();
                reports_shift(&read_capability(&path.join("device"), "key")).then(|| {
                    let name = std::fs::read_to_string(path.join("device/name")).unwrap();
                    let kept = is_physical_keyboard(&path, None);
                    format!(
                        "{} {} -> {}",
                        entry.file_name().to_string_lossy(),
                        name.trim(),
                        kept
                    )
                })
            })
            .collect();
        names.sort();
        println!("{}", names.join("\n"));
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
