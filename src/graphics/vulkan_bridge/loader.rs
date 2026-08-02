//! Native Vulkan Dynamic Loader Manager

use std::ffi::{c_char, c_void, CString};
use std::sync::OnceLock;

static NATIVE_VK_HANDLE: OnceLock<usize> = OnceLock::new();

pub fn get_native_vulkan_handle() -> usize {
    *NATIVE_VK_HANDLE.get_or_init(|| unsafe {
        let names = [
            b"libvulkan.so.1\0".as_ptr() as *const c_char,
            b"libvulkan.so\0".as_ptr() as *const c_char,
        ];
        for name in names {
            let h = libc::dlopen(name, libc::RTLD_NOW | libc::RTLD_GLOBAL);
            if !h.is_null() {
                println!("[VULKAN LOADER] Successfully loaded native libvulkan.so.1!");
                return h as usize;
            }
        }
        println!("[VULKAN LOADER] ERROR: Native libvulkan.so.1 not found on system!");
        0usize
    })
}

pub unsafe fn get_native_proc_addr(fn_name: &str) -> Option<usize> {
    let handle = get_native_vulkan_handle();
    if handle == 0 {
        return None;
    }
    let c_name = CString::new(fn_name).ok()?;
    let sym = libc::dlsym(handle as *mut c_void, c_name.as_ptr());
    if sym.is_null() {
        None
    } else {
        Some(sym as usize)
    }
}
