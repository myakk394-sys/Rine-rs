#![allow(non_snake_case)]
use crate::sys::{NtStatus, STATUS_SUCCESS};

pub type IDXGIFactory = u64;
pub type IDXGIAdapter = u64;
pub type IDXGISwapChain = u64;

pub unsafe extern "win64" fn CreateDXGIFactory(_riid: u64, factory_out: *mut u64) -> i32 {
    println!("[DXGI ENTRY] CreateDXGIFactory");
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object(get_adapter_vtable() as usize);
        *factory_out = mem as u64;
    }
    println!("[DXGI ENGINE] CreateDXGIFactory: Initialized DXGI Factory");
    0 // S_OK
}

pub unsafe extern "win64" fn CreateDXGIFactory1(_riid: u64, factory_out: *mut u64) -> i32 {
    println!("[DXGI ENTRY] CreateDXGIFactory1");
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object(get_adapter_vtable() as usize);
        *factory_out = mem as u64;
    }
    println!("[DXGI ENGINE] CreateDXGIFactory1: Initialized DXGI 1.1 Factory");
    0 // S_OK
}

pub unsafe extern "win64" fn CreateDXGIFactory2(
    _flags: u32,
    _riid: u64,
    factory_out: *mut u64,
) -> i32 {
    println!("[DXGI ENTRY] CreateDXGIFactory2");
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object(get_adapter_vtable() as usize);
        *factory_out = mem as u64;
    }
    println!("[DXGI ENGINE] CreateDXGIFactory2: Initialized DXGI 1.2 Factory");
    0 // S_OK
}

unsafe extern "win64" fn adapter_query_interface(
    this: *mut std::ffi::c_void,
    _riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if !ppv.is_null() {
        *ppv = this;
        crate::dlls::d3d11::com_object_add_ref(this);
    }
    0
}

unsafe extern "win64" fn adapter_get_desc1(_this: *mut std::ffi::c_void, p_desc: *mut u8) -> i32 {
    if !p_desc.is_null() {
        std::ptr::write_bytes(p_desc, 0, 156);
    }
    0 // S_OK
}

static mut ADAPTER_VTABLE: [*const (); 15] = [std::ptr::null(); 15];

pub fn get_adapter_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        ADAPTER_VTABLE[0] = adapter_query_interface as *const ();
        ADAPTER_VTABLE[1] = crate::dlls::d3d11::com_object_add_ref as *const ();
        ADAPTER_VTABLE[2] = crate::dlls::d3d11::com_object_release as *const ();
        ADAPTER_VTABLE[10] = adapter_get_desc1 as *const ();
    });
    unsafe { ADAPTER_VTABLE.as_ptr() }
}

#[no_mangle]
pub unsafe extern "win64" fn EnumAdapters1(
    _factory: u64,
    adapter_index: u32,
    pp_adapter: *mut u64,
) -> i32 {
    if adapter_index > 0 {
        return -2003238908; // DXGI_ERROR_NOT_FOUND (0x887A0004)
    }
    if !pp_adapter.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object(get_adapter_vtable() as usize);
        *pp_adapter = mem as u64;
    }
    println!("[DXGI ENGINE] EnumAdapters1: Provided Virtual GPU Adapter #0 with valid VTABLE");
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn EnumAdapters(
    factory: u64,
    adapter_index: u32,
    pp_adapter: *mut u64,
) -> i32 {
    EnumAdapters1(factory, adapter_index, pp_adapter)
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
    0 // S_OK
}
