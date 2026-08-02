#![allow(non_snake_case)]

pub const ERROR_FILE_NOT_FOUND: u32 = 2;
pub const ERROR_INVALID_PARAMETER: u32 = 87;
pub const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
pub const ERROR_NO_MORE_ITEMS: u32 = 259;
pub const INVALID_HANDLE_VALUE: usize = !0usize;

pub unsafe extern "win64" fn SetupDiGetClassDevsW(
    _class_guid: *const u8,
    _enumerator: *const u16,
    _parent: usize,
    _flags: u32,
) -> usize {
    crate::dlls::kernel32::SetLastError(0);
    eprintln!("[SETUPAPI] SetupDiGetClassDevsW -> HDEVINFO 0x5000 (empty device list)");
    0x5000 // Return valid non-null HDEVINFO handle for empty device set
}

pub unsafe extern "win64" fn SetupDiGetClassDevsA(
    _class_guid: *const u8,
    _enumerator: *const u8,
    _parent: usize,
    _flags: u32,
) -> usize {
    crate::dlls::kernel32::SetLastError(0);
    eprintln!("[SETUPAPI] SetupDiGetClassDevsA -> HDEVINFO 0x5000 (empty device list)");
    0x5000
}

pub unsafe extern "win64" fn SetupDiEnumDeviceInterfaces(
    _dev_info: usize,
    _dev_info_data: *const u8,
    _interface_class_guid: *const u8,
    _member_index: u32,
    _dev_interface_data: *mut u8,
) -> i32 {
    crate::dlls::kernel32::SetLastError(ERROR_NO_MORE_ITEMS);
    eprintln!("[SETUPAPI] SetupDiEnumDeviceInterfaces -> FALSE (ERROR_NO_MORE_ITEMS)");
    0 // FALSE
}

pub unsafe extern "win64" fn SetupDiGetDeviceInterfaceDetailW(
    _dev_info: usize,
    _dev_interface_data: *const u8,
    _dev_interface_detail_data: *mut u8,
    _dev_interface_detail_data_size: u32,
    required_size: *mut u32,
    _dev_info_data: *mut u8,
) -> i32 {
    if !required_size.is_null() {
        *required_size = 0;
    }
    crate::dlls::kernel32::SetLastError(ERROR_INVALID_PARAMETER);
    eprintln!("[SETUPAPI] SetupDiGetDeviceInterfaceDetailW -> FALSE (ERROR_INVALID_PARAMETER)");
    0 // FALSE
}

pub unsafe extern "win64" fn SetupDiGetDeviceInterfaceDetailA(
    _dev_info: usize,
    _dev_interface_data: *const u8,
    _dev_interface_detail_data: *mut u8,
    _dev_interface_detail_data_size: u32,
    required_size: *mut u32,
    _dev_info_data: *mut u8,
) -> i32 {
    if !required_size.is_null() {
        *required_size = 0;
    }
    crate::dlls::kernel32::SetLastError(ERROR_INVALID_PARAMETER);
    eprintln!("[SETUPAPI] SetupDiGetDeviceInterfaceDetailA -> FALSE (ERROR_INVALID_PARAMETER)");
    0 // FALSE
}

pub unsafe extern "win64" fn SetupDiOpenDevRegKey(
    _dev_info: usize,
    _dev_info_data: *const u8,
    _scope: u32,
    _hw_profile: u32,
    _key_type: u32,
    _sam_desired: u32,
) -> usize {
    crate::dlls::kernel32::SetLastError(ERROR_INVALID_PARAMETER);
    eprintln!("[SETUPAPI] SetupDiOpenDevRegKey -> INVALID_HANDLE_VALUE (ERROR_INVALID_PARAMETER)");
    INVALID_HANDLE_VALUE
}

pub unsafe extern "win64" fn SetupDiDestroyDeviceInfoList(_dev_info: usize) -> i32 {
    crate::dlls::kernel32::SetLastError(0);
    1 // TRUE
}

pub unsafe extern "win64" fn SetupDiEnumDeviceInfo(
    _dev_info: usize,
    _member_index: u32,
    _dev_info_data: *mut u8,
) -> i32 {
    crate::dlls::kernel32::SetLastError(ERROR_NO_MORE_ITEMS);
    0 // FALSE
}
