use crate::mem::mmap::MmapRegion;
use crate::mem::strategy::{LoadConfig, StorageStrategy};
use crate::pe::section::SectionCharacteristics;
use crate::pe::PeImage;
use crate::sys::{align_up, Result, PAGE_SIZE};

use crate::mem::peb_allocator::ProcessEnvironment;

pub struct LoadedModule {
    pub name: String,
    pub image_base: usize,
    pub entry_point: usize,
    pub size_of_image: usize,
    pub memory_region: MmapRegion,
    pub env: ProcessEnvironment,
    pub config: LoadConfig,
}

impl LoadedModule {
    /// Loads a Win64 PE image into memory according to the specified strategy (DiskDemand or RamOnly).
    pub fn load(name: &str, raw_pe_bytes: &[u8], config: LoadConfig) -> Result<Self> {
        let pe = PeImage::parse(raw_pe_bytes)?;

        let requested_image_base = pe.optional_header.image_base() as usize;
        let mut max_end = pe.optional_header.size_of_image() as usize;
        for section in pe.sections {
            let sec_end = (section.virtual_address() as usize)
                + std::cmp::max(section.virtual_size(), section.size_of_raw_data()) as usize;
            if sec_end > max_end {
                max_end = sec_end;
            }
        }
        let image_size = align_up(max_end, PAGE_SIZE);
        let mut entry_point_rva = pe.optional_header.address_of_entry_point() as usize;

        // 1. Allocate Virtual Address Space according to Strategy
        let mut region = match config.strategy {
            StorageStrategy::RamOnly { lock_in_memory } => {
                // Pre-cache memory allocation via RAM buffer / memfd_create at preferred ImageBase
                let mut mmap_reg = MmapRegion::allocate_memfd(
                    name,
                    Some(requested_image_base),
                    image_size,
                    raw_pe_bytes,
                    libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
                )?;

                if lock_in_memory {
                    mmap_reg.lock()?;
                }
                mmap_reg
            }
            StorageStrategy::DiskDemand => {
                // Allocate anonymous memory mapped on-demand
                MmapRegion::allocate_anonymous(
                    Some(requested_image_base),
                    image_size,
                    libc::PROT_READ | libc::PROT_WRITE,
                )?
            }
        };

        let base_ptr = region.as_mut_ptr();
        let allocated_base = base_ptr as usize;

        // Zero out entire 341MB image memory buffer
        unsafe {
            std::ptr::write_bytes(base_ptr, 0, image_size);
        }

        // Map a 64KB zero-page at virtual address 0x0000000000000000 for null-pointer resilience
        unsafe {
            let zero_page = libc::mmap(
                std::ptr::null_mut(),
                65536,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            if zero_page != libc::MAP_FAILED {
                println!("[RINE MEMORY LOADER] Initialized 64KB Null-Pointer Resilient Safe Buffer at {:p}", zero_page);
            }
        }

        // 2. Map PE Headers
        let headers_size = pe.optional_header.size_of_headers() as usize;
        let copy_headers_len = headers_size.min(raw_pe_bytes.len());
        unsafe {
            std::ptr::copy_nonoverlapping(raw_pe_bytes.as_ptr(), base_ptr, copy_headers_len);
        }

        let env =
            ProcessEnvironment::new(allocated_base as u64, pe.optional_header.subsystem() as u32)?;
        crate::dlls::kernel32::set_current_image_base(allocated_base as u64);
        let _ = crate::sys::trampoline::set_thread_teb_gs_base(env.teb_address);

        // 3. Map PE Sections
        for section in pe.sections {
            let vaddr = section.virtual_address() as usize;
            let raw_data_ptr = section.pointer_to_raw_data() as usize;
            let raw_data_size = section.size_of_raw_data() as usize;
            let virtual_size = section.virtual_size() as usize;

            if vaddr >= image_size {
                continue;
            }

            let section_dest = unsafe { base_ptr.add(vaddr) };

            // Copy raw data into section virtual space if present.
            // Bounded by distance to next section so embedded data sections (like Godot PCK) don't overwrite neighboring sections
            if raw_data_ptr > 0 && raw_data_size > 0 {
                let mut max_copy_len = raw_data_size;
                for next_sec in pe.sections {
                    let next_vaddr = next_sec.virtual_address() as usize;
                    if next_vaddr > vaddr {
                        let dist = next_vaddr - vaddr;
                        if dist < max_copy_len {
                            max_copy_len = dist;
                        }
                    }
                }
                let bytes_to_copy = raw_data_size.min(max_copy_len);
                let end_offset = (raw_data_ptr + bytes_to_copy).min(raw_pe_bytes.len());

                if raw_data_ptr < end_offset {
                    let src_slice = &raw_pe_bytes[raw_data_ptr..end_offset];
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            src_slice.as_ptr(),
                            section_dest,
                            src_slice.len(),
                        );
                    }
                }
            }

            // Zero uninitialized data (.bss) if virtual_size > raw_data_size
            let bss_start_offset = vaddr + raw_data_size;
            let total_sec_len = if virtual_size == 0 {
                raw_data_size
            } else {
                virtual_size
            };
            if total_sec_len > raw_data_size {
                let zero_len = total_sec_len - raw_data_size;
                if bss_start_offset + zero_len <= image_size {
                    unsafe {
                        std::ptr::write_bytes(base_ptr.add(bss_start_offset), 0, zero_len);
                    }
                }
            }
        }

        // 3.5. Detect and auto-decrypt SteamStub DRM if present
        if let Some(oep) = crate::pe::steamstub::try_unpack_steamstub(
            base_ptr,
            image_size,
            raw_pe_bytes,
            entry_point_rva,
        ) {
            entry_point_rva = oep;
        }

        // 4. Find .reloc section and apply base relocations
        for section in pe.sections {
            if section.name_str().starts_with(".reloc") {
                let reloc_rva = section.virtual_address() as usize;
                let reloc_size = section.virtual_size() as usize;
                unsafe {
                    crate::mem::reloc::BaseRelocator::apply_relocations(
                        allocated_base,
                        requested_image_base,
                        reloc_rva,
                        reloc_size,
                    )?;
                }
                break;
            }
        }

        // 5. Register main module in companion modules and loaded module registry
        let mut main_exports = std::collections::HashMap::new();
        for (exp_name, exp_rva, _ord) in pe.exports() {
            main_exports.insert(exp_name.to_string(), exp_rva);
        }
        let clean_name = name.replace('\\', "/");
        let file_name = std::path::Path::new(&clean_name)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| name.to_string());
        let name_lower = file_name.to_lowercase();
        let target_exe = crate::dlls::kernel32::get_target_exe_path();
        let target_exe_clean = target_exe.replace('\\', "/");
        let target_exe_file = std::path::Path::new(&target_exe_clean)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default()
            .to_lowercase();

        register_module_path(allocated_base, &target_exe);

        {
            let mut guard = get_companion_modules().lock().unwrap();
            guard.insert(name_lower.clone(), (allocated_base, main_exports.clone()));
            guard.insert(clean_name.to_lowercase(), (allocated_base, main_exports.clone()));
            guard.insert(name.to_lowercase(), (allocated_base, main_exports.clone()));
            if !target_exe_file.is_empty() {
                guard.insert(target_exe_file.clone(), (allocated_base, main_exports.clone()));
                guard.insert(target_exe_clean.to_lowercase(), (allocated_base, main_exports.clone()));
            }
            guard.insert("target.exe".to_string(), (allocated_base, main_exports.clone()));
        }

        crate::pe::imports::register_loaded_module(name.to_string(), allocated_base, image_size);
        crate::pe::imports::register_loaded_module(name_lower.clone(), allocated_base, image_size);
        if !target_exe_file.is_empty() {
            crate::pe::imports::register_loaded_module(target_exe_file, allocated_base, image_size);
        }

        // 5b. Bind Import Address Table (IAT) function pointers in process memory
        let imports = pe.imports();
        let mut resolved_count = 0;
        let mut total_imports = 0;

        // Protect .CRT section from being overwritten by IAT binder or DataDirectory patcher
        let mut crt_start = 0usize;
        let mut crt_end = 0usize;
        for sec in pe.sections {
            if sec.name_str().trim_matches('\0') == ".CRT" {
                crt_start = sec.virtual_address() as usize;
                crt_end = crt_start
                    + (if sec.virtual_size() > 0 {
                        sec.virtual_size() as usize
                    } else {
                        sec.size_of_raw_data() as usize
                    });
                break;
            }
        }
        let is_crt_rva =
            |rva: usize| -> bool { crt_start > 0 && rva >= crt_start && rva < crt_end };

        for (dll_name, funcs) in imports {
            for (fn_name, iat_thunk_rva) in funcs {
                total_imports += 1;
                if is_crt_rva(iat_thunk_rva as usize) {
                    println!(
                        "[RINE PE LOADER] PROTECT .CRT section slot @ RVA 0x{:X}",
                        iat_thunk_rva
                    );
                    continue;
                }

                let full_symbol_name = format!("{}!{}", dll_name, fn_name);
                let slot_addr = allocated_base as u64 + iat_thunk_rva as u64;
                let resolved_addr =
                    crate::providers::RineApiResolver::resolve_symbol(dll_name, fn_name)
                        .map(|res| res.address as usize)
                        .unwrap_or_else(|| {
                            crate::pe::imports::create_unresolved_stub(
                                name,
                                dll_name,
                                fn_name,
                                iat_thunk_rva as usize,
                                slot_addr as usize,
                                "normal",
                            )
                        });

                crate::pe::imports::register_iat_symbol(slot_addr, full_symbol_name);

                let iat_slot_ptr = unsafe { base_ptr.add(iat_thunk_rva as usize) as *mut u64 };
                unsafe {
                    *iat_slot_ptr = resolved_addr as u64;
                }
                resolved_count += 1;
            }
        }

        // Patch IAT Directory Table (data_directory[12]) for MSVC binaries
        // Any slot still holding an RVA inside the image (raw disk OFT/Hint pointer) or 0
        // is an unbound import that was not resolved above. Replace it with a safe dynamic stub!
        let iat_dir = &pe.optional_header.data_directory[12]; // IMAGE_DIRECTORY_ENTRY_IAT
        if iat_dir.virtual_address() > 0 && iat_dir.size() > 0 {
            let iat_start_rva = iat_dir.virtual_address() as usize;
            let iat_size = iat_dir.size() as usize;

            let mut offset = 0;
            let mut extra_filled = 0usize;
            let base_u64 = allocated_base as u64;
            let image_end = base_u64 + image_size as u64;

            while offset < iat_size {
                let slot_rva = iat_start_rva + offset;
                if !is_crt_rva(slot_rva) && slot_rva + 8 <= image_size {
                    let slot_ptr = unsafe { base_ptr.add(slot_rva) as *mut u64 };
                    let val = unsafe { *slot_ptr };
                    // If val is 0 or points inside the image range (raw OFT hint RVA), it is unbound!
                    if val == 0 || (val >= base_u64 && val < image_end) || val < 0x10000 {
                        let stub_addr = crate::pe::imports::create_unresolved_stub(
                            name,
                            "DataDir12Fallback",
                            &format!("RVA_0x{:X}_Val_0x{:X}", slot_rva, val),
                            slot_rva,
                            base_u64 as usize + slot_rva,
                            "iat_directory_fallback",
                        ) as u64;
                        unsafe {
                            *slot_ptr = stub_addr;
                        }
                        extra_filled += 1;
                    }
                }
                offset += 8;
            }
            if extra_filled > 0 {
                println!(
                    "[RINE IAT DIRECTORY] Bound {} remaining unresolved/RVA slots in DataDirectory[12] range (0x{:X}..0x{:X}) to Dynamic Stubs.",
                    extra_filled, iat_start_rva, iat_start_rva + iat_size
                );
            }
        }

        // 6. Apply RWX Protection for entire mapped image buffer (341MB)
        region.protect(
            0,
            image_size,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
        )?;
        println!("[RINE MEMORY PROTECTOR] Applied RWX protection to entire 341MB Image Virtual Address Space.");

        // Initialize TLS Directory (data_directory[9]) for MSVC CRT TLS callbacks & indices (AFTER executable protection!)
        register_module_tls(allocated_base, image_size, &pe);

        // Register Exception Directory (.pdata) for x86_64 Stack Unwinding & SEH
        register_module_pdata(allocated_base, image_size, &pe);

        let actual_entry_point = allocated_base + entry_point_rva;

        Ok(Self {
            name: name.to_string(),
            image_base: allocated_base,
            entry_point: actual_entry_point,
            size_of_image: image_size,
            memory_region: region,
            env,
            config,
        })
    }
}

static COMPANION_MODULES: std::sync::OnceLock<
    std::sync::Mutex<
        std::collections::HashMap<String, (usize, std::collections::HashMap<String, u32>)>,
    >,
> = std::sync::OnceLock::new();

fn get_companion_modules() -> &'static std::sync::Mutex<
    std::collections::HashMap<String, (usize, std::collections::HashMap<String, u32>)>,
> {
    COMPANION_MODULES.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

static FAILED_COMPANION_DLLS: std::sync::Mutex<Option<std::collections::HashSet<String>>> =
    std::sync::Mutex::new(None);

static MODULE_FULL_PATHS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<usize, String>>> = std::sync::OnceLock::new();

pub fn register_module_path(base: usize, path: &str) {
    let map = MODULE_FULL_PATHS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    map.lock().unwrap().insert(base, path.to_string());
}

pub fn get_module_path(base: usize) -> Option<String> {
    let map = MODULE_FULL_PATHS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    map.lock().unwrap().get(&base).cloned()
}

impl LoadedModule {
    pub fn load_companion_dll(dll_name: &str) -> Option<usize> {
        let mut clean_name = dll_name.replace('\\', "/");
        while clean_name.to_ascii_lowercase().ends_with(".dll.dll") {
            clean_name.truncate(clean_name.len() - 4);
        }
        let mut file_name = std::path::Path::new(&clean_name)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| clean_name.clone());
        while file_name.to_ascii_lowercase().ends_with(".dll.dll") {
            file_name.truncate(file_name.len() - 4);
        }
        let name_lower = file_name.to_lowercase();
        let clean_lower = clean_name.to_lowercase();
        {
            let guard = get_companion_modules().lock().unwrap();
            if let Some((base, _)) = guard.get(&name_lower).or_else(|| guard.get(&clean_lower)) {
                return Some(*base);
            }
        }

        {
            let mut failed_guard = FAILED_COMPANION_DLLS.lock().unwrap();
            let failed_set = failed_guard.get_or_insert_with(std::collections::HashSet::new);
            if failed_set.contains(&name_lower) || failed_set.contains(&clean_lower) {
                return None;
            }

            let is_system_dll = match name_lower.as_str() {
                "kernel32.dll" | "ntdll.dll" | "user32.dll" | "gdi32.dll" | "advapi32.dll"
                | "ws2_32.dll" | "wsock32.dll" | "winmm.dll" | "imm32.dll" | "ole32.dll"
                | "oleaut32.dll" | "shell32.dll" | "shlwapi.dll" | "version.dll" | "crypt32.dll"
                | "bcrypt.dll" | "setupapi.dll" | "dwmapi.dll" | "dinput8.dll" | "dsound.dll"
                | "dxgi.dll" | "d3d11.dll" | "vulkan-1.dll" | "msvcrt.dll" | "ucrtbase.dll" => true,
                s if s.starts_with("api-ms-win-") || s.starts_with("ext-ms-win-") => true,
                _ => false,
            };
            if is_system_dll {
                failed_set.insert(name_lower);
                failed_set.insert(clean_lower);
                return None;
            }
        }

        let target_dir = crate::dlls::kernel32::get_process_current_directory();
        let target_exe = crate::dlls::kernel32::get_target_exe_path();
        let host_target_dir = crate::dlls::kernel32::win32_to_host_path(&target_dir);
        let host_target_exe = crate::dlls::kernel32::win32_to_host_path(&target_exe);
        let host_exe_dir = std::path::Path::new(&host_target_exe)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let host_direct_path = crate::dlls::kernel32::win32_to_host_path(dll_name);

        let mut candidate_paths = vec![
            std::path::PathBuf::from(&host_direct_path),
            std::path::PathBuf::from(&host_target_dir).join(&file_name),
            std::path::PathBuf::from(&host_target_dir).join(&name_lower),
            std::path::PathBuf::from(&host_exe_dir).join(&file_name),
            std::path::PathBuf::from(&host_exe_dir).join(&name_lower),
            std::path::PathBuf::from(&file_name),
            std::path::PathBuf::from(&name_lower),
        ];

        for dir in crate::dlls::kernel32::get_dll_directories() {
            candidate_paths.insert(0, std::path::PathBuf::from(&dir).join(&file_name));
            candidate_paths.insert(1, std::path::PathBuf::from(&dir).join(&name_lower));
        }

        if let Ok(entries) = std::fs::read_dir(&host_exe_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let path = entry.path();
                        let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                        if dir_name.ends_with("_Data") {
                            candidate_paths.push(path.join("Plugins").join("x86_64").join(&file_name));
                            candidate_paths.push(path.join("Plugins").join("x86_64").join(&name_lower));
                            candidate_paths.push(path.join("Plugins").join(&file_name));
                            candidate_paths.push(path.join("Plugins").join(&name_lower));
                            candidate_paths.push(path.join("Managed").join(&file_name));
                            candidate_paths.push(path.join("Managed").join(&name_lower));
                        }
                    }
                }
            }
        }
        candidate_paths.push(std::path::PathBuf::from(&host_exe_dir).join("Plugins").join("x86_64").join(&file_name));
        candidate_paths.push(std::path::PathBuf::from(&host_exe_dir).join("Plugins").join("x86_64").join(&name_lower));
        candidate_paths.push(std::path::PathBuf::from(&host_exe_dir).join("Plugins").join(&file_name));
        candidate_paths.push(std::path::PathBuf::from(&host_exe_dir).join("Plugins").join(&name_lower));

        if name_lower == "steamclient64.dll" || name_lower == "steamclient.dll" {
            if let Ok(home) = std::env::var("HOME") {
                candidate_paths.push(std::path::PathBuf::from(format!(
                    "{}/.local/share/Steam/steamapps/common/Proton - Experimental/files/lib/wine/x86_64-windows/lsteamclient.dll",
                    home
                )));
            }
        }

        let mut raw_bytes = None;
        println!(
            "[RINE COMPANION LOADER] Searching for DLL '{}' in candidate paths: {:?}",
            dll_name, candidate_paths
        );
        for path in &candidate_paths {
            if path.exists() && path.is_file() {
                if let Ok(bytes) = std::fs::read(path) {
                    println!(
                        "[RINE COMPANION LOADER] Found '{}' at '{:?}' (size {} bytes)",
                        dll_name,
                        path,
                        bytes.len()
                    );
                    raw_bytes = Some(bytes);
                    break;
                }
            }
        }

        let raw_bytes = match raw_bytes {
            Some(b) => b,
            None => {
                println!(
                    "[RINE COMPANION LOADER] DLL '{}' not found in candidate paths.",
                    dll_name
                );
                let mut failed_guard = FAILED_COMPANION_DLLS.lock().unwrap();
                let failed_set = failed_guard.get_or_insert_with(std::collections::HashSet::new);
                failed_set.insert(name_lower);
                failed_set.insert(clean_lower);
                return None;
            }
        };
        let pe = match PeImage::parse(&raw_bytes) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "[RINE COMPANION LOADER] Failed to parse PE DLL '{}': {:?}",
                    dll_name, e
                );
                return None;
            }
        };

        let requested_image_base = pe.optional_header.image_base() as usize;
        let image_size = pe.optional_header.size_of_image() as usize;
        let mut region = match MmapRegion::allocate_anonymous(
            Some(requested_image_base),
            image_size,
            libc::PROT_READ | libc::PROT_WRITE,
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "[RINE COMPANION LOADER] Memory allocation failed for '{}': {:?}",
                    dll_name, e
                );
                return None;
            }
        };

        let base_ptr = region.as_mut_ptr();
        let allocated_base = base_ptr as usize;

        unsafe {
            std::ptr::write_bytes(base_ptr, 0, image_size);
        }

        let headers_size = pe.optional_header.size_of_headers() as usize;
        let copy_headers_len = headers_size.min(raw_bytes.len());
        unsafe {
            std::ptr::copy_nonoverlapping(raw_bytes.as_ptr(), base_ptr, copy_headers_len);
        }

        for section in pe.sections {
            let vaddr = section.virtual_address() as usize;
            let raw_data_ptr = section.pointer_to_raw_data() as usize;
            let raw_data_size = section.size_of_raw_data() as usize;

            if vaddr >= image_size {
                continue;
            }

            let section_dest = unsafe { base_ptr.add(vaddr) };
            if raw_data_ptr > 0 && raw_data_size > 0 {
                let end_offset = (raw_data_ptr + raw_data_size).min(raw_bytes.len());
                if raw_data_ptr < end_offset {
                    let src_slice = &raw_bytes[raw_data_ptr..end_offset];
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            src_slice.as_ptr(),
                            section_dest,
                            src_slice.len(),
                        );
                    }
                }
            }
        }

        for section in pe.sections {
            if section.name_str().starts_with(".reloc") {
                let reloc_rva = section.virtual_address() as usize;
                let reloc_size = section.virtual_size() as usize;
                let reloc_slice = &raw_bytes[section.pointer_to_raw_data() as usize
                    ..(section.pointer_to_raw_data() as usize + reloc_size)];
                let delta = allocated_base as i64 - requested_image_base as i64;
                let mut offset = 0;
                while offset + 8 <= reloc_slice.len() {
                    let page_rva =
                        u32::from_le_bytes(reloc_slice[offset..offset + 4].try_into().unwrap())
                            as usize;
                    let block_size =
                        u32::from_le_bytes(reloc_slice[offset + 4..offset + 8].try_into().unwrap())
                            as usize;
                    if block_size < 8 {
                        break;
                    }
                    let entry_count = (block_size - 8) / 2;
                    for i in 0..entry_count {
                        let entry_offset = offset + 8 + i * 2;
                        if entry_offset + 2 > reloc_slice.len() {
                            break;
                        }
                        let entry = u16::from_le_bytes(
                            reloc_slice[entry_offset..entry_offset + 2]
                                .try_into()
                                .unwrap(),
                        );
                        let reloc_type = (entry >> 12) as u8;
                        let offset_in_page = (entry & 0x0FFF) as usize;
                        if reloc_type == 10 {
                            // IMAGE_REL_BASED_DIR64
                            let target_rva = page_rva + offset_in_page;
                            if target_rva + 8 <= image_size {
                                unsafe {
                                    let target_ptr = base_ptr.add(target_rva) as *mut u64;
                                    let current_val = *target_ptr;
                                    *target_ptr = (current_val as i64 + delta) as u64;
                                }
                            }
                        }
                    }
                    offset += block_size;
                }
                break;
            }
        }

        crate::pe::imports::register_loaded_module(name_lower.clone(), allocated_base, image_size);
        if name_lower == "steamclient64.dll" {
            crate::pe::imports::register_loaded_module("lsteamclient.dll".to_string(), allocated_base, image_size);
            crate::pe::imports::register_loaded_module("steamclient.dll".to_string(), allocated_base, image_size);
        } else if name_lower == "lsteamclient.dll" {
            crate::pe::imports::register_loaded_module("steamclient64.dll".to_string(), allocated_base, image_size);
            crate::pe::imports::register_loaded_module("steamclient.dll".to_string(), allocated_base, image_size);
        }

        let pe_name = name_lower.clone();
        let companion_imports = pe.imports();
        for (imp_dll, funcs) in companion_imports {
            for (fn_name, iat_thunk_rva) in funcs {
                let resolved =
                    crate::providers::RineApiResolver::resolve_symbol(&imp_dll, &fn_name)
                        .map(|res| res.address as usize)
                        .unwrap_or_else(|| {
                            crate::pe::imports::create_unresolved_stub(
                                &pe_name,
                                &imp_dll,
                                &fn_name,
                                iat_thunk_rva as usize,
                                allocated_base + iat_thunk_rva as usize,
                                "companion_iat",
                            )
                        });
                let iat_slot_ptr = unsafe { base_ptr.add(iat_thunk_rva as usize) as *mut u64 };
                unsafe {
                    *iat_slot_ptr = resolved as u64;
                }
            }
        }

        let mut export_map = std::collections::HashMap::new();
        for (exp_name, exp_rva, _ord) in pe.exports() {
            export_map.insert(exp_name.to_string(), exp_rva);
        }

        // Apply executable protection BEFORE calling TLS callbacks or DllMain!
        let _ = region.protect(
            0,
            image_size,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
        );

        std::mem::forget(region);

        // Initialize TLS Directory for companion DLL
        register_module_tls(allocated_base, image_size, &pe);

        println!(
            "[RINE COMPANION LOADER] Loaded Companion PE DLL '{}' @ 0x{:016X} ({} exports)",
            dll_name,
            allocated_base,
            export_map.len()
        );

        let mut guard = get_companion_modules().lock().unwrap();
        guard.insert(name_lower.clone(), (allocated_base, export_map.clone()));
        if name_lower == "steamclient64.dll" {
            guard.insert("lsteamclient.dll".to_string(), (allocated_base, export_map.clone()));
            guard.insert("steamclient.dll".to_string(), (allocated_base, export_map.clone()));
        } else if name_lower == "lsteamclient.dll" {
            guard.insert("steamclient64.dll".to_string(), (allocated_base, export_map.clone()));
            guard.insert("steamclient.dll".to_string(), (allocated_base, export_map.clone()));
        }
        drop(guard);
        register_module_path(allocated_base, dll_name);

        if name_lower.contains("unityplayer") {
            let func_addr = allocated_base + 0x13CC240;
            let expected_prologue: [u8; 11] = [0x40, 0x55, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41, 0x56, 0x41, 0x57];
            let actual = unsafe { std::slice::from_raw_parts(func_addr as *const u8, 11) };
            if actual == expected_prologue {
                let stub_hint = (allocated_base + 0x2500000) as *mut libc::c_void;
                let stub_mem = unsafe {
                    libc::mmap(
                        stub_hint,
                        4096,
                        libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
                        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                        -1,
                        0,
                    )
                };
                if stub_mem != libc::MAP_FAILED {
                    let stub_addr = stub_mem as usize;
                    let return_target = (func_addr + 11) as u64; // 0x1813cc24b
                    let mut stub_bytes: Vec<u8> = Vec::new();

                    // test %rcx, %rcx
                    stub_bytes.extend_from_slice(&[0x48, 0x85, 0xC9]);
                    // jnz +4 (skip xor eax, eax; ret)
                    stub_bytes.extend_from_slice(&[0x75, 0x04]);
                    // xor %eax, %eax; ret
                    stub_bytes.extend_from_slice(&[0x31, 0xC0, 0xC3, 0x90]);

                    // do_prologue: original 11 bytes
                    stub_bytes.extend_from_slice(&expected_prologue);

                    // movabs $return_target, %r11; jmp *%r11
                    stub_bytes.extend_from_slice(&[0x49, 0xBB]);
                    stub_bytes.extend_from_slice(&return_target.to_le_bytes());
                    stub_bytes.extend_from_slice(&[0x41, 0xFF, 0xE3]);

                    unsafe {
                        std::ptr::copy_nonoverlapping(stub_bytes.as_ptr(), stub_mem as *mut u8, stub_bytes.len());

                        // Patch func_addr with jmp rel32 to stub
                        let rel_offset = (stub_addr as isize - (func_addr + 5) as isize) as i32;
                        let mut patch = vec![0xE9];
                        patch.extend_from_slice(&rel_offset.to_le_bytes());
                        patch.extend_from_slice(&[0x90, 0x90, 0x90, 0x90, 0x90, 0x90]); // 6 nops to fill 11 bytes

                        std::ptr::copy_nonoverlapping(patch.as_ptr(), func_addr as *mut u8, 11);
                    }
                    println!(
                        "[RINE MEMORY LOADER] Installed Unity null-queue guard at 0x{:016X} -> stub @ 0x{:016X}",
                        func_addr, stub_addr
                    );
                }
            }
        }

        let is_dll = (pe.file_header.characteristics & 0x2000) != 0;
        let entry_point_rva = pe.optional_header.address_of_entry_point();
        if is_dll && entry_point_rva != 0 {
            let ep_va = allocated_base + entry_point_rva as usize;
            println!(
                "[RINE COMPANION LOADER] Invoking DllMain for '{}' @ 0x{:016X}",
                dll_name, ep_va
            );
            let dll_main: unsafe extern "win64" fn(hinst: u64, reason: u32, reserved: u64) -> i32 =
                unsafe { std::mem::transmute(ep_va) };
            const DLL_PROCESS_ATTACH: u32 = 1;
            let ret = unsafe { dll_main(allocated_base as u64, DLL_PROCESS_ATTACH, 0) };
            println!(
                "[RINE COMPANION LOADER] DllMain for '{}' returned {}",
                dll_name, ret
            );
        }

        Some(allocated_base)
    }

    pub fn lookup_companion_export(dll_name: &str, symbol_name: &str) -> Option<usize> {
        let mut clean_name = dll_name.replace('\\', "/");
        while clean_name.to_ascii_lowercase().ends_with(".dll.dll") {
            clean_name.truncate(clean_name.len() - 4);
        }
        let mut file_name = std::path::Path::new(&clean_name)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| clean_name.clone());
        while file_name.to_ascii_lowercase().ends_with(".dll.dll") {
            file_name.truncate(file_name.len() - 4);
        }
        let name_lower = file_name.to_lowercase();
        let clean_lower = clean_name.to_lowercase();
        Self::load_companion_dll(dll_name);

        let guard = get_companion_modules().lock().unwrap();
        if let Some((base, exports)) = guard.get(&name_lower).or_else(|| guard.get(&clean_lower)) {
            if let Some(rva) = exports.get(symbol_name) {
                return Some(*base + *rva as usize);
            }
        }
        None
    }

    pub fn get_companion_module_name(base: usize) -> Option<String> {
        let guard = get_companion_modules().lock().unwrap();
        for (name, (mod_base, _)) in guard.iter() {
            if *mod_base == base {
                return Some(name.clone());
            }
        }
        None
    }

    pub fn get_companion_module_base(raw_name: &str) -> Option<usize> {
        let clean = raw_name.replace('\\', "/");
        let file_name = std::path::Path::new(&clean)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| raw_name.to_string());
        let name_lower = file_name.to_lowercase();
        let clean_lower = clean.to_lowercase();
        let guard = get_companion_modules().lock().unwrap();
        guard.get(&name_lower).or_else(|| guard.get(&clean_lower)).map(|(b, _)| *b)
    }

    pub fn find_companion_export_anywhere(symbol_name: &str) -> Option<(String, usize)> {
        let guard = get_companion_modules().lock().unwrap();
        for (name, (base, exports)) in guard.iter() {
            if let Some(rva) = exports.get(symbol_name) {
                return Some((name.clone(), *base + *rva as usize));
            }
        }
        None
    }
}

pub fn get_companion_module_name(base: usize) -> Option<String> {
    LoadedModule::get_companion_module_name(base)
}

pub fn get_companion_module_base(raw_name: &str) -> Option<usize> {
    LoadedModule::get_companion_module_base(raw_name)
}

pub fn find_companion_export_anywhere(symbol_name: &str) -> Option<(String, usize)> {
    LoadedModule::find_companion_export_anywhere(symbol_name)
}

static NEXT_MODULE_TLS_INDEX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[derive(Debug, Clone)]
pub struct TlsTemplate {
    pub raw_data: Vec<u8>,
    pub zero_fill_size: usize,
    pub index_ptr_va: u64,
    pub callbacks_va: u64,
    pub tls_index: u32,
}

static REGISTERED_TLS_TEMPLATES: std::sync::Mutex<Vec<TlsTemplate>> =
    std::sync::Mutex::new(Vec::new());

pub fn register_module_tls(
    allocated_base: usize,
    image_size: usize,
    pe: &crate::pe::parser::PeImage,
) -> Option<u32> {
    let tls_dir = &pe.optional_header.data_directory[9]; // IMAGE_DIRECTORY_ENTRY_TLS
    if tls_dir.virtual_address() == 0 || tls_dir.size() == 0 {
        return None;
    }
    let tls_rva = tls_dir.virtual_address() as usize;
    if tls_rva + 40 > image_size {
        return None;
    }

    unsafe {
        let base_ptr = allocated_base as *mut u8;
        let tls_struct = base_ptr.add(tls_rva) as *const u64;
        let start_raw = *tls_struct;
        let end_raw = *tls_struct.add(1);
        let address_of_index = *tls_struct.add(2);
        let address_of_callbacks = *tls_struct.add(3);
        let size_of_zero_fill = *(base_ptr.add(tls_rva + 32) as *const u32);

        let tls_index = NEXT_MODULE_TLS_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Write assigned TLS index into AddressOfIndex
        if address_of_index != 0 {
            let index_ptr = address_of_index as *mut u32;
            *index_ptr = tls_index;
            println!(
                "[RINE TLS ENGINE] Set AddressOfIndex (0x{:016X}) -> TLS Index {}",
                address_of_index, tls_index
            );
        }

        let raw_len = if end_raw > start_raw && start_raw != 0 {
            (end_raw - start_raw) as usize
        } else {
            0
        };

        let raw_bytes = if raw_len > 0 && start_raw != 0 {
            std::slice::from_raw_parts(start_raw as *const u8, raw_len).to_vec()
        } else {
            Vec::new()
        };

        let total_size = (raw_len + size_of_zero_fill as usize).max(512);
        let tls_block = libc::calloc(1, total_size) as *mut u8;
        if !raw_bytes.is_empty() {
            std::ptr::copy_nonoverlapping(raw_bytes.as_ptr(), tls_block, raw_bytes.len());
        }

        // Initialize current thread's TLS slot in TEB
        let teb = crate::dlls::kernel32::get_teb_ptr();
        if !teb.is_null() && (teb as usize) >= 0x10000 {
            let tls_array_ptr = (*teb).thread_local_storage_pointer as *mut u64;
            if !tls_array_ptr.is_null() && (tls_index as usize) < 64 {
                *tls_array_ptr.add(tls_index as usize) = tls_block as u64;
                println!(
                    "[RINE TLS ENGINE] Allocated TLS Block for Module (Index {}) @ 0x{:016X} (size {}) in TEB 0x{:016X}",
                    tls_index, tls_block as u64, total_size, teb as usize
                );
            }
        }

        // Save template for future threads
        let mut guard = REGISTERED_TLS_TEMPLATES.lock().unwrap();
        guard.push(TlsTemplate {
            raw_data: raw_bytes,
            zero_fill_size: size_of_zero_fill as usize,
            index_ptr_va: address_of_index,
            callbacks_va: address_of_callbacks,
            tls_index,
        });

        // Run TLS callbacks
        if address_of_callbacks != 0 {
            let mut cb_ptr = address_of_callbacks as *const u64;
            while *cb_ptr != 0 {
                let cb_va = *cb_ptr;
                println!("[RINE TLS ENGINE] Invoking TLS Callback @ 0x{:016X}", cb_va);
                let cb: unsafe extern "win64" fn(hinst: u64, reason: u32, reserved: u64) =
                    std::mem::transmute(cb_va);
                const DLL_PROCESS_ATTACH: u32 = 1;
                cb(allocated_base as u64, DLL_PROCESS_ATTACH, 0);
                cb_ptr = cb_ptr.add(1);
            }
        }

        Some(tls_index)
    }
}

pub fn init_thread_tls(tls_array_addr: u64) {
    if tls_array_addr == 0 {
        return;
    }
    let guard = REGISTERED_TLS_TEMPLATES.lock().unwrap();
    let tls_array_ptr = tls_array_addr as *mut u64;
    for tmpl in guard.iter() {
        let total_size = (tmpl.raw_data.len() + tmpl.zero_fill_size).max(512);
        unsafe {
            let tls_block = libc::calloc(1, total_size) as *mut u8;
            if !tmpl.raw_data.is_empty() {
                std::ptr::copy_nonoverlapping(
                    tmpl.raw_data.as_ptr(),
                    tls_block,
                    tmpl.raw_data.len(),
                );
            }
            if (tmpl.tls_index as usize) < 64 {
                *tls_array_ptr.add(tmpl.tls_index as usize) = tls_block as u64;
            }
        }
    }
}

pub fn register_module_pdata(
    allocated_base: usize,
    image_size: usize,
    pe: &crate::pe::parser::PeImage,
) {
    let mut pdata_rva = 0usize;
    let mut pdata_size = 0usize;

    // 1. Try Exception Directory (data_directory[3] = IMAGE_DIRECTORY_ENTRY_EXCEPTION)
    if pe.optional_header.data_directory.len() > 3 {
        let exc_dir = &pe.optional_header.data_directory[3];
        pdata_rva = exc_dir.virtual_address() as usize;
        pdata_size = exc_dir.size() as usize;
    }

    // 2. Fallback: search for .pdata section
    if pdata_rva == 0 || pdata_size == 0 {
        for section in pe.sections {
            if section.name_str().starts_with(".pdata") {
                pdata_rva = section.virtual_address() as usize;
                pdata_size = (if section.virtual_size() > 0 {
                    section.virtual_size()
                } else {
                    section.size_of_raw_data()
                }) as usize;
                break;
            }
        }
    }

    if pdata_rva != 0 && pdata_size >= 12 && pdata_rva + pdata_size <= image_size {
        let entry_count = (pdata_size / 12) as u32; // sizeof(RUNTIME_FUNCTION) = 12
        let table_addr = allocated_base + pdata_rva;
        unsafe {
            crate::dlls::kernel32::RtlAddFunctionTable(table_addr, entry_count, allocated_base);
        }
        println!(
            "[RINE SEH] Registered PE Exception Directory (.pdata): table=0x{:X}, count={}, base=0x{:X}",
            table_addr, entry_count, allocated_base
        );
    }
}
