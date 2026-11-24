//! Native Vulkan Dynamic Loader Manager

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CString};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{OnceLock, RwLock};

static NATIVE_VK_HANDLE: OnceLock<usize> = OnceLock::new();
static NATIVE_INSTANCE: AtomicUsize = AtomicUsize::new(0);
static NATIVE_DEVICE: AtomicUsize = AtomicUsize::new(0);
static PROC_CACHE: OnceLock<RwLock<HashMap<String, usize>>> = OnceLock::new();

type VkGetInstanceProcAddrFn = unsafe extern "C" fn(usize, *const c_char) -> *const c_void;
type VkGetDeviceProcAddrFn = unsafe extern "C" fn(usize, *const c_char) -> *const c_void;

static NATIVE_GIPA: OnceLock<Option<VkGetInstanceProcAddrFn>> = OnceLock::new();
static NATIVE_GDPA: OnceLock<Option<VkGetDeviceProcAddrFn>> = OnceLock::new();

pub fn set_native_instance(instance: usize) {
    NATIVE_INSTANCE.store(instance, Ordering::Release);
}

pub fn set_native_device(device: usize) {
    NATIVE_DEVICE.store(device, Ordering::Release);
}

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

fn get_native_gipa() -> Option<VkGetInstanceProcAddrFn> {
    *NATIVE_GIPA.get_or_init(|| unsafe {
        let handle = get_native_vulkan_handle();
        if handle == 0 {
            return None;
        }
        let sym = libc::dlsym(
            handle as *mut c_void,
            b"vkGetInstanceProcAddr\0".as_ptr() as *const c_char,
        );
        if sym.is_null() {
            None
        } else {
            Some(std::mem::transmute(sym))
        }
    })
}

fn get_native_gdpa() -> Option<VkGetDeviceProcAddrFn> {
    *NATIVE_GDPA.get_or_init(|| unsafe {
        let handle = get_native_vulkan_handle();
        if handle == 0 {
            return None;
        }
        let sym = libc::dlsym(
            handle as *mut c_void,
            b"vkGetDeviceProcAddr\0".as_ptr() as *const c_char,
        );
        if sym.is_null() {
            None
        } else {
            Some(std::mem::transmute(sym))
        }
    })
}

pub unsafe fn get_native_proc_addr(fn_name: &str) -> Option<usize> {
    let cache_lock = PROC_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    if let Ok(cache) = cache_lock.read() {
        if let Some(&ptr) = cache.get(fn_name) {
            return Some(ptr);
        }
    }

    let handle = get_native_vulkan_handle();
    if handle == 0 {
        return None;
    }
    let c_name = CString::new(fn_name).ok()?;

    // 1. Try libc::dlsym first
    let sym = libc::dlsym(handle as *mut c_void, c_name.as_ptr());
    let mut resolved = if !sym.is_null() {
        sym as usize
    } else {
        0usize
    };

    // 2. Try native vkGetDeviceProcAddr if device is set
    if resolved == 0 {
        let dev = NATIVE_DEVICE.load(Ordering::Acquire);
        if dev != 0 {
            if let Some(gdpa) = get_native_gdpa() {
                let p = gdpa(dev, c_name.as_ptr());
                if !p.is_null() {
                    resolved = p as usize;
                }
            }
        }
    }

    // 3. Try native vkGetInstanceProcAddr if instance is set
    if resolved == 0 {
        let inst = NATIVE_INSTANCE.load(Ordering::Acquire);
        if inst != 0 {
            if let Some(gipa) = get_native_gipa() {
                let p = gipa(inst, c_name.as_ptr());
                if !p.is_null() {
                    resolved = p as usize;
                }
            }
        }
    }

    if resolved != 0 {
        if let Ok(mut cache) = cache_lock.write() {
            cache.insert(fn_name.to_string(), resolved);
        }
        Some(resolved)
    } else {
        None
    }
}
