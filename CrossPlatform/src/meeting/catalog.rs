//! Meetings as the desktop lists them, newest first, read from their folders.
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::bundle::read_committed;
use super::bundle::scan::{DiskState, scan_all};
use super::{
    Artifact, MeetingId, MeetingView, Offset, RecoveryReason, SAMPLE_RATE, SessionInfo,
    absolute_mark_links,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listed {
    pub(crate) info: SessionInfo,
    pub(crate) duration: Option<Offset>,
    pub(crate) folder: PathBuf,
    pub(crate) state: Listing,
    /// `recover` would run: never transcribed, or transcribed by the fallback with audio kept.
    pub(crate) recoverable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Listing {
    Transcribed(Artifact),
    NotTranscribed(RecoveryReason),
}

/// Live and unreadable folders are left out: the tray owns the live one, the CLI reports the rest.
pub(crate) fn list(root: &Path) -> io::Result<Vec<Listed>> {
    Ok(scan_all(root)?
        .into_iter()
        .filter_map(|state| listed(root, state))
        .collect())
}

pub(crate) fn recoverable(root: &Path) -> io::Result<Vec<Listed>> {
    Ok(list(root)?
        .into_iter()
        .filter(|meeting| meeting.recoverable)
        .collect())
}

/// What Copy puts on the clipboard: the committed transcript with each mark its
/// receipt lists at an absolute file. `None` once the folder is gone.
pub(crate) fn copy_text(root: &Path, id: MeetingId) -> io::Result<Option<String>> {
    let folder = root.join(id.to_string());
    let (markdown, receipt) = match read_committed(&folder) {
        Ok(committed) => committed,
        Err(error) if error.kind() == io::ErrorKind::NotFound && !folder.try_exists()? => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    absolute_mark_links(&markdown, &folder, &receipt.publication.assets)
        .map(Some)
        .map_err(io::Error::other)
}

fn listed(root: &Path, state: DiskState) -> Option<Listed> {
    let recoverable = state.is_recoverable();
    let sealed = state.duration();
    let folder = root.join(state.id().to_string());
    let (info, state) = match state.view(root) {
        MeetingView::Complete(info, artifact, _) => (info, Listing::Transcribed(artifact)),
        MeetingView::Recoverable(info, reason) => (info, Listing::NotTranscribed(reason)),
        _ => return None,
    };
    let duration = sealed.or_else(|| last_written(&folder, info.started_at_ms));
    Some(Listed {
        info,
        duration,
        folder,
        state,
        recoverable,
    })
}

/// A crash leaves no seal; the journals were written up to the moment it happened.
fn last_written(folder: &Path, started_at_ms: u64) -> Option<Offset> {
    let written = ["you.journal", "other.journal"]
        .iter()
        .filter_map(|name| {
            let metadata = folder.join("audio").join(name).metadata().ok()?;
            metadata.modified().ok()
        })
        .max()?;
    let written_ms = u64::try_from(written.duration_since(UNIX_EPOCH).ok()?.as_millis()).ok()?;
    let span_ms = written_ms.checked_sub(started_at_ms)?;
    Some(Offset(span_ms * u64::from(SAMPLE_RATE) / 1000))
}

#[cfg(test)]
mod tests;
