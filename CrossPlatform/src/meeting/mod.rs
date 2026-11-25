#![allow(dead_code)]

pub(crate) mod bundle;
pub(crate) mod capture;

use uuid::Uuid;

pub(crate) const SAMPLE_RATE: u32 = 16_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MeetingId(Uuid);

impl MeetingId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        Uuid::parse_str(text).ok().map(Self)
    }

    pub(crate) fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for MeetingId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.hyphenated().fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Offset(pub(crate) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) first: Offset,
    pub(crate) end: Offset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Channel {
    You,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Tracks<T> {
    pub(crate) you: T,
    pub(crate) other: T,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TrackSeal {
    pub(crate) valid_bytes: u64,
    pub(crate) captured_until: Offset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Backend {
    Gpu,
    Parakeet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CloseGoal {
    Finish,
    Hold,
    Interrupted(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryReason {
    Interrupted,
    HeldByUser,
    CaptureFailed(String),
    DecodeFailed(String),
    PublishFailed(String),
    FallbackUsed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionInfo {
    pub(crate) id: MeetingId,
    pub(crate) started_at_ms: u64,
}
