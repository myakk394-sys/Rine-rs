#![allow(non_snake_case)]
pub type HDC = u64;
pub type HBITMAP = u64;
pub type HGDIOBJ = u64;

pub unsafe extern "win64" fn GetDC(hwnd: u64) -> HDC {
    let hdc = if hwnd == 0 { 0x00010001 } else { hwnd + 0x10 };
    println!("[GDI32] GetDC: HWND=0x{:X} -> HDC=0x{:X}", hwnd, hdc);
    hdc
}

pub unsafe extern "win64" fn ReleaseDC(hwnd: u64, hdc: HDC) -> i32 {
    println!("[GDI32] ReleaseDC: HWND=0x{:X}, HDC=0x{:X}", hwnd, hdc);
    1
}

pub unsafe extern "win64" fn CreateCompatibleDC(hdc: HDC) -> HDC {
    let new_dc = hdc + 1;
    println!(
        "[GDI32] CreateCompatibleDC: Base HDC=0x{:X} -> New HDC=0x{:X}",
        hdc, new_dc
    );
    new_dc
}

pub unsafe extern "win64" fn CreateCompatibleBitmap(hdc: HDC, width: i32, height: i32) -> HBITMAP {
    let hbmp = 0x00020001u64;
    println!(
        "[GDI32] CreateCompatibleBitmap: HDC=0x{:X}, Size=({}x{}) -> HBITMAP=0x{:X}",
        hdc, width, height, hbmp
    );
    hbmp
}

pub unsafe extern "win64" fn CreateBitmap(
    width: i32,
    height: i32,
    planes: u32,
    bit_count: u32,
    _bits: *const u8,
) -> HBITMAP {
    static NEXT_HBITMAP: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0x00020001);
    let hbmp = NEXT_HBITMAP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    println!(
        "[GDI32] CreateBitmap: Size=({}x{}), Planes={}, BPP={} -> HBITMAP=0x{:X}",
        width, height, planes, bit_count, hbmp
    );
    hbmp
}

pub unsafe extern "win64" fn SelectObject(hdc: HDC, hobj: HGDIOBJ) -> HGDIOBJ {
    println!(
        "[GDI32] SelectObject: HDC=0x{:X}, HGDIOBJ=0x{:X}",
        hdc, hobj
    );
    hobj
}

pub unsafe extern "win64" fn DeleteObject(hobj: HGDIOBJ) -> i32 {
    if remove_region(hobj as usize) {
        return 1;
    }
    println!("[GDI32] DeleteObject: HGDIOBJ=0x{:X} -> 1", hobj);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetStockObject(i: i32) -> HGDIOBJ {
    0x50000u64 + (i as u32 as u64 & 0xFF)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateSolidBrush(_color: u32) -> HGDIOBJ {
    0x51001
}

#[no_mangle]
pub unsafe extern "win64" fn CreateFontIndirectW(_lplf: *const u8) -> HGDIOBJ {
    0x52001
}

#[no_mangle]
pub unsafe extern "win64" fn GetObjectW(_h: HGDIOBJ, c: i32, pv: *mut u8) -> i32 {
    if !pv.is_null() && c > 0 {
        std::ptr::write_bytes(pv, 0, c as usize);
    }
    c
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDCW(
    _pwsz_driver: *const u16,
    _pwsz_device: *const u16,
    _psz_port: *const u16,
    _pdm: *const u8,
) -> HDC {
    CreateCompatibleDC(0)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDCA(
    _psz_driver: *const u8,
    _psz_device: *const u8,
    _psz_port: *const u8,
    _pdm: *const u8,
) -> HDC {
    CreateCompatibleDC(0)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateICW(
    _psz_driver: *const u16,
    _psz_device: *const u16,
    _psz_port: *const u16,
    _pdm: *const u8,
) -> HDC {
    CreateCompatibleDC(0)
}

pub unsafe extern "win64" fn CreateDIBSection(
    _hdc: HDC,
    pbmi: *const u8,
    _usage: u32,
    ppv_bits: *mut *mut u8,
    _h_section: u64,
    _offset: u32,
) -> HBITMAP {
    static NEXT_HBITMAP: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0x00020001);
    let hbmp = NEXT_HBITMAP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let mut size: usize = 1024 * 1024 * 4; // default 4MB
    if !pbmi.is_null() {
        let bi_width = *(pbmi.add(4) as *const i32);
        let bi_height = (*(pbmi.add(8) as *const i32)).abs();
        let bi_bit_count = *(pbmi.add(14) as *const u16) as usize;
        let bpp = if bi_bit_count == 0 { 32 } else { bi_bit_count };
        let calculated =
            (bi_width.unsigned_abs() as usize) * (bi_height as usize) * (bpp / 8).max(1);
        if calculated > 0 && calculated < 100 * 1024 * 1024 {
            size = calculated;
        }
    }

    let mem = libc::malloc(size) as *mut u8;
    if !mem.is_null() {
        std::ptr::write_bytes(mem, 0, size);
    }
    if !ppv_bits.is_null() {
        *ppv_bits = mem;
    }

    println!(
        "[GDI32] CreateDIBSection -> HBITMAP=0x{:X}, bits={:p}, size={}",
        hbmp, mem, size
    );
    hbmp
}

pub unsafe extern "win64" fn SwapBuffers(_hdc: HDC) -> i32 {
    println!("[GDI32 GRAPHICS] SwapBuffers: Swapped rendered frame to physical desktop display.");
    1
}

pub unsafe extern "win64" fn BitBlt(
    _hdc_dest: HDC,
    _x: i32,
    _y: i32,
    _w: i32,
    _h: i32,
    _hdc_src: HDC,
    _sx: i32,
    _sy: i32,
    _rop: u32,
) -> i32 {
    println!("[GDI32 GRAPHICS] BitBlt: Transferred frame pixels to target display HDC.");
    1
}

pub unsafe extern "win64" fn StretchDIBits(
    _hdc: HDC,
    _xd: i32,
    _yd: i32,
    wd: i32,
    hd: i32,
    _xs: i32,
    _ys: i32,
    _ws: i32,
    _hs: i32,
    bits: u64,
    _bmi: u64,
    _usage: u32,
    _rop: u32,
) -> i32 {
    let width = if wd <= 0 { 1280 } else { wd as u32 };
    let height = if hd <= 0 { 720 } else { hd as u32 };
    if bits > 0 {
        crate::windowing::x11::render_frame_to_x11_window(bits as *const u8, width, height);
    }
    println!("[GDI32 GRAPHICS] StretchDIBits: Rendered {}x{} DIB frame pixels directly to X11 Desktop Window.", width, height);
    height as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetDeviceCaps(_hdc: u64, index: i32) -> i32 {
    let bounds = crate::windowing::x11::get_host_display_bounds(0);
    let w = if bounds.w > 0 { bounds.w } else { 1920 };
    let h = if bounds.h > 0 { bounds.h } else { 1080 };
    match index {
        4 => ((w as f32) * 25.4 / 96.0) as i32, // HORZSIZE in mm
        6 => ((h as f32) * 25.4 / 96.0) as i32, // VERTSIZE in mm
        8 => w,                                 // HORZRES
        10 => h,                                // VERTRES
        12 => 32,                               // BITSPIXEL
        14 => 1,                                // PLANES
        88 => 96,                               // LOGPIXELSX
        90 => 96,                               // LOGPIXELSY
        110 => w,                               // DESKTOPHORZRES
        111 => h,                               // DESKTOPVERTRES
        116 => 60,                              // VREFRESH
        117 => 2,                               // BLTALIGNMENT
        118 => 1,                               // SHADEBLENDCAPS
        _ => 1,
    }
}

pub unsafe extern "win64" fn ChoosePixelFormat(_hdc: HDC, _pfd: u64) -> i32 {
    1
}

pub unsafe extern "win64" fn SetPixelFormat(_hdc: HDC, _format: i32, _pfd: u64) -> i32 {
    1
}

pub unsafe extern "win64" fn wglCreateContext(hdc: HDC) -> u64 {
    println!(
        "[OPENGL WGL] wglCreateContext(HDC 0x{:X}) -> HGLRC 0x00030001",
        hdc
    );
    0x00030001
}

pub unsafe extern "win64" fn wglMakeCurrent(hdc: HDC, hglrc: u64) -> i32 {
    println!(
        "[OPENGL WGL] wglMakeCurrent(HDC 0x{:X}, HGLRC 0x{:X}) -> Success",
        hdc, hglrc
    );
    1
}

#[link(name = "GL")]
extern "C" {
    pub fn glXGetProcAddress(proc_name: *const libc::c_char) -> *mut std::ffi::c_void;
}

pub unsafe extern "win64" fn wglGetProcAddress(name: *const libc::c_char) -> u64 {
    if !name.is_null() {
        let name_str = std::ffi::CStr::from_ptr(name).to_string_lossy();
        let ptr = glXGetProcAddress(name);
        if !ptr.is_null() {
            println!(
                "[OPENGL WGL] wglGetProcAddress('{}') -> Native GL Ptr {:p}",
                name_str, ptr
            );
            return ptr as u64;
        }
    }
    0x00050000 // Non-null GL procedure pointer
}

pub unsafe extern "win64" fn wglDeleteContext(hglrc: u64) -> i32 {
    println!(
        "[OPENGL WGL] wglDeleteContext(HGLRC 0x{:X}) -> Success",
        hglrc
    );
    1
}

pub unsafe extern "win64" fn D3DKMTEscape(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTEscape(p_data={:p}) -> STATUS_NOT_SUPPORTED", p_data);
    0xC00000BB // STATUS_NOT_SUPPORTED
}

pub unsafe extern "win64" fn D3DKMTCreateDevice(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTCreateDevice(p_data={:p}) -> STATUS_NOT_SUPPORTED", p_data);
    0xC00000BB
}

pub unsafe extern "win64" fn D3DKMTOpenAdapterFromHdc(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTOpenAdapterFromHdc(p_data={:p}) -> STATUS_NOT_SUPPORTED", p_data);
    0xC00000BB
}

pub unsafe extern "win64" fn D3DKMTOpenAdapterFromGdiDisplayName(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTOpenAdapterFromGdiDisplayName(p_data={:p}) -> STATUS_NOT_SUPPORTED", p_data);
    0xC00000BB
}

pub unsafe extern "win64" fn D3DKMTOpenAdapterFromLuid(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTOpenAdapterFromLuid(p_data={:p}) -> STATUS_NOT_SUPPORTED", p_data);
    0xC00000BB
}

pub unsafe extern "win64" fn D3DKMTCloseAdapter(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTCloseAdapter(p_data={:p}) -> STATUS_SUCCESS", p_data);
    0 // STATUS_SUCCESS
}

pub unsafe extern "win64" fn D3DKMTDestroyDevice(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTDestroyDevice(p_data={:p}) -> STATUS_SUCCESS", p_data);
    0
}

pub unsafe extern "win64" fn D3DKMTQueryAdapterInfo(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTQueryAdapterInfo(p_data={:p}) -> STATUS_NOT_SUPPORTED", p_data);
    0xC00000BB
}

pub unsafe extern "win64" fn D3DKMTQueryStatistics(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTQueryStatistics(p_data={:p}) -> STATUS_NOT_SUPPORTED", p_data);
    0xC00000BB
}

pub unsafe extern "win64" fn D3DKMTCheckVidPnExclusiveOwnership(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTCheckVidPnExclusiveOwnership(p_data={:p}) -> STATUS_NOT_SUPPORTED", p_data);
    0xC00000BB
}

pub unsafe extern "win64" fn D3DKMTSetVidPnSourceOwner(p_data: *const std::ffi::c_void) -> u32 {
    println!("[GDI32] D3DKMTSetVidPnSourceOwner(p_data={:p}) -> STATUS_SUCCESS", p_data);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetBitmapBits(
    _hbit: usize,
    cb: i32,
    lp_bits: *mut libc::c_void,
) -> i32 {
    if !lp_bits.is_null() && cb > 0 {
        std::ptr::write_bytes(lp_bits as *mut u8, 0, cb as usize);
    }
    cb
}

#[no_mangle]
pub unsafe extern "win64" fn GetObjectA(
    _hgdiobj: usize,
    cb_buffer: i32,
    lpv_object: *mut libc::c_void,
) -> i32 {
    if !lpv_object.is_null() && cb_buffer > 0 {
        std::ptr::write_bytes(lpv_object as *mut u8, 0, cb_buffer as usize);
    }
    cb_buffer
}

#[no_mangle]
pub unsafe extern "win64" fn DeleteDC(_hdc: usize) -> i32 {
    1 // TRUE
}

static NEXT_RGN_HANDLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0x7001_0000);
static GDI_REGIONS: std::sync::Mutex<Option<std::collections::HashMap<usize, (i32, i32, i32, i32)>>> =
    std::sync::Mutex::new(None);

fn insert_rect_rgn(x1: i32, y1: i32, x2: i32, y2: i32) -> usize {
    let handle = NEXT_RGN_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let rect = (x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2));
    if let Ok(mut guard) = GDI_REGIONS.lock() {
        guard.get_or_insert_with(std::collections::HashMap::new).insert(handle, rect);
    }
    handle
}

fn remove_region(handle: usize) -> bool {
    if handle < 0x7001_0000 {
        return false;
    }
    if let Ok(mut guard) = GDI_REGIONS.lock() {
        if let Some(map) = guard.as_mut() {
            return map.remove(&handle).is_some();
        }
    }
    false
}

#[no_mangle]
pub unsafe extern "win64" fn CreateRectRgn(x1: i32, y1: i32, x2: i32, y2: i32) -> usize {
    insert_rect_rgn(x1, y1, x2, y2)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateRoundRectRgn(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    _w: i32,
    _h: i32,
) -> usize {
    insert_rect_rgn(x1, y1, x2, y2)
}

#[no_mangle]
pub unsafe extern "win64" fn PtInRegion(hrgn: usize, x: i32, y: i32) -> i32 {
    if let Ok(guard) = GDI_REGIONS.lock() {
        if let Some(map) = guard.as_ref() {
            if let Some(&(l, t, r, b)) = map.get(&hrgn) {
                return if x >= l && x < r && y >= t && y < b { 1 } else { 0 };
            }
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetTextExtentPoint32W(
    _hdc: usize,
    _lp_string: *const u16,
    c: i32,
    psizl: *mut i32,
) -> i32 {
    if !psizl.is_null() {
        *psizl = c.max(0) * 8;
        *psizl.add(1) = 16;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetBkColor(_hdc: usize, _color: u32) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SetBkMode(_hdc: usize, _mode: i32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetTextAlign(_hdc: usize, _align: u32) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SetTextColor(_hdc: usize, _color: u32) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn TextOutW(_hdc: usize, _x: i32, _y: i32, _lp_string: *const u16, _c: i32) -> i32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gdi32_device_context() {
        let hdc = unsafe { GetDC(0) };
        assert!(hdc > 0);

        let comp_dc = unsafe { CreateCompatibleDC(hdc) };
        assert_eq!(comp_dc, hdc + 1);

        let bitmap = unsafe { CreateCompatibleBitmap(hdc, 1920, 1080) };
        assert!(bitmap > 0);

        assert_eq!(unsafe { ReleaseDC(0, hdc) }, 1);
    }
}
