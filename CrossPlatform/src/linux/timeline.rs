//! Whole-dictation level history for the pill timeline. Every column comes
//! from measured microphone RMS; nothing moves without input.
use crate::sketch::SketchEvent;

/// Columns in the strip (2 px bar + 1 px gap each). The strip is always this
/// wide, from the first frame of a dictation to the last.
pub(super) const COLUMNS: usize = 64;
/// Stored resolution: one bucket per 250 ms (the column size before 16 s).
pub(super) const BASE_MS: u64 = 250;
/// Until this much time has passed, columns are fixed 250 ms slots and the
/// unrecorded rest of the strip shows as a faint track. After it, the whole
/// dictation is resampled into the same 64 columns on every frame, so the
/// strip compresses continuously instead of halving in one jump.
pub(super) const MIN_SPAN_MS: u64 = COLUMNS as u64 * BASE_MS;
/// Bucket storage bound; pairs merge beyond it. Invisible, because 64 display
/// columns are always fewer than half this many buckets.
pub(super) const MAX_BUCKETS: usize = 4_096;
/// Columns quieter than this draw as a silence dot.
pub(super) const SILENT: f64 = 0.08;
/// Distinct sketches kept per dictation. Later new indices are ignored;
/// already-known sketches keep updating. Bounds memory and per-frame drawing.
pub(super) const MAX_SKETCHES: usize = 256;

/// One display column: `None` is the not-yet-recorded track.
pub(super) type Column = Option<f64>;

pub(super) struct Timeline {
    buckets: Vec<f64>,
    bucket_ms: u64,
    elapsed_ms: u64,
    sketches: Vec<SketchEvent>,
    smoothed: f64,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            buckets: Vec::new(),
            bucket_ms: BASE_MS,
            elapsed_ms: 0,
            sketches: Vec::new(),
            smoothed: 0.0,
        }
    }
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

    pub fn push(&mut self, elapsed_ms: u64, rms: f32) {
        self.smoothed = self.smoothed * 0.55 + level(rms) * 0.45;
        let mut index = elapsed_ms / self.bucket_ms;
        while index >= MAX_BUCKETS as u64 {
            self.buckets = self
                .buckets
                .chunks(2)
                .map(|pair| pair.iter().copied().fold(0.0, f64::max))
                .collect();
            self.bucket_ms = self.bucket_ms.saturating_mul(2);
            index = elapsed_ms / self.bucket_ms;
        }
        let index = index as usize;
        if self.buckets.len() <= index {
            self.buckets.resize(index + 1, 0.0);
        }
        self.buckets[index] = self.buckets[index].max(self.smoothed);
        self.elapsed_ms = self.elapsed_ms.max(elapsed_ms);
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// Time the 64 columns cover: 16 s at first, then the whole dictation.
    pub fn span_ms(&self) -> u64 {
        self.elapsed_ms.max(MIN_SPAN_MS)
    }

    /// Exactly [`COLUMNS`] columns, each the loudest bucket it covers.
    pub fn columns(&self) -> Vec<Column> {
        if self.buckets.is_empty() {
            return vec![None; COLUMNS];
        }
        let span = self.span_ms() as f64;
        let recorded = (self.elapsed_ms + 1) as f64;
        let bucket = self.bucket_ms as f64;
        (0..COLUMNS)
            .map(|column| {
                let start = column as f64 * span / COLUMNS as f64;
                if start >= recorded {
                    return None;
                }
                let end = ((column + 1) as f64 * span / COLUMNS as f64).min(recorded);
                let first = (start / bucket).floor() as usize;
                let last = ((end / bucket).ceil() as usize)
                    .max(first + 1)
                    .min(self.buckets.len());
                Some(
                    self.buckets
                        .get(first..last)
                        .map_or(0.0, |range| range.iter().copied().fold(0.0, f64::max)),
                )
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

    /// Fractional column position of a time, on the current span.
    fn position(&self, ms: u64) -> f64 {
        (ms as f64 * COLUMNS as f64 / self.span_ms() as f64).clamp(0.0, COLUMNS as f64)
    }

    /// Start and end of a sketch band in fractional columns; moves smoothly
    /// with the strip as it compresses.
    pub fn span(&self, sketch: &SketchEvent) -> (f64, f64) {
        let start = self.position(sketch.start_ms);
        (start, self.position(sketch.end_ms).max(start))
    }

    /// Column reached by decoding, for a 0..=1 fraction of the recording.
    pub fn decode_head(&self, fraction: f64) -> f64 {
        let fraction = if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.position(self.elapsed_ms) * fraction
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

    fn filled(timeline: &Timeline) -> usize {
        timeline.columns().iter().filter(|c| c.is_some()).count()
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
    fn strip_is_full_width_from_the_first_frame() {
        let mut timeline = Timeline::default();
        assert_eq!(timeline.columns(), vec![None; COLUMNS]);
        timeline.push(0, 1.0);
        let columns = timeline.columns();
        assert_eq!(columns.len(), COLUMNS);
        assert!(columns[0].is_some_and(|level| level > SILENT));
        assert!(
            columns[1..].iter().all(Option::is_none),
            "the rest is track"
        );
        timeline.push(1_000, 0.0);
        let columns = timeline.columns();
        assert_eq!(filled(&timeline), 5);
        assert_eq!(&columns[1..4], &[Some(0.0); 3]);
        for ms in [5_000, 60_000, 300_000, 600_000, 1_200_000] {
            timeline.push(ms, 0.3);
            assert_eq!(timeline.columns().len(), COLUMNS, "{ms} ms");
        }
    }

    #[test]
    fn compression_is_continuous_with_no_jump_at_the_full_point() {
        let mut timeline = Timeline::default();
        // One loud burst at 4 s; quiet otherwise.
        let mut previous_burst = None::<f64>;
        let mut previous_filled = 0;
        for step in 0..=(1_200_000 / 80) {
            let ms = step * 80;
            let rms = if (4_000..4_250).contains(&ms) {
                1.0
            } else {
                0.0
            };
            timeline.push(ms, rms);
            let columns = timeline.columns();
            let filled = filled(&timeline);
            assert!(filled >= previous_filled, "the strip never shrinks back");
            previous_filled = filled;
            // The burst only ever drifts left, at most one column per frame.
            let burst = columns
                .iter()
                .rposition(|c| c.is_some_and(|level| level > 0.6))
                .map(|index| index as f64);
            if let (Some(before), Some(now)) = (previous_burst, burst) {
                assert!(
                    now <= before && before - now <= 1.0,
                    "{ms} ms: {before} -> {now}"
                );
            }
            if ms >= 4_250 {
                assert!(burst.is_some(), "{ms} ms: the burst stays visible");
            }
            previous_burst = burst.or(previous_burst);
        }
        assert_eq!(previous_filled, COLUMNS);
        assert!(timeline.buckets.len() <= MAX_BUCKETS);
        timeline.reset();
        assert!(timeline.is_empty());
        assert_eq!(timeline.span_ms(), MIN_SPAN_MS);
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
    fn fake_sketch_events_upsert_and_map_to_bands() {
        let mut timeline = Timeline::default();
        timeline.push(8_000, 0.2);
        timeline.apply(sketch(2, 9_000, 9_500, true));
        timeline.apply(sketch(1, 1_000, 3_000, false));
        assert!(timeline.drawing());
        timeline.apply(sketch(2, 9_000, 11_000, false));
        assert!(!timeline.drawing());
        assert_eq!(
            timeline
                .sketches()
                .iter()
                .map(|s| s.index)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(timeline.span(&timeline.sketches()[0]), (4.0, 12.0));
        assert_eq!(timeline.span(&timeline.sketches()[1]), (36.0, 44.0));
        // Bands move with the strip as it compresses, and never leave it.
        timeline.push(32_000, 0.2);
        assert_eq!(timeline.span(&timeline.sketches()[1]), (18.0, 22.0));
        let late = sketch(3, 900_000, 999_000, true);
        let max = COLUMNS as f64;
        assert_eq!(timeline.span(&late), (max, max));
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
        // An existing index still finishes after the limit is reached.
        assert!(timeline.apply(sketch(7, 0, 900, false)));
        let seventh = timeline.sketches().iter().find(|s| s.index == 7).unwrap();
        assert_eq!((seventh.end_ms, seventh.live), (900, false));
        timeline.reset();
        assert!(timeline.sketches().is_empty());
        assert!(timeline.apply(sketch(100_001, 0, 10, false)));
    }

    #[test]
    fn decode_head_is_bounded_by_recorded_columns() {
        let mut timeline = Timeline::default();
        assert_eq!(timeline.decode_head(0.5), 0.0);
        timeline.push(10_000, 0.3);
        assert_eq!(timeline.decode_head(0.0), 0.0);
        assert_eq!(timeline.decode_head(0.5), 20.0);
        assert_eq!(timeline.decode_head(2.0), 40.0);
        assert_eq!(timeline.decode_head(f64::NAN), 0.0);
        timeline.push(64_000, 0.3);
        assert_eq!(timeline.decode_head(1.0), COLUMNS as f64);
    }
}
