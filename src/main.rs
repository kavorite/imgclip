#[macro_use]
extern crate lazy_static;
extern crate crossbeam_channel;
extern crate png;
extern crate reqwest;
extern crate winapi;
extern crate winrt_notification;

mod dib;
mod pin;

use dib::DIB;
use pin::*;
use std::fs::File;
use std::io::Write;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

fn main() -> std::io::Result<()> {
    let listener = unsafe { WinMsgSink::open() }?;
    let sink = listener.sig();
    unsafe {
        loop {
            if let Ok(msg) = sink.try_recv() {
                if msg.msg != WM_CLIPBOARDUPDATE {
                    continue;
                }
                let clipboard = Clipboard::open()?;
                if let Some(gptr) = clipboard.get(CF_DIB) {
                    let lock = gptr?;
                    let info = lock.as_ref::<BITMAPINFO>();
                    let file_info = &info.bmiHeader;
                    let file_hdr = {
                        let offset = {
                            std::mem::size_of::<BITMAPFILEHEADER>()
                                + std::mem::size_of::<BITMAPINFOHEADER>()
                        } as u32;
                        BITMAPFILEHEADER {
                            bfType: 0x4D42,
                            bfReserved1: 0,
                            bfReserved2: 0,
                            bfSize: file_info.biSizeImage + offset,
                            bfOffBits: offset as u32,
                        }
                    };
                    let mut ostrm = File::create("clip.bmp")?;
                    println!("{:?}", file_hdr);
                    println!("{:?}", file_info);
                    ostrm.write({
                        let ptr = <*const _>::cast(&file_hdr);
                        let len = std::mem::size_of::<BITMAPFILEHEADER>();
                        std::slice::from_raw_parts(ptr, len)
                    })?;
                    ostrm.write({
                        let ptr = <*const _>::cast(file_info);
                        let len = std::mem::size_of::<BITMAPINFOHEADER>();
                        std::slice::from_raw_parts(ptr, len)
                    })?;
                    ostrm.write({
                        let ptr = (file_info as *const BITMAPINFOHEADER).offset(1) as *const u8;
                        let len = file_info.biSizeImage as usize;
                        std::slice::from_raw_parts(ptr, len)
                    })?;
                }
                // let mut clipboard = Clipboard::open()?;
                // if let Some(result) = DIB::get_clip(&mut clipboard) {
                //     let dib = result?;
                //     let ostrm = File::create("clip.png")?;
                //     dib.encode_png(ostrm)?;
                // }
            }
            listener.poll()?;
        }
    }
    Ok(())
    // set clipboard
    // return unsafe {
    //     let mut clipboard = Clipboard::open()?;
    //     clipboard.set(CF_TEXT, b"this was a triumph\0")
    // };
}
