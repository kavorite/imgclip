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
                let mut clipboard = Clipboard::open()?;
                if let Some(result) = DIB::unclip(&mut clipboard) {
                    let dib = result?;
                    let mut ostrm = File::create("clip.bmp")?;
                    dib.encode_to(&mut ostrm)?;
                }
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
