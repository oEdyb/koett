//! One still image of one monitor, with the sketch ink still on screen.
//!
//! wlroots/niri: `grim -o CONNECTOR` (wlr-screencopy, silent, exact output).
//! KDE and other portal desktops: the non-interactive Screenshot portal, which
//! asks once for blanket permission, writes a file we read and then delete,
//! and may cover every monitor, so the monitor is cropped out.
use std::process::{Command, Stdio};
use std::time::Duration;

/// Logical rectangle: x, y, width, height.
pub type Rect = (i32, i32, i32, i32);

#[derive(Clone, Debug)]
pub struct Target {
    pub connector: Option<String>,
    pub monitor: Rect,
    pub desktop: Rect,
}

pub fn capture(target: &Target) -> Result<Vec<u8>, String> {
    if super::super::gnome_focus::desktop() {
        return super::gnome_capture::capture(target.monitor);
    }
    match grim(target) {
        Ok(png) => Ok(png),
        Err(grim) => portal(target).map_err(|portal| format!("{grim}; {portal}")),
    }
}

fn grim(target: &Target) -> Result<Vec<u8>, String> {
    let mut command = Command::new("grim");
    match &target.connector {
        Some(connector) => command.args(["-o", connector]),
        None => {
            let (x, y, width, height) = target.monitor;
            command.args(["-g", &format!("{x},{y} {width}x{height}")])
        }
    };
    let output = command
        .args(["-t", "png", "-l", "1", "-"])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("grim unavailable: {error}"))?;
    if !output.status.success() || !output.stdout.starts_with(b"\x89PNG") {
        return Err(format!(
            "grim failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn portal(target: &Target) -> Result<Vec<u8>, String> {
    static RUNTIME: std::sync::OnceLock<Result<tokio::runtime::Runtime, String>> =
        std::sync::OnceLock::new();
    let runtime = RUNTIME
        .get_or_init(|| {
            tokio::runtime::Runtime::new().map_err(|_| "Screenshot runtime unavailable".into())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let path = runtime.block_on(async {
        let request = ashpd::desktop::screenshot::Screenshot::request()
            .interactive(false)
            .modal(false)
            .send();
        let response = tokio::time::timeout(Duration::from_secs(30), request)
            .await
            .map_err(|_| "screenshot portal timed out (permission prompt open?)".to_string())?
            .map_err(|error| format!("screenshot portal unavailable: {error}"))?
            .response()
            .map_err(|error| format!("screenshot permission denied: {error}"))?;
        url::Url::parse(response.uri().as_str())
            .ok()
            .and_then(|uri| uri.to_file_path().ok())
            .ok_or_else(|| "screenshot portal returned a non-file URI".to_string())
    })?;
    let png = std::fs::read(&path).map_err(|error| format!("screenshot unreadable: {error}"));
    // The portal saved this file only for this request (KDE uses ~/Pictures).
    let _ = std::fs::remove_file(&path);
    let png = png?;
    let (width, height) = super::sheet::png_size(&png)?;
    match crop_rect(target, width, height) {
        Some((x, y, w, h)) => super::sheet::crop(&png, x, y, w, h),
        None => Ok(png),
    }
}

/// Pixel rectangle of the monitor inside a whole-desktop image, or `None`
/// when the image already shows just that monitor.
fn crop_rect(target: &Target, width: i32, height: i32) -> Option<Rect> {
    let (dx, dy, dw, dh) = target.desktop;
    let (mx, my, mw, mh) = target.monitor;
    if (dw, dh) == (mw, mh) || dw <= 0 || dh <= 0 {
        return None;
    }
    let same_aspect = |w: i32, h: i32| {
        (i64::from(width) * i64::from(h) - i64::from(height) * i64::from(w)).abs()
            <= i64::from(width.max(height))
    };
    if same_aspect(mw, mh) && !same_aspect(dw, dh) {
        return None;
    }
    let scale = f64::from(width) / f64::from(dw);
    let pixel = |value: i32| (f64::from(value) * scale).round() as i32;
    Some((pixel(mx - dx), pixel(my - dy), pixel(mw), pixel(mh)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(monitor: Rect, desktop: Rect) -> Target {
        Target {
            connector: None,
            monitor,
            desktop,
        }
    }

    #[test]
    fn single_monitor_image_is_used_whole() {
        let one = target((0, 0, 1920, 1080), (0, 0, 1920, 1080));
        assert_eq!(crop_rect(&one, 3840, 2160), None);
        let two = target((1920, 0, 1920, 1080), (0, 0, 3840, 1080));
        assert_eq!(crop_rect(&two, 1920, 1080), None);
    }

    #[test]
    fn whole_desktop_image_is_cropped_to_the_monitor_at_its_scale() {
        let right = target((1920, 0, 1920, 1080), (0, 0, 3840, 1080));
        assert_eq!(crop_rect(&right, 7680, 2160), Some((3840, 0, 3840, 2160)));
        let below = target((0, 1080, 1280, 720), (0, 0, 1920, 1800));
        assert_eq!(crop_rect(&below, 1920, 1800), Some((0, 1080, 1280, 720)));
    }
}
