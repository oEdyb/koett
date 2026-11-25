use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Backend, CloseGoal, MeetingId, Offset, SessionInfo, TrackSeal, Tracks};
use crate::meeting::InterruptCause;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Manifest {
    #[serde(flatten, with = "SessionInfoWire")]
    pub(crate) info: SessionInfo,
    pub(crate) mic: Option<String>,
    #[serde(with = "BackendWire")]
    pub(crate) backend_pref: Backend,
    pub(crate) keep_audio: bool,
    pub(crate) export_dir: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "SessionInfo")]
struct SessionInfoWire {
    #[serde(with = "meeting_id")]
    id: MeetingId,
    started_at_ms: u64,
}

mod meeting_id {
    use super::*;

    pub(super) fn serialize<S: Serializer>(
        id: &MeetingId,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(id)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<MeetingId, D::Error> {
        let value = String::deserialize(deserializer)?;
        MeetingId::parse(&value).ok_or_else(|| serde::de::Error::custom("invalid meeting id"))
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct ManifestFile {
    pub(super) version: u32,
    #[serde(flatten)]
    pub(super) manifest: Manifest,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Backend", rename_all = "snake_case")]
enum BackendWire {
    Gpu,
    Parakeet,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Offset")]
struct OffsetWire(u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Intent {
    #[serde(with = "OffsetWire")]
    pub(crate) cutoff: Offset,
    #[serde(flatten, with = "intent_goal")]
    pub(crate) goal: CloseGoal,
}

mod intent_goal {
    use super::*;

    #[derive(Serialize, Deserialize)]
    #[serde(tag = "goal", rename_all = "snake_case")]
    enum GoalWire {
        Finish,
        Hold,
        Interrupted {
            #[serde(default = "unknown_cause", with = "InterruptCauseWire")]
            cause: InterruptCause,
        },
    }

    #[derive(Serialize, Deserialize)]
    #[serde(
        remote = "InterruptCause",
        tag = "kind",
        content = "message",
        rename_all = "snake_case"
    )]
    enum InterruptCauseWire {
        Shutdown,
        CaptureFault(String),
        IntentUncertain(String),
    }

    fn unknown_cause() -> InterruptCause {
        InterruptCause::IntentUncertain(String::new())
    }

    pub(super) fn serialize<S: Serializer>(
        goal: &CloseGoal,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let wire = match goal {
            CloseGoal::Finish => GoalWire::Finish,
            CloseGoal::Hold => GoalWire::Hold,
            CloseGoal::Interrupted(cause) => GoalWire::Interrupted {
                cause: cause.clone(),
            },
        };
        wire.serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<CloseGoal, D::Error> {
        Ok(match GoalWire::deserialize(deserializer)? {
            GoalWire::Finish => CloseGoal::Finish,
            GoalWire::Hold => CloseGoal::Hold,
            GoalWire::Interrupted { cause } => CloseGoal::Interrupted(cause),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Seal {
    #[serde(with = "OffsetWire")]
    pub(crate) cutoff: Offset,
    #[serde(flatten, with = "TracksWire")]
    pub(crate) tracks: Tracks<TrackSeal>,
    pub(crate) mark_issues: Vec<MarkIssue>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Tracks<TrackSeal>")]
struct TracksWire {
    #[serde(with = "TrackSealWire")]
    you: TrackSeal,
    #[serde(with = "TrackSealWire")]
    other: TrackSeal,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "TrackSeal")]
struct TrackSealWire {
    valid_bytes: u64,
    #[serde(with = "OffsetWire")]
    captured_until: Offset,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MarkIssue {
    pub(crate) id: u64,
    pub(crate) issue: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Job {
    pub(crate) attempts: u64,
    pub(crate) last_error: JobError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub(crate) enum JobError {
    Decode(String),
    Publish(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HistoryReceipt {
    pub(crate) id: String,
    pub(crate) created_at_ms: u64,
    pub(crate) model: String,
    pub(crate) text_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Publication {
    pub(crate) history: HistoryReceipt,
    pub(crate) assets: Vec<String>,
    pub(crate) backends: BTreeMap<String, usize>,
    pub(crate) fallback: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub(crate) transcript_sha256: String,
    #[serde(flatten)]
    pub(crate) publication: Publication,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Projections {
    pub(crate) history: HistoryProjection,
    pub(crate) export: ExportProjection,
}

#[derive(Serialize, Deserialize)]
pub(super) struct ProjectionFile {
    pub(super) transcript_sha256: String,
    #[serde(flatten)]
    pub(super) projections: Projections,
}

impl Projections {
    pub(super) fn pending(manifest: &Manifest) -> Self {
        Self {
            history: HistoryProjection::Pending,
            export: if manifest.export_dir.is_some() {
                ExportProjection::Pending
            } else {
                ExportProjection::Disabled
            },
        }
    }

    pub(in crate::meeting) fn has_pending(self) -> bool {
        self.history == HistoryProjection::Pending || self.export == ExportProjection::Pending
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HistoryProjection {
    Indexed,
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExportProjection {
    Disabled,
    Exported,
    Pending,
}
