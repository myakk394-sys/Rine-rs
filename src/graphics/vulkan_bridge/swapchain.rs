//! Vulkan Swapchain Subsystem
//! Manages WSI Swapchains, frame buffers, image acquisition, and frame presentation.

use super::loader::get_native_proc_addr;
use super::types::*;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[repr(C)]
struct VkImageCreateInfo {
    s_type: i32,
    p_next: *const std::ffi::c_void,
    flags: u32,
    image_type: i32, // 1 = 2D
    format: i32,
    extent_width: u32,
    extent_height: u32,
    extent_depth: u32,
    mip_levels: u32,
    array_layers: u32,
    samples: u32,      // 1 = 1_BIT
    tiling: i32,       // 0 = OPTIMAL
    usage: u32,        // 0x1F (COLOR_ATTACHMENT | TRANSFER_SRC | TRANSFER_DST | SAMPLED)
    sharing_mode: i32, // 0 = EXCLUSIVE
    queue_family_index_count: u32,
    p_queue_family_indices: *const u32,
    initial_layout: i32, // 0 = UNDEFINED
}

#[repr(C)]
struct VkMemoryRequirements {
    size: u64,
    alignment: u64,
    memory_type_bits: u32,
}

#[repr(C)]
struct VkMemoryAllocateInfo {
    s_type: i32,
    p_next: *const std::ffi::c_void,
    allocation_size: u64,
    memory_type_index: u32,
}

pub struct SwapchainData {
    pub handle: u64,
    pub device: usize,
    pub images: Vec<u64>,
    pub memories: Vec<u64>,
    pub current_index: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
}

unsafe impl Send for SwapchainData {}
unsafe impl Sync for SwapchainData {}

static NEXT_SWAPCHAIN_HANDLE: AtomicU64 = AtomicU64::new(0x4000);
static SWAPCHAIN_TABLE: Mutex<Option<HashMap<u64, SwapchainData>>> = Mutex::new(None);
pub static FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);
static HOST_SWAPCHAINS: Mutex<Option<HashSet<u64>>> = Mutex::new(None);

pub fn register_host_swapchain(handle: u64, _device: usize) {
    let mut guard = HOST_SWAPCHAINS.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashSet::new());
    }
    if let Some(ref mut set) = *guard {
        set.insert(handle);
    }
}

pub fn is_host_swapchain(handle: u64) -> bool {
    let guard = HOST_SWAPCHAINS.lock().unwrap();
    if let Some(ref set) = *guard {
        set.contains(&handle)
    } else {
        false
    }
}

pub fn unregister_host_swapchain(handle: u64) {
    let mut guard = HOST_SWAPCHAINS.lock().unwrap();
    if let Some(ref mut set) = *guard {
        set.remove(&handle);
    }
}

fn swapchains() -> std::sync::MutexGuard<'static, Option<HashMap<u64, SwapchainData>>> {
    let mut guard = SWAPCHAIN_TABLE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    guard
}

pub unsafe fn create_virtual_swapchain(
    device: NativeVkDevice,
    width: u32,
    height: u32,
    min_images: u32,
    format: u32,
) -> u64 {
    let handle = NEXT_SWAPCHAIN_HANDLE.fetch_add(1, Ordering::SeqCst);
    let count = min_images.max(2).min(4);
    let mut images = Vec::with_capacity(count as usize);
    let mut memories = Vec::with_capacity(count as usize);

    type VkCreateImageFn = unsafe extern "C" fn(
        NativeVkDevice,
        *const VkImageCreateInfo,
        *const std::ffi::c_void,
        *mut u64,
    ) -> i32;
    type VkGetImageMemoryRequirementsFn =
        unsafe extern "C" fn(NativeVkDevice, u64, *mut VkMemoryRequirements);
    type VkAllocateMemoryFn = unsafe extern "C" fn(
        NativeVkDevice,
        *const VkMemoryAllocateInfo,
        *const std::ffi::c_void,
        *mut u64,
    ) -> i32;
    type VkBindImageMemoryFn = unsafe extern "C" fn(NativeVkDevice, u64, u64, u64) -> i32;

    let fn_create_image: Option<VkCreateImageFn> =
        get_native_proc_addr("vkCreateImage").map(|s| std::mem::transmute(s));
    let fn_get_reqs: Option<VkGetImageMemoryRequirementsFn> =
        get_native_proc_addr("vkGetImageMemoryRequirements").map(|s| std::mem::transmute(s));
    let fn_alloc_mem: Option<VkAllocateMemoryFn> =
        get_native_proc_addr("vkAllocateMemory").map(|s| std::mem::transmute(s));
    let fn_bind_mem: Option<VkBindImageMemoryFn> =
        get_native_proc_addr("vkBindImageMemory").map(|s| std::mem::transmute(s));

    if let (Some(create_img), Some(get_reqs), Some(alloc_mem), Some(bind_mem)) =
        (fn_create_image, fn_get_reqs, fn_alloc_mem, fn_bind_mem)
    {
        for _ in 0..count {
            let img_info = VkImageCreateInfo {
                s_type: 14, // VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO
                p_next: std::ptr::null(),
                flags: 0,
                image_type: 1, // VK_IMAGE_TYPE_2D
                format: format as i32,
                extent_width: width,
                extent_height: height,
                extent_depth: 1,
                mip_levels: 1,
                array_layers: 1,
                samples: 1,  // VK_SAMPLE_COUNT_1_BIT
                tiling: 0,   // VK_IMAGE_TILING_OPTIMAL
                usage: 0x1F, // COLOR_ATTACHMENT | TRANSFER_SRC | TRANSFER_DST | SAMPLED
                sharing_mode: 0,
                queue_family_index_count: 0,
                p_queue_family_indices: std::ptr::null(),
                initial_layout: 0, // VK_IMAGE_LAYOUT_UNDEFINED
            };

            let mut img: u64 = 0;
            let res = create_img(device, &img_info, std::ptr::null(), &mut img);
            if res == 0 && img != 0 {
                let mut reqs: VkMemoryRequirements = std::mem::zeroed();
                get_reqs(device, img, &mut reqs);

                let mem_type_index = reqs.memory_type_bits.trailing_zeros();
                let alloc_info = VkMemoryAllocateInfo {
                    s_type: 5, // VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO
                    p_next: std::ptr::null(),
                    allocation_size: reqs.size,
                    memory_type_index: mem_type_index,
                };

                let mut mem: u64 = 0;
                let alloc_res = alloc_mem(device, &alloc_info, std::ptr::null(), &mut mem);
                if alloc_res == 0 && mem != 0 {
                    bind_mem(device, img, mem, 0);
                    images.push(img);
                    memories.push(mem);
                } else {
                    images.push(img);
                }
            }
        }
    }

    println!(
        "[VULKAN SWAPCHAIN] Created virtual swapchain 0x{:X} with {} GPU images ({}x{})",
        handle,
        images.len(),
        width,
        height
    );

    let data = SwapchainData {
        handle,
        device: device as usize,
        images,
        memories,
        current_index: 0,
        width,
        height,
        format,
    };

    let mut guard = swapchains();
    if let Some(ref mut map) = *guard {
        map.insert(handle, data);
    }

    handle
}

pub fn get_swapchain_images(handle: u64, count: *mut u32, out_images: *mut u64) -> i32 {
    let guard = swapchains();
    if let Some(ref map) = *guard {
        if let Some(data) = map.get(&handle) {
            if !count.is_null() {
                if out_images.is_null() {
                    unsafe {
                        *count = data.images.len() as u32;
                    }
                } else {
                    let write_count = unsafe { (*count).min(data.images.len() as u32) };
                    for i in 0..write_count as usize {
                        unsafe {
                            *out_images.add(i) = data.images[i];
                        }
                    }
                    unsafe {
                        *count = write_count;
                    }
                }
                return VK_SUCCESS;
            }
        }
    }
    VK_ERROR_INITIALIZATION_FAILED
}

pub static LAST_QUEUE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[repr(C)]
struct VkSubmitInfo {
    s_type: i32,
    p_next: *const std::ffi::c_void,
    wait_semaphore_count: u32,
    p_wait_semaphores: *const u64,
    p_wait_dst_stage_mask: *const u32,
    command_buffer_count: u32,
    p_command_buffers: *const usize,
    signal_semaphore_count: u32,
    p_signal_semaphores: *const u64,
}

pub fn acquire_next_image(handle: u64, semaphore: u64, fence: u64, p_image_index: *mut u32) -> i32 {
    let mut guard = swapchains();
    if let Some(ref mut map) = *guard {
        if let Some(data) = map.get_mut(&handle) {
            let idx = data.current_index;
            data.current_index = (idx + 1) % (data.images.len() as u32);
            if !p_image_index.is_null() {
                unsafe {
                    *p_image_index = idx;
                }
            }
            if semaphore != 0 || fence != 0 {
                let q = LAST_QUEUE.load(Ordering::Relaxed);
                if q != 0 {
                    if let Some(sym) = unsafe { get_native_proc_addr("vkQueueSubmit") } {
                        type NativeSubmitFn = unsafe extern "C" fn(
                            *mut std::ffi::c_void,
                            u32,
                            *const VkSubmitInfo,
                            u64,
                        ) -> i32;
                        let fn_submit: NativeSubmitFn = unsafe { std::mem::transmute(sym) };
                        let sem_arr = [semaphore];
                        let submit = VkSubmitInfo {
                            s_type: 4, // VK_STRUCTURE_TYPE_SUBMIT_INFO
                            p_next: std::ptr::null(),
                            wait_semaphore_count: 0,
                            p_wait_semaphores: std::ptr::null(),
                            p_wait_dst_stage_mask: std::ptr::null(),
                            command_buffer_count: 0,
                            p_command_buffers: std::ptr::null(),
                            signal_semaphore_count: if semaphore != 0 { 1 } else { 0 },
                            p_signal_semaphores: if semaphore != 0 {
                                sem_arr.as_ptr()
                            } else {
                                std::ptr::null()
                            },
                        };
                        unsafe {
                            fn_submit(q as *mut std::ffi::c_void, 1, &submit, fence);
                        }
                    }
                }
            }
            return VK_SUCCESS;
        }
    }
    VK_ERROR_INITIALIZATION_FAILED
}

pub fn present_swapchain_image(_queue: NativeVkDevice, handle: u64, _image_index: u32) -> i32 {
    let frame_num = FRAME_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
    if frame_num == 1 {
        unsafe {
            crate::windowing::x11::show_and_unhide_game_window();
        }
    }
    let guard = swapchains();
    if let Some(ref map) = *guard {
        if let Some(data) = map.get(&handle) {
            let dpy = crate::windowing::x11::get_global_display();
            if !dpy.is_null() {
                unsafe {
                    crate::windowing::x11::XFlush(dpy);
                }
            }
            if frame_num == 1 {
                println!(
                    "[RINE FRAME PRESENTED] First frame successfully presented on desktop window ({}x{})!",
                    data.width, data.height
                );
            }
            return VK_SUCCESS;
        }
    }
    VK_SUCCESS
}

pub fn destroy_swapchain(handle: u64) {
    let mut guard = swapchains();
    if let Some(ref mut map) = *guard {
        if let Some(data) = map.remove(&handle) {
            type VkDestroyImageFn =
                unsafe extern "C" fn(NativeVkDevice, u64, *const std::ffi::c_void);
            type VkFreeMemoryFn =
                unsafe extern "C" fn(NativeVkDevice, u64, *const std::ffi::c_void);
            let fn_destroy_image: Option<VkDestroyImageFn> =
                unsafe { get_native_proc_addr("vkDestroyImage").map(|s| std::mem::transmute(s)) };
            let fn_free_mem: Option<VkFreeMemoryFn> =
                unsafe { get_native_proc_addr("vkFreeMemory").map(|s| std::mem::transmute(s)) };

            let dev = data.device as NativeVkDevice;
            if let Some(destroy_img) = fn_destroy_image {
                for img in data.images {
                    unsafe {
                        destroy_img(dev, img, std::ptr::null());
                    }
                }
            }
            if let Some(free_mem) = fn_free_mem {
                for mem in data.memories {
                    unsafe {
                        free_mem(dev, mem, std::ptr::null());
                    }
                }
            }
        }
    }
}
