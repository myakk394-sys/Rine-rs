#![allow(non_snake_case)]
use crate::mem::MmapRegion;
use crate::rineserver::{RineServer, RineServerReply, RineServerRequest};
use crate::sys::{
    NtStatus, WinPageProtection, STATUS_ACCESS_VIOLATION, STATUS_INVALID_HANDLE, STATUS_NO_MEMORY,
    STATUS_SUCCESS,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[repr(C)]
pub struct IO_STATUS_BLOCK {
    pub status: i32,
    pub _pad: u32,
    pub information: usize,
}

#[derive(Clone, Debug)]
struct NtDirEnumEntry {
    name: String,
    is_dir: bool,
    size: u64,
    time_sec: u64,
    time_nsec: u32,
}

static NT_DIR_ENUM_MAP: Mutex<Option<HashMap<usize, (PathBuf, Vec<NtDirEnumEntry>, usize)>>> =
    Mutex::new(None);

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

pub unsafe extern "win64" fn RtlCreateHeap(
    flags: u32,
    _heap_base: usize,
    reserve_size: usize,
    commit_size: usize,
    _lock: usize,
    _parameters: usize,
) -> usize {
    crate::dlls::kernel32::HeapCreate(flags, commit_size, reserve_size)
}

pub unsafe extern "win64" fn RtlDestroyHeap(heap: usize) -> usize {
    if crate::dlls::kernel32::HeapDestroy(heap) != 0 {
        0
    } else {
        heap
    }
}

pub use crate::dlls::kernel32::{
    RtlAddFunctionTable, RtlAddGrowableFunctionTable, RtlDeleteFunctionTable,
    RtlDeleteGrowableFunctionTable, RtlGrowFunctionTable, RtlInstallFunctionTableCallback,
    RtlLookupFunctionTable,
};

pub unsafe extern "win64" fn RtlLookupFunctionEntry(
    control_pc: usize,
    image_base_out: *mut usize,
    history_table: usize,
) -> usize {
    crate::dlls::kernel32::RtlLookupFunctionEntry(control_pc, image_base_out, history_table)
}

pub unsafe extern "win64" fn RtlVirtualUnwind(
    handler_type: u32,
    image_base: usize,
    control_pc: usize,
    function_entry: usize,
    context_record: usize,
    handler_data: *mut usize,
    establisher_frame: *mut usize,
    context_pointers: usize,
) -> usize {
    crate::dlls::kernel32::RtlVirtualUnwind(
        handler_type,
        image_base,
        control_pc,
        function_entry,
        context_record,
        handler_data,
        establisher_frame,
        context_pointers,
    )
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

pub unsafe extern "win64" fn RtlInitializeConditionVariable(var: *mut usize) {
    crate::dlls::kernel32::InitializeConditionVariable(var);
}

pub unsafe extern "win64" fn RtlWakeConditionVariable(var: *mut usize) {
    crate::dlls::kernel32::WakeConditionVariable(var);
}

pub unsafe extern "win64" fn RtlWakeAllConditionVariable(var: *mut usize) {
    crate::dlls::kernel32::WakeAllConditionVariable(var);
}

pub unsafe extern "win64" fn RtlSleepConditionVariableCS(
    var: *mut usize,
    cs: *mut crate::dlls::kernel32::RtlCriticalSection,
    timeout: *const i64,
) -> i32 {
    let ms = if timeout.is_null() {
        0xFFFFFFFF
    } else if (timeout as usize) < 0x10000 {
        timeout as usize as u32
    } else {
        let t = *timeout;
        if t < 0 {
            ((-t) / 10000).max(1) as u32
        } else {
            (t / 10000) as u32
        }
    };
    let ok = crate::dlls::kernel32::SleepConditionVariableCS(var, cs, ms);
    if ok != 0 {
        0 // STATUS_SUCCESS
    } else {
        0x00000102 // STATUS_TIMEOUT
    }
}

pub unsafe extern "win64" fn RtlSleepConditionVariableSRW(
    var: *mut usize,
    lock: *mut usize,
    timeout: *const i64,
    flags: u32,
) -> i32 {
    let ms = if timeout.is_null() {
        0xFFFFFFFF
    } else if (timeout as usize) < 0x10000 {
        timeout as usize as u32
    } else {
        let t = *timeout;
        if t < 0 {
            ((-t) / 10000).max(1) as u32
        } else {
            (t / 10000) as u32
        }
    };
    let ok = crate::dlls::kernel32::SleepConditionVariableSRW(var, lock, ms, flags);
    if ok != 0 {
        0 // STATUS_SUCCESS
    } else {
        0x00000102 // STATUS_TIMEOUT
    }
}

pub unsafe extern "win64" fn RtlWaitOnAddress(
    addr: *const std::ffi::c_void,
    cmp: *const std::ffi::c_void,
    size: usize,
    timeout: *const i64,
) -> i32 {
    let ms = if timeout.is_null() {
        0xFFFFFFFF
    } else {
        let t = *timeout;
        if t < 0 {
            ((-t) / 10000).max(1) as u32 // 100ns units -> ms
        } else {
            0xFFFFFFFF
        }
    };
    if crate::dlls::kernel32::WaitOnAddress(addr, cmp, size, ms) != 0 {
        0 // STATUS_SUCCESS
    } else {
        0x00000102 // STATUS_TIMEOUT
    }
}

pub unsafe extern "win64" fn RtlWakeAddressSingle(addr: *const std::ffi::c_void) {
    crate::dlls::kernel32::WakeByAddressSingle(addr);
}

pub unsafe extern "win64" fn RtlWakeAddressAll(addr: *const std::ffi::c_void) {
    crate::dlls::kernel32::WakeByAddressAll(addr);
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

#[no_mangle]
pub unsafe extern "win64" fn RtlVerifyVersionInfo(
    _version_info: *const OsVersionInfoExW,
    _type_mask: u32,
    _condition_mask: u64,
) -> i32 {
    0 // STATUS_SUCCESS: version matches expected criteria
}

#[no_mangle]
pub unsafe extern "win64" fn __wine_dbg_output(msg: *const std::ffi::c_char) -> i32 {
    if !msg.is_null() {
        if let Ok(s) = std::ffi::CStr::from_ptr(msg).to_str() {
            print!("[DXVK] {}", s);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn NtSetInformationThread(
    _thread_handle: u64,
    _thread_information_class: u32,
    _thread_information: *const std::ffi::c_void,
    _thread_information_length: u32,
) -> i32 {
    0 // STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn ZwSetInformationThread(
    thread_handle: u64,
    thread_information_class: u32,
    thread_information: *const std::ffi::c_void,
    thread_information_length: u32,
) -> i32 {
    NtSetInformationThread(
        thread_handle,
        thread_information_class,
        thread_information,
        thread_information_length,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtQueryInformationThread(
    _thread_handle: u64,
    _thread_information_class: u32,
    _thread_information: *mut std::ffi::c_void,
    _thread_information_length: u32,
    return_length: *mut u32,
) -> i32 {
    if !return_length.is_null() {
        *return_length = 0;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ZwQueryInformationThread(
    thread_handle: u64,
    thread_information_class: u32,
    thread_information: *mut std::ffi::c_void,
    thread_information_length: u32,
    return_length: *mut u32,
) -> i32 {
    NtQueryInformationThread(
        thread_handle,
        thread_information_class,
        thread_information,
        thread_information_length,
        return_length,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtQueryInformationProcess(
    _process_handle: u64,
    _process_information_class: u32,
    _process_information: *mut std::ffi::c_void,
    _process_information_length: u32,
    return_length: *mut u32,
) -> i32 {
    if !return_length.is_null() {
        *return_length = 0;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ZwQueryInformationProcess(
    process_handle: u64,
    process_information_class: u32,
    process_information: *mut std::ffi::c_void,
    process_information_length: u32,
    return_length: *mut u32,
) -> i32 {
    NtQueryInformationProcess(
        process_handle,
        process_information_class,
        process_information,
        process_information_length,
        return_length,
    )
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UNICODE_STRING {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Buffer: *mut u16,
}

#[no_mangle]
pub unsafe extern "win64" fn NtQueryVirtualMemory(
    _process_handle: u64,
    base_address: u64,
    memory_information_class: u32,
    memory_information: *mut u8,
    memory_information_length: usize,
    return_length: *mut usize,
) -> i32 {
    println!(
        "[NTDLL] NtQueryVirtualMemory: base=0x{:X}, class=0x{:X}, len={}",
        base_address, memory_information_class, memory_information_length
    );
    if memory_information_class == 0 {
        // MemoryBasicInformation
        if !memory_information.is_null()
            && memory_information_length
                >= std::mem::size_of::<crate::dlls::kernel32::MEMORY_BASIC_INFORMATION>()
        {
            let mbi =
                memory_information as *mut crate::dlls::kernel32::MEMORY_BASIC_INFORMATION;
            let page_base = (base_address & !4095) as *mut libc::c_void;
            (*mbi).BaseAddress = page_base;
            (*mbi).AllocationBase = page_base;
            (*mbi).AllocationProtect = 0x40; // PAGE_EXECUTE_READWRITE
            (*mbi).PartitionId = 0;
            (*mbi).RegionSize = 65536;
            (*mbi).State = 0x1000; // MEM_COMMIT
            (*mbi).Protect = 0x40;
            (*mbi).Type = 0x20000; // MEM_PRIVATE
        }
        if !return_length.is_null() {
            *return_length =
                std::mem::size_of::<crate::dlls::kernel32::MEMORY_BASIC_INFORMATION>();
        }
        return 0; // STATUS_SUCCESS
    } else if memory_information_class == 0x3E8 {
        // Wine MemoryWineUnixFuncs (class 1000)
        let funcs_table = get_wine_unix_funcs_table();
        if !memory_information.is_null() && memory_information_length >= 8 {
            *(memory_information as *mut u64) = funcs_table as u64;
        }
        if !return_length.is_null() {
            *return_length = 8;
        }
        return 0; // STATUS_SUCCESS
    } else if memory_information_class == 0x3EA {
        // Wine MemoryWineLoadUnixLib
        if !return_length.is_null() {
            *return_length = 8;
        }
        return 0; // STATUS_SUCCESS
    }

    if !return_length.is_null() {
        *return_length = 0;
    }
    0 // STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn ZwQueryVirtualMemory(
    process_handle: u64,
    base_address: u64,
    memory_information_class: u32,
    memory_information: *mut u8,
    memory_information_length: usize,
    return_length: *mut usize,
) -> i32 {
    NtQueryVirtualMemory(
        process_handle,
        base_address,
        memory_information_class,
        memory_information,
        memory_information_length,
        return_length,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn RtlImageNtHeader(base: u64) -> u64 {
    if base < 0x10000 {
        return 0;
    }
    let ptr = base as *const u8;
    let magic = *(ptr as *const u16);
    if magic != 0x5A4D {
        return 0;
    }
    let e_lfanew = *(ptr.add(0x3C) as *const i32);
    if e_lfanew <= 0 {
        return 0;
    }
    base + e_lfanew as u64
}

static WINE_UNIX_FUNCS_TABLE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub fn get_wine_unix_funcs_table() -> usize {
    let cached = WINE_UNIX_FUNCS_TABLE.load(std::sync::atomic::Ordering::SeqCst);
    if cached != 0 {
        return cached;
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let candidate_dirs = [
        format!("{}/.local/share/Steam/steamapps/common/Proton - Experimental/files/lib/wine/x86_64-unix", home),
        format!("{}/.local/share/Steam/steamapps/common/Proton 9.0 (Beta)/files/lib/wine/x86_64-unix", home),
        format!("{}/.local/share/Steam/steamapps/common/Proton 8.0/files/lib/wine/x86_64-unix", home),
        "/usr/lib/wine/x86_64-unix".to_string(),
        "/usr/lib64/wine/x86_64-unix".to_string(),
    ];

    for dir in &candidate_dirs {
        let ntdll_path = format!("{}/ntdll.so", dir);
        let lsteam_path = format!("{}/lsteamclient.so", dir);
        if std::path::Path::new(&lsteam_path).exists() {
            unsafe {
                if std::path::Path::new(&ntdll_path).exists() {
                    if let Ok(c_ntdll) = std::ffi::CString::new(ntdll_path) {
                        libc::dlopen(c_ntdll.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
                    }
                }
                if let Ok(c_lsteam) = std::ffi::CString::new(lsteam_path) {
                    let handle = libc::dlopen(c_lsteam.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
                    if !handle.is_null() {
                        let sym_name = std::ffi::CStr::from_bytes_with_nul(b"__wine_unix_call_funcs\0").unwrap();
                        let sym = libc::dlsym(handle, sym_name.as_ptr());
                        if !sym.is_null() {
                            let addr = sym as usize;
                            println!(
                                "[RINE WINE UNIX] Successfully loaded lsteamclient.so, __wine_unix_call_funcs @ 0x{:X}",
                                addr
                            );
                            WINE_UNIX_FUNCS_TABLE.store(addr, std::sync::atomic::Ordering::SeqCst);
                            return addr;
                        }
                    }
                }
            }
        }
    }

    0x1000
}

static UNIX_DISPATCHER_FN_PTR: unsafe extern "win64" fn(u64, u32, *mut libc::c_void) -> i32 =
    rine_wine_unix_dispatcher;

#[no_mangle]
pub unsafe extern "win64" fn rine_wine_unix_dispatcher(
    handle: u64,
    code: u32,
    args: *mut libc::c_void,
) -> i32 {
    let funcs_table = if handle >= 0x10000 {
        handle as usize
    } else {
        get_wine_unix_funcs_table()
    };

    println!(
        "[WINE UNIX DISPATCHER] handle=0x{:X} (funcs_table=0x{:X}), code={}, args={:p}",
        handle, funcs_table, code, args
    );

    if code == 4 && !args.is_null() {
        let name_ptr = *(args.add(8) as *const *const libc::c_char);
        if !name_ptr.is_null() && (name_ptr as usize) >= 0x10000 {
            let s = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy();
            println!("[WINE UNIX DISPATCHER] -> CreateInterface requested: '{}'", s);
        }
    }

    if funcs_table >= 0x10000 {
        let funcs = funcs_table as *const extern "C" fn(*mut libc::c_void) -> i32;
        let fn_ptr = *funcs.add(code as usize);
        if (fn_ptr as usize) >= 0x10000 {
            let res = fn_ptr(args);
            let out_val = if !args.is_null() && (args as usize) >= 0x10000 {
                std::ptr::read_unaligned(args as *const u32) as u64
            } else {
                0
            };
            println!(
                "[WINE UNIX DISPATCHER] -> executed unix call code={}, res=0x{:X}, out_val=0x{:X}",
                code, res, out_val
            );
            return res;
        }
    }

    if !args.is_null() && (args as usize) >= 0x10000 {
        let out_ptr = args as *mut u32;
        if std::ptr::read_unaligned(out_ptr) == 0 {
            std::ptr::write_unaligned(out_ptr, 0x1000);
        }
    }
    0 // STATUS_SUCCESS
}

pub fn get_wine_unix_dispatcher_ptr() -> usize {
    &raw const UNIX_DISPATCHER_FN_PTR as usize
}

#[no_mangle]
pub unsafe extern "win64" fn RtlFindExportedRoutineByName(
    base: u64,
    name: *const libc::c_char,
) -> u64 {
    if base < 0x10000 || name.is_null() {
        return 0;
    }
    let s = std::ffi::CStr::from_ptr(name).to_string_lossy();
    if s == "__wine_unix_call_dispatcher" {
        return get_wine_unix_dispatcher_ptr() as u64;
    }
    crate::dlls::kernel32::GetProcAddress(base as usize, name) as u64
}

#[no_mangle]
pub unsafe extern "win64" fn LdrGetDllHandle(
    _dll_path: *const u16,
    _dll_characteristics: *const u32,
    dll_name: *const UNICODE_STRING,
    dll_handle: *mut u64,
) -> i32 {
    let name_str = if !dll_name.is_null() && !(*dll_name).Buffer.is_null() {
        let len = (*dll_name).Length as usize / 2;
        let slice = std::slice::from_raw_parts((*dll_name).Buffer, len);
        String::from_utf16_lossy(slice)
    } else {
        String::new()
    };
    let handle = crate::dlls::kernel32::ModuleRegistry::get_module_handle(if name_str.is_empty() {
        None
    } else {
        Some(&name_str)
    }) as u64;
    println!("[NTDLL] LdrGetDllHandle('{}') -> 0x{:X}", name_str, handle);
    if !dll_handle.is_null() {
        *dll_handle = handle;
    }
    if handle != 0 {
        0 // STATUS_SUCCESS
    } else {
        -1073741515 // STATUS_DLL_NOT_FOUND (0xC0000135)
    }
}

#[no_mangle]
pub unsafe extern "win64" fn NtQueryDirectoryFile(
    file_handle: usize,
    _event: usize,
    _apc_routine: usize,
    _apc_context: usize,
    io_status_block: *mut IO_STATUS_BLOCK,
    file_information: *mut u8,
    length: u32,
    file_information_class: u32,
    return_single_entry: u8,
    _file_name: *const UNICODE_STRING,
    restart_scan: u8,
) -> i32 {
    if io_status_block.is_null() || file_information.is_null() || length < 70 {
        if !io_status_block.is_null() {
            (*io_status_block).status = -1073741811; // STATUS_INVALID_PARAMETER (0xC000000D)
            (*io_status_block).information = 0;
        }
        return -1073741811;
    }

    let dir_path = match std::fs::read_link(format!("/proc/self/fd/{}", file_handle)) {
        Ok(p) => p,
        Err(_) => {
            if !io_status_block.is_null() {
                (*io_status_block).status = -1073741816; // STATUS_INVALID_HANDLE (0xC0000008)
                (*io_status_block).information = 0;
            }
            return -1073741816;
        }
    };

    let mut map_guard = NT_DIR_ENUM_MAP.lock().unwrap();
    let map = map_guard.get_or_insert_with(HashMap::new);

    let needs_init = restart_scan != 0 || !map.contains_key(&file_handle);
    if needs_init {
        let mut entries = Vec::new();
        entries.push(NtDirEnumEntry {
            name: ".".into(),
            is_dir: true,
            size: 0,
            time_sec: 0,
            time_nsec: 0,
        });
        entries.push(NtDirEnumEntry {
            name: "..".into(),
            is_dir: true,
            size: 0,
            time_sec: 0,
            time_nsec: 0,
        });

        if let Ok(rd) = std::fs::read_dir(&dir_path) {
            for item in rd.flatten() {
                let name = item.file_name().to_string_lossy().to_string();
                let meta = item.metadata().ok();
                let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let (time_sec, time_nsec) = if let Some(m) = meta {
                    if let Ok(mod_time) = m.modified() {
                        if let Ok(dur) = mod_time.duration_since(std::time::UNIX_EPOCH) {
                            (dur.as_secs(), dur.subsec_nanos())
                        } else {
                            (0, 0)
                        }
                    } else {
                        (0, 0)
                    }
                } else {
                    (0, 0)
                };

                entries.push(NtDirEnumEntry {
                    name,
                    is_dir,
                    size,
                    time_sec,
                    time_nsec,
                });
            }
        }
        map.insert(file_handle, (dir_path, entries, 0));
    }

    let entry_state = map.get_mut(&file_handle).unwrap();
    let entries = &entry_state.1;
    let curr_idx = &mut entry_state.2;

    if *curr_idx >= entries.len() {
        if !io_status_block.is_null() {
            (*io_status_block).status = -2147483642; // STATUS_NO_MORE_FILES (0x80000006)
            (*io_status_block).information = 0;
        }
        return -2147483642;
    }

    let header_size = match file_information_class {
        1 => 64usize,
        3 => 94usize,
        37 => 104usize,
        _ => 68usize, // Class 2: FileFullDirectoryInformation
    };

    let mut current_offset = 0usize;
    let mut last_entry_offset = 0usize;
    let mut entries_written = 0usize;

    while *curr_idx < entries.len() {
        let entry = &entries[*curr_idx];
        let name_utf16: Vec<u16> = entry.name.encode_utf16().collect();
        let name_len_bytes = (name_utf16.len() * 2) as u32;
        let entry_size = header_size + name_len_bytes as usize;
        let padded_size = (entry_size + 7) & !7;

        if current_offset + entry_size > length as usize {
            if entries_written == 0 {
                if !io_status_block.is_null() {
                    (*io_status_block).status = -1073741789; // STATUS_BUFFER_TOO_SMALL (0xC0000023)
                    (*io_status_block).information = 0;
                }
                return -1073741789;
            }
            break;
        }

        std::ptr::write_bytes(file_information.add(current_offset), 0, padded_size.min(length as usize - current_offset));

        let dest = file_information.add(current_offset);
        *((dest.add(4)) as *mut u32) = *curr_idx as u32;

        let ft = if entry.time_sec > 0 {
            ((entry.time_sec + 11644473600) * 10_000_000 + (entry.time_nsec as u64 / 100)) as i64
        } else {
            116444736000000000i64
        };

        *((dest.add(8)) as *mut i64) = ft;
        *((dest.add(16)) as *mut i64) = ft;
        *((dest.add(24)) as *mut i64) = ft;
        *((dest.add(32)) as *mut i64) = ft;

        let file_size = if entry.is_dir { 0i64 } else { entry.size as i64 };
        *((dest.add(40)) as *mut i64) = file_size;
        *((dest.add(48)) as *mut i64) = (file_size + 4095) & !4095;

        let attr = if entry.is_dir { 0x10u32 } else { 0x20u32 };
        *((dest.add(56)) as *mut u32) = attr;
        *((dest.add(60)) as *mut u32) = name_len_bytes;

        std::ptr::copy_nonoverlapping(
            name_utf16.as_ptr() as *const u8,
            dest.add(header_size),
            name_len_bytes as usize,
        );

        if entries_written > 0 {
            let prev_ptr = file_information.add(last_entry_offset) as *mut u32;
            *prev_ptr = (current_offset - last_entry_offset) as u32;
        }

        last_entry_offset = current_offset;
        current_offset += padded_size;
        *curr_idx += 1;
        entries_written += 1;

        if return_single_entry != 0 {
            break;
        }
    }

    if entries_written > 0 {
        let last_ptr = file_information.add(last_entry_offset) as *mut u32;
        *last_ptr = 0;
    }

    let total_bytes = current_offset;
    if !io_status_block.is_null() {
        (*io_status_block).status = 0; // STATUS_SUCCESS
        (*io_status_block).information = total_bytes;
    }

    0 // STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn ZwQueryDirectoryFile(
    file_handle: usize,
    event: usize,
    apc_routine: usize,
    apc_context: usize,
    io_status_block: *mut IO_STATUS_BLOCK,
    file_information: *mut u8,
    length: u32,
    file_information_class: u32,
    return_single_entry: u8,
    file_name: *const UNICODE_STRING,
    restart_scan: u8,
) -> i32 {
    NtQueryDirectoryFile(
        file_handle,
        event,
        apc_routine,
        apc_context,
        io_status_block,
        file_information,
        length,
        file_information_class,
        return_single_entry,
        file_name,
        restart_scan,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtQueryDirectoryFileEx(
    file_handle: usize,
    event: usize,
    apc_routine: usize,
    apc_context: usize,
    io_status_block: *mut IO_STATUS_BLOCK,
    file_information: *mut u8,
    length: u32,
    file_information_class: u32,
    query_flags: u32,
    file_name: *const UNICODE_STRING,
) -> i32 {
    let restart_scan = (query_flags & 0x01) as u8;
    let return_single_entry = ((query_flags & 0x02) != 0) as u8;
    NtQueryDirectoryFile(
        file_handle,
        event,
        apc_routine,
        apc_context,
        io_status_block,
        file_information,
        length,
        file_information_class,
        return_single_entry,
        file_name,
        restart_scan,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn ZwQueryDirectoryFileEx(
    file_handle: usize,
    event: usize,
    apc_routine: usize,
    apc_context: usize,
    io_status_block: *mut IO_STATUS_BLOCK,
    file_information: *mut u8,
    length: u32,
    file_information_class: u32,
    query_flags: u32,
    file_name: *const UNICODE_STRING,
) -> i32 {
    NtQueryDirectoryFileEx(
        file_handle,
        event,
        apc_routine,
        apc_context,
        io_status_block,
        file_information,
        length,
        file_information_class,
        query_flags,
        file_name,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtQueryInformationFile(
    file_handle: usize,
    io_status_block: *mut IO_STATUS_BLOCK,
    file_information: *mut u8,
    length: u32,
    file_information_class: u32,
) -> i32 {
    if io_status_block.is_null() || file_information.is_null() || length == 0 {
        return -1073741811; // STATUS_INVALID_PARAMETER
    }

    let dir_path = std::fs::read_link(format!("/proc/self/fd/{}", file_handle)).ok();
    let meta = dir_path.as_ref().and_then(|p| std::fs::metadata(p).ok());

    match file_information_class {
        4 => {
            // FileBasicInformation (40 bytes)
            if length < 40 {
                return -1073741789; // STATUS_BUFFER_TOO_SMALL
            }
            std::ptr::write_bytes(file_information, 0, 40);
            let ft = if let Some(ref m) = meta {
                if let Ok(mod_time) = m.modified() {
                    if let Ok(dur) = mod_time.duration_since(std::time::UNIX_EPOCH) {
                        ((dur.as_secs() + 11644473600) * 10_000_000 + (dur.subsec_nanos() as u64 / 100)) as i64
                    } else { 116444736000000000i64 }
                } else { 116444736000000000i64 }
            } else { 116444736000000000i64 };

            *((file_information.add(8)) as *mut i64) = ft;
            *((file_information.add(16)) as *mut i64) = ft;
            *((file_information.add(24)) as *mut i64) = ft;
            *((file_information.add(32)) as *mut i64) = ft;
            let attr = if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) { 0x10u32 } else { 0x20u32 };
            *((file_information.add(36)) as *mut u32) = attr;
            if !io_status_block.is_null() {
                (*io_status_block).status = 0;
                (*io_status_block).information = 40;
            }
            0
        }
        5 => {
            // FileStandardInformation (24 bytes)
            if length < 24 {
                return -1073741789; // STATUS_BUFFER_TOO_SMALL
            }
            std::ptr::write_bytes(file_information, 0, 24);
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0) as i64;
            *((file_information.add(0)) as *mut i64) = (size + 4095) & !4095;
            *((file_information.add(8)) as *mut i64) = size;
            *((file_information.add(16)) as *mut u32) = 1;
            *file_information.add(20) = 0;
            *file_information.add(21) = if is_dir { 1 } else { 0 };
            if !io_status_block.is_null() {
                (*io_status_block).status = 0;
                (*io_status_block).information = 24;
            }
            0
        }
        _ => {
            std::ptr::write_bytes(file_information, 0, length as usize);
            if !io_status_block.is_null() {
                (*io_status_block).status = 0;
                (*io_status_block).information = length as usize;
            }
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn ZwQueryInformationFile(
    file_handle: usize,
    io_status_block: *mut IO_STATUS_BLOCK,
    file_information: *mut u8,
    length: u32,
    file_information_class: u32,
) -> i32 {
    NtQueryInformationFile(file_handle, io_status_block, file_information, length, file_information_class)
}

#[no_mangle]
pub unsafe extern "win64" fn NtSetInformationFile(
    _file_handle: usize,
    io_status_block: *mut IO_STATUS_BLOCK,
    _file_information: *mut u8,
    _length: u32,
    _file_information_class: u32,
) -> i32 {
    if !io_status_block.is_null() {
        (*io_status_block).status = 0;
        (*io_status_block).information = 0;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ZwSetInformationFile(
    file_handle: usize,
    io_status_block: *mut IO_STATUS_BLOCK,
    file_information: *mut u8,
    length: u32,
    file_information_class: u32,
) -> i32 {
    NtSetInformationFile(file_handle, io_status_block, file_information, length, file_information_class)
}

#[no_mangle]
pub unsafe extern "win64" fn RtlNtStatusToDosError(status: i32) -> u32 {
    if status >= 0 {
        return 0; // ERROR_SUCCESS
    }
    match status as u32 {
        0x80000006 => 18, // STATUS_NO_MORE_FILES -> ERROR_NO_MORE_FILES
        0xC0000034 => 2,  // STATUS_OBJECT_NAME_NOT_FOUND -> ERROR_FILE_NOT_FOUND
        0xC000003A => 3,  // STATUS_OBJECT_PATH_NOT_FOUND -> ERROR_PATH_NOT_FOUND
        0xC0000022 => 5,  // STATUS_ACCESS_DENIED -> ERROR_ACCESS_DENIED
        0xC0000008 => 6,  // STATUS_INVALID_HANDLE -> ERROR_INVALID_HANDLE
        0xC000000D => 87, // STATUS_INVALID_PARAMETER -> ERROR_INVALID_PARAMETER
        0xC000000F => 2,  // STATUS_NO_SUCH_FILE -> ERROR_FILE_NOT_FOUND
        0xC0000023 => 122,// STATUS_BUFFER_TOO_SMALL -> ERROR_INSUFFICIENT_BUFFER
        0xC0000035 => 80, // STATUS_OBJECT_NAME_COLLISION -> ERROR_FILE_EXISTS
        0xC00000BA => 111,// STATUS_FILE_IS_A_DIRECTORY -> ERROR_BUFFER_OVERFLOW
        0xC0000101 => 145,// STATUS_DIRECTORY_NOT_EMPTY -> ERROR_DIR_NOT_EMPTY
        0xC0000135 => 126,// STATUS_DLL_NOT_FOUND -> ERROR_MOD_NOT_FOUND
        0xC0000139 => 127,// STATUS_ENTRYPOINT_NOT_FOUND -> ERROR_PROC_NOT_FOUND
        _ => 31,          // ERROR_GEN_FAILURE
    }
}

#[no_mangle]
pub unsafe extern "win64" fn RtlNtStatusToDosErrorNoTeb(status: i32) -> u32 {
    RtlNtStatusToDosError(status)
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
