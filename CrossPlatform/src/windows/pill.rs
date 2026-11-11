use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::{Common::*, *};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::{Common::DXGI_ALPHA_MODE_PREMULTIPLIED, IDXGIDevice};
use windows::core::Interface;
use windows::core::w;
use windows_numerics::{Matrix3x2, Vector2};

pub(super) struct TimelinePaint<'a> {
    pub bars: &'a [(f32, f32)],
    pub bands: &'a [(u32, f32, f32)],
}
pub(super) struct Renderer {
    device: IDCompositionDesktopDevice,
    _target: IDCompositionTarget,
    visual: IDCompositionVisual2,
    opacity: IDCompositionEffectGroup,
    surface: IDCompositionSurface,
    size: (u32, u32),
    text: IDWriteTextFormat,
    body: IDWriteTextFormat,
}

impl Renderer {
    pub fn new(hwnd: HWND, size: (u32, u32)) -> windows::core::Result<Self> {
        unsafe {
            let mut d3d = None;
            let create = |driver, output: &mut Option<ID3D11Device>| {
                D3D11CreateDevice(
                    None,
                    driver,
                    Default::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(output),
                    None,
                    None,
                )
            };
            create(D3D_DRIVER_TYPE_HARDWARE, &mut d3d)
                .or_else(|_| create(D3D_DRIVER_TYPE_WARP, &mut d3d))?;
            let dxgi: IDXGIDevice = d3d.ok_or_else(windows::core::Error::from_thread)?.cast()?;
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d = factory.CreateDevice(&dxgi)?;
            let device: IDCompositionDesktopDevice = DCompositionCreateDevice2(&d2d)?;
            let target = device.CreateTargetForHwnd(hwnd, false)?;
            let visual = device.CreateVisual()?;
            let opacity = device.CreateEffectGroup()?;
            visual.SetEffect(&opacity)?;
            let surface = device.CreateSurface(
                size.0,
                size.1,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_ALPHA_MODE_PREMULTIPLIED,
            )?;
            visual.SetContent(&surface)?;
            target.SetRoot(&visual)?;
            let write: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let text = write.CreateTextFormat(
                w!("Consolas"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                12.0,
                w!("en-US"),
            )?;
            text.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)?;
            text.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            let body = write.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                12.0,
                w!("en-US"),
            )?;
            body.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            body.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            Ok(Self {
                device,
                _target: target,
                visual,
                opacity,
                surface,
                size,
                text,
                body,
            })
        }
    }

    pub fn paint(
        &mut self,
        hwnd: HWND,
        bounds: RECT,
        timeline: TimelinePaint<'_>,
        elapsed: u64,
        state: &super::pill_state::State,
        now: u64,
    ) -> windows::core::Result<()> {
        unsafe {
            let size = (
                (bounds.right - bounds.left) as u32,
                (bounds.bottom - bounds.top) as u32,
            );
            if self.size != size {
                self.surface = self.device.CreateSurface(
                    size.0,
                    size.1,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_ALPHA_MODE_PREMULTIPLIED,
                )?;
                self.visual.SetContent(&self.surface)?;
                self.size = size;
            }
            let mut offset = windows::Win32::Foundation::POINT::default();
            let target: ID2D1DeviceContext = self.surface.BeginDraw(None, &mut offset)?;
            let result = (|| -> windows::core::Result<()> {
                let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) as f32;
                target.SetDpi(dpi, dpi);
                let light = light_theme();
                let mut background = if light {
                    color(1.0, 1.0, 1.0)
                } else {
                    color(0.075, 0.075, 0.08)
                };
                background.a = 0.97;
                let coral = target.CreateSolidColorBrush(
                    &if light {
                        color(0.85, 0.267, 0.227)
                    } else {
                        color(1.0, 0.31, 0.26)
                    },
                    None,
                )?;
                let white = target.CreateSolidColorBrush(
                    &if light {
                        color(0.11, 0.098, 0.09)
                    } else {
                        color(1.0, 1.0, 1.0)
                    },
                    None,
                )?;
                target.SetTransform(&Matrix3x2 {
                    M11: 1.0,
                    M12: 0.0,
                    M21: 0.0,
                    M22: 1.0,
                    M31: offset.x as f32 * 96.0 / dpi,
                    M32: offset.y as f32 * 96.0 / dpi,
                });
                target.Clear(Some(&D2D1_COLOR_F::default()));
                let fill = target.CreateSolidColorBrush(&background, None)?;
                target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: rect(0.0, 0.0, 300.0, 48.0),
                        radiusX: 23.0,
                        radiusY: 23.0,
                    },
                    &fill,
                );
                let stroke = target.CreateSolidColorBrush(
                    &if light {
                        color(0.11, 0.098, 0.09)
                    } else {
                        color(1.0, 1.0, 1.0)
                    },
                    None,
                )?;
                stroke.SetOpacity(if light { 0.12 } else { 0.10 });
                target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: rect(0.5, 0.5, 299.5, 47.5),
                        radiusX: 23.0,
                        radiusY: 23.0,
                    },
                    &stroke,
                    1.0,
                    None,
                );
                use super::pill_state::Phase;
                let reduced = reduced_motion();
                self.opacity.SetOpacity2(state.opacity(now, reduced))?;
                let active = matches!(state.phase, Phase::Recording | Phase::Transcribing);
                let morph = state.morph(now, reduced);
                white.SetOpacity(0.90);
                let draw = |label: &str, left: f32, right: f32, clock: bool| {
                    target.DrawText(
                        &label.encode_utf16().collect::<Vec<_>>(),
                        if clock { &self.text } else { &self.body },
                        &rect(left, 0.0, right, 48.0),
                        &white,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                };
                if matches!(state.phase, Phase::Done(_) | Phase::Copied) {
                    for (i, h) in [6.0, 12.0, 18.0, 12.0, 6.0].iter().enumerate() {
                        let x = 16.0 + i as f32 * 3.0;
                        target.FillRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: rect(x, 24.0 - h / 2.0, x + 2.0, 24.0 + h / 2.0),
                                radiusX: 1.0,
                                radiusY: 1.0,
                            },
                            &coral,
                        );
                    }
                } else if state.drawing {
                    draw("✎", 16.0, 33.0, false);
                } else if matches!(
                    state.phase,
                    Phase::NotPasted { .. } | Phase::Partial { .. } | Phase::Error { .. }
                ) {
                    draw("!", 19.0, 30.0, false);
                } else if matches!(state.phase, Phase::Transcribing) {
                    let index = if reduced { 0 } else { (now / 100) % 8 };
                    for i in 0..8 {
                        let angle = i as f32 * std::f32::consts::TAU / 8.0;
                        coral.SetOpacity(if i == index { 1.0 } else { 0.25 });
                        let x = 23.0 + angle.cos() * 5.0;
                        let y = 24.0 + angle.sin() * 5.0;
                        target.FillRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: rect(x - 1.0, y - 1.0, x + 1.0, y + 1.0),
                                radiusX: 1.0,
                                radiusY: 1.0,
                            },
                            &coral,
                        );
                    }
                    coral.SetOpacity(1.0);
                } else {
                    target.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: rect(19.0, 20.0, 27.0, 28.0),
                            radiusX: 4.0,
                            radiusY: 4.0,
                        },
                        &coral,
                    );
                }
                let notice = state.notice(now).filter(|_| active);
                if (active && notice.is_none()) || (!active && morph < 1.0) {
                    let wave = target.CreateSolidColorBrush(
                        &if light {
                            color(0.11, 0.098, 0.09)
                        } else {
                            color(1.0, 1.0, 1.0)
                        },
                        None,
                    )?;
                    for (index, left, right) in timeline.bands {
                        coral.SetOpacity(0.14);
                        let x = 44.0 + left * 3.0;
                        let end = 44.0 + right * 3.0;
                        target.FillRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: rect(x, 9.0, end.max(x + 3.0), 39.0),
                                radiusX: 3.0,
                                radiusY: 3.0,
                            },
                            &coral,
                        );
                        coral.SetOpacity(0.9);
                        let label = index.to_string().encode_utf16().collect::<Vec<_>>();
                        target.DrawText(
                            &label,
                            &self.body,
                            &rect(x + 1.0, 30.0, (x + 24.0).min(236.0), 44.0),
                            &coral,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
                    }
                    coral.SetOpacity(1.0);
                    for (position, level) in timeline.bars {
                        let silent = *level < 0.08;
                        let dim = silent || matches!(state.phase, Phase::Transcribing);
                        let alpha = if light {
                            if dim { 0.22 } else { 0.72 }
                        } else if dim {
                            0.28
                        } else {
                            0.82
                        };
                        let fade = ((position + 1.0) / 6.0).clamp(0.0, 1.0);
                        let morph_alpha = if active {
                            1.0
                        } else {
                            (1.0 - 1.6 * morph).max(0.0)
                        };
                        wave.SetOpacity(alpha * fade * morph_alpha);
                        let height = if silent { 2.0 } else { 4.0 + level * 22.0 };
                        let x = 44.0 + position * 3.0;
                        target.FillRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: rect(x, 24.0 - height / 2.0, x + 2.0, 24.0 + height / 2.0),
                                radiusX: 1.0,
                                radiusY: 1.0,
                            },
                            &wave,
                        );
                    }
                }
                if active {
                    if let Some(notice) = notice {
                        draw(notice, 44.0, 235.0, false);
                    }
                    // Hover turns the clock into ✕ in the same place; the strip stays.
                    if state.hovered {
                        // Mac's cancel button: a plain cross, filled only under the pointer.
                        if state.on_cancel {
                            white.SetOpacity(if light { 0.13 } else { 0.20 });
                            target.FillEllipse(
                                &D2D1_ELLIPSE {
                                    point: Vector2 { X: 272.0, Y: 24.0 },
                                    radiusX: 13.0,
                                    radiusY: 13.0,
                                },
                                &white,
                            );
                        }
                        white.SetOpacity(0.90);
                        for (from, to) in [
                            ((268.0, 20.0), (276.0, 28.0)),
                            ((268.0, 28.0), (276.0, 20.0)),
                        ] {
                            target.DrawLine(
                                Vector2 {
                                    X: from.0,
                                    Y: from.1,
                                },
                                Vector2 { X: to.0, Y: to.1 },
                                &white,
                                1.5,
                                None,
                            );
                        }
                    } else if !matches!(state.phase, Phase::Transcribing) {
                        draw(
                            &format!("{}:{:02}", elapsed / 60, elapsed % 60),
                            238.0,
                            284.0,
                            true,
                        );
                    }
                } else {
                    white.SetOpacity(0.90 * morph);
                    let shift = 8.0 * (1.0 - morph);
                    match &state.phase {
                        Phase::Done(preview) => {
                            draw(
                                &preview.replace(['\n', '\r'], " "),
                                44.0 + shift,
                                if state.repeat_paste { 182.0 } else { 234.0 },
                                false,
                            );
                            if state.repeat_paste {
                                draw("Paste", 192.0, 237.0, false);
                            }
                            draw(
                                if state.copied { "Copied" } else { "Copy" },
                                246.0,
                                295.0,
                                false,
                            );
                        }
                        Phase::NotPasted { saved } => {
                            draw(
                                if *saved {
                                    "Not pasted · saved"
                                } else {
                                    "Not pasted · unsaved"
                                },
                                44.0 + shift,
                                189.0,
                                false,
                            );
                            if state.repeat_paste {
                                draw("Paste", 192.0, 237.0, false);
                            }
                            draw("Copy", 245.0, 292.0, false);
                        }
                        Phase::Partial {
                            text_landed,
                            other_copied,
                            saved,
                        } => {
                            draw(
                                super::pill_state::partial_label(
                                    *text_landed,
                                    *other_copied,
                                    *saved,
                                ),
                                44.0 + shift,
                                234.0,
                                false,
                            );
                            draw(
                                if state.copied { "Copied" } else { "Copy" },
                                246.0,
                                295.0,
                                false,
                            );
                        }
                        Phase::Copied => draw("Copied to clipboard", 44.0, 284.0, false),
                        Phase::Cancelled => draw("Cancelled", 44.0, 284.0, false),
                        Phase::Error { message, .. } => draw(message, 44.0, 284.0, false),
                        _ => {}
                    }
                }
                Ok(())
            })();
            let ended = self.surface.EndDraw();
            result?;
            ended?;
            self.device.Commit()
        }
    }
}
fn color(r: f32, g: f32, b: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a: 1.0 }
}
fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

pub(super) fn reduced_motion() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut enabled = windows::core::BOOL(1);
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut enabled as *mut windows::core::BOOL).cast()),
            Default::default(),
        );
    }
    !enabled.as_bool()
}

pub(super) fn light_theme() -> bool {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let mut value = 0_u32;
    let mut size = 4_u32;
    unsafe {
        let _ = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        );
    }
    value != 0
}
