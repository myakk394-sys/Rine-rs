//! IP Helper API (iphlpapi.dll) implementation for Rine
#![allow(non_snake_case)]

use std::ffi::c_void;

pub const ERROR_SUCCESS: u32 = 0;
pub const ERROR_NOT_SUPPORTED: u32 = 50;
pub const ERROR_NO_DATA: u32 = 232;
pub const INVALID_HANDLE_VALUE: usize = !0usize;

#[no_mangle]
pub unsafe extern "win64" fn IcmpCreateFile() -> usize {
    crate::dlls::kernel32::SetLastError(ERROR_NOT_SUPPORTED);
    INVALID_HANDLE_VALUE
}

#[no_mangle]
pub unsafe extern "win64" fn IcmpCloseHandle(_handle: usize) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn IcmpSendEcho(
    _icmp_handle: usize,
    _destination_address: u32,
    _request_data: *const c_void,
    _request_size: u16,
    _request_options: *const c_void,
    _reply_buffer: *mut c_void,
    _reply_size: u32,
    _timeout: u32,
) -> u32 {
    crate::dlls::kernel32::SetLastError(ERROR_NOT_SUPPORTED);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn Icmp6CreateFile() -> usize {
    crate::dlls::kernel32::SetLastError(ERROR_NOT_SUPPORTED);
    INVALID_HANDLE_VALUE
}

#[no_mangle]
pub unsafe extern "win64" fn Icmp6CloseHandle(_handle: usize) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn Icmp6SendEcho2(
    _icmp_handle: usize,
    _event: usize,
    _apc_routine: usize,
    _apc_context: usize,
    _source_address: *const c_void,
    _destination_address: *const c_void,
    _request_data: *const c_void,
    _request_size: u16,
    _request_options: *const c_void,
    _reply_buffer: *mut c_void,
    _reply_size: u32,
    _timeout: u32,
) -> u32 {
    crate::dlls::kernel32::SetLastError(ERROR_NOT_SUPPORTED);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetAdaptersInfo(
    _adapter_info: *mut c_void,
    size_pointer: *mut u32,
) -> u32 {
    if !size_pointer.is_null() {
        *size_pointer = 0;
    }
    ERROR_NO_DATA
}

#[no_mangle]
pub unsafe extern "win64" fn GetAdaptersAddresses(
    _family: u32,
    _flags: u32,
    _reserved: *const c_void,
    _adapter_addresses: *mut c_void,
    size_pointer: *mut u32,
) -> u32 {
    if !size_pointer.is_null() {
        *size_pointer = 0;
    }
    ERROR_NO_DATA
}

#[no_mangle]
pub unsafe extern "win64" fn GetNetworkParams(
    _fixed_info: *mut c_void,
    size_pointer: *mut u32,
) -> u32 {
    if !size_pointer.is_null() {
        *size_pointer = 0;
    }
    ERROR_NO_DATA
}

#[no_mangle]
pub unsafe extern "win64" fn GetBestInterfaceEx(
    _dest_addr: *const c_void,
    best_if_index: *mut u32,
) -> u32 {
    if !best_if_index.is_null() {
        *best_if_index = 1;
    }
    ERROR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn NotifyAddrChange(
    handle: *mut usize,
    overlapped: *mut c_void,
) -> u32 {
    if !handle.is_null() {
        *handle = 0x6001;
    }
    if !overlapped.is_null() {
        997 // ERROR_IO_PENDING
    } else {
        ERROR_SUCCESS
    }
}

#[no_mangle]
pub unsafe extern "win64" fn NotifyRouteChange(
    handle: *mut usize,
    overlapped: *mut c_void,
) -> u32 {
    if !handle.is_null() {
        *handle = 0x6002;
    }
    if !overlapped.is_null() {
        997 // ERROR_IO_PENDING
    } else {
        ERROR_SUCCESS
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CancelIPChangeNotify(_notify_overlapped: *const c_void) -> i32 {
    1 // TRUE
}
