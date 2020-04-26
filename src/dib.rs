use super::Clipboard;
use image::bmp::BmpDecoder;
use image::png::PNGEncoder;
use std::ops::Deref;
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

    pub unsafe fn rgb_data(&self) -> Option<impl Deref<Target = [[u8; 3]]>> {
        match self.info.biCompression {
            // TODO: support palletized payloads (BI_BITFIELDS for depth <= 8)
            BI_BITFIELDS if self.info.biBitCount == 32 || self.info.biBitCount == 16 => {
                self.clrs.as_ref().map(|clrs| {
                    if self.info.biBitCount == 32 {
                        let n = (self.info.biWidth * self.info.biHeight) as usize;
                        let data = std::slice::from_raw_parts(self.data.as_ptr() as *const u32, n);
                        let red = u32::from_le_bytes([
                            clrs[0].rgbRed,
                            clrs[0].rgbGreen,
                            clrs[0].rgbBlue,
                            0,
                        ]);
                        let grn = u32::from_le_bytes([
                            clrs[1].rgbRed,
                            clrs[1].rgbGreen,
                            clrs[1].rgbBlue,
                            0,
                        ]);
                        let blu = u32::from_le_bytes([
                            clrs[2].rgbRed,
                            clrs[2].rgbGreen,
                            clrs[2].rgbBlue,
                            0,
                        ]);
                        data.iter()
                            .map(|px| {
                                [
                                    ((red & px) >> 24) as u8,
                                    ((grn & px) >> 16) as u8,
                                    ((blu & px) >> 8) as u8,
                                ]
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice()
                    } else {
                        let n = (self.info.biWidth * self.info.biHeight) as usize;
                        let data = std::slice::from_raw_parts(self.data.as_ptr() as *const u32, n);
                        let red = u32::from_le_bytes([
                            clrs[0].rgbRed,
                            clrs[0].rgbGreen,
                            clrs[0].rgbBlue,
                            0,
                        ]);
                        let grn = u32::from_le_bytes([
                            clrs[1].rgbRed,
                            clrs[1].rgbGreen,
                            clrs[1].rgbBlue,
                            0,
                        ]);
                        let blu = u32::from_le_bytes([
                            clrs[2].rgbRed,
                            clrs[2].rgbGreen,
                            clrs[2].rgbBlue,
                            0,
                        ]);
                        data.iter()
                            .map(|px| {
                                [
                                    ((red & px) >> 11) as u8,
                                    ((grn & px) >> 5) as u8,
                                    (blu & px) as u8,
                                ]
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice()
                    }
                })
            }
            BI_RGB if self.info.biBitCount == 32 || self.info.biBitCount == 24 => Some({
                let n = (self.info.biWidth * self.info.biHeight) as usize;
                if self.info.biBitCount == 32 {
                    let data = std::slice::from_raw_parts(self.data.as_ptr() as *const u32, n);
                    let red = 0xff000000;
                    let grn = 0x00ff0000;
                    let blu = 0x0000ff00;
                    data.iter()
                        .map(|px| {
                            [
                                ((red & px) >> 24) as u8,
                                ((grn & px) >> 16) as u8,
                                ((blu & px) >> 8) as u8,
                            ]
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice()
                } else {
                    let data = std::slice::from_raw_parts(<*const _>::cast(self.data.as_ptr()), n);
                    Box::<_>::from(data)
                }
            }),
            _ => None,
        }
    }

    pub unsafe fn unclip(clipboard: &mut Clipboard) -> Option<std::io::Result<Self>> {
        clipboard.get(CF_DIB).map(|gptr| {
            let lock = gptr?;
            let info = lock.as_ref::<BITMAPINFO>();
            let clrs = DIBFmt::from(&info)
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
                let bmi_end = (&info.bmiHeader as *const BITMAPINFOHEADER).offset(1) as *const u8;
                let ptr = bmi_end.offset((colorc * std::mem::size_of::<RGBQUAD>()) as isize);
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

    pub unsafe fn encode_bmp<O: std::io::Write>(&self, ostrm: &mut O) -> std::io::Result<()> {
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

    pub unsafe fn encode_png<O: std::io::Write>(
        &self,
        ostrm: O,
    ) -> Option<Result<(), png::EncodingError>> {
        self.rgb_data().map(|payload| {
            let mut enc = Encoder::new(ostrm, self.info.biWidth as u32, self.info.biHeight as u32);
            enc.set_color(ColorType::RGB);
            enc.set_depth(BitDepth::Eight);
            let mut ostrm = enc.write_header()?;
            let istrm = {
                let ptr = payload.as_ptr() as *const _ as *const u8;
                let slc = std::slice::from_raw_parts(ptr, payload.len() * 3);
                println!("{:?}", slc[..6]);
                slc
            };
            ostrm.write_image_data(istrm)?;
            Ok(())
        })
    }
}
