//! Vulkan Bridge ABI Thunk and Layout Verification Tests

#[cfg(test)]
mod tests {
    use crate::graphics::vulkan_bridge::*;

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Structure Layout Tests (PE64 binary compatibility)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[test]
    fn test_vk_extension_properties_layout() {
        assert_eq!(std::mem::size_of::<VkExtensionProperties>(), 260);
        assert_eq!(std::mem::align_of::<VkExtensionProperties>(), 4);
        assert_eq!(
            std::mem::offset_of!(VkExtensionProperties, extensionName),
            0
        );
        assert_eq!(
            std::mem::offset_of!(VkExtensionProperties, specVersion),
            256
        );
    }

    #[test]
    fn test_vk_win32_surface_create_info_layout() {
        assert_eq!(std::mem::offset_of!(VkWin32SurfaceCreateInfoKHR, sType), 0);
        assert_eq!(std::mem::offset_of!(VkWin32SurfaceCreateInfoKHR, pNext), 8);
        assert_eq!(std::mem::offset_of!(VkWin32SurfaceCreateInfoKHR, flags), 16);
        assert_eq!(
            std::mem::offset_of!(VkWin32SurfaceCreateInfoKHR, hinstance),
            24
        );
        assert_eq!(std::mem::offset_of!(VkWin32SurfaceCreateInfoKHR, hwnd), 32);
        // Total size for PE64: sType(4) + pad(4) + pNext(8) + flags(4) + pad(4) + hinstance(8) + hwnd(8) = 40
        assert_eq!(std::mem::size_of::<VkWin32SurfaceCreateInfoKHR>(), 40);
    }

    #[test]
    fn test_vk_xlib_surface_create_info_layout() {
        assert_eq!(std::mem::offset_of!(VkXlibSurfaceCreateInfoKHR, sType), 0);
        assert_eq!(std::mem::offset_of!(VkXlibSurfaceCreateInfoKHR, pNext), 8);
        assert_eq!(std::mem::offset_of!(VkXlibSurfaceCreateInfoKHR, flags), 16);
        assert_eq!(std::mem::offset_of!(VkXlibSurfaceCreateInfoKHR, dpy), 24);
        assert_eq!(std::mem::offset_of!(VkXlibSurfaceCreateInfoKHR, window), 32);
        // Total size: sType(4) + pad(4) + pNext(8) + flags(4) + pad(4) + dpy(8) + window(8) = 40
        assert_eq!(std::mem::size_of::<VkXlibSurfaceCreateInfoKHR>(), 40);
    }

    #[test]
    fn test_vk_instance_create_info_layout() {
        assert_eq!(std::mem::offset_of!(VkInstanceCreateInfo, sType), 0);
        assert_eq!(std::mem::offset_of!(VkInstanceCreateInfo, pNext), 8);
        assert_eq!(std::mem::offset_of!(VkInstanceCreateInfo, flags), 16);
        assert_eq!(
            std::mem::offset_of!(VkInstanceCreateInfo, pApplicationInfo),
            24
        );
        assert_eq!(
            std::mem::offset_of!(VkInstanceCreateInfo, enabledLayerCount),
            32
        );
        assert_eq!(
            std::mem::offset_of!(VkInstanceCreateInfo, ppEnabledLayerNames),
            40
        );
        assert_eq!(
            std::mem::offset_of!(VkInstanceCreateInfo, enabledExtensionCount),
            48
        );
        assert_eq!(
            std::mem::offset_of!(VkInstanceCreateInfo, ppEnabledExtensionNames),
            56
        );
        assert_eq!(std::mem::size_of::<VkInstanceCreateInfo>(), 64);
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Handle Type Tests (PE64)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[test]
    fn test_handle_sizes() {
        // All Vulkan dispatchable handles are pointers (8 bytes on 64-bit)
        assert_eq!(std::mem::size_of::<NativeVkInstance>(), 8);
        assert_eq!(std::mem::size_of::<NativeVkDevice>(), 8);
        assert_eq!(std::mem::size_of::<NativeVkPhysicalDevice>(), 8);
        // Non-dispatchable handles are u64
        assert_eq!(std::mem::size_of::<NativeVkSurfaceKHR>(), 8);
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // ProcAddr Resolution Tests
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[test]
    fn test_proc_addr_known_commands() {
        // These should always resolve to our typed thunks
        let sym_win32 = resolve_vulkan_symbol("vkCreateWin32SurfaceKHR");
        assert!(sym_win32.is_some(), "vkCreateWin32SurfaceKHR must resolve");

        let sym_inst = resolve_vulkan_symbol("vkCreateInstance");
        assert!(sym_inst.is_some(), "vkCreateInstance must resolve");

        let sym_present = resolve_vulkan_symbol("vkQueuePresentKHR");
        assert!(sym_present.is_some(), "vkQueuePresentKHR must resolve");

        let sym_create_device = resolve_vulkan_symbol("vkCreateDevice");
        assert!(sym_create_device.is_some(), "vkCreateDevice must resolve");

        let sym_create_sem = resolve_vulkan_symbol("vkCreateSemaphore");
        assert!(sym_create_sem.is_some(), "vkCreateSemaphore must resolve");

        let sym_create_fence = resolve_vulkan_symbol("vkCreateFence");
        assert!(sym_create_fence.is_some(), "vkCreateFence must resolve");

        let sym_create_cmd_pool = resolve_vulkan_symbol("vkCreateCommandPool");
        assert!(
            sym_create_cmd_pool.is_some(),
            "vkCreateCommandPool must resolve"
        );

        let sym_alloc_cmd_buf = resolve_vulkan_symbol("vkAllocateCommandBuffers");
        assert!(
            sym_alloc_cmd_buf.is_some(),
            "vkAllocateCommandBuffers must resolve"
        );

        let sym_begin_cmd = resolve_vulkan_symbol("vkBeginCommandBuffer");
        assert!(sym_begin_cmd.is_some(), "vkBeginCommandBuffer must resolve");

        let sym_end_cmd = resolve_vulkan_symbol("vkEndCommandBuffer");
        assert!(sym_end_cmd.is_some(), "vkEndCommandBuffer must resolve");
    }

    #[test]
    fn test_proc_addr_unknown_returns_null() {
        // Unknown / nonexistent commands must return NULL, not a native System V pointer
        let sym_fake = resolve_vulkan_symbol("vkFakeCommandThatDoesNotExist");
        assert_eq!(
            sym_fake, None,
            "Must not return raw System V pointer fallback"
        );

        let name_c = std::ffi::CString::new("vkFakeCommandThatDoesNotExist").unwrap();
        let ptr = unsafe { rine_vkGetInstanceProcAddr(std::ptr::null_mut(), name_c.as_ptr()) };
        assert!(
            ptr.is_null(),
            "Unknown command must return NULL from GetInstanceProcAddr"
        );

        let ptr2 = unsafe { rine_vkGetDeviceProcAddr(std::ptr::null_mut(), name_c.as_ptr()) };
        assert!(
            ptr2.is_null(),
            "Unknown command must return NULL from GetDeviceProcAddr"
        );
    }

    #[test]
    fn test_gdpa_rejects_instance_commands() {
        let name_c = std::ffi::CString::new("vkCreateInstance").unwrap();
        let ptr_gdpa = unsafe { rine_vkGetDeviceProcAddr(std::ptr::null_mut(), name_c.as_ptr()) };
        let ptr_gipa = unsafe { rine_vkGetInstanceProcAddr(std::ptr::null_mut(), name_c.as_ptr()) };
        assert!(
            ptr_gdpa.is_null(),
            "GDPA must return NULL for instance command vkCreateInstance"
        );
        assert!(!ptr_gipa.is_null(), "GIPA must resolve vkCreateInstance");

        let phys_c = std::ffi::CString::new("vkEnumeratePhysicalDevices").unwrap();
        let ptr_gdpa_phys =
            unsafe { rine_vkGetDeviceProcAddr(std::ptr::null_mut(), phys_c.as_ptr()) };
        assert!(
            ptr_gdpa_phys.is_null(),
            "GDPA must return NULL for physical device command"
        );
    }

    #[test]
    fn test_proc_addr_khr_variants() {
        // KHR variants should map to the same thunk as core versions
        let core = resolve_vulkan_symbol("vkGetPhysicalDeviceProperties2");
        let khr = resolve_vulkan_symbol("vkGetPhysicalDeviceProperties2KHR");
        assert!(core.is_some());
        assert!(khr.is_some());
        assert_eq!(core, khr, "Core and KHR variant must resolve to same thunk");
    }

    #[test]
    fn test_vk_enumerate_instance_extension_properties_behavior() {
        let mut count: u32 = 0;
        // 1. pProperties == NULL (count-only query)
        let res = unsafe {
            rine_vkEnumerateInstanceExtensionProperties(
                std::ptr::null(),
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if res == VK_ERROR_INITIALIZATION_FAILED {
            println!(
                "[VULKAN TESTS] Native Vulkan loader unavailable, skipping live extension tests"
            );
            return;
        }
        assert_eq!(res, VK_SUCCESS);
        assert!(
            count > 0,
            "Should enumerate at least some extensions or virtual WSI"
        );

        // 2. точная ёмкость (exact capacity)
        let mut props: Vec<VkExtensionProperties> = vec![
            VkExtensionProperties {
                extensionName: [0; VK_MAX_EXTENSION_NAME_SIZE],
                specVersion: 0,
            };
            count as usize
        ];
        let mut exact_count = count;
        let res_exact = unsafe {
            rine_vkEnumerateInstanceExtensionProperties(
                std::ptr::null(),
                &mut exact_count,
                props.as_mut_ptr(),
            )
        };
        assert_eq!(res_exact, VK_SUCCESS);
        assert_eq!(exact_count, count);

        // 3. отсутствие дубликата VK_KHR_win32_surface & сохранение нативных расширений
        let mut win32_count = 0;
        for prop in &props {
            let name = unsafe { std::ffi::CStr::from_ptr(prop.extensionName.as_ptr()) }.to_bytes();
            if name == b"VK_KHR_win32_surface" {
                win32_count += 1;
            }
        }
        assert!(
            win32_count <= 1,
            "Must not have duplicate VK_KHR_win32_surface"
        );

        // 4. недостаточная ёмкость (insufficient capacity) & 5. нулевая ёмкость (zero capacity)
        if count > 1 {
            let mut small_count = count - 1;
            let mut small_props = props.clone();
            let res_incomp = unsafe {
                rine_vkEnumerateInstanceExtensionProperties(
                    std::ptr::null(),
                    &mut small_count,
                    small_props.as_mut_ptr(),
                )
            };
            assert_eq!(
                res_incomp, VK_INCOMPLETE,
                "Must return VK_INCOMPLETE when capacity < total"
            );
            assert_eq!(
                small_count,
                count - 1,
                "pPropertyCount must equal actual copied entries"
            );
        }

        let mut zero_count = 0;
        let res_zero = unsafe {
            rine_vkEnumerateInstanceExtensionProperties(
                std::ptr::null(),
                &mut zero_count,
                props.as_mut_ptr(),
            )
        };
        assert_eq!(
            res_zero, VK_INCOMPLETE,
            "Zero capacity with >0 extensions must return VK_INCOMPLETE"
        );
        assert_eq!(zero_count, 0);
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Window Registry Tests
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[test]
    fn test_window_registry_register_and_lookup() {
        let registry = WindowRegistry::global();
        let win = RineWindow {
            guest_hwnd: 0xDEAD0001,
            backend: WindowBackendType::X11,
            native_display: 0x12345 as *mut std::ffi::c_void,
            native_window: 0x67890,
            native_visual_id: 0x21,
            width: 1280,
            height: 720,
            visible: true,
        };
        registry.register_window(0xDEAD0001, win);

        let found = registry.lookup(0xDEAD0001);
        assert!(found.is_some());
        let f = found.unwrap();
        assert_eq!(f.native_window, 0x67890);
        assert_eq!(f.native_visual_id, 0x21);
        assert_eq!(f.width, 1280);
        assert_eq!(f.height, 720);
        assert!(f.visible);
        assert_eq!(f.backend, WindowBackendType::X11);
    }

    #[test]
    fn test_window_registry_multiple_hwnds() {
        let registry = WindowRegistry::global();

        let win1 = RineWindow {
            guest_hwnd: 0xAAAA0001,
            backend: WindowBackendType::X11,
            native_display: 0x1111 as *mut std::ffi::c_void,
            native_window: 0x2222,
            native_visual_id: 0x33,
            width: 800,
            height: 600,
            visible: true,
        };
        let win2 = RineWindow {
            guest_hwnd: 0xAAAA0002,
            backend: WindowBackendType::X11,
            native_display: 0x4444 as *mut std::ffi::c_void,
            native_window: 0x5555,
            native_visual_id: 0x66,
            width: 1920,
            height: 1080,
            visible: false,
        };

        registry.register_window(0xAAAA0001, win1);
        registry.register_window(0xAAAA0002, win2);

        let f1 = registry.lookup(0xAAAA0001).unwrap();
        let f2 = registry.lookup(0xAAAA0002).unwrap();

        assert_eq!(f1.native_window, 0x2222);
        assert_eq!(f2.native_window, 0x5555);
        assert_ne!(f1.native_visual_id, f2.native_visual_id);
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Constants Tests
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[test]
    fn test_vk_constants() {
        assert_eq!(VK_SUCCESS, 0);
        assert_eq!(VK_INCOMPLETE, 5);
        assert_eq!(VK_ERROR_INITIALIZATION_FAILED, -3);
        assert_eq!(VK_ERROR_EXTENSION_NOT_PRESENT, -7);
        assert_eq!(VK_STRUCTURE_TYPE_XLIB_SURFACE_CREATE_INFO_KHR, 1000004000);
        assert_eq!(VK_STRUCTURE_TYPE_WIN32_SURFACE_CREATE_INFO_KHR, 1000009000);
        assert_eq!(VK_MAX_EXTENSION_NAME_SIZE, 256);
    }
}
