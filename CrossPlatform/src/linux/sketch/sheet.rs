//! Numbered contact sheet: keyframes in time order, packed into
//! aspect-justified rows on a dark canvas, each with a small number chip that
//! matches its `- #N (M:SS)` bullet. PNG decode/encode uses GDK textures, so no
//! image crate is added; cairo does the drawing.
use gtk4::{cairo, gdk, glib, prelude::*};

const LONG_EDGE: f64 = 1600.0;
const MAX_PIXELS: f64 = 1_560_000.0;
const GUTTER: f64 = 6.0;
const CHIP: f64 = 24.0;
const INSET: f64 = 5.0;

fn decode(png: &[u8]) -> Result<cairo::ImageSurface, String> {
    let texture = gdk::Texture::from_bytes(&glib::Bytes::from(png))
        .map_err(|error| format!("could not read screenshot: {error}"))?;
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, texture.width(), texture.height())
            .map_err(|error| format!("could not allocate image: {error}"))?;
    let stride = surface.stride() as usize;
    {
        let mut data = surface
            .data()
            .map_err(|error| format!("could not access image: {error}"))?;
        // GDK's default download format is cairo's native ARGB32 layout.
        texture.download(&mut data, stride);
    }
    surface.mark_dirty();
    Ok(surface)
}

fn encode(mut surface: cairo::ImageSurface) -> Result<Vec<u8>, String> {
    surface.flush();
    let (width, height, stride) = (surface.width(), surface.height(), surface.stride());
    let data = surface
        .data()
        .map_err(|error| format!("could not access image: {error}"))?
        .to_vec();
    let texture = gdk::MemoryTexture::new(
        width,
        height,
        gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &glib::Bytes::from_owned(data),
        stride as usize,
    );
    Ok(texture.save_to_png_bytes().to_vec())
}

pub fn png_size(png: &[u8]) -> Result<(i32, i32), String> {
    // IHDR width/height follow the 8-byte signature and 8-byte chunk header.
    if png.len() < 24 || !png.starts_with(b"\x89PNG") {
        return Err("screenshot is not a PNG".into());
    }
    let read = |at: usize| i32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
    Ok((read(16), read(20)))
}

pub fn crop(png: &[u8], x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u8>, String> {
    let source = decode(png)?;
    let target = cairo::ImageSurface::create(cairo::Format::ARgb32, width.max(1), height.max(1))
        .map_err(|error| error.to_string())?;
    let context = cairo::Context::new(&target).map_err(|error| error.to_string())?;
    context
        .set_source_surface(&source, -f64::from(x), -f64::from(y))
        .map_err(|error| error.to_string())?;
    context.paint().map_err(|error| error.to_string())?;
    drop(context);
    encode(target)
}

/// Rows of frame indices plus each row's height, for a canvas `width` wide.
fn layout(aspects: &[f64], width: f64) -> Vec<(Vec<usize>, f64)> {
    let columns = (aspects.len() as f64).sqrt().ceil().max(1.0) as usize;
    (0..aspects.len())
        .collect::<Vec<_>>()
        .chunks(columns)
        .map(|row| {
            let sum: f64 = row.iter().map(|index| aspects[*index]).sum();
            let free = width - GUTTER * (row.len() as f64 + 1.0);
            (row.to_vec(), free / sum)
        })
        .collect()
}

fn canvas_height(rows: &[(Vec<usize>, f64)]) -> f64 {
    rows.iter().map(|(_, height)| height).sum::<f64>() + GUTTER * (rows.len() as f64 + 1.0)
}

/// Canvas width plus row layout fitting the long-edge and pixel caps.
fn fit(aspects: &[f64]) -> (f64, Vec<(Vec<usize>, f64)>) {
    let mut width = LONG_EDGE;
    for _ in 0..4 {
        let rows = layout(aspects, width);
        let height = canvas_height(&rows);
        let shrink = (LONG_EDGE / height).min((MAX_PIXELS / (width * height)).sqrt());
        if shrink >= 0.999 {
            return (width, rows);
        }
        width = (width * shrink).floor();
    }
    let rows = layout(aspects, width);
    (width, rows)
}

fn chip(context: &cairo::Context, x: f64, y: f64, number: usize) -> Result<(), cairo::Error> {
    let label = number.to_string();
    context.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    context.set_font_size(15.0);
    let extents = context.text_extents(&label)?;
    let width = CHIP.max(extents.x_advance() + 10.0);
    let radius = 5.0;
    context.new_sub_path();
    context.arc(
        x + width - radius,
        y + radius,
        radius,
        -90f64.to_radians(),
        0.0,
    );
    context.arc(
        x + width - radius,
        y + CHIP - radius,
        radius,
        0.0,
        90f64.to_radians(),
    );
    context.arc(
        x + radius,
        y + CHIP - radius,
        radius,
        90f64.to_radians(),
        180f64.to_radians(),
    );
    context.arc(
        x + radius,
        y + radius,
        radius,
        180f64.to_radians(),
        270f64.to_radians(),
    );
    context.close_path();
    context.set_source_rgb(1.0, 1.0, 1.0);
    context.fill_preserve()?;
    context.set_source_rgba(0.0, 0.0, 0.0, 0.7);
    context.set_line_width(1.5);
    context.stroke()?;
    context.set_source_rgb(0.0, 0.0, 0.0);
    context.move_to(
        x + (width - extents.x_advance()) / 2.0,
        y + (CHIP + extents.height()) / 2.0,
    );
    context.show_text(&label)
}

/// Builds the contact sheet PNG from keyframe PNGs in time order.
pub fn contact_sheet(frames: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if frames.is_empty() {
        return Err("no keyframes".into());
    }
    let images = frames
        .iter()
        .map(|png| decode(png))
        .collect::<Result<Vec<_>, _>>()?;
    let aspects: Vec<f64> = images
        .iter()
        .map(|image| f64::from(image.width()) / f64::from(image.height().max(1)))
        .collect();
    let (width, rows) = fit(&aspects);
    let height = canvas_height(&rows);
    let canvas =
        cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height.ceil() as i32)
            .map_err(|error| error.to_string())?;
    let context = cairo::Context::new(&canvas).map_err(|error| error.to_string())?;
    let drawn = (|| -> Result<(), cairo::Error> {
        context.set_source_rgb(0.09, 0.09, 0.1);
        context.paint()?;
        let mut y = GUTTER;
        for (row, row_height) in &rows {
            let mut x = GUTTER;
            for index in row {
                let image = &images[*index];
                let tile_width = aspects[*index] * row_height;
                context.save()?;
                context.rectangle(x, y, tile_width, *row_height);
                context.clip();
                context.translate(x, y);
                context.scale(
                    tile_width / f64::from(image.width()),
                    row_height / f64::from(image.height()),
                );
                context.set_source_surface(image, 0.0, 0.0)?;
                context.source().set_filter(cairo::Filter::Good);
                context.paint()?;
                context.restore()?;
                chip(&context, x + INSET, y + INSET, index + 1)?;
                x += tile_width + GUTTER;
            }
            y += row_height + GUTTER;
        }
        Ok(())
    })();
    drawn.map_err(|error| format!("could not draw contact sheet: {error}"))?;
    drop(context);
    encode(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: i32, height: i32, shade: f64) -> Vec<u8> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let context = cairo::Context::new(&surface).unwrap();
        context.set_source_rgb(shade, shade, shade);
        context.paint().unwrap();
        drop(context);
        encode(surface).unwrap()
    }

    #[test]
    fn layout_respects_the_long_edge_and_pixel_caps() {
        for count in [1, 2, 3, 5, 12] {
            let aspects = vec![16.0 / 9.0; count];
            let (width, rows) = fit(&aspects);
            let height = canvas_height(&rows);
            assert!(
                width <= LONG_EDGE && height <= LONG_EDGE + 0.5,
                "{count}: {width}x{height}"
            );
            assert!(
                width * height <= MAX_PIXELS * 1.01,
                "{count}: {width}x{height}"
            );
            assert_eq!(rows.iter().map(|(row, _)| row.len()).sum::<usize>(), count);
        }
    }

    #[test]
    fn crop_and_sheet_round_trip_through_png() {
        let frame = solid(320, 180, 0.8);
        assert_eq!(png_size(&frame).unwrap(), (320, 180));
        let cropped = crop(&frame, 10, 20, 100, 50).unwrap();
        assert_eq!(png_size(&cropped).unwrap(), (100, 50));
        let sheet = contact_sheet(&[frame.clone(), solid(180, 320, 0.2), frame]).unwrap();
        let (width, height) = png_size(&sheet).unwrap();
        assert!(width > 0 && width <= 1600 && height > 0 && height <= 1600);
    }
}

/// Composite in logical coordinates, then retain generous context around the ink.
pub fn composite(
    png: &[u8],
    logical: (i32, i32),
    strokes: &[Vec<(f64, f64)>],
) -> Result<Vec<u8>, String> {
    let source = decode(png)?;
    let (w, h) = (source.width(), source.height());
    if logical.0 <= 0 || logical.1 <= 0 {
        return Err("invalid monitor geometry".into());
    }
    let context = cairo::Context::new(&source).map_err(|e| e.to_string())?;
    context.scale(
        f64::from(w) / f64::from(logical.0),
        f64::from(h) / f64::from(logical.1),
    );
    context.set_source_rgb(super::INK.0, super::INK.1, super::INK.2);
    context.set_line_width(super::INK_WIDTH);
    context.set_line_cap(cairo::LineCap::Round);
    context.set_line_join(cairo::LineJoin::Round);
    for stroke in strokes {
        if let Some((first, rest)) = stroke.split_first() {
            context.move_to(first.0, first.1);
            if rest.is_empty() {
                context.line_to(first.0 + 0.1, first.1);
            }
            for &(x, y) in rest {
                context.line_to(x, y);
            }
            context.stroke().map_err(|e| e.to_string())?;
        }
    }
    drop(context);
    let (x, y, cw, ch) = ink_crop(logical, strokes);
    let sx = f64::from(w) / f64::from(logical.0);
    let sy = f64::from(h) / f64::from(logical.1);
    let left = (x * sx).floor() as i32;
    let top = (y * sy).floor() as i32;
    crop(
        &encode(source)?,
        left,
        top,
        ((x + cw) * sx).ceil() as i32 - left,
        ((y + ch) * sy).ceil() as i32 - top,
    )
}

fn ink_crop(logical: (i32, i32), strokes: &[Vec<(f64, f64)>]) -> (f64, f64, f64, f64) {
    let (w, h) = (f64::from(logical.0), f64::from(logical.1));
    let mut bounds = (w, h, 0.0_f64, 0.0_f64);
    for &(x, y) in strokes.iter().flatten() {
        bounds = (
            bounds.0.min(x),
            bounds.1.min(y),
            bounds.2.max(x),
            bounds.3.max(y),
        );
    }
    let cw = (bounds.2 - bounds.0 + 80.0).max(w * 0.45).min(w);
    let ch = (bounds.3 - bounds.1 + 80.0).max(h * 0.45).min(h);
    (
        ((bounds.0 + bounds.2 - cw) / 2.0).clamp(0.0, w - cw),
        ((bounds.1 + bounds.3 - ch) / 2.0).clamp(0.0, h - ch),
        cw,
        ch,
    )
}

#[cfg(test)]
mod composite_tests {
    use super::*;
    #[test]
    fn edge_crop_is_clamped_and_keeps_context() {
        assert_eq!(
            ink_crop((1000, 800), &[vec![(0.0, 0.0)]]),
            (0.0, 0.0, 450.0, 360.0)
        );
    }
    #[test]
    fn fractional_mapping_composites_red_ink() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1250, 1000).unwrap();
        let png = encode(surface).unwrap();
        let result = composite(&png, (1000, 800), &[vec![(500.0, 400.0), (510.0, 400.0)]]).unwrap();
        if let Ok(path) = std::env::var("KOETT_SAMPLE_PNG") {
            std::fs::write(path, &result).unwrap();
        }
        let (w, h) = png_size(&result).unwrap();
        assert!((562..=564).contains(&w));
        assert_eq!(h, 450);
        let mut decoded = decode(&result).unwrap();
        assert!(
            decoded
                .data()
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| p[2] == 255 && p[1] == 59 && p[0] == 48)
        );
    }
}
