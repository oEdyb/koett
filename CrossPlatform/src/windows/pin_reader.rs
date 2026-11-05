use super::pin_policy::{private_indicator, video_seconds, web_url, youtube_time};
use crate::pin_context::{PinContext, SELECTION_LIMIT, TITLE_LIMIT, clean};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::*,
        System::Com::*,
        UI::{Accessibility::*, WindowsAndMessaging::*},
    },
    core::Interface,
};
static ACTIVE: AtomicBool = AtomicBool::new(false);
struct Reading;
impl Drop for Reading {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}
#[derive(Clone)]
pub(super) struct Snapshot {
    hwnd: usize,
    pid: u32,
    rect: RECT,
    title: String,
    app: Option<String>,
    point: POINT,
}
pub(super) struct Read {
    pub context: Option<PinContext>,
    pub status: String,
}
fn title(hwnd: HWND) -> String {
    let mut text = [0; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut text) };
    String::from_utf16_lossy(&text[..len.max(0) as usize])
}
impl Snapshot {
    pub fn at(point: POINT) -> Option<Self> {
        let mut window = unsafe { GetTopWindow(None) }.ok()?;
        for _ in 0..256 {
            let mut pid = 0;
            let mut rect = RECT::default();
            unsafe {
                GetWindowThreadProcessId(window, Some(&mut pid));
            }
            let mut cloaked = 1_u32;
            let visible = unsafe {
                windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(
                    window,
                    windows::Win32::Graphics::Dwm::DWMWA_CLOAKED,
                    (&mut cloaked as *mut u32).cast(),
                    4,
                )
            }
            .is_ok()
                && cloaked == 0;
            if visible
                && unsafe { IsWindowVisible(window) }.as_bool()
                && unsafe { GetWindowRect(window, &mut rect) }.is_ok()
                && inside(rect, point)
            {
                if pid == std::process::id() {
                    let mut class = [0; 64];
                    let count = unsafe { GetClassNameW(window, &mut class) };
                    if !matches!(
                        String::from_utf16_lossy(&class[..count.max(0) as usize]).as_str(),
                        "KoettDrawingCanvas" | "KoettOverlayWindow"
                    ) {
                        return None;
                    }
                    window = unsafe { GetWindow(window, GW_HWNDNEXT) }.ok()?;
                    continue;
                }
                return Some(Self {
                    hwnd: window.0 as usize,
                    pid,
                    rect,
                    title: title(window),
                    app: super::paste_chord::process_key(pid).map(|key| {
                        key.trim_start_matches("windows:")
                            .trim_end_matches(".exe")
                            .to_owned()
                    }),
                    point,
                });
            }
            window = unsafe { GetWindow(window, GW_HWNDNEXT) }.ok()?;
        }
        None
    }
    fn current(&self) -> bool {
        let hwnd = HWND(self.hwnd as *mut _);
        let mut pid = 0;
        let mut rect = RECT::default();
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        pid == self.pid
            && unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok()
            && rect == self.rect
            && title(hwnd) == self.title
    }
    pub fn read(&self, selected: bool) -> Read {
        let missing = |message: &str| Read {
            context: None,
            status: message.into(),
        };
        if ACTIVE.swap(true, Ordering::AcqRel) {
            return missing("Context unavailable: reader busy");
        }
        let _reading = Reading;
        if !self.current() {
            return missing("Context unavailable: window changed");
        }
        if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.is_err() {
            return missing("Context unavailable: accessibility failed");
        }
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _apartment = Apartment;
        let result = self.accessible(selected);
        if !self.current() {
            return missing("Context unavailable: window changed");
        }
        result.unwrap_or_else(|_| missing("Context unavailable: app does not expose accessibility"))
    }
    fn accessible(&self, selected: bool) -> windows::core::Result<Read> {
        unsafe {
            let automation: IUIAutomation2 =
                CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)?;
            automation.SetConnectionTimeout(100)?;
            automation.SetTransactionTimeout(100)?;
            let automation: IUIAutomation = automation.cast()?;
            let root = automation.ElementFromHandle(HWND(self.hwnd as *mut _))?;
            if root.CurrentProcessId()? != self.pid as i32 {
                return Err(windows::core::Error::new(E_FAIL, "Window changed"));
            }
            let walker = automation.ControlViewWalker()?;
            let deadline = Instant::now() + Duration::from_millis(800);
            let mut queue = std::collections::VecDeque::from([(root, false)]);
            let mut context = PinContext {
                app: clean(self.app.as_deref(), TITLE_LIMIT),
                title: clean(Some(&self.title), TITLE_LIMIT),
                ..Default::default()
            };
            let browser = matches!(
                context.app.as_deref(),
                Some("msedge" | "chrome" | "firefox")
            );
            let mut private = false;
            let mut password = automation
                .GetFocusedElement()
                .ok()
                .filter(|element| element.CurrentProcessId().ok() == Some(self.pid as i32))
                .is_some_and(|element| {
                    element
                        .CurrentIsPassword()
                        .map_or(true, |value| value.as_bool())
                });
            let mut examined = 0;
            let mut video_time = None;
            while let Some((element, in_document)) = queue.pop_front() {
                if examined >= 96 || Instant::now() >= deadline {
                    break;
                }
                examined += 1;
                let bounds = element.CurrentBoundingRectangle()?;
                // Failed password queries are never treated as non-password.
                let is_password = element.CurrentIsPassword()?.as_bool();
                let kind = element.CurrentControlType()?;
                let document = kind == UIA_DocumentControlTypeId;
                if is_password && inside(bounds, self.point) {
                    password = true;
                }
                if !is_password {
                    let name = element.CurrentName()?.to_string();
                    let id = element.CurrentAutomationId()?.to_string();
                    if browser
                        && !in_document
                        && private_indicator(&name)
                        && (kind == UIA_ButtonControlTypeId || kind == UIA_TextControlTypeId)
                    {
                        private = true;
                    }
                    if browser
                        && context.url.is_none()
                        && !in_document
                        && (document && inside(bounds, self.point)
                            || matches!(id.as_str(), "addressEditBox" | "urlbar-input")
                            || name == "Address and search bar")
                        && let Ok(value) = element
                            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                            .and_then(|p| p.CurrentValue())
                    {
                        context.url = web_url(&value.to_string());
                    }
                    if inside(bounds, self.point) {
                        if kind == UIA_ImageControlTypeId {
                            context.image = clean(Some(&name), TITLE_LIMIT);
                        }
                        if kind == UIA_HyperlinkControlTypeId
                            && let Ok(value) = element
                                .GetCurrentPatternAs::<IUIAutomationValuePattern>(
                                    UIA_ValuePatternId,
                                )
                                .and_then(|p| p.CurrentValue())
                        {
                            context.link = web_url(&value.to_string());
                        }
                        if selected
                            && let Ok(pattern) = element
                                .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                            && let Ok(ranges) = pattern.GetSelection()
                            && ranges.Length()? > 0
                        {
                            context.selection = clean(
                                Some(
                                    &ranges
                                        .GetElement(0)?
                                        .GetText(SELECTION_LIMIT as i32)?
                                        .to_string(),
                                ),
                                SELECTION_LIMIT,
                            );
                        }
                    }
                    if name == "Seek slider"
                        && let Ok(properties) = element.CurrentAriaProperties()
                    {
                        video_time = video_seconds(&properties.to_string());
                    }
                }
                if Instant::now() >= deadline {
                    break;
                }
                let mut child = walker.GetFirstChildElement(&element).ok();
                while let Some(element) = child {
                    if examined + queue.len() >= 96 || Instant::now() >= deadline {
                        break;
                    }
                    child = walker.GetNextSiblingElement(&element).ok();
                    queue.push_back((element, in_document || document));
                }
            }
            if password {
                context.selection = None;
                context.link = None;
                context.image = None;
            }
            if private {
                context.title = None;
                context.image = None;
                context.url = None;
                context.link = None;
                context.selection = None;
            }
            let mut status = Vec::new();
            if context.app.is_none() {
                status.push("App unavailable");
            }
            if context.url.is_none() {
                status.push("URL unavailable");
            }
            status.push(if private {
                "Private mode: context omitted"
            } else {
                "Private mode unknown"
            });
            if selected && context.selection.is_none() {
                status.push(if password {
                    "Password text omitted"
                } else {
                    "Selection unavailable"
                });
            }
            if let Some(url) = context.url.as_deref()
                && youtube_time(url, 0.0).is_some()
            {
                if let Some(time) = video_time.and_then(|time| youtube_time(url, time)) {
                    context.url = Some(time);
                } else {
                    status.push("Video time unavailable");
                }
            }
            Ok(Read {
                context: Some(context),
                status: status.join(" · "),
            })
        }
    }
}
fn inside(rect: RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}
