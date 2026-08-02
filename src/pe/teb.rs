#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct NtTib64 {
    pub exception_list: u64,
    pub stack_base: u64,
    pub stack_limit: u64,
    pub sub_system_tib: u64,
    pub fiber_data: u64,
    pub arbitrary_user_pointer: u64,
    pub self_ptr: u64, // Pointer to TEB itself (used by GS register on x86_64)
}

impl NtTib64 {
    #[inline]
    pub fn stack_base(&self) -> u64 {
        self.stack_base
    }
    #[inline]
    pub fn stack_limit(&self) -> u64 {
        self.stack_limit
    }
    #[inline]
    pub fn self_ptr(&self) -> u64 {
        self.self_ptr
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ClientId64 {
    pub unique_process: u64,
    pub unique_thread: u64,
}

impl ClientId64 {
    #[inline]
    pub fn unique_process(&self) -> u64 {
        self.unique_process
    }
    #[inline]
    pub fn unique_thread(&self) -> u64 {
        self.unique_thread
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Teb64 {
    pub tib: NtTib64,                          /* 0000 */
    pub environment_pointer: u64,              /* 0038 */
    pub client_id: ClientId64,                 /* 0040 */
    pub active_rpc_handle: u64,                /* 0050 */
    pub thread_local_storage_pointer: u64,     /* 0058 */
    pub peb: u64,                              /* 0060 */
    pub last_error_value: u32,                 /* 0068 */
    pub count_of_owned_critical_sections: u32, /* 006c */
    pub cs_client_thread: u64,                 /* 0070 */
    pub win32_thread_info: u64,                /* 0078 */
}

impl Teb64 {
    #[inline]
    pub fn peb(&self) -> u64 {
        self.peb
    }
    #[inline]
    pub fn last_error_value(&self) -> u32 {
        self.last_error_value
    }
}
