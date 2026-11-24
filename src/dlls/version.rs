#![allow(non_snake_case)]

pub unsafe extern "win64" fn GetFileVersionInfoSizeW(
    _lptstrFilename: *const u16,
    lpdwHandle: *mut u32,
) -> u32 {
    if !lpdwHandle.is_null() {
        *lpdwHandle = 0;
    }
    0
}

pub unsafe extern "win64" fn GetFileVersionInfoSizeA(
    _lptstrFilename: *const libc::c_char,
    lpdwHandle: *mut u32,
) -> u32 {
    if !lpdwHandle.is_null() {
        *lpdwHandle = 0;
    }
    0
}

pub unsafe extern "win64" fn GetFileVersionInfoW(
    _lptstrFilename: *const u16,
    _dwHandle: u32,
    _dwLen: u32,
    _lpData: *mut libc::c_void,
) -> i32 {
    0
}

pub unsafe extern "win64" fn GetFileVersionInfoA(
    _lptstrFilename: *const libc::c_char,
    _dwHandle: u32,
    _dwLen: u32,
    _lpData: *mut libc::c_void,
) -> i32 {
    0
}

pub unsafe extern "win64" fn VerQueryValueW(
    _pBlock: *const libc::c_void,
    _lpSubBlock: *const u16,
    lplpBuffer: *mut *mut libc::c_void,
    puLen: *mut u32,
) -> i32 {
    if !lplpBuffer.is_null() {
        *lplpBuffer = std::ptr::null_mut();
    }
    if !puLen.is_null() {
        *puLen = 0;
    }
    0
}

pub unsafe extern "win64" fn VerQueryValueA(
    _pBlock: *const libc::c_void,
    _lpSubBlock: *const libc::c_char,
    lplpBuffer: *mut *mut libc::c_void,
    puLen: *mut u32,
) -> i32 {
    if !lplpBuffer.is_null() {
        *lplpBuffer = std::ptr::null_mut();
    }
    if !puLen.is_null() {
        *puLen = 0;
    }
    0
}
