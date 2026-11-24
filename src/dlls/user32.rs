#![allow(non_snake_case)]
use crate::sys::{InputSubsystem, WinMsg, WM_NULL, WM_QUIT};
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::Mutex;

pub type HWND = u64;

static USER32_INPUT: Mutex<Option<InputSubsystem>> = Mutex::new(None);

/// Global HWND -> SDL_Window* registry.
/// When the guest calls CreateWindowExW, we create a real SDL2 window and store its pointer here.
static HWND_REGISTRY: Mutex<Option<HashMap<u64, usize>>> = Mutex::new(None);

pub static RAW_INPUT_REGISTERED_MOUSE: AtomicBool = AtomicBool::new(false);
pub static RAW_INPUT_REGISTERED_KEYBOARD: AtomicBool = AtomicBool::new(false);

static KEY_STATE_TABLE: [AtomicBool; 256] = {
    const INIT: AtomicBool = AtomicBool::new(false);
    [INIT; 256]
};

type SdlPollEventFn = unsafe extern "C" fn(*mut u8) -> i32;
type SdlGetMouseStateFn = unsafe extern "C" fn(*mut i32, *mut i32) -> u32;
type SdlGetGlobalMouseStateFn = unsafe extern "C" fn(*mut i32, *mut i32) -> u32;
type SdlWarpMouseFn = unsafe extern "C" fn(*mut c_void, i32, i32);
type SdlSetWindowGrabFn = unsafe extern "C" fn(*mut c_void, i32);
type SdlRaiseWindowFn = unsafe extern "C" fn(*mut c_void);
type SdlGetCurrentDisplayModeFn = unsafe extern "C" fn(i32, *mut SdlDisplayMode) -> i32;
type SdlSetRelativeMouseModeFn = unsafe extern "C" fn(i32) -> i32;

static FN_POLL_EVENT: std::sync::OnceLock<Option<SdlPollEventFn>> = std::sync::OnceLock::new();
static FN_GET_MOUSE_STATE: std::sync::OnceLock<Option<SdlGetMouseStateFn>> = std::sync::OnceLock::new();
static FN_GET_GLOBAL_MOUSE_STATE: std::sync::OnceLock<Option<SdlGetGlobalMouseStateFn>> = std::sync::OnceLock::new();
static FN_WARP_MOUSE: std::sync::OnceLock<Option<SdlWarpMouseFn>> = std::sync::OnceLock::new();
static FN_SET_WINDOW_GRAB: std::sync::OnceLock<Option<SdlSetWindowGrabFn>> = std::sync::OnceLock::new();
static FN_SET_RELATIVE_MOUSE_MODE: std::sync::OnceLock<Option<SdlSetRelativeMouseModeFn>> = std::sync::OnceLock::new();
static FN_RAISE_WINDOW: std::sync::OnceLock<Option<SdlRaiseWindowFn>> = std::sync::OnceLock::new();
static FN_GET_DISPLAY_MODE: std::sync::OnceLock<Option<SdlGetCurrentDisplayModeFn>> = std::sync::OnceLock::new();

pub fn get_sdl_poll_event_fn() -> Option<SdlPollEventFn> {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if sdl_handle.is_null() {
        return None;
    }
    *FN_POLL_EVENT.get_or_init(|| unsafe {
        let sym = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_PollEvent").unwrap().as_ptr(),
        );
        if !sym.is_null() {
            Some(std::mem::transmute(sym))
        } else {
            None
        }
    })
}

pub fn get_sdl_mouse_state_fn() -> Option<SdlGetMouseStateFn> {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if sdl_handle.is_null() {
        return None;
    }
    *FN_GET_MOUSE_STATE.get_or_init(|| unsafe {
        let sym = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_GetMouseState").unwrap().as_ptr(),
        );
        if !sym.is_null() {
            Some(std::mem::transmute(sym))
        } else {
            None
        }
    })
}

pub fn get_sdl_warp_mouse_fn() -> Option<SdlWarpMouseFn> {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if sdl_handle.is_null() {
        return None;
    }
    *FN_WARP_MOUSE.get_or_init(|| unsafe {
        let sym = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_WarpMouseInWindow").unwrap().as_ptr(),
        );
        if !sym.is_null() {
            Some(std::mem::transmute(sym))
        } else {
            None
        }
    })
}

pub fn get_sdl_global_mouse_state_fn() -> Option<SdlGetGlobalMouseStateFn> {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if sdl_handle.is_null() {
        return None;
    }
    *FN_GET_GLOBAL_MOUSE_STATE.get_or_init(|| unsafe {
        let sym = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_GetGlobalMouseState").unwrap().as_ptr(),
        );
        if !sym.is_null() {
            Some(std::mem::transmute(sym))
        } else {
            None
        }
    })
}

pub fn get_sdl_set_window_grab_fn() -> Option<SdlSetWindowGrabFn> {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if sdl_handle.is_null() {
        return None;
    }
    *FN_SET_WINDOW_GRAB.get_or_init(|| unsafe {
        let sym = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_SetWindowGrab").unwrap().as_ptr(),
        );
        if !sym.is_null() {
            Some(std::mem::transmute(sym))
        } else {
            None
        }
    })
}

pub fn get_sdl_set_relative_mouse_mode_fn() -> Option<SdlSetRelativeMouseModeFn> {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if sdl_handle.is_null() {
        return None;
    }
    *FN_SET_RELATIVE_MOUSE_MODE.get_or_init(|| unsafe {
        let sym = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_SetRelativeMouseMode")
                .unwrap()
                .as_ptr(),
        );
        if !sym.is_null() {
            Some(std::mem::transmute(sym))
        } else {
            None
        }
    })
}

pub fn get_sdl_raise_window_fn() -> Option<SdlRaiseWindowFn> {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if sdl_handle.is_null() {
        return None;
    }
    *FN_RAISE_WINDOW.get_or_init(|| unsafe {
        let sym = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_RaiseWindow").unwrap().as_ptr(),
        );
        if !sym.is_null() {
            Some(std::mem::transmute(sym))
        } else {
            None
        }
    })
}

static OVERLAY_PROCESS: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);
static OVERLAY_LAST_TRIGGER_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn release_cursor_and_show() {
    IS_CURSOR_CLIPPED.store(false, Ordering::Relaxed);
    if let Some(f) = get_sdl_set_relative_mouse_mode_fn() {
        unsafe { f(0); }
    }
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if !sdl_handle.is_null() {
        unsafe {
            type SdlShowCursorFn = unsafe extern "C" fn(i32) -> i32;
            let sym = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_ShowCursor").unwrap().as_ptr(),
            );
            if !sym.is_null() {
                let f: SdlShowCursorFn = std::mem::transmute(sym);
                f(1);
            }
        }
    }
}

fn toggle_in_game_overlay() {
    let now = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as u64,
        Err(_) => 0,
    };
    let last = OVERLAY_LAST_TRIGGER_MS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < 400 {
        return; // Debounce rapid repeats
    }
    OVERLAY_LAST_TRIGGER_MS.store(now, Ordering::Relaxed);

    std::thread::spawn(move || {
        if let Ok(mut lock) = OVERLAY_PROCESS.lock() {
            if let Some(mut child) = lock.take() {
                match child.try_wait() {
                    Ok(None) => {
                        println!("[RINE HOTKEY] Toggling In-Game Overlay OFF (closing PID {})...", child.id());
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                    _ => {}
                }
            }

            release_cursor_and_show();

            println!("[RINE HOTKEY] Shift+Tab detected! Spawning In-Game Steam Overlay...");
            let rine_exe = if std::path::Path::new("/home/fizzy/.local/bin/rine").exists() {
                std::path::PathBuf::from("/home/fizzy/.local/bin/rine")
            } else if let Ok(target) = std::env::var("RINE_BINARY_PATH") {
                std::path::PathBuf::from(target)
            } else {
                std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("rine"))
            };

            match std::process::Command::new(&rine_exe).arg("--overlay").spawn() {
                Ok(child) => {
                    println!("[RINE HOTKEY] Overlay spawned successfully with PID {}", child.id());
                    *lock = Some(child);
                }
                Err(e) => {
                    eprintln!("[RINE HOTKEY] Failed to spawn overlay (exe={:?}): {}", rine_exe, e);
                }
            }
        }
    });
}



#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct SdlDisplayMode {
    pub format: u32,
    pub w: i32,
    pub h: i32,
    pub refresh_rate: i32,
    pub driverdata: *mut c_void,
}

pub fn get_host_refresh_rate() -> u32 {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if !sdl_handle.is_null() {
        let get_mode = *FN_GET_DISPLAY_MODE.get_or_init(|| unsafe {
            let sym = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetCurrentDisplayMode")
                    .unwrap()
                    .as_ptr(),
            );
            if !sym.is_null() {
                Some(std::mem::transmute(sym))
            } else {
                None
            }
        });
        if let Some(f) = get_mode {
            let mut mode = SdlDisplayMode::default();
            if unsafe { f(0, &mut mode) } == 0 && mode.refresh_rate > 0 {
                return mode.refresh_rate as u32;
            }
        }
    }
    165
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawInputHeader {
    pub dw_type: u32,
    pub dw_size: u32,
    pub h_device: u64,
    pub w_param: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawMouse {
    pub us_flags: u16,
    pub _pad: u16,
    pub us_button_flags: u16,
    pub us_button_data: u16,
    pub ul_raw_buttons: u32,
    pub l_last_x: i32,
    pub l_last_y: i32,
    pub ul_extra_info: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawKeyboard {
    pub make_code: u16,
    pub flags: u16,
    pub reserved: u16,
    pub v_key: u16,
    pub message: u32,
    pub extra_information: u32,
    pub _pad: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union RawInputData {
    pub mouse: RawMouse,
    pub keyboard: RawKeyboard,
    pub raw: [u8; 24],
}

impl std::fmt::Debug for RawInputData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RawInputData")
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawInput {
    pub header: RawInputHeader,
    pub data: RawInputData,
}

impl RawInput {
    pub fn new_mouse(mouse: RawMouse) -> Self {
        RawInput {
            header: RawInputHeader {
                dw_type: 0, // RIM_TYPEMOUSE
                dw_size: std::mem::size_of::<RawInput>() as u32,
                h_device: 0x00010001,
                w_param: 0,
            },
            data: RawInputData { mouse },
        }
    }

    pub fn new_keyboard(keyboard: RawKeyboard) -> Self {
        RawInput {
            header: RawInputHeader {
                dw_type: 1, // RIM_TYPEKEYBOARD
                dw_size: std::mem::size_of::<RawInput>() as u32,
                h_device: 0x00010002,
                w_param: 0,
            },
            data: RawInputData { keyboard },
        }
    }
}

static CURRENT_RAW_INPUT: Mutex<RawInput> = Mutex::new(RawInput {
    header: RawInputHeader {
        dw_type: 0,
        dw_size: 48,
        h_device: 0x00010001,
        w_param: 0,
    },
    data: RawInputData {
        mouse: RawMouse {
            us_flags: 0,
            _pad: 0,
            us_button_flags: 0,
            us_button_data: 0,
            ul_raw_buttons: 0,
            l_last_x: 0,
            l_last_y: 0,
            ul_extra_info: 0,
        },
    },
});

static RAW_INPUT_BUFFER_QUEUE: Mutex<Option<VecDeque<RawInput>>> = Mutex::new(None);
static RAW_INPUT_MOUSE_HWND: AtomicU64 = AtomicU64::new(0);
static RAW_INPUT_KBD_HWND: AtomicU64 = AtomicU64::new(0);
static WARP_TARGET_X: AtomicI32 = AtomicI32::new(i32::MIN);
static WARP_TARGET_Y: AtomicI32 = AtomicI32::new(i32::MIN);
static WARP_PENDING: AtomicBool = AtomicBool::new(false);
static CURSOR_DISPLAY_COUNT: AtomicI32 = AtomicI32::new(0);
static RAW_INPUT_HANDLE_COUNTER: AtomicUsize = AtomicUsize::new(0xCAFE0001);
static RAW_INPUT_MAP: Mutex<Option<HashMap<usize, RawInput>>> = Mutex::new(None);

pub fn push_raw_input_event(input: RawInput) -> usize {
    let handle = RAW_INPUT_HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    {
        let mut map_guard = RAW_INPUT_MAP.lock().unwrap();
        let map = map_guard.get_or_insert_with(HashMap::new);
        if map.len() > 512 {
            map.clear();
        }
        map.insert(handle, input);
    }
    {
        let mut guard = RAW_INPUT_BUFFER_QUEUE.lock().unwrap();
        let q = guard.get_or_insert_with(VecDeque::new);
        if q.len() < 128 {
            q.push_back(input);
        }
    }
    *CURRENT_RAW_INPUT.lock().unwrap() = input;
    handle
}

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

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicUsize, Ordering};

pub static PRIMARY_HWND: AtomicU64 = AtomicU64::new(0x1E00002);
pub static ACTIVE_HWND: AtomicU64 = AtomicU64::new(0);
pub static FOREGROUND_HWND: AtomicU64 = AtomicU64::new(0);
pub static FOCUS_HWND: AtomicU64 = AtomicU64::new(0);

pub fn is_primary_window(hwnd: u64) -> bool {
    if hwnd == 0 {
        return false;
    }
    if cfg!(test) {
        return true;
    }
    let pri = PRIMARY_HWND.load(Ordering::Relaxed);
    let glob = crate::windowing::x11::GLOBAL_WINDOW.load(Ordering::Relaxed);
    if pri != 0 {
        hwnd == pri || hwnd == glob
    } else {
        hwnd == glob || hwnd < 0x00020000
    }
}
pub static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(false);
pub static WINDOW_ACTIVATED: AtomicBool = AtomicBool::new(false);
pub static IS_CURSOR_CLIPPED: AtomicBool = AtomicBool::new(false);
pub static LAST_CURSOR_X: AtomicI32 = AtomicI32::new(960);
pub static LAST_CURSOR_Y: AtomicI32 = AtomicI32::new(540);
pub static CACHED_WIN_X: AtomicI32 = AtomicI32::new(0);
pub static CACHED_WIN_Y: AtomicI32 = AtomicI32::new(0);

static LAST_POS_CHECK_TICK: AtomicU32 = AtomicU32::new(0);

pub fn refresh_cached_window_position() -> (i32, i32) {
    let now = unsafe { crate::dlls::kernel32::GetTickCount() };
    let last = LAST_POS_CHECK_TICK.load(Ordering::Relaxed);
    let cached_x = CACHED_WIN_X.load(Ordering::Relaxed);
    let cached_y = CACHED_WIN_Y.load(Ordering::Relaxed);
    if now.wrapping_sub(last) < 50 && (cached_x != 0 || cached_y != 0 || last != 0) {
        return (cached_x, cached_y);
    }
    LAST_POS_CHECK_TICK.store(now, Ordering::Relaxed);

    let (x, y) = crate::windowing::x11::sdl_get_window_position();
    CACHED_WIN_X.store(x, Ordering::Relaxed);
    CACHED_WIN_Y.store(y, Ordering::Relaxed);
    (x, y)
}
static INIT_MSG_STEP: AtomicU32 = AtomicU32::new(0);
static GODOT_WNDPROC: AtomicU64 = AtomicU64::new(0);
static CURRENT_CAPTURE_HWND: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

#[repr(C)]
pub struct CreateStructW {
    pub lp_create_params: *mut std::ffi::c_void,
    pub h_instance: u64,
    pub h_menu: u64,
    pub hwnd_parent: u64,
    pub cy: i32,
    pub cx: i32,
    pub y: i32,
    pub x: i32,
    pub style: i32,
    pub lpsz_name: *const u16,
    pub lpsz_class: *const u16,
    pub dw_ex_style: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WindowPos {
    pub hwnd: u64,
    pub hwnd_insert_after: u64,
    pub x: i32,
    pub y: i32,
    pub cx: i32,
    pub cy: i32,
    pub flags: u32,
    pub _pad: u32,
}

pub unsafe fn write_win64_msg(
    lp_msg: *mut u8,
    hwnd: u64,
    message: u32,
    wparam: u64,
    lparam: u64,
    time: u32,
    pt_x: i32,
    pt_y: i32,
) {
    let tick = if time != 0 {
        time
    } else {
        crate::dlls::kernel32::GetTickCount()
    };
    let msg = Win64Msg {
        hwnd,
        message,
        _pad1: 0,
        w_param: wparam,
        l_param: lparam,
        time: tick,
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

static CLASS_WNDPROCS: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);
static ATOM_WNDPROCS: Mutex<Option<HashMap<u16, u64>>> = Mutex::new(None);
static HWND_WNDPROCS: Mutex<Option<HashMap<u64, u64>>> = Mutex::new(None);
static HWND_USERDATA: Mutex<Option<HashMap<u64, u64>>> = Mutex::new(None);
static HWND_STYLES: Mutex<Option<HashMap<u64, (u32, u32)>>> = Mutex::new(None);
static NEXT_ATOM: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0xC001);
static PRIMARY_WINDOW_BOUND: AtomicBool = AtomicBool::new(false);
static NEXT_VIRTUAL_HWND: AtomicU64 = AtomicU64::new(0x00020000);

pub fn register_window_class_wndproc_name(wndproc: u64, name_opt: Option<String>, atom_opt: Option<u16>) -> u16 {
    let atom = atom_opt.unwrap_or_else(|| NEXT_ATOM.fetch_add(1, Ordering::SeqCst));
    if wndproc > 0x10000 && wndproc < 0x7FFFFFFFFFFF {
        {
            let mut guard = ATOM_WNDPROCS.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(atom, wndproc);
        }
        let mut is_helper = false;
        if let Some(name) = name_opt {
            let lower = name.to_lowercase();
            if lower.contains("message") || lower.contains("helper") || lower.contains("hidden") {
                is_helper = true;
            }
            let mut guard = CLASS_WNDPROCS.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(lower.clone(), wndproc);
            println!(
                "[USER32 ENGINE] Registered Class '{}' (atom=0x{:04X}) with WndProc=0x{:016X}",
                name, atom, wndproc
            );
        }
        if !is_helper && (GODOT_WNDPROC.load(Ordering::Relaxed) == 0 || !PRIMARY_WINDOW_BOUND.load(Ordering::Relaxed)) {
            GODOT_WNDPROC.store(wndproc, Ordering::Relaxed);
        }
    }
    atom
}

pub fn register_window_class_wndproc(wndproc: u64, class_name_ptr: *const u16) -> u16 {
    let (name, atom) = if class_name_ptr.is_null() {
        (None, None)
    } else if (class_name_ptr as usize) < 0x10000 {
        (None, Some(class_name_ptr as usize as u16))
    } else {
        (unsafe { read_utf16_safe(class_name_ptr) }, None)
    };
    register_window_class_wndproc_name(wndproc, name, atom)
}

pub fn register_window_class_wndproc_a(wndproc: u64, class_name_ptr: *const u8) -> u16 {
    let (name, atom) = if class_name_ptr.is_null() {
        (None, None)
    } else if (class_name_ptr as usize) < 0x10000 {
        (None, Some(class_name_ptr as usize as u16))
    } else {
        unsafe {
            let cstr = std::ffi::CStr::from_ptr(class_name_ptr as *const libc::c_char);
            (Some(cstr.to_string_lossy().into_owned()), None)
        }
    };
    register_window_class_wndproc_name(wndproc, name, atom)
}

pub fn lookup_class_wndproc(class_name_ptr: *const u16) -> Option<u64> {
    if class_name_ptr.is_null() {
        return None;
    }
    let val = class_name_ptr as usize;
    if val < 0x10000 {
        let guard = ATOM_WNDPROCS.lock().unwrap();
        guard.as_ref().and_then(|m| m.get(&(val as u16)).copied())
    } else {
        if let Some(name) = unsafe { read_utf16_safe(class_name_ptr) } {
            let guard = CLASS_WNDPROCS.lock().unwrap();
            if let Some(w) = guard.as_ref().and_then(|m| m.get(&name.to_lowercase()).copied()) {
                return Some(w);
            }
        }
        unsafe {
            let cstr = std::ffi::CStr::from_ptr(class_name_ptr as *const libc::c_char);
            if let Ok(s) = cstr.to_str() {
                let guard = CLASS_WNDPROCS.lock().unwrap();
                if let Some(w) = guard.as_ref().and_then(|m| m.get(&s.to_lowercase()).copied()) {
                    return Some(w);
                }
            }
        }
        None
    }
}

pub fn set_wndproc_for_hwnd(hwnd: u64, wndproc: u64) {
    if hwnd != 0 && wndproc > 0x10000 && wndproc < 0x7FFFFFFFFFFF {
        let mut guard = HWND_WNDPROCS.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(hwnd, wndproc);
    }
}

pub fn get_wndproc_for_hwnd(hwnd: u64) -> u64 {
    if hwnd != 0 {
        let guard = HWND_WNDPROCS.lock().unwrap();
        if let Some(map) = guard.as_ref() {
            if let Some(&proc) = map.get(&hwnd) {
                if proc > 0x10000 && proc < 0x7FFFFFFFFFFF {
                    return proc;
                }
            }
        }
    }
    GODOT_WNDPROC.load(Ordering::Relaxed)
}

pub fn register_godot_wndproc(wndproc_ptr: u64) {
    if wndproc_ptr != 0 {
        GODOT_WNDPROC.store(wndproc_ptr, Ordering::Relaxed);
        println!(
            "[USER32 ENGINE] Registered WndProc at: 0x{:016X}",
            wndproc_ptr
        );
        let hwnd = PRIMARY_HWND.load(Ordering::Relaxed);
        if hwnd != 0 {
            set_wndproc_for_hwnd(hwnd, wndproc_ptr);
            unsafe {
                SendMessageW(hwnd, 0x001C /* WM_ACTIVATEAPP */, 1, 0);
                SendMessageW(hwnd, 0x0006 /* WM_ACTIVATE */, 1 /* WA_ACTIVE */, 0);
                SendMessageW(hwnd, 0x0007 /* WM_SETFOCUS */, 0, 0);
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterClassExW(lp_wcx: *const u8) -> u16 {
    if !lp_wcx.is_null() {
        let wndproc = *(lp_wcx.add(8) as *const u64);
        let class_name_ptr = *(lp_wcx.add(64) as *const u64) as *const u16;
        return register_window_class_wndproc(wndproc, class_name_ptr);
    }
    0x1001
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterClassExA(lp_wcx: *const u8) -> u16 {
    if !lp_wcx.is_null() {
        let wndproc = *(lp_wcx.add(8) as *const u64);
        let class_name_ptr = *(lp_wcx.add(64) as *const u64) as *const u8;
        return register_window_class_wndproc_a(wndproc, class_name_ptr);
    }
    0x1001
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterClassW(lp_wnd_class: *const u8) -> u16 {
    if !lp_wnd_class.is_null() {
        let wndproc = *(lp_wnd_class.add(8) as *const u64);
        let class_name_ptr = *(lp_wnd_class.add(64) as *const u64) as *const u16;
        return register_window_class_wndproc(wndproc, class_name_ptr);
    }
    0x1001
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterClassA(lp_wnd_class: *const u8) -> u16 {
    if !lp_wnd_class.is_null() {
        let wndproc = *(lp_wnd_class.add(8) as *const u64);
        let class_name_ptr = *(lp_wnd_class.add(64) as *const u64) as *const u8;
        return register_window_class_wndproc_a(wndproc, class_name_ptr);
    }
    0x1001
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

    let is_message_or_hidden = title.to_lowercase().contains("message")
        || title.to_lowercase().contains("hidden")
        || _h_wnd_parent == 0xFFFFFFFFFFFFFFFD
        || _h_wnd_parent == 0xFFFFFFFFFFFFFFFF
        || (n_width <= 1 && n_height <= 1 && (_dw_style & 0x10000000 == 0));

    let wndproc = lookup_class_wndproc(lp_class_name)
        .unwrap_or_else(|| GODOT_WNDPROC.load(Ordering::Relaxed));

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
        "[USER32 ENGINE] CreateWindowExW: title='{}', size={}x{}, is_msg={}, wndproc=0x{:016X}",
        title, width, height, is_message_or_hidden, wndproc
    );

    // If we already have a global SDL window (created from main.rs), reuse it for the primary game presentation window
    let existing = crate::windowing::x11::GLOBAL_WINDOW.load(Ordering::SeqCst);
    if existing != 0 && !is_message_or_hidden && !PRIMARY_WINDOW_BOUND.load(Ordering::SeqCst) {
        PRIMARY_WINDOW_BOUND.store(true, Ordering::SeqCst);
        PRIMARY_HWND.store(existing, Ordering::SeqCst);
        set_wndproc_for_hwnd(existing, wndproc);
        GODOT_WNDPROC.store(wndproc, Ordering::Relaxed);
        {
            let mut guard = HWND_STYLES.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(existing, (_dw_style, _dw_ex_style));
        }

        let (real_w, real_h) = crate::windowing::x11::sdl_get_window_size();
        let host_w = if real_w > 0 { real_w as u32 } else { 1920 };
        let host_h = if real_h > 0 { real_h as u32 } else { 1080 };
        let eff_w = if width > 320 { width } else { host_w };
        let eff_h = if height > 240 { height } else { host_h };

        if width > 320 && height > 240 {
            crate::windowing::x11::sdl_set_window_size(width as i32, height as i32);
        }
        if _x >= 0 && _y >= 0 && _x <= 5000 && _y <= 5000 {
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
        if (_dw_style & 0x10000000) != 0 {
            WINDOW_VISIBLE.store(true, Ordering::SeqCst);
        }
        let sdl_win = crate::windowing::x11::get_global_sdl_window() as usize;
        register_hwnd(existing, sdl_win, eff_w, eff_h);
        println!(
            "[USER32 ENGINE] Bound primary window title='{}' to physical HWND=0x{:X}, size={}x{} (requested {}x{}), wndproc=0x{:016X}",
            title, existing, eff_w, eff_h, width, height, wndproc
        );
        let mut cs = CreateStructW {
            lp_create_params: _lp_param,
            h_instance: _h_instance,
            h_menu: _h_menu,
            hwnd_parent: _h_wnd_parent,
            cy: eff_h as i32,
            cx: eff_w as i32,
            y: if _y >= 0 && _y <= 5000 { _y } else { 0 },
            x: if _x >= 0 && _x <= 5000 { _x } else { 0 },
            style: _dw_style as i32,
            lpsz_name: lp_window_name,
            lpsz_class: lp_class_name,
            dw_ex_style: _dw_ex_style,
        };
        let cs_ptr = &mut cs as *mut CreateStructW as u64;

        SendMessageW(existing, 0x0081 /* WM_NCCREATE */, 0, cs_ptr);
        SendMessageW(existing, 0x0083 /* WM_NCCALCSIZE */, 0, 0);
        SendMessageW(existing, 0x0001 /* WM_CREATE */, 0, cs_ptr);
        SendMessageW(existing, 0x0005 /* WM_SIZE */, 0 /* SIZE_RESTORED */, ((eff_h as u64) << 16) | (eff_w as u64));
        SendMessageW(existing, 0x0006 /* WM_ACTIVATE */, 1 /* WA_ACTIVE */, 0);
        SendMessageW(existing, 0x0007 /* WM_SETFOCUS */, 0, 0);
        return existing;
    }

    // 2. Secondary / Virtual / Message window: do NOT overwrite physical presentation window
    if existing != 0 || is_message_or_hidden {
        let virtual_hwnd = NEXT_VIRTUAL_HWND.fetch_add(4, Ordering::SeqCst);
        set_wndproc_for_hwnd(virtual_hwnd, wndproc);
        {
            let mut guard = HWND_STYLES.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(virtual_hwnd, (_dw_style, _dw_ex_style));
        }
        register_hwnd(virtual_hwnd, 0, width, height);
        println!(
            "[USER32 ENGINE] Created virtual/message window title='{}' HWND=0x{:X}, size={}x{}, wndproc=0x{:016X}",
            title, virtual_hwnd, width, height, wndproc
        );

        let mut cs = CreateStructW {
            lp_create_params: _lp_param,
            h_instance: _h_instance,
            h_menu: _h_menu,
            hwnd_parent: _h_wnd_parent,
            cy: height as i32,
            cx: width as i32,
            y: _y,
            x: _x,
            style: _dw_style as i32,
            lpsz_name: lp_window_name,
            lpsz_class: lp_class_name,
            dw_ex_style: _dw_ex_style,
        };
        let cs_ptr = &mut cs as *mut CreateStructW as u64;

        SendMessageW(virtual_hwnd, 0x0081 /* WM_NCCREATE */, 0, cs_ptr);
        SendMessageW(virtual_hwnd, 0x0001 /* WM_CREATE */, 0, cs_ptr);
        return virtual_hwnd;
    }

    // 3. Create a new window (SDL2 with X11 fallback) if no existing global window
    let win_id = crate::windowing::x11::create_physical_x11_window(&title, width, height);
    if win_id != 0 {
        PRIMARY_HWND.store(win_id, Ordering::SeqCst);
        set_wndproc_for_hwnd(win_id, wndproc);
        GODOT_WNDPROC.store(wndproc, Ordering::Relaxed);
        {
            let mut guard = HWND_STYLES.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(win_id, (_dw_style, _dw_ex_style));
        }
        if (_dw_style & 0x10000000) != 0 {
            WINDOW_VISIBLE.store(true, Ordering::SeqCst);
        }
        let sdl_win = crate::windowing::x11::get_global_sdl_window() as usize;
        register_hwnd(win_id, sdl_win, width, height);
        let mut cs = CreateStructW {
            lp_create_params: _lp_param,
            h_instance: _h_instance,
            h_menu: _h_menu,
            hwnd_parent: _h_wnd_parent,
            cy: height as i32,
            cx: width as i32,
            y: _y,
            x: _x,
            style: _dw_style as i32,
            lpsz_name: lp_window_name,
            lpsz_class: lp_class_name,
            dw_ex_style: _dw_ex_style,
        };
        let cs_ptr = &mut cs as *mut CreateStructW as u64;

        SendMessageW(win_id, 0x0081 /* WM_NCCREATE */, 0, cs_ptr);
        SendMessageW(win_id, 0x0083 /* WM_NCCALCSIZE */, 0, 0);
        SendMessageW(win_id, 0x0001 /* WM_CREATE */, 0, cs_ptr);
        SendMessageW(win_id, 0x0005 /* WM_SIZE */, 0 /* SIZE_RESTORED */, ((height as u64) << 16) | (width as u64));
        SendMessageW(win_id, 0x0006 /* WM_ACTIVATE */, 1 /* WA_ACTIVE */, 0);
        SendMessageW(win_id, 0x0007 /* WM_SETFOCUS */, 0, 0);
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
            crate::windowing::x11::ensure_x11_environment();
            let d = crate::windowing::x11::XOpenDisplay(std::ptr::null());
            if !d.is_null() {
                crate::windowing::x11::GLOBAL_DISPLAY
                    .store(d as u64, std::sync::atomic::Ordering::SeqCst);
                global_dpy = d;
            }
        }
    }
    let global_vid = crate::windowing::x11::get_global_visual_id();

    let actual_x11_win = if sdl_window_ptr != 0 && sdl_window_ptr != 0x10000 {
        let x11_id = unsafe { crate::windowing::x11::get_x11_window_id(sdl_window_ptr as *mut std::ffi::c_void) };
        if x11_id != 0 {
            crate::windowing::x11::GLOBAL_WINDOW.store(x11_id, Ordering::SeqCst);
            x11_id
        } else if global_win != 0 {
            global_win
        } else {
            hwnd
        }
    } else if global_win != 0 {
        global_win
    } else {
        hwnd
    };

    let win_to_use = actual_x11_win;

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
            visible: WINDOW_VISIBLE.load(Ordering::SeqCst),
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
        // Return, Escape, Backspace, Tab, Space
        13 => 0x0D,                         // VK_RETURN
        27 => 0x1B,                         // VK_ESCAPE
        32 => 0x20,                         // VK_SPACE
        8 => 0x08,                          // VK_BACK
        9 => 0x09,                          // VK_TAB
        127 => 0x2E,                        // VK_DELETE
        // Navigation
        0x40000049 => 0x2D,                 // VK_INSERT
        0x4000004A => 0x24,                 // VK_HOME
        0x4000004B => 0x21,                 // VK_PRIOR (PageUp)
        0x4000004C => 0x2E,                 // VK_DELETE
        0x4000004D => 0x23,                 // VK_END
        0x4000004E => 0x22,                 // VK_NEXT (PageDown)
        0x4000004F => 0x27,                 // VK_RIGHT
        0x40000050 => 0x25,                 // VK_LEFT
        0x40000051 => 0x28,                 // VK_DOWN
        0x40000052 => 0x26,                 // VK_UP
        // Locks
        0x40000039 => 0x14,                 // VK_CAPITAL
        0x40000046 => 0x2C,                 // VK_SNAPSHOT (PrintScreen)
        0x40000047 => 0x91,                 // VK_SCROLL
        0x40000048 => 0x13,                 // VK_PAUSE
        0x40000053 => 0x90,                 // VK_NUMLOCK
        // Keypad
        0x40000054 => 0x6F,                 // VK_DIVIDE
        0x40000055 => 0x6A,                 // VK_MULTIPLY
        0x40000056 => 0x6D,                 // VK_SUBTRACT
        0x40000057 => 0x6B,                 // VK_ADD
        0x40000058 => 0x0D,                 // VK_RETURN (KP Enter)
        0x40000059..=0x40000061 => 0x61 + (sym - 0x40000059) as u32, // VK_NUMPAD1..VK_NUMPAD9
        0x40000062 => 0x60,                 // VK_NUMPAD0
        0x40000063 => 0x6E,                 // VK_DECIMAL
        // Modifiers
        0x400000E0 => 0xA2,                 // VK_LCONTROL
        0x400000E1 => 0xA0,                 // VK_LSHIFT
        0x400000E2 => 0xA4,                 // VK_LMENU
        0x400000E3 => 0x5B,                 // VK_LWIN
        0x400000E4 => 0xA3,                 // VK_RCONTROL
        0x400000E5 => 0xA1,                 // VK_RSHIFT
        0x400000E6 => 0xA5,                 // VK_RMENU
        0x400000E7 => 0x5C,                 // VK_RWIN
        // Function keys
        0x4000003A..=0x40000045 => 0x70 + (sym - 0x4000003A) as u32, // VK_F1 .. VK_F12
        // Letters and digits
        0x61..=0x7A => (sym - 0x20) as u32, // a-z -> A-Z
        0x30..=0x39 => sym as u32,          // 0-9
        // Punctuation
        45 => 0xBD,                         // - -> VK_OEM_MINUS
        61 => 0xBB,                         // = -> VK_OEM_PLUS
        91 => 0xDB,                         // [ -> VK_OEM_4
        93 => 0xDD,                         // ] -> VK_OEM_6
        92 => 0xDC,                         // \ -> VK_OEM_5
        59 => 0xBA,                         // ; -> VK_OEM_1
        39 => 0xDE,                         // ' -> VK_OEM_7
        96 => 0xC0,                         // ` -> VK_OEM_3
        44 => 0xBC,                         // , -> VK_OEM_COMMA
        46 => 0xBE,                         // . -> VK_OEM_PERIOD
        47 => 0xBF,                         // / -> VK_OEM_2
        _ => {
            if sym > 0 && sym < 256 {
                sym as u32
            } else {
                0
            }
        }
    }
}

fn vk_to_scancode(vk: u32) -> (u16, bool) {
    match vk {
        0x1B => (0x01, false), // VK_ESCAPE
        0x31 => (0x02, false), // 1
        0x32 => (0x03, false), // 2
        0x33 => (0x04, false), // 3
        0x34 => (0x05, false), // 4
        0x35 => (0x06, false), // 5
        0x36 => (0x07, false), // 6
        0x37 => (0x08, false), // 7
        0x38 => (0x09, false), // 8
        0x39 => (0x0A, false), // 9
        0x30 => (0x0B, false), // 0
        0xBD => (0x0C, false), // VK_OEM_MINUS (-)
        0xBB => (0x0D, false), // VK_OEM_PLUS (=)
        0x08 => (0x0E, false), // VK_BACK
        0x09 => (0x0F, false), // VK_TAB
        0x51 => (0x10, false), // Q
        0x57 => (0x11, false), // W
        0x45 => (0x12, false), // E
        0x52 => (0x13, false), // R
        0x54 => (0x14, false), // T
        0x59 => (0x15, false), // Y
        0x55 => (0x16, false), // U
        0x49 => (0x17, false), // I
        0x4F => (0x18, false), // O
        0x50 => (0x19, false), // P
        0xDB => (0x1A, false), // [
        0xDD => (0x1B, false), // ]
        0x0D => (0x1C, false), // VK_RETURN
        0xA2 | 0x11 => (0x1D, false), // LCONTROL / CONTROL
        0x41 => (0x1E, false), // A
        0x53 => (0x1F, false), // S
        0x44 => (0x20, false), // D
        0x46 => (0x21, false), // F
        0x47 => (0x22, false), // G
        0x48 => (0x23, false), // H
        0x4A => (0x24, false), // J
        0x4B => (0x25, false), // K
        0x4C => (0x26, false), // L
        0xBA => (0x27, false), // ;
        0xDE => (0x28, false), // '
        0xC0 => (0x29, false), // `
        0xA0 | 0x10 => (0x2A, false), // LSHIFT / SHIFT
        0xDC => (0x2B, false), // \
        0x5A => (0x2C, false), // Z
        0x58 => (0x2D, false), // X
        0x43 => (0x2E, false), // C
        0x56 => (0x2F, false), // V
        0x42 => (0x30, false), // B
        0x4E => (0x31, false), // N
        0x4D => (0x32, false), // M
        0xBC => (0x33, false), // ,
        0xBE => (0x34, false), // .
        0xBF => (0x35, false), // /
        0xA1 => (0x36, false), // RSHIFT
        0x6A => (0x37, false), // VK_MULTIPLY (*)
        0xA4 | 0x12 => (0x38, false), // LMENU / MENU (Alt)
        0x20 => (0x39, false), // VK_SPACE
        0x14 => (0x3A, false), // VK_CAPITAL
        0x70 => (0x3B, false), // F1
        0x71 => (0x3C, false), // F2
        0x72 => (0x3D, false), // F3
        0x73 => (0x3E, false), // F4
        0x74 => (0x3F, false), // F5
        0x75 => (0x40, false), // F6
        0x76 => (0x41, false), // F7
        0x77 => (0x42, false), // F8
        0x78 => (0x43, false), // F9
        0x79 => (0x44, false), // F10
        0x90 => (0x45, false), // VK_NUMLOCK
        0x91 => (0x46, false), // VK_SCROLL
        0x67 => (0x47, false), // VK_NUMPAD7
        0x68 => (0x48, false), // VK_NUMPAD8
        0x69 => (0x49, false), // VK_NUMPAD9
        0x6D => (0x4A, false), // VK_SUBTRACT
        0x64 => (0x4B, false), // VK_NUMPAD4
        0x65 => (0x4C, false), // VK_NUMPAD5
        0x66 => (0x4D, false), // VK_NUMPAD6
        0x6B => (0x4E, false), // VK_ADD
        0x61 => (0x4F, false), // VK_NUMPAD1
        0x62 => (0x50, false), // VK_NUMPAD2
        0x63 => (0x51, false), // VK_NUMPAD3
        0x60 => (0x52, false), // VK_NUMPAD0
        0x6E => (0x53, false), // VK_DECIMAL
        0x7A => (0x57, false), // F11
        0x7B => (0x58, false), // F12
        // Extended keys (E0 prefix)
        0x26 => (0x48, true),  // VK_UP
        0x25 => (0x4B, true),  // VK_LEFT
        0x27 => (0x4D, true),  // VK_RIGHT
        0x28 => (0x50, true),  // VK_DOWN
        0x2D => (0x52, true),  // VK_INSERT
        0x2E => (0x53, true),  // VK_DELETE
        0x24 => (0x47, true),  // VK_HOME
        0x23 => (0x4F, true),  // VK_END
        0x21 => (0x49, true),  // VK_PRIOR (PageUp)
        0x22 => (0x51, true),  // VK_NEXT (PageDown)
        0xA3 => (0x1D, true),  // VK_RCONTROL
        0xA5 => (0x38, true),  // VK_RMENU (Right Alt)
        0x6F => (0x35, true),  // VK_DIVIDE
        0x5B => (0x5B, true),  // VK_LWIN
        0x5C => (0x5C, true),  // VK_RWIN
        _ => (0, false),
    }
}

fn scancode_to_vk(sc: u16, is_extended: bool) -> u32 {
    if is_extended {
        match sc {
            0x48 => 0x26, // UP
            0x4B => 0x25, // LEFT
            0x4D => 0x27, // RIGHT
            0x50 => 0x28, // DOWN
            0x52 => 0x2D, // INSERT
            0x53 => 0x2E, // DELETE
            0x47 => 0x24, // HOME
            0x4F => 0x23, // END
            0x49 => 0x21, // PAGEUP
            0x51 => 0x22, // PAGEDOWN
            0x1D => 0xA3, // RCONTROL
            0x38 => 0xA5, // RMENU
            0x35 => 0x6F, // DIVIDE
            0x1C => 0x0D, // NUMPAD ENTER
            0x5B => 0x5B, // LWIN
            0x5C => 0x5C, // RWIN
            _ => 0,
        }
    } else {
        match sc {
            0x01 => 0x1B, // ESCAPE
            0x02 => 0x31, // 1
            0x03 => 0x32, // 2
            0x04 => 0x33, // 3
            0x05 => 0x34, // 4
            0x06 => 0x35, // 5
            0x07 => 0x36, // 6
            0x08 => 0x37, // 7
            0x09 => 0x38, // 8
            0x0A => 0x39, // 9
            0x0B => 0x30, // 0
            0x0C => 0xBD, // -
            0x0D => 0xBB, // =
            0x0E => 0x08, // BACK
            0x0F => 0x09, // TAB
            0x10 => 0x51, // Q
            0x11 => 0x57, // W
            0x12 => 0x45, // E
            0x13 => 0x52, // R
            0x14 => 0x54, // T
            0x15 => 0x59, // Y
            0x16 => 0x55, // U
            0x17 => 0x49, // I
            0x18 => 0x4F, // O
            0x19 => 0x50, // P
            0x1A => 0xDB, // [
            0x1B => 0xDD, // ]
            0x1C => 0x0D, // RETURN
            0x1D => 0xA2, // LCONTROL
            0x1E => 0x41, // A
            0x1F => 0x53, // S
            0x20 => 0x44, // D
            0x21 => 0x46, // F
            0x22 => 0x47, // G
            0x23 => 0x48, // H
            0x24 => 0x4A, // J
            0x25 => 0x4B, // K
            0x26 => 0x4C, // L
            0x27 => 0xBA, // ;
            0x28 => 0xDE, // '
            0x29 => 0xC0, // `
            0x2A => 0xA0, // LSHIFT
            0x2B => 0xDC, // \
            0x2C => 0x5A, // Z
            0x2D => 0x58, // X
            0x2E => 0x43, // C
            0x2F => 0x56, // V
            0x30 => 0x42, // B
            0x31 => 0x4E, // N
            0x32 => 0x4D, // M
            0x33 => 0xBC, // ,
            0x34 => 0xBE, // .
            0x35 => 0xBF, // /
            0x36 => 0xA1, // RSHIFT
            0x37 => 0x6A, // *
            0x38 => 0xA4, // LMENU
            0x39 => 0x20, // SPACE
            0x3A => 0x14, // CAPITAL
            0x3B => 0x70, // F1
            0x3C => 0x71, // F2
            0x3D => 0x72, // F3
            0x3E => 0x73, // F4
            0x3F => 0x74, // F5
            0x40 => 0x75, // F6
            0x41 => 0x76, // F7
            0x42 => 0x77, // F8
            0x43 => 0x78, // F9
            0x44 => 0x79, // F10
            0x45 => 0x90, // NUMLOCK
            0x46 => 0x91, // SCROLL
            0x47 => 0x67, // NUMPAD7
            0x48 => 0x68, // NUMPAD8
            0x49 => 0x69, // NUMPAD9
            0x4A => 0x6D, // NUMPAD -
            0x4B => 0x64, // NUMPAD4
            0x4C => 0x65, // NUMPAD5
            0x4D => 0x66, // NUMPAD6
            0x4E => 0x6B, // NUMPAD +
            0x4F => 0x61, // NUMPAD1
            0x50 => 0x62, // NUMPAD2
            0x51 => 0x63, // NUMPAD3
            0x52 => 0x60, // NUMPAD0
            0x53 => 0x6E, // NUMPAD .
            0x57 => 0x7A, // F11
            0x58 => 0x7B, // F12
            _ => 0,
        }
    }
}

fn ensure_window_active_for_input(
    input: &mut InputSubsystem,
    target_hwnd: u64,
    tick: u32,
    pt_x: i32,
    pt_y: i32,
    is_click: bool,
) {
    let was_active = WINDOW_ACTIVATED.swap(true, Ordering::Relaxed);
    if !was_active || is_click {
        if !was_active {
            println!(
                "[USER32 SDL] Activating guest window (is_click={}, HWND=0x{:X})",
                is_click, target_hwnd
            );
            input.message_queue.push_back(WinMsg {
                hwnd: target_hwnd,
                message: 0x001C, // WM_ACTIVATEAPP
                w_param: 1,
                l_param: 0,
                time: tick,
                pt_x,
                pt_y,
            });
        }
        if is_click {
            input.message_queue.push_back(WinMsg {
                hwnd: target_hwnd,
                message: 0x0021, // WM_MOUSEACTIVATE
                w_param: target_hwnd,
                l_param: 0x02010001, // HTCLIENT | (WM_LBUTTONDOWN << 16)
                time: tick,
                pt_x,
                pt_y,
            });
            input.message_queue.push_back(WinMsg {
                hwnd: target_hwnd,
                message: 0x0006, // WM_ACTIVATE
                w_param: 2, // WA_CLICKACTIVE
                l_param: 0,
                time: tick,
                pt_x,
                pt_y,
            });
        } else if !was_active {
            input.message_queue.push_back(WinMsg {
                hwnd: target_hwnd,
                message: 0x0006, // WM_ACTIVATE
                w_param: 1, // WA_ACTIVE
                l_param: 0,
                time: tick,
                pt_x,
                pt_y,
            });
        }
        input.message_queue.push_back(WinMsg {
            hwnd: target_hwnd,
            message: 0x0007, // WM_SETFOCUS
            w_param: 0,
            l_param: 0,
            time: tick,
            pt_x,
            pt_y,
        });
    }
}

fn poll_and_enqueue_sdl_event(sdl_event: &[u8; 56]) {
    let event_type = u32::from_le_bytes([sdl_event[0], sdl_event[1], sdl_event[2], sdl_event[3]]);
    let target_hwnd = PRIMARY_HWND.load(Ordering::Relaxed);
    let (wx, wy) = refresh_cached_window_position();
    let tick = unsafe { crate::dlls::kernel32::GetTickCount() };

    let mut input_guard = get_user32_input();
    let input = match input_guard.as_mut() {
        Some(i) => i,
        None => return,
    };

    match event_type {
        0x400 => {
            // SDL_MOUSEMOTION
            let x = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let y = i32::from_le_bytes(sdl_event[24..28].try_into().unwrap());
            let mut xrel = i32::from_le_bytes(sdl_event[28..32].try_into().unwrap());
            let mut yrel = i32::from_le_bytes(sdl_event[32..36].try_into().unwrap());
            let state = u32::from_le_bytes(sdl_event[16..20].try_into().unwrap());
            LAST_CURSOR_X.store(x, Ordering::Relaxed);
            LAST_CURSOR_Y.store(y, Ordering::Relaxed);

            // Suppress synthetic motion deltas from SetCursorPos warping
            if WARP_PENDING.load(Ordering::Relaxed) {
                let wx_t = WARP_TARGET_X.load(Ordering::Relaxed);
                let wy_t = WARP_TARGET_Y.load(Ordering::Relaxed);
                if (x - wx_t).abs() <= 1 && (y - wy_t).abs() <= 1 {
                    WARP_PENDING.store(false, Ordering::Relaxed);
                    xrel = 0;
                    yrel = 0;
                }
            }

            ensure_window_active_for_input(input, target_hwnd, tick, wx + x, wy + y, false);

            let raw_mouse = RawMouse {
                us_flags: 0, // MOUSE_MOVE_RELATIVE
                _pad: 0,
                us_button_flags: 0,
                us_button_data: 0,
                ul_raw_buttons: 0,
                l_last_x: xrel,
                l_last_y: yrel,
                ul_extra_info: 0,
            };
            let raw_handle = push_raw_input_event(RawInput::new_mouse(raw_mouse));

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

            // 1. WM_INPUT if registered
            if RAW_INPUT_REGISTERED_MOUSE.load(Ordering::Relaxed) {
                let mouse_hwnd = RAW_INPUT_MOUSE_HWND.load(Ordering::Relaxed);
                let target = if mouse_hwnd != 0 { mouse_hwnd } else { target_hwnd };
                input.message_queue.push_back(WinMsg {
                    hwnd: target,
                    message: 0x00FF, // WM_INPUT
                    w_param: 0,
                    l_param: raw_handle as u64,
                    time: tick,
                    pt_x: wx + x,
                    pt_y: wy + y,
                });
            }

            // 2. WM_MOUSEMOVE
            input.message_queue.push_back(WinMsg {
                hwnd: target_hwnd,
                message: 0x0200, // WM_MOUSEMOVE
                w_param,
                l_param,
                time: tick,
                pt_x: wx + x,
                pt_y: wy + y,
            });
        }
        0x401 => {
            // SDL_MOUSEBUTTONDOWN
            let button = sdl_event[16];
            let x = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let y = i32::from_le_bytes(sdl_event[24..28].try_into().unwrap());
            LAST_CURSOR_X.store(x, Ordering::Relaxed);
            LAST_CURSOR_Y.store(y, Ordering::Relaxed);
            ensure_window_active_for_input(input, target_hwnd, tick, wx + x, wy + y, true);
            let l_param = ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF);

            match button {
                1 => KEY_STATE_TABLE[0x01].store(true, Ordering::Relaxed),
                2 => KEY_STATE_TABLE[0x04].store(true, Ordering::Relaxed), // MK_MBUTTON
                3 => KEY_STATE_TABLE[0x02].store(true, Ordering::Relaxed), // MK_RBUTTON
                _ => {}
            }

            let (msg, wp, raw_flag) = match button {
                1 => (0x0201, 0x0001, 0x0001), // WM_LBUTTONDOWN, MK_LBUTTON, RI_MOUSE_LEFT_BUTTON_DOWN
                3 => (0x0204, 0x0002, 0x0004), // WM_RBUTTONDOWN, MK_RBUTTON, RI_MOUSE_RIGHT_BUTTON_DOWN
                2 => (0x0207, 0x0010, 0x0010), // WM_MBUTTONDOWN, MK_MBUTTON, RI_MOUSE_MIDDLE_BUTTON_DOWN
                _ => (0, 0, 0),
            };

            if msg != 0 {
                let raw_mouse = RawMouse {
                    us_flags: 0,
                    _pad: 0,
                    us_button_flags: raw_flag,
                    us_button_data: 0,
                    ul_raw_buttons: 0,
                    l_last_x: 0,
                    l_last_y: 0,
                    ul_extra_info: 0,
                };
                let raw_handle = push_raw_input_event(RawInput::new_mouse(raw_mouse));

                // 1. WM_INPUT if registered
                if RAW_INPUT_REGISTERED_MOUSE.load(Ordering::Relaxed) {
                    let mouse_hwnd = RAW_INPUT_MOUSE_HWND.load(Ordering::Relaxed);
                    let target = if mouse_hwnd != 0 { mouse_hwnd } else { target_hwnd };
                    input.message_queue.push_back(WinMsg {
                        hwnd: target,
                        message: 0x00FF, // WM_INPUT
                        w_param: 0,
                        l_param: raw_handle as u64,
                        time: tick,
                        pt_x: wx + x,
                        pt_y: wy + y,
                    });
                }

                // 2. Click message (WM_LBUTTONDOWN / WM_RBUTTONDOWN / WM_MBUTTONDOWN)
                input.message_queue.push_back(WinMsg {
                    hwnd: target_hwnd,
                    message: msg,
                    w_param: wp,
                    l_param,
                    time: tick,
                    pt_x: wx + x,
                    pt_y: wy + y,
                });
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
                1 => KEY_STATE_TABLE[0x01].store(false, Ordering::Relaxed),
                2 => KEY_STATE_TABLE[0x04].store(false, Ordering::Relaxed),
                3 => KEY_STATE_TABLE[0x02].store(false, Ordering::Relaxed),
                _ => {}
            }

            let (msg, raw_flag) = match button {
                1 => (0x0202, 0x0002), // WM_LBUTTONUP, RI_MOUSE_LEFT_BUTTON_UP
                3 => (0x0205, 0x0008), // WM_RBUTTONUP, RI_MOUSE_RIGHT_BUTTON_UP
                2 => (0x0208, 0x0020), // WM_MBUTTONUP, RI_MOUSE_MIDDLE_BUTTON_UP
                _ => (0, 0),
            };

            if msg != 0 {
                let raw_mouse = RawMouse {
                    us_flags: 0,
                    _pad: 0,
                    us_button_flags: raw_flag,
                    us_button_data: 0,
                    ul_raw_buttons: 0,
                    l_last_x: 0,
                    l_last_y: 0,
                    ul_extra_info: 0,
                };
                let raw_handle = push_raw_input_event(RawInput::new_mouse(raw_mouse));

                // 1. WM_INPUT if registered
                if RAW_INPUT_REGISTERED_MOUSE.load(Ordering::Relaxed) {
                    let mouse_hwnd = RAW_INPUT_MOUSE_HWND.load(Ordering::Relaxed);
                    let target = if mouse_hwnd != 0 { mouse_hwnd } else { target_hwnd };
                    input.message_queue.push_back(WinMsg {
                        hwnd: target,
                        message: 0x00FF, // WM_INPUT
                        w_param: 0,
                        l_param: raw_handle as u64,
                        time: tick,
                        pt_x: wx + x,
                        pt_y: wy + y,
                    });
                }

                // 2. WM_LBUTTONUP
                input.message_queue.push_back(WinMsg {
                    hwnd: target_hwnd,
                    message: msg,
                    w_param: 0,
                    l_param,
                    time: tick,
                    pt_x: wx + x,
                    pt_y: wy + y,
                });
            }
        }
        0x403 => {
            // SDL_MOUSEWHEEL
            let wheel_y = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let x = LAST_CURSOR_X.load(Ordering::Relaxed);
            let y = LAST_CURSOR_Y.load(Ordering::Relaxed);
            let l_param = ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF);
            let w_param = ((wheel_y * 120) as u64 & 0xFFFF) << 16;
            let raw_mouse = RawMouse {
                us_flags: 0,
                _pad: 0,
                us_button_flags: 0x0400, // RI_MOUSE_WHEEL
                us_button_data: (wheel_y * 120) as u16,
                ul_raw_buttons: 0,
                l_last_x: 0,
                l_last_y: 0,
                ul_extra_info: 0,
            };
            let raw_handle = push_raw_input_event(RawInput::new_mouse(raw_mouse));
            if RAW_INPUT_REGISTERED_MOUSE.load(Ordering::Relaxed) {
                let mouse_hwnd = RAW_INPUT_MOUSE_HWND.load(Ordering::Relaxed);
                let target = if mouse_hwnd != 0 { mouse_hwnd } else { target_hwnd };
                input.message_queue.push_back(WinMsg {
                    hwnd: target,
                    message: 0x00FF,
                    w_param: 0,
                    l_param: raw_handle as u64,
                    time: tick,
                    pt_x: wx + x,
                    pt_y: wy + y,
                });
            }
            input.message_queue.push_back(WinMsg {
                hwnd: target_hwnd,
                message: 0x020A, // WM_MOUSEWHEEL
                w_param,
                l_param,
                time: tick,
                pt_x: wx + x,
                pt_y: wy + y,
            });
        }
        0x300 => {
            // SDL_KEYDOWN
            let sym = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let vk = map_sdl_key_to_vk(sym);
            let (scancode, is_extended) = vk_to_scancode(vk);
            let mut was_down = false;
            if (vk as usize) < 256 {
                was_down = KEY_STATE_TABLE[vk as usize].swap(true, Ordering::Relaxed);
                if vk == 0xA0 || vk == 0xA1 {
                    KEY_STATE_TABLE[0x10].store(true, Ordering::Relaxed);
                }
                if vk == 0xA2 || vk == 0xA3 {
                    KEY_STATE_TABLE[0x11].store(true, Ordering::Relaxed);
                }
                if vk == 0xA4 || vk == 0xA5 {
                    KEY_STATE_TABLE[0x12].store(true, Ordering::Relaxed);
                }
                // In-game overlay hotkey disabled per user request
                // if (vk == 0x7B || vk == 0x09) && KEY_STATE_TABLE[0x10].load(Ordering::Relaxed) {
                //     toggle_in_game_overlay();
                // }
            }

            let alt_down = KEY_STATE_TABLE[0x12].load(Ordering::Relaxed)
                || KEY_STATE_TABLE[0xA4].load(Ordering::Relaxed)
                || KEY_STATE_TABLE[0xA5].load(Ordering::Relaxed);

            let msg = if alt_down && vk != 0x12 && vk != 0xA4 && vk != 0xA5 {
                0x0104 // WM_SYSKEYDOWN
            } else {
                0x0100 // WM_KEYDOWN
            };

            let mut l_param = 1u64; // repeat count = 1
            l_param |= ((scancode as u64) & 0xFF) << 16;
            if is_extended {
                l_param |= 1 << 24;
            }
            if alt_down {
                l_param |= 1 << 29;
            }
            if was_down {
                l_param |= 1 << 30;
            }

            if RAW_INPUT_REGISTERED_KEYBOARD.load(Ordering::Relaxed) {
                let raw_kbd = RawKeyboard {
                    make_code: scancode,
                    flags: if is_extended { 2 } else { 0 }, // RI_KEY_MAKE | (RI_KEY_E0 if extended)
                    reserved: 0,
                    v_key: vk as u16,
                    message: msg,
                    extra_information: 0,
                    _pad: [0; 8],
                };
                let raw_handle = push_raw_input_event(RawInput::new_keyboard(raw_kbd));
                let kbd_hwnd = RAW_INPUT_KBD_HWND.load(Ordering::Relaxed);
                let target = if kbd_hwnd != 0 { kbd_hwnd } else { target_hwnd };
                input.message_queue.push_back(WinMsg {
                    hwnd: target,
                    message: 0x00FF, // WM_INPUT
                    w_param: 0, // RIM_INPUT
                    l_param: raw_handle as u64,
                    time: tick,
                    pt_x: 0,
                    pt_y: 0,
                });
            }

            input.message_queue.push_back(WinMsg {
                hwnd: target_hwnd,
                message: msg,
                w_param: vk as u64,
                l_param,
                time: tick,
                pt_x: 0,
                pt_y: 0,
            });
        }
        0x301 => {
            // SDL_KEYUP
            let sym = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
            let vk = map_sdl_key_to_vk(sym);
            let (scancode, is_extended) = vk_to_scancode(vk);
            if (vk as usize) < 256 {
                KEY_STATE_TABLE[vk as usize].store(false, Ordering::Relaxed);
                if vk == 0xA0 || vk == 0xA1 {
                    let other = if vk == 0xA0 { 0xA1 } else { 0xA0 };
                    if !KEY_STATE_TABLE[other].load(Ordering::Relaxed) {
                        KEY_STATE_TABLE[0x10].store(false, Ordering::Relaxed);
                    }
                }
                if vk == 0xA2 || vk == 0xA3 {
                    let other = if vk == 0xA2 { 0xA3 } else { 0xA2 };
                    if !KEY_STATE_TABLE[other].load(Ordering::Relaxed) {
                        KEY_STATE_TABLE[0x11].store(false, Ordering::Relaxed);
                    }
                }
                if vk == 0xA4 || vk == 0xA5 {
                    let other = if vk == 0xA4 { 0xA5 } else { 0xA4 };
                    if !KEY_STATE_TABLE[other].load(Ordering::Relaxed) {
                        KEY_STATE_TABLE[0x12].store(false, Ordering::Relaxed);
                    }
                }
            }

            let alt_down = KEY_STATE_TABLE[0x12].load(Ordering::Relaxed)
                || KEY_STATE_TABLE[0xA4].load(Ordering::Relaxed)
                || KEY_STATE_TABLE[0xA5].load(Ordering::Relaxed);

            let msg = if alt_down || vk == 0x12 || vk == 0xA4 || vk == 0xA5 {
                0x0105 // WM_SYSKEYUP
            } else {
                0x0101 // WM_KEYUP
            };

            let mut l_param = 1u64; // repeat count = 1
            l_param |= ((scancode as u64) & 0xFF) << 16;
            if is_extended {
                l_param |= 1 << 24;
            }
            if alt_down {
                l_param |= 1 << 29;
            }
            l_param |= 1 << 30; // previous key state was down
            l_param |= 1 << 31; // transition state: key being released

            if RAW_INPUT_REGISTERED_KEYBOARD.load(Ordering::Relaxed) {
                let raw_kbd = RawKeyboard {
                    make_code: scancode,
                    flags: 1 | (if is_extended { 2 } else { 0 }), // RI_KEY_BREAK | (RI_KEY_E0 if extended)
                    reserved: 0,
                    v_key: vk as u16,
                    message: msg,
                    extra_information: 0,
                    _pad: [0; 8],
                };
                let raw_handle = push_raw_input_event(RawInput::new_keyboard(raw_kbd));
                let kbd_hwnd = RAW_INPUT_KBD_HWND.load(Ordering::Relaxed);
                let target = if kbd_hwnd != 0 { kbd_hwnd } else { target_hwnd };
                input.message_queue.push_back(WinMsg {
                    hwnd: target,
                    message: 0x00FF, // WM_INPUT
                    w_param: 0, // RIM_INPUT
                    l_param: raw_handle as u64,
                    time: tick,
                    pt_x: 0,
                    pt_y: 0,
                });
            }

            input.message_queue.push_back(WinMsg {
                hwnd: target_hwnd,
                message: msg,
                w_param: vk as u64,
                l_param,
                time: tick,
                pt_x: 0,
                pt_y: 0,
            });
        }
        0x200 => {
            // SDL_WINDOWEVENT
            let event_id = sdl_event[12];
            match event_id {
                0x04 => {
                    // SDL_WINDOWEVENT_MOVED
                    let data1 = i32::from_le_bytes(sdl_event[16..20].try_into().unwrap());
                    let data2 = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
                    CACHED_WIN_X.store(data1, Ordering::Relaxed);
                    CACHED_WIN_Y.store(data2, Ordering::Relaxed);
                    let l_param = ((data2 as u64 & 0xFFFF) << 16) | (data1 as u64 & 0xFFFF);
                    input.message_queue.push_back(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0003, // WM_MOVE
                        w_param: 0,
                        l_param,
                        time: tick,
                        pt_x: 0,
                        pt_y: 0,
                    });
                    drop(input_guard);
                    let (cur_w, cur_h) = crate::windowing::x11::sdl_get_window_size();
                    let mut winpos = WindowPos {
                        hwnd: target_hwnd,
                        hwnd_insert_after: 0,
                        x: data1,
                        y: data2,
                        cx: cur_w,
                        cy: cur_h,
                        flags: 0x0015, // SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE
                        _pad: 0,
                    };
                    unsafe {
                        SendMessageW(
                            target_hwnd,
                            0x0047, // WM_WINDOWPOSCHANGED
                            0,
                            &mut winpos as *mut WindowPos as u64,
                        );
                    }
                    return;
                }
                0x05 | 0x06 => {
                    // SDL_WINDOWEVENT_RESIZED (0x05), SDL_WINDOWEVENT_SIZE_CHANGED (0x06)
                    let new_w = i32::from_le_bytes(sdl_event[16..20].try_into().unwrap());
                    let new_h = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
                    if new_w > 0 && new_h > 0 {
                        crate::graphics::vulkan_bridge::handles::WindowRegistry::global()
                            .update_window_size(target_hwnd as usize, new_w as u32, new_h as u32);
                        static LAST_DISPATCHED_SIZE: AtomicU64 = AtomicU64::new(0);
                        let packed = ((new_w as u64) << 32) | (new_h as u64);
                        if LAST_DISPATCHED_SIZE.swap(packed, Ordering::Relaxed) != packed {
                            println!(
                                "[USER32] Window resized to {}x{}, queuing WM_SIZE & sending WM_WINDOWPOSCHANGED",
                                new_w, new_h
                            );
                            let l_param = ((new_h as u64 & 0xFFFF) << 16) | (new_w as u64 & 0xFFFF);
                            input.message_queue.push_back(WinMsg {
                                hwnd: target_hwnd,
                                message: 0x0005, // WM_SIZE
                                w_param: 0, // SIZE_RESTORED
                                l_param,
                                time: tick,
                                pt_x: 0,
                                pt_y: 0,
                            });
                            input.message_queue.push_back(WinMsg {
                                hwnd: target_hwnd,
                                message: 0x000F, // WM_PAINT
                                w_param: 0,
                                l_param: 0,
                                time: tick,
                                pt_x: 0,
                                pt_y: 0,
                            });
                            drop(input_guard);
                            let mut winpos = WindowPos {
                                hwnd: target_hwnd,
                                hwnd_insert_after: 0,
                                x: wx,
                                y: wy,
                                cx: new_w,
                                cy: new_h,
                                flags: 0x0014, // SWP_NOZORDER | SWP_NOACTIVATE (SWP_NOSIZE is NOT set!)
                                _pad: 0,
                            };
                            unsafe {
                                SendMessageW(
                                    target_hwnd,
                                    0x0047, // WM_WINDOWPOSCHANGED
                                    0,
                                    &mut winpos as *mut WindowPos as u64,
                                );
                            }
                            return;
                        }
                    }
                }
                0x01 | 0x03 => {
                    // SHOWN, EXPOSED
                    refresh_cached_window_position();
                    input.message_queue.push_back(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x000F, // WM_PAINT
                        w_param: 0,
                        l_param: 0,
                        time: tick,
                        pt_x: 0,
                        pt_y: 0,
                    });
                }
                0x0A => {
                    // ENTER
                    WINDOW_ACTIVATED.store(true, Ordering::Relaxed);
                    refresh_cached_window_position();
                    input.message_queue.push_back(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0006, // WM_ACTIVATE
                        w_param: 1, // WA_ACTIVE
                        l_param: 0,
                        time: tick,
                        pt_x: 0,
                        pt_y: 0,
                    });
                    input.message_queue.push_back(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0007, // WM_SETFOCUS
                        w_param: 0,
                        l_param: 0,
                        time: tick,
                        pt_x: 0,
                        pt_y: 0,
                    });
                }
                0x0C => {
                    // SDL_WINDOWEVENT_FOCUS_GAINED
                    WINDOW_ACTIVATED.store(true, Ordering::Relaxed);
                    if IS_CURSOR_CLIPPED.load(Ordering::Relaxed) {
                        if let Some(f) = get_sdl_set_relative_mouse_mode_fn() {
                            unsafe { f(1); }
                        }
                    }
                    input.message_queue.push_back(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x001C, // WM_ACTIVATEAPP
                        w_param: 1,
                        l_param: 0,
                        time: tick,
                        pt_x: 0,
                        pt_y: 0,
                    });
                    input.message_queue.push_back(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0006, // WM_ACTIVATE
                        w_param: 1, // WA_ACTIVE
                        l_param: 0,
                        time: tick,
                        pt_x: 0,
                        pt_y: 0,
                    });
                    input.message_queue.push_back(WinMsg {
                        hwnd: target_hwnd,
                        message: 0x0007, // WM_SETFOCUS
                        w_param: 0,
                        l_param: 0,
                        time: tick,
                        pt_x: 0,
                        pt_y: 0,
                    });
                }
                0x0D => {
                    // SDL_WINDOWEVENT_FOCUS_LOST
                    // Under Linux compositors (e.g. Hyprland/Wayland), focus can flutter.
                    // We update WINDOW_ACTIVATED so subsequent mouse interaction immediately restores focus,
                    // but we DO NOT send WM_ACTIVATE(WA_INACTIVE) to avoid freezing Unity's mouse pipeline.
                    WINDOW_ACTIVATED.store(false, Ordering::Relaxed);
                    if let Some(f) = get_sdl_set_relative_mouse_mode_fn() {
                        unsafe { f(0); }
                    }
                }
                0x0E => {
                    // SDL_WINDOWEVENT_CLOSE
                    println!("[USER32] Window close event received (Win+Q). Cleanly terminating process...");
                    crate::sys::shutdown::request_host_shutdown("Win+Q window close");
                    unsafe { libc::_exit(0); }
                }
                _ => {}
            }
        }
        0x100 => {
            // SDL_QUIT
            println!("[USER32] SDL_QUIT received (Win+Q). Cleanly terminating process...");
            crate::sys::shutdown::request_host_shutdown("Win+Q SDL_QUIT");
            unsafe { libc::_exit(0); }
        }
        _ => {}
    }
}

#[allow(dead_code)]
fn translate_sdl_event(sdl_event: &[u8; 56]) -> Option<WinMsg> {
    poll_and_enqueue_sdl_event(sdl_event);
    let mut guard = get_user32_input();
    if let Some(input) = guard.as_mut() {
        input.message_queue.pop_back()
    } else {
        None
    }
}

#[derive(Clone, Copy)]
pub struct WinTimer {
    pub hwnd: usize,
    pub id: usize,
    pub elapse: u32,
    pub func: usize,
    pub last_tick: std::time::Instant,
}

pub static ACTIVE_TIMERS: std::sync::Mutex<Vec<WinTimer>> = std::sync::Mutex::new(Vec::new());

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
    let hwnd = if _h_wnd != 0 && _h_wnd != u64::MAX {
        _h_wnd
    } else {
        PRIMARY_HWND.load(Ordering::Relaxed)
    };
    if hwnd != 0 {
        let step = INIT_MSG_STEP.load(Ordering::Relaxed);
        if step < 4 {
            let (msg, wp, lp) = match step {
                0 => (0x0018 /* WM_SHOWWINDOW */, 1, 0),
                1 => (0x0006 /* WM_ACTIVATE */, 1 /* WA_ACTIVE */, 0),
                2 => (0x0007 /* WM_SETFOCUS */, 0, 0),
                _ => (0x000F /* WM_PAINT */, 0, 0),
            };
            if (w_remove_msg & 1) != 0 {
                INIT_MSG_STEP.fetch_add(1, Ordering::Relaxed);
            }
            write_win64_msg(lp_msg, hwnd, msg, wp, lp, crate::dlls::kernel32::GetTickCount(), 0, 0);
            return 1;
        }
    }

    // 2. Poll SDL2 events and enqueue into message_queue
    if let Some(fn_poll) = get_sdl_poll_event_fn() {
        let mut sdl_event = [0u8; 56];
        while fn_poll(sdl_event.as_mut_ptr()) != 0 {
            poll_and_enqueue_sdl_event(&sdl_event);
        }
    }

    // 3. Retrieve next message
    let should_remove = (w_remove_msg & 1) != 0; // PM_REMOVE
    let next_msg = {
        let mut input_guard = get_user32_input();
        if let Some(input) = input_guard.as_mut() {
            let filter_active = _w_msg_filter_max > 0 && _w_msg_filter_max >= _w_msg_filter_min;
            let target_pos = input.message_queue.iter().position(|m| {
                let hwnd_ok = _h_wnd == 0 || (_h_wnd == u64::MAX && m.hwnd == 0) || m.hwnd == _h_wnd;
                let filter_ok = !filter_active || (m.message >= _w_msg_filter_min && m.message <= _w_msg_filter_max);
                hwnd_ok && filter_ok
            });

            if let Some(idx) = target_pos {
                if should_remove {
                    input.message_queue.remove(idx)
                } else {
                    input.message_queue.get(idx).cloned()
                }
            } else {
                None
            }
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
            msg.time,
            msg.pt_x,
            msg.pt_y,
        );
        return 1;
    }

    // 3. Check active timers (WM_TIMER = 0x0113)
    if let Ok(mut timers) = ACTIVE_TIMERS.lock() {
        let now = std::time::Instant::now();
        for t in timers.iter_mut() {
            if now.duration_since(t.last_tick).as_millis() >= t.elapse as u128 {
                t.last_tick = now;
                let target_hwnd = if t.hwnd != 0 { t.hwnd as u64 } else { hwnd };
                if (_h_wnd == 0 || _h_wnd == target_hwnd)
                    && (_w_msg_filter_max == 0 || (0x0113 >= _w_msg_filter_min && 0x0113 <= _w_msg_filter_max))
                {
                    write_win64_msg(
                        lp_msg,
                        target_hwnd,
                        0x0113, // WM_TIMER
                        t.id as u64,
                        t.func as u64,
                        crate::dlls::kernel32::GetTickCount(),
                        LAST_CURSOR_X.load(Ordering::Relaxed),
                        LAST_CURSOR_Y.load(Ordering::Relaxed),
                    );
                    return 1;
                }
            }
        }
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
        std::thread::sleep(std::time::Duration::from_millis(1));
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

        let wndproc = get_wndproc_for_hwnd(hwnd);
        if wndproc > 0x10000 && wndproc < 0x7FFFFFFFFFFF {
            if wndproc == 0x1811BDE30 {
                let active_ptr = (wndproc + 0x10B4EAD) as *mut u8;
                let susp_ptr = (wndproc + 0x10B4EA8) as *mut u8;
                let old_act = *active_ptr;
                let old_susp = *susp_ptr;
                if old_act == 0 || old_susp != 0 {
                    println!(
                        "[USER32 UNITY] Enforcing active state: active_flag {} -> 1, susp_flag {} -> 0",
                        old_act, old_susp
                    );
                    *active_ptr = 1;
                    *susp_ptr = 0;
                }
            }

            let wndproc_fn: unsafe extern "win64" fn(u64, u32, u64, u64) -> isize =
                std::mem::transmute(wndproc);
            return wndproc_fn(hwnd, msg, wparam, lparam);
        } else {
            static UNREGISTERED_WNDPROC_COUNT: AtomicU64 = AtomicU64::new(0);
            if UNREGISTERED_WNDPROC_COUNT.fetch_add(1, Ordering::Relaxed) < 10 {
                println!("[USER32 WNDPROC] DispatchMessageW with UNREGISTERED or NULL wndproc=0x{:X}, msg=0x{:04X}", wndproc, msg);
            }
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
    hwnd: u64,
    msg: u32,
    wparam: u64,
    lparam: u64,
) -> isize {
    match msg {
        0x0010 => {
            // WM_CLOSE: Standard Win32 calls DestroyWindow(hwnd)
            println!("[USER32] DefWindowProcW WM_CLOSE -> calling DestroyWindow(0x{:X})", hwnd);
            DestroyWindow(hwnd as usize);
            0
        }
        0x0081 => 1, // WM_NCCREATE: Must return TRUE (1)
        0x0082 => 0, // WM_NCDESTROY
        0x0084 => 1, // WM_NCHITTEST: HTCLIENT
        0x0020 => 1, // WM_SETCURSOR: Handled
        0x0021 => 1, // WM_MOUSEACTIVATE: MA_ACTIVATE (1)
        0x0086 => 1, // WM_NCACTIVATE: TRUE (1)
        0x0014 => 1, // WM_ERASEBKGND: Erased
        0x0006 => 0, // WM_ACTIVATE
        0x0007 => 0, // WM_SETFOCUS
        0x0008 => 0, // WM_KILLFOCUS
        0x0018 => 0, // WM_SHOWWINDOW
        0x00FF => 0, // WM_INPUT: handled/cleaned
        0x0047 => 0, // WM_WINDOWPOSCHANGED
        0x0215 => 0, // WM_CAPTURECHANGED
        _ => 0,
    }
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
pub unsafe extern "win64" fn TranslateMessage(lp_msg: *const u8) -> i32 {
    if !lp_msg.is_null() && (lp_msg as usize) > 0x10000 {
        let msg = std::ptr::read_unaligned(lp_msg.add(8) as *const u32);
        if msg == 0x0100 /* WM_KEYDOWN */ {
            let hwnd = std::ptr::read_unaligned(lp_msg as *const u64);
            let wparam = std::ptr::read_unaligned(lp_msg.add(16) as *const u64);
            let lparam = std::ptr::read_unaligned(lp_msg.add(24) as *const u64);
            let char_code = if (0x20..=0x7E).contains(&wparam) {
                let shift = KEY_STATE_TABLE[0x10].load(Ordering::Relaxed);
                if (0x41..=0x5A).contains(&wparam) && !shift {
                    wparam + 32
                } else {
                    wparam
                }
            } else if wparam == 0x0D || wparam == 0x08 || wparam == 0x1B {
                wparam
            } else {
                0
            };
            if char_code != 0 {
                let mut input_guard = get_user32_input();
                if let Some(input) = input_guard.as_mut() {
                    input.message_queue.push_back(WinMsg {
                        hwnd,
                        message: 0x0102, // WM_CHAR
                        w_param: char_code,
                        l_param: lparam,
                        time: crate::dlls::kernel32::GetTickCount(),
                        pt_x: 0,
                        pt_y: 0,
                    });
                }
                return 1;
            }
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SendMessageW(hwnd: u64, msg: u32, wparam: u64, lparam: u64) -> isize {
    println!(
        "[USER32] SendMessageW(hWnd=0x{:X}, msg=0x{:X}, wParam=0x{:X}, lParam=0x{:X})",
        hwnd, msg, wparam, lparam
    );
    let wndproc = GODOT_WNDPROC.load(Ordering::Relaxed);
    if wndproc > 0x10000 && wndproc < 0x7FFFFFFFFFFF {
        let wndproc_fn: unsafe extern "win64" fn(u64, u32, u64, u64) -> isize =
            std::mem::transmute(wndproc);
        return wndproc_fn(hwnd, msg, wparam, lparam);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[no_mangle]
pub unsafe extern "win64" fn SendMessageA(hwnd: u64, msg: u32, wparam: u64, lparam: u64) -> isize {
    SendMessageW(hwnd, msg, wparam, lparam)
}

#[no_mangle]
pub unsafe extern "win64" fn PostMessageW(hwnd: u64, msg: u32, wparam: u64, lparam: u64) -> i32 {
    let mut input_guard = get_user32_input();
    if let Some(input) = input_guard.as_mut() {
        let tick = unsafe { crate::dlls::kernel32::GetTickCount() };
        let pt_x = LAST_CURSOR_X.load(Ordering::Relaxed);
        let pt_y = LAST_CURSOR_Y.load(Ordering::Relaxed);
        input.message_queue.push_back(WinMsg {
            hwnd,
            message: msg,
            w_param: wparam,
            l_param: lparam,
            time: tick,
            pt_x,
            pt_y,
        });
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn PostMessageA(hwnd: u64, msg: u32, wparam: u64, lparam: u64) -> i32 {
    PostMessageW(hwnd, msg, wparam, lparam)
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
        23 => 0,                       // SM_SWAPBUTTON (0 = normal mouse layout)
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
    let mut stack_vals = [0u64; 8];
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let rbp_val: u64;
        let rsp_val: u64;
        std::arch::asm!("mov {}, rbp", "mov {}, rsp", out(reg) rbp_val, out(reg) rsp_val);
        println!("[GODOT MESSAGE BOX A] rbp=0x{:X}, rsp=0x{:X}", rbp_val, rsp_val);
        if rbp_val > 0x10000 {
            let rbp_ptr = rbp_val as *const u64;
            for i in 0..8 {
                stack_vals[i] = *rbp_ptr.offset(i as isize);
            }
            println!("[GODOT MESSAGE BOX A] rbp relative: {:X?}", stack_vals);
        }
    }
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
    if is_primary_window(hwnd) {
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

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawInputDevice {
    pub us_usage_page: u16,
    pub us_usage: u16,
    pub dw_flags: u32,
    pub hwnd_target: u64,
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterRawInputDevices(
    p_raw_input_devices: *const RawInputDevice,
    ui_num_devices: u32,
    cb_size: u32,
) -> i32 {
    if p_raw_input_devices.is_null() || cb_size < std::mem::size_of::<RawInputDevice>() as u32 {
        return 0;
    }
    for i in 0..ui_num_devices {
        let dev = &*p_raw_input_devices.add(i as usize);
        let is_remove = (dev.dw_flags & 1) != 0; // RIDEV_REMOVE
        if dev.us_usage_page == 1 && dev.us_usage == 2 {
            RAW_INPUT_REGISTERED_MOUSE.store(!is_remove, Ordering::Relaxed);
            RAW_INPUT_MOUSE_HWND.store(if !is_remove { dev.hwnd_target } else { 0 }, Ordering::Relaxed);
            println!("[USER32] RegisterRawInputDevices: Mouse active={}, hwnd=0x{:X}", !is_remove, dev.hwnd_target);
        } else if dev.us_usage_page == 1 && dev.us_usage == 6 {
            RAW_INPUT_REGISTERED_KEYBOARD.store(!is_remove, Ordering::Relaxed);
            RAW_INPUT_KBD_HWND.store(if !is_remove { dev.hwnd_target } else { 0 }, Ordering::Relaxed);
            println!("[USER32] RegisterRawInputDevices: Keyboard active={}, hwnd=0x{:X}", !is_remove, dev.hwnd_target);
        }
    }
    1
}

#[repr(C)]
pub struct RawInputDeviceList {
    pub h_device: usize,
    pub dw_type: u32,
    pub _pad: u32,
}

#[no_mangle]
pub unsafe extern "win64" fn GetRawInputDeviceList(
    p_raw_input_device_list: *mut RawInputDeviceList,
    pui_num_devices: *mut u32,
    cb_size: u32,
) -> u32 {
    if pui_num_devices.is_null() {
        return 0xFFFFFFFF;
    }
    if p_raw_input_device_list.is_null() {
        *pui_num_devices = 2; // 1 mouse, 1 keyboard
        return 0;
    }
    if cb_size < std::mem::size_of::<RawInputDeviceList>() as u32 {
        return 0xFFFFFFFF;
    }
    let count = (*pui_num_devices).min(2);
    if count >= 1 {
        *p_raw_input_device_list = RawInputDeviceList {
            h_device: 0x00010001,
            dw_type: 0, // RIM_TYPEMOUSE
            _pad: 0,
        };
    }
    if count >= 2 {
        *p_raw_input_device_list.add(1) = RawInputDeviceList {
            h_device: 0x00010002,
            dw_type: 1, // RIM_TYPEKEYBOARD
            _pad: 0,
        };
    }
    *pui_num_devices = count;
    count
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RidDeviceInfoMouse {
    pub dw_id: u32,
    pub dw_number_of_buttons: u32,
    pub dw_sample_rate: u32,
    pub f_has_horizontal_wheel: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RidDeviceInfoKeyboard {
    pub dw_type: u32,
    pub dw_sub_type: u32,
    pub dw_keyboard_mode: u32,
    pub dw_number_of_function_keys: u32,
    pub dw_number_of_indicators: u32,
    pub dw_number_of_keys_total: u32,
}

#[repr(C)]
pub struct RidDeviceInfo {
    pub cb_size: u32,
    pub dw_type: u32,
    pub u: [u8; 24],
}

#[no_mangle]
pub unsafe extern "win64" fn GetRawInputDeviceInfoW(
    h_device: usize,
    ui_command: u32,
    p_data: *mut std::ffi::c_void,
    pcb_size: *mut u32,
) -> u32 {
    const RIDI_PREPARSEDDATA: u32 = 0x20000005;
    const RIDI_DEVICENAME: u32 = 0x20000007;
    const RIDI_DEVICEINFO: u32 = 0x2000000B;

    if pcb_size.is_null() {
        crate::dlls::kernel32::SetLastError(87);
        return 0xFFFFFFFF;
    }

    if ui_command == RIDI_PREPARSEDDATA {
        // Standard mice and keyboards do not provide preparsed HID data
        crate::dlls::kernel32::SetLastError(87);
        return 0xFFFFFFFF;
    }

    if ui_command == RIDI_DEVICEINFO {
        let size = std::mem::size_of::<RidDeviceInfo>() as u32; // 32 bytes
        if p_data.is_null() {
            *pcb_size = size;
            return 0;
        }
        if *pcb_size < size {
            return 0xFFFFFFFF;
        }
        let is_kbd = h_device == 0x00010002;
        let mut info = RidDeviceInfo {
            cb_size: size,
            dw_type: if is_kbd { 1 } else { 0 },
            u: [0u8; 24],
        };
        if is_kbd {
            let kbd = RidDeviceInfoKeyboard {
                dw_type: 81,
                dw_sub_type: 0,
                dw_keyboard_mode: 1,
                dw_number_of_function_keys: 12,
                dw_number_of_indicators: 3,
                dw_number_of_keys_total: 101,
            };
            std::ptr::copy_nonoverlapping(
                &kbd as *const _ as *const u8,
                info.u.as_mut_ptr(),
                std::mem::size_of::<RidDeviceInfoKeyboard>(),
            );
        } else {
            let mouse = RidDeviceInfoMouse {
                dw_id: 1,
                dw_number_of_buttons: 5,
                dw_sample_rate: 1000,
                f_has_horizontal_wheel: 0,
            };
            std::ptr::copy_nonoverlapping(
                &mouse as *const _ as *const u8,
                info.u.as_mut_ptr(),
                std::mem::size_of::<RidDeviceInfoMouse>(),
            );
        }
        std::ptr::copy_nonoverlapping(
            &info as *const _ as *const u8,
            p_data as *mut u8,
            size as usize,
        );
        return size;
    }

    if ui_command == RIDI_DEVICENAME {
        let name: Vec<u16> = if h_device == 0x00010002 {
            "\\\\?\\HID#VID_046D&PID_C31C#1&0000&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}\0"
                .encode_utf16()
                .collect()
        } else {
            "\\\\?\\HID#VID_046D&PID_C077#1&0000&0&0000#{378de44c-5643-11d1-acf9-00a0c940843e}\0"
                .encode_utf16()
                .collect()
        };
        let req_chars = name.len() as u32;
        if p_data.is_null() {
            *pcb_size = req_chars;
            return 0;
        }
        let copy_len = (*pcb_size as usize).min(name.len());
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            p_data as *mut u16,
            copy_len,
        );
        return copy_len as u32;
    }

    crate::dlls::kernel32::SetLastError(87);
    0xFFFFFFFF
}

#[no_mangle]
pub unsafe extern "win64" fn GetRawInputDeviceInfoA(
    h_device: usize,
    ui_command: u32,
    p_data: *mut std::ffi::c_void,
    pcb_size: *mut u32,
) -> u32 {
    const RIDI_PREPARSEDDATA: u32 = 0x20000005;
    const RIDI_DEVICENAME: u32 = 0x20000007;
    const RIDI_DEVICEINFO: u32 = 0x2000000B;
    if pcb_size.is_null() {
        crate::dlls::kernel32::SetLastError(87);
        return 0xFFFFFFFF;
    }
    if ui_command == RIDI_PREPARSEDDATA {
        crate::dlls::kernel32::SetLastError(87);
        return 0xFFFFFFFF;
    }
    if ui_command == RIDI_DEVICEINFO {
        return GetRawInputDeviceInfoW(h_device, ui_command, p_data, pcb_size);
    }
    if ui_command == RIDI_DEVICENAME {
        let name = if h_device == 0x00010002 {
            b"\\\\?\\HID#VID_046D&PID_C31C#1&0000&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}\0"
        } else {
            b"\\\\?\\HID#VID_046D&PID_C077#1&0000&0&0000#{378de44c-5643-11d1-acf9-00a0c940843e}\0"
        };
        let req_len = name.len() as u32;
        if p_data.is_null() {
            *pcb_size = req_len;
            return 0;
        }
        let copy_len = (*pcb_size as usize).min(name.len());
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            p_data as *mut u8,
            copy_len,
        );
        return copy_len as u32;
    }
    crate::dlls::kernel32::SetLastError(87);
    0xFFFFFFFF
}

#[no_mangle]
pub unsafe extern "win64" fn GetRawInputData(
    h_raw_input: usize,
    ui_command: u32,
    p_data: *mut std::ffi::c_void,
    pcb_size: *mut u32,
    _cb_size_header: u32,
) -> u32 {
    const RID_INPUT: u32 = 0x10000003;
    const RID_HEADER: u32 = 0x10000005;

    if pcb_size.is_null() {
        return 0xFFFFFFFF;
    }

    let cur = {
        let guard = RAW_INPUT_MAP.lock().unwrap();
        guard.as_ref().and_then(|m| m.get(&h_raw_input).copied())
    }.unwrap_or_else(|| *CURRENT_RAW_INPUT.lock().unwrap());

    // When an event is retrieved via GetRawInputData, clear the queue so GetRawInputBuffer
    // doesn't duplicate the exact same event
    if let Ok(mut q_guard) = RAW_INPUT_BUFFER_QUEUE.lock() {
        if let Some(ref mut q) = *q_guard {
            q.clear();
        }
    }

    if ui_command == RID_HEADER {
        let header_size = std::mem::size_of::<RawInputHeader>() as u32; // 24
        if p_data.is_null() {
            *pcb_size = header_size;
            return 0;
        }
        if *pcb_size < header_size {
            return 0xFFFFFFFF;
        }
        *pcb_size = header_size;
        std::ptr::copy_nonoverlapping(
            &cur.header as *const RawInputHeader as *const u8,
            p_data as *mut u8,
            header_size as usize,
        );
        return header_size;
    }

    if ui_command == RID_INPUT {
        let total_size = std::mem::size_of::<RawInput>() as u32; // 48
        if p_data.is_null() {
            *pcb_size = total_size;
            return 0;
        }
        if *pcb_size < total_size {
            return 0xFFFFFFFF;
        }
        *pcb_size = total_size;
        std::ptr::copy_nonoverlapping(
            &cur as *const RawInput as *const u8,
            p_data as *mut u8,
            total_size as usize,
        );
        return total_size;
    }

    0
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterDeviceNotificationW(
    h_recipient: usize,
    notification_filter: *const std::ffi::c_void,
    flags: u32,
) -> usize {
    println!(
        "[USER32] RegisterDeviceNotificationW(hRecipient=0x{:X}, filter={:p}, flags=0x{:X}) -> 0xCAFE0001",
        h_recipient, notification_filter, flags
    );
    0xCAFE0001
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterDeviceNotificationA(
    h_recipient: usize,
    notification_filter: *const std::ffi::c_void,
    flags: u32,
) -> usize {
    println!(
        "[USER32] RegisterDeviceNotificationA(hRecipient=0x{:X}, filter={:p}, flags=0x{:X}) -> 0xCAFE0001",
        h_recipient, notification_filter, flags
    );
    0xCAFE0001
}

#[no_mangle]
pub unsafe extern "win64" fn UnregisterDeviceNotification(h_dev_notify: usize) -> i32 {
    println!(
        "[USER32] UnregisterDeviceNotification(hDevNotify=0x{:X}) -> TRUE",
        h_dev_notify
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
pub unsafe extern "win64" fn GetPointerDevices(
    device_count: *mut u32,
    _pointer_devices: *mut std::ffi::c_void,
) -> i32 {
    if !device_count.is_null() {
        *device_count = 0;
    }
    println!("[USER32] GetPointerDevices -> count=0, TRUE");
    1 // TRUE (0 pointer devices attached)
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerDevice(
    _device: u64,
    _pointer_device: *mut std::ffi::c_void,
) -> i32 {
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerDeviceProperties(
    _device: u64,
    property_count: *mut u32,
    _pointer_properties: *mut std::ffi::c_void,
) -> i32 {
    if !property_count.is_null() {
        *property_count = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerDeviceRects(
    _device: u64,
    _pointer_device_rect: *mut std::ffi::c_void,
    _display_rect: *mut std::ffi::c_void,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterPointerDeviceNotifications(
    _window: u64,
    _notify_range: i32,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerInfo(
    _pointer_id: u32,
    _pointer_info: *mut std::ffi::c_void,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerInfoHistory(
    _pointer_id: u32,
    entries_count: *mut u32,
    _pointer_info: *mut std::ffi::c_void,
) -> i32 {
    if !entries_count.is_null() {
        *entries_count = 0;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerTouchInfo(
    _pointer_id: u32,
    _touch_info: *mut std::ffi::c_void,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerFrameTouchInfo(
    _pointer_id: u32,
    entries_count: *mut u32,
    _touch_info: *mut std::ffi::c_void,
) -> i32 {
    if !entries_count.is_null() {
        *entries_count = 0;
    }
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

#[no_mangle]
pub unsafe extern "win64" fn GetProcessDpiAwareness(
    _hprocess: u64,
    value: *mut i32,
) -> i32 {
    if !value.is_null() {
        *value = 2; // PROCESS_PER_MONITOR_DPI_AWARE
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
        let (wx, wy) = refresh_cached_window_position();
        (*lp_point).x += wx;
        (*lp_point).y += wy;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn ScreenToClient(_hwnd: usize, lp_point: *mut Point) -> i32 {
    if !lp_point.is_null() && (lp_point as usize) > 0x10000 {
        let (wx, wy) = refresh_cached_window_position();
        (*lp_point).x -= wx;
        (*lp_point).y -= wy;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn MapWindowPoints(
    hwnd_from: usize,
    hwnd_to: usize,
    lp_points: *mut Point,
    c_points: u32,
) -> i32 {
    if !lp_points.is_null() && c_points > 0 && (lp_points as usize) > 0x10000 {
        let (wx, wy) = refresh_cached_window_position();
        if hwnd_from == 0 && hwnd_to != 0 {
            // Screen to client
            for i in 0..c_points {
                let pt = &mut *lp_points.add(i as usize);
                pt.x -= wx;
                pt.y -= wy;
            }
        } else if hwnd_from != 0 && hwnd_to == 0 {
            // Client to screen
            for i in 0..c_points {
                let pt = &mut *lp_points.add(i as usize);
                pt.x += wx;
                pt.y += wy;
            }
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetCursorPos(lp_point: *mut Point) -> i32 {
    if !lp_point.is_null() && (lp_point as usize) > 0x10000 {
        let (wx, wy) = refresh_cached_window_position();
        let x = LAST_CURSOR_X.load(Ordering::Relaxed);
        let y = LAST_CURSOR_Y.load(Ordering::Relaxed);
        *lp_point = Point {
            x: wx + x,
            y: wy + y,
        };
        return 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetMessagePos() -> u32 {
    let mut pt = Point { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    ((pt.y as u32 & 0xFFFF) << 16) | (pt.x as u32 & 0xFFFF)
}

#[no_mangle]
pub unsafe extern "win64" fn GetMessageTime() -> i32 {
    crate::dlls::kernel32::GetTickCount() as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetAsyncKeyState(v_key: i32) -> i16 {
    if v_key <= 0 || v_key >= 256 {
        return 0;
    }
    if v_key == 0x01 || v_key == 0x02 || v_key == 0x04 {
        let is_pressed = match v_key {
            0x01 => KEY_STATE_TABLE[0x01].load(Ordering::Relaxed),
            0x02 => KEY_STATE_TABLE[0x02].load(Ordering::Relaxed),
            0x04 => KEY_STATE_TABLE[0x04].load(Ordering::Relaxed),
            _ => false,
        };
        return if is_pressed { -32767 } else { 0 };
    }
    let is_down = KEY_STATE_TABLE[v_key as usize].load(Ordering::Relaxed);
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
pub unsafe extern "win64" fn keybd_event(
    _b_vk: u8,
    _b_scan: u8,
    _dw_flags: u32,
    _dw_extra_info: usize,
) {
}

#[no_mangle]
pub unsafe extern "win64" fn GetKeyboardState(lp_key_state: *mut u8) -> i32 {
    if !lp_key_state.is_null() && (lp_key_state as usize) > 0x10000 {
        for i in 0..256 {
            let is_down = if i == 0x01 || i == 0x02 || i == 0x04 {
                GetAsyncKeyState(i as i32) != 0
            } else {
                KEY_STATE_TABLE[i].load(Ordering::Relaxed)
            };
            *lp_key_state.add(i) = if is_down { 0x80 } else { 0x00 };
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
        0 => {
            // MAPVK_VK_TO_VSC: VK to scan code (unextended)
            let (sc, _) = vk_to_scancode(u_code);
            sc as u32
        }
        1 => {
            // MAPVK_VSC_TO_VK: Scan code to VK (unextended)
            scancode_to_vk(u_code as u16, false)
        }
        2 => {
            // MAPVK_VK_TO_CHAR: VK to unshifted character
            if (0x30..=0x39).contains(&u_code)
                || (0x41..=0x5A).contains(&u_code)
                || u_code == 0x20
                || u_code == 0x0D
                || u_code == 0x08
            {
                u_code
            } else {
                0
            }
        }
        3 => {
            // MAPVK_VSC_TO_VK_EX: Scan code to VK (distinguishes left/right)
            let is_ext = (u_code & 0xE000) != 0 || u_code > 0x7F;
            scancode_to_vk((u_code & 0xFF) as u16, is_ext)
        }
        4 => {
            // MAPVK_VK_TO_VSC_EX: VK to scan code (distinguishes extended)
            let (sc, is_ext) = vk_to_scancode(u_code);
            if is_ext {
                0xE000 | (sc as u32)
            } else {
                sc as u32
            }
        }
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetKeyboardLayout(id_thread: u32) -> usize {
    println!(
        "[USER32] GetKeyboardLayout(idThread={}) -> 0x04090409",
        id_thread
    );
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
    ToAsciiEx(
        w_virt_key,
        w_scan_code,
        lp_key_state,
        lp_char,
        w_flags,
        0x04090409,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn VkKeyScanA(ch: i8) -> i16 {
    let u_ch = (ch as u8) as u16;
    VkKeyScanW(u_ch)
}

#[no_mangle]
pub unsafe extern "win64" fn VkKeyScanW(ch: u16) -> i16 {
    match ch {
        0x61..=0x7A => (ch - 0x20) as i16,     // a-z -> VK_A..VK_Z
        0x41..=0x5A => (ch as i16) | (1 << 8), // A-Z -> VK_A + shift
        0x30..=0x39 => ch as i16,
        0x20 => 0x20,
        _ => -1,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetCursor(hcursor: u64) -> u64 {
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if !sdl_handle.is_null() {
        type SdlShowCursorFn = unsafe extern "C" fn(i32) -> i32;
        if let Ok(sym) = std::ffi::CString::new("SDL_ShowCursor") {
            let p = libc::dlsym(sdl_handle, sym.as_ptr());
            if !p.is_null() {
                let f: SdlShowCursorFn = std::mem::transmute(p);
                f(if hcursor != 0 { 1 } else { 0 });
            }
        }
    }
    hcursor
}

#[no_mangle]
pub unsafe extern "win64" fn SetCursorPos(x: i32, y: i32) -> i32 {
    let mut wx = CACHED_WIN_X.load(Ordering::Relaxed);
    let mut wy = CACHED_WIN_Y.load(Ordering::Relaxed);
    if wx == 0 && wy == 0 {
        let (nx, ny) = refresh_cached_window_position();
        wx = nx;
        wy = ny;
    }
    let client_x = x - wx;
    let client_y = y - wy;
    LAST_CURSOR_X.store(client_x, Ordering::Relaxed);
    LAST_CURSOR_Y.store(client_y, Ordering::Relaxed);

    WARP_TARGET_X.store(client_x, Ordering::Relaxed);
    WARP_TARGET_Y.store(client_y, Ordering::Relaxed);
    WARP_PENDING.store(true, Ordering::Relaxed);

    let sdl_win = crate::windowing::x11::get_global_sdl_window();
    if !sdl_win.is_null() {
        if let Some(f) = get_sdl_warp_mouse_fn() {
            f(sdl_win, client_x, client_y);
        }
    }
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
pub unsafe extern "win64" fn ClipCursor(lp_rect: *const Rect) -> i32 {
    let is_clip = !lp_rect.is_null();
    IS_CURSOR_CLIPPED.store(is_clip, Ordering::Relaxed);
    if is_clip {
        let r = *lp_rect;
        println!(
            "[USER32] ClipCursor(is_clip=true, rect=[{}, {}, {}, {}]) -> TRUE",
            r.left, r.top, r.right, r.bottom
        );
        if let Some(f) = get_sdl_set_relative_mouse_mode_fn() {
            f(1);
        }
    } else {
        println!("[USER32] ClipCursor(is_clip=false) -> TRUE");
        if let Some(f) = get_sdl_set_relative_mouse_mode_fn() {
            f(0);
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetClipCursor(lp_rect: *mut Rect) -> i32 {
    if !lp_rect.is_null() {
        let (x, y) = refresh_cached_window_position();
        let (w, h) = crate::windowing::x11::sdl_get_window_size();
        *lp_rect = Rect {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        };
    }
    1 // TRUE
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[no_mangle]
pub unsafe extern "win64" fn GetClientRect(hwnd: usize, lp_rect: *mut Rect) -> i32 {
    let (w, h) = if is_primary_window(hwnd as u64) {
        let (real_w, real_h) = crate::windowing::x11::sdl_get_window_size();
        let fallback_w = if real_w > 0 { real_w } else { 1920 };
        let fallback_h = if real_h > 0 { real_h } else { 1080 };
        if let Some(win) =
            crate::graphics::vulkan_bridge::handles::WindowRegistry::global().lookup(hwnd)
        {
            let reg_w = win.width as i32;
            let reg_h = win.height as i32;
            (
                if reg_w > 320 { reg_w } else { fallback_w },
                if reg_h > 240 { reg_h } else { fallback_h },
            )
        } else {
            (fallback_w, fallback_h)
        }
    } else if let Some(win) =
        crate::graphics::vulkan_bridge::handles::WindowRegistry::global().lookup(hwnd)
    {
        (win.width as i32, win.height as i32)
    } else {
        (1, 1)
    };
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
        hwnd, w, h
    );
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowRect(hwnd: usize, lp_rect: *mut Rect) -> i32 {
    let is_pri = is_primary_window(hwnd as u64);
    let (x, y) = if is_pri {
        refresh_cached_window_position()
    } else {
        (0, 0)
    };
    let (w, h) = if is_pri {
        let (real_w, real_h) = crate::windowing::x11::sdl_get_window_size();
        let fallback_w = if real_w > 0 { real_w } else { 1920 };
        let fallback_h = if real_h > 0 { real_h } else { 1080 };
        if let Some(win) =
            crate::graphics::vulkan_bridge::handles::WindowRegistry::global().lookup(hwnd)
        {
            let reg_w = win.width as i32;
            let reg_h = win.height as i32;
            (
                if reg_w > 320 { reg_w } else { fallback_w },
                if reg_h > 240 { reg_h } else { fallback_h },
            )
        } else {
            (fallback_w, fallback_h)
        }
    } else if let Some(win) =
        crate::graphics::vulkan_bridge::handles::WindowRegistry::global().lookup(hwnd)
    {
        (win.width as i32, win.height as i32)
    } else {
        (1, 1)
    };
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
        hwnd,
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
pub unsafe extern "win64" fn AdjustWindowRectExForDpi(
    lp_rect: *mut Rect,
    dw_style: u32,
    b_menu: i32,
    dw_ex_style: u32,
    _dpi: u32,
) -> i32 {
    AdjustWindowRectEx(lp_rect, dw_style, b_menu, dw_ex_style)
}

#[no_mangle]
pub unsafe extern "win64" fn SystemParametersInfoW(
    ui_action: u32,
    _ui_param: u32,
    pv_param: *mut std::ffi::c_void,
    _f_win_ini: u32,
) -> i32 {
    if !pv_param.is_null() {
        match ui_action {
            0x0030 /* SPI_GETWORKAREA */ => {
                let r = pv_param as *mut Rect;
                let bounds = crate::windowing::x11::get_host_display_bounds(0);
                (*r).left = 0;
                (*r).top = 0;
                (*r).right = bounds.w;
                (*r).bottom = bounds.h;
            }
            _ => {}
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SystemParametersInfoA(
    ui_action: u32,
    ui_param: u32,
    pv_param: *mut std::ffi::c_void,
    f_win_ini: u32,
) -> i32 {
    SystemParametersInfoW(ui_action, ui_param, pv_param, f_win_ini)
}

#[no_mangle]
pub unsafe extern "win64" fn SystemParametersInfoForDpi(
    ui_action: u32,
    ui_param: u32,
    pv_param: *mut std::ffi::c_void,
    f_win_ini: u32,
    _dpi: u32,
) -> i32 {
    SystemParametersInfoW(ui_action, ui_param, pv_param, f_win_ini)
}

#[no_mangle]
pub unsafe extern "win64" fn EnableNonClientDpiScaling(_hwnd: usize) -> i32 {
    1
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
    let is_pri = is_primary_window(hwnd as u64);
    if is_pri {
        // SWP_NOSIZE = 0x0001
        if (flags & 0x0001) == 0 && cx > 0 && cy > 0 {
            if cx > 320 && cy > 240 {
                crate::graphics::vulkan_bridge::handles::WindowRegistry::global()
                    .update_window_size(hwnd, cx as u32, cy as u32);
                crate::windowing::x11::sdl_set_window_size(cx, cy);
            }
        }
        // SWP_NOMOVE = 0x0002
        if (flags & 0x0002) == 0 {
            if x >= -5000 && x <= 5000 && y >= -5000 && y <= 5000 {
                crate::windowing::x11::sdl_set_window_position(x, y);
            }
        }
    } else if (flags & 0x0001) == 0 && cx > 0 && cy > 0 {
        crate::graphics::vulkan_bridge::handles::WindowRegistry::global()
            .update_window_size(hwnd, cx as u32, cy as u32);
    }
    let (real_w, real_h) = if is_pri {
        crate::windowing::x11::sdl_get_window_size()
    } else {
        (cx, cy)
    };
    let final_w = if is_pri {
        if (flags & 0x0001) == 0 && cx > 320 { cx } else if real_w > 0 { real_w } else { 1920 }
    } else {
        if (flags & 0x0001) == 0 && cx > 0 { cx } else if real_w > 0 { real_w } else { cx }
    };
    let final_h = if is_pri {
        if (flags & 0x0001) == 0 && cy > 240 { cy } else if real_h > 0 { real_h } else { 1080 }
    } else {
        if (flags & 0x0001) == 0 && cy > 0 { cy } else if real_h > 0 { real_h } else { cy }
    };
    if (flags & 0x0002) == 0 {
        SendMessageW(hwnd as u64, 0x0003 /* WM_MOVE */, 0, ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF));
    }
    if (flags & 0x0001) == 0 {
        SendMessageW(hwnd as u64, 0x0005 /* WM_SIZE */, 0 /* SIZE_RESTORED */, ((final_h as u64 & 0xFFFF) << 16) | (final_w as u64 & 0xFFFF));
    }
    let mut winpos = WindowPos {
        hwnd: hwnd as u64,
        hwnd_insert_after: _hwnd_insert_after as u64,
        x,
        y,
        cx: final_w,
        cy: final_h,
        flags,
        _pad: 0,
    };
    SendMessageW(hwnd as u64, 0x0047 /* WM_WINDOWPOSCHANGED */, 0, &mut winpos as *mut WindowPos as u64);
    // SWP_SHOWWINDOW = 0x0040
    if (flags & 0x0040) != 0 {
        if is_pri {
            WINDOW_VISIBLE.store(true, Ordering::SeqCst);
            crate::windowing::x11::sdl_show_window();
            crate::windowing::x11::sdl_raise_window();
        }
        SendMessageW(hwnd as u64, 0x0018 /* WM_SHOWWINDOW */, 1, 0);
        // SWP_NOACTIVATE = 0x0010
        if (flags & 0x0010) == 0 {
            SendMessageW(hwnd as u64, 0x0006 /* WM_ACTIVATE */, 1 /* WA_ACTIVE */, 0);
            SendMessageW(hwnd as u64, 0x0007 /* WM_SETFOCUS */, 0, 0);
        }
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
    let is_pri = is_primary_window(hwnd as u64);
    if is_pri {
        if x >= -5000 && x <= 5000 && y >= -5000 && y <= 5000 {
            crate::windowing::x11::sdl_set_window_position(x, y);
        }
        if n_width > 320 && n_height > 240 {
            crate::graphics::vulkan_bridge::handles::WindowRegistry::global()
                .update_window_size(hwnd, n_width as u32, n_height as u32);
            crate::windowing::x11::sdl_set_window_size(n_width, n_height);
        }
    } else if n_width > 0 && n_height > 0 {
        crate::graphics::vulkan_bridge::handles::WindowRegistry::global()
            .update_window_size(hwnd, n_width as u32, n_height as u32);
    }
    let (real_w, real_h) = if is_pri {
        crate::windowing::x11::sdl_get_window_size()
    } else {
        (n_width, n_height)
    };
    let final_w = if is_pri {
        if n_width > 320 { n_width } else if real_w > 0 { real_w } else { 1920 }
    } else {
        if n_width > 0 { n_width } else if real_w > 0 { real_w } else { n_width }
    };
    let final_h = if is_pri {
        if n_height > 240 { n_height } else if real_h > 0 { real_h } else { 1080 }
    } else {
        if n_height > 0 { n_height } else if real_h > 0 { real_h } else { n_height }
    };
    SendMessageW(hwnd as u64, 0x0003 /* WM_MOVE */, 0, ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF));
    SendMessageW(hwnd as u64, 0x0005 /* WM_SIZE */, 0 /* SIZE_RESTORED */, ((final_h as u64 & 0xFFFF) << 16) | (final_w as u64 & 0xFFFF));
    let mut winpos = WindowPos {
        hwnd: hwnd as u64,
        hwnd_insert_after: 0,
        x,
        y,
        cx: final_w,
        cy: final_h,
        flags: 0x0014, // SWP_NOZORDER | SWP_NOACTIVATE
        _pad: 0,
    };
    SendMessageW(hwnd as u64, 0x0047 /* WM_WINDOWPOSCHANGED */, 0, &mut winpos as *mut WindowPos as u64);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn ShowWindow(hwnd: usize, n_cmd_show: i32) -> i32 {
    println!(
        "[USER32] ShowWindow(hWnd=0x{:X}, nCmdShow={}) -> TRUE",
        hwnd, n_cmd_show
    );
    let is_pri = is_primary_window(hwnd as u64);
    if is_pri {
        if n_cmd_show == 0 /* SW_HIDE */ {
            WINDOW_VISIBLE.store(false, Ordering::SeqCst);
        } else {
            WINDOW_VISIBLE.store(true, Ordering::SeqCst);
        }
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
    }
    SendMessageW(hwnd as u64, 0x0018 /* WM_SHOWWINDOW */, if n_cmd_show == 0 { 0 } else { 1 }, 0);
    if n_cmd_show != 0 {
        SendMessageW(hwnd as u64, 0x0006 /* WM_ACTIVATE */, 1, 0);
        SendMessageW(hwnd as u64, 0x0007 /* WM_SETFOCUS */, 0, 0);
        SendMessageW(hwnd as u64, 0x000F /* WM_PAINT */, 0, 0);
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn UpdateWindow(hwnd: usize) -> i32 {
    SendMessageW(hwnd as u64, 0x000F /* WM_PAINT */, 0, 0);
    crate::windowing::x11::pump_x11_sdl_events();
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn ValidateRect(_hwnd: usize, _lp_rect: *const Rect) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn InvalidateRect(
    _hwnd: usize,
    _lp_rect: *const Rect,
    _b_erase: i32,
) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetForegroundWindow(hwnd: usize) -> i32 {
    println!("[USER32] SetForegroundWindow(hWnd=0x{:X}) -> TRUE", hwnd);
    FOREGROUND_HWND.store(hwnd as u64, Ordering::Relaxed);
    if is_primary_window(hwnd as u64) {
        crate::windowing::x11::sdl_raise_window();
    }
    SendMessageW(hwnd as u64, 0x0006 /* WM_ACTIVATE */, 1 /* WA_ACTIVE */, 0);
    SendMessageW(hwnd as u64, 0x0007 /* WM_SETFOCUS */, 0, 0);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetForegroundWindow() -> usize {
    let fg = FOREGROUND_HWND.load(Ordering::Relaxed);
    if fg != 0 {
        return fg as usize;
    }
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
    ACTIVE_HWND.store(hwnd as u64, Ordering::Relaxed);
    SendMessageW(hwnd as u64, 0x0006 /* WM_ACTIVATE */, 1 /* WA_ACTIVE */, 0);
    hwnd
}

#[no_mangle]
pub unsafe extern "win64" fn GetActiveWindow() -> usize {
    let act = ACTIVE_HWND.load(Ordering::Relaxed);
    if act != 0 {
        return act as usize;
    }
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
    FOCUS_HWND.store(hwnd as u64, Ordering::Relaxed);
    SendMessageW(hwnd as u64, 0x0007 /* WM_SETFOCUS */, 0, 0);
    hwnd
}

#[no_mangle]
pub unsafe extern "win64" fn GetFocus() -> usize {
    let foc = FOCUS_HWND.load(Ordering::Relaxed);
    if foc != 0 {
        return foc as usize;
    }
    let act = ACTIVE_HWND.load(Ordering::Relaxed);
    if act != 0 {
        return act as usize;
    }
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
pub unsafe extern "win64" fn IsWindowVisible(hwnd: usize) -> i32 {
    if !is_primary_window(hwnd as u64) {
        return 0;
    }
    if WINDOW_VISIBLE.load(Ordering::SeqCst) {
        1 // TRUE
    } else {
        0 // FALSE
    }
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
static WINDOW_EXTRA_0: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[no_mangle]
pub unsafe extern "win64" fn SetWindowLongPtrA(hwnd: u64, n_index: i32, new_long: isize) -> isize {
    println!(
        "[USER32] SetWindowLongPtrA(hWnd=0x{:X}, index={}, new_long=0x{:X})",
        hwnd, n_index, new_long
    );
    match n_index {
        0 => {
            // cbWndExtra index 0 (e.g. Unity Player internal state pointer)
            let prev = WINDOW_EXTRA_0.swap(new_long as u64, Ordering::Relaxed);
            prev as isize
        }
        -4 => {
            // GWLP_WNDPROC
            let prev = get_wndproc_for_hwnd(hwnd);
            set_wndproc_for_hwnd(hwnd, new_long as u64);
            if is_primary_window(hwnd) {
                GODOT_WNDPROC.store(new_long as u64, Ordering::Relaxed);
            }
            prev as isize
        }
        -16 => {
            // GWL_STYLE
            let mut guard = HWND_STYLES.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            let entry = map.entry(hwnd).or_insert((WINDOW_STYLE.load(Ordering::Relaxed), WINDOW_EX_STYLE.load(Ordering::Relaxed)));
            let prev = entry.0;
            entry.0 = new_long as u32;
            if is_primary_window(hwnd) {
                WINDOW_STYLE.store(new_long as u32, Ordering::Relaxed);
            }
            prev as isize
        }
        -20 => {
            // GWL_EXSTYLE
            let mut guard = HWND_STYLES.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            let entry = map.entry(hwnd).or_insert((WINDOW_STYLE.load(Ordering::Relaxed), WINDOW_EX_STYLE.load(Ordering::Relaxed)));
            let prev = entry.1;
            entry.1 = new_long as u32;
            if is_primary_window(hwnd) {
                WINDOW_EX_STYLE.store(new_long as u32, Ordering::Relaxed);
            }
            prev as isize
        }
        -21 => {
            // GWLP_USERDATA
            let mut guard = HWND_USERDATA.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            let prev = map.insert(hwnd, new_long as u64).unwrap_or(0);
            if is_primary_window(hwnd) {
                WINDOW_USER_DATA.store(new_long as u64, Ordering::Relaxed);
            }
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
        0 => WINDOW_EXTRA_0.load(Ordering::Relaxed) as isize,
        -4 => get_wndproc_for_hwnd(hwnd) as isize,
        -16 => {
            let guard = HWND_STYLES.lock().unwrap();
            if let Some(map) = guard.as_ref() {
                if let Some(&(s, _)) = map.get(&hwnd) {
                    return s as isize;
                }
            }
            WINDOW_STYLE.load(Ordering::Relaxed) as isize
        }
        -20 => {
            let guard = HWND_STYLES.lock().unwrap();
            if let Some(map) = guard.as_ref() {
                if let Some(&(_, es)) = map.get(&hwnd) {
                    return es as isize;
                }
            }
            WINDOW_EX_STYLE.load(Ordering::Relaxed) as isize
        }
        -21 => {
            let guard = HWND_USERDATA.lock().unwrap();
            if let Some(map) = guard.as_ref() {
                if let Some(&ud) = map.get(&hwnd) {
                    return ud as isize;
                }
            }
            WINDOW_USER_DATA.load(Ordering::Relaxed) as isize
        }
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
    CURRENT_CAPTURE_HWND.swap(hwnd, Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "win64" fn ReleaseCapture() -> i32 {
    CURRENT_CAPTURE_HWND.store(0, Ordering::SeqCst);
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
            let hdc_monitor = 0x00010001usize + (i as usize);
            let res = cb(h_monitor, hdc_monitor, &rect, dw_data);
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

    // If caller provided MONITORINFOEXW (size >= 104), copy szDevice
    if size >= 104 {
        let dev_name = format!("\\\\.\\DISPLAY{}\0", display_idx + 1);
        let u16_chars: Vec<u16> = dev_name.encode_utf16().collect();
        let ptr_sz_device = (lpmi as *mut u8).add(40) as *mut u16;
        std::ptr::write_bytes(ptr_sz_device, 0, 32);
        for (i, &ch) in u16_chars.iter().take(32).enumerate() {
            *ptr_sz_device.add(i) = ch;
        }
    }

    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetMonitorInfoA(h_monitor: usize, lpmi: *mut MonitorInfo) -> i32 {
    if lpmi.is_null() {
        crate::dlls::kernel32::SetLastError(87);
        return 0;
    }
    let res = GetMonitorInfoW(h_monitor, lpmi);
    let size = (*lpmi).cb_size;
    let display_idx = if h_monitor >= 0x00010001 {
        (h_monitor - 0x00010001) as i32
    } else {
        0
    };
    if size >= 72 {
        let dev_name = format!("\\\\.\\DISPLAY{}\0", display_idx + 1);
        let ptr_sz_device = (lpmi as *mut u8).add(40) as *mut u8;
        std::ptr::write_bytes(ptr_sz_device, 0, 32);
        for (i, &b) in dev_name.as_bytes().iter().take(32).enumerate() {
            *ptr_sz_device.add(i) = b;
        }
    }
    res
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
    lpsz_device_name: *const u16,
    i_mode_num: u32,
    lp_dev_mode: *mut DevModeW,
) -> i32 {
    if lp_dev_mode.is_null() {
        return 0;
    }

    let bounds = crate::windowing::x11::get_host_display_bounds(0);
    let (w, h) = match i_mode_num {
        0xFFFFFFFF | 0xFFFFFFFE | 0 => (bounds.w as u32, bounds.h as u32),
        1 => (1920, 1080),
        2 => (1600, 900),
        3 => (1366, 768),
        4 => (1280, 720),
        5 => (1024, 768),
        6 => (800, 600),
        _ => return 0,
    };

    let dev = &mut *lp_dev_mode;
    dev.dm_size = std::mem::size_of::<DevModeW>() as u16;
    dev.dm_fields = 0x00040000 | 0x00080000 | 0x00100000 | 0x00400000;
    dev.dm_bits_per_pel = 32;
    dev.dm_pels_width = w;
    dev.dm_pels_height = h;
    dev.dm_display_frequency = get_host_refresh_rate();
    dev.dm_display_flags = 0;

    let dev_name = if !lpsz_device_name.is_null() {
        let mut len = 0;
        while *lpsz_device_name.add(len) != 0 && len < 31 {
            len += 1;
        }
        std::slice::from_raw_parts(lpsz_device_name, len).to_vec()
    } else {
        "\\\\.\\DISPLAY1\0".encode_utf16().collect()
    };
    for (i, &ch) in dev_name.iter().take(32).enumerate() {
        dev.dm_device_name[i] = ch;
    }

    1
}

#[no_mangle]
pub unsafe extern "win64" fn EnumDisplaySettingsA(
    lpsz_device_name: *const u8,
    i_mode_num: u32,
    lp_dev_mode: *mut u8,
) -> i32 {
    if lp_dev_mode.is_null() {
        return 0;
    }

    let bounds = crate::windowing::x11::get_host_display_bounds(0);
    let (w, h) = match i_mode_num {
        0xFFFFFFFF | 0xFFFFFFFE | 0 => (bounds.w as u32, bounds.h as u32),
        1 => (1920, 1080),
        2 => (1600, 900),
        3 => (1366, 768),
        4 => (1280, 720),
        5 => (1024, 768),
        6 => (800, 600),
        _ => return 0,
    };

    *(lp_dev_mode.add(36) as *mut u16) = 156;
    *(lp_dev_mode.add(40) as *mut u32) = 0x00040000 | 0x00080000 | 0x00100000 | 0x00400000;
    *(lp_dev_mode.add(104) as *mut u32) = 32;
    *(lp_dev_mode.add(108) as *mut u32) = w;
    *(lp_dev_mode.add(112) as *mut u32) = h;
    *(lp_dev_mode.add(120) as *mut u32) = 60;

    let dev_name = if !lpsz_device_name.is_null() {
        let mut len = 0;
        while *lpsz_device_name.add(len) != 0 && len < 31 {
            len += 1;
        }
        std::slice::from_raw_parts(lpsz_device_name, len).to_vec()
    } else {
        b"\\\\.\\DISPLAY1\0".to_vec()
    };
    for (i, &b) in dev_name.iter().take(32).enumerate() {
        *lp_dev_mode.add(i) = b;
    }

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
pub unsafe extern "win64" fn ChangeDisplaySettingsExW(
    _lpsz_device_name: *const u16,
    lp_dev_mode: *const DevModeW,
    _hwnd: u64,
    _dw_flags: u32,
    _lparam: *mut std::ffi::c_void,
) -> i32 {
    if !lp_dev_mode.is_null() {
        let w = (*lp_dev_mode).dm_pels_width;
        let h = (*lp_dev_mode).dm_pels_height;
        println!("[USER32] ChangeDisplaySettingsExW: requested {}x{}", w, h);
    }
    0 // DISP_CHANGE_SUCCESSFUL = 0
}

#[no_mangle]
pub unsafe extern "win64" fn ChangeDisplaySettingsW(
    lp_dev_mode: *const DevModeW,
    _dw_flags: u32,
) -> i32 {
    if !lp_dev_mode.is_null() {
        let w = (*lp_dev_mode).dm_pels_width;
        let h = (*lp_dev_mode).dm_pels_height;
        println!("[USER32] ChangeDisplaySettingsW: requested {}x{}", w, h);
    }
    0 // DISP_CHANGE_SUCCESSFUL = 0
}

#[no_mangle]
pub unsafe extern "win64" fn ChangeDisplaySettingsExA(
    _lpsz_device_name: *const u8,
    _lp_dev_mode: *const u8,
    _hwnd: u64,
    _dw_flags: u32,
    _lparam: *mut std::ffi::c_void,
) -> i32 {
    0 // DISP_CHANGE_SUCCESSFUL = 0
}

#[no_mangle]
pub unsafe extern "win64" fn ChangeDisplaySettingsA(
    _lp_dev_mode: *const u8,
    _dw_flags: u32,
) -> i32 {
    0 // DISP_CHANGE_SUCCESSFUL = 0
}

#[repr(C)]
pub struct DisplayDeviceA {
    pub cb: u32,
    pub device_name: [u8; 32],
    pub device_string: [u8; 128],
    pub state_flags: u32,
    pub device_id: [u8; 128],
    pub device_key: [u8; 128],
}

#[repr(C)]
pub struct DisplayDeviceW {
    pub cb: u32,
    pub device_name: [u16; 32],
    pub device_string: [u16; 128],
    pub state_flags: u32,
    pub device_id: [u16; 128],
    pub device_key: [u16; 128],
}

#[no_mangle]
pub unsafe extern "win64" fn EnumDisplayDevicesA(
    _lp_device: *const u8,
    i_dev_num: u32,
    lp_display_device: *mut DisplayDeviceA,
    _dw_flags: u32,
) -> i32 {
    let count = crate::windowing::x11::get_host_num_displays() as u32;
    if lp_display_device.is_null() || i_dev_num >= count {
        return 0;
    }
    let dev = &mut *lp_display_device;
    dev.state_flags = 0x00000001 | 0x00000004; // ATTACHED_TO_DESKTOP | PRIMARY_DEVICE
    let name = format!("\\\\.\\DISPLAY{}\0", i_dev_num + 1);
    let desc = b"Rine Primary Display\0";
    let dev_id = b"PCI\\VEN_10DE&DEV_RINE&SUBSYS_RINE\0";
    std::ptr::write_bytes(dev.device_name.as_mut_ptr(), 0, 32);
    std::ptr::copy_nonoverlapping(name.as_ptr(), dev.device_name.as_mut_ptr(), name.len());
    std::ptr::write_bytes(dev.device_string.as_mut_ptr(), 0, 128);
    std::ptr::copy_nonoverlapping(desc.as_ptr(), dev.device_string.as_mut_ptr(), desc.len());
    std::ptr::write_bytes(dev.device_id.as_mut_ptr(), 0, 128);
    std::ptr::copy_nonoverlapping(dev_id.as_ptr(), dev.device_id.as_mut_ptr(), dev_id.len());
    1
}

#[no_mangle]
pub unsafe extern "win64" fn EnumDisplayDevicesW(
    _lp_device: *const u16,
    i_dev_num: u32,
    lp_display_device: *mut DisplayDeviceW,
    _dw_flags: u32,
) -> i32 {
    let count = crate::windowing::x11::get_host_num_displays() as u32;
    if lp_display_device.is_null() || i_dev_num >= count {
        return 0;
    }
    let dev = &mut *lp_display_device;
    dev.state_flags = 0x00000001 | 0x00000004;
    let name: Vec<u16> = format!("\\\\.\\DISPLAY{}\0", i_dev_num + 1)
        .encode_utf16()
        .collect();
    let desc: Vec<u16> = "Rine Primary Display\0".encode_utf16().collect();
    let dev_id: Vec<u16> = "PCI\\VEN_10DE&DEV_RINE&SUBSYS_RINE\0"
        .encode_utf16()
        .collect();
    std::ptr::write_bytes(dev.device_name.as_mut_ptr(), 0, 32);
    std::ptr::copy_nonoverlapping(name.as_ptr(), dev.device_name.as_mut_ptr(), name.len());
    std::ptr::write_bytes(dev.device_string.as_mut_ptr(), 0, 128);
    std::ptr::copy_nonoverlapping(desc.as_ptr(), dev.device_string.as_mut_ptr(), desc.len());
    std::ptr::write_bytes(dev.device_id.as_mut_ptr(), 0, 128);
    std::ptr::copy_nonoverlapping(dev_id.as_ptr(), dev.device_id.as_mut_ptr(), dev_id.len());
    1
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Luid {
    pub low_part: u32,
    pub high_part: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfigRational {
    pub numerator: u32,
    pub denominator: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfigPathSourceInfo {
    pub adapter_id: Luid,
    pub id: u32,
    pub mode_info_idx: u32,
    pub status_flags: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfigPathTargetInfo {
    pub adapter_id: Luid,
    pub id: u32,
    pub mode_info_idx: u32,
    pub output_technology: u32,
    pub rotation: u32,
    pub scaling: u32,
    pub refresh_rate: DisplayConfigRational,
    pub scanline_ordering: u32,
    pub target_available: i32,
    pub status_flags: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfigPathInfo {
    pub source_info: DisplayConfigPathSourceInfo,
    pub target_info: DisplayConfigPathTargetInfo,
    pub flags: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfig2DRegion {
    pub cx: u32,
    pub cy: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfigSourceMode {
    pub width: u32,
    pub height: u32,
    pub pixel_format: u32,
    pub position: Point,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfigVideoSignalInfo {
    pub pixel_rate: u64,
    pub h_sync_freq: DisplayConfigRational,
    pub v_sync_freq: DisplayConfigRational,
    pub active_size: DisplayConfig2DRegion,
    pub total_size: DisplayConfig2DRegion,
    pub video_standard: u32,
    pub scanline_ordering: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfigTargetMode {
    pub target_video_signal_info: DisplayConfigVideoSignalInfo,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DisplayConfigModeInfo {
    pub info_type: u32,
    pub id: u32,
    pub adapter_id: Luid,
    pub mode_union: [u8; 48],
}

impl Default for DisplayConfigModeInfo {
    fn default() -> Self {
        Self {
            info_type: 0,
            id: 0,
            adapter_id: Luid::default(),
            mode_union: [0u8; 48],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfigDeviceInfoHeader {
    pub info_type: u32,
    pub size: u32,
    pub adapter_id: Luid,
    pub id: u32,
}

#[repr(C)]
pub struct DisplayConfigTargetDeviceName {
    pub header: DisplayConfigDeviceInfoHeader,
    pub flags: u32,
    pub output_technology: u32,
    pub edid_manufacture_id: u16,
    pub edid_product_code_id: u16,
    pub connector_instance: u32,
    pub monitor_friendly_device_name: [u16; 64],
    pub monitor_device_path: [u16; 128],
}

#[no_mangle]
pub unsafe extern "win64" fn GetDisplayConfigBufferSizes(
    flags: u32,
    num_path_array_elements: *mut u32,
    num_mode_info_array_elements: *mut u32,
) -> i32 {
    println!(
        "[USER32 DISPLAYCONFIG] GetDisplayConfigBufferSizes(flags=0x{:X})",
        flags
    );
    if !num_path_array_elements.is_null() {
        *num_path_array_elements = 1;
    }
    if !num_mode_info_array_elements.is_null() {
        *num_mode_info_array_elements = 2;
    }
    0 // ERROR_SUCCESS = 0
}

#[no_mangle]
pub unsafe extern "win64" fn QueryDisplayConfig(
    flags: u32,
    num_path_array_elements: *mut u32,
    path_info_array: *mut DisplayConfigPathInfo,
    num_mode_info_array_elements: *mut u32,
    mode_info_array: *mut DisplayConfigModeInfo,
    current_topology_id: *mut u32,
) -> i32 {
    println!(
        "[USER32 DISPLAYCONFIG] QueryDisplayConfig(flags=0x{:X})",
        flags
    );
    let bounds = crate::windowing::x11::get_host_display_bounds(0);
    let w = if bounds.w > 0 { bounds.w as u32 } else { 1920 };
    let h = if bounds.h > 0 { bounds.h as u32 } else { 1080 };

    if !num_path_array_elements.is_null()
        && !path_info_array.is_null()
        && *num_path_array_elements >= 1
    {
        *num_path_array_elements = 1;
        let path = &mut *path_info_array;
        path.source_info = DisplayConfigPathSourceInfo {
            adapter_id: Luid {
                low_part: 1,
                high_part: 0,
            },
            id: 0,
            mode_info_idx: 0,
            status_flags: 1,
        };
        path.target_info = DisplayConfigPathTargetInfo {
            adapter_id: Luid {
                low_part: 1,
                high_part: 0,
            },
            id: 0,
            mode_info_idx: 1,
            output_technology: 11, // DISPLAYPORT_EMBEDDED
            rotation: 1,           // IDENTITY
            scaling: 1,            // IDENTITY
            refresh_rate: DisplayConfigRational {
                numerator: 60,
                denominator: 1,
            },
            scanline_ordering: 1, // PROGRESSIVE
            target_available: 1,  // TRUE
            status_flags: 1,      // IN_USE
        };
        path.flags = 1; // DISPLAYCONFIG_PATH_ACTIVE
    }

    if !num_mode_info_array_elements.is_null()
        && !mode_info_array.is_null()
        && *num_mode_info_array_elements >= 2
    {
        *num_mode_info_array_elements = 2;
        // Mode 0: SOURCE mode
        let mode0 = &mut *mode_info_array.add(0);
        mode0.info_type = 1; // SOURCE
        mode0.id = 0;
        mode0.adapter_id = Luid {
            low_part: 1,
            high_part: 0,
        };
        let src_mode = DisplayConfigSourceMode {
            width: w,
            height: h,
            pixel_format: 3, // 32BPP
            position: Point { x: 0, y: 0 },
        };
        std::ptr::copy_nonoverlapping(
            &src_mode as *const _ as *const u8,
            mode0.mode_union.as_mut_ptr(),
            std::mem::size_of::<DisplayConfigSourceMode>(),
        );

        // Mode 1: TARGET mode
        let mode1 = &mut *mode_info_array.add(1);
        mode1.info_type = 2; // TARGET
        mode1.id = 0;
        mode1.adapter_id = Luid {
            low_part: 1,
            high_part: 0,
        };
        let target_mode = DisplayConfigTargetMode {
            target_video_signal_info: DisplayConfigVideoSignalInfo {
                pixel_rate: (w as u64) * (h as u64) * 60,
                h_sync_freq: DisplayConfigRational {
                    numerator: 60 * h,
                    denominator: 1,
                },
                v_sync_freq: DisplayConfigRational {
                    numerator: 60,
                    denominator: 1,
                },
                active_size: DisplayConfig2DRegion { cx: w, cy: h },
                total_size: DisplayConfig2DRegion { cx: w, cy: h },
                video_standard: 0,
                scanline_ordering: 1,
            },
        };
        std::ptr::copy_nonoverlapping(
            &target_mode as *const _ as *const u8,
            mode1.mode_union.as_mut_ptr(),
            std::mem::size_of::<DisplayConfigTargetMode>(),
        );
    }

    if !current_topology_id.is_null() {
        *current_topology_id = 1; // DISPLAYCONFIG_TOPOLOGY_INTERNAL
    }

    0 // ERROR_SUCCESS = 0
}

#[no_mangle]
pub unsafe extern "win64" fn DisplayConfigGetDeviceInfo(
    request_packet: *mut DisplayConfigDeviceInfoHeader,
) -> i32 {
    if request_packet.is_null() {
        return 87; // ERROR_INVALID_PARAMETER
    }
    let header = &*request_packet;
    println!(
        "[USER32 DISPLAYCONFIG] DisplayConfigGetDeviceInfo(type={}, size={})",
        header.info_type, header.size
    );
    match header.info_type {
        1 => {
            // DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME
            if header.size >= 72 {
                let name = "\\\\.\\DISPLAY1\0";
                let ptr = (request_packet as *mut u8)
                    .add(std::mem::size_of::<DisplayConfigDeviceInfoHeader>())
                    as *mut u16;
                for (i, ch) in name.encode_utf16().enumerate().take(32) {
                    *ptr.add(i) = ch;
                }
            }
        }
        2 => {
            // DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME
            if header.size >= 420 {
                let target = request_packet as *mut DisplayConfigTargetDeviceName;
                (*target).flags = 1;
                (*target).output_technology = 11; // DISPLAYPORT_EMBEDDED
                (*target).edid_manufacture_id = 0x10AC;
                (*target).edid_product_code_id = 0x4001;
                (*target).connector_instance = 0;
                let friendly_name = "Generic PnP Monitor\0";
                for (i, ch) in friendly_name.encode_utf16().enumerate().take(64) {
                    (*target).monitor_friendly_device_name[i] = ch;
                }
                let path = "\\\\?\\DISPLAY#MONITOR#0\0";
                for (i, ch) in path.encode_utf16().enumerate().take(128) {
                    (*target).monitor_device_path[i] = ch;
                }
            }
        }
        4 => {
            // DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME
            if header.size >= 72 {
                let name = "\\\\.\\DISPLAY1\0";
                let ptr = (request_packet as *mut u8)
                    .add(std::mem::size_of::<DisplayConfigDeviceInfoHeader>())
                    as *mut u16;
                for (i, ch) in name.encode_utf16().enumerate().take(32) {
                    *ptr.add(i) = ch;
                }
            }
        }
        _ => {}
    }
    0 // ERROR_SUCCESS = 0
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

#[no_mangle]
pub unsafe extern "win64" fn IsMouseInPointerEnabled() -> i32 {
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn EnableMouseInPointer(_f_enable: i32) -> i32 {
    0 // FALSE
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
            let wndproc = get_wndproc_for_hwnd(hwnd);
            if hwnd != 0 && wndproc > 0x10000 && wndproc < 0x7FFFFFFFFFFF {
                let wndproc_fn: unsafe extern "win64" fn(u64, u32, u64, u64) -> isize =
                    std::mem::transmute(wndproc);
                wndproc_fn(hwnd, WM_PAINT, 0, 0);
            }

            // Present SDL2 frame: clear background and invoke present
            // Вывод кадра через SDL2 с очисткой фона
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

#[no_mangle]
pub unsafe extern "win64" fn DialogBoxParamW(
    h_instance: u64,
    lp_template_name: u64,
    h_wnd_parent: u64,
    lp_dialog_func: u64,
    dw_init_param: u64,
) -> isize {
    println!(
        "[USER32] DialogBoxParamW: hInst=0x{:X}, template=0x{:X}, parent=0x{:X}, dlgProc=0x{:X}, initParam=0x{:X}",
        h_instance, lp_template_name, h_wnd_parent, lp_dialog_func, dw_init_param
    );
    if lp_dialog_func > 0x10000 && lp_dialog_func < 0x7FFFFFFFFFFF {
        let dlg_proc: unsafe extern "win64" fn(u64, u32, u64, u64) -> isize =
            std::mem::transmute(lp_dialog_func);
        let _ = dlg_proc(0x00010001, 0x0110, 0, dw_init_param); // WM_INITDIALOG = 0x0110
    }
    if dw_init_param != 0 {
        let ptr = dw_init_param as *const u16;
        let mut u16_chars = Vec::new();
        for i in 0..256 {
            let ch = *ptr.add(i);
            if ch == 0 {
                break;
            }
            u16_chars.push(ch);
        }
        if let Ok(s) = String::from_utf16(&u16_chars) {
            println!("[USER32 DIALOG TEXT] '{}'", s);
        }
    }
    1 // IDOK = 1
}

#[no_mangle]
pub unsafe extern "win64" fn DialogBoxParamA(
    h_instance: u64,
    lp_template_name: u64,
    h_wnd_parent: u64,
    lp_dialog_func: u64,
    dw_init_param: u64,
) -> isize {
    println!(
        "[USER32] DialogBoxParamA: hInst=0x{:X}, template=0x{:X}, parent=0x{:X}, dlgProc=0x{:X}, initParam=0x{:X}",
        h_instance, lp_template_name, h_wnd_parent, lp_dialog_func, dw_init_param
    );
    1 // IDOK = 1
}

#[no_mangle]
pub unsafe extern "win64" fn EndDialog(_h_dlg: u64, _n_result: isize) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetDlgItem(_h_dlg: u64, _n_id_dlg_item: i32) -> u64 {
    0x10001 // Sentinel HWND
}

#[no_mangle]
pub unsafe extern "win64" fn GetParent(_hwnd: u64) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetDesktopWindow() -> u64 {
    0x00010000
}

#[no_mangle]
pub unsafe extern "win64" fn OffsetRect(lprc: *mut Rect, dx: i32, dy: i32) -> i32 {
    if !lprc.is_null() {
        (*lprc).left += dx;
        (*lprc).right += dx;
        (*lprc).top += dy;
        (*lprc).bottom += dy;
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CopyRect(lprc_dst: *mut Rect, lprc_src: *const Rect) -> i32 {
    if !lprc_dst.is_null() && !lprc_src.is_null() {
        *lprc_dst = *lprc_src;
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetRect(
    lprc: *mut Rect,
    x_left: i32,
    y_top: i32,
    x_right: i32,
    y_bottom: i32,
) -> i32 {
    if !lprc.is_null() {
        (*lprc).left = x_left;
        (*lprc).top = y_top;
        (*lprc).right = x_right;
        (*lprc).bottom = y_bottom;
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn PtInRect(lprc: *const Rect, pt_x: i32, pt_y: i32) -> i32 {
    if !lprc.is_null() {
        let r = &*lprc;
        if pt_x >= r.left && pt_x < r.right && pt_y >= r.top && pt_y < r.bottom {
            1
        } else {
            0
        }
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn ShowCursor(b_show: i32) -> i32 {
    let new_count = if b_show != 0 {
        CURSOR_DISPLAY_COUNT.fetch_add(1, Ordering::SeqCst) + 1
    } else {
        CURSOR_DISPLAY_COUNT.fetch_sub(1, Ordering::SeqCst) - 1
    };
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if !sdl_handle.is_null() {
        type SdlShowCursorFn = unsafe extern "C" fn(i32) -> i32;
        if let Ok(sym) = std::ffi::CString::new("SDL_ShowCursor") {
            let p = libc::dlsym(sdl_handle, sym.as_ptr());
            if !p.is_null() {
                let f: SdlShowCursorFn = std::mem::transmute(p);
                f(if new_count >= 0 { 1 } else { 0 });
            }
        }
    }
    new_count
}

#[no_mangle]
pub unsafe extern "win64" fn DestroyCursor(_h_cursor: usize) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn AllowSetForegroundWindow(_dw_process_id: u32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetCaretBlinkTime() -> u32 {
    530
}

#[no_mangle]
pub unsafe extern "win64" fn GetDoubleClickTime() -> u32 {
    500
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterWindowMessageA(lp_string: *const libc::c_char) -> u32 {
    if !lp_string.is_null() {
        let s = std::ffi::CStr::from_ptr(lp_string).to_string_lossy();
        let mut h = 0u32;
        for b in s.bytes() {
            h = h.wrapping_mul(31).wrapping_add(b as u32);
        }
        0xC000 + (h % 0x3FFF)
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn DestroyWindow(hwnd: usize) -> i32 {
    println!("[USER32] DestroyWindow(hWnd=0x{:X})", hwnd);
    SendMessageW(hwnd as u64, 0x0002 /* WM_DESTROY */, 0, 0);
    SendMessageW(hwnd as u64, 0x0082 /* WM_NCDESTROY */, 0, 0);
    if hwnd == 0 || is_primary_window(hwnd as u64) {
        PostQuitMessage(0);
        crate::sys::shutdown::request_host_shutdown("DestroyWindow primary window");
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn UnregisterClassW(_lp_class_name: *const u16, _h_instance: usize) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetTimer(
    hwnd: usize,
    n_id_event: usize,
    u_elapse: u32,
    lp_timer_func: usize,
) -> usize {
    let id = if n_id_event == 0 { 1 } else { n_id_event };
    let elapse = u_elapse.max(10);
    if let Ok(mut timers) = ACTIVE_TIMERS.lock() {
        if let Some(t) = timers.iter_mut().find(|t| t.hwnd == hwnd && t.id == id) {
            t.elapse = elapse;
            t.func = lp_timer_func;
            t.last_tick = std::time::Instant::now();
        } else {
            timers.push(WinTimer {
                hwnd,
                id,
                elapse,
                func: lp_timer_func,
                last_tick: std::time::Instant::now(),
            });
        }
    }
    id
}

#[no_mangle]
pub unsafe extern "win64" fn KillTimer(hwnd: usize, u_id_event: usize) -> i32 {
    if let Ok(mut timers) = ACTIVE_TIMERS.lock() {
        timers.retain(|t| !(t.hwnd == hwnd && t.id == u_id_event));
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn PostQuitMessage(n_exit_code: i32) {
    eprintln!("[USER32] PostQuitMessage({})", n_exit_code);
    let mut guard = get_user32_input();
    if let Some(input) = guard.as_mut() {
        input.message_queue.push_back(WinMsg {
            hwnd: 0,
            message: 0x0012, // WM_QUIT
            w_param: n_exit_code as u64,
            l_param: 0,
            time: crate::dlls::kernel32::GetTickCount(),
            pt_x: 0,
            pt_y: 0,
        });
    }
    crate::sys::shutdown::request_host_shutdown(&format!("PostQuitMessage({})", n_exit_code));
}

#[no_mangle]
pub unsafe extern "win64" fn MsgWaitForMultipleObjects(
    n_count: u32,
    p_handles: *const usize,
    b_wait_all: i32,
    dw_milliseconds: u32,
    dw_wake_mask: u32,
) -> u32 {
    let dw_flags = if b_wait_all != 0 { 1 } else { 0 };
    MsgWaitForMultipleObjectsEx(n_count, p_handles, dw_milliseconds, dw_wake_mask, dw_flags)
}

#[no_mangle]
pub unsafe extern "win64" fn MsgWaitForMultipleObjectsEx(
    n_count: u32,
    p_handles: *const usize,
    dw_milliseconds: u32,
    _dw_wake_mask: u32,
    dw_flags: u32,
) -> u32 {
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 0x102;
    const WAIT_FAILED: u32 = 0xFFFFFFFF;

    if n_count > 64 {
        return WAIT_FAILED;
    }

    let b_wait_all = (dw_flags & 1) != 0;
    let handles: Vec<u64> = if n_count > 0 && !p_handles.is_null() {
        (0..n_count as usize)
            .map(|i| *p_handles.add(i) as u64)
            .collect()
    } else {
        Vec::new()
    };

    let cur_tid = crate::dlls::kernel32::GetCurrentThreadId();
    let start = std::time::Instant::now();
    let timeout = if dw_milliseconds == 0xFFFFFFFF {
        None
    } else {
        Some(std::time::Duration::from_millis(dw_milliseconds as u64))
    };

    loop {
        // 1. Poll SDL2 events and enqueue into message_queue
        if let Some(fn_poll) = get_sdl_poll_event_fn() {
            let mut sdl_event = [0u8; 56];
            while fn_poll(sdl_event.as_mut_ptr()) != 0 {
                poll_and_enqueue_sdl_event(&sdl_event);
            }
        }

        // 2. Check for pending window messages
        let has_messages = {
            let guard = get_user32_input();
            guard.as_ref().map(|input| !input.message_queue.is_empty()).unwrap_or(false)
        };

        if has_messages {
            return WAIT_OBJECT_0 + n_count;
        }

        // 3. Check handles non-destructively
        if !handles.is_empty() {
            if b_wait_all {
                let mut all_signaled = true;
                for &h in &handles {
                    if !crate::dlls::kernel32::is_handle_signaled(h, cur_tid) {
                        all_signaled = false;
                        break;
                    }
                }
                if all_signaled {
                    for &h in &handles {
                        crate::dlls::kernel32::consume_handle_signal(h, cur_tid);
                    }
                    return WAIT_OBJECT_0;
                }
            } else {
                for (i, &h) in handles.iter().enumerate() {
                    if crate::dlls::kernel32::is_handle_signaled(h, cur_tid) {
                        crate::dlls::kernel32::consume_handle_signal(h, cur_tid);
                        return WAIT_OBJECT_0 + i as u32;
                    }
                }
            }
        }

        if let Some(t) = timeout {
            if start.elapsed() >= t {
                return WAIT_TIMEOUT;
            }
        }

        std::thread::sleep(std::time::Duration::from_micros(500));
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindow(_hwnd: u64, _u_cmd: u32) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn EnumWindows(
    lp_enum_func: Option<unsafe extern "win64" fn(u64, isize) -> i32>,
    l_param: isize,
) -> i32 {
    if let Some(f) = lp_enum_func {
        let hwnd = PRIMARY_HWND.load(std::sync::atomic::Ordering::Relaxed);
        if hwnd != 0 {
            f(hwnd, l_param);
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn LoadImageW(
    _h_inst: usize,
    _name: *const u16,
    _type: u32,
    _cx: i32,
    _cy: i32,
    _fu_load: u32,
) -> usize {
    0x1000
}

#[no_mangle]
pub unsafe extern "win64" fn OpenClipboard(_hwnd: usize) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CloseClipboard() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn EmptyClipboard() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetClipboardData(_u_format: u32) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SetClipboardData(_u_format: u32, h_mem: usize) -> usize {
    h_mem
}

#[no_mangle]
pub unsafe extern "win64" fn IsClipboardFormatAvailable(_u_format: u32) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetKeyboardLayoutNameW(pwsz_klid: *mut u16) -> i32 {
    if !pwsz_klid.is_null() {
        let s = [
            b'0' as u16, b'0' as u16, b'0' as u16, b'0' as u16,
            b'0' as u16, b'4' as u16, b'0' as u16, b'9' as u16, 0,
        ];
        std::ptr::copy_nonoverlapping(s.as_ptr(), pwsz_klid, s.len());
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn MapVirtualKeyExA(u_code: u32, u_map_type: u32, _dwhkl: usize) -> u32 {
    MapVirtualKeyA(u_code, u_map_type)
}

#[no_mangle]
pub unsafe extern "win64" fn GetKeyNameTextW(_l_param: i32, lp_string: *mut u16, cch_size: i32) -> i32 {
    if !lp_string.is_null() && cch_size > 0 {
        *lp_string = 0;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetThreadDesktop(_dw_thread_id: u32) -> usize {
    0x100
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserObjectInformationA(
    _h_obj: usize,
    _n_index: i32,
    _pv_info: *mut c_void,
    _n_length: u32,
    lpn_length_needed: *mut u32,
) -> i32 {
    if !lpn_length_needed.is_null() {
        *lpn_length_needed = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SendMessageTimeoutA(
    hwnd: usize,
    msg: u32,
    w_param: usize,
    l_param: isize,
    _fu_flags: u32,
    _u_timeout: u32,
    lpdw_result: *mut usize,
) -> isize {
    let res = SendMessageA(hwnd as u64, msg, w_param as u64, l_param as u64);
    if !lpdw_result.is_null() {
        *lpdw_result = res as usize;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetRawInputBuffer(
    p_data: *mut c_void,
    pcb_size: *mut u32,
    _cb_size_header: u32,
) -> u32 {
    if pcb_size.is_null() {
        return 0xFFFFFFFF;
    }
    let item_size = std::mem::size_of::<RawInput>() as u32; // 48
    let mut q = RAW_INPUT_BUFFER_QUEUE.lock().unwrap();
    let queue = q.get_or_insert_with(VecDeque::new);

    if p_data.is_null() {
        let count = queue.len() as u32;
        *pcb_size = count * item_size;
        return 0;
    }

    let buf_capacity = *pcb_size / item_size;
    if buf_capacity == 0 {
        *pcb_size = item_size;
        return 0xFFFFFFFF;
    }

    let count_to_copy = (queue.len() as u32).min(buf_capacity);
    if count_to_copy == 0 {
        *pcb_size = 0;
        return 0; // Buffer is empty - critical to terminate Unity/Rewired while(GetRawInputBuffer > 0) loops
    }

    let mut out_ptr = p_data as *mut RawInput;
    for _ in 0..count_to_copy {
        if let Some(item) = queue.pop_front() {
            std::ptr::write_unaligned(out_ptr, item);
            out_ptr = out_ptr.add(1);
        }
    }

    *pcb_size = count_to_copy * item_size;
    count_to_copy
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerTouchInfoHistory(
    _pointer_id: u32,
    entries_count: *mut u32,
    _touch_info: *mut c_void,
) -> i32 {
    if !entries_count.is_null() {
        *entries_count = 0;
    }
    crate::dlls::kernel32::SetLastError(crate::dlls::setupapi::ERROR_INVALID_PARAMETER);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetPointerPenInfoHistory(
    _pointer_id: u32,
    entries_count: *mut u32,
    _pen_info: *mut c_void,
) -> i32 {
    if !entries_count.is_null() {
        *entries_count = 0;
    }
    crate::dlls::kernel32::SetLastError(crate::dlls::setupapi::ERROR_INVALID_PARAMETER);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SendDlgItemMessageW(
    _h_dlg: u64,
    _n_id_dlg_item: i32,
    _msg: u32,
    _w_param: u64,
    _l_param: u64,
) -> isize {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SetDlgItemTextW(
    _h_dlg: u64,
    _n_id_dlg_item: i32,
    lp_string: *const u16,
) -> i32 {
    if !lp_string.is_null() {
        let mut u16_chars = Vec::new();
        for i in 0..256 {
            let ch = *lp_string.add(i);
            if ch == 0 {
                break;
            }
            u16_chars.push(ch);
        }
        if let Ok(s) = String::from_utf16(&u16_chars) {
            println!("[USER32 DLG ITEM TEXT] '{}'", s);
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetDlgItemTextA(
    _h_dlg: u64,
    _n_id_dlg_item: i32,
    lp_string: *const u8,
) -> i32 {
    if !lp_string.is_null() {
        let s = std::ffi::CStr::from_ptr(lp_string as *const i8).to_string_lossy();
        println!("[USER32 DLG ITEM TEXT] '{}'", s);
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDialogParamW(
    h_instance: u64,
    lp_template_name: u64,
    h_wnd_parent: u64,
    lp_dialog_func: u64,
    dw_init_param: u64,
) -> u64 {
    println!(
        "[USER32] CreateDialogParamW: hInst=0x{:X}, template=0x{:X}, parent=0x{:X}, dlgProc=0x{:X}, initParam=0x{:X}",
        h_instance, lp_template_name, h_wnd_parent, lp_dialog_func, dw_init_param
    );
    0x10001
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDialogParamA(
    h_instance: u64,
    lp_template_name: u64,
    h_wnd_parent: u64,
    lp_dialog_func: u64,
    dw_init_param: u64,
) -> u64 {
    println!(
        "[USER32] CreateDialogParamA: hInst=0x{:X}, template=0x{:X}, parent=0x{:X}, dlgProc=0x{:X}, initParam=0x{:X}",
        h_instance, lp_template_name, h_wnd_parent, lp_dialog_func, dw_init_param
    );
    0x10001
}

#[no_mangle]
pub unsafe extern "win64" fn SendMessageTimeoutW(
    hwnd: usize,
    msg: u32,
    w_param: usize,
    l_param: isize,
    _fu_flags: u32,
    _u_timeout: u32,
    lpdw_result: *mut usize,
) -> isize {
    let res = SendMessageW(hwnd as u64, msg, w_param as u64, l_param as u64);
    if !lpdw_result.is_null() {
        *lpdw_result = res as usize;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn MapVirtualKeyExW(u_code: u32, u_map_type: u32, _dwhkl: usize) -> u32 {
    MapVirtualKeyW(u_code, u_map_type)
}

#[no_mangle]
pub unsafe extern "win64" fn GetCursorInfo(pci: *mut libc::c_void) -> i32 {
    if pci.is_null() { return 0; }
    let ptr = pci as *mut u32;
    let cb_size = *ptr;
    if cb_size < 24 { return 0; }
    std::ptr::write_bytes(pci as *mut u8, 0, cb_size as usize);
    *ptr = cb_size;
    *ptr.add(1) = 1; // CURSOR_SHOWING
    *(ptr.add(2) as *mut u64) = 0x10001; // hCursor
    let cx = LAST_CURSOR_X.load(Ordering::Relaxed);
    let cy = LAST_CURSOR_Y.load(Ordering::Relaxed);
    let wx = CACHED_WIN_X.load(Ordering::Relaxed);
    let wy = CACHED_WIN_Y.load(Ordering::Relaxed);
    *(ptr.add(4) as *mut i32) = wx + cx;
    *(ptr.add(5) as *mut i32) = wy + cy;
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn EnumChildWindows(
    _hwnd_parent: usize,
    _lp_enum_func: usize,
    _l_param: isize,
) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetIconInfo(
    _h_icon: usize,
    p_icon_info: *mut libc::c_void,
) -> i32 {
    if p_icon_info.is_null() { return 0; }
    std::ptr::write_bytes(p_icon_info as *mut u8, 0, 32);
    *(p_icon_info as *mut i32) = 1; // fIcon = TRUE
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowThreadProcessId(
    _hwnd: usize,
    lpdw_process_id: *mut u32,
) -> u32 {
    if !lpdw_process_id.is_null() {
        *lpdw_process_id = std::process::id();
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetClassLongPtrW(
    _hwnd: usize,
    _n_index: i32,
    _dw_new_long: isize,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetClassLongPtrW(
    _hwnd: usize,
    _n_index: i32,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn IsWindowUnicode(_hwnd: usize) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SendInput(
    c_inputs: u32,
    _p_inputs: *mut libc::c_void,
    _cb_size: i32,
) -> u32 {
    c_inputs
}

#[no_mangle]
pub unsafe extern "win64" fn IsChild(_hwnd_parent: usize, _hwnd: usize) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn CreateCaret(_hwnd: usize, _h_bitmap: usize, _n_width: i32, _n_height: i32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn DestroyCaret() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetCaretPos(_x: i32, _y: i32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetKeyboardLayoutList(n_buff: i32, lp_list: *mut usize) -> i32 {
    if n_buff > 0 && !lp_list.is_null() {
        *lp_list = 0x04090409;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ActivateKeyboardLayout(hkl: usize, _flags: u32) -> usize {
    if hkl != 0 { hkl } else { 0x04090409 }
}

#[no_mangle]
pub unsafe extern "win64" fn WindowFromDC(_hdc: usize) -> usize {
    GetActiveWindow()
}

#[no_mangle]
pub unsafe extern "win64" fn VkKeyScanExW(ch: u16, _dwhkl: usize) -> i16 {
    VkKeyScanW(ch)
}

#[no_mangle]
pub unsafe extern "win64" fn CallWindowProcW(
    lp_prev_wnd_func: u64,
    hwnd: u64,
    msg: u32,
    wparam: u64,
    lparam: u64,
) -> isize {
    if lp_prev_wnd_func != 0 {
        let f: unsafe extern "win64" fn(u64, u32, u64, u64) -> isize =
            std::mem::transmute(lp_prev_wnd_func);
        f(hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CallWindowProcA(
    lp_prev_wnd_func: u64,
    hwnd: u64,
    msg: u32,
    wparam: u64,
    lparam: u64,
) -> isize {
    CallWindowProcW(lp_prev_wnd_func, hwnd, msg, wparam, lparam)
}

#[no_mangle]
pub unsafe extern "win64" fn FillRect(_hdc: u64, _lprc: *const u8, _hbr: u64) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn FlashWindowEx(_pfwi: *mut u8) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetDCEx(hwnd: u64, _hrgn_clip: u64, _flags: u32) -> u64 {
    GetDC(hwnd)
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowDC(hwnd: u64) -> u64 {
    GetDC(hwnd)
}

#[no_mangle]
pub unsafe extern "win64" fn GetUpdateRect(
    _hwnd: usize,
    lp_rect: *mut [i32; 4],
    _b_erase: i32,
) -> i32 {
    if !lp_rect.is_null() {
        *lp_rect = [0, 0, 0, 0];
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SetWindowRgn(
    _hwnd: usize,
    _hrgn: usize,
    _b_redraw: i32,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn RegisterClipboardFormatA(_lpsz_format: *const libc::c_char) -> u32 {
    0xC001
}

#[no_mangle]
pub unsafe extern "win64" fn WaitForInputIdle(_h_process: usize, _dw_milliseconds: u32) -> u32 {
    0 // WAIT_OBJECT_0
}

#[no_mangle]
pub unsafe extern "win64" fn wsprintfA(
    buffer: *mut libc::c_char,
    format: *const libc::c_char,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
    a7: u64,
    a8: u64,
    a9: u64,
    a10: u64,
    a11: u64,
    a12: u64,
    a13: u64,
    a14: u64,
) -> i32 {
    let args = [a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14];
    crate::dlls::crt::format_win64_vsprintf(buffer, 1024, format, args.as_ptr() as *mut u64)
}

#[no_mangle]
pub unsafe extern "win64" fn wsprintfW(
    buffer: *mut u16,
    format: *const u16,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
    a7: u64,
    a8: u64,
    a9: u64,
    a10: u64,
    a11: u64,
    a12: u64,
    a13: u64,
    a14: u64,
) -> i32 {
    let args = [a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14];
    crate::dlls::crt::format_win64_vswprintf(buffer, 1024, format, args.as_ptr() as *mut u64)
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

    #[test]
    fn test_window_creation_visibility_and_x11_contract() {
        let test_hwnd = 0x00020001u64;
        let width = 1280u32;
        let height = 720u32;

        // 1. Register window in WindowRegistry
        register_hwnd(test_hwnd, 0x10000, width, height);

        // 2. Initial state contract: window should NOT be visible initially
        WINDOW_VISIBLE.store(false, Ordering::SeqCst);
        assert_eq!(unsafe { IsWindowVisible(test_hwnd as usize) }, 0);

        // 3. Geometry contract: GetClientRect and GetWindowRect return non-zero dimensions
        let mut client_rc = Rect::default();
        let mut window_rc = Rect::default();
        unsafe {
            assert_eq!(GetClientRect(test_hwnd as usize, &mut client_rc), 1);
            assert_eq!(GetWindowRect(test_hwnd as usize, &mut window_rc), 1);
        }
        assert_eq!(client_rc.right - client_rc.left, width as i32);
        assert_eq!(client_rc.bottom - client_rc.top, height as i32);
        assert_eq!(window_rc.right - window_rc.left, width as i32);
        assert_eq!(window_rc.bottom - window_rc.top, height as i32);

        // 4. ShowWindow(SW_SHOW) transitions visibility to TRUE and dispatches messages
        let show_res = unsafe {
            ShowWindow(test_hwnd as usize, 1 /* SW_SHOWNORMAL */)
        };
        assert_eq!(show_res, 1);
        assert_eq!(unsafe { IsWindowVisible(test_hwnd as usize) }, 1);

        // 5. WindowRegistry contract: window is mapped to X11 backend with valid dimensions
        let reg_win = crate::graphics::vulkan_bridge::handles::WindowRegistry::global()
            .lookup(test_hwnd as usize)
            .expect("RineWindow must be registered");
        assert_eq!(reg_win.guest_hwnd, test_hwnd as usize);
        assert_eq!(
            reg_win.backend,
            crate::graphics::vulkan_bridge::handles::WindowBackendType::X11
        );
        assert_eq!(reg_win.width, width);
        assert_eq!(reg_win.height, height);

        // 6. ShowWindow(SW_HIDE) transitions visibility back to FALSE
        let hide_res = unsafe {
            ShowWindow(test_hwnd as usize, 0 /* SW_HIDE */)
        };
        assert_eq!(hide_res, 1);
        assert_eq!(unsafe { IsWindowVisible(test_hwnd as usize) }, 0);
    }

    #[test]
    fn test_display_config_mode_info_layout() {
        assert_eq!(
            std::mem::size_of::<DisplayConfigModeInfo>(),
            64,
            "DISPLAYCONFIG_MODE_INFO must be exactly 64 bytes in Win32/Win64 ABI"
        );
        assert_eq!(
            std::mem::size_of::<DisplayConfigPathInfo>(),
            72,
            "DISPLAYCONFIG_PATH_INFO must be exactly 72 bytes in Win32/Win64 ABI"
        );
        assert_eq!(
            std::mem::size_of::<DisplayConfigTargetDeviceName>(),
            420,
            "DISPLAYCONFIG_TARGET_DEVICE_NAME must be exactly 420 bytes in Win32/Win64 ABI"
        );
    }
}
