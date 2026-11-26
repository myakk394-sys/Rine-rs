//! Native Steamworks & Steam Bridge Implementation for Rine
//!
//! Provides high-performance, native Win64 ABI implementations of Steamworks API
//! functions (steam_api64.dll, steam_api.dll, steamclient64.dll, lsteamclient.dll).
//! Eliminates crashes caused by null vtables or missing Proton bridges, emulates
//! license verification checks (SteamAPI_RestartAppIfNecessary -> 0, BIsSubscribed -> 1),
//! and maintains a full mock C++ vtable table so virtual interface calls succeed safely.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::steam_scanner::{detect_unity_generation, UnityGeneration};

static DEFAULT_STEAM_ID: u64 = 76561198763491797;
static ACTIVE_APP_ID: AtomicU32 = AtomicU32::new(4001890);
static DETECTED_UNITY_GENERATION: AtomicU8 = AtomicU8::new(UnityGeneration::Unknown as u8);

pub fn set_unity_generation(gen: UnityGeneration) {
    DETECTED_UNITY_GENERATION.store(gen as u8, Ordering::SeqCst);
}

pub fn get_unity_generation() -> UnityGeneration {
    UnityGeneration::from_u8(DETECTED_UNITY_GENERATION.load(Ordering::SeqCst))
}

/// Inspect the target executable's Unity engine version and select the matching
/// Steamworks license/API compatibility profile (Unity 5/2017 legacy vs Unity 6+).
pub fn configure_unity_license_profile(exe_path: &Path) -> UnityGeneration {
    let (gen, version) = detect_unity_generation(exe_path);
    set_unity_generation(gen);
    let ver_label = version.as_deref().unwrap_or("unknown");
    let profile_desc = match gen {
        UnityGeneration::Unity5OrLegacy => "Legacy Unity 4/5/2017 (CSteamworks / classic _Get interfaces)",
        UnityGeneration::Unity2018To2022 => "Unity 2018-2022 (hybrid classic + ManualDispatch)",
        UnityGeneration::Unity6Plus => "Unity 6+ (strict ManualDispatch / modern Steamworks.NET)",
        UnityGeneration::Unknown => "Generic / non-Unity (default compatibility)",
    };
    println!(
        "[RINE STEAM] Engine profile: version='{}' -> {}",
        ver_label, profile_desc
    );
    gen
}

pub fn set_active_app_id(appid: u32) {
    ACTIVE_APP_ID.store(appid, Ordering::SeqCst);
}

pub fn get_active_app_id() -> u32 {
    if let Ok(val) = std::env::var("SteamAppId") {
        if let Ok(id) = val.parse::<u32>() {
            return id;
        }
    }
    ACTIVE_APP_ID.load(Ordering::SeqCst)
}

// Mock C++ interface and vtable infrastructure
// Инфраструктура mock-объектов и виртуальных таблиц C++ для Steamworks

#[repr(C)]
pub struct MockSteamInterface {
    pub vtable: *const usize,
}

static mut MOCK_VTABLE: [usize; 256] = [0; 256];
static mut MOCK_STEAM_INTERFACE: MockSteamInterface = MockSteamInterface {
    vtable: std::ptr::null(),
};
static VTABLE_INIT: OnceLock<()> = OnceLock::new();

// Client interface
static mut MOCK_CLIENT_VTABLE: [usize; 256] = [0; 256];
static mut MOCK_CLIENT_INTERFACE: MockSteamInterface = MockSteamInterface {
    vtable: std::ptr::null(),
};
static CLIENT_VTABLE_INIT: OnceLock<()> = OnceLock::new();

// User interface
static mut MOCK_USER_VTABLE: [usize; 256] = [0; 256];
static mut MOCK_USER_INTERFACE: MockSteamInterface = MockSteamInterface {
    vtable: std::ptr::null(),
};
static USER_VTABLE_INIT: OnceLock<()> = OnceLock::new();

// Utils interface
static mut MOCK_UTILS_VTABLE: [usize; 256] = [0; 256];
static mut MOCK_UTILS_INTERFACE: MockSteamInterface = MockSteamInterface {
    vtable: std::ptr::null(),
};
static UTILS_VTABLE_INIT: OnceLock<()> = OnceLock::new();

// Friends interface
static mut MOCK_FRIENDS_VTABLE: [usize; 256] = [0; 256];
static mut MOCK_FRIENDS_INTERFACE: MockSteamInterface = MockSteamInterface {
    vtable: std::ptr::null(),
};
static FRIENDS_VTABLE_INIT: OnceLock<()> = OnceLock::new();

// Apps interface
static mut MOCK_APPS_VTABLE: [usize; 256] = [0; 256];
static mut MOCK_APPS_INTERFACE: MockSteamInterface = MockSteamInterface {
    vtable: std::ptr::null(),
};
static APPS_VTABLE_INIT: OnceLock<()> = OnceLock::new();

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_generic_stub(
    _rcx: usize,
    _rdx: usize,
    _r8: usize,
    _r9: usize,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_zero_stub(
    _rcx: usize,
    _rdx: usize,
    _r8: usize,
    _r9: usize,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_return_one_stub(
    _rcx: usize,
    _rdx: usize,
    _r8: usize,
    _r9: usize,
) -> u64 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_return_interface_stub(
    _rcx: usize,
    _rdx: usize,
    _r8: usize,
    _r9: usize,
) -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_query_interface(
    _this: *mut c_void,
    _riid: *const c_void,
    ppv_object: *mut *mut c_void,
) -> i32 {
    let p = ppv_object as usize;
    if p > 0x10000 && p < 0x7FFFFFFFFFFF {
        *ppv_object = &raw mut MOCK_STEAM_INTERFACE as *mut c_void;
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_add_ref(_this: *mut c_void) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_release(_this: *mut c_void) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_get_current_beta_name(
    _this: *mut c_void,
    pch_name: *mut c_char,
    cch_name_buffer_size: i32,
) -> u32 {
    let p = pch_name as usize;
    if p > 0x10000 && p < 0x7FFFFFFFFFFF && cch_name_buffer_size > 0 {
        *pch_name = 0; // null-terminated empty string: public release branch
    }
    0 // false = not on a beta branch
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_get_current_game_language(
    _this: *mut c_void,
) -> *const c_char {
    b"english\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_get_available_game_languages(
    _this: *mut c_void,
) -> *const c_char {
    b"english,russian\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_get_steam_id(_this: *mut c_void) -> u64 {
    DEFAULT_STEAM_ID
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_get_app_id(_this: *mut c_void) -> u32 {
    get_active_app_id()
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_get_persona_name(_this: *mut c_void) -> *const c_char {
    b"Player\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_get_ip_country(_this: *mut c_void) -> *const c_char {
    b"US\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn mock_vtable_get_steam_ui_language(_this: *mut c_void) -> *const c_char {
    b"english\0".as_ptr() as *const c_char
}

pub fn get_mock_steam_interface() -> *mut c_void {
    VTABLE_INIT.get_or_init(|| unsafe {
        for slot in &mut MOCK_VTABLE {
            *slot = mock_vtable_return_interface_stub as usize;
        }

        MOCK_VTABLE[0] = mock_vtable_query_interface as usize;
        MOCK_VTABLE[1] = mock_vtable_add_ref as usize;
        MOCK_VTABLE[2] = mock_vtable_release as usize;
        MOCK_VTABLE[3] = mock_vtable_zero_stub as usize;
        MOCK_VTABLE[4] = mock_vtable_get_current_game_language as usize;
        MOCK_VTABLE[5] = mock_vtable_get_available_game_languages as usize;
        MOCK_VTABLE[15] = mock_vtable_get_current_beta_name as usize;

        MOCK_STEAM_INTERFACE.vtable = MOCK_VTABLE.as_ptr();
    });

    unsafe { &raw mut MOCK_STEAM_INTERFACE as *mut c_void }
}

pub fn get_mock_client_interface() -> *mut c_void {
    CLIENT_VTABLE_INIT.get_or_init(|| unsafe {
        for slot in &mut MOCK_CLIENT_VTABLE {
            *slot = mock_vtable_return_interface_stub as usize;
        }
        MOCK_CLIENT_VTABLE[0] = mock_vtable_return_one_stub as usize; // CreateSteamPipe -> 1
        MOCK_CLIENT_VTABLE[1] = mock_vtable_return_one_stub as usize; // BReleaseSteamPipe -> 1
        MOCK_CLIENT_VTABLE[2] = mock_vtable_return_one_stub as usize; // ConnectToGlobalUser -> 1
        MOCK_CLIENT_VTABLE[3] = mock_vtable_return_one_stub as usize; // CreateLocalUser -> 1
        MOCK_CLIENT_VTABLE[4] = mock_vtable_zero_stub as usize;       // ReleaseUser
        MOCK_CLIENT_VTABLE[5] = get_mock_user_interface as usize;     // GetISteamUser
        MOCK_CLIENT_VTABLE[8] = get_mock_friends_interface as usize;  // GetISteamFriends
        MOCK_CLIENT_VTABLE[9] = get_mock_utils_interface as usize;    // GetISteamUtils
        MOCK_CLIENT_VTABLE[13] = get_mock_userstats_interface as usize; // GetISteamUserStats
        MOCK_CLIENT_VTABLE[15] = get_mock_apps_interface as usize;    // GetISteamApps
        MOCK_CLIENT_VTABLE[17] = get_mock_remotestorage_interface as usize; // GetISteamRemoteStorage
        MOCK_CLIENT_VTABLE[33] = mock_vtable_return_one_stub as usize; // BShutdownIfAllPipesClosed -> 1

        MOCK_CLIENT_INTERFACE.vtable = MOCK_CLIENT_VTABLE.as_ptr();
    });
    unsafe { &raw mut MOCK_CLIENT_INTERFACE as *mut c_void }
}

pub fn get_mock_user_interface() -> *mut c_void {
    USER_VTABLE_INIT.get_or_init(|| unsafe {
        for slot in &mut MOCK_USER_VTABLE {
            *slot = mock_vtable_zero_stub as usize;
        }
        MOCK_USER_VTABLE[0] = mock_vtable_return_one_stub as usize; // GetHSteamUser -> 1
        MOCK_USER_VTABLE[1] = mock_vtable_return_one_stub as usize; // BLoggedOn -> 1
        MOCK_USER_VTABLE[2] = mock_vtable_get_steam_id as usize;    // GetSteamID
        MOCK_USER_VTABLE[6] = mock_vtable_return_one_stub as usize; // GetUserDataFolder
        MOCK_USER_VTABLE[7] = mock_vtable_return_one_stub as usize; // UserHasLicenseForApp
        MOCK_USER_VTABLE[9] = mock_vtable_return_one_stub as usize; // GetAuthSessionTicket
        MOCK_USER_VTABLE[14] = mock_vtable_return_one_stub as usize; // BIsPhoneVerified
        MOCK_USER_VTABLE[18] = mock_vtable_return_one_stub as usize; // GetPlayerSteamLevel

        MOCK_USER_INTERFACE.vtable = MOCK_USER_VTABLE.as_ptr();
    });
    unsafe { &raw mut MOCK_USER_INTERFACE as *mut c_void }
}

pub fn get_mock_utils_interface() -> *mut c_void {
    UTILS_VTABLE_INIT.get_or_init(|| unsafe {
        for slot in &mut MOCK_UTILS_VTABLE {
            *slot = mock_vtable_zero_stub as usize;
        }
        MOCK_UTILS_VTABLE[0] = mock_vtable_return_one_stub as usize; // GetSecondsSinceAppActive
        MOCK_UTILS_VTABLE[1] = mock_vtable_return_one_stub as usize; // GetSecondsSinceComputerActive
        MOCK_UTILS_VTABLE[2] = mock_vtable_return_one_stub as usize; // GetConnectedUniverse
        MOCK_UTILS_VTABLE[4] = mock_vtable_get_ip_country as usize;  // GetIPCountry
        MOCK_UTILS_VTABLE[8] = mock_vtable_return_one_stub as usize; // GetCurrentBatteryPower
        MOCK_UTILS_VTABLE[9] = mock_vtable_get_app_id as usize;      // GetAppID
        MOCK_UTILS_VTABLE[11] = mock_vtable_return_one_stub as usize; // IsAPICallCompleted
        MOCK_UTILS_VTABLE[13] = mock_vtable_return_one_stub as usize; // GetAPICallResult
        MOCK_UTILS_VTABLE[15] = mock_vtable_return_one_stub as usize; // IsOverlayEnabled
        MOCK_UTILS_VTABLE[21] = mock_vtable_get_steam_ui_language as usize; // GetSteamUILanguage

        MOCK_UTILS_INTERFACE.vtable = MOCK_UTILS_VTABLE.as_ptr();
    });
    unsafe { &raw mut MOCK_UTILS_INTERFACE as *mut c_void }
}

pub fn get_mock_friends_interface() -> *mut c_void {
    FRIENDS_VTABLE_INIT.get_or_init(|| unsafe {
        for slot in &mut MOCK_FRIENDS_VTABLE {
            *slot = mock_vtable_zero_stub as usize;
        }
        MOCK_FRIENDS_VTABLE[0] = mock_vtable_get_persona_name as usize; // GetPersonaName
        MOCK_FRIENDS_VTABLE[1] = mock_vtable_return_one_stub as usize;  // SetPersonaName
        MOCK_FRIENDS_VTABLE[2] = mock_vtable_return_one_stub as usize;  // GetPersonaState (1 = Online)
        MOCK_FRIENDS_VTABLE[5] = mock_vtable_return_one_stub as usize;  // GetFriendRelationship
        MOCK_FRIENDS_VTABLE[9] = mock_vtable_return_one_stub as usize;  // SetRichPresence

        MOCK_FRIENDS_INTERFACE.vtable = MOCK_FRIENDS_VTABLE.as_ptr();
    });
    unsafe { &raw mut MOCK_FRIENDS_INTERFACE as *mut c_void }
}

pub fn get_mock_apps_interface() -> *mut c_void {
    APPS_VTABLE_INIT.get_or_init(|| unsafe {
        for slot in &mut MOCK_APPS_VTABLE {
            *slot = mock_vtable_zero_stub as usize;
        }
        MOCK_APPS_VTABLE[0] = mock_vtable_return_one_stub as usize; // BIsSubscribed
        MOCK_APPS_VTABLE[4] = mock_vtable_get_current_game_language as usize;
        MOCK_APPS_VTABLE[5] = mock_vtable_get_available_game_languages as usize;
        MOCK_APPS_VTABLE[6] = mock_vtable_return_one_stub as usize; // BIsSubscribedApp
        MOCK_APPS_VTABLE[7] = mock_vtable_return_one_stub as usize; // BIsDlcInstalled
        MOCK_APPS_VTABLE[15] = mock_vtable_get_current_beta_name as usize;

        MOCK_APPS_INTERFACE.vtable = MOCK_APPS_VTABLE.as_ptr();
    });
    unsafe { &raw mut MOCK_APPS_INTERFACE as *mut c_void }
}

pub fn get_mock_userstats_interface() -> *mut c_void {
    get_mock_steam_interface()
}

pub fn get_mock_remotestorage_interface() -> *mut c_void {
    get_mock_steam_interface()
}

// Core SteamAPI lifecycle and license validation functions
// Базовые функции жизненного цикла и проверки лицензий SteamAPI

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_Init() -> u8 {
    println!("[RINE STEAM] SteamAPI_Init() -> SUCCESS (1)");
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_InitFlat(p_out_err_msg: *mut u8) -> u32 {
    println!("[RINE STEAM] SteamAPI_InitFlat() -> SUCCESS (0)");
    if !p_out_err_msg.is_null() {
        *p_out_err_msg = 0;
    }
    0 // 0 = k_ESteamAPIInitResult_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SteamInternal_SteamAPI_Init(p_out_err_msg: *mut u8) -> u32 {
    println!("[RINE STEAM] SteamInternal_SteamAPI_Init() -> SUCCESS (0 = k_ESteamAPIInitResult_OK)");
    if !p_out_err_msg.is_null() {
        *p_out_err_msg = 0;
    }
    0 // 0 = k_ESteamAPIInitResult_OK
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_Shutdown() {
    println!("[RINE STEAM] SteamAPI_Shutdown()");
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_RestartAppIfNecessary(un_own_app_id: u32) -> u8 {
    println!(
        "[RINE STEAM] SteamAPI_RestartAppIfNecessary(appId={}) -> 0 (License Validated)",
        un_own_app_id
    );
    if un_own_app_id != 0 {
        set_active_app_id(un_own_app_id);
    }
    0 // 0 = false: do NOT restart via Steam, game is legitimately authorized to run
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_IsSteamRunning() -> u8 {
    1 // Steam is actively running
}

static REGISTERED_CALLBACKS: std::sync::Mutex<Vec<(usize, i32, bool)>> =
    std::sync::Mutex::new(Vec::new());

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SteamRelayNetworkStatusDetails {
    pub m_e_avail: i32,
    pub m_b_ping_measurement_in_progress: i32,
    pub m_e_avail_network_config: i32,
    pub m_e_avail_any_relay: i32,
    pub m_debug_msg: [u8; 256],
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_RunCallbacks() {
    let to_run: Vec<(usize, i32)> = {
        let mut guard = REGISTERED_CALLBACKS.lock().unwrap();
        let mut items = Vec::new();
        for item in guard.iter_mut() {
            if !item.2 {
                item.2 = true;
                items.push((item.0, item.1));
            }
        }
        items
    };

    for (cb_addr, i_cb) in to_run {
        // NEVER dispatch failure or disconnect callbacks!
        if i_cb == 102 || i_cb == 103 || i_cb == 104 || i_cb == 143 {
            continue;
        }

        let p_cb = cb_addr as *mut c_void;
        if cb_addr > 0x10000 && cb_addr < 0x7FFFFFFFFFFF {
            let vtable = *(p_cb as *const *const usize);
            if !vtable.is_null() && (vtable as usize) > 0x10000 {
                let run_ptr = *vtable;
                if run_ptr > 0x10000 && run_ptr < 0x7FFFFFFFFFFF {
                    let run_fn: unsafe extern "win64" fn(*mut c_void, *const c_void) =
                        std::mem::transmute(run_ptr);

                    match i_cb {
                        101 => {
                            // SteamServersConnected_t
                            println!("[RINE STEAM] Dispatching SteamServersConnected_t (101) to {:p}", p_cb);
                            let dummy = [0u8; 16];
                            run_fn(p_cb, dummy.as_ptr() as *const c_void);
                        }
                        125 => {
                            // LicensesUpdated_t
                            println!("[RINE STEAM] Dispatching LicensesUpdated_t (125) to {:p}", p_cb);
                            let dummy = [0u8; 16];
                            run_fn(p_cb, dummy.as_ptr() as *const c_void);
                        }
                        154 => {
                            // ValidateAuthTicketResponse_t
                            println!("[RINE STEAM] Dispatching ValidateAuthTicketResponse_t (154) to {:p}", p_cb);
                            #[repr(C)]
                            struct ValidateAuthTicketResponse {
                                steam_id: u64,
                                auth_session_response: u32,
                                owner_steam_id: u64,
                            }
                            let resp = ValidateAuthTicketResponse {
                                steam_id: DEFAULT_STEAM_ID,
                                auth_session_response: 0, // k_EAuthSessionResponseOK
                                owner_steam_id: DEFAULT_STEAM_ID,
                            };
                            run_fn(p_cb, &resp as *const _ as *const c_void);
                        }
                        304 => {
                            // PersonaStateChange_t
                            println!("[RINE STEAM] Dispatching PersonaStateChange_t (304) to {:p}", p_cb);
                            #[repr(C)]
                            struct PersonaStateChange {
                                steam_id: u64,
                                change_flags: i32,
                            }
                            let change = PersonaStateChange {
                                steam_id: DEFAULT_STEAM_ID,
                                change_flags: 0x0001 | 0x0002 | 0x0004,
                            };
                            run_fn(p_cb, &change as *const _ as *const c_void);
                        }
                        1101 => {
                            // UserStatsReceived_t
                            println!("[RINE STEAM] Dispatching UserStatsReceived_t (1101) to {:p}", p_cb);
                            #[repr(C)]
                            struct UserStatsReceived {
                                game_id: u64,
                                result: u32,
                                steam_id: u64,
                            }
                            let stats = UserStatsReceived {
                                game_id: get_active_app_id() as u64,
                                result: 1, // k_EResultOK
                                steam_id: DEFAULT_STEAM_ID,
                            };
                            run_fn(p_cb, &stats as *const _ as *const c_void);
                        }
                        1281 => {
                            // SteamRelayNetworkStatus_t
                            println!("[RINE STEAM] Dispatching SteamRelayNetworkStatus_t (1281) to {:p}", p_cb);
                            let mut status = SteamRelayNetworkStatusDetails {
                                m_e_avail: 100, // k_ESteamNetworkingAvailability_Current
                                m_b_ping_measurement_in_progress: 0,
                                m_e_avail_network_config: 100,
                                m_e_avail_any_relay: 100,
                                m_debug_msg: [0u8; 256],
                            };
                            let msg = b"OK\0";
                            status.m_debug_msg[..msg.len()].copy_from_slice(msg);
                            run_fn(p_cb, &status as *const _ as *const c_void);
                        }
                        _ => {
                            println!("[RINE STEAM] Dispatching generic callback ({}) to {:p}", i_cb, p_cb);
                            let dummy = [0u8; 64];
                            run_fn(p_cb, dummy.as_ptr() as *const c_void);
                        }
                    }
                }
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ReleaseCurrentThreadMemory() {}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_GetHSteamPipe() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_GetHSteamUser() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_GetSteamInstallPath() -> *const c_char {
    b"C:\\Program Files (x86)\\Steam\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_RegisterCallback(
    p_callback: *mut c_void,
    i_callback: i32,
) {
    println!(
        "[RINE STEAM] SteamAPI_RegisterCallback(pCallback={:p}, iCallback={})",
        p_callback, i_callback
    );
    if !p_callback.is_null() && (p_callback as usize) > 0x10000 {
        let mut guard = REGISTERED_CALLBACKS.lock().unwrap();
        guard.push((p_callback as usize, i_callback, false));
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_UnregisterCallback(p_callback: *mut c_void) {
    if !p_callback.is_null() {
        let mut guard = REGISTERED_CALLBACKS.lock().unwrap();
        guard.retain(|item| item.0 != (p_callback as usize));
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_RegisterCallResult(
    _p_callback: *mut c_void,
    _h_api_call: u64,
) {
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_UnregisterCallResult(
    _p_callback: *mut c_void,
    _h_api_call: u64,
) {
}

// Interface pointers and C++ accessors
// Указатели интерфейсов и функции доступа C++

#[no_mangle]
pub unsafe extern "win64" fn SteamApps() -> *mut c_void {
    get_mock_apps_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamUser() -> *mut c_void {
    get_mock_user_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamUtils() -> *mut c_void {
    get_mock_utils_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamUserStats() -> *mut c_void {
    get_mock_userstats_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamFriends() -> *mut c_void {
    get_mock_friends_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamClient() -> *mut c_void {
    get_mock_client_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamScreenshots() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamRemoteStorage() -> *mut c_void {
    get_mock_remotestorage_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamHTTP() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamUGC() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamInput() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamController() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamNetworking() -> *mut c_void {
    get_mock_steam_interface()
}

#[repr(C)]
pub struct CallbackMsgT {
    pub m_h_steam_user: i32,
    pub m_i_callback: i32,
    pub m_pub_param: *mut u8,
    pub m_cub_param: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SteamAPICallCompletedT {
    m_h_async_call: u64,
    m_i_callback: i32,
    m_cub_param: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LobbyCreatedT {
    m_e_result: u32,
    m_pad: u32,
    m_ul_steam_id_lobby: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LobbyEnterT {
    m_ul_steam_id_lobby: u64,
    m_rgf_chat_permissions: u32,
    m_b_locked: u8,
    m_pad: [u8; 3],
    m_e_chat_room_enter_response: u32,
}

#[derive(Clone)]
struct LocalLobby {
    owner: u64,
    lobby_type: i32,
    max_members: i32,
    joinable: bool,
    data: HashMap<String, String>,
}

static NEXT_API_CALL_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1001);
static PENDING_CALLBACKS: OnceLock<Mutex<std::collections::VecDeque<(i32, Vec<u8>)>>> =
    OnceLock::new();
static PENDING_CALL_RESULTS: OnceLock<Mutex<HashMap<u64, (i32, Vec<u8>, bool)>>> = OnceLock::new();
static ACTIVE_MANUAL_CALLBACK_BUF: OnceLock<Mutex<Option<Vec<u8>>>> = OnceLock::new();
static LOCAL_LOBBIES: OnceLock<Mutex<HashMap<u64, LocalLobby>>> = OnceLock::new();
static LOBBY_DATA_CSTR_CACHE: OnceLock<Mutex<HashMap<(u64, String), std::ffi::CString>>> =
    OnceLock::new();

fn pending_callbacks() -> &'static Mutex<std::collections::VecDeque<(i32, Vec<u8>)>> {
    PENDING_CALLBACKS.get_or_init(|| Mutex::new(std::collections::VecDeque::new()))
}

fn pending_call_results() -> &'static Mutex<HashMap<u64, (i32, Vec<u8>, bool)>> {
    PENDING_CALL_RESULTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn active_manual_cb_buf() -> &'static Mutex<Option<Vec<u8>>> {
    ACTIVE_MANUAL_CALLBACK_BUF.get_or_init(|| Mutex::new(None))
}

fn local_lobbies() -> &'static Mutex<HashMap<u64, LocalLobby>> {
    LOCAL_LOBBIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lobby_cstr_cache() -> &'static Mutex<HashMap<(u64, String), std::ffi::CString>> {
    LOBBY_DATA_CSTR_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe fn struct_to_bytes<T: Copy>(val: &T) -> Vec<u8> {
    std::slice::from_raw_parts((val as *const T) as *const u8, std::mem::size_of::<T>()).to_vec()
}

unsafe fn enqueue_api_call_result<T: Copy>(i_callback: i32, payload: &T) -> u64 {
    let h_call = NEXT_API_CALL_ID.fetch_add(1, Ordering::SeqCst);
    let payload_bytes = struct_to_bytes(payload);
    let cub_param = payload_bytes.len() as u32;
    pending_call_results()
        .lock()
        .unwrap()
        .insert(h_call, (i_callback, payload_bytes, false));
    let completed = SteamAPICallCompletedT {
        m_h_async_call: h_call,
        m_i_callback: i_callback,
        m_cub_param: cub_param,
    };
    pending_callbacks()
        .lock()
        .unwrap()
        .push_back((703, struct_to_bytes(&completed))); // SteamAPICallCompleted_t = 703
    h_call
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ManualDispatch_Init() {
    println!("[RINE STEAM] ManualDispatch_Init()");
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ManualDispatch_RunFrame(_h_steam_pipe: i32) {}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ManualDispatch_GetNextCallback(
    h_steam_pipe: i32,
    p_callback_msg: *mut CallbackMsgT,
) -> u32 {
    if p_callback_msg.is_null() {
        return 0;
    }
    if let Some((i_cb, payload)) = pending_callbacks().lock().unwrap().pop_front() {
        let len = payload.len() as i32;
        let mut active = active_manual_cb_buf().lock().unwrap();
        *active = Some(payload);
        let ptr = active.as_mut().unwrap().as_mut_ptr();
        (*p_callback_msg).m_h_steam_user = 1;
        (*p_callback_msg).m_i_callback = i_cb;
        (*p_callback_msg).m_pub_param = ptr;
        (*p_callback_msg).m_cub_param = len;
        println!(
            "[RINE STEAM] ManualDispatch_GetNextCallback -> i_cb={}, cub={}",
            i_cb, len
        );
        return 1;
    }
    if crate::dlls::steam_bridge::is_bridge_enabled() {
        return crate::dlls::steam_bridge::steamclient64_Steam_BGetCallback(
            h_steam_pipe,
            p_callback_msg as *mut c_void,
        ) as u32;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ManualDispatch_FreeLastCallback(h_steam_pipe: i32) {
    let mut active = active_manual_cb_buf().lock().unwrap();
    if active.is_some() {
        *active = None;
        return;
    }
    drop(active);
    if crate::dlls::steam_bridge::is_bridge_enabled() {
        crate::dlls::steam_bridge::steamclient64_Steam_FreeLastCallback(h_steam_pipe);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ManualDispatch_GetAPICallResult(
    h_steam_pipe: i32,
    h_steam_api_call: u64,
    p_callback: *mut c_void,
    cub_callback: i32,
    i_callback_expected: i32,
    pb_failed: *mut u8,
) -> u32 {
    if let Some((i_cb, payload, failed)) = pending_call_results().lock().unwrap().remove(&h_steam_api_call) {
        println!(
            "[RINE STEAM] GetAPICallResult(call={}, expected={}, actual={}, cub={}, payload_len={}, failed={})",
            h_steam_api_call, i_callback_expected, i_cb, cub_callback, payload.len(), failed
        );
        if i_callback_expected != 0 && i_cb != i_callback_expected {
            if !pb_failed.is_null() {
                // C# Steamworks.NET marshals `out bool pbFailed` as 4-byte Win32 BOOL
                std::ptr::write_unaligned(pb_failed as *mut u32, 1);
            }
            return 0;
        }
        if !p_callback.is_null() && cub_callback > 0 {
            let copy_len = payload.len().min(cub_callback as usize);
            std::ptr::copy_nonoverlapping(payload.as_ptr(), p_callback as *mut u8, copy_len);
        }
        if !pb_failed.is_null() {
            std::ptr::write_unaligned(pb_failed as *mut u32, if failed { 1 } else { 0 });
        }
        return 1;
    }
    if crate::dlls::steam_bridge::is_bridge_enabled() {
        return crate::dlls::steam_bridge::steamclient64_Steam_GetAPICallResult(
            h_steam_pipe,
            h_steam_api_call,
            p_callback,
            cub_callback,
            i_callback_expected,
            pb_failed,
        ) as u32;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_CreateLobby(
    _this: *mut c_void,
    e_lobby_type: i32,
    c_max_members: i32,
) -> u64 {
    let owner = SteamAPI_ISteamUser_GetSteamID(std::ptr::null_mut());
    let seq = NEXT_API_CALL_ID.load(Ordering::SeqCst);
    // Universe=1 (Public), AccountType=8 (Chat), Instance=0xC0000 (Lobby | MMSLobby)
    let lobby_id = 0x018C_0000_0000_0000u64 | (seq & 0xFFFF_FFFF);
    local_lobbies().lock().unwrap().insert(
        lobby_id,
        LocalLobby {
            owner,
            lobby_type: e_lobby_type,
            max_members: c_max_members.max(1),
            joinable: true,
            data: HashMap::new(),
        },
    );
    let created = LobbyCreatedT {
        m_e_result: 1, // k_EResultOK
        m_pad: 0,
        m_ul_steam_id_lobby: lobby_id,
    };
    let h_call = enqueue_api_call_result(513, &created); // LobbyCreated_t = 513
    let entered = LobbyEnterT {
        m_ul_steam_id_lobby: lobby_id,
        m_rgf_chat_permissions: 0,
        m_b_locked: 0,
        m_pad: [0; 3],
        m_e_chat_room_enter_response: 1, // k_EChatRoomEnterResponseSuccess
    };
    {
        let mut q = pending_callbacks().lock().unwrap();
        q.push_back((513, struct_to_bytes(&created))); // LobbyCreated_t = 513 (for Callback<LobbyCreated_t>)
        q.push_back((504, struct_to_bytes(&entered))); // LobbyEnter_t = 504
    }
    println!(
        "[RINE STEAM] CreateLobby(type={}, max={}) -> lobby=0x{:016X}, call={}",
        e_lobby_type, c_max_members, lobby_id, h_call
    );
    h_call
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_JoinLobby(
    _this: *mut c_void,
    steam_id_lobby: u64,
) -> u64 {
    let entered = LobbyEnterT {
        m_ul_steam_id_lobby: steam_id_lobby,
        m_rgf_chat_permissions: 0,
        m_b_locked: 0,
        m_pad: [0; 3],
        m_e_chat_room_enter_response: 1,
    };
    let h_call = enqueue_api_call_result(504, &entered);
    pending_callbacks()
        .lock()
        .unwrap()
        .push_back((504, struct_to_bytes(&entered)));
    h_call
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_LeaveLobby(
    _this: *mut c_void,
    steam_id_lobby: u64,
) {
    println!("[RINE STEAM] LeaveLobby(0x{:016X})", steam_id_lobby);
    local_lobbies().lock().unwrap().remove(&steam_id_lobby);
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_SetLobbyData(
    _this: *mut c_void,
    steam_id_lobby: u64,
    pch_key: *const c_char,
    pch_value: *const c_char,
) -> u8 {
    if pch_key.is_null() || pch_value.is_null() {
        return 0;
    }
    let key = CStr::from_ptr(pch_key).to_string_lossy().into_owned();
    let val = CStr::from_ptr(pch_value).to_string_lossy().into_owned();
    println!(
        "[RINE STEAM] SetLobbyData(0x{:016X}, '{}', '{}')",
        steam_id_lobby, key, val
    );
    lobby_cstr_cache()
        .lock()
        .unwrap()
        .remove(&(steam_id_lobby, key.clone()));
    if let Some(lobby) = local_lobbies().lock().unwrap().get_mut(&steam_id_lobby) {
        lobby.data.insert(key, val);
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_GetLobbyData(
    _this: *mut c_void,
    steam_id_lobby: u64,
    pch_key: *const c_char,
) -> *const c_char {
    if pch_key.is_null() {
        return b"\0".as_ptr() as *const c_char;
    }
    let key = CStr::from_ptr(pch_key).to_string_lossy().into_owned();
    let val_opt = local_lobbies()
        .lock()
        .unwrap()
        .get(&steam_id_lobby)
        .and_then(|l| l.data.get(&key).cloned());
    println!(
        "[RINE STEAM] GetLobbyData(0x{:016X}, '{}') -> {:?}",
        steam_id_lobby, key, val_opt
    );
    if let Some(val) = val_opt {
        if let Ok(cstr) = std::ffi::CString::new(val) {
            let mut cache = lobby_cstr_cache().lock().unwrap();
            let entry = cache.entry((steam_id_lobby, key)).or_insert(cstr);
            return entry.as_ptr();
        }
    }
    b"\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_DeleteLobbyData(
    _this: *mut c_void,
    steam_id_lobby: u64,
    pch_key: *const c_char,
) -> u8 {
    if !pch_key.is_null() {
        let key = CStr::from_ptr(pch_key).to_string_lossy().into_owned();
        lobby_cstr_cache()
            .lock()
            .unwrap()
            .remove(&(steam_id_lobby, key.clone()));
        if let Some(lobby) = local_lobbies().lock().unwrap().get_mut(&steam_id_lobby) {
            lobby.data.remove(&key);
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_GetLobbyOwner(
    _this: *mut c_void,
    steam_id_lobby: u64,
) -> u64 {
    local_lobbies()
        .lock()
        .unwrap()
        .get(&steam_id_lobby)
        .map(|l| l.owner)
        .unwrap_or_else(|| SteamAPI_ISteamUser_GetSteamID(std::ptr::null_mut()))
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_SetLobbyOwner(
    _this: *mut c_void,
    steam_id_lobby: u64,
    steam_id_new_owner: u64,
) -> u8 {
    if let Some(lobby) = local_lobbies().lock().unwrap().get_mut(&steam_id_lobby) {
        lobby.owner = steam_id_new_owner;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_GetNumLobbyMembers(
    _this: *mut c_void,
    _steam_id_lobby: u64,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_GetLobbyMemberByIndex(
    _this: *mut c_void,
    steam_id_lobby: u64,
    i_member: i32,
) -> u64 {
    if i_member == 0 {
        SteamAPI_ISteamMatchmaking_GetLobbyOwner(_this, steam_id_lobby)
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_SetLobbyType(
    _this: *mut c_void,
    steam_id_lobby: u64,
    e_lobby_type: i32,
) -> u8 {
    if let Some(lobby) = local_lobbies().lock().unwrap().get_mut(&steam_id_lobby) {
        lobby.lobby_type = e_lobby_type;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_SetLobbyJoinable(
    _this: *mut c_void,
    steam_id_lobby: u64,
    b_joinable: u8,
) -> u8 {
    if let Some(lobby) = local_lobbies().lock().unwrap().get_mut(&steam_id_lobby) {
        lobby.joinable = b_joinable != 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_SetLobbyMemberLimit(
    _this: *mut c_void,
    steam_id_lobby: u64,
    c_max_members: i32,
) -> u8 {
    if let Some(lobby) = local_lobbies().lock().unwrap().get_mut(&steam_id_lobby) {
        lobby.max_members = c_max_members;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_GetLobbyMemberLimit(
    _this: *mut c_void,
    steam_id_lobby: u64,
) -> i32 {
    local_lobbies()
        .lock()
        .unwrap()
        .get(&steam_id_lobby)
        .map(|l| l.max_members)
        .unwrap_or(8)
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_RequestLobbyData(
    _this: *mut c_void,
    _steam_id_lobby: u64,
) -> u8 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamMatchmaking_RequestLobbyList(_this: *mut c_void) -> u64 {
    let empty_count: u32 = 0;
    enqueue_api_call_result(510, &empty_count) // LobbyMatchList_t = 510
}

#[no_mangle]
pub unsafe extern "win64" fn SteamMatchmaking() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamMusic() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamMusicRemote() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamHTMLSurface() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamInventory() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamVideo() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamParentalSettings() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamNetworkingUtils() -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamNetworkingSockets() -> *mut c_void {
    get_mock_steam_interface()
}

static NEXT_LISTEN_SOCKET: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
static NEXT_NET_CONNECTION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
static NEXT_POLL_GROUP: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_CreateListenSocketP2P(
    _this: *mut c_void,
    n_local_virtual_port: i32,
    _n_options: i32,
    _p_options: *const c_void,
) -> u32 {
    let sock = NEXT_LISTEN_SOCKET.fetch_add(1, Ordering::SeqCst);
    println!(
        "[RINE STEAM] CreateListenSocketP2P(port={}) -> {}",
        n_local_virtual_port, sock
    );
    sock
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_CloseListenSocket(
    _this: *mut c_void,
    _h_socket: u32,
) -> u8 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_ConnectP2P(
    _this: *mut c_void,
    _identity_remote: *const c_void,
    _n_remote_virtual_port: i32,
    _n_options: i32,
    _p_options: *const c_void,
) -> u32 {
    NEXT_NET_CONNECTION.fetch_add(1, Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_AcceptConnection(
    _this: *mut c_void,
    _h_conn: u32,
) -> i32 {
    1 // k_EResultOK
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_CloseConnection(
    _this: *mut c_void,
    _h_conn: u32,
    _n_reason: i32,
    _psz_debug: *const c_char,
    _b_enable_linger: u8,
) -> u8 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_CreatePollGroup(
    _this: *mut c_void,
) -> u32 {
    NEXT_POLL_GROUP.fetch_add(1, Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_DestroyPollGroup(
    _this: *mut c_void,
    _h_poll_group: u32,
) -> u8 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_SetConnectionPollGroup(
    _this: *mut c_void,
    _h_conn: u32,
    _h_poll_group: u32,
) -> u8 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_ReceiveMessagesOnConnection(
    _this: *mut c_void,
    _h_conn: u32,
    _pp_out_messages: *mut *mut c_void,
    _n_max_messages: i32,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_ReceiveMessagesOnPollGroup(
    _this: *mut c_void,
    _h_poll_group: u32,
    _pp_out_messages: *mut *mut c_void,
    _n_max_messages: i32,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_SendMessageToConnection(
    _this: *mut c_void,
    _h_conn: u32,
    _p_data: *const c_void,
    _cb_data: u32,
    _n_send_flags: i32,
    p_out_message_number: *mut i64,
) -> i32 {
    if !p_out_message_number.is_null() {
        *p_out_message_number = 1;
    }
    1 // k_EResultOK
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_FlushMessagesOnConnection(
    _this: *mut c_void,
    _h_conn: u32,
) -> i32 {
    1 // k_EResultOK
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_RunCallbacks(_this: *mut c_void) {}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_GetConnectionInfo(
    _this: *mut c_void,
    _h_conn: u32,
    _p_info: *mut c_void,
) -> u8 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingSockets_ConfigureConnectionLanes(
    _this: *mut c_void,
    _h_conn: u32,
    _n_num_lanes: i32,
    _p_lane_priorities: *const i32,
    _p_lane_weights: *const u16,
) -> i32 {
    1 // k_EResultOK
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingUtils_InitRelayNetworkAccess(
    _this: *mut c_void,
) {
    println!("[RINE STEAM] InitRelayNetworkAccess() -> OK");
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamNetworkingUtils_GetRelayNetworkStatus(
    _this: *mut c_void,
    p_details: *mut c_void,
) -> i32 {
    if !p_details.is_null() && (p_details as usize) > 0x10000 {
        // Zero all 272 bytes of SteamRelayNetworkStatus_t
        std::ptr::write_bytes(p_details as *mut u8, 0, 272);
        // m_eAvail = 100 (k_ESteamNetworkingAvailability_Current)
        *(p_details as *mut i32) = 100;
        // m_bPingMeasurementInProgress = 0
        *((p_details as *mut i32).add(1)) = 0;
        // m_eAvailNetworkConfig = 100
        *((p_details as *mut i32).add(2)) = 100;
        // m_eAvailAnyRelay = 100
        *((p_details as *mut i32).add(3)) = 100;
        let msg = b"OK\0";
        std::ptr::copy_nonoverlapping(msg.as_ptr(), (p_details as *mut u8).add(16), msg.len());
    }
    100 // k_ESteamNetworkingAvailability_Current
}

// Steamworks flat API functions (C ABI exports for Unity and C# wrappers)
// Функции Flat API Steamworks (экспорты C ABI для Unity и оберток C#)

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_BIsSubscribed(_this: *mut c_void) -> u32 {
    1 // User owns a subscription license to this game
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_BIsSubscribedApp(
    _this: *mut c_void,
    _app_id: u32,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_BIsDlcInstalled(
    _this: *mut c_void,
    _app_id: u32,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_BIsAppInstalled(
    _this: *mut c_void,
    _app_id: u32,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_BIsCybercafe(_this: *mut c_void) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_BIsVACBanned(_this: *mut c_void) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_BIsLowViolence(_this: *mut c_void) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_BIsSubscribedFromFreeWeekend(
    _this: *mut c_void,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_GetCurrentGameLanguage(
    _this: *mut c_void,
) -> *const c_char {
    b"english\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_GetAvailableGameLanguages(
    _this: *mut c_void,
) -> *const c_char {
    b"english,russian\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_GetCurrentBetaName(
    _this: *mut c_void,
    pch_name: *mut c_char,
    cch_name_buffer_size: i32,
) -> u32 {
    if !pch_name.is_null() && cch_name_buffer_size > 0 {
        *pch_name = 0;
    }
    0 // not on a beta branch
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_GetAppInstallDir(
    _this: *mut c_void,
    _app_id: u32,
    pch_folder: *mut c_char,
    cch_folder_buffer_size: u32,
) -> u32 {
    let current_dir = std::env::current_dir().unwrap_or_default();
    let dir_str = current_dir.to_string_lossy();
    let bytes = dir_str.as_bytes();
    let copy_len = bytes.len().min((cch_folder_buffer_size.saturating_sub(1)) as usize);

    if !pch_folder.is_null() && cch_folder_buffer_size > 0 {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pch_folder as *mut u8, copy_len);
        *pch_folder.add(copy_len) = 0;
    }
    copy_len as u32
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_GetAppOwner(_this: *mut c_void) -> u64 {
    DEFAULT_STEAM_ID
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_GetAppBuildId(_this: *mut c_void) -> i32 {
    25604825
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_GetDLCCount(_this: *mut c_void) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamApps_GetInstalledDepots(
    _this: *mut c_void,
    _app_id: u32,
    pvec_depots: *mut u32,
    _c_max_depots: u32,
) -> u32 {
    if !pvec_depots.is_null() {
        *pvec_depots = get_active_app_id() + 1;
        1
    } else {
        1
    }
}

// ISteamUser
#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUser_GetSteamID(_this: *mut c_void) -> u64 {
    DEFAULT_STEAM_ID
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUser_BLoggedOn(_this: *mut c_void) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUser_GetPlayerSteamLevel(_this: *mut c_void) -> i32 {
    20
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUser_GetAuthSessionTicket(
    _this: *mut c_void,
    p_ticket: *mut c_void,
    cb_max_ticket: i32,
    pcb_ticket: *mut u32,
) -> u32 {
    if !pcb_ticket.is_null() {
        *pcb_ticket = 32;
    }
    if !p_ticket.is_null() && cb_max_ticket >= 32 {
        std::ptr::write_bytes(p_ticket as *mut u8, 0xAA, 32);
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUser_GetHSteamUser(_this: *mut c_void) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUser_UserHasLicenseForApp(
    _this: *mut c_void,
    _steam_id: u64,
    _app_id: u32,
) -> u32 {
    0 // 0 = k_EUserHasLicenseResultHasLicense
}

// ISteamUtils
#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_GetAppID(_this: *mut c_void) -> u32 {
    get_active_app_id()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_GetSecondsSinceAppActive(
    _this: *mut c_void,
) -> u32 {
    60
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_GetSecondsSinceComputerActive(
    _this: *mut c_void,
) -> u32 {
    3600
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_GetConnectedUniverse(_this: *mut c_void) -> i32 {
    1 // k_EUniversePublic
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_GetServerRealTime(_this: *mut c_void) -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(1700000000)
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_GetIPCountry(_this: *mut c_void) -> *const c_char {
    b"RU\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_IsOverlayEnabled(_this: *mut c_void) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_BOverlayNeedsPresent(_this: *mut c_void) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_GetSteamUILanguage(_this: *mut c_void) -> *const c_char {
    b"english\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUtils_GetCurrentBatteryPower(_this: *mut c_void) -> u8 {
    255 // AC Power
}

// ISteamUserStats
#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_RequestCurrentStats(
    _this: *mut c_void,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_GetAchievement(
    _this: *mut c_void,
    _pch_name: *const c_char,
    pb_achieved: *mut u32,
) -> u32 {
    if !pb_achieved.is_null() {
        *pb_achieved = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_SetAchievement(
    _this: *mut c_void,
    _pch_name: *const c_char,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_ClearAchievement(
    _this: *mut c_void,
    _pch_name: *const c_char,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_GetAchievementAndUnlockTime(
    _this: *mut c_void,
    _pch_name: *const c_char,
    pb_achieved: *mut u32,
    pun_unlock_time: *mut u32,
) -> u32 {
    if !pb_achieved.is_null() {
        *pb_achieved = 0;
    }
    if !pun_unlock_time.is_null() {
        *pun_unlock_time = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_StoreStats(_this: *mut c_void) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_GetStatInt32(
    _this: *mut c_void,
    _pch_name: *const c_char,
    p_data: *mut i32,
) -> u32 {
    if !p_data.is_null() {
        *p_data = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_GetStatFloat(
    _this: *mut c_void,
    _pch_name: *const c_char,
    p_data: *mut f32,
) -> u32 {
    if !p_data.is_null() {
        *p_data = 0.0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_SetStatInt32(
    _this: *mut c_void,
    _pch_name: *const c_char,
    _data: i32,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_SetStatFloat(
    _this: *mut c_void,
    _pch_name: *const c_char,
    _data: f32,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_UpdateAvgRateStat(
    _this: *mut c_void,
    _pch_name: *const c_char,
    _fl_count_this_session: f32,
    _d_session_length: f64,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_GetNumAchievements(
    _this: *mut c_void,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_GetAchievementName(
    _this: *mut c_void,
    _i_achievement: u32,
) -> *const c_char {
    b"\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_GetAchievementDisplayAttribute(
    _this: *mut c_void,
    _pch_name: *const c_char,
    _pch_key: *const c_char,
) -> *const c_char {
    b"\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamUserStats_IndicateAchievementProgress(
    _this: *mut c_void,
    _pch_name: *const c_char,
    _n_cur_progress: u32,
    _n_max_progress: u32,
) -> u32 {
    1
}

// ISteamFriends
#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamFriends_GetPersonaName(
    _this: *mut c_void,
) -> *const c_char {
    b"RineUser\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamFriends_GetPersonaState(_this: *mut c_void) -> i32 {
    1 // k_EPersonaStateOnline
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamFriends_GetFriendCount(
    _this: *mut c_void,
    _i_friend_flags: i32,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamFriends_SetRichPresence(
    _this: *mut c_void,
    _pch_key: *const c_char,
    _pch_value: *const c_char,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamFriends_ClearRichPresence(_this: *mut c_void) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamFriends_ActivateGameOverlay(
    _this: *mut c_void,
    _pch_dialog: *const c_char,
) -> u32 {
    1
}

// Helper to get persistent save directories for Steam Remote Storage
fn get_steam_save_dirs() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let mut dirs = Vec::new();

    // 1. AppData/Roaming/Cuphead
    dirs.push(std::path::PathBuf::from(&home)
        .join(".local/share/rine/prefixes/default/drive_c/users/fizzy/AppData/Roaming/Cuphead"));
    // 2. AppData/LocalLow/Studio MDHR/Cuphead
    dirs.push(std::path::PathBuf::from(&home)
        .join(".local/share/rine/prefixes/default/drive_c/users/fizzy/AppData/LocalLow/Studio MDHR/Cuphead"));
    // 3. User home AppData
    dirs.push(std::path::PathBuf::from(&home).join("AppData/Roaming/Cuphead"));

    // 4. Dedicated Steam AppID saves directory
    let appid = get_active_app_id();
    let appid_str = if appid > 0 { appid.to_string() } else { "268910".to_string() };
    let dedicated = std::path::PathBuf::from(&home)
        .join(".local/share/rine/saves")
        .join(&appid_str)
        .join("remote");
    std::fs::create_dir_all(&dedicated).ok();
    dirs.push(dedicated);

    dirs
}

// ISteamRemoteStorage
#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_FileExists(
    _this: *mut c_void,
    pch_file: *const c_char,
) -> u32 {
    if pch_file.is_null() {
        return 0;
    }
    let filename = std::ffi::CStr::from_ptr(pch_file).to_string_lossy();
    let dirs = get_steam_save_dirs();
    for dir in &dirs {
        let p = dir.join(filename.as_ref());
        if p.is_file() {
            println!("[STEAMWORKS] FileExists('{}') -> TRUE ({:?})", filename, p);
            return 1;
        }
    }
    println!("[STEAMWORKS] FileExists('{}') -> FALSE", filename);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_FileRead(
    _this: *mut c_void,
    pch_file: *const c_char,
    pv_data: *mut c_void,
    cub_data_to_read: i32,
) -> i32 {
    if pch_file.is_null() || pv_data.is_null() || cub_data_to_read <= 0 {
        return 0;
    }
    let filename = std::ffi::CStr::from_ptr(pch_file).to_string_lossy();
    let dirs = get_steam_save_dirs();
    for dir in &dirs {
        let p = dir.join(filename.as_ref());
        if let Ok(data) = std::fs::read(&p) {
            let to_read = data.len().min(cub_data_to_read as usize);
            std::ptr::copy_nonoverlapping(data.as_ptr(), pv_data as *mut u8, to_read);
            println!("[STEAMWORKS] FileRead('{}') -> read {} bytes ({:?})", filename, to_read, p);
            return to_read as i32;
        }
    }
    println!("[STEAMWORKS] FileRead('{}') -> NOT FOUND", filename);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_FileWrite(
    _this: *mut c_void,
    pch_file: *const c_char,
    pv_data: *const c_void,
    cub_data: i32,
) -> u32 {
    if pch_file.is_null() || cub_data < 0 || pv_data.is_null() {
        return 0;
    }
    let filename = std::ffi::CStr::from_ptr(pch_file).to_string_lossy();
    let slice = std::slice::from_raw_parts(pv_data as *const u8, cub_data as usize);
    let dirs = get_steam_save_dirs();
    let mut written = false;
    for dir in &dirs {
        if std::fs::create_dir_all(dir).is_ok() {
            let p = dir.join(filename.as_ref());
            if std::fs::write(&p, slice).is_ok() {
                println!("[STEAMWORKS] FileWrite('{}', {} bytes) -> {:?}", filename, cub_data, p);
                written = true;
            }
        }
    }
    if written { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_GetFileSize(
    _this: *mut c_void,
    pch_file: *const c_char,
) -> i32 {
    if pch_file.is_null() {
        return 0;
    }
    let filename = std::ffi::CStr::from_ptr(pch_file).to_string_lossy();
    let dirs = get_steam_save_dirs();
    for dir in &dirs {
        let p = dir.join(filename.as_ref());
        if let Ok(meta) = std::fs::metadata(&p) {
            return meta.len() as i32;
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_GetFileCount(_this: *mut c_void) -> i32 {
    let dirs = get_steam_save_dirs();
    let mut count = 0;
    for dir in &dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    count += 1;
                }
            }
        }
        if count > 0 { break; }
    }
    count
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_GetFileNameAndSize(
    _this: *mut c_void,
    index: i32,
    pn_file_size_in_bytes: *mut i32,
) -> *const c_char {
    static FILE_NAME_CACHE: std::sync::Mutex<Vec<std::ffi::CString>> = std::sync::Mutex::new(Vec::new());
    let dirs = get_steam_save_dirs();
    let mut files = Vec::new();
    for dir in &dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    if let (Some(name), Ok(meta)) = (entry.file_name().to_str(), entry.metadata()) {
                        files.push((name.to_string(), meta.len() as i32));
                    }
                }
            }
        }
        if !files.is_empty() { break; }
    }
    if index >= 0 && (index as usize) < files.len() {
        let (name, size) = &files[index as usize];
        if !pn_file_size_in_bytes.is_null() {
            *pn_file_size_in_bytes = *size;
        }
        let cstr = std::ffi::CString::new(name.as_str()).unwrap_or_default();
        let ptr = cstr.as_ptr();
        let mut guard = FILE_NAME_CACHE.lock().unwrap();
        guard.push(cstr);
        ptr
    } else {
        std::ptr::null()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_FileDelete(
    _this: *mut c_void,
    pch_file: *const c_char,
) -> u32 {
    if pch_file.is_null() {
        return 0;
    }
    let filename = std::ffi::CStr::from_ptr(pch_file).to_string_lossy();
    let dirs = get_steam_save_dirs();
    let mut deleted = 0;
    for dir in &dirs {
        let p = dir.join(filename.as_ref());
        if p.is_file() && std::fs::remove_file(&p).is_ok() {
            deleted = 1;
        }
    }
    deleted
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_GetQuota(
    _this: *mut c_void,
    pn_total_bytes: *mut u64,
    pu_available_bytes: *mut u64,
) -> u32 {
    if !pn_total_bytes.is_null() {
        *pn_total_bytes = 100 * 1024 * 1024 * 1024; // 100 GB
    }
    if !pu_available_bytes.is_null() {
        *pu_available_bytes = 99 * 1024 * 1024 * 1024;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_IsCloudEnabledForAccount(_this: *mut c_void) -> u32 { 1 }

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_IsCloudEnabledForApp(_this: *mut c_void) -> u32 { 1 }

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_SetCloudEnabledForApp(_this: *mut c_void, _enabled: u32) {}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamRemoteStorage_GetFileTimestamp(
    _this: *mut c_void,
    pch_file: *const c_char,
) -> i64 {
    if pch_file.is_null() {
        return 0;
    }
    let filename = std::ffi::CStr::from_ptr(pch_file).to_string_lossy();
    let dirs = get_steam_save_dirs();
    for dir in &dirs {
        let p = dir.join(filename.as_ref());
        if let Ok(meta) = std::fs::metadata(&p) {
            if let Ok(mtime) = meta.modified() {
                if let Ok(duration) = mtime.duration_since(std::time::UNIX_EPOCH) {
                    return duration.as_secs() as i64;
                }
            }
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_SteamRemoteStorage_v014() -> *mut c_void {
    get_mock_remotestorage_interface()
}

// Interface helpers and versioned accessor proxies
// Вспомогательные функции интерфейсов и версии аксессоров

#[no_mangle]
pub unsafe extern "win64" fn steam_get_client_interface() -> *mut c_void {
    get_mock_client_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn steam_get_user_interface() -> *mut c_void {
    get_mock_user_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn steam_get_utils_interface() -> *mut c_void {
    get_mock_utils_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn steam_get_friends_interface() -> *mut c_void {
    get_mock_friends_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn steam_get_apps_interface() -> *mut c_void {
    get_mock_apps_interface()
}

// Flat ISteamClient API exports
// Экспорты функций интерфейса ISteamClient

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_CreateSteamPipe(_this: *mut c_void) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_BReleaseSteamPipe(_this: *mut c_void, _pipe: i32) -> u8 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_ConnectToGlobalUser(_this: *mut c_void, _pipe: i32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_CreateLocalUser(
    _this: *mut c_void,
    _pipe: *mut i32,
    _user_type: i32,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_ReleaseUser(
    _this: *mut c_void,
    _pipe: i32,
    _user: i32,
) {
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_GetISteamUser(
    _this: *mut c_void,
    _h_user: i32,
    _h_pipe: i32,
    _version: *const c_char,
) -> *mut c_void {
    get_mock_user_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_GetISteamFriends(
    _this: *mut c_void,
    _h_user: i32,
    _h_pipe: i32,
    _version: *const c_char,
) -> *mut c_void {
    get_mock_friends_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_GetISteamUtils(
    _this: *mut c_void,
    _h_pipe: i32,
    _version: *const c_char,
) -> *mut c_void {
    get_mock_utils_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_GetISteamUserStats(
    _this: *mut c_void,
    _h_user: i32,
    _h_pipe: i32,
    _version: *const c_char,
) -> *mut c_void {
    get_mock_userstats_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_GetISteamApps(
    _this: *mut c_void,
    _h_user: i32,
    _h_pipe: i32,
    _version: *const c_char,
) -> *mut c_void {
    get_mock_apps_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_GetISteamRemoteStorage(
    _this: *mut c_void,
    _h_user: i32,
    _h_pipe: i32,
    _version: *const c_char,
) -> *mut c_void {
    get_mock_remotestorage_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_GetISteamGenericInterface(
    _this: *mut c_void,
    _h_user: i32,
    _h_pipe: i32,
    version: *const c_char,
) -> *mut c_void {
    SteamInternal_CreateInterface(version)
}

#[no_mangle]
pub unsafe extern "win64" fn SteamAPI_ISteamClient_BShutdownIfAllPipesClosed(
    _this: *mut c_void,
) -> u8 {
    1
}

// Internal Steam client factory exports
// Внутренние фабричные функции клиента Steam

#[no_mangle]
pub unsafe extern "win64" fn SteamInternal_CreateInterface(
    version: *const c_char,
) -> *mut c_void {
    if !version.is_null() && (version as usize) > 0x10000 {
        if let Ok(ver_str) = std::ffi::CStr::from_ptr(version).to_str() {
            println!("[RINE STEAM] SteamInternal_CreateInterface('{}')", ver_str);
            if ver_str.starts_with("SteamClient") || ver_str.starts_with("STEAMCLIENT_") {
                return get_mock_client_interface();
            } else if ver_str.starts_with("SteamUser0") || ver_str.starts_with("STEAMUSER_") {
                return get_mock_user_interface();
            } else if ver_str.starts_with("SteamUtils") || ver_str.starts_with("STEAMUTILS_") {
                return get_mock_utils_interface();
            } else if ver_str.starts_with("SteamFriends") || ver_str.starts_with("STEAMFRIENDS_") {
                return get_mock_friends_interface();
            } else if ver_str.starts_with("SteamApps") || ver_str.starts_with("STEAMAPPS_") {
                return get_mock_apps_interface();
            } else if ver_str.starts_with("SteamUserStats") || ver_str.starts_with("STEAMUSERSTATS_") {
                return get_mock_userstats_interface();
            } else if ver_str.starts_with("SteamRemoteStorage") || ver_str.starts_with("STEAMREMOTESTORAGE_") {
                return get_mock_remotestorage_interface();
            }
        }
    }
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn SteamInternal_FindOrCreateUserInterface(
    _h_steam_user: i32,
    version: *const c_char,
) -> *mut c_void {
    SteamInternal_CreateInterface(version)
}

#[no_mangle]
pub unsafe extern "win64" fn SteamInternal_FindOrCreateUtilsInterface(
    _h_steam_pipe: i32,
    version: *const c_char,
) -> *mut c_void {
    SteamInternal_CreateInterface(version)
}

#[no_mangle]
pub unsafe extern "win64" fn SteamInternal_ContextInit(_p_context: *mut c_void) -> *mut c_void {
    get_mock_steam_interface()
}

#[no_mangle]
pub unsafe extern "win64" fn CreateInterface(
    _p_name: *const c_char,
    p_return_code: *mut i32,
) -> *mut c_void {
    if !p_return_code.is_null() {
        *p_return_code = 0; // IFACE_OK
    }
    get_mock_steam_interface()
}

// Universal Steam fallback stubs
// Универсальные заглушки для неподдерживаемых вызовов Steam

#[no_mangle]
pub unsafe extern "win64" fn steam_generic_stub() -> usize {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn steam_generic_true_stub() -> usize {
    1
}

/// Resolves any SteamAPI / Steamworks symbol by name.
pub fn resolve_steam_symbol(fn_name: &str) -> Option<usize> {
    match fn_name {
        // Core initialization and lifecycle (both modern and older Unity/CSteamworks)
        "SteamAPI_Init" | "InitSafe" | "SteamAPI_InitSafe" | "SteamGameServer_InitSafe" | "Init" => {
            Some(SteamAPI_Init as *const () as usize)
        }
        "SteamAPI_InitFlat" | "SteamInternal_SteamAPI_Init" => {
            Some(SteamInternal_SteamAPI_Init as *const () as usize)
        }
        "SteamAPI_Shutdown" | "Shutdown" => Some(SteamAPI_Shutdown as *const () as usize),
        "SteamAPI_RestartAppIfNecessary" | "RestartAppIfNecessary" => {
            Some(SteamAPI_RestartAppIfNecessary as *const () as usize)
        }
        "SteamAPI_IsSteamRunning" | "IsSteamRunning" => {
            Some(SteamAPI_IsSteamRunning as *const () as usize)
        }
        "SteamAPI_RunCallbacks" | "RunCallbacks" | "Steam_RunCallbacks" => {
            Some(SteamAPI_RunCallbacks as *const () as usize)
        }
        "GetHSteamPipe_" => Some(SteamAPI_GetHSteamPipe as *const () as usize),
        "GetHSteamUser_" => Some(SteamAPI_GetHSteamUser as *const () as usize),
        "SteamClient_" => Some(get_mock_client_interface as *const () as usize),
        "SteamUser_" => Some(get_mock_user_interface as *const () as usize),
        "SteamFriends_" => Some(get_mock_friends_interface as *const () as usize),
        "SteamUtils_" => Some(get_mock_utils_interface as *const () as usize),
        "SteamApps_" => Some(get_mock_apps_interface as *const () as usize),
        "SteamUserStats_" => Some(get_mock_userstats_interface as *const () as usize),
        "SteamRemoteStorage_" => Some(get_mock_remotestorage_interface as *const () as usize),
        "SteamAPI_ReleaseCurrentThreadMemory" => {
            Some(SteamAPI_ReleaseCurrentThreadMemory as *const () as usize)
        }
        "SteamAPI_GetHSteamPipe" | "Steam_GetHSteamPipe" => {
            Some(SteamAPI_GetHSteamPipe as *const () as usize)
        }
        "SteamAPI_GetHSteamUser"
        | "Steam_GetHSteamUser"
        | "Steam_GetHSteamUserCurrent" => {
            Some(SteamAPI_GetHSteamUser as *const () as usize)
        }
        "SteamAPI_GetSteamInstallPath" => {
            Some(SteamAPI_GetSteamInstallPath as *const () as usize)
        }
        "SteamAPI_RegisterCallback" => Some(SteamAPI_RegisterCallback as *const () as usize),
        "SteamAPI_UnregisterCallback" => {
            Some(SteamAPI_UnregisterCallback as *const () as usize)
        }
        "SteamAPI_RegisterCallResult" => {
            Some(SteamAPI_RegisterCallResult as *const () as usize)
        }
        "SteamAPI_UnregisterCallResult" => {
            Some(SteamAPI_UnregisterCallResult as *const () as usize)
        }

        // Interface Accessors
        "SteamApps" => Some(SteamApps as *const () as usize),
        "SteamUser" => Some(SteamUser as *const () as usize),
        "SteamUtils" => Some(SteamUtils as *const () as usize),
        "SteamUserStats" => Some(SteamUserStats as *const () as usize),
        "SteamFriends" => Some(SteamFriends as *const () as usize),
        "SteamClient" => Some(SteamClient as *const () as usize),
        "SteamScreenshots" => Some(SteamScreenshots as *const () as usize),
        "SteamRemoteStorage" => Some(SteamRemoteStorage as *const () as usize),
        "SteamHTTP" => Some(SteamHTTP as *const () as usize),
        "SteamUGC" => Some(SteamUGC as *const () as usize),
        "SteamInput" | "SteamController" => Some(SteamInput as *const () as usize),
        "SteamNetworking" => Some(SteamNetworking as *const () as usize),
        "SteamMatchmaking" => Some(SteamMatchmaking as *const () as usize),
        "SteamMusic" => Some(SteamMusic as *const () as usize),
        "SteamMusicRemote" => Some(SteamMusicRemote as *const () as usize),
        "SteamHTMLSurface" => Some(SteamHTMLSurface as *const () as usize),
        "SteamInventory" => Some(SteamInventory as *const () as usize),
        "SteamVideo" => Some(SteamVideo as *const () as usize),
        "SteamParentalSettings" => Some(SteamParentalSettings as *const () as usize),
        "SteamNetworkingUtils" => Some(SteamNetworkingUtils as *const () as usize),
        "SteamNetworkingSockets" => Some(SteamNetworkingSockets as *const () as usize),
        "SteamAPI_ISteamNetworkingUtils_InitRelayNetworkAccess" => {
            Some(SteamAPI_ISteamNetworkingUtils_InitRelayNetworkAccess as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingUtils_GetRelayNetworkStatus" => {
            Some(SteamAPI_ISteamNetworkingUtils_GetRelayNetworkStatus as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_CreateListenSocketP2P" => {
            Some(SteamAPI_ISteamNetworkingSockets_CreateListenSocketP2P as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_CloseListenSocket" => {
            Some(SteamAPI_ISteamNetworkingSockets_CloseListenSocket as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_ConnectP2P" => {
            Some(SteamAPI_ISteamNetworkingSockets_ConnectP2P as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_AcceptConnection" => {
            Some(SteamAPI_ISteamNetworkingSockets_AcceptConnection as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_CloseConnection" => {
            Some(SteamAPI_ISteamNetworkingSockets_CloseConnection as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_CreatePollGroup" => {
            Some(SteamAPI_ISteamNetworkingSockets_CreatePollGroup as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_DestroyPollGroup" => {
            Some(SteamAPI_ISteamNetworkingSockets_DestroyPollGroup as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_SetConnectionPollGroup" => {
            Some(SteamAPI_ISteamNetworkingSockets_SetConnectionPollGroup as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_ReceiveMessagesOnConnection" => {
            Some(SteamAPI_ISteamNetworkingSockets_ReceiveMessagesOnConnection as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_ReceiveMessagesOnPollGroup" => {
            Some(SteamAPI_ISteamNetworkingSockets_ReceiveMessagesOnPollGroup as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_SendMessageToConnection" => {
            Some(SteamAPI_ISteamNetworkingSockets_SendMessageToConnection as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_FlushMessagesOnConnection" => {
            Some(SteamAPI_ISteamNetworkingSockets_FlushMessagesOnConnection as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_RunCallbacks" => {
            Some(SteamAPI_ISteamNetworkingSockets_RunCallbacks as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_GetConnectionInfo" => {
            Some(SteamAPI_ISteamNetworkingSockets_GetConnectionInfo as *const () as usize)
        }
        "SteamAPI_ISteamNetworkingSockets_ConfigureConnectionLanes" => {
            Some(SteamAPI_ISteamNetworkingSockets_ConfigureConnectionLanes as *const () as usize)
        }

        // Flat ISteamApps
        "SteamAPI_ISteamApps_BIsSubscribed" => {
            Some(SteamAPI_ISteamApps_BIsSubscribed as *const () as usize)
        }
        "SteamAPI_ISteamApps_BIsSubscribedApp" => {
            Some(SteamAPI_ISteamApps_BIsSubscribedApp as *const () as usize)
        }
        "SteamAPI_ISteamApps_BIsDlcInstalled" => {
            Some(SteamAPI_ISteamApps_BIsDlcInstalled as *const () as usize)
        }
        "SteamAPI_ISteamApps_BIsAppInstalled" => {
            Some(SteamAPI_ISteamApps_BIsAppInstalled as *const () as usize)
        }
        "SteamAPI_ISteamApps_BIsCybercafe" => {
            Some(SteamAPI_ISteamApps_BIsCybercafe as *const () as usize)
        }
        "SteamAPI_ISteamApps_BIsVACBanned" => {
            Some(SteamAPI_ISteamApps_BIsVACBanned as *const () as usize)
        }
        "SteamAPI_ISteamApps_BIsLowViolence" => {
            Some(SteamAPI_ISteamApps_BIsLowViolence as *const () as usize)
        }
        "SteamAPI_ISteamApps_BIsSubscribedFromFreeWeekend" => {
            Some(SteamAPI_ISteamApps_BIsSubscribedFromFreeWeekend as *const () as usize)
        }
        "SteamAPI_ISteamApps_GetCurrentGameLanguage" => {
            Some(SteamAPI_ISteamApps_GetCurrentGameLanguage as *const () as usize)
        }
        "SteamAPI_ISteamApps_GetAvailableGameLanguages" => {
            Some(SteamAPI_ISteamApps_GetAvailableGameLanguages as *const () as usize)
        }
        "SteamAPI_ISteamApps_GetCurrentBetaName" => {
            Some(SteamAPI_ISteamApps_GetCurrentBetaName as *const () as usize)
        }
        "SteamAPI_ISteamApps_GetAppInstallDir" => {
            Some(SteamAPI_ISteamApps_GetAppInstallDir as *const () as usize)
        }
        "SteamAPI_ISteamApps_GetAppOwner" => {
            Some(SteamAPI_ISteamApps_GetAppOwner as *const () as usize)
        }
        "SteamAPI_ISteamApps_GetAppBuildId" => {
            Some(SteamAPI_ISteamApps_GetAppBuildId as *const () as usize)
        }
        "SteamAPI_ISteamApps_GetDLCCount" => {
            Some(SteamAPI_ISteamApps_GetDLCCount as *const () as usize)
        }
        "SteamAPI_ISteamApps_GetInstalledDepots" => {
            Some(SteamAPI_ISteamApps_GetInstalledDepots as *const () as usize)
        }

        // Flat ISteamUser
        "SteamAPI_ISteamUser_GetSteamID" => {
            Some(SteamAPI_ISteamUser_GetSteamID as *const () as usize)
        }
        "SteamAPI_ISteamUser_GetHSteamUser" => {
            Some(SteamAPI_ISteamUser_GetHSteamUser as *const () as usize)
        }
        "SteamAPI_ISteamUser_BLoggedOn" => {
            Some(SteamAPI_ISteamUser_BLoggedOn as *const () as usize)
        }
        "SteamAPI_ISteamUser_GetPlayerSteamLevel" => {
            Some(SteamAPI_ISteamUser_GetPlayerSteamLevel as *const () as usize)
        }
        "SteamAPI_ISteamUser_GetAuthSessionTicket" => {
            Some(SteamAPI_ISteamUser_GetAuthSessionTicket as *const () as usize)
        }
        "SteamAPI_ISteamUser_UserHasLicenseForApp" => {
            Some(SteamAPI_ISteamUser_UserHasLicenseForApp as *const () as usize)
        }

        // Flat ISteamUtils
        "SteamAPI_ISteamUtils_GetAppID" => {
            Some(SteamAPI_ISteamUtils_GetAppID as *const () as usize)
        }
        "SteamAPI_ISteamUtils_GetSecondsSinceAppActive" => {
            Some(SteamAPI_ISteamUtils_GetSecondsSinceAppActive as *const () as usize)
        }
        "SteamAPI_ISteamUtils_GetSecondsSinceComputerActive" => {
            Some(SteamAPI_ISteamUtils_GetSecondsSinceComputerActive as *const () as usize)
        }
        "SteamAPI_ISteamUtils_GetConnectedUniverse" => {
            Some(SteamAPI_ISteamUtils_GetConnectedUniverse as *const () as usize)
        }
        "SteamAPI_ISteamUtils_GetServerRealTime" => {
            Some(SteamAPI_ISteamUtils_GetServerRealTime as *const () as usize)
        }
        "SteamAPI_ISteamUtils_GetIPCountry" => {
            Some(SteamAPI_ISteamUtils_GetIPCountry as *const () as usize)
        }
        "SteamAPI_ISteamUtils_IsOverlayEnabled" => {
            Some(SteamAPI_ISteamUtils_IsOverlayEnabled as *const () as usize)
        }
        "SteamAPI_ISteamUtils_BOverlayNeedsPresent" => {
            Some(SteamAPI_ISteamUtils_BOverlayNeedsPresent as *const () as usize)
        }
        "SteamAPI_ISteamUtils_GetSteamUILanguage" => {
            Some(SteamAPI_ISteamUtils_GetSteamUILanguage as *const () as usize)
        }
        "SteamAPI_ISteamUtils_GetCurrentBatteryPower" => {
            Some(SteamAPI_ISteamUtils_GetCurrentBatteryPower as *const () as usize)
        }

        // Flat ISteamUserStats
        "SteamAPI_ISteamUserStats_RequestCurrentStats" => {
            Some(SteamAPI_ISteamUserStats_RequestCurrentStats as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_GetAchievement" => {
            Some(SteamAPI_ISteamUserStats_GetAchievement as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_SetAchievement" => {
            Some(SteamAPI_ISteamUserStats_SetAchievement as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_ClearAchievement" => {
            Some(SteamAPI_ISteamUserStats_ClearAchievement as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_GetAchievementAndUnlockTime" => {
            Some(SteamAPI_ISteamUserStats_GetAchievementAndUnlockTime as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_StoreStats" => {
            Some(SteamAPI_ISteamUserStats_StoreStats as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_GetStatInt32" => {
            Some(SteamAPI_ISteamUserStats_GetStatInt32 as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_GetStatFloat" => {
            Some(SteamAPI_ISteamUserStats_GetStatFloat as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_SetStatInt32" => {
            Some(SteamAPI_ISteamUserStats_SetStatInt32 as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_SetStatFloat" => {
            Some(SteamAPI_ISteamUserStats_SetStatFloat as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_UpdateAvgRateStat" => {
            Some(SteamAPI_ISteamUserStats_UpdateAvgRateStat as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_GetNumAchievements" => {
            Some(SteamAPI_ISteamUserStats_GetNumAchievements as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_GetAchievementName" => {
            Some(SteamAPI_ISteamUserStats_GetAchievementName as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_GetAchievementDisplayAttribute" => {
            Some(SteamAPI_ISteamUserStats_GetAchievementDisplayAttribute as *const () as usize)
        }
        "SteamAPI_ISteamUserStats_IndicateAchievementProgress" => {
            Some(SteamAPI_ISteamUserStats_IndicateAchievementProgress as *const () as usize)
        }

        // Flat ISteamFriends
        "SteamAPI_ISteamFriends_GetPersonaName" => {
            Some(SteamAPI_ISteamFriends_GetPersonaName as *const () as usize)
        }
        "SteamAPI_ISteamFriends_GetPersonaState" => {
            Some(SteamAPI_ISteamFriends_GetPersonaState as *const () as usize)
        }
        "SteamAPI_ISteamFriends_GetFriendCount" => {
            Some(SteamAPI_ISteamFriends_GetFriendCount as *const () as usize)
        }
        "SteamAPI_ISteamFriends_SetRichPresence" => {
            Some(SteamAPI_ISteamFriends_SetRichPresence as *const () as usize)
        }
        "SteamAPI_ISteamFriends_ClearRichPresence" => {
            Some(SteamAPI_ISteamFriends_ClearRichPresence as *const () as usize)
        }
        "SteamAPI_ISteamFriends_ActivateGameOverlay" => {
            Some(SteamAPI_ISteamFriends_ActivateGameOverlay as *const () as usize)
        }

        // Flat ISteamRemoteStorage
        "SteamAPI_ISteamRemoteStorage_FileExists" => {
            Some(SteamAPI_ISteamRemoteStorage_FileExists as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_GetFileCount" => {
            Some(SteamAPI_ISteamRemoteStorage_GetFileCount as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_FileWrite" => {
            Some(SteamAPI_ISteamRemoteStorage_FileWrite as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_FileRead" => {
            Some(SteamAPI_ISteamRemoteStorage_FileRead as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_GetFileSize" => {
            Some(SteamAPI_ISteamRemoteStorage_GetFileSize as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_GetFileNameAndSize" => {
            Some(SteamAPI_ISteamRemoteStorage_GetFileNameAndSize as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_FileDelete" => {
            Some(SteamAPI_ISteamRemoteStorage_FileDelete as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_GetQuota" => {
            Some(SteamAPI_ISteamRemoteStorage_GetQuota as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_IsCloudEnabledForAccount" => {
            Some(SteamAPI_ISteamRemoteStorage_IsCloudEnabledForAccount as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_IsCloudEnabledForApp" => {
            Some(SteamAPI_ISteamRemoteStorage_IsCloudEnabledForApp as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_SetCloudEnabledForApp" => {
            Some(SteamAPI_ISteamRemoteStorage_SetCloudEnabledForApp as *const () as usize)
        }
        "SteamAPI_ISteamRemoteStorage_GetFileTimestamp" => {
            Some(SteamAPI_ISteamRemoteStorage_GetFileTimestamp as *const () as usize)
        }
        "SteamAPI_SteamRemoteStorage_v014" | "SteamAPI_SteamRemoteStorage_v013" | "SteamAPI_SteamRemoteStorage_v012" => {
            Some(SteamAPI_SteamRemoteStorage_v014 as *const () as usize)
        }

        // Flat ISteamClient
        "SteamAPI_ISteamClient_CreateSteamPipe" => {
            Some(SteamAPI_ISteamClient_CreateSteamPipe as *const () as usize)
        }
        "SteamAPI_ISteamClient_BReleaseSteamPipe" => {
            Some(SteamAPI_ISteamClient_BReleaseSteamPipe as *const () as usize)
        }
        "SteamAPI_ISteamClient_ConnectToGlobalUser" => {
            Some(SteamAPI_ISteamClient_ConnectToGlobalUser as *const () as usize)
        }
        "SteamAPI_ISteamClient_CreateLocalUser" => {
            Some(SteamAPI_ISteamClient_CreateLocalUser as *const () as usize)
        }
        "SteamAPI_ISteamClient_ReleaseUser" => {
            Some(SteamAPI_ISteamClient_ReleaseUser as *const () as usize)
        }
        "SteamAPI_ISteamClient_GetISteamUser" => {
            Some(SteamAPI_ISteamClient_GetISteamUser as *const () as usize)
        }
        "SteamAPI_ISteamClient_GetISteamFriends" => {
            Some(SteamAPI_ISteamClient_GetISteamFriends as *const () as usize)
        }
        "SteamAPI_ISteamClient_GetISteamUtils" => {
            Some(SteamAPI_ISteamClient_GetISteamUtils as *const () as usize)
        }
        "SteamAPI_ISteamClient_GetISteamUserStats" => {
            Some(SteamAPI_ISteamClient_GetISteamUserStats as *const () as usize)
        }
        "SteamAPI_ISteamClient_GetISteamApps" => {
            Some(SteamAPI_ISteamClient_GetISteamApps as *const () as usize)
        }
        "SteamAPI_ISteamClient_GetISteamRemoteStorage" => {
            Some(SteamAPI_ISteamClient_GetISteamRemoteStorage as *const () as usize)
        }
        "SteamAPI_ISteamClient_GetISteamGenericInterface" => {
            Some(SteamAPI_ISteamClient_GetISteamGenericInterface as *const () as usize)
        }
        name if name.starts_with("SteamAPI_ISteamClient_GetISteam") => {
            Some(SteamAPI_ISteamClient_GetISteamGenericInterface as *const () as usize)
        }
        "SteamAPI_ISteamClient_BShutdownIfAllPipesClosed" => {
            Some(SteamAPI_ISteamClient_BShutdownIfAllPipesClosed as *const () as usize)
        }

        // Internal / Client
        "SteamInternal_CreateInterface" => {
            Some(SteamInternal_CreateInterface as *const () as usize)
        }
        "SteamInternal_FindOrCreateUserInterface" => {
            Some(SteamInternal_FindOrCreateUserInterface as *const () as usize)
        }
        "SteamInternal_FindOrCreateUtilsInterface" => {
            Some(SteamInternal_FindOrCreateUtilsInterface as *const () as usize)
        }
        "SteamInternal_ContextInit" => {
            Some(SteamInternal_ContextInit as *const () as usize)
        }
        "CreateInterface" => Some(CreateInterface as *const () as usize),

        // Versioned Interface Accessors
        name if name.starts_with("SteamAPI_SteamClient_v") => {
            Some(steam_get_client_interface as *const () as usize)
        }
        name if name.starts_with("SteamAPI_SteamUser_v") => {
            Some(steam_get_user_interface as *const () as usize)
        }
        name if name.starts_with("SteamAPI_SteamUtils_v") => {
            Some(steam_get_utils_interface as *const () as usize)
        }
        name if name.starts_with("SteamAPI_SteamFriends_v") => {
            Some(steam_get_friends_interface as *const () as usize)
        }
        name if name.starts_with("SteamAPI_SteamApps_v") => {
            Some(steam_get_apps_interface as *const () as usize)
        }
        name if name.starts_with("SteamAPI_SteamUserStats_v") => {
            Some(get_mock_userstats_interface as *const () as usize)
        }
        name if name.starts_with("SteamAPI_SteamRemoteStorage_v") => {
            Some(get_mock_remotestorage_interface as *const () as usize)
        }

        // ManualDispatch API (used by modern Steamworks.NET / Heathen)
        "SteamAPI_ManualDispatch_Init" => Some(SteamAPI_ManualDispatch_Init as *const () as usize),
        "SteamAPI_ManualDispatch_RunFrame" => {
            Some(SteamAPI_ManualDispatch_RunFrame as *const () as usize)
        }
        "SteamAPI_ManualDispatch_GetNextCallback" => {
            Some(SteamAPI_ManualDispatch_GetNextCallback as *const () as usize)
        }
        "SteamAPI_ManualDispatch_FreeLastCallback" => {
            Some(SteamAPI_ManualDispatch_FreeLastCallback as *const () as usize)
        }
        "SteamAPI_ManualDispatch_GetAPICallResult" => {
            Some(SteamAPI_ManualDispatch_GetAPICallResult as *const () as usize)
        }

        // Matchmaking API
        "SteamAPI_ISteamMatchmaking_CreateLobby" => {
            Some(SteamAPI_ISteamMatchmaking_CreateLobby as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_JoinLobby" => {
            Some(SteamAPI_ISteamMatchmaking_JoinLobby as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_LeaveLobby" => {
            Some(SteamAPI_ISteamMatchmaking_LeaveLobby as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_SetLobbyData" => {
            Some(SteamAPI_ISteamMatchmaking_SetLobbyData as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_GetLobbyData" => {
            Some(SteamAPI_ISteamMatchmaking_GetLobbyData as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_DeleteLobbyData" => {
            Some(SteamAPI_ISteamMatchmaking_DeleteLobbyData as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_GetLobbyOwner" => {
            Some(SteamAPI_ISteamMatchmaking_GetLobbyOwner as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_SetLobbyOwner" => {
            Some(SteamAPI_ISteamMatchmaking_SetLobbyOwner as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_GetNumLobbyMembers" => {
            Some(SteamAPI_ISteamMatchmaking_GetNumLobbyMembers as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_GetLobbyMemberByIndex" => {
            Some(SteamAPI_ISteamMatchmaking_GetLobbyMemberByIndex as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_SetLobbyType" => {
            Some(SteamAPI_ISteamMatchmaking_SetLobbyType as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_SetLobbyJoinable" => {
            Some(SteamAPI_ISteamMatchmaking_SetLobbyJoinable as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_SetLobbyMemberLimit" => {
            Some(SteamAPI_ISteamMatchmaking_SetLobbyMemberLimit as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_GetLobbyMemberLimit" => {
            Some(SteamAPI_ISteamMatchmaking_GetLobbyMemberLimit as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_RequestLobbyData" => {
            Some(SteamAPI_ISteamMatchmaking_RequestLobbyData as *const () as usize)
        }
        "SteamAPI_ISteamMatchmaking_RequestLobbyList" => {
            Some(SteamAPI_ISteamMatchmaking_RequestLobbyList as *const () as usize)
        }

        // Prefix fallback for older Unity / CSteamworks flat functions (e.g. ISteamApps_BIsSubscribed)
        name if name.starts_with("ISteam") => {
            let prefixed = format!("SteamAPI_{}", name);
            if let Some(ptr) = resolve_steam_symbol(&prefixed) {
                return Some(ptr);
            }
            let allow_broad_get = get_unity_generation() != UnityGeneration::Unity6Plus;
            if name.contains("IsSubscribed") || name.contains("BIs") || name.contains("Installed") {
                Some(steam_generic_true_stub as *const () as usize)
            } else if name.contains("_GetISteam") || (allow_broad_get && name.contains("_Get")) {
                Some(get_mock_steam_interface as *const () as usize)
            } else {
                Some(steam_generic_stub as *const () as usize)
            }
        }

        // Fallback for any other Steam export
        other => {
            if other.starts_with("SteamAPI_")
                || other.starts_with("Steam_")
                || other.starts_with("SteamInternal_")
                || other.starts_with("SteamClient_")
                || other.starts_with("ISteam")
            {
                let allow_broad_get = get_unity_generation() != UnityGeneration::Unity6Plus;
                if other.contains("Init")
                    || other.contains("IsSubscribed")
                    || other.contains("IsLogged")
                    || other.contains("BIs")
                    || other.contains("IsSteamRunning")
                {
                    Some(steam_generic_true_stub as *const () as usize)
                } else if other.contains("_GetISteam") || (allow_broad_get && other.contains("_Get")) {
                    Some(get_mock_steam_interface as *const () as usize)
                } else {
                    Some(steam_generic_stub as *const () as usize)
                }
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steam_remote_storage_persistence_lifecycle() {
        unsafe {
            let test_filename = std::ffi::CString::new("test_save_persistence_slot_0.sav").unwrap();
            let test_data = b"CUPHEAD_SAVE_MAGIC_TEST_DATA_12345";

            // 1. Write file
            let write_res = SteamAPI_ISteamRemoteStorage_FileWrite(
                std::ptr::null_mut(),
                test_filename.as_ptr(),
                test_data.as_ptr() as *const c_void,
                test_data.len() as i32,
            );
            assert_eq!(write_res, 1, "FileWrite must succeed");

            // 2. Check FileExists
            let exists = SteamAPI_ISteamRemoteStorage_FileExists(
                std::ptr::null_mut(),
                test_filename.as_ptr(),
            );
            assert_eq!(exists, 1, "FileExists must return 1 after write");

            // 3. Check GetFileSize
            let size = SteamAPI_ISteamRemoteStorage_GetFileSize(
                std::ptr::null_mut(),
                test_filename.as_ptr(),
            );
            assert_eq!(size, test_data.len() as i32, "GetFileSize must match written data length");

            // 4. Read back
            let mut read_buf = [0u8; 64];
            let read_bytes = SteamAPI_ISteamRemoteStorage_FileRead(
                std::ptr::null_mut(),
                test_filename.as_ptr(),
                read_buf.as_mut_ptr() as *mut c_void,
                read_buf.len() as i32,
            );
            assert_eq!(read_bytes, test_data.len() as i32, "FileRead must read all written bytes");
            assert_eq!(&read_buf[..test_data.len()], test_data, "Read data must match written data");

            // 5. Delete file
            let del_res = SteamAPI_ISteamRemoteStorage_FileDelete(
                std::ptr::null_mut(),
                test_filename.as_ptr(),
            );
            assert_eq!(del_res, 1, "FileDelete must succeed");

            // 6. Check FileExists is now 0
            let exists_after = SteamAPI_ISteamRemoteStorage_FileExists(
                std::ptr::null_mut(),
                test_filename.as_ptr(),
            );
            assert_eq!(exists_after, 0, "FileExists must return 0 after delete");
        }
    }

    #[test]
    fn test_unity_generation_steam_license_profiles() {
        assert_eq!(
            crate::steam_scanner::parse_unity_version_from_bytes(b"\0\0\0\x145.6.7f1\0"),
            Some("5.6.7f1".to_string())
        );
        assert_eq!(
            crate::steam_scanner::parse_unity_version_from_bytes(b"\0\0\x142017.4.9f1\0"),
            Some("2017.4.9f1".to_string())
        );
        assert_eq!(
            crate::steam_scanner::parse_unity_version_from_bytes(b"\0\0\x146000.0.25f1\0"),
            Some("6000.0.25f1".to_string())
        );

        let generic_stub = steam_generic_stub as *const () as usize;
        let mock_iface = get_mock_steam_interface as *const () as usize;
        let huser_fn = SteamAPI_GetHSteamUser as *const () as usize;
        let manual_next_fn = SteamAPI_ManualDispatch_GetNextCallback as *const () as usize;

        set_unity_generation(UnityGeneration::Unity5OrLegacy);
        assert_eq!(resolve_steam_symbol("Steam_GetHSteamUserCurrent"), Some(huser_fn));
        assert_eq!(resolve_steam_symbol("SteamAPI_ISteamLegacy_GetCustomPtr"), Some(mock_iface));
        assert_eq!(resolve_steam_symbol("SteamAPI_ManualDispatch_GetNextCallback"), Some(manual_next_fn));

        set_unity_generation(UnityGeneration::Unity6Plus);
        assert_eq!(resolve_steam_symbol("Steam_GetHSteamUserCurrent"), Some(huser_fn));
        assert_eq!(resolve_steam_symbol("SteamAPI_ISteamLegacy_GetCustomPtr"), Some(generic_stub));
        assert_eq!(resolve_steam_symbol("SteamAPI_ManualDispatch_GetNextCallback"), Some(manual_next_fn));

        set_unity_generation(UnityGeneration::Unknown);
    }

    #[test]
    fn test_steam_matchmaking_create_lobby_manual_dispatch() {
        unsafe {
            let h_call = SteamAPI_ISteamMatchmaking_CreateLobby(std::ptr::null_mut(), 0, 4);
            assert_ne!(h_call, 0);

            let mut msg = CallbackMsgT {
                m_h_steam_user: 0,
                m_i_callback: 0,
                m_pub_param: std::ptr::null_mut(),
                m_cub_param: 0,
            };
            assert_eq!(SteamAPI_ManualDispatch_GetNextCallback(1, &mut msg), 1);
            assert_eq!(msg.m_i_callback, 703); // SteamAPICallCompleted_t
            assert_eq!(msg.m_cub_param, 16);

            let mut lobby_created = LobbyCreatedT {
                m_e_result: 0,
                m_pad: 0,
                m_ul_steam_id_lobby: 0,
            };
            let mut failed: u32 = 0x1234_5678;
            assert_eq!(
                SteamAPI_ManualDispatch_GetAPICallResult(
                    1,
                    h_call,
                    &mut lobby_created as *mut _ as *mut c_void,
                    std::mem::size_of::<LobbyCreatedT>() as i32,
                    513,
                    &mut failed as *mut u32 as *mut u8,
                ),
                1
            );
            assert_eq!(failed, 0);
            assert_eq!(lobby_created.m_e_result, 1);
            assert_ne!(lobby_created.m_ul_steam_id_lobby, 0);
            SteamAPI_ManualDispatch_FreeLastCallback(1);

            // Next callback should be LobbyCreated_t (513) for Callback<LobbyCreated_t>
            assert_eq!(SteamAPI_ManualDispatch_GetNextCallback(1, &mut msg), 1);
            assert_eq!(msg.m_i_callback, 513);
            assert_eq!(msg.m_cub_param, 16);
            let cb_created = *(msg.m_pub_param as *const LobbyCreatedT);
            assert_eq!(cb_created.m_e_result, 1);
            assert_eq!(cb_created.m_ul_steam_id_lobby, lobby_created.m_ul_steam_id_lobby);
            SteamAPI_ManualDispatch_FreeLastCallback(1);

            // Next callback should be LobbyEnter_t (504)
            assert_eq!(SteamAPI_ManualDispatch_GetNextCallback(1, &mut msg), 1);
            assert_eq!(msg.m_i_callback, 504);
            SteamAPI_ManualDispatch_FreeLastCallback(1);

            // SetLobbyData & GetLobbyData (including overwrite invalidating CStr cache)
            let lobby_id = lobby_created.m_ul_steam_id_lobby;
            assert_eq!(
                SteamAPI_ISteamMatchmaking_SetLobbyData(
                    std::ptr::null_mut(),
                    lobby_id,
                    b"version\0".as_ptr() as *const c_char,
                    b"1.0\0".as_ptr() as *const c_char,
                ),
                1
            );
            let v1 = CStr::from_ptr(SteamAPI_ISteamMatchmaking_GetLobbyData(
                std::ptr::null_mut(),
                lobby_id,
                b"version\0".as_ptr() as *const c_char,
            ))
            .to_string_lossy()
            .into_owned();
            assert_eq!(v1, "1.0");

            assert_eq!(
                SteamAPI_ISteamMatchmaking_SetLobbyData(
                    std::ptr::null_mut(),
                    lobby_id,
                    b"version\0".as_ptr() as *const c_char,
                    b"2.0\0".as_ptr() as *const c_char,
                ),
                1
            );
            let v2 = CStr::from_ptr(SteamAPI_ISteamMatchmaking_GetLobbyData(
                std::ptr::null_mut(),
                lobby_id,
                b"version\0".as_ptr() as *const c_char,
            ))
            .to_string_lossy()
            .into_owned();
            assert_eq!(v2, "2.0");
        }
    }
}
