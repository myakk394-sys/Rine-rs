//! Implementation of SETUPAPI.DLL device installation and enumeration APIs
#![allow(non_snake_case)]

pub const ERROR_FILE_NOT_FOUND: u32 = 2;
pub const ERROR_INVALID_PARAMETER: u32 = 87;
pub const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
pub const ERROR_NO_MORE_ITEMS: u32 = 259;
pub const INVALID_HANDLE_VALUE: usize = !0usize;

// Device interface GUIDs
// GUID_DEVINTERFACE_MOUSE = {378DE44C-5643-11D1-ACF9-00A0C940843E}
pub const GUID_DEVINTERFACE_MOUSE: [u8; 16] = [
    0x4C, 0xE4, 0x8D, 0x37, 0x43, 0x56, 0xD1, 0x11, 0xAC, 0xF9, 0x00, 0xA0, 0xC9, 0x40, 0x84, 0x3E,
];

// GUID_DEVCLASS_MOUSE = {4D36E96F-E325-11CE-BFC1-08002BE10318}
pub const GUID_DEVCLASS_MOUSE: [u8; 16] = [
    0x6F, 0xE9, 0x36, 0x4D, 0x25, 0xE3, 0xCE, 0x11, 0xBF, 0xC1, 0x08, 0x00, 0x2B, 0xE1, 0x03, 0x18,
];

// GUID_DEVINTERFACE_KEYBOARD = {884B96C3-5643-11D1-ACF9-00A0C940843E}
pub const GUID_DEVINTERFACE_KEYBOARD: [u8; 16] = [
    0xC3, 0x96, 0x4B, 0x88, 0x43, 0x56, 0xD1, 0x11, 0xAC, 0xF9, 0x00, 0xA0, 0xC9, 0x40, 0x84, 0x3E,
];

// GUID_DEVCLASS_KEYBOARD = {4D36E96B-E325-11CE-BFC1-08002BE10318}
pub const GUID_DEVCLASS_KEYBOARD: [u8; 16] = [
    0x6B, 0xE9, 0x36, 0x4D, 0x25, 0xE3, 0xCE, 0x11, 0xBF, 0xC1, 0x08, 0x00, 0x2B, 0xE1, 0x03, 0x18,
];

// GUID_DEVINTERFACE_HID = {4D1E55B2-F16F-11CF-88CB-001111000030}
pub const GUID_DEVINTERFACE_HID: [u8; 16] = [
    0xB2, 0x55, 0x1E, 0x4D, 0x6F, 0xF1, 0xCF, 0x11, 0x88, 0xCB, 0x00, 0x11, 0x11, 0x00, 0x00, 0x30,
];

// GUID_DEVINTERFACE_MONITOR = {E6F07B5F-EE97-4A90-B076-33F57BF4EAA7}
pub const GUID_DEVINTERFACE_MONITOR: [u8; 16] = [
    0x5F, 0x7B, 0xF0, 0xE6, 0x97, 0xEE, 0x90, 0x4A, 0xB0, 0x76, 0x33, 0xF5, 0x7B, 0xF4, 0xEA, 0xA7,
];

// GUID_DEVCLASS_MONITOR = {4D36E96E-E325-11CE-BFC1-08002BE10318}
pub const GUID_DEVCLASS_MONITOR: [u8; 16] = [
    0x6E, 0xE9, 0x36, 0x4D, 0x25, 0xE3, 0xCE, 0x11, 0xBF, 0xC1, 0x08, 0x00, 0x2B, 0xE1, 0x03, 0x18,
];

// GUID_DEVCLASS_HIDCLASS = {745A17A0-74D3-11D0-B6FE-00A0C90F57DA}
pub const GUID_DEVCLASS_HIDCLASS: [u8; 16] = [
    0xA0, 0x17, 0x5A, 0x74, 0xD3, 0x74, 0xD0, 0x11, 0xB6, 0xFE, 0x00, 0xA0, 0xC9, 0x0F, 0x57, 0xDA,
];

pub const MOUSE_DEVICE_PATH: &str =
    "\\\\?\\HID#VID_046D&PID_C077#1&0000&0&0000#{378de44c-5643-11d1-acf9-00a0c940843e}";
pub const KEYBOARD_DEVICE_PATH: &str =
    "\\\\?\\HID#VID_046D&PID_C31C#1&0000&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}";
pub const MONITOR_DEVICE_PATH: &str =
    "\\\\?\\DISPLAY#Default_Monitor#1&0000&0&0000#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}";
pub const JOYSTICK_DEVICE_PATH: &str =
    "\\\\?\\HID#VID_045E&PID_028E&IG_00#1&0000&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";

fn guid_to_string(guid: &[u8]) -> String {
    if guid.len() < 16 {
        return "invalid".to_string();
    }
    let d1 = u32::from_le_bytes([guid[0], guid[1], guid[2], guid[3]]);
    let d2 = u16::from_le_bytes([guid[4], guid[5]]);
    let d3 = u16::from_le_bytes([guid[6], guid[7]]);
    format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        d1, d2, d3, guid[8], guid[9], guid[10], guid[11], guid[12], guid[13], guid[14], guid[15]
    )
}

fn determine_device_mask(class_guid: *const u8) -> usize {
    if class_guid.is_null() {
        return 0x500B; // Mouse (1), keyboard (2), and joystick (8)
    }
    let guid = unsafe { std::slice::from_raw_parts(class_guid, 16) };
    if guid == GUID_DEVINTERFACE_MOUSE || guid == GUID_DEVCLASS_MOUSE {
        0x5001 // Mouse only
    } else if guid == GUID_DEVINTERFACE_KEYBOARD || guid == GUID_DEVCLASS_KEYBOARD {
        0x5002 // Keyboard only
    } else if guid == GUID_DEVINTERFACE_MONITOR || guid == GUID_DEVCLASS_MONITOR {
        0x5004 // Monitor only
    } else {
        0x500B // Mouse, keyboard, and joystick (default fallback for HID or generic)
    }
}

pub unsafe extern "win64" fn SetupDiGetClassDevsW(
    class_guid: *const u8,
    _enumerator: *const u16,
    _parent: usize,
    flags: u32,
) -> usize {
    let handle = determine_device_mask(class_guid);
    crate::dlls::kernel32::SetLastError(0);
    let guid_str = if !class_guid.is_null() {
        guid_to_string(std::slice::from_raw_parts(class_guid, 16))
    } else {
        "ALL_CLASSES".to_string()
    };
    println!(
        "[SETUPAPI] SetupDiGetClassDevsW(guid={}, flags=0x{:X}) -> HDEVINFO 0x{:X}",
        guid_str, flags, handle
    );
    handle
}

#[no_mangle]
pub unsafe extern "win64" fn SetupDiGetClassDevsExW(
    class_guid: *const u8,
    enumerator: *const u16,
    parent: usize,
    flags: u32,
    _device_info_set: usize,
    _machine_name: *const u16,
    _reserved: usize,
) -> usize {
    SetupDiGetClassDevsW(class_guid, enumerator, parent, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn SetupDiGetClassDevsExA(
    class_guid: *const u8,
    enumerator: *const u8,
    parent: usize,
    flags: u32,
    _device_info_set: usize,
    _machine_name: *const u8,
    _reserved: usize,
) -> usize {
    SetupDiGetClassDevsA(class_guid, enumerator, parent, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn SetupGetInfDriverStoreLocationW(
    _file_name: *const u16,
    _alternate_platform_info: usize,
    _locale_name: *const u16,
    _return_buffer: *mut u16,
    _return_buffer_size: u32,
    required_size: *mut u32,
) -> i32 {
    if !required_size.is_null() {
        *required_size = 0;
    }
    crate::dlls::kernel32::SetLastError(2); // ERROR_FILE_NOT_FOUND
    0 // FALSE
}

pub unsafe extern "win64" fn SetupDiGetClassDevsA(
    class_guid: *const u8,
    _enumerator: *const u8,
    _parent: usize,
    flags: u32,
) -> usize {
    let handle = determine_device_mask(class_guid);
    crate::dlls::kernel32::SetLastError(0);
    let guid_str = if !class_guid.is_null() {
        guid_to_string(std::slice::from_raw_parts(class_guid, 16))
    } else {
        "ALL_CLASSES".to_string()
    };
    println!(
        "[SETUPAPI] SetupDiGetClassDevsA(guid={}, flags=0x{:X}) -> HDEVINFO 0x{:X}",
        guid_str, flags, handle
    );
    handle
}

pub unsafe extern "win64" fn SetupDiEnumDeviceInterfaces(
    dev_info: usize,
    _dev_info_data: *const u8,
    interface_class_guid: *const u8,
    member_index: u32,
    dev_interface_data: *mut u8,
) -> i32 {
    let has_mouse = (dev_info & 1) != 0;
    let has_kbd = (dev_info & 2) != 0;
    let has_monitor = (dev_info & 4) != 0;
    let has_joystick = (dev_info & 8) != 0;

    let mut candidates = Vec::new();
    if !interface_class_guid.is_null() {
        let iguid = std::slice::from_raw_parts(interface_class_guid, 16);
        if iguid == GUID_DEVINTERFACE_MOUSE || iguid == GUID_DEVCLASS_MOUSE {
            if has_mouse {
                candidates.push(0);
            }
        } else if iguid == GUID_DEVINTERFACE_KEYBOARD || iguid == GUID_DEVCLASS_KEYBOARD {
            if has_kbd {
                candidates.push(1);
            }
        } else if iguid == GUID_DEVINTERFACE_MONITOR || iguid == GUID_DEVCLASS_MONITOR {
            if has_monitor {
                candidates.push(2);
            }
        } else {
            // GUID_DEVINTERFACE_HID or other
            if has_mouse {
                candidates.push(0);
            }
            if has_kbd {
                candidates.push(1);
            }
            if has_joystick {
                candidates.push(3);
            }
        }
    } else {
        if has_mouse {
            candidates.push(0);
        }
        if has_kbd {
            candidates.push(1);
        }
        if has_monitor {
            candidates.push(2);
        }
        if has_joystick {
            candidates.push(3);
        }
    }

    if member_index as usize >= candidates.len() {
        crate::dlls::kernel32::SetLastError(ERROR_NO_MORE_ITEMS);
        return 0; // FALSE
    }

    let dev_id = candidates[member_index as usize];
    if !dev_interface_data.is_null() {
        let target_guid = match dev_id {
            1 => GUID_DEVINTERFACE_KEYBOARD,
            2 => GUID_DEVINTERFACE_MONITOR,
            3 => GUID_DEVINTERFACE_HID,
            _ => GUID_DEVINTERFACE_MOUSE,
        };
        // SP_DEVICE_INTERFACE_DATA layout:
        // offset 0: cbSize (DWORD, 4 bytes)
        // offset 4: InterfaceClassGuid (GUID, 16 bytes)
        // offset 20: Flags (DWORD, 4 bytes) -> SPINT_ACTIVE (1)
        // offset 24: Reserved (ULONG_PTR, 8 bytes) -> dev_id
        std::ptr::copy_nonoverlapping(target_guid.as_ptr(), dev_interface_data.add(4), 16);
        *(dev_interface_data.add(20) as *mut u32) = 1; // SPINT_ACTIVE
        *(dev_interface_data.add(24) as *mut usize) = dev_id;
    }

    crate::dlls::kernel32::SetLastError(0);
    println!(
        "[SETUPAPI] SetupDiEnumDeviceInterfaces(dev_info=0x{:X}, idx={}) -> TRUE (dev_id={})",
        dev_info, member_index, dev_id
    );
    1 // TRUE
}

pub unsafe extern "win64" fn SetupDiGetDeviceInterfaceDetailW(
    _dev_info: usize,
    dev_interface_data: *const u8,
    dev_interface_detail_data: *mut u8,
    dev_interface_detail_data_size: u32,
    required_size: *mut u32,
    dev_info_data: *mut u8,
) -> i32 {
    if dev_interface_data.is_null() {
        crate::dlls::kernel32::SetLastError(ERROR_INVALID_PARAMETER);
        return 0;
    }

    let dev_id = *(dev_interface_data.add(24) as *const usize);
    let path_str = match dev_id {
        1 => KEYBOARD_DEVICE_PATH,
        2 => MONITOR_DEVICE_PATH,
        3 => JOYSTICK_DEVICE_PATH,
        _ => MOUSE_DEVICE_PATH,
    };

    let path_utf16: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
    // Offset 0..4 is cbSize. DevicePath starts at offset 4.
    let req = (4 + path_utf16.len() * 2) as u32;
    if !required_size.is_null() {
        *required_size = req;
    }

    if dev_interface_detail_data.is_null() || dev_interface_detail_data_size < req {
        crate::dlls::kernel32::SetLastError(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }

    let dest = dev_interface_detail_data.add(4) as *mut u16;
    std::ptr::copy_nonoverlapping(path_utf16.as_ptr(), dest, path_utf16.len());

    if !dev_info_data.is_null() {
        let class_guid = match dev_id {
            1 => GUID_DEVCLASS_KEYBOARD,
            2 => GUID_DEVCLASS_MONITOR,
            3 => GUID_DEVCLASS_HIDCLASS,
            _ => GUID_DEVCLASS_MOUSE,
        };
        *(dev_info_data as *mut u32) = 32;
        std::ptr::copy_nonoverlapping(class_guid.as_ptr(), dev_info_data.add(4), 16);
        *(dev_info_data.add(20) as *mut u32) = (dev_id + 1) as u32;
        *(dev_info_data.add(24) as *mut usize) = 0;
    }

    crate::dlls::kernel32::SetLastError(0);
    println!(
        "[SETUPAPI] SetupDiGetDeviceInterfaceDetailW(dev_id={}) -> TRUE path='{}'",
        dev_id, path_str
    );
    1 // TRUE
}

pub unsafe extern "win64" fn SetupDiGetDeviceInterfaceDetailA(
    _dev_info: usize,
    dev_interface_data: *const u8,
    dev_interface_detail_data: *mut u8,
    dev_interface_detail_data_size: u32,
    required_size: *mut u32,
    dev_info_data: *mut u8,
) -> i32 {
    if dev_interface_data.is_null() {
        crate::dlls::kernel32::SetLastError(ERROR_INVALID_PARAMETER);
        return 0;
    }

    let dev_id = *(dev_interface_data.add(24) as *const usize);
    let path_str = match dev_id {
        1 => KEYBOARD_DEVICE_PATH,
        2 => MONITOR_DEVICE_PATH,
        3 => JOYSTICK_DEVICE_PATH,
        _ => MOUSE_DEVICE_PATH,
    };

    let path_bytes = path_str.as_bytes();
    let req = (4 + path_bytes.len() + 1) as u32;
    if !required_size.is_null() {
        *required_size = req;
    }

    if dev_interface_detail_data.is_null() || dev_interface_detail_data_size < req {
        crate::dlls::kernel32::SetLastError(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }

    let dest = dev_interface_detail_data.add(4);
    std::ptr::copy_nonoverlapping(path_bytes.as_ptr(), dest, path_bytes.len());
    *dest.add(path_bytes.len()) = 0;

    if !dev_info_data.is_null() {
        let class_guid = match dev_id {
            1 => GUID_DEVCLASS_KEYBOARD,
            2 => GUID_DEVCLASS_MONITOR,
            3 => GUID_DEVCLASS_HIDCLASS,
            _ => GUID_DEVCLASS_MOUSE,
        };
        *(dev_info_data as *mut u32) = 32;
        std::ptr::copy_nonoverlapping(class_guid.as_ptr(), dev_info_data.add(4), 16);
        *(dev_info_data.add(20) as *mut u32) = (dev_id + 1) as u32;
        *(dev_info_data.add(24) as *mut usize) = 0;
    }

    crate::dlls::kernel32::SetLastError(0);
    println!(
        "[SETUPAPI] SetupDiGetDeviceInterfaceDetailA(dev_id={}) -> TRUE path='{}'",
        dev_id, path_str
    );
    1 // TRUE
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
    INVALID_HANDLE_VALUE
}

pub unsafe extern "win64" fn SetupDiDestroyDeviceInfoList(_dev_info: usize) -> i32 {
    crate::dlls::kernel32::SetLastError(0);
    1 // TRUE
}

pub unsafe extern "win64" fn SetupDiEnumDeviceInfo(
    dev_info: usize,
    member_index: u32,
    dev_info_data: *mut u8,
) -> i32 {
    let has_mouse = (dev_info & 1) != 0;
    let has_kbd = (dev_info & 2) != 0;
    let has_monitor = (dev_info & 4) != 0;
    let has_joystick = (dev_info & 8) != 0;
    let mut candidates = Vec::new();
    if has_mouse {
        candidates.push(0);
    }
    if has_kbd {
        candidates.push(1);
    }
    if has_monitor {
        candidates.push(2);
    }
    if has_joystick {
        candidates.push(3);
    }

    if member_index as usize >= candidates.len() {
        crate::dlls::kernel32::SetLastError(ERROR_NO_MORE_ITEMS);
        return 0; // FALSE
    }

    let dev_id = candidates[member_index as usize];
    if !dev_info_data.is_null() {
        let class_guid = match dev_id {
            1 => GUID_DEVCLASS_KEYBOARD,
            2 => GUID_DEVCLASS_MONITOR,
            3 => GUID_DEVCLASS_HIDCLASS,
            _ => GUID_DEVCLASS_MOUSE,
        };
        *(dev_info_data as *mut u32) = 32;
        std::ptr::copy_nonoverlapping(class_guid.as_ptr(), dev_info_data.add(4), 16);
        *(dev_info_data.add(20) as *mut u32) = (dev_id + 1) as u32;
        *(dev_info_data.add(24) as *mut usize) = 0;
    }

    crate::dlls::kernel32::SetLastError(0);
    1 // TRUE
}

pub const SPDRP_DEVICEDESC: u32 = 0x00000000;
pub const SPDRP_HARDWAREID: u32 = 0x00000001;
pub const SPDRP_CLASS: u32 = 0x00000007;
pub const SPDRP_MFG: u32 = 0x0000000B;
pub const SPDRP_FRIENDLYNAME: u32 = 0x0000000C;

pub const REG_SZ: u32 = 1;
pub const REG_MULTI_SZ: u32 = 7;

#[no_mangle]
pub unsafe extern "win64" fn SetupDiGetDeviceRegistryPropertyW(
    _dev_info: usize,
    dev_info_data: *const u8,
    property: u32,
    property_reg_data_type: *mut u32,
    property_buffer: *mut u8,
    property_buffer_size: u32,
    required_size: *mut u32,
) -> i32 {
    if dev_info_data.is_null() {
        crate::dlls::kernel32::SetLastError(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let dev_inst = *(dev_info_data.add(20) as *const u32);
    let dev_id = if dev_inst > 0 { dev_inst - 1 } else { 0 };

    let (val_str, is_multi) = match property {
        SPDRP_DEVICEDESC | SPDRP_FRIENDLYNAME => {
            if dev_id == 1 {
                ("Standard PS/2 Keyboard\0", false)
            } else if dev_id == 2 {
                ("Generic Non-PnP Monitor\0", false)
            } else if dev_id == 3 {
                ("Xbox 360 Controller for Windows\0", false)
            } else {
                ("HID-compliant Mouse\0", false)
            }
        }
        SPDRP_HARDWAREID => {
            if dev_id == 1 {
                ("HID\\VID_046D&PID_C31C\0\0", true)
            } else if dev_id == 2 {
                ("MONITOR\\Default_Monitor\0\0", true)
            } else if dev_id == 3 {
                ("HID\\VID_045E&PID_028E&IG_00\0HID_DEVICE_SYSTEM_GAME\0\0", true)
            } else {
                ("HID\\VID_046D&PID_C077\0\0", true)
            }
        }
        SPDRP_CLASS => {
            if dev_id == 1 {
                ("Keyboard\0", false)
            } else if dev_id == 2 {
                ("Monitor\0", false)
            } else if dev_id == 3 {
                ("HIDClass\0", false)
            } else {
                ("Mouse\0", false)
            }
        }
        SPDRP_MFG => {
            if dev_id == 3 {
                ("Microsoft\0", false)
            } else {
                ("(Standard system devices)\0", false)
            }
        }
        _ => {
            crate::dlls::kernel32::SetLastError(ERROR_FILE_NOT_FOUND);
            return 0;
        }
    };

    let utf16: Vec<u16> = val_str.encode_utf16().collect();
    let bytes_len = (utf16.len() * 2) as u32;

    if !required_size.is_null() {
        *required_size = bytes_len;
    }
    if !property_reg_data_type.is_null() {
        *property_reg_data_type = if is_multi { REG_MULTI_SZ } else { REG_SZ };
    }

    if property_buffer.is_null() || property_buffer_size < bytes_len {
        crate::dlls::kernel32::SetLastError(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }

    std::ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, property_buffer, bytes_len as usize);
    crate::dlls::kernel32::SetLastError(0);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetupDiGetDeviceRegistryPropertyA(
    _dev_info: usize,
    dev_info_data: *const u8,
    property: u32,
    property_reg_data_type: *mut u32,
    property_buffer: *mut u8,
    property_buffer_size: u32,
    required_size: *mut u32,
) -> i32 {
    if dev_info_data.is_null() {
        crate::dlls::kernel32::SetLastError(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let dev_inst = *(dev_info_data.add(20) as *const u32);
    let dev_id = if dev_inst > 0 { dev_inst - 1 } else { 0 };

    let (val_str, is_multi) = match property {
        SPDRP_DEVICEDESC | SPDRP_FRIENDLYNAME => {
            if dev_id == 1 {
                ("Standard PS/2 Keyboard\0", false)
            } else if dev_id == 2 {
                ("Generic Non-PnP Monitor\0", false)
            } else if dev_id == 3 {
                ("Xbox 360 Controller for Windows\0", false)
            } else {
                ("HID-compliant Mouse\0", false)
            }
        }
        SPDRP_HARDWAREID => {
            if dev_id == 1 {
                ("HID\\VID_046D&PID_C31C\0\0", true)
            } else if dev_id == 2 {
                ("MONITOR\\Default_Monitor\0\0", true)
            } else if dev_id == 3 {
                ("HID\\VID_045E&PID_028E&IG_00\0HID_DEVICE_SYSTEM_GAME\0\0", true)
            } else {
                ("HID\\VID_046D&PID_C077\0\0", true)
            }
        }
        SPDRP_CLASS => {
            if dev_id == 1 {
                ("Keyboard\0", false)
            } else if dev_id == 2 {
                ("Monitor\0", false)
            } else if dev_id == 3 {
                ("HIDClass\0", false)
            } else {
                ("Mouse\0", false)
            }
        }
        SPDRP_MFG => {
            if dev_id == 3 {
                ("Microsoft\0", false)
            } else {
                ("(Standard system devices)\0", false)
            }
        }
        _ => {
            crate::dlls::kernel32::SetLastError(ERROR_FILE_NOT_FOUND);
            return 0;
        }
    };

    let bytes = val_str.as_bytes();
    let bytes_len = bytes.len() as u32;

    if !required_size.is_null() {
        *required_size = bytes_len;
    }
    if !property_reg_data_type.is_null() {
        *property_reg_data_type = if is_multi { REG_MULTI_SZ } else { REG_SZ };
    }

    if property_buffer.is_null() || property_buffer_size < bytes_len {
        crate::dlls::kernel32::SetLastError(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }

    std::ptr::copy_nonoverlapping(bytes.as_ptr(), property_buffer, bytes_len as usize);
    crate::dlls::kernel32::SetLastError(0);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetupDiGetDeviceInstanceIdW(
    _device_info_set: usize,
    device_info_data: *const u8,
    device_instance_id: *mut u16,
    device_instance_id_size: u32,
    required_size: *mut u32,
) -> i32 {
    let dev_id = if !device_info_data.is_null() {
        let dev_inst = *(device_info_data.add(20) as *const u32);
        if dev_inst > 0 { dev_inst - 1 } else { 0 }
    } else {
        0
    };
    let id_str: &str = if dev_id == 1 {
        "HID\\VID_046D&PID_C31C\\1&0000&0&0000"
    } else if dev_id == 3 {
        "HID\\VID_045E&PID_028E&IG_00\\1&0000&0&0000"
    } else {
        "HID\\VID_046D&PID_C077\\1&0000&0&0000"
    };
    let utf16: Vec<u16> = id_str.encode_utf16().chain(std::iter::once(0)).collect();
    if !required_size.is_null() {
        *required_size = utf16.len() as u32;
    }
    if device_instance_id.is_null() || device_instance_id_size < (utf16.len() as u32) {
        crate::dlls::kernel32::SetLastError(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    std::ptr::copy_nonoverlapping(utf16.as_ptr(), device_instance_id, utf16.len());
    crate::dlls::kernel32::SetLastError(0);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetupDiGetDeviceInstanceIdA(
    _device_info_set: usize,
    device_info_data: *const u8,
    device_instance_id: *mut u8,
    device_instance_id_size: u32,
    required_size: *mut u32,
) -> i32 {
    let dev_id = if !device_info_data.is_null() {
        let dev_inst = *(device_info_data.add(20) as *const u32);
        if dev_inst > 0 { dev_inst - 1 } else { 0 }
    } else {
        0
    };
    let id_str: &[u8] = if dev_id == 1 {
        b"HID\\VID_046D&PID_C31C\\1&0000&0&0000\0"
    } else if dev_id == 3 {
        b"HID\\VID_045E&PID_028E&IG_00\\1&0000&0&0000\0"
    } else {
        b"HID\\VID_046D&PID_C077\\1&0000&0&0000\0"
    };
    if !required_size.is_null() {
        *required_size = id_str.len() as u32;
    }
    if device_instance_id.is_null() || device_instance_id_size < (id_str.len() as u32) {
        crate::dlls::kernel32::SetLastError(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    std::ptr::copy_nonoverlapping(id_str.as_ptr(), device_instance_id, id_str.len());
    crate::dlls::kernel32::SetLastError(0);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetupDiGetDevicePropertyW(
    _device_info_set: usize,
    device_info_data: *const u8,
    _property_key: *const u8,
    property_type: *mut u32,
    property_buffer: *mut u8,
    property_buffer_size: u32,
    required_size: *mut u32,
    _flags: u32,
) -> i32 {
    let dev_id = if !device_info_data.is_null() {
        let dev_inst = *(device_info_data.add(20) as *const u32);
        if dev_inst > 0 {
            dev_inst - 1
        } else {
            0
        }
    } else {
        0
    };

    let name = if dev_id == 1 {
        "USB Keyboard\0"
    } else {
        "USB Optical Mouse\0"
    };
    let utf16: Vec<u16> = name.encode_utf16().collect();
    let needed = (utf16.len() * 2) as u32;

    if !property_type.is_null() {
        *property_type = 18; // DEVPROP_TYPE_STRING
    }
    if !required_size.is_null() {
        *required_size = needed;
    }

    if property_buffer.is_null() || property_buffer_size < needed {
        crate::dlls::kernel32::SetLastError(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }

    std::ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, property_buffer, needed as usize);
    crate::dlls::kernel32::SetLastError(0);
    1
}
