#![allow(non_snake_case)]

pub type HRESULT = i32;

pub unsafe extern "win64" fn CoInitializeEx(_pv_reserved: u64, _dw_co_init: u32) -> HRESULT {
    0 // S_OK
}

pub unsafe extern "win64" fn CoUninitialize() {}

#[no_mangle]
pub unsafe extern "win64" fn RoInitialize(_init_type: u32) -> HRESULT {
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn RoUninitialize() {}

#[no_mangle]
pub unsafe extern "win64" fn RoActivateInstance(
    _activatable_class_id: *const libc::c_void,
    instance: *mut *mut libc::c_void,
) -> HRESULT {
    if !instance.is_null() {
        *instance = std::ptr::null_mut();
    }
    0x80004001u32 as i32 // E_NOTIMPL
}

#[no_mangle]
pub unsafe extern "win64" fn RoGetActivationFactory(
    _activatable_class_id: *const libc::c_void,
    _iid: *const libc::c_void,
    factory: *mut *mut libc::c_void,
) -> HRESULT {
    if !factory.is_null() {
        *factory = std::ptr::null_mut();
    }
    0x80040111u32 as i32 // CLASS_E_CLASSNOTAVAILABLE
}

#[no_mangle]
pub unsafe extern "win64" fn WindowsCreateString(
    source_string: *const u16,
    length: u32,
    string: *mut *mut libc::c_void,
) -> HRESULT {
    if string.is_null() {
        return 0x80070057u32 as i32; // E_INVALIDARG
    }
    if source_string.is_null() || length == 0 {
        *string = std::ptr::null_mut();
        return 0; // S_OK
    }
    let total_bytes = 4 + (length as usize + 1) * 2;
    let mem = libc::calloc(1, total_bytes) as *mut u8;
    if mem.is_null() {
        return 0x8007000Eu32 as i32; // E_OUTOFMEMORY
    }
    *(mem as *mut u32) = length;
    let raw = mem.add(4) as *mut u16;
    std::ptr::copy_nonoverlapping(source_string, raw, length as usize);
    *raw.add(length as usize) = 0;
    *string = raw as *mut libc::c_void;
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn WindowsDeleteString(string: *mut libc::c_void) -> HRESULT {
    if !string.is_null() {
        let mem = (string as *mut u8).sub(4);
        libc::free(mem as *mut libc::c_void);
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn WindowsGetStringRawBuffer(
    string: *mut libc::c_void,
    length: *mut u32,
) -> *const u16 {
    if string.is_null() {
        if !length.is_null() {
            *length = 0;
        }
        static EMPTY: [u16; 1] = [0];
        return EMPTY.as_ptr();
    }
    let mem = (string as *mut u8).sub(4);
    let len = *(mem as *const u32);
    if !length.is_null() {
        *length = len;
    }
    string as *const u16
}

#[no_mangle]
pub unsafe extern "win64" fn WindowsCreateStringReference(
    source_string: *const u16,
    length: u32,
    _hstring_header: *mut libc::c_void,
    string: *mut *mut libc::c_void,
) -> HRESULT {
    WindowsCreateString(source_string, length, string)
}

#[no_mangle]
pub unsafe extern "win64" fn WindowsDuplicateString(
    string: *mut libc::c_void,
    new_string: *mut *mut libc::c_void,
) -> HRESULT {
    if new_string.is_null() {
        return 0x80070057u32 as i32; // E_INVALIDARG
    }
    if string.is_null() {
        *new_string = std::ptr::null_mut();
        return 0;
    }
    let mut len = 0u32;
    let ptr = WindowsGetStringRawBuffer(string, &mut len);
    WindowsCreateString(ptr, len, new_string)
}

#[no_mangle]
pub unsafe extern "win64" fn RoSetErrorReportingFlags(_flags: u32) -> HRESULT {
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn GetRestrictedErrorInfo(
    pp_restricted_error_info: *mut *mut libc::c_void,
) -> HRESULT {
    if !pp_restricted_error_info.is_null() {
        *pp_restricted_error_info = std::ptr::null_mut();
    }
    1 // S_FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn RoOriginateError(_error: i32, _message: *mut libc::c_void) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn RoOriginateErrorW(
    _error: i32,
    _cch_max: u32,
    _message: *const u16,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn RoGetBufferMarshaler(
    buffer_marshaler: *mut *mut libc::c_void,
) -> HRESULT {
    if !buffer_marshaler.is_null() {
        *buffer_marshaler = std::ptr::null_mut();
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn CoGetApartmentType(
    p_apt_type: *mut i32,
    p_apt_qualifier: *mut i32,
) -> HRESULT {
    if !p_apt_type.is_null() {
        *p_apt_type = 1; // APTTYPE_MTA
    }
    if !p_apt_qualifier.is_null() {
        *p_apt_qualifier = 0; // APTTYPEQUALIFIER_NONE
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn CoCreateFreeThreadedMarshaler(
    _punk_outer: *mut libc::c_void,
    ppunk_marshaler: *mut *mut libc::c_void,
) -> HRESULT {
    if !ppunk_marshaler.is_null() {
        *ppunk_marshaler = std::ptr::null_mut();
    }
    0 // S_OK
}

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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SafeArrayBound {
    pub c_elements: u32,
    pub l_bound: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SafeArray {
    pub c_dims: u16,
    pub f_features: u16,
    pub cb_elements: u32,
    pub c_locks: u32,
    pub pv_data: *mut libc::c_void,
    pub rgsabound: [SafeArrayBound; 1],
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayCreate(
    vt: u16,
    c_dims: u32,
    rgsabound: *const SafeArrayBound,
) -> *mut SafeArray {
    if c_dims == 0 || rgsabound.is_null() {
        return std::ptr::null_mut();
    }
    let elem_size = match vt {
        2 | 18 => 2,      // VT_I2, VT_UI2
        3 | 19 | 4 => 4,  // VT_I4, VT_UI4, VT_R4
        20 | 21 | 5 => 8, // VT_I8, VT_UI8, VT_R8
        17 => 1,          // VT_UI1
        8 => 8,           // VT_BSTR
        _ => 8,
    };
    let total_elements = (*rgsabound).c_elements as usize;
    let data_bytes = total_elements * elem_size;
    let sa_bytes = std::mem::size_of::<SafeArray>();
    let total_alloc = sa_bytes + data_bytes;
    let raw = libc::calloc(1, total_alloc) as *mut u8;
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    let sa = raw as *mut SafeArray;
    (*sa).c_dims = c_dims as u16;
    (*sa).cb_elements = elem_size as u32;
    (*sa).pv_data = raw.add(sa_bytes) as *mut libc::c_void;
    (*sa).rgsabound[0] = *rgsabound;
    sa
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayDestroy(psa: *mut SafeArray) -> HRESULT {
    if !psa.is_null() {
        libc::free(psa as *mut libc::c_void);
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayGetDim(psa: *const SafeArray) -> u32 {
    if psa.is_null() {
        0
    } else {
        (*psa).c_dims as u32
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayGetLBound(
    psa: *const SafeArray,
    _n_dim: u32,
    pl_lbound: *mut i32,
) -> HRESULT {
    if psa.is_null() || pl_lbound.is_null() {
        return 0x80070057u32 as i32; // E_INVALIDARG
    }
    *pl_lbound = (*psa).rgsabound[0].l_bound;
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayGetUBound(
    psa: *const SafeArray,
    _n_dim: u32,
    pl_ubound: *mut i32,
) -> HRESULT {
    if psa.is_null() || pl_ubound.is_null() {
        return 0x80070057u32 as i32; // E_INVALIDARG
    }
    *pl_ubound = (*psa).rgsabound[0].l_bound + (*psa).rgsabound[0].c_elements as i32 - 1;
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayGetVartype(_psa: *const SafeArray, pvt: *mut u16) -> HRESULT {
    if !pvt.is_null() {
        *pvt = 17; // VT_UI1 default
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayAccessData(
    psa: *mut SafeArray,
    ppv_data: *mut *mut libc::c_void,
) -> HRESULT {
    if psa.is_null() || ppv_data.is_null() {
        return 0x80070057u32 as i32; // E_INVALIDARG
    }
    (*psa).c_locks += 1;
    *ppv_data = (*psa).pv_data;
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayUnaccessData(psa: *mut SafeArray) -> HRESULT {
    if psa.is_null() {
        return 0x80070057u32 as i32; // E_INVALIDARG
    }
    (*psa).c_locks = (*psa).c_locks.saturating_sub(1);
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayPtrOfIndex(
    psa: *mut SafeArray,
    rgIndices: *const i32,
    ppvData: *mut *mut libc::c_void,
) -> HRESULT {
    if psa.is_null() || ppvData.is_null() {
        return 0x80070057u32 as i32; // E_INVALIDARG
    }
    let elem_size = (*psa).cb_elements as usize;
    let idx = if rgIndices.is_null() { 0 } else { *rgIndices as usize };
    let data_base = (*psa).pv_data as *mut u8;
    *ppvData = data_base.add(idx * elem_size) as *mut libc::c_void;
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SafeArrayPutElement(
    psa: *mut SafeArray,
    rgIndices: *const i32,
    pv: *const libc::c_void,
) -> HRESULT {
    if psa.is_null() || pv.is_null() {
        return 0x80070057u32 as i32; // E_INVALIDARG
    }
    let elem_size = (*psa).cb_elements as usize;
    let idx = if rgIndices.is_null() { 0 } else { *rgIndices as usize };
    let data_base = (*psa).pv_data as *mut u8;
    std::ptr::copy_nonoverlapping(pv as *const u8, data_base.add(idx * elem_size), elem_size);
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SymInitialize(
    _h_process: u64,
    _user_search_path: usize,
    _f_invade_process: i32,
) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SymFromAddr(
    _h_process: u64,
    _address: u64,
    displacement: *mut u64,
    _symbol: *mut libc::c_void,
) -> i32 {
    if !displacement.is_null() {
        *displacement = 0;
    }
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn SymCleanup(_h_process: u64) -> i32 {
    1 // TRUE
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
pub unsafe extern "win64" fn SysAllocStringLen(psz: *const u16, len: u32) -> *mut u16 {
    let byte_len = (len as usize) * 2;
    let total_bytes = 4 + byte_len + 2;
    let mem = libc::calloc(1, total_bytes) as *mut u8;
    if mem.is_null() {
        return std::ptr::null_mut();
    }
    *(mem as *mut u32) = byte_len as u32;
    let bstr = mem.add(4) as *mut u16;
    if !psz.is_null() {
        std::ptr::copy_nonoverlapping(psz, bstr, len as usize);
    }
    *bstr.add(len as usize) = 0;
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
pub unsafe extern "win64" fn SysStringLen(bstr: *const u16) -> u32 {
    if bstr.is_null() {
        return 0;
    }
    let mem = (bstr as *const u8).sub(4);
    let bytes = *(mem as *const u32);
    bytes / 2
}

#[no_mangle]
pub unsafe extern "win64" fn SysStringByteLen(bstr: *const u16) -> u32 {
    if bstr.is_null() {
        return 0;
    }
    let mem = (bstr as *const u8).sub(4);
    *(mem as *const u32)
}

#[no_mangle]
pub unsafe extern "win64" fn CoTaskMemFree(pv: *mut std::ffi::c_void) {
    if !pv.is_null() {
        libc::free(pv);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CoTaskMemRealloc(
    pv: *mut std::ffi::c_void,
    cb: usize,
) -> *mut std::ffi::c_void {
    if pv.is_null() {
        return CoTaskMemAlloc(cb);
    }
    if cb == 0 {
        CoTaskMemFree(pv);
        return std::ptr::null_mut();
    }
    libc::realloc(pv, cb)
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[no_mangle]
pub unsafe extern "win64" fn CoCreateGuid(pguid: *mut Guid) -> HRESULT {
    if pguid.is_null() {
        return 0x80070057u32 as i32; // E_INVALIDARG
    }
    static GUID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let cnt = GUID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut bytes = [0u8; 16];
    let _ = libc::getrandom(bytes.as_mut_ptr() as *mut libc::c_void, 16, 0);
    // RFC 4122 version 4
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    // RFC 4122 variant
    bytes[8] = (bytes[8] & 0x3F) | 0x80;

    let data1 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).wrapping_add(cnt as u32);
    let data2 = u16::from_le_bytes([bytes[4], bytes[5]]);
    let data3 = u16::from_le_bytes([bytes[6], bytes[7]]);
    let mut data4 = [0u8; 8];
    data4.copy_from_slice(&bytes[8..16]);

    *pguid = Guid {
        data1,
        data2,
        data3,
        data4,
    };
    println!(
        "[OLE32] CoCreateGuid -> {{0x{:08X}-0x{:04X}-0x{:04X}-...}}",
        data1, data2, data3
    );
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn StringFromGUID2(
    rguid: *const Guid,
    lpsz: *mut u16,
    cch_max: i32,
) -> i32 {
    if rguid.is_null() || lpsz.is_null() || cch_max < 39 {
        return 0;
    }
    let g = &*rguid;
    let s = format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}\0",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7]
    );
    let mut len = 0;
    for (i, ch) in s.encode_utf16().enumerate().take(cch_max as usize) {
        *lpsz.add(i) = ch;
        len = (i + 1) as i32;
    }
    len
}

#[no_mangle]
pub unsafe extern "win64" fn StringFromIID(
    rclsid: *const Guid,
    lplpsz: *mut *mut u16,
) -> i32 {
    if rclsid.is_null() || lplpsz.is_null() {
        return -2147024809; // E_INVALIDARG (0x80070057)
    }
    let mem = CoTaskMemAlloc(80) as *mut u16;
    if mem.is_null() {
        return -2147024882; // E_OUTOFMEMORY
    }
    StringFromGUID2(rclsid, mem, 40);
    *lplpsz = mem;
    0 // S_OK
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
pub unsafe extern "win64" fn ShellExecuteA(
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
pub unsafe extern "win64" fn ShellExecuteExW(_p_exec_info: *mut std::ffi::c_void) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn ShellExecuteExA(_p_exec_info: *mut std::ffi::c_void) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SHFileOperationW(_lp_file_op: *mut std::ffi::c_void) -> i32 {
    0 // SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn SHFileOperationA(_lp_file_op: *mut std::ffi::c_void) -> i32 {
    0 // SUCCESS
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
    if !psz_path.is_null() && (psz_path as usize) > 0x10000 {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        std::ptr::copy_nonoverlapping(path.as_ptr(), psz_path, path.len());
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn PathFileExistsW(psz_path: *const u16) -> i32 {
    if psz_path.is_null() || (psz_path as usize) < 0x10000 {
        return 0;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn PathFileExistsA(psz_path: *const u8) -> i32 {
    if psz_path.is_null() || (psz_path as usize) < 0x10000 {
        return 0;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn PathCanonicalizeW(psz_buf: *mut u16, psz_path: *const u16) -> i32 {
    if psz_buf.is_null() || (psz_buf as usize) < 0x10000 {
        return 0;
    }
    if !psz_path.is_null() && (psz_path as usize) >= 0x10000 {
        let mut len = 0usize;
        while *psz_path.add(len) != 0 && len < 4096 {
            len += 1;
        }
        std::ptr::copy_nonoverlapping(psz_path, psz_buf, len);
        *psz_buf.add(len) = 0;
    } else {
        *psz_buf = 0;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn PathCombineW(
    psz_dest: *mut u16,
    psz_dir: *const u16,
    psz_file: *const u16,
) -> *mut u16 {
    if psz_dest.is_null() || (psz_dest as usize) < 0x10000 {
        return std::ptr::null_mut();
    }
    let mut out_len = 0usize;
    if !psz_dir.is_null() && (psz_dir as usize) >= 0x10000 {
        while *psz_dir.add(out_len) != 0 && out_len < 2048 {
            *psz_dest.add(out_len) = *psz_dir.add(out_len);
            out_len += 1;
        }
    }
    if out_len > 0
        && *psz_dest.add(out_len - 1) != ('\\' as u16)
        && *psz_dest.add(out_len - 1) != ('/' as u16)
    {
        *psz_dest.add(out_len) = '\\' as u16;
        out_len += 1;
    }
    if !psz_file.is_null() && (psz_file as usize) >= 0x10000 {
        let mut file_len = 0usize;
        while *psz_file.add(file_len) != 0 && out_len < 4095 {
            *psz_dest.add(out_len) = *psz_file.add(file_len);
            out_len += 1;
            file_len += 1;
        }
    }
    *psz_dest.add(out_len) = 0;
    psz_dest
}

#[no_mangle]
pub unsafe extern "win64" fn PathCombineA(
    psz_dest: *mut u8,
    psz_dir: *const u8,
    psz_file: *const u8,
) -> *mut u8 {
    if psz_dest.is_null() || (psz_dest as usize) < 0x10000 {
        return std::ptr::null_mut();
    }
    let mut out_len = 0usize;
    if !psz_dir.is_null() && (psz_dir as usize) >= 0x10000 {
        while *psz_dir.add(out_len) != 0 && out_len < 2048 {
            *psz_dest.add(out_len) = *psz_dir.add(out_len);
            out_len += 1;
        }
    }
    if out_len > 0 && *psz_dest.add(out_len - 1) != b'\\' && *psz_dest.add(out_len - 1) != b'/' {
        *psz_dest.add(out_len) = b'\\';
        out_len += 1;
    }
    if !psz_file.is_null() && (psz_file as usize) >= 0x10000 {
        let mut file_len = 0usize;
        while *psz_file.add(file_len) != 0 && out_len < 4095 {
            *psz_dest.add(out_len) = *psz_file.add(file_len);
            out_len += 1;
            file_len += 1;
        }
    }
    *psz_dest.add(out_len) = 0;
    psz_dest
}

#[no_mangle]
pub unsafe extern "win64" fn SHDeleteKeyW(_hkey: usize, _psz_subkey: *const u16) -> i32 {
    0 // ERROR_SUCCESS
}

fn ensure_appdata_dir_exists() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/fizzy".to_string());
    let prefix = format!("{}/.local/share/rine/prefixes/default/drive_c/users/fizzy", home);
    for sub in &["AppData/Roaming", "AppData/Local", "AppData/LocalLow", "AppData/LocalLow/Studio MDHR/Cuphead", "AppData/Roaming/Cuphead", "Documents"] {
        std::fs::create_dir_all(format!("{}/{}", home, sub)).ok();
        std::fs::create_dir_all(format!("{}/{}", prefix, sub)).ok();
    }
}

fn get_folder_path_for_csidl(csidl: i32) -> &'static str {
    let folder = csidl & 0xFF;
    match folder {
        0x1c => "C:\\Users\\fizzy\\AppData\\Local",
        0x1a => "C:\\Users\\fizzy\\AppData\\Roaming",
        0x05 => "C:\\Users\\fizzy\\Documents",
        0x28 => "C:\\Users\\fizzy\\Saved Games",
        _ => "C:\\Users\\fizzy\\AppData\\Roaming",
    }
}

fn get_folder_path_for_rfid(rfid_ptr: u64) -> &'static str {
    if rfid_ptr == 0 {
        return "C:\\Users\\fizzy\\AppData\\Roaming";
    }
    unsafe {
        let data1 = *(rfid_ptr as *const u32);
        match data1 {
            // FOLDERID_LocalAppDataLow {A520A1A4-1780-4FF6-BD18-167343C5AF16}
            0xA520A1A4 => "C:\\Users\\fizzy\\AppData\\LocalLow",
            // FOLDERID_LocalAppData {F1B32785-6FBA-4FCF-9D55-7B8E7F157091}
            0xF1B32785 => "C:\\Users\\fizzy\\AppData\\Local",
            // FOLDERID_RoamingAppData {3EB685FD-984F-4FF0-B28D-4530F4335787}
            0x3EB685FD => "C:\\Users\\fizzy\\AppData\\Roaming",
            // FOLDERID_Documents {FDD39AD0-238F-46AF-ADB4-6C85480369C7}
            0xFDD39AD0 => "C:\\Users\\fizzy\\Documents",
            // FOLDERID_SavedGames {4C5C32FF-BB9D-43B0-B5B4-2D72E54EAAA4}
            0x4C5C32FF => "C:\\Users\\fizzy\\Saved Games",
            _ => "C:\\Users\\fizzy\\AppData\\Roaming",
        }
    }
}

pub unsafe extern "win64" fn SHGetFolderPathW(
    _hwnd: u64,
    csidl: i32,
    _h_token: u64,
    _dw_flags: u32,
    psz_path: *mut u16,
) -> i32 {
    ensure_appdata_dir_exists();
    if !psz_path.is_null() {
        let path = get_folder_path_for_csidl(csidl);
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        std::ptr::copy_nonoverlapping(wide.as_ptr(), psz_path, wide.len());
    }
    0 // S_OK
}

pub unsafe extern "win64" fn SHGetFolderPathA(
    _hwnd: u64,
    csidl: i32,
    _h_token: u64,
    _dw_flags: u32,
    psz_path: *mut u8,
) -> i32 {
    ensure_appdata_dir_exists();
    if !psz_path.is_null() {
        let path = format!("{}\0", get_folder_path_for_csidl(csidl));
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
    rfid: u64,
    _dw_flags: u32,
    _h_token: u64,
    ppsz_path: *mut *mut u16,
) -> i32 {
    ensure_appdata_dir_exists();
    if !ppsz_path.is_null() {
        let path = get_folder_path_for_rfid(rfid);
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
    csidl: i32,
    _f_create: i32,
) -> i32 {
    ensure_appdata_dir_exists();
    if !psz_path.is_null() {
        let path = get_folder_path_for_csidl(csidl);
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        std::ptr::copy_nonoverlapping(wide.as_ptr(), psz_path, wide.len());
    }
    1 // TRUE
}

pub unsafe extern "win64" fn SHGetSpecialFolderPathA(
    _hwnd: u64,
    psz_path: *mut u8,
    csidl: i32,
    _f_create: i32,
) -> i32 {
    ensure_appdata_dir_exists();
    if !psz_path.is_null() {
        let path = format!("{}\0", get_folder_path_for_csidl(csidl));
        std::ptr::copy_nonoverlapping(path.as_ptr(), psz_path, path.len());
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CommandLineToArgvW(
    lp_cmd_line: *const u16,
    p_num_args: *mut i32,
) -> *mut *mut u16 {
    if p_num_args.is_null() {
        return std::ptr::null_mut();
    }

    let cmd_line_ptr = if !lp_cmd_line.is_null() && (lp_cmd_line as usize) > 0x10000 {
        lp_cmd_line
    } else {
        crate::dlls::kernel32::GetCommandLineW() as *const u16
    };

    let mut raw_u16 = Vec::new();
    if !cmd_line_ptr.is_null() && (cmd_line_ptr as usize) > 0x10000 {
        let mut idx = 0;
        while *cmd_line_ptr.add(idx) != 0 && idx < 32768 {
            raw_u16.push(*cmd_line_ptr.add(idx));
            idx += 1;
        }
    }
    let cmd_str = String::from_utf16_lossy(&raw_u16);
    let trimmed = cmd_str.trim();

    let mut args: Vec<String> = Vec::new();
    let mut cur_arg = String::new();
    let mut in_quotes = false;
    for ch in trimmed.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch.is_whitespace() && !in_quotes {
            if !cur_arg.is_empty() {
                args.push(cur_arg.clone());
                cur_arg.clear();
            }
        } else {
            cur_arg.push(ch);
        }
    }
    if !cur_arg.is_empty() {
        args.push(cur_arg);
    }
    if args.is_empty() {
        let exe_path = crate::dlls::kernel32::get_target_exe_path();
        args.push(exe_path);
    }

    let num_args = args.len();
    *p_num_args = num_args as i32;

    let args_utf16: Vec<Vec<u16>> = args
        .iter()
        .map(|s| s.encode_utf16().chain(std::iter::once(0)).collect())
        .collect();

    let table_size = (num_args + 1) * std::mem::size_of::<*mut u16>();
    let strings_size: usize = args_utf16.iter().map(|s| s.len() * 2).sum();
    let total_size = table_size + strings_size + 16;

    let block = crate::dlls::kernel32::HeapAlloc(
        crate::dlls::kernel32::GetProcessHeap(),
        0,
        total_size,
    ) as *mut u8;

    if block.is_null() {
        *p_num_args = 0;
        return std::ptr::null_mut();
    }

    let ptr_table = block as *mut *mut u16;
    let mut str_ptr = block.add(table_size) as *mut u16;

    for (i, arg_u16) in args_utf16.iter().enumerate() {
        *ptr_table.add(i) = str_ptr;
        std::ptr::copy_nonoverlapping(arg_u16.as_ptr(), str_ptr, arg_u16.len());
        str_ptr = str_ptr.add(arg_u16.len());
    }
    *ptr_table.add(num_args) = std::ptr::null_mut();

    ptr_table
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

    #[test]
    fn test_winrt_string_and_ro_init() {
        unsafe {
            let res = RoInitialize(1);
            assert_eq!(res, 0);

            let src_u16: Vec<u16> = "Windows.Gaming.Input\0".encode_utf16().collect();
            let mut hstr: *mut libc::c_void = std::ptr::null_mut();
            let str_res =
                WindowsCreateString(src_u16.as_ptr(), (src_u16.len() - 1) as u32, &mut hstr);
            assert_eq!(str_res, 0);
            assert!(!hstr.is_null());

            let mut len = 0u32;
            let raw_buf = WindowsGetStringRawBuffer(hstr, &mut len);
            assert_eq!(len, (src_u16.len() - 1) as u32);
            assert_eq!(*raw_buf, src_u16[0]);

            assert_eq!(WindowsDeleteString(hstr), 0);
            RoUninitialize();
        }
    }

    #[test]
    fn test_sh_get_folder_paths_and_known_folders() {
        unsafe {
            let mut path_buf = [0u16; 260];
            // Test CSIDL_LOCAL_APPDATA (0x1c) -> AppData\Local
            assert_eq!(SHGetFolderPathW(0, 0x1c, 0, 0, path_buf.as_mut_ptr()), 0);
            let s = String::from_utf16_lossy(&path_buf);
            assert!(s.starts_with("C:\\Users\\fizzy\\AppData\\Local"));

            // Test CSIDL_APPDATA (0x1a) -> AppData\Roaming
            assert_eq!(SHGetFolderPathW(0, 0x1a, 0, 0, path_buf.as_mut_ptr()), 0);
            let s = String::from_utf16_lossy(&path_buf);
            assert!(s.starts_with("C:\\Users\\fizzy\\AppData\\Roaming"));

            // Test FOLDERID_LocalAppDataLow {A520A1A4-1780-4FF6-BD18-167343C5AF16}
            let guid_data: [u32; 4] = [0xA520A1A4, 0x4FF61780, 0x167318BD, 0x16AF5543];
            let mut ppsz: *mut u16 = std::ptr::null_mut();
            assert_eq!(SHGetKnownFolderPath(guid_data.as_ptr() as u64, 0, 0, &mut ppsz), 0);
            assert!(!ppsz.is_null());
            let mut len = 0;
            while *ppsz.add(len) != 0 { len += 1; }
            let slice = std::slice::from_raw_parts(ppsz, len);
            let s_low = String::from_utf16_lossy(slice);
            assert_eq!(s_low, "C:\\Users\\fizzy\\AppData\\LocalLow");
            libc::free(ppsz as *mut libc::c_void);
        }
    }
}
