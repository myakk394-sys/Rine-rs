#![allow(non_snake_case)]

pub unsafe extern "system" fn DwmEnableBlurBehindWindow(_hwnd: u64, _blur: u64) -> i32 {
    0 // S_OK
}

pub unsafe extern "system" fn DwmGetWindowAttribute(
    _hwnd: u64,
    _attr: u32,
    _pv_attr: u64,
    _cb_attr: u32,
) -> i32 {
    0 // S_OK
}

pub unsafe extern "system" fn DwmSetWindowAttribute(
    _hwnd: u64,
    _attr: u32,
    _pv_attr: u64,
    _cb_attr: u32,
) -> i32 {
    0 // S_OK
}

pub unsafe extern "system" fn DwmIsCompositionEnabled(pf_enabled: *mut i32) -> i32 {
    if !pf_enabled.is_null() {
        *pf_enabled = 1;
    }
    0 // S_OK
}

pub unsafe extern "system" fn DwmFlush() -> i32 {
    0 // S_OK
}
