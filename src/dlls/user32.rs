#![allow(non_snake_case)]
use crate::sys::{InputSubsystem, WinMsg, WM_NULL, WM_QUIT};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

pub type HWND = u64;

static USER32_INPUT: Mutex<Option<InputSubsystem>> = Mutex::new(None);

/// Global HWND -> SDL_Window* registry.
/// When the guest calls CreateWindowExW, we create a real SDL2 window and store its pointer here.
static HWND_REGISTRY: Mutex<Option<HashMap<u64, usize>>> = Mutex::new(None);

pub fn get_user32_input() -> std::sync::MutexGuard<'static, Option<InputSubsystem>> {
    let mut guard = USER32_INPUT.lock().unwrap();
    if guard.is_none() {
        *guard = Some(InputSubsystem::new());
    }
    guard
}

fn hwnd_registry() -> std::sync::MutexGuard<'static, Option<HashMap<u64, usize>>> {
    let mut guard = HWND_REGISTRY.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    guard
}

pub fn CreateWindowExW(
    _ex_style: u32,
    class_name: &str,
    window_name: &str,
    _style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    _parent: HWND,
    _menu: u64,
    _instance: u64,
    _param: usize,
) -> HWND {
    let actual_width = if width <= 0 { 1280 } else { width as u32 };
    let actual_height = if height <= 0 { 720 } else { height as u32 };
    let title_str = if window_name.is_empty() {
        class_name
    } else {
        window_name
    };

    let created_hwnd = unsafe {
        crate::windowing::x11::create_physical_x11_window(title_str, actual_width, actual_height)
    };

    let hwnd = if created_hwnd != 0 {
        created_hwnd
    } else {
        0x00010001u64
    };
    println!(
        "[USER32] CreateWindowExW: Class='{}', Title='{}', Geometry=({}x{})@({},{}) -> HWND=0x{:X}",
        class_name, window_name, width, height, x, y, hwnd
    );
    hwnd
}

pub fn GetMessageW(
    msg: &mut WinMsg,
    _hwnd: HWND,
    _msg_filter_min: u32,
    _msg_filter_max: u32,
) -> i32 {
    let mut input_guard = get_user32_input();
    if let Some(input) = input_guard.as_mut() {
        if let Some(m) = input.message_queue.pop_front() {
            *msg = m;
            return if m.message == WM_QUIT { 0 } else { 1 };
        }
    }

    // Default empty message
    msg.hwnd = _hwnd;
    msg.message = WM_NULL;
    1
}

pub fn DispatchMessageW(msg: &WinMsg) -> u64 {
    let _ = msg;
    0
}

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

static INIT_MSG_STEP: AtomicU32 = AtomicU32::new(0);
static GODOT_WNDPROC: AtomicU64 = AtomicU64::new(0);

pub fn register_godot_wndproc(wndproc_ptr: u64) {
    if wndproc_ptr != 0 {
        GODOT_WNDPROC.store(wndproc_ptr, Ordering::Relaxed);
        println!(
            "[USER32 ENGINE] Registered WndProc at: 0x{:016X}",
            wndproc_ptr
        );
    }
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterClassExW(lp_wcx: *const u8) -> u16 {
    if !lp_wcx.is_null() {
        let wndproc = *(lp_wcx.add(8) as *const u64);
        register_godot_wndproc(wndproc);
    }
    0x1001
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterClassExA(lp_wcx: *const u8) -> u16 {
    RegisterClassExW(lp_wcx)
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterClassW(lp_wnd_class: *const u8) -> u16 {
    if !lp_wnd_class.is_null() {
        let wndproc = *(lp_wnd_class.add(8) as *const u64);
        register_godot_wndproc(wndproc);
    }
    0x1001
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterClassA(lp_wnd_class: *const u8) -> u16 {
    RegisterClassW(lp_wnd_class)
}

/// Real implementation: reads title from UTF-16 args, creates an SDL2 window,
/// stores SDL_Window* in HWND_REGISTRY, returns it as HWND.
#[no_mangle]
pub unsafe extern "win64" fn CreateWindowExW_System(
    _dw_ex_style: u32,
    lp_class_name: *const u16,
    lp_window_name: *const u16,
    _dw_style: u32,
    _x: i32,
    _y: i32,
    n_width: i32,
    n_height: i32,
    _h_wnd_parent: u64,
    _h_menu: u64,
    _h_instance: u64,
    _lp_param: *mut std::ffi::c_void,
) -> u64 {
    // Read title from UTF-16 window name; fall back to class name
    let title = read_utf16_safe(lp_window_name)
        .or_else(|| read_utf16_safe(lp_class_name))
        .unwrap_or_else(|| "Rine Application".to_string());

    let width = if n_width <= 0 {
        1280u32
    } else {
        n_width as u32
    };
    let height = if n_height <= 0 {
        720u32
    } else {
        n_height as u32
    };

    println!(
        "[USER32 ENGINE] CreateWindowExW: title='{}', size={}x{}",
        title, width, height
    );

    // If we already have a global SDL window (created from main.rs), reuse it and update title
    let existing = crate::windowing::x11::GLOBAL_WINDOW.load(Ordering::SeqCst);
    if existing != 0 {
        // Update the window title to match what the application requests
        let sdl_handle = crate::windowing::x11::get_sdl_handle();
        if !sdl_handle.is_null() {
            type SdlSetTitleFn = unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char);
            let sym = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_SetWindowTitle")
                    .unwrap()
                    .as_ptr(),
            );
            if !sym.is_null() {
                let fn_set_title: SdlSetTitleFn = std::mem::transmute(sym);
                if let Ok(c_title) = std::ffi::CString::new(title.as_str()) {
                    fn_set_title(existing as *mut c_void, c_title.as_ptr());
                }
            }
        }
        // Ensure renderer exists
        let renderer = crate::windowing::x11::get_global_renderer();
        if renderer.is_null() {
            let sdl_h = crate::windowing::x11::get_sdl_handle();
            crate::windowing::x11::create_sdl2_renderer(sdl_h, existing as *mut c_void);
        }
        register_hwnd(existing, existing as usize);
        println!(
            "[USER32 ENGINE] Reusing existing SDL2 window as HWND=0x{:X}",
            existing
        );
        return existing;
    }

    // Create a new SDL2 window
    let (_, window_ptr) = crate::windowing::x11::init_sdl2_and_create_window(&title, width, height);
    if !window_ptr.is_null() {
        let hwnd = window_ptr as u64;
        register_hwnd(hwnd, window_ptr as usize);
        return hwnd;
    }

    // Last resort: return a sentinel value so the guest doesn't crash
    println!("[USER32 ENGINE] CreateWindowExW: failed to create window, returning sentinel HWND");
    0x00010001
}

fn register_hwnd(hwnd: u64, sdl_window_ptr: usize) {
    let mut guard = hwnd_registry();
    if let Some(ref mut map) = *guard {
        map.insert(hwnd, sdl_window_ptr);
    }
}

/// Safely reads a null-terminated UTF-16 string from a raw pointer.
unsafe fn read_utf16_safe(ptr: *const u16) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x10000 {
        return None;
    }
    // Read up to 256 chars
    let mut buf = Vec::new();
    for i in 0..256usize {
        let ch = *ptr.add(i);
        if ch == 0 {
            break;
        }
        buf.push(ch);
    }
    if buf.is_empty() {
        return None;
    }
    let s = String::from_utf16_lossy(&buf)
        .trim_matches('\0')
        .to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateWindowExA(
    dw_ex_style: u32,
    lp_class_name: *const u8,
    lp_window_name: *const u8,
    dw_style: u32,
    x: i32,
    y: i32,
    n_width: i32,
    n_height: i32,
    h_wnd_parent: u64,
    h_menu: u64,
    h_instance: u64,
    lp_param: *mut std::ffi::c_void,
) -> u64 {
    // Convert ANSI title to UTF-16 slice and call the W version
    let title_w: Vec<u16>;
    let title_w_ptr = if !lp_window_name.is_null() && (lp_window_name as usize) > 0x10000 {
        let mut chars = Vec::new();
        let mut i = 0usize;
        loop {
            let ch = *lp_window_name.add(i);
            if ch == 0 || i > 255 {
                break;
            }
            chars.push(ch as u16);
            i += 1;
        }
        chars.push(0u16);
        title_w = chars;
        title_w.as_ptr()
    } else {
        title_w = vec![0u16];
        std::ptr::null()
    };

    let class_w: Vec<u16>;
    let class_w_ptr = if !lp_class_name.is_null() && (lp_class_name as usize) > 0x10000 {
        let mut chars = Vec::new();
        let mut i = 0usize;
        loop {
            let ch = *lp_class_name.add(i);
            if ch == 0 || i > 255 {
                break;
            }
            chars.push(ch as u16);
            i += 1;
        }
        chars.push(0u16);
        class_w = chars;
        class_w.as_ptr()
    } else {
        class_w = vec![0u16];
        std::ptr::null()
    };

    CreateWindowExW_System(
        dw_ex_style,
        class_w_ptr,
        title_w_ptr,
        dw_style,
        x,
        y,
        n_width,
        n_height,
        h_wnd_parent,
        h_menu,
        h_instance,
        lp_param,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn PeekMessageW(
    lp_msg: *mut u8,
    _h_wnd: u64,
    _w_msg_filter_min: u32,
    _w_msg_filter_max: u32,
    _w_remove_msg: u32,
) -> i32 {
    if !lp_msg.is_null() {
        std::ptr::write_bytes(lp_msg, 0, 48);

        let step = INIT_MSG_STEP.fetch_add(1, Ordering::Relaxed);
        let msg_ptr = lp_msg as *mut u32;

        match step {
            0 => {
                // WM_SHOWWINDOW (0x0018)
                *msg_ptr.offset(1) = 0x0018;
                return 1;
            }
            1 => {
                // WM_ACTIVATE (0x0006)
                *msg_ptr.offset(1) = 0x0006;
                return 1;
            }
            2 => {
                // WM_PAINT (0x000F)
                *msg_ptr.offset(1) = 0x000F;
                return 1;
            }
            _ => {}
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn DispatchMessageW_System(lp_msg: *const u8) -> isize {
    if !lp_msg.is_null() && (lp_msg as usize) > 0x10000 {
        let hwnd = std::ptr::read_unaligned(lp_msg as *const u64);
        let msg = std::ptr::read_unaligned(lp_msg.add(8) as *const u32);
        let wparam = std::ptr::read_unaligned(lp_msg.add(16) as *const u64);
        let lparam = std::ptr::read_unaligned(lp_msg.add(24) as *const u64);

        let wndproc = GODOT_WNDPROC.load(Ordering::Relaxed);
        if wndproc > 0x10000 && wndproc < 0x7FFFFFFFFFFF {
            let wndproc_fn: unsafe extern "win64" fn(u64, u32, u64, u64) -> isize =
                std::mem::transmute(wndproc);
            return wndproc_fn(hwnd, msg, wparam, lparam);
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn DefWindowProcW(
    hwnd: u64,
    msg: u32,
    wparam: u64,
    lparam: u64,
) -> isize {
    let wndproc = GODOT_WNDPROC.load(Ordering::Relaxed);
    if wndproc > 0x10000 && wndproc < 0x7FFFFFFFFFFF {
        let wndproc_fn: unsafe extern "win64" fn(u64, u32, u64, u64) -> isize =
            std::mem::transmute(wndproc);
        return wndproc_fn(hwnd, msg, wparam, lparam);
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn DefWindowProcA(
    hwnd: u64,
    msg: u32,
    wparam: u64,
    lparam: u64,
) -> isize {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[no_mangle]
pub unsafe extern "win64" fn TranslateMessage(_lp_msg: *const u8) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemMetrics(n_index: i32) -> i32 {
    match n_index {
        0 => 1280, // SM_CXSCREEN
        1 => 720,  // SM_CYSCREEN
        _ => 1,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn MessageBoxA(
    _h_wnd: u64,
    lp_text: *const libc::c_char,
    lp_caption: *const libc::c_char,
    _u_type: u32,
) -> i32 {
    let caption = if !lp_caption.is_null() {
        std::ffi::CStr::from_ptr(lp_caption).to_string_lossy()
    } else {
        std::borrow::Cow::Borrowed("MessageBox")
    };
    let text = if !lp_text.is_null() {
        std::ffi::CStr::from_ptr(lp_text).to_string_lossy()
    } else {
        std::borrow::Cow::Borrowed("")
    };
    println!(
        "[GODOT MESSAGE BOX A] Caption: '{}', Text: '{}'",
        caption, text
    );
    1 // IDOK
}

#[no_mangle]
pub unsafe extern "win64" fn MessageBoxW(
    _h_wnd: u64,
    lp_text: *const u16,
    lp_caption: *const u16,
    _u_type: u32,
) -> i32 {
    let caption = if !lp_caption.is_null() {
        let mut len = 0usize;
        while *lp_caption.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(lp_caption, len))
    } else {
        "MessageBox".to_string()
    };
    let text = if !lp_text.is_null() {
        let mut len = 0usize;
        while *lp_text.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(lp_text, len))
    } else {
        "".to_string()
    };
    println!(
        "[GODOT MESSAGE BOX W] Caption: '{}', Text: '{}'",
        caption, text
    );
    1 // IDOK
}

pub const WM_PAINT: u32 = 0x000F;

pub fn run_main_window_event_loop() {
    eprintln!("[RINE EVENT PUMP] Main Thread enters SDL2 Active Render Loop. Window is persistent on Desktop.");

    let mut msg_buf = [0u64; 6];
    let msg_ptr = msg_buf.as_mut_ptr() as *mut u8;

    loop {
        unsafe {
            // 1. Pump SDL2 events to keep OS from thinking the app is frozen
            let sdl_handle = crate::windowing::x11::get_sdl_handle();
            if !sdl_handle.is_null() {
                let mut sdl_event = [0u8; 56]; // SDL_Event is max 56 bytes
                type SdlPollEventFn = unsafe extern "C" fn(*mut u8) -> i32;
                let sym = libc::dlsym(
                    sdl_handle,
                    std::ffi::CString::new("SDL_PollEvent").unwrap().as_ptr(),
                );
                if !sym.is_null() {
                    let fn_poll: SdlPollEventFn = std::mem::transmute(sym);
                    while fn_poll(sdl_event.as_mut_ptr()) != 0 {
                        // SDL_QUIT = 0x100
                        let event_type = u32::from_le_bytes([
                            sdl_event[0],
                            sdl_event[1],
                            sdl_event[2],
                            sdl_event[3],
                        ]);
                        if event_type == 0x100 {
                            eprintln!("[RINE EVENT PUMP] SDL_QUIT received, exiting gracefully.");
                            std::process::exit(0);
                        }
                    }
                }
            }

            // 2. Windows Message Pumping (fake Win32 messages for wndproc)
            let has_msg = PeekMessageW(msg_ptr, 0, 0, 0, 1);
            if has_msg != 0 {
                TranslateMessage(msg_ptr);
                DispatchMessageW_System(msg_ptr);
            }

            // 3. Dispatch WM_PAINT to registered WndProc to drive scene rendering
            let hwnd = crate::windowing::x11::get_global_window();
            let wndproc = GODOT_WNDPROC.load(Ordering::Relaxed);
            if hwnd != 0 && wndproc > 0x10000 && wndproc < 0x7FFFFFFFFFFF {
                let wndproc_fn: unsafe extern "win64" fn(u64, u32, u64, u64) -> isize =
                    std::mem::transmute(wndproc);
                wndproc_fn(hwnd, WM_PAINT, 0, 0);
            }

            // 4. Present SDL2 frame — clears to visible background colour + SDL_RenderPresent
            let display = crate::windowing::x11::get_global_display();
            if hwnd != 0 {
                crate::graphics::dxgi_vulkan::VulkanSwapChainPresenter::present_frame_to_screen(
                    display as *mut std::ffi::c_void,
                    hwnd,
                );
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_window_ex_w() {
        let hwnd = CreateWindowExW(
            0,
            "TestClass",
            "TestTitle",
            0,
            100,
            100,
            800,
            600,
            0,
            0,
            0,
            0,
        );
        assert!(hwnd != 0);
    }

    #[test]
    fn test_get_message_w() {
        let mut msg = WinMsg {
            hwnd: 0,
            message: 0,
            w_param: 0,
            l_param: 0,
            time: 0,
            pt_x: 0,
            pt_y: 0,
        };
        let res = GetMessageW(&mut msg, 0x00010001, 0, 0);
        assert_eq!(res, 1);
    }
}
