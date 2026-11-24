//! DXVK PE DLL Loader
//!
//! Loads DXVK's d3d11.dll and dxgi.dll as guest Windows PE DLLs inside Rine's
//! address space. Once loaded, their exports replace empty D3D11/DXGI stubs in
//! the game's IAT, giving the game real D3D11 → Vulkan rendering via DXVK.
//!
//! Architecture (same as Proton):
//!   Game IAT → DXVK d3d11.dll (PE inside Rine) → vulkan-1.dll (our bridge) → libvulkan.so.1 → GPU

use std::collections::HashMap;
use std::sync::OnceLock;

/// Forwards `vulkan-1.dll` symbol requests from DXVK to Vulkan Bridge.
pub fn resolve_vulkan_symbol(fn_name: &str) -> Option<usize> {
    crate::graphics::vulkan_bridge::resolve_vulkan_symbol(fn_name)
}

/// Returns dynamic DXVK search paths, prioritizing bundled/standalone Rine DXVK
/// before falling back to system-wide or external compatibility tool paths.
fn get_dxvk_search_paths() -> Vec<String> {
    let mut paths = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());

    // 1. Standalone bundled DXVK next to executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths.push(exe_dir.join("assets/dxvk").to_string_lossy().into_owned());
            paths.push(exe_dir.join("dxvk").to_string_lossy().into_owned());
        }
    }

    // 2. Repository / current working directory bundled DXVK
    paths.push("assets/dxvk".to_string());

    // 3. Rine user-local standalone DXVK directory
    paths.push(format!("{}/.local/share/rine/dxvk", home));

    // 4. System-wide DXVK packages
    paths.push("/usr/share/dxvk/x64".to_string());
    paths.push("/usr/share/dxvk".to_string());
    paths.push("/opt/dxvk/x64".to_string());
    paths.push(format!("{}/.local/share/dxvk/x64", home));

    // 5. Optional fallback: Steam Proton / Bottles / Heroic (only if standalone is absent)
    paths.push(format!("{}/.local/share/Steam/steamapps/common/Proton - Experimental/files/lib/wine/dxvk/x86_64-windows", home));
    paths.push(format!("{}/.local/share/Steam/steamapps/common/Proton 9.0 (Beta)/files/lib/wine/dxvk/x86_64-windows", home));
    paths.push(format!("{}/.local/share/Steam/steamapps/common/Proton 8.0/files/lib/wine/dxvk/x86_64-windows", home));
    paths.push(format!("{}/.steam/steam/steamapps/common/Proton - Experimental/files/lib/wine/dxvk/x86_64-windows", home));
    paths.push(format!("{}/.var/app/com.usebottles.bottles/data/bottles/dxvk/dxvk-2.7.1/x64", home));
    paths.push(format!("{}/.var/app/com.heroicgameslauncher.hgl/config/heroic/tools/dxvk/dxvk-3.0.2/x64", home));
    paths.push(format!("{}/.var/app/com.heroicgameslauncher.hgl/config/heroic/tools/dxvk/dxvk-2.7.1/x64", home));
    paths.push(format!("{}/snap/steam/common/.local/share/Steam/steamapps/common/Proton - Experimental/files/lib/wine/dxvk/x86_64-windows", home));

    paths
}

/// Ensures bundled DXVK DLLs from `assets/dxvk` are deployed to `~/.local/share/rine/dxvk`
/// if not already present, so installed `rine` binaries work standalone without Proton.
fn ensure_standalone_dxvk_deployed(source_dir: &str) {
    let home = match std::env::var("HOME") {
        Ok(h) if !h.is_empty() => h,
        _ => return,
    };
    let target_dir = std::path::PathBuf::from(format!("{}/.local/share/rine/dxvk", home));
    if target_dir.join("dxgi.dll").exists() && target_dir.join("d3d11.dll").exists() {
        return;
    }
    if std::fs::create_dir_all(&target_dir).is_ok() {
        if let Ok(entries) = std::fs::read_dir(source_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("dll") {
                    if let Some(name) = path.file_name() {
                        let _ = std::fs::copy(&path, target_dir.join(name));
                    }
                }
            }
        }
    }
}

/// A DXVK DLL loaded as a guest PE image. Exports are resolved from the
/// PE export table and stored by name → virtual address.
pub struct DxvkPeModule {
    pub image_base: usize,
    pub image_size: usize,
    /// export name → virtual address (absolute, in Rine's process memory)
    pub exports: HashMap<String, usize>,
}

static DXVK_DIR: OnceLock<String> = OnceLock::new();
static DXVK_DXGI: OnceLock<Option<DxvkPeModule>> = OnceLock::new();
static DXVK_D3D11: OnceLock<Option<DxvkPeModule>> = OnceLock::new();

/// Finds and stores the DXVK directory path.
pub fn find_dxvk_path() -> Option<&'static str> {
    DXVK_DIR.get_or_init(|| {
        if let Ok(env_path) = std::env::var("RINE_DXVK_PATH") {
            let dxgi = format!("{}/dxgi.dll", env_path);
            if std::path::Path::new(&dxgi).exists() {
                println!("[DXVK LOADER] Using RINE_DXVK_PATH: {}", env_path);
                return env_path;
            }
        }
        for path in get_dxvk_search_paths() {
            let p = std::path::Path::new(&path);
            if p.exists() && p.join("dxgi.dll").exists() && p.join("d3d11.dll").exists() {
                if path.ends_with("assets/dxvk") {
                    ensure_standalone_dxvk_deployed(&path);
                }
                println!("[DXVK LOADER] Found DXVK at: {}", path);
                return path;
            }
        }
        println!("[DXVK LOADER] DXVK not found in any search path");
        String::new()
    });
    let s = DXVK_DIR.get().unwrap();
    if s.is_empty() {
        None
    } else {
        Some(s.as_str())
    }
}

/// Resolves a symbol from a loaded DXVK module.
/// Returns Some(virtual_address) if the symbol is found.
pub fn resolve_dxvk_symbol(dll_name_lower: &str, fn_name: &str) -> Option<usize> {
    let module = match dll_name_lower {
        "dxgi.dll" | "dxgi" => DXVK_DXGI.get()?.as_ref()?,
        "d3d11.dll" | "d3d11" => DXVK_D3D11.get()?.as_ref()?,
        "d3d10core.dll" | "d3d10core" => DXVK_D3D11.get()?.as_ref()?,
        _ => return None,
    };
    module.exports.get(fn_name).copied()
}

/// Preloads DXVK DLLs into Rine's address space.
/// Must be called before jump_to_win64_entry_point.
pub fn preload_dxvk(dxvk_dir: &str) {
    DXVK_DXGI.get_or_init(|| load_dxvk_dll(dxvk_dir, "dxgi.dll"));
    DXVK_D3D11.get_or_init(|| load_dxvk_dll(dxvk_dir, "d3d11.dll"));
}

fn load_dxvk_dll(dxvk_dir: &str, dll_name: &str) -> Option<DxvkPeModule> {
    let path = format!("{}/{}", dxvk_dir, dll_name);
    println!("[DXVK LOADER] Loading {} from {}", dll_name, path);

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            println!("[DXVK LOADER] Failed to read {}: {}", path, e);
            return None;
        }
    };

    // Use DiskDemand strategy: allocates anonymous memory at any address (no preferred base).
    // This avoids conflicts with the game's fixed ImageBase (0x140000000).
    // DXVK has its own preferred bases (0x2EDF00000, 0x359050000) which we ignore;
    // the .reloc section takes care of fixups.
    let config = crate::mem::strategy::LoadConfig {
        strategy: crate::mem::strategy::StorageStrategy::DiskDemand,
        enable_prefetch: false,
    };

    let module = match crate::mem::loader::LoadedModule::load(&path, &bytes, config) {
        Ok(m) => m,
        Err(e) => {
            println!(
                "[DXVK LOADER] Failed to load {} into memory: {:?}",
                dll_name, e
            );
            return None;
        }
    };

    // Parse exports from the in-memory image to build our symbol table.
    // At this point base relocations have been applied, so RVAs → correct VAs.
    let exports = parse_pe_exports(module.image_base, &bytes);
    println!(
        "[DXVK LOADER] {} loaded @ 0x{:016X}, {} exports",
        dll_name,
        module.image_base,
        exports.len()
    );
    for (name, addr) in exports.iter().take(6) {
        println!("  [DXVK] {}!{} @ 0x{:016X}", dll_name, name, addr);
    }

    if module.entry_point != 0 {
        unsafe {
            let dll_main: unsafe extern "win64" fn(usize, u32, usize) -> i32 =
                std::mem::transmute(module.entry_point);
            println!(
                "[DXVK LOADER] Invoking DllMain for {} @ 0x{:016X}",
                dll_name, module.entry_point
            );
            let res = dll_main(module.image_base, 1 /* DLL_PROCESS_ATTACH */, 0);
            println!("[DXVK LOADER] DllMain for {} returned {}", dll_name, res);
        }
    }

    let result = DxvkPeModule {
        image_base: module.image_base,
        image_size: module.size_of_image,
        exports,
    };

    // Leak the LoadedModule so its mmap region stays alive permanently
    std::mem::forget(module);
    Some(result)
}

/// Parses the PE export directory to extract function name → RVA mappings,
/// then translates RVAs to absolute VAs using the actual loaded image_base.
fn parse_pe_exports(image_base: usize, raw_pe_bytes: &[u8]) -> HashMap<String, usize> {
    let mut exports = HashMap::new();

    // Parse PE headers from raw bytes to find export directory RVA
    let pe = match crate::pe::PeImage::parse(raw_pe_bytes) {
        Ok(p) => p,
        Err(_) => return exports,
    };

    // Export directory is data_directory[0]
    let export_dir_entry = &pe.optional_header.data_directory[0];
    let export_rva = export_dir_entry.virtual_address() as usize;
    let export_size = export_dir_entry.size() as usize;

    if export_rva == 0 || export_size == 0 {
        return exports;
    }

    // The DLL is now loaded at `image_base` in memory
    let image_size = pe.optional_header.size_of_image() as usize;
    if export_rva + 40 > image_size {
        return exports;
    }

    // Read export directory from loaded image memory
    unsafe {
        let base = image_base as *const u8;

        // IMAGE_EXPORT_DIRECTORY layout:
        // +0  Characteristics      u32
        // +4  TimeDateStamp        u32
        // +8  MajorVersion         u16
        // +10 MinorVersion         u16
        // +12 Name                 u32 (RVA)
        // +16 Base                 u32
        // +20 NumberOfFunctions    u32
        // +24 NumberOfNames        u32
        // +28 AddressOfFunctions   u32 (RVA to array of u32 RVAs)
        // +32 AddressOfNames       u32 (RVA to array of u32 RVAs)
        // +36 AddressOfNameOrdinals u32 (RVA to array of u16)

        let exp_ptr = base.add(export_rva);
        let num_names = std::ptr::read_unaligned(exp_ptr.add(24) as *const u32) as usize;
        let fns_rva = std::ptr::read_unaligned(exp_ptr.add(28) as *const u32) as usize;
        let names_rva = std::ptr::read_unaligned(exp_ptr.add(32) as *const u32) as usize;
        let ords_rva = std::ptr::read_unaligned(exp_ptr.add(36) as *const u32) as usize;

        if names_rva == 0 || fns_rva == 0 || ords_rva == 0 {
            return exports;
        }
        if names_rva >= image_size || fns_rva >= image_size || ords_rva >= image_size {
            return exports;
        }

        let names_ptr = base.add(names_rva) as *const u32;
        let fns_ptr = base.add(fns_rva) as *const u32;
        let ords_ptr = base.add(ords_rva) as *const u16;

        for i in 0..num_names {
            if i * 4 + 4 > image_size.saturating_sub(names_rva) {
                break;
            }
            let name_rva = std::ptr::read_unaligned(names_ptr.add(i)) as usize;
            if name_rva == 0 || name_rva >= image_size {
                continue;
            }
            let name_ptr = base.add(name_rva);
            let mut name_bytes = Vec::new();
            let mut j = 0;
            loop {
                let ch = *name_ptr.add(j);
                if ch == 0 || j > 127 {
                    break;
                }
                name_bytes.push(ch);
                j += 1;
            }
            let fn_name = match std::str::from_utf8(&name_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            let ordinal = std::ptr::read_unaligned(ords_ptr.add(i)) as usize;
            if ordinal * 4 + 4 > image_size.saturating_sub(fns_rva) {
                continue;
            }
            let fn_rva = std::ptr::read_unaligned(fns_ptr.add(ordinal)) as usize;
            if fn_rva == 0 || fn_rva >= image_size {
                continue;
            }

            // Check for forwarder (fn_rva inside export section)
            if fn_rva >= export_rva && fn_rva < export_rva + export_size {
                // Forwarder string (skip for now)
                // Строка перенаправления экспорта (пропуск)
                continue;
            }

            let fn_va = image_base + fn_rva;
            exports.insert(fn_name, fn_va);
        }
    }

    exports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_dxvk_path() {
        let path = find_dxvk_path();
        println!("Found DXVK path: {:?}", path);
        assert!(path.is_some(), "DXVK should be found on system");
        let p = path.unwrap();
        assert!(std::path::Path::new(&format!("{}/dxgi.dll", p)).exists());
        assert!(std::path::Path::new(&format!("{}/d3d11.dll", p)).exists());
    }

    #[test]
    fn test_unity_diagnostics() {
        let _guard = crate::providers::STATS_TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let unity_player_path = format!("{}/.local/share/Steam/steamapps/common/How to Fish/How to Fish/UnityPlayer.dll", home);
        if !std::path::Path::new(&unity_player_path).exists() {
            println!("UnityPlayer.dll not found, skipping");
            return;
        }

        // Initialize DXVK so DXVK symbols can be resolved
        if let Some(path) = find_dxvk_path() {
            preload_dxvk(path);
        }

        let bytes = std::fs::read(&unity_player_path).expect("Failed to read UnityPlayer.dll");
        let pe = crate::pe::PeImage::parse(&bytes).expect("Failed to parse UnityPlayer.dll");

        let mut total_imports = 0;
        let mut resolved_imports = 0;
        let mut unresolved_imports = Vec::new();
        let mut target_dll_status: HashMap<String, Vec<(String, bool)>> = HashMap::new();

        let target_dlls = ["dxgi.dll", "d3d11.dll", "user32.dll", "gdi32.dll", "kernel32.dll"];

        for (dll, funcs) in pe.imports() {
            let dll_lower = dll.to_lowercase();
            let is_target = target_dlls.iter().any(|t| *t == dll_lower || dll_lower.starts_with(t.trim_end_matches(".dll")));

            for (fn_name, _) in funcs {
                total_imports += 1;
                let resolved = crate::providers::RineApiResolver::resolve_symbol(&dll, &fn_name).is_some();
                if resolved {
                    resolved_imports += 1;
                } else {
                    unresolved_imports.push((dll.clone(), fn_name.clone()));
                }

                if is_target {
                    target_dll_status.entry(dll_lower.clone()).or_default().push((fn_name.to_string(), resolved));
                }
            }
        }

        println!("=== UNITYPLAYER.DLL IMPORT DIAGNOSTICS ===");
        println!("Total Imports: {}, Resolved: {}, Unresolved: {}", total_imports, resolved_imports, unresolved_imports.len());

        for (dll, funcs) in &target_dll_status {
            let res_count = funcs.iter().filter(|(_, r)| *r).count();
            println!("\n--- {} (Resolved {} / {}) ---", dll, res_count, funcs.len());
            for (fn_name, res) in funcs {
                if !res {
                    println!("  [MISSING/STUB] {}", fn_name);
                } else {
                    println!("  [RESOLVED]     {}", fn_name);
                }
            }
        }
    }
}
