//! Vulkan Bridge Types
//! Defines exact binary layouts for Vulkan WSI structures across PE32/PE64 and Linux Native boundaries.

use std::ffi::c_char;

pub const VK_STRUCTURE_TYPE_XLIB_SURFACE_CREATE_INFO_KHR: i32 = 1000004000;
pub const VK_STRUCTURE_TYPE_WIN32_SURFACE_CREATE_INFO_KHR: i32 = 1000009000;
pub const VK_MAX_EXTENSION_NAME_SIZE: usize = 256;

pub const VK_SUCCESS: i32 = 0;
pub const VK_NOT_READY: i32 = 1;
pub const VK_TIMEOUT: i32 = 2;
pub const VK_EVENT_SET: i32 = 3;
pub const VK_EVENT_RESET: i32 = 4;
pub const VK_INCOMPLETE: i32 = 5;
pub const VK_ERROR_OUT_OF_HOST_MEMORY: i32 = -1;
pub const VK_ERROR_OUT_OF_DEVICE_MEMORY: i32 = -2;
pub const VK_ERROR_INITIALIZATION_FAILED: i32 = -3;
pub const VK_ERROR_EXTENSION_NOT_PRESENT: i32 = -7;
pub const VK_ERROR_NATIVE_WINDOW_IN_USE_KHR: i32 = -1000000001;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VkExtensionProperties {
    pub extensionName: [c_char; VK_MAX_EXTENSION_NAME_SIZE],
    pub specVersion: u32,
}

#[repr(C)]
pub struct VkWin32SurfaceCreateInfoKHR {
    pub sType: i32,
    pub pNext: *const std::ffi::c_void,
    pub flags: u32,
    pub hinstance: usize,
    pub hwnd: usize,
}

#[repr(C)]
pub struct VkXlibSurfaceCreateInfoKHR {
    pub sType: i32,
    pub pNext: *const std::ffi::c_void,
    pub flags: u32,
    pub dpy: *mut std::ffi::c_void,
    pub window: u64,
}

#[repr(C)]
pub struct VkApplicationInfo {
    pub sType: i32,
    pub pNext: *const std::ffi::c_void,
    pub pApplicationName: *const c_char,
    pub applicationVersion: u32,
    pub pEngineName: *const c_char,
    pub engineVersion: u32,
    pub apiVersion: u32,
}

#[repr(C)]
pub struct VkInstanceCreateInfo {
    pub sType: i32,
    pub pNext: *const std::ffi::c_void,
    pub flags: u32,
    pub pApplicationInfo: *const std::ffi::c_void,
    pub enabledLayerCount: u32,
    pub ppEnabledLayerNames: *const *const c_char,
    pub enabledExtensionCount: u32,
    pub ppEnabledExtensionNames: *const *const c_char,
}

pub type NativeVkInstance = *mut std::ffi::c_void;
pub type NativeVkDevice = *mut std::ffi::c_void;
pub type NativeVkPhysicalDevice = *mut std::ffi::c_void;
pub type NativeVkSurfaceKHR = u64;
