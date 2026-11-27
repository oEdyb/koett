//! Cairo content of the pill: the glyph, the level strip and its sketch bands.
//! The pill's fill, rim and light are CSS on the stage.
use super::timeline::{self, Bar, COLUMNS};
use super::{Phase, Pill, STEP, STRIP_X, WIDTH, shows_pencil};
use gtk4 as gtk;

type Rgb = (f64, f64, f64);
type Rgba = (f64, f64, f64, f64);

/// Meeting lanes draw a side quieter than this as nothing.
const LANE_FLOOR: f64 = 0.12;

pub(super) struct Palette {
    bar: Rgba,
    dim: Rgba,
    accent: Rgb,
    meeting: Rgb,
    amber: Rgb,
    band: Rgba,
}

pub(super) const DARK: Palette = Palette {
    bar: (1.0, 1.0, 1.0, 0.82),
    dim: (1.0, 1.0, 1.0, 0.28),
    accent: (1.0, 0.31, 0.26),
    meeting: (0.518, 0.549, 1.0),
    amber: (1.0, 0.698, 0.22),
    band: (1.0, 0.31, 0.26, 0.24),
};

pub(super) const LIGHT: Palette = Palette {
    bar: (0.11, 0.098, 0.09, 0.72),
    dim: (0.11, 0.098, 0.09, 0.22),
    accent: (0.85, 0.267, 0.227),
    meeting: (0.345, 0.376, 0.902),
    amber: (0.851, 0.545, 0.067),
    band: (0.85, 0.267, 0.227, 0.16),
};

fn rgb(context: &gtk::cairo::Context, (r, g, b): Rgb) {
    context.set_source_rgb(r, g, b);
}

fn rgba(context: &gtk::cairo::Context, (r, g, b, a): Rgba, alpha: f64) {
    context.set_source_rgba(r, g, b, a * alpha);
}

fn tint(context: &gtk::cairo::Context, (r, g, b): Rgb, alpha: f64) {
    context.set_source_rgba(r, g, b, alpha);
}

pub(super) fn rounded(
    context: &gtk::cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    let radius = radius.min(width / 2.0).min(height / 2.0);
    context.new_sub_path();
    for (cx, cy, angle) in [
        (x + width - radius, y + radius, -std::f64::consts::FRAC_PI_2),
        (x + width - radius, y + height - radius, 0.0),
        (x + radius, y + height - radius, std::f64::consts::FRAC_PI_2),
        (x + radius, y + radius, std::f64::consts::PI),
    ] {
        context.arc(cx, cy, radius, angle, angle + std::f64::consts::FRAC_PI_2);
    }
    context.close_path();
}

pub(super) fn paint(
    context: &gtk::cairo::Context,
    pill: &Pill,
    height: f64,
) -> Result<(), gtk::cairo::Error> {
    if pill.phase == Phase::Hidden {
        return Ok(());
    }
    let palette = if pill.dark { &DARK } else { &LIGHT };
    // Morphing content slides; keep it inside the pill.
    rounded(
        context,
        1.0,
        1.0,
        f64::from(WIDTH) - 2.0,
        height - 2.0,
        height / 2.0,
    );
    context.clip();
    let cy = height / 2.0;
    match (pill.morph_from, pill.morph_progress()) {
        // Same-size morph: the old content slides left and fades quickly while
        // the new content slides in from the right (matches the Mac pill).
        (Some(from), Some(m)) => {
            context.push_group();
            context.translate(-8.0 * m, 0.0);
            paint_body(context, pill, from, cy, palette)?;
            context.pop_group_to_source()?;
            context.paint_with_alpha((1.0 - m * 1.6).max(0.0))?;
            context.push_group();
            context.translate(8.0 * (1.0 - m), 0.0);
            paint_body(context, pill, pill.phase, cy, palette)?;
            context.pop_group_to_source()?;
            context.paint_with_alpha(m)?;
            Ok(())
        }
        _ => paint_body(context, pill, pill.phase, cy, palette),
    }
}

fn paint_body(
    context: &gtk::cairo::Context,
    pill: &Pill,
    phase: Phase,
    cy: f64,
    palette: &Palette,
) -> Result<(), gtk::cairo::Error> {
    glyph(context, pill, phase, cy, palette)?;
    let strip = match phase {
        Phase::Recording | Phase::Transcribing => Some(false),
        Phase::Meeting | Phase::MeetingConfirm | Phase::MeetingSaving => Some(true),
        _ => None,
    };
    match strip {
        Some(lanes) if !pill.timeline.is_empty() => {
            let dim = matches!(phase, Phase::Transcribing | Phase::MeetingSaving);
            let strip = pill.strip;
            bands(context, pill, strip, cy, palette)?;
            for bar in pill.timeline.bars() {
                let left = strip.x(bar.x);
                let alpha = strip.alpha(left);
                if alpha <= 0.0 {
                    continue;
                }
                if lanes {
                    two_lanes(context, &bar, (left, alpha), cy, dim, palette)?;
                } else {
                    one_lane(context, &bar, (left, alpha), cy, dim, palette)?;
                }
            }
            badges(context, pill, strip, palette)
        }
        _ => Ok(()),
    }
}

fn glyph(
    context: &gtk::cairo::Context,
    pill: &Pill,
    phase: Phase,
    cy: f64,
    palette: &Palette,
) -> Result<(), gtk::cairo::Error> {
    let drawing = phase.records() && shows_pencil(pill);
    let meeting = matches!(
        phase,
        Phase::Meeting | Phase::MeetingConfirm | Phase::MeetingSaving
    );
    rgb(context, palette.accent);
    context.set_line_cap(gtk::cairo::LineCap::Round);
    context.set_line_join(gtk::cairo::LineJoin::Round);
    match phase {
        _ if drawing => {
            context.set_line_width(1.8);
            context.move_to(18.0, cy + 6.0);
            context.line_to(19.0, cy + 2.0);
            context.line_to(27.0, cy - 6.0);
            context.line_to(29.5, cy - 3.5);
            context.line_to(21.5, cy + 4.5);
            context.close_path();
            context.stroke()
        }
        Phase::Recording | Phase::Meeting | Phase::MeetingConfirm => {
            if meeting {
                rgb(context, palette.meeting);
            }
            context.arc(23.0, cy, 4.0, 0.0, std::f64::consts::TAU);
            context.fill()
        }
        Phase::Done if pill.cancelled => {
            context.arc(23.0, cy, 4.0, 0.0, std::f64::consts::TAU);
            context.fill()
        }
        Phase::Transcribing | Phase::MeetingSaving => {
            rgba(context, palette.dim, 1.0);
            context.set_line_width(2.0);
            context.arc(23.0, cy, 6.0, 0.0, std::f64::consts::TAU);
            context.stroke()?;
            rgb(
                context,
                if meeting {
                    palette.meeting
                } else {
                    palette.accent
                },
            );
            context.arc(23.0, cy, 6.0, pill.spin, pill.spin + 1.6);
            context.stroke()
        }
        Phase::Recovery | Phase::Message => {
            context.set_line_width(1.8);
            context.move_to(23.0, cy - 7.0);
            context.line_to(30.0, cy + 6.0);
            context.line_to(16.0, cy + 6.0);
            context.close_path();
            context.move_to(23.0, cy - 2.0);
            context.line_to(23.0, cy + 1.5);
            context.stroke()?;
            context.arc(23.0, cy + 3.8, 1.0, 0.0, std::f64::consts::TAU);
            context.fill()
        }
        Phase::Done | Phase::Media => {
            for (index, bar) in [7.0, 13.0, 19.0, 13.0, 7.0].into_iter().enumerate() {
                rounded(
                    context,
                    14.0 + index as f64 * 4.5,
                    cy - bar / 2.0,
                    2.5,
                    bar,
                    1.25,
                );
            }
            context.fill()
        }
        Phase::Hidden => Ok(()),
    }
}

fn x_of(unit: f64) -> f64 {
    STRIP_X + unit * STEP
}

/// Where the strip's columns sit. It ends where its full width ends, or
/// further left when a wide clock (1:12:40 in a wide font) needs the room.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Strip {
    shift: f64,
}

impl Strip {
    pub(super) fn ending_at(end: f64) -> Self {
        Self {
            shift: (end - x_of(COLUMNS as f64)).min(0.0),
        }
    }

    fn x(self, unit: f64) -> f64 {
        x_of(unit) + self.shift
    }

    fn end(self) -> f64 {
        self.x(COLUMNS as f64)
    }

    /// Bars fade in over the six columns after the glyph.
    fn alpha(self, left: f64) -> f64 {
        (((left - STRIP_X) / STEP + 1.0) / 6.0).clamp(0.0, 1.0)
    }
}

fn silence(
    context: &gtk::cairo::Context,
    left: f64,
    cy: f64,
    alpha: f64,
    palette: &Palette,
) -> Result<(), gtk::cairo::Error> {
    rgba(context, palette.dim, alpha);
    rounded(context, left, cy - 1.0, 2.0, 2.0, 1.0);
    context.fill()
}

fn one_lane(
    context: &gtk::cairo::Context,
    bar: &Bar,
    (left, alpha): (f64, f64),
    cy: f64,
    dim: bool,
    palette: &Palette,
) -> Result<(), gtk::cairo::Error> {
    match bar.level() {
        Some(level) if level >= timeline::SILENT => {
            rgba(context, if dim { palette.dim } else { palette.bar }, alpha);
            let height = 4.0 + level * 22.0;
            rounded(context, left, cy - height / 2.0, 2.0, height, 1.0);
            context.fill()
        }
        _ => silence(context, left, cy, alpha, palette),
    }
}

/// You above the centre line, the other side below it in the meeting colour;
/// a side that is reconnecting leaves an amber dot on its side of the line.
fn two_lanes(
    context: &gtk::cairo::Context,
    bar: &Bar,
    (left, alpha): (f64, f64),
    cy: f64,
    dim: bool,
    palette: &Palette,
) -> Result<(), gtk::cairo::Error> {
    let Some(column) = bar.column else {
        return silence(context, left, cy, alpha, palette);
    };
    let you = (!column.lost.you && column.level.you >= LANE_FLOOR).then_some(column.level.you);
    let other =
        (!column.lost.other && column.level.other >= LANE_FLOOR).then_some(column.level.other);
    if let Some(level) = you {
        let height = 2.5 + level * 15.0;
        rgba(context, if dim { palette.dim } else { palette.bar }, alpha);
        rounded(context, left, cy - 0.75 - height, 2.0, height, 1.0);
        context.fill()?;
    }
    if let Some(level) = other {
        let height = 2.5 + level * 15.0;
        if dim {
            rgba(context, palette.dim, alpha);
        } else {
            tint(context, palette.meeting, 0.95 * alpha);
        }
        rounded(context, left, cy + 0.75, 2.0, height, 1.0);
        context.fill()?;
    }
    for (lost, y) in [(column.lost.you, cy - 4.5), (column.lost.other, cy + 2.5)] {
        if lost {
            tint(context, palette.amber, 0.75 * alpha);
            rounded(context, left, y, 2.0, 2.0, 1.0);
            context.fill()?;
        }
    }
    if you.is_none() && other.is_none() && !column.lost.you && !column.lost.other {
        silence(context, left, cy, alpha, palette)?;
    }
    Ok(())
}

fn bands(
    context: &gtk::cairo::Context,
    pill: &Pill,
    strip: Strip,
    cy: f64,
    palette: &Palette,
) -> Result<(), gtk::cairo::Error> {
    for sketch in pill.timeline.sketches() {
        let Some((start, end)) = pill.timeline.band(sketch) else {
            continue;
        };
        let left = (strip.x(start) - 1.0).max(STRIP_X - 1.0);
        let right = strip.x(end);
        if right <= left {
            continue;
        }
        rounded(context, left, cy - 15.0, right - left, 30.0, 3.0);
        rgba(context, palette.band, 1.0);
        if sketch.live {
            context.fill_preserve()?;
            rgb(context, palette.accent);
            context.set_line_width(1.5);
            context.stroke()?;
        } else {
            context.fill()?;
        }
    }
    Ok(())
}

/// Where one band's badge goes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Badge {
    Number { index: u32, left: f64, width: f64 },
    Dot { x: f64 },
}

/// One numbered badge per band on its left edge, newest first. When an older
/// badge would touch a newer one it shrinks to a dot; the newer keeps its
/// number. No badge passes the strip's right end, so none reaches the clock.
/// `bands` are (number, band start in columns).
fn place_badges(
    mut bands: Vec<(u32, f64)>,
    strip: Strip,
    width: impl Fn(u32) -> f64,
) -> Vec<Badge> {
    bands.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut kept_left = f64::INFINITY;
    bands
        .into_iter()
        .map(|(index, start)| {
            let width = width(index);
            let left = (strip.x(start) - 1.5).max(STRIP_X).min(strip.end() - width);
            if left + width + 2.0 > kept_left {
                let x = (strip.x(start) + 2.0).max(STRIP_X + 3.0);
                kept_left = kept_left.min(x - 2.5);
                Badge::Dot { x }
            } else {
                kept_left = left;
                Badge::Number { index, left, width }
            }
        })
        .collect()
}

fn badges(
    context: &gtk::cairo::Context,
    pill: &Pill,
    strip: Strip,
    palette: &Palette,
) -> Result<(), gtk::cairo::Error> {
    context.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Bold,
    );
    context.set_font_size(9.0);
    let bands = pill
        .timeline
        .sketches()
        .iter()
        .filter_map(|sketch| {
            pill.timeline
                .band(sketch)
                .map(|(start, _)| (sketch.index, start))
        })
        .collect();
    let width = |index: u32| {
        context
            .text_extents(&index.to_string())
            .map_or(13.0, |extents| (extents.x_advance().ceil() + 8.0).max(13.0))
    };
    for badge in place_badges(bands, strip, width) {
        rgb(context, palette.accent);
        match badge {
            Badge::Dot { x } => {
                context.arc(x, 9.0, 2.5, 0.0, std::f64::consts::TAU);
                context.fill()?;
            }
            Badge::Number { index, left, width } => {
                rounded(context, left, 2.5, width, 13.0, 6.5);
                context.fill()?;
                let label = index.to_string();
                let extents = context.text_extents(&label)?;
                context.set_source_rgb(1.0, 1.0, 1.0);
                context.move_to(
                    left + width / 2.0 - extents.x_bearing() - extents.width() / 2.0,
                    9.0 - extents.y_bearing() - extents.height() / 2.0,
                );
                context.show_text(&label)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width(index: u32) -> f64 {
        if index < 10 { 13.0 } else { 18.0 }
    }

    #[test]
    fn an_older_badge_that_would_touch_a_newer_one_becomes_a_dot() {
        let placed = place_badges(
            vec![(10, 2.0), (11, 36.0), (12, 41.0)],
            Strip::default(),
            width,
        );
        assert!(matches!(placed[0], Badge::Number { index: 12, .. }));
        assert!(matches!(placed[1], Badge::Dot { .. }), "{placed:?}");
        assert!(matches!(placed[2], Badge::Number { index: 10, .. }));
        let apart = place_badges(vec![(1, 12.0), (2, 45.0)], Strip::default(), width);
        assert!(
            apart
                .iter()
                .all(|badge| matches!(badge, Badge::Number { .. }))
        );
    }

    #[test]
    fn badges_stay_on_the_strip_when_a_wide_clock_shortens_it() {
        let full = x_of(COLUMNS as f64);
        assert_eq!(Strip::ending_at(full + 9.0), Strip::default());
        for strip in [Strip::default(), Strip::ending_at(full - 7.0)] {
            assert!(strip.end() <= full);
            let placed = place_badges(vec![(1, -3.0), (12, 63.5)], strip, width);
            for badge in placed {
                match badge {
                    Badge::Number { left, width, .. } => {
                        assert!(left >= STRIP_X && left + width <= strip.end(), "{badge:?}");
                    }
                    Badge::Dot { x } => assert!(x >= STRIP_X),
                }
            }
        }
        let short = Strip::ending_at(full - 6.0);
        assert_eq!(short.x(COLUMNS as f64 - 1.0), full - 6.0 - STEP);
        assert_eq!(short.alpha(STRIP_X - STEP), 0.0, "slid under the glyph");
    }
}
