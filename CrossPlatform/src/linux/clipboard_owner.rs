//! Regular clipboard ownership for the niri/wtype path; never sends keyboard input.
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, Write},
    os::fd::AsRawFd,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    protocol::{wl_callback, wl_registry, wl_seat},
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1 as device, zwlr_data_control_manager_v1 as manager,
    zwlr_data_control_offer_v1 as offer, zwlr_data_control_source_v1 as source,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const DROP_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_WRITERS: usize = 4;
#[derive(Clone)]
pub(super) struct Payload(pub Vec<(&'static str, Arc<[u8]>)>, Option<ReadSignal>);
#[derive(Clone)]
struct ReadSignal {
    armed: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
    done: mpsc::SyncSender<()>,
}
impl ReadSignal {
    fn begin(&self) -> Option<Self> {
        self.armed.load(Ordering::Acquire).then(|| {
            self.started.store(true, Ordering::Release);
            self.clone()
        })
    }
}
pub(super) struct ImageRead {
    signal: ReadSignal,
    done: mpsc::Receiver<()>,
}
impl ImageRead {
    pub fn arm(&self) {
        self.signal.armed.store(true, Ordering::Release);
    }
    pub fn started(&self) -> bool {
        self.signal.started.load(Ordering::Acquire)
    }
    /// Whether anyone started reading after `arm`, within `limit` of `posted`. A paste
    /// has to read the clipboard, so this is the evidence that one happened.
    pub fn wait_started(&self, posted: Instant, limit: Duration) -> bool {
        loop {
            if self.started() {
                return true;
            }
            if posted.elapsed() >= limit {
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    /// A reader already in flight gets longer to finish than an unknown target.
    /// Completion means the whole pipe was written and closed, not PNG decoding.
    pub fn wait(&self, posted: Instant, fallback: Duration, transfer: Duration) -> bool {
        loop {
            let limit = if self.signal.started.load(Ordering::Acquire) {
                transfer
            } else {
                fallback
            };
            let remaining = (posted + limit).saturating_duration_since(Instant::now());
            match self
                .done
                .recv_timeout(remaining.min(Duration::from_millis(10)))
            {
                Ok(()) => return true,
                Err(_) if remaining.is_zero() => return false,
                Err(_) => {}
            }
        }
    }
}
/// Tells clipboard managers to skip an offer: CopyQ, Klipper and cliphist don't store, show
/// or act on it. The same hint `wl-copy --sensitive` and KeePassXC set.
const SECRET_HINT: &str = "x-kde-passwordManagerHint";
impl Payload {
    pub fn image(png: &[u8]) -> Self {
        Self(vec![("image/png", Arc::from(png))], None)
    }
    pub fn tracked_image(png: &[u8]) -> (Self, ImageRead) {
        let (done, receiver) = mpsc::sync_channel(1);
        let signal = ReadSignal {
            armed: Arc::new(AtomicBool::new(false)),
            started: Arc::new(AtomicBool::new(false)),
            done,
        };
        // The drawing's offer lasts only until its paste, so no clipboard manager needs it.
        let mut payload = Self::image(png);
        payload.0.push((SECRET_HINT, Arc::from(&b"secret"[..])));
        payload.1 = Some(signal.clone());
        (
            payload,
            ImageRead {
                signal,
                done: receiver,
            },
        )
    }
    /// Text that reports the first read after `arm`, as `tracked_image` does.
    pub fn tracked_text(text: &str) -> (Self, ImageRead) {
        let (done, receiver) = mpsc::sync_channel(1);
        let signal = ReadSignal {
            armed: Arc::new(AtomicBool::new(false)),
            started: Arc::new(AtomicBool::new(false)),
            done,
        };
        let mut payload = Self::text(text);
        payload.1 = Some(signal.clone());
        (
            payload,
            ImageRead {
                signal,
                done: receiver,
            },
        )
    }
    pub fn text(text: &str) -> Self {
        let bytes = Arc::from(text.as_bytes());
        Self(
            vec![
                ("text/plain;charset=utf-8", Arc::clone(&bytes)),
                ("text/plain", bytes),
            ],
            None,
        )
    }
    pub fn bundle(png: &[u8], text: &str) -> Self {
        let mut data = Self::image(png).0;
        data.push((
            "text/html",
            Arc::from(super::feedback::sketch_html(text, png).into_bytes()),
        ));
        data.extend(Self::text(text).0);
        Self(data, None)
    }
}
/// Whether the clipboard's current owner serves exactly `bytes` as `mime`.
/// A clipboard manager that takes every new selection serves the same bytes, so a paste
/// from it inserts what Koett offered.
pub(super) fn clipboard_holds(mime: &str, bytes: &[u8]) -> bool {
    read_command("wl-paste", &["--no-newline", "--type", mime]).is_some_and(|held| held == bytes)
}

/// A clipboard reader's output, or None if it fails or takes over two seconds: an owner
/// that never answers would otherwise hold the caller back.
pub(super) fn read_command(program: &str, args: &[&str]) -> Option<Vec<u8>> {
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut stdout, &mut bytes).map(|_| bytes)
    });
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if started.elapsed() > std::time::Duration::from_secs(2) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
    reader.join().ok()?.ok()
}
struct Request {
    payload: Payload,
    deadline: Instant,
    reply: mpsc::SyncSender<Result<(), String>>,
}
struct Owner {
    requests: mpsc::Sender<Request>,
    stop: Arc<AtomicBool>,
    /// Whether this owner's offer is still the selection. Each owner has its own, so a
    /// stalled old thread can't clear the live one's.
    owned: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let deadline = Instant::now() + DROP_TIMEOUT;
            while !worker.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(2));
            }
            if !worker.is_finished() {
                // No unbounded join, even if a connection setup or library call stalls.
                // The in-process thread is detached, not a surviving helper process.
                eprintln!("clipboard_owner event=shutdown_timeout");
            }
        }
    }
}
static OWNER: Mutex<Option<Owner>> = Mutex::new(None);
pub(super) fn stop() {
    OWNER.lock().unwrap_or_else(|e| e.into_inner()).take();
}
/// Whether Koett's current offer is still the selection. Cleared the moment another client
/// sets one, from the event itself rather than the next loop turn.
pub(super) fn owned() -> bool {
    OWNER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|owner| owner.owned.load(Ordering::Acquire))
}
/// Offers a payload without leaving the clipboard empty: a live owner switches its selection
/// in place, the new source set before the old one goes. Stopping one owner and starting
/// another leaves a moment with no clipboard, and CopyQ fills it with its last item, which
/// then takes the paste.
pub(super) fn offer(payload: Payload) -> Result<(), String> {
    if owned() && replace(payload.clone()).is_ok() {
        return Ok(());
    }
    start(payload)
}
pub(super) fn start(payload: Payload) -> Result<(), String> {
    let mut owner = OWNER.lock().map_err(|e| e.to_string())?;
    owner.take();
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let owned = Arc::new(AtomicBool::new(false));
    let (flag, serving) = (stop.clone(), owned.clone());
    let worker = thread::Builder::new()
        .name("koett-clipboard".into())
        .spawn(move || {
            if let Err(error) = serve(rx, flag, serving) {
                eprintln!("clipboard_owner event=stopped error={error}");
            }
        })
        .map_err(|e| format!("Could not start clipboard owner: {e}"))?;
    let next = Owner {
        requests: tx,
        stop,
        owned,
        worker: Some(worker),
    };
    let result = request(&next, payload);
    if result.is_ok() {
        *owner = Some(next);
    }
    result
}
pub(super) fn replace(payload: Payload) -> Result<(), String> {
    let owner = OWNER.lock().map_err(|e| e.to_string())?;
    request(
        owner.as_ref().ok_or("Sketch clipboard ownership ended")?,
        payload,
    )
}
fn request(owner: &Owner, payload: Payload) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel(1);
    owner
        .requests
        .send(Request {
            payload,
            deadline: Instant::now() + REQUEST_TIMEOUT,
            reply: tx,
        })
        .map_err(|_| "Sketch clipboard was replaced")?;
    match rx.recv_timeout(REQUEST_TIMEOUT + Duration::from_millis(100)) {
        Ok(result) => result,
        Err(_) => {
            owner.stop.store(true, Ordering::Release);
            Err("Sketch clipboard owner timed out".into())
        }
    }
}

/// Selection notifications distinguish an expected self-replacement from a foreign copy.
/// A foreign selection during a transition is terminal, even if our queued claim wins later.
#[derive(Default)]
struct Ownership {
    active: bool,
    owned: bool,
    /// The owner's shared `owned` flag, cleared from the selection event itself.
    shared: Arc<AtomicBool>,
    transitioning: bool,
    foreign_seen: bool,
    latest_ours: bool,
}
impl Ownership {
    fn begin(&mut self) {
        self.active = true;
        self.owned = true;
        self.transitioning = true;
        self.foreign_seen = false;
        self.latest_ours = false;
    }
    fn selection(&mut self, ours: bool) {
        if !self.active {
            return;
        }
        self.latest_ours = ours;
        if !ours {
            self.shared.store(false, Ordering::Release);
            self.owned = false;
            if self.transitioning {
                self.foreign_seen = true;
            }
        }
    }
    fn finish(&mut self) -> Result<(), String> {
        self.transitioning = false;
        if self.foreign_seen && self.latest_ours {
            self.owned = false;
            eprintln!("clipboard_owner event=ownership_race overwritten=true");
            Err("Clipboard changed during delivery and raced with Koett's offer. Copy the newer item again.".into())
        } else if !self.owned || !self.latest_ours {
            self.owned = false;
            Err("Clipboard ownership changed; no further clipboard writes".into())
        } else {
            Ok(())
        }
    }
}
struct SourceData {
    payload: Payload,
    marker: String,
}
// Deterministically place a foreign copy in the non-atomic protocol gap, in the private lab only.
#[cfg(test)]
static BEFORE_CLAIM: Mutex<Option<Box<dyn FnOnce() + Send>>> = Mutex::new(None);

/// What a selection offer announced: Koett's marker if it has one, and its type names, which
/// say who took the clipboard (CopyQ adds `application/x-copyq-*`, Chromium `chromium/x-*`).
#[derive(Default)]
struct Offer {
    marker: Option<String>,
    mimes: Vec<String>,
}
struct State {
    manager: Option<manager::ZwlrDataControlManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    current: Option<source::ZwlrDataControlSourceV1>,
    ownership: Ownership,
    offers: HashMap<wayland_client::backend::ObjectId, Offer>,
    markers: HashSet<String>,
    sync_done: u64,
    next_sync: u64,
}
impl State {
    fn new() -> Self {
        Self {
            manager: None,
            seat: None,
            current: None,
            ownership: Ownership::default(),
            offers: HashMap::new(),
            markers: HashSet::new(),
            sync_done: 0,
            next_sync: 0,
        }
    }
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
            if interface == "zwlr_data_control_manager_v1" {
                state.manager = Some(registry.bind(name, version.min(2), qh, ()));
            }
            if interface == "wl_seat" && state.seat.is_none() {
                state.seat = Some(registry.bind(name, version.min(2), qh, ()));
            }
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
            state.sync_done = state.sync_done.max(*serial);
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
impl Dispatch<manager::ZwlrDataControlManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &manager::ZwlrDataControlManagerV1,
        _: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<device::ZwlrDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &device::ZwlrDataControlDeviceV1,
        event: device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            device::Event::DataOffer { id } => {
                state.offers.entry(id.id()).or_default();
            }
            device::Event::Selection { id } => {
                let offer = id
                    .as_ref()
                    .and_then(|offer| state.offers.remove(&offer.id()))
                    .unwrap_or_default();
                let ours = offer
                    .marker
                    .as_ref()
                    .is_some_and(|marker| state.markers.contains(marker));
                // Koett's source hears it's cancelled before this, so check the last selection,
                // or a claim still in flight.
                let ownership = &state.ownership;
                if !ours && ownership.active && (ownership.latest_ours || ownership.transitioning) {
                    // Data control doesn't say which client set it; its types usually do.
                    eprintln!(
                        "clipboard_owner event=taken_by mimes={}",
                        taker_mimes(&offer.mimes)
                    );
                }
                state.ownership.selection(ours);
                if let Some(offer) = id {
                    offer.destroy();
                }
            }
            device::Event::PrimarySelection { id: Some(offer) } => {
                state.offers.remove(&offer.id());
                offer.destroy();
            }
            device::Event::Finished => {
                state.ownership.active = true;
                state.ownership.owned = false;
            }
            _ => {}
        }
    }
    wayland_client::event_created_child!(State,device::ZwlrDataControlDeviceV1,[0=>(offer::ZwlrDataControlOfferV1,())]);
}
impl Dispatch<offer::ZwlrDataControlOfferV1, ()> for State {
    fn event(
        state: &mut Self,
        offer: &offer::ZwlrDataControlOfferV1,
        event: offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let offer::Event::Offer { mime_type } = event {
            let entry = state.offers.entry(offer.id()).or_default();
            if state.markers.contains(&mime_type) {
                entry.marker = Some(mime_type);
            } else if entry.mimes.len() < 24 {
                entry.mimes.push(mime_type);
            }
        }
    }
}

/// Type names only, never contents, kept to one short log field.
fn taker_mimes(mimes: &[String]) -> String {
    if mimes.is_empty() {
        return "none".into();
    }
    mimes
        .iter()
        .map(|mime| {
            mime.chars()
                .filter(|c| c.is_ascii_graphic() && *c != ',')
                .take(64)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Default)]
struct WriterPool(AtomicUsize);
struct Permit(Arc<WriterPool>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.0.fetch_sub(1, Ordering::AcqRel);
    }
}
impl WriterPool {
    fn acquire(self: &Arc<Self>) -> Option<Permit> {
        self.0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_WRITERS).then_some(n + 1)
            })
            .ok()
            .map(|_| Permit(self.clone()))
    }
}
static WRITERS: LazyLock<Arc<WriterPool>> = LazyLock::new(|| Arc::new(WriterPool::default()));
type WriterJob = Box<dyn FnOnce() + Send>;
fn spawn_writer(
    file: File,
    data: Arc<[u8]>,
    pool: &Arc<WriterPool>,
    completed: Option<ReadSignal>,
    spawn: impl FnOnce(WriterJob) -> io::Result<thread::JoinHandle<()>>,
) -> io::Result<thread::JoinHandle<()>> {
    let permit = pool.acquire().ok_or_else(|| {
        io::Error::new(io::ErrorKind::WouldBlock, "clipboard writer limit reached")
    })?;
    spawn(Box::new(move || {
        let _permit = permit;
        match write_transfer(file, &data, Duration::from_secs(5)) {
            Ok(()) => {
                if let Some(signal) = completed {
                    let _ = signal.done.try_send(());
                }
            }
            Err(error) => eprintln!("clipboard_owner event=transfer_failed error={error}"),
        }
    }))
}
fn write_transfer(mut file: File, data: &[u8], idle_timeout: Duration) -> io::Result<()> {
    let mut deadline = Instant::now() + idle_timeout;
    let raw = file.as_raw_fd();
    // SAFETY: both calls operate on this live File; preserve existing file flags.
    unsafe {
        let flags = libc::fcntl(raw, libc::F_GETFL);
        if flags < 0 || libc::fcntl(raw, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let mut bytes = data;
    while !bytes.is_empty() {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "clipboard reader stalled",
            ));
        }
        match file.write(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "clipboard pipe closed",
                ));
            }
            Ok(n) => {
                bytes = &bytes[n..];
                deadline = Instant::now() + idle_timeout;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let mut poll = libc::pollfd {
                    fd: raw,
                    events: libc::POLLOUT,
                    revents: 0,
                };
                // SAFETY: one initialized pollfd references the live File.
                unsafe {
                    libc::poll(&mut poll, 1, 25);
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
impl Dispatch<source::ZwlrDataControlSourceV1, Arc<SourceData>> for State {
    fn event(
        state: &mut Self,
        source: &source::ZwlrDataControlSourceV1,
        event: source::Event,
        data: &Arc<SourceData>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            source::Event::Cancelled => {
                // An old source is retained through the transition barrier. Its expected
                // cancellation cannot hide a foreign Selection notification.
                if state.current.as_ref() == Some(source) {
                    state.ownership.shared.store(false, Ordering::Release);
                    state.ownership.owned = false;
                }
                eprintln!(
                    "clipboard_owner event=source_cancelled transition={}",
                    state.ownership.transitioning
                );
            }
            source::Event::Send { mime_type, fd } => {
                if mime_type == data.marker {
                    return;
                } // Empty provenance format, not user content.
                if let Some((_, bytes)) = data.payload.0.iter().find(|(mime, _)| *mime == mime_type)
                {
                    eprintln!(
                        "clipboard_owner event=read mime={} bytes={}",
                        mime_type,
                        bytes.len()
                    );
                    // Arc keeps this source's immutable data alive even after replacement.
                    // Ignore pre-paste clipboard-manager reads. This signal only advances
                    // an already authorized delivery; reads never trigger keyboard input.
                    // A manager checking the hint isn't a paste.
                    let completed = if mime_type == SECRET_HINT {
                        None
                    } else {
                        data.payload.1.as_ref().and_then(ReadSignal::begin)
                    };
                    let result = spawn_writer(
                        File::from(fd),
                        Arc::clone(bytes),
                        &WRITERS,
                        completed,
                        |job| {
                            thread::Builder::new()
                                .name("koett-clip-write".into())
                                .spawn(job)
                        },
                    );
                    if let Err(error) = result {
                        eprintln!("clipboard_owner event=writer_rejected error={error}");
                    }
                }
            }
            _ => {}
        }
    }
}
/// Nonblocking socket pump shared by barriers and the serving loop.
fn pump(
    queue: &mut EventQueue<State>,
    state: &mut State,
    deadline: Instant,
    stop: &AtomicBool,
) -> Result<(), String> {
    if stop.load(Ordering::Acquire) {
        return Err("Clipboard owner stopped".into());
    }
    if Instant::now() >= deadline {
        return Err("Clipboard compositor deadline exceeded".into());
    }
    queue.dispatch_pending(state).map_err(|e| e.to_string())?;
    let blocked = match queue.flush() {
        Ok(()) => false,
        Err(wayland_client::backend::WaylandError::Io(e))
            if e.kind() == io::ErrorKind::WouldBlock =>
        {
            true
        }
        Err(e) => return Err(e.to_string()),
    };
    if let Some(read) = queue.prepare_read() {
        let mut poll = libc::pollfd {
            fd: read.connection_fd().as_raw_fd(),
            events: libc::POLLIN | if blocked { libc::POLLOUT } else { 0 },
            revents: 0,
        };
        let wait = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(25))
            .as_millis() as i32;
        // SAFETY: one initialized pollfd; the read guard keeps its connection alive.
        let ready = unsafe { libc::poll(&mut poll, 1, wait) };
        if ready < 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(e.to_string());
            }
        } else if ready > 0 && poll.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            read.read().map_err(|e| e.to_string())?;
        }
    }
    queue.dispatch_pending(state).map_err(|e| e.to_string())?;
    Ok(())
}
fn drain_available(
    queue: &mut EventQueue<State>,
    state: &mut State,
    deadline: Instant,
    stop: &AtomicBool,
) -> Result<(), String> {
    loop {
        if stop.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err("Clipboard transition expired".into());
        }
        queue.dispatch_pending(state).map_err(|e| e.to_string())?;
        let Some(read) = queue.prepare_read() else {
            continue;
        };
        let mut poll = libc::pollfd {
            fd: read.connection_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one initialized pollfd; the guard holds its connection alive.
        let ready = unsafe { libc::poll(&mut poll, 1, 0) };
        if ready <= 0 {
            return Ok(());
        }
        read.read().map_err(|e| e.to_string())?;
    }
}

fn barrier(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    deadline: Instant,
    stop: &AtomicBool,
) -> Result<(), String> {
    state.next_sync += 1;
    let serial = state.next_sync;
    connection.display().sync(&queue.handle(), serial);
    while state.sync_done < serial {
        pump(queue, state, deadline, stop)?;
    }
    Ok(())
}
fn serve(
    requests: mpsc::Receiver<Request>,
    stop: Arc<AtomicBool>,
    owned: Arc<AtomicBool>,
) -> Result<(), String> {
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let connection = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let mut queue: EventQueue<State> = connection.new_event_queue();
    let qh = queue.handle();
    let mut state = State::new();
    state.ownership.shared = owned;
    let _registry = connection.display().get_registry(&qh, ());
    barrier(&connection, &mut queue, &mut state, deadline, &stop)?;
    let manager = state
        .manager
        .clone()
        .ok_or("Compositor lacks data-control clipboard support")?;
    let device = manager.get_data_device(state.seat.as_ref().ok_or("No Wayland seat")?, &qh, ());
    loop {
        pump(
            &mut queue,
            &mut state,
            Instant::now() + Duration::from_millis(100),
            &stop,
        )?;
        if state.ownership.active && !state.ownership.owned {
            break;
        }
        if let Ok(request) = requests.try_recv() {
            barrier(&connection, &mut queue, &mut state, request.deadline, &stop)?;
            if state.ownership.active && !state.ownership.owned {
                let _ = request
                    .reply
                    .send(Err("Newer clipboard copy preserved".into()));
                break;
            }
            if Instant::now() >= request.deadline || stop.load(Ordering::Acquire) {
                let _ = request.reply.send(Err("Clipboard request expired".into()));
                break;
            }
            static NEXT: AtomicUsize = AtomicUsize::new(1);
            let marker = format!(
                "application/x-koett-offer-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            state.markers.insert(marker.clone());
            let data = Arc::new(SourceData {
                payload: request.payload,
                marker,
            });
            let source = manager.create_data_source(&qh, data.clone());
            for (mime, _) in &data.payload.0 {
                source.offer((*mime).into());
            }
            source.offer(data.marker.clone());
            // No allocation or blocking work between the last event drain and claim.
            drain_available(&mut queue, &mut state, request.deadline, &stop)?;
            if state.ownership.active && !state.ownership.owned {
                let _ = request
                    .reply
                    .send(Err("Newer clipboard copy preserved".into()));
                break;
            }
            #[cfg(test)]
            if let Some(hook) = BEFORE_CLAIM.lock().unwrap().take() {
                hook();
            }
            state.ownership.begin();
            let old = state.current.replace(source.clone());
            device.set_selection(Some(&source));
            barrier(&connection, &mut queue, &mut state, request.deadline, &stop)?;
            let result = state.ownership.finish();
            state
                .ownership
                .shared
                .store(result.is_ok(), Ordering::Release);
            if let Some(old) = old {
                old.destroy();
            }
            let failed = result.is_err();
            let _ = request.reply.send(result);
            if failed {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    const TEXT: &str = "- #1 <literal> & “Unicode”\nsecond line";
    #[test]
    fn a_text_read_counts_only_after_the_key_and_is_bounded() {
        let (payload, read) = Payload::tracked_text("hello");
        // A clipboard manager reading the new selection before the key isn't a paste.
        assert!(payload.1.as_ref().unwrap().begin().is_none());
        let start = Instant::now();
        assert!(!read.wait_started(start, Duration::from_millis(30)));
        assert!(start.elapsed() < Duration::from_millis(500));
        read.arm();
        payload.1.as_ref().unwrap().begin().unwrap();
        assert!(read.wait_started(Instant::now(), Duration::ZERO));
    }

    #[test]
    fn image_completion_ignores_pre_paste_reads_and_other_offers() {
        let (payload, read) = Payload::tracked_image(&[1, 2, 3]);
        assert!(payload.1.as_ref().unwrap().begin().is_none());
        read.arm();
        let signal = payload.1.as_ref().unwrap().begin().unwrap();
        let (_, other) = Payload::tracked_image(&[4]);
        signal.done.try_send(()).unwrap();
        assert!(read.wait(Instant::now(), Duration::ZERO, Duration::ZERO));
        assert!(!other.wait(Instant::now(), Duration::ZERO, Duration::ZERO));
    }
    #[test]
    fn image_completion_waits_for_the_whole_large_transfer_and_rejects_a_broken_pipe() {
        use std::{io::Read, os::unix::net::UnixStream};
        for broken in [false, true] {
            let (payload, read) = Payload::tracked_image(&vec![42; 2 * 1024 * 1024]);
            read.arm();
            let signal = payload.1.as_ref().unwrap().begin();
            let (writer, mut reader) = UnixStream::pair().unwrap();
            let worker = spawn_writer(
                File::from(std::os::fd::OwnedFd::from(writer)),
                payload.0[0].1.clone(),
                &Arc::new(WriterPool::default()),
                signal,
                |job| thread::Builder::new().spawn(job),
            )
            .unwrap();
            assert!(!read.wait(
                Instant::now(),
                Duration::from_millis(1),
                Duration::from_millis(20)
            ));
            if !broken {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).unwrap();
                assert_eq!(bytes.as_slice(), payload.0[0].1.as_ref());
            }
            drop(reader);
            worker.join().unwrap();
            assert_eq!(
                read.wait(Instant::now(), Duration::ZERO, Duration::ZERO),
                !broken
            );
        }
    }
    #[test]
    fn only_the_drawing_offer_asks_clipboard_managers_to_skip_it() {
        let (drawing, _) = Payload::tracked_image(&[1, 2, 3]);
        let hint = |payload: &Payload| {
            payload
                .0
                .iter()
                .find(|(mime, _)| *mime == SECRET_HINT)
                .map(|(_, bytes)| bytes.to_vec())
        };
        assert_eq!(hint(&drawing), Some(b"secret".to_vec()));
        // Text and the kept drawing with its text stay in clipboard history, as before.
        assert_eq!(hint(&Payload::tracked_text("hello").0), None);
        assert_eq!(hint(&Payload::bundle(&[1], "hello")), None);
    }

    #[test]
    fn a_taker_is_logged_by_its_type_names_only() {
        assert_eq!(taker_mimes(&[]), "none");
        let mimes = [
            "application/x-copyq-owner".to_string(),
            "text/plain;charset=utf-8".into(),
            "bad name,\nnext".into(),
            "x".repeat(100),
        ];
        let logged = taker_mimes(&mimes);
        assert!(
            logged.starts_with("application/x-copyq-owner,text/plain;charset=utf-8,badnamenext,")
        );
        assert_eq!(logged.split(',').count(), 4);
        assert_eq!(logged.rsplit(',').next().unwrap().len(), 64);
    }

    #[test]
    fn bundle_keeps_each_representation_and_escapes_only_html() {
        let payload = Payload::bundle(&[137, 80, 78, 71], TEXT);
        assert_eq!(payload.0.len(), 4);
        assert_eq!(payload.0[0].1.as_ref(), [137, 80, 78, 71]);
        let html = String::from_utf8(payload.0[1].1.to_vec()).unwrap();
        assert!(html.contains("data:image/png;base64,iVBORw=="));
        assert!(html.contains("&lt;literal&gt; &amp; “Unicode”\nsecond line"));
        for (_, bytes) in &payload.0[2..] {
            assert_eq!(bytes.as_ref(), TEXT.as_bytes());
        }
    }
    #[test]
    fn self_cancellation_cannot_hide_a_foreign_copy_during_transition() {
        let mut owner = Ownership::default();
        owner.begin();
        owner.selection(true);
        assert!(owner.finish().is_ok());
        owner.begin();
        owner.selection(false);
        owner.selection(true);
        let error = owner.finish().unwrap_err();
        assert!(error.contains("raced"));
        assert!(!owner.owned);
    }
    #[test]
    fn normal_self_transition_succeeds_but_foreign_selection_is_terminal() {
        let mut owner = Ownership::default();
        owner.begin();
        owner.selection(true);
        assert!(owner.finish().is_ok());
        owner.begin();
        owner.selection(true);
        assert!(owner.finish().is_ok());
        owner.selection(false);
        assert!(!owner.owned);
        owner.selection(true);
        assert!(!owner.owned, "no resurrection after foreign copy");
    }
    #[test]
    fn payload_clones_share_all_bytes() {
        let payload = Payload::bundle(&[1, 2, 3], TEXT);
        let clone = payload.clone();
        for (a, b) in payload.0.iter().zip(&clone.0) {
            assert!(Arc::ptr_eq(&a.1, &b.1));
        }
        assert!(Arc::ptr_eq(&payload.0[2].1, &payload.0[3].1));
    }
    #[test]
    fn writer_limit_and_spawn_failure_release_permits() {
        use std::os::unix::net::UnixStream;
        let pool = Arc::new(WriterPool::default());
        let permits: Vec<_> = (0..MAX_WRITERS).map(|_| pool.acquire().unwrap()).collect();
        assert!(pool.acquire().is_none());
        let (writer, _reader) = UnixStream::pair().unwrap();
        assert!(
            spawn_writer(
                File::from(std::os::fd::OwnedFd::from(writer)),
                Arc::from(&b"x"[..]),
                &pool,
                None,
                |_| panic!("spawn must not run at capacity")
            )
            .is_err()
        );
        drop(permits);
        assert_eq!(pool.0.load(Ordering::Acquire), 0);
        let (writer, _reader) = UnixStream::pair().unwrap();
        assert!(
            spawn_writer(
                File::from(std::os::fd::OwnedFd::from(writer)),
                Arc::from(&b"x"[..]),
                &pool,
                None,
                |_| Err(io::Error::other("injected spawn failure"))
            )
            .is_err()
        );
        assert_eq!(pool.0.load(Ordering::Acquire), 0);
    }
    #[test]
    fn active_large_transfer_finishes_after_ownership_is_lost() {
        use std::{io::Read, os::unix::net::UnixStream};
        let (writer, mut reader) = UnixStream::pair().unwrap();
        reader
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let data: Arc<[u8]> = Arc::from(vec![42; 2 * 1024 * 1024]);
        let pool = Arc::new(WriterPool::default());
        let worker = spawn_writer(
            File::from(std::os::fd::OwnedFd::from(writer)),
            data.clone(),
            &pool,
            None,
            |job| thread::Builder::new().spawn(job),
        )
        .unwrap();
        // The pipe fills before the reader starts, then the selection is replaced.
        thread::sleep(Duration::from_millis(20));
        let mut owner = Ownership::default();
        owner.begin();
        owner.selection(false);
        assert!(!owner.owned);
        let mut received = Vec::new();
        reader.read_to_end(&mut received).unwrap();
        worker.join().unwrap();
        assert_eq!(received.as_slice(), data.as_ref());
        assert_eq!(pool.0.load(Ordering::Acquire), 0);
    }
    #[test]
    fn transfer_deadline_tracks_inactivity_instead_of_total_duration() {
        use std::{io::Read, os::unix::net::UnixStream};
        let (writer, mut reader) = UnixStream::pair().unwrap();
        reader
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let size: libc::c_int = 4096;
        // SAFETY: writer is a live socket and size points to one initialized c_int.
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    writer.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_SNDBUF,
                    (&size as *const libc::c_int).cast(),
                    std::mem::size_of_val(&size) as libc::socklen_t,
                )
            },
            0
        );
        let data = vec![31; 64 * 1024];
        let expected = data.clone();
        let start = Instant::now();
        let worker = thread::spawn(move || {
            write_transfer(
                File::from(std::os::fd::OwnedFd::from(writer)),
                &data,
                Duration::from_millis(300),
            )
        });
        let mut received = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let count = reader.read(&mut chunk).unwrap();
            if count == 0 {
                break;
            }
            received.extend_from_slice(&chunk[..count]);
            thread::sleep(Duration::from_millis(30));
        }
        worker.join().unwrap().unwrap();
        assert_eq!(received, expected);
        assert!(start.elapsed() > Duration::from_millis(300));
    }
    #[test]
    fn compositor_sync_has_a_deadline_and_is_interruptible() {
        use std::os::unix::net::UnixStream;
        let (client, _silent_server) = UnixStream::pair().unwrap();
        let connection = Connection::from_socket(client).unwrap();
        let mut queue = connection.new_event_queue();
        let mut state = State::new();
        let stop = AtomicBool::new(false);
        let start = Instant::now();
        let error = barrier(
            &connection,
            &mut queue,
            &mut state,
            start + Duration::from_millis(60),
            &stop,
        )
        .unwrap_err();
        assert!(error.contains("deadline"));
        assert!(start.elapsed() < Duration::from_millis(500));
        stop.store(true, Ordering::Release);
        let start = Instant::now();
        assert!(
            barrier(
                &connection,
                &mut queue,
                &mut state,
                start + REQUEST_TIMEOUT,
                &stop
            )
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_millis(100));
    }
    #[test]
    fn owner_drop_never_joins_a_stuck_worker_forever() {
        let (tx, _rx) = mpsc::channel();
        let worker = thread::spawn(|| thread::sleep(Duration::from_secs(1)));
        let owner = Owner {
            requests: tx,
            stop: Arc::new(AtomicBool::new(false)),
            owned: Arc::new(AtomicBool::new(true)),
            worker: Some(worker),
        };
        let start = Instant::now();
        drop(owner);
        assert!(start.elapsed() < Duration::from_millis(600));
    }
    fn foreign_copy() {
        let mut copy = Command::new("wl-copy")
            .args(["--type", "text/plain"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        copy.stdin
            .take()
            .unwrap()
            .write_all(b"newer copy wins")
            .unwrap();
        assert!(copy.wait().unwrap().success());
    }
    fn read(mime: &str) -> Vec<u8> {
        let result = Command::new("wl-paste")
            .args(["--no-newline", "--type", mime])
            .output()
            .unwrap();
        assert!(result.status.success(), "read failed: {mime}");
        result.stdout
    }
    #[test]
    #[ignore = "requires an owned agent-lab nested niri; never run on the host clipboard"]
    fn niri_owner_keeps_bundle_yields_to_new_copies_and_stops() {
        assert!(
            std::env::var("AGENT_LAB_RUNTIME")
                .unwrap()
                .contains("/agent-lab/")
        );
        assert_eq!(
            std::env::var("WAYLAND_DISPLAY").unwrap(),
            std::env::var("KOETT_TEST_NIRI_DISPLAY").unwrap()
        );
        assert!(std::env::var_os("NIRI_SOCKET").is_some());
        let fixture = std::fs::read(std::env::var("KOETT_TEST_PNG").unwrap()).unwrap();
        assert!(
            super::super::output::deliver_image_read(&fixture, None)
                .0
                .is_err()
        );
        for _ in 0..3 {
            assert_eq!(read("image/png"), fixture);
        }
        assert!(
            super::super::output::deliver(TEXT, None, crate::settings::PasteShortcut::CtrlV)
                .is_err()
        );
        assert_eq!(read("text/plain"), TEXT.as_bytes());
        replace(Payload::bundle(&fixture, TEXT)).unwrap();
        for _ in 0..3 {
            assert_eq!(read("image/png"), fixture);
            assert_eq!(read("text/plain"), TEXT.as_bytes());
            assert_eq!(
                read("text/html"),
                super::super::feedback::sketch_html(TEXT, &fixture).as_bytes()
            );
        }
        assert!(
            super::super::recovery::bundle_on_clipboard(TEXT),
            "Koett's own bundle reads back as image and text"
        );
        foreign_copy();
        assert!(replace(Payload::bundle(&fixture, TEXT)).is_err());
        assert_eq!(read("text/plain"), b"newer copy wins");
        assert!(clipboard_holds("text/plain", b"newer copy wins"));
        assert!(!clipboard_holds("text/plain", TEXT.as_bytes()));
        assert!(
            !owned(),
            "a foreign copy clears ownership before any paste key"
        );
        offer(Payload::text(TEXT)).unwrap();
        assert!(owned(), "offering again takes the clipboard back");
        assert_eq!(read("text/plain"), TEXT.as_bytes());
        offer(Payload::bundle(&fixture, TEXT)).unwrap();
        assert!(owned(), "a live owner switches in place");
        assert_eq!(read("image/png"), fixture);
        foreign_copy();
        assert_eq!(read("text/plain"), b"newer copy wins");
        assert!(
            !super::super::recovery::bundle_on_clipboard(TEXT),
            "a newer copy means the drawing is not on the clipboard"
        );
        stop();
        assert_eq!(read("text/plain"), b"newer copy wins");
        start(Payload::bundle(&fixture, TEXT)).unwrap();
        assert_eq!(read("image/png"), fixture);
        stop();
        assert!(OWNER.lock().unwrap().is_none());
        let listing = Command::new("wl-paste")
            .arg("--list-types")
            .output()
            .unwrap();
        assert!(!String::from_utf8_lossy(&listing.stdout).contains("image/png"));
        // Hold a real wl-paste reader behind its stdout pipe, then replace the selection.
        let large = vec![17u8; 2 * 1024 * 1024];
        start(Payload::image(&large)).unwrap();
        let reading = Command::new("wl-paste")
            .args(["--no-newline", "--type", "image/png"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        while WRITERS.0.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(WRITERS.0.load(Ordering::Acquire) > 0);
        foreign_copy();
        let received = reading.wait_with_output().unwrap();
        assert!(received.status.success());
        assert_eq!(received.stdout, large);
        assert_eq!(read("text/plain"), b"newer copy wins");
        stop();

        // Force a foreign claim after the final preflight drain and before our set_selection.
        start(Payload::text(TEXT)).unwrap();
        *BEFORE_CLAIM.lock().unwrap() = Some(Box::new(foreign_copy));
        let error = replace(Payload::bundle(&fixture, TEXT)).unwrap_err();
        assert!(error.contains("raced"), "race was not detected: {error}");
        stop();
        start(Payload::text(TEXT)).unwrap();
        assert!(
            !super::super::recovery::bundle_on_clipboard(TEXT),
            "the same text without the image is not the drawing"
        );
        stop();
    }
}
