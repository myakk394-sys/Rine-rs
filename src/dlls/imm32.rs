#![allow(non_snake_case)]

#[no_mangle]
pub unsafe extern "win64" fn ImmGetContext(_hwnd: u64) -> u64 {
    0x00080001
}

#[no_mangle]
pub unsafe extern "win64" fn ImmReleaseContext(_hwnd: u64, _himc: u64) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ImmAssociateContext(_hwnd: u64, himc: u64) -> u64 {
    himc
}

#[no_mangle]
pub unsafe extern "win64" fn ImmAssociateContextEx(_hwnd: u64, _himc: u64, _flags: u32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ImmGetCompositionStringW(
    _himc: u64,
    _index: u32,
    _buf: u64,
    _buf_len: u32,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ImmSetCompositionStringW(
    _himc: u64,
    _index: u32,
    _comp: *const libc::c_void,
    _comp_len: u32,
    _read: *const libc::c_void,
    _read_len: u32,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ImmSetCandidateWindow(_himc: u64, _cand_form: u64) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ImmSetCompositionWindow(_himc: u64, _comp_form: u64) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ImmSetOpenStatus(_himc: u64, _open: i32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ImmGetProperty(_hkl: u64, _prop: u32) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ImmCreateContext() -> u64 {
    0x00080002
}

#[no_mangle]
pub unsafe extern "win64" fn ImmNotifyIME(
    _himc: u64,
    _action: u32,
    _index: u32,
    _value: u32,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ImmDestroyContext(_himc: u64) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ImmGetCandidateListCountW(
    _himc: u64,
    list_count: *mut u32,
) -> u32 {
    if !list_count.is_null() {
        *list_count = 0;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ImmGetCandidateListW(
    _himc: u64,
    _index: u32,
    _list: *mut libc::c_void,
    _buf_len: u32,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ImmGetConversionStatus(
    _himc: u64,
    conv: *mut u32,
    sent: *mut u32,
) -> i32 {
    if !conv.is_null() {
        *conv = 0;
    }
    if !sent.is_null() {
        *sent = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ImmGetOpenStatus(_himc: u64) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ImmSetConversionStatus(
    _himc: u64,
    _conv: u32,
    _sent: u32,
) -> i32 {
    1
}
