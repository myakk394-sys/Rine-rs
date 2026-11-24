#![allow(non_snake_case)]

use std::ffi::c_void;

#[repr(C)]
pub struct DInput8Mock {
    pub lpVtbl: *const c_void,
    pub refcount: u32,
}

unsafe extern "win64" fn di8_query_interface(
    _this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> i32 {
    if !ppv.is_null() {
        *ppv = std::ptr::null_mut();
    }
    0x80004002u32 as i32 // E_NOINTERFACE
}

unsafe extern "win64" fn di8_add_ref(_this: *mut c_void) -> u32 {
    1
}

unsafe extern "win64" fn di8_release(_this: *mut c_void) -> u32 {
    0
}

unsafe extern "win64" fn di8_create_device(
    _this: *mut c_void,
    _rguid: *const u8,
    lplpDirectInputDevice: *mut *mut c_void,
    _pUnkOuter: *mut c_void,
) -> i32 {
    if !lplpDirectInputDevice.is_null() {
        *lplpDirectInputDevice = std::ptr::null_mut();
    }
    0x80040154u32 as i32
}

unsafe extern "win64" fn di8_enum_devices(
    _this: *mut c_void,
    _dwDevType: u32,
    _lpCallback: *mut c_void,
    _pvRef: *mut c_void,
    _dwFlags: u32,
) -> i32 {
    println!("[DINPUT8] IDirectInput8W::EnumDevices called -> DI_OK (0 devices)");
    0 // DI_OK
}

unsafe extern "win64" fn di8_generic_ok(_this: *mut c_void) -> i32 {
    0
}

static mut DI8_VTBL: [*const c_void; 16] = [
    di8_query_interface as *const c_void, // 0
    di8_add_ref as *const c_void,         // 1
    di8_release as *const c_void,         // 2
    di8_create_device as *const c_void,   // 3 (0x18)
    di8_enum_devices as *const c_void,    // 4 (0x20)
    di8_generic_ok as *const c_void,      // 5
    di8_generic_ok as *const c_void,      // 6
    di8_generic_ok as *const c_void,      // 7
    di8_generic_ok as *const c_void,      // 8
    di8_generic_ok as *const c_void,      // 9
    di8_generic_ok as *const c_void,      // 10
    di8_generic_ok as *const c_void,      // 11
    di8_generic_ok as *const c_void,      // 12
    di8_generic_ok as *const c_void,      // 13
    di8_generic_ok as *const c_void,      // 14
    di8_generic_ok as *const c_void,      // 15
];

static mut DI8_INSTANCE: DInput8Mock = DInput8Mock {
    lpVtbl: std::ptr::null(),
    refcount: 1,
};

pub unsafe extern "win64" fn DirectInput8Create(
    _hinst: u64,
    _version: u32,
    _riid: u64,
    out_di: *mut u64,
    _unk: u64,
) -> i32 {
    if !out_di.is_null() {
        DI8_INSTANCE.lpVtbl = DI8_VTBL.as_ptr() as *const c_void;
        *out_di = (&raw mut DI8_INSTANCE) as u64;
    }
    println!(
        "[DINPUT8 ENGINE] DirectInput8Create: Created DirectInput8 COM Object at {:p}!",
        &raw mut DI8_INSTANCE
    );
    0 // DI_OK
}
