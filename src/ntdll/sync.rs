#![allow(non_snake_case)]
use crate::server::{KernelObject, RineServer, ServerReply, ServerRequest};
use crate::sys::{NtStatus, STATUS_INVALID_HANDLE, STATUS_SUCCESS, STATUS_TIMEOUT};
use std::time::Duration;

pub fn NtCreateEvent(
    event_handle: &mut u32,
    manual_reset: bool,
    initial_state: bool,
    name: &str,
) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::CreateEvent {
        manual_reset,
        initial_state,
        name: name.to_string(),
    });

    if let ServerReply::HandleCreated { handle, status } = reply {
        *event_handle = handle;
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtSetEvent(event_handle: u32) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::SetEvent { handle: event_handle });

    if let ServerReply::EventStatus { status } = reply {
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtResetEvent(event_handle: u32) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::ResetEvent { handle: event_handle });

    if let ServerReply::EventStatus { status } = reply {
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtCreateMutant(
    mutex_handle: &mut u32,
    initial_owner: bool,
    name: &str,
) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::CreateMutant {
        initial_owner,
        name: name.to_string(),
    });

    if let ServerReply::HandleCreated { handle, status } = reply {
        *mutex_handle = handle;
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtReleaseMutant(mutex_handle: u32) -> NtStatus {
    let current_tid = unsafe { libc::gettid() } as u64;
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::ReleaseMutant {
        handle: mutex_handle,
        current_tid,
    });

    if let ServerReply::EventStatus { status } = reply {
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtCreateSemaphore(
    semaphore_handle: &mut u32,
    initial_count: u32,
    maximum_count: u32,
    name: &str,
) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::CreateSemaphore {
        initial_count,
        maximum_count,
        name: name.to_string(),
    });

    if let ServerReply::HandleCreated { handle, status } = reply {
        *semaphore_handle = handle;
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtReleaseSemaphore(semaphore_handle: u32, release_count: u32, previous_count: Option<&mut u32>) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::ReleaseSemaphore {
        handle: semaphore_handle,
        count: release_count,
    });

    if let ServerReply::SemaphoreReleased { previous_count: prev, status } = reply {
        if let Some(p) = previous_count {
            *p = prev;
        }
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtWaitForSingleObject(handle: u32, timeout_ms: Option<u64>) -> NtStatus {
    let server = RineServer::get_instance();
    let Some(obj) = server.handle_table.get(handle) else {
        return STATUS_INVALID_HANDLE;
    };

    match obj {
        KernelObject::Event { state, .. } => {
            let (lock, cvar) = &*state;
            let mut event = lock.lock().unwrap();
            while !event.signaled {
                if let Some(ms) = timeout_ms {
                    let result = cvar.wait_timeout(event, Duration::from_millis(ms)).unwrap();
                    event = result.0;
                    if result.1.timed_out() && !event.signaled {
                        return STATUS_TIMEOUT;
                    }
                } else {
                    event = cvar.wait(event).unwrap();
                }
            }
            if !event.manual_reset {
                event.signaled = false;
            }
            STATUS_SUCCESS
        }
        _ => STATUS_SUCCESS,
    }
}
