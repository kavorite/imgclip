use super::Clipboard;
use winapi::shared::minwindef::TRUE as WIN_TRUE;
use winapi::shared::ntdef::HANDLE;
use winapi::shared::windef::HBITMAP;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

pub(crate) struct DIB {
    width: u32,
    height: u32,
    data: Vec<[u8; 4]>,
}

impl DIB {
    pub unsafe fn get_bmp(clipboard: &mut Clipboard) -> Option<std::io::Result<Self>> {
        if IsClipboardFormatAvailable(CF_BITMAP) != WIN_TRUE {
            return None;
        }
        let hdl = GetClipboardData(CF_BITMAP);
        Some(|| -> std::io::Result<Self> {
            if hdl == std::ptr::null_mut() {
                return Err(std::io::Error::last_os_error());
            }
            let mut hdr = BITMAP::default();
            if GetObjectW(
                hdl as HANDLE,
                std::mem::size_of::<BITMAP> as i32,
                &mut hdr as *mut BITMAP as *mut std::ffi::c_void,
            ) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            let mut dib = {
                let (width, height) = (hdr.bmWidth as u32, hdr.bmHeight as u32);
                Self {
                    width: width,
                    height: height,
                    data: vec![[0u8; 4]; (width * height) as usize],
                }
            };
            if GetBitmapBits(
                hdl as HBITMAP,
                (dib.data.capacity() * 4) as i32,
                dib.data.as_mut_ptr() as *mut std::ffi::c_void,
            ) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            for px in dib.data.iter_mut() {
                *px = [px[2], px[1], px[0], px[3]];
            }
            Ok(dib)
        }())
    }

    pub unsafe fn get_clip(clipboard: &mut Clipboard) -> Option<std::io::Result<Self>> {
        clipboard.get(CF_DIB).map(|gptr| {
            let lock = gptr?;
            let info = lock.as_ref::<BITMAPINFO>();
            let hdr = info.bmiHeader;
            let (width, height) = (hdr.biWidth as u32, hdr.biHeight as u32);
            println!(
                "image compression = {}",
                match hdr.biCompression {
                    BI_JPEG => "jpeg",
                    BI_PNG => "png",
                    _ => "misc.",
                }
            );
            let src = {
                let ptr = (info as *const BITMAPINFO).offset(1) as *const [u8; 4];
                let slc = std::slice::from_raw_parts(ptr, (hdr.biSizeImage / 4) as usize);
                slc.iter().map(|px| [px[2], px[1], px[0], px[3]])
            };
            let mut dib = Self {
                width: width,
                height: height,
                data: Vec::with_capacity((width * height) as usize),
            };
            dib.data.extend(src);
            Ok(dib)
        })
    }

    pub fn encode_png<W: std::io::Write>(&self, dst: W) -> std::io::Result<()> {
        let mut enc = png::Encoder::new(dst, self.width, self.height);
        enc.set_color(png::ColorType::RGBA);
        enc.set_depth(png::BitDepth::Eight);
        let mut steno = enc
            .write_header()
            .map_err(|err| -> std::io::Error { err.into() })?;
        steno
            .write_image_data(unsafe {
                let ptr = self.data.as_ptr() as *const u8;
                let len = self.data.len() * 4;
                std::slice::from_raw_parts(ptr, len)
            })
            .map_err(|err| -> std::io::Error { err.into() })?;
        Ok(())
    }
}
