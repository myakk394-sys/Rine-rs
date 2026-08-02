#![allow(non_snake_case)]

pub type HRESULT = i32;

pub unsafe extern "system" fn CoInitializeEx(_pv_reserved: u64, _dw_co_init: u32) -> HRESULT {
    println!("[OLE32] CoInitializeEx initialized COM runtime");
    0 // S_OK
}

pub unsafe extern "system" fn CoUninitialize() {
    println!("[OLE32] CoUninitialize called");
}

#[no_mangle]
pub unsafe extern "system" fn CoCreateInstance(
    _rclsid: *const u8,
    _p_unk_outer: *mut std::ffi::c_void,
    _dw_cls_context: u32,
    _riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if !ppv.is_null() {
        let dummy_obj = libc::calloc(1, 64);
        *ppv = dummy_obj;
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "system" fn SysAllocString(psz: *const u16) -> *mut u16 {
    if psz.is_null() {
        return std::ptr::null_mut();
    }
    let mut len = 0;
    while *psz.add(len) != 0 {
        len += 1;
    }
    let total_bytes = 4 + (len + 1) * 2;
    let mem = libc::calloc(1, total_bytes) as *mut u8;
    if mem.is_null() {
        return std::ptr::null_mut();
    }
    *(mem as *mut u32) = (len * 2) as u32;
    let bstr = mem.add(4) as *mut u16;
    std::ptr::copy_nonoverlapping(psz, bstr, len + 1);
    bstr
}

#[no_mangle]
pub unsafe extern "system" fn SysFreeString(bstr: *mut u16) {
    if !bstr.is_null() {
        let mem = (bstr as *mut u8).sub(4);
        libc::free(mem as *mut std::ffi::c_void);
    }
}

#[no_mangle]
pub unsafe extern "system" fn CoTaskMemFree(pv: *mut std::ffi::c_void) {
    if !pv.is_null() {
        libc::free(pv);
    }
}

#[no_mangle]
pub unsafe extern "system" fn PropVariantClear(pvar: *mut u8) -> i32 {
    if !pvar.is_null() {
        std::ptr::write_bytes(pvar, 0, 24);
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "system" fn VariantInit(pvar: *mut u8) {
    if !pvar.is_null() {
        std::ptr::write_bytes(pvar, 0, 24);
    }
}

pub unsafe extern "system" fn ShellExecuteW(
    _hwnd: u64,
    _operation: u64,
    _file: u64,
    _parameters: u64,
    _directory: u64,
    _show_cmd: i32,
) -> u64 {
    42 // Successful instance handle (> 32)
}

#[no_mangle]
pub unsafe extern "system" fn rine_shell_folder_stub(
    _hwnd: *mut std::ffi::c_void,
    _csidl: i32,
    _h_token: *mut std::ffi::c_void,
    _flags: u32,
    psz_path: *mut u16,
) -> i32 {
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        std::ptr::copy_nonoverlapping(path.as_ptr(), psz_path, path.len());
    }
    println!("[SHELL32 ENGINE] rine_shell_folder_stub: Handled Shell path request with 'C:\\Users\\fizzy\\AppData\\Roaming'");
    0 // S_OK
}

pub unsafe extern "system" fn SHGetFolderPathW(
    _hwnd: u64,
    _csidl: i32,
    _h_token: u64,
    _dw_flags: u32,
    psz_path: *mut u16,
) -> i32 {
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming";
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        std::ptr::copy_nonoverlapping(wide.as_ptr(), psz_path, wide.len());
    }
    println!("[SHELL32 ENGINE] SHGetFolderPathW: Returned AppData directory 'C:\\Users\\fizzy\\AppData\\Roaming'");
    0 // S_OK
}

pub unsafe extern "system" fn SHGetFolderPathA(
    _hwnd: u64,
    _csidl: i32,
    _h_token: u64,
    _dw_flags: u32,
    psz_path: *mut u8,
) -> i32 {
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming\0";
        std::ptr::copy_nonoverlapping(path.as_ptr(), psz_path, path.len());
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "system" fn CoTaskMemAlloc(cb: usize) -> *mut std::ffi::c_void {
    let size = if cb < 16 { 16 } else { (cb + 15) & !15 };
    let ptr = libc::calloc(1, size);
    if ptr.is_null() {
        static FALLBACK_COTASK: [u8; 4096] = [0; 4096];
        FALLBACK_COTASK.as_ptr() as *mut std::ffi::c_void
    } else {
        ptr
    }
}

pub unsafe extern "system" fn SHGetKnownFolderPath(
    _rfid: u64,
    _dw_flags: u32,
    _h_token: u64,
    ppsz_path: *mut *mut u16,
) -> i32 {
    if !ppsz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming";
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        let buf = CoTaskMemAlloc(bytes) as *mut u16;
        if !buf.is_null() {
            std::ptr::copy_nonoverlapping(wide.as_ptr(), buf, wide.len());
            *ppsz_path = buf;
        }
    }
    0 // S_OK
}

pub unsafe extern "system" fn SHGetSpecialFolderPathW(
    _hwnd: u64,
    psz_path: *mut u16,
    _csidl: i32,
    _f_create: i32,
) -> i32 {
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming";
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        std::ptr::copy_nonoverlapping(wide.as_ptr(), psz_path, wide.len());
    }
    1 // TRUE
}

pub unsafe extern "system" fn SHGetSpecialFolderPathA(
    _hwnd: u64,
    psz_path: *mut u8,
    _csidl: i32,
    _f_create: i32,
) -> i32 {
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming\0";
        std::ptr::copy_nonoverlapping(path.as_ptr(), psz_path, path.len());
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "system" fn CommandLineToArgvW(
    _lp_cmd_line: *const u16,
    p_num_args: *mut i32,
) -> *mut *mut u16 {
    if !p_num_args.is_null() {
        *p_num_args = 1;
    }
    let ptr_array = libc::calloc(2, std::mem::size_of::<*mut u16>()) as *mut *mut u16;
    let arg0 = "That's not my neighbor.exe\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let buf = libc::calloc(arg0.len(), 2) as *mut u16;
    std::ptr::copy_nonoverlapping(arg0.as_ptr(), buf, arg0.len());
    *ptr_array = buf;
    ptr_array
}
