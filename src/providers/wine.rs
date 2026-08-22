use crate::pe::imports::IatSymbolResolver;
use crate::pe::PeImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct WineConfig {
    pub enabled: bool,
    pub dll_path: Option<PathBuf>,
    pub allowed_modules: Vec<String>,
    pub allowed_symbols: Vec<String>,
}

impl Default for WineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dll_path: None,
            allowed_modules: Vec::new(),
            allowed_symbols: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyClass {
    ClassANativeRine,
    ClassBWineAllowlist,
    ClassCWineInternal,
    ClassDUnsupported,
}

pub fn find_wine_dll_dir() -> Option<PathBuf> {
    let candidate_dirs = [
        "/usr/lib/wine/x86_64-windows",
        "/usr/lib64/wine/x86_64-windows",
        "/usr/lib/x86_64-linux-gnu/wine/x86_64-windows",
        "/usr/share/wine",
    ];

    for dir in &candidate_dirs {
        let p = PathBuf::from(dir);
        if p.exists() && p.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&p) {
                for entry in entries.flatten() {
                    if let Some(ext) = entry.path().extension() {
                        if ext.to_string_lossy().eq_ignore_ascii_case("dll") {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let user_wine = PathBuf::from(home).join(".wine");
        if user_wine.exists() {
            return Some(user_wine);
        }
    }

    None
}

pub fn detect_wine_version() -> String {
    if let Ok(output) = std::process::Command::new("wine").arg("--version").output() {
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !ver.is_empty() {
                return ver;
            }
        }
    }
    "wine-10.0 (detected)".to_string()
}

pub static LOADED_WINE_MODULES: Mutex<Option<HashMap<String, usize>>> = Mutex::new(None);
pub static WINE_MODULE_EXPORTS: Mutex<Option<HashMap<String, HashMap<String, usize>>>> =
    Mutex::new(None);

pub fn is_symbol_allowlisted(config: &WineConfig, dll_name: &str, symbol_name: &str) -> bool {
    if !config.enabled {
        return false;
    }
    let dll_lower = dll_name.to_lowercase();
    let dll_stem = dll_lower.trim_end_matches(".dll");
    let target_sym_lower = symbol_name.to_lowercase();
    let full_target = format!("{}!{}", dll_stem, target_sym_lower);

    // 1. Explicit DLL!Symbol allowlist check
    for allowed_sym in &config.allowed_symbols {
        let allowed_lower = allowed_sym.to_lowercase();
        if allowed_lower == full_target
            || allowed_lower == format!("{}.dll!{}", dll_stem, target_sym_lower)
        {
            return true;
        }
    }

    // 2. Full module allowlist check
    for allowed in &config.allowed_modules {
        let allowed_lower = allowed.to_lowercase();
        if dll_lower == allowed_lower
            || dll_lower == format!("{}.dll", allowed_lower)
            || allowed_lower == format!("{}.dll", dll_lower)
        {
            return true;
        }
    }

    false
}

pub fn is_module_allowlisted(config: &WineConfig, dll_name: &str) -> bool {
    if !config.enabled {
        return false;
    }
    let dll_lower = dll_name.to_lowercase();
    for allowed in &config.allowed_modules {
        let allowed_lower = allowed.to_lowercase();
        if dll_lower == allowed_lower
            || dll_lower == format!("{}.dll", allowed_lower)
            || allowed_lower == format!("{}.dll", dll_lower)
        {
            return true;
        }
    }
    false
}

pub fn classify_wine_dependency(
    config: &WineConfig,
    dep_dll: &str,
    dep_symbol: &str,
) -> DependencyClass {
    let dep_dll_lower = dep_dll.to_lowercase();
    let dep_symbol_lower = dep_symbol.to_lowercase();

    if dep_dll_lower.contains("wine")
        || dep_symbol_lower.contains("wine_server")
        || dep_symbol_lower.contains("__wine_unix_call")
        || dep_symbol_lower.contains("unixlib")
        || dep_dll_lower.contains("wineserver")
    {
        return DependencyClass::ClassCWineInternal;
    }

    if IatSymbolResolver::resolve_symbol(dep_dll, dep_symbol).is_some() {
        return DependencyClass::ClassANativeRine;
    }

    if is_module_allowlisted(config, dep_dll) {
        return DependencyClass::ClassBWineAllowlist;
    }

    DependencyClass::ClassDUnsupported
}

pub struct LoadedWinePeModule {
    pub name: String,
    pub path: PathBuf,
    pub image_base: usize,
    pub image_size: usize,
    pub entry_point: usize,
    pub exports: HashMap<String, usize>,
}

pub static TEB_INITIALIZED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn ensure_thread_teb_setup() {
    if !TEB_INITIALIZED.load(std::sync::atomic::Ordering::Relaxed) {
        if let Ok(env) = crate::mem::peb_allocator::ProcessEnvironment::new(0x0000000180000000, 3) {
            let _ = crate::sys::trampoline::set_thread_teb_gs_base(env.teb_address);
            TEB_INITIALIZED.store(true, std::sync::atomic::Ordering::Relaxed);
            std::mem::forget(env);
        }
    }
}

pub fn load_and_map_wine_pe_dll(
    dll_path: &Path,
    config: &WineConfig,
) -> Result<LoadedWinePeModule, String> {
    ensure_thread_teb_setup();
    let bytes = std::fs::read(dll_path).map_err(|e| {
        format!(
            "Failed to read Wine DLL file '{}': {}",
            dll_path.display(),
            e
        )
    })?;

    let pe = PeImage::parse(&bytes).map_err(|e| {
        format!(
            "Failed to parse Wine PE image '{}': {}",
            dll_path.display(),
            e
        )
    })?;

    if pe.file_header.machine() != 0x8664 {
        return Err(format!(
            "Wine DLL '{}' is not x86_64 PE (Machine: 0x{:04X})",
            dll_path.display(),
            pe.file_header.machine()
        ));
    }

    let requested_base = pe.optional_header.image_base() as usize;
    let mut max_end = pe.optional_header.size_of_image() as usize;
    for section in pe.sections {
        let sec_end = (section.virtual_address() as usize)
            + std::cmp::max(section.virtual_size(), section.size_of_raw_data()) as usize;
        if sec_end > max_end {
            max_end = sec_end;
        }
    }
    let image_size = crate::sys::align_up(max_end, crate::sys::PAGE_SIZE);
    let entry_point_rva = pe.optional_header.address_of_entry_point() as usize;

    let region = crate::mem::mmap::MmapRegion::allocate_anonymous(
        Some(requested_base),
        image_size,
        libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
    )
    .map_err(|e| format!("Failed to allocate virtual memory for Wine DLL: {}", e))?;

    let base_ptr = region.as_ptr() as *mut u8;
    let allocated_base = base_ptr as usize;

    unsafe {
        std::ptr::write_bytes(base_ptr, 0, image_size);
    }

    let headers_size = pe.optional_header.size_of_headers() as usize;
    let copy_headers_len = headers_size.min(bytes.len());
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), base_ptr, copy_headers_len);
    }

    for section in pe.sections {
        let vaddr = section.virtual_address() as usize;
        let raw_data_ptr = section.pointer_to_raw_data() as usize;
        let raw_data_size = section.size_of_raw_data() as usize;

        if vaddr >= image_size || raw_data_ptr == 0 || raw_data_size == 0 {
            continue;
        }

        let section_dest = unsafe { base_ptr.add(vaddr) };
        let end_offset = (raw_data_ptr + raw_data_size).min(bytes.len());
        if raw_data_ptr < end_offset {
            let src_slice = &bytes[raw_data_ptr..end_offset];
            unsafe {
                std::ptr::copy_nonoverlapping(src_slice.as_ptr(), section_dest, src_slice.len());
            }
        }
    }

    for section in pe.sections {
        if section.name_str().starts_with(".reloc") {
            let reloc_rva = section.virtual_address() as usize;
            let reloc_size = section.virtual_size() as usize;
            unsafe {
                let _ = crate::mem::reloc::BaseRelocator::apply_relocations(
                    allocated_base,
                    requested_base,
                    reloc_rva,
                    reloc_size,
                );
            }
            break;
        }
    }

    let file_name = dll_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "wine.dll".to_string());

    let imports = pe.imports();
    for (dep_dll, funcs) in &imports {
        for (fn_name, iat_thunk_rva) in funcs {
            let classification = classify_wine_dependency(config, dep_dll, fn_name);
            println!(
                "[RINE WINE DEPENDENCY]\nmodule={}\ndependency={}!{}\nclassification={:?}",
                file_name, dep_dll, fn_name, classification
            );

            if classification == DependencyClass::ClassCWineInternal {
                eprintln!(
                    "[RINE WINE ADAPTER UNSUPPORTED DEPENDENCY]\nmodule={}\nsymbol={}\ndependency={}\nreason=Wine internal / Unix bridge dependency detected",
                    file_name, fn_name, dep_dll
                );
                return Err(format!(
                    "Wine internal dependency in '{}': {}!{}",
                    file_name, dep_dll, fn_name
                ));
            }

            let resolved_addr = match IatSymbolResolver::resolve_symbol(dep_dll, fn_name) {
                Some(addr) => addr,
                None => {
                    let stub_va = allocated_base + *iat_thunk_rva as usize;
                    crate::pe::imports::create_unresolved_stub(
                        &file_name,
                        dep_dll,
                        fn_name,
                        *iat_thunk_rva as usize,
                        stub_va,
                        "wine_adapter_dependency",
                    )
                }
            };

            let slot_ptr = unsafe { base_ptr.add(*iat_thunk_rva as usize) as *mut u64 };
            unsafe {
                *slot_ptr = resolved_addr as u64;
            }
        }
    }

    let exports_map = parse_pe_exports(&bytes, &pe, allocated_base);
    println!(
        "[DEBUG WINE EXPORTS] Found {} exports in {}: {:?}",
        exports_map.len(),
        file_name,
        exports_map.keys().collect::<Vec<_>>()
    );

    crate::pe::imports::register_loaded_module(file_name.clone(), allocated_base, image_size);
    std::mem::forget(region);

    {
        let mut guard = WINE_MODULE_EXPORTS.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(file_name.to_lowercase(), exports_map.clone());
    }
    {
        let mut guard = LOADED_WINE_MODULES.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(file_name.to_lowercase(), allocated_base);
    }

    let ep_va = allocated_base + entry_point_rva;
    if entry_point_rva > 0 {
        println!(
            "[RINE WINE MODULE] Calling DllMain(0x{:016X}, DLL_PROCESS_ATTACH) @ 0x{:016X}...",
            allocated_base, ep_va
        );
        let init_res = unsafe { call_win64_export_safely(ep_va, allocated_base, 1, 0, 0) };
        println!("[RINE WINE MODULE] DllMain returned status {}", init_res);
    }

    println!(
        "[RINE WINE MODULE]\nname={}\npath={}\nimage_base=0x{:016X}\nimage_size={}\nentry_point=0x{:016X}\nimports={}\nstatus=mapped",
        file_name,
        dll_path.display(),
        allocated_base,
        image_size,
        ep_va,
        imports.len()
    );

    Ok(LoadedWinePeModule {
        name: file_name,
        path: dll_path.to_path_buf(),
        image_base: allocated_base,
        image_size,
        entry_point: allocated_base + entry_point_rva,
        exports: exports_map,
    })
}

pub fn resolve_wine_export(
    config: &WineConfig,
    dll_name: &str,
    symbol_name: &str,
) -> Option<usize> {
    if !is_symbol_allowlisted(config, dll_name, symbol_name) {
        return None;
    }

    let dll_dir = match &config.dll_path {
        Some(p) => p.clone(),
        None => find_wine_dll_dir()?,
    };

    let target_dll = if dll_name.to_lowercase().ends_with(".dll") {
        dll_name.to_lowercase()
    } else {
        format!("{}.dll", dll_name.to_lowercase())
    };

    {
        let guard = WINE_MODULE_EXPORTS.lock().unwrap();
        if let Some(map) = guard.as_ref() {
            if let Some(exports) = map.get(&target_dll) {
                if let Some(&addr) = exports.get(symbol_name) {
                    return Some(addr);
                }
            }
        }
    }

    let dll_file_path = dll_dir.join(&target_dll);
    if !dll_file_path.exists() {
        return None;
    }

    match load_and_map_wine_pe_dll(&dll_file_path, config) {
        Ok(module) => module.exports.get(symbol_name).copied(),
        Err(e) => {
            eprintln!("[RINE WINE ADAPTER ERROR] {}", e);
            None
        }
    }
}

#[unsafe(naked)]
pub unsafe extern "win64" fn call_win64_export_safely(
    export_fn: usize,
    rcx: usize,
    rdx: usize,
    r8: usize,
    r9: usize,
) -> usize {
    std::arch::naked_asm!(
        "push rbx",
        "push rbp",
        "push rsi",
        "push rdi",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "sub rsp, 0xA8",
        "movdqu [rsp + 0x00], xmm6",
        "movdqu [rsp + 0x10], xmm7",
        "movdqu [rsp + 0x20], xmm8",
        "movdqu [rsp + 0x30], xmm9",
        "movdqu [rsp + 0x40], xmm10",
        "movdqu [rsp + 0x50], xmm11",
        "movdqu [rsp + 0x60], xmm12",
        "movdqu [rsp + 0x70], xmm13",
        "movdqu [rsp + 0x80], xmm14",
        "movdqu [rsp + 0x90], xmm15",
        "sub rsp, 0x20",
        "mov rax, rcx",
        "mov rcx, rdx",
        "mov rdx, r8",
        "mov r8, r9",
        "mov r9, [rsp + 0x130]",
        "call rax",
        "add rsp, 0x20",
        "movdqu xmm6, [rsp + 0x00]",
        "movdqu xmm7, [rsp + 0x10]",
        "movdqu xmm8, [rsp + 0x20]",
        "movdqu xmm9, [rsp + 0x30]",
        "movdqu xmm10, [rsp + 0x40]",
        "movdqu xmm11, [rsp + 0x50]",
        "movdqu xmm12, [rsp + 0x60]",
        "movdqu xmm13, [rsp + 0x70]",
        "movdqu xmm14, [rsp + 0x80]",
        "movdqu xmm15, [rsp + 0x90]",
        "add rsp, 0xA8",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rdi",
        "pop rsi",
        "pop rbp",
        "pop rbx",
        "ret",
    );
}

pub fn parse_pe_exports(
    raw_bytes: &[u8],
    pe: &PeImage,
    allocated_base: usize,
) -> HashMap<String, usize> {
    let mut exports = HashMap::new();
    let export_dir = &pe.optional_header.data_directory[0];
    if export_dir.virtual_address() == 0 || export_dir.size() == 0 {
        return exports;
    }

    let exp_rva = export_dir.virtual_address() as usize;
    let exp_size = export_dir.size() as usize;

    let rva_to_off = |rva: usize| -> Option<usize> {
        for section in pe.sections {
            let vaddr = section.virtual_address() as usize;
            let vsize = section.virtual_size() as usize;
            let rawptr = section.pointer_to_raw_data() as usize;
            let rawsize = section.size_of_raw_data() as usize;
            let len = std::cmp::max(vsize, rawsize);
            if rva >= vaddr && rva < vaddr + len {
                let offset = rawptr + (rva - vaddr);
                if offset < raw_bytes.len() {
                    return Some(offset);
                }
            }
        }
        None
    };

    let exp_off = match rva_to_off(exp_rva) {
        Some(off) => off,
        None => return exports,
    };

    if exp_off + 40 > raw_bytes.len() {
        return exports;
    }

    let num_names = u32::from_le_bytes(raw_bytes[exp_off + 24..exp_off + 28].try_into().unwrap());
    let funcs_rva = u32::from_le_bytes(raw_bytes[exp_off + 28..exp_off + 32].try_into().unwrap());
    let names_rva = u32::from_le_bytes(raw_bytes[exp_off + 32..exp_off + 36].try_into().unwrap());
    let ords_rva = u32::from_le_bytes(raw_bytes[exp_off + 36..exp_off + 40].try_into().unwrap());

    let names_off = match rva_to_off(names_rva as usize) {
        Some(o) => o,
        None => return exports,
    };
    let funcs_off = match rva_to_off(funcs_rva as usize) {
        Some(o) => o,
        None => return exports,
    };
    let ords_off = match rva_to_off(ords_rva as usize) {
        Some(o) => o,
        None => return exports,
    };

    for i in 0..num_names as usize {
        if names_off + (i + 1) * 4 > raw_bytes.len() || ords_off + (i + 1) * 2 > raw_bytes.len() {
            break;
        }
        let name_ptr_rva = u32::from_le_bytes(
            raw_bytes[names_off + i * 4..names_off + (i + 1) * 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let ord_val = u16::from_le_bytes(
            raw_bytes[ords_off + i * 2..ords_off + (i + 1) * 2]
                .try_into()
                .unwrap(),
        ) as usize;

        if funcs_off + (ord_val + 1) * 4 > raw_bytes.len() {
            continue;
        }
        let func_rva = u32::from_le_bytes(
            raw_bytes[funcs_off + ord_val * 4..funcs_off + (ord_val + 1) * 4]
                .try_into()
                .unwrap(),
        ) as usize;

        if let Some(name_off) = rva_to_off(name_ptr_rva) {
            if let Some(end_idx) = raw_bytes[name_off..].iter().position(|&b| b == 0) {
                if let Ok(func_name) = std::str::from_utf8(&raw_bytes[name_off..name_off + end_idx])
                {
                    let is_forwarder = func_rva >= exp_rva && func_rva < exp_rva + exp_size;
                    if !is_forwarder {
                        exports.insert(func_name.to_string(), allocated_base + func_rva);
                    }
                }
            }
        }
    }

    exports
}
