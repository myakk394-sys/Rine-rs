pub mod stats;
pub mod wine;

pub use stats::*;
use std::sync::Mutex;
pub use wine::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiProviderKind {
    NativeRine,
    WineAdapter,
}

#[derive(Debug, Clone)]
pub struct ResolvedApi {
    pub address: u64,
    pub provider: ApiProviderKind,
    pub module: String,
    pub symbol: String,
}

pub trait ApiProvider {
    fn name(&self) -> &'static str;
    fn resolve(&self, dll_name: &str, symbol_name: &str) -> Option<ResolvedApi>;
}

pub static GLOBAL_WINE_CONFIG: Mutex<Option<WineConfig>> = Mutex::new(None);

pub fn set_global_wine_config(config: WineConfig) {
    if config.enabled {
        let dll_path_str = config
            .dll_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "auto-detected".to_string());
        let version = detect_wine_version();
        println!(
            "[RINE WINE PROVIDER]\nenabled=true\ndll_path={}\nwine_version={}\narchitecture=x86_64",
            dll_path_str, version
        );
    } else {
        println!("[RINE WINE PROVIDER]\nenabled=false");
    }
    let mut guard = GLOBAL_WINE_CONFIG.lock().unwrap();
    *guard = Some(config);
}

pub fn get_global_wine_config() -> WineConfig {
    let guard = GLOBAL_WINE_CONFIG.lock().unwrap();
    guard.as_ref().cloned().unwrap_or_default()
}

pub struct RineApiResolver;

impl RineApiResolver {
    pub fn resolve_symbol(dll_name: &str, symbol_name: &str) -> Option<ResolvedApi> {
        // Priority 1: Native Rine implementation
        if let Some(addr) =
            crate::pe::imports::IatSymbolResolver::resolve_symbol(dll_name, symbol_name)
        {
            record_native_call(dll_name, symbol_name);
            return Some(ResolvedApi {
                address: addr as u64,
                provider: ApiProviderKind::NativeRine,
                module: dll_name.to_string(),
                symbol: symbol_name.to_string(),
            });
        }

        // Priority 1.5: Application Companion PE DLL (e.g. UnityPlayer.dll, GameAssembly.dll)
        if let Some(comp_addr) =
            crate::mem::loader::LoadedModule::lookup_companion_export(dll_name, symbol_name)
        {
            record_native_call(dll_name, symbol_name);
            println!(
                "[RINE COMPANION RESOLUTION] dll='{}' symbol='{}' address=0x{:016X}",
                dll_name, symbol_name, comp_addr
            );
            return Some(ResolvedApi {
                address: comp_addr as u64,
                provider: ApiProviderKind::NativeRine,
                module: dll_name.to_string(),
                symbol: symbol_name.to_string(),
            });
        }

        // Priority 2: Wine Adapter provider (if enabled & allowlisted)
        let config = get_global_wine_config();
        if config.enabled {
            if let Some(wine_addr) = resolve_wine_export(&config, dll_name, symbol_name) {
                record_wine_call(dll_name, symbol_name);
                return Some(ResolvedApi {
                    address: wine_addr as u64,
                    provider: ApiProviderKind::WineAdapter,
                    module: dll_name.to_string(),
                    symbol: symbol_name.to_string(),
                });
            }
        }

        // Priority 3: Diagnostic fail-fast
        record_unresolved_call(dll_name, symbol_name);
        None
    }
}

pub static STATS_TEST_MUTEX: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_prefers_native_rine() {
        let _guard = STATS_TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        set_global_wine_config(WineConfig {
            enabled: true,
            dll_path: find_wine_dll_dir(),
            allowed_modules: vec!["kernel32.dll".into()],
            allowed_symbols: Vec::new(),
        });

        let res = RineApiResolver::resolve_symbol("kernel32.dll", "HeapAlloc");
        assert!(res.is_some());
        let api = res.unwrap();
        assert_eq!(api.provider, ApiProviderKind::NativeRine);
    }

    #[test]
    fn provider_uses_wine_when_native_missing() {
        let _guard = STATS_TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let wine_dir = match find_wine_dll_dir() {
            Some(d) => d,
            None => {
                println!("[SKIP] Wine is not installed locally");
                return;
            }
        };

        set_global_wine_config(WineConfig {
            enabled: true,
            dll_path: Some(wine_dir),
            allowed_modules: vec!["version.dll".into()],
            allowed_symbols: Vec::new(),
        });

        let res = RineApiResolver::resolve_symbol("version.dll", "GetFileVersionInfoW");
        assert!(res.is_some());
        let api = res.unwrap();
        assert_eq!(api.provider, ApiProviderKind::WineAdapter);
    }

    #[test]
    fn provider_fails_when_both_missing() {
        let _guard = STATS_TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        set_global_wine_config(WineConfig {
            enabled: true,
            dll_path: find_wine_dll_dir(),
            allowed_modules: vec!["kernel32.dll".into()],
            allowed_symbols: Vec::new(),
        });

        let res = RineApiResolver::resolve_symbol("nonexistent.dll", "NonExistentFunction1234");
        assert!(res.is_none());
    }

    #[test]
    fn wine_provider_disabled_by_default() {
        let _guard = STATS_TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        set_global_wine_config(WineConfig {
            enabled: false,
            dll_path: find_wine_dll_dir(),
            allowed_modules: vec!["wine_unimplemented.dll".into()],
            allowed_symbols: Vec::new(),
        });

        let res = RineApiResolver::resolve_symbol("wine_unimplemented.dll", "WineOnlySymbol");
        assert!(res.is_none());
    }

    #[test]
    fn wine_provider_allowlist_enforced() {
        let _guard = STATS_TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        set_global_wine_config(WineConfig {
            enabled: true,
            dll_path: find_wine_dll_dir(),
            allowed_modules: vec!["other.dll".into()],
            allowed_symbols: Vec::new(),
        });

        let res = RineApiResolver::resolve_symbol("wine_unimplemented.dll", "WineOnlySymbol");
        assert!(res.is_none());
    }

    #[test]
    fn wine_adapter_poc_load_and_call_export() {
        let handle = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                let wine_dir = match find_wine_dll_dir() {
                    Some(d) => d,
                    None => {
                        println!("[SKIP] Wine is not installed locally");
                        return;
                    }
                };

                let target_dll_path = wine_dir.join("avrt.dll");
                if !target_dll_path.exists() {
                    println!("[SKIP] avrt.dll not found in {:?}", wine_dir);
                    return;
                }

                let config = WineConfig {
                    enabled: true,
                    dll_path: Some(wine_dir.clone()),
                    allowed_modules: vec!["avrt.dll".into()],
                    allowed_symbols: Vec::new(),
                };

                let loaded = load_and_map_wine_pe_dll(&target_dll_path, &config)
                    .expect("PE loader failed to map avrt.dll");

                println!(
                    "[RINE WINE POC] Loaded avrt.dll @ Base 0x{:016X}",
                    loaded.image_base
                );

                let export_set_addr = *loaded
                    .exports
                    .get("AvSetMmThreadCharacteristicsA")
                    .expect("AvSetMmThreadCharacteristicsA export not found in avrt.dll");

                let export_revert_addr = *loaded
                    .exports
                    .get("AvRevertMmThreadCharacteristics")
                    .expect("AvRevertMmThreadCharacteristics export not found in avrt.dll");

                println!(
                    "[RINE WINE POC] Found exports AvSetMmThreadCharacteristicsA @ 0x{:016X}, AvRevertMmThreadCharacteristics @ 0x{:016X}",
                    export_set_addr, export_revert_addr
                );

                let mut task_idx: u32 = 0;
                let task_name = "Audio\0";
                let handle = unsafe {
                    call_win64_export_safely(
                        export_set_addr,
                        task_name.as_ptr() as usize,
                        &mut task_idx as *mut u32 as usize,
                        0,
                        0,
                    )
                };

                println!(
                    "[RINE WINE POC] AvSetMmThreadCharacteristicsA('Audio') returned handle = 0x{:016X}, task_idx = {}",
                    handle, task_idx
                );

                assert!(
                    handle != 0,
                    "AvSetMmThreadCharacteristicsA returned NULL handle"
                );

                let revert_res =
                    unsafe { call_win64_export_safely(export_revert_addr, handle, 0, 0, 0) };

                println!(
                    "[RINE WINE POC] AvRevertMmThreadCharacteristics(0x{:016X}) returned status = {}",
                    handle, revert_res
                );

                assert_ne!(revert_res, 0, "AvRevertMmThreadCharacteristics failed");
            })
            .unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn provider_stats_counts_native_calls() {
        let _guard = STATS_TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        reset_stats();
        record_native_call("kernel32.dll", "TestNativeSym");
        let (n_calls, n_syms, _, _, _, _) = get_stats();
        assert_eq!(n_calls, 1);
        assert_eq!(n_syms, 1);
    }

    #[test]
    fn provider_stats_counts_wine_calls() {
        let _guard = STATS_TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        reset_stats();
        record_wine_call("version.dll", "TestWineSym");
        let (_, _, w_calls, w_syms, _, _) = get_stats();
        assert_eq!(w_calls, 1);
        assert_eq!(w_syms, 1);
    }

    #[test]
    fn provider_stats_counts_unresolved() {
        let _guard = STATS_TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        reset_stats();
        record_unresolved_call("unknown.dll", "TestUnresolvedSym");
        let (_, _, _, _, u_calls, u_syms) = get_stats();
        assert_eq!(u_calls, 1);
        assert_eq!(u_syms, 1);
    }

    #[test]
    fn wine_provider_abi_boundary_preserves_gprs() {
        let fn_ptr = dummy_abi_callee as *const () as usize;
        let res = unsafe { call_win64_export_safely(fn_ptr, 42, 100, 200, 300) };
        assert_eq!(res, 42);
    }
}

extern "win64" fn dummy_abi_callee(val: usize) -> usize {
    val
}
