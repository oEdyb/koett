//! Whole-dictation level history for the pill timeline. Every column comes
//! from measured microphone RMS; nothing moves without input.
use crate::sketch::SketchEvent;

/// Columns in the strip (2 px bar + 1 px gap each).
pub(super) const COLUMNS: usize = 64;
/// Time per column at the start of a dictation; doubles each time it fills.
pub(super) const BASE_MS: u64 = 250;
/// Columns quieter than this draw as a silence dot.
pub(super) const SILENT: f64 = 0.08;
/// Distinct sketches kept per dictation. Later new indices are ignored;
/// already-known sketches keep updating. Bounds memory and per-frame drawing.
pub(super) const MAX_SKETCHES: usize = 256;

pub(super) struct Timeline {
    levels: Vec<f64>,
    ms_per_column: u64,
    sketches: Vec<SketchEvent>,
    smoothed: f64,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            levels: Vec::with_capacity(COLUMNS),
            ms_per_column: BASE_MS,
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
        let mut index = elapsed_ms / self.ms_per_column;
        while index >= COLUMNS as u64 {
            self.compress();
            index = elapsed_ms / self.ms_per_column;
        }
        let index = index as usize;
        if self.levels.len() <= index {
            self.levels.resize(index + 1, 0.0);
        }
        self.levels[index] = self.levels[index].max(self.smoothed);
    }

    fn compress(&mut self) {
        self.levels = self
            .levels
            .chunks(2)
            .map(|pair| pair.iter().copied().fold(0.0, f64::max))
            .collect();
        self.ms_per_column = self.ms_per_column.saturating_mul(2);
    }

    pub fn columns(&self) -> &[f64] {
        &self.levels
    }

    #[cfg(test)]
    pub fn ms_per_column(&self) -> u64 {
        self.ms_per_column
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

    pub fn sketches(&self) -> &[SketchEvent] {
        &self.sketches
    }

    pub fn drawing(&self) -> bool {
        self.sketches.iter().any(|sketch| sketch.live)
    }

    /// Inclusive column range covered by a sketch, compressed with the strip.
    pub fn span(&self, sketch: &SketchEvent) -> (usize, usize) {
        let last = COLUMNS as u64 - 1;
        let start = (sketch.start_ms / self.ms_per_column).min(last);
        let end = (sketch.end_ms / self.ms_per_column).clamp(start, last);
        (start as usize, end as usize)
    }

    /// Column reached by decoding, for a 0..=1 fraction of the recording.
    pub fn decode_head(&self, fraction: f64) -> usize {
        let fraction = if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };
        (self.levels.len() as f64 * fraction).round() as usize
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
    fn columns_follow_time_and_keep_gaps_as_silence() {
        let mut timeline = Timeline::default();
        timeline.push(0, 1.0);
        timeline.push(100, 1.0);
        assert_eq!(timeline.columns().len(), 1);
        timeline.push(1_000, 0.0);
        assert_eq!(timeline.columns().len(), 5);
        assert_eq!(&timeline.columns()[1..4], &[0.0; 3]);
        assert!(timeline.columns()[0] > SILENT);
    }

    #[test]
    fn strip_halves_resolution_when_full_and_keeps_peaks() {
        let mut timeline = Timeline::default();
        for step in 0..COLUMNS as u64 {
            timeline.push(step * BASE_MS, if step == 5 { 1.0 } else { 0.0 });
        }
        assert_eq!(timeline.columns().len(), COLUMNS);
        timeline.push(COLUMNS as u64 * BASE_MS, 0.0);
        assert_eq!(timeline.ms_per_column(), BASE_MS * 2);
        assert_eq!(timeline.columns().len(), COLUMNS / 2 + 1);
        assert!(
            timeline.columns()[2] > SILENT,
            "peak from column 5 survives"
        );
        // Ten minutes, the recording limit, always fits.
        timeline.push(600_000, 0.5);
        assert!(timeline.columns().len() <= COLUMNS);
        timeline.reset();
        assert!(timeline.columns().is_empty());
        assert_eq!(timeline.ms_per_column(), BASE_MS);
    }

    #[test]
    fn fake_sketch_events_upsert_and_map_to_bands() {
        let mut timeline = Timeline::default();
        timeline.push(15_750, 0.2);
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
        assert_eq!(timeline.span(&timeline.sketches()[0]), (4, 12));
        assert_eq!(timeline.span(&timeline.sketches()[1]), (36, 44));
        // Bands compress with the strip and never leave it.
        timeline.push(20_000, 0.2);
        assert_eq!(timeline.span(&timeline.sketches()[1]), (18, 22));
        let late = sketch(3, 900_000, 999_000, true);
        assert_eq!(timeline.span(&late), (COLUMNS - 1, COLUMNS - 1));
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
        assert_eq!(timeline.decode_head(0.5), 0);
        timeline.push(9_750, 0.3);
        assert_eq!(timeline.columns().len(), 40);
        assert_eq!(timeline.decode_head(0.0), 0);
        assert_eq!(timeline.decode_head(0.5), 20);
        assert_eq!(timeline.decode_head(2.0), 40);
        assert_eq!(timeline.decode_head(f64::NAN), 0);
    }
}
