//! Native Steamclient Bridge (`steamclient64.dll` -> Linux `steamclient.so`)
//!
//! When a game ships its original `steam_api64.dll` and the native Linux Steam
//! client (`steamclient.so`) is available, Rine loads the real `steam_api64.dll`
//! PE binary and bridges its `steamclient64.dll` calls directly to Valve's
//! Linux `steamclient.so` using Win64 <-> System V AMD64 ABI thunks.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

pub const STEAMCLIENT_BRIDGE_BASE: usize = 0x7FE1_8000;

static STEAM_BRIDGE_ENABLED: AtomicBool = AtomicBool::new(false);
static STEAM_REAL_DLL_FOUND: AtomicBool = AtomicBool::new(false);
static STEAM_OFFLINE_FORCED: AtomicBool = AtomicBool::new(false);

type SysVCreateInterfaceFn = unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void;
type SysVBGetCallbackFn = unsafe extern "C" fn(i32, *mut c_void) -> u8;
type SysVFreeLastCallbackFn = unsafe extern "C" fn(i32);
type SysVGetAPICallResultFn =
    unsafe extern "C" fn(i32, u64, *mut c_void, i32, i32, *mut u8) -> u8;
type SysVReleaseThreadLocalMemoryFn = unsafe extern "C" fn(i32);

struct NativeSteamClient {
    _handle: *mut c_void,
    create_interface: SysVCreateInterfaceFn,
    b_get_callback: Option<SysVBGetCallbackFn>,
    free_last_callback: Option<SysVFreeLastCallbackFn>,
    get_api_call_result: Option<SysVGetAPICallResultFn>,
    release_thread_local_memory: Option<SysVReleaseThreadLocalMemoryFn>,
}

unsafe impl Send for NativeSteamClient {}
unsafe impl Sync for NativeSteamClient {}

static NATIVE_STEAM_CLIENT: OnceLock<Option<NativeSteamClient>> = OnceLock::new();

pub fn is_bridge_enabled() -> bool {
    STEAM_BRIDGE_ENABLED.load(Ordering::SeqCst)
}

pub fn is_real_steam_dll_found() -> bool {
    STEAM_REAL_DLL_FOUND.load(Ordering::SeqCst)
}

pub fn is_offline_forced() -> bool {
    STEAM_OFFLINE_FORCED.load(Ordering::SeqCst)
}

/// Searches for the game's original `steam_api64.dll` next to the executable
/// or inside Unity/Unreal plugin subdirectories.
pub fn detect_game_steam_api_dll(exe_path: &str) -> Option<PathBuf> {
    let exe = Path::new(exe_path);
    let exe_dir = exe.parent()?;

    let direct_candidates = [
        exe_dir.join("steam_api64.dll"),
        exe_dir.join("Plugins").join("x86_64").join("steam_api64.dll"),
        exe_dir.join("Plugins").join("steam_api64.dll"),
    ];
    for c in &direct_candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }

    if let Ok(entries) = std::fs::read_dir(exe_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with("_Data") {
                    let p_x64 = p.join("Plugins").join("x86_64").join("steam_api64.dll");
                    if p_x64.is_file() {
                        return Some(p_x64);
                    }
                    let p_flat = p.join("Plugins").join("steam_api64.dll");
                    if p_flat.is_file() {
                        return Some(p_flat);
                    }
                }
            }
        }
    }

    None
}

fn find_linux_steamclient_so() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let candidates = [
        PathBuf::from(format!("{}/.local/share/Steam/linux64/steamclient.so", home)),
        PathBuf::from(format!("{}/.steam/sdk64/steamclient.so", home)),
        PathBuf::from(format!("{}/.steam/steam/linux64/steamclient.so", home)),
        PathBuf::from("/usr/lib/steam/steamclient.so"),
        PathBuf::from("/usr/lib64/steam/steamclient.so"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn load_native_steamclient() -> Option<&'static NativeSteamClient> {
    NATIVE_STEAM_CLIENT
        .get_or_init(|| {
            let so_path = find_linux_steamclient_so()?;
            let c_path = CString::new(so_path.to_string_lossy().as_bytes()).ok()?;
            unsafe {
                let handle = libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
                if handle.is_null() {
                    let err = libc::dlerror();
                    let err_msg = if err.is_null() {
                        "unknown dlopen error".to_string()
                    } else {
                        CStr::from_ptr(err).to_string_lossy().into_owned()
                    };
                    eprintln!(
                        "[RINE STEAM BRIDGE] Failed to dlopen {:?}: {}",
                        so_path, err_msg
                    );
                    return None;
                }

                let sym_ci = libc::dlsym(handle, b"CreateInterface\0".as_ptr() as *const c_char);
                if sym_ci.is_null() {
                    eprintln!("[RINE STEAM BRIDGE] CreateInterface not found in steamclient.so");
                    return None;
                }

                let sym_bget =
                    libc::dlsym(handle, b"Steam_BGetCallback\0".as_ptr() as *const c_char);
                let sym_free =
                    libc::dlsym(handle, b"Steam_FreeLastCallback\0".as_ptr() as *const c_char);
                let sym_apicall =
                    libc::dlsym(handle, b"Steam_GetAPICallResult\0".as_ptr() as *const c_char);
                let sym_rel_tls = libc::dlsym(
                    handle,
                    b"Steam_ReleaseThreadLocalMemory\0".as_ptr() as *const c_char,
                );

                println!(
                    "[RINE STEAM BRIDGE] Loaded native Linux steamclient.so from {:?}",
                    so_path
                );

                Some(NativeSteamClient {
                    _handle: handle,
                    create_interface: std::mem::transmute(sym_ci),
                    b_get_callback: if sym_bget.is_null() {
                        None
                    } else {
                        Some(std::mem::transmute(sym_bget))
                    },
                    free_last_callback: if sym_free.is_null() {
                        None
                    } else {
                        Some(std::mem::transmute(sym_free))
                    },
                    get_api_call_result: if sym_apicall.is_null() {
                        None
                    } else {
                        Some(std::mem::transmute(sym_apicall))
                    },
                    release_thread_local_memory: if sym_rel_tls.is_null() {
                        None
                    } else {
                        Some(std::mem::transmute(sym_rel_tls))
                    },
                })
            }
        })
        .as_ref()
}

/// Initializes Steam bridge policy for the target executable.
/// Checks for an authentic `steam_api64.dll` next to the game and connects
/// to Valve's native `steamclient.so` when available.
pub fn init_steam_bridge(exe_path: &str, offline_flag: bool) {
    STEAM_OFFLINE_FORCED.store(offline_flag, Ordering::SeqCst);

    match detect_game_steam_api_dll(exe_path) {
        Some(dll_path) => {
            STEAM_REAL_DLL_FOUND.store(true, Ordering::SeqCst);
            println!(
                "[RINE STEAM] Found original steam_api64.dll: {:?}",
                dll_path
            );

            if offline_flag {
                STEAM_BRIDGE_ENABLED.store(false, Ordering::SeqCst);
                println!("[RINE STEAM] --steam-offline enabled: using local offline Steamworks compatibility stub.");
                return;
            }

            if load_native_steamclient().is_some() {
                STEAM_BRIDGE_ENABLED.store(true, Ordering::SeqCst);
                println!("[RINE STEAM BRIDGE] Real Steamworks bridge enabled (steam_api64.dll -> steamclient64.dll -> steamclient.so).");
            } else {
                STEAM_BRIDGE_ENABLED.store(false, Ordering::SeqCst);
                println!("[RINE STEAM BRIDGE] Native steamclient.so not found (Steam client not installed). Falling back to offline stub.");
            }
        }
        None => {
            STEAM_REAL_DLL_FOUND.store(false, Ordering::SeqCst);
            STEAM_BRIDGE_ENABLED.store(false, Ordering::SeqCst);
            println!("[RINE STEAM] ВНИМАНИЕ: Оригинальный steam_api64.dll не найден рядом с игрой.");
            println!("[RINE STEAM] Игра не является официальной сборкой Steam (или не использует Steamworks).");
            println!("[RINE STEAM] Rine не несёт ответственности за запуск сторонних или нелицензионных сборок!");
        }
    }
}

// ---------------------------------------------------------------------------
// Win64 <-> SysV AMD64 C++ VTable Proxy Trampolines
// ---------------------------------------------------------------------------

const MAX_VTABLE_SLOTS: usize = 128;

#[repr(C)]
struct WrappedInterface {
    win64_vtable: *const usize,
    real_obj: *mut c_void,
    real_vtable: *const usize,
}

static TRAMPOLINE_VTABLE: OnceLock<[usize; MAX_VTABLE_SLOTS]> = OnceLock::new();
static STEAMCLIENT_VTABLE: OnceLock<[usize; MAX_VTABLE_SLOTS]> = OnceLock::new();
static WRAPPED_CACHE: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

/// Emits a 128-slot array of executable x86_64 Win64-to-SysV thunks.
///
/// Each thunk expects:
///   RCX = WrappedInterface* (Win64 `this`)
///   RDX = arg1, R8 = arg2, R9 = arg3, [RSP+0x28] = arg4, [RSP+0x30] = arg5
/// Converts to SysV AMD64:
///   RDI = real_obj ([RCX+8]), RSI = arg1, RDX = arg2, RCX = arg3, R8 = arg4, R9 = arg5
/// Calls `real_vtable[slot]` and preserves Win64 callee-saved RDI/RSI.
fn get_generic_trampoline_vtable() -> &'static [usize; MAX_VTABLE_SLOTS] {
    TRAMPOLINE_VTABLE.get_or_init(|| unsafe {
        let code_per_slot = 64usize;
        let total_size = MAX_VTABLE_SLOTS * code_per_slot;
        let mem = libc::mmap(
            std::ptr::null_mut(),
            total_size,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        assert_ne!(mem, libc::MAP_FAILED, "Failed to mmap Win64->SysV vtable thunks");

        let mut table = [0usize; MAX_VTABLE_SLOTS];
        let base = mem as *mut u8;

        for slot in 0..MAX_VTABLE_SLOTS {
            let p = base.add(slot * code_per_slot);
            table[slot] = p as usize;
            let mut idx = 0usize;
            let mut emit = |bytes: &[u8]| {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), p.add(idx), bytes.len());
                idx += bytes.len();
            };

            // push rdi; push rsi; sub rsp, 8
            emit(&[0x57, 0x56, 0x48, 0x83, 0xEC, 0x08]);
            // mov r11, [rcx + 16] (real_vtable)
            emit(&[0x4C, 0x8B, 0x59, 0x10]);
            // mov r11, [r11 + slot*8]
            let offset = (slot * 8) as i32;
            emit(&[0x4D, 0x8B, 0x9B]);
            emit(&offset.to_le_bytes());
            // mov rdi, [rcx + 8] (real_obj)
            emit(&[0x48, 0x8B, 0x79, 0x08]);
            // mov rsi, rdx (arg1)
            emit(&[0x48, 0x89, 0xD6]);
            // mov rdx, r8  (arg2)
            emit(&[0x4C, 0x89, 0xC2]);
            // mov rcx, r9  (arg3)
            emit(&[0x4C, 0x89, 0xC9]);
            // mov r8, [rsp + 0x40] (arg4: 0x28 + 24)
            emit(&[0x4C, 0x8B, 0x44, 0x24, 0x40]);
            // mov r9, [rsp + 0x48] (arg5: 0x30 + 24)
            emit(&[0x4C, 0x8B, 0x4C, 0x24, 0x48]);
            // call r11
            emit(&[0x41, 0xFF, 0xD3]);
            // add rsp, 8; pop rsi; pop rdi; ret
            emit(&[0x48, 0x83, 0xC4, 0x08, 0x5E, 0x5F, 0xC3]);
        }

        table
    })
}

/// Wraps a SysV C++ interface pointer from `steamclient.so` into a Win64-compatible proxy.
unsafe fn wrap_sysv_interface(real_obj: *mut c_void, is_steam_client: bool) -> *mut c_void {
    if real_obj.is_null() {
        return std::ptr::null_mut();
    }
    let cache = WRAPPED_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&existing) = guard.get(&(real_obj as usize)) {
        return existing as *mut c_void;
    }

    let real_vtable = *(real_obj as *const *const usize);
    let win64_vtable = if is_steam_client {
        get_steamclient_vtable().as_ptr()
    } else {
        get_generic_trampoline_vtable().as_ptr()
    };

    let boxed = Box::new(WrappedInterface {
        win64_vtable,
        real_obj,
        real_vtable,
    });
    let ptr = Box::into_raw(boxed) as *mut c_void;
    guard.insert(real_obj as usize, ptr as usize);
    ptr
}

/// Special Win64 vtable for `ISteamClient`: delegates to generic trampolines,
/// except methods returning sub-interfaces (`GetISteam*` / `GetISteamGenericInterface`)
/// automatically wrap the returned SysV interface pointer.
fn get_steamclient_vtable() -> &'static [usize; MAX_VTABLE_SLOTS] {
    STEAMCLIENT_VTABLE.get_or_init(|| {
        let mut table = *get_generic_trampoline_vtable();
        // ISteamClient vtable slots that return `ISteam*` interface pointers:
        // 5: GetISteamUser, 6: GetISteamGameServer, 8: GetISteamFriends,
        // 9: GetISteamUtils, 10: GetISteamMatchmaking, 11: GetISteamMatchmakingServers,
        // 12: GetISteamUserStats, 13: GetISteamGameServerStats, 14: GetISteamApps,
        // 15: GetISteamNetworking, 16: GetISteamRemoteStorage, 17: GetISteamScreenshots,
        // 18: GetISteamGameSearch, 20: GetIPCCallCount (skip),
        // 23: GetISteamHTTP, 24: GetISteamController, 25: GetISteamUGC,
        // 26: GetISteamMusic, 27: GetISteamMusicRemote, 28: GetISteamHTMLSurface,
        // 31: GetISteamInventory, 32: GetISteamVideo, 33: GetISteamParentalSettings,
        // 34: GetISteamInput, 35: GetISteamParties, 36: GetISteamRemotePlay
        // Plus slot 12/22 `GetISteamGenericInterface` depending on SDK version.
        unsafe extern "win64" fn wrap_ret_3_args(
            proxy: *mut WrappedInterface,
            a1: usize,
            a2: usize,
            a3: usize,
            slot: usize,
        ) -> *mut c_void {
            let real_obj = (*proxy).real_obj;
            let real_vt = (*proxy).real_vtable;
            let fn_ptr: unsafe extern "C" fn(*mut c_void, usize, usize, usize) -> *mut c_void =
                std::mem::transmute(*real_vt.add(slot));
            let ret = fn_ptr(real_obj, a1, a2, a3);
            wrap_sysv_interface(ret, false)
        }

        macro_rules! make_iface_getter {
            ($slot:expr) => {{
                unsafe extern "win64" fn thunk(
                    proxy: *mut WrappedInterface,
                    a1: usize,
                    a2: usize,
                    a3: usize,
                ) -> *mut c_void {
                    wrap_ret_3_args(proxy, a1, a2, a3, $slot)
                }
                thunk as *const () as usize
            }};
        }

        table[5] = make_iface_getter!(5);
        table[6] = make_iface_getter!(6);
        table[8] = make_iface_getter!(8);
        table[9] = make_iface_getter!(9);
        table[10] = make_iface_getter!(10);
        table[11] = make_iface_getter!(11);
        table[12] = make_iface_getter!(12);
        table[13] = make_iface_getter!(13);
        table[14] = make_iface_getter!(14);
        table[15] = make_iface_getter!(15);
        table[16] = make_iface_getter!(16);
        table[17] = make_iface_getter!(17);
        table[18] = make_iface_getter!(18);
        table[22] = make_iface_getter!(22);
        table[23] = make_iface_getter!(23);
        table[24] = make_iface_getter!(24);
        table[25] = make_iface_getter!(25);
        table[26] = make_iface_getter!(26);
        table[27] = make_iface_getter!(27);
        table[28] = make_iface_getter!(28);
        table[29] = make_iface_getter!(29);
        table[30] = make_iface_getter!(30);
        table[31] = make_iface_getter!(31);
        table[32] = make_iface_getter!(32);
        table[33] = make_iface_getter!(33);
        table[34] = make_iface_getter!(34);
        table[35] = make_iface_getter!(35);
        table[36] = make_iface_getter!(36);
        table[37] = make_iface_getter!(37);

        table
    })
}

// ---------------------------------------------------------------------------
// Exported `steamclient64.dll` Win64 Functions
// ---------------------------------------------------------------------------

pub unsafe extern "win64" fn steamclient64_CreateInterface(
    p_name: *const c_char,
    p_return_code: *mut i32,
) -> *mut c_void {
    let Some(client) = load_native_steamclient() else {
        if !p_return_code.is_null() {
            *p_return_code = 1;
        }
        return std::ptr::null_mut();
    };
    let name_str = if p_name.is_null() {
        "<null>"
    } else {
        CStr::from_ptr(p_name).to_str().unwrap_or("<utf8_err>")
    };
    let real_obj = (client.create_interface)(p_name, p_return_code);
    let is_client = name_str.starts_with("SteamClient");
    let wrapped = wrap_sysv_interface(real_obj, is_client);
    println!(
        "[RINE STEAM BRIDGE] CreateInterface('{}') -> real={:?} proxy={:?}",
        name_str, real_obj, wrapped
    );
    wrapped
}

pub unsafe extern "win64" fn steamclient64_Steam_BGetCallback(
    h_steam_pipe: i32,
    p_callback_msg: *mut c_void,
) -> u32 {
    if let Some(client) = load_native_steamclient() {
        if let Some(f) = client.b_get_callback {
            return f(h_steam_pipe, p_callback_msg) as u32;
        }
    }
    0
}

pub unsafe extern "win64" fn steamclient64_Steam_FreeLastCallback(h_steam_pipe: i32) {
    if let Some(client) = load_native_steamclient() {
        if let Some(f) = client.free_last_callback {
            f(h_steam_pipe);
        }
    }
}

pub unsafe extern "win64" fn steamclient64_Steam_GetAPICallResult(
    h_steam_pipe: i32,
    h_steam_api_call: u64,
    p_callback: *mut c_void,
    cub_callback: i32,
    i_callback_expected: i32,
    pb_failed: *mut u8,
) -> u32 {
    if let Some(client) = load_native_steamclient() {
        if let Some(f) = client.get_api_call_result {
            let mut local_failed: u8 = 0;
            let ok = f(
                h_steam_pipe,
                h_steam_api_call,
                p_callback,
                cub_callback,
                i_callback_expected,
                &mut local_failed as *mut u8,
            );
            if !pb_failed.is_null() {
                std::ptr::write_unaligned(pb_failed as *mut u32, local_failed as u32);
            }
            return ok as u32;
        }
    }
    0
}

pub unsafe extern "win64" fn steamclient64_Steam_ReleaseThreadLocalMemory(b_thread_exit: i32) {
    if let Some(client) = load_native_steamclient() {
        if let Some(f) = client.release_thread_local_memory {
            f(b_thread_exit);
        }
    }
}

/// Resolves `steamclient64.dll` exported functions to the native Linux Steam bridge.
pub fn resolve_steamclient_symbol(name: &str) -> Option<usize> {
    match name {
        "CreateInterface" => Some(steamclient64_CreateInterface as *const () as usize),
        "Steam_BGetCallback" => Some(steamclient64_Steam_BGetCallback as *const () as usize),
        "Steam_FreeLastCallback" => {
            Some(steamclient64_Steam_FreeLastCallback as *const () as usize)
        }
        "Steam_GetAPICallResult" => {
            Some(steamclient64_Steam_GetAPICallResult as *const () as usize)
        }
        "Steam_ReleaseThreadLocalMemory" => {
            Some(steamclient64_Steam_ReleaseThreadLocalMemory as *const () as usize)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trampoline_vtable_generation() {
        let vt = get_generic_trampoline_vtable();
        assert_ne!(vt[0], 0);
        assert_ne!(vt[127], 0);
        assert_eq!(vt[1] - vt[0], 64);
    }

    #[test]
    fn test_detect_missing_steam_dll_warns_cleanly() {
        assert!(detect_game_steam_api_dll("/tmp/nonexistent_game_dir/game.exe").is_none());
    }
}
