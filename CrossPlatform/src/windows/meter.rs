#[derive(Default)]
pub struct Meter {
    values: std::collections::VecDeque<f32>,
    column: u64,
    smoothed: f32,
}
impl Meter {
    pub fn sample(&mut self, milliseconds: u64, rms: f32) {
        let level = if rms.is_finite() && rms > 0.0 {
            ((20.0 * rms.log10()).clamp(-50.0, 0.0) / 50.0 + 1.0).powf(0.75)
        } else {
            0.0
        };
        self.smoothed = self.smoothed * 0.55 + level * 0.45;
        let column = milliseconds / 150;
        for _ in 0..column.saturating_sub(self.column).min(66) {
            self.values.push_back(0.0);
            if self.values.len() > 64 {
                self.values.pop_front();
            }
        }
        if self.values.is_empty() {
            self.values.push_back(0.0);
        }
        self.column = column;
        if let Some(last) = self.values.back_mut() {
            *last = last.max(self.smoothed);
        }
    }
    pub fn bars(&self) -> [f32; 64] {
        let mut result = [0.0; 64];
        for (slot, value) in result[64 - self.values.len()..]
            .iter_mut()
            .zip(&self.values)
        {
            *slot = *value;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::Meter;
    #[test]
    fn silence_stays_flat() {
        let mut meter = Meter::default();
        for ms in (0..20_000).step_by(33) {
            meter.sample(ms, 0.0);
        }
        assert_eq!(meter.bars(), [0.0; 64]);
    }
    #[test]
    fn twenty_minutes_keeps_only_the_window() {
        let mut meter = Meter::default();
        for ms in (0..1_200_000).step_by(33) {
            meter.sample(ms, 0.1);
        }
        assert_eq!(meter.values.len(), 64);
        assert!(meter.bars().iter().all(|x| *x > 0.0 && *x <= 1.0));
    }
    #[test]
    fn a_long_pause_replaces_old_audio_with_silence() {
        let mut meter = Meter::default();
        meter.sample(0, 1.0);
        for ms in (33..12_000).step_by(33) {
            meter.sample(ms, 0.0);
        }
        assert!(meter.bars().iter().all(|x| *x < 0.001));
    }
    #[test]
    fn invalid_levels_do_not_poison_the_meter() {
        let mut meter = Meter::default();
        for rms in [f32::NAN, f32::INFINITY, -1.0] {
            meter.sample(0, rms);
        }
        assert_eq!(meter.bars(), [0.0; 64]);
    }
}
