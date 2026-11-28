#![allow(non_snake_case)]
#![allow(unused_variables)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WINHTTP_CURRENT_USER_IE_PROXY_CONFIG {
    pub fAutoDetect: i32,
    pub lpszAutoConfigUrl: *mut u16,
    pub lpszProxy: *mut u16,
    pub lpszProxyBypass: *mut u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WINHTTP_PROXY_INFO {
    pub dwAccessType: u32,
    pub lpszProxy: *mut u16,
    pub lpszProxyBypass: *mut u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WINHTTP_AUTOPROXY_OPTIONS {
    pub dwFlags: u32,
    pub dwAutoDetectFlags: u32,
    pub lpszAutoConfigUrl: *const u16,
    pub lpvReserved: *const c_void,
    pub dwReserved: u32,
    pub fAutoLogonIfChallenged: i32,
}

static WINHTTP_HANDLE_COUNTER: AtomicUsize = AtomicUsize::new(0x8100);

#[no_mangle]
pub unsafe extern "win64" fn WinHttpGetIEProxyConfigForCurrentUser(
    proxy_config: *mut WINHTTP_CURRENT_USER_IE_PROXY_CONFIG,
) -> i32 {
    if proxy_config.is_null() {
        crate::dlls::kernel32::SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0;
    }
    // Return direct connection (no proxy)
    (*proxy_config).fAutoDetect = 0;
    (*proxy_config).lpszAutoConfigUrl = std::ptr::null_mut();
    (*proxy_config).lpszProxy = std::ptr::null_mut();
    (*proxy_config).lpszProxyBypass = std::ptr::null_mut();
    println!("[WINHTTP] WinHttpGetIEProxyConfigForCurrentUser -> DIRECT (no proxy)");
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpGetDefaultProxyConfiguration(
    p_proxy_info: *mut WINHTTP_PROXY_INFO,
) -> i32 {
    if p_proxy_info.is_null() {
        crate::dlls::kernel32::SetLastError(87);
        return 0;
    }
    (*p_proxy_info).dwAccessType = 1; // WINHTTP_ACCESS_TYPE_NO_PROXY
    (*p_proxy_info).lpszProxy = std::ptr::null_mut();
    (*p_proxy_info).lpszProxyBypass = std::ptr::null_mut();
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpOpen(
    _psz_agent: *const u16,
    _dw_access_type: u32,
    _psz_proxy: *const u16,
    _psz_proxy_bypass: *const u16,
    _dw_flags: u32,
) -> usize {
    let handle = WINHTTP_HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    println!("[WINHTTP] WinHttpOpen -> handle=0x{:X}", handle);
    handle
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpGetProxyForUrl(
    _h_session: usize,
    _lpcwsz_url: *const u16,
    _p_auto_proxy_options: *const c_void,
    p_proxy_info: *mut WINHTTP_PROXY_INFO,
) -> i32 {
    if !p_proxy_info.is_null() {
        // WINHTTP_ACCESS_TYPE_NO_PROXY = 1
        (*p_proxy_info).dwAccessType = 1;
        (*p_proxy_info).lpszProxy = std::ptr::null_mut();
        (*p_proxy_info).lpszProxyBypass = std::ptr::null_mut();
    }
    println!("[WINHTTP] WinHttpGetProxyForUrl -> WINHTTP_ACCESS_TYPE_NO_PROXY");
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpCloseHandle(h_internet: usize) -> i32 {
    println!("[WINHTTP] WinHttpCloseHandle(0x{:X})", h_internet);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpConnect(
    h_session: usize,
    _pswz_server_name: *const u16,
    _n_server_port: u16,
    _dw_reserved: u32,
) -> usize {
    let handle = WINHTTP_HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    println!("[WINHTTP] WinHttpConnect(session=0x{:X}) -> handle=0x{:X}", h_session, handle);
    handle
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpOpenRequest(
    h_connect: usize,
    _pwsz_verb: *const u16,
    _pwsz_object_name: *const u16,
    _pwsz_version: *const u16,
    _pwsz_referrer: *const u16,
    _ppwsz_accept_types: *const *const u16,
    _dw_flags: u32,
) -> usize {
    let handle = WINHTTP_HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    println!("[WINHTTP] WinHttpOpenRequest(connect=0x{:X}) -> handle=0x{:X}", h_connect, handle);
    handle
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpSendRequest(
    h_request: usize,
    _lpsz_headers: *const u16,
    _dw_headers_length: u32,
    _lp_optional: *const c_void,
    _dw_optional_length: u32,
    _dw_total_length: u32,
    _dw_context: usize,
) -> i32 {
    println!("[WINHTTP] WinHttpSendRequest(req=0x{:X})", h_request);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpReceiveResponse(
    h_request: usize,
    _lp_reserved: *const c_void,
) -> i32 {
    println!("[WINHTTP] WinHttpReceiveResponse(req=0x{:X})", h_request);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpQueryHeaders(
    h_request: usize,
    _dw_info_level: u32,
    _pwsz_name: *const u16,
    lp_buffer: *mut c_void,
    lpdw_buffer_length: *mut u32,
    _lpdw_index: *mut u32,
) -> i32 {
    println!("[WINHTTP] WinHttpQueryHeaders(req=0x{:X})", h_request);
    if !lpdw_buffer_length.is_null() {
        *lpdw_buffer_length = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpReadData(
    h_request: usize,
    lp_buffer: *mut c_void,
    dw_number_of_bytes_to_read: u32,
    lpdw_number_of_bytes_read: *mut u32,
) -> i32 {
    println!("[WINHTTP] WinHttpReadData(req=0x{:X}, requested={})", h_request, dw_number_of_bytes_to_read);
    if !lpdw_number_of_bytes_read.is_null() {
        *lpdw_number_of_bytes_read = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpSetOption(
    h_internet: usize,
    dw_option: u32,
    _lp_buffer: *const c_void,
    _dw_buffer_length: u32,
) -> i32 {
    println!("[WINHTTP] WinHttpSetOption(0x{:X}, opt={})", h_internet, dw_option);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpQueryOption(
    h_internet: usize,
    dw_option: u32,
    _lp_buffer: *mut c_void,
    lpdw_buffer_length: *mut u32,
) -> i32 {
    println!("[WINHTTP] WinHttpQueryOption(0x{:X}, opt={})", h_internet, dw_option);
    if !lpdw_buffer_length.is_null() {
        *lpdw_buffer_length = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpSetTimeouts(
    h_internet: usize,
    _n_resolve_timeout: i32,
    _n_connect_timeout: i32,
    _n_send_timeout: i32,
    _n_receive_timeout: i32,
) -> i32 {
    println!("[WINHTTP] WinHttpSetTimeouts(0x{:X})", h_internet);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpSetStatusCallback(
    h_internet: usize,
    _lpfn_callback: usize,
    _dw_notification_flags: u32,
    _dw_reserved: usize,
) -> usize {
    println!("[WINHTTP] WinHttpSetStatusCallback(0x{:X})", h_internet);
    0 // Returns WINHTTP_INVALID_STATUS_CALLBACK on error or prev callback
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpCrackUrl(
    _pwsz_url: *const u16,
    _dw_url_length: u32,
    _dw_flags: u32,
    _lp_url_components: *mut c_void,
) -> i32 {
    println!("[WINHTTP] WinHttpCrackUrl");
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WinHttpCreateUrl(
    _lp_url_components: *const c_void,
    _dw_flags: u32,
    _pwsz_url: *mut u16,
    lpdw_url_length: *mut u32,
) -> i32 {
    println!("[WINHTTP] WinHttpCreateUrl");
    if !lpdw_url_length.is_null() {
        *lpdw_url_length = 0;
    }
    1
}
