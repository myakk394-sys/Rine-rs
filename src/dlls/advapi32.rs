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
