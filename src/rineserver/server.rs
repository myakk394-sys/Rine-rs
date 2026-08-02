use crate::rineserver::handles::HandleTable;
use crate::rineserver::objects::KernelObject;
use crate::rineserver::protocol::{RineServerReply, RineServerRequest};
use crate::sys::{STATUS_INVALID_HANDLE, STATUS_SUCCESS};
use std::sync::{Arc, OnceLock};

pub struct RineServer {
    pub handle_table: HandleTable,
}

static RINESERVER_INSTANCE: OnceLock<Arc<RineServer>> = OnceLock::new();

impl RineServer {
    pub fn get_instance() -> Arc<Self> {
        RINESERVER_INSTANCE
            .get_or_init(|| {
                Arc::new(Self {
                    handle_table: HandleTable::new(),
                })
            })
            .clone()
    }

    /// Dispatches IPC requests to rineserver kernel objects and handle table.
    pub fn dispatch(&self, req: RineServerRequest) -> RineServerReply {
        match req {
            RineServerRequest::CreateEvent {
                manual_reset,
                initial_state,
                name,
            } => {
                let obj = KernelObject::new_event(manual_reset, initial_state, &name);
                let handle = self.handle_table.allocate(obj);
                RineServerReply::HandleCreated {
                    handle,
                    status: STATUS_SUCCESS,
                }
            }
            RineServerRequest::SetEvent { handle } => {
                if let Some(obj) = self.handle_table.get(handle) {
                    let status = obj.set_event();
                    RineServerReply::EventStatus { status }
                } else {
                    RineServerReply::Error {
                        status: STATUS_INVALID_HANDLE,
                    }
                }
            }
            RineServerRequest::ResetEvent { handle } => {
                if let Some(obj) = self.handle_table.get(handle) {
                    let status = obj.reset_event();
                    RineServerReply::EventStatus { status }
                } else {
                    RineServerReply::Error {
                        status: STATUS_INVALID_HANDLE,
                    }
                }
            }
            RineServerRequest::CreateMutant {
                initial_owner,
                name,
            } => {
                let tid = unsafe { libc::gettid() } as u64;
                let obj = KernelObject::new_mutex(initial_owner, tid, &name);
                let handle = self.handle_table.allocate(obj);
                RineServerReply::HandleCreated {
                    handle,
                    status: STATUS_SUCCESS,
                }
            }
            RineServerRequest::ReleaseMutant {
                handle,
                current_tid,
            } => {
                if let Some(obj) = self.handle_table.get(handle) {
                    let status = obj.release_mutex(current_tid);
                    RineServerReply::EventStatus { status }
                } else {
                    RineServerReply::Error {
                        status: STATUS_INVALID_HANDLE,
                    }
                }
            }
            RineServerRequest::CreateSemaphore {
                initial_count,
                maximum_count,
                name,
            } => {
                let obj = KernelObject::new_semaphore(initial_count, maximum_count, &name);
                let handle = self.handle_table.allocate(obj);
                RineServerReply::HandleCreated {
                    handle,
                    status: STATUS_SUCCESS,
                }
            }
            RineServerRequest::ReleaseSemaphore { handle, count } => {
                if let Some(obj) = self.handle_table.get(handle) {
                    let mut prev = 0;
                    let status = obj.release_semaphore(count, Some(&mut prev));
                    RineServerReply::SemaphoreReleased {
                        previous_count: prev,
                        status,
                    }
                } else {
                    RineServerReply::Error {
                        status: STATUS_INVALID_HANDLE,
                    }
                }
            }
            RineServerRequest::CreateSection {
                size,
                memory_address,
                name,
            } => {
                let obj = KernelObject::new_section(size, memory_address, &name);
                let handle = self.handle_table.allocate(obj);
                RineServerReply::HandleCreated {
                    handle,
                    status: STATUS_SUCCESS,
                }
            }
            RineServerRequest::CreateProcess {
                pid,
                is_wow64,
                name,
            } => {
                let obj = KernelObject::new_process(pid, is_wow64, &name);
                let handle = self.handle_table.allocate(obj);
                RineServerReply::HandleCreated {
                    handle,
                    status: STATUS_SUCCESS,
                }
            }
            RineServerRequest::CreateThread {
                tid,
                teb_address,
                name,
            } => {
                let obj = KernelObject::new_thread(tid, teb_address, &name);
                let handle = self.handle_table.allocate(obj);
                RineServerReply::HandleCreated {
                    handle,
                    status: STATUS_SUCCESS,
                }
            }
            RineServerRequest::CloseHandle { handle } => {
                let status = self.handle_table.close(handle);
                RineServerReply::HandleClosed { status }
            }
            RineServerRequest::DuplicateHandle { handle } => {
                match self.handle_table.duplicate(handle) {
                    Ok(new_handle) => RineServerReply::HandleDuplicated {
                        new_handle,
                        status: STATUS_SUCCESS,
                    },
                    Err(status) => RineServerReply::Error { status },
                }
            }
            RineServerRequest::QueryServerStatus => {
                let active_handles = self.handle_table.len();
                RineServerReply::ServerStatusResponse {
                    active_handles,
                    status: STATUS_SUCCESS,
                }
            }
        }
    }
}
