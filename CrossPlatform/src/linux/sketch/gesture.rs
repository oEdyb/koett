//! Tells a draw-key click (a pin) from draw-key movement (ink), without windows
//! or capture. Same rules as the Mac's `SketchBoard`; the table and its reasons
//! are in `docs/screen-context-proposal.md`.
use std::time::{Duration, Instant};

/// Pixels the pointer must travel before a hold starts ink, so Shift for a
/// capital letter never leaves a dot.
const START_DISTANCE: f64 = 3.0;
/// A press that travels this far becomes ink instead of a pin.
const PIN_SLOP: f64 = 12.0;
/// A press held longer than this drops no pin.
const PIN_PRESS_LIMIT: Duration = Duration::from_secs(1);
/// Ink shorter than this before a click is aiming jitter and is dropped for
/// the pin; longer ink makes the click part of the drawing.
const PIN_JITTER_INK: f64 = 12.0;
/// Raw button events carry no click count, so the second click of a double
/// click is found by time and distance.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

pub(super) type Point = (f64, f64);

fn distance(a: Point, b: Point) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Mark {
    pub surface: usize,
    pub point: Point,
    pub at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Motion {
    Ignore,
    /// Add `to` to the current stroke, first starting one at `from` when set.
    Ink {
        from: Option<Mark>,
        to: Point,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Down {
    /// No hold, or the click is part of a drawing: nothing changes.
    Ignore,
    /// A possible pin: unclosed ink of this hold is jitter and must go.
    Press,
}

#[derive(Default)]
pub(super) struct PinGesture {
    held: bool,
    origin: Option<Mark>,
    /// Surface of the stroke being drawn and its latest point.
    stroke: Option<(usize, Point)>,
    /// Ink travelled in the current hold.
    ink: f64,
    press: Option<(Mark, bool)>,
    previous_press: Option<Mark>,
}

impl PinGesture {
    /// The draw key went down.
    pub fn hold(&mut self) {
        self.held = true;
        self.origin = None;
        self.stroke = None;
        self.ink = 0.0;
        self.press = None;
    }

    /// The draw key came up. A click still pressed counts as released now.
    pub fn release(&mut self, now: Instant) -> Option<Mark> {
        let pin = self.button_up(now);
        self.held = false;
        self.origin = None;
        self.stroke = None;
        pin
    }

    /// The pointer left the surface: the stroke ends, the hold does not.
    pub fn lift(&mut self) {
        self.origin = None;
        self.stroke = None;
    }

    pub fn motion(&mut self, surface: usize, point: Point, now: Instant) -> Motion {
        if !self.held {
            return Motion::Ignore;
        }
        if let Some((press, _)) = self.press {
            if press.surface != surface {
                self.press = None;
            } else if distance(point, press.point) < PIN_SLOP {
                return Motion::Ignore;
            } else {
                self.press = None;
                self.origin = Some(press);
            }
        }
        if let Some((stroke, last)) = self.stroke
            && stroke == surface
        {
            self.ink += distance(point, last);
            self.stroke = Some((surface, point));
            return Motion::Ink {
                from: None,
                to: point,
            };
        }
        let Some(origin) = self.origin.filter(|origin| origin.surface == surface) else {
            self.origin = Some(Mark {
                surface,
                point,
                at: now,
            });
            return Motion::Ignore;
        };
        let travel = distance(point, origin.point);
        if travel < START_DISTANCE {
            return Motion::Ignore;
        }
        self.ink += travel;
        self.stroke = Some((surface, point));
        Motion::Ink {
            from: Some(origin),
            to: point,
        }
    }

    pub fn button_down(&mut self, surface: usize, point: Point, now: Instant) -> Down {
        if !self.held {
            return Down::Ignore;
        }
        if self.ink >= PIN_JITTER_INK {
            self.press = None;
            return Down::Ignore;
        }
        let mark = Mark {
            surface,
            point,
            at: now,
        };
        let repeat = self.previous_press.is_some_and(|previous| {
            previous.surface == surface
                && now.saturating_duration_since(previous.at) <= DOUBLE_CLICK
                && distance(point, previous.point) < PIN_SLOP
        });
        self.previous_press = Some(mark);
        self.ink = 0.0;
        self.origin = None;
        self.stroke = None;
        self.press = Some((mark, !repeat));
        Down::Press
    }

    /// The button came up: a short, still press becomes a pin.
    pub fn button_up(&mut self, now: Instant) -> Option<Mark> {
        let (press, pins) = self.press.take()?;
        self.origin = None;
        (pins && self.held && now.saturating_duration_since(press.at) <= PIN_PRESS_LIMIT)
            .then_some(press)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Run {
        gesture: PinGesture,
        start: Instant,
    }

    impl Run {
        fn held() -> Self {
            let mut gesture = PinGesture::default();
            gesture.hold();
            Self {
                gesture,
                start: Instant::now(),
            }
        }

        fn at(&self, seconds: f64) -> Instant {
            self.start + Duration::from_secs_f64(seconds)
        }

        fn motion(&mut self, x: f64, seconds: f64) -> Motion {
            let at = self.at(seconds);
            self.gesture.motion(0, (x, 10.0), at)
        }

        fn down(&mut self, x: f64, seconds: f64) -> Down {
            let at = self.at(seconds);
            self.gesture.button_down(0, (x, 10.0), at)
        }

        fn up(&mut self, seconds: f64) -> Option<Point> {
            let at = self.at(seconds);
            self.gesture.button_up(at).map(|pin| pin.point)
        }

        fn click(&mut self, x: f64, seconds: f64) -> Option<Point> {
            self.down(x, seconds);
            self.up(seconds + 0.1)
        }
    }

    fn is_ink(motion: Motion) -> bool {
        matches!(motion, Motion::Ink { .. })
    }

    #[test]
    fn hold_without_click_draws_and_never_pins() {
        let mut run = Run::held();
        assert_eq!(run.motion(0.0, 0.0), Motion::Ignore, "first sample only");
        assert!(is_ink(run.motion(20.0, 0.1)));
        assert_eq!(run.gesture.release(run.at(0.2)), None);
    }

    #[test]
    fn click_drops_a_pin_and_starts_no_stroke() {
        let mut run = Run::held();
        assert_eq!(run.down(50.0, 0.0), Down::Press);
        assert_eq!(run.up(0.1), Some((50.0, 10.0)));
        assert_eq!(run.gesture.stroke, None);
    }

    #[test]
    fn jitter_before_a_click_is_dropped_for_the_pin() {
        let mut run = Run::held();
        run.motion(0.0, 0.0);
        assert!(is_ink(run.motion(6.0, 0.05)), "tiny ink exists first");
        assert_eq!(run.down(6.0, 0.1), Down::Press, "the session drops the ink");
        assert_eq!(run.up(0.2), Some((6.0, 10.0)));
    }

    #[test]
    fn click_mid_stroke_stays_ink() {
        let mut run = Run::held();
        run.motion(0.0, 0.0);
        run.motion(30.0, 0.1);
        assert_eq!(run.down(30.0, 0.2), Down::Ignore);
        assert_eq!(run.up(0.3), None);
        assert_eq!(
            run.motion(40.0, 0.4),
            Motion::Ink {
                from: None,
                to: (40.0, 10.0)
            }
        );
    }

    #[test]
    fn drag_with_the_button_draws_from_the_press_point() {
        let mut run = Run::held();
        run.down(0.0, 0.0);
        assert_eq!(run.motion(5.0, 0.1), Motion::Ignore, "within pin slop");
        let Motion::Ink {
            from: Some(from),
            to,
        } = run.motion(20.0, 0.2)
        else {
            panic!("a drag past the slop draws");
        };
        assert_eq!(from.point, (0.0, 10.0));
        assert_eq!(from.at, run.at(0.0));
        assert_eq!(to, (20.0, 10.0));
        assert_eq!(run.up(0.3), None);
    }

    #[test]
    fn long_still_press_drops_nothing() {
        let mut run = Run::held();
        run.down(10.0, 0.0);
        assert_eq!(run.up(1.5), None);
    }

    #[test]
    fn double_click_drops_one_pin() {
        let mut run = Run::held();
        assert!(run.click(10.0, 0.0).is_some());
        assert_eq!(run.click(10.0, 0.2), None);
    }

    #[test]
    fn two_separate_clicks_drop_two_pins() {
        let mut run = Run::held();
        assert!(run.click(10.0, 0.0).is_some());
        assert!(run.click(10.0, 1.0).is_some(), "too late to be a double");
        assert!(run.click(200.0, 1.2).is_some(), "too far to be a double");
    }

    #[test]
    fn key_lifted_before_the_button_still_pins() {
        let mut run = Run::held();
        run.down(10.0, 0.0);
        let pin = run.gesture.release(run.at(0.05));
        assert_eq!(pin.map(|pin| pin.point), Some((10.0, 10.0)));
    }

    #[test]
    fn ink_after_a_pin_starts_a_new_stroke() {
        let mut run = Run::held();
        assert!(run.click(10.0, 0.0).is_some());
        assert_eq!(run.motion(100.0, 0.5), Motion::Ignore);
        assert!(matches!(
            run.motion(130.0, 0.6),
            Motion::Ink { from: Some(_), .. }
        ));
    }

    #[test]
    fn bare_key_tap_drops_nothing() {
        let mut run = Run::held();
        assert_eq!(run.gesture.release(run.at(0.1)), None);
    }

    #[test]
    fn click_without_the_key_does_nothing() {
        let mut gesture = PinGesture::default();
        let now = Instant::now();
        assert_eq!(gesture.button_down(0, (10.0, 10.0), now), Down::Ignore);
        assert_eq!(gesture.button_up(now), None);
    }

    #[test]
    fn movement_without_the_key_draws_nothing() {
        let mut gesture = PinGesture::default();
        let now = Instant::now();
        assert_eq!(gesture.motion(0, (0.0, 0.0), now), Motion::Ignore);
        assert_eq!(gesture.motion(0, (50.0, 0.0), now), Motion::Ignore);
    }

    #[test]
    fn moving_to_another_monitor_mid_press_drops_no_pin() {
        let mut run = Run::held();
        run.down(10.0, 0.0);
        let at = run.at(0.1);
        assert_eq!(run.gesture.motion(1, (10.0, 10.0), at), Motion::Ignore);
        assert_eq!(run.up(0.2), None);
    }

    #[test]
    fn a_small_wiggle_leaves_no_dot() {
        let mut run = Run::held();
        run.motion(0.0, 0.0);
        assert_eq!(run.motion(2.0, 0.1), Motion::Ignore);
    }
}
