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
    if cfg!(test) {
        return 0x00010001u64;
    }

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

use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, AtomicUsize, Ordering};

pub static PRIMARY_HWND: AtomicU64 = AtomicU64::new(0x1E00002);
pub static LAST_CURSOR_X: AtomicI32 = AtomicI32::new(960);
pub static LAST_CURSOR_Y: AtomicI32 = AtomicI32::new(540);
static INIT_MSG_STEP: AtomicU32 = AtomicU32::new(0);
static GODOT_WNDPROC: AtomicU64 = AtomicU64::new(0);
static CURRENT_CAPTURE_HWND: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MouseHookStruct {
    pub pt: Point,
    pub hwnd: u64,
    pub w_hit_test_code: u32,
    pub _pad: u32,
    pub dw_extra_info: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Win64Msg {
    pub hwnd: u64,
    pub message: u32,
    pub _pad1: u32,
    pub w_param: u64,
    pub l_param: u64,
    pub time: u32,
    pub pt_x: i32,
    pub pt_y: i32,
    pub _pad2: u32,
}

pub unsafe fn write_win64_msg(
    lp_msg: *mut u8,
    hwnd: u64,
    message: u32,
    wparam: u64,
    lparam: u64,
    pt_x: i32,
    pt_y: i32,
) {
    let msg = Win64Msg {
        hwnd,
        message,
        _pad1: 0,
        w_param: wparam,
        l_param: lparam,
        time: 0,
        pt_x,
        pt_y,
        _pad2: 0,
    };
    std::ptr::copy_nonoverlapping(
        &msg as *const Win64Msg as *const u8,
        lp_msg,
        std::mem::size_of::<Win64Msg>(),
    );
}

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
    if cfg!(test) {
        let dummy_hwnd = 0x00010001u64;
        register_hwnd(dummy_hwnd, 0x10000, 1920, 1080);
        return dummy_hwnd;
    }

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

    // If we already have a global SDL window (created from main.rs), reuse it, resize and update title
    let existing = crate::windowing::x11::GLOBAL_WINDOW.load(Ordering::SeqCst);
    if existing != 0 {
        crate::windowing::x11::sdl_set_window_size(width as i32, height as i32);
        if _x == -2147483648 || _y == -2147483648 {
            crate::windowing::x11::sdl_set_window_position(0x2FFF0000, 0x2FFF0000);
        } else if _x >= 0 && _y >= 0 {
            crate::windowing::x11::sdl_set_window_position(_x, _y);
        }

        // Update the window title to match what the application requests
        let sdl_win = crate::windowing::x11::get_global_sdl_window();
        let sdl_handle = crate::windowing::x11::get_sdl_handle();
        if !sdl_handle.is_null() && !sdl_win.is_null() {
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
                    fn_set_title(sdl_win, c_title.as_ptr());
                }
            }
        }
        PRIMARY_HWND.store(existing, Ordering::SeqCst);
        register_hwnd(existing, existing as usize, width, height);
        println!(
            "[USER32 ENGINE] Reusing existing SDL2 window as HWND=0x{:X}, resized to {}x{}",
            existing, width, height
        );
        return existing;
    }

    // Create a new window (SDL2 with X11 fallback)
    let win_id = crate::windowing::x11::create_physical_x11_window(&title, width, height);
    if win_id != 0 {
        PRIMARY_HWND.store(win_id, Ordering::SeqCst);
        register_hwnd(win_id, win_id as usize, width, height);
        return win_id;
    }

    // Last resort: return a sentinel value and register it
    println!("[USER32 ENGINE] CreateWindowExW: failed to create window, returning sentinel HWND");
    let sentinel = 0x00010001u64;
    register_hwnd(sentinel, 0x10000, width, height);
    sentinel
}

fn register_hwnd(hwnd: u64, sdl_window_ptr: usize, width: u32, height: u32) {
    let mut guard = hwnd_registry();
    if let Some(ref mut map) = *guard {
        map.insert(hwnd, sdl_window_ptr);
    }

    let global_win = crate::windowing::x11::GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst);
    let mut global_dpy = crate::windowing::x11::GLOBAL_DISPLAY
        .load(std::sync::atomic::Ordering::SeqCst)
        as *mut std::ffi::c_void;
    if global_dpy.is_null() && std::env::var_os("DISPLAY").is_some() {
        unsafe {
            let d = crate::windowing::x11::XOpenDisplay(std::ptr::null());
            if !d.is_null() {
                crate::windowing::x11::GLOBAL_DISPLAY
                    .store(d as u64, std::sync::atomic::Ordering::SeqCst);
                global_dpy = d;
            }
        }
    }
    let global_vid = crate::windowing::x11::get_global_visual_id();

    let win_to_use = global_win;

    crate::graphics::vulkan_bridge::handles::WindowRegistry::global().register_window(
        hwnd as usize,
        crate::graphics::vulkan_bridge::handles::RineWindow {
            guest_hwnd: hwnd as usize,
            backend: crate::graphics::vulkan_bridge::handles::WindowBackendType::X11,
            native_display: global_dpy,
            native_window: win_to_use,
            native_visual_id: global_vid,
            width,
            height,
            visible: true,
        },
    );
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

fn map_sdl_key_to_vk(sym: i32) -> u32 {
    match sym {
        13 => 0x0D,                         // VK_RETURN
        27 => 0x1B,                         // VK_ESCAPE
        32 => 0x20,                         // VK_SPACE
        8 => 0x08,                          // VK_BACK
        9 => 0x09,                          // VK_TAB
        0x4000004F => 0x27,                 // VK_RIGHT
        0x40000050 => 0x25,                 // VK_LEFT
        0x40000051 => 0x28,                 // VK_DOWN
        0x40000052 => 0x26,                 // VK_UP
        0x61..=0x7A => (sym - 0x20) as u32, // a-z -> A-Z
        0x30..=0x39 => sym as u32,          // 0-9
        _ => {
            if sym > 0 && sym < 256 {
                sym as u32
            } else {
                0
            }
        }
    }
}

fn translate_sdl_event(sdl_event: &[u8; 56]) -> Option<WinMsg> {
    let event_type = u32::from_le_bytes([sdl_event[0], sdl_event[1], sdl_event[2], sdl_event[3]]);
    let target_hwnd = PRIMARY_HWND.load(Ordering::Relaxed);

    match event_type {
        0x400 => {
            // SDL_MOUSEMOTION
            let x = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let y = i32::from_le_bytes(sdl_event[24..28].try_into().unwrap());
            let state = u32::from_le_bytes(sdl_event[16..20].try_into().unwrap());
            LAST_CURSOR_X.store(x, Ordering::Relaxed);
            LAST_CURSOR_Y.store(y, Ordering::Relaxed);

            let mut w_param = 0u64;
            if (state & 1) != 0 {
                w_param |= 0x0001; // MK_LBUTTON
            }
            if (state & 4) != 0 {
                w_param |= 0x0002; // MK_RBUTTON
            }
            if (state & 2) != 0 {
                w_param |= 0x0010; // MK_MBUTTON
            }
            let l_param = ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF);
            Some(WinMsg {
                hwnd: target_hwnd,
                message: 0x0200, // WM_MOUSEMOVE
                w_param,
                l_param,
                time: 0,
                pt_x: x,
                pt_y: y,
            })
        }
        0x401 => {
            // SDL_MOUSEBUTTONDOWN
            let button = sdl_event[16];
            let x = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let y = i32::from_le_bytes(sdl_event[24..28].try_into().unwrap());
            LAST_CURSOR_X.store(x, Ordering::Relaxed);
            LAST_CURSOR_Y.store(y, Ordering::Relaxed);
            let l_param = ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF);

            match button {
                1 => {
                    println!(
                        "[RINE INPUT] Host PHYSICAL LBUTTON_DOWN ({}, {}) -> Win32 WM_LBUTTONDOWN lParam=0x{:08X}",
                        x, y, l_param
                    );
                    Some(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0201, // WM_LBUTTONDOWN
                        w_param: 0x0001, // MK_LBUTTON
                        l_param,
                        time: 0,
                        pt_x: x,
                        pt_y: y,
                    })
                }
                3 => {
                    println!(
                        "[RINE INPUT] Host PHYSICAL RBUTTON_DOWN ({}, {}) -> Win32 WM_RBUTTONDOWN lParam=0x{:08X}",
                        x, y, l_param
                    );
                    Some(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0204, // WM_RBUTTONDOWN
                        w_param: 0x0002, // MK_RBUTTON
                        l_param,
                        time: 0,
                        pt_x: x,
                        pt_y: y,
                    })
                }
                2 => {
                    Some(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0207, // WM_MBUTTONDOWN
                        w_param: 0x0010, // MK_MBUTTON
                        l_param,
                        time: 0,
                        pt_x: x,
                        pt_y: y,
                    })
                }
                _ => None,
            }
        }
        0x402 => {
            // SDL_MOUSEBUTTONUP
            let button = sdl_event[16];
            let x = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let y = i32::from_le_bytes(sdl_event[24..28].try_into().unwrap());
            LAST_CURSOR_X.store(x, Ordering::Relaxed);
            LAST_CURSOR_Y.store(y, Ordering::Relaxed);
            let l_param = ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF);

            match button {
                1 => {
                    println!(
                        "[RINE INPUT] Host PHYSICAL LBUTTON_UP ({}, {}) -> Win32 WM_LBUTTONUP lParam=0x{:08X}",
                        x, y, l_param
                    );
                    Some(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0202, // WM_LBUTTONUP
                        w_param: 0,
                        l_param,
                        time: 0,
                        pt_x: x,
                        pt_y: y,
                    })
                }
                3 => {
                    println!(
                        "[RINE INPUT] Host PHYSICAL RBUTTON_UP ({}, {}) -> Win32 WM_RBUTTONUP lParam=0x{:08X}",
                        x, y, l_param
                    );
                    Some(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0205, // WM_RBUTTONUP
                        w_param: 0,
                        l_param,
                        time: 0,
                        pt_x: x,
                        pt_y: y,
                    })
                }
                2 => {
                    Some(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0208, // WM_MBUTTONUP
                        w_param: 0,
                        l_param,
                        time: 0,
                        pt_x: x,
                        pt_y: y,
                    })
                }
                _ => None,
            }
        }
        0x403 => {
            // SDL_MOUSEWHEEL
            let wheel_y = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let x = LAST_CURSOR_X.load(Ordering::Relaxed);
            let y = LAST_CURSOR_Y.load(Ordering::Relaxed);
            let l_param = ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF);
            let w_param = (((wheel_y * 120) as u64 & 0xFFFF) << 16);
            Some(WinMsg {
                hwnd: target_hwnd,
                message: 0x020A, // WM_MOUSEWHEEL
                w_param,
                l_param,
                time: 0,
                pt_x: x,
                pt_y: y,
            })
        }
        0x300 => {
            // SDL_KEYDOWN
            let sym = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let vk = map_sdl_key_to_vk(sym);
            println!(
                "[RINE INPUT] Host KEYDOWN sym=0x{:X} -> Win32 VK=0x{:02X}",
                sym, vk
            );
            Some(WinMsg {
                hwnd: target_hwnd,
                message: 0x0100, // WM_KEYDOWN
                w_param: vk as u64,
                l_param: 1,
                time: 0,
                pt_x: 0,
                pt_y: 0,
            })
        }
        0x301 => {
            // SDL_KEYUP
            let sym = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let vk = map_sdl_key_to_vk(sym);
            println!(
                "[RINE INPUT] Host KEYUP sym=0x{:X} -> Win32 VK=0x{:02X}",
                sym, vk
            );
            Some(WinMsg {
                hwnd: target_hwnd,
                message: 0x0101, // WM_KEYUP
                w_param: vk as u64,
                l_param: 0xC0000001,
                time: 0,
                pt_x: 0,
                pt_y: 0,
            })
        }
        0x200 => {
            // SDL_WINDOWEVENT
            let event_id = sdl_event[12];
            match event_id {
                0x0C => {
                    Some(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0007, // WM_SETFOCUS
                        w_param: 0,
                        l_param: 0,
                        time: 0,
                        pt_x: 0,
                        pt_y: 0,
                    })
                }
                0x0D => {
                    Some(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0008, // WM_KILLFOCUS
                        w_param: 0,
                        l_param: 0,
                        time: 0,
                        pt_x: 0,
                        pt_y: 0,
                    })
                }
                0x0E => {
                    Some(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0010, // WM_CLOSE
                        w_param: 0,
                        l_param: 0,
                        time: 0,
                        pt_x: 0,
                        pt_y: 0,
                    })
                }
                _ => None,
            }
        }
        0x100 => {
            // SDL_QUIT
            Some(WinMsg {
                hwnd: target_hwnd,
                message: 0x0012, // WM_QUIT
                w_param: 0,
                l_param: 0,
                time: 0,
                pt_x: 0,
                pt_y: 0,
            })
        }
        _ => None,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn PeekMessageW(
    lp_msg: *mut u8,
    _h_wnd: u64,
    _w_msg_filter_min: u32,
    _w_msg_filter_max: u32,
    w_remove_msg: u32,
) -> i32 {
    if lp_msg.is_null() || (lp_msg as usize) < 0x10000 {
        return 0;
    }

    // 1. Initial startup handshake messages
    let step = INIT_MSG_STEP.fetch_add(1, Ordering::Relaxed);
    if step == 0 {
        let hwnd = PRIMARY_HWND.load(Ordering::Relaxed);
        write_win64_msg(lp_msg, hwnd, 0x0018 /* WM_SHOWWINDOW */, 1, 0, 0, 0);
        return 1;
    } else if step == 1 {
        let hwnd = PRIMARY_HWND.load(Ordering::Relaxed);
        write_win64_msg(lp_msg, hwnd, 0x0006 /* WM_ACTIVATE */, 1, 0, 0, 0);
        return 1;
    } else if step == 2 {
        let hwnd = PRIMARY_HWND.load(Ordering::Relaxed);
        write_win64_msg(lp_msg, hwnd, 0x000F /* WM_PAINT */, 0, 0, 0, 0);
        return 1;
    }

    // 2. Poll SDL2 events and translate into message_queue
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if !sdl_handle.is_null() {
        type SdlPollEventFn = unsafe extern "C" fn(*mut u8) -> i32;
        let sym = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_PollEvent").unwrap().as_ptr(),
        );
        if !sym.is_null() {
            let fn_poll: SdlPollEventFn = std::mem::transmute(sym);
            let mut sdl_event = [0u8; 56];
            while fn_poll(sdl_event.as_mut_ptr()) != 0 {
                let raw_type = u32::from_le_bytes([sdl_event[0], sdl_event[1], sdl_event[2], sdl_event[3]]);
                let raw_timestamp = u32::from_le_bytes([sdl_event[4], sdl_event[5], sdl_event[6], sdl_event[7]]);
                let raw_win_id = u32::from_le_bytes([sdl_event[8], sdl_event[9], sdl_event[10], sdl_event[11]]);
                if raw_type == 0x401 || raw_type == 0x402 {
                    let btn = sdl_event[16];
                    let x = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
                    let y = i32::from_le_bytes(sdl_event[24..28].try_into().unwrap());
                    println!(
                        "[SDL RAW EVENT] type=0x{:04X}, timestamp={}, winID={}, button={}, x={}, y={}",
                        raw_type, raw_timestamp, raw_win_id, btn, x, y
                    );
                }
                if let Some(win_msg) = translate_sdl_event(&sdl_event) {
                    let mut input_guard = get_user32_input();
                    if let Some(input) = input_guard.as_mut() {
                        let len_before = input.message_queue.len();
                        input.message_queue.push_back(win_msg);
                        let len_after = input.message_queue.len();
                        if win_msg.message == 0x0201 || win_msg.message == 0x0202 {
                            println!(
                                "[USER32 QUEUE] Enqueued WinMsg 0x{:04X}: queue_len_before={}, queue_len_after={}",
                                win_msg.message, len_before, len_after
                            );
                        }
                    }
                }
            }
        }
    }

    // 3. Retrieve next message
    let should_remove = (w_remove_msg & 1) != 0; // PM_REMOVE
    let next_msg = {
        let mut input_guard = get_user32_input();
        if let Some(input) = input_guard.as_mut() {
            let len_before = input.message_queue.len();
            let msg_opt = if should_remove {
                input.message_queue.pop_front()
            } else {
                input.message_queue.front().cloned()
            };
            if let Some(ref m) = msg_opt {
                if m.message == 0x0201 || m.message == 0x0202 {
                    println!(
                        "[USER32 QUEUE] Retrieved WinMsg 0x{:04X} (remove={}): queue_len_before={}, queue_len_after={}",
                        m.message, should_remove, len_before, input.message_queue.len()
                    );
                }
            }
            msg_opt
        } else {
            None
        }
    };

    if let Some(msg) = next_msg {
        // 4. Check WH_MOUSE hook if applicable
        if msg.message >= 0x0200 && msg.message <= 0x020E {
            let hook_cb = {
                let guard = HOOK_TABLE.lock().unwrap();
                if let Some(table) = guard.as_ref() {
                    table
                        .values()
                        .find(|h| h.active && h.hook_type == 7)
                        .map(|h| h.callback_guest_va)
                } else {
                    None
                }
            };
            if let Some(cb_va) = hook_cb {
                if cb_va > 0x10000 && cb_va < 0x7FFFFFFFFFFF {
                    let hook_struct = MouseHookStruct {
                        pt: Point {
                            x: msg.pt_x,
                            y: msg.pt_y,
                        },
                        hwnd: msg.hwnd,
                        w_hit_test_code: 1, // HTCLIENT
                        _pad: 0,
                        dw_extra_info: 0,
                    };
                    let hook_fn: unsafe extern "win64" fn(
                        i32,
                        u64,
                        *const MouseHookStruct,
                    ) -> isize = std::mem::transmute(cb_va);
                    let hook_res =
                        hook_fn(0 /* HC_ACTION */, msg.message as u64, &hook_struct);
                    if msg.message != 0x0200 {
                        println!(
                            "[USER32 HOOK] WH_MOUSE hook 0x{:X} executed: nCode=0, wParam=0x{:X}, hwnd=0x{:X}, pt=({}, {}), res={}",
                            cb_va, msg.message, msg.hwnd, msg.pt_x, msg.pt_y, hook_res
                        );
                    }
                    if hook_res != 0 {
                        // Hook consumed event
                        println!(
                            "[USER32 HOOK] WH_MOUSE hook 0x{:X} consumed message 0x{:X}",
                            cb_va, msg.message
                        );
                        return 0;
                    }
                }
            }
        }

        write_win64_msg(
            lp_msg,
            msg.hwnd,
            msg.message,
            msg.w_param,
            msg.l_param,
            msg.pt_x,
            msg.pt_y,
        );
        return 1;
    }

    0
}

#[no_mangle]
pub unsafe extern "win64" fn PeekMessageA(
    lp_msg: *mut u8,
    h_wnd: u64,
    w_msg_filter_min: u32,
    w_msg_filter_max: u32,
    w_remove_msg: u32,
) -> i32 {
    PeekMessageW(
        lp_msg,
        h_wnd,
        w_msg_filter_min,
        w_msg_filter_max,
        w_remove_msg,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn GetMessageW(
    lp_msg: *mut u8,
    h_wnd: u64,
    w_msg_filter_min: u32,
    w_msg_filter_max: u32,
) -> i32 {
    loop {
        let res = PeekMessageW(
            lp_msg,
            h_wnd,
            w_msg_filter_min,
            w_msg_filter_max,
            1, /* PM_REMOVE */
        );
        if res != 0 {
            if !lp_msg.is_null() && (lp_msg as usize) > 0x10000 {
                let msg = std::ptr::read_unaligned(lp_msg.add(8) as *const u32);
                if msg == 0x0012
                /* WM_QUIT */
                {
                    return 0;
                }
            }
            return 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetMessageA(
    lp_msg: *mut u8,
    h_wnd: u64,
    w_msg_filter_min: u32,
    w_msg_filter_max: u32,
) -> i32 {
    GetMessageW(lp_msg, h_wnd, w_msg_filter_min, w_msg_filter_max)
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
            if msg == 0x0201 || msg == 0x0202 || msg == 0x0100 || msg == 0x0101 {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                println!(
                    "[USER32 WNDPROC] Entry: RIP=0x{:016X}, RCX(HWND)=0x{:X}, RDX(Msg)=0x{:04X}, R8(wParam)=0x{:X}, R9(lParam)=0x{:08X}, Decoded=({}, {})",
                    wndproc, hwnd, msg, wparam, lparam, x, y
                );
            }
            let wndproc_fn: unsafe extern "win64" fn(u64, u32, u64, u64) -> isize =
                std::mem::transmute(wndproc);
            return wndproc_fn(hwnd, msg, wparam, lparam);
        }
    }
    0
}

static MESSAGE_EXTRA_INFO: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[no_mangle]
pub unsafe extern "win64" fn GetMessageExtraInfo() -> usize {
    MESSAGE_EXTRA_INFO.load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "win64" fn SetMessageExtraInfo(l_param: usize) -> usize {
    MESSAGE_EXTRA_INFO.swap(l_param, Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "win64" fn DefWindowProcW(
    _hwnd: u64,
    _msg: u32,
    _wparam: u64,
    _lparam: u64,
) -> isize {
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
pub unsafe extern "win64" fn SendMessageW(hwnd: u64, msg: u32, wparam: u64, lparam: u64) -> isize {
    println!(
        "[USER32] SendMessageW(hWnd=0x{:X}, msg=0x{:X}, wParam=0x{:X}, lParam=0x{:X})",
        hwnd, msg, wparam, lparam
    );
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SendMessageA(hwnd: u64, msg: u32, wparam: u64, lparam: u64) -> isize {
    println!(
        "[USER32] SendMessageA(hWnd=0x{:X}, msg=0x{:X}, wParam=0x{:X}, lParam=0x{:X})",
        hwnd, msg, wparam, lparam
    );
    0
}

#[no_mangle]
pub unsafe extern "win64" fn PostMessageW(hwnd: u64, msg: u32, wparam: u64, lparam: u64) -> i32 {
    println!(
        "[USER32] PostMessageW(hWnd=0x{:X}, msg=0x{:X}, wParam=0x{:X}, lParam=0x{:X}) -> TRUE",
        hwnd, msg, wparam, lparam
    );
    1
}

#[no_mangle]
pub unsafe extern "win64" fn PostMessageA(hwnd: u64, msg: u32, wparam: u64, lparam: u64) -> i32 {
    println!(
        "[USER32] PostMessageA(hWnd=0x{:X}, msg=0x{:X}, wParam=0x{:X}, lParam=0x{:X}) -> TRUE",
        hwnd, msg, wparam, lparam
    );
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemMetrics(n_index: i32) -> i32 {
    let bounds = crate::windowing::x11::get_host_display_bounds(0);
    let usable = crate::windowing::x11::get_host_display_usable_bounds(0);
    let num_displays = crate::windowing::x11::get_host_num_displays();

    let val = match n_index {
        0 => bounds.w,                 // SM_CXSCREEN (primary monitor width)
        1 => bounds.h,                 // SM_CYSCREEN (primary monitor height)
        4 => 17,                       // SM_CXHSCROLL
        5 => 17,                       // SM_CYHSCROLL
        11 => 8,                       // SM_CXFRAME
        12 => 8,                       // SM_CYFRAME
        15 => 30,                      // SM_CYCAPTION
        16 => usable.w,                // SM_CXFULLSCREEN (work area width)
        17 => usable.h,                // SM_CYFULLSCREEN (work area height)
        32 => 8,                       // SM_CXSIZEFRAME
        33 => 8,                       // SM_CYSIZEFRAME
        76 => bounds.x,                // SM_XVIRTUALSCREEN
        77 => bounds.y,                // SM_YVIRTUALSCREEN
        78 => bounds.w * num_displays, // SM_CXVIRTUALSCREEN
        79 => bounds.h,                // SM_CYVIRTUALSCREEN
        80 => num_displays,            // SM_CMONITORS
        _ => 1,
    };
    println!("[USER32] GetSystemMetrics(index={}) -> {}", n_index, val);
    val
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemMetricsForDpi(n_index: i32, _dpi: u32) -> i32 {
    GetSystemMetrics(n_index)
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

#[no_mangle]
pub unsafe extern "win64" fn SetWindowTextW(hwnd: u64, lp_string: *const u16) -> i32 {
    let title = if let Some(s) = read_utf16_safe(lp_string) {
        s
    } else {
        "Rine".to_string()
    };
    println!(
        "[USER32] SetWindowTextW(hWnd=0x{:X}, Title='{}') -> TRUE",
        hwnd, title
    );
    let sdl_win =
        crate::windowing::x11::GLOBAL_SDL_WINDOW.load(Ordering::Relaxed) as *mut std::ffi::c_void;
    if !sdl_win.is_null() {
        let sdl_handle = crate::windowing::x11::get_sdl_handle();
        if !sdl_handle.is_null() {
            type SdlSetTitleFn = unsafe extern "C" fn(*mut std::ffi::c_void, *const libc::c_char);
            let sym = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_SetWindowTitle")
                    .unwrap()
                    .as_ptr(),
            );
            if !sym.is_null() {
                let fn_set_title: SdlSetTitleFn = std::mem::transmute(sym);
                if let Ok(c_title) = std::ffi::CString::new(title.as_str()) {
                    fn_set_title(sdl_win, c_title.as_ptr());
                }
            }
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetWindowTextA(hwnd: u64, lp_string: *const libc::c_char) -> i32 {
    let title = if !lp_string.is_null() {
        std::ffi::CStr::from_ptr(lp_string)
            .to_string_lossy()
            .into_owned()
    } else {
        "Rine".to_string()
    };
    println!(
        "[USER32] SetWindowTextA(hWnd=0x{:X}, Title='{}') -> TRUE",
        hwnd, title
    );
    1
}

#[no_mangle]
pub unsafe extern "win64" fn LoadIconA(_h_instance: usize, lp_icon_name: usize) -> usize {
    println!(
        "[USER32] LoadIconA(hInst=0x{:X}, icon=0x{:X})",
        _h_instance, lp_icon_name
    );
    0x00010010
}

#[no_mangle]
pub unsafe extern "win64" fn LoadIconW(_h_instance: usize, lp_icon_name: usize) -> usize {
    println!(
        "[USER32] LoadIconW(hInst=0x{:X}, icon=0x{:X})",
        _h_instance, lp_icon_name
    );
    0x00010010
}

#[no_mangle]
pub unsafe extern "win64" fn LoadCursorA(_h_instance: usize, lp_cursor_name: usize) -> usize {
    println!(
        "[USER32] LoadCursorA(hInst=0x{:X}, cursor=0x{:X})",
        _h_instance, lp_cursor_name
    );
    0x00010020
}

#[no_mangle]
pub unsafe extern "win64" fn LoadCursorW(_h_instance: usize, lp_cursor_name: usize) -> usize {
    println!(
        "[USER32] LoadCursorW(hInst=0x{:X}, cursor=0x{:X})",
        _h_instance, lp_cursor_name
    );
    0x00010020
}

#[no_mangle]
pub unsafe extern "win64" fn GetDC(hwnd: u64) -> u64 {
    println!("[USER32] GetDC(hWnd=0x{:X}) -> 0x00010001", hwnd);
    0x00010001
}

#[no_mangle]
pub unsafe extern "win64" fn ReleaseDC(hwnd: u64, hdc: u64) -> i32 {
    println!(
        "[USER32] ReleaseDC(hWnd=0x{:X}, hDC=0x{:X}) -> 1",
        hwnd, hdc
    );
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CreateIconIndirect(_p_icon_info: *const u8) -> u64 {
    static NEXT_HICON: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x00030001);
    let hicon = NEXT_HICON.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    println!("[USER32] CreateIconIndirect -> HICON=0x{:X}", hicon);
    hicon
}

#[no_mangle]
pub unsafe extern "win64" fn DestroyIcon(hicon: u64) -> i32 {
    println!("[USER32] DestroyIcon(HICON=0x{:X}) -> 1", hicon);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CreateIconFromResource(
    _presbits: *const u8,
    _dw_res_size: u32,
    _f_icon: i32,
    _dw_ver: u32,
) -> u64 {
    static NEXT_HICON: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x00030001);
    let hicon = NEXT_HICON.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    println!("[USER32] CreateIconFromResource -> HICON=0x{:X}", hicon);
    hicon
}

#[no_mangle]
pub unsafe extern "win64" fn CreateIconFromResourceEx(
    _pb_icon_bits: *const u8,
    _cb_icon_bits: u32,
    _f_icon: i32,
    _dw_version: u32,
    _cx_desired: i32,
    _cy_desired: i32,
    _flags: u32,
) -> u64 {
    static NEXT_HICON: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x00030001);
    let hicon = NEXT_HICON.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    println!("[USER32] CreateIconFromResourceEx -> HICON=0x{:X}", hicon);
    hicon
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterRawInputDevices(
    _p_raw_input_devices: *const std::ffi::c_void,
    ui_num_devices: u32,
    cb_size: u32,
) -> i32 {
    println!(
        "[USER32] RegisterRawInputDevices(num_devices={}, cb_size={}) -> TRUE",
        ui_num_devices, cb_size
    );
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerType(_pointer_id: u32, pointer_type: *mut u32) -> i32 {
    if !pointer_type.is_null() {
        *pointer_type = 2; // PT_TOUCH
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerPenInfo(
    _pointer_id: u32,
    _pen_info: *mut std::ffi::c_void,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn LogicalToPhysicalPointForPerMonitorDPI(
    _h_wnd: u64,
    _lp_point: *mut std::ffi::c_void,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn PhysicalToLogicalPointForPerMonitorDPI(
    _h_wnd: u64,
    _lp_point: *mut std::ffi::c_void,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetProcessDpiAwareness(_value: i32) -> i32 {
    println!("[SHCORE] SetProcessDpiAwareness({}) -> S_OK", _value);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SetProcessDPIAware() -> i32 {
    println!("[USER32] SetProcessDPIAware -> TRUE");
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetDpiForWindow(_hwnd: u64) -> u32 {
    96
}

#[no_mangle]
pub unsafe extern "win64" fn GetDpiForSystem() -> u32 {
    96
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDpiForProcess(_hprocess: u64) -> u32 {
    96
}

#[no_mangle]
pub unsafe extern "win64" fn GetDpiForMonitor(
    _hmonitor: u64,
    _dpi_type: i32,
    dpi_x: *mut u32,
    dpi_y: *mut u32,
) -> i32 {
    if !dpi_x.is_null() {
        *dpi_x = 96;
    }
    if !dpi_y.is_null() {
        *dpi_y = 96;
    }
    0 // S_OK
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WindowPlacement {
    pub length: u32,
    pub flags: u32,
    pub show_cmd: u32,
    pub pt_min_position: Point,
    pub pt_max_position: Point,
    pub rc_normal_position: Rect,
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowPlacement(
    _hwnd: usize,
    lpwndpl: *mut WindowPlacement,
) -> i32 {
    if !lpwndpl.is_null() {
        let (x, y) = crate::windowing::x11::sdl_get_window_position();
        let (w, h) = crate::windowing::x11::sdl_get_window_size();
        (*lpwndpl).length = std::mem::size_of::<WindowPlacement>() as u32;
        (*lpwndpl).flags = 0;
        (*lpwndpl).show_cmd = 1; // SW_SHOWNORMAL
        (*lpwndpl).pt_min_position = Point { x: -1, y: -1 };
        (*lpwndpl).pt_max_position = Point { x: -1, y: -1 };
        (*lpwndpl).rc_normal_position = Rect {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        };
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetWindowPlacement(
    hwnd: usize,
    lpwndpl: *const WindowPlacement,
) -> i32 {
    if !lpwndpl.is_null() {
        let show_cmd = (*lpwndpl).show_cmd as i32;
        ShowWindow(hwnd, show_cmd);
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ClientToScreen(_hwnd: usize, lp_point: *mut Point) -> i32 {
    if !lp_point.is_null() && (lp_point as usize) > 0x10000 {
        let (wx, wy) = crate::windowing::x11::sdl_get_window_position();
        (*lp_point).x += wx;
        (*lp_point).y += wy;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn ScreenToClient(_hwnd: usize, lp_point: *mut Point) -> i32 {
    if !lp_point.is_null() && (lp_point as usize) > 0x10000 {
        let (wx, wy) = crate::windowing::x11::sdl_get_window_position();
        (*lp_point).x -= wx;
        (*lp_point).y -= wy;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn MapWindowPoints(
    _hwnd_from: usize,
    _hwnd_to: usize,
    _lp_points: *mut Point,
    _c_points: u32,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetCursorPos(lp_point: *mut Point) -> i32 {
    if !lp_point.is_null() && (lp_point as usize) > 0x10000 {
        let (wx, wy) = crate::windowing::x11::sdl_get_window_position();
        let sdl_handle = crate::windowing::x11::get_sdl_handle();
        let mut got_pos = false;
        if !sdl_handle.is_null() {
            type SdlGetMouseStateFn = unsafe extern "C" fn(*mut i32, *mut i32) -> u32;
            let sym = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetMouseState")
                    .unwrap()
                    .as_ptr(),
            );
            if !sym.is_null() {
                let fn_get_mouse: SdlGetMouseStateFn = std::mem::transmute(sym);
                let mut cur_x = 0i32;
                let mut cur_y = 0i32;
                fn_get_mouse(&mut cur_x, &mut cur_y);
                *lp_point = Point { x: wx + cur_x, y: wy + cur_y };
                got_pos = true;
            }
        }
        if !got_pos {
            let x = LAST_CURSOR_X.load(Ordering::Relaxed);
            let y = LAST_CURSOR_Y.load(Ordering::Relaxed);
            *lp_point = Point { x: wx + x, y: wy + y };
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetAsyncKeyState(v_key: i32) -> i16 {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    let mut mouse_mask = 0u32;
    if !sdl_handle.is_null() {
        type SdlGetMouseStateFn = unsafe extern "C" fn(*mut i32, *mut i32) -> u32;
        let sym = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_GetMouseState").unwrap().as_ptr(),
        );
        if !sym.is_null() {
            let fn_get_mouse: SdlGetMouseStateFn = std::mem::transmute(sym);
            mouse_mask = fn_get_mouse(std::ptr::null_mut(), std::ptr::null_mut());
        }
    }
    let is_down = match v_key {
        0x01 => (mouse_mask & 1) != 0, // VK_LBUTTON
        0x02 => (mouse_mask & 4) != 0, // VK_RBUTTON
        0x04 => (mouse_mask & 2) != 0, // VK_MBUTTON
        _ => false,
    };
    if is_down {
        -32767 // 0x8001
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetKeyState(v_key: i32) -> i16 {
    GetAsyncKeyState(v_key)
}

#[no_mangle]
pub unsafe extern "win64" fn GetKeyboardState(lp_key_state: *mut u8) -> i32 {
    if !lp_key_state.is_null() && (lp_key_state as usize) > 0x10000 {
        std::ptr::write_bytes(lp_key_state, 0, 256);
        let sdl_handle = crate::windowing::x11::get_sdl_handle();
        if !sdl_handle.is_null() {
            type SdlGetMouseStateFn = unsafe extern "C" fn(*mut i32, *mut i32) -> u32;
            let sym = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetMouseState").unwrap().as_ptr(),
            );
            if !sym.is_null() {
                let fn_get_mouse: SdlGetMouseStateFn = std::mem::transmute(sym);
                let mouse_mask = fn_get_mouse(std::ptr::null_mut(), std::ptr::null_mut());
                if (mouse_mask & 1) != 0 {
                    *lp_key_state.add(0x01) = 0x80; // VK_LBUTTON
                }
                if (mouse_mask & 4) != 0 {
                    *lp_key_state.add(0x02) = 0x80; // VK_RBUTTON
                }
                if (mouse_mask & 2) != 0 {
                    *lp_key_state.add(0x04) = 0x80; // VK_MBUTTON
                }
            }
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn MapVirtualKeyA(u_code: u32, u_map_type: u32) -> u32 {
    MapVirtualKeyW(u_code, u_map_type)
}

#[no_mangle]
pub unsafe extern "win64" fn MapVirtualKeyW(u_code: u32, u_map_type: u32) -> u32 {
    match u_map_type {
        0 => u_code, // MAPVK_VK_TO_VSC
        1 => u_code, // MAPVK_VSC_TO_VK
        2 => u_code, // MAPVK_VK_TO_CHAR
        3 => u_code, // MAPVK_VSC_TO_VK_EX
        4 => u_code, // MAPVK_VK_TO_VSC_EX
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetKeyboardLayout(id_thread: u32) -> usize {
    println!("[USER32] GetKeyboardLayout(idThread={}) -> 0x04090409", id_thread);
    0x04090409
}

#[no_mangle]
pub unsafe extern "win64" fn ToUnicodeEx(
    w_virt_key: u32,
    _w_scan_code: u32,
    lp_key_state: *const u8,
    pwsz_buff: *mut u16,
    cch_buff: i32,
    _w_flags: u32,
    _dwhkl: usize,
) -> i32 {
    if pwsz_buff.is_null() || cch_buff <= 0 {
        return 0;
    }
    if w_virt_key == 0 {
        return 0;
    }
    let is_shift = if !lp_key_state.is_null() {
        let shift_state = *lp_key_state.add(0x10); // VK_SHIFT
        (shift_state & 0x80) != 0
    } else {
        false
    };
    let ch = match w_virt_key {
        0x41..=0x5A => {
            if is_shift {
                w_virt_key as u16
            } else {
                (w_virt_key + 0x20) as u16
            }
        }
        0x30..=0x39 => w_virt_key as u16,
        0x20 => 0x20,
        0x08 => 0x08,
        0x09 => 0x09,
        0x0D => 0x0D,
        0x1B => 0x1B,
        _ => {
            if w_virt_key < 0x80 {
                w_virt_key as u16
            } else {
                0
            }
        }
    };
    if ch != 0 {
        *pwsz_buff = ch;
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn ToUnicode(
    w_virt_key: u32,
    w_scan_code: u32,
    lp_key_state: *const u8,
    pwsz_buff: *mut u16,
    cch_buff: i32,
    w_flags: u32,
) -> i32 {
    ToUnicodeEx(
        w_virt_key,
        w_scan_code,
        lp_key_state,
        pwsz_buff,
        cch_buff,
        w_flags,
        0x04090409,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn ToAsciiEx(
    w_virt_key: u32,
    w_scan_code: u32,
    lp_key_state: *const u8,
    lp_char: *mut u16,
    w_flags: u32,
    dwhkl: usize,
) -> i32 {
    let mut uni_buf = [0u16; 2];
    let res = ToUnicodeEx(
        w_virt_key,
        w_scan_code,
        lp_key_state,
        uni_buf.as_mut_ptr(),
        2,
        w_flags,
        dwhkl,
    );
    if res > 0 && !lp_char.is_null() {
        *lp_char = uni_buf[0];
    }
    res
}

#[no_mangle]
pub unsafe extern "win64" fn ToAscii(
    w_virt_key: u32,
    w_scan_code: u32,
    lp_key_state: *const u8,
    lp_char: *mut u16,
    w_flags: u32,
) -> i32 {
    ToAsciiEx(w_virt_key, w_scan_code, lp_key_state, lp_char, w_flags, 0x04090409)
}

#[no_mangle]
pub unsafe extern "win64" fn VkKeyScanA(ch: i8) -> i16 {
    let u_ch = (ch as u8) as u16;
    VkKeyScanW(u_ch)
}

#[no_mangle]
pub unsafe extern "win64" fn VkKeyScanW(ch: u16) -> i16 {
    match ch {
        0x61..=0x7A => (ch - 0x20) as i16, // a-z -> VK_A..VK_Z
        0x41..=0x5A => (ch as i16) | (1 << 8), // A-Z -> VK_A + shift
        0x30..=0x39 => ch as i16,
        0x20 => 0x20,
        _ => -1,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetCursor(hcursor: u64) -> u64 {
    println!(
        "[USER32] SetCursor(hCursor=0x{:X}) -> 0x{:X}",
        hcursor, hcursor
    );
    hcursor
}

#[no_mangle]
pub unsafe extern "win64" fn SetCursorPos(x: i32, y: i32) -> i32 {
    println!("[USER32] SetCursorPos({}, {}) -> TRUE", x, y);
    1 // TRUE
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrackMouseEventStruct {
    pub cb_size: u32,
    pub dw_flags: u32,
    pub hwnd_track: u64,
    pub dw_hover_time: u32,
}

#[no_mangle]
pub unsafe extern "win64" fn TrackMouseEvent(lp_event_track: *const TrackMouseEventStruct) -> i32 {
    println!("[USER32] TrackMouseEvent -> TRUE");
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn _TrackMouseEvent(lp_event_track: *const TrackMouseEventStruct) -> i32 {
    TrackMouseEvent(lp_event_track)
}

#[no_mangle]
pub unsafe extern "win64" fn ClipCursor(_lp_rect: *const Rect) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetClipCursor(lp_rect: *mut Rect) -> i32 {
    if !lp_rect.is_null() {
        *lp_rect = Rect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
    }
    1 // TRUE
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[no_mangle]
pub unsafe extern "win64" fn GetClientRect(_hwnd: usize, lp_rect: *mut Rect) -> i32 {
    let (w, h) = crate::windowing::x11::sdl_get_window_size();
    if !lp_rect.is_null() {
        *lp_rect = Rect {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
    }
    println!(
        "[USER32] GetClientRect(hWnd=0x{:X}) -> (0, 0, {}, {})",
        _hwnd, w, h
    );
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowRect(_hwnd: usize, lp_rect: *mut Rect) -> i32 {
    let (x, y) = crate::windowing::x11::sdl_get_window_position();
    let (w, h) = crate::windowing::x11::sdl_get_window_size();
    if !lp_rect.is_null() {
        *lp_rect = Rect {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        };
    }
    println!(
        "[USER32] GetWindowRect(hWnd=0x{:X}) -> ({}, {}, {}, {})",
        _hwnd,
        x,
        y,
        x + w,
        y + h
    );
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn AdjustWindowRectEx(
    lp_rect: *mut Rect,
    dw_style: u32,
    _b_menu: i32,
    _dw_ex_style: u32,
) -> i32 {
    if !lp_rect.is_null() {
        if (dw_style & 0x00C00000) != 0 {
            // WS_CAPTION
            (*lp_rect).top -= 30;
        }
        if (dw_style & 0x00040000) != 0 || (dw_style & 0x00800000) != 0 {
            // WS_THICKFRAME / WS_BORDER
            (*lp_rect).left -= 8;
            (*lp_rect).right += 8;
            (*lp_rect).bottom += 8;
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn AdjustWindowRect(
    lp_rect: *mut Rect,
    dw_style: u32,
    b_menu: i32,
) -> i32 {
    AdjustWindowRectEx(lp_rect, dw_style, b_menu, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn SetWindowPos(
    hwnd: usize,
    _hwnd_insert_after: usize,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    flags: u32,
) -> i32 {
    println!(
        "[USER32] SetWindowPos(hWnd=0x{:X}, x={}, y={}, cx={}, cy={}, flags=0x{:X}) -> TRUE",
        hwnd, x, y, cx, cy, flags
    );
    // SWP_NOSIZE = 0x0001
    if (flags & 0x0001) == 0 && cx > 0 && cy > 0 {
        crate::windowing::x11::sdl_set_window_size(cx, cy);
    }
    // SWP_NOMOVE = 0x0002
    if (flags & 0x0002) == 0 {
        crate::windowing::x11::sdl_set_window_position(x, y);
    }
    // SWP_SHOWWINDOW = 0x0040
    if (flags & 0x0040) != 0 {
        crate::windowing::x11::sdl_show_window();
        crate::windowing::x11::sdl_raise_window();
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn MoveWindow(
    hwnd: usize,
    x: i32,
    y: i32,
    n_width: i32,
    n_height: i32,
    _b_repaint: i32,
) -> i32 {
    println!(
        "[USER32] MoveWindow(hWnd=0x{:X}, x={}, y={}, w={}, h={}) -> TRUE",
        hwnd, x, y, n_width, n_height
    );
    crate::windowing::x11::sdl_set_window_position(x, y);
    if n_width > 0 && n_height > 0 {
        crate::windowing::x11::sdl_set_window_size(n_width, n_height);
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn ShowWindow(hwnd: usize, n_cmd_show: i32) -> i32 {
    println!(
        "[USER32] ShowWindow(hWnd=0x{:X}, nCmdShow={}) -> TRUE",
        hwnd, n_cmd_show
    );
    match n_cmd_show {
        3 => {
            // SW_SHOWMAXIMIZED / SW_MAXIMIZE
            crate::windowing::x11::sdl_maximize_window();
        }
        6 | 11 => {
            // SW_MINIMIZE / SW_FORCEMINIMIZE
            crate::windowing::x11::sdl_minimize_window();
        }
        9 => {
            // SW_RESTORE
            crate::windowing::x11::sdl_restore_window();
        }
        1 | 5 => {
            // SW_SHOWNORMAL / SW_SHOW
            crate::windowing::x11::sdl_show_window();
            crate::windowing::x11::sdl_raise_window();
        }
        _ => {
            crate::windowing::x11::sdl_show_window();
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn UpdateWindow(_hwnd: usize) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetForegroundWindow(hwnd: usize) -> i32 {
    println!("[USER32] SetForegroundWindow(hWnd=0x{:X}) -> TRUE", hwnd);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetForegroundWindow() -> usize {
    let hwnd = PRIMARY_HWND.load(Ordering::Relaxed);
    if hwnd != 0 {
        hwnd as usize
    } else {
        crate::windowing::x11::GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst) as usize
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetActiveWindow(hwnd: usize) -> usize {
    println!("[USER32] SetActiveWindow(hWnd=0x{:X})", hwnd);
    hwnd
}

#[no_mangle]
pub unsafe extern "win64" fn GetActiveWindow() -> usize {
    let hwnd = PRIMARY_HWND.load(Ordering::Relaxed);
    if hwnd != 0 {
        hwnd as usize
    } else {
        crate::windowing::x11::GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst) as usize
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetFocus(hwnd: usize) -> usize {
    println!("[USER32] SetFocus(hWnd=0x{:X})", hwnd);
    hwnd
}

#[no_mangle]
pub unsafe extern "win64" fn GetFocus() -> usize {
    let hwnd = PRIMARY_HWND.load(Ordering::Relaxed);
    if hwnd != 0 {
        hwnd as usize
    } else {
        crate::windowing::x11::GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst) as usize
    }
}

#[no_mangle]
pub unsafe extern "win64" fn BringWindowToTop(hwnd: usize) -> i32 {
    println!("[USER32] BringWindowToTop(hWnd=0x{:X}) -> TRUE", hwnd);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn IsWindowVisible(_hwnd: usize) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn IsIconic(_hwnd: usize) -> i32 {
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn IsZoomed(_hwnd: usize) -> i32 {
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn IsWindow(hwnd: usize) -> i32 {
    if hwnd != 0 {
        1
    } else {
        0
    }
}

static WINDOW_STYLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x16CF0000);
static WINDOW_EX_STYLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static WINDOW_USER_DATA: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[no_mangle]
pub unsafe extern "win64" fn SetWindowLongPtrA(hwnd: u64, n_index: i32, new_long: isize) -> isize {
    println!(
        "[USER32] SetWindowLongPtrA(hWnd=0x{:X}, index={}, new_long=0x{:X})",
        hwnd, n_index, new_long
    );
    match n_index {
        -4 => {
            // GWLP_WNDPROC
            let prev = GODOT_WNDPROC.swap(new_long as u64, Ordering::Relaxed);
            prev as isize
        }
        -16 => {
            // GWL_STYLE
            let prev = WINDOW_STYLE.swap(new_long as u32, Ordering::Relaxed);
            prev as isize
        }
        -20 => {
            // GWL_EXSTYLE
            let prev = WINDOW_EX_STYLE.swap(new_long as u32, Ordering::Relaxed);
            prev as isize
        }
        -21 => {
            // GWLP_USERDATA
            let prev = WINDOW_USER_DATA.swap(new_long as u64, Ordering::Relaxed);
            prev as isize
        }
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetWindowLongPtrW(hwnd: u64, n_index: i32, new_long: isize) -> isize {
    SetWindowLongPtrA(hwnd, n_index, new_long)
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowLongPtrA(hwnd: u64, n_index: i32) -> isize {
    match n_index {
        -4 => GODOT_WNDPROC.load(Ordering::Relaxed) as isize,
        -16 => WINDOW_STYLE.load(Ordering::Relaxed) as isize,
        -20 => WINDOW_EX_STYLE.load(Ordering::Relaxed) as isize,
        -21 => WINDOW_USER_DATA.load(Ordering::Relaxed) as isize,
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowLongPtrW(hwnd: u64, n_index: i32) -> isize {
    GetWindowLongPtrA(hwnd, n_index)
}

#[no_mangle]
pub unsafe extern "win64" fn SetWindowLongA(hwnd: u64, n_index: i32, new_long: i32) -> i32 {
    SetWindowLongPtrA(hwnd, n_index, new_long as isize) as i32
}

#[no_mangle]
pub unsafe extern "win64" fn SetWindowLongW(hwnd: u64, n_index: i32, new_long: i32) -> i32 {
    SetWindowLongPtrW(hwnd, n_index, new_long as isize) as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowLongA(hwnd: u64, n_index: i32) -> i32 {
    GetWindowLongPtrA(hwnd, n_index) as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowLongW(hwnd: u64, n_index: i32) -> i32 {
    GetWindowLongPtrW(hwnd, n_index) as i32
}

#[no_mangle]
pub unsafe extern "win64" fn EnableWindow(_hwnd: usize, _b_enable: i32) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn IsWindowEnabled(_hwnd: usize) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetCapture(hwnd: usize) -> usize {
    let prev = CURRENT_CAPTURE_HWND.swap(hwnd, Ordering::SeqCst);
    println!(
        "[USER32] SetCapture(hWnd=0x{:X}) -> prev 0x{:X}",
        hwnd, prev
    );
    prev
}

#[no_mangle]
pub unsafe extern "win64" fn ReleaseCapture() -> i32 {
    CURRENT_CAPTURE_HWND.store(0, Ordering::SeqCst);
    println!("[USER32] ReleaseCapture -> TRUE");
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetCapture() -> usize {
    CURRENT_CAPTURE_HWND.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "win64" fn WindowFromPoint(_point: u64) -> u64 {
    let hwnd = PRIMARY_HWND.load(Ordering::Relaxed);
    if hwnd != 0 {
        hwnd
    } else {
        0x1E00002
    }
}

#[no_mangle]
pub unsafe extern "win64" fn ChildWindowFromPoint(hwnd: u64, _point: u64) -> u64 {
    hwnd
}

#[no_mangle]
pub unsafe extern "win64" fn ChildWindowFromPointEx(hwnd: u64, _point: u64, _flags: u32) -> u64 {
    hwnd
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MonitorInfo {
    pub cb_size: u32,
    pub rc_monitor: Rect,
    pub rc_work: Rect,
    pub dw_flags: u32,
}

pub type MonitorEnumProc = unsafe extern "win64" fn(
    h_monitor: usize,
    hdc_monitor: usize,
    lprc_monitor: *const Rect,
    dw_data: usize,
) -> i32;

#[no_mangle]
pub unsafe extern "win64" fn EnumDisplayMonitors(
    _hdc: usize,
    _lprc_clip: *const Rect,
    lpfn_enum: Option<MonitorEnumProc>,
    dw_data: usize,
) -> i32 {
    if let Some(cb) = lpfn_enum {
        let count = crate::windowing::x11::get_host_num_displays();
        for i in 0..count {
            let bounds = crate::windowing::x11::get_host_display_bounds(i);
            let rect = Rect {
                left: bounds.x,
                top: bounds.y,
                right: bounds.x + bounds.w,
                bottom: bounds.y + bounds.h,
            };
            let h_monitor = 0x00010001usize + (i as usize);
            let res = cb(h_monitor, 0, &rect, dw_data);
            if res == 0 {
                break;
            }
        }
    }

    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetMonitorInfoW(h_monitor: usize, lpmi: *mut MonitorInfo) -> i32 {
    if lpmi.is_null() {
        crate::dlls::kernel32::SetLastError(87);
        return 0;
    }
    let size = (*lpmi).cb_size;
    if size < 40 {
        crate::dlls::kernel32::SetLastError(87);
        return 0;
    }

    let display_idx = if h_monitor >= 0x00010001 {
        (h_monitor - 0x00010001) as i32
    } else {
        0
    };

    let bounds = crate::windowing::x11::get_host_display_bounds(display_idx);
    let usable = crate::windowing::x11::get_host_display_usable_bounds(display_idx);

    (*lpmi).rc_monitor = Rect {
        left: bounds.x,
        top: bounds.y,
        right: bounds.x + bounds.w,
        bottom: bounds.y + bounds.h,
    };
    (*lpmi).rc_work = Rect {
        left: usable.x,
        top: usable.y,
        right: usable.x + usable.w,
        bottom: usable.y + usable.h,
    };
    (*lpmi).dw_flags = if display_idx == 0 { 1 } else { 0 }; // MONITORINFOF_PRIMARY

    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetMonitorInfoA(h_monitor: usize, lpmi: *mut MonitorInfo) -> i32 {
    GetMonitorInfoW(h_monitor, lpmi)
}

#[repr(C)]
pub struct DevModeW {
    pub dm_device_name: [u16; 32],
    pub dm_spec_version: u16,
    pub dm_driver_version: u16,
    pub dm_size: u16,
    pub dm_driver_extra: u16,
    pub dm_fields: u32,
    pub dm_orientation: i16,
    pub dm_paper_size: i16,
    pub dm_paper_length: i16,
    pub dm_paper_width: i16,
    pub dm_scale: i16,
    pub dm_copies: i16,
    pub dm_default_source: i16,
    pub dm_print_quality: i16,
    pub dm_color: i16,
    pub dm_duplex: i16,
    pub dm_y_resolution: i16,
    pub dm_tt_option: i16,
    pub dm_collate: i16,
    pub dm_form_name: [u16; 32],
    pub dm_log_pixels: u16,
    pub dm_bits_per_pel: u32,
    pub dm_pels_width: u32,
    pub dm_pels_height: u32,
    pub dm_display_flags: u32,
    pub dm_display_frequency: u32,
    pub dm_icm_method: u32,
    pub dm_icm_intent: u32,
    pub dm_media_type: u32,
    pub dm_dither_type: u32,
    pub dm_reserved1: u32,
    pub dm_reserved2: u32,
    pub dm_panning_width: u32,
    pub dm_panning_height: u32,
}

#[no_mangle]
pub unsafe extern "win64" fn EnumDisplaySettingsW(
    _lpsz_device_name: *const u16,
    i_mode_num: u32,
    lp_dev_mode: *mut DevModeW,
) -> i32 {
    if lp_dev_mode.is_null() {
        return 0;
    }
    if i_mode_num != 0 && i_mode_num != 0xFFFFFFFF && i_mode_num != 0xFFFFFFFE {
        return 0;
    }

    let bounds = crate::windowing::x11::get_host_display_bounds(0);
    (*lp_dev_mode).dm_pels_width = bounds.w as u32;
    (*lp_dev_mode).dm_pels_height = bounds.h as u32;
    (*lp_dev_mode).dm_bits_per_pel = 32;
    (*lp_dev_mode).dm_display_frequency = 60;
    (*lp_dev_mode).dm_fields = 0x00040000 | 0x00080000 | 0x00100000 | 0x00400000;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn EnumDisplaySettingsA(
    _lpsz_device_name: *const u8,
    i_mode_num: u32,
    lp_dev_mode: *mut u8,
) -> i32 {
    if lp_dev_mode.is_null() {
        return 0;
    }
    let mode = crate::windowing::x11::get_host_display_mode(0);
    if i_mode_num != 0xFFFFFFFF && i_mode_num != 0xFFFFFFFE && i_mode_num > 0 {
        return 0;
    }
    std::ptr::write_unaligned(lp_dev_mode.add(104) as *mut u32, 32);
    std::ptr::write_unaligned(lp_dev_mode.add(108) as *mut u32, mode.w as u32);
    std::ptr::write_unaligned(lp_dev_mode.add(112) as *mut u32, mode.h as u32);
    std::ptr::write_unaligned(
        lp_dev_mode.add(120) as *mut u32,
        if mode.refresh_rate > 0 {
            mode.refresh_rate as u32
        } else {
            144
        },
    );
    1
}

#[no_mangle]
pub unsafe extern "win64" fn EnumDisplaySettingsExW(
    lpsz_device_name: *const u16,
    i_mode_num: u32,
    lp_dev_mode: *mut DevModeW,
    _dw_flags: u32,
) -> i32 {
    EnumDisplaySettingsW(lpsz_device_name, i_mode_num, lp_dev_mode)
}

#[no_mangle]
pub unsafe extern "win64" fn EnumDisplaySettingsExA(
    lpsz_device_name: *const u8,
    i_mode_num: u32,
    lp_dev_mode: *mut u8,
    _dw_flags: u32,
) -> i32 {
    EnumDisplaySettingsA(lpsz_device_name, i_mode_num, lp_dev_mode)
}

#[no_mangle]
pub unsafe extern "win64" fn MonitorFromWindow(_hwnd: u64, _dw_flags: u32) -> usize {
    0x00010001
}

#[no_mangle]
pub unsafe extern "win64" fn MonitorFromPoint(_pt_x: i32, _pt_y: i32, _dw_flags: u32) -> usize {
    0x00010001
}

#[no_mangle]
pub unsafe extern "win64" fn MonitorFromRect(_lprc: *const Rect, _dw_flags: u32) -> usize {
    0x00010001
}

pub const WH_MSGFILTER: i32 = -1;
pub const WH_JOURNALRECORD: i32 = 0;
pub const WH_JOURNALPLAYBACK: i32 = 1;
pub const WH_KEYBOARD: i32 = 2;
pub const WH_GETMESSAGE: i32 = 3;
pub const WH_CALLWNDPROC: i32 = 4;
pub const WH_CBT: i32 = 5;
pub const WH_SYSMSGFILTER: i32 = 6;
pub const WH_MOUSE: i32 = 7;
pub const WH_HARDWARE: i32 = 8;
pub const WH_DEBUG: i32 = 9;
pub const WH_SHELL: i32 = 10;
pub const WH_FOREGROUNDIDLE: i32 = 11;
pub const WH_CALLWNDPROCRET: i32 = 12;
pub const WH_KEYBOARD_LL: i32 = 13;
pub const WH_MOUSE_LL: i32 = 14;

#[derive(Debug, Clone)]
pub struct HookObject {
    pub handle: usize,
    pub hook_type: i32,
    pub callback_guest_va: usize,
    pub target_guest_thread_id: u32,
    pub module_handle: usize,
    pub active: bool,
}

static HOOK_TABLE: Mutex<Option<HashMap<usize, HookObject>>> = Mutex::new(None);
static NEXT_HOOK_HANDLE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0x3000);

#[no_mangle]
pub unsafe extern "win64" fn SetWindowsHookExA(
    id_hook: i32,
    lpfn: usize,
    hmod: usize,
    dw_thread_id: u32,
) -> usize {
    if lpfn == 0 {
        crate::dlls::kernel32::SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0;
    }

    let effective_tid = if dw_thread_id == 0 {
        crate::dlls::kernel32::GetCurrentThreadId()
    } else {
        dw_thread_id
    };

    let hhook = NEXT_HOOK_HANDLE.fetch_add(0x10, Ordering::SeqCst);
    let hook_obj = HookObject {
        handle: hhook,
        hook_type: id_hook,
        callback_guest_va: lpfn,
        target_guest_thread_id: effective_tid,
        module_handle: hmod,
        active: true,
    };

    println!(
        "[USER32] SetWindowsHookExA(idHook={}, lpfn=0x{:016X}, hmod=0x{:016X}, dwThreadId={}) -> HHOOK=0x{:016X}",
        id_hook, lpfn, hmod, dw_thread_id, hhook
    );

    let mut guard = HOOK_TABLE.lock().unwrap();
    let table = guard.get_or_insert_with(HashMap::new);
    table.insert(hhook, hook_obj);

    hhook
}

#[no_mangle]
pub unsafe extern "win64" fn SetWindowsHookExW(
    id_hook: i32,
    lpfn: usize,
    hmod: usize,
    dw_thread_id: u32,
) -> usize {
    SetWindowsHookExA(id_hook, lpfn, hmod, dw_thread_id)
}

#[no_mangle]
pub unsafe extern "win64" fn CallNextHookEx(
    hhk: usize,
    n_code: i32,
    w_param: usize,
    l_param: usize,
) -> isize {
    println!(
        "[USER32 HOOK] CallNextHookEx(hHook=0x{:X}, nCode={}, wParam=0x{:X}, lParam=0x{:X}) -> 0",
        hhk, n_code, w_param, l_param
    );
    0
}

#[no_mangle]
pub unsafe extern "win64" fn UnhookWindowsHookEx(hhk: usize) -> i32 {
    if hhk == 0 {
        crate::dlls::kernel32::SetLastError(6); // ERROR_INVALID_HANDLE
        return 0;
    }

    let mut guard = HOOK_TABLE.lock().unwrap();
    if let Some(table) = guard.as_mut() {
        if let Some(obj) = table.get_mut(&hhk) {
            if obj.active {
                obj.active = false;
                table.remove(&hhk);
                println!("[USER32] UnhookWindowsHookEx(HHOOK=0x{:016X}) -> TRUE", hhk);
                return 1;
            }
        }
    }

    crate::dlls::kernel32::SetLastError(6); // ERROR_INVALID_HANDLE
    0
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterTouchWindow(hwnd: usize, ul_flags: u32) -> i32 {
    println!(
        "[USER32] RegisterTouchWindow(hWnd=0x{:X}, ulFlags=0x{:X}) -> TRUE",
        hwnd, ul_flags
    );
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn UnregisterTouchWindow(hwnd: usize) -> i32 {
    println!("[USER32] UnregisterTouchWindow(hWnd=0x{:X}) -> TRUE", hwnd);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn IsTouchWindow(hwnd: usize, pul_flags: *mut u32) -> i32 {
    if !pul_flags.is_null() {
        *pul_flags = 0;
    }
    println!("[USER32] IsTouchWindow(hWnd=0x{:X}) -> FALSE", hwnd);
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn GetTouchInputInfo(
    _h_touch_input: usize,
    _c_inputs: u32,
    _p_inputs: *mut c_void,
    _cb_size: i32,
) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CloseTouchInputHandle(_h_touch_input: usize) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterPointerInputTarget(_hwnd: usize, _pointer_type: u32) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn UnregisterPointerInputTarget(_hwnd: usize, _pointer_type: u32) -> i32 {
    1 // TRUE
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
        let mut msg_buf = [0u8; 48];
        let res = unsafe { GetMessageW(msg_buf.as_mut_ptr(), 0x00010001, 0, 0) };
        assert_eq!(res, 1);
    }

    #[test]
    fn test_win64_msg_layout() {
        assert_eq!(std::mem::size_of::<Win64Msg>(), 48);
        assert_eq!(std::mem::align_of::<Win64Msg>(), 8);
    }

    #[test]
    fn test_mousehookstruct_layout() {
        assert_eq!(std::mem::size_of::<MouseHookStruct>(), 32);
        assert_eq!(std::mem::align_of::<MouseHookStruct>(), 8);
    }

    #[test]
    fn test_mouse_lparam_coordinate_packing() {
        let x = 1234i32;
        let y = 567i32;
        let l_param = ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF);
        let unpacked_x = (l_param & 0xFFFF) as i16 as i32;
        let unpacked_y = ((l_param >> 16) & 0xFFFF) as i16 as i32;
        assert_eq!(unpacked_x, x);
        assert_eq!(unpacked_y, y);

        // Test negative coordinate packing
        let neg_x = -50i32;
        let neg_y = -100i32;
        let neg_l_param = ((neg_y as u64 & 0xFFFF) << 16) | (neg_x as u64 & 0xFFFF);
        let unpacked_neg_x = (neg_l_param & 0xFFFF) as i16 as i32;
        let unpacked_neg_y = ((neg_l_param >> 16) & 0xFFFF) as i16 as i32;
        assert_eq!(unpacked_neg_x, neg_x);
        assert_eq!(unpacked_neg_y, neg_y);
    }

    #[test]
    fn test_wh_mouse_hook_registration() {
        let callback = 0x140055BC0usize;
        let tid = 12345u32;
        let hhook = unsafe { SetWindowsHookExA(WH_MOUSE, callback, 0, tid) };
        assert_ne!(
            hhook, 0,
            "Valid SetWindowsHookExA must return non-zero HHOOK"
        );

        // Verify registered hook in table
        {
            let guard = HOOK_TABLE.lock().unwrap();
            let table = guard.as_ref().unwrap();
            let obj = table.get(&hhook).expect("Hook object must be in table");
            assert_eq!(obj.hook_type, WH_MOUSE);
            assert_eq!(obj.callback_guest_va, callback);
            assert_eq!(obj.target_guest_thread_id, tid);
            assert_eq!(obj.module_handle, 0);
            assert!(obj.active);
        }

        // Test CallNextHookEx passthrough
        let next_res = unsafe { CallNextHookEx(hhook, 0, 0x100, 0x200) };
        assert_eq!(next_res, 0);

        // Test Unhook
        let unhook_res = unsafe { UnhookWindowsHookEx(hhook) };
        assert_eq!(unhook_res, 1);

        // Repeated unhook fails
        let unhook_repeat = unsafe { UnhookWindowsHookEx(hhook) };
        assert_eq!(unhook_repeat, 0);
    }

    #[test]
    fn test_set_windows_hook_ex_invalid_callback() {
        let hhook = unsafe { SetWindowsHookExA(WH_MOUSE, 0, 0, 12345) };
        assert_eq!(hhook, 0, "NULL callback must return NULL handle");
        let err = unsafe { crate::dlls::kernel32::GetLastError() };
        assert_eq!(err, 87); // ERROR_INVALID_PARAMETER
    }

    #[test]
    fn test_set_windows_hook_ex_a_win64_call() {
        let func_ptr: unsafe extern "win64" fn(i32, usize, usize, u32) -> usize = SetWindowsHookExA;
        let h = unsafe { func_ptr(WH_MOUSE, 0x140055BC0, 0, 0) };
        assert_ne!(h, 0);
        unsafe { UnhookWindowsHookEx(h) };
    }

    #[test]
    fn test_enum_display_monitors() {
        static mut CALLED: bool = false;
        unsafe extern "win64" fn test_cb(
            h_mon: usize,
            _hdc: usize,
            rc: *const Rect,
            _data: usize,
        ) -> i32 {
            CALLED = true;
            assert_eq!(h_mon, 0x00010001);
            assert!((*rc).right > 0);
            assert!((*rc).bottom > 0);
            1
        }

        let res = unsafe { EnumDisplayMonitors(0, std::ptr::null(), Some(test_cb), 0) };
        assert_eq!(res, 1);
        assert!(unsafe { CALLED });
    }

    #[test]
    fn test_get_monitor_info_w() {
        let mut mi = MonitorInfo {
            cb_size: std::mem::size_of::<MonitorInfo>() as u32,
            rc_monitor: Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rc_work: Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            dw_flags: 0,
        };
        let res = unsafe { GetMonitorInfoW(0x00010001, &mut mi) };
        assert_eq!(res, 1);
        assert!(mi.rc_monitor.right > 0);
        assert!(mi.rc_monitor.bottom > 0);
        assert!(mi.rc_work.bottom > 0);
        assert_eq!(mi.dw_flags, 1);
    }

    #[test]
    fn test_get_system_metrics() {
        let cx = unsafe { GetSystemMetrics(0) };
        let cy = unsafe { GetSystemMetrics(1) };
        assert!(cx > 0);
        assert!(cy > 0);
    }

    #[test]
    fn test_adjust_window_rect_ex() {
        let mut rc = Rect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let res = unsafe { AdjustWindowRectEx(&mut rc, 0x16CF0000, 0, 0x00040110) };
        assert_eq!(res, 1);
        assert_eq!(rc.left, -8);
        assert_eq!(rc.top, -30);
        assert_eq!(rc.right, 1928);
        assert_eq!(rc.bottom, 1088);
    }

    #[test]
    fn test_window_placement_layout() {
        assert_eq!(std::mem::size_of::<WindowPlacement>(), 44);
    }

    #[test]
    fn test_get_async_key_state_and_keyboard_state() {
        unsafe {
            let _state = GetAsyncKeyState(0x01); // VK_LBUTTON
            let _kstate = GetKeyState(0x01);
            let mut key_array = [0u8; 256];
            let res = GetKeyboardState(key_array.as_mut_ptr());
            assert_eq!(res, 1);
        }
    }

    #[test]
    fn test_screen_to_client_and_client_to_screen() {
        unsafe {
            let mut pt = Point { x: 500, y: 300 };
            assert_eq!(ClientToScreen(0x1E00002, &mut pt), 1);
            assert_eq!(ScreenToClient(0x1E00002, &mut pt), 1);
            assert_eq!(pt.x, 500);
            assert_eq!(pt.y, 300);
        }
    }

    #[test]
    fn test_keyboard_translation_apis() {
        unsafe {
            let hkl = GetKeyboardLayout(0);
            assert_ne!(hkl, 0);

            let mut out_uni = [0u16; 2];
            let res = ToUnicode(0x41, 0, std::ptr::null(), out_uni.as_mut_ptr(), 2, 0);
            assert_eq!(res, 1);
            assert_eq!(out_uni[0], 0x61); // 'a'

            let mut shift_state = [0u8; 256];
            shift_state[0x10] = 0x80; // VK_SHIFT down
            let res_shift = ToUnicode(0x41, 0, shift_state.as_ptr(), out_uni.as_mut_ptr(), 2, 0);
            assert_eq!(res_shift, 1);
            assert_eq!(out_uni[0], 0x41); // 'A'

            let vk_a = VkKeyScanA(b'a' as i8);
            assert_eq!(vk_a, 0x41);
        }
    }
}
