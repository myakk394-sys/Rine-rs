#![allow(non_snake_case)]
use crate::sys::{NtStatus, STATUS_SUCCESS};

pub type IDXGIFactory = u64;
pub type IDXGIAdapter = u64;
pub type IDXGISwapChain = u64;

const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;

pub unsafe extern "win64" fn CreateDXGIFactory(_riid: u64, factory_out: *mut u64) -> i32 {
    println!("[DXGI ENTRY] CreateDXGIFactory");
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_adapter_vtable() as usize,
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
    0 // S_OK
}

pub unsafe extern "win64" fn CreateDXGIFactory1(_riid: u64, factory_out: *mut u64) -> i32 {
    println!("[DXGI ENTRY] CreateDXGIFactory1");
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_adapter_vtable() as usize,
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
    0 // S_OK
}

pub unsafe extern "win64" fn CreateDXGIFactory2(
    _flags: u32,
    _riid: u64,
    factory_out: *mut u64,
) -> i32 {
    println!("[DXGI ENTRY] CreateDXGIFactory2");
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_adapter_vtable() as usize,
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
    0 // S_OK
}

unsafe extern "win64" fn adapter_query_interface(
    this: *mut std::ffi::c_void,
    riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }

    *ppv = std::ptr::null_mut();
    if riid.is_null() || this.is_null() {
        return E_NOINTERFACE;
    }

    // Allow standard DXGI Factory & Adapter interfaces
    *ppv = this;
    crate::dlls::d3d11::com_object_add_ref(this);
    0
}

unsafe extern "win64" fn adapter_get_desc1(_this: *mut std::ffi::c_void, p_desc: *mut u8) -> i32 {
    if !p_desc.is_null() {
        std::ptr::write_bytes(p_desc, 0, 156);
    }
    0 // S_OK
}

unsafe extern "win64" fn dxgi_method_not_implemented(
    _this: *mut std::ffi::c_void,
    _a2: u64,
    _a3: u64,
    _a4: u64,
) -> i32 {
    E_NOTIMPL
}

static mut ADAPTER_VTABLE: [*const (); 25] = [std::ptr::null(); 25];

pub fn get_adapter_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        ADAPTER_VTABLE[0] = adapter_query_interface as *const ();
        ADAPTER_VTABLE[1] = crate::dlls::d3d11::com_object_add_ref as *const ();
        ADAPTER_VTABLE[2] = crate::dlls::d3d11::com_object_release as *const ();
        for i in 3..25 {
            ADAPTER_VTABLE[i] = dxgi_method_not_implemented as *const ();
        }
        ADAPTER_VTABLE[10] = adapter_get_desc1 as *const ();
    });
    unsafe { ADAPTER_VTABLE.as_ptr() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_query_interface_nulls_output_and_fails() {
        let object = crate::dlls::d3d11::allocate_mock_com_object(get_adapter_vtable() as usize);
        let unsupported_iid = [0xFFu8; 16];

        let result = unsafe {
            adapter_query_interface(object, unsupported_iid.as_ptr(), std::ptr::null_mut())
        };

        assert_eq!(result, E_NOINTERFACE);
        unsafe { crate::dlls::d3d11::com_object_release(object) };
    }

    #[test]
    fn unimplemented_factory_slot_is_not_success() {
        let vtable = get_adapter_vtable();
        let unsupported: unsafe extern "win64" fn(*mut std::ffi::c_void, u64, u64, u64) -> i32 =
            unsafe { std::mem::transmute(*vtable.add(3)) };

        assert_eq!(
            unsafe { unsupported(std::ptr::null_mut(), 0, 0, 0) },
            E_NOTIMPL
        );
    }
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
