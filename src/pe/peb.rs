#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UnicodeString64 {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: u64, // Pointer to UTF-16 buffer
}

impl UnicodeString64 {
    #[inline]
    pub fn length(&self) -> u16 {
        self.length
    }
    #[inline]
    pub fn maximum_length(&self) -> u16 {
        self.maximum_length
    }
    #[inline]
    pub fn buffer(&self) -> u64 {
        self.buffer
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ListEntry64 {
    pub flink: u64,
    pub blink: u64,
}

impl ListEntry64 {
    #[inline]
    pub fn flink(&self) -> u64 {
        self.flink
    }
    #[inline]
    pub fn blink(&self) -> u64 {
        self.blink
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PebLdrData64 {
    pub length: u32,
    pub initialized: u8,
    pub ss_handle: u64,
    pub in_load_order_module_list: ListEntry64,
    pub in_memory_order_module_list: ListEntry64,
    pub in_initialization_order_module_list: ListEntry64,
    pub entry_in_progress: u64,
    pub shutdown_in_progress: u8,
    pub shutdown_thread_id: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RtlUserProcessParameters64 {
    pub maximum_length: u32,
    pub length: u32,
    pub flags: u32,
    pub debug_flags: u32,
    pub console_handle: u64,
    pub console_flags: u32,
    pub standard_input: u64,
    pub standard_output: u64,
    pub standard_error: u64,
    pub current_directory_path: UnicodeString64,
    pub current_directory_handle: u64,
    pub dll_path: UnicodeString64,
    pub image_path_name: UnicodeString64,
    pub command_line: UnicodeString64,
    pub environment: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Peb64 {
    pub inherited_address_space: u8,             /* 0000 */
    pub read_image_file_exec_options: u8,        /* 0001 */
    pub being_debugged: u8,                      /* 0002 */
    pub bit_flags: u8,                           /* 0003 */
    pub padding0: [u8; 4],                       /* 0004 */
    pub mutant: u64,                             /* 0008 */
    pub image_base_address: u64,                 /* 0010 */
    pub ldr_data: u64,                           /* 0018 */
    pub process_parameters: u64,                 /* 0020 */
    pub sub_system_data: u64,                    /* 0028 */
    pub process_heap: u64,                       /* 0030 */
    pub fast_peb_lock: u64,                      /* 0038 */
    pub atl_thunk_slist_ptr: u64,                /* 0040 */
    pub ifeo_key: u64,                           /* 0048 */
    pub process_in_job: u32,                     /* 0050 */
    pub kernel_callback_table: u64,              /* 0058 */
    pub reserved: u32,                           /* 0060 */
    pub atl_thunk_slist_ptr32: u32,              /* 0064 */
    pub api_set_map: u64,                        /* 0068 */
    pub tls_expansion_counter: u32,              /* 0070 */
    pub padding1: u32,                           /* 0074 */
    pub tls_bitmap: u64,                         /* 0078 */
    pub tls_bitmap_bits: [u32; 2],               /* 0080 */
    pub read_only_shared_memory_base: u64,       /* 0088 */
    pub shared_data: u64,                        /* 0090 */
    pub read_only_static_server_data: u64,       /* 0098 */
    pub ansi_code_page_data: u64,                /* 00a0 */
    pub oem_code_page_data: u64,                 /* 00a8 */
    pub unicode_case_table_data: u64,            /* 00b0 */
    pub number_of_processors: u32,               /* 00b8 */
    pub nt_global_flag: u32,                     /* 00bc */
    pub critical_section_timeout: u64,           /* 00c0 */
    pub heap_segment_reserve: u64,               /* 00c8 */
    pub heap_segment_commit: u64,                /* 00d0 */
    pub heap_decommit_total_free_threshold: u64, /* 00d8 */
    pub heap_decommit_free_block_threshold: u64, /* 00e0 */
    pub number_of_heaps: u32,                    /* 00e8 */
    pub maximum_number_of_heaps: u32,            /* 00ec */
    pub process_heaps: u64,                      /* 00f0 */
    pub gdi_shared_handle_table: u64,            /* 00f8 */
    pub process_starter_helper: u64,             /* 0100 */
    pub gdi_dc_attribute_list: u64,              /* 0108 */
    pub loader_lock: u64,                        /* 0110 */
    pub os_major_version: u32,                   /* 0118 */
    pub os_minor_version: u32,                   /* 011c */
    pub os_build_number: u16,                    /* 0120 */
    pub os_csd_version: u16,                     /* 0122 */
    pub os_platform_id: u32,                     /* 0124 */
    pub image_subsystem: u32,                    /* 0128 */
    pub image_subsystem_major_version: u32,      /* 012c */
    pub image_subsystem_minor_version: u32,      /* 0130 */
    pub padding2: u32,                           /* 0134 */
    pub active_process_affinity_mask: u64,       /* 0138 */
}

impl Peb64 {
    #[inline]
    pub fn image_base_address(&self) -> u64 {
        self.image_base_address
    }
    #[inline]
    pub fn ldr_data(&self) -> u64 {
        self.ldr_data
    }
    #[inline]
    pub fn process_parameters(&self) -> u64 {
        self.process_parameters
    }
    #[inline]
    pub fn process_heap(&self) -> u64 {
        self.process_heap
    }
    #[inline]
    pub fn number_of_processors(&self) -> u32 {
        self.number_of_processors
    }
    #[inline]
    pub fn os_major_version(&self) -> u32 {
        self.os_major_version
    }
    #[inline]
    pub fn os_minor_version(&self) -> u32 {
        self.os_minor_version
    }
    #[inline]
    pub fn os_build_number(&self) -> u16 {
        self.os_build_number
    }
}
