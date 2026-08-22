//! Window Handle Registry
//! Thread-safe registry mapping Guest HWND -> Native Window (X11 Display* + Window ID).

use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowBackendType {
    X11,
    Wayland,
    Headless,
}

#[derive(Clone)]
pub struct RineWindow {
    pub guest_hwnd: usize,
    pub backend: WindowBackendType,
    pub native_display: *mut std::ffi::c_void,
    pub native_window: u64,
    pub native_visual_id: u64,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
}

unsafe impl Send for RineWindow {}
unsafe impl Sync for RineWindow {}

pub struct WindowRegistry {
    windows: RwLock<HashMap<usize, RineWindow>>,
}

impl WindowRegistry {
    pub fn new() -> Self {
        Self {
            windows: RwLock::new(HashMap::new()),
        }
    }

    pub fn global() -> &'static WindowRegistry {
        static INSTANCE: std::sync::OnceLock<WindowRegistry> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(WindowRegistry::new)
    }

    pub fn register_window(&self, hwnd: usize, window: RineWindow) {
        let mut lock = self.windows.write().unwrap();
        println!(
            "[WINDOW REGISTRY] Registered HWND=0x{:X} -> NativeWindow=0x{:X}, Display={:p}, VisualID=0x{:X}",
            hwnd, window.native_window, window.native_display, window.native_visual_id
        );
        lock.insert(hwnd, window);
    }

    pub fn lookup(&self, hwnd: usize) -> Option<RineWindow> {
        let lock = self.windows.read().unwrap();
        if let Some(win) = lock.get(&hwnd) {
            return Some(win.clone());
        }
        let global_window =
            crate::windowing::x11::GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst);
        let global_display = crate::windowing::x11::GLOBAL_DISPLAY
            .load(std::sync::atomic::Ordering::SeqCst)
            as *mut std::ffi::c_void;
        let global_visual_id = crate::windowing::x11::get_global_visual_id();

        if global_window != 0 && !global_display.is_null() {
            Some(RineWindow {
                guest_hwnd: hwnd,
                backend: WindowBackendType::X11,
                native_display: global_display,
                native_window: global_window,
                native_visual_id: global_visual_id,
                width: 1280,
                height: 720,
                visible: true,
            })
        } else if global_window != 0 {
            let dpy = unsafe { crate::windowing::x11::XOpenDisplay(std::ptr::null()) };
            if !dpy.is_null() {
                crate::windowing::x11::GLOBAL_DISPLAY
                    .store(dpy as u64, std::sync::atomic::Ordering::SeqCst);
                Some(RineWindow {
                    guest_hwnd: hwnd,
                    backend: WindowBackendType::X11,
                    native_display: dpy,
                    native_window: global_window,
                    native_visual_id: 0,
                    width: 1280,
                    height: 720,
                    visible: true,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}
