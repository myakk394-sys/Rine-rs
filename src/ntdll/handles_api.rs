#![allow(non_snake_case)]
use crate::server::{RineServer, ServerReply, ServerRequest};
use crate::sys::{NtStatus, STATUS_INVALID_HANDLE};

pub fn NtClose(handle: u32) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::CloseHandle { handle });

    if let ServerReply::HandleClosed { status } = reply {
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtDuplicateObject(source_handle: u32, target_handle: &mut u32) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::DuplicateHandle { handle: source_handle });

    if let ServerReply::HandleDuplicated { new_handle, status } = reply {
        *target_handle = new_handle;
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}
