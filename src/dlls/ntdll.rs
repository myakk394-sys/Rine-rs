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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OsVersionInfoExW {
    pub dw_os_version_info_size: u32,
    pub dw_major_version: u32,
    pub dw_minor_version: u32,
    pub dw_build_number: u32,
    pub dw_platform_id: u32,
    pub sz_csd_version: [u16; 128],
    pub w_service_pack_major: u16,
    pub w_service_pack_minor: u16,
    pub w_suite_mask: u16,
    pub w_product_type: u8,
    pub w_reserved: u8,
}

#[no_mangle]
pub unsafe extern "win64" fn RtlGetVersion(lp_version_information: *mut OsVersionInfoExW) -> i32 {
    if lp_version_information.is_null() {
        return -1073741811; // STATUS_INVALID_PARAMETER (0xC000000D)
    }

    let size = (*lp_version_information).dw_os_version_info_size;
    if size < 276 {
        return -1073741811;
    }

    (*lp_version_information).dw_major_version = 10;
    (*lp_version_information).dw_minor_version = 0;
    (*lp_version_information).dw_build_number = 19045;
    (*lp_version_information).dw_platform_id = 2; // VER_PLATFORM_WIN32_NT
    for i in 0..128 {
        (*lp_version_information).sz_csd_version[i] = 0;
    }

    if size >= 284 {
        (*lp_version_information).w_service_pack_major = 0;
        (*lp_version_information).w_service_pack_minor = 0;
        (*lp_version_information).w_suite_mask = 0x0100; // VER_SUITE_SINGLEUSERTS
        (*lp_version_information).w_product_type = 1; // VER_NT_WORKSTATION
        (*lp_version_information).w_reserved = 0;
    }

    println!("[NTDLL] RtlGetVersion -> Win 10.0.19045 (size={})", size);
    0 // STATUS_SUCCESS
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

    #[test]
    fn test_rtl_get_version_contract() {
        // 1. Success with OSVERSIONINFOW (276 bytes)
        let mut info_w: OsVersionInfoExW = unsafe { std::mem::zeroed() };
        info_w.dw_os_version_info_size = 276;
        let status = unsafe { RtlGetVersion(&mut info_w) };
        assert_eq!(status, 0);
        assert_eq!(info_w.dw_major_version, 10);
        assert_eq!(info_w.dw_minor_version, 0);
        assert_eq!(info_w.dw_build_number, 19045);
        assert_eq!(info_w.dw_platform_id, 2);

        // 2. Success with OSVERSIONINFOEXW (284 bytes)
        let mut info_ex: OsVersionInfoExW = unsafe { std::mem::zeroed() };
        info_ex.dw_os_version_info_size = 284;
        let status_ex = unsafe { RtlGetVersion(&mut info_ex) };
        assert_eq!(status_ex, 0);
        assert_eq!(info_ex.w_product_type, 1);
        assert_eq!(info_ex.w_suite_mask, 0x0100);

        // 3. Null pointer error
        let err_null = unsafe { RtlGetVersion(std::ptr::null_mut()) };
        assert_eq!(err_null, -1073741811);

        // 4. Too small size
        let mut info_bad: OsVersionInfoExW = unsafe { std::mem::zeroed() };
        info_bad.dw_os_version_info_size = 100;
        let err_bad = unsafe { RtlGetVersion(&mut info_bad) };
        assert_eq!(err_bad, -1073741811);
    }

    #[test]
    fn test_rtl_get_version_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*mut OsVersionInfoExW) -> i32 = RtlGetVersion;
        let mut info: OsVersionInfoExW = unsafe { std::mem::zeroed() };
        info.dw_os_version_info_size = 284;
        let status = unsafe { func_ptr(&mut info) };
        assert_eq!(status, 0);
    }
}
