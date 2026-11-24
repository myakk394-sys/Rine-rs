//! Implementation of bthprops.cpl and bluetoothapis.dll for Bluetooth discovery
#![allow(non_snake_case)]

pub const ERROR_NO_MORE_ITEMS: u32 = 259;
pub const ERROR_DEVICE_NOT_CONNECTED: u32 = 1167;

#[no_mangle]
pub unsafe extern "win64" fn BluetoothFindFirstRadio(
    _pbtfrp: usize,
    ph_radio: *mut usize,
) -> usize {
    if !ph_radio.is_null() {
        *ph_radio = 0;
    }
    crate::dlls::kernel32::SetLastError(ERROR_NO_MORE_ITEMS);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn BluetoothFindNextRadio(
    _h_find: usize,
    ph_radio: *mut usize,
) -> i32 {
    if !ph_radio.is_null() {
        *ph_radio = 0;
    }
    crate::dlls::kernel32::SetLastError(ERROR_NO_MORE_ITEMS);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn BluetoothFindRadioClose(_h_find: usize) -> i32 {
    crate::dlls::kernel32::SetLastError(0);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn BluetoothFindFirstDevice(
    _pbtsp: usize,
    _pbtdi: usize,
) -> usize {
    crate::dlls::kernel32::SetLastError(ERROR_NO_MORE_ITEMS);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn BluetoothFindNextDevice(
    _h_find: usize,
    _pbtdi: usize,
) -> i32 {
    crate::dlls::kernel32::SetLastError(ERROR_NO_MORE_ITEMS);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn BluetoothFindDeviceClose(_h_find: usize) -> i32 {
    crate::dlls::kernel32::SetLastError(0);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn BluetoothGetRadioInfo(
    _h_radio: usize,
    _p_radio_info: usize,
) -> u32 {
    ERROR_DEVICE_NOT_CONNECTED
}

#[no_mangle]
pub unsafe extern "win64" fn BluetoothGetDeviceInfo(
    _h_radio: usize,
    _pbtdi: usize,
) -> u32 {
    ERROR_DEVICE_NOT_CONNECTED
}

#[no_mangle]
pub unsafe extern "win64" fn bthprops_generic_stub() -> u64 {
    crate::dlls::kernel32::SetLastError(ERROR_NO_MORE_ITEMS);
    0
}
