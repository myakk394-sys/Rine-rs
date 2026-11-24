//! Vulkan Command ABI Wrappers (Win64 ABI ↔ System V ABI Thunks)
//! Provides exact typed wrappers for Vulkan functions so that MSVC PE64 DXVK code
//! calling via Win64 ABI translates seamlessly into native System V AMD64 Linux Vulkan Loader calls.
//!
//! POLICY: All `pAllocator` parameters from guest PE64 are replaced with NULL
//! to prevent cross-ABI callback corruption (Win64 function pointers in VkAllocationCallbacks
//! would be invoked by native Linux driver using System V calling convention).

#![allow(non_snake_case)]

use super::loader::get_native_proc_addr;
use super::types::*;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Helper macros
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Resolve native proc address by name and invoke via System V ABI.
/// Returns VK_ERROR_INITIALIZATION_FAILED if symbol is missing.
macro_rules! invoke_native {
    ($fn_name:expr, $fn_type:ty, $($arg:expr),*) => {{
        if let Some(sym) = get_native_proc_addr($fn_name) {
            let native_fn: $fn_type = std::mem::transmute(sym);
            native_fn($($arg),*)
        } else {
            VK_ERROR_INITIALIZATION_FAILED
        }
    }};
}

/// Resolve native proc address and invoke a void-returning function.
macro_rules! invoke_native_void {
    ($fn_name:expr, $fn_type:ty, $($arg:expr),*) => {{
        if let Some(sym) = get_native_proc_addr($fn_name) {
            let native_fn: $fn_type = std::mem::transmute(sym);
            native_fn($($arg),*);
        }
    }};
}

/// Log a Vulkan call on first occurrence only.
macro_rules! log_first {
    ($flag:expr, $($arg:tt)*) => {
        if !$flag.swap(true, Ordering::Relaxed) {
            println!($($arg)*);
        }
    };
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 1: Global Commands
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkEnumerateInstanceVersion(p_api_version: *mut u32) -> i32 {
    type NativeFn = unsafe extern "C" fn(*mut u32) -> i32;
    invoke_native!("vkEnumerateInstanceVersion", NativeFn, p_api_version)
}

pub unsafe extern "win64" fn guest_vkEnumerateInstanceLayerProperties(
    p_property_count: *mut u32,
    p_properties: *mut c_void,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(*mut u32, *mut c_void) -> i32;
    invoke_native!(
        "vkEnumerateInstanceLayerProperties",
        NativeFn,
        p_property_count,
        p_properties
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 2: Instance Lifecycle
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkDestroyInstance(
    instance: NativeVkInstance,
    _p_allocator: *const c_void,
) {
    // NULL allocator: guest pAllocator is Win64 ABI, cannot be passed to native driver
    invoke_native_void!(
        "vkDestroyInstance",
        unsafe extern "C" fn(NativeVkInstance, *const c_void),
        instance,
        std::ptr::null()
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 3: Physical Device Enumeration & Properties
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

static LOG_ENUM_PHYS_DEVS: AtomicBool = AtomicBool::new(false);

pub unsafe extern "win64" fn guest_vkEnumeratePhysicalDevices(
    instance: NativeVkInstance,
    p_physical_device_count: *mut u32,
    p_physical_devices: *mut NativeVkPhysicalDevice,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkInstance, *mut u32, *mut NativeVkPhysicalDevice) -> i32;
    let res = invoke_native!(
        "vkEnumeratePhysicalDevices",
        NativeFn,
        instance,
        p_physical_device_count,
        p_physical_devices
    );
    log_first!(
        LOG_ENUM_PHYS_DEVS,
        "[VULKAN BRIDGE] vkEnumeratePhysicalDevices -> Result={}, Count={}",
        res,
        if !p_physical_device_count.is_null() {
            *p_physical_device_count
        } else {
            0
        }
    );
    res
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceProperties(
    physical_device: NativeVkPhysicalDevice,
    p_properties: *mut c_void,
) {
    invoke_native_void!(
        "vkGetPhysicalDeviceProperties",
        unsafe extern "C" fn(NativeVkPhysicalDevice, *mut c_void),
        physical_device,
        p_properties
    );
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceProperties2(
    physical_device: NativeVkPhysicalDevice,
    p_properties: *mut c_void,
) {
    // Try vkGetPhysicalDeviceProperties2 first, then KHR variant
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceProperties2") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, p_properties);
    } else if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceProperties2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, p_properties);
    }
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceFeatures(
    physical_device: NativeVkPhysicalDevice,
    p_features: *mut c_void,
) {
    invoke_native_void!(
        "vkGetPhysicalDeviceFeatures",
        unsafe extern "C" fn(NativeVkPhysicalDevice, *mut c_void),
        physical_device,
        p_features
    );
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceFeatures2(
    physical_device: NativeVkPhysicalDevice,
    p_features: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceFeatures2") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, p_features);
    } else if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceFeatures2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, p_features);
    }
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceMemoryProperties(
    physical_device: NativeVkPhysicalDevice,
    p_memory_properties: *mut c_void,
) {
    invoke_native_void!(
        "vkGetPhysicalDeviceMemoryProperties",
        unsafe extern "C" fn(NativeVkPhysicalDevice, *mut c_void),
        physical_device,
        p_memory_properties
    );
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceMemoryProperties2(
    physical_device: NativeVkPhysicalDevice,
    p_memory_properties: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceMemoryProperties2") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, p_memory_properties);
    } else if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceMemoryProperties2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, p_memory_properties);
    }
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceQueueFamilyProperties(
    physical_device: NativeVkPhysicalDevice,
    p_count: *mut u32,
    p_queue_family_properties: *mut c_void,
) {
    invoke_native_void!(
        "vkGetPhysicalDeviceQueueFamilyProperties",
        unsafe extern "C" fn(NativeVkPhysicalDevice, *mut u32, *mut c_void),
        physical_device,
        p_count,
        p_queue_family_properties
    );
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceQueueFamilyProperties2(
    physical_device: NativeVkPhysicalDevice,
    p_count: *mut u32,
    p_queue_family_properties: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceQueueFamilyProperties2") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, *mut u32, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, p_count, p_queue_family_properties);
    } else if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceQueueFamilyProperties2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, *mut u32, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, p_count, p_queue_family_properties);
    }
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceFormatProperties(
    physical_device: NativeVkPhysicalDevice,
    format: u32, // VkFormat is i32 in some defs, u32 in others; Vulkan spec uses int32_t enum
    p_format_properties: *mut c_void,
) {
    invoke_native_void!(
        "vkGetPhysicalDeviceFormatProperties",
        unsafe extern "C" fn(NativeVkPhysicalDevice, u32, *mut c_void),
        physical_device,
        format,
        p_format_properties
    );
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceFormatProperties2(
    physical_device: NativeVkPhysicalDevice,
    format: u32,
    p_format_properties: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceFormatProperties2") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, u32, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, format, p_format_properties);
    } else if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceFormatProperties2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, u32, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(physical_device, format, p_format_properties);
    }
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceImageFormatProperties(
    physical_device: NativeVkPhysicalDevice,
    format: u32,
    img_type: u32,
    tiling: u32,
    usage: u32,
    flags: u32,
    p_image_format_properties: *mut c_void,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkPhysicalDevice, u32, u32, u32, u32, u32, *mut c_void) -> i32;
    invoke_native!(
        "vkGetPhysicalDeviceImageFormatProperties",
        NativeFn,
        physical_device,
        format,
        img_type,
        tiling,
        usage,
        flags,
        p_image_format_properties
    )
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceImageFormatProperties2(
    physical_device: NativeVkPhysicalDevice,
    p_image_format_info: *const c_void,
    p_image_format_properties: *mut c_void,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceImageFormatProperties2") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkPhysicalDevice, *const c_void, *mut c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(
            physical_device,
            p_image_format_info,
            p_image_format_properties,
        )
    } else if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceImageFormatProperties2KHR") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkPhysicalDevice, *const c_void, *mut c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(
            physical_device,
            p_image_format_info,
            p_image_format_properties,
        )
    } else {
        VK_ERROR_INITIALIZATION_FAILED
    }
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceSparseImageFormatProperties(
    physical_device: NativeVkPhysicalDevice,
    format: u32,
    img_type: u32,
    samples: u32,
    usage: u32,
    tiling: u32,
    p_property_count: *mut u32,
    p_properties: *mut c_void,
) {
    invoke_native_void!(
        "vkGetPhysicalDeviceSparseImageFormatProperties",
        unsafe extern "C" fn(
            NativeVkPhysicalDevice,
            u32,
            u32,
            u32,
            u32,
            u32,
            *mut u32,
            *mut c_void,
        ),
        physical_device,
        format,
        img_type,
        samples,
        usage,
        tiling,
        p_property_count,
        p_properties
    );
}

pub unsafe extern "win64" fn guest_vkEnumerateDeviceExtensionProperties(
    physical_device: NativeVkPhysicalDevice,
    p_layer_name: *const c_char,
    p_property_count: *mut u32,
    p_properties: *mut VkExtensionProperties,
) -> i32 {
    let sym = match get_native_proc_addr("vkEnumerateDeviceExtensionProperties") {
        Some(s) => s,
        None => return VK_ERROR_INITIALIZATION_FAILED,
    };
    type NativeFn = unsafe extern "C" fn(
        NativeVkPhysicalDevice,
        *const c_char,
        *mut u32,
        *mut VkExtensionProperties,
    ) -> i32;
    let native_fn: NativeFn = std::mem::transmute(sym);

    let mut native_count: u32 = 0;
    let res = native_fn(
        physical_device,
        p_layer_name,
        &mut native_count,
        std::ptr::null_mut(),
    );
    if res != VK_SUCCESS {
        return res;
    }

    let mut temp_props: Vec<VkExtensionProperties> = (0..native_count)
        .map(|_| VkExtensionProperties {
            extensionName: [0; VK_MAX_EXTENSION_NAME_SIZE],
            specVersion: 0,
        })
        .collect();
    let mut fetch_count = native_count;
    let res2 = native_fn(
        physical_device,
        p_layer_name,
        &mut fetch_count,
        temp_props.as_mut_ptr(),
    );
    if res2 != VK_SUCCESS && res2 != VK_INCOMPLETE {
        return res2;
    }
    temp_props.truncate(fetch_count as usize);

    // Filter out extensions whose Win64 ABI thunks are not implemented in Rine
    temp_props.retain(|ext| {
        let name = CStr::from_ptr(ext.extensionName.as_ptr());
        let b = name.to_bytes();
        b != b"VK_EXT_descriptor_buffer"
            && b != b"VK_EXT_shader_module_identifier"
            && b != b"VK_EXT_multi_draw"
            && b != b"VK_EXT_graphics_pipeline_library"
            && b != b"VK_EXT_device_generated_commands"
            && b != b"VK_NV_device_generated_commands"
            && b != b"VK_EXT_mesh_shader"
            && b != b"VK_NV_mesh_shader"
            && b != b"VK_NVX_binary_import"
            && b != b"VK_NV_low_latency2"
            && b != b"VK_EXT_opacity_micromap"
            && b != b"VK_NV_copy_memory_indirect"
            && b != b"VK_NV_memory_decompression"
            && b != b"VK_EXT_pageable_device_local_memory"
            && b != b"VK_EXT_line_rasterization"
            && b != b"VK_EXT_hdr_metadata"
            && b != b"VK_EXT_full_screen_exclusive"
            && b != b"VK_EXT_custom_border_color"
            && b != b"VK_EXT_descriptor_heap"
            && b != b"VK_EXT_extended_dynamic_state3"
            && b != b"VK_NV_raw_access_chains"
    });

    if p_properties.is_null() {
        if !p_property_count.is_null() {
            *p_property_count = temp_props.len() as u32;
        }
        return VK_SUCCESS;
    }

    let capacity = if !p_property_count.is_null() {
        *p_property_count as usize
    } else {
        return VK_ERROR_INITIALIZATION_FAILED;
    };

    let copy_count = std::cmp::min(capacity, temp_props.len());
    for i in 0..copy_count {
        *p_properties.add(i) = temp_props[i];
    }
    *p_property_count = copy_count as u32;

    if copy_count < temp_props.len() {
        VK_INCOMPLETE
    } else {
        VK_SUCCESS
    }
}

pub unsafe extern "win64" fn guest_vkEnumerateDeviceLayerProperties(
    physical_device: NativeVkPhysicalDevice,
    p_property_count: *mut u32,
    p_properties: *mut c_void,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkPhysicalDevice, *mut u32, *mut c_void) -> i32;
    invoke_native!(
        "vkEnumerateDeviceLayerProperties",
        NativeFn,
        physical_device,
        p_property_count,
        p_properties
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 4: Device Lifecycle
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCreateDevice(
    physical_device: NativeVkPhysicalDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_device: *mut NativeVkDevice,
) -> i32 {
    if !p_create_info.is_null() {
        let s_type = *(p_create_info as *const u32);
        let p_next = *((p_create_info as usize + 8) as *const usize);
        let flags = *((p_create_info as usize + 16) as *const u32);
        let q_count = *((p_create_info as usize + 20) as *const u32);
        let p_qinfos = *((p_create_info as usize + 24) as *const usize);
        let layer_count = *((p_create_info as usize + 32) as *const u32);
        let pp_layers = *((p_create_info as usize + 40) as *const usize);
        let ext_count = *((p_create_info as usize + 48) as *const u32);
        let pp_ext_names = *((p_create_info as usize + 56) as *const usize);
        let p_features = *((p_create_info as usize + 64) as *const usize);
        println!(
            "[VULKAN BRIDGE] vkCreateDevice sType={}, pNext=0x{:X}, flags=0x{:X}, qCount={}, pQInfos=0x{:X}, layerCount={}, ppLayers=0x{:X}, extCount={}, ppExtNames=0x{:X}, pFeatures=0x{:X}",
            s_type, p_next, flags, q_count, p_qinfos, layer_count, pp_layers, ext_count, pp_ext_names, p_features
        );
        if p_qinfos != 0 && q_count > 0 {
            for i in 0..q_count as usize {
                let qinfo_ptr = p_qinfos + i * 40; // VkDeviceQueueCreateInfo is 40 bytes on 64-bit
                let q_st = *(qinfo_ptr as *const u32);
                let q_next = *((qinfo_ptr + 8) as *const usize);
                let q_flags = *((qinfo_ptr + 16) as *const u32);
                let q_family = *((qinfo_ptr + 20) as *const u32);
                let q_cnt = *((qinfo_ptr + 24) as *const u32);
                let q_priorities = *((qinfo_ptr + 32) as *const usize);
                let priority_val = if q_priorities != 0 && q_cnt > 0 {
                    *(q_priorities as *const f32)
                } else {
                    -1.0
                };
                println!(
                    "  QueueInfo[{}]: sType={}, pNext=0x{:X}, flags=0x{:X}, family={}, count={}, priorities_ptr=0x{:X} (first={})",
                    i, q_st, q_next, q_flags, q_family, q_cnt, q_priorities, priority_val
                );
            }
        }
        if pp_ext_names != 0 && ext_count > 0 {
            for i in 0..ext_count as usize {
                let p_str = *((pp_ext_names + i * 8) as *const usize);
                if p_str != 0 {
                    let s = CStr::from_ptr(p_str as *const c_char).to_string_lossy();
                    println!("  Device Ext[{}]: {}", i, s);
                }
            }
        }
        if p_next != 0 {
            let mut curr = p_next;
            let mut depth = 0;
            while curr != 0 && depth < 64 {
                let st = *(curr as *const u32);
                let next = *((curr + 8) as *const usize);
                println!("  pNext[{}]: sType={} (0x{:X}), next=0x{:X}", depth, st, st, next);
                curr = next;
                depth += 1;
            }
        }
    }
    use std::io::Write;
    std::io::stdout().flush().ok();
    type NativeFn = unsafe extern "C" fn(
        NativeVkPhysicalDevice,
        *const c_void,
        *const c_void,
        *mut NativeVkDevice,
    ) -> i32;
    // NULL allocator: guest pAllocator contains Win64 function pointers
    let mut res = invoke_native!(
        "vkCreateDevice",
        NativeFn,
        physical_device,
        p_create_info,
        std::ptr::null(),
        p_device
    );
    if res != 0 && !p_create_info.is_null() {
        println!("[VULKAN BRIDGE DIAGNOSTIC] vkCreateDevice failed with {}, running diagnostic tests...", res);
        let sym = get_native_proc_addr("vkCreateDevice").unwrap();
        let native_fn: NativeFn = std::mem::transmute(sym);

        // Test A: Minimal (no pNext, no exts, no features)
        let mut t_min = *(p_create_info as *const [usize; 9]);
        t_min[1] = 0; // pNext = 0
        t_min[6] = 0; // extCount = 0
        t_min[7] = 0; // ppExtNames = 0
        t_min[8] = 0; // pFeatures = 0
        let mut dev_min: NativeVkDevice = std::ptr::null_mut();
        let res_min = native_fn(physical_device, t_min.as_ptr() as *const c_void, std::ptr::null(), &mut dev_min);
        println!("  [TEST A] Minimal (no pNext, no exts, no features) -> Result={}", res_min);
        if res_min == 0 {
            invoke_native_void!("vkDestroyDevice", unsafe extern "C" fn(NativeVkDevice, *const c_void), dev_min, std::ptr::null());
        }

        // Test B: Extensions only (no pNext, no features)
        let mut t_ext = *(p_create_info as *const [usize; 9]);
        t_ext[1] = 0; // pNext = 0
        t_ext[8] = 0; // pFeatures = 0
        let mut dev_ext: NativeVkDevice = std::ptr::null_mut();
        let res_ext = native_fn(physical_device, t_ext.as_ptr() as *const c_void, std::ptr::null(), &mut dev_ext);
        println!("  [TEST B] Extensions only (40 exts, no pNext, no features) -> Result={}", res_ext);
        if res_ext == 0 {
            invoke_native_void!("vkDestroyDevice", unsafe extern "C" fn(NativeVkDevice, *const c_void), dev_ext, std::ptr::null());
        } else {
            // Test each extension individually!
            let ext_count = t_ext[6] as u32;
            let pp_ext_names = t_ext[7];
            for i in 0..ext_count as usize {
                let mut t_single = t_min;
                let p_one_ext = pp_ext_names + i * 8;
                t_single[6] = 1;
                t_single[7] = p_one_ext;
                let mut dev_s: NativeVkDevice = std::ptr::null_mut();
                let res_s = native_fn(physical_device, t_single.as_ptr() as *const c_void, std::ptr::null(), &mut dev_s);
                let ext_name_ptr = *(p_one_ext as *const usize);
                let ext_name = if ext_name_ptr != 0 {
                    CStr::from_ptr(ext_name_ptr as *const c_char).to_string_lossy()
                } else {
                    "NULL".into()
                };
                if res_s != 0 {
                    println!("    --> EXTENSION FAILS: [{}] {} returned {}", i, ext_name, res_s);
                } else {
                    invoke_native_void!("vkDestroyDevice", unsafe extern "C" fn(NativeVkDevice, *const c_void), dev_s, std::ptr::null());
                }
            }
        }

        // Test C: pFeatures only (no pNext, no exts)
        let mut t_feat = *(p_create_info as *const [usize; 9]);
        t_feat[1] = 0; // pNext = 0
        t_feat[6] = 0; // exts = 0
        t_feat[7] = 0;
        let mut dev_feat: NativeVkDevice = std::ptr::null_mut();
        let res_feat = native_fn(physical_device, t_feat.as_ptr() as *const c_void, std::ptr::null(), &mut dev_feat);
        println!("  [TEST C] pFeatures only (no pNext, no exts) -> Result={}", res_feat);
        if res_feat == 0 {
            invoke_native_void!("vkDestroyDevice", unsafe extern "C" fn(NativeVkDevice, *const c_void), dev_feat, std::ptr::null());
        }

        // Test D: pNext only (no exts, no features)
        let mut t_pnext = *(p_create_info as *const [usize; 9]);
        t_pnext[6] = 0; // exts = 0
        t_pnext[7] = 0;
        t_pnext[8] = 0; // features = 0
        let mut dev_pn: NativeVkDevice = std::ptr::null_mut();
        let res_pn = native_fn(physical_device, t_pnext.as_ptr() as *const c_void, std::ptr::null(), &mut dev_pn);
        println!("  [TEST D] pNext only (no exts, no features) -> Result={}", res_pn);
        if res_pn == 0 {
            invoke_native_void!("vkDestroyDevice", unsafe extern "C" fn(NativeVkDevice, *const c_void), dev_pn, std::ptr::null());
        }

        // Test E: pNext + exts (no features)
        let mut t_pne = *(p_create_info as *const [usize; 9]);
        t_pne[8] = 0; // features = 0
        let mut dev_pne: NativeVkDevice = std::ptr::null_mut();
        let res_pne = native_fn(physical_device, t_pne.as_ptr() as *const c_void, std::ptr::null(), &mut dev_pne);
        println!("  [TEST E] pNext + exts (no features) -> Result={}", res_pne);
        if res_pne == 0 {
            println!("  --> SUCCESS with pEnabledFeatures=NULL! Using dev_pne!");
            res = 0;
            if !p_device.is_null() {
                *p_device = dev_pne;
            }
        }
    }
    if res == 0 && !p_device.is_null() && !(*p_device).is_null() {
        super::loader::set_native_device(*p_device as usize);
    }
    println!(
        "[VULKAN BRIDGE] vkCreateDevice -> Result={} (Device={:p})",
        res,
        if !p_device.is_null() {
            *p_device
        } else {
            std::ptr::null_mut()
        }
    );
    std::io::stdout().flush().ok();
    res
}

pub unsafe extern "win64" fn guest_vkDestroyDevice(
    device: NativeVkDevice,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyDevice",
        unsafe extern "C" fn(NativeVkDevice, *const c_void),
        device,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkGetDeviceQueue(
    device: NativeVkDevice,
    queue_family_index: u32,
    queue_index: u32,
    p_queue: *mut *mut c_void,
) {
    invoke_native_void!(
        "vkGetDeviceQueue",
        unsafe extern "C" fn(NativeVkDevice, u32, u32, *mut *mut c_void),
        device,
        queue_family_index,
        queue_index,
        p_queue
    );
    if !p_queue.is_null() && !(*p_queue).is_null() {
        super::swapchain::LAST_QUEUE.store(*p_queue as usize, std::sync::atomic::Ordering::Relaxed);
    }
}

pub unsafe extern "win64" fn guest_vkGetDeviceQueue2(
    device: NativeVkDevice,
    p_queue_info: *const c_void,
    p_queue: *mut *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetDeviceQueue2") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_queue_info, p_queue);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 5: Surface & WSI (handled via surface.rs for creation)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceSurfaceSupportKHR(
    physical_device: NativeVkPhysicalDevice,
    queue_family_index: u32,
    surface: NativeVkSurfaceKHR,
    p_supported: *mut u32,
) -> i32 {
    let mut res = VK_ERROR_INITIALIZATION_FAILED;
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceSurfaceSupportKHR") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkPhysicalDevice, u32, NativeVkSurfaceKHR, *mut u32) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        res = f(physical_device, queue_family_index, surface, p_supported);
    }
    if !p_supported.is_null() && *p_supported == 0 {
        *p_supported = 1;
        res = 0;
    }
    println!(
        "[VK SURFACE SUPPORT] physical_device={:p}, queue_family={}, surface=0x{:X}, supported={} -> res={}",
        physical_device,
        queue_family_index,
        surface,
        if !p_supported.is_null() { *p_supported } else { 0 },
        res
    );
    res
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceSurfaceCapabilitiesKHR(
    physical_device: NativeVkPhysicalDevice,
    surface: NativeVkSurfaceKHR,
    p_surface_capabilities: *mut c_void,
) -> i32 {
    let mut res = VK_ERROR_INITIALIZATION_FAILED;
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceSurfaceCapabilitiesKHR") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkPhysicalDevice, NativeVkSurfaceKHR, *mut c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        res = f(physical_device, surface, p_surface_capabilities);
    }
    let (real_w, real_h) = crate::windowing::x11::sdl_get_window_size();
    if res != 0 && !p_surface_capabilities.is_null() {
        // Fallback surface capabilities
        let caps = p_surface_capabilities as *mut u32;
        *caps.add(0) = 2; // minImageCount
        *caps.add(1) = 8; // maxImageCount
        let cur_w = if real_w > 0 { real_w as u32 } else { 1920 };
        let cur_h = if real_h > 0 { real_h as u32 } else { 1080 };
        *caps.add(2) = cur_w; // currentExtent.width
        *caps.add(3) = cur_h; // currentExtent.height
        *caps.add(4) = 1; // minImageExtent.width
        *caps.add(5) = 1; // minImageExtent.height
        *caps.add(6) = 4096; // maxImageExtent.width
        *caps.add(7) = 4096; // maxImageExtent.height
        *caps.add(8) = 1; // maxImageArrayLayers
        *caps.add(9) = 1; // supportedTransforms (VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR)
        *caps.add(10) = 1; // currentTransform
        *caps.add(11) = 1; // supportedCompositeAlpha (VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR)
        *caps.add(12) = 0x9F; // supportedUsageFlags (COLOR_ATTACHMENT | TRANSFER_SRC | TRANSFER_DST | SAMPLED | STORAGE)
        res = 0;
    } else if res == 0 && !p_surface_capabilities.is_null() {
        let caps = p_surface_capabilities as *mut u32;
        let cw = *caps.add(2);
        let ch = *caps.add(3);
        if cw == 0xFFFFFFFF && real_w > 0 && real_h > 0 {
            *caps.add(2) = real_w as u32;
            *caps.add(3) = real_h as u32;
        }
        println!(
            "[VK SURFACE CAPABILITIES] physical_device={:p}, surface=0x{:X} -> extent={}x{} (synced={}x{})",
            physical_device, surface, cw, ch, *caps.add(2), *caps.add(3)
        );
    }
    println!(
        "[VK SURFACE CAPABILITIES] physical_device={:p}, surface=0x{:X} -> res={}",
        physical_device, surface, res
    );
    res
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceSurfaceCapabilities2KHR(
    physical_device: NativeVkPhysicalDevice,
    p_surface_info: *const c_void,
    p_surface_capabilities: *mut c_void,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceSurfaceCapabilities2KHR") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkPhysicalDevice, *const c_void, *mut c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        let res = f(physical_device, p_surface_info, p_surface_capabilities);
        if res == 0 && !p_surface_capabilities.is_null() {
            let caps = (p_surface_capabilities as *mut u8).add(16) as *mut u32;
            let (real_w, real_h) = crate::windowing::x11::sdl_get_window_size();
            let cw = *caps.add(2);
            if cw != 0xFFFFFFFF && real_w > 0 && real_h > 0 {
                *caps.add(2) = real_w as u32;
                *caps.add(3) = real_h as u32;
            }
        }
        res
    } else {
        VK_ERROR_EXTENSION_NOT_PRESENT
    }
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceSurfaceFormatsKHR(
    physical_device: NativeVkPhysicalDevice,
    surface: NativeVkSurfaceKHR,
    p_surface_format_count: *mut u32,
    p_surface_formats: *mut c_void,
) -> i32 {
    let mut res = VK_ERROR_INITIALIZATION_FAILED;
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceSurfaceFormatsKHR") {
        type NativeFn = unsafe extern "C" fn(
            NativeVkPhysicalDevice,
            NativeVkSurfaceKHR,
            *mut u32,
            *mut c_void,
        ) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        res = f(
            physical_device,
            surface,
            p_surface_format_count,
            p_surface_formats,
        );
    }
    if res != 0 || (!p_surface_format_count.is_null() && *p_surface_format_count == 0) {
        if !p_surface_format_count.is_null() {
            if p_surface_formats.is_null() {
                *p_surface_format_count = 4;
            } else {
                let count = (*p_surface_format_count).min(4);
                let formats = p_surface_formats as *mut [u32; 2];
                // 1. VK_FORMAT_B8G8R8A8_UNORM = 44, VK_COLOR_SPACE_SRGB_NONLINEAR_KHR = 0
                if count >= 1 {
                    *formats.add(0) = [44, 0];
                }
                // 2. VK_FORMAT_R8G8B8A8_UNORM = 37, VK_COLOR_SPACE_SRGB_NONLINEAR_KHR = 0
                if count >= 2 {
                    *formats.add(1) = [37, 0];
                }
                // 3. VK_FORMAT_B8G8R8A8_SRGB = 50, VK_COLOR_SPACE_SRGB_NONLINEAR_KHR = 0
                if count >= 3 {
                    *formats.add(2) = [50, 0];
                }
                // 4. VK_FORMAT_R8G8B8A8_SRGB = 43, VK_COLOR_SPACE_SRGB_NONLINEAR_KHR = 0
                if count >= 4 {
                    *formats.add(3) = [43, 0];
                }
                *p_surface_format_count = count;
            }
            res = 0;
        }
    }
    println!(
        "[VK SURFACE FORMATS] physical_device={:p}, surface=0x{:X}, count={} -> res={}",
        physical_device,
        surface,
        if !p_surface_format_count.is_null() {
            *p_surface_format_count
        } else {
            0
        },
        res
    );
    res
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceSurfaceFormats2KHR(
    physical_device: NativeVkPhysicalDevice,
    p_surface_info: *const c_void,
    p_surface_format_count: *mut u32,
    p_surface_formats: *mut c_void,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceSurfaceFormats2KHR") {
        type NativeFn = unsafe extern "C" fn(
            NativeVkPhysicalDevice,
            *const c_void,
            *mut u32,
            *mut c_void,
        ) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(
            physical_device,
            p_surface_info,
            p_surface_format_count,
            p_surface_formats,
        )
    } else {
        VK_ERROR_EXTENSION_NOT_PRESENT
    }
}

pub unsafe extern "win64" fn guest_vkGetPhysicalDeviceSurfacePresentModesKHR(
    physical_device: NativeVkPhysicalDevice,
    surface: NativeVkSurfaceKHR,
    p_present_mode_count: *mut u32,
    p_present_modes: *mut c_void,
) -> i32 {
    let mut res = VK_ERROR_INITIALIZATION_FAILED;
    if let Some(sym) = get_native_proc_addr("vkGetPhysicalDeviceSurfacePresentModesKHR") {
        type NativeFn = unsafe extern "C" fn(
            NativeVkPhysicalDevice,
            NativeVkSurfaceKHR,
            *mut u32,
            *mut c_void,
        ) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        res = f(
            physical_device,
            surface,
            p_present_mode_count,
            p_present_modes,
        );
    }
    if res != 0 || (!p_present_mode_count.is_null() && *p_present_mode_count == 0) {
        if !p_present_mode_count.is_null() {
            if p_present_modes.is_null() {
                *p_present_mode_count = 3;
            } else {
                let count = (*p_present_mode_count).min(3);
                let modes = p_present_modes as *mut u32;
                // VK_PRESENT_MODE_FIFO_KHR = 2
                if count >= 1 {
                    *modes.add(0) = 2;
                }
                // VK_PRESENT_MODE_MAILBOX_KHR = 1
                if count >= 2 {
                    *modes.add(1) = 1;
                }
                // VK_PRESENT_MODE_IMMEDIATE_KHR = 0
                if count >= 3 {
                    *modes.add(2) = 0;
                }
                *p_present_mode_count = count;
            }
            res = 0;
        }
    }
    println!(
        "[VK PRESENT MODES] physical_device={:p}, surface=0x{:X}, count={} -> res={}",
        physical_device,
        surface,
        if !p_present_mode_count.is_null() {
            *p_present_mode_count
        } else {
            0
        },
        res
    );
    res
}

pub unsafe extern "win64" fn guest_vkDestroySurfaceKHR(
    instance: NativeVkInstance,
    surface: NativeVkSurfaceKHR,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroySurfaceKHR",
        unsafe extern "C" fn(NativeVkInstance, NativeVkSurfaceKHR, *const c_void),
        instance,
        surface,
        std::ptr::null()
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 6: Swapchain
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCreateSwapchainKHR(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_swapchain: *mut u64,
) -> i32 {
    let mut extent_w = 1920;
    let mut extent_h = 1080;
    let mut min_images = 3;
    let mut format = 44; // VK_FORMAT_B8G8R8A8_UNORM

    if !p_create_info.is_null() {
        let ptr = p_create_info as *const u32;
        let s_type = *ptr.add(0);
        let flags = *ptr.add(4);
        let surface_lo = *ptr.add(6);
        let surface_hi = *ptr.add(7);
        let surface = (surface_lo as u64) | ((surface_hi as u64) << 32);
        min_images = *ptr.add(8);
        format = *ptr.add(9);
        let color_space = *ptr.add(10);
        extent_w = *ptr.add(11);
        extent_h = *ptr.add(12);
        let present_mode = *ptr.add(22);
        println!(
            "[VK SWAPCHAIN CREATE] sType={}, flags=0x{:X}, surface=0x{:X}, minImages={}, format={}, colorSpace={}, extent={}x{}, presentMode={}",
            s_type, flags, surface, min_images, format, color_space, extent_w, extent_h, present_mode
        );
    }

    if let Some(sym) = get_native_proc_addr("vkCreateSwapchainKHR") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
        let fn_create: NativeFn = std::mem::transmute(sym);
        let res = fn_create(device, p_create_info, std::ptr::null(), p_swapchain);
        println!(
            "[VULKAN BRIDGE] Host vkCreateSwapchainKHR -> Result={} (Swapchain=0x{:X})",
            res,
            if !p_swapchain.is_null() {
                *p_swapchain
            } else {
                0
            }
        );
        if res == 0 {
            if !p_swapchain.is_null() {
                super::swapchain::register_host_swapchain(*p_swapchain, device as usize);
            }
            crate::graphics::dxgi_vulkan::set_vulkan_presentation_confirmed();
            return VK_SUCCESS;
        }
    }

    let v_swapchain =
        super::swapchain::create_virtual_swapchain(device, extent_w, extent_h, min_images, format);

    if !p_swapchain.is_null() {
        *p_swapchain = v_swapchain;
    }

    println!(
        "[VULKAN BRIDGE] Fallback vkCreateSwapchainKHR -> Result=0 (Swapchain=0x{:X})",
        v_swapchain
    );
    VK_SUCCESS
}

pub unsafe extern "win64" fn guest_vkDestroySwapchainKHR(
    device: NativeVkDevice,
    swapchain: u64,
    _p_allocator: *const c_void,
) {
    if super::swapchain::is_host_swapchain(swapchain) {
        if let Some(sym) = get_native_proc_addr("vkDestroySwapchainKHR") {
            type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, *const c_void);
            let f: NativeFn = std::mem::transmute(sym);
            f(device, swapchain, std::ptr::null());
        }
        super::swapchain::unregister_host_swapchain(swapchain);
    } else {
        super::swapchain::destroy_swapchain(swapchain);
    }
}

pub unsafe extern "win64" fn guest_vkGetSwapchainImagesKHR(
    device: NativeVkDevice,
    swapchain: u64,
    p_swapchain_image_count: *mut u32,
    p_swapchain_images: *mut u64,
) -> i32 {
    if super::swapchain::is_host_swapchain(swapchain) {
        if let Some(sym) = get_native_proc_addr("vkGetSwapchainImagesKHR") {
            type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, *mut u32, *mut u64) -> i32;
            let f: NativeFn = std::mem::transmute(sym);
            let res = f(
                device,
                swapchain,
                p_swapchain_image_count,
                p_swapchain_images,
            );
            println!(
                "[VULKAN BRIDGE] Host vkGetSwapchainImagesKHR -> Result={} (count={})",
                res,
                if !p_swapchain_image_count.is_null() {
                    *p_swapchain_image_count
                } else {
                    0
                }
            );
            return res;
        }
    }
    let res = super::swapchain::get_swapchain_images(
        swapchain,
        p_swapchain_image_count,
        p_swapchain_images,
    );
    println!(
        "[VULKAN BRIDGE] Virtual vkGetSwapchainImagesKHR -> Result={} (count={})",
        res,
        if !p_swapchain_image_count.is_null() {
            *p_swapchain_image_count
        } else {
            0
        }
    );
    res
}

pub unsafe extern "win64" fn guest_vkAcquireNextImageKHR(
    device: NativeVkDevice,
    swapchain: u64,
    timeout: u64,
    semaphore: u64,
    fence: u64,
    p_image_index: *mut u32,
) -> i32 {
    crate::sys::diagnostics::record_gfx_api("vkAcquireNextImageKHR");
    let is_host = super::swapchain::is_host_swapchain(swapchain);
    let res = if is_host {
        if let Some(sym) = get_native_proc_addr("vkAcquireNextImageKHR") {
            type NativeFn =
                unsafe extern "C" fn(NativeVkDevice, u64, u64, u64, u64, *mut u32) -> i32;
            let f: NativeFn = std::mem::transmute(sym);
            f(device, swapchain, timeout, semaphore, fence, p_image_index)
        } else {
            VK_ERROR_EXTENSION_NOT_PRESENT
        }
    } else {
        super::swapchain::acquire_next_image(swapchain, semaphore, fence, p_image_index)
    };
    res
}

pub unsafe extern "win64" fn guest_vkAcquireNextImage2KHR(
    device: NativeVkDevice,
    p_acquire_info: *const c_void,
    p_image_index: *mut u32,
) -> i32 {
    crate::sys::diagnostics::record_gfx_api("vkAcquireNextImage2KHR");
    if !p_acquire_info.is_null() {
        let ptr = p_acquire_info as *const u32;
        let swapchain_lo = *ptr.add(4);
        let swapchain_hi = *ptr.add(5);
        let swapchain = (swapchain_lo as u64) | ((swapchain_hi as u64) << 32);
        let timeout_lo = *ptr.add(6);
        let timeout_hi = *ptr.add(7);
        let timeout = (timeout_lo as u64) | ((timeout_hi as u64) << 32);
        let sem_lo = *ptr.add(8);
        let sem_hi = *ptr.add(9);
        let semaphore = (sem_lo as u64) | ((sem_hi as u64) << 32);
        let fence_lo = *ptr.add(10);
        let fence_hi = *ptr.add(11);
        let fence = (fence_lo as u64) | ((fence_hi as u64) << 32);

        let is_host = super::swapchain::is_host_swapchain(swapchain);
        println!(
            "[VK ACQUIRE2 ENTRY] is_host={}, dev=0x{:X}, swapchain=0x{:X}, timeout={}, sem=0x{:X}, fence=0x{:X}",
            is_host, device as usize, swapchain, timeout, semaphore, fence
        );

        let res = if is_host {
            if let Some(sym) = get_native_proc_addr("vkAcquireNextImage2KHR") {
                type NativeFn =
                    unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut u32) -> i32;
                let f: NativeFn = std::mem::transmute(sym);
                f(device, p_acquire_info, p_image_index)
            } else {
                guest_vkAcquireNextImageKHR(
                    device,
                    swapchain,
                    timeout,
                    semaphore,
                    fence,
                    p_image_index,
                )
            }
        } else {
            super::swapchain::acquire_next_image(swapchain, semaphore, fence, p_image_index)
        };
        let idx = if !p_image_index.is_null() { *p_image_index } else { 999 };
        println!("[VK ACQUIRE2 RESULT] res={}, out_image_index={}", res, idx);
        return res;
    }
    VK_ERROR_INITIALIZATION_FAILED
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 7: Queue Operations (Submit / Present / Wait)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkQueueSubmit(
    queue: *mut c_void,
    submit_count: u32,
    p_submits: *const c_void,
    fence: u64,
) -> i32 {
    if !queue.is_null() {
        super::swapchain::LAST_QUEUE.store(queue as usize, std::sync::atomic::Ordering::Relaxed);
    }
    type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void, u64) -> i32;
    invoke_native!(
        "vkQueueSubmit",
        NativeFn,
        queue,
        submit_count,
        p_submits,
        fence
    )
}

pub unsafe extern "win64" fn guest_vkQueueSubmit2(
    queue: *mut c_void,
    submit_count: u32,
    p_submits: *const c_void,
    fence: u64,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkQueueSubmit2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void, u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(queue, submit_count, p_submits, fence)
    } else if let Some(sym) = get_native_proc_addr("vkQueueSubmit2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void, u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(queue, submit_count, p_submits, fence)
    } else {
        VK_ERROR_EXTENSION_NOT_PRESENT
    }
}

pub unsafe extern "win64" fn guest_vkQueuePresentKHR(
    queue: *mut c_void,
    p_present_info: *const c_void,
) -> i32 {
    let mut is_host = false;
    let mut swapchain_count = 0u32;
    let mut swapchain = 0u64;
    let mut image_idx = 0u32;

    if !p_present_info.is_null() {
        let ptr = p_present_info as *const u32;
        swapchain_count = *ptr.add(8);
        let p_swapchains_ptr = *(ptr.add(10) as *const u64);
        if swapchain_count > 0 && p_swapchains_ptr != 0 {
            swapchain = *(p_swapchains_ptr as *const u64);
            is_host = super::swapchain::is_host_swapchain(swapchain);
            let p_indices_ptr = *(ptr.add(12) as *const u64);
            if p_indices_ptr != 0 {
                image_idx = *(p_indices_ptr as *const u32);
            }
            if !is_host {
                super::swapchain::present_swapchain_image(queue, swapchain, image_idx);
            }
        }
    }

    if is_host {
        if let Some(sym) = get_native_proc_addr("vkQueuePresentKHR") {
            type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void) -> i32;
            let f: NativeFn = std::mem::transmute(sym);
            let res = f(queue, p_present_info);
            if res == -1000000000 /* VK_ERROR_SURFACE_LOST_KHR */ {
                println!("[RINE VULKAN] Surface lost (window closed by Win+Q). Cleanly terminating process...");
                crate::sys::shutdown::request_host_shutdown("Vulkan surface lost / Win+Q");
                unsafe { libc::_exit(0); }
            }
            let frame_num = super::swapchain::FRAME_COUNTER
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            crate::sys::diagnostics::record_gfx_api("vkQueuePresentKHR");
            crate::sys::diagnostics::notify_frame_presented();
            if frame_num <= 2 {
                crate::windowing::x11::show_and_unhide_game_window();
                println!(
                    "[RINE REAL HOST PRESENT] Frame #{} successfully presented on native X11 surface via vkQueuePresentKHR (VkResult={})!",
                    frame_num, res
                );
                println!("[RINE_READY] Game window active and rendering.");
                use std::io::Write;
                let _ = std::io::stdout().flush();
            } else if frame_num % 120 == 0 {
                println!("[RINE HOST PRESENT] Frame #{} rendered and presented on host swapchain 0x{:X} (VkResult={})", frame_num, swapchain, res);
            }
            crate::graphics::dxgi_vulkan::set_vulkan_presentation_confirmed();
            if res == 1000001003 /* VK_SUBOPTIMAL_KHR */ {
                return 0; // VK_SUCCESS: prevent DXVK from thrashing and destroying swapchain on Xwayland
            }
            return res;
        }
    }

    crate::graphics::dxgi_vulkan::set_vulkan_presentation_confirmed();
    VK_SUCCESS
}

pub unsafe extern "win64" fn guest_vkDeviceWaitIdle(device: NativeVkDevice) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice) -> i32;
    invoke_native!("vkDeviceWaitIdle", NativeFn, device)
}

pub unsafe extern "win64" fn guest_vkQueueWaitIdle(queue: *mut c_void) -> i32 {
    type NativeFn = unsafe extern "C" fn(*mut c_void) -> i32;
    invoke_native!("vkQueueWaitIdle", NativeFn, queue)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 8: Synchronization Primitives
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCreateSemaphore(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_semaphore: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateSemaphore",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_semaphore
    )
}

pub unsafe extern "win64" fn guest_vkDestroySemaphore(
    device: NativeVkDevice,
    semaphore: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroySemaphore",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        semaphore,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkCreateFence(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_fence: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateFence",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_fence
    )
}

pub unsafe extern "win64" fn guest_vkDestroyFence(
    device: NativeVkDevice,
    fence: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyFence",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        fence,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkWaitForFences(
    device: NativeVkDevice,
    fence_count: u32,
    p_fences: *const u64,
    wait_all: u32,
    timeout: u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u32, *const u64, u32, u64) -> i32;
    invoke_native!(
        "vkWaitForFences",
        NativeFn,
        device,
        fence_count,
        p_fences,
        wait_all,
        timeout
    )
}

pub unsafe extern "win64" fn guest_vkResetFences(
    device: NativeVkDevice,
    fence_count: u32,
    p_fences: *const u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u32, *const u64) -> i32;
    invoke_native!("vkResetFences", NativeFn, device, fence_count, p_fences)
}

pub unsafe extern "win64" fn guest_vkCreateEvent(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_event: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateEvent",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_event
    )
}

pub unsafe extern "win64" fn guest_vkDestroyEvent(
    device: NativeVkDevice,
    event: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyEvent",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        event,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkGetEventStatus(device: NativeVkDevice, event: u64) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64) -> i32;
    invoke_native!("vkGetEventStatus", NativeFn, device, event)
}

pub unsafe extern "win64" fn guest_vkSetEvent(device: NativeVkDevice, event: u64) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64) -> i32;
    invoke_native!("vkSetEvent", NativeFn, device, event)
}

pub unsafe extern "win64" fn guest_vkResetEvent(device: NativeVkDevice, event: u64) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64) -> i32;
    invoke_native!("vkResetEvent", NativeFn, device, event)
}

pub unsafe extern "win64" fn guest_vkGetSemaphoreCounterValue(
    device: NativeVkDevice,
    semaphore: u64,
    p_value: *mut u64,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkGetSemaphoreCounterValue") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, *mut u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, semaphore, p_value)
    } else if let Some(sym) = get_native_proc_addr("vkGetSemaphoreCounterValueKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, *mut u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, semaphore, p_value)
    } else {
        VK_ERROR_INITIALIZATION_FAILED
    }
}

pub unsafe extern "win64" fn guest_vkWaitSemaphores(
    device: NativeVkDevice,
    p_wait_info: *const c_void,
    timeout: u64,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkWaitSemaphores") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_wait_info, timeout)
    } else if let Some(sym) = get_native_proc_addr("vkWaitSemaphoresKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_wait_info, timeout)
    } else {
        VK_ERROR_INITIALIZATION_FAILED
    }
}

pub unsafe extern "win64" fn guest_vkSignalSemaphore(
    device: NativeVkDevice,
    p_signal_info: *const c_void,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkSignalSemaphore") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_signal_info)
    } else if let Some(sym) = get_native_proc_addr("vkSignalSemaphoreKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_signal_info)
    } else {
        VK_ERROR_INITIALIZATION_FAILED
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 9: Memory Management
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkAllocateMemory(
    device: NativeVkDevice,
    p_allocate_info: *const c_void,
    _p_allocator: *const c_void,
    p_memory: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkAllocateMemory",
        NativeFn,
        device,
        p_allocate_info,
        std::ptr::null(),
        p_memory
    )
}

pub unsafe extern "win64" fn guest_vkFreeMemory(
    device: NativeVkDevice,
    memory: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkFreeMemory",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        memory,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkMapMemory(
    device: NativeVkDevice,
    memory: u64,
    offset: u64,
    size: u64,
    flags: u32,
    pp_data: *mut *mut c_void,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, u64, u64, u64, u32, *mut *mut c_void) -> i32;
    invoke_native!(
        "vkMapMemory",
        NativeFn,
        device,
        memory,
        offset,
        size,
        flags,
        pp_data
    )
}

pub unsafe extern "win64" fn guest_vkUnmapMemory(device: NativeVkDevice, memory: u64) {
    invoke_native_void!(
        "vkUnmapMemory",
        unsafe extern "C" fn(NativeVkDevice, u64),
        device,
        memory
    );
}

pub unsafe extern "win64" fn guest_vkFlushMappedMemoryRanges(
    device: NativeVkDevice,
    memory_range_count: u32,
    p_memory_ranges: *const c_void,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u32, *const c_void) -> i32;
    invoke_native!(
        "vkFlushMappedMemoryRanges",
        NativeFn,
        device,
        memory_range_count,
        p_memory_ranges
    )
}

pub unsafe extern "win64" fn guest_vkInvalidateMappedMemoryRanges(
    device: NativeVkDevice,
    memory_range_count: u32,
    p_memory_ranges: *const c_void,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u32, *const c_void) -> i32;
    invoke_native!(
        "vkInvalidateMappedMemoryRanges",
        NativeFn,
        device,
        memory_range_count,
        p_memory_ranges
    )
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 10: Buffer & Image Resources
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCreateBuffer(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_buffer: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateBuffer",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_buffer
    )
}

pub unsafe extern "win64" fn guest_vkDestroyBuffer(
    device: NativeVkDevice,
    buffer: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyBuffer",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        buffer,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkCreateImage(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_image: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateImage",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_image
    )
}

pub unsafe extern "win64" fn guest_vkDestroyImage(
    device: NativeVkDevice,
    image: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyImage",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        image,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkCreateImageView(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_view: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateImageView",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_view
    )
}

pub unsafe extern "win64" fn guest_vkDestroyImageView(
    device: NativeVkDevice,
    view: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyImageView",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        view,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkCreateBufferView(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_view: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateBufferView",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_view
    )
}

pub unsafe extern "win64" fn guest_vkDestroyBufferView(
    device: NativeVkDevice,
    view: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyBufferView",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        view,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkGetBufferMemoryRequirements(
    device: NativeVkDevice,
    buffer: u64,
    p_reqs: *mut c_void,
) {
    invoke_native_void!(
        "vkGetBufferMemoryRequirements",
        unsafe extern "C" fn(NativeVkDevice, u64, *mut c_void),
        device,
        buffer,
        p_reqs
    );
}

pub unsafe extern "win64" fn guest_vkGetBufferMemoryRequirements2(
    device: NativeVkDevice,
    p_info: *const c_void,
    p_reqs: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetBufferMemoryRequirements2") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_info, p_reqs);
    } else if let Some(sym) = get_native_proc_addr("vkGetBufferMemoryRequirements2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_info, p_reqs);
    }
}

pub unsafe extern "win64" fn guest_vkGetImageMemoryRequirements(
    device: NativeVkDevice,
    image: u64,
    p_reqs: *mut c_void,
) {
    invoke_native_void!(
        "vkGetImageMemoryRequirements",
        unsafe extern "C" fn(NativeVkDevice, u64, *mut c_void),
        device,
        image,
        p_reqs
    );
}

pub unsafe extern "win64" fn guest_vkGetImageMemoryRequirements2(
    device: NativeVkDevice,
    p_info: *const c_void,
    p_reqs: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetImageMemoryRequirements2") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_info, p_reqs);
    } else if let Some(sym) = get_native_proc_addr("vkGetImageMemoryRequirements2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_info, p_reqs);
    }
}

pub unsafe extern "win64" fn guest_vkGetDeviceBufferMemoryRequirements(
    device: NativeVkDevice,
    p_info: *const c_void,
    p_reqs: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetDeviceBufferMemoryRequirements") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_info, p_reqs);
    } else if let Some(sym) = get_native_proc_addr("vkGetDeviceBufferMemoryRequirementsKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_info, p_reqs);
    }
}

pub unsafe extern "win64" fn guest_vkGetDeviceImageMemoryRequirements(
    device: NativeVkDevice,
    p_info: *const c_void,
    p_reqs: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetDeviceImageMemoryRequirements") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_info, p_reqs);
    } else if let Some(sym) = get_native_proc_addr("vkGetDeviceImageMemoryRequirementsKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_info, p_reqs);
    }
}

pub unsafe extern "win64" fn guest_vkGetDeviceImageSparseMemoryRequirements(
    device: NativeVkDevice,
    p_info: *const c_void,
    p_sparse_memory_requirement_count: *mut u32,
    p_sparse_memory_requirements: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetDeviceImageSparseMemoryRequirements") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut u32, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(
            device,
            p_info,
            p_sparse_memory_requirement_count,
            p_sparse_memory_requirements,
        );
    } else if let Some(sym) = get_native_proc_addr("vkGetDeviceImageSparseMemoryRequirementsKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut u32, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(
            device,
            p_info,
            p_sparse_memory_requirement_count,
            p_sparse_memory_requirements,
        );
    }
}

pub unsafe extern "win64" fn guest_vkGetImageSparseMemoryRequirements(
    device: NativeVkDevice,
    image: u64,
    p_sparse_memory_requirement_count: *mut u32,
    p_sparse_memory_requirements: *mut c_void,
) {
    invoke_native_void!(
        "vkGetImageSparseMemoryRequirements",
        unsafe extern "C" fn(NativeVkDevice, u64, *mut u32, *mut c_void),
        device,
        image,
        p_sparse_memory_requirement_count,
        p_sparse_memory_requirements
    );
}

pub unsafe extern "win64" fn guest_vkGetImageSparseMemoryRequirements2(
    device: NativeVkDevice,
    p_info: *const c_void,
    p_sparse_memory_requirement_count: *mut u32,
    p_sparse_memory_requirements: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetImageSparseMemoryRequirements2") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut u32, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(
            device,
            p_info,
            p_sparse_memory_requirement_count,
            p_sparse_memory_requirements,
        );
    } else if let Some(sym) = get_native_proc_addr("vkGetImageSparseMemoryRequirements2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut u32, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(
            device,
            p_info,
            p_sparse_memory_requirement_count,
            p_sparse_memory_requirements,
        );
    }
}

pub unsafe extern "win64" fn guest_vkGetBufferDeviceAddress(
    device: NativeVkDevice,
    p_info: *const c_void,
) -> u64 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void) -> u64;
    if let Some(sym) = get_native_proc_addr("vkGetBufferDeviceAddress") {
        let native_fn: NativeFn = std::mem::transmute(sym);
        native_fn(device, p_info)
    } else if let Some(sym) = get_native_proc_addr("vkGetBufferDeviceAddressKHR") {
        let native_fn: NativeFn = std::mem::transmute(sym);
        native_fn(device, p_info)
    } else {
        0
    }
}

pub unsafe extern "win64" fn guest_vkCmdBeginRendering(
    command_buffer: *mut c_void,
    p_rendering_info: *const c_void,
) {
    type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
    if let Some(sym) = get_native_proc_addr("vkCmdBeginRendering") {
        let native_fn: NativeFn = std::mem::transmute(sym);
        native_fn(command_buffer, p_rendering_info);
    } else if let Some(sym) = get_native_proc_addr("vkCmdBeginRenderingKHR") {
        let native_fn: NativeFn = std::mem::transmute(sym);
        native_fn(command_buffer, p_rendering_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdEndRendering(command_buffer: *mut c_void) {
    type NativeFn = unsafe extern "C" fn(*mut c_void);
    if let Some(sym) = get_native_proc_addr("vkCmdEndRendering") {
        let native_fn: NativeFn = std::mem::transmute(sym);
        native_fn(command_buffer);
    } else if let Some(sym) = get_native_proc_addr("vkCmdEndRenderingKHR") {
        let native_fn: NativeFn = std::mem::transmute(sym);
        native_fn(command_buffer);
    }
}

pub unsafe extern "win64" fn guest_vkBindBufferMemory(
    device: NativeVkDevice,
    buffer: u64,
    memory: u64,
    offset: u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u64, u64) -> i32;
    invoke_native!(
        "vkBindBufferMemory",
        NativeFn,
        device,
        buffer,
        memory,
        offset
    )
}

pub unsafe extern "win64" fn guest_vkBindBufferMemory2(
    device: NativeVkDevice,
    bind_count: u32,
    p_bind_infos: *const c_void,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkBindBufferMemory2") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u32, *const c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, bind_count, p_bind_infos)
    } else if let Some(sym) = get_native_proc_addr("vkBindBufferMemory2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u32, *const c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, bind_count, p_bind_infos)
    } else {
        VK_ERROR_INITIALIZATION_FAILED
    }
}

pub unsafe extern "win64" fn guest_vkBindImageMemory(
    device: NativeVkDevice,
    image: u64,
    memory: u64,
    offset: u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u64, u64) -> i32;
    invoke_native!("vkBindImageMemory", NativeFn, device, image, memory, offset)
}

pub unsafe extern "win64" fn guest_vkBindImageMemory2(
    device: NativeVkDevice,
    bind_count: u32,
    p_bind_infos: *const c_void,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkBindImageMemory2") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u32, *const c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, bind_count, p_bind_infos)
    } else if let Some(sym) = get_native_proc_addr("vkBindImageMemory2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u32, *const c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, bind_count, p_bind_infos)
    } else {
        VK_ERROR_INITIALIZATION_FAILED
    }
}

pub unsafe extern "win64" fn guest_vkGetImageSubresourceLayout(
    device: NativeVkDevice,
    image: u64,
    p_subresource: *const c_void,
    p_layout: *mut c_void,
) {
    invoke_native_void!(
        "vkGetImageSubresourceLayout",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void, *mut c_void),
        device,
        image,
        p_subresource,
        p_layout
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 11: Command Pools & Command Buffers
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCreateCommandPool(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_command_pool: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateCommandPool",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_command_pool
    )
}

pub unsafe extern "win64" fn guest_vkDestroyCommandPool(
    device: NativeVkDevice,
    command_pool: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyCommandPool",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        command_pool,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkResetCommandPool(
    device: NativeVkDevice,
    command_pool: u64,
    flags: u32,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u32) -> i32;
    invoke_native!("vkResetCommandPool", NativeFn, device, command_pool, flags)
}

pub unsafe extern "win64" fn guest_vkAllocateCommandBuffers(
    device: NativeVkDevice,
    p_allocate_info: *const c_void,
    p_command_buffers: *mut *mut c_void,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut *mut c_void) -> i32;
    invoke_native!(
        "vkAllocateCommandBuffers",
        NativeFn,
        device,
        p_allocate_info,
        p_command_buffers
    )
}

pub unsafe extern "win64" fn guest_vkFreeCommandBuffers(
    device: NativeVkDevice,
    command_pool: u64,
    count: u32,
    p_command_buffers: *const *mut c_void,
) {
    invoke_native_void!(
        "vkFreeCommandBuffers",
        unsafe extern "C" fn(NativeVkDevice, u64, u32, *const *mut c_void),
        device,
        command_pool,
        count,
        p_command_buffers
    );
}

pub unsafe extern "win64" fn guest_vkBeginCommandBuffer(
    command_buffer: *mut c_void,
    p_begin_info: *const c_void,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void) -> i32;
    invoke_native!(
        "vkBeginCommandBuffer",
        NativeFn,
        command_buffer,
        p_begin_info
    )
}

pub unsafe extern "win64" fn guest_vkEndCommandBuffer(command_buffer: *mut c_void) -> i32 {
    type NativeFn = unsafe extern "C" fn(*mut c_void) -> i32;
    invoke_native!("vkEndCommandBuffer", NativeFn, command_buffer)
}

pub unsafe extern "win64" fn guest_vkResetCommandBuffer(
    command_buffer: *mut c_void,
    flags: u32,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(*mut c_void, u32) -> i32;
    invoke_native!("vkResetCommandBuffer", NativeFn, command_buffer, flags)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 12: Render Pass & Framebuffer
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCreateRenderPass(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_render_pass: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateRenderPass",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_render_pass
    )
}

pub unsafe extern "win64" fn guest_vkCreateRenderPass2(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_render_pass: *mut u64,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkCreateRenderPass2") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_create_info, std::ptr::null(), p_render_pass)
    } else if let Some(sym) = get_native_proc_addr("vkCreateRenderPass2KHR") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_create_info, std::ptr::null(), p_render_pass)
    } else {
        VK_ERROR_INITIALIZATION_FAILED
    }
}

pub unsafe extern "win64" fn guest_vkDestroyRenderPass(
    device: NativeVkDevice,
    render_pass: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyRenderPass",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        render_pass,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkCreateFramebuffer(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_framebuffer: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateFramebuffer",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_framebuffer
    )
}

pub unsafe extern "win64" fn guest_vkDestroyFramebuffer(
    device: NativeVkDevice,
    framebuffer: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyFramebuffer",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        framebuffer,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkGetRenderAreaGranularity(
    device: NativeVkDevice,
    render_pass: u64,
    p_granularity: *mut c_void,
) {
    invoke_native_void!(
        "vkGetRenderAreaGranularity",
        unsafe extern "C" fn(NativeVkDevice, u64, *mut c_void),
        device,
        render_pass,
        p_granularity
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 13: Pipeline
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCreateShaderModule(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_shader_module: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateShaderModule",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_shader_module
    )
}

pub unsafe extern "win64" fn guest_vkDestroyShaderModule(
    device: NativeVkDevice,
    shader_module: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyShaderModule",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        shader_module,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkCreatePipelineCache(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_cache: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreatePipelineCache",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_cache
    )
}

pub unsafe extern "win64" fn guest_vkDestroyPipelineCache(
    device: NativeVkDevice,
    cache: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyPipelineCache",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        cache,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkGetPipelineCacheData(
    device: NativeVkDevice,
    cache: u64,
    p_data_size: *mut usize,
    p_data: *mut c_void,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, *mut usize, *mut c_void) -> i32;
    invoke_native!(
        "vkGetPipelineCacheData",
        NativeFn,
        device,
        cache,
        p_data_size,
        p_data
    )
}

pub unsafe extern "win64" fn guest_vkMergePipelineCaches(
    device: NativeVkDevice,
    dst: u64,
    count: u32,
    p_src_caches: *const u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u32, *const u64) -> i32;
    invoke_native!(
        "vkMergePipelineCaches",
        NativeFn,
        device,
        dst,
        count,
        p_src_caches
    )
}

#[no_mangle]
pub unsafe extern "win64" fn guest_vkCreatePipelineLayout(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_layout: *mut u64,
) -> i32 {
    if !p_create_info.is_null() {
        let s_type = *(p_create_info as *const u32);
        let p_next = *((p_create_info as usize + 8) as *const usize);
        let flags = *((p_create_info as usize + 16) as *const u32);
        let set_count = *((p_create_info as usize + 20) as *const u32);
        let p_sets = *((p_create_info as usize + 24) as *const usize);
        let push_count = *((p_create_info as usize + 32) as *const u32);
        let p_push = *((p_create_info as usize + 40) as *const usize);
        println!(
            "[VK BRIDGE] guest_vkCreatePipelineLayout device={:p}, sType={}, pNext=0x{:X}, flags=0x{:X}, setCount={}, pSets=0x{:X}, pushCount={}, pPush=0x{:X}, pLayout={:p}",
            device, s_type, p_next, flags, set_count, p_sets, push_count, p_push, p_layout
        );
        if p_sets != 0 && set_count > 0 {
            for i in 0..set_count.min(16) as usize {
                println!("  setLayout[{}] = 0x{:X}", i, *((p_sets + i * 8) as *const u64));
            }
        }
        if p_push != 0 && push_count > 0 {
            for i in 0..push_count.min(8) as usize {
                let range_ptr = p_push + i * 12;
                let stage_flags = *(range_ptr as *const u32);
                let offset = *((range_ptr + 4) as *const u32);
                let size = *((range_ptr + 8) as *const u32);
                println!(
                    "  pushConstant[{}] = stageFlags=0x{:X}, offset={}, size={}",
                    i, stage_flags, offset, size
                );
            }
        }
    }
    use std::io::Write;
    std::io::stdout().flush().ok();
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    let res = invoke_native!(
        "vkCreatePipelineLayout",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_layout
    );
    println!(
        "[VK BRIDGE] guest_vkCreatePipelineLayout returned {}, layout=0x{:X}",
        res,
        if !p_layout.is_null() { *p_layout } else { 0 }
    );
    std::io::stdout().flush().ok();
    res
}

pub unsafe extern "win64" fn guest_vkDestroyPipelineLayout(
    device: NativeVkDevice,
    layout: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyPipelineLayout",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        layout,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkCreateGraphicsPipelines(
    device: NativeVkDevice,
    pipeline_cache: u64,
    count: u32,
    p_create_infos: *const c_void,
    _p_allocator: *const c_void,
    p_pipelines: *mut u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(
        NativeVkDevice,
        u64,
        u32,
        *const c_void,
        *const c_void,
        *mut u64,
    ) -> i32;
    invoke_native!(
        "vkCreateGraphicsPipelines",
        NativeFn,
        device,
        pipeline_cache,
        count,
        p_create_infos,
        std::ptr::null(),
        p_pipelines
    )
}

pub unsafe extern "win64" fn guest_vkCreateComputePipelines(
    device: NativeVkDevice,
    pipeline_cache: u64,
    count: u32,
    p_create_infos: *const c_void,
    _p_allocator: *const c_void,
    p_pipelines: *mut u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(
        NativeVkDevice,
        u64,
        u32,
        *const c_void,
        *const c_void,
        *mut u64,
    ) -> i32;
    invoke_native!(
        "vkCreateComputePipelines",
        NativeFn,
        device,
        pipeline_cache,
        count,
        p_create_infos,
        std::ptr::null(),
        p_pipelines
    )
}

pub unsafe extern "win64" fn guest_vkDestroyPipeline(
    device: NativeVkDevice,
    pipeline: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyPipeline",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        pipeline,
        std::ptr::null()
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 14: Descriptors
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCreateDescriptorSetLayout(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_layout: *mut u64,
) -> i32 {
    if !p_create_info.is_null() {
        let s_type = *(p_create_info as *const u32);
        let p_next = *((p_create_info as usize + 8) as *const usize);
        let flags = *((p_create_info as usize + 16) as *const u32);
        let binding_count = *((p_create_info as usize + 20) as *const u32);
        let p_bindings = *((p_create_info as usize + 24) as *const usize);
        println!(
            "[VK BRIDGE] guest_vkCreateDescriptorSetLayout device={:p}, sType={}, pNext=0x{:X}, flags=0x{:X}, bindingCount={}, pBindings=0x{:X}",
            device, s_type, p_next, flags, binding_count, p_bindings
        );
        if p_bindings != 0 && binding_count > 0 {
            for i in 0..binding_count.min(8) as usize {
                let binding_ptr = p_bindings + i * 24;
                let binding_idx = *(binding_ptr as *const u32);
                let desc_type = *((binding_ptr + 4) as *const u32);
                let desc_count = *((binding_ptr + 8) as *const u32);
                let stage_flags = *((binding_ptr + 12) as *const u32);
                let imm_samplers = *((binding_ptr + 16) as *const usize);
                println!(
                    "  binding[{}] = binding_idx={}, type={}, count={}, stageFlags=0x{:X}, immSamplers=0x{:X}",
                    i, binding_idx, desc_type, desc_count, stage_flags, imm_samplers
                );
            }
        }
    }
    use std::io::Write;
    std::io::stdout().flush().ok();
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    let res = invoke_native!(
        "vkCreateDescriptorSetLayout",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_layout
    );
    println!(
        "[VK BRIDGE] guest_vkCreateDescriptorSetLayout returned {}, layout=0x{:X}",
        res,
        if !p_layout.is_null() { *p_layout } else { 0 }
    );
    std::io::stdout().flush().ok();
    res
}

pub unsafe extern "win64" fn guest_vkDestroyDescriptorSetLayout(
    device: NativeVkDevice,
    layout: u64,
    _p_allocator: *const c_void,
) {
    println!("[VK BRIDGE] guest_vkDestroyDescriptorSetLayout device={:p}, layout=0x{:X}", device, layout);
    use std::io::Write;
    std::io::stdout().flush().ok();
    invoke_native_void!(
        "vkDestroyDescriptorSetLayout",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        layout,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkCreateDescriptorPool(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_pool: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateDescriptorPool",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_pool
    )
}

pub unsafe extern "win64" fn guest_vkDestroyDescriptorPool(
    device: NativeVkDevice,
    pool: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyDescriptorPool",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        pool,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkResetDescriptorPool(
    device: NativeVkDevice,
    pool: u64,
    flags: u32,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u32) -> i32;
    invoke_native!("vkResetDescriptorPool", NativeFn, device, pool, flags)
}

pub unsafe extern "win64" fn guest_vkAllocateDescriptorSets(
    device: NativeVkDevice,
    p_allocate_info: *const c_void,
    p_descriptor_sets: *mut u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkAllocateDescriptorSets",
        NativeFn,
        device,
        p_allocate_info,
        p_descriptor_sets
    )
}

pub unsafe extern "win64" fn guest_vkFreeDescriptorSets(
    device: NativeVkDevice,
    pool: u64,
    count: u32,
    p_sets: *const u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u32, *const u64) -> i32;
    invoke_native!(
        "vkFreeDescriptorSets",
        NativeFn,
        device,
        pool,
        count,
        p_sets
    )
}

pub unsafe extern "win64" fn guest_vkUpdateDescriptorSets(
    device: NativeVkDevice,
    write_count: u32,
    p_writes: *const c_void,
    copy_count: u32,
    p_copies: *const c_void,
) {
    invoke_native_void!(
        "vkUpdateDescriptorSets",
        unsafe extern "C" fn(NativeVkDevice, u32, *const c_void, u32, *const c_void),
        device,
        write_count,
        p_writes,
        copy_count,
        p_copies
    );
}

pub unsafe extern "win64" fn guest_vkCreateDescriptorUpdateTemplate(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_template: *mut u64,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkCreateDescriptorUpdateTemplate") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_create_info, std::ptr::null(), p_template)
    } else if let Some(sym) = get_native_proc_addr("vkCreateDescriptorUpdateTemplateKHR") {
        type NativeFn =
            unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_create_info, std::ptr::null(), p_template)
    } else {
        VK_ERROR_INITIALIZATION_FAILED
    }
}

pub unsafe extern "win64" fn guest_vkDestroyDescriptorUpdateTemplate(
    device: NativeVkDevice,
    template: u64,
    _p_allocator: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkDestroyDescriptorUpdateTemplate") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, template, std::ptr::null());
    } else if let Some(sym) = get_native_proc_addr("vkDestroyDescriptorUpdateTemplateKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, template, std::ptr::null());
    }
}

pub unsafe extern "win64" fn guest_vkUpdateDescriptorSetWithTemplate(
    device: NativeVkDevice,
    descriptor_set: u64,
    template: u64,
    p_data: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkUpdateDescriptorSetWithTemplate") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u64, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, descriptor_set, template, p_data);
    } else if let Some(sym) = get_native_proc_addr("vkUpdateDescriptorSetWithTemplateKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u64, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, descriptor_set, template, p_data);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 15: Sampler & Query Pool
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCreateSampler(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_sampler: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateSampler",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_sampler
    )
}

pub unsafe extern "win64" fn guest_vkDestroySampler(
    device: NativeVkDevice,
    sampler: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroySampler",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        sampler,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkCreateQueryPool(
    device: NativeVkDevice,
    p_create_info: *const c_void,
    _p_allocator: *const c_void,
    p_pool: *mut u64,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, *const c_void, *const c_void, *mut u64) -> i32;
    invoke_native!(
        "vkCreateQueryPool",
        NativeFn,
        device,
        p_create_info,
        std::ptr::null(),
        p_pool
    )
}

pub unsafe extern "win64" fn guest_vkDestroyQueryPool(
    device: NativeVkDevice,
    pool: u64,
    _p_allocator: *const c_void,
) {
    invoke_native_void!(
        "vkDestroyQueryPool",
        unsafe extern "C" fn(NativeVkDevice, u64, *const c_void),
        device,
        pool,
        std::ptr::null()
    );
}

pub unsafe extern "win64" fn guest_vkGetQueryPoolResults(
    device: NativeVkDevice,
    pool: u64,
    first_query: u32,
    query_count: u32,
    data_size: usize,
    p_data: *mut c_void,
    stride: u64,
    flags: u32,
) -> i32 {
    type NativeFn =
        unsafe extern "C" fn(NativeVkDevice, u64, u32, u32, usize, *mut c_void, u64, u32) -> i32;
    invoke_native!(
        "vkGetQueryPoolResults",
        NativeFn,
        device,
        pool,
        first_query,
        query_count,
        data_size,
        p_data,
        stride,
        flags
    )
}

pub unsafe extern "win64" fn guest_vkResetQueryPool(
    device: NativeVkDevice,
    pool: u64,
    first: u32,
    count: u32,
) {
    if let Some(sym) = get_native_proc_addr("vkResetQueryPool") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u32, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, pool, first, count);
    } else if let Some(sym) = get_native_proc_addr("vkResetQueryPoolEXT") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u32, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, pool, first, count);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 16: Command Buffer Recording (Cmd* commands)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCmdBindPipeline(
    cb: *mut c_void,
    bind_point: u32,
    pipeline: u64,
) {
    invoke_native_void!(
        "vkCmdBindPipeline",
        unsafe extern "C" fn(*mut c_void, u32, u64),
        cb,
        bind_point,
        pipeline
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetViewport(
    cb: *mut c_void,
    first: u32,
    count: u32,
    p_viewports: *const c_void,
) {
    invoke_native_void!(
        "vkCmdSetViewport",
        unsafe extern "C" fn(*mut c_void, u32, u32, *const c_void),
        cb,
        first,
        count,
        p_viewports
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetScissor(
    cb: *mut c_void,
    first: u32,
    count: u32,
    p_scissors: *const c_void,
) {
    invoke_native_void!(
        "vkCmdSetScissor",
        unsafe extern "C" fn(*mut c_void, u32, u32, *const c_void),
        cb,
        first,
        count,
        p_scissors
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetLineWidth(cb: *mut c_void, line_width: f32) {
    invoke_native_void!(
        "vkCmdSetLineWidth",
        unsafe extern "C" fn(*mut c_void, f32),
        cb,
        line_width
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetDepthBias(
    cb: *mut c_void,
    constant: f32,
    clamp: f32,
    slope: f32,
) {
    invoke_native_void!(
        "vkCmdSetDepthBias",
        unsafe extern "C" fn(*mut c_void, f32, f32, f32),
        cb,
        constant,
        clamp,
        slope
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetDepthBias2EXT(
    cb: *mut c_void,
    p_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetDepthBias2EXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let native_fn: NativeFn = std::mem::transmute(sym);
        native_fn(cb, p_info);
    } else if !p_info.is_null() {
        let constant_factor = *((p_info as usize + 16) as *const f32);
        let clamp = *((p_info as usize + 20) as *const f32);
        let slope_factor = *((p_info as usize + 24) as *const f32);
        guest_vkCmdSetDepthBias(cb, constant_factor, clamp, slope_factor);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetBlendConstants(
    cb: *mut c_void,
    blend_constants: *const f32,
) {
    invoke_native_void!(
        "vkCmdSetBlendConstants",
        unsafe extern "C" fn(*mut c_void, *const f32),
        cb,
        blend_constants
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetDepthBounds(cb: *mut c_void, min: f32, max: f32) {
    invoke_native_void!(
        "vkCmdSetDepthBounds",
        unsafe extern "C" fn(*mut c_void, f32, f32),
        cb,
        min,
        max
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetStencilCompareMask(
    cb: *mut c_void,
    face_mask: u32,
    compare_mask: u32,
) {
    invoke_native_void!(
        "vkCmdSetStencilCompareMask",
        unsafe extern "C" fn(*mut c_void, u32, u32),
        cb,
        face_mask,
        compare_mask
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetStencilWriteMask(
    cb: *mut c_void,
    face_mask: u32,
    write_mask: u32,
) {
    invoke_native_void!(
        "vkCmdSetStencilWriteMask",
        unsafe extern "C" fn(*mut c_void, u32, u32),
        cb,
        face_mask,
        write_mask
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetStencilReference(
    cb: *mut c_void,
    face_mask: u32,
    reference: u32,
) {
    invoke_native_void!(
        "vkCmdSetStencilReference",
        unsafe extern "C" fn(*mut c_void, u32, u32),
        cb,
        face_mask,
        reference
    );
}

pub unsafe extern "win64" fn guest_vkCmdBindDescriptorSets(
    cb: *mut c_void,
    bind_point: u32,
    layout: u64,
    first_set: u32,
    count: u32,
    p_sets: *const u64,
    dyn_offset_count: u32,
    p_dyn_offsets: *const u32,
) {
    invoke_native_void!(
        "vkCmdBindDescriptorSets",
        unsafe extern "C" fn(*mut c_void, u32, u64, u32, u32, *const u64, u32, *const u32),
        cb,
        bind_point,
        layout,
        first_set,
        count,
        p_sets,
        dyn_offset_count,
        p_dyn_offsets
    );
}

pub unsafe extern "win64" fn guest_vkCmdBindIndexBuffer(
    cb: *mut c_void,
    buffer: u64,
    offset: u64,
    index_type: u32,
) {
    invoke_native_void!(
        "vkCmdBindIndexBuffer",
        unsafe extern "C" fn(*mut c_void, u64, u64, u32),
        cb,
        buffer,
        offset,
        index_type
    );
}

pub unsafe extern "win64" fn guest_vkCmdBindIndexBuffer2KHR(
    cb: *mut c_void,
    buffer: u64,
    offset: u64,
    size: u64,
    index_type: u32,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdBindIndexBuffer2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, u64, u64, u32);
        let native_fn: NativeFn = std::mem::transmute(sym);
        native_fn(cb, buffer, offset, size, index_type);
    } else {
        guest_vkCmdBindIndexBuffer(cb, buffer, offset, index_type);
    }
}

pub unsafe extern "win64" fn guest_vkGetRenderingAreaGranularityKHR(
    device: NativeVkDevice,
    p_rendering_area_info: *const c_void,
    p_granularity: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetRenderingAreaGranularityKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_rendering_area_info, p_granularity);
    } else if !p_granularity.is_null() {
        *(p_granularity as *mut u32) = 1;
        *((p_granularity as usize + 4) as *mut u32) = 1;
    }
}

pub unsafe extern "win64" fn guest_vkGetDeviceImageSubresourceLayoutKHR(
    device: NativeVkDevice,
    p_info: *const c_void,
    p_layout: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetDeviceImageSubresourceLayoutKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_info, p_layout);
    }
}

pub unsafe extern "win64" fn guest_vkGetImageSubresourceLayout2KHR(
    device: NativeVkDevice,
    image: u64,
    p_subresource: *const c_void,
    p_layout: *mut c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkGetImageSubresourceLayout2KHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, *const c_void, *mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, image, p_subresource, p_layout);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetSampleLocationsEXT(
    cb: *mut c_void,
    p_sample_locations_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetSampleLocationsEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_sample_locations_info);
    }
}

pub unsafe extern "win64" fn guest_vkGetCalibratedTimestampsKHR(
    device: NativeVkDevice,
    timestamp_count: u32,
    p_timestamp_infos: *const c_void,
    p_timestamps: *mut u64,
    p_max_deviation: *mut u64,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkGetCalibratedTimestampsKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u32, *const c_void, *mut u64, *mut u64) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, timestamp_count, p_timestamp_infos, p_timestamps, p_max_deviation)
    } else {
        VK_SUCCESS
    }
}

pub unsafe extern "win64" fn guest_vkCmdBindVertexBuffers(
    cb: *mut c_void,
    first: u32,
    count: u32,
    p_buffers: *const u64,
    p_offsets: *const u64,
) {
    invoke_native_void!(
        "vkCmdBindVertexBuffers",
        unsafe extern "C" fn(*mut c_void, u32, u32, *const u64, *const u64),
        cb,
        first,
        count,
        p_buffers,
        p_offsets
    );
}

pub unsafe extern "win64" fn guest_vkCmdDraw(
    cb: *mut c_void,
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
) {
    invoke_native_void!(
        "vkCmdDraw",
        unsafe extern "C" fn(*mut c_void, u32, u32, u32, u32),
        cb,
        vertex_count,
        instance_count,
        first_vertex,
        first_instance
    );
}

pub unsafe extern "win64" fn guest_vkCmdDrawIndexed(
    cb: *mut c_void,
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    vertex_offset: i32,
    first_instance: u32,
) {
    invoke_native_void!(
        "vkCmdDrawIndexed",
        unsafe extern "C" fn(*mut c_void, u32, u32, u32, i32, u32),
        cb,
        index_count,
        instance_count,
        first_index,
        vertex_offset,
        first_instance
    );
}

pub unsafe extern "win64" fn guest_vkCmdDrawIndirect(
    cb: *mut c_void,
    buffer: u64,
    offset: u64,
    draw_count: u32,
    stride: u32,
) {
    invoke_native_void!(
        "vkCmdDrawIndirect",
        unsafe extern "C" fn(*mut c_void, u64, u64, u32, u32),
        cb,
        buffer,
        offset,
        draw_count,
        stride
    );
}

pub unsafe extern "win64" fn guest_vkCmdDrawIndexedIndirect(
    cb: *mut c_void,
    buffer: u64,
    offset: u64,
    draw_count: u32,
    stride: u32,
) {
    invoke_native_void!(
        "vkCmdDrawIndexedIndirect",
        unsafe extern "C" fn(*mut c_void, u64, u64, u32, u32),
        cb,
        buffer,
        offset,
        draw_count,
        stride
    );
}

pub unsafe extern "win64" fn guest_vkCmdDrawIndirectCount(
    cb: *mut c_void,
    buffer: u64,
    offset: u64,
    count_buffer: u64,
    count_offset: u64,
    max_draw_count: u32,
    stride: u32,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdDrawIndirectCount") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, u32, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(
            cb,
            buffer,
            offset,
            count_buffer,
            count_offset,
            max_draw_count,
            stride,
        );
    } else if let Some(sym) = get_native_proc_addr("vkCmdDrawIndirectCountKHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, u32, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(
            cb,
            buffer,
            offset,
            count_buffer,
            count_offset,
            max_draw_count,
            stride,
        );
    }
}

pub unsafe extern "win64" fn guest_vkCmdDrawIndexedIndirectCount(
    cb: *mut c_void,
    buffer: u64,
    offset: u64,
    count_buffer: u64,
    count_offset: u64,
    max_draw_count: u32,
    stride: u32,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdDrawIndexedIndirectCount") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, u32, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(
            cb,
            buffer,
            offset,
            count_buffer,
            count_offset,
            max_draw_count,
            stride,
        );
    } else if let Some(sym) = get_native_proc_addr("vkCmdDrawIndexedIndirectCountKHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, u32, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(
            cb,
            buffer,
            offset,
            count_buffer,
            count_offset,
            max_draw_count,
            stride,
        );
    }
}

pub unsafe extern "win64" fn guest_vkCmdDispatch(cb: *mut c_void, x: u32, y: u32, z: u32) {
    invoke_native_void!(
        "vkCmdDispatch",
        unsafe extern "C" fn(*mut c_void, u32, u32, u32),
        cb,
        x,
        y,
        z
    );
}

pub unsafe extern "win64" fn guest_vkCmdDispatchIndirect(
    cb: *mut c_void,
    buffer: u64,
    offset: u64,
) {
    invoke_native_void!(
        "vkCmdDispatchIndirect",
        unsafe extern "C" fn(*mut c_void, u64, u64),
        cb,
        buffer,
        offset
    );
}

pub unsafe extern "win64" fn guest_vkCmdCopyBuffer(
    cb: *mut c_void,
    src: u64,
    dst: u64,
    count: u32,
    p_regions: *const c_void,
) {
    invoke_native_void!(
        "vkCmdCopyBuffer",
        unsafe extern "C" fn(*mut c_void, u64, u64, u32, *const c_void),
        cb,
        src,
        dst,
        count,
        p_regions
    );
}

pub unsafe extern "win64" fn guest_vkCmdCopyImage(
    cb: *mut c_void,
    src: u64,
    src_layout: u32,
    dst: u64,
    dst_layout: u32,
    count: u32,
    p_regions: *const c_void,
) {
    invoke_native_void!(
        "vkCmdCopyImage",
        unsafe extern "C" fn(*mut c_void, u64, u32, u64, u32, u32, *const c_void),
        cb,
        src,
        src_layout,
        dst,
        dst_layout,
        count,
        p_regions
    );
}

pub unsafe extern "win64" fn guest_vkCmdBlitImage(
    cb: *mut c_void,
    src: u64,
    src_layout: u32,
    dst: u64,
    dst_layout: u32,
    count: u32,
    p_regions: *const c_void,
    filter: u32,
) {
    invoke_native_void!(
        "vkCmdBlitImage",
        unsafe extern "C" fn(*mut c_void, u64, u32, u64, u32, u32, *const c_void, u32),
        cb,
        src,
        src_layout,
        dst,
        dst_layout,
        count,
        p_regions,
        filter
    );
}

pub unsafe extern "win64" fn guest_vkCmdCopyBufferToImage(
    cb: *mut c_void,
    src: u64,
    dst: u64,
    dst_layout: u32,
    count: u32,
    p_regions: *const c_void,
) {
    invoke_native_void!(
        "vkCmdCopyBufferToImage",
        unsafe extern "C" fn(*mut c_void, u64, u64, u32, u32, *const c_void),
        cb,
        src,
        dst,
        dst_layout,
        count,
        p_regions
    );
}

pub unsafe extern "win64" fn guest_vkCmdCopyImageToBuffer(
    cb: *mut c_void,
    src: u64,
    src_layout: u32,
    dst: u64,
    count: u32,
    p_regions: *const c_void,
) {
    invoke_native_void!(
        "vkCmdCopyImageToBuffer",
        unsafe extern "C" fn(*mut c_void, u64, u32, u64, u32, *const c_void),
        cb,
        src,
        src_layout,
        dst,
        count,
        p_regions
    );
}

pub unsafe extern "win64" fn guest_vkCmdUpdateBuffer(
    cb: *mut c_void,
    dst: u64,
    offset: u64,
    size: u64,
    p_data: *const c_void,
) {
    invoke_native_void!(
        "vkCmdUpdateBuffer",
        unsafe extern "C" fn(*mut c_void, u64, u64, u64, *const c_void),
        cb,
        dst,
        offset,
        size,
        p_data
    );
}

pub unsafe extern "win64" fn guest_vkCmdFillBuffer(
    cb: *mut c_void,
    dst: u64,
    offset: u64,
    size: u64,
    data: u32,
) {
    invoke_native_void!(
        "vkCmdFillBuffer",
        unsafe extern "C" fn(*mut c_void, u64, u64, u64, u32),
        cb,
        dst,
        offset,
        size,
        data
    );
}

pub unsafe extern "win64" fn guest_vkCmdCopyBuffer2(
    cb: *mut c_void,
    p_copy_buffer_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdCopyBuffer2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_copy_buffer_info);
    } else if let Some(sym) = get_native_proc_addr("vkCmdCopyBuffer2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_copy_buffer_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdCopyImage2(
    cb: *mut c_void,
    p_copy_image_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdCopyImage2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_copy_image_info);
    } else if let Some(sym) = get_native_proc_addr("vkCmdCopyImage2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_copy_image_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdBlitImage2(
    cb: *mut c_void,
    p_blit_image_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdBlitImage2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_blit_image_info);
    } else if let Some(sym) = get_native_proc_addr("vkCmdBlitImage2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_blit_image_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdCopyBufferToImage2(
    cb: *mut c_void,
    p_copy_buffer_to_image_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdCopyBufferToImage2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_copy_buffer_to_image_info);
    } else if let Some(sym) = get_native_proc_addr("vkCmdCopyBufferToImage2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_copy_buffer_to_image_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdCopyImageToBuffer2(
    cb: *mut c_void,
    p_copy_image_to_buffer_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdCopyImageToBuffer2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_copy_image_to_buffer_info);
    } else if let Some(sym) = get_native_proc_addr("vkCmdCopyImageToBuffer2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_copy_image_to_buffer_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdResolveImage2(
    cb: *mut c_void,
    p_resolve_image_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdResolveImage2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_resolve_image_info);
    } else if let Some(sym) = get_native_proc_addr("vkCmdResolveImage2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_resolve_image_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetEvent2(
    cb: *mut c_void,
    event: u64,
    p_dependency_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetEvent2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, event, p_dependency_info);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetEvent2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, event, p_dependency_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdResetEvent2(
    cb: *mut c_void,
    event: u64,
    stage_mask: u64,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdResetEvent2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, u64);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, event, stage_mask);
    } else if let Some(sym) = get_native_proc_addr("vkCmdResetEvent2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, u64);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, event, stage_mask);
    }
}

pub unsafe extern "win64" fn guest_vkCmdWaitEvents2(
    cb: *mut c_void,
    event_count: u32,
    p_events: *const u64,
    p_dependency_infos: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdWaitEvents2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const u64, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, event_count, p_events, p_dependency_infos);
    } else if let Some(sym) = get_native_proc_addr("vkCmdWaitEvents2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const u64, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, event_count, p_events, p_dependency_infos);
    }
}

pub unsafe extern "win64" fn guest_vkCmdWriteTimestamp2(
    cb: *mut c_void,
    stage: u64,
    query_pool: u64,
    query: u32,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdWriteTimestamp2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, u64, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, stage, query_pool, query);
    } else if let Some(sym) = get_native_proc_addr("vkCmdWriteTimestamp2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u64, u64, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, stage, query_pool, query);
    }
}

pub unsafe extern "win64" fn guest_vkCmdBindVertexBuffers2(
    cb: *mut c_void,
    first_binding: u32,
    binding_count: u32,
    p_buffers: *const u64,
    p_offsets: *const u64,
    p_sizes: *const u64,
    p_strides: *const u64,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdBindVertexBuffers2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, u32, *const u64, *const u64, *const u64, *const u64);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, first_binding, binding_count, p_buffers, p_offsets, p_sizes, p_strides);
    } else if let Some(sym) = get_native_proc_addr("vkCmdBindVertexBuffers2EXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, u32, *const u64, *const u64, *const u64, *const u64);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, first_binding, binding_count, p_buffers, p_offsets, p_sizes, p_strides);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetScissorWithCount(
    cb: *mut c_void,
    scissor_count: u32,
    p_scissors: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetScissorWithCount") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, scissor_count, p_scissors);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetScissorWithCountEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, scissor_count, p_scissors);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetViewportWithCount(
    cb: *mut c_void,
    viewport_count: u32,
    p_viewports: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetViewportWithCount") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, viewport_count, p_viewports);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetViewportWithCountEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, viewport_count, p_viewports);
    }
}

pub unsafe extern "win64" fn guest_vkCmdClearColorImage(
    cb: *mut c_void,
    image: u64,
    layout: u32,
    p_color: *const c_void,
    count: u32,
    p_ranges: *const c_void,
) {
    invoke_native_void!(
        "vkCmdClearColorImage",
        unsafe extern "C" fn(*mut c_void, u64, u32, *const c_void, u32, *const c_void),
        cb,
        image,
        layout,
        p_color,
        count,
        p_ranges
    );
}

pub unsafe extern "win64" fn guest_vkCmdClearDepthStencilImage(
    cb: *mut c_void,
    image: u64,
    layout: u32,
    p_depth_stencil: *const c_void,
    count: u32,
    p_ranges: *const c_void,
) {
    invoke_native_void!(
        "vkCmdClearDepthStencilImage",
        unsafe extern "C" fn(*mut c_void, u64, u32, *const c_void, u32, *const c_void),
        cb,
        image,
        layout,
        p_depth_stencil,
        count,
        p_ranges
    );
}

pub unsafe extern "win64" fn guest_vkCmdClearAttachments(
    cb: *mut c_void,
    count: u32,
    p_attachments: *const c_void,
    rect_count: u32,
    p_rects: *const c_void,
) {
    invoke_native_void!(
        "vkCmdClearAttachments",
        unsafe extern "C" fn(*mut c_void, u32, *const c_void, u32, *const c_void),
        cb,
        count,
        p_attachments,
        rect_count,
        p_rects
    );
}

pub unsafe extern "win64" fn guest_vkCmdResolveImage(
    cb: *mut c_void,
    src: u64,
    src_layout: u32,
    dst: u64,
    dst_layout: u32,
    count: u32,
    p_regions: *const c_void,
) {
    invoke_native_void!(
        "vkCmdResolveImage",
        unsafe extern "C" fn(*mut c_void, u64, u32, u64, u32, u32, *const c_void),
        cb,
        src,
        src_layout,
        dst,
        dst_layout,
        count,
        p_regions
    );
}

pub unsafe extern "win64" fn guest_vkCmdBeginRenderPass(
    cb: *mut c_void,
    p_begin_info: *const c_void,
    contents: u32,
) {
    invoke_native_void!(
        "vkCmdBeginRenderPass",
        unsafe extern "C" fn(*mut c_void, *const c_void, u32),
        cb,
        p_begin_info,
        contents
    );
}

pub unsafe extern "win64" fn guest_vkCmdNextSubpass(cb: *mut c_void, contents: u32) {
    invoke_native_void!(
        "vkCmdNextSubpass",
        unsafe extern "C" fn(*mut c_void, u32),
        cb,
        contents
    );
}

pub unsafe extern "win64" fn guest_vkCmdEndRenderPass(cb: *mut c_void) {
    invoke_native_void!("vkCmdEndRenderPass", unsafe extern "C" fn(*mut c_void), cb);
}

pub unsafe extern "win64" fn guest_vkCmdBeginRenderPass2(
    cb: *mut c_void,
    p_begin_info: *const c_void,
    p_subpass_begin: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdBeginRenderPass2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_begin_info, p_subpass_begin);
    } else if let Some(sym) = get_native_proc_addr("vkCmdBeginRenderPass2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_begin_info, p_subpass_begin);
    }
}

pub unsafe extern "win64" fn guest_vkCmdNextSubpass2(
    cb: *mut c_void,
    p_subpass_begin: *const c_void,
    p_subpass_end: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdNextSubpass2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_subpass_begin, p_subpass_end);
    } else if let Some(sym) = get_native_proc_addr("vkCmdNextSubpass2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_subpass_begin, p_subpass_end);
    }
}

pub unsafe extern "win64" fn guest_vkCmdEndRenderPass2(
    cb: *mut c_void,
    p_subpass_end: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdEndRenderPass2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_subpass_end);
    } else if let Some(sym) = get_native_proc_addr("vkCmdEndRenderPass2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_subpass_end);
    }
}

pub unsafe extern "win64" fn guest_vkCmdPipelineBarrier(
    cb: *mut c_void,
    src_stage: u32,
    dst_stage: u32,
    dep_flags: u32,
    mem_barrier_count: u32,
    p_mem_barriers: *const c_void,
    buf_barrier_count: u32,
    p_buf_barriers: *const c_void,
    img_barrier_count: u32,
    p_img_barriers: *const c_void,
) {
    invoke_native_void!(
        "vkCmdPipelineBarrier",
        unsafe extern "C" fn(
            *mut c_void,
            u32,
            u32,
            u32,
            u32,
            *const c_void,
            u32,
            *const c_void,
            u32,
            *const c_void,
        ),
        cb,
        src_stage,
        dst_stage,
        dep_flags,
        mem_barrier_count,
        p_mem_barriers,
        buf_barrier_count,
        p_buf_barriers,
        img_barrier_count,
        p_img_barriers
    );
}

pub unsafe extern "win64" fn guest_vkCmdPipelineBarrier2(
    cb: *mut c_void,
    p_dependency_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdPipelineBarrier2") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_dependency_info);
    } else if let Some(sym) = get_native_proc_addr("vkCmdPipelineBarrier2KHR") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_dependency_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdPushConstants(
    cb: *mut c_void,
    layout: u64,
    stage_flags: u32,
    offset: u32,
    size: u32,
    p_values: *const c_void,
) {
    invoke_native_void!(
        "vkCmdPushConstants",
        unsafe extern "C" fn(*mut c_void, u64, u32, u32, u32, *const c_void),
        cb,
        layout,
        stage_flags,
        offset,
        size,
        p_values
    );
}

pub unsafe extern "win64" fn guest_vkCmdExecuteCommands(
    cb: *mut c_void,
    count: u32,
    p_command_buffers: *const *mut c_void,
) {
    invoke_native_void!(
        "vkCmdExecuteCommands",
        unsafe extern "C" fn(*mut c_void, u32, *const *mut c_void),
        cb,
        count,
        p_command_buffers
    );
}

pub unsafe extern "win64" fn guest_vkCmdBeginQuery(
    cb: *mut c_void,
    pool: u64,
    query: u32,
    flags: u32,
) {
    invoke_native_void!(
        "vkCmdBeginQuery",
        unsafe extern "C" fn(*mut c_void, u64, u32, u32),
        cb,
        pool,
        query,
        flags
    );
}

pub unsafe extern "win64" fn guest_vkCmdEndQuery(cb: *mut c_void, pool: u64, query: u32) {
    invoke_native_void!(
        "vkCmdEndQuery",
        unsafe extern "C" fn(*mut c_void, u64, u32),
        cb,
        pool,
        query
    );
}

pub unsafe extern "win64" fn guest_vkCmdResetQueryPool(
    cb: *mut c_void,
    pool: u64,
    first: u32,
    count: u32,
) {
    invoke_native_void!(
        "vkCmdResetQueryPool",
        unsafe extern "C" fn(*mut c_void, u64, u32, u32),
        cb,
        pool,
        first,
        count
    );
}

pub unsafe extern "win64" fn guest_vkCmdWriteTimestamp(
    cb: *mut c_void,
    stage: u32,
    pool: u64,
    query: u32,
) {
    invoke_native_void!(
        "vkCmdWriteTimestamp",
        unsafe extern "C" fn(*mut c_void, u32, u64, u32),
        cb,
        stage,
        pool,
        query
    );
}

pub unsafe extern "win64" fn guest_vkCmdCopyQueryPoolResults(
    cb: *mut c_void,
    pool: u64,
    first: u32,
    count: u32,
    dst: u64,
    offset: u64,
    stride: u64,
    flags: u32,
) {
    invoke_native_void!(
        "vkCmdCopyQueryPoolResults",
        unsafe extern "C" fn(*mut c_void, u64, u32, u32, u64, u64, u64, u32),
        cb,
        pool,
        first,
        count,
        dst,
        offset,
        stride,
        flags
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetEvent(cb: *mut c_void, event: u64, stage_mask: u32) {
    invoke_native_void!(
        "vkCmdSetEvent",
        unsafe extern "C" fn(*mut c_void, u64, u32),
        cb,
        event,
        stage_mask
    );
}

pub unsafe extern "win64" fn guest_vkCmdResetEvent(cb: *mut c_void, event: u64, stage_mask: u32) {
    invoke_native_void!(
        "vkCmdResetEvent",
        unsafe extern "C" fn(*mut c_void, u64, u32),
        cb,
        event,
        stage_mask
    );
}

pub unsafe extern "win64" fn guest_vkCmdWaitEvents(
    cb: *mut c_void,
    event_count: u32,
    p_events: *const u64,
    src_stage: u32,
    dst_stage: u32,
    mem_count: u32,
    p_mem: *const c_void,
    buf_count: u32,
    p_buf: *const c_void,
    img_count: u32,
    p_img: *const c_void,
) {
    invoke_native_void!(
        "vkCmdWaitEvents",
        unsafe extern "C" fn(
            *mut c_void,
            u32,
            *const u64,
            u32,
            u32,
            u32,
            *const c_void,
            u32,
            *const c_void,
            u32,
            *const c_void,
        ),
        cb,
        event_count,
        p_events,
        src_stage,
        dst_stage,
        mem_count,
        p_mem,
        buf_count,
        p_buf,
        img_count,
        p_img
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 17: Utility / Misc device commands used by DXVK
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkTrimCommandPool(
    device: NativeVkDevice,
    pool: u64,
    flags: u32,
) {
    if let Some(sym) = get_native_proc_addr("vkTrimCommandPool") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, pool, flags);
    } else if let Some(sym) = get_native_proc_addr("vkTrimCommandPoolKHR") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, pool, flags);
    }
}

pub unsafe extern "win64" fn guest_vkGetDeviceMemoryCommitment(
    device: NativeVkDevice,
    memory: u64,
    p_committed: *mut u64,
) {
    invoke_native_void!(
        "vkGetDeviceMemoryCommitment",
        unsafe extern "C" fn(NativeVkDevice, u64, *mut u64),
        device,
        memory,
        p_committed
    );
}

pub unsafe extern "win64" fn guest_vkSetDeviceMemoryPriorityEXT(
    device: NativeVkDevice,
    memory: u64,
    priority: f32,
) {
    if let Some(sym) = get_native_proc_addr("vkSetDeviceMemoryPriorityEXT") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64, f32);
        let f: NativeFn = std::mem::transmute(sym);
        f(device, memory, priority);
    }
}

pub unsafe extern "win64" fn guest_vkQueueBindSparse(
    queue: *mut c_void,
    bind_info_count: u32,
    p_bind_info: *const c_void,
    fence: u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void, u64) -> i32;
    invoke_native!(
        "vkQueueBindSparse",
        NativeFn,
        queue,
        bind_info_count,
        p_bind_info,
        fence
    )
}

pub unsafe extern "win64" fn guest_vkGetFenceStatus(device: NativeVkDevice, fence: u64) -> i32 {
    type NativeFn = unsafe extern "C" fn(NativeVkDevice, u64) -> i32;
    invoke_native!("vkGetFenceStatus", NativeFn, device, fence)
}

pub unsafe extern "win64" fn guest_vkCmdBindTransformFeedbackBuffersEXT(
    cb: *mut c_void,
    first: u32,
    count: u32,
    p_buffers: *const u64,
    p_offsets: *const u64,
    p_sizes: *const u64,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdBindTransformFeedbackBuffersEXT") {
        type NativeFn =
            unsafe extern "C" fn(*mut c_void, u32, u32, *const u64, *const u64, *const u64);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, first, count, p_buffers, p_offsets, p_sizes);
    }
}

pub unsafe extern "win64" fn guest_vkCmdBeginTransformFeedbackEXT(
    cb: *mut c_void,
    first: u32,
    count: u32,
    p_buffers: *const u64,
    p_offsets: *const u64,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdBeginTransformFeedbackEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, u32, *const u64, *const u64);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, first, count, p_buffers, p_offsets);
    }
}

pub unsafe extern "win64" fn guest_vkCmdEndTransformFeedbackEXT(
    cb: *mut c_void,
    first: u32,
    count: u32,
    p_buffers: *const u64,
    p_offsets: *const u64,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdEndTransformFeedbackEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, u32, *const u64, *const u64);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, first, count, p_buffers, p_offsets);
    }
}

pub unsafe extern "win64" fn guest_vkCmdBeginConditionalRenderingEXT(
    cb: *mut c_void,
    p_info: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdBeginConditionalRenderingEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_info);
    }
}

pub unsafe extern "win64" fn guest_vkCmdEndConditionalRenderingEXT(cb: *mut c_void) {
    if let Some(sym) = get_native_proc_addr("vkCmdEndConditionalRenderingEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetCullMode(cb: *mut c_void, cull_mode: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetCullMode") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, cull_mode);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetCullModeEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, cull_mode);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetFrontFace(cb: *mut c_void, front_face: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetFrontFace") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, front_face);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetFrontFaceEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, front_face);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetDepthTestEnable(cb: *mut c_void, enable: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetDepthTestEnable") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetDepthTestEnableEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetDepthWriteEnable(cb: *mut c_void, enable: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetDepthWriteEnable") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetDepthWriteEnableEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetDepthCompareOp(cb: *mut c_void, compare_op: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetDepthCompareOp") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, compare_op);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetDepthCompareOpEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, compare_op);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetStencilTestEnable(cb: *mut c_void, enable: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetStencilTestEnable") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetStencilTestEnableEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetStencilOp(
    cb: *mut c_void,
    face_mask: u32,
    fail_op: u32,
    pass_op: u32,
    depth_fail_op: u32,
    compare_op: u32,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetStencilOp") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, u32, u32, u32, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, face_mask, fail_op, pass_op, depth_fail_op, compare_op);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetStencilOpEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, u32, u32, u32, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, face_mask, fail_op, pass_op, depth_fail_op, compare_op);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetDepthBoundsTestEnable(cb: *mut c_void, enable: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetDepthBoundsTestEnable") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetDepthBoundsTestEnableEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetPrimitiveTopology(cb: *mut c_void, topology: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetPrimitiveTopology") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, topology);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetPrimitiveTopologyEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, topology);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetPrimitiveRestartEnable(cb: *mut c_void, enable: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetPrimitiveRestartEnable") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetPrimitiveRestartEnableEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetRasterizerDiscardEnable(cb: *mut c_void, enable: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetRasterizerDiscardEnable") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetRasterizerDiscardEnableEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetDepthBiasEnable(cb: *mut c_void, enable: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetDepthBiasEnable") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    } else if let Some(sym) = get_native_proc_addr("vkCmdSetDepthBiasEnableEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, enable);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetLogicOpEXT(cb: *mut c_void, logic_op: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetLogicOpEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, logic_op);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetPatchControlPointsEXT(cb: *mut c_void, count: u32) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetPatchControlPointsEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, count);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetColorWriteEnableEXT(
    cb: *mut c_void,
    count: u32,
    p_enables: *const u32,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetColorWriteEnableEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const u32);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, count, p_enables);
    }
}

pub unsafe extern "win64" fn guest_vkCmdSetVertexInputEXT(
    cb: *mut c_void,
    binding_count: u32,
    p_bindings: *const c_void,
    attr_count: u32,
    p_attrs: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdSetVertexInputEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void, u32, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, binding_count, p_bindings, attr_count, p_attrs);
    }
}

// Debug Utils (used by DXVK for object naming)
pub unsafe extern "win64" fn guest_vkSetDebugUtilsObjectNameEXT(
    device: NativeVkDevice,
    p_name_info: *const c_void,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkSetDebugUtilsObjectNameEXT") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_name_info)
    } else {
        VK_SUCCESS // Silently succeed if not available
    }
}

pub unsafe extern "win64" fn guest_vkCmdBeginDebugUtilsLabelEXT(
    cb: *mut c_void,
    p_label: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdBeginDebugUtilsLabelEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_label);
    }
}

pub unsafe extern "win64" fn guest_vkCmdEndDebugUtilsLabelEXT(cb: *mut c_void) {
    if let Some(sym) = get_native_proc_addr("vkCmdEndDebugUtilsLabelEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb);
    }
}

pub unsafe extern "win64" fn guest_vkCmdInsertDebugUtilsLabelEXT(
    cb: *mut c_void,
    p_label: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkCmdInsertDebugUtilsLabelEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(cb, p_label);
    }
}

pub unsafe extern "win64" fn guest_vkSetDebugUtilsObjectTagEXT(
    device: NativeVkDevice,
    p_tag_info: *const c_void,
) -> i32 {
    if let Some(sym) = get_native_proc_addr("vkSetDebugUtilsObjectTagEXT") {
        type NativeFn = unsafe extern "C" fn(NativeVkDevice, *const c_void) -> i32;
        let f: NativeFn = std::mem::transmute(sym);
        f(device, p_tag_info)
    } else {
        VK_SUCCESS
    }
}

pub unsafe extern "win64" fn guest_vkQueueBeginDebugUtilsLabelEXT(
    queue: *mut c_void,
    p_label: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkQueueBeginDebugUtilsLabelEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(queue, p_label);
    }
}

pub unsafe extern "win64" fn guest_vkQueueEndDebugUtilsLabelEXT(queue: *mut c_void) {
    if let Some(sym) = get_native_proc_addr("vkQueueEndDebugUtilsLabelEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(queue);
    }
}

pub unsafe extern "win64" fn guest_vkQueueInsertDebugUtilsLabelEXT(
    queue: *mut c_void,
    p_label: *const c_void,
) {
    if let Some(sym) = get_native_proc_addr("vkQueueInsertDebugUtilsLabelEXT") {
        type NativeFn = unsafe extern "C" fn(*mut c_void, *const c_void);
        let f: NativeFn = std::mem::transmute(sym);
        f(queue, p_label);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Section 32: Maintenance6, Present Wait & Descriptor Buffers
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub unsafe extern "win64" fn guest_vkCmdBindDescriptorSets2KHR(
    cb: *mut c_void,
    p_info: *const c_void,
) {
    invoke_native_void!(
        "vkCmdBindDescriptorSets2KHR",
        unsafe extern "C" fn(*mut c_void, *const c_void),
        cb,
        p_info
    );
}

pub unsafe extern "win64" fn guest_vkCmdPushConstants2KHR(
    cb: *mut c_void,
    p_info: *const c_void,
) {
    invoke_native_void!(
        "vkCmdPushConstants2KHR",
        unsafe extern "C" fn(*mut c_void, *const c_void),
        cb,
        p_info
    );
}

pub unsafe extern "win64" fn guest_vkCmdPushDescriptorSet2KHR(
    cb: *mut c_void,
    p_info: *const c_void,
) {
    invoke_native_void!(
        "vkCmdPushDescriptorSet2KHR",
        unsafe extern "C" fn(*mut c_void, *const c_void),
        cb,
        p_info
    );
}

pub unsafe extern "win64" fn guest_vkCmdPushDescriptorSetWithTemplate2KHR(
    cb: *mut c_void,
    p_info: *const c_void,
) {
    invoke_native_void!(
        "vkCmdPushDescriptorSetWithTemplate2KHR",
        unsafe extern "C" fn(*mut c_void, *const c_void),
        cb,
        p_info
    );
}

pub unsafe extern "win64" fn guest_vkCmdSetDescriptorBufferOffsets2EXT(
    cb: *mut c_void,
    p_info: *const c_void,
) {
    invoke_native_void!(
        "vkCmdSetDescriptorBufferOffsets2EXT",
        unsafe extern "C" fn(*mut c_void, *const c_void),
        cb,
        p_info
    );
}

pub unsafe extern "win64" fn guest_vkCmdBindDescriptorBufferEmbeddedSamplers2EXT(
    cb: *mut c_void,
    p_info: *const c_void,
) {
    invoke_native_void!(
        "vkCmdBindDescriptorBufferEmbeddedSamplers2EXT",
        unsafe extern "C" fn(*mut c_void, *const c_void),
        cb,
        p_info
    );
}

pub unsafe extern "win64" fn guest_vkWaitForPresentKHR(
    device: NativeVkDevice,
    swapchain: u64,
    present_id: u64,
    timeout: u64,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(
        NativeVkDevice,
        u64,
        u64,
        u64,
    ) -> i32;
    invoke_native!(
        "vkWaitForPresentKHR",
        NativeFn,
        device,
        swapchain,
        present_id,
        timeout
    )
}

pub unsafe extern "win64" fn guest_vkWaitForPresent2KHR(
    device: NativeVkDevice,
    swapchain: u64,
    p_info: *const c_void,
) -> i32 {
    type NativeFn = unsafe extern "C" fn(
        NativeVkDevice,
        u64,
        *const c_void,
    ) -> i32;
    invoke_native!(
        "vkWaitForPresent2KHR",
        NativeFn,
        device,
        swapchain,
        p_info
    )
}
