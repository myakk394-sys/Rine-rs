//! Vulkan Extension Virtualization & Translation
//! Handles 2-stage vkEnumerateInstanceExtensionProperties and converts VK_KHR_win32_surface -> VK_KHR_xlib_surface for vkCreateInstance.
//!
//! Vulkan spec compliance:
//! - When pProperties is NULL: *pPropertyCount = total available count. Return VK_SUCCESS.
//! - When pProperties is non-NULL: copy min(capacity, total) entries. Set *pPropertyCount = copied count.
//!   Return VK_INCOMPLETE if capacity < total, VK_SUCCESS otherwise.

#![allow(non_snake_case)]

use super::loader::get_native_proc_addr;
use super::types::*;
use std::ffi::{c_char, c_void, CStr, CString};

/// specVersion for VK_KHR_win32_surface as defined by Vulkan headers (VK_KHR_WIN32_SURFACE_SPEC_VERSION = 6)
const VK_KHR_WIN32_SURFACE_SPEC_VERSION: u32 = 6;

pub unsafe extern "win64" fn rine_vkEnumerateInstanceExtensionProperties(
    p_layer_name: *const c_char,
    p_property_count: *mut u32,
    p_properties: *mut VkExtensionProperties,
) -> i32 {
    let sym = match get_native_proc_addr("vkEnumerateInstanceExtensionProperties") {
        Some(s) => s,
        None => return VK_ERROR_INITIALIZATION_FAILED,
    };

    type VkEnumerateFn =
        unsafe extern "C" fn(*const c_char, *mut u32, *mut VkExtensionProperties) -> i32;

    let native_enumerate: VkEnumerateFn = std::mem::transmute(sym);

    // Step 1: Query native extension count
    let mut native_count: u32 = 0;
    let res = native_enumerate(p_layer_name, &mut native_count, std::ptr::null_mut());
    if res != VK_SUCCESS {
        return res;
    }

    // Check if host has VK_KHR_xlib_surface (prerequisite for our virtual VK_KHR_win32_surface)
    let has_host_wsi = {
        let mut temp: Vec<VkExtensionProperties> = vec![
            VkExtensionProperties {
                extensionName: [0; VK_MAX_EXTENSION_NAME_SIZE],
                specVersion: 0,
            };
            native_count as usize
        ];
        let mut tmp_count = native_count;
        let r = native_enumerate(p_layer_name, &mut tmp_count, temp.as_mut_ptr());
        if r == VK_SUCCESS || r == VK_INCOMPLETE {
            temp.iter().any(|ext| {
                let name = CStr::from_ptr(ext.extensionName.as_ptr());
                name.to_bytes() == b"VK_KHR_xlib_surface"
            })
        } else {
            false
        }
    };

    // Only add virtual VK_KHR_win32_surface if host has a working WSI backend
    let virtual_ext_count: u32 = if has_host_wsi { 1 } else { 0 };
    let total_count = native_count + virtual_ext_count;

    // Count-only query: pProperties is NULL
    if p_properties.is_null() {
        if !p_property_count.is_null() {
            *p_property_count = total_count;
        }
        return VK_SUCCESS;
    }

    // Fill query: copy extensions into caller buffer
    let capacity = if !p_property_count.is_null() {
        *p_property_count as usize
    } else {
        return VK_ERROR_INITIALIZATION_FAILED;
    };

    // Step 2: Get native extensions into temp buffer
    let mut temp_props: Vec<VkExtensionProperties> = vec![
        VkExtensionProperties {
            extensionName: [0; VK_MAX_EXTENSION_NAME_SIZE],
            specVersion: 0,
        };
        native_count as usize
    ];
    let mut fetch_count = native_count;
    let res2 = native_enumerate(p_layer_name, &mut fetch_count, temp_props.as_mut_ptr());
    if res2 != VK_SUCCESS && res2 != VK_INCOMPLETE {
        return res2;
    }
    temp_props.truncate(fetch_count as usize);

    // Remove native VK_KHR_xlib_surface / VK_KHR_xcb_surface from guest-visible list
    // (DXVK doesn't know about these; it expects VK_KHR_win32_surface)
    // But keep them since native code still needs them -- actually, DXVK only checks
    // for VK_KHR_win32_surface, so removing native WSI extensions from the guest view is optional.
    // For now, expose all native extensions plus our virtual one.

    // Append virtual VK_KHR_win32_surface if host WSI is available
    if has_host_wsi {
        let win32_ext_name = b"VK_KHR_win32_surface\0";
        let mut win32_entry = VkExtensionProperties {
            extensionName: [0; VK_MAX_EXTENSION_NAME_SIZE],
            specVersion: VK_KHR_WIN32_SURFACE_SPEC_VERSION,
        };
        std::ptr::copy_nonoverlapping(
            win32_ext_name.as_ptr() as *const c_char,
            win32_entry.extensionName.as_mut_ptr(),
            win32_ext_name.len(),
        );

        // Check for duplicates (shouldn't happen, but belt-and-suspenders)
        let already_exists = temp_props.iter().any(|ext| {
            let name = CStr::from_ptr(ext.extensionName.as_ptr());
            name.to_bytes() == b"VK_KHR_win32_surface"
        });
        if !already_exists {
            temp_props.push(win32_entry);
        }
    }

    let actual_total = temp_props.len();
    let copy_count = std::cmp::min(capacity, actual_total);

    for i in 0..copy_count {
        *p_properties.add(i) = temp_props[i];
    }

    // Vulkan spec: pPropertyCount is set to the number of entries actually written
    *p_property_count = copy_count as u32;

    if capacity < actual_total {
        VK_INCOMPLETE
    } else {
        VK_SUCCESS
    }
}

pub unsafe extern "win64" fn rine_vkCreateInstance(
    p_create_info: *const VkInstanceCreateInfo,
    _p_allocator: *const c_void,
    p_instance: *mut NativeVkInstance,
) -> i32 {
    let sym = match get_native_proc_addr("vkCreateInstance") {
        Some(s) => s,
        None => return VK_ERROR_INITIALIZATION_FAILED,
    };

    if p_create_info.is_null() || p_instance.is_null() {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    let info = &*p_create_info;
    let mut transformed_exts: Vec<CString> = Vec::new();
    let mut has_khr_surface = false;
    let mut has_xlib_surface = false;

    if info.enabledExtensionCount > 0 && !info.ppEnabledExtensionNames.is_null() {
        for i in 0..info.enabledExtensionCount as usize {
            let ext_ptr = *info.ppEnabledExtensionNames.add(i);
            if ext_ptr.is_null() {
                continue;
            }
            let c_str = CStr::from_ptr(ext_ptr);
            let ext_name = c_str.to_bytes();
            if ext_name == b"VK_KHR_win32_surface" {
                // Replace VK_KHR_win32_surface with native VK_KHR_xlib_surface
                if !has_xlib_surface {
                    transformed_exts.push(CString::new("VK_KHR_xlib_surface").unwrap());
                    has_xlib_surface = true;
                }
            } else {
                let ext_cstring = CString::from(c_str);
                // Track what we have
                if ext_name == b"VK_KHR_surface" {
                    has_khr_surface = true;
                }
                if ext_name == b"VK_KHR_xlib_surface" {
                    has_xlib_surface = true;
                }
                // Avoid adding duplicates
                if !transformed_exts
                    .iter()
                    .any(|s| s.as_c_str() == ext_cstring.as_c_str())
                {
                    transformed_exts.push(ext_cstring);
                }
            }
        }
    }

    // Always ensure VK_KHR_surface is present (required by VK_KHR_xlib_surface)
    if !has_khr_surface {
        transformed_exts.push(CString::new("VK_KHR_surface").unwrap());
    }

    // Ensure VK_KHR_xlib_surface is present if win32_surface was requested
    if !has_xlib_surface {
        transformed_exts.push(CString::new("VK_KHR_xlib_surface").unwrap());
    }

    let ext_ptrs: Vec<*const c_char> = transformed_exts.iter().map(|s| s.as_ptr()).collect();

    let new_create_info = VkInstanceCreateInfo {
        sType: info.sType,
        pNext: info.pNext,
        flags: info.flags,
        pApplicationInfo: info.pApplicationInfo,
        enabledLayerCount: info.enabledLayerCount,
        ppEnabledLayerNames: info.ppEnabledLayerNames,
        enabledExtensionCount: ext_ptrs.len() as u32,
        ppEnabledExtensionNames: ext_ptrs.as_ptr(),
    };

    type VkCreateInstanceFn = unsafe extern "C" fn(
        *const VkInstanceCreateInfo,
        *const c_void,
        *mut NativeVkInstance,
    ) -> i32;

    let native_create_instance: VkCreateInstanceFn = std::mem::transmute(sym);

    // Pass NULL for p_allocator to avoid cross-ABI callback corruption
    let res = native_create_instance(&new_create_info, std::ptr::null(), p_instance);

    // Log extension mapping for diagnostics
    if res == VK_SUCCESS {
        println!(
            "[VULKAN BRIDGE] vkCreateInstance SUCCESS with {} extensions (Instance={:p})",
            ext_ptrs.len(),
            *p_instance
        );
        for ext in &transformed_exts {
            println!("  -> Extension: {}", ext.to_string_lossy());
        }
    } else {
        println!(
            "[VULKAN BRIDGE] vkCreateInstance FAILED -> VkResult={}",
            res
        );
    }

    res
}
