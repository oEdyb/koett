//! The pill's surface: the pill, the light under it, and one lane of chips and
//! notices above it (below it when the pill sits too close to the screen top).
//! The surface is larger than the pill so the light is cut only by the screen
//! edge; the input region stays the pill and the chips on screen. The pill
//! widens past `WIDTH` only when its clock needs the room.
use super::{ROW, WIDTH};
use gtk4::{self as gtk, prelude::*};
use std::cell::Cell;

pub(super) const PAD_X: i32 = 32;
pub(super) const PAD_TOP: i32 = 48;
pub(super) const PAD_BOTTOM: i32 = 32;
/// The surface around a pill this wide.
pub(super) const fn surface(width: i32) -> (i32, i32) {
    (width + 2 * PAD_X, PAD_TOP + ROW + PAD_BOTTOM)
}
/// The pill on a surface anchored to the bottom edge: centred, 24 px up.
pub(super) const RESTING: (i32, i32) = (PAD_X, surface(WIDTH).1 - ROW - 24);
const LANE_GAP: i32 = 8;
const LANE_HEIGHT: i32 = 30;

/// The light each mode puts around the pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Light {
    Calm,
    Drawing,
    Meeting,
    MeetingDrawing,
    Confirm,
    Issue,
}

impl Light {
    const ALL: [Self; 6] = [
        Self::Calm,
        Self::Drawing,
        Self::Meeting,
        Self::MeetingDrawing,
        Self::Confirm,
        Self::Issue,
    ];

    fn class(self) -> &'static str {
        match self {
            Self::Calm => "koett-light-calm",
            Self::Drawing => "koett-light-drawing",
            Self::Meeting => "koett-light-meeting",
            Self::MeetingDrawing => "koett-light-meeting-drawing",
            Self::Confirm => "koett-light-confirm",
            Self::Issue => "koett-light-issue",
        }
    }

    fn breathes(self) -> bool {
        matches!(
            self,
            Self::Drawing | Self::Meeting | Self::MeetingDrawing | Self::Issue
        )
    }
}

/// True when the lane fits above a pill at this inset.
pub(super) fn lane_above(inset: (i32, i32)) -> bool {
    inset.1 >= LANE_GAP + LANE_HEIGHT
}

pub(super) struct Stage {
    root: gtk::Fixed,
    content: gtk::Widget,
    glow: gtk::Box,
    face: gtk::Box,
    lane: gtk::Box,
    notice: gtk::Box,
    notice_text: gtk::Label,
    chips: gtk::Box,
    inset: Cell<(i32, i32)>,
    width: Cell<i32>,
}

impl Stage {
    pub(super) fn new(content: &impl IsA<gtk::Widget>) -> Self {
        let root = gtk::Fixed::new();
        root.add_css_class("koett-pill-stage");
        let glow = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        glow.add_css_class("koett-pill-glow");
        glow.set_can_target(false);
        let face = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        face.add_css_class("koett-pill-face");
        face.set_overflow(gtk::Overflow::Hidden);
        face.append(content);

        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("koett-pill-notice-dot");
        dot.set_size_request(7, 7);
        dot.set_valign(gtk::Align::Center);
        let notice_text = gtk::Label::new(None);
        notice_text.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let notice = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        notice.add_css_class("koett-pill-notice");
        notice.append(&dot);
        notice.append(&notice_text);
        notice.set_halign(gtk::Align::Start);
        notice.set_valign(gtk::Align::Center);
        notice.set_can_target(false);
        notice.set_visible(false);
        let chips = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        chips.add_css_class("koett-pill-chips");
        chips.set_halign(gtk::Align::End);
        chips.set_hexpand(true);
        chips.set_valign(gtk::Align::Center);
        let lane = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        lane.add_css_class("koett-pill-lane");
        lane.append(&notice);
        lane.append(&chips);

        root.put(&glow, 0.0, 0.0);
        root.put(&face, 0.0, 0.0);
        root.put(&lane, 0.0, 0.0);
        let stage = Self {
            root,
            content: content.clone().upcast(),
            glow,
            face,
            lane,
            notice,
            notice_text,
            chips,
            inset: Cell::new((-1, -1)),
            width: Cell::new(0),
        };
        stage.set_width(WIDTH);
        stage.set_inset(RESTING);
        stage.set_light(Light::Calm, false);
        stage
    }

    pub(super) fn root(&self) -> &gtk::Fixed {
        &self.root
    }

    pub(super) fn add_chip(&self, chip: &gtk::Button) {
        chip.add_css_class("koett-chip");
        chip.set_visible(false);
        self.chips.append(chip);
    }

    pub(super) fn inset(&self) -> (i32, i32) {
        self.inset.get()
    }

    /// The pill's size now.
    pub(super) fn pill(&self) -> (i32, i32) {
        (self.width.get(), ROW)
    }

    pub(super) fn surface(&self) -> (i32, i32) {
        surface(self.width.get())
    }

    /// Resizes the pill; its light, its lane and the surface follow.
    pub(super) fn set_width(&self, width: i32) {
        if self.width.replace(width) == width {
            return;
        }
        let (surface_width, surface_height) = surface(width);
        self.root.set_size_request(surface_width, surface_height);
        for widget in [
            &self.content,
            self.glow.upcast_ref(),
            self.face.upcast_ref(),
        ] {
            widget.set_size_request(width, ROW);
        }
        self.lane.set_size_request(width, LANE_HEIGHT);
    }

    /// Moves the pill within its surface; the lane follows it.
    pub(super) fn set_inset(&self, inset: (i32, i32)) {
        if self.inset.replace(inset) == inset {
            return;
        }
        let (x, y) = (f64::from(inset.0), f64::from(inset.1));
        self.root.move_(&self.glow, x, y);
        self.root.move_(&self.face, x, y);
        let above = lane_above(inset);
        let lane_y = if above {
            inset.1 - LANE_GAP - LANE_HEIGHT
        } else {
            inset.1 + ROW + LANE_GAP
        };
        self.root.move_(&self.lane, x, f64::from(lane_y));
        if above {
            self.lane.remove_css_class("koett-lane-below");
        } else {
            self.lane.add_css_class("koett-lane-below");
        }
    }

    pub(super) fn set_light(&self, light: Light, animate: bool) {
        for other in Light::ALL {
            if other != light {
                self.root.remove_css_class(other.class());
            }
        }
        self.root.add_css_class(light.class());
        if animate && light.breathes() {
            self.glow.add_css_class("koett-breathe");
        } else {
            self.glow.remove_css_class("koett-breathe");
        }
    }

    /// Shows the lane with this notice and these chips, or fades it out. A
    /// fading lane keeps its last content so it leaves as it was.
    pub(super) fn show_lane(&self, notice: Option<&str>, chips: &[&gtk::Button]) {
        if notice.is_none() && chips.is_empty() {
            self.lane.remove_css_class("koett-lane-shown");
            return;
        }
        self.notice.set_visible(notice.is_some());
        if let Some(text) = notice {
            self.notice_text.set_text(text);
        }
        // The lane is as wide as the pill: a long notice leaves room for the chips.
        self.notice_text
            .set_max_width_chars(if chips.is_empty() { 40 } else { 30 });
        let mut child = self.chips.first_child();
        while let Some(widget) = child {
            widget.set_visible(
                chips
                    .iter()
                    .any(|chip| chip.upcast_ref::<gtk::Widget>() == &widget),
            );
            child = widget.next_sibling();
        }
        self.lane.add_css_class("koett-lane-shown");
    }

    /// The pill, plus each chip on screen and the gap between it and the pill,
    /// so the pointer can travel from one to the other without leaving.
    pub(super) fn input_region(&self, chips: &[&gtk::Button]) -> gtk::cairo::Region {
        let (x, y) = self.inset.get();
        let (width, height) = self.pill();
        let region = gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(
            x, y, width, height,
        ));
        for chip in chips {
            let Some(bounds) = chip.compute_bounds(&self.root) else {
                continue;
            };
            let left = bounds.x().floor() as i32;
            let width = bounds.width().ceil() as i32 + 1;
            let top = bounds.y().floor() as i32;
            let bottom = (bounds.y() + bounds.height()).ceil() as i32;
            if width <= 1 || bottom <= top {
                continue;
            }
            let (from, to) = if top < y { (top, y) } else { (y + ROW, bottom) };
            let _ = region.union_rectangle(&gtk::cairo::RectangleInt::new(
                left,
                from,
                width,
                to - from,
            ));
        }
        region
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lane_flips_below_only_when_the_pill_hugs_the_top() {
        assert!(lane_above(RESTING));
        assert!(lane_above((PAD_X, PAD_TOP)));
        assert!(lane_above((0, LANE_GAP + LANE_HEIGHT)));
        assert!(!lane_above((PAD_X, LANE_GAP + LANE_HEIGHT - 1)));
        assert!(!lane_above((PAD_X, 0)));
    }

    #[test]
    fn the_surface_leaves_room_for_the_light_and_a_lane_either_side() {
        assert_eq!(surface(WIDTH), (364, 128));
        assert_eq!(RESTING, (32, 56));
        let below = surface(WIDTH).1 - ROW - (LANE_GAP + LANE_HEIGHT - 1);
        assert!(below >= LANE_GAP + LANE_HEIGHT, "a flipped lane fits");
    }
}
