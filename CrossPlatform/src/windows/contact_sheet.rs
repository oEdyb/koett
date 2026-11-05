use super::{
    drawing::Frame,
    drawing_render::Surface,
    pixels::{MAX_PIXELS, Pixels},
};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

fn layout(aspects: &[f64]) -> Result<(u32, u32, Vec<D2D_RECT_F>), String> {
    if aspects.is_empty()
        || aspects.len() > 256
        || aspects.iter().any(|a| !a.is_finite() || *a <= 0.0)
    {
        return Err("Invalid contact-sheet frames".into());
    }
    let columns = (aspects.len() as f64).sqrt().ceil() as usize;
    let mut rects = Vec::with_capacity(aspects.len());
    let mut y = 6.0;
    for row in aspects.chunks(columns) {
        let height = (1600.0 - 6.0 * (row.len() as f64 + 1.0)) / row.iter().sum::<f64>();
        let mut x = 6.0;
        for aspect in row {
            let width = aspect * height;
            rects.push(D2D_RECT_F {
                left: x as f32,
                top: y as f32,
                right: (x + width) as f32,
                bottom: (y + height) as f32,
            });
            x += width + 6.0;
        }
        y += height + 6.0;
    }
    let mut factor = (1600.0 / y)
        .min(1.0)
        .min((MAX_PIXELS as f64 / (1600.0 * y)).sqrt());
    while (1600.0 * factor).ceil() * (y * factor).ceil() > MAX_PIXELS as f64 {
        factor *= 0.999;
    }
    for rect in &mut rects {
        rect.left *= factor as f32;
        rect.top *= factor as f32;
        rect.right *= factor as f32;
        rect.bottom *= factor as f32;
    }
    Ok((
        (1600.0 * factor).ceil().max(1.0) as u32,
        (y * factor).ceil().max(1.0) as u32,
        rects,
    ))
}

pub(super) fn compose(frames: &[Frame]) -> Result<Pixels, String> {
    let aspects = frames
        .iter()
        .map(|f| {
            let (w, h) = f.image.size();
            f64::from(w) / f64::from(h)
        })
        .collect::<Vec<_>>();
    let (width, height, rects) = layout(&aspects)?;
    let surface = Surface::new(width, height).map_err(|e| e.to_string())?;
    let draw = || -> windows::core::Result<()> {
        surface.begin(false);
        for (index, (frame, rect)) in frames.iter().zip(rects).enumerate() {
            surface.background(&frame.image, rect)?;
            let scale = ((rect.right - rect.left) / 42.0)
                .min((rect.bottom - rect.top) / 34.0)
                .min(1.0);
            surface.chip(
                rect.left + 5.0 * scale,
                rect.top + 5.0 * scale,
                index + 1,
                scale,
                1.0,
            )?;
        }
        surface.finish()
    };
    draw().map_err(|e| e.to_string())?;
    surface.pixels()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_aspect_layout_preserves_order_aspect_and_pixel_cap() {
        for count in [1, 2, 16, 64, 256] {
            let aspects = (0..count)
                .map(|i| if i % 2 == 0 { 16.0 / 9.0 } else { 9.0 / 16.0 })
                .collect::<Vec<_>>();
            let (w, h, rects) = layout(&aspects).unwrap();
            assert!(w <= 1600 && h <= 1600 && w as usize * h as usize <= MAX_PIXELS);
            for (r, a) in rects.iter().zip(aspects) {
                assert!((f64::from((r.right - r.left) / (r.bottom - r.top)) - a).abs() < 0.001);
                assert!(
                    r.left >= 0.0 && r.top >= 0.0 && r.right <= w as f32 && r.bottom <= h as f32
                );
            }
            assert!(
                rects
                    .windows(2)
                    .all(|r| r[1].top > r[0].top || r[1].left > r[0].left)
            );
        }
        assert!(layout(&[]).is_err());
        assert!(layout(&[f64::NAN]).is_err());
    }
}
