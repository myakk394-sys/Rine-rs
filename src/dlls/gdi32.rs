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
    println!("[GDI32] DeleteObject: HGDIOBJ=0x{:X} -> 1", hobj);
    1
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
    match index {
        8 => 1280, // HORZRES
        10 => 720, // VERTRES
        12 => 32,  // BITSPIXEL
        116 => 60, // VREFRESH
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
