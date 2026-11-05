#[path = "../linux/sketch/gesture.rs"]
mod gesture;
use super::{capture, draw_input, drawing_render::Surface, pixels::Pixels};
use gesture::{Down, Motion, PinGesture};
use std::{
    cell::RefCell,
    collections::VecDeque,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{Direct2D::Common::D2D_RECT_F, Gdi::*},
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
    core::w,
};

const MAX_FRAMES: usize = 256;
const MAX_FRAME_BYTES: usize = 96 * 1024 * 1024;
const MAX_POINTS: usize = 20_000;
type Stroke = Vec<(f64, f64)>;
#[derive(Clone, Copy, Debug, PartialEq)]
struct Display {
    id: usize,
    rect: RECT,
}
#[derive(Clone, Copy)]
enum Mouse {
    Move(usize, (f64, f64)),
    Down(usize, (f64, f64)),
    Up,
}
struct Overlay {
    hwnd: HWND,
    display: Display,
    scale: f32,
    surface: Surface,
    events: Box<RefCell<VecDeque<Mouse>>>,
}
impl Drop for Overlay {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lp.0 as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    match message {
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_SETCURSOR => {
            let cursor = unsafe { LoadCursorW(None, IDC_CROSS) }.unwrap_or_default();
            unsafe {
                SetCursor(Some(cursor));
            }
            return LRESULT(1);
        }
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) }
                as *const RefCell<VecDeque<Mouse>>;
            if !ptr.is_null()
                && let Ok(mut events) = unsafe { &*ptr }.try_borrow_mut()
                && events.len() < MAX_POINTS
            {
                let point = ((lp.0 as i16) as f64, ((lp.0 >> 16) as i16) as f64);
                events.push_back(match message {
                    WM_MOUSEMOVE => Mouse::Move(hwnd.0 as usize, point),
                    WM_LBUTTONDOWN => Mouse::Down(hwnd.0 as usize, point),
                    _ => Mouse::Up,
                });
            }
            return LRESULT(0);
        }
        WM_NCDESTROY => unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        },
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, message, wp, lp) }
}
fn displays() -> Result<Vec<Display>, String> {
    unsafe extern "system" fn visit(
        monitor: HMONITOR,
        _: HDC,
        rect: *mut RECT,
        lp: LPARAM,
    ) -> windows::core::BOOL {
        let list = unsafe { &mut *(lp.0 as *mut Vec<Display>) };
        list.push(Display {
            id: monitor.0 as usize,
            rect: unsafe { *rect },
        });
        true.into()
    }
    let mut list = Vec::new();
    if !unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(visit),
            LPARAM((&mut list as *mut Vec<Display>) as isize),
        )
    }
    .as_bool()
    {
        return Err("Could not read displays".into());
    }
    if list.is_empty() || list.len() > 8 {
        return Err("Drawing supports one to eight displays".into());
    }
    Ok(list)
}
impl Overlay {
    fn new(display: Display) -> Result<Self, String> {
        let instance = unsafe { GetModuleHandleW(None) }.map_err(|e| e.to_string())?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance.into(),
            lpszClassName: w!("KoettDrawingCanvas"),
            ..Default::default()
        };
        unsafe {
            RegisterClassW(&class);
        }
        let rect = display.rect;
        let surface = Surface::new(
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        )
        .map_err(|e| e.to_string())?;
        let events = Box::new(RefCell::new(VecDeque::new()));
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW
                    | WS_EX_LAYERED
                    | WS_EX_TRANSPARENT,
                w!("KoettDrawingCanvas"),
                w!("Koett drawing"),
                WS_POPUP,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                None,
                None,
                Some(instance.into()),
                Some((&*events as *const RefCell<VecDeque<Mouse>>).cast()),
            )
        }
        .map_err(|e| e.to_string())?;
        let overlay = Self {
            hwnd,
            display,
            scale: unsafe { GetDpiForWindow(hwnd) }.max(96) as f32 / 96.0,
            surface,
            events,
        };
        unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) }
            .map_err(|e| e.to_string())?;
        Ok(overlay)
    }
    fn input(&self, active: bool) {
        unsafe {
            let old = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32;
            let next = if active {
                old & !WS_EX_TRANSPARENT.0
            } else {
                old | WS_EX_TRANSPARENT.0
            };
            if next != old {
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, next as isize);
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct Frame {
    pub image: Pixels,
    pub start_ms: u64,
    pub end_ms: u64,
    pub pin: bool,
}
struct Pending {
    at: Instant,
    request: Request,
    result: std::sync::mpsc::Receiver<Result<Pixels, String>>,
}
#[derive(Clone)]
struct Request {
    display: Display,
    scale: f32,
    strokes: Vec<Stroke>,
    pin: Option<(f64, f64)>,
    index: usize,
    start_ms: u64,
    end_ms: u64,
}
pub(super) struct Board {
    overlays: Vec<Overlay>,
    gesture: PinGesture,
    held: bool,
    armed: bool,
    strokes: Vec<(usize, Stroke)>,
    started_ms: Option<u64>,
    frames: Vec<Frame>,
    pending: Option<Pending>,
    faded: Option<(Instant, Request)>,
    active: bool,
    last_display_check: Instant,
    pub notice: Option<String>,
}
impl Board {
    pub fn new() -> Result<Self, String> {
        let overlays = displays()?
            .into_iter()
            .map(Overlay::new)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            overlays,
            gesture: PinGesture::default(),
            held: false,
            armed: false,
            strokes: Vec::new(),
            started_ms: None,
            frames: Vec::new(),
            pending: None,
            faded: None,
            active: true,
            last_display_check: Instant::now(),
            notice: None,
        })
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn take_frames(&mut self) -> Vec<Frame> {
        std::mem::take(&mut self.frames)
    }
    pub fn undo(&mut self) {
        self.pending = None;
        self.strokes.clear();
        self.frames.pop();
        self.faded = None;
        self.gesture = PinGesture::default();
        self.held = false;
    }
    pub fn stop(&mut self, audio_ms: u64, cancel: bool) {
        if cancel {
            self.undo();
            self.frames.clear();
        } else {
            self.release(Instant::now(), audio_ms, false);
        }
        self.active = false;
        for overlay in &self.overlays {
            overlay.input(false);
            unsafe {
                let _ = ShowWindow(overlay.hwnd, SW_HIDE);
            }
        }
    }
    pub fn update(&mut self, state: draw_input::State, audio_ms: u64, now: Instant) {
        if self.last_display_check.elapsed() >= Duration::from_millis(500) {
            self.last_display_check = now;
            match displays() {
                Ok(displays)
                    if displays != self.overlays.iter().map(|o| o.display).collect::<Vec<_>>() =>
                {
                    self.release(now, audio_ms, true);
                    self.pending = None;
                    match displays
                        .into_iter()
                        .map(Overlay::new)
                        .collect::<Result<Vec<_>, _>>()
                    {
                        Ok(overlays) => self.overlays = overlays,
                        Err(e) => {
                            self.notice = Some(e);
                            self.active = false;
                        }
                    }
                    self.notice
                        .get_or_insert("Display changed; unfinished drawing discarded".into());
                }
                Err(e) => {
                    self.notice = Some(e);
                    self.active = false;
                }
                _ => {}
            }
        }
        let protect = self.active && state.protect_clicks;
        if protect && !self.held {
            self.held = true;
            self.gesture.hold();
        }
        if !protect && self.held {
            self.release(now, audio_ms, state.discard);
        }
        self.armed = protect && state.ink;
        let mut events = VecDeque::new();
        for overlay in &self.overlays {
            events.append(&mut overlay.events.borrow_mut());
        }
        for event in events {
            if !protect {
                continue;
            }
            match event {
                Mouse::Move(id, point) if self.armed => self.motion(id, point, now, audio_ms),
                Mouse::Down(id, point) => {
                    if let Some((surface, point)) = self.logical(id, point)
                        && self.gesture.button_down(surface, point, now) == Down::Press
                    {
                        self.strokes.clear();
                        self.started_ms = Some(audio_ms);
                    }
                }
                Mouse::Up => {
                    if let Some(mark) = self.gesture.button_up(now) {
                        self.pin(mark, audio_ms);
                    }
                }
                _ => {}
            }
        }
        if let Some(result) = self.pending.as_ref().and_then(|pending| {
            pending.result.try_recv().ok().or_else(|| {
                (now.saturating_duration_since(pending.at) > Duration::from_secs(3))
                    .then(|| Err("Screen capture timed out".into()))
            })
        }) {
            let pending = self.pending.take().expect("pending capture exists");
            match result.and_then(|image| render_capture(&pending.request, image)) {
                Ok(image)
                    if self
                        .frames
                        .iter()
                        .map(|f| f.image.bytes().len())
                        .sum::<usize>()
                        + image.bytes().len()
                        <= MAX_FRAME_BYTES =>
                {
                    self.frames.push(Frame {
                        image,
                        start_ms: pending.request.start_ms,
                        end_ms: pending.request.end_ms,
                        pin: pending.request.pin.is_some(),
                    })
                }
                Ok(_) => {
                    self.notice =
                        Some("Drawing memory limit reached; undo a frame to continue".into())
                }
                Err(e) => self.notice = Some(e),
            }
        }
        if let Err(e) = self.paint(protect, now) {
            self.notice = Some(e);
            self.active = false;
            for overlay in &self.overlays {
                overlay.input(false);
                unsafe {
                    let _ = ShowWindow(overlay.hwnd, SW_HIDE);
                }
            }
        }
    }
    fn logical(&self, id: usize, point: (f64, f64)) -> Option<(usize, (f64, f64))> {
        self.overlays
            .iter()
            .position(|o| o.hwnd.0 as usize == id)
            .map(|i| {
                (
                    i,
                    (
                        point.0 / f64::from(self.overlays[i].scale),
                        point.1 / f64::from(self.overlays[i].scale),
                    ),
                )
            })
    }
    fn motion(&mut self, id: usize, point: (f64, f64), now: Instant, audio_ms: u64) {
        let Some((surface, point)) = self.logical(id, point) else {
            self.gesture.lift();
            return;
        };
        let motion = self.gesture.motion(surface, point, now);
        self.started_ms.get_or_insert(audio_ms);
        if let Motion::Ink { from, to } = motion {
            if self.strokes.iter().map(|(_, p)| p.len()).sum::<usize>() >= MAX_POINTS {
                self.release(now, audio_ms, true);
                self.notice = Some("Stroke limit reached; release the draw key".into());
                return;
            }
            if let Some(from) = from {
                self.strokes.push((surface, vec![from.point]));
            }
            if let Some((_, points)) = self.strokes.last_mut().filter(|(s, _)| *s == surface) {
                points.push(to);
            }
        }
    }
    fn pin(&mut self, mark: gesture::Mark, audio_ms: u64) {
        self.request(mark.surface, Some(mark.point), audio_ms);
        self.strokes.clear();
        self.started_ms = None;
    }
    fn release(&mut self, now: Instant, audio_ms: u64, discard: bool) {
        if discard {
            self.gesture = PinGesture::default();
        } else {
            if let Some(mark) = self.gesture.release(now) {
                self.pin(mark, audio_ms);
            }
            let surfaces = self
                .strokes
                .iter()
                .map(|(s, _)| *s)
                .collect::<std::collections::BTreeSet<_>>();
            for surface in surfaces {
                self.request(surface, None, audio_ms);
            }
        }
        self.strokes.clear();
        self.started_ms = None;
        self.held = false;
        self.armed = false;
    }
    fn request(&mut self, surface: usize, pin: Option<(f64, f64)>, audio_ms: u64) {
        if self.pending.is_some() {
            self.notice = Some("Screen capture busy; this mark was not captured".into());
            return;
        }
        if self.frames.len() >= MAX_FRAMES {
            self.notice = Some("Drawing limit reached; undo a frame to continue".into());
            return;
        }
        let Some(overlay) = self.overlays.get(surface) else {
            return;
        };
        let strokes = self
            .strokes
            .iter()
            .filter(|(s, _)| *s == surface)
            .map(|(_, p)| p.clone())
            .collect::<Vec<_>>();
        if pin.is_none()
            && strokes
                .iter()
                .flat_map(|p| p.windows(2))
                .map(|p| (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1))
                .sum::<f64>()
                < 12.0
        {
            return;
        }
        let request = Request {
            display: overlay.display,
            scale: overlay.scale,
            strokes,
            pin,
            index: self.frames.len() + 1,
            start_ms: self.started_ms.unwrap_or(audio_ms),
            end_ms: audio_ms,
        };
        if let Err(e) = capture::exclude_own_windows() {
            self.notice = Some(e);
            return;
        }
        let (send, result) = std::sync::mpsc::sync_channel(1);
        let monitor = request.display.id;
        std::thread::spawn(move || {
            let _ = send.send(capture::monitor(monitor));
        });
        self.faded = Some((Instant::now(), request.clone()));
        self.pending = Some(Pending {
            at: Instant::now(),
            request,
            result,
        });
    }
    fn paint(&self, protect: bool, now: Instant) -> Result<(), String> {
        for (index, overlay) in self.overlays.iter().enumerate() {
            let faded = self.faded.as_ref().filter(|(at, r)| {
                r.display == overlay.display
                    && now.saturating_duration_since(*at) < Duration::from_millis(1900)
            });
            if !protect && faded.is_none() {
                overlay.input(false);
                unsafe {
                    let _ = ShowWindow(overlay.hwnd, SW_HIDE);
                }
                continue;
            }
            let draw = || -> windows::core::Result<()> {
                overlay.surface.begin(protect);
                for (_, points) in self.strokes.iter().filter(|(s, _)| *s == index) {
                    overlay.surface.stroke(points, overlay.scale, 1.0)?;
                }
                if let Some((at, request)) = faded {
                    let alpha = (1.0
                        - (now.saturating_duration_since(*at).as_secs_f32() - 1.5) / 0.4)
                        .clamp(0.0, 1.0);
                    for points in &request.strokes {
                        overlay.surface.stroke(points, overlay.scale, alpha)?;
                    }
                    if let Some(point) = request.pin {
                        overlay
                            .surface
                            .pin(point, request.index, overlay.scale, alpha)?;
                    }
                }
                overlay.surface.finish()?;
                overlay.surface.present(
                    overlay.hwnd,
                    POINT {
                        x: overlay.display.rect.left,
                        y: overlay.display.rect.top,
                    },
                )
            };
            draw().map_err(|e| e.to_string())?;
            overlay.input(protect);
            unsafe {
                let _ = ShowWindow(overlay.hwnd, SW_SHOWNOACTIVATE);
            }
        }
        Ok(())
    }
}
fn render_capture(request: &Request, image: Pixels) -> Result<Pixels, String> {
    let (width, height) = image.size();
    let surface = Surface::new(width, height).map_err(|e| e.to_string())?;
    let draw = || -> windows::core::Result<()> {
        surface.begin(false);
        surface.background(
            &image,
            D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: width as f32,
                bottom: height as f32,
            },
        )?;
        let scale = request.scale * width as f32
            / (request.display.rect.right - request.display.rect.left) as f32;
        for points in &request.strokes {
            surface.stroke(points, scale, 1.0)?;
        }
        if let Some(point) = request.pin {
            surface.pin(point, request.index, scale, 1.0)?;
        }
        surface.finish()
    };
    draw().map_err(|e| e.to_string())?;
    surface.pixels()
}
