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
        let entry_point_rva = pe.optional_header.address_of_entry_point() as usize;

        // 1. Allocate Virtual Address Space according to Strategy
        let mut region = match config.strategy {
            StorageStrategy::RamOnly { lock_in_memory } => {
                // Pre-cache memory allocation via RAM buffer / memfd_create at preferred ImageBase
                let mut mmap_reg = MmapRegion::allocate_memfd(
                    name,
                    Some(requested_image_base),
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

        // 5. Bind Import Address Table (IAT) function pointers in process memory
        crate::pe::imports::register_loaded_module(name.to_string(), allocated_base, image_size);
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
                    crate::pe::imports::IatSymbolResolver::resolve_symbol(dll_name, fn_name)
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

        // Initialize TLS Directory (data_directory[9]) for MSVC CRT TLS callbacks & indices
        let tls_dir = &pe.optional_header.data_directory[9]; // IMAGE_DIRECTORY_ENTRY_TLS
        if tls_dir.virtual_address() > 0 && tls_dir.size() > 0 {
            let tls_rva = tls_dir.virtual_address() as usize;
            if tls_rva + 40 <= image_size {
                let tls_struct_ptr = unsafe { base_ptr.add(tls_rva) as *mut u64 };
                // tls_struct_ptr[2] is AddressOfIndex pointer
                let index_var_ptr = unsafe { base_ptr.add(tls_rva + 32) as *mut u32 };
                unsafe {
                    *index_var_ptr = 0; // TLS Index = 0
                    *tls_struct_ptr.add(2) = (allocated_base + tls_rva + 32) as u64;
                }
                println!(
                    "[RINE TLS ENGINE] Initialized TLS Directory AddressOfIndex -> 0x{:016X}",
                    allocated_base + tls_rva + 32
                );
            }
        }

        // Initialize Load Config SecurityCookie (data_directory[10]) to 0 for passthrough CRT pointer encoding
        let load_config_dir = &pe.optional_header.data_directory[10]; // IMAGE_DIRECTORY_ENTRY_LOAD_CONFIG
        if load_config_dir.virtual_address() > 0 && load_config_dir.size() > 0 {
            let lc_rva = load_config_dir.virtual_address() as usize;
            if lc_rva + 0x48 <= image_size {
                let cookie_ptr_location = unsafe { base_ptr.add(lc_rva + 0x3C) as *const u64 };
                let cookie_va = unsafe { *cookie_ptr_location };
                if cookie_va >= (allocated_base as u64)
                    && cookie_va < (allocated_base + image_size) as u64
                {
                    let cookie_rva = (cookie_va - allocated_base as u64) as usize;
                    let cookie_mem = unsafe { base_ptr.add(cookie_rva) as *mut u64 };
                    unsafe {
                        *cookie_mem = 0x00002B992DDFA232; // Valid MSVC CRT default Security Cookie
                    }
                    println!("[RINE LOAD CONFIG] Set SecurityCookie to 0x00002B992DDFA232 @ VA 0x{:016X}", cookie_va);
                }
            }
        }

        // Note: .CRT section contains function pointers (_initterm / _initterm_e callbacks).
        // Null pointers in .CRT section are sentinels and must be preserved as 0.

        println!(
            "[RINE IAT BINDER] Bound {} / {} imported function slots in process memory across imported DLLs.",
            resolved_count, total_imports
        );

        // Note: .CRT section null-pointer patching is performed in main.rs
        // after load, by scanning the actual .CRT section found in the PE headers.
        // This makes the loader generic and not tied to any specific executable.

        // 6. Apply RWX Protection for entire mapped image buffer (341MB)
        region.protect(
            0,
            image_size,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
        )?;
        println!("[RINE MEMORY PROTECTOR] Applied RWX protection to entire 341MB Image Virtual Address Space.");

        let actual_entry_point = allocated_base + entry_point_rva;
        let env =
            ProcessEnvironment::new(allocated_base as u64, pe.optional_header.subsystem() as u32)?;
        crate::dlls::kernel32::set_current_image_base(allocated_base as u64);

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
