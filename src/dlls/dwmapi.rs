#![allow(non_snake_case)]

pub unsafe extern "win64" fn DwmEnableBlurBehindWindow(_hwnd: u64, _blur: u64) -> i32 {
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn DwmGetWindowAttribute(
    hwnd: u64,
    attr: u32,
    pv_attr: *mut std::ffi::c_void,
    cb_attr: u32,
) -> i32 {
    println!(
        "[DWMAPI] DwmGetWindowAttribute: HWND=0x{:X}, attr={}, size={}",
        hwnd, attr, cb_attr
    );
    if !pv_attr.is_null() {
        match attr {
            1 => {
                // DWMWA_NCRENDERING_ENABLED
                if cb_attr >= 4 {
                    *(pv_attr as *mut u32) = 1;
                }
            }
            2 => {
                // DWMWA_CAPTION_BUTTON_BOUNDS
                if cb_attr >= 16 {
                    *(pv_attr as *mut crate::dlls::user32::Rect) = crate::dlls::user32::Rect {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
                }
            }
            9 => {
                // DWMWA_EXTENDED_FRAME_BOUNDS
                if cb_attr >= 16 {
                    let (x, y) = crate::windowing::x11::sdl_get_window_position();
                    let (w, h) = crate::windowing::x11::sdl_get_window_size();
                    let rect = crate::dlls::user32::Rect {
                        left: x,
                        top: y,
                        right: x + w,
                        bottom: y + h,
                    };
                    *(pv_attr as *mut crate::dlls::user32::Rect) = rect;
                }
            }
            _ => {
                if cb_attr > 0 && cb_attr <= 1024 {
                    std::ptr::write_bytes(pv_attr as *mut u8, 0, cb_attr as usize);
                }
            }
        }
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn DwmSetWindowAttribute(
    hwnd: u64,
    attr: u32,
    pv_attr: *const std::ffi::c_void,
    cb_attr: u32,
) -> i32 {
    println!(
        "[DWMAPI] DwmSetWindowAttribute: HWND=0x{:X}, attr={}, size={}",
        hwnd, attr, cb_attr
    );
    0 // S_OK
}

pub unsafe extern "win64" fn DwmIsCompositionEnabled(pf_enabled: *mut i32) -> i32 {
    if !pf_enabled.is_null() {
        *pf_enabled = 1;
    }
    0 // S_OK
}

pub unsafe extern "win64" fn DwmFlush() -> i32 {
    0 // S_OK
}
