//! Vulkan Surface Creation Bridge (Win32 HWND -> Native X11 Surface)

use super::handles::WindowRegistry;
use super::loader::get_native_proc_addr;
use super::types::*;
use std::ffi::c_void;

pub static LAST_SURFACE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[no_mangle]
pub unsafe extern "win64" fn rine_vkCreateWin32SurfaceKHR(
    instance: NativeVkInstance,
    p_create_info: *const VkWin32SurfaceCreateInfoKHR,
    _p_allocator: *const c_void,
    p_surface: *mut NativeVkSurfaceKHR,
) -> i32 {
    println!("[VULKAN BRIDGE] Intercepted vkCreateWin32SurfaceKHR!");

    if p_create_info.is_null() || p_surface.is_null() {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    let info = &*p_create_info;
    let hwnd = info.hwnd;

    let (mut dpy, mut win, mut visual_id) = match WindowRegistry::global().lookup(hwnd) {
        Some(w) => {
            println!(
                "[VULKAN BRIDGE] Resolved HWND=0x{:X} -> Native Display={:p}, Window=0x{:X}, VisualID=0x{:X}",
                hwnd, w.native_display, w.native_window, w.native_visual_id
            );
            (w.native_display, w.native_window, w.native_visual_id)
        }
        None => {
            println!(
                "[VULKAN BRIDGE] HWND=0x{:X} not directly registered, activating fallback path",
                hwnd
            );
            (std::ptr::null_mut(), 0u64, 0u64)
        }
    };

    if win == 0 {
        let global_win = crate::windowing::x11::GLOBAL_WINDOW.load(std::sync::atomic::Ordering::SeqCst);
        if global_win != 0 {
            win = global_win;
        }
    }
    if dpy.is_null() {
        let global_dpy = crate::windowing::x11::GLOBAL_DISPLAY.load(std::sync::atomic::Ordering::SeqCst)
            as *mut c_void;
        if !global_dpy.is_null() {
            dpy = global_dpy;
        }
    }
    if visual_id == 0 {
        visual_id = crate::windowing::x11::get_global_visual_id();
    }

    if dpy.is_null() || win == 0 {
        crate::windowing::x11::ensure_x11_environment();
        if dpy.is_null() {
            dpy = crate::windowing::x11::XOpenDisplay(std::ptr::null());
            if !dpy.is_null() {
                crate::windowing::x11::GLOBAL_DISPLAY
                    .store(dpy as u64, std::sync::atomic::Ordering::SeqCst);
            }
        }
        if !dpy.is_null() && win == 0 {
            let screen = crate::windowing::x11::XDefaultScreen(dpy);
            let root = crate::windowing::x11::XRootWindow(dpy, screen);
            win = crate::windowing::x11::XCreateSimpleWindow(dpy, root, 0, 0, 1920, 1080, 0, 0, 0);
            crate::windowing::x11::GLOBAL_WINDOW
                .store(win as u64, std::sync::atomic::Ordering::SeqCst);
            println!(
                "[VULKAN BRIDGE] On-demand X11 Window created: Display={:p}, Window=0x{:X}",
                dpy, win
            );
        }
    }

    if dpy.is_null() || win == 0 {
        println!(
            "[VULKAN BRIDGE] ERROR: Unable to resolve or create native X11 window/display for HWND=0x{:X}",
            hwnd
        );
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    // Auto-register in WindowRegistry so future lookups succeed immediately
    WindowRegistry::global().register_window(
        hwnd,
        super::handles::RineWindow {
            guest_hwnd: hwnd,
            backend: super::handles::WindowBackendType::X11,
            native_display: dpy,
            native_window: win,
            native_visual_id: visual_id,
            width: 1920,
            height: 1080,
            visible: true,
        },
    );
    let hwnd_u32 = (hwnd & 0xFFFF_FFFF) as usize;
    if hwnd_u32 != hwnd && hwnd_u32 != 0 {
        WindowRegistry::global().register_window(
            hwnd_u32,
            super::handles::RineWindow {
                guest_hwnd: hwnd_u32,
                backend: super::handles::WindowBackendType::X11,
                native_display: dpy,
                native_window: win,
                native_visual_id: visual_id,
                width: 1920,
                height: 1080,
                visible: true,
            },
        );
    }

    let xlib_create_info = VkXlibSurfaceCreateInfoKHR {
        sType: VK_STRUCTURE_TYPE_XLIB_SURFACE_CREATE_INFO_KHR,
        pNext: std::ptr::null(),
        flags: 0,
        dpy,
        window: win,
    };

    // Query vkCreateXlibSurfaceKHR through native vkGetInstanceProcAddr as required by Vulkan spec
    let sym = match super::proc_addr::get_native_instance_proc_addr(
        instance,
        "vkCreateXlibSurfaceKHR",
    ) {
        Some(s) => s,
        None => {
            match get_native_proc_addr("vkCreateXlibSurfaceKHR") {
                Some(s) => s,
                None => {
                    println!("[VULKAN BRIDGE] ERROR: vkCreateXlibSurfaceKHR missing from libvulkan.so.1!");
                    return VK_ERROR_EXTENSION_NOT_PRESENT;
                }
            }
        }
    };

    type VkCreateXlibSurfaceKHRFn = unsafe extern "C" fn(
        NativeVkInstance,
        *const VkXlibSurfaceCreateInfoKHR,
        *const c_void,
        *mut NativeVkSurfaceKHR,
    ) -> i32;

    let create_xlib: VkCreateXlibSurfaceKHRFn = std::mem::transmute(sym);

    // Pass NULL for p_allocator to prevent cross-ABI callback corruption
    let res = create_xlib(instance, &xlib_create_info, std::ptr::null(), p_surface);
    println!(
        "[VULKAN BRIDGE] Created Native X11 Vulkan Surface=0x{:X} -> VkResult={}",
        *p_surface, res
    );
    if res == 0 && !p_surface.is_null() {
        LAST_SURFACE.store(*p_surface, std::sync::atomic::Ordering::SeqCst);
        let s = *p_surface;
        for i in 0..24 {
            let val = *((s + i * 8) as *const u64);
            println!("  [SURFACE MEM] +0x{:02X}: 0x{:016X}", i * 8, val);
        }
    }
    use std::io::Write;
    std::io::stdout().flush().ok();
    res
}

pub unsafe extern "win64" fn rine_vkGetPhysicalDeviceWin32PresentationSupportKHR(
    physical_device: NativeVkPhysicalDevice,
    queue_family_index: u32,
) -> u32 {
    let sym = get_native_proc_addr("vkGetPhysicalDeviceXlibPresentationSupportKHR");
    if let Some(s) = sym {
        let display = crate::windowing::x11::GLOBAL_DISPLAY
            .load(std::sync::atomic::Ordering::SeqCst) as *mut c_void;
        let visual_id = crate::windowing::x11::get_global_visual_id();
        if !display.is_null() && visual_id != 0 {
            type VkGetXlibPresentationSupportFn =
                unsafe extern "C" fn(NativeVkPhysicalDevice, u32, *mut c_void, u64) -> u32;
            let check_xlib_support: VkGetXlibPresentationSupportFn = std::mem::transmute(s);
            let supp = check_xlib_support(physical_device, queue_family_index, display, visual_id);
            if supp != 0 {
                println!(
                    "[VK PRESENT SUPPORT] Win32PresentationSupport for queue_family={} -> 1 (native Xlib)",
                    queue_family_index
                );
                return 1;
            }
        }
    }
    println!(
        "[VK PRESENT SUPPORT] Win32PresentationSupport for queue_family={} -> 1 (guaranteed)",
        queue_family_index
    );
    1 // VK_TRUE
}
