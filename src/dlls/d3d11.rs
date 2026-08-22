#![allow(non_snake_case)]
use crate::sys::{NtStatus, STATUS_SUCCESS};

pub type ID3D11Device = u64;
pub type ID3D11DeviceContext = u64;

const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;

#[no_mangle]
pub unsafe extern "win64" fn D3D11CreateDevice(
    p_adapter: *mut std::ffi::c_void,
    driver_type: i32,
    software: usize,
    flags: u32,
    p_feature_levels: *const i32,
    feature_levels: u32,
    sdk_version: u32,
    pp_device: *mut *mut std::ffi::c_void,
    p_feature_level: *mut i32,
    pp_immediate_context: *mut *mut std::ffi::c_void,
) -> i32 {
    println!("[D3D11 ENTRY] D3D11CreateDevice");
    D3D11CreateDeviceAndSwapChain(
        p_adapter,
        driver_type,
        software,
        flags,
        p_feature_levels,
        feature_levels,
        sdk_version,
        std::ptr::null(),
        std::ptr::null_mut(),
        pp_device,
        p_feature_level,
        pp_immediate_context,
    )
}

unsafe extern "win64" fn d3d11_device_query_interface(
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

    let iid = std::slice::from_raw_parts(riid, 16);
    const IID_IUNKNOWN: [u8; 16] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x46,
    ];
    if iid != IID_IUNKNOWN {
        return E_NOINTERFACE;
    }

    *ppv = this;
    com_object_add_ref(this);
    0
}

pub unsafe extern "win64" fn com_object_add_ref(this: *mut std::ffi::c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let obj = this as *mut MockComObject;
    let refcount_atomic =
        std::ptr::addr_of_mut!((*obj).refcount) as *const std::sync::atomic::AtomicU32;
    (*refcount_atomic).fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

pub unsafe extern "win64" fn com_object_release(this: *mut std::ffi::c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let obj = this as *mut MockComObject;
    let refcount_atomic =
        std::ptr::addr_of_mut!((*obj).refcount) as *const std::sync::atomic::AtomicU32;
    let old_ref = (*refcount_atomic).fetch_sub(1, std::sync::atomic::Ordering::Release);
    if old_ref <= 1 {
        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
        libc::free(this);
        return 0;
    }
    old_ref - 1
}

#[repr(C)]
pub struct D3D11_MAPPED_SUBRESOURCE {
    pub pData: *mut std::ffi::c_void,
    pub RowPitch: u32,
    pub DepthPitch: u32,
}

#[repr(C, align(16))]
pub struct MockComObject {
    pub vtable: usize,           // +0: Valid VTable pointer
    pub refcount: u32,           // +8: Real atomic reference count
    pub flags: u32,              // +12: Flags / Alignment padding
    pub device_ptr: usize,       // +16: Device pointer
    pub mapped_pdata: usize,     // +24: Mapped buffer pointer
    pub mapped_row_pitch: u32,   // +32
    pub mapped_depth_pitch: u32, // +36
    pub extra_data: [u8; 208],
}

pub fn allocate_mock_com_object_with_meta(
    vtable: usize,
    creator_symbol: &'static str,
    interface_name: &'static str,
    clsid: &str,
    iid: &str,
    vtable_slot_count: usize,
    initialized_slots: Vec<usize>,
) -> *mut std::ffi::c_void {
    let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    unsafe {
        let res = libc::posix_memalign(&mut ptr, 16, std::mem::size_of::<MockComObject>());
        if res == 0 && !ptr.is_null() {
            let obj = ptr as *mut MockComObject;
            std::ptr::write_bytes(ptr as *mut u8, 0, std::mem::size_of::<MockComObject>());
            (*obj).vtable = vtable;
            (*obj).refcount = 1;
            (*obj).flags = 0;
            (*obj).mapped_pdata = crate::dlls::registry::get_dummy_stub_buffer_ptr();
            (*obj).mapped_row_pitch = 4096;
            (*obj).mapped_depth_pitch = 4096;

            crate::dlls::object_registry::register_rine_object(
                ptr as u64,
                vtable as u64,
                creator_symbol,
                clsid,
                iid,
                interface_name,
                std::mem::size_of::<MockComObject>(),
                vtable_slot_count,
                initialized_slots,
            );
        }
    }
    ptr
}

pub fn allocate_mock_com_object(vtable: usize) -> *mut std::ffi::c_void {
    allocate_mock_com_object_with_meta(
        vtable,
        "d3d11/com_object",
        "IUnknown/ID3D11Object",
        "",
        "",
        30,
        vec![0, 1, 2],
    )
}

pub unsafe extern "win64" fn d3d11_get_device_stub(
    _this: *mut std::ffi::c_void,
    pp_device: *mut *mut std::ffi::c_void,
) {
    if !pp_device.is_null() {
        let vtable_ptr = get_d3d11_device_vtable() as usize;
        *pp_device = allocate_mock_com_object(vtable_ptr);
    }
}

pub unsafe extern "win64" fn d3d11_map_stub(
    _this: *mut std::ffi::c_void,
    _p_resource: *mut std::ffi::c_void,
    _subresource: u32,
    _map_type: u32,
    _map_flags: u32,
    p_mapped_resource: *mut D3D11_MAPPED_SUBRESOURCE,
) -> i32 {
    if !p_mapped_resource.is_null() {
        let dummy_buf = crate::dlls::registry::get_dummy_stub_buffer_ptr() as *mut std::ffi::c_void;
        (*p_mapped_resource).pData = dummy_buf;
        (*p_mapped_resource).RowPitch = 4096;
        (*p_mapped_resource).DepthPitch = 4096;
    }
    0 // S_OK
}

pub unsafe extern "win64" fn d3d11_unmap_stub(
    _this: *mut std::ffi::c_void,
    _p_resource: *mut std::ffi::c_void,
    _subresource: u32,
) {
    // Unmap successful
}

unsafe extern "win64" fn d3d11_method_not_implemented(
    _this: *mut std::ffi::c_void,
    _p1: usize,
    _p2: usize,
    _p3: usize,
    _p4: usize,
    _p5: usize,
    _p6: usize,
    _p7: usize,
    _p8: usize,
    _p9: usize,
) -> i32 {
    E_NOTIMPL
}

static mut D3D11_DEVICE_VTABLE: [*const (); 60] = [std::ptr::null(); 60];
static mut D3D11_CONTEXT_VTABLE: [*const (); 140] = [std::ptr::null(); 140];

fn get_d3d11_device_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        D3D11_DEVICE_VTABLE[0] = d3d11_device_query_interface as *const ();
        D3D11_DEVICE_VTABLE[1] = com_object_add_ref as *const ();
        D3D11_DEVICE_VTABLE[2] = com_object_release as *const ();
        for i in 3..60 {
            D3D11_DEVICE_VTABLE[i] = d3d11_method_not_implemented as *const ();
        }
        D3D11_DEVICE_VTABLE[40] = d3d11_get_device_stub as *const (); // GetImmediateContext (Index 40)
    });
    unsafe { D3D11_DEVICE_VTABLE.as_ptr() }
}

pub unsafe extern "win64" fn d3d11_om_set_render_targets_stub(
    _this: *mut std::ffi::c_void,
    _num_views: u32,
    _views: *const *mut std::ffi::c_void,
    _depth_stencil: *mut std::ffi::c_void,
) -> i32 {
    0 // S_OK
}

pub unsafe extern "win64" fn d3d11_rs_set_viewports_stub(
    _this: *mut std::ffi::c_void,
    _num_viewports: u32,
    _viewports: *const std::ffi::c_void,
) {
}

fn get_d3d11_context_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        D3D11_CONTEXT_VTABLE[0] = d3d11_device_query_interface as *const ();
        D3D11_CONTEXT_VTABLE[1] = com_object_add_ref as *const ();
        D3D11_CONTEXT_VTABLE[2] = com_object_release as *const ();
        D3D11_CONTEXT_VTABLE[3] = d3d11_get_device_stub as *const ();
        for i in 4..140 {
            D3D11_CONTEXT_VTABLE[i] = d3d11_method_not_implemented as *const ();
        }
        // Exact Wine ID3D11DeviceContext method indices from d3d11.idl
        D3D11_CONTEXT_VTABLE[14] = d3d11_map_stub as *const (); // Map method (Index 14)
        D3D11_CONTEXT_VTABLE[15] = d3d11_unmap_stub as *const (); // Unmap method (Index 15)
        D3D11_CONTEXT_VTABLE[33] = d3d11_om_set_render_targets_stub as *const (); // OMSetRenderTargets (Index 33)
        D3D11_CONTEXT_VTABLE[44] = d3d11_rs_set_viewports_stub as *const (); // RSSetViewports (Index 44)
    });
    unsafe { D3D11_CONTEXT_VTABLE.as_ptr() }
}

unsafe extern "win64" fn swapchain_present_stub(
    _this: *mut std::ffi::c_void,
    _sync_interval: u32,
    _flags: u32,
) -> i32 {
    let dummy_buf = crate::dlls::registry::get_dummy_stub_buffer_ptr() as *const u8;
    crate::windowing::x11::render_frame_to_x11_window(dummy_buf, 1280, 720);
    0 // S_OK
}

unsafe extern "win64" fn swapchain_get_buffer_stub(
    _this: *mut std::ffi::c_void,
    _buffer_idx: u32,
    _riid: *const std::ffi::c_void,
    surface: *mut *mut std::ffi::c_void,
) -> i32 {
    if !surface.is_null() {
        let vtable_ptr = get_d3d11_device_vtable() as usize;
        *surface = allocate_mock_com_object(vtable_ptr);
    }
    0 // S_OK
}

pub unsafe extern "win64" fn swapchain_get_device_stub(
    _this: *mut std::ffi::c_void,
    _riid: *const std::ffi::c_void,
    pp_device: *mut *mut std::ffi::c_void,
) -> i32 {
    if !pp_device.is_null() {
        let vtable_ptr = get_d3d11_device_vtable() as usize;
        *pp_device = allocate_mock_com_object(vtable_ptr);
    }
    0 // S_OK
}

pub unsafe extern "win64" fn swapchain_resize_buffers_stub(
    _this: *mut std::ffi::c_void,
    _buffer_count: u32,
    _width: u32,
    _height: u32,
    _format: u32,
    _flags: u32,
) -> i32 {
    0 // S_OK
}

static mut SWAPCHAIN_VTABLE: [*const (); 30] = [std::ptr::null(); 30];

fn get_swap_chain_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        SWAPCHAIN_VTABLE[0] = d3d11_device_query_interface as *const ();
        SWAPCHAIN_VTABLE[1] = com_object_add_ref as *const ();
        SWAPCHAIN_VTABLE[2] = com_object_release as *const ();
        for i in 3..30 {
            SWAPCHAIN_VTABLE[i] = d3d11_method_not_implemented as *const ();
        }
        SWAPCHAIN_VTABLE[7] = swapchain_get_device_stub as *const (); // GetDevice (Index 7)
        SWAPCHAIN_VTABLE[8] = swapchain_present_stub as *const (); // Present (Index 8)
        SWAPCHAIN_VTABLE[9] = swapchain_get_buffer_stub as *const (); // GetBuffer (Index 9)
        SWAPCHAIN_VTABLE[13] = swapchain_resize_buffers_stub as *const (); // ResizeBuffers (Index 13)
    });
    unsafe { SWAPCHAIN_VTABLE.as_ptr() }
}

#[no_mangle]
pub unsafe extern "win64" fn D3D11CreateDeviceAndSwapChain(
    _p_adapter: *mut std::ffi::c_void,
    _driver_type: i32,
    _software: usize,
    _flags: u32,
    _p_feature_levels: *const i32,
    _feature_levels: u32,
    _sdk_version: u32,
    _p_swap_chain_desc: *const u8,
    pp_swap_chain: *mut *mut std::ffi::c_void,
    pp_device: *mut *mut std::ffi::c_void,
    p_feature_level: *mut i32,
    pp_immediate_context: *mut *mut std::ffi::c_void,
) -> i32 {
    println!("[D3D11 ENTRY] D3D11CreateDeviceAndSwapChain");
    if !pp_device.is_null() {
        *pp_device = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    if !pp_immediate_context.is_null() {
        *pp_immediate_context = allocate_mock_com_object(get_d3d11_context_vtable() as usize);
    }
    if !pp_swap_chain.is_null() {
        *pp_swap_chain = allocate_mock_com_object(get_swap_chain_vtable() as usize);
    }
    if !p_feature_level.is_null() {
        *p_feature_level = 0xb100; // D3D_FEATURE_LEVEL_11_0
    }
    println!("[D3D11 ENGINE] D3D11CreateDeviceAndSwapChain: Created Virtual D3D11 Device, SwapChain & Context with VTABLE (S_OK)");
    0 // S_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_d3d11_device_creation() {
        let mut dev: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut ctx: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut feature = 0;

        let status = unsafe {
            D3D11CreateDevice(
                std::ptr::null_mut(),
                1,
                0,
                0,
                std::ptr::null(),
                0,
                7,
                &mut dev,
                &mut feature,
                &mut ctx,
            )
        };
        assert_eq!(status, 0);
        assert!(!dev.is_null());
        assert!(!ctx.is_null());
        assert_eq!(feature, 0xb100);
        unsafe {
            com_object_release(dev);
            com_object_release(ctx);
        }
    }

    #[test]
    fn test_mock_com_object_refcount_layout() {
        assert_eq!(std::mem::offset_of!(MockComObject, vtable), 0);
        assert_eq!(std::mem::offset_of!(MockComObject, refcount), 8);

        let vtable_dummy = 0x12345678usize;
        let mock_ptr = allocate_mock_com_object(vtable_dummy);
        assert!(!mock_ptr.is_null());

        unsafe {
            let obj = mock_ptr as *mut MockComObject;
            assert_eq!((*obj).vtable, vtable_dummy);
            assert_eq!((*obj).refcount, 1);

            let ref1 = com_object_add_ref(mock_ptr);
            assert_eq!(ref1, 2);
            assert_eq!((*obj).refcount, 2);

            let ref2 = com_object_release(mock_ptr);
            assert_eq!(ref2, 1);
            assert_eq!((*obj).refcount, 1);

            let ref3 = com_object_release(mock_ptr);
            assert_eq!(ref3, 0);
        }
    }
}
