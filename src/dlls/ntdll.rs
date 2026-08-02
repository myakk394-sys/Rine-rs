#![allow(non_snake_case)]
use crate::mem::MmapRegion;
use crate::rineserver::{RineServer, RineServerReply, RineServerRequest};
use crate::sys::{
    NtStatus, WinPageProtection, STATUS_ACCESS_VIOLATION, STATUS_INVALID_HANDLE, STATUS_NO_MEMORY,
    STATUS_SUCCESS,
};

pub fn NtCreateEvent(
    event_handle: &mut u32,
    manual_reset: bool,
    initial_state: bool,
    name: &str,
) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(RineServerRequest::CreateEvent {
        manual_reset,
        initial_state,
        name: name.to_string(),
    });

    if let RineServerReply::HandleCreated { handle, status } = reply {
        *event_handle = handle;
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtSetEvent(event_handle: u32) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(RineServerRequest::SetEvent {
        handle: event_handle,
    });

    if let RineServerReply::EventStatus { status } = reply {
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtResetEvent(event_handle: u32) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(RineServerRequest::ResetEvent {
        handle: event_handle,
    });

    if let RineServerReply::EventStatus { status } = reply {
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtCreateMutant(mutex_handle: &mut u32, initial_owner: bool, name: &str) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(RineServerRequest::CreateMutant {
        initial_owner,
        name: name.to_string(),
    });

    if let RineServerReply::HandleCreated { handle, status } = reply {
        *mutex_handle = handle;
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtReleaseMutant(mutex_handle: u32) -> NtStatus {
    let current_tid = unsafe { libc::gettid() } as u64;
    let server = RineServer::get_instance();
    let reply = server.dispatch(RineServerRequest::ReleaseMutant {
        handle: mutex_handle,
        current_tid,
    });

    if let RineServerReply::EventStatus { status } = reply {
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub fn NtAllocateVirtualMemory(
    target_address: &mut usize,
    size: usize,
    win_protection: WinPageProtection,
) -> Result<MmapRegion, NtStatus> {
    let posix_prot = win_protection.to_posix_prot();
    let preferred = if *target_address != 0 {
        Some(*target_address)
    } else {
        None
    };

    match MmapRegion::allocate_anonymous(preferred, size, posix_prot) {
        Ok(region) => {
            *target_address = region.as_ptr() as usize;
            Ok(region)
        }
        Err(_) => Err(STATUS_NO_MEMORY),
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
        Err(_) => STATUS_ACCESS_VIOLATION,
    }
}

pub fn NtClose(handle: u32) -> NtStatus {
    let server = RineServer::get_instance();
    let reply = server.dispatch(RineServerRequest::CloseHandle { handle });

    if let RineServerReply::HandleClosed { status } = reply {
        status
    } else {
        STATUS_INVALID_HANDLE
    }
}

pub unsafe extern "win64" fn RtlAllocateHeap(heap: usize, flags: u32, size: usize) -> usize {
    crate::dlls::kernel32::HeapAlloc(heap, flags, size)
}

pub unsafe extern "win64" fn RtlFreeHeap(heap: usize, flags: u32, ptr: usize) -> u8 {
    crate::dlls::kernel32::HeapFree(heap, flags, ptr) as u8
}

pub unsafe extern "win64" fn RtlReAllocateHeap(
    heap: usize,
    flags: u32,
    ptr: usize,
    size: usize,
) -> usize {
    crate::dlls::kernel32::HeapReAlloc(heap, flags, ptr, size)
}

pub unsafe extern "win64" fn RtlSizeHeap(heap: usize, flags: u32, ptr: usize) -> usize {
    crate::dlls::kernel32::HeapSize(heap, flags, ptr)
}

pub fn RtlLookupFunctionEntry(
    _control_pc: u64,
    image_base_out: &mut u64,
    _history_table: u64,
) -> u64 {
    *image_base_out =
        crate::dlls::kernel32::CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    0 // Return NULL (No C++ unwind entry required)
}

pub fn RtlVirtualUnwind(
    _handler_type: u32,
    _image_base: u64,
    _control_pc: u64,
    _fn_entry: u64,
    _context_frame: u64,
    _handler_data: u64,
    _establisher_frame: u64,
    _ctx_pointers: u64,
) -> u64 {
    0
}

pub unsafe extern "win64" fn RtlInitializeCriticalSection(
    cs: *mut crate::dlls::kernel32::RtlCriticalSection,
) -> i32 {
    crate::dlls::kernel32::InitializeCriticalSection(cs);
    0 // STATUS_SUCCESS
}

pub unsafe extern "win64" fn RtlInitializeCriticalSectionAndSpinCount(
    cs: *mut crate::dlls::kernel32::RtlCriticalSection,
    spin: u32,
) -> i32 {
    crate::dlls::kernel32::InitializeCriticalSectionAndSpinCount(cs, spin);
    0
}

pub unsafe extern "win64" fn RtlInitializeCriticalSectionEx(
    cs: *mut crate::dlls::kernel32::RtlCriticalSection,
    spin: u32,
    flags: u32,
) -> i32 {
    crate::dlls::kernel32::InitializeCriticalSectionEx(cs, spin, flags);
    0
}

pub unsafe extern "win64" fn RtlEnterCriticalSection(
    cs: *mut crate::dlls::kernel32::RtlCriticalSection,
) -> i32 {
    crate::dlls::kernel32::EnterCriticalSection(cs);
    0
}

pub unsafe extern "win64" fn RtlLeaveCriticalSection(
    cs: *mut crate::dlls::kernel32::RtlCriticalSection,
) -> i32 {
    crate::dlls::kernel32::LeaveCriticalSection(cs);
    0
}

pub unsafe extern "win64" fn RtlDeleteCriticalSection(
    cs: *mut crate::dlls::kernel32::RtlCriticalSection,
) -> i32 {
    crate::dlls::kernel32::DeleteCriticalSection(cs);
    0
}

pub unsafe extern "win64" fn RtlTryEnterCriticalSection(
    cs: *mut crate::dlls::kernel32::RtlCriticalSection,
) -> u8 {
    crate::dlls::kernel32::TryEnterCriticalSection(cs) as u8
}

pub unsafe extern "win64" fn RtlInitializeSRWLock(lock: *mut usize) {
    crate::dlls::kernel32::InitializeSRWLock(lock);
}

pub unsafe extern "win64" fn RtlAcquireSRWLockExclusive(lock: *mut usize) {
    crate::dlls::kernel32::AcquireSRWLockExclusive(lock);
}

pub unsafe extern "win64" fn RtlReleaseSRWLockExclusive(lock: *mut usize) {
    crate::dlls::kernel32::ReleaseSRWLockExclusive(lock);
}

pub unsafe extern "win64" fn RtlAcquireSRWLockShared(lock: *mut usize) {
    crate::dlls::kernel32::AcquireSRWLockShared(lock);
}

pub unsafe extern "win64" fn RtlReleaseSRWLockShared(lock: *mut usize) {
    crate::dlls::kernel32::ReleaseSRWLockShared(lock);
}

pub unsafe extern "win64" fn RtlTryAcquireSRWLockExclusive(lock: *mut usize) -> u8 {
    crate::dlls::kernel32::TryAcquireSRWLockExclusive(lock)
}

pub unsafe extern "win64" fn RtlTryAcquireSRWLockShared(lock: *mut usize) -> u8 {
    crate::dlls::kernel32::TryAcquireSRWLockShared(lock)
}

pub unsafe extern "win64" fn RtlEncodePointer(ptr: usize) -> usize {
    crate::dlls::kernel32::EncodePointer(ptr)
}

pub unsafe extern "win64" fn RtlDecodePointer(ptr: usize) -> usize {
    crate::dlls::kernel32::DecodePointer(ptr)
}

pub unsafe extern "win64" fn RtlEncodeSystemPointer(ptr: usize) -> usize {
    crate::dlls::kernel32::EncodeSystemPointer(ptr)
}

pub unsafe extern "win64" fn RtlDecodeSystemPointer(ptr: usize) -> usize {
    crate::dlls::kernel32::DecodeSystemPointer(ptr)
}

pub fn RtlCaptureContext(_context_record: u64) -> () {
    println!("[NTDLL] RtlCaptureContext captured context record");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ntdll_event_creation() {
        let mut handle = 0;
        let status = NtCreateEvent(&mut handle, true, false, "TestNtEvent");
        assert_eq!(status, STATUS_SUCCESS);
        assert!(handle >= 4);
        assert_eq!(NtSetEvent(handle), STATUS_SUCCESS);
        assert_eq!(NtResetEvent(handle), STATUS_SUCCESS);
        assert_eq!(NtClose(handle), STATUS_SUCCESS);
    }

    #[test]
    fn test_ntdll_mutex_creation() {
        let mut handle = 0;
        let status = NtCreateMutant(&mut handle, false, "TestNtMutex");
        assert_eq!(status, STATUS_SUCCESS);
        assert!(handle >= 4);
        assert_eq!(NtClose(handle), STATUS_SUCCESS);
    }

    #[test]
    fn test_ntdll_virtual_memory() {
        let mut addr = 0;
        let region = NtAllocateVirtualMemory(&mut addr, 4096, WinPageProtection::READWRITE);
        assert!(region.is_ok());
        assert!(addr > 0);
    }
}
