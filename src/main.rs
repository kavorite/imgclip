#[macro_use]
extern crate lazy_static;
extern crate crossbeam_channel;
extern crate png;
extern crate reqwest;
extern crate winapi;

mod dib;
mod pin;

use crossbeam_channel::TryRecvError;
use dib::DIB;
use pin::*;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use winapi::shared::minwindef::TRUE as WIN_TRUE;
use winapi::um::winuser::*;

fn main() -> std::io::Result<()> {
    let listener = unsafe { WinMsgSink::open() }?;
    let sink = listener.sig();
    unsafe {
        loop {
            if let Ok(u_msg) = sink.try_recv() {
                if u_msg != WM_CLIPBOARDUPDATE {
                    continue;
                }
                let mut clipboard = Clipboard::open()?;
                if let Some(result) = DIB::get_clip(&mut clipboard) {
                    let dib = result?;
                    let ostrm = File::create("clip.png")?;
                    dib.encode_png(ostrm)?;
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
