use std::ffi::{c_char, c_int, c_ulong, c_void, CString};

pub type Display = *mut std::ffi::c_void;
pub type Window = c_ulong;

#[link(name = "X11")]
extern "C" {
    pub fn XOpenDisplay(display_name: *const c_char) -> Display;
    pub fn XDefaultScreen(display: Display) -> c_int;
    pub fn XRootWindow(display: Display, screen_number: c_int) -> Window;
    pub fn XBlackPixel(display: Display, screen_number: c_int) -> c_ulong;
    pub fn XWhitePixel(display: Display, screen_number: c_int) -> c_ulong;
    pub fn XCreateSimpleWindow(
        display: Display,
        parent: Window,
        x: c_int,
        y: c_int,
        width: u32,
        height: u32,
        border_width: u32,
        border: c_ulong,
        background: c_ulong,
    ) -> Window;
    pub fn XStoreName(display: Display, w: Window, window_name: *const c_char) -> c_int;
    pub fn XSelectInput(display: Display, w: Window, event_mask: c_ulong) -> c_int;
    pub fn XMapWindow(display: Display, w: Window) -> c_int;
    pub fn XMapRaised(display: Display, w: Window) -> c_int;
    pub fn XSetInputFocus(
        display: Display,
        focus: Window,
        revert_to: c_int,
        time: c_ulong,
    ) -> c_int;
    pub fn XFlush(display: Display) -> c_int;
    pub fn XSync(display: Display, discard: c_int) -> c_int;
    pub fn XSetErrorHandler(handler: *const c_void) -> *mut c_void;
}

pub unsafe extern "C" fn rine_x11_error_handler(
    _display: Display,
    _error_event: *mut c_void,
) -> c_int {
    println!("[X11 ERROR HANDLER] Intercepted X11 protocol error (handled safely without exit)");
    0
}

pub static GLOBAL_DISPLAY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static GLOBAL_WINDOW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static GLOBAL_SDL_WINDOW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static GLOBAL_VISUAL_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static GLOBAL_GC: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// SDL_Renderer* — used by VulkanSwapChainPresenter for real frame presentation
pub static GLOBAL_RENDERER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// SDL library handle — cached for reuse
static GLOBAL_SDL_HANDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn get_global_display() -> Display {
    GLOBAL_DISPLAY.load(std::sync::atomic::Ordering::SeqCst) as Display
}

pub fn get_global_window() -> Window {
    GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst) as Window
}

pub fn get_global_sdl_window() -> *mut c_void {
    GLOBAL_SDL_WINDOW.load(std::sync::atomic::Ordering::SeqCst) as *mut c_void
}

pub fn get_global_visual_id() -> u64 {
    GLOBAL_VISUAL_ID.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn get_global_renderer() -> *mut c_void {
    GLOBAL_RENDERER.load(std::sync::atomic::Ordering::SeqCst) as *mut c_void
}

pub fn get_sdl_handle() -> *mut c_void {
    GLOBAL_SDL_HANDLE.load(std::sync::atomic::Ordering::SeqCst) as *mut c_void
}

#[link(name = "X11")]
extern "C" {
    pub fn XCreateGC(
        display: Display,
        d: Window,
        valuemask: c_ulong,
        values: *mut c_void,
    ) -> *mut c_void;
    pub fn XDefaultVisual(display: Display, screen_number: c_int) -> *mut c_void;
    pub fn XDefaultDepth(display: Display, screen_number: c_int) -> c_int;
    pub fn XVisualIDFromVisual(visual: *mut c_void) -> c_ulong;
    pub fn XCreateImage(
        display: Display,
        visual: *mut c_void,
        depth: u32,
        format: c_int,
        offset: c_int,
        data: *mut u8,
        width: u32,
        height: u32,
        bitmap_pad: c_int,
        bytes_per_line: c_int,
    ) -> *mut c_void;
    pub fn XPutImage(
        display: Display,
        d: Window,
        gc: *mut c_void,
        image: *mut c_void,
        src_x: c_int,
        src_y: c_int,
        dest_x: c_int,
        dest_y: c_int,
        width: u32,
        height: u32,
    ) -> c_int;
    pub fn XDestroyImage(image: *mut c_void) -> c_int;
}

#[link(name = "X11")]
extern "C" {
    pub fn XPending(display: Display) -> c_int;
    pub fn XNextEvent(display: Display, event_return: *mut u8) -> c_int;
}

/// Creates a SDL_Renderer attached to the given SDL_Window*.
/// Returns the SDL_Renderer* pointer, or null on failure.
pub unsafe fn create_sdl2_renderer(
    sdl_handle: *mut c_void,
    window_ptr: *mut c_void,
) -> *mut c_void {
    if sdl_handle.is_null() || window_ptr.is_null() {
        return std::ptr::null_mut();
    }

    type SdlCreateRendererFn = unsafe extern "C" fn(*mut c_void, c_int, u32) -> *mut c_void;
    let sym_name = CString::new("SDL_CreateRenderer").unwrap();
    let fn_ptr = libc::dlsym(sdl_handle, sym_name.as_ptr());
    if fn_ptr.is_null() {
        println!("[X11 SDL2] SDL_CreateRenderer symbol not found");
        return std::ptr::null_mut();
    }
    let fn_create_renderer: SdlCreateRendererFn = std::mem::transmute(fn_ptr);

    // SDL_RENDERER_ACCELERATED = 0x00000002, SDL_RENDERER_PRESENTVSYNC = 0x00000004
    let renderer = fn_create_renderer(window_ptr, -1, 0x00000002);
    if !renderer.is_null() {
        GLOBAL_RENDERER.store(renderer as u64, std::sync::atomic::Ordering::SeqCst);
        println!("[X11 SDL2] SDL_Renderer created: {:p}", renderer);
    } else {
        // Fallback: software renderer
        let renderer_sw = fn_create_renderer(window_ptr, -1, 0x00000001); // SDL_RENDERER_SOFTWARE
        if !renderer_sw.is_null() {
            GLOBAL_RENDERER.store(renderer_sw as u64, std::sync::atomic::Ordering::SeqCst);
            println!(
                "[X11 SDL2] SDL_Renderer (software fallback) created: {:p}",
                renderer_sw
            );
        }
    }

    GLOBAL_RENDERER.load(std::sync::atomic::Ordering::SeqCst) as *mut c_void
}

pub fn ensure_x11_environment() {
    let need_display = match std::env::var("DISPLAY") {
        Ok(s) => s.trim().is_empty(),
        Err(_) => true,
    };
    if need_display {
        std::env::set_var("DISPLAY", ":0");
    }

    let need_auth = match std::env::var("XAUTHORITY") {
        Ok(s) => s.trim().is_empty(),
        Err(_) => true,
    };
    if need_auth {
        let runtime_dir = format!("/run/user/{}", unsafe { libc::getuid() });
        if let Ok(entries) = std::fs::read_dir(&runtime_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with(".mutter-Xwaylandauth") || name_str == ".Xauthority" {
                    let full_path = entry.path();
                    std::env::set_var("XAUTHORITY", &full_path);
                    println!(
                        "[X11 WINDOWING] Auto-detected XAUTHORITY={}",
                        full_path.display()
                    );
                    break;
                }
            }
        }
    }

    unsafe {
        XSetErrorHandler(rine_x11_error_handler as *const c_void);
    }
}

/// Initialises SDL2 video subsystem and returns (sdl_handle, window_ptr).
/// The window is shown on the desktop with the given title and size.
pub unsafe fn init_sdl2_and_create_window(
    title: &str,
    width: u32,
    height: u32,
) -> (*mut c_void, *mut c_void) {
    let existing_win = GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst);
    let existing_sdl = GLOBAL_SDL_HANDLE.load(std::sync::atomic::Ordering::SeqCst);
    if existing_win != 0 && existing_sdl != 0 {
        return (existing_sdl as *mut c_void, existing_win as *mut c_void);
    }

    ensure_x11_environment();
    let sdl_lib_name = CString::new("libSDL2-2.0.so.0").unwrap();
    let sdl_handle = libc::dlopen(sdl_lib_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);

    if sdl_handle.is_null() {
        println!("[X11 SDL2] libSDL2-2.0.so.0 not found");
        return (std::ptr::null_mut(), std::ptr::null_mut());
    }

    GLOBAL_SDL_HANDLE.store(sdl_handle as u64, std::sync::atomic::Ordering::SeqCst);

    type SdlInitFn = unsafe extern "C" fn(u32) -> i32;
    type SdlCreateWindowFn =
        unsafe extern "C" fn(*const c_char, i32, i32, i32, i32, u32) -> *mut c_void;

    let fn_init: SdlInitFn = std::mem::transmute(libc::dlsym(
        sdl_handle,
        CString::new("SDL_Init").unwrap().as_ptr(),
    ));
    let fn_create_window: SdlCreateWindowFn = std::mem::transmute(libc::dlsym(
        sdl_handle,
        CString::new("SDL_CreateWindow").unwrap().as_ptr(),
    ));

    if std::env::var_os("DISPLAY").is_some() {
        std::env::set_var("SDL_VIDEODRIVER", "x11");
    }

    fn_init(0x00000020); // SDL_INIT_VIDEO

    let title_c = CString::new(title).unwrap_or_else(|_| CString::new("Rine").unwrap());
    let window_ptr = fn_create_window(
        title_c.as_ptr(),
        0x2FFF0000, // SDL_WINDOWPOS_CENTERED
        0x2FFF0000,
        width as i32,
        height as i32,
        0x00000004 | 0x00000020 | 0x10000000, // SDL_WINDOW_SHOWN | SDL_WINDOW_RESIZABLE | SDL_WINDOW_VULKAN
    );

    if !window_ptr.is_null() {
        GLOBAL_SDL_WINDOW.store(window_ptr as u64, std::sync::atomic::Ordering::SeqCst);
        GLOBAL_WINDOW.store(window_ptr as u64, std::sync::atomic::Ordering::SeqCst);
        println!(
            "[NATIVE DESKTOP WINDOW] SDL2 window created: {:p}, Title='{}', Size={}x{}",
            window_ptr, title, width, height
        );
        let sym_pump = libc::dlsym(sdl_handle, CString::new("SDL_PumpEvents").unwrap().as_ptr());
        if !sym_pump.is_null() {
            let fn_pump: unsafe extern "C" fn() = std::mem::transmute(sym_pump);
            fn_pump();
        }
        let x11_id = get_x11_window_id(window_ptr);
        if x11_id != 0 {
            GLOBAL_WINDOW.store(x11_id, std::sync::atomic::Ordering::SeqCst);
        }
    }

    (sdl_handle, window_ptr)
}

#[repr(C)]
struct SdlSysWMinfo {
    version: [u8; 3], // major, minor, patch
    subsystem: u32,   // SYSWM_X11 = 1, SYSWM_WAYLAND = 6
    info: [u8; 64],   // union containing x11.window at offset 8 (Display* at 0, Window at 8)
}

pub unsafe fn get_x11_window_id(window_ptr: *mut c_void) -> u64 {
    let sdl_handle = get_sdl_handle();
    if sdl_handle.is_null() || window_ptr.is_null() {
        return 0;
    }
    type SdlGetWMInfoFn = unsafe extern "C" fn(*mut c_void, *mut SdlSysWMinfo) -> i32;
    let sym = libc::dlsym(
        sdl_handle,
        CString::new("SDL_GetWindowWMInfo").unwrap().as_ptr(),
    );
    if sym.is_null() {
        return 0;
    }
    let get_wm_info: SdlGetWMInfoFn = std::mem::transmute(sym);
    let mut info: SdlSysWMinfo = std::mem::zeroed();
    type SdlGetVersionFn = unsafe extern "C" fn(*mut [u8; 3]);
    let sym_ver = libc::dlsym(sdl_handle, CString::new("SDL_GetVersion").unwrap().as_ptr());
    if !sym_ver.is_null() {
        let get_ver: SdlGetVersionFn = std::mem::transmute(sym_ver);
        get_ver(&mut info.version);
    } else {
        info.version[0] = 2;
        info.version[1] = 0;
        info.version[2] = 0;
    }
    println!(
        "[X11 SYSWM] Probing WMInfo with SDL Version {}.{}.{}",
        info.version[0], info.version[1], info.version[2]
    );
    let wm_res = get_wm_info(window_ptr, &mut info);
    println!(
        "[X11 SYSWM] SDL_GetWindowWMInfo returned {}, subsystem={}",
        wm_res, info.subsystem
    );
    if wm_res != 0 {
        let x11_dpy_ptr = (&info.info[0]) as *const u8 as *const Display;
        let x11_win_ptr = (&info.info[8]) as *const u8 as *const u64;
        let dpy = *x11_dpy_ptr;
        let x11_id = *x11_win_ptr;

        if !dpy.is_null() {
            GLOBAL_DISPLAY.store(dpy as u64, std::sync::atomic::Ordering::SeqCst);
            XFlush(dpy);
            let screen = XDefaultScreen(dpy);
            let visual = XDefaultVisual(dpy, screen);
            if !visual.is_null() {
                let vid = XVisualIDFromVisual(visual) as u64;
                GLOBAL_VISUAL_ID.store(vid, std::sync::atomic::Ordering::SeqCst);
                println!(
                    "[X11 SYSWM] Extracted Display={:p}, Window=0x{:X}, VisualID=0x{:X}",
                    dpy, x11_id, vid
                );
            }
            XFlush(dpy);
        }
        return x11_id;
    }
    0
}

/// Opens an ACTUAL physical window on the user's Linux desktop screen via SDL2 (supporting Wayland and X11).
/// Called from main.rs at startup with the EXE name as title.
pub unsafe fn create_physical_x11_window(title: &str, width: u32, height: u32) -> u64 {
    let existing = GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst);
    if existing != 0 {
        return existing;
    }
    ensure_x11_environment();
    if cfg!(test) {
        return 0x00010001u64;
    }

    let (_, window_ptr) = init_sdl2_and_create_window(title, width, height);
    if !window_ptr.is_null() && GLOBAL_DISPLAY.load(std::sync::atomic::Ordering::SeqCst) != 0 {
        return window_ptr as u64;
    }

    // X11 creation / fallback
    if std::env::var_os("DISPLAY").is_none() {
        if !window_ptr.is_null() {
            return window_ptr as u64;
        }
        println!(
            "[X11 WINDOWING] Skipping X11 window creation: DISPLAY environment variable not set."
        );
        return 0;
    }

    let display = XOpenDisplay(std::ptr::null());

    if display.is_null() {
        println!("[X11 WINDOWING] Failed to open X11 Display (DISPLAY env var)");
        return 0;
    }

    let screen = XDefaultScreen(display);
    let root_win = XRootWindow(display, screen);

    let window = XCreateSimpleWindow(
        display,
        root_win,
        100,
        100,
        width,
        height,
        1,
        XBlackPixel(display, screen),
        XBlackPixel(display, screen),
    );

    let gc = XCreateGC(display, window, 0, std::ptr::null_mut());

    let title_c = CString::new(title).unwrap();
    XStoreName(display, window, title_c.as_ptr());
    XSelectInput(display, window, 1 | 2 | 4 | 64); // Exposure, KeyPress, ButtonPress, PointerMotion
    XMapWindow(display, window);
    XMapRaised(display, window);
    XFlush(display);

    let visual = XDefaultVisual(display, screen);
    if !visual.is_null() {
        let vid = XVisualIDFromVisual(visual) as u64;
        GLOBAL_VISUAL_ID.store(vid, std::sync::atomic::Ordering::SeqCst);
    }

    GLOBAL_DISPLAY.store(display as u64, std::sync::atomic::Ordering::SeqCst);
    GLOBAL_WINDOW.store(window as u64, std::sync::atomic::Ordering::SeqCst);
    GLOBAL_GC.store(gc as u64, std::sync::atomic::Ordering::SeqCst);

    println!(
        "[X11 WINDOWING] X11 fallback window created: Display={:p}, Window=0x{:X}, Title='{}', Size={}x{}",
        display, window, title, width, height
    );

    window as u64
}

pub unsafe fn render_frame_to_x11_window(bits: *const u8, width: u32, height: u32) {
    let display = GLOBAL_DISPLAY.load(std::sync::atomic::Ordering::SeqCst) as Display;
    let window = GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst) as Window;
    let gc = GLOBAL_GC.load(std::sync::atomic::Ordering::SeqCst) as *mut c_void;

    if !display.is_null() && window != 0 && !gc.is_null() && !bits.is_null() {
        let screen = XDefaultScreen(display);
        let visual = XDefaultVisual(display, screen);
        let depth = XDefaultDepth(display, screen) as u32;

        let img = XCreateImage(
            display,
            visual,
            depth,
            2, // ZPixmap
            0,
            bits as *mut u8,
            width,
            height,
            32,
            (width * 4) as c_int,
        );

        if !img.is_null() {
            XPutImage(display, window, gc, img, 0, 0, 0, 0, width, height);
            XFlush(display);
        }
    }
}

pub unsafe fn process_x11_events(display: Display, _window: Window) {
    if !display.is_null() {
        XFlush(display);
    }
}

pub fn pump_x11_sdl_events() {
    let sdl_handle = get_sdl_handle();
    if !sdl_handle.is_null() {
        type SdlPumpEventsFn = unsafe extern "C" fn();
        let sym_pump = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_PumpEvents").unwrap().as_ptr(),
            )
        };
        if !sym_pump.is_null() {
            let fn_pump: SdlPumpEventsFn = unsafe { std::mem::transmute(sym_pump) };
            unsafe {
                fn_pump();
            }
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SdlRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SdlDisplayMode {
    pub format: u32,
    pub w: i32,
    pub h: i32,
    pub refresh_rate: i32,
    pub driverdata: *mut std::ffi::c_void,
}

pub fn get_host_num_displays() -> i32 {
    let sdl_handle = get_sdl_handle();
    if !sdl_handle.is_null() {
        type SdlGetNumDisplaysFn = unsafe extern "C" fn() -> i32;
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetNumVideoDisplays")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlGetNumDisplaysFn = unsafe { std::mem::transmute(sym) };
            let count = unsafe { f() };
            if count > 0 {
                return count;
            }
        }
    }
    1
}

pub fn get_host_display_bounds(display_idx: i32) -> SdlRect {
    let sdl_handle = get_sdl_handle();
    if !sdl_handle.is_null() {
        type SdlGetDisplayBoundsFn = unsafe extern "C" fn(i32, *mut SdlRect) -> i32;
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetDisplayBounds")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlGetDisplayBoundsFn = unsafe { std::mem::transmute(sym) };
            let mut rect = SdlRect::default();
            if unsafe { f(display_idx, &mut rect) } == 0 && rect.w > 0 && rect.h > 0 {
                return rect;
            }
        }
    }
    // Fallback: query X11 display directly
    let dpy = get_global_display();
    if !dpy.is_null() {
        let screen = unsafe { XDefaultScreen(dpy) };
        let root = unsafe { XRootWindow(dpy, screen) };
        // Default to 2560x1440 if known host
        SdlRect {
            x: 0,
            y: 0,
            w: 2560,
            h: 1440,
        }
    } else {
        SdlRect {
            x: 0,
            y: 0,
            w: 2560,
            h: 1440,
        }
    }
}

pub fn get_host_display_usable_bounds(display_idx: i32) -> SdlRect {
    let sdl_handle = get_sdl_handle();
    if !sdl_handle.is_null() {
        type SdlGetDisplayUsableBoundsFn = unsafe extern "C" fn(i32, *mut SdlRect) -> i32;
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetDisplayUsableBounds")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlGetDisplayUsableBoundsFn = unsafe { std::mem::transmute(sym) };
            let mut rect = SdlRect::default();
            if unsafe { f(display_idx, &mut rect) } == 0 && rect.w > 0 && rect.h > 0 {
                return rect;
            }
        }
    }
    // Fallback to display bounds with top panel offset (32px)
    let full = get_host_display_bounds(display_idx);
    SdlRect {
        x: full.x,
        y: full.y + 32,
        w: full.w,
        h: full.h - 32,
    }
}

pub fn get_host_display_mode(display_idx: i32) -> SdlDisplayMode {
    let sdl_handle = get_sdl_handle();
    if !sdl_handle.is_null() {
        type SdlGetCurrentDisplayModeFn = unsafe extern "C" fn(i32, *mut SdlDisplayMode) -> i32;
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetCurrentDisplayMode")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlGetCurrentDisplayModeFn = unsafe { std::mem::transmute(sym) };
            let mut mode = SdlDisplayMode::default();
            if unsafe { f(display_idx, &mut mode) } == 0 && mode.w > 0 && mode.h > 0 {
                return mode;
            }
        }
    }
    let bounds = get_host_display_bounds(display_idx);
    SdlDisplayMode {
        format: 0,
        w: bounds.w,
        h: bounds.h,
        refresh_rate: 144,
        driverdata: std::ptr::null_mut(),
    }
}

pub fn sdl_set_window_size(w: i32, h: i32) {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlSetWindowSizeFn = unsafe extern "C" fn(*mut c_void, i32, i32);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_SetWindowSize")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlSetWindowSizeFn = unsafe { std::mem::transmute(sym) };
            unsafe {
                f(sdl_win, w, h);
            }
            println!("[NATIVE WINDOW] SDL_SetWindowSize applied: {}x{}", w, h);
        }
    }
}

pub fn sdl_get_window_size() -> (i32, i32) {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlGetWindowSizeFn = unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetWindowSize")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlGetWindowSizeFn = unsafe { std::mem::transmute(sym) };
            let mut w = 0i32;
            let mut h = 0i32;
            unsafe {
                f(sdl_win, &mut w, &mut h);
            }
            if w > 0 && h > 0 {
                return (w, h);
            }
        }
    }
    (1920, 1080)
}

pub fn sdl_set_window_position(x: i32, y: i32) {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlSetWindowPosFn = unsafe extern "C" fn(*mut c_void, i32, i32);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_SetWindowPosition")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlSetWindowPosFn = unsafe { std::mem::transmute(sym) };
            unsafe {
                f(sdl_win, x, y);
            }
            println!(
                "[NATIVE WINDOW] SDL_SetWindowPosition applied: ({}, {})",
                x, y
            );
        }
    }
}

pub fn sdl_get_window_position() -> (i32, i32) {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlGetWindowPosFn = unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetWindowPosition")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlGetWindowPosFn = unsafe { std::mem::transmute(sym) };
            let mut x = 0i32;
            let mut y = 0i32;
            unsafe {
                f(sdl_win, &mut x, &mut y);
            }
            return (x, y);
        }
    }
    (0, 0)
}

pub fn sdl_maximize_window() {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlMaximizeFn = unsafe extern "C" fn(*mut c_void);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_MaximizeWindow")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlMaximizeFn = unsafe { std::mem::transmute(sym) };
            unsafe {
                f(sdl_win);
            }
            println!("[NATIVE WINDOW] SDL_MaximizeWindow invoked");
        }
    }
}

pub fn sdl_minimize_window() {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlMinimizeFn = unsafe extern "C" fn(*mut c_void);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_MinimizeWindow")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlMinimizeFn = unsafe { std::mem::transmute(sym) };
            unsafe {
                f(sdl_win);
            }
            println!("[NATIVE WINDOW] SDL_MinimizeWindow invoked");
        }
    }
}

pub fn sdl_restore_window() {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlRestoreFn = unsafe extern "C" fn(*mut c_void);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_RestoreWindow")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlRestoreFn = unsafe { std::mem::transmute(sym) };
            unsafe {
                f(sdl_win);
            }
            println!("[NATIVE WINDOW] SDL_RestoreWindow invoked");
        }
    }
}

pub fn sdl_show_window() {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlShowFn = unsafe extern "C" fn(*mut c_void);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_ShowWindow").unwrap().as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlShowFn = unsafe { std::mem::transmute(sym) };
            unsafe {
                f(sdl_win);
            }
        }
    }
}

pub fn sdl_raise_window() {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlRaiseFn = unsafe extern "C" fn(*mut c_void);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_RaiseWindow").unwrap().as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlRaiseFn = unsafe { std::mem::transmute(sym) };
            unsafe {
                f(sdl_win);
            }
        }
    }
}

pub fn sdl_set_window_bordered(bordered: bool) {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlSetBorderedFn = unsafe extern "C" fn(*mut c_void, i32);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_SetWindowBordered")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlSetBorderedFn = unsafe { std::mem::transmute(sym) };
            unsafe {
                f(sdl_win, if bordered { 1 } else { 0 });
            }
            println!("[NATIVE WINDOW] SDL_SetWindowBordered: {}", bordered);
        }
    }
}

pub fn sdl_set_window_resizable(resizable: bool) {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlSetResizableFn = unsafe extern "C" fn(*mut c_void, i32);
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_SetWindowResizable")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlSetResizableFn = unsafe { std::mem::transmute(sym) };
            unsafe {
                f(sdl_win, if resizable { 1 } else { 0 });
            }
            println!("[NATIVE WINDOW] SDL_SetWindowResizable: {}", resizable);
        }
    }
}

pub fn sdl_set_window_fullscreen(flags: u32) {
    let sdl_win = get_global_sdl_window();
    let sdl_handle = get_sdl_handle();
    if !sdl_win.is_null() && !sdl_handle.is_null() {
        type SdlSetFullscreenFn = unsafe extern "C" fn(*mut c_void, u32) -> i32;
        let sym = unsafe {
            libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_SetWindowFullscreen")
                    .unwrap()
                    .as_ptr(),
            )
        };
        if !sym.is_null() {
            let f: SdlSetFullscreenFn = unsafe { std::mem::transmute(sym) };
            let res = unsafe { f(sdl_win, flags) };
            println!(
                "[NATIVE WINDOW] SDL_SetWindowFullscreen(flags=0x{:X}) -> {}",
                flags, res
            );
        }
    }
}
