mod state;

use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use super::Offset;
use super::files::{private_directory, sync_directory, write_temp};
use super::records::{MarkIssue, Seal};
use state::{Input, Lifecycle, Operation, State};

const DRAIN_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MarkRef {
    pub(crate) id: u64,
    pub(crate) offset: Offset,
    pub(crate) relative_path: String,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

#[derive(Clone)]
pub(crate) struct MarkSink(Arc<Shared>);

impl MarkSink {
    pub(crate) fn begin(&self, id: u64, offset: Offset) -> bool {
        self.input(id, Input::Begin(offset))
    }

    pub(crate) fn submit(&self, id: u64, offset: Offset, png: Vec<u8>) -> bool {
        self.input(id, Input::Submit(offset, png))
    }

    pub(crate) fn undo(&self, id: u64) -> bool {
        self.input(id, Input::Undo)
    }

    pub(crate) fn miss(&self, id: u64, issue: String) -> bool {
        self.input(id, Input::Miss(issue))
    }

    fn input(&self, id: u64, input: Input) -> bool {
        let accepted = self.0.lock().input(id, input);
        self.0.changed.notify_all();
        accepted
    }
}

pub(crate) struct MarkWriter {
    shared: Arc<Shared>,
}

impl MarkWriter {
    pub(super) fn start(directory: &Path) -> io::Result<Self> {
        let assets = directory.join("assets");
        private_directory(&assets)?;
        sync_directory(directory)?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        });
        let worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("meeting-marks".into())
            .spawn(move || run(&assets, &worker))?;
        Ok(Self { shared })
    }

    pub(crate) fn sink(&self) -> MarkSink {
        MarkSink(Arc::clone(&self.shared))
    }

    pub(crate) fn close(self, cutoff: Offset) -> Vec<MarkIssue> {
        self.close_for(cutoff, DRAIN_TIMEOUT)
    }

    pub(super) fn close_for(self, cutoff: Offset, timeout: Duration) -> Vec<MarkIssue> {
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.lock();
        state.drain(cutoff);
        self.shared.changed.notify_all();
        while state.pending() && Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (next, _) = self
                .shared
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|error| error.into_inner());
            state = next;
        }

        let issues = state.close();
        self.shared.changed.notify_all();
        issues
    }
}

impl Drop for MarkWriter {
    fn drop(&mut self) {
        self.shared.lock().stop();
        self.shared.changed.notify_all();
    }
}

fn run(assets: &Path, shared: &Shared) {
    let mut state = shared.lock();
    loop {
        let Some((id, offset, operation)) = state.take_operation() else {
            if matches!(state.lifecycle, Lifecycle::Closed) {
                return;
            }
            state = shared
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
            continue;
        };
        drop(state);
        let path = assets.join(filename(offset, id));
        let result = match &operation {
            Operation::Write(png) => write_mark(assets, &path, id, png, || {
                shared.lock().can_commit(id, offset)
            }),
            Operation::Remove => remove_mark(assets, &path),
        };
        state = shared.lock();
        if matches!(state.lifecycle, Lifecycle::Closed) && matches!(operation, Operation::Write(_))
        {
            drop(state);
            let _ = remove_mark(assets, &path);
            state = shared.lock();
        }
        state.finish(id, result.map_err(|error| error.to_string()));
        shared.changed.notify_all();
    }
}

fn write_mark(
    assets: &Path,
    path: &Path,
    id: u64,
    png: &[u8],
    can_commit: impl FnOnce() -> bool,
) -> io::Result<()> {
    let temporary = assets.join(format!(".tmp-{id}"));
    write_temp(&temporary, png)?;
    let result = if can_commit() {
        fs::rename(&temporary, path).and_then(|()| sync_directory(assets))
    } else {
        fs::remove_file(&temporary)
    };
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn remove_mark(assets: &Path, path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => sync_directory(assets),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn filename(offset: Offset, id: u64) -> String {
    format!("mark-{:010}-{id:04}.png", offset.0)
}

pub(crate) fn list_marks(directory: &Path, seal: Option<&Seal>) -> io::Result<Vec<MarkRef>> {
    let assets = match fs::read_dir(directory.join("assets")) {
        Ok(assets) => assets,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut marks = Vec::new();
    for entry in assets {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some((offset, id)) = name.to_str().and_then(parse_name) else {
            continue;
        };
        // A stalled rename or undo can outlive the drain deadline.
        if seal.is_some_and(|seal| {
            offset > seal.cutoff || seal.mark_issues.iter().any(|issue| issue.id == id)
        }) {
            continue;
        }
        marks.push(MarkRef {
            id,
            offset,
            relative_path: format!("assets/{}", filename(offset, id)),
        });
    }
    marks.sort_unstable_by_key(|mark| (mark.offset, mark.id));
    Ok(marks)
}

fn parse_name(name: &str) -> Option<(Offset, u64)> {
    let (offset, id) = name
        .strip_prefix("mark-")?
        .strip_suffix(".png")?
        .split_once('-')?;
    let offset = Offset(offset.parse().ok()?);
    let id = id.parse().ok()?;
    (filename(offset, id) == name).then_some((offset, id))
}
