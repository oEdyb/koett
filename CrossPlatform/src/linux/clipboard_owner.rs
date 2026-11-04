//! A regular-clipboard owner for the niri/wtype path. No keyboard input here.
use std::{
    fs::File,
    io::Write,
    os::fd::AsFd,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle,
    protocol::{wl_registry, wl_seat},
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1 as device, zwlr_data_control_manager_v1 as manager,
    zwlr_data_control_offer_v1 as offer, zwlr_data_control_source_v1 as source,
};

#[derive(Clone)]
pub(super) struct Payload(pub Vec<(&'static str, Vec<u8>)>);
impl Payload {
    pub fn image(png: &[u8]) -> Self {
        Self(vec![("image/png", png.to_vec())])
    }
    pub fn text(text: &str) -> Self {
        Self(vec![
            ("text/plain;charset=utf-8", text.as_bytes().to_vec()),
            ("text/plain", text.as_bytes().to_vec()),
        ])
    }
    pub fn bundle(png: &[u8], text: &str) -> Self {
        let mut data = Self::image(png).0;
        data.push((
            "text/html",
            super::feedback::sketch_html(text, png).into_bytes(),
        ));
        data.extend(Self::text(text).0);
        Self(data)
    }
}
struct Request {
    payload: Payload,
    deadline: Instant,
    reply: mpsc::SyncSender<Result<(), String>>,
}
struct Owner {
    requests: mpsc::Sender<Request>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
static OWNER: Mutex<Option<Owner>> = Mutex::new(None);
pub(super) fn stop() {
    OWNER.lock().unwrap_or_else(|e| e.into_inner()).take();
}
/// Starts a new, user-authorized copy; guarded transitions must use `replace`.
pub(super) fn start(payload: Payload) -> Result<(), String> {
    let mut owner = OWNER.lock().map_err(|e| e.to_string())?;
    owner.take();
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let worker = thread::spawn(move || {
        if let Err(error) = serve(rx, flag) {
            eprintln!("clipboard_owner event=stopped error={error}");
        }
    });
    let next = Owner {
        requests: tx,
        stop,
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
        owner
            .as_ref()
            .ok_or("Sketch clipboard ownership ended; newer copies are preserved")?,
        payload,
    )
}
fn request(owner: &Owner, payload: Payload) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel(1);
    owner
        .requests
        .send(Request {
            payload,
            deadline: Instant::now() + Duration::from_secs(2),
            reply: tx,
        })
        .map_err(|_| "Sketch clipboard was replaced")?;
    rx.recv_timeout(Duration::from_secs(3))
        .map_err(|_| "Sketch clipboard owner unavailable")?
}
struct State {
    manager: Option<manager::ZwlrDataControlManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    current: Option<source::ZwlrDataControlSourceV1>,
    payload: Payload,
    owned: Arc<AtomicBool>,
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
            device::Event::DataOffer { id } => id.destroy(),
            device::Event::Finished => {
                state.owned.store(false, Ordering::Release);
            }
            _ => {}
        }
    }
    wayland_client::event_created_child!(State, device::ZwlrDataControlDeviceV1, [0 => (offer::ZwlrDataControlOfferV1, ())]);
}
impl Dispatch<offer::ZwlrDataControlOfferV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &offer::ZwlrDataControlOfferV1,
        _: offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<source::ZwlrDataControlSourceV1, ()> for State {
    fn event(
        state: &mut Self,
        source: &source::ZwlrDataControlSourceV1,
        event: source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if state.current.as_ref() != Some(source) {
            return;
        }
        match event {
            source::Event::Cancelled => {
                state.owned.store(false, Ordering::Release);
                eprintln!("clipboard_owner event=replaced");
            }
            source::Event::Send { mime_type, fd } => {
                if let Some((_, data)) = state.payload.0.iter().find(|(mime, _)| *mime == mime_type)
                {
                    let data = data.clone();
                    let alive = state.owned.clone();
                    thread::spawn(move || {
                        use std::os::fd::AsRawFd;
                        let mut file = File::from(fd);
                        let raw = file.as_raw_fd();
                        unsafe {
                            libc::fcntl(raw, libc::F_SETFL, libc::O_NONBLOCK);
                        }
                        let deadline = Instant::now() + Duration::from_secs(5);
                        let mut bytes = data.as_slice();
                        while !bytes.is_empty()
                            && alive.load(Ordering::Acquire)
                            && Instant::now() < deadline
                        {
                            match file.write(bytes) {
                                Ok(0) => break,
                                Ok(n) => bytes = &bytes[n..],
                                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    let mut poll = libc::pollfd {
                                        fd: raw,
                                        events: libc::POLLOUT,
                                        revents: 0,
                                    };
                                    unsafe {
                                        libc::poll(&mut poll, 1, 50);
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }
            }
            _ => {}
        }
    }
}
fn serve(requests: mpsc::Receiver<Request>, stop: Arc<AtomicBool>) -> Result<(), String> {
    let connection = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let mut queue: EventQueue<State> = connection.new_event_queue();
    let qh = queue.handle();
    let _registry = connection.display().get_registry(&qh, ());
    let owned = Arc::new(AtomicBool::new(true));
    let mut state = State {
        manager: None,
        seat: None,
        current: None,
        payload: Payload(vec![]),
        owned: owned.clone(),
    };
    queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
    let manager = state
        .manager
        .clone()
        .ok_or("Compositor lacks data-control clipboard support")?;
    let device = manager.get_data_device(state.seat.as_ref().ok_or("No Wayland seat")?, &qh, ());
    while !stop.load(Ordering::Acquire) && owned.load(Ordering::Acquire) {
        queue
            .dispatch_pending(&mut state)
            .map_err(|e| e.to_string())?;
        if let Ok(request) = requests.try_recv() {
            // Process cancellation before changing our offer. Never resurrect a lost selection.
            queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
            if !owned.load(Ordering::Acquire) {
                let _ = request
                    .reply
                    .send(Err("Newer clipboard copy preserved".into()));
                break;
            }
            if Instant::now() > request.deadline {
                let _ = request.reply.send(Err("Clipboard request expired".into()));
                continue;
            }
            let source = manager.create_data_source(&qh, ());
            for (mime, _) in &request.payload.0 {
                source.offer((*mime).into());
            }
            let old = state.current.replace(source.clone());
            state.payload = request.payload;
            device.set_selection(Some(&source));
            if let Some(old) = old {
                old.destroy();
            }
            queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
            let _ = request.reply.send(if owned.load(Ordering::Acquire) {
                Ok(())
            } else {
                Err("Newer clipboard copy preserved".into())
            });
        }
        use std::os::fd::AsRawFd;
        if let Some(read) = queue.prepare_read() {
            let mut poll = libc::pollfd {
                fd: connection.as_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            if unsafe { libc::poll(&mut poll, 1, 25) } > 0 {
                read.read().map_err(|e| e.to_string())?;
            }
        }
    }
    owned.store(false, Ordering::Release);
    Ok(())
}
