pub mod handles;
pub mod objects;
pub mod protocol;

pub use handles::HandleTable;
pub use objects::KernelObject;
pub use protocol::{ServerReply, ServerRequest};

use crate::sys::{STATUS_INVALID_HANDLE, STATUS_SUCCESS};
use std::sync::{Arc, OnceLock};

pub struct RineServer {
    pub handle_table: HandleTable,
}

static SERVER_INSTANCE: OnceLock<Arc<RineServer>> = OnceLock::new();

impl RineServer {
    pub fn get_instance() -> Arc<Self> {
        SERVER_INSTANCE
            .get_or_init(|| {
                Arc::new(Self {
                    handle_table: HandleTable::new(),
                })
            })
            .clone()
    }

    /// Dispatches an IPC request to rineserver kernel objects and handle table.
    pub fn dispatch(&self, req: ServerRequest) -> ServerReply {
        match req {
            ServerRequest::CreateEvent { manual_reset, initial_state, name } => {
                let obj = KernelObject::new_event(manual_reset, initial_state, &name);
                let handle = self.handle_table.allocate(obj);
                ServerReply::HandleCreated { handle, status: STATUS_SUCCESS }
            }
            ServerRequest::SetEvent { handle } => {
                if let Some(obj) = self.handle_table.get(handle) {
                    let status = obj.set_event();
                    ServerReply::EventStatus { status }
                } else {
                    ServerReply::Error { status: STATUS_INVALID_HANDLE }
                }
            }
            ServerRequest::ResetEvent { handle } => {
                if let Some(obj) = self.handle_table.get(handle) {
                    let status = obj.reset_event();
                    ServerReply::EventStatus { status }
                } else {
                    ServerReply::Error { status: STATUS_INVALID_HANDLE }
                }
            }
            ServerRequest::CreateMutant { initial_owner, name } => {
                let tid = unsafe { libc::gettid() } as u64;
                let obj = KernelObject::new_mutex(initial_owner, tid, &name);
                let handle = self.handle_table.allocate(obj);
                ServerReply::HandleCreated { handle, status: STATUS_SUCCESS }
            }
            ServerRequest::ReleaseMutant { handle, current_tid } => {
                if let Some(obj) = self.handle_table.get(handle) {
                    let status = obj.release_mutex(current_tid);
                    ServerReply::EventStatus { status }
                } else {
                    ServerReply::Error { status: STATUS_INVALID_HANDLE }
                }
            }
            ServerRequest::CreateSemaphore { initial_count, maximum_count, name } => {
                let obj = KernelObject::new_semaphore(initial_count, maximum_count, &name);
                let handle = self.handle_table.allocate(obj);
                ServerReply::HandleCreated { handle, status: STATUS_SUCCESS }
            }
            ServerRequest::ReleaseSemaphore { handle, count } => {
                if let Some(obj) = self.handle_table.get(handle) {
                    let mut prev = 0;
                    let status = obj.release_semaphore(count, Some(&mut prev));
                    ServerReply::SemaphoreReleased { previous_count: prev, status }
                } else {
                    ServerReply::Error { status: STATUS_INVALID_HANDLE }
                }
            }
            ServerRequest::CreateSection { size, memory_address, name } => {
                let obj = KernelObject::new_section(size, memory_address, &name);
                let handle = self.handle_table.allocate(obj);
                ServerReply::HandleCreated { handle, status: STATUS_SUCCESS }
            }
            ServerRequest::CloseHandle { handle } => {
                let status = self.handle_table.close(handle);
                ServerReply::HandleClosed { status }
            }
            ServerRequest::DuplicateHandle { handle } => {
                match self.handle_table.duplicate(handle) {
                    Ok(new_handle) => ServerReply::HandleDuplicated { new_handle, status: STATUS_SUCCESS },
                    Err(status) => ServerReply::Error { status },
                }
            }
            ServerRequest::QueryServerStatus => {
                let active_handles = self.handle_table.len();
                ServerReply::ServerStatusResponse { active_handles, status: STATUS_SUCCESS }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rineserver_handle_lifecycle() {
        let server = RineServer::get_instance();
        let reply = server.dispatch(ServerRequest::CreateEvent {
            manual_reset: true,
            initial_state: false,
            name: "TestEvent".into(),
        });

        if let ServerReply::HandleCreated { handle, status } = reply {
            assert_eq!(status, STATUS_SUCCESS);
            assert!(handle >= 4);

            let set_reply = server.dispatch(ServerRequest::SetEvent { handle });
            if let ServerReply::EventStatus { status } = set_reply {
                assert_eq!(status, STATUS_SUCCESS);
            } else {
                panic!("SetEvent failed");
            }

            let close_reply = server.dispatch(ServerRequest::CloseHandle { handle });
            if let ServerReply::HandleClosed { status } = close_reply {
                assert_eq!(status, STATUS_SUCCESS);
            } else {
                panic!("CloseHandle failed");
            }
        } else {
            panic!("CreateEvent failed");
        }
    }
}
