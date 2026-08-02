use crate::sys::NtStatus;

#[derive(Debug, Clone)]
pub enum ServerRequest {
    CreateEvent { manual_reset: bool, initial_state: bool, name: String },
    SetEvent { handle: u32 },
    ResetEvent { handle: u32 },
    CreateMutant { initial_owner: bool, name: String },
    ReleaseMutant { handle: u32, current_tid: u64 },
    CreateSemaphore { initial_count: u32, maximum_count: u32, name: String },
    ReleaseSemaphore { handle: u32, count: u32 },
    CreateSection { size: u64, memory_address: usize, name: String },
    CloseHandle { handle: u32 },
    DuplicateHandle { handle: u32 },
    QueryServerStatus,
}

#[derive(Debug, Clone)]
pub enum ServerReply {
    HandleCreated { handle: u32, status: NtStatus },
    EventStatus { status: NtStatus },
    SemaphoreReleased { previous_count: u32, status: NtStatus },
    HandleClosed { status: NtStatus },
    HandleDuplicated { new_handle: u32, status: NtStatus },
    ServerStatusResponse { active_handles: usize, status: NtStatus },
    Error { status: NtStatus },
}
