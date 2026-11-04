//! Paste chords on the compositor's actual keymap. A private replacement map
//! (as used by wtype) can turn V into Escape when niri restores its native map.
//! This client reads keymap metadata, never ordinary keyboard events.
use crate::settings::PasteShortcut;
use std::{
    fs::File,
    io::{Read, Seek},
    os::fd::{AsFd, AsRawFd},
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
    protocol::{wl_callback, wl_keyboard, wl_registry, wl_seat},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1 as manager, zwp_virtual_keyboard_v1 as keyboard,
};

struct State {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<manager::ZwpVirtualKeyboardManagerV1>,
    keymap: Option<(File, u32)>,
    done: u64,
    serial: u64,
}
impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_seat" && state.seat.is_none() {
                state.seat = Some(registry.bind(name, version.min(4), qh, ()));
            }
            if interface == "zwp_virtual_keyboard_manager_v1" {
                state.manager = Some(registry.bind(name, version.min(1), qh, ()));
            }
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Keymap {
            format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
            fd,
            size,
        } = event
            && size > 0
            && size <= 4 * 1024 * 1024
        {
            state.keymap = Some((File::from(fd), size));
        }
    }
}
impl Dispatch<wl_callback::WlCallback, u64> for State {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        serial: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.done = state.done.max(*serial);
        }
    }
}
impl Dispatch<manager::ZwpVirtualKeyboardManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &manager::ZwpVirtualKeyboardManagerV1,
        _: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<keyboard::ZwpVirtualKeyboardV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &keyboard::ZwpVirtualKeyboardV1,
        _: keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
fn sync(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    deadline: Instant,
) -> Result<(), String> {
    state.serial += 1;
    let serial = state.serial;
    connection.display().sync(&queue.handle(), serial);
    while state.done < serial {
        if Instant::now() >= deadline {
            return Err("Virtual keyboard synchronization timed out".into());
        }
        queue.dispatch_pending(state).map_err(|e| e.to_string())?;
        let blocked = match queue.flush() {
            Ok(()) => false,
            Err(wayland_client::backend::WaylandError::Io(e))
                if e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                true
            }
            Err(e) => return Err(e.to_string()),
        };
        if state.done >= serial {
            break;
        }
        if let Some(read) = queue.prepare_read() {
            let mut poll = libc::pollfd {
                fd: read.connection_fd().as_raw_fd(),
                events: libc::POLLIN | if blocked { libc::POLLOUT } else { 0 },
                revents: 0,
            };
            // SAFETY: one initialized pollfd, kept alive by the read guard.
            if unsafe { libc::poll(&mut poll, 1, 10) } > 0
                && poll.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
            {
                read.read().map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct Chord {
    key: u32,
    mask: u32,
    group: u32,
}
/// Resolve V in the real map, rather than assigning it an arbitrary keycode.
fn chord(map: &[u8], group: u32, shortcut: PasteShortcut) -> Result<Chord, String> {
    use std::ffi::{CString, c_char, c_void};
    let bytes = map.strip_suffix(&[0]).unwrap_or(map);
    let string = CString::new(bytes).map_err(|_| "Invalid compositor keymap")?;
    // SAFETY: pinned xkbcommon C signatures; all pointers are checked and unref'd
    // before unloading the library. No symbol or pointer escapes this function.
    unsafe {
        let lib = libloading::Library::new("libxkbcommon.so.0").map_err(|e| e.to_string())?;
        let context_new = lib
            .get::<unsafe extern "C" fn(u32) -> *mut c_void>(b"xkb_context_new\0")
            .map_err(|e| e.to_string())?;
        let context_unref = lib
            .get::<unsafe extern "C" fn(*mut c_void)>(b"xkb_context_unref\0")
            .map_err(|e| e.to_string())?;
        let map_new = lib
            .get::<unsafe extern "C" fn(*mut c_void, *const c_char, u32, u32) -> *mut c_void>(
                b"xkb_keymap_new_from_string\0",
            )
            .map_err(|e| e.to_string())?;
        let map_unref = lib
            .get::<unsafe extern "C" fn(*mut c_void)>(b"xkb_keymap_unref\0")
            .map_err(|e| e.to_string())?;
        let minimum = lib
            .get::<unsafe extern "C" fn(*mut c_void) -> u32>(b"xkb_keymap_min_keycode\0")
            .map_err(|e| e.to_string())?;
        let maximum = lib
            .get::<unsafe extern "C" fn(*mut c_void) -> u32>(b"xkb_keymap_max_keycode\0")
            .map_err(|e| e.to_string())?;
        let layouts = lib
            .get::<unsafe extern "C" fn(*mut c_void) -> u32>(b"xkb_keymap_num_layouts\0")
            .map_err(|e| e.to_string())?;
        let symbols = lib
            .get::<unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut *const u32) -> i32>(
                b"xkb_keymap_key_get_syms_by_level\0",
            )
            .map_err(|e| e.to_string())?;
        let modifier = lib
            .get::<unsafe extern "C" fn(*mut c_void, *const c_char) -> u32>(
                b"xkb_keymap_mod_get_index\0",
            )
            .map_err(|e| e.to_string())?;
        let context = context_new(0);
        if context.is_null() {
            return Err("Could not create keyboard context".into());
        }
        let map = map_new(context, string.as_ptr(), 1, 0);
        if map.is_null() {
            context_unref(context);
            return Err("Could not parse compositor keymap".into());
        }
        let result = (|| {
            if group >= layouts(map) {
                return Err("Keyboard layout changed".into());
            }
            let control = modifier(map, c"Control".as_ptr());
            let shift = modifier(map, c"Shift".as_ptr());
            if control >= 32 || shift >= 32 {
                return Err("Keyboard modifiers unavailable".into());
            }
            let start = minimum(map).max(8);
            let end = maximum(map);
            if end > 65535 {
                return Err("Keyboard keycode range is invalid".into());
            }
            for key in start..=end {
                let mut syms = std::ptr::null();
                let count = symbols(map, key, group, 0, &mut syms);
                if count > 0
                    && !syms.is_null()
                    && std::slice::from_raw_parts(syms, count as usize).contains(&0x76)
                {
                    return Ok(Chord {
                        key: key - 8,
                        mask: (1 << control)
                            | if shortcut == PasteShortcut::CtrlShiftV {
                                1 << shift
                            } else {
                                0
                            },
                        group,
                    });
                }
            }
            Err("The active keyboard layout has no V shortcut key".into())
        })();
        map_unref(map);
        context_unref(context);
        result
    }
}
fn timestamp() -> Result<u32, String> {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: value is initialized and writable for one timespec.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut value) } != 0 {
        return Err("Keyboard clock unavailable".into());
    }
    Ok(((value.tv_sec as u64) * 1000 + (value.tv_nsec as u64) / 1_000_000) as u32)
}
pub(super) fn paste(
    shortcut: PasteShortcut,
    before_post: impl FnOnce(u32, u32) -> Result<(), String>,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_millis(700);
    let group = super::output::keyboard_layout_group().ok_or("Keyboard layout unavailable")?;
    let connection = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut state = State {
        seat: None,
        manager: None,
        keymap: None,
        done: 0,
        serial: 0,
    };
    let _registry = connection.display().get_registry(&qh, ());
    sync(&connection, &mut queue, &mut state, deadline)?;
    let seat = state.seat.as_ref().ok_or("Keyboard seat unavailable")?;
    let reader = seat.get_keyboard(&qh, ());
    sync(&connection, &mut queue, &mut state, deadline)?;
    let (mut map, size) = state
        .keymap
        .take()
        .ok_or("Compositor keyboard map unavailable")?;
    map.rewind().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    map.by_ref()
        .take(u64::from(size))
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() != size as usize {
        return Err("Incomplete compositor keyboard map".into());
    }
    map.rewind().map_err(|e| e.to_string())?;
    let plan = chord(&bytes, group, shortcut)?;
    let keyboard = state
        .manager
        .as_ref()
        .ok_or("Virtual keyboard protocol unavailable")?
        .create_virtual_keyboard(
            state.seat.as_ref().ok_or("Keyboard seat unavailable")?,
            &qh,
            (),
        );
    keyboard.keymap(1, map.as_fd(), size);
    sync(&connection, &mut queue, &mut state, deadline)?;
    if super::output::keyboard_layout_group() != Some(group) {
        return Err("Keyboard layout changed during delivery".into());
    }
    before_post(plan.key, plan.mask)?;
    keyboard.modifiers(plan.mask, 0, 0, plan.group);
    keyboard.key(timestamp()?, plan.key, 1);
    keyboard.key(timestamp()?, plan.key, 0);
    keyboard.modifiers(0, 0, 0, plan.group);
    sync(
        &connection,
        &mut queue,
        &mut state,
        Instant::now() + Duration::from_millis(300),
    )?;
    keyboard.destroy();
    reader.release();
    connection.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn keymap(layout: &str, variant: &str) -> Vec<u8> {
        use std::ffi::{CStr, CString, c_char, c_void};
        #[repr(C)]
        struct Names {
            rules: *const c_char,
            model: *const c_char,
            layout: *const c_char,
            variant: *const c_char,
            options: *const c_char,
        }
        let layout = CString::new(layout).unwrap();
        let variant = CString::new(variant).unwrap();
        // SAFETY: xkbcommon's public C ABI; all names outlive the call, all allocations freed before unloading.
        unsafe {
            let lib = libloading::Library::new("libxkbcommon.so.0").unwrap();
            let context_new = lib
                .get::<unsafe extern "C" fn(u32) -> *mut c_void>(b"xkb_context_new\0")
                .unwrap();
            let context_unref = lib
                .get::<unsafe extern "C" fn(*mut c_void)>(b"xkb_context_unref\0")
                .unwrap();
            let map_new = lib
                .get::<unsafe extern "C" fn(*mut c_void, *const Names, u32) -> *mut c_void>(
                    b"xkb_keymap_new_from_names\0",
                )
                .unwrap();
            let map_unref = lib
                .get::<unsafe extern "C" fn(*mut c_void)>(b"xkb_keymap_unref\0")
                .unwrap();
            let get = lib
                .get::<unsafe extern "C" fn(*mut c_void, u32) -> *mut c_char>(
                    b"xkb_keymap_get_as_string\0",
                )
                .unwrap();
            let context = context_new(0);
            assert!(!context.is_null());
            let names = Names {
                rules: c"evdev".as_ptr(),
                model: c"pc105".as_ptr(),
                layout: layout.as_ptr(),
                variant: variant.as_ptr(),
                options: c"".as_ptr(),
            };
            let map = map_new(context, &names, 0);
            assert!(!map.is_null());
            let text = get(map, 1);
            assert!(!text.is_null());
            let bytes = CStr::from_ptr(text).to_bytes_with_nul().to_vec();
            libc::free(text.cast());
            map_unref(map);
            context_unref(context);
            bytes
        }
    }
    #[test]
    fn native_v_uses_real_us_swedish_and_dvorak_keycodes_not_escapes_slot() {
        for (layout, variant, key) in [("us", "", 47), ("se", "", 47), ("us", "dvorak", 52)] {
            let map = keymap(layout, variant);
            let plain = chord(&map, 0, PasteShortcut::CtrlV).unwrap();
            assert_eq!(plain.key, key);
            assert_ne!(plain.key, 1);
            assert_eq!(plain.mask, 4);
            let shifted = chord(&map, 0, PasteShortcut::CtrlShiftV).unwrap();
            assert_eq!(shifted.key, key);
            assert_eq!(shifted.mask, 5);
            assert!(chord(&map, 99, PasteShortcut::CtrlV).is_err());
        }
    }
    #[test]
    fn malformed_keymap_is_never_used_for_a_paste() {
        assert!(chord(b"invalid\0data", 0, PasteShortcut::CtrlV).is_err());
    }
    #[test]
    fn synchronization_is_bounded_without_a_compositor_reply() {
        let (client, _server) = std::os::unix::net::UnixStream::pair().unwrap();
        let connection = Connection::from_socket(client).unwrap();
        let mut queue = connection.new_event_queue();
        let mut state = State {
            seat: None,
            manager: None,
            keymap: None,
            done: 0,
            serial: 0,
        };
        let start = Instant::now();
        assert!(
            sync(
                &connection,
                &mut queue,
                &mut state,
                start + Duration::from_millis(40)
            )
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_millis(400));
    }
}
