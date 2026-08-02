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
}

pub static GLOBAL_DISPLAY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static GLOBAL_WINDOW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
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

/// Initialises SDL2 video subsystem and returns (sdl_handle, window_ptr).
/// The window is shown on the desktop with the given title and size.
pub unsafe fn init_sdl2_and_create_window(
    title: &str,
    width: u32,
    height: u32,
) -> (*mut c_void, *mut c_void) {
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

    fn_init(0x00000020); // SDL_INIT_VIDEO

    let title_c = CString::new(title).unwrap_or_else(|_| CString::new("Rine").unwrap());
    let window_ptr = fn_create_window(
        title_c.as_ptr(),
        0x2FFF0000, // SDL_WINDOWPOS_CENTERED
        0x2FFF0000,
        width as i32,
        height as i32,
        0x00000004 | 0x00000020, // SDL_WINDOW_SHOWN | SDL_WINDOW_RESIZABLE
    );

    if !window_ptr.is_null() {
        GLOBAL_WINDOW.store(window_ptr as u64, std::sync::atomic::Ordering::SeqCst);
        println!(
            "[NATIVE DESKTOP WINDOW] SDL2 window created: {:p}, Title='{}', Size={}x{}",
            window_ptr, title, width, height
        );
        let x11_id = get_x11_window_id(window_ptr);
        if x11_id != 0 {
            GLOBAL_WINDOW.store(x11_id, std::sync::atomic::Ordering::SeqCst);
        }
        create_sdl2_renderer(sdl_handle, window_ptr);
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
    info.version[0] = 2;
    info.version[1] = 0;
    info.version[2] = 0;
    if get_wm_info(window_ptr, &mut info) != 0 {
        let x11_dpy_ptr = (&info.info[0]) as *const u8 as *const Display;
        let x11_win_ptr = (&info.info[8]) as *const u8 as *const u64;
        let dpy = *x11_dpy_ptr;
        let x11_id = *x11_win_ptr;

        if !dpy.is_null() {
            GLOBAL_DISPLAY.store(dpy as u64, std::sync::atomic::Ordering::SeqCst);
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
        }
        return x11_id;
    }
    0
}

/// Opens an ACTUAL physical window on the user's Linux desktop screen via SDL2 (supporting Wayland and X11).
/// Called from main.rs at startup with the EXE name as title.
pub unsafe fn create_physical_x11_window(title: &str, width: u32, height: u32) -> u64 {
    let (_, window_ptr) = init_sdl2_and_create_window(title, width, height);
    if !window_ptr.is_null() {
        return window_ptr as u64;
    }

    // X11 fallback when SDL2 is unavailable
    if std::env::var_os("DISPLAY").is_none() {
        println!(
            "[X11 WINDOWING] Skipping X11 window creation: DISPLAY environment variable not set."
        );
        return 0;
    }

    let display_name = CString::new("").unwrap();
    let display = XOpenDisplay(display_name.as_ptr());

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
