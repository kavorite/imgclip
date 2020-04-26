#[macro_use]
extern crate lazy_static;
extern crate crossbeam_channel;
extern crate image;
extern crate reqwest;
extern crate serde_derive;
extern crate winapi;

mod dib;
mod pin;

use dib::DIB;
use pin::*;
use serde_derive::Deserialize;
use winapi::um::winuser::*;

#[derive(Deserialize)]
struct Post {
    link: String,
    deletehash: String,
}

#[derive(Deserialize)]
struct Response<T> {
    success: bool,
    status: i32,
    data: Option<T>,
}

trait StrExt {
    fn popup_err(self);
}

impl StrExt for &str {
    fn popup_err(self) {
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                WStr::from(&format!("{:?}", self)).as_mut_ptr(),
                WStr::from("imgclip: error").as_mut_ptr(),
                MB_OK | MB_ICONWARNING,
            );
        }
    }
}

trait ResultExt<T, E> {
    fn popup_err(self) -> Option<T>;
}

impl<T, E: std::fmt::Debug> ResultExt<T, E> for Result<T, E> {
    fn popup_err(self) -> Option<T> {
        match self {
            Err(err) => {
                format!("{:?}", err).as_str().popup_err();
                None
            }
            Ok(rtn) => Some(rtn),
        }
    }
}

fn main() -> image::error::ImageResult<()> {
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
                    let mut body = Vec::new();
                    dib.encode_png(&mut body)?;
                    let http = reqwest::blocking::Client::new();
                    http.post("https://api.imgur.com/3/upload")
                        .body(body)
                        .header(reqwest::header::CONTENT_TYPE, "image/png")
                        .header(
                            reqwest::header::AUTHORIZATION,
                            format!("Client-ID 2ec689e7311c575"),
                        )
                        .send()
                        .and_then(|rsp| {
                            rsp.error_for_status()
                                .map(|rsp| rsp.json::<Response<Post>>())
                        })
                        .popup_err()
                        .and_then(|stat| {
                            stat.and_then(|rsp| {
                                if let Some(post) = rsp.data {
                                    clipboard
                                        .set(CF_UNICODETEXT, WStr::from(&post.link).as_bytes())
                                        .popup_err();
                                }
                                Ok(())
                            })
                            .popup_err()
                        });
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
