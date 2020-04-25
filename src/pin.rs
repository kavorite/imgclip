use crossbeam_channel::{Receiver, Sender};
use std::os::windows::ffi::OsStrExt;
use std::time::Instant;
use winapi::shared::minwindef::*;
use winapi::shared::ntdef::HANDLE;
use winapi::shared::windef::HWND;
use winapi::shared::winerror::ERROR_SUCCESS;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winbase::*;
use winapi::um::winuser::*;

struct WStr {
    data: Vec<u16>,
}

impl WStr {
    pub fn from(src: &str) -> Self {
        Self {
            data: std::ffi::OsStr::new(src)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect(),
        }
    }

    pub fn as_slice(&self) -> &[u16] {
        unsafe {
            let start = self.data.as_ptr();
            let len = self.data.len();
            std::slice::from_raw_parts(start, len)
        }
    }
}

#[derive(Clone)]
pub(crate) struct GPtr {
    hdl: HANDLE,
    ptr: *mut std::ffi::c_void,
}

impl GPtr {
    pub unsafe fn lock(hdl: HANDLE) -> std::io::Result<Self> {
        let ptr = GlobalLock(hdl);
        if ptr == std::ptr::null_mut() {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(Self { hdl, ptr })
        }
    }

    pub unsafe fn as_slice<T>(&self) -> std::io::Result<&[T]> {
        let sz = GlobalSize(self.ptr);
        if sz == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let n = sz / std::mem::size_of::<T>();
        Ok(std::slice::from_raw_parts(self.as_ptr(), n))
    }

    pub unsafe fn as_ptr<T>(&self) -> *const T {
        self.ptr as *const T
    }

    pub unsafe fn as_ref<T>(&self) -> &T {
        &*self.as_ptr()
    }

    pub unsafe fn as_mut_ref<T>(&self) -> &mut T {
        &mut *(self.ptr as *mut T)
    }

    pub unsafe fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.ptr as *mut T
    }
}

impl Drop for GPtr {
    fn drop(&mut self) {
        unsafe {
            GlobalUnlock(self.hdl);
        }
    }
}

pub(crate) struct Clipboard;

impl Clipboard {
    pub unsafe fn open() -> std::io::Result<Self> {
        if OpenClipboard(std::ptr::null_mut()) != TRUE {
            return Err(std::io::Error::last_os_error());
        }

        Ok(Self)
    }

    pub unsafe fn fmts_available(&self) -> std::io::Result<Vec<u32>> {
        let mut k = EnumClipboardFormats(0);
        let mut formats = Vec::new();
        loop {
            k = EnumClipboardFormats(k);
            if k == 0 {
                if GetLastError() != ERROR_SUCCESS {
                    return Err(std::io::Error::last_os_error());
                } else {
                    return Ok(formats);
                }
            } else {
                formats.push(k);
            }
        }
    }

    pub unsafe fn has_fmt(&self, fmt: u32) -> bool {
        IsClipboardFormatAvailable(fmt) == TRUE
    }

    pub unsafe fn set(&mut self, fmt: u32, src: &[u8]) -> std::io::Result<()> {
        let hdl = GlobalAlloc(GMEM_MOVEABLE, src.len());
        if hdl == std::ptr::null_mut() {
            return Err(std::io::Error::last_os_error());
        }
        {
            // tightscope our global lock
            let mut dst = GPtr::lock(hdl)?;
            std::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), src.len());
        }
        if EmptyClipboard() != TRUE {
            GlobalFree(hdl);
            return Err(std::io::Error::last_os_error());
        }
        if SetClipboardData(fmt, hdl) == std::ptr::null_mut() {
            GlobalFree(hdl);
            return Err(std::io::Error::last_os_error());
        }
        // no need to free the data; it's owned by the system now
        Ok(())
    }

    pub unsafe fn get(&self, fmt: u32) -> Option<std::io::Result<GPtr>> {
        if !self.has_fmt(fmt) {
            return None;
        }
        let hdl = GetClipboardData(fmt);
        Some({
            if hdl == std::ptr::null_mut() {
                Err(std::io::Error::last_os_error())
            } else {
                GPtr::lock(hdl)
            }
        })
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

pub(crate) struct WinMsgSink {
    pub hwnd: HWND,
}

pub(crate) struct WinMsgContent {
    pub msg: UINT,
    pub w_param: WPARAM,
    pub l_param: LPARAM,
    pub time: Instant,
}

impl WinMsgContent {
    pub fn from(msg: u32, w_param: WPARAM, l_param: LPARAM) -> Self {
        let time = Instant::now();
        Self {
            msg,
            w_param,
            l_param,
            time,
        }
    }
}

struct WinMsgUpdates {
    pub tx: Sender<WinMsgContent>,
    pub rx: Receiver<WinMsgContent>,
}

impl WinMsgUpdates {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
}

lazy_static! {
    static ref MSG_UPDATES: WinMsgUpdates = WinMsgUpdates::new();
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    u_msg: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    MSG_UPDATES
        .tx
        .send(WinMsgContent::from(u_msg, w_param, l_param));
    DefWindowProcW(hwnd, u_msg, w_param, l_param)
}

struct WinClassRegistration {
    atom: ATOM,
    cls_name: WStr,
}

impl WinClassRegistration {
    pub unsafe fn pin(name: &str, cfg: WNDCLASSEXW) -> std::io::Result<Self> {
        let cls_name = WStr::from(name);
        let cls = WNDCLASSEXW {
            lpszClassName: cls_name.as_slice().as_ptr(),
            ..cfg
        };
        let atom = RegisterClassExW(&cls);
        if atom == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(Self { atom, cls_name })
        }
    }
}

impl Drop for WinClassRegistration {
    fn drop(&mut self) {
        unsafe {
            UnregisterClassW(
                self.cls_name.as_slice().as_ptr(),
                GetModuleHandleW(std::ptr::null()),
            );
        }
    }
}

impl WinMsgSink {
    unsafe fn wndcls_cfg() -> WNDCLASSEXW {
        WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: GetModuleHandleW(std::ptr::null()),
            ..Default::default()
        }
    }

    const WNDCLS_NAME: &'static str = "imgclip_msg_sink";

    pub unsafe fn open() -> std::io::Result<Self> {
        let reg = WinClassRegistration::pin(Self::WNDCLS_NAME, Self::wndcls_cfg());
        let name = WStr::from(Self::WNDCLS_NAME);
        let hwnd = CreateWindowExW(
            0,
            name.as_slice().as_ptr(),
            name.as_slice().as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        if hwnd == std::ptr::null_mut() {
            return Err(std::io::Error::last_os_error());
        }
        if AddClipboardFormatListener(hwnd) != TRUE {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { hwnd })
    }

    pub unsafe fn poll(&self) -> std::io::Result<()> {
        let mut msg = MSG::default();
        if GetMessageW(&mut msg, self.hwnd, 0, 0) == -1 {
            return Err(std::io::Error::last_os_error());
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
        Ok(())
    }

    pub fn sig(&self) -> Receiver<WinMsgContent> {
        MSG_UPDATES.rx.clone()
    }
}

impl Drop for WinMsgSink {
    fn drop(&mut self) {
        unsafe {
            RemoveClipboardFormatListener(self.hwnd);
            DestroyWindow(self.hwnd);
        }
    }
}
