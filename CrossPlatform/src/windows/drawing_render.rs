use super::pixels::Pixels;
use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
            Dxgi::Common::*,
            Gdi::*,
        },
    },
    core::w,
};
use windows_numerics::Vector2;

struct Dib {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    data: *mut u8,
}
impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}
pub(super) struct Surface {
    target: ID2D1DCRenderTarget,
    text: IDWriteTextFormat,
    write: IDWriteFactory,
    dib: Dib,
    width: u32,
    height: u32,
}
fn color(alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 59.0 / 255.0,
        b: 48.0 / 255.0,
        a: alpha,
    }
}
impl Surface {
    pub fn new(width: u32, height: u32) -> windows::core::Result<Self> {
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 24_000_000 {
            return Err(windows::core::Error::new(
                E_INVALIDARG,
                "Drawing surface exceeds its limit",
            ));
        }
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return Err(windows::core::Error::from_thread());
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut data = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut data, None, 0)
            {
                Ok(v) => v,
                Err(e) => {
                    let _ = DeleteDC(dc);
                    return Err(e);
                }
            };
            let old = SelectObject(dc, bitmap.into());
            let dib = Dib {
                dc,
                bitmap,
                old,
                data: data.cast(),
            };
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let target = factory.CreateDCRenderTarget(&D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            })?;
            target.BindDC(
                dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                },
            )?;
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let text = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                16.0,
                w!("en-US"),
            )?;
            text.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            text.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            Ok(Self {
                target,
                text,
                write: factory,
                dib,
                width,
                height,
            })
        }
    }
    pub fn begin(&self, shield: bool) {
        unsafe {
            self.target.BeginDraw();
            self.target.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: if shield { 1.0 / 255.0 } else { 0.0 },
            }));
        }
    }
    pub fn background(&self, image: &Pixels, rect: D2D_RECT_F) -> windows::core::Result<()> {
        let (width, height) = image.size();
        unsafe {
            let bitmap = self.target.CreateBitmap(
                D2D_SIZE_U { width, height },
                Some(image.bytes().as_ptr().cast()),
                width * 4,
                &D2D1_BITMAP_PROPERTIES {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                },
            )?;
            self.target.DrawBitmap(
                &bitmap,
                Some(&rect),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }
        Ok(())
    }
    pub fn stroke(
        &self,
        points: &[(f64, f64)],
        scale: f32,
        alpha: f32,
    ) -> windows::core::Result<()> {
        unsafe {
            let brush = self.target.CreateSolidColorBrush(&color(alpha), None)?;
            for pair in points.windows(2) {
                self.target.DrawLine(
                    Vector2 {
                        X: pair[0].0 as f32 * scale,
                        Y: pair[0].1 as f32 * scale,
                    },
                    Vector2 {
                        X: pair[1].0 as f32 * scale,
                        Y: pair[1].1 as f32 * scale,
                    },
                    &brush,
                    5.0 * scale,
                    None,
                );
            }
        }
        Ok(())
    }
    pub fn pin(
        &self,
        point: (f64, f64),
        index: usize,
        scale: f32,
        alpha: f32,
    ) -> windows::core::Result<()> {
        unsafe {
            let brush = self.target.CreateSolidColorBrush(&color(alpha), None)?;
            let center = Vector2 {
                X: point.0 as f32 * scale,
                Y: point.1 as f32 * scale,
            };
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: center,
                    radiusX: 16.0 * scale,
                    radiusY: 16.0 * scale,
                },
                &brush,
                3.0 * scale,
                None,
            );
            self.chip(
                (center.X + 10.0 * scale).clamp(0.0, (self.width as f32 - 32.0 * scale).max(0.0)),
                (center.Y - 32.0 * scale).clamp(0.0, (self.height as f32 - 24.0 * scale).max(0.0)),
                index,
                scale,
                alpha,
            )?;
        }
        Ok(())
    }
    pub fn chip(
        &self,
        x: f32,
        y: f32,
        index: usize,
        scale: f32,
        alpha: f32,
    ) -> windows::core::Result<()> {
        unsafe {
            let brush = self.target.CreateSolidColorBrush(&color(alpha), None)?;
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: x,
                        top: y,
                        right: x + 32.0 * scale,
                        bottom: y + 24.0 * scale,
                    },
                    radiusX: 5.0 * scale,
                    radiusY: 5.0 * scale,
                },
                &brush,
            );
            let white = self.target.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: alpha,
                },
                None,
            )?;
            let label = index.to_string().encode_utf16().collect::<Vec<_>>();
            let layout =
                self.write
                    .CreateTextLayout(&label, &self.text, 32.0 * scale, 24.0 * scale)?;
            layout.SetFontSize(
                16.0 * scale,
                DWRITE_TEXT_RANGE {
                    startPosition: 0,
                    length: label.len() as u32,
                },
            )?;
            self.target.DrawTextLayout(
                Vector2 { X: x, Y: y },
                &layout,
                &white,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
        Ok(())
    }
    pub fn finish(&self) -> windows::core::Result<()> {
        unsafe { self.target.EndDraw(None, None) }
    }
    pub fn pixels(&self) -> Result<Pixels, String> {
        Pixels::new(
            self.width,
            self.height,
            unsafe {
                std::slice::from_raw_parts(
                    self.dib.data,
                    self.width as usize * self.height as usize * 4,
                )
            }
            .to_vec(),
        )
    }
    pub fn present(&self, hwnd: HWND, origin: POINT) -> windows::core::Result<()> {
        unsafe {
            UpdateLayeredWindow(
                hwnd,
                None,
                Some(&origin),
                Some(&SIZE {
                    cx: self.width as i32,
                    cy: self.height as i32,
                }),
                Some(self.dib.dc),
                Some(&POINT::default()),
                COLORREF(0),
                Some(&BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                }),
                ULW_ALPHA,
            )
        }
    }
}
