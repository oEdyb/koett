use windows::Win32::{
    Graphics::Imaging::*,
    System::Com::{
        CLSCTX_INPROC_SERVER, CoCreateInstance, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET,
    },
    UI::Shell::SHCreateMemStream,
};

pub(super) const MAX_PIXELS: usize = 1_560_000;

#[derive(Clone)]
pub(super) struct Pixels {
    width: u32,
    height: u32,
    bgra: Vec<u8>,
}

impl Pixels {
    pub fn new(width: u32, height: u32, bgra: Vec<u8>) -> Result<Self, String> {
        let pixels = (width as usize).checked_mul(height as usize);
        if width == 0
            || height == 0
            || !pixels.is_some_and(|n| n <= MAX_PIXELS && n * 4 == bgra.len())
        {
            return Err("Drawing image exceeds its pixel limit or has invalid dimensions".into());
        }
        Ok(Self {
            width,
            height,
            bgra,
        })
    }

    pub fn png(&self) -> Result<Vec<u8>, String> {
        self.encode()
            .map_err(|e| format!("Could not encode drawing: {e}"))
    }

    fn encode(&self) -> windows::core::Result<Vec<u8>> {
        unsafe {
            let factory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let stream = SHCreateMemStream(None).ok_or_else(windows::core::Error::from_thread)?;
            let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
            encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
            let (mut frame, mut options) = (None, None);
            encoder.CreateNewFrame(&mut frame, &mut options)?;
            let frame = frame.ok_or_else(windows::core::Error::from_thread)?;
            frame.Initialize(options.as_ref())?;
            frame.SetSize(self.width, self.height)?;
            let mut format = GUID_WICPixelFormat32bppBGRA;
            frame.SetPixelFormat(&mut format)?;
            if format != GUID_WICPixelFormat32bppBGRA {
                return Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL,
                    "PNG encoder changed pixel format",
                ));
            }
            frame.WritePixels(self.height, self.width * 4, &self.bgra)?;
            frame.Commit()?;
            encoder.Commit()?;
            let mut stat = STATSTG::default();
            stream.Stat(&mut stat, STATFLAG_NONAME)?;
            if stat.cbSize > (MAX_PIXELS * 5 + 4096) as u64 {
                return Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL,
                    "PNG exceeded image limit",
                ));
            }
            stream.Seek(0, STREAM_SEEK_SET, None)?;
            let mut data = vec![0; stat.cbSize as usize];
            let mut read = 0;
            stream
                .Read(data.as_mut_ptr().cast(), data.len() as u32, Some(&mut read))
                .ok()?;
            if read as usize != data.len() {
                return Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL,
                    "PNG stream ended early",
                ));
            }
            Ok(data)
        }
    }

    pub fn dibv5(&self) -> Vec<u8> {
        let mut dib = vec![0; 124];
        let mut put = |offset: usize, value: u32| {
            dib[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
        };
        put(0, 124);
        put(4, self.width);
        put(8, (-(self.height as i32)) as u32);
        put(12, 1 | (32 << 16));
        put(16, 3);
        put(20, self.bgra.len() as u32);
        put(40, 0x00ff0000);
        put(44, 0x0000ff00);
        put(48, 0x000000ff);
        put(52, 0xff000000);
        put(56, 0x73524742);
        put(108, 4);
        dib.extend_from_slice(&self.bgra);
        dib
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dib_keeps_bgra_top_down_and_rejects_invalid_allocations() {
        let pixels = Pixels::new(1, 2, vec![0, 0, 255, 255, 0, 255, 0, 255]).unwrap();
        let dib = pixels.dibv5();
        assert_eq!(i32::from_le_bytes(dib[8..12].try_into().unwrap()), -2);
        assert_eq!(&dib[124..], pixels.bgra);
        assert!(Pixels::new(0, 2, vec![]).is_err());
        assert!(Pixels::new(u32::MAX, u32::MAX, vec![]).is_err());
        assert!(Pixels::new(1, 2, vec![0; 4]).is_err());
    }
}
