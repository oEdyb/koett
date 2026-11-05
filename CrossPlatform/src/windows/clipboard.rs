use super::pixels::Pixels;
use base64::Engine;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::time::Instant;
static NEXT: AtomicU64 = AtomicU64::new(1);
thread_local! { static LAB: RefCell<Option<(Bundle, Offer)>> = const { RefCell::new(None) }; }
thread_local! { static DELAYED: RefCell<Option<Delayed>> = const { RefCell::new(None) }; }
struct Delayed {
    offer: Offer,
    formats: Vec<(u32, Vec<u8>)>,
    reader: Vec<(u32, Instant)>,
}
use windows::Win32::{
    Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND},
    System::{
        DataExchange::*,
        Memory::{GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        Ole::{CF_DIBV5, CF_UNICODETEXT},
    },
};
use windows::core::w;

const MAX_BYTES: usize = 32 * 1024 * 1024;

pub(super) struct Bundle {
    text: String,
    png: Vec<u8>,
    dib: Vec<u8>,
    html: Vec<u8>,
}

impl Bundle {
    pub fn image_first(&self, owner: HWND) -> Result<Offer, String> {
        deferred(
            owner,
            vec![
                (image_formats()?.0, self.png.clone()),
                (CF_DIBV5.0.into(), self.dib.clone()),
            ],
            Condition::Any,
        )
    }
    pub fn new(image: &Pixels, text: String) -> Result<Self, String> {
        validate_text(&text)?;
        let png = image.png()?;
        let dib = image.dibv5();
        let html = html(&png, &text);
        if png.len() + dib.len() + html.len() + text.len() * 2 > MAX_BYTES {
            return Err("Drawing clipboard exceeds its memory limit".into());
        }
        Ok(Self {
            text,
            png,
            dib,
            html,
        })
    }

    pub fn copy(&self, owner: HWND) -> Result<Offer, String> {
        self.publish(owner, Condition::Any)
    }
    pub fn copy_if_unchanged(&self, owner: HWND, sequence: u32) -> Result<Offer, String> {
        self.publish(owner, Condition::Sequence(sequence))
    }
    pub fn restore(&self, offer: Offer) -> Result<Offer, String> {
        self.publish(offer.owner, Condition::Offer(offer))
    }
    fn publish(&self, owner: HWND, condition: Condition) -> Result<Offer, String> {
        let formats = image_formats()?;
        publish(
            owner,
            &[
                (formats.0, &self.png),
                (CF_DIBV5.0.into(), &self.dib),
                (formats.1, &self.html),
                (CF_UNICODETEXT.0.into(), &utf16(&self.text)?),
            ],
            condition,
        )
    }
}

pub(super) fn render(format: u32) {
    DELAYED.with(|slot| {
        let Ok(mut state) = slot.try_borrow_mut() else {
            return;
        };
        let Some(state) = state.as_mut() else {
            return;
        };
        if unsafe { GetClipboardOwner() }.ok() != Some(state.offer.owner) {
            return;
        }
        let Some((_, bytes)) = state.formats.iter().find(|(id, _)| *id == format) else {
            return;
        };
        let reader = unsafe { GetOpenClipboardWindow() }.ok().and_then(|hwnd| {
            let mut pid = 0;
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                    hwnd,
                    Some(&mut pid),
                );
            }
            (pid != 0 && pid != std::process::id()).then_some(pid)
        });
        if Memory::new(bytes)
            .and_then(|memory| memory.give(format))
            .is_ok()
        {
            state.offer.sequence = unsafe { GetClipboardSequenceNumber() };
            let pid = reader.unwrap_or(0);
            if std::env::var_os("KOETT_CLIPBOARD_TRACE").is_some() {
                eprintln!("drawing_clipboard_read format={format} pid={pid}");
            }
            if state.reader.len() < 8 && !state.reader.iter().any(|(seen, _)| *seen == pid) {
                state.reader.push((pid, Instant::now()));
            }
        }
    });
}
pub(super) fn rendered(offer: Offer, target_pid: u32) -> (Offer, Option<Instant>, bool) {
    DELAYED.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|state| state.offer.marker == offer.marker)
            .map_or((offer, None, false), |state| {
                let read = state
                    .reader
                    .iter()
                    .find(|(pid, _)| *pid == target_pid)
                    .or_else(|| state.reader.iter().find(|(pid, _)| *pid == 0))
                    .map(|(_, at)| *at);
                (state.offer, read, read.is_some())
            })
    })
}
pub(super) fn forget_delayed() {
    DELAYED.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            *slot = None;
        }
    });
}

#[derive(Clone, Copy)]
pub(super) struct Offer {
    owner: HWND,
    sequence: u32,
    marker: [u8; 16],
}

impl Offer {
    pub fn verify(self) -> Result<(), String> {
        unsafe { OpenClipboard(Some(self.owner)) }.map_err(|e| e.to_string())?;
        let open = Open;
        let current = self.current();
        drop(open);
        if current && unsafe { GetClipboardSequenceNumber() } == self.sequence {
            Ok(())
        } else {
            Err("Clipboard changed; the newer copy was preserved".into())
        }
    }
    pub fn replace_with_text(self, text: &str) -> Result<Offer, String> {
        publish(
            self.owner,
            &[(CF_UNICODETEXT.0.into(), &utf16(text)?)],
            Condition::Offer(self),
        )
    }
    pub fn replace_with_delayed_text(self, text: &str) -> Result<Offer, String> {
        deferred(
            self.owner,
            vec![(CF_UNICODETEXT.0.into(), utf16(text)?)],
            Condition::Offer(self),
        )
    }
    pub fn unchanged(self) -> bool {
        unsafe {
            GetClipboardOwner().ok() == Some(self.owner)
                && GetClipboardSequenceNumber() == self.sequence
        }
    }
    fn current(self) -> bool {
        unsafe {
            GetClipboardOwner().ok() == Some(self.owner)
                && GetClipboardSequenceNumber() == self.sequence
                && marker_matches(self.marker)
        }
    }
}

pub(super) fn render_all(owner: HWND) {
    if unsafe { OpenClipboard(Some(owner)) }.is_err() {
        return;
    }
    let open = Open;
    if unsafe { GetClipboardOwner() }.ok() == Some(owner) {
        let formats = DELAYED.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|state| state.formats.iter().map(|(id, _)| *id).collect::<Vec<_>>())
                .unwrap_or_default()
        });
        for format in formats {
            render(format);
        }
    }
    drop(open);
}

pub(super) fn copy_text(owner: HWND, text: &str) -> Result<(), String> {
    text_offer(owner, text).map(|_| ())
}
pub(super) fn text_offer(owner: HWND, text: &str) -> Result<Offer, String> {
    publish(
        owner,
        &[(CF_UNICODETEXT.0.into(), &utf16(text)?)],
        Condition::Any,
    )
}

fn validate_text(text: &str) -> Result<(), String> {
    if text.contains('\0') || text.len() > MAX_BYTES / 2 - 2 {
        return Err("Clipboard text contains a null character or exceeds its limit".into());
    }
    Ok(())
}

fn utf16(text: &str) -> Result<Vec<u8>, String> {
    validate_text(text)?;
    Ok(text
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect())
}

fn html(png: &[u8], text: &str) -> Vec<u8> {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;");
    let fragment = format!(
        "<div><img src=\"data:image/png;base64,{}\"><pre>{escaped}</pre></div>",
        base64::engine::general_purpose::STANDARD.encode(png)
    );
    let prefix = "<!doctype html><html><body><!--StartFragment-->";
    let suffix = "<!--EndFragment--></body></html>";
    let header = |start: usize, end: usize, from: usize, to: usize| {
        format!(
            "Version:1.0\r\nStartHTML:{start:010}\r\nEndHTML:{end:010}\r\nStartFragment:{from:010}\r\nEndFragment:{to:010}\r\n"
        )
    };
    let start = header(0, 0, 0, 0).len();
    let from = start + prefix.len();
    let to = from + fragment.len();
    let end = to + suffix.len();
    let mut bytes =
        format!("{}{prefix}{fragment}{suffix}", header(start, end, from, to)).into_bytes();
    bytes.push(0);
    bytes
}

fn image_formats() -> Result<(u32, u32), String> {
    let formats = unsafe {
        (
            RegisterClipboardFormatW(w!("PNG")),
            RegisterClipboardFormatW(w!("HTML Format")),
        )
    };
    if formats.0 == 0 || formats.1 == 0 {
        Err("Could not register drawing clipboard formats".into())
    } else {
        Ok(formats)
    }
}

struct Memory(HGLOBAL);
impl Memory {
    fn new(data: &[u8]) -> Result<Self, String> {
        if data.is_empty() || data.len() > MAX_BYTES {
            return Err("Clipboard allocation exceeds its limit".into());
        }
        let memory = Self(
            unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, data.len()) }
                .map_err(|e| e.to_string())?,
        );
        let destination = unsafe { GlobalLock(memory.0) }.cast::<u8>();
        if destination.is_null() {
            return Err("Could not lock clipboard allocation".into());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), destination, data.len());
            let _ = GlobalUnlock(memory.0);
        }
        Ok(memory)
    }
    fn give(mut self, format: u32) -> Result<(), String> {
        unsafe { SetClipboardData(format, Some(HANDLE(self.0.0))) }.map_err(|e| e.to_string())?;
        self.0 = HGLOBAL::default();
        Ok(())
    }
}
impl Drop for Memory {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { GlobalFree(Some(self.0)) };
        }
    }
}
struct Open;
impl Drop for Open {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

enum Condition {
    Any,
    Sequence(u32),
    Offer(Offer),
}

fn deferred(
    owner: HWND,
    formats: Vec<(u32, Vec<u8>)>,
    expected: Condition,
) -> Result<Offer, String> {
    let marker_format = unsafe { RegisterClipboardFormatW(w!("Koett.DrawingOffer")) };
    if marker_format == 0 {
        return Err("Could not register clipboard ownership marker".into());
    }
    let mut marker = [0; 16];
    marker[..4].copy_from_slice(&std::process::id().to_le_bytes());
    marker[4..12].copy_from_slice(&NEXT.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let prepared = Memory::new(&marker)?;
    let mut open = None;
    for _ in 0..10 {
        if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
            open = Some(Open);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let open = open.ok_or("Could not open the Windows clipboard")?;
    let changed = match expected {
        Condition::Any => false,
        Condition::Sequence(sequence) => (unsafe { GetClipboardSequenceNumber() }) != sequence,
        Condition::Offer(offer) => !offer.current(),
    };
    if changed {
        return Err("Clipboard changed; the newer copy was preserved".into());
    }
    unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
    prepared.give(marker_format)?;
    for (format, _) in &formats {
        unsafe {
            windows::Win32::Foundation::SetLastError(windows::Win32::Foundation::WIN32_ERROR(0));
            let _ = SetClipboardData(*format, None);
        }
        let error = unsafe { windows::Win32::Foundation::GetLastError() };
        if error.0 != 0 {
            return Err(format!("Could not offer clipboard data: {}", error.0));
        }
    }
    drop(open);
    let offer = Offer {
        owner,
        sequence: unsafe { GetClipboardSequenceNumber() },
        marker,
    };
    DELAYED.with(|slot| {
        *slot.borrow_mut() = Some(Delayed {
            offer,
            formats,
            reader: Vec::new(),
        });
    });
    Ok(offer)
}

fn publish(owner: HWND, formats: &[(u32, &[u8])], expected: Condition) -> Result<Offer, String> {
    let mut marker = [0u8; 16];
    marker[..4].copy_from_slice(&std::process::id().to_le_bytes());
    marker[4..12].copy_from_slice(&NEXT.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let marker_format = unsafe { RegisterClipboardFormatW(w!("Koett.DrawingOffer")) };
    if marker_format == 0 {
        return Err("Could not register clipboard ownership marker".into());
    }
    let mut prepared = formats
        .iter()
        .map(|(id, data)| Memory::new(data).map(|m| (*id, m)))
        .collect::<Result<Vec<_>, _>>()?;
    prepared.push((marker_format, Memory::new(&marker)?));
    let mut open = None;
    for _ in 0..10 {
        if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
            open = Some(Open);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _open = open.ok_or("Could not open the Windows clipboard")?;
    let changed = match expected {
        Condition::Any => false,
        Condition::Sequence(sequence) => (unsafe { GetClipboardSequenceNumber() }) != sequence,
        Condition::Offer(offer) => !offer.current(),
    };
    if changed {
        return Err("Clipboard changed; the newer copy was preserved".into());
    }
    unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
    for (id, data) in prepared {
        data.give(id)?;
    }
    // Closing publishes the offer and lets Windows add synthesized formats.
    // Its sequence inside the transaction is not the published sequence.
    drop(_open);
    Ok(Offer {
        owner,
        sequence: unsafe { GetClipboardSequenceNumber() },
        marker,
    })
}

fn marker_matches(expected: [u8; 16]) -> bool {
    unsafe {
        let format = RegisterClipboardFormatW(w!("Koett.DrawingOffer"));
        let Ok(handle) = GetClipboardData(format) else {
            return false;
        };
        let handle = HGLOBAL(handle.0);
        if GlobalSize(handle) < expected.len() {
            return false;
        }
        let p = GlobalLock(handle).cast::<u8>();
        if p.is_null() {
            return false;
        }
        let same = std::slice::from_raw_parts(p, expected.len()) == expected;
        let _ = GlobalUnlock(handle);
        same
    }
}

pub(super) fn lab_bundle(owner: HWND, action: usize) -> Result<(), String> {
    LAB.with(|lab| {
        let mut state = lab.borrow_mut();
        if action == 1 {
            let (_, offer) = state.as_mut().ok_or("No drawing clipboard offer")?;
            *offer = offer.replace_with_text("M4 staged text")?;
        } else if action == 2 {
            let (bundle, offer) = state.as_mut().ok_or("No drawing clipboard bundle")?;
            *offer = bundle.copy(owner)?;
        } else if action == 0 {
            let image = Pixels::new(
                2,
                2,
                vec![
                    0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255,
                ],
            )?;
            let bundle = Bundle::new(&image, "M4 drawing fixture ÅÄÖ <tag> & text".into())?;
            let offer = bundle.copy(owner)?;
            *state = Some((bundle, offer));
        } else {
            return Err("Unknown drawing clipboard check".into());
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rendered_receipts_ignore_known_observers_and_accept_anonymous_reads() {
        let offer = Offer {
            owner: HWND::default(),
            sequence: 0,
            marker: [1; 16],
        };
        let first = Instant::now();
        DELAYED.with(|slot| {
            *slot.borrow_mut() = Some(Delayed {
                offer,
                formats: vec![],
                reader: vec![(42, first)],
            });
        });
        let (_, read, requested) = rendered(offer, 7);
        assert_eq!(read, None);
        assert!(!requested);
        DELAYED.with(|slot| {
            slot.borrow_mut().as_mut().unwrap().reader.push((0, first));
        });
        let (_, read, requested) = rendered(offer, 7);
        assert_eq!(read, Some(first));
        assert!(requested);
        let target = first + Duration::from_millis(20);
        DELAYED.with(|slot| {
            slot.borrow_mut().as_mut().unwrap().reader.push((7, target));
        });
        assert_eq!(rendered(offer, 7).1, Some(target));
        forget_delayed();
    }
    #[test]
    fn native_bundle_round_trip_and_newer_copy_preservation() {
        use windows::Win32::System::Com::*;
        use windows::Win32::UI::WindowsAndMessaging::*;
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap() };
        let owner = unsafe {
            CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("Clipboard test"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        let image = Pixels::new(1, 1, vec![0, 0, 255, 255]).unwrap();
        let bundle = Bundle::new(&image, "one ÅÄÖ <tag>".into()).unwrap();
        let offer = bundle.copy(owner).unwrap();
        unsafe { OpenClipboard(Some(owner)) }.unwrap();
        {
            let _open = Open;
            assert!(
                offer.current(),
                "published sequence={} current={} owner={:?} expected={:?} marker={}",
                offer.sequence,
                unsafe { GetClipboardSequenceNumber() },
                unsafe { GetClipboardOwner() },
                owner,
                marker_matches(offer.marker)
            );
            for format in [
                image_formats().unwrap().0,
                image_formats().unwrap().1,
                CF_DIBV5.0.into(),
                CF_UNICODETEXT.0.into(),
            ] {
                assert!(!unsafe { GetClipboardData(format) }.unwrap().is_invalid());
            }
        }
        let text = offer.replace_with_delayed_text("staged ÅÄÖ").unwrap();
        assert!(unsafe { IsClipboardFormatAvailable(image_formats().unwrap().0) }.is_err());
        unsafe { OpenClipboard(Some(owner)) }.unwrap();
        {
            let _open = Open;
            render(CF_UNICODETEXT.0.into());
            assert!(
                !unsafe { GetClipboardData(CF_UNICODETEXT.0.into()) }
                    .unwrap()
                    .is_invalid()
            );
        }
        let (text, read, _) = rendered(text, std::process::id());
        assert!(read.is_some());
        let combined = bundle.restore(text).unwrap();
        for format in [
            image_formats().unwrap().0,
            image_formats().unwrap().1,
            CF_DIBV5.0.into(),
            CF_UNICODETEXT.0.into(),
        ] {
            assert!(unsafe { IsClipboardFormatAvailable(format) }.is_ok());
        }
        let text = combined.replace_with_delayed_text("second text").unwrap();
        let sequence = unsafe { GetClipboardSequenceNumber() };
        copy_text(owner, "new user copy").unwrap();
        assert!(bundle.restore(text).is_err());
        assert!(bundle.copy_if_unchanged(owner, sequence).is_err());
        assert!(
            offer
                .replace_with_text("must not replace user copy")
                .is_err()
        );
        unsafe { OpenClipboard(Some(owner)) }.unwrap();
        {
            let _open = Open;
            let handle = unsafe { GetClipboardData(CF_UNICODETEXT.0.into()) }.unwrap();
            let global = HGLOBAL(handle.0);
            let data = unsafe { GlobalLock(global) }.cast::<u16>();
            assert!(!data.is_null());
            let expected = "new user copy"
                .encode_utf16()
                .chain([0])
                .collect::<Vec<_>>();
            assert_eq!(
                unsafe { std::slice::from_raw_parts(data, expected.len()) },
                expected
            );
            let _ = unsafe { GlobalUnlock(global) };
        }
        unsafe { DestroyWindow(owner) }.unwrap();
        unsafe { CoUninitialize() };
    }

    #[test]
    fn html_offsets_are_utf8_bytes_and_text_is_not_markup() {
        let bytes = html(&[1, 2, 3], "ÅÄÖ <tag> & \"text\"");
        let value = String::from_utf8(bytes.clone()).unwrap();
        let offset = |key: &str| {
            value
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .unwrap()
                .parse::<usize>()
                .unwrap()
        };
        let (a, b) = (offset("StartFragment:"), offset("EndFragment:"));
        let fragment = std::str::from_utf8(&bytes[a..b]).unwrap();
        assert!(fragment.contains("ÅÄÖ &lt;tag&gt; &amp; &quot;text&quot;"));
        assert!(fragment.contains("data:image/png;base64,AQID"));
        assert_eq!(offset("EndHTML:"), bytes.len() - 1);
        assert_eq!(&bytes[offset("StartHTML:")..][..15], b"<!doctype html>");
    }
    #[test]
    fn clipboard_text_is_exact_utf16_and_invalid_text_is_not_truncated() {
        assert_eq!(utf16("Å\n").unwrap(), [197, 0, 10, 0, 0, 0]);
        assert!(utf16("before\0after").is_err());
        assert!(Memory::new(&[]).is_err());
    }
}
