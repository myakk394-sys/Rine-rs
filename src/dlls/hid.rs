//! Implementation of HID.DLL (Human Interface Device APIs)
#![allow(non_snake_case)]

use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WinGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

// GUID_DEVINTERFACE_HID = {4D1E55B2-F16F-11CF-88CB-001111000030}
pub const GUID_DEVINTERFACE_HID: WinGuid = WinGuid {
    data1: 0x4D1E55B2,
    data2: 0xF16F,
    data3: 0x11CF,
    data4: [0x88, 0xCB, 0x00, 0x11, 0x11, 0x00, 0x00, 0x30],
};

pub const HIDP_STATUS_SUCCESS: i32 = 0x00110000;
pub const HIDP_STATUS_INVALID_PREPARSED_DATA: i32 = 0xC0110001u32 as i32;
pub const HIDP_STATUS_INVALID_REPORT_TYPE: i32 = 0xC0110002u32 as i32;
pub const HIDP_STATUS_INVALID_REPORT_LENGTH: i32 = 0xC0110003u32 as i32;
pub const HIDP_STATUS_USAGE_NOT_FOUND: i32 = 0xC0110004u32 as i32;

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetHidGuid(hid_guid: *mut WinGuid) {
    if !hid_guid.is_null() {
        *hid_guid = GUID_DEVINTERFACE_HID;
    }
    eprintln!(
        "[HID] HidD_GetHidGuid -> {:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        GUID_DEVINTERFACE_HID.data1,
        GUID_DEVINTERFACE_HID.data2,
        GUID_DEVINTERFACE_HID.data3,
        GUID_DEVINTERFACE_HID.data4[0],
        GUID_DEVINTERFACE_HID.data4[1],
        GUID_DEVINTERFACE_HID.data4[2],
        GUID_DEVINTERFACE_HID.data4[3],
        GUID_DEVINTERFACE_HID.data4[4],
        GUID_DEVINTERFACE_HID.data4[5],
        GUID_DEVINTERFACE_HID.data4[6],
        GUID_DEVINTERFACE_HID.data4[7]
    );
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetPreparsedData(
    _hid_device_object: usize,
    preparsed_data: *mut usize,
) -> u8 {
    if !preparsed_data.is_null() {
        *preparsed_data = 0x200000;
    }
    crate::dlls::kernel32::SetLastError(0);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_FreePreparsedData(_preparsed_data: usize) -> u8 {
    eprintln!("[HID] HidD_FreePreparsedData -> TRUE");
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetAttributes(
    _hid_device_object: usize,
    attributes: *mut c_void,
) -> u8 {
    if attributes.is_null() {
        crate::dlls::kernel32::SetLastError(crate::dlls::setupapi::ERROR_INVALID_PARAMETER);
        return 0;
    }
    // HIDD_ATTRIBUTES: size(4), vendor_id(2), product_id(2), version_number(2)
    let size = *(attributes as *const u32);
    if size >= 10 {
        *(attributes.add(4) as *mut u16) = 0x046D; // Logitech
        *(attributes.add(6) as *mut u16) = 0xC077; // Optical Mouse
        *(attributes.add(8) as *mut u16) = 0x0100;
        crate::dlls::kernel32::SetLastError(0);
        eprintln!("[HID] HidD_GetAttributes -> TRUE (046D:C077)");
        return 1;
    }
    crate::dlls::kernel32::SetLastError(crate::dlls::setupapi::ERROR_INVALID_PARAMETER);
    eprintln!("[HID] HidD_GetAttributes -> FALSE");
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetManufacturerString(
    _hid_device_object: usize,
    buffer: *mut u16,
    buffer_length: u32,
) -> u8 {
    if !buffer.is_null() && buffer_length >= 18 {
        let s: Vec<u16> = "Logitech\0".encode_utf16().collect();
        let copy_len = (buffer_length as usize / 2).min(s.len());
        std::ptr::copy_nonoverlapping(s.as_ptr(), buffer, copy_len);
        crate::dlls::kernel32::SetLastError(0);
        return 1;
    }
    crate::dlls::kernel32::SetLastError(crate::dlls::setupapi::ERROR_INVALID_PARAMETER);
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetProductString(
    _hid_device_object: usize,
    buffer: *mut u16,
    buffer_length: u32,
) -> u8 {
    if !buffer.is_null() && buffer_length >= 28 {
        let s: Vec<u16> = "Optical Mouse\0".encode_utf16().collect();
        let copy_len = (buffer_length as usize / 2).min(s.len());
        std::ptr::copy_nonoverlapping(s.as_ptr(), buffer, copy_len);
        crate::dlls::kernel32::SetLastError(0);
        return 1;
    }
    crate::dlls::kernel32::SetLastError(crate::dlls::setupapi::ERROR_INVALID_PARAMETER);
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetSerialNumberString(
    _hid_device_object: usize,
    buffer: *mut u16,
    buffer_length: u32,
) -> u8 {
    if !buffer.is_null() && buffer_length >= 10 {
        let s: Vec<u16> = "0001\0".encode_utf16().collect();
        let copy_len = (buffer_length as usize / 2).min(s.len());
        std::ptr::copy_nonoverlapping(s.as_ptr(), buffer, copy_len);
        crate::dlls::kernel32::SetLastError(0);
        return 1;
    }
    crate::dlls::kernel32::SetLastError(crate::dlls::setupapi::ERROR_INVALID_PARAMETER);
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetIndexedString(
    _hid_device_object: usize,
    _string_index: u32,
    _buffer: *mut u16,
    _buffer_length: u32,
) -> u8 {
    crate::dlls::kernel32::SetLastError(crate::dlls::setupapi::ERROR_INVALID_PARAMETER);
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetFeature(
    _hid_device_object: usize,
    _report_buffer: *mut c_void,
    _report_buffer_length: u32,
) -> u8 {
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_SetFeature(
    _hid_device_object: usize,
    _report_buffer: *const c_void,
    _report_buffer_length: u32,
) -> u8 {
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetInputReport(
    _hid_device_object: usize,
    _report_buffer: *mut c_void,
    _report_buffer_length: u32,
) -> u8 {
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_SetOutputReport(
    _hid_device_object: usize,
    _report_buffer: *const c_void,
    _report_buffer_length: u32,
) -> u8 {
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_GetNumInputBuffers(
    _hid_device_object: usize,
    number_buffers: *mut u32,
) -> u8 {
    if !number_buffers.is_null() {
        *number_buffers = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_SetNumInputBuffers(
    _hid_device_object: usize,
    _number_buffers: u32,
) -> u8 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn HidD_FlushQueue(_hid_device_object: usize) -> u8 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn HidP_GetCaps(
    _preparsed_data: usize,
    capabilities: *mut c_void,
) -> i32 {
    if capabilities.is_null() {
        return HIDP_STATUS_INVALID_PREPARSED_DATA;
    }
    std::ptr::write_bytes(capabilities as *mut u8, 0, 64);
    let caps = capabilities as *mut u16;
    *caps.add(0) = 0x0001; // Usage (Generic Desktop)
    *caps.add(1) = 0x0002; // UsagePage (Mouse)
    *caps.add(2) = 64;     // InputReportByteLength
    *caps.add(3) = 64;     // OutputReportByteLength
    *caps.add(4) = 64;     // FeatureReportByteLength
    *caps.add(22) = 5;     // NumberInputButtonCaps
    *caps.add(23) = 3;     // NumberInputValueCaps
    HIDP_STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn HidP_GetButtonCaps(
    _report_type: u32,
    _button_caps: *mut c_void,
    button_caps_length: *mut u16,
    _preparsed_data: usize,
) -> i32 {
    if !button_caps_length.is_null() {
        *button_caps_length = 0;
    }
    HIDP_STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn HidP_GetValueCaps(
    _report_type: u32,
    _value_caps: *mut c_void,
    value_caps_length: *mut u16,
    _preparsed_data: usize,
) -> i32 {
    if !value_caps_length.is_null() {
        *value_caps_length = 0;
    }
    HIDP_STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn HidP_GetData(
    _report_type: u32,
    _data_list: *mut c_void,
    data_length: *mut u32,
    _preparsed_data: usize,
    _report: *const u8,
    _report_length: u32,
) -> i32 {
    if !data_length.is_null() {
        *data_length = 0;
    }
    HIDP_STATUS_INVALID_PREPARSED_DATA
}

#[no_mangle]
pub unsafe extern "win64" fn HidP_MaxDataListLength(
    _report_type: u32,
    _preparsed_data: usize,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn HidP_SetUsages(
    _report_type: u32,
    _usage_page: u16,
    _link_collection: u16,
    _usage_list: *mut u16,
    usage_length: *mut u32,
    _preparsed_data: usize,
    _report: *mut u8,
    _report_length: u32,
) -> i32 {
    if !usage_length.is_null() {
        *usage_length = 0;
    }
    HIDP_STATUS_INVALID_PREPARSED_DATA
}

#[no_mangle]
pub unsafe extern "win64" fn HidP_SetUsageValue(
    _report_type: u32,
    _usage_page: u16,
    _link_collection: u16,
    _usage: u16,
    _usage_value: u32,
    _preparsed_data: usize,
    _report: *mut u8,
    _report_length: u32,
) -> i32 {
    HIDP_STATUS_INVALID_PREPARSED_DATA
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hid_guid() {
        let mut guid: WinGuid = WinGuid {
            data1: 0,
            data2: 0,
            data3: 0,
            data4: [0; 8],
        };
        unsafe {
            HidD_GetHidGuid(&mut guid);
        }
        assert_eq!(guid.data1, 0x4D1E55B2);
        assert_eq!(guid.data2, 0xF16F);
        assert_eq!(guid.data3, 0x11CF);
        assert_eq!(guid.data4, [0x88, 0xCB, 0x00, 0x11, 0x11, 0x00, 0x00, 0x30]);
    }
}
