#![allow(non_snake_case)]
use crate::dlls::ntdll;
use crate::mem::MmapRegion;
use crate::sys::WinPageProtection;
use std::collections::HashMap;
use std::sync::Mutex;

pub const HEAP_ZERO_MEMORY: u32 = 0x00000008;

pub struct ProcessHeap {
    allocations: Mutex<HashMap<usize, MmapRegion>>,
}

impl ProcessHeap {
    pub fn new() -> Self {
        Self {
            allocations: Mutex::new(HashMap::new()),
        }
    }

    pub fn alloc(&self, size: usize, flags: u32) -> Option<usize> {
        let size = if size == 0 { 16 } else { size };
        let alloc_size = (size + 4095) & !4095;
        unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                alloc_size,
                libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            if ptr == libc::MAP_FAILED {
                None
            } else {
                if (flags & HEAP_ZERO_MEMORY) != 0 {
                    libc::memset(ptr, 0, alloc_size);
                }
                Some(ptr as usize)
            }
        }
    }

    pub fn free(&self, ptr: usize) -> bool {
        if ptr == 0 {
            return true;
        }
        unsafe { libc::munmap(ptr as *mut libc::c_void, 4096) == 0 }
    }
}

pub unsafe extern "win64" fn GetProcessHeap() -> usize {
    0x180000000
}

pub unsafe extern "win64" fn HeapAlloc(_h_heap: usize, _flags: u32, size: usize) -> usize {
    let alloc_size = if size == 0 { 64 } else { (size + 15) & !15 };
    let ptr = libc::calloc(1, alloc_size);
    if ptr.is_null() {
        let fallback_ptr = libc::mmap(
            std::ptr::null_mut(),
            alloc_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        if fallback_ptr == libc::MAP_FAILED || fallback_ptr.is_null() {
            crate::dlls::registry::get_dummy_stub_buffer_ptr() as usize
        } else {
            fallback_ptr as usize
        }
    } else {
        ptr as usize
    }
}

pub unsafe extern "win64" fn HeapFree(_h_heap: usize, _flags: u32, ptr: usize) -> i32 {
    if ptr > 0x10000 && ptr < 0x700000000000 {
        libc::free(ptr as *mut libc::c_void);
    }
    1 // TRUE (Win32 HeapFree success)
}

pub unsafe extern "win64" fn HeapReAlloc(
    _h_heap: usize,
    _flags: u32,
    ptr: usize,
    size: usize,
) -> usize {
    if ptr == 0 {
        return HeapAlloc(_h_heap, _flags, size);
    }
    let alloc_size = if size == 0 { 64 } else { (size + 15) & !15 };
    let new_ptr = libc::realloc(ptr as *mut libc::c_void, alloc_size);
    if new_ptr.is_null() {
        ptr
    } else {
        new_ptr as usize
    }
}

pub unsafe extern "win64" fn HeapSize(_h_heap: usize, _flags: u32, ptr: usize) -> usize {
    if ptr == 0 {
        0
    } else {
        64
    }
}

pub unsafe extern "win64" fn HeapCreate(
    _flags: u32,
    _initial_size: usize,
    _max_size: usize,
) -> usize {
    0x100000 // Return valid non-null default Heap handle
}

pub unsafe extern "win64" fn HeapDestroy(_h_heap: usize) -> i32 {
    1 // TRUE
}

pub unsafe extern "win64" fn HeapValidate(_h_heap: usize, _flags: u32, _ptr: usize) -> i32 {
    1 // TRUE
}

pub unsafe extern "win64" fn LocalAlloc(flags: u32, size: usize) -> usize {
    HeapAlloc(GetProcessHeap(), flags, size)
}

pub unsafe extern "win64" fn LocalFree(ptr: usize) -> usize {
    if ptr > 0 {
        HeapFree(GetProcessHeap(), 0, ptr);
    }
    0 // Win32 LocalFree returns NULL (0) on success
}

pub unsafe extern "win64" fn LocalReAlloc(ptr: usize, size: usize, flags: u32) -> usize {
    HeapReAlloc(GetProcessHeap(), flags, ptr, size)
}

pub unsafe extern "win64" fn LocalSize(ptr: usize) -> usize {
    HeapSize(GetProcessHeap(), 0, ptr)
}

pub unsafe extern "win64" fn LocalLock(ptr: usize) -> usize {
    if ptr == 0 {
        crate::dlls::registry::get_dummy_stub_buffer_ptr() as usize
    } else {
        ptr
    }
}

pub unsafe extern "win64" fn LocalUnlock(_ptr: usize) -> i32 {
    1 // TRUE
}

pub unsafe extern "win64" fn LocalHandle(ptr: usize) -> usize {
    if ptr == 0 {
        0
    } else {
        ptr
    }
}

pub unsafe extern "win64" fn GlobalAlloc(flags: u32, size: usize) -> usize {
    LocalAlloc(flags, size)
}

pub unsafe extern "win64" fn GlobalFree(ptr: usize) -> usize {
    LocalFree(ptr)
}

pub unsafe extern "win64" fn GlobalReAlloc(ptr: usize, size: usize, flags: u32) -> usize {
    LocalReAlloc(ptr, size, flags)
}

pub unsafe extern "win64" fn GlobalSize(ptr: usize) -> usize {
    LocalSize(ptr)
}

pub unsafe extern "win64" fn GlobalLock(ptr: usize) -> usize {
    LocalLock(ptr)
}

pub unsafe extern "win64" fn GlobalUnlock(ptr: usize) -> i32 {
    LocalUnlock(ptr)
}

pub unsafe extern "win64" fn GlobalHandle(ptr: usize) -> usize {
    LocalHandle(ptr)
}

pub unsafe extern "win64" fn VirtualAlloc(
    lp_address: usize,
    dw_size: usize,
    _fl_allocation_type: u32,
    _fl_protect: u32,
) -> usize {
    let alloc_size = if dw_size == 0 {
        4096
    } else {
        (dw_size + 4095) & !4095
    };
    let target = if lp_address > 0 {
        lp_address as *mut libc::c_void
    } else {
        std::ptr::null_mut()
    };
    let ptr = libc::mmap(
        target,
        alloc_size,
        libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
        -1,
        0,
    );
    if ptr == libc::MAP_FAILED || ptr.is_null() {
        let fallback = libc::calloc(1, alloc_size);
        if fallback.is_null() {
            static FALLBACK_VIRT: [u8; 1048576] = [0; 1048576];
            FALLBACK_VIRT.as_ptr() as usize
        } else {
            fallback as usize
        }
    } else {
        ptr as usize
    }
}

pub unsafe extern "win64" fn VirtualFree(address: usize, _size: usize, _free_type: u32) -> i32 {
    if address == 0 {
        return 0;
    }
    1
}

pub static CURRENT_IMAGE_BASE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0x0000000140000000);

pub fn set_current_image_base(base: u64) {
    CURRENT_IMAGE_BASE.store(base, std::sync::atomic::Ordering::SeqCst);
}

pub fn get_current_image_base() -> u64 {
    CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst)
}

#[repr(C)]
pub struct RtlCriticalSection {
    pub debug_info: usize,
    pub lock_count: i32,
    pub recursion_count: i32,
    pub owning_thread: usize,
    pub lock_semaphore: usize,
    pub spin_count: usize,
}

pub unsafe extern "win64" fn InitializeCriticalSection(cs: *mut RtlCriticalSection) {
    if !cs.is_null() {
        (*cs).debug_info = 0;
        (*cs).lock_count = -1;
        (*cs).recursion_count = 0;
        (*cs).owning_thread = 0;
        (*cs).lock_semaphore = 0;
        (*cs).spin_count = 0;
    }
}

pub unsafe extern "win64" fn InitializeCriticalSectionAndSpinCount(
    cs: *mut RtlCriticalSection,
    spin: u32,
) -> i32 {
    if !cs.is_null() {
        (*cs).debug_info = 0;
        (*cs).lock_count = -1;
        (*cs).recursion_count = 0;
        (*cs).owning_thread = 0;
        (*cs).lock_semaphore = 0;
        (*cs).spin_count = spin as usize;
    }
    1
}

pub unsafe extern "win64" fn InitializeCriticalSectionEx(
    cs: *mut RtlCriticalSection,
    spin: u32,
    _flags: u32,
) -> i32 {
    InitializeCriticalSectionAndSpinCount(cs, spin)
}

pub unsafe extern "win64" fn EnterCriticalSection(cs: *mut RtlCriticalSection) {
    if !cs.is_null() {
        let tid = libc::pthread_self() as usize;
        if (*cs).owning_thread == tid {
            (*cs).recursion_count += 1;
        } else {
            (*cs).owning_thread = tid;
            (*cs).recursion_count = 1;
            (*cs).lock_count = 0;
        }
    }
}

pub unsafe extern "win64" fn LeaveCriticalSection(cs: *mut RtlCriticalSection) {
    if !cs.is_null() {
        if (*cs).recursion_count > 1 {
            (*cs).recursion_count -= 1;
        } else {
            (*cs).recursion_count = 0;
            (*cs).owning_thread = 0;
            (*cs).lock_count = -1;
        }
    }
}

pub unsafe extern "win64" fn TryEnterCriticalSection(cs: *mut RtlCriticalSection) -> i32 {
    if !cs.is_null() {
        EnterCriticalSection(cs);
        1
    } else {
        0
    }
}

pub unsafe extern "win64" fn DeleteCriticalSection(cs: *mut RtlCriticalSection) {
    if !cs.is_null() {
        (*cs).lock_count = -1;
        (*cs).recursion_count = 0;
        (*cs).owning_thread = 0;
    }
}

pub unsafe extern "win64" fn InitializeSRWLock(lock: *mut usize) {
    if !lock.is_null() {
        *lock = 0;
    }
}

pub unsafe extern "win64" fn AcquireSRWLockExclusive(lock: *mut usize) {
    if !lock.is_null() {
        *lock = 1;
    }
}

pub unsafe extern "win64" fn ReleaseSRWLockExclusive(lock: *mut usize) {
    if !lock.is_null() {
        *lock = 0;
    }
}

pub unsafe extern "win64" fn AcquireSRWLockShared(lock: *mut usize) {
    if !lock.is_null() {
        *lock = 1;
    }
}

pub unsafe extern "win64" fn ReleaseSRWLockShared(lock: *mut usize) {
    if !lock.is_null() {
        *lock = 0;
    }
}

pub unsafe extern "win64" fn TryAcquireSRWLockExclusive(lock: *mut usize) -> u8 {
    if !lock.is_null() {
        *lock = 1;
        1
    } else {
        0
    }
}

pub unsafe extern "win64" fn TryAcquireSRWLockShared(lock: *mut usize) -> u8 {
    if !lock.is_null() {
        *lock = 1;
        1
    } else {
        0
    }
}

pub unsafe extern "win64" fn InitializeConditionVariable(var: *mut usize) {
    if !var.is_null() {
        *var = 0;
    }
}

pub unsafe extern "win64" fn WakeConditionVariable(_var: *mut usize) {}
pub unsafe extern "win64" fn WakeAllConditionVariable(_var: *mut usize) {}

pub unsafe extern "win64" fn SleepConditionVariableCS(
    _var: *mut usize,
    cs: *mut RtlCriticalSection,
    _timeout: u32,
) -> i32 {
    LeaveCriticalSection(cs);
    EnterCriticalSection(cs);
    1
}

pub unsafe extern "win64" fn SleepConditionVariableSRW(
    _var: *mut usize,
    lock: *mut usize,
    _timeout: u32,
    flags: u32,
) -> i32 {
    if flags & 1 != 0 {
        ReleaseSRWLockShared(lock);
        AcquireSRWLockShared(lock);
    } else {
        ReleaseSRWLockExclusive(lock);
        AcquireSRWLockExclusive(lock);
    }
    1
}

pub unsafe extern "win64" fn CreateEventW(
    _attr: usize,
    _manual_reset: i32,
    _initial_state: i32,
    _name: usize,
) -> usize {
    static EVENT_HANDLE_COUNTER: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0x2000);
    EVENT_HANDLE_COUNTER.fetch_add(4, std::sync::atomic::Ordering::SeqCst)
}

pub unsafe extern "win64" fn CreateEventA(
    attr: usize,
    manual_reset: i32,
    initial_state: i32,
    name: usize,
) -> usize {
    CreateEventW(attr, manual_reset, initial_state, name)
}

pub unsafe extern "win64" fn OpenEventW(_access: u32, _inherit: i32, _name: usize) -> usize {
    0x2080
}

pub unsafe extern "win64" fn OpenEventA(access: u32, inherit: i32, name: usize) -> usize {
    OpenEventW(access, inherit, name)
}

pub unsafe extern "win64" fn CreateMutexW(
    _attr: usize,
    _initial_owner: i32,
    _name: usize,
) -> usize {
    static MUTEX_HANDLE_COUNTER: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0x3000);
    MUTEX_HANDLE_COUNTER.fetch_add(4, std::sync::atomic::Ordering::SeqCst)
}

pub unsafe extern "win64" fn CreateMutexA(attr: usize, initial_owner: i32, name: usize) -> usize {
    CreateMutexW(attr, initial_owner, name)
}

pub unsafe extern "win64" fn OpenMutexW(_access: u32, _inherit: i32, _name: usize) -> usize {
    0x3080
}

pub unsafe extern "win64" fn OpenMutexA(access: u32, inherit: i32, name: usize) -> usize {
    OpenMutexW(access, inherit, name)
}

pub unsafe extern "win64" fn CreateSemaphoreW(
    _attr: usize,
    _initial_count: i32,
    _max_count: i32,
    _name: usize,
) -> usize {
    static SEMAPHORE_HANDLE_COUNTER: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0x4000);
    SEMAPHORE_HANDLE_COUNTER.fetch_add(4, std::sync::atomic::Ordering::SeqCst)
}

pub unsafe extern "win64" fn CreateSemaphoreA(
    attr: usize,
    initial_count: i32,
    max_count: i32,
    name: usize,
) -> usize {
    CreateSemaphoreW(attr, initial_count, max_count, name)
}

pub unsafe extern "win64" fn ReleaseSemaphore(
    _sem: usize,
    _release_count: i32,
    prev_count: *mut i32,
) -> i32 {
    if !prev_count.is_null() {
        *prev_count = 1;
    }
    1
}

pub unsafe extern "win64" fn InitOnceInitialize(once: *mut usize) {
    if !once.is_null() {
        *once = 0;
    }
}

pub unsafe extern "win64" fn InitOnceBeginInitialize(
    once: *mut usize,
    _flags: u32,
    pending: *mut i32,
    _context: *mut usize,
) -> i32 {
    if !pending.is_null() {
        if !once.is_null() && *once == 2 {
            *pending = 0;
        } else {
            *pending = 1;
        }
    }
    1
}

pub unsafe extern "win64" fn InitOnceComplete(
    once: *mut usize,
    _flags: u32,
    _context: usize,
) -> i32 {
    if !once.is_null() {
        *once = 2;
    }
    1
}

pub unsafe extern "win64" fn InitOnceExecuteOnce(
    once: *mut usize,
    func: unsafe extern "win64" fn(*mut usize, usize, *mut usize) -> i32,
    param: usize,
    context: *mut usize,
) -> i32 {
    if !once.is_null() && *once == 2 {
        return 1;
    }
    let res = func(once, param, context);
    if res != 0 && !once.is_null() {
        *once = 2;
    }
    res
}

pub unsafe extern "win64" fn GetModuleHandleExW(
    _flags: u32,
    _name: u64,
    module_out: *mut u64,
) -> i32 {
    let base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    if !module_out.is_null() {
        *module_out = base;
    }
    1
}

pub unsafe extern "win64" fn GetModuleHandleExA(
    flags: u32,
    name: u64,
    module_out: *mut u64,
) -> i32 {
    GetModuleHandleExW(flags, name, module_out)
}

static TARGET_EXE_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_target_exe_path(path: &str) {
    let _ = TARGET_EXE_PATH.set(path.to_string());
}

pub fn get_target_exe_path() -> String {
    TARGET_EXE_PATH.get().cloned().unwrap_or_else(|| {
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "That's not my neighbor.exe".into())
    })
}

unsafe fn read_utf16_ptr(ptr: *const u16) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x10000 {
        return None;
    }
    let mut buf = Vec::new();
    for i in 0..256usize {
        let ch = *ptr.add(i);
        if ch == 0 {
            break;
        }
        buf.push(ch);
    }
    if buf.is_empty() {
        None
    } else {
        Some(String::from_utf16_lossy(&buf))
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleHandleW(module_name: u64) -> u64 {
    let base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    if module_name == 0 || module_name < 0x10000 {
        return base;
    }
    let name_opt = read_utf16_ptr(module_name as *const u16);
    if let Some(ref name) = name_opt {
        let lname = name.to_lowercase();
        if lname.contains("kernel32") || lname.contains("kernelbase") {
            return 0x7FFF0000;
        }
        if lname.contains("user32") {
            return 0x7FFE0000;
        }
        if lname.contains("ntdll") {
            return 0x7FFD0000;
        }
        if lname.contains("dxgi") {
            return 0x00000002EDF00000;
        }
        if lname.contains("d3d11") {
            return 0x0000000359050000;
        }
    }
    base
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleHandleA(module_name: u64) -> u64 {
    if module_name == 0 || module_name < 0x10000 {
        return CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    }
    let cstr = std::ffi::CStr::from_ptr(module_name as *const std::ffi::c_char);
    let s = cstr.to_string_lossy().to_lowercase();
    if s.contains("kernel32") || s.contains("kernelbase") {
        return 0x7FFF0000;
    }
    if s.contains("user32") {
        return 0x7FFE0000;
    }
    if s.contains("ntdll") {
        return 0x7FFD0000;
    }
    if s.contains("dxgi") {
        return 0x00000002EDF00000;
    }
    if s.contains("d3d11") {
        return 0x0000000359050000;
    }
    CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst)
}

static LAST_ERROR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[no_mangle]
pub unsafe extern "win64" fn SetLastError(dw_err_code: u32) {
    LAST_ERROR.store(dw_err_code, std::sync::atomic::Ordering::Relaxed);
}

#[no_mangle]
pub unsafe extern "win64" fn GetLastError() -> u32 {
    LAST_ERROR.load(std::sync::atomic::Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "win64" fn ExitProcess(u_exit_code: u32) -> ! {
    eprintln!(
        "[RINE EXIT PROCESS] KERNEL32!ExitProcess called with exit_code=0x{:08X} ({})",
        u_exit_code, u_exit_code
    );
    eprintln!("[GUEST EXITED] KERNEL32!ExitProcess invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(u_exit_code as i32);
}

#[no_mangle]
pub unsafe extern "win64" fn TerminateProcess(h_process: u64, u_exit_code: u32) -> i32 {
    eprintln!(
        "[RINE EXIT PROCESS] KERNEL32!TerminateProcess(h_process=0x{:X}, exit_code=0x{:08X})",
        h_process, u_exit_code
    );
    eprintln!("[GUEST EXITED] KERNEL32!TerminateProcess invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(u_exit_code as i32);
}

#[no_mangle]
pub unsafe extern "win64" fn OutputDebugStringA(lp_output_string: *const libc::c_char) {
    if !lp_output_string.is_null() {
        if let Ok(s) = std::ffi::CStr::from_ptr(lp_output_string).to_str() {
            println!("[GODOT OUTPUT DEBUG STRING A] {}", s);
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn OutputDebugStringW(lp_output_string: *const u16) {
    if !lp_output_string.is_null() {
        let mut len = 0usize;
        while *lp_output_string.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(lp_output_string, len));
        println!("[GODOT OUTPUT DEBUG STRING W] {}", s);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleFileNameW(
    _h_module: u64,
    lp_filename: *mut u16,
    n_size: u32,
) -> u32 {
    SetLastError(0);
    if lp_filename.is_null() || n_size == 0 {
        return 0;
    }
    let path = get_target_exe_path();
    let win_path = path.replace('/', "\\");
    let wide: Vec<u16> = win_path.encode_utf16().chain(std::iter::once(0)).collect();
    let to_copy = std::cmp::min(n_size as usize, wide.len());
    std::ptr::copy_nonoverlapping(wide.as_ptr(), lp_filename, to_copy);
    println!("[KERNEL32] GetModuleFileNameW -> '{}'", win_path);
    (to_copy.saturating_sub(1)) as u32
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleFileNameA(
    _h_module: u64,
    lp_filename: *mut u8,
    n_size: u32,
) -> u32 {
    if lp_filename.is_null() || n_size == 0 {
        return 0;
    }
    let path = get_target_exe_path();
    let win_path = path.replace('/', "\\");
    let bytes = win_path.as_bytes();
    let to_copy = std::cmp::min(n_size as usize - 1, bytes.len());
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), lp_filename, to_copy);
    *lp_filename.add(to_copy) = 0;
    println!("[KERNEL32] GetModuleFileNameA -> '{}'", win_path);
    to_copy as u32
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

#[no_mangle]
pub unsafe extern "win64" fn QueryPerformanceCounter(lp_performance_count: *mut i64) -> i32 {
    if !lp_performance_count.is_null() {
        let inst = START_TIME.get_or_init(std::time::Instant::now);
        *lp_performance_count = inst.elapsed().as_nanos() as i64;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn QueryPerformanceFrequency(lp_frequency: *mut i64) -> i32 {
    if !lp_frequency.is_null() {
        *lp_frequency = 1_000_000_000; // 1 GHz
    }
    1 // TRUE
}

thread_local! {
    static THREAD_TLS_MAP: std::cell::RefCell<std::collections::HashMap<u32, usize>> = std::cell::RefCell::new(std::collections::HashMap::new());
    static THREAD_FLS_MAP: std::cell::RefCell<std::collections::HashMap<u32, usize>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

static NEXT_TLS_INDEX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(10);
static NEXT_FLS_INDEX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(10);

#[no_mangle]
pub unsafe extern "win64" fn EncodePointer(ptr: usize) -> usize {
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn DecodePointer(ptr: usize) -> usize {
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn EncodeSystemPointer(ptr: usize) -> usize {
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn DecodeSystemPointer(ptr: usize) -> usize {
    ptr
}

pub unsafe extern "win64" fn TlsAlloc() -> u32 {
    let idx = NEXT_TLS_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    println!("[KERNEL32] TlsAlloc -> TLS Index {}", idx);
    idx
}

#[inline]
pub unsafe fn get_teb_ptr() -> *mut crate::pe::teb::Teb64 {
    let mut gs_base: u64 = 0;
    libc::syscall(libc::SYS_arch_prctl, 0x1004, &raw mut gs_base); // ARCH_GET_GS
    gs_base as *mut crate::pe::teb::Teb64
}

pub unsafe extern "win64" fn TlsGetValue(dw_tls_index: u32) -> usize {
    let teb = get_teb_ptr();
    if !teb.is_null() {
        let tls_array = (*teb).thread_local_storage_pointer as *const usize;
        if !tls_array.is_null() && (dw_tls_index as usize) < 64 {
            let val = *tls_array.add(dw_tls_index as usize);
            if val != 0 {
                SetLastError(0);
                return val;
            }
        }
    }
    let val = THREAD_TLS_MAP.with(|map| *map.borrow().get(&dw_tls_index).unwrap_or(&0));
    SetLastError(0);
    val
}

pub unsafe extern "win64" fn TlsSetValue(dw_tls_index: u32, lp_tls_value: usize) -> i32 {
    THREAD_TLS_MAP.with(|map| {
        map.borrow_mut().insert(dw_tls_index, lp_tls_value);
    });
    let teb = get_teb_ptr();
    if !teb.is_null() {
        let tls_array = (*teb).thread_local_storage_pointer as *mut usize;
        if !tls_array.is_null() && (dw_tls_index as usize) < 64 {
            *tls_array.add(dw_tls_index as usize) = lp_tls_value;
        }
    }
    SetLastError(0);
    1 // TRUE
}

pub unsafe extern "win64" fn TlsFree(dw_tls_index: u32) -> i32 {
    THREAD_TLS_MAP.with(|map| {
        map.borrow_mut().remove(&dw_tls_index);
    });
    SetLastError(0);
    1 // TRUE
}

pub unsafe extern "win64" fn FlsAlloc(_callback: usize) -> u32 {
    let idx = NEXT_FLS_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    println!("[KERNEL32] FlsAlloc -> FLS Index {}", idx);
    idx
}

pub unsafe extern "win64" fn FlsGetValue(dw_fls_index: u32) -> usize {
    let val = THREAD_FLS_MAP.with(|map| *map.borrow().get(&dw_fls_index).unwrap_or(&0));
    SetLastError(0);
    val
}

pub unsafe extern "win64" fn FlsSetValue(dw_fls_index: u32, lp_fls_value: usize) -> i32 {
    THREAD_FLS_MAP.with(|map| {
        map.borrow_mut().insert(dw_fls_index, lp_fls_value);
    });
    SetLastError(0);
    1 // TRUE
}

pub unsafe extern "win64" fn FlsFree(dw_fls_index: u32) -> i32 {
    THREAD_FLS_MAP.with(|map| {
        map.borrow_mut().remove(&dw_fls_index);
    });
    SetLastError(0);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetCommandLineW() -> u64 {
    static CMDLINE_UTF16: std::sync::Mutex<Option<Vec<u16>>> = std::sync::Mutex::new(None);
    let mut guard = CMDLINE_UTF16.lock().unwrap();
    if guard.is_none() {
        let path = get_target_exe_path();
        let cmd = format!("\"{}\" --verbose\0", path.replace('/', "\\"));
        *guard = Some(cmd.encode_utf16().collect());
    }
    let ptr = guard.as_ref().unwrap().as_ptr() as u64;
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn GetCommandLineA() -> u64 {
    static CMDLINE_ASCII: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);
    let mut guard = CMDLINE_ASCII.lock().unwrap();
    if guard.is_none() {
        let path = get_target_exe_path();
        let cmd = format!("\"{}\" --verbose\0", path.replace('/', "\\"));
        *guard = Some(cmd.into_bytes());
    }
    let ptr = guard.as_ref().unwrap().as_ptr() as u64;
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn GetStartupInfoW(startup_info_ptr: u64) {
    println!(
        "[KERNEL32] GetStartupInfoW called by CRT (ptr=0x{:016X})",
        startup_info_ptr
    );
    if startup_info_ptr != 0 {
        std::ptr::write_bytes(startup_info_ptr as *mut u8, 0, 104);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateFileW(
    lp_file_name: *const u16,
    dw_desired_access: u32,
    _dw_share_mode: u32,
    _lp_security_attributes: *mut u8,
    dw_creation_disposition: u32,
    _dw_flags_and_attributes: u32,
    _h_template_file: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = !0usize as *mut std::ffi::c_void;
    const GENERIC_READ: u32 = 0x80000000;
    const GENERIC_WRITE: u32 = 0x40000000;
    const OPEN_EXISTING: u32 = 3;
    const CREATE_ALWAYS: u32 = 2;
    const CREATE_NEW: u32 = 1;

    if lp_file_name.is_null() {
        return INVALID_HANDLE_VALUE;
    }

    // Convert Windows UTF-16 path to Rust String
    let mut len = 0usize;
    while *lp_file_name.add(len) != 0 && len < 4096 {
        len += 1;
    }
    let wide = std::slice::from_raw_parts(lp_file_name, len);
    let win_path = String::from_utf16_lossy(wide);

    // Convert Windows path separators and drive letters to Linux paths
    let linux_path = win_path_to_linux(&win_path);
    println!(
        "[KERNEL32 FILE] CreateFileW: '{}' -> '{}'",
        win_path, linux_path
    );

    // Determine open flags
    let write_mode = (dw_desired_access & GENERIC_WRITE) != 0;
    let flags = if write_mode {
        match dw_creation_disposition {
            CREATE_ALWAYS => libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
            CREATE_NEW => libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            _ => libc::O_RDWR | libc::O_CREAT,
        }
    } else {
        libc::O_RDONLY
    };

    let path_c = match std::ffi::CString::new(linux_path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return INVALID_HANDLE_VALUE,
    };

    let fd = libc::open(path_c.as_ptr(), flags, 0o644);
    if fd < 0 {
        println!(
            "[KERNEL32 FILE]   -> FAILED (errno={}), returning INVALID_HANDLE",
            *libc::__errno_location()
        );
        return INVALID_HANDLE_VALUE;
    }

    // Register fd in handle table
    register_fd_handle(fd as u64);
    println!("[KERNEL32 FILE]   -> fd={} OK", fd);
    fd as *mut std::ffi::c_void
}

/// Converts a Windows path to a Linux path best-effort
fn win_path_to_linux(win_path: &str) -> String {
    // Strip drive letter like C:\ or \\?\ prefix
    let path = if win_path.starts_with("\\\\?\\") {
        &win_path[4..]
    } else {
        win_path
    };

    // Remove drive letter C:\ -> /
    let path = if path.len() >= 3 && path.as_bytes()[1] == b':' {
        &path[2..]
    } else {
        path
    };

    // Replace backslashes with forward slashes
    path.replace('\\', "/")
}

static FD_TABLE: std::sync::Mutex<Option<std::collections::HashSet<u64>>> =
    std::sync::Mutex::new(None);

fn register_fd_handle(fd: u64) {
    let mut g = FD_TABLE.lock().unwrap();
    g.get_or_insert_with(std::collections::HashSet::new)
        .insert(fd);
}

fn is_fd_handle(handle: u64) -> bool {
    if handle > 0xFFFF {
        return false;
    } // fds are small numbers
    let g = FD_TABLE.lock().unwrap();
    g.as_ref().map(|s| s.contains(&handle)).unwrap_or(false)
}

#[no_mangle]
pub unsafe extern "win64" fn ReadFile(
    h_file: *mut std::ffi::c_void,
    lp_buffer: *mut u8,
    n_number_of_bytes_to_read: u32,
    lp_number_of_bytes_read: *mut u32,
    _lp_overlapped: *mut u8,
) -> i32 {
    let fd = h_file as i64 as i32;
    if fd > 2 && is_fd_handle(fd as u64) && !lp_buffer.is_null() && n_number_of_bytes_to_read > 0 {
        let ret = libc::read(
            fd,
            lp_buffer as *mut libc::c_void,
            n_number_of_bytes_to_read as usize,
        );
        let bytes_read = if ret >= 0 { ret as u32 } else { 0 };

        let first_4 = if bytes_read >= 4 {
            let magic = std::ptr::read_unaligned(lp_buffer as *const u32);
            format!(
                "0x{:08X} ASCII='{}'",
                magic,
                String::from_utf8_lossy(&magic.to_le_bytes())
            )
        } else {
            "N/A".to_string()
        };

        eprintln!(
            "[KERNEL32 FILE TRACE] ReadFile: fd={}, requested={}, read={}, first_4_bytes=[{}]",
            fd, n_number_of_bytes_to_read, bytes_read, first_4
        );

        if ret < 0 {
            if !lp_number_of_bytes_read.is_null() {
                *lp_number_of_bytes_read = 0;
            }
            return 0; // FALSE
        }
        if !lp_number_of_bytes_read.is_null() {
            *lp_number_of_bytes_read = bytes_read;
        }
        return 1; // TRUE
    }
    // fallback: zero fill
    if !lp_buffer.is_null() && n_number_of_bytes_to_read > 0 {
        std::ptr::write_bytes(lp_buffer, 0, n_number_of_bytes_to_read as usize);
    }
    if !lp_number_of_bytes_read.is_null() {
        *lp_number_of_bytes_read = n_number_of_bytes_to_read;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WriteFile(
    h_file: *mut std::ffi::c_void,
    lp_buffer: *const u8,
    n_number_of_bytes_to_write: u32,
    lp_number_of_bytes_written: *mut u32,
    _lp_overlapped: *mut u8,
) -> i32 {
    let fd = h_file as i64 as i32;
    if !lp_buffer.is_null() && n_number_of_bytes_to_write > 0 {
        if fd == 1 || fd == 2 || fd == 4 || fd == 8 || fd == 12 || is_fd_handle(fd as u64) {
            let target_fd = if fd <= 12 { 1 } else { fd };
            let ret = libc::write(
                target_fd,
                lp_buffer as *const libc::c_void,
                n_number_of_bytes_to_write as usize,
            );
            if ret >= 0 {
                if !lp_number_of_bytes_written.is_null() {
                    *lp_number_of_bytes_written = ret as u32;
                }
                return 1; // TRUE
            }
        }
    }
    if !lp_number_of_bytes_written.is_null() {
        *lp_number_of_bytes_written = n_number_of_bytes_to_write;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileSize(
    h_file: *mut std::ffi::c_void,
    lp_file_size_high: *mut u32,
) -> u32 {
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::fstat(fd, &mut st) == 0 {
            let size = st.st_size as u64;
            if !lp_file_size_high.is_null() {
                *lp_file_size_high = (size >> 32) as u32;
            }
            eprintln!(
                "[KERNEL32 FILE TRACE] GetFileSize: fd={}, size={}",
                fd, size
            );
            return size as u32;
        }
    }
    const INVALID_FILE_SIZE: u32 = 0xFFFFFFFF;
    INVALID_FILE_SIZE
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileSizeEx(
    h_file: *mut std::ffi::c_void,
    lp_file_size: *mut i64,
) -> i32 {
    if lp_file_size.is_null() {
        return 0;
    }
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::fstat(fd, &mut st) == 0 {
            *lp_file_size = st.st_size as i64;
            eprintln!(
                "[KERNEL32 FILE TRACE] GetFileSizeEx: fd={}, size={}",
                fd, st.st_size
            );
            return 1; // TRUE
        }
    }
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn SetFilePointer(
    h_file: *mut std::ffi::c_void,
    l_distance_to_move: i32,
    lp_distance_to_move_high: *mut i32,
    dw_move_method: u32,
) -> u32 {
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let offset: i64 = if !lp_distance_to_move_high.is_null() {
            let high = *lp_distance_to_move_high as i64;
            let low = l_distance_to_move as u32 as u64;
            (high << 32) | (low as i64)
        } else {
            l_distance_to_move as i64
        };
        let whence = match dw_move_method {
            0 => libc::SEEK_SET,
            1 => libc::SEEK_CUR,
            2 => libc::SEEK_END,
            _ => libc::SEEK_SET,
        };
        let new_off = libc::lseek(fd, offset, whence);
        eprintln!(
            "[KERNEL32 FILE TRACE] SetFilePointer: fd={}, move={}(0x{:X}), high_ptr={:?}, method={}, offset_64={}, new_off={}",
            fd, l_distance_to_move, l_distance_to_move as u32, lp_distance_to_move_high, dw_move_method, offset, new_off
        );
        if new_off < 0 {
            return !0u32;
        }
        if !lp_distance_to_move_high.is_null() {
            *lp_distance_to_move_high = (new_off >> 32) as i32;
        }
        return new_off as u32;
    }
    !0u32
}

#[no_mangle]
pub unsafe extern "win64" fn SetFilePointerEx(
    h_file: *mut std::ffi::c_void,
    li_distance_to_move: i64,
    lp_new_file_pointer: *mut i64,
    dw_move_method: u32,
) -> i32 {
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let whence = match dw_move_method {
            0 => libc::SEEK_SET,
            1 => libc::SEEK_CUR,
            2 => libc::SEEK_END,
            _ => libc::SEEK_SET,
        };
        let new_off = libc::lseek(fd, li_distance_to_move, whence);
        eprintln!(
            "[KERNEL32 FILE TRACE] SetFilePointerEx: fd={}, dist={}, method={}, new_off={}",
            fd, li_distance_to_move, dw_move_method, new_off
        );
        if new_off < 0 {
            return 0;
        }
        if !lp_new_file_pointer.is_null() {
            *lp_new_file_pointer = new_off;
        }
        return 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn WriteConsoleW(
    _h_console_output: u64,
    lp_buffer: *const u16,
    n_number_of_chars_to_write: u32,
    lp_number_of_chars_written: *mut u32,
    _lp_reserved: *mut std::ffi::c_void,
) -> i32 {
    if !lp_buffer.is_null() && n_number_of_chars_to_write > 0 {
        let slice = std::slice::from_raw_parts(lp_buffer, n_number_of_chars_to_write as usize);
        let s = String::from_utf16_lossy(slice);
        eprint!("{}", s);
        if !lp_number_of_chars_written.is_null() {
            *lp_number_of_chars_written = n_number_of_chars_to_write;
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WriteConsoleA(
    _h_console_output: u64,
    lp_buffer: *const u8,
    n_number_of_chars_to_write: u32,
    lp_number_of_chars_written: *mut u32,
    _lp_reserved: *mut std::ffi::c_void,
) -> i32 {
    if !lp_buffer.is_null() && n_number_of_chars_to_write > 0 {
        let slice = std::slice::from_raw_parts(lp_buffer, n_number_of_chars_to_write as usize);
        let s = String::from_utf8_lossy(slice);
        eprint!("{}", s);
        if !lp_number_of_chars_written.is_null() {
            *lp_number_of_chars_written = n_number_of_chars_to_write;
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetStdHandle(std_handle_id: i32) -> u64 {
    match std_handle_id {
        -10 => 0x0000000000000004,
        -11 => 0x0000000000000008,
        -12 => 0x000000000000000C,
        _ => 0xFFFFFFFFFFFFFFFF,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileType(_handle: u64) -> u32 {
    0x0002
}

#[no_mangle]
pub unsafe extern "win64" fn GetConsoleMode(_handle: u64, mode_out: *mut u32) -> i32 {
    if !mode_out.is_null() {
        *mode_out = 0x0007;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetConsoleMode(_handle: u64, _mode: u32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn FindResourceW(_module: u64, _name: u64, _type: u64) -> u64 {
    let base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    base + 0x043A9000
}

#[no_mangle]
pub unsafe extern "win64" fn FindResourceA(module: u64, name: u64, rtype: u64) -> u64 {
    FindResourceW(module, name, rtype)
}

#[no_mangle]
pub unsafe extern "win64" fn LoadResource(_module: u64, resource_handle: u64) -> u64 {
    resource_handle
}

#[no_mangle]
pub unsafe extern "win64" fn LockResource(resource_data_handle: u64) -> u64 {
    resource_data_handle
}

#[no_mangle]
pub unsafe extern "win64" fn SizeofResource(_module: u64, _resource_handle: u64) -> u32 {
    4096
}

#[no_mangle]
pub unsafe extern "win64" fn CreateThread(
    _lp_thread_attributes: *mut u8,
    dw_stack_size: usize,
    lp_start_address: u64,
    lp_parameter: u64,
    dw_creation_flags: u32,
    lp_thread_id: *mut u32,
) -> u64 {
    use std::sync::atomic::{AtomicU32, Ordering as AO};
    static THREAD_COUNTER: AtomicU32 = AtomicU32::new(0x200);

    let thread_id = THREAD_COUNTER.fetch_add(1, AO::Relaxed);
    if !lp_thread_id.is_null() {
        *lp_thread_id = thread_id;
    }

    // CREATE_SUSPENDED = 0x4: don't start yet (simplification: we ignore and start immediately)
    if lp_start_address < 0x10000 || lp_start_address > 0x7FFF_FFFF_FFFFu64 {
        println!(
            "[KERNEL32 THREAD] CreateThread: invalid start address 0x{:016X}",
            lp_start_address
        );
        return 0;
    }

    println!(
        "[KERNEL32 THREAD] CreateThread: id={}, start=0x{:016X}, param=0x{:016X}, flags=0x{:X}",
        thread_id, lp_start_address, lp_parameter, dw_creation_flags
    );

    // Allocate a fake HANDLE value for this thread (odd numbers to avoid confusion with ptrs)
    let handle = (thread_id as u64) << 4 | 0x3;

    // Store in handle table so WaitForSingleObject can join later
    {
        let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
        let table = guard.get_or_insert_with(HashMap::new);
        // We use a channel to signal thread completion
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        table.insert(handle, rx);

        // Clone image_base for TLS setup inside the new thread
        let image_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
        let stack_size = if dw_stack_size == 0 {
            2 * 1024 * 1024
        } else {
            dw_stack_size
        };

        std::thread::Builder::new()
            .stack_size(stack_size)
            .spawn(move || {
                // Set up a minimal TEB-like GS base for this thread
                // so Windows code using fs:[0] / gs:[0] doesn't crash immediately.
                // We allocate a small per-thread block and point GS at it.
                let teb_size = 0x2000usize;
                let teb_buf = vec![0u8; teb_size];
                let teb_ptr = teb_buf.as_ptr() as u64;

                // Write the self-pointer (TEB->NtTib.Self) at offset 0x30
                std::ptr::write_unaligned((teb_ptr + 0x30) as *mut u64, teb_ptr);
                // Write image_base into a TEB slot so DXVK can find it (PEB offset 0x10 from TEB)
                std::ptr::write_unaligned((teb_ptr + 0x60) as *mut u64, image_base);

                // Set GS base for this new thread
                let _ = crate::sys::trampoline::set_thread_teb_gs_base(teb_ptr);

                std::mem::forget(teb_buf); // keep alive for thread duration

                let thread_fn: unsafe extern "win64" fn(u64) -> u32 =
                    std::mem::transmute(lp_start_address);
                let ret = thread_fn(lp_parameter);
                println!(
                    "[KERNEL32 THREAD] Thread 0x{:X} exited with code {}",
                    handle, ret
                );
                let _ = tx.send(()); // signal completion
            })
            .expect("Failed to spawn guest thread");
    }

    handle
}

static THREAD_HANDLE_TABLE: std::sync::Mutex<Option<HashMap<u64, std::sync::mpsc::Receiver<()>>>> =
    std::sync::Mutex::new(None);

/// HANDLE-based event table — maps fake HANDLE → (Mutex<bool>, Condvar) pair
static EVENT_TABLE: std::sync::Mutex<
    Option<HashMap<u64, std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>>>,
> = std::sync::Mutex::new(None);
static HANDLE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x1000);

fn next_handle() -> u64 {
    HANDLE_COUNTER.fetch_add(4, std::sync::atomic::Ordering::Relaxed)
}

fn event_table_insert(handle: u64, manual_reset: bool, initial_state: bool) {
    let cv = std::sync::Arc::new((
        std::sync::Mutex::new(initial_state),
        std::sync::Condvar::new(),
    ));
    let mut g = EVENT_TABLE.lock().unwrap();
    g.get_or_insert_with(HashMap::new).insert(handle, cv);
    let _ = manual_reset; // TODO: manual reset vs auto-reset
}

fn event_table_get(
    handle: u64,
) -> Option<std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>> {
    let g = EVENT_TABLE.lock().unwrap();
    g.as_ref()?.get(&handle).cloned()
}

#[no_mangle]
pub unsafe extern "win64" fn SetEvent(h_event: u64) -> i32 {
    if let Some(cv) = event_table_get(h_event) {
        let (lock, cvar) = &*cv;
        let mut signaled = lock.lock().unwrap();
        *signaled = true;
        cvar.notify_all();
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn ResetEvent(h_event: u64) -> i32 {
    if let Some(cv) = event_table_get(h_event) {
        let (lock, _) = &*cv;
        let mut signaled = lock.lock().unwrap();
        *signaled = false;
        1
    } else {
        0
    }
}

/// WAIT_OBJECT_0 = 0, WAIT_TIMEOUT = 0x102, INFINITE = 0xFFFFFFFF
#[no_mangle]
pub unsafe extern "win64" fn WaitForSingleObject(h_handle: u64, dw_milliseconds: u32) -> u32 {
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 0x102;

    // Check if it's an event handle
    if let Some(cv) = event_table_get(h_handle) {
        let (lock, cvar) = &*cv;
        if dw_milliseconds == 0xFFFFFFFF {
            // Infinite wait
            let mut signaled = lock.lock().unwrap();
            while !*signaled {
                signaled = cvar.wait(signaled).unwrap();
            }
            return WAIT_OBJECT_0;
        } else {
            let timeout = std::time::Duration::from_millis(dw_milliseconds as u64);
            let signaled = lock.lock().unwrap();
            let result = cvar.wait_timeout(signaled, timeout).unwrap();
            return if *result.0 {
                WAIT_OBJECT_0
            } else {
                WAIT_TIMEOUT
            };
        }
    }

    // Check thread handles
    {
        let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_mut() {
            if let Some(rx) = table.get(&h_handle) {
                if dw_milliseconds == 0xFFFFFFFF {
                    let _ = rx.recv();
                } else {
                    let _ =
                        rx.recv_timeout(std::time::Duration::from_millis(dw_milliseconds as u64));
                }
                return WAIT_OBJECT_0;
            }
        }
    }

    // Unknown handle — return immediately signaled (safe default for stubs)
    WAIT_OBJECT_0
}

#[no_mangle]
pub unsafe extern "win64" fn WaitForMultipleObjects(
    n_count: u32,
    lp_handles: *const u64,
    _b_wait_all: i32,
    dw_milliseconds: u32,
) -> u32 {
    // Wait for first signaled object (simplified: wait on first)
    if n_count == 0 || lp_handles.is_null() {
        return 0;
    }
    for i in 0..n_count as usize {
        let h = *lp_handles.add(i);
        let r = WaitForSingleObject(
            h,
            if dw_milliseconds == 0xFFFFFFFF {
                0
            } else {
                dw_milliseconds
            },
        );
        if r == 0 {
            return i as u32; // WAIT_OBJECT_0 + i
        }
    }
    0x102 // WAIT_TIMEOUT
}

#[no_mangle]
pub unsafe extern "win64" fn CloseHandle(h_object: u64) -> i32 {
    // Remove from event table
    {
        let mut g = EVENT_TABLE.lock().unwrap();
        if let Some(table) = g.as_mut() {
            table.remove(&h_object);
        }
    }
    // Remove from thread table
    {
        let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_mut() {
            table.remove(&h_object);
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn OpenThread(
    _dw_desired_access: u32,
    _b_inherit_handle: i32,
    dw_thread_id: u32,
) -> u64 {
    // Return a fake handle based on thread ID
    (dw_thread_id as u64) << 4 | 0x3
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentThread() -> u64 {
    0xFFFFFFFFFFFFFFFEu64 // INVALID_HANDLE_VALUE sentinel for current thread
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentThreadId() -> u32 {
    unsafe { libc::gettid() as u32 }
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentProcessId() -> u32 {
    unsafe { libc::getpid() as u32 }
}

#[no_mangle]
pub unsafe extern "win64" fn GetExitCodeThread(h_thread: u64, lp_exit_code: *mut u32) -> i32 {
    if !lp_exit_code.is_null() {
        *lp_exit_code = 0; // STILL_ACTIVE would be 259, but we return 0 (exited)
    }
    let _ = h_thread;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetThreadPriority(_h_thread: u64, _n_priority: i32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SuspendThread(_h_thread: u64) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ResumeThread(_h_thread: u64) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn Sleep(dw_milliseconds: u32) {
    std::thread::sleep(std::time::Duration::from_millis(dw_milliseconds as u64));
}

#[no_mangle]
pub unsafe extern "win64" fn SleepEx(dw_milliseconds: u32, _b_alertable: i32) -> u32 {
    Sleep(dw_milliseconds);
    0 // WAIT_IO_COMPLETION would be 0xC0
}

#[no_mangle]
pub unsafe extern "win64" fn CreateMutexExW(
    attr: usize,
    _name: usize,
    dw_flags: u32,
    _access: u32,
) -> usize {
    let initial_owner = if (dw_flags & 0x00000001) != 0 { 1 } else { 0 };
    CreateMutexW(attr, initial_owner, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateMutexExA(
    attr: usize,
    name: usize,
    dw_flags: u32,
    access: u32,
) -> usize {
    CreateMutexExW(attr, name, dw_flags, access)
}

#[no_mangle]
pub unsafe extern "win64" fn ReleaseMutex(h_mutex: u64) -> i32 {
    SetEvent(h_mutex)
}

#[no_mangle]
pub unsafe extern "win64" fn GetProcAddress(
    h_module: usize,
    lp_proc_name: *const libc::c_char,
) -> usize {
    if lp_proc_name.is_null() {
        return 0;
    }
    let func_name = if (lp_proc_name as usize) < 0x10000 {
        format!("#{}", lp_proc_name as usize)
    } else {
        std::ffi::CStr::from_ptr(lp_proc_name)
            .to_string_lossy()
            .to_string()
    };

    let module_name = crate::pe::imports::find_module_name_by_base(h_module)
        .unwrap_or_else(|| format!("HMODULE_0x{:016X}", h_module));

    if let Some(addr) = crate::dlls::dxvk_loader::resolve_dxvk_symbol("d3d11.dll", &func_name) {
        println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
        return addr;
    }
    if let Some(addr) = crate::dlls::dxvk_loader::resolve_dxvk_symbol("dxgi.dll", &func_name) {
        println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
        return addr;
    }
    if let Some(addr) = crate::dlls::dxvk_loader::resolve_vulkan_symbol(&func_name) {
        println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
        return addr;
    }
    if let Some(addr) =
        crate::pe::imports::IatSymbolResolver::resolve_symbol(&module_name, &func_name)
    {
        println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
        return addr;
    }
    if let Some(addr) =
        crate::pe::imports::IatSymbolResolver::resolve_symbol("KERNEL32.DLL", &func_name)
    {
        println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
        return addr;
    }
    if let Some(addr) =
        crate::pe::imports::IatSymbolResolver::resolve_symbol("USER32.DLL", &func_name)
    {
        println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
        return addr;
    }

    println!(
        "[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x0 stub_id=0 (UNRESOLVED)",
        h_module, module_name, func_name
    );
    0
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryW(lp_lib_name: *const u16) -> usize {
    if lp_lib_name.is_null() {
        return 0;
    }
    let mut len = 0;
    while *lp_lib_name.add(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(lp_lib_name, len);
    let name = String::from_utf16_lossy(slice);
    println!("[KERNEL32] LoadLibraryW('{}')", name);

    0x140000000
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryA(lp_lib_name: *const libc::c_char) -> usize {
    if lp_lib_name.is_null() {
        return 0;
    }
    let name = std::ffi::CStr::from_ptr(lp_lib_name).to_string_lossy();
    println!("[KERNEL32] LoadLibraryA('{}')", name);
    0x140000000
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryExW(
    lp_lib_name: *const u16,
    _h_file: usize,
    _flags: u32,
) -> usize {
    LoadLibraryW(lp_lib_name)
}

static TOP_LEVEL_EXCEPTION_FILTER: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[no_mangle]
pub unsafe extern "win64" fn SetUnhandledExceptionFilter(
    lp_top_level_exception_filter: usize,
) -> usize {
    TOP_LEVEL_EXCEPTION_FILTER.swap(
        lp_top_level_exception_filter,
        std::sync::atomic::Ordering::SeqCst,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentProcess() -> usize {
    0xFFFFFFFFFFFFFFFF
}

#[no_mangle]
pub unsafe extern "win64" fn GetThreadPriority(_h_thread: usize) -> i32 {
    0 // THREAD_PRIORITY_NORMAL
}

#[no_mangle]
pub unsafe extern "win64" fn DuplicateHandle(
    _h_source_process_handle: usize,
    h_source_handle: usize,
    _h_target_process_handle: usize,
    lp_target_handle: *mut usize,
    _dw_desired_access: u32,
    _b_inherit_handle: u32,
    _dw_options: u32,
) -> i32 {
    if !lp_target_handle.is_null() {
        *lp_target_handle = h_source_handle;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WideCharToMultiByte(
    _code_page: u32,
    _dw_flags: u32,
    lp_wide_char_str: *const u16,
    cch_wide_char: i32,
    lp_multi_byte_str: *mut u8,
    cb_multi_byte: i32,
    _lp_default_char: *const u8,
    _lp_used_default_char: *mut i32,
) -> i32 {
    if lp_wide_char_str.is_null() {
        return 0;
    }
    let len = if cch_wide_char == -1 {
        let mut l = 0;
        while *lp_wide_char_str.add(l) != 0 {
            l += 1;
        }
        l + 1
    } else if cch_wide_char < 0 {
        return 0;
    } else {
        cch_wide_char as usize
    };

    let slice = std::slice::from_raw_parts(lp_wide_char_str, len);
    let mut out_bytes = Vec::with_capacity(len);
    for &wc in slice {
        if wc <= 0x7F {
            out_bytes.push(wc as u8);
        } else if wc <= 0x7FF {
            out_bytes.push((0xC0 | (wc >> 6)) as u8);
            out_bytes.push((0x80 | (wc & 0x3F)) as u8);
        } else {
            out_bytes.push((0xE0 | (wc >> 12)) as u8);
            out_bytes.push((0x80 | ((wc >> 6) & 0x3F)) as u8);
            out_bytes.push((0x80 | (wc & 0x3F)) as u8);
        }
    }

    if cb_multi_byte == 0 || lp_multi_byte_str.is_null() {
        return out_bytes.len() as i32;
    }

    if (cb_multi_byte as usize) < out_bytes.len() {
        if cb_multi_byte > 0 {
            std::ptr::copy_nonoverlapping(
                out_bytes.as_ptr(),
                lp_multi_byte_str,
                cb_multi_byte as usize,
            );
        }
        return 0;
    }

    std::ptr::copy_nonoverlapping(out_bytes.as_ptr(), lp_multi_byte_str, out_bytes.len());
    out_bytes.len() as i32
}

#[no_mangle]
pub unsafe extern "win64" fn MultiByteToWideChar(
    _code_page: u32,
    _dw_flags: u32,
    lp_multi_byte_str: *const u8,
    cb_multi_byte: i32,
    lp_wide_char_str: *mut u16,
    cch_wide_char: i32,
) -> i32 {
    if lp_multi_byte_str.is_null() {
        return 0;
    }
    let len = if cb_multi_byte == -1 {
        let mut l = 0;
        while *lp_multi_byte_str.add(l) != 0 {
            l += 1;
        }
        l + 1
    } else if cb_multi_byte < 0 {
        return 0;
    } else {
        cb_multi_byte as usize
    };

    let slice = std::slice::from_raw_parts(lp_multi_byte_str, len);
    let mut out_u16 = Vec::with_capacity(len);
    let mut i = 0;
    while i < slice.len() {
        let b = slice[i];
        if b <= 0x7F {
            out_u16.push(b as u16);
            i += 1;
        } else if (0xC0..=0xDF).contains(&b) && i + 1 < slice.len() {
            let cp = (((b & 0x1F) as u16) << 6) | ((slice[i + 1] & 0x3F) as u16);
            out_u16.push(cp);
            i += 2;
        } else if (0xE0..=0xEF).contains(&b) && i + 2 < slice.len() {
            let cp = (((b & 0x0F) as u16) << 12)
                | (((slice[i + 1] & 0x3F) as u16) << 6)
                | ((slice[i + 2] & 0x3F) as u16);
            out_u16.push(cp);
            i += 3;
        } else {
            out_u16.push(b as u16);
            i += 1;
        }
    }

    if cch_wide_char == 0 || lp_wide_char_str.is_null() {
        return out_u16.len() as i32;
    }

    if (cch_wide_char as usize) < out_u16.len() {
        if cch_wide_char > 0 {
            std::ptr::copy_nonoverlapping(
                out_u16.as_ptr(),
                lp_wide_char_str,
                cch_wide_char as usize,
            );
        }
        return 0;
    }

    std::ptr::copy_nonoverlapping(out_u16.as_ptr(), lp_wide_char_str, out_u16.len());
    out_u16.len() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_heap_allocation() {
        let heap = unsafe { GetProcessHeap() };
        let ptr = unsafe { HeapAlloc(heap, HEAP_ZERO_MEMORY, 256) };
        assert!(ptr > 0);

        unsafe {
            let slice = std::slice::from_raw_parts(ptr as *const u8, 256);
            assert!(slice.iter().all(|&b| b == 0));
        }

        assert_eq!(unsafe { HeapFree(heap, 0, ptr) }, 1);
    }
}
