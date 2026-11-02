//! Live level strip with a fixed width, matching the Mac `PillTimeline`: the
//! newest audio enters at the right edge and older columns slide left and out
//! at a constant rate, silence included. The width never changes and no column
//! is ever re-binned, so the strip looks the same at 5 s and at 20 min. Every
//! column comes from measured microphone input.
use crate::sketch::SketchEvent;

/// Columns in the strip (2 px bar + 1 px gap each).
pub(super) const COLUMNS: usize = 64;
/// Time per column: the strip shows the last 9.6 s.
pub(super) const COLUMN_MS: u64 = 150;
/// Columns quieter than this draw as a silence dot.
pub(super) const SILENT: f64 = 0.08;
/// Distinct sketches kept per dictation. Later new indices are ignored;
/// already-known sketches keep updating. Bounds memory and per-frame drawing.
pub(super) const MAX_SKETCHES: usize = 256;
/// Columns kept in memory: the visible window plus the two sliding in/out.
const KEPT: usize = COLUMNS + 2;

/// One drawn column. `x` is in column units from the strip's left edge
/// (fractional while sliding); `level` is `None` before the dictation started.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Bar {
    pub x: f64,
    pub level: Option<f64>,
}

#[derive(Default)]
pub(super) struct Timeline {
    levels: std::collections::VecDeque<f64>,
    first_column: u64,
    latest_ms: u64,
    sketches: Vec<SketchEvent>,
    smoothed: f64,
    started: bool,
}

/// Mac dB mapping: -50..0 dBFS to 0..1 with a 0.75 curve.
pub(super) fn level(rms: f32) -> f64 {
    if rms.is_finite() && rms > 0.0 {
        ((20.0 * f64::from(rms).log10()).clamp(-50.0, 0.0) / 50.0 + 1.0).powf(0.75)
    } else {
        0.0
    }
}

impl Timeline {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Adds one meter sample. Call it on a steady clock (30 Hz) even in
    /// silence: the strip's position follows the latest sample time.
    pub fn push(&mut self, elapsed_ms: u64, rms: f32) {
        self.smoothed = self.smoothed * 0.55 + level(rms) * 0.45;
        self.started = true;
        self.latest_ms = self.latest_ms.max(elapsed_ms);
        let column = elapsed_ms / COLUMN_MS;
        let last = self.first_column + self.levels.len() as u64;
        if column >= last {
            let missing = (column + 1 - last).min(KEPT as u64 + 1) as usize;
            if column + 1 - last > missing as u64 {
                // A long gap: everything kept is out of the window anyway.
                self.levels.clear();
                self.first_column = column + 1 - missing as u64;
            }
            self.levels.extend(std::iter::repeat_n(0.0, missing));
        }
        if let Some(position) = column.checked_sub(self.first_column)
            && let Some(slot) = self.levels.get_mut(position as usize)
        {
            *slot = slot.max(self.smoothed);
        }
        while self.levels.len() > KEPT {
            self.levels.pop_front();
            self.first_column += 1;
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.started
    }

    /// The strip's right edge in column units since the dictation started.
    fn head(&self) -> f64 {
        self.latest_ms as f64 / COLUMN_MS as f64
    }

    fn unit(&self, ms: u64) -> f64 {
        ms as f64 / COLUMN_MS as f64 - self.head() + (COLUMNS - 1) as f64
    }

    /// Columns to draw, oldest first, at their sliding positions.
    pub fn bars(&self) -> Vec<Bar> {
        let head = self.head();
        let newest = (self.latest_ms / COLUMN_MS) as i64;
        (newest - COLUMNS as i64..=newest)
            .filter_map(|column| {
                let x = column as f64 - head + (COLUMNS - 1) as f64;
                if x <= -1.0 {
                    return None;
                }
                if column < 0 {
                    return Some(Bar { x, level: None });
                }
                let level = (column as u64)
                    .checked_sub(self.first_column)
                    .and_then(|position| self.levels.get(position as usize))
                    .copied()
                    .unwrap_or(0.0);
                Some(Bar {
                    x,
                    level: Some(level),
                })
            })
            .collect()
    }

    /// Replaces the sketch with the same index, or inserts it while fewer than
    /// [`MAX_SKETCHES`] are known. Returns false when a new index was dropped.
    pub fn apply(&mut self, event: SketchEvent) -> bool {
        match self
            .sketches
            .binary_search_by_key(&event.index, |sketch| sketch.index)
        {
            Ok(position) => self.sketches[position] = event,
            Err(_) if self.sketches.len() >= MAX_SKETCHES => return false,
            Err(position) => self.sketches.insert(position, event),
        }
        true
    }

    /// Drops an undone sketch's band.
    pub fn remove(&mut self, index: u32) {
        self.sketches.retain(|sketch| sketch.index != index);
    }

    pub fn sketches(&self) -> &[SketchEvent] {
        &self.sketches
    }

    pub fn drawing(&self) -> bool {
        self.sketches.iter().any(|sketch| sketch.live)
    }

    /// A sketch's band in column units, clipped to the strip; `None` once it
    /// has slid out on the left.
    pub fn band(&self, sketch: &SketchEvent) -> Option<(f64, f64)> {
        let start = self.unit(sketch.start_ms).max(0.0);
        let end = (self.unit(sketch.end_ms) + 1.0).min(COLUMNS as f64);
        (end > 0.0 && start < COLUMNS as f64 && end >= start).then_some((start, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sketch(index: u32, start_ms: u64, end_ms: u64, live: bool) -> SketchEvent {
        SketchEvent {
            index,
            start_ms,
            end_ms,
            live,
        }
    }

    /// Pushes 30 Hz samples from `from` to `to` ms; loud inside `loud`.
    fn run(timeline: &mut Timeline, from: u64, to: u64, loud: std::ops::Range<u64>) {
        let mut ms = from;
        while ms <= to {
            timeline.push(ms, if loud.contains(&ms) { 1.0 } else { 0.0 });
            ms += 33;
        }
    }

    #[test]
    fn level_uses_mac_mapping_and_rejects_non_finite_input() {
        assert!((level(0.1) - 0.6_f64.powf(0.75)).abs() < 1e-6);
        assert_eq!(level(1.0), 1.0);
        assert_eq!(level(10.0), 1.0);
        for rms in [0.0, -1.0, 1e-30, f32::NAN, f32::INFINITY] {
            let value = level(rms);
            assert!(value.is_finite() && (0.0..=1.0).contains(&value));
        }
        assert_eq!(level(1e-30), 0.0);
    }

    #[test]
    fn strip_spans_both_edges_from_the_first_frame_to_twenty_minutes() {
        let mut timeline = Timeline::default();
        assert!(timeline.is_empty());
        timeline.push(0, 1.0);
        let bars = timeline.bars();
        assert!(bars.first().unwrap().x <= 0.0, "reaches the left edge");
        assert_eq!(bars.last().unwrap().x, (COLUMNS - 1) as f64);
        assert!(bars[..bars.len() - 1].iter().all(|bar| bar.level.is_none()));
        assert!(bars.last().unwrap().level.unwrap() > SILENT);
        for end in [5_000, 30_000, 60_000, 300_000, 1_200_000] {
            let from = timeline.latest_ms + 33;
            run(&mut timeline, from, end, 0..0);
            let bars = timeline.bars();
            assert!(bars.first().unwrap().x <= 0.0, "{end} ms left edge");
            assert!(
                bars.last().unwrap().x > (COLUMNS - 2) as f64,
                "{end} ms right edge"
            );
            assert!(bars.len() <= COLUMNS + 1);
            assert!(timeline.levels.len() <= KEPT, "memory stays bounded");
        }
        assert!(timeline.bars().iter().all(|bar| bar.level.is_some()));
    }

    #[test]
    fn a_peak_moves_only_by_elapsed_time_and_never_squeezes() {
        let mut timeline = Timeline::default();
        run(&mut timeline, 0, 4_000, 3_000..3_150);
        let peak = |timeline: &Timeline| {
            timeline
                .bars()
                .iter()
                .filter(|bar| bar.level.is_some_and(|level| level > 0.5))
                .map(|bar| bar.x)
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let at_4s = peak(&timeline);
        // The same silence-only time later, the peak sits exactly that many
        // columns further left: constant slide, no compression.
        run(&mut timeline, 4_033, 7_000, 0..0);
        let elapsed_columns = (timeline.latest_ms - 3_993) as f64 / COLUMN_MS as f64;
        assert!(
            (at_4s - peak(&timeline) - elapsed_columns).abs() < 0.3,
            "{at_4s} -> {}",
            peak(&timeline)
        );
        // After 9.6 s more it has left the strip.
        run(&mut timeline, 7_033, 17_000, 0..0);
        assert_eq!(peak(&timeline), f64::NEG_INFINITY);
    }

    #[test]
    fn long_gaps_keep_memory_bounded_and_read_as_silence() {
        let mut timeline = Timeline::default();
        timeline.push(0, 1.0);
        timeline.push(3_600_000, 0.0);
        assert!(timeline.levels.len() <= KEPT);
        let bars = timeline.bars();
        // Only the newest column carries the smoothed decay of the old peak.
        assert!(
            bars[..bars.len() - 1]
                .iter()
                .all(|bar| bar.level == Some(0.0))
        );
        assert!(bars.last().unwrap().level.unwrap() < 0.5);
        timeline.reset();
        assert!(timeline.is_empty());
    }

    #[test]
    fn removed_sketch_drops_only_its_band() {
        let mut timeline = Timeline::default();
        timeline.apply(sketch(1, 1_000, 2_000, false));
        timeline.apply(sketch(2, 3_000, 4_000, false));
        timeline.remove(2);
        timeline.remove(7);
        assert_eq!(timeline.sketches(), [sketch(1, 1_000, 2_000, false)]);
    }

    #[test]
    fn sketch_bands_slide_with_the_strip_and_leave_on_the_left() {
        let mut timeline = Timeline::default();
        timeline.push(9_450, 0.2);
        timeline.apply(sketch(2, 9_000, 9_300, true));
        timeline.apply(sketch(1, 1_500, 3_000, false));
        assert!(timeline.drawing());
        timeline.apply(sketch(2, 9_000, 9_450, false));
        assert!(!timeline.drawing());
        assert_eq!(
            timeline
                .sketches()
                .iter()
                .map(|s| s.index)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(timeline.band(&timeline.sketches()[0]), Some((10.0, 21.0)));
        assert_eq!(timeline.band(&timeline.sketches()[1]), Some((60.0, 64.0)));
        timeline.push(12_450, 0.2);
        assert_eq!(timeline.band(&timeline.sketches()[0]), Some((0.0, 1.0)));
        timeline.push(20_000, 0.2);
        assert_eq!(timeline.band(&timeline.sketches()[0]), None);
    }

    #[test]
    fn sketch_history_is_bounded_but_known_sketches_keep_updating() {
        let mut timeline = Timeline::default();
        for index in 1..=100_000 {
            timeline.apply(sketch(index, 0, 10, true));
        }
        assert_eq!(timeline.sketches().len(), MAX_SKETCHES);
        assert!(!timeline.apply(sketch(100_001, 0, 10, false)));
        assert_eq!(timeline.sketches().len(), MAX_SKETCHES);
        assert!(timeline.apply(sketch(7, 0, 900, false)));
        let seventh = timeline.sketches().iter().find(|s| s.index == 7).unwrap();
        assert_eq!((seventh.end_ms, seventh.live), (900, false));
        timeline.reset();
        assert!(timeline.sketches().is_empty());
        assert!(timeline.apply(sketch(100_001, 0, 10, false)));
    }
}
