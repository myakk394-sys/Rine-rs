#![allow(non_snake_case)]

pub type HRESULT = i32;

pub unsafe extern "win64" fn CoInitializeEx(_pv_reserved: u64, _dw_co_init: u32) -> HRESULT {
    0 // S_OK
}

pub unsafe extern "win64" fn CoUninitialize() {}

pub unsafe extern "win64" fn CoCreateInstance(
    rclsid: *const u8,
    _p_unk_outer: *mut std::ffi::c_void,
    _dw_cls_context: u32,
    _riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if !ppv.is_null() {
        *ppv = std::ptr::null_mut();
    }
    if !rclsid.is_null() {
        let clsid_slice = std::slice::from_raw_parts(rclsid, 16);
        if clsid_slice == crate::dlls::wasapi::CLSID_MM_DEVICE_ENUMERATOR {
            return crate::dlls::wasapi::create_mm_device_enumerator(ppv);
        }
    }
    0x80040154u32 as i32 // REGDB_E_CLASSNOTREG
}

#[no_mangle]
pub unsafe extern "win64" fn SysAllocString(psz: *const u16) -> *mut u16 {
    if psz.is_null() {
        return std::ptr::null_mut();
    }
    let mut len = 0;
    while *psz.add(len) != 0 {
        len += 1;
    }
    let total_bytes = 4 + (len + 1) * 2;
    let mem = libc::calloc(1, total_bytes) as *mut u8;
    if mem.is_null() {
        return std::ptr::null_mut();
    }
    *(mem as *mut u32) = (len * 2) as u32;
    let bstr = mem.add(4) as *mut u16;
    std::ptr::copy_nonoverlapping(psz, bstr, len + 1);
    bstr
}

#[no_mangle]
pub unsafe extern "win64" fn SysFreeString(bstr: *mut u16) {
    if !bstr.is_null() {
        let mem = (bstr as *mut u8).sub(4);
        libc::free(mem as *mut std::ffi::c_void);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CoTaskMemFree(pv: *mut std::ffi::c_void) {
    if !pv.is_null() {
        libc::free(pv);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn PropVariantClear(pvar: *mut u8) -> i32 {
    if !pvar.is_null() {
        std::ptr::write_bytes(pvar, 0, 24);
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn VariantInit(pvar: *mut u8) {
    if !pvar.is_null() {
        std::ptr::write_bytes(pvar, 0, 24);
    }
}

pub unsafe extern "win64" fn ShellExecuteW(
    _hwnd: u64,
    _operation: u64,
    _file: u64,
    _parameters: u64,
    _directory: u64,
    _show_cmd: i32,
) -> u64 {
    42 // Successful instance handle (> 32)
}

#[no_mangle]
pub unsafe extern "win64" fn DragAcceptFiles(hwnd: usize, f_accept: i32) {
    println!(
        "[SHELL32] DragAcceptFiles(hWnd=0x{:X}, fAccept={})",
        hwnd, f_accept
    );
}

#[no_mangle]
pub unsafe extern "win64" fn DragFinish(_h_drop: usize) {}

#[no_mangle]
pub unsafe extern "win64" fn DragQueryFileW(
    _h_drop: usize,
    _i_file: u32,
    _lpsz_file: *mut u16,
    _cch: u32,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn DragQueryFileA(
    _h_drop: usize,
    _i_file: u32,
    _lpsz_file: *mut u8,
    _cch: u32,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn DragQueryPoint(_h_drop: usize, _lppt: *mut std::ffi::c_void) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn rine_shell_folder_stub(
    _hwnd: *mut std::ffi::c_void,
    _csidl: i32,
    _h_token: *mut std::ffi::c_void,
    _flags: u32,
    psz_path: *mut u16,
) -> i32 {
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        std::ptr::copy_nonoverlapping(path.as_ptr(), psz_path, path.len());
    }
    0 // S_OK
}

fn ensure_appdata_dir_exists() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/fizzy".to_string());
    let host_dir = format!("{}/AppData/Roaming", home);
    std::fs::create_dir_all(&host_dir).ok();
}

pub unsafe extern "win64" fn SHGetFolderPathW(
    _hwnd: u64,
    _csidl: i32,
    _h_token: u64,
    _dw_flags: u32,
    psz_path: *mut u16,
) -> i32 {
    ensure_appdata_dir_exists();
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming";
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        std::ptr::copy_nonoverlapping(wide.as_ptr(), psz_path, wide.len());
    }
    0 // S_OK
}

pub unsafe extern "win64" fn SHGetFolderPathA(
    _hwnd: u64,
    _csidl: i32,
    _h_token: u64,
    _dw_flags: u32,
    psz_path: *mut u8,
) -> i32 {
    ensure_appdata_dir_exists();
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming\0";
        std::ptr::copy_nonoverlapping(path.as_ptr(), psz_path, path.len());
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn CoTaskMemAlloc(cb: usize) -> *mut std::ffi::c_void {
    let size = if cb < 16 { 16 } else { (cb + 15) & !15 };
    let ptr = libc::calloc(1, size);
    if ptr.is_null() {
        static FALLBACK_COTASK: [u8; 4096] = [0; 4096];
        FALLBACK_COTASK.as_ptr() as *mut std::ffi::c_void
    } else {
        ptr
    }
}

pub unsafe extern "win64" fn SHGetKnownFolderPath(
    _rfid: u64,
    _dw_flags: u32,
    _h_token: u64,
    ppsz_path: *mut *mut u16,
) -> i32 {
    ensure_appdata_dir_exists();
    if !ppsz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming";
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        let buf = CoTaskMemAlloc(bytes) as *mut u16;
        if !buf.is_null() {
            std::ptr::copy_nonoverlapping(wide.as_ptr(), buf, wide.len());
            *ppsz_path = buf;
        }
    }
    0 // S_OK
}

pub unsafe extern "win64" fn SHGetSpecialFolderPathW(
    _hwnd: u64,
    psz_path: *mut u16,
    _csidl: i32,
    _f_create: i32,
) -> i32 {
    ensure_appdata_dir_exists();
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming";
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        std::ptr::copy_nonoverlapping(wide.as_ptr(), psz_path, wide.len());
    }
    1 // TRUE
}

pub unsafe extern "win64" fn SHGetSpecialFolderPathA(
    _hwnd: u64,
    psz_path: *mut u8,
    _csidl: i32,
    _f_create: i32,
) -> i32 {
    if !psz_path.is_null() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming\0";
        std::ptr::copy_nonoverlapping(path.as_ptr(), psz_path, path.len());
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CommandLineToArgvW(
    _lp_cmd_line: *const u16,
    p_num_args: *mut i32,
) -> *mut *mut u16 {
    if !p_num_args.is_null() {
        *p_num_args = 1;
    }
    let ptr_array = libc::calloc(2, std::mem::size_of::<*mut u16>()) as *mut *mut u16;
    let arg0 = "That's not my neighbor.exe\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let buf = libc::calloc(arg0.len(), 2) as *mut u16;
    std::ptr::copy_nonoverlapping(arg0.as_ptr(), buf, arg0.len());
    *ptr_array = buf;
    ptr_array
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::imports::IatSymbolResolver;

    #[test]
    fn executable_co_initialize_ex_preserves_win64_nonvolatile_registers() {
        let fn_addr = IatSymbolResolver::resolve_symbol("ole32.dll", "CoInitializeEx")
            .expect("CoInitializeEx must resolve from IAT resolver table");
        let fn_ptr: unsafe extern "win64" fn(*mut std::ffi::c_void, u32) -> i32 =
            unsafe { std::mem::transmute(fn_addr) };

        let res1: i32;
        let res2: i32;
        let mut saved_regs = [0u64; 7];
        let ptr_val = saved_regs.as_mut_ptr();

        unsafe {
            std::arch::asm!(
                "mov rax, {fn_ptr}",
                "mov r10, {ptr}",

                "push rbx",
                "push rbp",
                "push rsi",
                "push rdi",
                "push r12",
                "push r13",
                "push r14",
                "push r15",

                "mov rbp, 0x1111222233334444",
                "mov rsi, 0x5555666677778888",
                "mov rdi, 0x9999AAAABBBBCCCC",
                "mov r12, 0xDDDDEEEEFFFF0000",
                "mov r13, 0x123456789ABCDEF0",
                "mov r14, 0x0FEDCBA987654321",
                "mov r15, 0xA1B2C3D4E5F60718",

                "xor ecx, ecx",
                "mov edx, 2",
                "sub rsp, 0x20",
                "call rax",
                "add rsp, 0x20",

                "mov {res1:e}, eax",

                "mov [r10 + 0x00], rbp",
                "mov [r10 + 0x08], rsi",
                "mov [r10 + 0x10], rdi",
                "mov [r10 + 0x18], r12",
                "mov [r10 + 0x20], r13",
                "mov [r10 + 0x28], r14",
                "mov [r10 + 0x30], r15",

                "pop r15",
                "pop r14",
                "pop r13",
                "pop r12",
                "pop rdi",
                "pop rsi",
                "pop rbp",
                "pop rbx",
                fn_ptr = in(reg) fn_ptr,
                ptr = in(reg) ptr_val,
                res1 = out(reg) res1,
                out("rax") _,
                out("r10") _,
            );
        }

        res2 = unsafe { fn_ptr(std::ptr::null_mut(), 2) };

        assert_eq!(res1, 0, "CoInitializeEx first call must return S_OK (0)");
        assert_eq!(
            res2, 0,
            "CoInitializeEx repeated call on same thread must return S_OK (0)"
        );
        assert_eq!(saved_regs[0], 0x1111222233334444, "RBP clobbered!");
        assert_eq!(saved_regs[1], 0x5555666677778888, "RSI clobbered!");
        assert_eq!(saved_regs[2], 0x9999AAAABBBBCCCC, "RDI clobbered!");
        assert_eq!(saved_regs[3], 0xDDDDEEEEFFFF0000, "R12 clobbered!");
        assert_eq!(saved_regs[4], 0x123456789ABCDEF0, "R13 clobbered!");
        assert_eq!(saved_regs[5], 0x0FEDCBA987654321, "R14 clobbered!");
        assert_eq!(saved_regs[6], 0xA1B2C3D4E5F60718, "R15 clobbered!");

        let uninit_addr = IatSymbolResolver::resolve_symbol("ole32.dll", "CoUninitialize")
            .expect("CoUninitialize must resolve from IAT resolver table");
        let uninit_ptr: unsafe extern "win64" fn() = unsafe { std::mem::transmute(uninit_addr) };
        unsafe { uninit_ptr() };
    }
}
