use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::{Common::*, *};
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::HDC;
use windows::core::w;

pub(super) struct Renderer {
    target: ID2D1DCRenderTarget,
    text: IDWriteTextFormat,
}

impl Renderer {
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let target = factory.CreateDCRenderTarget(&D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                ..Default::default()
            })?;
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
            Ok(Self { target, text })
        }
    }

    pub fn paint(
        &self,
        hwnd: HWND,
        dc: HDC,
        bounds: RECT,
        bars: &[f32; 64],
        elapsed: u64,
        busy: bool,
    ) -> windows::core::Result<()> {
        unsafe {
            self.target.BindDC(dc, &bounds)?;
            let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) as f32;
            self.target.SetDpi(dpi, dpi);
            let background = color(0.13, 0.12, 0.11);
            let coral = self
                .target
                .CreateSolidColorBrush(&color(1.0, 0.49, 0.42), None)?;
            let white = self
                .target
                .CreateSolidColorBrush(&color(0.98, 0.98, 0.97), None)?;
            self.target.BeginDraw();
            self.target.Clear(Some(&background));
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: rect(19.0, 20.0, 27.0, 28.0),
                    radiusX: 4.0,
                    radiusY: 4.0,
                },
                &coral,
            );
            for (index, level) in bars.iter().enumerate() {
                let height = 2.0 + level * 24.0;
                let x = 44.0 + index as f32 * 3.0;
                self.target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: rect(x, 24.0 - height / 2.0, x + 2.0, 24.0 + height / 2.0),
                        radiusX: 1.0,
                        radiusY: 1.0,
                    },
                    &coral,
                );
            }
            let label = if busy {
                "…".to_string()
            } else {
                format!("{}:{:02}", elapsed / 60, elapsed % 60)
            };
            self.target.DrawText(
                &label.encode_utf16().collect::<Vec<_>>(),
                &self.text,
                &rect(238.0, 0.0, 284.0, 48.0),
                &white,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            self.target.EndDraw(None, None)
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
