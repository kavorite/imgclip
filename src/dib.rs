use super::Clipboard;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

#[derive(Copy, Clone, Debug)]
pub(crate) struct RGBMask {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    reserved: u8,
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum BitsPPx {
    Binary,
    HalfByte,
    Byte,
    HalfWord,
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
            16 => Self::HalfWord,
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
            Self::HalfWord => 16,
            Self::Triplet => 24,
            Self::Word => 32,
            Self::Other(n) => n,
        }
    }
}

#[derive(Debug)]
pub(crate) enum DIBFmt {
    RGB,
    PNG,
    JPG,
    Masked(RGBMask, BitsPPx),
}

impl DIBFmt {
    pub fn from(info: &BITMAPINFO) -> Option<Self> {
        match info.bmiHeader.biCompression {
            BI_RGB => Some(Self::RGB),
            BI_PNG => Some(Self::PNG),
            BI_JPEG => Some(Self::JPG),
            BI_BITFIELDS => Some({
                Self::Masked(
                    RGBMask {
                        red: info.bmiColors[0].rgbRed,
                        green: info.bmiColors[0].rgbGreen,
                        blue: info.bmiColors[0].rgbBlue,
                        reserved: 0,
                    },
                    BitsPPx::from(info.bmiHeader.biBitCount),
                )
            }),
            _ => None,
        }
    }
}

pub(crate) struct DIB {
    pub head: BITMAPFILEHEADER,
    pub info: BITMAPINFOHEADER,
    pub data: Box<[u8]>,
}

impl DIB {
    fn file_header(info: &BITMAPINFOHEADER) -> BITMAPFILEHEADER {
        let offset =
            { std::mem::size_of::<BITMAPFILEHEADER>() + std::mem::size_of::<BITMAPINFOHEADER>() }
                as u32;
        BITMAPFILEHEADER {
            bfType: 0x4D42, // 'BM'
            bfReserved1: 0,
            bfReserved2: 0,
            bfSize: info.biSizeImage + offset,
            bfOffBits: offset as u32,
        }
    }

    pub unsafe fn unclip(clipboard: &mut Clipboard) -> Option<std::io::Result<Self>> {
        clipboard.get(CF_DIB).map(|gptr| {
            let lock = gptr?;
            let info = lock.as_ref::<BITMAPINFO>();
            let mut local_info = info.bmiHeader;
            if let Some(fmt) = DIBFmt::from(&info) {
                if let DIBFmt::Masked(_, bitsppx) = fmt {
                    match bitsppx {
                        BitsPPx::Word | BitsPPx::Triplet => {
                            // TODO: figure out why this workaround gives us bad scanlines
                            local_info.biCompression = BI_RGB;
                        }
                        _ => {}
                    }
                }
            }
            let head = Self::file_header(&info.bmiHeader);
            let data = Box::<_>::from({
                let ptr = (&info.bmiHeader as *const BITMAPINFOHEADER).offset(1) as *const u8;
                let len = info.bmiHeader.biSizeImage as usize;
                std::slice::from_raw_parts(ptr, len)
            });
            let info = local_info;
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
