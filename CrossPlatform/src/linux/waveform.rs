//! Bounded recent microphone amplitude history; no synthetic motion.
#[derive(Default)]
pub(super) struct Waveform {
    pub levels: [f64; 17],
    smoothed: f64,
    reduced_motion: bool,
}

impl Waveform {
    pub fn reset(&mut self) {
        self.levels.fill(0.0);
        self.smoothed = 0.0;
    }

    pub fn set_reduced_motion(&mut self, reduced: bool) {
        if self.reduced_motion != reduced {
            self.reset();
            self.reduced_motion = reduced;
        }
    }

    pub fn push(&mut self, rms: f32) {
        if self.reduced_motion {
            return;
        }
        let target = if rms.is_finite() && rms > 0.0 {
            ((20.0 * f64::from(rms).log10()).clamp(-50.0, 0.0) / 50.0 + 1.0).powf(0.75)
        } else {
            0.0
        };
        self.smoothed = self.smoothed * 0.55 + target * 0.45;
        self.levels.rotate_left(1);
        self.levels[16] = self.smoothed;
    }
}

#[cfg(test)]
mod tests {
    use super::Waveform;

    #[test]
    fn real_amplitude_is_scaled_smoothed_and_rolled() {
        let mut wave = Waveform::default();
        wave.push(0.1); // -20dB, Mac normalization and smoothing.
        let first = 0.6_f64.powf(0.75) * 0.45;
        assert!((wave.levels[16] - first).abs() < 1e-7);
        wave.push(1.0);
        assert!((wave.levels[15] - first).abs() < 1e-7);
        assert!((wave.levels[16] - (first * 0.55 + 0.45)).abs() < 1e-7);
        for _ in 0..100 {
            wave.push(0.0);
        }
        assert!(wave.levels.iter().all(|level| *level < 1e-18));
        assert_eq!(wave.levels.len(), 17);
    }

    #[test]
    fn finite_bounded_levels_and_new_capture_reset() {
        let mut wave = Waveform::default();
        for rms in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1.0,
            1e-30,
            10.0,
        ] {
            wave.push(rms);
            assert!(
                wave.levels
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            );
        }
        wave.reset();
        assert_eq!(wave.levels, [0.0; 17]);
        wave.push(0.0);
        assert_eq!(wave.levels, [0.0; 17]);
    }

    #[test]
    fn reduced_motion_clears_history_and_resumes_without_backfill() {
        let mut wave = Waveform::default();
        wave.push(1.0);
        wave.set_reduced_motion(true);
        for _ in 0..100 {
            wave.push(1.0);
        }
        assert_eq!(wave.levels, [0.0; 17]);
        wave.set_reduced_motion(false);
        wave.push(1.0);
        assert_eq!(&wave.levels[..16], &[0.0; 16]);
        assert_eq!(wave.levels[16], 0.45);
    }
}
