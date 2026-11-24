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
    let d1 = u32::from_le_bytes([iid[0], iid[1], iid[2], iid[3]]);
    let d2 = u16::from_le_bytes([iid[4], iid[5]]);
    let d3 = u16::from_le_bytes([iid[6], iid[7]]);
    println!(
        "[D3D11 COM] QueryInterface: {{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        d1, d2, d3, iid[8], iid[9], iid[10], iid[11], iid[12], iid[13], iid[14], iid[15]
    );

    // IDXGIDevice: {54ec77fa-1377-44e6-8c32-88fd5f44c84c}
    // IDXGIDevice1: {77db970f-6276-48ba-ba28-070143b4392c}
    // IDXGIDevice2: {05008617-fbfd-4051-a790-144884b4f6a9}
    // IDXGIDevice3: {6007896c-3244-4afd-bf18-a6d3beda5023}
    // IDXGIDevice4: {9179e4c3-07ae-4dc4-8f0e-70cecbed4ed7}
    // IDXGIObject: {aec22fb8-76f3-4639-9be0-28eb43a67a2e}
    if d1 == 0x54ec77fa
        || d1 == 0x77db970f
        || d1 == 0x05008617
        || d1 == 0x6007896c
        || d1 == 0x9179e4c3
        || d1 == 0x50c83a1c
        || d1 == 0xaec22fb8
    {
        let dev = crate::dlls::dxgi::get_dxgi_device_object();
        *ppv = dev;
        return 0; // S_OK
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
    pub width: u32,              // +40
    pub height: u32,             // +44
    pub format: u32,             // +48
    pub mip_levels: u32,         // +52
    pub array_size: u32,         // +56
    pub bind_flags: u32,         // +60
    pub cpu_access_flags: u32,   // +64
    pub misc_flags: u32,         // +68
    pub extra_data: [u8; 176],
}

use std::sync::OnceLock;

pub fn get_d3d11_mapped_buffer_ptr() -> *mut std::ffi::c_void {
    static D3D11_BUF: OnceLock<usize> = OnceLock::new();
    let ptr = *D3D11_BUF.get_or_init(|| {
        let size = 64 * 1024 * 1024; // 64MB scratch mapped buffer
        let region = crate::mem::mmap::MmapRegion::allocate_anonymous(
            None,
            size,
            libc::PROT_READ | libc::PROT_WRITE,
        )
        .unwrap();
        let p = region.as_ptr() as usize;
        std::mem::forget(region);
        p
    });
    ptr as *mut std::ffi::c_void
}

pub fn allocate_mock_com_object_with_meta(
    vtable: usize,
    creator_symbol: &'static str,
    interface_name: &'static str,
    iid: &'static str,
    clsid: &'static str,
    _vtable_slots_count: usize,
    _implemented_slots: Vec<usize>,
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
            (*obj).mapped_pdata = get_d3d11_mapped_buffer_ptr() as usize;
            (*obj).mapped_row_pitch = 16384;
            (*obj).mapped_depth_pitch = 16384 * 4096;

            crate::dlls::object_registry::register_rine_object(
                ptr as u64,
                vtable as u64,
                creator_symbol,
                clsid,
                iid,
                interface_name,
                std::mem::size_of::<MockComObject>(),
                _vtable_slots_count,
                _implemented_slots,
            );
            return ptr;
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

use std::sync::atomic::{AtomicUsize, Ordering};

pub struct D3D11CallStats {
    pub create_buffer: AtomicUsize,
    pub create_texture1d: AtomicUsize,
    pub create_texture2d: AtomicUsize,
    pub create_texture3d: AtomicUsize,
    pub create_srv: AtomicUsize,
    pub create_uav: AtomicUsize,
    pub create_rtv: AtomicUsize,
    pub create_dsv: AtomicUsize,
    pub create_input_layout: AtomicUsize,
    pub create_vs: AtomicUsize,
    pub create_gs: AtomicUsize,
    pub create_ps: AtomicUsize,
    pub create_hs: AtomicUsize,
    pub create_ds: AtomicUsize,
    pub create_cs: AtomicUsize,
    pub create_sampler_state: AtomicUsize,
    pub create_blend_state: AtomicUsize,
    pub create_depth_stencil_state: AtomicUsize,
    pub create_rasterizer_state: AtomicUsize,
    pub create_query: AtomicUsize,
    pub check_ms_quality: AtomicUsize,
    pub get_feature_level: AtomicUsize,
    pub get_immediate_context: AtomicUsize,

    // Context
    pub ia_set_input_layout: AtomicUsize,
    pub ia_set_vertex_buffers: AtomicUsize,
    pub ia_set_index_buffer: AtomicUsize,
    pub ia_set_primitive_topology: AtomicUsize,
    pub vs_set_shader: AtomicUsize,
    pub ps_set_shader: AtomicUsize,
    pub gs_set_shader: AtomicUsize,
    pub vs_set_constant_buffers: AtomicUsize,
    pub ps_set_constant_buffers: AtomicUsize,
    pub vs_set_shader_resources: AtomicUsize,
    pub ps_set_shader_resources: AtomicUsize,
    pub vs_set_samplers: AtomicUsize,
    pub ps_set_samplers: AtomicUsize,
    pub om_set_render_targets: AtomicUsize,
    pub om_set_blend_state: AtomicUsize,
    pub om_set_depth_stencil_state: AtomicUsize,
    pub rs_set_state: AtomicUsize,
    pub rs_set_viewports: AtomicUsize,
    pub rs_set_scissor_rects: AtomicUsize,
    pub clear_rtv: AtomicUsize,
    pub clear_dsv: AtomicUsize,
    pub update_subresource: AtomicUsize,
    pub map: AtomicUsize,
    pub unmap: AtomicUsize,
    pub draw: AtomicUsize,
    pub draw_indexed: AtomicUsize,
    pub draw_instanced: AtomicUsize,
    pub draw_indexed_instanced: AtomicUsize,
}

pub static STATS: D3D11CallStats = D3D11CallStats {
    create_buffer: AtomicUsize::new(0),
    create_texture1d: AtomicUsize::new(0),
    create_texture2d: AtomicUsize::new(0),
    create_texture3d: AtomicUsize::new(0),
    create_srv: AtomicUsize::new(0),
    create_uav: AtomicUsize::new(0),
    create_rtv: AtomicUsize::new(0),
    create_dsv: AtomicUsize::new(0),
    create_input_layout: AtomicUsize::new(0),
    create_vs: AtomicUsize::new(0),
    create_gs: AtomicUsize::new(0),
    create_ps: AtomicUsize::new(0),
    create_hs: AtomicUsize::new(0),
    create_ds: AtomicUsize::new(0),
    create_cs: AtomicUsize::new(0),
    create_sampler_state: AtomicUsize::new(0),
    create_blend_state: AtomicUsize::new(0),
    create_depth_stencil_state: AtomicUsize::new(0),
    create_rasterizer_state: AtomicUsize::new(0),
    create_query: AtomicUsize::new(0),
    check_ms_quality: AtomicUsize::new(0),
    get_feature_level: AtomicUsize::new(0),
    get_immediate_context: AtomicUsize::new(0),

    ia_set_input_layout: AtomicUsize::new(0),
    ia_set_vertex_buffers: AtomicUsize::new(0),
    ia_set_index_buffer: AtomicUsize::new(0),
    ia_set_primitive_topology: AtomicUsize::new(0),
    vs_set_shader: AtomicUsize::new(0),
    ps_set_shader: AtomicUsize::new(0),
    gs_set_shader: AtomicUsize::new(0),
    vs_set_constant_buffers: AtomicUsize::new(0),
    ps_set_constant_buffers: AtomicUsize::new(0),
    vs_set_shader_resources: AtomicUsize::new(0),
    ps_set_shader_resources: AtomicUsize::new(0),
    vs_set_samplers: AtomicUsize::new(0),
    ps_set_samplers: AtomicUsize::new(0),
    om_set_render_targets: AtomicUsize::new(0),
    om_set_blend_state: AtomicUsize::new(0),
    om_set_depth_stencil_state: AtomicUsize::new(0),
    rs_set_state: AtomicUsize::new(0),
    rs_set_viewports: AtomicUsize::new(0),
    rs_set_scissor_rects: AtomicUsize::new(0),
    clear_rtv: AtomicUsize::new(0),
    clear_dsv: AtomicUsize::new(0),
    update_subresource: AtomicUsize::new(0),
    map: AtomicUsize::new(0),
    unmap: AtomicUsize::new(0),
    draw: AtomicUsize::new(0),
    draw_indexed: AtomicUsize::new(0),
    draw_instanced: AtomicUsize::new(0),
    draw_indexed_instanced: AtomicUsize::new(0),
};

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
    this: *mut std::ffi::c_void,
    p_resource: *mut std::ffi::c_void,
    subresource: u32,
    map_type: u32,
    map_flags: u32,
    p_mapped_resource: *mut D3D11_MAPPED_SUBRESOURCE,
) -> i32 {
    let c = STATS.map.fetch_add(1, Ordering::Relaxed);
    let buf = get_d3d11_mapped_buffer_ptr();
    if !p_mapped_resource.is_null() {
        let (row_pitch, depth_pitch) = if !p_resource.is_null() {
            let obj = p_resource as *mut MockComObject;
            if (*obj).mapped_row_pitch > 0 {
                ((*obj).mapped_row_pitch, (*obj).mapped_depth_pitch)
            } else {
                (8192, 8192 * 1024)
            }
        } else {
            (8192, 8192 * 1024)
        };

        (*p_mapped_resource).pData = buf;
        (*p_mapped_resource).RowPitch = row_pitch;
        (*p_mapped_resource).DepthPitch = depth_pitch;
    }
    println!(
        "[D3D11 CONTEXT] Map #{} this={:p}, res={:p}, subres={}, type={}, flags={}, out_mapped={:p}, buf={:p}",
        c + 1, this, p_resource, subresource, map_type, map_flags, p_mapped_resource, buf
    );
    0 // S_OK
}

pub unsafe extern "win64" fn d3d11_unmap_stub(
    _this: *mut std::ffi::c_void,
    _p_resource: *mut std::ffi::c_void,
    _subresource: u32,
) {
    let c = STATS.unmap.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 CONTEXT] Unmap #{}", c + 1);
}

unsafe extern "win64" fn d3d11_ok_stub(
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
    0 // S_OK
}

#[repr(C)]
pub struct D3D11_TEXTURE2D_DESC {
    pub Width: u32,
    pub Height: u32,
    pub MipLevels: u32,
    pub ArraySize: u32,
    pub Format: u32,
    pub SampleDesc_Count: u32,
    pub SampleDesc_Quality: u32,
    pub Usage: u32,
    pub BindFlags: u32,
    pub CPUAccessFlags: u32,
    pub MiscFlags: u32,
}

#[repr(C)]
pub struct D3D11_BUFFER_DESC {
    pub ByteWidth: u32,
    pub Usage: u32,
    pub BindFlags: u32,
    pub CPUAccessFlags: u32,
    pub MiscFlags: u32,
    pub StructureByteStride: u32,
}

unsafe extern "win64" fn texture2d_get_desc(
    this: *mut std::ffi::c_void,
    p_desc: *mut D3D11_TEXTURE2D_DESC,
) {
    if !this.is_null() && !p_desc.is_null() {
        let obj = this as *mut MockComObject;
        (*p_desc).Width = if (*obj).width > 0 { (*obj).width } else { 2048 };
        (*p_desc).Height = if (*obj).height > 0 {
            (*obj).height
        } else {
            1024
        };
        (*p_desc).MipLevels = if (*obj).mip_levels > 0 {
            (*obj).mip_levels
        } else {
            1
        };
        (*p_desc).ArraySize = if (*obj).array_size > 0 {
            (*obj).array_size
        } else {
            1
        };
        (*p_desc).Format = if (*obj).format > 0 { (*obj).format } else { 28 };
        (*p_desc).SampleDesc_Count = 1;
        (*p_desc).SampleDesc_Quality = 0;
        (*p_desc).Usage = 0;
        (*p_desc).BindFlags = if (*obj).bind_flags > 0 {
            (*obj).bind_flags
        } else {
            0x8
        };
        (*p_desc).CPUAccessFlags = (*obj).cpu_access_flags;
        (*p_desc).MiscFlags = (*obj).misc_flags;
        println!(
            "[D3D11 TEXTURE2D] GetDesc this={:p} ({}x{}, format={}, mips={})",
            this,
            (*p_desc).Width,
            (*p_desc).Height,
            (*p_desc).Format,
            (*p_desc).MipLevels
        );
    }
}

unsafe extern "win64" fn buffer_get_desc(
    this: *mut std::ffi::c_void,
    p_desc: *mut D3D11_BUFFER_DESC,
) {
    if !this.is_null() && !p_desc.is_null() {
        let obj = this as *mut MockComObject;
        (*p_desc).ByteWidth = if (*obj).width > 0 {
            (*obj).width
        } else {
            65536
        };
        (*p_desc).Usage = 0;
        (*p_desc).BindFlags = if (*obj).bind_flags > 0 {
            (*obj).bind_flags
        } else {
            0x4
        };
        (*p_desc).CPUAccessFlags = (*obj).cpu_access_flags;
        (*p_desc).MiscFlags = (*obj).misc_flags;
        (*p_desc).StructureByteStride = 0;
    }
}

static mut D3D11_TEXTURE2D_VTABLE: [*const (); 40] = [std::ptr::null(); 40];
static mut D3D11_BUFFER_VTABLE: [*const (); 40] = [std::ptr::null(); 40];

pub fn get_d3d11_texture2d_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        for i in 0..40 {
            D3D11_TEXTURE2D_VTABLE[i] = d3d11_ok_stub as *const ();
        }
        D3D11_TEXTURE2D_VTABLE[0] = d3d11_device_query_interface as *const ();
        D3D11_TEXTURE2D_VTABLE[1] = com_object_add_ref as *const ();
        D3D11_TEXTURE2D_VTABLE[2] = com_object_release as *const ();
        D3D11_TEXTURE2D_VTABLE[3] = d3d11_get_device_stub as *const ();
        D3D11_TEXTURE2D_VTABLE[10] = texture2d_get_desc as *const ();
    });
    unsafe { D3D11_TEXTURE2D_VTABLE.as_ptr() }
}

pub fn get_d3d11_buffer_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        for i in 0..40 {
            D3D11_BUFFER_VTABLE[i] = d3d11_ok_stub as *const ();
        }
        D3D11_BUFFER_VTABLE[0] = d3d11_device_query_interface as *const ();
        D3D11_BUFFER_VTABLE[1] = com_object_add_ref as *const ();
        D3D11_BUFFER_VTABLE[2] = com_object_release as *const ();
        D3D11_BUFFER_VTABLE[3] = d3d11_get_device_stub as *const ();
        D3D11_BUFFER_VTABLE[10] = buffer_get_desc as *const ();
    });
    unsafe { D3D11_BUFFER_VTABLE.as_ptr() }
}

unsafe extern "win64" fn d3d11_create_buffer_impl(
    _this: *mut std::ffi::c_void,
    desc: *const u32,
    _initial_data: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_buffer.fetch_add(1, Ordering::Relaxed);
    let mut byte_width = 65536u32;
    let mut bind_flags = 0x4u32;
    let mut cpu_access = 0u32;
    let mut misc_flags = 0u32;
    if !desc.is_null() {
        byte_width = *desc;
        bind_flags = *desc.add(2);
        cpu_access = *desc.add(3);
        misc_flags = *desc.add(4);
        println!(
            "[D3D11 DEVICE] CreateBuffer #{} size={}, bind={}",
            c + 1,
            byte_width,
            bind_flags
        );
    } else {
        println!("[D3D11 DEVICE] CreateBuffer #{}", c + 1);
    }
    if !pp_out.is_null() {
        let mem = allocate_mock_com_object(get_d3d11_buffer_vtable() as usize);
        if !mem.is_null() {
            let obj = mem as *mut MockComObject;
            (*obj).width = byte_width;
            (*obj).bind_flags = bind_flags;
            (*obj).cpu_access_flags = cpu_access;
            (*obj).misc_flags = misc_flags;
            (*obj).mapped_row_pitch = byte_width;
            (*obj).mapped_depth_pitch = byte_width;
        }
        *pp_out = mem;
    }
    0
}

unsafe extern "win64" fn d3d11_create_texture1d_impl(
    _this: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    _initial_data: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_texture1d.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateTexture1D #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_texture2d_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_texture2d_impl(
    _this: *mut std::ffi::c_void,
    desc: *const u32,
    _initial_data: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_texture2d.fetch_add(1, Ordering::Relaxed);
    let mut width = 2048u32;
    let mut height = 1024u32;
    let mut mip_levels = 1u32;
    let mut array_size = 1u32;
    let mut format = 28u32;
    let mut bind_flags = 0x8u32;
    let mut cpu_access = 0u32;
    let mut misc_flags = 0u32;

    if !desc.is_null() {
        width = *desc;
        height = *desc.add(1);
        mip_levels = *desc.add(2);
        array_size = *desc.add(3);
        format = *desc.add(4);
        bind_flags = *desc.add(6);
        cpu_access = *desc.add(7);
        misc_flags = *desc.add(8);
        println!(
            "[D3D11 DEVICE] CreateTexture2D #{} ({}x{}, format={}, mips={})",
            c + 1,
            width,
            height,
            format,
            mip_levels
        );
    } else {
        println!("[D3D11 DEVICE] CreateTexture2D #{}", c + 1);
    }
    if !pp_out.is_null() {
        let bpp = match format {
            1..=5 => 16,
            6..=9 => 12,
            10..=14 => 8,
            15..=18 => 8,
            27..=32 => 4,
            87 => 1,
            _ => 4,
        };
        let row_pitch = std::cmp::max(width * bpp, 256);
        let depth_pitch = row_pitch * std::cmp::max(height, 1);

        let mem = allocate_mock_com_object(get_d3d11_texture2d_vtable() as usize);
        if !mem.is_null() {
            let obj = mem as *mut MockComObject;
            (*obj).mapped_row_pitch = row_pitch;
            (*obj).mapped_depth_pitch = depth_pitch;
            (*obj).width = width;
            (*obj).height = height;
            (*obj).format = format;
            (*obj).mip_levels = mip_levels;
            (*obj).array_size = array_size;
            (*obj).bind_flags = bind_flags;
            (*obj).cpu_access_flags = cpu_access;
            (*obj).misc_flags = misc_flags;
        }
        *pp_out = mem;
    }
    0
}

unsafe extern "win64" fn d3d11_create_texture3d_impl(
    _this: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    _initial_data: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_texture3d.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateTexture3D #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_srv_impl(
    _this: *mut std::ffi::c_void,
    _res: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_srv.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateShaderResourceView #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_uav_impl(
    _this: *mut std::ffi::c_void,
    _res: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_uav.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateUnorderedAccessView #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_rtv_impl(
    _this: *mut std::ffi::c_void,
    _res: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_rtv.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 DEVICE] CreateRenderTargetView #{} res={:p}",
        c + 1,
        _res
    );
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_dsv_impl(
    _this: *mut std::ffi::c_void,
    _res: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_dsv.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateDepthStencilView #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_input_layout_impl(
    _this: *mut std::ffi::c_void,
    _descs: *const std::ffi::c_void,
    num: u32,
    _code: *const std::ffi::c_void,
    len: usize,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_input_layout.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 DEVICE] CreateInputLayout #{} elements={}, code_len={}",
        c + 1,
        num,
        len
    );
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_vs_impl(
    _this: *mut std::ffi::c_void,
    _code: *const std::ffi::c_void,
    len: usize,
    _linkage: *mut std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_vs.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateVertexShader #{} len={}", c + 1, len);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_gs_impl(
    _this: *mut std::ffi::c_void,
    _code: *const std::ffi::c_void,
    len: usize,
    _linkage: *mut std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_gs.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateGeometryShader #{} len={}", c + 1, len);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_ps_impl(
    _this: *mut std::ffi::c_void,
    _code: *const std::ffi::c_void,
    len: usize,
    _linkage: *mut std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_ps.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreatePixelShader #{} len={}", c + 1, len);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_hs_impl(
    _this: *mut std::ffi::c_void,
    _code: *const std::ffi::c_void,
    len: usize,
    _linkage: *mut std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_hs.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateHullShader #{} len={}", c + 1, len);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_ds_impl(
    _this: *mut std::ffi::c_void,
    _code: *const std::ffi::c_void,
    len: usize,
    _linkage: *mut std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_ds.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateDomainShader #{} len={}", c + 1, len);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_cs_impl(
    _this: *mut std::ffi::c_void,
    _code: *const std::ffi::c_void,
    len: usize,
    _linkage: *mut std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_cs.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateComputeShader #{} len={}", c + 1, len);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_blend_state_impl(
    _this: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_blend_state.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateBlendState #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_depth_stencil_state_impl(
    _this: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS
        .create_depth_stencil_state
        .fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateDepthStencilState #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_rasterizer_state_impl(
    _this: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS
        .create_rasterizer_state
        .fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateRasterizerState #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_sampler_state_impl(
    _this: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_sampler_state.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateSamplerState #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_create_query_impl(
    _this: *mut std::ffi::c_void,
    _desc: *const std::ffi::c_void,
    pp_out: *mut *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.create_query.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] CreateQuery #{}", c + 1);
    if !pp_out.is_null() {
        *pp_out = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    0
}

unsafe extern "win64" fn d3d11_check_ms_quality_levels(
    _this: *mut std::ffi::c_void,
    _format: u32,
    _sample_count: u32,
    p_num_quality_levels: *mut u32,
) -> i32 {
    let c = STATS.check_ms_quality.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 DEVICE] CheckMultisampleQualityLevels #{} format={}, samples={}",
        c + 1,
        _format,
        _sample_count
    );
    if !p_num_quality_levels.is_null() {
        *p_num_quality_levels = 1;
    }
    0
}

unsafe extern "win64" fn d3d11_get_feature_level(_this: *mut std::ffi::c_void) -> u32 {
    STATS.get_feature_level.fetch_add(1, Ordering::Relaxed);
    0xb000 // D3D_FEATURE_LEVEL_11_0
}

unsafe extern "win64" fn d3d11_get_immediate_context(
    _this: *mut std::ffi::c_void,
    pp_immediate_context: *mut *mut std::ffi::c_void,
) {
    STATS.get_immediate_context.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 DEVICE] GetImmediateContext");
    if !pp_immediate_context.is_null() {
        *pp_immediate_context = allocate_mock_com_object(get_d3d11_context_vtable() as usize);
    }
}

// Context method implementations
unsafe extern "win64" fn ctx_vs_set_constant_buffers(
    _this: *mut std::ffi::c_void,
    start: u32,
    num: u32,
    _buffers: *const *mut std::ffi::c_void,
) {
    let c = STATS
        .vs_set_constant_buffers
        .fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] VSSetConstantBuffers #{} start={}, num={}",
        c + 1,
        start,
        num
    );
}

unsafe extern "win64" fn ctx_ps_set_shader_resources(
    _this: *mut std::ffi::c_void,
    start: u32,
    num: u32,
    _views: *const *mut std::ffi::c_void,
) {
    let c = STATS
        .ps_set_shader_resources
        .fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] PSSetShaderResources #{} start={}, num={}",
        c + 1,
        start,
        num
    );
}

unsafe extern "win64" fn ctx_ps_set_shader(
    _this: *mut std::ffi::c_void,
    _ps: *mut std::ffi::c_void,
    _class_instances: *const *mut std::ffi::c_void,
    _num_instances: u32,
) {
    let c = STATS.ps_set_shader.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 CONTEXT] PSSetShader #{} shader={:p}", c + 1, _ps);
}

unsafe extern "win64" fn ctx_ps_set_samplers(
    _this: *mut std::ffi::c_void,
    start: u32,
    num: u32,
    _samplers: *const *mut std::ffi::c_void,
) {
    let c = STATS.ps_set_samplers.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] PSSetSamplers #{} start={}, num={}",
        c + 1,
        start,
        num
    );
}

unsafe extern "win64" fn ctx_vs_set_shader(
    _this: *mut std::ffi::c_void,
    _vs: *mut std::ffi::c_void,
    _class_instances: *const *mut std::ffi::c_void,
    _num_instances: u32,
) {
    let c = STATS.vs_set_shader.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 CONTEXT] VSSetShader #{} shader={:p}", c + 1, _vs);
}

unsafe extern "win64" fn ctx_draw_indexed(
    _this: *mut std::ffi::c_void,
    index_count: u32,
    start_index: u32,
    base_vertex: i32,
) {
    let c = STATS.draw_indexed.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] DrawIndexed #{} count={}, start={}, base={}",
        c + 1,
        index_count,
        start_index,
        base_vertex
    );
}

unsafe extern "win64" fn ctx_draw(
    _this: *mut std::ffi::c_void,
    vertex_count: u32,
    start_vertex: u32,
) {
    let c = STATS.draw.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] Draw #{} count={}, start={}",
        c + 1,
        vertex_count,
        start_vertex
    );
}

unsafe extern "win64" fn ctx_ps_set_constant_buffers(
    _this: *mut std::ffi::c_void,
    start: u32,
    num: u32,
    _buffers: *const *mut std::ffi::c_void,
) {
    let c = STATS
        .ps_set_constant_buffers
        .fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] PSSetConstantBuffers #{} start={}, num={}",
        c + 1,
        start,
        num
    );
}

unsafe extern "win64" fn ctx_ia_set_input_layout(
    _this: *mut std::ffi::c_void,
    _layout: *mut std::ffi::c_void,
) {
    let c = STATS.ia_set_input_layout.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] IASetInputLayout #{} layout={:p}",
        c + 1,
        _layout
    );
}

unsafe extern "win64" fn ctx_ia_set_vertex_buffers(
    _this: *mut std::ffi::c_void,
    start: u32,
    num: u32,
    _buffers: *const *mut std::ffi::c_void,
    _strides: *const u32,
    _offsets: *const u32,
) {
    let c = STATS.ia_set_vertex_buffers.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] IASetVertexBuffers #{} start={}, num={}",
        c + 1,
        start,
        num
    );
}

unsafe extern "win64" fn ctx_ia_set_index_buffer(
    _this: *mut std::ffi::c_void,
    _buffer: *mut std::ffi::c_void,
    format: u32,
    offset: u32,
) {
    let c = STATS.ia_set_index_buffer.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] IASetIndexBuffer #{} format={}, offset={}",
        c + 1,
        format,
        offset
    );
}

unsafe extern "win64" fn ctx_draw_indexed_instanced(
    _this: *mut std::ffi::c_void,
    index_count: u32,
    instance_count: u32,
    start_index: u32,
    base_vertex: i32,
    start_instance: u32,
) {
    let c = STATS.draw_indexed_instanced.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] DrawIndexedInstanced #{} idx_cnt={}, inst_cnt={}",
        c + 1,
        index_count,
        instance_count
    );
}

unsafe extern "win64" fn ctx_draw_instanced(
    _this: *mut std::ffi::c_void,
    vertex_count: u32,
    instance_count: u32,
    start_vertex: u32,
    start_instance: u32,
) {
    let c = STATS.draw_instanced.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] DrawInstanced #{} vtx_cnt={}, inst_cnt={}",
        c + 1,
        vertex_count,
        instance_count
    );
}

unsafe extern "win64" fn ctx_ia_set_primitive_topology(
    _this: *mut std::ffi::c_void,
    topology: u32,
) {
    let c = STATS
        .ia_set_primitive_topology
        .fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] IASetPrimitiveTopology #{} topology={}",
        c + 1,
        topology
    );
}

unsafe extern "win64" fn ctx_vs_set_shader_resources(
    _this: *mut std::ffi::c_void,
    start: u32,
    num: u32,
    _views: *const *mut std::ffi::c_void,
) {
    let c = STATS
        .vs_set_shader_resources
        .fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] VSSetShaderResources #{} start={}, num={}",
        c + 1,
        start,
        num
    );
}

unsafe extern "win64" fn ctx_vs_set_samplers(
    _this: *mut std::ffi::c_void,
    start: u32,
    num: u32,
    _samplers: *const *mut std::ffi::c_void,
) {
    let c = STATS.vs_set_samplers.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] VSSetSamplers #{} start={}, num={}",
        c + 1,
        start,
        num
    );
}

unsafe extern "win64" fn ctx_om_set_render_targets(
    _this: *mut std::ffi::c_void,
    num_views: u32,
    views: *const *mut std::ffi::c_void,
    depth_stencil: *mut std::ffi::c_void,
) -> i32 {
    let c = STATS.om_set_render_targets.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] OMSetRenderTargets #{} num_views={}, dsv={:p}",
        c + 1,
        num_views,
        depth_stencil
    );
    0
}

unsafe extern "win64" fn ctx_om_set_blend_state(
    _this: *mut std::ffi::c_void,
    _blend_state: *mut std::ffi::c_void,
    _blend_factor: *const f32,
    _sample_mask: u32,
) {
    let c = STATS.om_set_blend_state.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 CONTEXT] OMSetBlendState #{}", c + 1);
}

unsafe extern "win64" fn ctx_om_set_depth_stencil_state(
    _this: *mut std::ffi::c_void,
    _depth_stencil_state: *mut std::ffi::c_void,
    stencil_ref: u32,
) {
    let c = STATS
        .om_set_depth_stencil_state
        .fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] OMSetDepthStencilState #{} ref={}",
        c + 1,
        stencil_ref
    );
}

unsafe extern "win64" fn ctx_rs_set_state(
    _this: *mut std::ffi::c_void,
    _rasterizer_state: *mut std::ffi::c_void,
) {
    let c = STATS.rs_set_state.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 CONTEXT] RSSetState #{}", c + 1);
}

unsafe extern "win64" fn ctx_rs_set_viewports(
    _this: *mut std::ffi::c_void,
    num: u32,
    _viewports: *const std::ffi::c_void,
) {
    let c = STATS.rs_set_viewports.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 CONTEXT] RSSetViewports #{} num={}", c + 1, num);
}

unsafe extern "win64" fn ctx_rs_set_scissor_rects(
    _this: *mut std::ffi::c_void,
    num: u32,
    _rects: *const std::ffi::c_void,
) {
    let c = STATS.rs_set_scissor_rects.fetch_add(1, Ordering::Relaxed);
    println!("[D3D11 CONTEXT] RSSetScissorRects #{} num={}", c + 1, num);
}

unsafe extern "win64" fn ctx_update_subresource(
    _this: *mut std::ffi::c_void,
    _dst_res: *mut std::ffi::c_void,
    dst_subres: u32,
    _dst_box: *const std::ffi::c_void,
    _src_data: *const std::ffi::c_void,
    src_row_pitch: u32,
    src_depth_pitch: u32,
) {
    let c = STATS.update_subresource.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] UpdateSubresource #{} subres={}, row_pitch={}",
        c + 1,
        dst_subres,
        src_row_pitch
    );
}

unsafe extern "win64" fn ctx_clear_rtv(
    _this: *mut std::ffi::c_void,
    _rtv: *mut std::ffi::c_void,
    _color: *const f32,
) {
    let c = STATS.clear_rtv.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] ClearRenderTargetView #{} rtv={:p}",
        c + 1,
        _rtv
    );
}

unsafe extern "win64" fn ctx_clear_dsv(
    _this: *mut std::ffi::c_void,
    _dsv: *mut std::ffi::c_void,
    flags: u32,
    depth: f32,
    stencil: u8,
) {
    let c = STATS.clear_dsv.fetch_add(1, Ordering::Relaxed);
    println!(
        "[D3D11 CONTEXT] ClearDepthStencilView #{} flags={}, depth={}, stencil={}",
        c + 1,
        flags,
        depth,
        stencil
    );
}

static mut D3D11_DEVICE_VTABLE: [*const (); 60] = [std::ptr::null(); 60];
static mut D3D11_CONTEXT_VTABLE: [*const (); 140] = [std::ptr::null(); 140];

pub fn get_d3d11_device_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        for i in 0..60 {
            D3D11_DEVICE_VTABLE[i] = d3d11_ok_stub as *const ();
        }
        D3D11_DEVICE_VTABLE[0] = d3d11_device_query_interface as *const ();
        D3D11_DEVICE_VTABLE[1] = com_object_add_ref as *const ();
        D3D11_DEVICE_VTABLE[2] = com_object_release as *const ();
        D3D11_DEVICE_VTABLE[3] = d3d11_create_buffer_impl as *const (); // CreateBuffer (3)
        D3D11_DEVICE_VTABLE[4] = d3d11_create_texture1d_impl as *const (); // CreateTexture1D (4)
        D3D11_DEVICE_VTABLE[5] = d3d11_create_texture2d_impl as *const (); // CreateTexture2D (5)
        D3D11_DEVICE_VTABLE[6] = d3d11_create_texture3d_impl as *const (); // CreateTexture3D (6)
        D3D11_DEVICE_VTABLE[7] = d3d11_create_srv_impl as *const (); // CreateShaderResourceView (7)
        D3D11_DEVICE_VTABLE[8] = d3d11_create_uav_impl as *const (); // CreateUnorderedAccessView (8)
        D3D11_DEVICE_VTABLE[9] = d3d11_create_rtv_impl as *const (); // CreateRenderTargetView (9)
        D3D11_DEVICE_VTABLE[10] = d3d11_create_dsv_impl as *const (); // CreateDepthStencilView (10)
        D3D11_DEVICE_VTABLE[11] = d3d11_create_input_layout_impl as *const (); // CreateInputLayout (11)
        D3D11_DEVICE_VTABLE[12] = d3d11_create_vs_impl as *const (); // CreateVertexShader (12)
        D3D11_DEVICE_VTABLE[13] = d3d11_create_gs_impl as *const (); // CreateGeometryShader (13)
        D3D11_DEVICE_VTABLE[15] = d3d11_create_ps_impl as *const (); // CreatePixelShader (15)
        D3D11_DEVICE_VTABLE[16] = d3d11_create_hs_impl as *const (); // CreateHullShader (16)
        D3D11_DEVICE_VTABLE[17] = d3d11_create_ds_impl as *const (); // CreateDomainShader (17)
        D3D11_DEVICE_VTABLE[18] = d3d11_create_cs_impl as *const (); // CreateComputeShader (18)
        D3D11_DEVICE_VTABLE[20] = d3d11_create_blend_state_impl as *const (); // CreateBlendState (20)
        D3D11_DEVICE_VTABLE[21] = d3d11_create_depth_stencil_state_impl as *const (); // CreateDepthStencilState (21)
        D3D11_DEVICE_VTABLE[22] = d3d11_create_rasterizer_state_impl as *const (); // CreateRasterizerState (22)
        D3D11_DEVICE_VTABLE[23] = d3d11_create_sampler_state_impl as *const (); // CreateSamplerState (23)
        D3D11_DEVICE_VTABLE[24] = d3d11_create_query_impl as *const (); // CreateQuery (24)
        D3D11_DEVICE_VTABLE[30] = d3d11_check_ms_quality_levels as *const (); // CheckMultisampleQualityLevels (30)
        D3D11_DEVICE_VTABLE[37] = d3d11_get_feature_level as *const (); // GetFeatureLevel (37)
        D3D11_DEVICE_VTABLE[40] = d3d11_get_immediate_context as *const (); // GetImmediateContext (40)
    });
    unsafe { D3D11_DEVICE_VTABLE.as_ptr() }
}

pub fn get_d3d11_context_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        for i in 0..140 {
            D3D11_CONTEXT_VTABLE[i] = d3d11_ok_stub as *const ();
        }
        D3D11_CONTEXT_VTABLE[0] = d3d11_device_query_interface as *const ();
        D3D11_CONTEXT_VTABLE[1] = com_object_add_ref as *const ();
        D3D11_CONTEXT_VTABLE[2] = com_object_release as *const ();
        D3D11_CONTEXT_VTABLE[3] = d3d11_get_device_stub as *const ();
        D3D11_CONTEXT_VTABLE[7] = ctx_vs_set_constant_buffers as *const ();
        D3D11_CONTEXT_VTABLE[8] = ctx_ps_set_shader_resources as *const ();
        D3D11_CONTEXT_VTABLE[9] = ctx_ps_set_shader as *const ();
        D3D11_CONTEXT_VTABLE[10] = ctx_ps_set_samplers as *const ();
        D3D11_CONTEXT_VTABLE[11] = ctx_vs_set_shader as *const ();
        D3D11_CONTEXT_VTABLE[12] = ctx_draw_indexed as *const ();
        D3D11_CONTEXT_VTABLE[13] = ctx_draw as *const ();
        D3D11_CONTEXT_VTABLE[14] = d3d11_map_stub as *const ();
        D3D11_CONTEXT_VTABLE[15] = d3d11_unmap_stub as *const ();
        D3D11_CONTEXT_VTABLE[16] = ctx_ps_set_constant_buffers as *const ();
        D3D11_CONTEXT_VTABLE[17] = ctx_ia_set_input_layout as *const ();
        D3D11_CONTEXT_VTABLE[18] = ctx_ia_set_vertex_buffers as *const ();
        D3D11_CONTEXT_VTABLE[19] = ctx_ia_set_index_buffer as *const ();
        D3D11_CONTEXT_VTABLE[20] = ctx_draw_indexed_instanced as *const ();
        D3D11_CONTEXT_VTABLE[21] = ctx_draw_instanced as *const ();
        D3D11_CONTEXT_VTABLE[24] = ctx_ia_set_primitive_topology as *const ();
        D3D11_CONTEXT_VTABLE[25] = ctx_vs_set_shader_resources as *const ();
        D3D11_CONTEXT_VTABLE[26] = ctx_vs_set_samplers as *const ();
        D3D11_CONTEXT_VTABLE[33] = ctx_om_set_render_targets as *const ();
        D3D11_CONTEXT_VTABLE[35] = ctx_om_set_blend_state as *const ();
        D3D11_CONTEXT_VTABLE[36] = ctx_om_set_depth_stencil_state as *const ();
        D3D11_CONTEXT_VTABLE[43] = ctx_rs_set_state as *const ();
        D3D11_CONTEXT_VTABLE[44] = ctx_rs_set_viewports as *const ();
        D3D11_CONTEXT_VTABLE[45] = ctx_rs_set_scissor_rects as *const ();
        D3D11_CONTEXT_VTABLE[48] = ctx_update_subresource as *const ();
        D3D11_CONTEXT_VTABLE[50] = ctx_clear_rtv as *const ();
        D3D11_CONTEXT_VTABLE[53] = ctx_clear_dsv as *const ();
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
            SWAPCHAIN_VTABLE[i] = d3d11_ok_stub as *const ();
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
    if !_p_swap_chain_desc.is_null() {
        let desc = unsafe { &*(_p_swap_chain_desc as *const crate::dlls::dxgi::DXGI_SWAP_CHAIN_DESC) };
        let hwnd = desc.OutputWindow;
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
                        width: desc.BufferDesc.Width.max(1280),
                        height: desc.BufferDesc.Height.max(720),
                        visible: true,
                    },
                );
            }
        }
    }
    if !pp_device.is_null() {
        *pp_device = allocate_mock_com_object(get_d3d11_device_vtable() as usize);
    }
    if !pp_immediate_context.is_null() {
        *pp_immediate_context = allocate_mock_com_object(get_d3d11_context_vtable() as usize);
    }
    if !pp_swap_chain.is_null() {
        *pp_swap_chain = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            crate::dlls::dxgi::get_swapchain_vtable() as usize,
            "d3d11!D3D11CreateDeviceAndSwapChain",
            "IDXGISwapChain",
            "{310D36A0-D2E7-4C0A-AA04-6A9D23B8886A}",
            "{310D36A0-D2E7-4C0A-AA04-6A9D23B8886A}",
            50,
            (0..50).collect(),
        );
    }
    if !p_feature_level.is_null() {
        *p_feature_level = 0xb000; // D3D_FEATURE_LEVEL_11_0
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
        assert_eq!(feature, 0xb000);
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
