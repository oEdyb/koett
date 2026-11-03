//! A left click at the current pointer position, sent through
//! `zwlr_virtual_pointer_v1`. The Wayland wire format is small enough to speak
//! directly, so this adds no dependency and no generated protocol code.
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const IO_TIMEOUT: Duration = Duration::from_millis(100);
const ROUNDTRIP_TIMEOUT: Duration = Duration::from_millis(250);
const MANAGER_INTERFACE: &str = "zwlr_virtual_pointer_manager_v1";
const BTN_LEFT: u32 = 0x110;

// A client must allocate new object ids consecutively, in the order it sends
// them, or the compositor treats the connection as a protocol error.
const DISPLAY: u32 = 1;
const REGISTRY: u32 = 2;
const REGISTRY_CALLBACK: u32 = 3;
const MANAGER: u32 = 4;
const POINTER: u32 = 5;
const FIRST_CALLBACK: u32 = 6;

const DISPLAY_SYNC: u16 = 0;
const DISPLAY_GET_REGISTRY: u16 = 1;
const DISPLAY_ERROR_EVENT: u16 = 0;
const REGISTRY_BIND: u16 = 0;
const REGISTRY_GLOBAL_EVENT: u16 = 0;
const CALLBACK_DONE_EVENT: u16 = 0;
const MANAGER_CREATE_POINTER: u16 = 0;
const POINTER_BUTTON: u16 = 2;
const POINTER_FRAME: u16 = 4;
const POINTER_DESTROY: u16 = 8;

fn message(object: u32, opcode: u16, body: &[u8]) -> Vec<u8> {
    let size = u32::try_from(body.len() + 8).unwrap_or(u32::MAX);
    let mut bytes = Vec::with_capacity(body.len() + 8);
    bytes.extend_from_slice(&object.to_ne_bytes());
    bytes.extend_from_slice(&((size << 16) | u32::from(opcode)).to_ne_bytes());
    bytes.extend_from_slice(body);
    bytes
}

fn push_u32(body: &mut Vec<u8>, value: u32) {
    body.extend_from_slice(&value.to_ne_bytes());
}

/// Length (with the terminating NUL), bytes, then padding to four bytes.
fn push_string(body: &mut Vec<u8>, value: &str) {
    push_u32(body, u32::try_from(value.len() + 1).unwrap_or(u32::MAX));
    body.extend_from_slice(value.as_bytes());
    body.push(0);
    body.resize(body.len().next_multiple_of(4), 0);
}

fn sync_request(callback: u32) -> Vec<u8> {
    message(DISPLAY, DISPLAY_SYNC, &callback.to_ne_bytes())
}

fn get_registry_request() -> Vec<u8> {
    message(DISPLAY, DISPLAY_GET_REGISTRY, &REGISTRY.to_ne_bytes())
}

fn bind_request(name: u32, version: u32) -> Vec<u8> {
    let mut body = Vec::new();
    push_u32(&mut body, name);
    push_string(&mut body, MANAGER_INTERFACE);
    push_u32(&mut body, version);
    push_u32(&mut body, MANAGER);
    message(REGISTRY, REGISTRY_BIND, &body)
}

fn create_pointer_request() -> Vec<u8> {
    let mut body = Vec::new();
    // A null seat lets the compositor pick its default seat.
    push_u32(&mut body, 0);
    push_u32(&mut body, POINTER);
    message(MANAGER, MANAGER_CREATE_POINTER, &body)
}

fn button_request(time: u32, pressed: bool) -> Vec<u8> {
    let mut body = Vec::new();
    push_u32(&mut body, time);
    push_u32(&mut body, BTN_LEFT);
    push_u32(&mut body, u32::from(pressed));
    message(POINTER, POINTER_BUTTON, &body)
}

fn click_requests(time: u32) -> Vec<u8> {
    let mut bytes = button_request(time, true);
    bytes.extend(message(POINTER, POINTER_FRAME, &[]));
    bytes.extend(button_request(time, false));
    bytes.extend(message(POINTER, POINTER_FRAME, &[]));
    bytes
}

#[derive(Debug, PartialEq)]
struct Event<'a> {
    object: u32,
    opcode: u16,
    body: &'a [u8],
}

/// The next complete event in `buffer`; `None` until all of it has arrived.
fn next_event(buffer: &[u8]) -> Result<Option<(Event<'_>, usize)>, String> {
    let Some(header) = buffer.get(..8) else {
        return Ok(None);
    };
    let object = u32::from_ne_bytes([header[0], header[1], header[2], header[3]]);
    let word = u32::from_ne_bytes([header[4], header[5], header[6], header[7]]);
    let size = (word >> 16) as usize;
    if size < 8 {
        return Err("Wayland event has an invalid size".into());
    }
    let Some(body) = buffer.get(8..size) else {
        return Ok(None);
    };
    let opcode = (word & 0xffff) as u16;
    Ok(Some((
        Event {
            object,
            opcode,
            body,
        },
        size,
    )))
}

/// Decodes `wl_registry.global`: name, interface, version.
fn parse_global(body: &[u8]) -> Option<(u32, &str, u32)> {
    let word = |at: usize| -> Option<u32> {
        let bytes = body.get(at..at + 4)?;
        Some(u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    let name = word(0)?;
    let length = word(4)? as usize;
    let interface = body.get(8..8 + length.checked_sub(1)?)?;
    let version = word(8 + length.next_multiple_of(4))?;
    Some((name, std::str::from_utf8(interface).ok()?, version))
}

fn socket_path(runtime: Option<PathBuf>, display: Option<String>) -> Result<PathBuf, String> {
    let display = display.unwrap_or_else(|| "wayland-0".into());
    let path = PathBuf::from(display);
    if path.is_absolute() {
        return Ok(path);
    }
    let runtime = runtime.ok_or("XDG_RUNTIME_DIR is not set")?;
    Ok(runtime.join(path))
}

pub struct VirtualPointer {
    stream: UnixStream,
    next_callback: u32,
    started: Instant,
}

impl VirtualPointer {
    /// Fails when the compositor does not offer virtual pointers.
    pub fn connect() -> Result<Self, String> {
        let path = socket_path(
            std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
            std::env::var("WAYLAND_DISPLAY").ok(),
        )?;
        let stream = UnixStream::connect(&path)
            .map_err(|error| format!("Wayland socket unavailable: {error}"))?;
        let timeouts = stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)));
        timeouts.map_err(|error| format!("Wayland socket timeout failed: {error}"))?;
        let mut pointer = Self {
            stream,
            next_callback: REGISTRY_CALLBACK,
            started: Instant::now(),
        };
        pointer.send(&get_registry_request())?;
        let globals = pointer.roundtrip()?;
        let (name, version) = globals
            .into_iter()
            .find(|(_, interface, _)| interface == MANAGER_INTERFACE)
            .map(|(name, _, version)| (name, version))
            .ok_or("Compositor has no virtual pointer support")?;
        pointer.next_callback = FIRST_CALLBACK;
        pointer.send(&bind_request(name, version.min(1)))?;
        pointer.send(&create_pointer_request())?;
        pointer.roundtrip()?;
        Ok(pointer)
    }

    /// Presses and releases the left button where the pointer already is, and
    /// returns once the compositor has processed both.
    pub fn click(&mut self) -> Result<(), String> {
        let time = u32::try_from(self.started.elapsed().as_millis()).unwrap_or(u32::MAX);
        self.send(&click_requests(time))?;
        self.roundtrip().map(|_| ())
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.stream
            .write_all(bytes)
            .map_err(|error| format!("Wayland write failed: {error}"))
    }

    /// Sends `wl_display.sync` and reads until it is answered. Returns the
    /// globals announced on the way.
    fn roundtrip(&mut self) -> Result<Vec<(u32, String, u32)>, String> {
        let callback = self.next_callback;
        self.next_callback += 1;
        self.send(&sync_request(callback))?;
        let deadline = Instant::now() + ROUNDTRIP_TIMEOUT;
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 4096];
        let mut globals = Vec::new();
        loop {
            let mut consumed = 0;
            while let Some((event, size)) = next_event(&buffer[consumed..])? {
                consumed += size;
                match (event.object, event.opcode) {
                    (DISPLAY, DISPLAY_ERROR_EVENT) => {
                        return Err("Compositor rejected the virtual pointer".into());
                    }
                    (REGISTRY, REGISTRY_GLOBAL_EVENT) => {
                        if let Some((name, interface, version)) = parse_global(event.body) {
                            globals.push((name, interface.to_owned(), version));
                        }
                    }
                    (object, CALLBACK_DONE_EVENT) if object == callback => return Ok(globals),
                    _ => {}
                }
            }
            buffer.drain(..consumed);
            if Instant::now() >= deadline {
                return Err("Compositor did not answer in time".into());
            }
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err("Compositor closed the connection".into()),
                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(format!("Wayland read failed: {error}")),
            }
        }
    }
}

impl Drop for VirtualPointer {
    fn drop(&mut self) {
        let _ = self.send(&message(POINTER, POINTER_DESTROY, &[]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|word| u32::from_ne_bytes([word[0], word[1], word[2], word[3]]))
            .collect()
    }

    #[test]
    fn sync_asks_the_display_for_a_callback() {
        assert_eq!(words(&sync_request(7)), [DISPLAY, 12 << 16, 7]);
    }

    #[test]
    fn bind_pads_the_interface_name_to_four_bytes() {
        let bytes = bind_request(7, 1);
        assert_eq!(bytes.len() % 4, 0);
        let (name, interface, version) = parse_global(&bytes[8..]).unwrap();
        assert_eq!((name, interface, version), (7, MANAGER_INTERFACE, 1));
        assert_eq!(words(&bytes).last(), Some(&MANAGER));
    }

    #[test]
    fn create_pointer_uses_the_default_seat() {
        assert_eq!(
            words(&create_pointer_request()),
            [MANAGER, 16 << 16, 0, POINTER]
        );
    }

    #[test]
    fn a_click_is_press_frame_release_frame_on_the_left_button() {
        assert_eq!(
            words(&click_requests(9)),
            [
                POINTER,
                (20 << 16) | 2,
                9,
                BTN_LEFT,
                1,
                POINTER,
                (8 << 16) | 4,
                POINTER,
                (20 << 16) | 2,
                9,
                BTN_LEFT,
                0,
                POINTER,
                (8 << 16) | 4,
            ]
        );
    }

    #[test]
    fn an_event_is_returned_only_when_complete() {
        let bytes = message(5, 0, &[1, 0, 0, 0]);
        assert_eq!(next_event(&bytes[..11]).unwrap(), None);
        let (event, size) = next_event(&bytes).unwrap().unwrap();
        assert_eq!((event.object, event.opcode, size), (5, 0, 12));
    }

    #[test]
    fn an_event_shorter_than_its_header_is_an_error() {
        let bytes = 4_u32
            .to_ne_bytes()
            .into_iter()
            .chain((4_u32 << 16).to_ne_bytes())
            .collect::<Vec<_>>();
        assert!(next_event(&bytes).is_err());
    }

    #[test]
    fn a_truncated_global_is_ignored() {
        assert_eq!(parse_global(&[1, 0, 0, 0, 200, 0, 0, 0]), None);
    }

    #[test]
    fn socket_path_joins_a_relative_display_and_keeps_an_absolute_one() {
        let runtime = Some(PathBuf::from("/run/user/1000"));
        assert_eq!(
            socket_path(runtime.clone(), Some("wayland-1".into())).unwrap(),
            PathBuf::from("/run/user/1000/wayland-1")
        );
        assert_eq!(
            socket_path(runtime, Some("/tmp/wl".into())).unwrap(),
            PathBuf::from("/tmp/wl")
        );
        assert!(socket_path(None, Some("wayland-1".into())).is_err());
    }
}
