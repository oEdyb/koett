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
    Undo,
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
        WM_NCHITTEST => {
            if let Ok(pill) = unsafe { FindWindowW(w!("KoettOverlayWindow"), None) } {
                let mut point = POINT::default();
                let _ = unsafe { GetCursorPos(&mut point) };
                let mut rect = RECT::default();
                if unsafe { IsWindowVisible(pill) }.as_bool()
                    && unsafe { GetWindowRect(pill, &mut rect) }.is_ok()
                    && point.x >= rect.left
                    && point.x < rect.right
                    && point.y >= rect.top
                    && point.y < rect.bottom
                {
                    return LRESULT(HTTRANSPARENT as isize);
                }
            }
        }
        WM_SETCURSOR => {
            if !unsafe { GetPropW(hwnd, w!("KoettInkArmed")) }.is_invalid() {
                let cursor = unsafe { LoadCursorW(None, IDC_CROSS) }.unwrap_or_default();
                unsafe {
                    SetCursor(Some(cursor));
                }
            }
            return LRESULT(1);
        }
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONUP => {
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
                    WM_RBUTTONUP => Mouse::Undo,
                    _ => Mouse::Up,
                });
            }
            return LRESULT(0);
        }
        WM_NCDESTROY => unsafe {
            let _ = RemovePropW(hwnd, w!("KoettInkArmed"));
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
    pub context: Option<crate::pin_context::PinContext>,
    pub context_status: String,
    pub wall_start: u64,
    pub wall_end: u64,
}
struct Pending {
    at: Instant,
    remaining: usize,
    started_count: usize,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    result: std::sync::mpsc::Receiver<(Request, Result<Captured, String>)>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
    }
}
struct Captured {
    image: Pixels,
    context: Option<crate::pin_context::PinContext>,
    status: String,
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
    context: Option<super::pin_reader::Snapshot>,
    wall_start: u64,
    wall_end: u64,
}
pub(super) struct Board {
    overlays: Vec<Overlay>,
    gesture: PinGesture,
    held: bool,
    armed: bool,
    lock_mouse: bool,
    strokes: Vec<(usize, Stroke)>,
    started_ms: Option<u64>,
    recording_start: Instant,
    started_wall: Option<u64>,
    frames: Vec<Frame>,
    pending: Option<Pending>,
    faded: Vec<(Instant, Request)>,
    active: bool,
    selected: bool,
    last_display_check: Instant,
    pub notice: Option<String>,
}
impl Board {
    pub fn new(selected: bool, recording_start: Instant) -> Result<Self, String> {
        let overlays = displays()?
            .into_iter()
            .map(Overlay::new)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            overlays,
            gesture: PinGesture::default(),
            held: false,
            armed: false,
            lock_mouse: false,
            strokes: Vec::new(),
            started_ms: None,
            recording_start,
            started_wall: None,
            frames: Vec::new(),
            pending: None,
            faded: Vec::new(),
            active: true,
            selected,
            last_display_check: Instant::now(),
            notice: None,
        })
    }
    pub fn events(&self) -> Vec<crate::sketch::SketchEvent> {
        let mut events = self
            .frames
            .iter()
            .enumerate()
            .map(|(i, frame)| crate::sketch::SketchEvent {
                index: i as u32 + 1,
                start_ms: frame.wall_start,
                end_ms: frame.wall_end,
                live: false,
            })
            .collect::<Vec<_>>();
        if self.held && !self.strokes.is_empty() {
            events.push(crate::sketch::SketchEvent {
                index: self.frames.len() as u32 + 1,
                start_ms: self.started_wall.unwrap_or(0),
                end_ms: self.recording_start.elapsed().as_millis() as u64,
                live: true,
            });
        }
        events
    }
    pub fn end_hold(&mut self, audio_ms: u64) {
        self.release(Instant::now(), audio_ms, false);
    }
    pub fn undo_stroke(&mut self) {
        self.gesture.lift();
        self.lock_mouse = false;
        if self.strokes.pop().is_none() {
            self.undo();
        }
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn can_undo(&self) -> bool {
        self.pending.is_some() || !self.strokes.is_empty() || !self.frames.is_empty()
    }
    pub fn take_frames(&mut self) -> Vec<Frame> {
        std::mem::take(&mut self.frames)
    }
    pub fn undo(&mut self) {
        if let Some(pending) = self.pending.take() {
            self.frames.truncate(pending.started_count);
        } else if self.strokes.is_empty() {
            self.frames.pop();
        }
        self.strokes.clear();
        self.faded.clear();
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
                    if self.overlays.iter().any(|o| {
                        (unsafe { GetDpiForWindow(o.hwnd) }.max(96) as f32 / 96.0 - o.scale).abs()
                            > 0.01
                    }) || displays
                        != self.overlays.iter().map(|o| o.display).collect::<Vec<_>>() =>
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
        if state.locked
            && self.lock_mouse
            && unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(1) } >= 0
        {
            self.lock_mouse = false;
            self.gesture.lift();
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
                Mouse::Move(id, point) if self.armed && (!state.locked || self.lock_mouse) => {
                    self.motion(id, point, now, audio_ms)
                }
                Mouse::Down(id, point) if state.locked => {
                    self.lock_mouse = true;
                    self.gesture.lift();
                    self.motion(id, point, now, audio_ms);
                }
                Mouse::Up if state.locked => {
                    self.lock_mouse = false;
                    self.gesture.lift();
                }
                Mouse::Undo if state.locked => self.undo_stroke(),
                Mouse::Down(id, point) => {
                    if let Some((surface, point)) = self.logical(id, point)
                        && self.gesture.button_down(surface, point, now) == Down::Press
                    {
                        self.strokes.clear();
                        self.started_ms = Some(audio_ms);
                        self.started_wall = Some(
                            now.saturating_duration_since(self.recording_start)
                                .as_millis() as u64,
                        );
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
        loop {
            let next = self.pending.as_mut().and_then(|pending| {
                pending.result.try_recv().ok().inspect(|_| {
                    pending.remaining = pending.remaining.saturating_sub(1);
                })
            });
            let Some((mut request, result)) = next else {
                break;
            };
            request.index = self.frames.len() + 1;
            match result.and_then(|captured| {
                render_capture(&request, captured.image)
                    .map(|image| (image, captured.context, captured.status))
            }) {
                Ok((image, context, context_status))
                    if self
                        .frames
                        .iter()
                        .map(|frame| frame.image.bytes().len())
                        .sum::<usize>()
                        + image.bytes().len()
                        <= MAX_FRAME_BYTES =>
                {
                    self.frames.push(Frame {
                        image,
                        start_ms: request.start_ms,
                        end_ms: request.end_ms,
                        pin: request.pin.is_some(),
                        context,
                        context_status,
                        wall_start: request.wall_start,
                        wall_end: request.wall_end,
                    })
                }
                Ok(_) => {
                    self.notice =
                        Some("Drawing memory limit reached; undo a frame to continue".into())
                }
                Err(error) => self.notice = Some(error),
            }
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.remaining == 0)
        {
            self.pending = None;
        }
        if self.pending.as_ref().is_some_and(|pending| {
            now.saturating_duration_since(pending.at) > Duration::from_secs(3)
        }) {
            self.pending = None;
            self.notice = Some("Screen capture timed out; unfinished frames discarded".into());
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
        self.started_wall.get_or_insert(
            now.saturating_duration_since(self.recording_start)
                .as_millis() as u64,
        );
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
        self.started_wall = None;
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
            let requests = surfaces
                .into_iter()
                .filter_map(|surface| self.make_request(surface, None, audio_ms))
                .collect();
            self.capture(requests);
        }
        self.strokes.clear();
        self.started_ms = None;
        self.started_wall = None;
        self.held = false;
        self.armed = false;
        self.lock_mouse = false;
    }
    fn request(&mut self, surface: usize, pin: Option<(f64, f64)>, audio_ms: u64) {
        if let Some(request) = self.make_request(surface, pin, audio_ms) {
            self.capture(vec![request]);
        }
    }
    fn make_request(
        &self,
        surface: usize,
        pin: Option<(f64, f64)>,
        audio_ms: u64,
    ) -> Option<Request> {
        let overlay = self.overlays.get(surface)?;
        let strokes = self
            .strokes
            .iter()
            .filter(|(s, _)| *s == surface)
            .map(|(_, points)| points.clone())
            .collect::<Vec<_>>();
        if pin.is_none()
            && strokes
                .iter()
                .flat_map(|points| points.windows(2))
                .map(|pair| (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1))
                .sum::<f64>()
                < 12.0
        {
            return None;
        }
        Some(Request {
            display: overlay.display,
            scale: overlay.scale,
            strokes,
            pin,
            index: 0,
            start_ms: self.started_ms.unwrap_or(audio_ms),
            end_ms: audio_ms,
            wall_start: self
                .started_wall
                .unwrap_or(self.recording_start.elapsed().as_millis() as u64),
            wall_end: self.recording_start.elapsed().as_millis() as u64,
            context: pin.and_then(|point| {
                super::pin_reader::Snapshot::at(POINT {
                    x: overlay.display.rect.left + (point.0 * f64::from(overlay.scale)) as i32,
                    y: overlay.display.rect.top + (point.1 * f64::from(overlay.scale)) as i32,
                })
            }),
        })
    }
    fn capture(&mut self, mut requests: Vec<Request>) {
        if requests.is_empty() {
            return;
        }
        if self.pending.is_some() {
            self.notice = Some("Screen capture busy; this mark was not captured".into());
            return;
        }
        if requests.len() > 8 || self.frames.len() + requests.len() > MAX_FRAMES {
            self.notice = Some("Drawing limit reached; undo a frame to continue".into());
            return;
        }
        if let Err(error) = capture::exclude_own_windows() {
            self.notice = Some(error);
            return;
        }
        for (index, request) in requests.iter_mut().enumerate() {
            request.index = self.frames.len() + index + 1;
        }
        self.faded = requests
            .iter()
            .cloned()
            .map(|request| (Instant::now(), request))
            .collect();
        let (send, result) = std::sync::mpsc::sync_channel(8);
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let selected = self.selected;
        self.pending = Some(Pending {
            at: Instant::now(),
            remaining: requests.len(),
            started_count: self.frames.len(),
            cancelled,
            result,
        });
        std::thread::spawn(move || {
            for request in requests {
                if worker_cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }
                let captured = capture::monitor(request.display.id).map(|image| {
                    let read = if worker_cancelled.load(std::sync::atomic::Ordering::Acquire) {
                        None
                    } else {
                        request
                            .context
                            .as_ref()
                            .map(|snapshot| snapshot.read(selected))
                    };
                    Captured {
                        image,
                        context: read.as_ref().and_then(|read| read.context.clone()),
                        status: read
                            .map_or_else(|| "Context unavailable".into(), |read| read.status),
                    }
                });
                if send.send((request, captured)).is_err() {
                    break;
                }
            }
        });
    }
    fn paint(&self, protect: bool, now: Instant) -> Result<(), String> {
        for (index, overlay) in self.overlays.iter().enumerate() {
            if self.armed {
                let _ = unsafe {
                    SetPropW(
                        overlay.hwnd,
                        w!("KoettInkArmed"),
                        Some(HANDLE(overlay.hwnd.0)),
                    )
                };
            } else {
                let _ = unsafe { RemovePropW(overlay.hwnd, w!("KoettInkArmed")) };
            }
            let faded = self.faded.iter().find(|(at, r)| {
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
