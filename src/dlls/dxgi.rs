#![allow(non_snake_case)]
use crate::sys::{NtStatus, STATUS_SUCCESS};

pub type IDXGIFactory = u64;
pub type IDXGIAdapter = u64;
pub type IDXGISwapChain = u64;

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const DXGI_ERROR_NOT_FOUND: i32 = 0x887A_0004u32 as i32;

// --- Data Structures ---

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DXGI_ADAPTER_DESC {
    pub Description: [u16; 128],
    pub VendorId: u32,
    pub DeviceId: u32,
    pub SubSysId: u32,
    pub Revision: u32,
    pub DedicatedVideoMemory: usize,
    pub DedicatedSystemMemory: usize,
    pub SharedSystemMemory: usize,
    pub AdapterLuid: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DXGI_ADAPTER_DESC1 {
    pub Description: [u16; 128],
    pub VendorId: u32,
    pub DeviceId: u32,
    pub SubSysId: u32,
    pub Revision: u32,
    pub DedicatedVideoMemory: usize,
    pub DedicatedSystemMemory: usize,
    pub SharedSystemMemory: usize,
    pub AdapterLuid: [u32; 2],
    pub Flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DXGI_OUTPUT_DESC {
    pub DeviceName: [u16; 32],
    pub DesktopCoordinates: [i32; 4], // left, top, right, bottom
    pub AttachedToDesktop: i32,
    pub Rotation: u32,
    pub Monitor: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DXGI_MODE_DESC {
    pub Width: u32,
    pub Height: u32,
    pub RefreshRate_Numerator: u32,
    pub RefreshRate_Denominator: u32,
    pub Format: u32,
    pub ScanlineOrdering: u32,
    pub Scaling: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DXGI_SWAP_CHAIN_DESC {
    pub BufferDesc: DXGI_MODE_DESC,
    pub SampleDesc_Count: u32,
    pub SampleDesc_Quality: u32,
    pub BufferUsage: u32,
    pub BufferCount: u32,
    pub OutputWindow: u64,
    pub Windowed: i32,
    pub SwapEffect: u32,
    pub Flags: u32,
}

// --- Factory VTable Methods ---

unsafe extern "win64" fn factory_query_interface(
    this: *mut std::ffi::c_void,
    _riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }
    *ppv = this;
    crate::dlls::d3d11::com_object_add_ref(this);
    S_OK
}

unsafe extern "win64" fn dxgi_set_private_data(
    _this: *mut std::ffi::c_void,
    _guid: *const u8,
    _data_size: u32,
    _data: *const std::ffi::c_void,
) -> i32 {
    S_OK
}

unsafe extern "win64" fn dxgi_set_private_data_interface(
    _this: *mut std::ffi::c_void,
    _guid: *const u8,
    _p_unknown: *mut std::ffi::c_void,
) -> i32 {
    S_OK
}

unsafe extern "win64" fn dxgi_get_private_data(
    _this: *mut std::ffi::c_void,
    _guid: *const u8,
    p_data_size: *mut u32,
    _p_data: *mut std::ffi::c_void,
) -> i32 {
    if !p_data_size.is_null() {
        *p_data_size = 0;
    }
    S_OK
}

unsafe extern "win64" fn dxgi_get_parent(
    this: *mut std::ffi::c_void,
    _riid: *const u8,
    pp_parent: *mut *mut std::ffi::c_void,
) -> i32 {
    if !pp_parent.is_null() {
        *pp_parent = this;
        crate::dlls::d3d11::com_object_add_ref(this);
    }
    S_OK
}

unsafe extern "win64" fn factory_enum_adapters(
    _this: *mut std::ffi::c_void,
    adapter_index: u32,
    pp_adapter: *mut *mut std::ffi::c_void,
) -> i32 {
    if adapter_index > 0 {
        return DXGI_ERROR_NOT_FOUND;
    }
    if !pp_adapter.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_adapter_vtable() as usize,
            "dxgi!IDXGIFactory::EnumAdapters",
            "IDXGIAdapter",
            "{2411E03E-10C1-4410-BC14-2563E2E7F905}",
            "{2411E03E-10C1-4410-BC14-2563E2E7F905}",
            25,
            (0..25).collect(),
        );
        *pp_adapter = mem;
    }
    println!("[DXGI ENGINE] EnumAdapters: Created virtual adapter #0");
    S_OK
}

unsafe extern "win64" fn factory_make_window_association(
    _this: *mut std::ffi::c_void,
    _hwnd: u64,
    _flags: u32,
) -> i32 {
    S_OK
}

unsafe extern "win64" fn factory_get_window_association(
    _this: *mut std::ffi::c_void,
    p_hwnd: *mut u64,
) -> i32 {
    if !p_hwnd.is_null() {
        *p_hwnd = crate::windowing::x11::get_global_window();
    }
    S_OK
}

unsafe extern "win64" fn factory_create_swap_chain(
    _this: *mut std::ffi::c_void,
    _p_device: *mut std::ffi::c_void,
    p_desc: *mut DXGI_SWAP_CHAIN_DESC,
    pp_swapchain: *mut *mut std::ffi::c_void,
) -> i32 {
    if !p_desc.is_null() {
        let hwnd = (*p_desc).OutputWindow;
        if hwnd != 0 {
            let x11_win = crate::windowing::x11::get_global_window();
            let x11_dpy = crate::windowing::x11::get_global_display();
            let x11_vid = crate::windowing::x11::get_global_visual_id();
            if x11_win != 0 {
                crate::graphics::vulkan_bridge::handles::WindowRegistry::global().register_window(
                    hwnd as usize,
                    crate::graphics::vulkan_bridge::handles::RineWindow {
                        guest_hwnd: hwnd as usize,
                        backend: crate::graphics::vulkan_bridge::handles::WindowBackendType::X11,
                        native_display: x11_dpy,
                        native_window: x11_win,
                        native_visual_id: x11_vid,
                        width: (*p_desc).BufferDesc.Width.max(1280),
                        height: (*p_desc).BufferDesc.Height.max(720),
                        visible: true,
                    },
                );
            }
        }
    }
    if !pp_swapchain.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_swapchain_vtable() as usize,
            "dxgi!IDXGIFactory::CreateSwapChain",
            "IDXGISwapChain",
            "{310D36A0-D2E7-4C0A-AA04-6A9D23B8886A}",
            "{310D36A0-D2E7-4C0A-AA04-6A9D23B8886A}",
            25,
            (0..25).collect(),
        );
        *pp_swapchain = mem;
    }
    println!("[DXGI ENGINE] CreateSwapChain: Initialized virtual IDXGISwapChain");
    S_OK
}

unsafe extern "win64" fn factory_create_software_adapter(
    _this: *mut std::ffi::c_void,
    _module: usize,
    _pp_adapter: *mut *mut std::ffi::c_void,
) -> i32 {
    DXGI_ERROR_NOT_FOUND
}

unsafe extern "win64" fn factory_enum_adapters1(
    _this: *mut std::ffi::c_void,
    adapter_index: u32,
    pp_adapter: *mut *mut std::ffi::c_void,
) -> i32 {
    if adapter_index > 0 {
        return DXGI_ERROR_NOT_FOUND;
    }
    if !pp_adapter.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_adapter_vtable() as usize,
            "dxgi!IDXGIFactory1::EnumAdapters1",
            "IDXGIAdapter1",
            "{29038F61-3839-4626-91FD-086879011A05}",
            "{29038F61-3839-4626-91FD-086879011A05}",
            25,
            (0..25).collect(),
        );
        *pp_adapter = mem;
    }
    println!("[DXGI ENGINE] EnumAdapters1: Created virtual adapter1 #0");
    S_OK
}

unsafe extern "win64" fn factory_is_current(_this: *mut std::ffi::c_void) -> i32 {
    1 // TRUE
}

// --- Adapter VTable Methods ---

unsafe extern "win64" fn adapter_query_interface(
    this: *mut std::ffi::c_void,
    _riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }
    *ppv = this;
    crate::dlls::d3d11::com_object_add_ref(this);
    S_OK
}

unsafe extern "win64" fn adapter_enum_outputs(
    _this: *mut std::ffi::c_void,
    output_index: u32,
    pp_output: *mut *mut std::ffi::c_void,
) -> i32 {
    if output_index > 0 {
        return DXGI_ERROR_NOT_FOUND;
    }
    if !pp_output.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_output_vtable() as usize,
            "dxgi!IDXGIAdapter::EnumOutputs",
            "IDXGIOutput",
            "{AE02EEDB-C735-4690-8D52-5A8DC20213AA}",
            "{AE02EEDB-C735-4690-8D52-5A8DC20213AA}",
            25,
            (0..25).collect(),
        );
        *pp_output = mem;
    }
    println!("[DXGI ENGINE] EnumOutputs: Created virtual display output #0");
    S_OK
}

unsafe extern "win64" fn adapter_get_desc(
    _this: *mut std::ffi::c_void,
    p_desc: *mut DXGI_ADAPTER_DESC,
) -> i32 {
    if !p_desc.is_null() {
        let desc = &mut *p_desc;
        let name: Vec<u16> = "Rine Virtual GPU Adapter (DirectX 11 / Vulkan)\0"
            .encode_utf16()
            .collect();
        let copy_len = name.len().min(127);
        desc.Description[..copy_len].copy_from_slice(&name[..copy_len]);
        desc.Description[copy_len] = 0;
        desc.VendorId = 0x10DE; // NVIDIA
        desc.DeviceId = 0x2204; // RTX 3090
        desc.SubSysId = 0;
        desc.Revision = 0;
        desc.DedicatedVideoMemory = 8 * 1024 * 1024 * 1024;
        desc.DedicatedSystemMemory = 0;
        desc.SharedSystemMemory = 16 * 1024 * 1024 * 1024;
        desc.AdapterLuid = [1, 0];
    }
    S_OK
}

unsafe extern "win64" fn adapter_check_interface_support(
    _this: *mut std::ffi::c_void,
    _interface_name: *const u8,
    p_umd_version: *mut u64,
) -> i32 {
    if !p_umd_version.is_null() {
        *p_umd_version = 0x000B000000000000; // 11.0
    }
    S_OK
}

unsafe extern "win64" fn adapter_get_desc1(
    _this: *mut std::ffi::c_void,
    p_desc: *mut DXGI_ADAPTER_DESC1,
) -> i32 {
    if !p_desc.is_null() {
        let desc = &mut *p_desc;
        let name: Vec<u16> = "Rine Virtual GPU Adapter (DirectX 11 / Vulkan)\0"
            .encode_utf16()
            .collect();
        let copy_len = name.len().min(127);
        desc.Description[..copy_len].copy_from_slice(&name[..copy_len]);
        desc.Description[copy_len] = 0;
        desc.VendorId = 0x10DE;
        desc.DeviceId = 0x2204;
        desc.SubSysId = 0;
        desc.Revision = 0;
        desc.DedicatedVideoMemory = 8 * 1024 * 1024 * 1024;
        desc.DedicatedSystemMemory = 0;
        desc.SharedSystemMemory = 16 * 1024 * 1024 * 1024;
        desc.AdapterLuid = [1, 0];
        desc.Flags = 0;
    }
    S_OK
}

// --- Output VTable Methods ---

unsafe extern "win64" fn output_query_interface(
    this: *mut std::ffi::c_void,
    _riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }
    *ppv = this;
    crate::dlls::d3d11::com_object_add_ref(this);
    S_OK
}

unsafe extern "win64" fn output_get_desc(
    _this: *mut std::ffi::c_void,
    p_desc: *mut DXGI_OUTPUT_DESC,
) -> i32 {
    if !p_desc.is_null() {
        let desc = &mut *p_desc;
        let name: Vec<u16> = "\\\\.\\DISPLAY1\0".encode_utf16().collect();
        let copy_len = name.len().min(31);
        desc.DeviceName[..copy_len].copy_from_slice(&name[..copy_len]);
        desc.DeviceName[copy_len] = 0;
        desc.DesktopCoordinates = [0, 0, 2560, 1440];
        desc.AttachedToDesktop = 1;
        desc.Rotation = 0;
        desc.Monitor = 0x00010001;
    }
    S_OK
}

unsafe extern "win64" fn output_get_display_mode_list(
    _this: *mut std::ffi::c_void,
    enum_format: u32,
    _flags: u32,
    p_num_modes: *mut u32,
    p_desc: *mut DXGI_MODE_DESC,
) -> i32 {
    if !p_num_modes.is_null() {
        *p_num_modes = 1;
    }
    if !p_desc.is_null() {
        let bounds = crate::windowing::x11::get_host_display_bounds(0);
        let desc = &mut *p_desc;
        desc.Width = if bounds.w > 0 { bounds.w as u32 } else { 2560 };
        desc.Height = if bounds.h > 0 { bounds.h as u32 } else { 1440 };
        desc.RefreshRate_Numerator = crate::dlls::user32::get_host_refresh_rate();
        desc.RefreshRate_Denominator = 1;
        desc.Format = if enum_format != 0 { enum_format } else { 28 }; // DXGI_FORMAT_R8G8B8A8_UNORM
        desc.ScanlineOrdering = 0;
        desc.Scaling = 0;
    }
    S_OK
}

unsafe extern "win64" fn output_find_closest_matching_mode(
    _this: *mut std::ffi::c_void,
    p_mode_to_match: *const DXGI_MODE_DESC,
    p_closest_match: *mut DXGI_MODE_DESC,
    _p_device: *mut std::ffi::c_void,
) -> i32 {
    if !p_closest_match.is_null() && !p_mode_to_match.is_null() {
        *p_closest_match = *p_mode_to_match;
    }
    S_OK
}

unsafe extern "win64" fn output_wait_for_vblank(_this: *mut std::ffi::c_void) -> i32 {
    S_OK
}

unsafe extern "win64" fn output_take_ownership(
    _this: *mut std::ffi::c_void,
    _p_device: *mut std::ffi::c_void,
    _exclusive: i32,
) -> i32 {
    S_OK
}

unsafe extern "win64" fn output_release_ownership(_this: *mut std::ffi::c_void) -> i32 {
    S_OK
}

// --- SwapChain VTable Methods ---

unsafe extern "win64" fn swapchain_query_interface(
    this: *mut std::ffi::c_void,
    _riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }
    *ppv = this;
    crate::dlls::d3d11::com_object_add_ref(this);
    S_OK
}

unsafe extern "win64" fn swapchain_get_device(
    _this: *mut std::ffi::c_void,
    _riid: *const u8,
    pp_device: *mut *mut std::ffi::c_void,
) -> i32 {
    if !pp_device.is_null() {
        *pp_device = std::ptr::null_mut();
    }
    S_OK
}

use std::sync::atomic::{AtomicUsize, Ordering};

pub static SWAPCHAIN_PRESENT_CALLS: AtomicUsize = AtomicUsize::new(0);
pub static SWAPCHAIN_PRESENT1_CALLS: AtomicUsize = AtomicUsize::new(0);
pub static SWAPCHAIN_GET_BUFFER_CALLS: AtomicUsize = AtomicUsize::new(0);
pub static CREATE_SWAPCHAIN_CALLS: AtomicUsize = AtomicUsize::new(0);
pub static CREATE_SWAPCHAIN_FOR_HWND_CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "win64" fn swapchain_present(
    this: *mut std::ffi::c_void,
    sync_interval: u32,
    flags: u32,
) -> i32 {
    let c = SWAPCHAIN_PRESENT_CALLS.fetch_add(1, Ordering::Relaxed);
    let mut caller_pc: usize = 0;
    std::arch::asm!("mov {}, [rsp + 0x28]", out(reg) caller_pc, options(nostack, preserves_flags));
    println!(
        "[DXGI SWAPCHAIN] Present #{} this={:p}, sync_interval={}, flags=0x{:X}, caller_pc=0x{:X}",
        c + 1,
        this,
        sync_interval,
        flags,
        caller_pc
    );
    IDXGISwapChain_Present(this as u64, sync_interval, flags)
}

unsafe extern "win64" fn swapchain_present1(
    this: *mut std::ffi::c_void,
    sync_interval: u32,
    flags: u32,
    p_present_parameters: *const std::ffi::c_void,
) -> i32 {
    let c = SWAPCHAIN_PRESENT1_CALLS.fetch_add(1, Ordering::Relaxed);
    let mut caller_pc: usize = 0;
    std::arch::asm!("mov {}, [rsp + 0x28]", out(reg) caller_pc, options(nostack, preserves_flags));
    println!(
        "[DXGI SWAPCHAIN] Present1 #{} this={:p}, sync_interval={}, flags=0x{:X}, params={:p}, caller_pc=0x{:X}",
        c + 1, this, sync_interval, flags, p_present_parameters, caller_pc
    );
    IDXGISwapChain_Present(this as u64, sync_interval, flags)
}

unsafe extern "win64" fn swapchain_get_buffer(
    this: *mut std::ffi::c_void,
    buffer: u32,
    riid: *const u8,
    pp_surface: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = SWAPCHAIN_GET_BUFFER_CALLS.fetch_add(1, Ordering::Relaxed);
    let mut iid_str = String::from("NULL");
    if !riid.is_null() {
        let iid = std::slice::from_raw_parts(riid, 16);
        let d1 = u32::from_le_bytes([iid[0], iid[1], iid[2], iid[3]]);
        let d2 = u16::from_le_bytes([iid[4], iid[5]]);
        let d3 = u16::from_le_bytes([iid[6], iid[7]]);
        iid_str = format!(
            "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
            d1, d2, d3, iid[8], iid[9], iid[10], iid[11], iid[12], iid[13], iid[14], iid[15]
        );
    }
    println!(
        "[DXGI SWAPCHAIN] GetBuffer #{} buffer_idx={}, riid={}, pp_surface={:p}",
        c + 1,
        buffer,
        iid_str,
        pp_surface
    );
    if !pp_surface.is_null() {
        let vtable_ptr = crate::dlls::d3d11::get_d3d11_device_vtable() as usize;
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            vtable_ptr,
            "dxgi!IDXGISwapChain::GetBuffer",
            "ID3D11Texture2D",
            "{6F15AAF2-D208-4E89-9AB4-489535D34F9C}",
            "{6F15AAF2-D208-4E89-9AB4-489535D34F9C}",
            30,
            (0..30).collect(),
        );
        *pp_surface = mem;
    }
    S_OK
}

unsafe extern "win64" fn swapchain_set_fullscreen_state(
    _this: *mut std::ffi::c_void,
    _fullscreen: i32,
    _p_target: *mut std::ffi::c_void,
) -> i32 {
    S_OK
}

unsafe extern "win64" fn swapchain_get_fullscreen_state(
    _this: *mut std::ffi::c_void,
    p_fullscreen: *mut i32,
    pp_target: *mut *mut std::ffi::c_void,
) -> i32 {
    if !p_fullscreen.is_null() {
        *p_fullscreen = 0; // Windowed
    }
    if !pp_target.is_null() {
        *pp_target = std::ptr::null_mut();
    }
    S_OK
}

unsafe extern "win64" fn swapchain_get_desc(
    _this: *mut std::ffi::c_void,
    p_desc: *mut DXGI_SWAP_CHAIN_DESC,
) -> i32 {
    if !p_desc.is_null() {
        let bounds = crate::windowing::x11::get_host_display_bounds(0);
        let desc = &mut *p_desc;
        desc.BufferDesc.Width = if bounds.w > 0 { bounds.w as u32 } else { 2560 };
        desc.BufferDesc.Height = if bounds.h > 0 { bounds.h as u32 } else { 1440 };
        desc.BufferDesc.RefreshRate_Numerator = crate::dlls::user32::get_host_refresh_rate();
        desc.BufferDesc.RefreshRate_Denominator = 1;
        desc.BufferDesc.Format = 28; // DXGI_FORMAT_R8G8B8A8_UNORM
        desc.SampleDesc_Count = 1;
        desc.SampleDesc_Quality = 0;
        desc.BufferUsage = 0x20 | 0x40; // DXGI_USAGE_RENDER_TARGET_OUTPUT
        desc.BufferCount = 2;
        desc.OutputWindow = crate::windowing::x11::get_global_window();
        desc.Windowed = 1;
        desc.SwapEffect = 0; // DXGI_SWAP_EFFECT_DISCARD
        desc.Flags = 0;
    }
    S_OK
}

unsafe extern "win64" fn swapchain_resize_buffers(
    _this: *mut std::ffi::c_void,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: u32,
    swap_chain_flags: u32,
) -> i32 {
    println!(
        "[DXGI SWAPCHAIN] ResizeBuffers: count={}, ({}x{}), format={}, flags=0x{:X}",
        buffer_count, width, height, new_format, swap_chain_flags
    );
    S_OK
}

unsafe extern "win64" fn swapchain_resize_target(
    _this: *mut std::ffi::c_void,
    _p_new_target_params: *const DXGI_MODE_DESC,
) -> i32 {
    S_OK
}

unsafe extern "win64" fn swapchain_get_containing_output(
    _this: *mut std::ffi::c_void,
    pp_output: *mut *mut std::ffi::c_void,
) -> i32 {
    if !pp_output.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_output_vtable() as usize,
            "dxgi!IDXGISwapChain::GetContainingOutput",
            "IDXGIOutput",
            "{AE02EEDB-C735-4690-8D52-5A8DC20213AA}",
            "{AE02EEDB-C735-4690-8D52-5A8DC20213AA}",
            25,
            (0..25).collect(),
        );
        *pp_output = mem;
    }
    S_OK
}

unsafe extern "win64" fn factory_check_feature_support(
    _this: *mut std::ffi::c_void,
    feature: u32,
    p_feature_support_data: *mut std::ffi::c_void,
    feature_support_data_size: u32,
) -> i32 {
    println!(
        "[DXGI FACTORY] CheckFeatureSupport: feature={}, size={}",
        feature, feature_support_data_size
    );
    if !p_feature_support_data.is_null() && feature_support_data_size >= 4 {
        // Return 0 (disabled / not supported) for DXGI_FEATURE_PRESENT_ALLOW_TEARING
        *(p_feature_support_data as *mut u32) = 0;
    }
    S_OK
}

unsafe extern "win64" fn factory_create_swap_chain_for_hwnd(
    _this: *mut std::ffi::c_void,
    _p_device: *mut std::ffi::c_void,
    hwnd: u64,
    p_desc: *const u32,
    _p_fullscreen_desc: *const u8,
    _p_restrict_to_output: *mut std::ffi::c_void,
    pp_swap_chain: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = CREATE_SWAPCHAIN_FOR_HWND_CALLS.fetch_add(1, Ordering::Relaxed);
    if hwnd != 0 {
        let x11_win = crate::windowing::x11::get_global_window();
        let x11_dpy = crate::windowing::x11::get_global_display();
        let x11_vid = crate::windowing::x11::get_global_visual_id();
        if x11_win != 0 {
            crate::graphics::vulkan_bridge::handles::WindowRegistry::global().register_window(
                hwnd as usize,
                crate::graphics::vulkan_bridge::handles::RineWindow {
                    guest_hwnd: hwnd as usize,
                    backend: crate::graphics::vulkan_bridge::handles::WindowBackendType::X11,
                    native_display: x11_dpy,
                    native_window: x11_win,
                    native_visual_id: x11_vid,
                    width: 2560,
                    height: 1440,
                    visible: true,
                },
            );
        }
    }
    if !p_desc.is_null() {
        let width = *p_desc;
        let height = *p_desc.add(1);
        let format = *p_desc.add(2);
        let buffer_count = *p_desc.add(5);
        let swap_effect = *p_desc.add(7);
        println!(
            "[DXGI FACTORY] CreateSwapChainForHwnd #{} hwnd=0x{:X}, ({}x{}), format={}, buffer_count={}, swap_effect={}",
            c + 1, hwnd, width, height, format, buffer_count, swap_effect
        );
    } else {
        println!(
            "[DXGI FACTORY] CreateSwapChainForHwnd #{} hwnd=0x{:X}",
            c + 1,
            hwnd
        );
    }
    if !pp_swap_chain.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_swapchain_vtable() as usize,
            "dxgi!IDXGIFactory2::CreateSwapChainForHwnd",
            "IDXGISwapChain1",
            "{790A45F8-80A8-4818-9852-3AC3F8D80F15}",
            "{790A45F8-80A8-4818-9852-3AC3F8D80F15}",
            50,
            (0..50).collect(),
        );
        *pp_swap_chain = mem;
    }
    S_OK
}

unsafe extern "win64" fn dxgi_device_query_interface(
    this: *mut std::ffi::c_void,
    _riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }
    *ppv = this;
    crate::dlls::d3d11::com_object_add_ref(this);
    S_OK
}

unsafe extern "win64" fn dxgi_device_get_adapter(
    _this: *mut std::ffi::c_void,
    p_adapter: *mut *mut std::ffi::c_void,
) -> i32 {
    if !p_adapter.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_adapter_vtable() as usize,
            "dxgi!IDXGIDevice::GetAdapter",
            "IDXGIAdapter",
            "{2411E03E-10C1-4410-BC14-2563E2E7F905}",
            "{2411E03E-10C1-4410-BC14-2563E2E7F905}",
            25,
            (0..25).collect(),
        );
        *p_adapter = mem;
    }
    S_OK
}

unsafe extern "win64" fn dxgi_device_set_maximum_frame_latency(
    _this: *mut std::ffi::c_void,
    _max_latency: u32,
) -> i32 {
    S_OK
}

unsafe extern "win64" fn dxgi_device_get_maximum_frame_latency(
    _this: *mut std::ffi::c_void,
    p_max_latency: *mut u32,
) -> i32 {
    if !p_max_latency.is_null() {
        *p_max_latency = 1;
    }
    S_OK
}

unsafe extern "win64" fn dxgi_device_trim(_this: *mut std::ffi::c_void) -> i32 {
    S_OK
}

pub fn get_dxgi_device_object() -> *mut std::ffi::c_void {
    crate::dlls::d3d11::allocate_mock_com_object_with_meta(
        get_dxgidevice_vtable() as usize,
        "dxgi!IDXGIDevice",
        "IDXGIDevice3",
        "{50C83A1C-E072-4C48-87B0-3630FA36A6D0}",
        "{50C83A1C-E072-4C48-87B0-3630FA36A6D0}",
        25,
        (0..25).collect(),
    )
}

unsafe extern "win64" fn dxgi_method_ok(
    _this: *mut std::ffi::c_void,
    _a2: u64,
    _a3: u64,
    _a4: u64,
) -> i32 {
    S_OK
}

// --- VTable Storage & Initialization ---

static mut FACTORY_VTABLE: [*const (); 50] = [std::ptr::null(); 50];
static mut ADAPTER_VTABLE: [*const (); 50] = [std::ptr::null(); 50];
static mut OUTPUT_VTABLE: [*const (); 50] = [std::ptr::null(); 50];
static mut SWAPCHAIN_VTABLE: [*const (); 50] = [std::ptr::null(); 50];
static mut DEVICE_VTABLE: [*const (); 50] = [std::ptr::null(); 50];

pub fn get_dxgidevice_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        for i in 0..50 {
            DEVICE_VTABLE[i] = dxgi_method_ok as *const ();
        }
        DEVICE_VTABLE[0] = dxgi_device_query_interface as *const ();
        DEVICE_VTABLE[1] = crate::dlls::d3d11::com_object_add_ref as *const ();
        DEVICE_VTABLE[2] = crate::dlls::d3d11::com_object_release as *const ();
        DEVICE_VTABLE[3] = dxgi_set_private_data as *const ();
        DEVICE_VTABLE[4] = dxgi_set_private_data_interface as *const ();
        DEVICE_VTABLE[5] = dxgi_get_private_data as *const ();
        DEVICE_VTABLE[6] = dxgi_get_parent as *const ();
        DEVICE_VTABLE[7] = dxgi_device_get_adapter as *const (); // GetAdapter
        DEVICE_VTABLE[12] = dxgi_device_set_maximum_frame_latency as *const (); // SetMaximumFrameLatency
        DEVICE_VTABLE[13] = dxgi_device_get_maximum_frame_latency as *const (); // GetMaximumFrameLatency
        DEVICE_VTABLE[17] = dxgi_device_trim as *const (); // Trim
    });
    std::ptr::addr_of!(DEVICE_VTABLE) as *const *const ()
}

pub fn get_factory_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        for i in 0..50 {
            FACTORY_VTABLE[i] = dxgi_method_ok as *const ();
        }
        FACTORY_VTABLE[0] = factory_query_interface as *const ();
        FACTORY_VTABLE[1] = crate::dlls::d3d11::com_object_add_ref as *const ();
        FACTORY_VTABLE[2] = crate::dlls::d3d11::com_object_release as *const ();
        FACTORY_VTABLE[3] = dxgi_set_private_data as *const ();
        FACTORY_VTABLE[4] = dxgi_set_private_data_interface as *const ();
        FACTORY_VTABLE[5] = dxgi_get_private_data as *const ();
        FACTORY_VTABLE[6] = dxgi_get_parent as *const ();
        FACTORY_VTABLE[7] = factory_enum_adapters as *const ();
        FACTORY_VTABLE[8] = factory_make_window_association as *const ();
        FACTORY_VTABLE[9] = factory_get_window_association as *const ();
        FACTORY_VTABLE[10] = factory_create_swap_chain as *const ();
        FACTORY_VTABLE[11] = factory_create_software_adapter as *const ();
        FACTORY_VTABLE[12] = factory_enum_adapters1 as *const ();
        FACTORY_VTABLE[13] = factory_is_current as *const ();
        FACTORY_VTABLE[15] = factory_create_swap_chain_for_hwnd as *const ();
        FACTORY_VTABLE[28] = factory_check_feature_support as *const (); // IDXGIFactory5::CheckFeatureSupport (0xe0)
    });
    std::ptr::addr_of!(FACTORY_VTABLE) as *const *const ()
}

pub fn get_adapter_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        for i in 0..50 {
            ADAPTER_VTABLE[i] = dxgi_method_ok as *const ();
        }
        ADAPTER_VTABLE[0] = adapter_query_interface as *const ();
        ADAPTER_VTABLE[1] = crate::dlls::d3d11::com_object_add_ref as *const ();
        ADAPTER_VTABLE[2] = crate::dlls::d3d11::com_object_release as *const ();
        ADAPTER_VTABLE[3] = dxgi_set_private_data as *const ();
        ADAPTER_VTABLE[4] = dxgi_set_private_data_interface as *const ();
        ADAPTER_VTABLE[5] = dxgi_get_private_data as *const ();
        ADAPTER_VTABLE[6] = dxgi_get_parent as *const ();
        ADAPTER_VTABLE[7] = adapter_enum_outputs as *const ();
        ADAPTER_VTABLE[8] = adapter_get_desc as *const ();
        ADAPTER_VTABLE[9] = adapter_check_interface_support as *const ();
        ADAPTER_VTABLE[10] = adapter_get_desc1 as *const ();
    });
    std::ptr::addr_of!(ADAPTER_VTABLE) as *const *const ()
}

pub fn get_output_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        for i in 0..50 {
            OUTPUT_VTABLE[i] = dxgi_method_ok as *const ();
        }
        OUTPUT_VTABLE[0] = output_query_interface as *const ();
        OUTPUT_VTABLE[1] = crate::dlls::d3d11::com_object_add_ref as *const ();
        OUTPUT_VTABLE[2] = crate::dlls::d3d11::com_object_release as *const ();
        OUTPUT_VTABLE[3] = dxgi_set_private_data as *const ();
        OUTPUT_VTABLE[4] = dxgi_set_private_data_interface as *const ();
        OUTPUT_VTABLE[5] = dxgi_get_private_data as *const ();
        OUTPUT_VTABLE[6] = dxgi_get_parent as *const ();
        OUTPUT_VTABLE[7] = output_get_desc as *const ();
        OUTPUT_VTABLE[8] = output_get_display_mode_list as *const ();
        OUTPUT_VTABLE[9] = output_find_closest_matching_mode as *const ();
        OUTPUT_VTABLE[10] = output_wait_for_vblank as *const ();
        OUTPUT_VTABLE[11] = output_take_ownership as *const ();
        OUTPUT_VTABLE[12] = output_release_ownership as *const ();
    });
    std::ptr::addr_of!(OUTPUT_VTABLE) as *const *const ()
}

pub fn get_swapchain_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        for i in 0..50 {
            SWAPCHAIN_VTABLE[i] = dxgi_method_ok as *const ();
        }
        SWAPCHAIN_VTABLE[0] = swapchain_query_interface as *const ();
        SWAPCHAIN_VTABLE[1] = crate::dlls::d3d11::com_object_add_ref as *const ();
        SWAPCHAIN_VTABLE[2] = crate::dlls::d3d11::com_object_release as *const ();
        SWAPCHAIN_VTABLE[3] = dxgi_set_private_data as *const ();
        SWAPCHAIN_VTABLE[4] = dxgi_set_private_data_interface as *const ();
        SWAPCHAIN_VTABLE[5] = dxgi_get_private_data as *const ();
        SWAPCHAIN_VTABLE[6] = dxgi_get_parent as *const ();
        SWAPCHAIN_VTABLE[7] = swapchain_get_device as *const ();
        SWAPCHAIN_VTABLE[8] = swapchain_present as *const ();
        SWAPCHAIN_VTABLE[9] = swapchain_get_buffer as *const ();
        SWAPCHAIN_VTABLE[10] = swapchain_set_fullscreen_state as *const ();
        SWAPCHAIN_VTABLE[11] = swapchain_get_fullscreen_state as *const ();
        SWAPCHAIN_VTABLE[12] = swapchain_get_desc as *const ();
        SWAPCHAIN_VTABLE[13] = swapchain_resize_buffers as *const ();
        SWAPCHAIN_VTABLE[14] = swapchain_resize_target as *const ();
        SWAPCHAIN_VTABLE[15] = swapchain_get_containing_output as *const ();
        SWAPCHAIN_VTABLE[22] = swapchain_present1 as *const ();
    });
    std::ptr::addr_of!(SWAPCHAIN_VTABLE) as *const *const ()
}

// --- Exported Factory Creation Functions ---

#[no_mangle]
pub unsafe extern "win64" fn CreateDXGIFactory(_riid: u64, factory_out: *mut u64) -> i32 {
    println!("[DXGI ENTRY] CreateDXGIFactory");
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_factory_vtable() as usize,
            "dxgi!CreateDXGIFactory",
            "IDXGIFactory",
            "{7B7166EC-21C7-44AE-B21A-C9AE321AE369}",
            "{7B7166EC-21C7-44AE-B21A-C9AE321AE369}",
            25,
            (0..25).collect(),
        );
        *factory_out = mem as u64;
    }
    println!("[DXGI ENGINE] CreateDXGIFactory: Initialized DXGI Factory");
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDXGIFactory1(_riid: u64, factory_out: *mut u64) -> i32 {
    println!("[DXGI ENTRY] CreateDXGIFactory1");
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_factory_vtable() as usize,
            "dxgi!CreateDXGIFactory1",
            "IDXGIFactory1",
            "{770A644D-A7E0-470F-A927-5956341CCF56}",
            "{770A644D-A7E0-470F-A927-5956341CCF56}",
            25,
            (0..25).collect(),
        );
        *factory_out = mem as u64;
    }
    println!("[DXGI ENGINE] CreateDXGIFactory1: Initialized DXGI 1.1 Factory");
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDXGIFactory2(
    _flags: u32,
    _riid: u64,
    factory_out: *mut u64,
) -> i32 {
    println!("[DXGI ENTRY] CreateDXGIFactory2");
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_factory_vtable() as usize,
            "dxgi!CreateDXGIFactory2",
            "IDXGIFactory2",
            "{50C83A58-C072-4C6E-917D-01D23F510F40}",
            "{50C83A58-C072-4C6E-917D-01D23F510F40}",
            25,
            (0..25).collect(),
        );
        *factory_out = mem as u64;
    }
    println!("[DXGI ENGINE] CreateDXGIFactory2: Initialized DXGI 1.2 Factory");
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn EnumAdapters1(
    factory: u64,
    adapter_index: u32,
    pp_adapter: *mut u64,
) -> i32 {
    factory_enum_adapters1(
        factory as *mut std::ffi::c_void,
        adapter_index,
        pp_adapter as *mut *mut std::ffi::c_void,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn EnumAdapters(
    factory: u64,
    adapter_index: u32,
    pp_adapter: *mut u64,
) -> i32 {
    factory_enum_adapters(
        factory as *mut std::ffi::c_void,
        adapter_index,
        pp_adapter as *mut *mut std::ffi::c_void,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn IDXGISwapChain_Present(
    _swap_chain: u64,
    _sync_interval: u32,
    _flags: u32,
) -> i32 {
    let display = crate::windowing::x11::get_global_display();
    let window = crate::windowing::x11::get_global_window();
    if (display as usize) != 0 && window != 0 {
        crate::graphics::dxgi_vulkan::VulkanSwapChainPresenter::present_frame_to_screen(
            display, window,
        );
    }
    S_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dxgi_swapchain_present_and_frame_count() {
        unsafe {
            let mut swap_chain_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let res = factory_create_swap_chain_for_hwnd(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0x1234,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                &mut swap_chain_ptr,
            );
            assert_eq!(res, 0); // S_OK
            assert!(!swap_chain_ptr.is_null());

            // Check GetBuffer returns valid texture mock
            let mut buffer_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let res_buf =
                swapchain_get_buffer(swap_chain_ptr, 0, std::ptr::null(), &mut buffer_ptr);
            assert_eq!(res_buf, 0); // S_OK
            assert!(!buffer_ptr.is_null());

            // Call Present and verify return code S_OK (0)
            let prev_count = SWAPCHAIN_PRESENT_CALLS.load(Ordering::Relaxed);
            let res_present = swapchain_present(swap_chain_ptr, 1, 0);
            assert_eq!(res_present, 0); // S_OK
            assert_eq!(
                SWAPCHAIN_PRESENT_CALLS.load(Ordering::Relaxed),
                prev_count + 1
            );

            // Call Present1 and verify return code S_OK (0)
            let prev1_count = SWAPCHAIN_PRESENT1_CALLS.load(Ordering::Relaxed);
            let res_present1 = swapchain_present1(swap_chain_ptr, 1, 0, std::ptr::null());
            assert_eq!(res_present1, 0); // S_OK
            assert_eq!(
                SWAPCHAIN_PRESENT1_CALLS.load(Ordering::Relaxed),
                prev1_count + 1
            );
        }
    }
}
