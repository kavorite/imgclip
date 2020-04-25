use super::Clipboard;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

pub(crate) struct DIB {
    pub head: BITMAPFILEHEADER,
    pub info: BITMAPINFOHEADER,
    pub data: Box<[u8]>,
}

impl DIB {
    pub unsafe fn unclip(clipboard: &mut Clipboard) -> Option<std::io::Result<Self>> {
        clipboard.get(CF_DIB).map(|gptr| {
            let lock = gptr?;
            let info = lock.as_ref::<BITMAPINFO>();
            let head = {
                let offset = {
                    std::mem::size_of::<BITMAPFILEHEADER>()
                        + std::mem::size_of::<BITMAPINFOHEADER>()
                } as u32;
                BITMAPFILEHEADER {
                    bfType: 0x4D42, // 'BM'
                    bfReserved1: 0,
                    bfReserved2: 0,
                    bfSize: info.bmiHeader.biSizeImage + offset,
                    bfOffBits: offset as u32,
                }
            };
            let data = Box::<_>::from({
                let ptr = (&info.bmiHeader as *const BITMAPINFOHEADER).offset(1) as *const u8;
                let len = info.bmiHeader.biSizeImage as usize;
                std::slice::from_raw_parts(ptr, len)
            });
            let info = info.bmiHeader;
            Ok(Self { head, info, data })
        })
    }

    pub unsafe fn encode_to<O: std::io::Write>(&self, ostrm: &mut O) -> std::io::Result<()> {
        ostrm.write({
            let ptr = <*const _>::cast(&self.head);
            let len = std::mem::size_of::<BITMAPFILEHEADER>();
            std::slice::from_raw_parts(ptr, len)
        })?;
        ostrm.write({
            let ptr = <*const _>::cast(&self.info);
            let len = std::mem::size_of::<BITMAPINFOHEADER>();
            std::slice::from_raw_parts(ptr, len)
        })?;
        ostrm.write(&self.data)?;
        Ok(())
    }
}
