use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct PickGeometry {
    pointer_global: (f64, f64),
    point_in_buffer: (f64, f64),
    bounds_in_buffer: (i32, i32, i32, i32),
}

impl PickGeometry {
    pub fn point(&self, pin: (f64, f64)) -> Option<(i32, i32)> {
        let values = [
            pin.0,
            pin.1,
            self.pointer_global.0,
            self.pointer_global.1,
            self.point_in_buffer.0,
            self.point_in_buffer.1,
        ];
        if values
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 10_000_000.0)
        {
            return None;
        }
        let delta = (pin.0 - self.pointer_global.0, pin.1 - self.pointer_global.1);
        if delta.0.abs() > super::PICK_HOLE || delta.1.abs() > super::PICK_HOLE {
            return None;
        }
        let (x, y, width, height) = self.bounds_in_buffer;
        if x.unsigned_abs() > 65536
            || y.unsigned_abs() > 65536
            || !(1..=65536).contains(&width)
            || !(1..=65536).contains(&height)
        {
            return None;
        }
        let local = (
            self.point_in_buffer.0 + delta.0,
            self.point_in_buffer.1 + delta.1,
        );
        let inside = |point: (f64, f64)| {
            point.0 >= f64::from(x)
                && point.1 >= f64::from(y)
                && point.0 < f64::from(x + width)
                && point.1 < f64::from(y + height)
        };
        if !inside(self.point_in_buffer) || !inside(local) {
            return None;
        }
        let rounded = (local.0.round(), local.1.round());
        inside(rounded).then_some((rounded.0 as i32, rounded.1 as i32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> PickGeometry {
        serde_json::from_value(serde_json::json!({
            "pointer_global":[-300.0,400.0], "point_in_buffer":[62.0,42.0],
            "bounds_in_buffer":[12,12,800,600]
        }))
        .unwrap()
    }

    #[test]
    fn uses_buffer_coordinates_with_csd_and_negative_monitor_origins() {
        assert_eq!(geometry().point((-300.0, 400.0)), Some((62, 42)));
        assert_eq!(geometry().point((-295.0, 397.0)), Some((67, 39)));
    }

    #[test]
    fn rejects_pointer_motion_outside_the_pick_hole_and_nonfinite_values() {
        assert_eq!(geometry().point((-267.0, 400.0)), None);
        assert_eq!(geometry().point((f64::NAN, 400.0)), None);
        let mut invalid = geometry();
        invalid.point_in_buffer.0 = f64::INFINITY;
        assert_eq!(invalid.point((-300.0, 400.0)), None);
    }

    #[test]
    fn rejects_border_hits_invalid_bounds_and_rounding_outside_the_window() {
        let mut invalid = geometry();
        invalid.point_in_buffer.0 = 0.0;
        assert_eq!(invalid.point((-300.0, 400.0)), None);
        invalid = geometry();
        invalid.bounds_in_buffer.2 = 0;
        assert_eq!(invalid.point((-300.0, 400.0)), None);
        invalid = geometry();
        invalid.bounds_in_buffer.0 = i32::MAX;
        assert_eq!(invalid.point((-300.0, 400.0)), None);
        invalid = geometry();
        invalid.point_in_buffer.0 = 811.9;
        assert_eq!(invalid.point((-300.0, 400.0)), None);
    }
}
