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

const DXVK_SEARCH_PATHS: &[&str] = &[
    // Bottles (Flatpak)
    "/home/fizzy/.var/app/com.usebottles.bottles/data/bottles/dxvk/dxvk-2.7.1/x64",
    // Vinegar / Heroic / Lutris common paths
    "/home/fizzy/.local/share/dxvk/x64",
    "/usr/share/dxvk/x64",
    "/usr/share/dxvk",
    "/opt/dxvk/x64",
];

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
        for path in DXVK_SEARCH_PATHS {
            if std::path::Path::new(path).exists() {
                let dxgi = format!("{}/dxgi.dll", path);
                if std::path::Path::new(&dxgi).exists() {
                    println!("[DXVK LOADER] Found DXVK at: {}", path);
                    return path.to_string();
                }
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
                // Forwarder string — skip for now
                continue;
            }

            let fn_va = image_base + fn_rva;
            exports.insert(fn_name, fn_va);
        }
    }

    exports
}
