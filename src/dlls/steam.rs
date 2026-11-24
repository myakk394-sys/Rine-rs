//! Native Steamworks & Steam Bridge Implementation for Rine
//!
//! Provides high-performance, native Win64 ABI implementations of Steamworks API
//! functions (steam_api64.dll, steam_api.dll, steamclient64.dll, lsteamclient.dll).
//! Eliminates crashes caused by null vtables or missing Proton bridges, emulates
//! license verification checks (SteamAPI_RestartAppIfNecessary -> 0, BIsSubscribed -> 1),
//! and maintains a full mock C++ vtable table so virtual interface calls succeed safely.

use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

static DEFAULT_STEAM_ID: u64 = 76561198763491797;
static ACTIVE_APP_ID: AtomicU32 = AtomicU32::new(4001890);

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
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/fizzy".to_string());
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
        "SteamAPI_GetHSteamUser" | "Steam_GetHSteamUser" => {
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

        // Prefix fallback for older Unity / CSteamworks flat functions (e.g. ISteamApps_BIsSubscribed)
        name if name.starts_with("ISteam") => {
            let prefixed = format!("SteamAPI_{}", name);
            if let Some(ptr) = resolve_steam_symbol(&prefixed) {
                return Some(ptr);
            }
            if name.contains("IsSubscribed") || name.contains("BIs") || name.contains("Installed") {
                Some(steam_generic_true_stub as *const () as usize)
            } else if name.contains("_GetISteam") || name.contains("_Get") {
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
                if other.contains("Init")
                    || other.contains("IsSubscribed")
                    || other.contains("IsLogged")
                    || other.contains("BIs")
                    || other.contains("IsSteamRunning")
                {
                    Some(steam_generic_true_stub as *const () as usize)
                } else if other.contains("_Get") {
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
}
