//! Meeting rows in History. A meeting opens and copies from its own folder. It is
//! never pasted, never becomes Last Transcript and never enters dictation recovery.
use crate::history::{HistoryCursor, HistorySummary};
use crate::meeting::catalog::{Listed, Listing};
use crate::meeting::{Artifact, MeetingId, MeetingView, RecoveryReason};
use std::collections::HashMap;

const PREFIX: &str = "meeting:";

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Row {
    Dictation(HistorySummary),
    Meeting(MeetingRow),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MeetingRow {
    pub(super) id: MeetingId,
    pub(super) started_at_ms: i64,
    /// History's text row, once transcribed and indexed.
    pub(super) summary: Option<HistorySummary>,
    /// The words of the first speech turn in History's text.
    pub(super) opening: Option<String>,
    /// The folder as it is now; `None` once it is gone.
    pub(super) listed: Option<Listed>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Copy,
    Export,
    FixWord,
    OpenTranscript,
    OpenFolder,
    Transcribe,
}

/// The second line of a meeting row.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Excerpt {
    Speech(String),
    Status(&'static str),
}

impl Row {
    pub(super) fn id(&self) -> String {
        match self {
            Self::Dictation(summary) => summary.id.clone(),
            Self::Meeting(meeting) => format!("{PREFIX}{}", meeting.id),
        }
    }

    pub(super) fn created_at_ms(&self) -> i64 {
        match self {
            Self::Dictation(summary) => summary.created_at_ms,
            Self::Meeting(meeting) => meeting.started_at_ms,
        }
    }

    pub(super) fn actions(&self) -> Vec<Action> {
        match self {
            Self::Dictation(_) => vec![Action::Copy, Action::Export, Action::FixWord],
            Self::Meeting(meeting) => meeting.actions(),
        }
    }
}

impl MeetingRow {
    pub(super) fn transcript(&self) -> Option<&Artifact> {
        match &self.listed.as_ref()?.state {
            Listing::Transcribed(artifact) => Some(artifact),
            Listing::NotTranscribed(_) => None,
        }
    }

    fn actions(&self) -> Vec<Action> {
        let transcribed = self.transcript().is_some();
        [
            (Action::Copy, transcribed || self.summary.is_some()),
            (Action::OpenTranscript, transcribed),
            (Action::OpenFolder, self.listed.is_some()),
            (
                Action::Transcribe,
                self.listed
                    .as_ref()
                    .is_some_and(|listed| listed.recoverable),
            ),
        ]
        .into_iter()
        .filter_map(|(action, offered)| offered.then_some(action))
        .collect()
    }

    pub(super) fn excerpt(&self) -> Option<Excerpt> {
        if self.summary.is_some() {
            return Some(
                self.opening
                    .clone()
                    .map_or(Excerpt::Status("No speech"), Excerpt::Speech),
            );
        }
        match &self.listed.as_ref()?.state {
            Listing::NotTranscribed(reason) => Some(Excerpt::Status(status(reason))),
            Listing::Transcribed(_) => None,
        }
    }

    /// The Transcribe button: its label, and whether a click can start now.
    pub(super) fn transcribe(&self, view: &MeetingView) -> (&'static str, bool) {
        match view {
            MeetingView::Processing { info, .. } if info.id == self.id => ("Transcribing…", false),
            _ if self.transcript().is_some() => {
                ("Transcribe again", matches!(view, MeetingView::Idle))
            }
            _ => ("Transcribe", matches!(view, MeetingView::Idle)),
        }
    }
}

fn status(reason: &RecoveryReason) -> &'static str {
    match reason {
        RecoveryReason::HeldByUser | RecoveryReason::FallbackUsed => "Not transcribed",
        RecoveryReason::Interrupted => "Interrupted",
        RecoveryReason::CaptureFailed(_) => "Recording failed",
        RecoveryReason::DecodeFailed(_) => "Transcription failed",
        RecoveryReason::PublishFailed(_) => "Saving failed",
    }
}

/// History's page, each meeting with its folder attached, plus the meetings
/// History has no text for whose start falls inside this page. A search only
/// shows what History matched. `text` reads a row's whole text from History.
pub(super) fn page_rows(
    page: Vec<HistorySummary>,
    meetings: Vec<Listed>,
    query: &str,
    before: Option<&HistoryCursor>,
    next: Option<&HistoryCursor>,
    text: impl Fn(&str) -> Option<String>,
) -> Vec<Row> {
    let mut folders: HashMap<MeetingId, Listed> = meetings
        .into_iter()
        .map(|meeting| (meeting.info.id, meeting))
        .collect();
    let mut rows: Vec<Row> = page
        .into_iter()
        .map(
            |summary| match summary.id.strip_prefix(PREFIX).and_then(MeetingId::parse) {
                Some(id) => Row::Meeting(MeetingRow {
                    id,
                    started_at_ms: summary.created_at_ms,
                    listed: folders.remove(&id),
                    opening: whole_lines(&summary.preview)
                        .and_then(first_turn)
                        .or_else(|| text(&summary.id).as_deref().and_then(first_turn)),
                    summary: Some(summary),
                }),
                None => Row::Dictation(summary),
            },
        )
        .collect();
    if query.trim().is_empty() {
        rows.extend(folders.into_values().filter_map(|listed| {
            let row = Row::Meeting(MeetingRow {
                id: listed.info.id,
                started_at_ms: i64::try_from(listed.info.started_at_ms).ok()?,
                summary: None,
                opening: None,
                listed: Some(listed),
            });
            let key = (row.created_at_ms(), row.id());
            let below = |cursor: &HistoryCursor| key < (cursor.created_at_ms, cursor.id.clone());
            (before.is_none_or(below) && !next.is_some_and(below) && text(&row.id()).is_none())
                .then_some(row)
        }));
        rows.sort_by_cached_key(|row| std::cmp::Reverse((row.created_at_ms(), row.id())));
    }
    rows
}

/// History's preview stops after 240 characters; a line it cut is not whole.
fn whole_lines(preview: &str) -> Option<&str> {
    preview.rfind('\n').map(|end| &preview[..end])
}

fn first_turn(text: &str) -> Option<String> {
    text.lines().find_map(|line| match parse(line) {
        Reading::Turn { text, .. } => Some(text),
        _ => None,
    })
}

/// One line of a meeting as the reading pane shows it.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Reading {
    Turn {
        time: String,
        speaker: String,
        text: String,
    },
    Mark {
        time: String,
        name: String,
    },
    Missing {
        span: String,
        what: String,
    },
    Other(String),
    Skip,
}

/// History's meeting text without Markdown: the title, blank lines and link
/// targets go; every timestamp, speaker and word stays.
pub(super) fn reading(text: &str) -> Vec<Reading> {
    text.lines()
        .map(parse)
        .filter(|line| *line != Reading::Skip)
        .collect()
}

fn parse(line: &str) -> Reading {
    let line = line.trim_end();
    if line.is_empty() || line.starts_with("# ") {
        return Reading::Skip;
    }
    if let Some((span, what)) = line
        .strip_prefix("*[")
        .and_then(|rest| rest.strip_suffix('*'))
        .and_then(|rest| rest.split_once("] "))
    {
        return Reading::Missing {
            span: span.into(),
            what: what.into(),
        };
    }
    let Some((time, rest)) = line
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] "))
    else {
        return Reading::Other(unescape(line));
    };
    if let Some((speaker, words)) = rest
        .strip_prefix("**")
        .and_then(|rest| rest.split_once(":** "))
    {
        return Reading::Turn {
            time: time.into(),
            speaker: speaker.into(),
            text: unescape(words),
        };
    }
    if let Some((name, _)) = rest
        .strip_prefix("![")
        .and_then(|rest| rest.split_once("]("))
    {
        return Reading::Mark {
            time: time.into(),
            name: name.into(),
        };
    }
    Reading::Other(unescape(line))
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&amp;", "&")
}

#[cfg(test)]
mod tests;
