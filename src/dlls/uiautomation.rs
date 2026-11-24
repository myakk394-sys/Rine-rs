//! UIAutomationCore API stubs for Rine
//!
//! Provides Windows UI Automation endpoints queried by modern game engines (e.g. Unity 6).
//! Reporting `UiaClientsAreListening() == FALSE` instructs engines to skip expensive
//! accessibility tree construction and event dispatching.
#![allow(non_snake_case)]

use std::ffi::c_void;

pub const S_OK: i32 = 0;

#[no_mangle]
pub unsafe extern "win64" fn UiaClientsAreListening() -> i32 {
    0 // FALSE: No automation clients / screen readers attached
}

#[no_mangle]
pub unsafe extern "win64" fn UiaHostProviderFromHwnd(
    _hwnd: usize,
    ppProvider: *mut *mut c_void,
) -> i32 {
    if !ppProvider.is_null() {
        *ppProvider = std::ptr::null_mut();
    }
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn UiaReturnRawElementProvider(
    _hwnd: usize,
    _wParam: usize,
    _lParam: usize,
    _el: usize,
) -> isize {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn UiaRaiseNotificationEvent(
    _pProvider: *mut c_void,
    _notificationKind: i32,
    _notificationProcessing: i32,
    _displayString: *const u16,
    _activityId: *const u16,
) -> i32 {
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn UiaRaiseStructureChangedEvent(
    _pProvider: *mut c_void,
    _structureChangeType: i32,
    _runtimeId: *const i32,
    _runtimeIdLength: i32,
) -> i32 {
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn UiaRaiseAutomationPropertyChangedEvent(
    _pProvider: *mut c_void,
    _propertyId: i32,
    _oldValue: *const c_void,
    _newValue: *const c_void,
) -> i32 {
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn UiaRaiseAutomationEvent(
    _pProvider: *mut c_void,
    _eventId: i32,
) -> i32 {
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn UiaGetReservedMixedAttributeValue(
    ppValue: *mut *mut c_void,
) -> i32 {
    if !ppValue.is_null() {
        *ppValue = std::ptr::null_mut();
    }
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn UiaGetReservedNotSupportedValue(
    ppValue: *mut *mut c_void,
) -> i32 {
    if !ppValue.is_null() {
        *ppValue = std::ptr::null_mut();
    }
    S_OK
}
