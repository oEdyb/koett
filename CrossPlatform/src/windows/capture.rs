use super::pixels::{MAX_PIXELS, Pixels};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
static ACTIVE: AtomicBool = AtomicBool::new(false);
struct Active;
impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}
use windows::{
    Graphics::{
        Capture::*,
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            Dxgi::{Common::*, IDXGIDevice},
            Gdi::*,
        },
        System::WinRT::{Direct3D11::*, Graphics::Capture::IGraphicsCaptureItemInterop, *},
    },
    core::{Interface, factory},
};

const MAX_CAPTURE_PIXELS: i64 = 24_000_000;

pub(super) fn exclude_own_windows() -> Result<(), String> {
    use windows::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
    unsafe extern "system" fn exclude(hwnd: HWND, data: LPARAM) -> windows::core::BOOL {
        let error = unsafe { &mut *(data.0 as *mut Option<String>) };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == std::process::id() {
            let result = unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) }
                .and_then(|()| {
                    let mut value = 0;
                    unsafe { GetWindowDisplayAffinity(hwnd, &mut value) }?;
                    if value != WDA_EXCLUDEFROMCAPTURE.0 {
                        return Err(failure("Capture exclusion was not applied"));
                    }
                    Ok(())
                });
            if let Err(e) = result {
                *error = Some(format!("Could not exclude Koett from capture: {e}"));
                return false.into();
            }
        }
        true.into()
    }
    let mut error: Option<String> = None;
    let result = unsafe {
        EnumWindows(
            Some(exclude),
            LPARAM((&mut error as *mut Option<String>) as isize),
        )
    };
    if let Some(error) = error {
        return Err(error);
    }
    result.map_err(|e| e.to_string())
}

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}
struct Capture {
    session: GraphicsCaptureSession,
    pool: Direct3D11CaptureFramePool,
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

pub(super) fn monitor(handle: usize) -> Result<Pixels, String> {
    if ACTIVE.swap(true, Ordering::AcqRel) {
        return Err("A screen capture is already pending".into());
    }
    let _active = Active;
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|e| e.to_string())?;
    let _apartment = Apartment;
    capture(HMONITOR(handle as *mut _)).map_err(|e| format!("Screen capture failed: {e}"))
}

fn capture(monitor: HMONITOR) -> windows::core::Result<Pixels> {
    if !GraphicsCaptureSession::IsSupported()? {
        return Err(failure("Screen capture is unavailable"));
    }
    let mut device = None;
    let mut context = None;
    unsafe {
        let mut create = |driver| {
            D3D11CreateDevice(
                None,
                driver,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        create(D3D_DRIVER_TYPE_HARDWARE).or_else(|_| create(D3D_DRIVER_TYPE_WARP))?;
    }
    let device = device.ok_or_else(windows::core::Error::from_thread)?;
    let context = context.ok_or_else(windows::core::Error::from_thread)?;
    let dxgi: IDXGIDevice = device.cast()?;
    let direct: IDirect3DDevice = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }?.cast()?;
    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(monitor) }?;
    let size = item.Size()?;
    if size.Width <= 0
        || size.Height <= 0
        || i64::from(size.Width) * i64::from(size.Height) > MAX_CAPTURE_PIXELS
    {
        return Err(failure("Display exceeds the capture pixel limit"));
    }
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &direct,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        size,
    )?;
    let session = pool.CreateCaptureSession(&item)?;
    let capture = Capture { session, pool };
    capture.session.SetIsCursorCaptureEnabled(false)?;
    capture.session.StartCapture()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let frame = loop {
        if let Ok(frame) = capture.pool.TryGetNextFrame() {
            break frame;
        }
        if Instant::now() >= deadline {
            return Err(failure("Screen capture timed out"));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let result = (|| {
        if frame.ContentSize()? != size {
            return Err(failure("Display changed during capture"));
        }
        let surface = frame.Surface()?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
        let texture: ID3D11Texture2D = unsafe { access.GetInterface() }?;
        read_pixels(
            &device,
            &context,
            &texture,
            size.Width as u32,
            size.Height as u32,
        )
    })();
    let _ = frame.Close();
    result
}

fn read_pixels(
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
    texture: &ID3D11Texture2D,
    width: u32,
    height: u32,
) -> windows::core::Result<Pixels> {
    let mut staging = None;
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        ..Default::default()
    };
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) }?;
    let staging = staging.ok_or_else(windows::core::Error::from_thread)?;
    unsafe { context.CopyResource(&staging, texture) };
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }?;
    let result = (|| {
        if mapped.pData.is_null() || mapped.RowPitch < width * 4 {
            return Err(failure("Capture returned invalid pixels"));
        }
        let scale = (1600.0 / f64::from(width.max(height)))
            .min((MAX_PIXELS as f64 / (f64::from(width) * f64::from(height))).sqrt())
            .min(1.0);
        let w = (f64::from(width) * scale).floor().max(1.0) as u32;
        let h = (f64::from(height) * scale).floor().max(1.0) as u32;
        let mut pixels = vec![0; w as usize * h as usize * 4];
        for y in 0..h {
            let source_y = u64::from(y) * u64::from(height) / u64::from(h);
            for x in 0..w {
                let source_x = u64::from(x) * u64::from(width) / u64::from(w);
                let at = source_y as usize * mapped.RowPitch as usize + source_x as usize * 4;
                let to = (y as usize * w as usize + x as usize) * 4;
                let source =
                    unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>().add(at), 4) };
                pixels[to..to + 4].copy_from_slice(source);
                pixels[to + 3] = 255;
            }
        }
        Pixels::new(w, h, pixels).map_err(|e| failure(&e))
    })();
    unsafe { context.Unmap(&staging, 0) };
    result
}

fn failure(message: &str) -> windows::core::Error {
    windows::core::Error::new(windows::Win32::Foundation::E_FAIL, message)
}
