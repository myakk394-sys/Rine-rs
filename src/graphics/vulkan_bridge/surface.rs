//! Vulkan Surface Creation Bridge (Win32 HWND -> Native X11 Surface)

use super::handles::WindowRegistry;
use super::loader::get_native_proc_addr;
use super::types::*;
use std::ffi::c_void;

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

    let rine_win = match WindowRegistry::global().lookup(hwnd) {
        Some(w) => w,
        None => {
            println!(
                "[VULKAN BRIDGE] ERROR: No registered RineWindow found for HWND=0x{:X}",
                hwnd
            );
            return VK_ERROR_NATIVE_WINDOW_IN_USE_KHR;
        }
    };

    println!(
        "[VULKAN BRIDGE] Resolved HWND=0x{:X} -> Native Display={:p}, Window=0x{:X}, VisualID=0x{:X}",
        hwnd, rine_win.native_display, rine_win.native_window, rine_win.native_visual_id
    );

    let xlib_create_info = VkXlibSurfaceCreateInfoKHR {
        sType: VK_STRUCTURE_TYPE_XLIB_SURFACE_CREATE_INFO_KHR,
        pNext: std::ptr::null(),
        flags: 0,
        dpy: rine_win.native_display,
        window: rine_win.native_window,
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
    res
}

pub unsafe extern "win64" fn rine_vkGetPhysicalDeviceWin32PresentationSupportKHR(
    physical_device: NativeVkPhysicalDevice,
    queue_family_index: u32,
) -> u32 {
    let sym = match get_native_proc_addr("vkGetPhysicalDeviceXlibPresentationSupportKHR") {
        Some(s) => s,
        None => return 0,
    };

    let display = crate::windowing::x11::GLOBAL_DISPLAY.load(std::sync::atomic::Ordering::SeqCst)
        as *mut c_void;
    let visual_id = crate::windowing::x11::get_global_visual_id();

    if display.is_null() || visual_id == 0 {
        return 0;
    }

    type VkGetXlibPresentationSupportFn =
        unsafe extern "C" fn(NativeVkPhysicalDevice, u32, *mut c_void, u64) -> u32;

    let check_xlib_support: VkGetXlibPresentationSupportFn = std::mem::transmute(sym);
    check_xlib_support(physical_device, queue_family_index, display, visual_id)
}
