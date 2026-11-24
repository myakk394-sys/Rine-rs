use crate::mem::mmap::MmapRegion;
use crate::pe::peb::{Peb64, RtlUserProcessParameters64};
use crate::pe::teb::{ClientId64, NtTib64, Teb64};
use crate::sys::{Result, PAGE_SIZE};

pub struct ProcessEnvironment {
    pub peb_region: MmapRegion,
    pub teb_region: MmapRegion,
    pub peb_address: u64,
    pub teb_address: u64,
}

pub fn init_fixed_zero_page_at_10000() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        let ptr = libc::mmap(
            0x10000 as *mut libc::c_void,
            65536,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED,
            -1,
            0,
        );
        if ptr != libc::MAP_FAILED {
            std::ptr::write_bytes(ptr as *mut u8, 0, 65536);
            let buf = ptr as *mut u64;
            *buf = 1;
            *buf.add(1) = 1;
            *buf.add(2) = (ptr as u64) + 32;
            println!("[RINE MEMORY ENGINE] Successfully mapped 64KB Win64 Resilient Process Buffer at fixed address 0x10000.");
        }
    });
}

impl ProcessEnvironment {
    /// Initializes PEB64 and TEB64 virtual memory blocks for a loaded Win64 image.
    pub fn new(image_base: u64, subsystem: u32) -> Result<Self> {
        init_fixed_zero_page_at_10000();

        // 1. Allocate PEB block (1 page = 4096 bytes)
        let peb_region =
            MmapRegion::allocate_anonymous(None, PAGE_SIZE, libc::PROT_READ | libc::PROT_WRITE)?;
        let peb_addr = peb_region.as_ptr() as u64;

        // 2. Allocate TEB block (64KB = 16 pages to cover TlsSlots at 0x1480 and expansion)
        let teb_region =
            MmapRegion::allocate_anonymous(None, PAGE_SIZE * 16, libc::PROT_READ | libc::PROT_WRITE)?;
        let teb_addr = teb_region.as_ptr() as u64;

        // 3. Initialize Process Parameters
        let process_params_offset = std::mem::size_of::<Peb64>();
        let process_params_addr = peb_addr + process_params_offset as u64;

        let env_str = "PATH=C:\\Windows\\System32\0\0";
        let env_bytes = env_str.as_bytes();
        let env_buf = unsafe { libc::malloc(env_bytes.len()) as *mut u8 };
        if !env_buf.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(env_bytes.as_ptr(), env_buf, env_bytes.len());
            }
        }

        static CMDLINE_UTF16: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
        let cmd_wide = CMDLINE_UTF16
            .get_or_init(|| "\"That's not my neighbor.exe\"\0".encode_utf16().collect());

        let cmd_len = ((cmd_wide.len() - 1) * 2) as u16;
        let cmd_max = (cmd_wide.len() * 2) as u16;

        let command_line_str = crate::pe::peb::UnicodeString64 {
            length: cmd_len,
            maximum_length: cmd_max,
            buffer: cmd_wide.as_ptr() as u64,
        };

        let process_params = RtlUserProcessParameters64 {
            maximum_length: std::mem::size_of::<RtlUserProcessParameters64>() as u32,
            length: std::mem::size_of::<RtlUserProcessParameters64>() as u32,
            flags: 0,
            debug_flags: 0,
            console_handle: 0,
            console_flags: 0,
            standard_input: 0x0000000000000004,
            standard_output: 0x0000000000000008,
            standard_error: 0x000000000000000C,
            current_directory_path: command_line_str,
            current_directory_handle: 0,
            dll_path: command_line_str,
            image_path_name: command_line_str,
            command_line: command_line_str,
            environment: env_buf as u64,
        };

        // 4. Initialize PEB64
        let process_heap_addr =
            unsafe { crate::dlls::kernel32::VirtualAlloc(0x0000000180000000, 65536, 0x1000, 0x04) };
        let peb = Peb64 {
            inherited_address_space: 0,
            read_image_file_exec_options: 0,
            being_debugged: 0,
            bit_flags: 0,
            padding0: [0; 4],
            mutant: 0,
            image_base_address: image_base,
            ldr_data: 0,
            process_parameters: process_params_addr,
            sub_system_data: 0,
            process_heap: process_heap_addr as u64, // Valid PEB ProcessHeap address
            fast_peb_lock: 0,
            atl_thunk_slist_ptr: 0,
            ifeo_key: 0,
            process_in_job: 0,
            kernel_callback_table: 0,
            reserved: 0,
            atl_thunk_slist_ptr32: 0,
            api_set_map: 0,
            tls_expansion_counter: 0,
            padding1: 0,
            tls_bitmap: 0,
            tls_bitmap_bits: [0; 2],
            read_only_shared_memory_base: 0,
            shared_data: 0,
            read_only_static_server_data: 0,
            ansi_code_page_data: 0,
            oem_code_page_data: 0,
            unicode_case_table_data: 0,
            number_of_processors: 8,
            nt_global_flag: 0,
            critical_section_timeout: 0,
            heap_segment_reserve: 0,
            heap_segment_commit: 0,
            heap_decommit_total_free_threshold: 0,
            heap_decommit_free_block_threshold: 0,
            number_of_heaps: 0,
            maximum_number_of_heaps: 0,
            process_heaps: 0,
            gdi_shared_handle_table: 0,
            process_starter_helper: 0,
            gdi_dc_attribute_list: 0,
            loader_lock: 0,
            os_major_version: 10,
            os_minor_version: 0,
            os_build_number: 19045,
            os_csd_version: 0,
            os_platform_id: 2, // VER_PLATFORM_WIN32_NT
            image_subsystem: subsystem,
            image_subsystem_major_version: 6,
            image_subsystem_minor_version: 0,
            padding2: 0,
            active_process_affinity_mask: 1,
        };

        // Write PEB & ProcessParameters to PEB region
        unsafe {
            let peb_ptr = peb_region.as_mut_ptr() as *mut Peb64;
            std::ptr::write(peb_ptr, peb);

            let params_ptr = peb_region.as_mut_ptr().add(process_params_offset)
                as *mut RtlUserProcessParameters64;
            std::ptr::write(params_ptr, process_params);
        }

        // 5. Initialize TEB64
        let pid = unsafe { libc::getpid() } as u64;
        let tid = unsafe { libc::gettid() } as u64;

        // Allocate TLS array (64 slots * 8 bytes = 512 bytes) inside TEB page
        let tls_array_offset = std::mem::size_of::<Teb64>();
        let tls_array_addr = teb_addr + tls_array_offset as u64;

        let teb = Teb64 {
            tib: NtTib64 {
                exception_list: 0,
                stack_base: 0,
                stack_limit: 0,
                sub_system_tib: 0,
                fiber_data: 0,
                arbitrary_user_pointer: 0,
                self_ptr: teb_addr, // Self pointer for GS segment on x86_64
            },
            environment_pointer: 0,
            client_id: ClientId64 {
                unique_process: pid,
                unique_thread: tid,
            },
            active_rpc_handle: 0,
            thread_local_storage_pointer: tls_array_addr, // Valid TLS slots array pointer
            peb: peb_addr,                                // Link to PEB
            last_error_value: 0,
            count_of_owned_critical_sections: 0,
            cs_client_thread: 0,
            win32_thread_info: 0,
        };

        unsafe {
            let teb_ptr = teb_region.as_mut_ptr() as *mut Teb64;
            std::ptr::write(teb_ptr, teb);
        }

        crate::dlls::kernel32::set_current_peb_address(peb_addr);
        crate::dlls::kernel32::set_current_image_base(image_base);

        Ok(Self {
            peb_region,
            teb_region,
            peb_address: peb_addr,
            teb_address: teb_addr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_environment_peb_teb_link() {
        let env =
            ProcessEnvironment::new(0x0000000140000000, 3).expect("failed to allocate PEB/TEB");

        assert!(env.peb_address > 0);
        assert!(env.teb_address > 0);

        unsafe {
            let peb = &*(env.peb_region.as_ptr() as *const Peb64);
            let teb = &*(env.teb_region.as_ptr() as *const Teb64);

            assert_eq!(peb.image_base_address(), 0x0000000140000000);
            assert_eq!(peb.os_major_version(), 10);
            assert_eq!(peb.os_build_number(), 19045);

            assert_eq!(teb.peb(), env.peb_address);
            assert_eq!(teb.tib.self_ptr(), env.teb_address);
        }
    }
}
