//! ADVAPI32.DLL implementation (Windows Registry & Security APIs)
#![allow(non_snake_case)]
use crate::sys::{NtStatus, STATUS_OBJECT_NAME_NOT_FOUND, STATUS_SUCCESS};
use std::collections::HashMap;
use std::sync::Mutex;

pub type HKEY = u64;

pub const HKEY_CLASSES_ROOT: HKEY = 0x80000000;
pub const HKEY_CURRENT_USER: HKEY = 0x80000001;
pub const HKEY_LOCAL_MACHINE: HKEY = 0x80000002;
pub const HKEY_USERS: HKEY = 0x80000003;
pub const HKEY_PERFORMANCE_DATA: HKEY = 0x80000004;
pub const HKEY_CURRENT_CONFIG: HKEY = 0x80000005;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum RegValue {
    Sz(String),
    Dword(u32),
    Binary(Vec<u8>),
}

impl RegValue {
    pub fn dw_type(&self) -> u32 {
        match self {
            RegValue::Sz(_) => 1,     // REG_SZ
            RegValue::Binary(_) => 3, // REG_BINARY
            RegValue::Dword(_) => 4,  // REG_DWORD
        }
    }

    pub fn to_wide_bytes(&self) -> Vec<u8> {
        match self {
            RegValue::Sz(s) => {
                let mut v: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
                v.push(0);
                v.push(0);
                v
            }
            RegValue::Dword(d) => d.to_le_bytes().to_vec(),
            RegValue::Binary(b) => b.clone(),
        }
    }

    pub fn to_ansi_bytes(&self) -> Vec<u8> {
        match self {
            RegValue::Sz(s) => {
                let mut v = s.as_bytes().to_vec();
                v.push(0);
                v
            }
            RegValue::Dword(d) => d.to_le_bytes().to_vec(),
            RegValue::Binary(b) => b.clone(),
        }
    }
}

pub struct RegistryHive {
    values: Mutex<HashMap<String, RegValue>>,
}

impl RegistryHive {
    pub fn new() -> Self {
        Self {
            values: Mutex::new(init_default_registry()),
        }
    }
}

static REGISTRY_STORE: std::sync::Mutex<Option<HashMap<String, RegValue>>> =
    std::sync::Mutex::new(None);
static REGISTRY_HANDLE_PATHS: std::sync::Mutex<Option<HashMap<u64, String>>> =
    std::sync::Mutex::new(None);
static NEXT_REG_HANDLE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0x8000_1000);

fn normalize_key_str(s: &str) -> String {
    s.trim_matches('\\').to_ascii_lowercase()
}

fn combine_reg_path(prefix: &str, sub: &str) -> String {
    let p1 = prefix.trim_matches('\\');
    let p2 = sub.trim_matches('\\');
    if p1.is_empty() {
        p2.to_ascii_lowercase()
    } else if p2.is_empty() {
        p1.to_ascii_lowercase()
    } else {
        format!("{}\\{}", p1, p2).to_ascii_lowercase()
    }
}

fn get_parent_path(hkey: u64) -> String {
    // Handle standard sign-extended Win64 root keys
    let masked = (hkey & 0xFFFFFFFF) as u32;
    if masked == 0x80000000 {
        return "classes_root".to_string();
    } else if masked == 0x80000001 {
        return "".to_string(); // HKCU root maps directly to subkey paths
    } else if masked == 0x80000002 {
        return "".to_string(); // HKLM root maps directly to subkey paths
    }

    let hpaths = REGISTRY_HANDLE_PATHS.lock().unwrap();
    hpaths.as_ref().and_then(|m| m.get(&hkey).cloned()).unwrap_or_default()
}

pub fn get_registry_file_path() -> Option<std::path::PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        let p = std::path::PathBuf::from(home).join(".local/share/rine/prefixes/default/registry.json");
        Some(p)
    } else {
        None
    }
}

pub fn save_registry_store(map: &HashMap<String, RegValue>) {
    if let Some(path) = get_registry_file_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(map) {
            Ok(json) => {
                let tmp_path = path.with_extension("tmp");
                if std::fs::write(&tmp_path, json.as_bytes()).is_ok() {
                    let _ = std::fs::rename(&tmp_path, &path);
                    println!("[ADVAPI32] Persisted registry store ({} keys) to {:?}", map.len(), path);
                } else {
                    eprintln!("[ADVAPI32] Failed to write tmp registry file at {:?}", tmp_path);
                }
            }
            Err(e) => eprintln!("[ADVAPI32] Failed to serialize registry JSON: {}", e),
        }
    }
}

fn flush_registry_on_shutdown() {
    if let Ok(store) = REGISTRY_STORE.lock() {
        if let Some(map) = store.as_ref() {
            save_registry_store(map);
        }
    }
}

fn init_default_registry() -> HashMap<String, RegValue> {
    let mut map = HashMap::new();

    // Windows NT version keys
    map.insert(
        "software\\microsoft\\windows nt\\currentversion\\currentversion".into(),
        RegValue::Sz("10.0".into()),
    );
    map.insert(
        "software\\microsoft\\windows nt\\currentversion\\productname".into(),
        RegValue::Sz("Windows 10 Pro".into()),
    );
    map.insert(
        "software\\microsoft\\windows nt\\currentversion\\buildlabex".into(),
        RegValue::Sz("19041.1.amd64fre.vb_release.191206-1406".into()),
    );
    map.insert(
        "software\\microsoft\\windows nt\\currentversion\\currentbuild".into(),
        RegValue::Sz("19045".into()),
    );
    map.insert(
        "software\\microsoft\\windows nt\\currentversion\\currentbuildnumber".into(),
        RegValue::Sz("19045".into()),
    );
    map.insert(
        "software\\microsoft\\windows nt\\currentversion\\editionid".into(),
        RegValue::Sz("Professional".into()),
    );
    map.insert(
        "software\\microsoft\\windows nt\\currentversion\\installationtype".into(),
        RegValue::Sz("Client".into()),
    );

    // Valve Steam keys
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let steam_pid = std::fs::read_to_string(format!("{}/.steam/steam.pid", home))
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(54820);
    map.insert(
        "software\\valve\\steam\\activeprocess\\pid".into(),
        RegValue::Dword(steam_pid),
    );
    map.insert(
        "software\\valve\\steam\\activeprocess\\activeuser".into(),
        RegValue::Dword(1),
    );
    map.insert(
        "software\\valve\\steam\\activeprocess\\steampath".into(),
        RegValue::Sz("C:\\Program Files (x86)\\Steam".into()),
    );
    map.insert(
        "software\\valve\\steam\\activeprocess\\steamclientdll".into(),
        RegValue::Sz("C:\\Program Files (x86)\\Steam\\steamclient.dll".into()),
    );
    map.insert(
        "software\\valve\\steam\\activeprocess\\steamclientdll64".into(),
        RegValue::Sz("C:\\Program Files (x86)\\Steam\\steamclient64.dll".into()),
    );
    map.insert(
        "software\\valve\\steam\\activeprocess\\universe".into(),
        RegValue::Sz("Public".into()),
    );

    // Valve Steam general keys
    map.insert(
        "software\\valve\\steam\\steampath".into(),
        RegValue::Sz("C:\\Program Files (x86)\\Steam".into()),
    );
    map.insert(
        "software\\valve\\steam\\installpath".into(),
        RegValue::Sz("C:\\Program Files (x86)\\Steam".into()),
    );
    map.insert(
        "software\\valve\\steam\\steamexe".into(),
        RegValue::Sz("C:\\Program Files (x86)\\Steam\\Steam.exe".into()),
    );
    map.insert(
        "software\\valve\\steam\\language".into(),
        RegValue::Sz("english".into()),
    );
    map.insert(
        "software\\valve\\steam\\autologinuser".into(),
        RegValue::Sz("player".into()),
    );

    // Register shutdown cleanup hook once
    static HOOK_REGISTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !HOOK_REGISTERED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        crate::sys::shutdown::register_cleanup_hook(flush_registry_on_shutdown);
    }

    // Load persisted registry keys from disk if present
    if let Some(path) = get_registry_file_path() {
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(loaded) = serde_json::from_str::<HashMap<String, RegValue>>(&data) {
                    println!("[ADVAPI32] Loaded {} persisted registry keys from {:?}", loaded.len(), path);
                    for (k, v) in loaded {
                        map.insert(k, v);
                    }
                }
            }
        }
    }

    map
}

fn get_registry_store() -> std::sync::MutexGuard<'static, Option<HashMap<String, RegValue>>> {
    let mut g = REGISTRY_STORE.lock().unwrap();
    if g.is_none() {
        *g = Some(init_default_registry());
    }
    g
}

pub unsafe extern "win64" fn RegOpenKeyExW(
    hkey: u64,
    lp_sub_key: *const u16,
    _ul_options: u32,
    _sam_desired: u32,
    phk_result: *mut u64,
) -> i32 {
    let sub = if !lp_sub_key.is_null() {
        let mut len = 0;
        while *lp_sub_key.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(lp_sub_key, len);
        String::from_utf16_lossy(slice)
    } else {
        String::new()
    };
    let parent = get_parent_path(hkey);
    let combined = combine_reg_path(&parent, &sub);
    println!(
        "[ADVAPI32] RegOpenKeyExW: HKEY=0x{:08X}, SubKey='{}' -> combined='{}'",
        hkey, sub, combined
    );
    let handle = NEXT_REG_HANDLE.fetch_add(4, std::sync::atomic::Ordering::Relaxed);
    if !phk_result.is_null() {
        *phk_result = handle;
    }
    let mut hpaths = REGISTRY_HANDLE_PATHS.lock().unwrap();
    hpaths.get_or_insert_with(HashMap::new).insert(handle, combined);
    0 // ERROR_SUCCESS
}

pub unsafe extern "win64" fn RegOpenKeyExA(
    hkey: u64,
    lp_sub_key: *const u8,
    _ul_options: u32,
    _sam_desired: u32,
    phk_result: *mut u64,
) -> i32 {
    let sub = if !lp_sub_key.is_null() {
        let cstr = std::ffi::CStr::from_ptr(lp_sub_key as *const libc::c_char);
        cstr.to_str().unwrap_or("").to_string()
    } else {
        String::new()
    };
    let parent = get_parent_path(hkey);
    let combined = combine_reg_path(&parent, &sub);
    println!(
        "[ADVAPI32] RegOpenKeyExA: HKEY=0x{:08X}, SubKey='{}' -> combined='{}'",
        hkey, sub, combined
    );
    let handle = NEXT_REG_HANDLE.fetch_add(4, std::sync::atomic::Ordering::Relaxed);
    if !phk_result.is_null() {
        *phk_result = handle;
    }
    let mut hpaths = REGISTRY_HANDLE_PATHS.lock().unwrap();
    hpaths.get_or_insert_with(HashMap::new).insert(handle, combined);
    0
}

pub unsafe extern "win64" fn RegOpenKeyW(
    hkey: u64,
    lp_sub_key: *const u16,
    phk_result: *mut u64,
) -> i32 {
    RegOpenKeyExW(hkey, lp_sub_key, 0, 0, phk_result)
}

pub unsafe extern "win64" fn RegOpenKeyA(
    hkey: u64,
    lp_sub_key: *const u8,
    phk_result: *mut u64,
) -> i32 {
    RegOpenKeyExA(hkey, lp_sub_key, 0, 0, phk_result)
}

pub unsafe extern "win64" fn RegCreateKeyW(
    hkey: u64,
    lp_sub_key: *const u16,
    phk_result: *mut u64,
) -> i32 {
    RegOpenKeyExW(hkey, lp_sub_key, 0, 0, phk_result)
}

pub unsafe extern "win64" fn RegCreateKeyA(
    hkey: u64,
    lp_sub_key: *const u8,
    phk_result: *mut u64,
) -> i32 {
    RegOpenKeyExA(hkey, lp_sub_key, 0, 0, phk_result)
}

pub unsafe extern "win64" fn RegCreateKeyExW(
    hkey: u64,
    lp_sub_key: *const u16,
    _reserved: u32,
    _lp_class: *const u16,
    _dw_options: u32,
    _sam_desired: u32,
    _lp_security_attributes: *const libc::c_void,
    phk_result: *mut u64,
    lpdw_disposition: *mut u32,
) -> i32 {
    if !lpdw_disposition.is_null() {
        *lpdw_disposition = 1;
    }
    RegOpenKeyExW(hkey, lp_sub_key, 0, 0, phk_result)
}

pub unsafe extern "win64" fn RegCreateKeyExA(
    hkey: u64,
    lp_sub_key: *const u8,
    _reserved: u32,
    _lp_class: *const u8,
    _dw_options: u32,
    _sam_desired: u32,
    _lp_security_attributes: *const libc::c_void,
    phk_result: *mut u64,
    lpdw_disposition: *mut u32,
) -> i32 {
    if !lpdw_disposition.is_null() {
        *lpdw_disposition = 1;
    }
    RegOpenKeyExA(hkey, lp_sub_key, 0, 0, phk_result)
}

fn query_value_internal(full_key: &str) -> Option<RegValue> {
    let lower = full_key.to_lowercase();
    let store = get_registry_store();

    // 1. Direct match
    if let Some(val) = store.as_ref().and_then(|m| m.get(&lower)).cloned() {
        return Some(val);
    }

    // 2. Unity hash fallback (e.g. key stored with `_h123456789` or queried without hash)
    if let Some(val) = store.as_ref().and_then(|m| {
        let prefix = format!("{}_h", lower);
        m.iter().find(|(k, _)| k.starts_with(&prefix)).map(|(_, v)| v.clone())
    }) {
        return Some(val);
    }

    // 3. Unity defaults virtualization
    if lower.contains("screenmanager") && lower.contains("fullscreen mode") {
        Some(RegValue::Dword(1))
    } else if lower.contains("unitygraphicsquality") {
        Some(RegValue::Dword(5))
    } else if lower.contains("screenmanager resolution width") {
        Some(RegValue::Dword(1920))
    } else if lower.contains("screenmanager resolution height") {
        Some(RegValue::Dword(1080))
    } else if lower.contains("unityselectmonitor") {
        Some(RegValue::Dword(0))
    } else {
        None
    }
}

pub unsafe extern "win64" fn RegQueryValueExW(
    hkey: u64,
    lp_value_name: *const u16,
    _lp_reserved: *mut u32,
    lp_type: *mut u32,
    lp_data: *mut u8,
    lpcb_data: *mut u32,
) -> i32 {
    let val_name = if !lp_value_name.is_null() {
        let mut len = 0;
        while *lp_value_name.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(lp_value_name, len);
        String::from_utf16_lossy(slice)
    } else {
        String::new()
    };

    let prefix = get_parent_path(hkey);
    let full_key = combine_reg_path(&prefix, &val_name);
    println!("[ADVAPI32] RegQueryValueExW: hkey=0x{:X}, full_key='{}'", hkey, full_key);

    let maybe_val = query_value_internal(&full_key);

    if let Some(val) = maybe_val {
        if !lp_type.is_null() {
            *lp_type = val.dw_type();
        }
        let bytes = val.to_wide_bytes();
        if !lpcb_data.is_null() {
            let buf_len = *lpcb_data as usize;
            *lpcb_data = bytes.len() as u32;
            if !lp_data.is_null() {
                if buf_len < bytes.len() {
                    return 234; // ERROR_MORE_DATA
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), lp_data, bytes.len());
            }
        }
        0 // ERROR_SUCCESS
    } else {
        println!("[ADVAPI32] RegQueryValueExW: key not found '{}'", full_key);
        2 // ERROR_FILE_NOT_FOUND
    }
}

pub unsafe extern "win64" fn RegQueryValueExA(
    hkey: u64,
    lp_value_name: *const u8,
    _lp_reserved: *mut u32,
    lp_type: *mut u32,
    lp_data: *mut u8,
    lpcb_data: *mut u32,
) -> i32 {
    let val_name = if !lp_value_name.is_null() {
        let cstr = std::ffi::CStr::from_ptr(lp_value_name as *const libc::c_char);
        cstr.to_str().unwrap_or("").to_string()
    } else {
        String::new()
    };

    let prefix = get_parent_path(hkey);
    let full_key = combine_reg_path(&prefix, &val_name);
    println!("[ADVAPI32] RegQueryValueExA: hkey=0x{:X}, full_key='{}'", hkey, full_key);

    let maybe_val = query_value_internal(&full_key);

    if let Some(val) = maybe_val {
        if !lp_type.is_null() {
            *lp_type = val.dw_type();
        }
        let bytes = val.to_ansi_bytes();
        if !lpcb_data.is_null() {
            let buf_len = *lpcb_data as usize;
            *lpcb_data = bytes.len() as u32;
            if !lp_data.is_null() {
                if buf_len < bytes.len() {
                    return 234; // ERROR_MORE_DATA
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), lp_data, bytes.len());
            }
        }
        0 // ERROR_SUCCESS
    } else {
        println!("[ADVAPI32] RegQueryValueExA: key not found '{}'", full_key);
        2 // ERROR_FILE_NOT_FOUND
    }
}

pub unsafe extern "win64" fn RegSetValueExW(
    hkey: u64,
    lp_value_name: *const u16,
    _reserved: u32,
    dw_type: u32,
    lp_data: *const u8,
    cb_data: u32,
) -> i32 {
    let val_name = if !lp_value_name.is_null() {
        let mut len = 0;
        while *lp_value_name.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(lp_value_name, len);
        String::from_utf16_lossy(slice)
    } else {
        String::new()
    };

    let prefix = get_parent_path(hkey);
    let full_key = combine_reg_path(&prefix, &val_name);

    let val = if dw_type == 1 || dw_type == 2 {
        if !lp_data.is_null() && cb_data >= 2 {
            let u16_len = (cb_data as usize) / 2;
            let slice = std::slice::from_raw_parts(lp_data as *const u16, u16_len);
            let s = String::from_utf16_lossy(slice);
            RegValue::Sz(s.trim_end_matches('\0').to_string())
        } else {
            RegValue::Sz(String::new())
        }
    } else if dw_type == 4 {
        if !lp_data.is_null() && cb_data >= 4 {
            let mut arr = [0u8; 4];
            std::ptr::copy_nonoverlapping(lp_data, arr.as_mut_ptr(), 4);
            RegValue::Dword(u32::from_le_bytes(arr))
        } else {
            RegValue::Dword(0)
        }
    } else {
        let data_vec = if !lp_data.is_null() && cb_data > 0 {
            std::slice::from_raw_parts(lp_data, cb_data as usize).to_vec()
        } else {
            Vec::new()
        };
        RegValue::Binary(data_vec)
    };

    println!("[ADVAPI32] RegSetValueExW: full_key='{}', val={:?}", full_key, val);
    let mut store = REGISTRY_STORE.lock().unwrap();
    let map = store.get_or_insert_with(init_default_registry);
    map.insert(full_key, val);
    save_registry_store(map);
    0
}

pub unsafe extern "win64" fn RegSetValueExA(
    hkey: u64,
    lp_value_name: *const u8,
    _reserved: u32,
    dw_type: u32,
    lp_data: *const u8,
    cb_data: u32,
) -> i32 {
    let val_name = if !lp_value_name.is_null() {
        let cstr = std::ffi::CStr::from_ptr(lp_value_name as *const libc::c_char);
        cstr.to_str().unwrap_or("").to_string()
    } else {
        String::new()
    };

    let prefix = get_parent_path(hkey);
    let full_key = combine_reg_path(&prefix, &val_name);

    let val = if dw_type == 1 || dw_type == 2 {
        if !lp_data.is_null() && cb_data > 0 {
            let slice = std::slice::from_raw_parts(lp_data, cb_data as usize);
            let s = String::from_utf8_lossy(slice);
            RegValue::Sz(s.trim_end_matches('\0').to_string())
        } else {
            RegValue::Sz(String::new())
        }
    } else if dw_type == 4 {
        if !lp_data.is_null() && cb_data >= 4 {
            let mut arr = [0u8; 4];
            std::ptr::copy_nonoverlapping(lp_data, arr.as_mut_ptr(), 4);
            RegValue::Dword(u32::from_le_bytes(arr))
        } else {
            RegValue::Dword(0)
        }
    } else {
        let data_vec = if !lp_data.is_null() && cb_data > 0 {
            std::slice::from_raw_parts(lp_data, cb_data as usize).to_vec()
        } else {
            Vec::new()
        };
        RegValue::Binary(data_vec)
    };

    println!("[ADVAPI32] RegSetValueExA: full_key='{}', val={:?}", full_key, val);
    let mut store = REGISTRY_STORE.lock().unwrap();
    let map = store.get_or_insert_with(init_default_registry);
    map.insert(full_key, val);
    save_registry_store(map);
    0
}

pub unsafe extern "win64" fn RegCloseKey(hkey: u64) -> i32 {
    let mut hpaths = REGISTRY_HANDLE_PATHS.lock().unwrap();
    if let Some(map) = hpaths.as_mut() {
        map.remove(&hkey);
    }
    0
}

pub unsafe extern "win64" fn RegFlushKey(_hkey: u64) -> i32 {
    let store = REGISTRY_STORE.lock().unwrap();
    if let Some(map) = store.as_ref() {
        save_registry_store(map);
    }
    0
}

pub unsafe extern "win64" fn RegDeleteKeyW(hkey: u64, lp_sub_key: *const u16) -> i32 {
    let sub = if !lp_sub_key.is_null() {
        let mut len = 0;
        while *lp_sub_key.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(lp_sub_key, len);
        String::from_utf16_lossy(slice)
    } else {
        String::new()
    };
    let prefix = get_parent_path(hkey);
    let full_key = combine_reg_path(&prefix, &sub);
    let mut store = REGISTRY_STORE.lock().unwrap();
    if let Some(map) = store.as_mut() {
        map.retain(|k, _| !k.starts_with(&full_key));
        save_registry_store(map);
    }
    0
}

pub unsafe extern "win64" fn RegDeleteKeyA(hkey: u64, lp_sub_key: *const u8) -> i32 {
    let sub = if !lp_sub_key.is_null() {
        let cstr = std::ffi::CStr::from_ptr(lp_sub_key as *const libc::c_char);
        cstr.to_str().unwrap_or("").to_string()
    } else {
        String::new()
    };
    let prefix = get_parent_path(hkey);
    let full_key = combine_reg_path(&prefix, &sub);
    let mut store = REGISTRY_STORE.lock().unwrap();
    if let Some(map) = store.as_mut() {
        map.retain(|k, _| !k.starts_with(&full_key));
        save_registry_store(map);
    }
    0
}

pub unsafe extern "win64" fn RegDeleteValueW(hkey: u64, lp_value_name: *const u16) -> i32 {
    let val_name = if !lp_value_name.is_null() {
        let mut len = 0;
        while *lp_value_name.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(lp_value_name, len);
        String::from_utf16_lossy(slice)
    } else {
        String::new()
    };
    let prefix = get_parent_path(hkey);
    let full_key = combine_reg_path(&prefix, &val_name);
    let mut store = REGISTRY_STORE.lock().unwrap();
    if let Some(map) = store.as_mut() {
        map.remove(&full_key);
        save_registry_store(map);
    }
    0
}

pub unsafe extern "win64" fn RegDeleteValueA(hkey: u64, lp_value_name: *const u8) -> i32 {
    let val_name = if !lp_value_name.is_null() {
        let cstr = std::ffi::CStr::from_ptr(lp_value_name as *const libc::c_char);
        cstr.to_str().unwrap_or("").to_string()
    } else {
        String::new()
    };
    let prefix = get_parent_path(hkey);
    let full_key = combine_reg_path(&prefix, &val_name);
    let mut store = REGISTRY_STORE.lock().unwrap();
    if let Some(map) = store.as_mut() {
        map.remove(&full_key);
        save_registry_store(map);
    }
    0
}

pub unsafe extern "win64" fn RegGetValueW(
    hkey: u64,
    lp_sub_key: *const u16,
    lp_value: *const u16,
    _dw_flags: u32,
    pdw_type: *mut u32,
    pv_data: *mut u8,
    pcb_data: *mut u32,
) -> i32 {
    let mut sub_hkey = hkey;
    let mut opened = false;
    if !lp_sub_key.is_null() {
        let ret = RegOpenKeyExW(hkey, lp_sub_key, 0, 0, &mut sub_hkey);
        if ret != 0 {
            return ret;
        }
        opened = true;
    }
    let res = RegQueryValueExW(sub_hkey, lp_value, std::ptr::null_mut(), pdw_type, pv_data, pcb_data);
    if opened {
        RegCloseKey(sub_hkey);
    }
    res
}

pub unsafe extern "win64" fn RegGetValueA(
    hkey: u64,
    lp_sub_key: *const u8,
    lp_value: *const u8,
    _dw_flags: u32,
    pdw_type: *mut u32,
    pv_data: *mut u8,
    pcb_data: *mut u32,
) -> i32 {
    let mut sub_hkey = hkey;
    let mut opened = false;
    if !lp_sub_key.is_null() {
        let ret = RegOpenKeyExA(hkey, lp_sub_key, 0, 0, &mut sub_hkey);
        if ret != 0 {
            return ret;
        }
        opened = true;
    }
    let res = RegQueryValueExA(sub_hkey, lp_value, std::ptr::null_mut(), pdw_type, pv_data, pcb_data);
    if opened {
        RegCloseKey(sub_hkey);
    }
    res
}

pub unsafe extern "win64" fn RegEnumValueW(
    hkey: u64,
    dw_index: u32,
    lp_value_name: *mut u16,
    lpcch_value_name: *mut u32,
    _lp_reserved: *mut u32,
    lp_type: *mut u32,
    lp_data: *mut u8,
    lpcb_data: *mut u32,
) -> i32 {
    let prefix = get_parent_path(hkey);
    let store = get_registry_store();
    if let Some(map) = store.as_ref() {
        let prefix_slash = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}\\", prefix.trim_matches('\\'))
        };
        let mut matching: Vec<(&String, &RegValue)> = map
            .iter()
            .filter(|(k, _)| {
                if prefix_slash.is_empty() {
                    !k.contains('\\')
                } else if let Some(rest) = k.strip_prefix(&prefix_slash) {
                    !rest.contains('\\')
                } else {
                    false
                }
            })
            .collect();
        matching.sort_by_key(|(k, _)| *k);

        if let Some((k, val)) = matching.get(dw_index as usize) {
            let val_name = if prefix_slash.is_empty() {
                k.as_str()
            } else {
                k.strip_prefix(&prefix_slash).unwrap_or(k)
            };
            if !lp_value_name.is_null() && !lpcch_value_name.is_null() {
                let u16_chars: Vec<u16> = val_name.encode_utf16().collect();
                let max_len = *lpcch_value_name as usize;
                if max_len <= u16_chars.len() {
                    return 234; // ERROR_MORE_DATA
                }
                std::ptr::copy_nonoverlapping(u16_chars.as_ptr(), lp_value_name, u16_chars.len());
                *lp_value_name.add(u16_chars.len()) = 0;
                *lpcch_value_name = u16_chars.len() as u32;
            }
            if !lp_type.is_null() {
                *lp_type = val.dw_type();
            }
            if !lpcb_data.is_null() {
                let bytes = val.to_wide_bytes();
                let max_bytes = *lpcb_data as usize;
                if !lp_data.is_null() {
                    if max_bytes < bytes.len() {
                        return 234; // ERROR_MORE_DATA
                    }
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), lp_data, bytes.len());
                }
                *lpcb_data = bytes.len() as u32;
            }
            return 0; // ERROR_SUCCESS
        }
    }
    259 // ERROR_NO_MORE_ITEMS
}

pub unsafe extern "win64" fn RegEnumValueA(
    hkey: u64,
    dw_index: u32,
    lp_value_name: *mut u8,
    lpcch_value_name: *mut u32,
    _lp_reserved: *mut u32,
    lp_type: *mut u32,
    lp_data: *mut u8,
    lpcb_data: *mut u32,
) -> i32 {
    let prefix = get_parent_path(hkey);
    let store = get_registry_store();
    if let Some(map) = store.as_ref() {
        let prefix_slash = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}\\", prefix.trim_matches('\\'))
        };
        let mut matching: Vec<(&String, &RegValue)> = map
            .iter()
            .filter(|(k, _)| {
                if prefix_slash.is_empty() {
                    !k.contains('\\')
                } else if let Some(rest) = k.strip_prefix(&prefix_slash) {
                    !rest.contains('\\')
                } else {
                    false
                }
            })
            .collect();
        matching.sort_by_key(|(k, _)| *k);

        if let Some((k, val)) = matching.get(dw_index as usize) {
            let val_name = if prefix_slash.is_empty() {
                k.as_str()
            } else {
                k.strip_prefix(&prefix_slash).unwrap_or(k)
            };
            if !lp_value_name.is_null() && !lpcch_value_name.is_null() {
                let bytes: Vec<u8> = val_name.bytes().collect();
                let max_len = *lpcch_value_name as usize;
                if max_len <= bytes.len() {
                    return 234; // ERROR_MORE_DATA
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), lp_value_name, bytes.len());
                *lp_value_name.add(bytes.len()) = 0;
                *lpcch_value_name = bytes.len() as u32;
            }
            if !lp_type.is_null() {
                *lp_type = val.dw_type();
            }
            if !lpcb_data.is_null() {
                let bytes = val.to_ansi_bytes();
                let max_bytes = *lpcb_data as usize;
                if !lp_data.is_null() {
                    if max_bytes < bytes.len() {
                        return 234; // ERROR_MORE_DATA
                    }
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), lp_data, bytes.len());
                }
                *lpcb_data = bytes.len() as u32;
            }
            return 0; // ERROR_SUCCESS
        }
    }
    259 // ERROR_NO_MORE_ITEMS
}

pub unsafe extern "win64" fn RegNotifyChangeKeyValue(
    _hkey: u64,
    _b_watch_subtree: i32,
    _dw_notify_filter: u32,
    _h_event: u64,
    _f_asynchronous: i32,
) -> i32 {
    0 // ERROR_SUCCESS
}

pub unsafe extern "win64" fn RegQueryInfoKeyW(
    hkey: u64,
    _lp_class: *mut u16,
    _lpc_class: *mut u32,
    _lp_reserved: *mut u32,
    lpc_sub_keys: *mut u32,
    lpc_max_sub_key_len: *mut u32,
    _lpc_max_class_len: *mut u32,
    lpc_values: *mut u32,
    lpc_max_value_name_len: *mut u32,
    lpc_max_value_len: *mut u32,
    _lpcb_security_descriptor: *mut u32,
    _lpft_last_write_time: *mut libc::c_void,
) -> i32 {
    let prefix = get_parent_path(hkey);
    let store = get_registry_store();
    let mut num_values = 0u32;
    let mut max_val_name_len = 0u32;
    let mut max_val_len = 0u32;
    let mut num_sub_keys = 0u32;
    let mut max_sub_key_len = 0u32;

    if let Some(map) = store.as_ref() {
        let prefix_slash = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}\\", prefix.trim_matches('\\'))
        };

        for (k, val) in map.iter() {
            if prefix_slash.is_empty() {
                if !k.contains('\\') {
                    num_values += 1;
                    max_val_name_len = max_val_name_len.max(k.len() as u32);
                    max_val_len = max_val_len.max(val.to_wide_bytes().len() as u32);
                }
            } else if let Some(rest) = k.strip_prefix(&prefix_slash) {
                if !rest.contains('\\') {
                    num_values += 1;
                    max_val_name_len = max_val_name_len.max(rest.len() as u32);
                    max_val_len = max_val_len.max(val.to_wide_bytes().len() as u32);
                } else {
                    let subkey_name = rest.split('\\').next().unwrap_or("");
                    num_sub_keys += 1;
                    max_sub_key_len = max_sub_key_len.max(subkey_name.len() as u32);
                }
            }
        }
    }

    if !lpc_sub_keys.is_null() {
        *lpc_sub_keys = num_sub_keys;
    }
    if !lpc_max_sub_key_len.is_null() {
        *lpc_max_sub_key_len = max_sub_key_len;
    }
    if !lpc_values.is_null() {
        *lpc_values = num_values;
    }
    if !lpc_max_value_name_len.is_null() {
        *lpc_max_value_name_len = max_val_name_len;
    }
    if !lpc_max_value_len.is_null() {
        *lpc_max_value_len = max_val_len;
    }
    0
}

pub unsafe extern "win64" fn RegQueryInfoKeyA(
    hkey: u64,
    _lp_class: *mut u8,
    _lpc_class: *mut u32,
    _lp_reserved: *mut u32,
    lpc_sub_keys: *mut u32,
    lpc_max_sub_key_len: *mut u32,
    _lpc_max_class_len: *mut u32,
    lpc_values: *mut u32,
    lpc_max_value_name_len: *mut u32,
    lpc_max_value_len: *mut u32,
    _lpcb_security_descriptor: *mut u32,
    _lpft_last_write_time: *mut libc::c_void,
) -> i32 {
    let prefix = get_parent_path(hkey);
    let store = get_registry_store();
    let mut num_values = 0u32;
    let mut max_val_name_len = 0u32;
    let mut max_val_len = 0u32;
    let mut num_sub_keys = 0u32;
    let mut max_sub_key_len = 0u32;

    if let Some(map) = store.as_ref() {
        let prefix_slash = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}\\", prefix.trim_matches('\\'))
        };

        for (k, val) in map.iter() {
            if prefix_slash.is_empty() {
                if !k.contains('\\') {
                    num_values += 1;
                    max_val_name_len = max_val_name_len.max(k.len() as u32);
                    max_val_len = max_val_len.max(val.to_ansi_bytes().len() as u32);
                }
            } else if let Some(rest) = k.strip_prefix(&prefix_slash) {
                if !rest.contains('\\') {
                    num_values += 1;
                    max_val_name_len = max_val_name_len.max(rest.len() as u32);
                    max_val_len = max_val_len.max(val.to_ansi_bytes().len() as u32);
                } else {
                    let subkey_name = rest.split('\\').next().unwrap_or("");
                    num_sub_keys += 1;
                    max_sub_key_len = max_sub_key_len.max(subkey_name.len() as u32);
                }
            }
        }
    }

    if !lpc_sub_keys.is_null() {
        *lpc_sub_keys = num_sub_keys;
    }
    if !lpc_max_sub_key_len.is_null() {
        *lpc_max_sub_key_len = max_sub_key_len;
    }
    if !lpc_values.is_null() {
        *lpc_values = num_values;
    }
    if !lpc_max_value_name_len.is_null() {
        *lpc_max_value_name_len = max_val_name_len;
    }
    if !lpc_max_value_len.is_null() {
        *lpc_max_value_len = max_val_len;
    }
    0
}

pub unsafe extern "win64" fn BCryptGenRandom(
    h_algorithm: usize,
    pb_buffer: *mut u8,
    cb_buffer: u32,
    dw_flags: u32,
) -> i32 {
    if (dw_flags & 0x00000002) != 0 {
        if h_algorithm != 0 {
            return -1073741816; // STATUS_INVALID_HANDLE
        }
    } else if h_algorithm == 0 {
        return -1073741816;
    }

    if !pb_buffer.is_null() && cb_buffer > 0 {
        let slice = std::slice::from_raw_parts_mut(pb_buffer, cb_buffer as usize);
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            use std::io::Read;
            let _ = f.read_exact(slice);
        } else {
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
    if !token_handle.is_null() {
        *token_handle = 0x1000;
    }
    1 // TRUE
}

pub unsafe extern "win64" fn GetSidSubAuthority(_sid: usize, _sub_authority: u32) -> *mut u32 {
    static mut SUB_AUTH: u32 = 1000;
    unsafe { &raw mut SUB_AUTH }
}

pub unsafe extern "win64" fn GetSidSubAuthorityCount(_sid: usize) -> *mut u8 {
    static mut COUNT: u8 = 1;
    unsafe { &raw mut COUNT }
}

pub unsafe extern "win64" fn LookupPrivilegeValueW(
    _system_name: usize,
    _name: usize,
    luid: *mut u64,
) -> u32 {
    if !luid.is_null() {
        *luid = 0x1234;
    }
    1 // TRUE
}

pub unsafe extern "win64" fn SystemFunction036(pb_buf: *mut u8, len: u32) -> u8 {
    if !pb_buf.is_null() && len > 0 {
        let slice = std::slice::from_raw_parts_mut(pb_buf, len as usize);
        let ret = libc::syscall(libc::SYS_getrandom, pb_buf as *mut libc::c_void, len as usize, 0);
        if ret < 0 {
            if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
                use std::io::Read;
                let _ = f.read_exact(slice);
            }
        }
    }
    1 // TRUE
}

pub unsafe extern "win64" fn EventSetInformation(
    _reg_handle: u64,
    _info_class: u32,
    _info: *mut std::ffi::c_void,
    _len: u32,
) -> u32 {
    0 // ERROR_SUCCESS
}

pub unsafe extern "win64" fn AllocateLocallyUniqueId(luid: *mut u64) -> i32 {
    static NEXT_LUID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);
    if !luid.is_null() {
        *luid = NEXT_LUID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    1 // TRUE
}

pub unsafe extern "win64" fn InitializeSecurityDescriptor(
    p_security_descriptor: *mut std::ffi::c_void,
    _dw_revision: u32,
) -> i32 {
    if !p_security_descriptor.is_null() {
        std::ptr::write_bytes(p_security_descriptor as *mut u8, 0, 40);
    }
    1 // TRUE
}

pub unsafe extern "win64" fn SetSecurityDescriptorDacl(
    _p_security_descriptor: *mut std::ffi::c_void,
    _b_dacl_present: i32,
    _p_dacl: *mut std::ffi::c_void,
    _b_dacl_defaulted: i32,
) -> i32 {
    1 // TRUE
}

pub unsafe extern "win64" fn SetSecurityDescriptorOwner(
    _p_security_descriptor: *mut std::ffi::c_void,
    _p_owner: *mut std::ffi::c_void,
    _b_owner_defaulted: i32,
) -> i32 {
    1 // TRUE
}

pub unsafe extern "win64" fn SetSecurityDescriptorGroup(
    _p_security_descriptor: *mut std::ffi::c_void,
    _p_group: *mut std::ffi::c_void,
    _b_group_defaulted: i32,
) -> i32 {
    1 // TRUE
}

pub unsafe extern "win64" fn IsValidSecurityDescriptor(
    p_security_descriptor: *const std::ffi::c_void,
) -> i32 {
    if p_security_descriptor.is_null() { 0 } else { 1 }
}

pub unsafe extern "win64" fn GetSecurityDescriptorDacl(
    _p_security_descriptor: *const std::ffi::c_void,
    lpb_dacl_present: *mut i32,
    p_dacl: *mut *mut std::ffi::c_void,
    lpb_dacl_defaulted: *mut i32,
) -> i32 {
    if !lpb_dacl_present.is_null() { *lpb_dacl_present = 0; }
    if !p_dacl.is_null() { *p_dacl = std::ptr::null_mut(); }
    if !lpb_dacl_defaulted.is_null() { *lpb_dacl_defaulted = 0; }
    1 // TRUE
}

pub unsafe extern "win64" fn InitializeAcl(
    p_acl: *mut std::ffi::c_void,
    n_acl_length: u32,
    _dw_acl_revision: u32,
) -> i32 {
    if !p_acl.is_null() && n_acl_length >= 8 {
        std::ptr::write_bytes(p_acl as *mut u8, 0, n_acl_length as usize);
    }
    1 // TRUE
}

pub unsafe extern "win64" fn AddAccessAllowedAce(
    _p_acl: *mut std::ffi::c_void,
    _dw_ace_revision: u32,
    _access_mask: u32,
    _p_sid: *mut std::ffi::c_void,
) -> i32 {
    1 // TRUE
}

pub unsafe extern "win64" fn IsValidSid(p_sid: *const std::ffi::c_void) -> i32 {
    if p_sid.is_null() { 0 } else { 1 }
}

pub unsafe extern "win64" fn EqualSid(
    p_sid1: *const std::ffi::c_void,
    p_sid2: *const std::ffi::c_void,
) -> i32 {
    if p_sid1 == p_sid2 { 1 } else { 0 }
}

pub unsafe extern "win64" fn GetLengthSid(_p_sid: *const std::ffi::c_void) -> u32 {
    68 // standard SID buffer size
}

pub unsafe extern "win64" fn CopySid(
    n_destination_sid_length: u32,
    p_destination_sid: *mut std::ffi::c_void,
    p_source_sid: *const std::ffi::c_void,
) -> i32 {
    if !p_destination_sid.is_null() && !p_source_sid.is_null() && n_destination_sid_length >= 28 {
        std::ptr::copy_nonoverlapping(p_source_sid as *const u8, p_destination_sid as *mut u8, 28);
        1
    } else {
        1
    }
}

pub unsafe extern "win64" fn CreateWellKnownSid(
    _well_known_sid_type: u32,
    _domain_sid: *const std::ffi::c_void,
    p_sid: *mut std::ffi::c_void,
    cb_sid: *mut u32,
) -> i32 {
    if !cb_sid.is_null() {
        *cb_sid = 28;
    }
    if !p_sid.is_null() {
        std::ptr::write_bytes(p_sid as *mut u8, 1, 28);
    }
    1
}

pub unsafe extern "win64" fn CryptAcquireContextW(
    ph_prov: *mut usize,
    _psz_container: *const u16,
    _psz_provider: *const u16,
    _dw_prov_type: u32,
    _dw_flags: u32,
) -> i32 {
    if !ph_prov.is_null() {
        *ph_prov = 0x50001;
    }
    1
}

pub unsafe extern "win64" fn CryptAcquireContextA(
    ph_prov: *mut usize,
    _psz_container: *const u8,
    _psz_provider: *const u8,
    _dw_prov_type: u32,
    _dw_flags: u32,
) -> i32 {
    if !ph_prov.is_null() {
        *ph_prov = 0x50001;
    }
    1
}

pub unsafe extern "win64" fn CryptReleaseContext(_h_prov: usize, _dw_flags: u32) -> i32 {
    1
}

pub unsafe extern "win64" fn CryptGenRandom(
    _h_prov: usize,
    dw_len: u32,
    pb_buffer: *mut u8,
) -> i32 {
    if !pb_buffer.is_null() && dw_len > 0 {
        let slice = std::slice::from_raw_parts_mut(pb_buffer, dw_len as usize);
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            use std::io::Read;
            let _ = f.read_exact(slice);
        } else {
            for (i, b) in slice.iter_mut().enumerate() {
                *b = (i as u8).wrapping_add(42);
            }
        }
    }
    1
}

pub unsafe extern "win64" fn CryptCreateHash(
    _h_prov: usize,
    _alg_id: u32,
    _h_key: usize,
    _dw_flags: u32,
    ph_hash: *mut usize,
) -> i32 {
    if !ph_hash.is_null() {
        *ph_hash = 0x60001;
    }
    1
}

pub unsafe extern "win64" fn CryptHashData(
    _h_hash: usize,
    _pb_data: *const u8,
    _dw_data_len: u32,
    _dw_flags: u32,
) -> i32 {
    1
}

pub unsafe extern "win64" fn CryptGetHashParam(
    _h_hash: usize,
    _dw_param: u32,
    pb_data: *mut u8,
    pdw_data_len: *mut u32,
    _dw_flags: u32,
) -> i32 {
    if !pdw_data_len.is_null() {
        let max_len = *pdw_data_len;
        *pdw_data_len = 16;
        if !pb_data.is_null() && max_len >= 16 {
            std::ptr::write_bytes(pb_data, 0xAB, 16);
        }
    }
    1
}

pub unsafe extern "win64" fn CryptDestroyHash(_h_hash: usize) -> i32 {
    1
}

pub unsafe extern "win64" fn EventRegister(
    _provider_id: *const std::ffi::c_void,
    _enable_callback: usize,
    _callback_context: *mut std::ffi::c_void,
    reg_handle: *mut u64,
) -> u32 {
    if !reg_handle.is_null() {
        *reg_handle = 0x70001;
    }
    0 // ERROR_SUCCESS
}

pub unsafe extern "win64" fn EventUnregister(_reg_handle: u64) -> u32 {
    0 // ERROR_SUCCESS
}

pub unsafe extern "win64" fn EventWriteTransfer(
    _reg_handle: u64,
    _event_descriptor: *const std::ffi::c_void,
    _activity_id: *const std::ffi::c_void,
    _related_activity_id: *const std::ffi::c_void,
    _user_data_count: u32,
    _user_data: *const std::ffi::c_void,
) -> u32 {
    0 // ERROR_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_query() {
        unsafe {
            let key_u16: Vec<u16> =
                "software\\microsoft\\windows nt\\currentversion\\currentversion\0"
                    .encode_utf16()
                    .collect();
            let mut data = vec![0u8; 64];
            let mut size = 64u32;
            let mut val_type = 0u32;

            let status = RegQueryValueExW(
                HKEY_LOCAL_MACHINE,
                key_u16.as_ptr(),
                std::ptr::null_mut(),
                &mut val_type,
                data.as_mut_ptr(),
                &mut size,
            );

            assert_eq!(status, 0); // ERROR_SUCCESS
            assert_eq!(val_type, 1); // REG_SZ
        }
    }
}
