//! DirectStorage API implementation for Rine
//!
//! Modern game engines (e.g. Unity 6) query DirectStorage (dstorage.dll) for NVMe asset streaming.
//! When running on Linux/Wine/Proton without native bypassIO drivers, returning E_NOINTERFACE (0x80004002)
//! signals to the engine that DirectStorage is unavailable, prompting it to gracefully fall back
//! to standard Win32 synchronous/asynchronous file streaming (CreateFileW / ReadFile).
#![allow(non_snake_case)]

use std::ffi::c_void;

pub const S_OK: i32 = 0;
pub const E_NOINTERFACE: i32 = 0x80004002_u32 as i32;
pub const E_NOTIMPL: i32 = 0x80004001_u32 as i32;

#[no_mangle]
pub unsafe extern "win64" fn DStorageGetFactory(
    _riid: *const c_void,
    ppv: *mut *mut c_void,
) -> i32 {
    println!("[DSTORAGE] DStorageGetFactory -> returning E_NOINTERFACE (DirectStorage fallback to Win32 file streaming)");
    if !ppv.is_null() {
        *ppv = std::ptr::null_mut();
    }
    E_NOINTERFACE
}

#[no_mangle]
pub unsafe extern "win64" fn DStorageGetFactoryCore(
    _riid: *const c_void,
    ppv: *mut *mut c_void,
) -> i32 {
    println!("[DSTORAGE] DStorageGetFactoryCore -> returning E_NOINTERFACE");
    if !ppv.is_null() {
        *ppv = std::ptr::null_mut();
    }
    E_NOINTERFACE
}

#[no_mangle]
pub unsafe extern "win64" fn DStorageSetConfiguration(
    _configuration: *const c_void,
) -> i32 {
    println!("[DSTORAGE] DStorageSetConfiguration -> returning E_NOINTERFACE");
    E_NOINTERFACE
}

#[no_mangle]
pub unsafe extern "win64" fn DStorageSetConfiguration1(
    _configuration: *const c_void,
) -> i32 {
    println!("[DSTORAGE] DStorageSetConfiguration1 -> returning E_NOINTERFACE");
    E_NOINTERFACE
}

#[no_mangle]
pub unsafe extern "win64" fn DStorageSetConfigurationCore(
    _configuration: *const c_void,
) -> i32 {
    println!("[DSTORAGE] DStorageSetConfigurationCore -> returning E_NOINTERFACE");
    E_NOINTERFACE
}

#[no_mangle]
pub unsafe extern "win64" fn DStorageCreateCompressionCodec(
    _format: u32,
    _num_threads: u32,
    _riid: *const c_void,
    ppv: *mut *mut c_void,
) -> i32 {
    if !ppv.is_null() {
        *ppv = std::ptr::null_mut();
    }
    E_NOINTERFACE
}

#[no_mangle]
pub unsafe extern "win64" fn DStorageCreateCompressionCodecCore(
    _format: u32,
    _num_threads: u32,
    _riid: *const c_void,
    ppv: *mut *mut c_void,
) -> i32 {
    if !ppv.is_null() {
        *ppv = std::ptr::null_mut();
    }
    E_NOINTERFACE
}
