#![allow(non_snake_case)]
use crate::sys::{NtStatus, STATUS_OBJECT_NAME_NOT_FOUND, STATUS_SUCCESS};
use std::collections::HashMap;
use std::sync::Mutex;

pub type HKEY = u64;

pub const HKEY_CLASSES_ROOT: HKEY = 0x80000000;
pub const HKEY_CURRENT_USER: HKEY = 0x80000001;
pub const HKEY_LOCAL_MACHINE: HKEY = 0x80000002;
pub const HKEY_USERS: HKEY = 0x80000003;

pub struct RegistryHive {
    values: Mutex<HashMap<String, Vec<u8>>>,
}

impl RegistryHive {
    pub fn new() -> Self {
        let mut values = HashMap::new();
        values.insert(
            "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\CurrentVersion".into(),
            "10.0".as_bytes().to_vec(),
        );
        values.insert(
            "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\ProductName".into(),
            "Windows 10 Pro".as_bytes().to_vec(),
        );
        Self {
            values: Mutex::new(values),
        }
    }
}

static REGISTRY_INSTANCE: std::sync::OnceLock<RegistryHive> = std::sync::OnceLock::new();

pub fn get_registry() -> &'static RegistryHive {
    REGISTRY_INSTANCE.get_or_init(RegistryHive::new)
}

pub fn RegOpenKeyExW(
    hkey: HKEY,
    sub_key: &str,
    _options: u32,
    _desired_access: u32,
    result_hkey: &mut HKEY,
) -> NtStatus {
    println!(
        "[ADVAPI32] RegOpenKeyExW: HKEY=0x{:08X}, SubKey='{}'",
        hkey, sub_key
    );
    *result_hkey = hkey + 0x10;
    STATUS_SUCCESS
}

pub fn RegQueryValueExW(
    _hkey: HKEY,
    value_name: &str,
    _reserved: Option<&mut u32>,
    type_out: Option<&mut u32>,
    data: Option<&mut Vec<u8>>,
    size: Option<&mut u32>,
) -> NtStatus {
    let reg = get_registry();
    let map = reg.values.lock().unwrap();

    if let Some(val_bytes) = map.get(value_name) {
        if let Some(t) = type_out {
            *t = 1; // REG_SZ (string)
        }
        if let Some(s) = size {
            *s = val_bytes.len() as u32;
        }
        if let Some(d) = data {
            *d = val_bytes.clone();
        }
        STATUS_SUCCESS
    } else {
        STATUS_OBJECT_NAME_NOT_FOUND
    }
}

pub fn RegCloseKey(hkey: HKEY) -> NtStatus {
    println!("[ADVAPI32] RegCloseKey: HKEY=0x{:08X}", hkey);
    STATUS_SUCCESS
}

pub unsafe extern "win64" fn BCryptGenRandom(
    h_algorithm: usize,
    pb_buffer: *mut u8,
    cb_buffer: u32,
    dw_flags: u32,
) -> i32 {
    println!(
        "[BCRYPT ENTRY] BCryptGenRandom hAlgorithm=0x{:X}, pb_buffer={:p}, cb_buffer={}, dwFlags=0x{:X}",
        h_algorithm, pb_buffer, cb_buffer, dw_flags
    );

    // BCRYPT_USE_SYSTEM_PREFERRED_RNG = 0x00000002
    if (dw_flags & 0x00000002) != 0 {
        if h_algorithm != 0 {
            // Must be NULL when BCRYPT_USE_SYSTEM_PREFERRED_RNG is specified
            return -1073741816; // STATUS_INVALID_HANDLE (0xC0000008)
        }
    } else if h_algorithm == 0 {
        return -1073741816; // STATUS_INVALID_HANDLE (0xC0000008)
    }

    if !pb_buffer.is_null() && cb_buffer > 0 {
        let slice = std::slice::from_raw_parts_mut(pb_buffer, cb_buffer as usize);
        // Fill buffer with real OS cryptographic randomness from /dev/urandom
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            use std::io::Read;
            let _ = f.read_exact(slice);
        } else {
            // Fallback to getentropy/system time entropy if /dev/urandom fails
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0x123456789ABCDEF0);
            for (i, byte) in slice.iter_mut().enumerate() {
                *byte = ((seed.wrapping_add(i as u128)) & 0xFF) as u8;
            }
        }
    }

    0 // STATUS_SUCCESS
}

pub unsafe extern "win64" fn AdjustTokenPrivileges(
    _token_handle: usize,
    _disable_all: u32,
    _new_state: usize,
    _buffer_length: u32,
    _previous_state: usize,
    return_length: *mut u32,
) -> u32 {
    println!("[ADVAPI32 ENTRY] AdjustTokenPrivileges");
    if !return_length.is_null() {
        *return_length = 0;
    }
    1 // TRUE
}

pub unsafe extern "win64" fn GetTokenInformation(
    _token_handle: usize,
    _class: u32,
    token_info: *mut u8,
    info_len: u32,
    return_length: *mut u32,
) -> u32 {
    println!(
        "[ADVAPI32 ENTRY] GetTokenInformation class={}, len={}",
        _class, info_len
    );
    if !return_length.is_null() {
        *return_length = info_len;
    }
    if !token_info.is_null() && info_len >= 4 {
        std::ptr::write_bytes(token_info, 0, info_len as usize);
    }
    1 // TRUE
}

pub unsafe extern "win64" fn OpenProcessToken(
    _process_handle: usize,
    _desired_access: u32,
    token_handle: *mut usize,
) -> u32 {
    println!("[ADVAPI32 ENTRY] OpenProcessToken");
    if !token_handle.is_null() {
        *token_handle = 0x1000;
    }
    1 // TRUE
}

pub unsafe extern "win64" fn GetSidSubAuthority(_sid: usize, _sub_authority: u32) -> *mut u32 {
    static mut SUB_AUTH: u32 = 1000;
    println!("[ADVAPI32 ENTRY] GetSidSubAuthority");
    unsafe { &raw mut SUB_AUTH }
}

pub unsafe extern "win64" fn GetSidSubAuthorityCount(_sid: usize) -> *mut u8 {
    static mut COUNT: u8 = 1;
    println!("[ADVAPI32 ENTRY] GetSidSubAuthorityCount");
    unsafe { &raw mut COUNT }
}

pub unsafe extern "win64" fn LookupPrivilegeValueW(
    _system_name: usize,
    _name: usize,
    luid: *mut u64,
) -> u32 {
    println!("[ADVAPI32 ENTRY] LookupPrivilegeValueW");
    if !luid.is_null() {
        *luid = 0x1234;
    }
    1 // TRUE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_query() {
        let mut data = Vec::new();
        let mut size = 0;
        let mut val_type = 0;

        let status = RegQueryValueExW(
            HKEY_LOCAL_MACHINE,
            "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\CurrentVersion",
            None,
            Some(&mut val_type),
            Some(&mut data),
            Some(&mut size),
        );

        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(val_type, 1);
        assert_eq!(std::str::from_utf8(&data).unwrap(), "10.0");
    }
}
