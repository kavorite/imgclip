use super::Clipboard;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

#[derive(Debug, Copy, Clone)]
pub(crate) enum BitsPPx {
    Binary,
    HalfByte,
    Byte,
    Short,
    Triplet,
    Word,
    Other(u16),
}

impl BitsPPx {
    pub fn from(n: u16) -> Self {
        match n {
            1 => Self::Binary,
            4 => Self::HalfByte,
            8 => Self::Byte,
            16 => Self::Short,
            24 => Self::Triplet,
            32 => Self::Word,
            _ => Self::Other(n),
        }
    }

    pub fn n(self) -> u16 {
        match self {
            Self::Binary => 1,
            Self::HalfByte => 4,
            Self::Byte => 8,
            Self::Short => 16,
            Self::Triplet => 24,
            Self::Word => 32,
            Self::Other(n) => n,
        }
    }
}

#[derive(Debug)]
pub(crate) enum DIBFmt {
    PNG,
    JPG,
    RGB(Box<[RGBQUAD]>, BitsPPx),
    Masked([RGBQUAD; 3], BitsPPx),
}

impl DIBFmt {
    pub unsafe fn from(info: &BITMAPINFO) -> Option<Self> {
        let bitc = info.bmiHeader.biBitCount;
        match info.bmiHeader.biCompression {
            BI_PNG => Some(Self::PNG),
            BI_JPEG => Some(Self::JPG),
            BI_RGB if bitc == 1 || bitc == 4 || bitc == 8 => Some(Self::RGB(
                Box::<_>::from({
                    let ptr = &info.bmiColors as *const RGBQUAD;
                    let len = info.bmiHeader.biClrUsed;
                    std::slice::from_raw_parts(ptr, len as usize)
                }),
                BitsPPx::from(info.bmiHeader.biBitCount),
            )),
            BI_BITFIELDS if bitc == 16 || bitc == 32 => Some(Self::Masked(
                *<*const _>::cast(&info.bmiColors),
                BitsPPx::from(info.bmiHeader.biBitCount),
            )),
            _ => None,
        }
    }
}

pub(crate) struct DIB {
    pub head: BITMAPFILEHEADER,
    pub info: BITMAPINFOHEADER,
    pub clrs: Option<Box<[RGBQUAD]>>,
    pub data: Box<[u8]>,
}

impl DIB {
    fn file_header(info: &BITMAPINFOHEADER, colorc: usize) -> BITMAPFILEHEADER {
        let offset = {
            std::mem::size_of::<BITMAPFILEHEADER>()
                + std::mem::size_of::<BITMAPINFOHEADER>()
                + colorc * std::mem::size_of::<RGBQUAD>()
        } as u32;
        BITMAPFILEHEADER {
            bfType: 0x4D42, // 'BM'
            bfReserved1: 0,
            bfReserved2: 0,
            bfSize: info.biSizeImage + offset,
            bfOffBits: offset,
        }
    }

    pub unsafe fn unclip(clipboard: &mut Clipboard) -> Option<std::io::Result<Self>> {
        clipboard.get(CF_DIB).map(|gptr| {
            let lock = gptr?;
            let info = lock.as_ref::<BITMAPINFO>();
            let fmt = DIBFmt::from(&info);
            let clrs = fmt
                .and_then(|fmt| match fmt {
                    DIBFmt::Masked(clrs, _) => Some(Box::<_>::from(clrs.as_ref())),
                    DIBFmt::RGB(clrs, _) => Some(clrs),
                    _ => None,
                })
                .and_then(|clrs| {
                    if clrs.len() == 0 {
                        return None;
                    }
                    return Some(clrs);
                });
            let colorc = if let Some(ref clrs) = clrs {
                clrs.len()
            } else {
                0
            };
            let head = Self::file_header(&info.bmiHeader, colorc);
            let data = Box::<_>::from({
                let ptr = (&info.bmiHeader as *const BITMAPINFOHEADER).offset(1) as *const u8;
                let len = info.bmiHeader.biSizeImage as usize;
                std::slice::from_raw_parts(ptr, len)
            });
            let info = info.bmiHeader;
            Ok(Self {
                head,
                info,
                clrs,
                data,
            })
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
        if let Some(ref clrs) = self.clrs {
            ostrm.write({
                let ptr = clrs.as_ptr() as *const _;
                let len = std::mem::size_of::<RGBQUAD>() * clrs.len();
                std::slice::from_raw_parts(ptr, len)
            })?;
        }
        ostrm.write(&self.data)?;
        Ok(())
    }
}
