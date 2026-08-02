#![allow(non_snake_case)]
use crate::mem::MmapRegion;
use crate::server::{RineServer, ServerReply, ServerRequest};
use crate::sys::{WinPageProtection, NtStatus, STATUS_INVALID_HANDLE, STATUS_SUCCESS};

pub fn NtAllocateVirtualMemory(
    target_address: &mut usize,
    size: usize,
    win_protection: WinPageProtection,
) -> Result<MmapRegion, NtStatus> {
    let posix_prot = win_protection.to_posix_prot();
    let preferred = if *target_address != 0 { Some(*target_address) } else { None };

    match MmapRegion::allocate_anonymous(preferred, size, posix_prot) {
        Ok(region) => {
            *target_address = region.as_ptr() as usize;
            Ok(region)
        }
        Err(_) => Err(crate::sys::STATUS_NO_MEMORY),
    }
}

pub fn NtProtectVirtualMemory(
    region: &mut MmapRegion,
    offset: usize,
    length: usize,
    win_protection: WinPageProtection,
) -> NtStatus {
    let posix_prot = win_protection.to_posix_prot();
    match region.protect(offset, length, posix_prot) {
        Ok(_) => STATUS_SUCCESS,
        Err(_) => crate::sys::STATUS_ACCESS_VIOLATION,
    }
}

pub fn NtCreateSection(
    section_handle: &mut u32,
    size: u64,
    memory_address: usize,
    name: &str,
) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(ServerRequest::CreateSection {
        size,
        memory_address,
        name: name.to_string(),
    });

    if let ServerReply::HandleCreated { handle, status } = reply {
        *section_handle = handle;
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}
