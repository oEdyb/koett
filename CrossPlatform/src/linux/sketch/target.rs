//! Where a drawing session's sketches go: a dictation's keyframes and pill
//! timeline, or a meeting's timestamped marks. Meeting marks never touch the
//! frame store, the pill timeline or the speech-linking clock.
use super::Group;
use super::frames::{self, Keyframe};
use crate::meeting::{MeetingId, MeetingMarkScope, Offset};
use crate::pin_context::PinContext;
use std::time::{Duration, Instant};

pub(in crate::linux) enum SketchTarget {
    Dictation { started: Instant, recording: u64 },
    Meeting(MeetingMarkScope),
}

/// Identifies one dictation or one meeting, so an overlay opens once for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Owner {
    Dictation(u64),
    Meeting(MeetingId),
}

/// Why a sketch left the session. A lost one is recorded as a missing mark.
pub(super) enum Removal {
    Undone,
    Lost(String),
}

impl SketchTarget {
    pub(super) fn owner(&self) -> Owner {
        match self {
            Self::Dictation { recording, .. } => Owner::Dictation(*recording),
            Self::Meeting(scope) => Owner::Meeting(scope.id()),
        }
    }

    /// Ask recordings never draw; a meeting is not an Ask recording.
    pub(super) fn skipped_while_asking(&self) -> bool {
        matches!(self, Self::Dictation { .. })
    }

    /// A meeting draws whatever the dictation setting says.
    pub(super) fn wants_overlay(&self, settings: &crate::settings::Settings) -> bool {
        match self {
            Self::Dictation { .. } => settings.draw_while_dictating,
            Self::Meeting(_) => true,
        }
    }

    /// A meeting mark is the image alone: no selected text or page details.
    pub(super) fn reads_pin_context(&self) -> bool {
        matches!(self, Self::Dictation { .. })
    }

    /// Meeting capture never pauses, so only dictation follows the pause state.
    pub(super) fn paused(&self, paused: bool) -> bool {
        paused && matches!(self, Self::Dictation { .. })
    }

    pub(super) fn audio_start(&self, clock: &super::AudioClock, at: Instant) -> Duration {
        match self {
            Self::Dictation { started, .. } => clock.at(*started, at),
            Self::Meeting(_) => Duration::ZERO,
        }
    }

    /// First ink or pin click. False when the meeting no longer takes marks.
    pub(super) fn begin(&self, id: u64, at: Instant) -> bool {
        match self {
            Self::Dictation { .. } => true,
            Self::Meeting(scope) => scope.begin(id, at),
        }
    }

    /// The meeting mark offset of a sketch that started at `at`.
    pub(super) fn offset(&self, at: Instant) -> Option<Offset> {
        match self {
            Self::Dictation { .. } => None,
            Self::Meeting(scope) => Some(scope.offset(at)),
        }
    }

    pub(super) fn deliver(&self, group: &Group, png: Vec<u8>, context: Option<PinContext>) {
        match self {
            Self::Dictation { recording, .. } => {
                frames::push(
                    *recording,
                    Keyframe {
                        order: group.id,
                        start: group.audio_start.as_secs_f32(),
                        png,
                        context,
                    },
                );
            }
            Self::Meeting(scope) => {
                scope.submit(group.id, scope.offset(group.start), png);
            }
        }
    }

    /// Republishes every dictation sketch to the recording pill's timeline.
    /// Numbers follow start order, matching the pasted contact sheet and
    /// bullets. Times are pill (wall-clock) times, which include pauses;
    /// speech linking uses `audio_start` instead. A meeting only tells the pill
    /// whether a stroke is live.
    pub(super) fn publish(&self, groups: &[Group], stroking: Option<u64>) {
        let Self::Dictation { started, .. } = self else {
            super::MEETING_INK.store(stroking.is_some(), std::sync::atomic::Ordering::Release);
            return;
        };
        let mut groups: Vec<&Group> = groups.iter().collect();
        groups.sort_by_key(|group| group.id);
        for (index, group) in groups.into_iter().enumerate() {
            crate::sketch::publish_for(
                *started,
                crate::sketch::SketchEvent {
                    index: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    start_ms: super::pill_ms(*started, group.start),
                    end_ms: super::pill_ms(*started, group.end),
                    live: stroking == Some(group.id),
                },
            );
        }
    }

    /// `remaining` excludes the removed sketch. A dictation's later numbers
    /// shift down, so the pill gets the renumbered list and loses the
    /// now-unused highest number.
    pub(super) fn remove(
        &self,
        removed: u64,
        removal: Removal,
        remaining: &[Group],
        stroking: Option<u64>,
    ) {
        match self {
            Self::Dictation { started, recording } => {
                frames::remove(*recording, removed);
                self.publish(remaining, stroking);
                let before = u32::try_from(remaining.len() + 1).unwrap_or(u32::MAX);
                crate::sketch::remove_for(*started, before);
            }
            Self::Meeting(scope) => {
                match removal {
                    Removal::Undone => scope.undo(removed),
                    Removal::Lost(issue) => scope.miss(removed, issue),
                };
                self.publish(remaining, stroking);
            }
        }
    }

    /// Delivery waits for an open dictation overlay to finish its captures.
    pub(super) fn set_open(&self, open: bool) {
        if let Self::Dictation { recording, .. } = self {
            frames::set_open(*recording, open);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::SAMPLE_RATE;
    use std::sync::atomic::Ordering;

    /// Serializes tests that move the shared pencil indicator.
    static INK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn ink() -> std::sync::MutexGuard<'static, ()> {
        INK.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    struct Folder(std::path::PathBuf);

    impl Folder {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("koett-sketch-target-{}", MeetingId::new()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn marks(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(self.0.join("assets"))
                .unwrap()
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sketch(id: u64, start: Instant) -> Group {
        Group {
            id,
            surface: 0,
            strokes: vec![vec![(1.0, 1.0), (40.0, 40.0)]],
            start,
            end: start,
            audio_start: Duration::from_millis(700),
            painted: std::cell::Cell::new(None),
            lifted: None,
            closed: true,
            phase: super::super::Phase::Captured,
            pin: None,
            context: None,
            png: None,
        }
    }

    #[test]
    fn a_meeting_sketch_lands_as_a_mark_at_its_first_ink() {
        let folder = Folder::new();
        let zero = Instant::now();
        let (scope, writer) = MeetingMarkScope::writing_to(&folder.0, zero);
        let target = SketchTarget::Meeting(scope);
        let first_ink = zero + Duration::from_millis(2_500);
        let group = sketch(1, first_ink);

        assert!(target.begin(group.id, group.start));
        target.deliver(&group, b"\x89PNG mark".to_vec(), None);
        assert!(writer.close(Offset(u64::MAX)).is_empty());

        let offset = 2_500 * u64::from(SAMPLE_RATE) / 1_000;
        assert_eq!(target.offset(first_ink), Some(Offset(offset)));
        assert_eq!(folder.marks(), [format!("mark-{offset:010}-0001.png")]);
    }

    #[test]
    fn undoing_a_meeting_sketch_removes_its_mark() {
        let _ink = ink();
        let folder = Folder::new();
        let zero = Instant::now();
        let (scope, writer) = MeetingMarkScope::writing_to(&folder.0, zero);
        let target = SketchTarget::Meeting(scope);
        let kept = sketch(1, zero + Duration::from_secs(1));
        let undone = sketch(2, zero + Duration::from_secs(3));

        for group in [&kept, &undone] {
            assert!(target.begin(group.id, group.start));
            target.deliver(group, b"\x89PNG".to_vec(), None);
        }
        target.remove(
            undone.id,
            Removal::Undone,
            std::slice::from_ref(&kept),
            None,
        );
        assert!(writer.close(Offset(u64::MAX)).is_empty());

        assert_eq!(folder.marks(), ["mark-0000016000-0001.png"]);
    }

    #[test]
    fn a_lost_meeting_sketch_is_reported_at_close() {
        let _ink = ink();
        let folder = Folder::new();
        let zero = Instant::now();
        let (scope, writer) = MeetingMarkScope::writing_to(&folder.0, zero);
        let target = SketchTarget::Meeting(scope);
        let group = sketch(1, zero);

        assert!(target.begin(group.id, group.start));
        target.remove(group.id, Removal::Lost("display changed".into()), &[], None);
        let issues = writer.close(Offset(u64::MAX));

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].issue, "display changed");
        assert!(folder.marks().is_empty());
    }

    #[test]
    fn undoing_the_live_meeting_sketch_turns_the_pencil_off() {
        let _ink = ink();
        let folder = Folder::new();
        let zero = Instant::now();
        let (scope, _writer) = MeetingMarkScope::writing_to(&folder.0, zero);
        let target = SketchTarget::Meeting(scope);
        let groups = [sketch(1, zero), sketch(2, zero + Duration::from_secs(1))];
        for group in &groups {
            assert!(target.begin(group.id, group.start));
        }

        target.publish(&groups, Some(1));
        target.remove(2, Removal::Undone, &groups[..1], Some(1));
        let other_undone = super::super::MEETING_INK.load(Ordering::Acquire);
        target.remove(1, Removal::Undone, &[], None);
        let live_undone = super::super::MEETING_INK.load(Ordering::Acquire);

        assert!(other_undone);
        assert!(!live_undone);
    }

    #[test]
    fn a_dictation_sketch_still_becomes_a_keyframe() {
        let _guard = frames::TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let started = Instant::now();
        let recording = frames::begin_recording();
        let target = SketchTarget::Dictation { started, recording };
        let group = sketch(4, started);

        assert!(target.begin(group.id, group.start));
        assert_eq!(target.offset(group.start), None);
        target.deliver(&group, vec![7], None);

        let taken = frames::take(Duration::ZERO);
        assert_eq!(taken.len(), 1);
        assert_eq!(
            (taken[0].order, taken[0].start, &taken[0].png),
            (4, 0.7, &vec![7])
        );
    }

    #[test]
    fn only_dictation_follows_pause_and_reads_pin_context() {
        let folder = Folder::new();
        let (scope, _writer) = MeetingMarkScope::writing_to(&folder.0, Instant::now());
        let meeting = SketchTarget::Meeting(scope);
        let dictation = SketchTarget::Dictation {
            started: Instant::now(),
            recording: 1,
        };
        let settings = crate::settings::Settings {
            draw_while_dictating: false,
            ..crate::settings::Settings::default()
        };

        assert!(!meeting.paused(true) && dictation.paused(true));
        assert!(!meeting.reads_pin_context() && dictation.reads_pin_context());
        assert!(meeting.wants_overlay(&settings) && !dictation.wants_overlay(&settings));
        assert!(!meeting.skipped_while_asking() && dictation.skipped_while_asking());
    }
}
