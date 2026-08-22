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
    if ptr > 0x10000 {
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

static VIRTUAL_ALLOC_REGISTRY: Mutex<Option<std::collections::HashMap<usize, usize>>> = Mutex::new(None);

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
    let res = if ptr == libc::MAP_FAILED || ptr.is_null() {
        let fallback = libc::calloc(1, alloc_size);
        if fallback.is_null() {
            static FALLBACK_VIRT: [u8; 1048576] = [0; 1048576];
            FALLBACK_VIRT.as_ptr() as usize
        } else {
            fallback as usize
        }
    } else {
        ptr as usize
    };
    if res > 0x10000 {
        let mut guard = VIRTUAL_ALLOC_REGISTRY.lock().unwrap();
        if guard.is_none() {
            *guard = Some(std::collections::HashMap::new());
        }
        if let Some(map) = guard.as_mut() {
            map.insert(res, alloc_size);
        }
    }
    res
}

pub unsafe extern "win64" fn VirtualFree(address: usize, size: usize, free_type: u32) -> i32 {
    if address == 0 {
        return 0;
    }
    const MEM_DECOMMIT: u32 = 0x4000;
    const MEM_RELEASE: u32 = 0x8000;

    if (free_type & MEM_RELEASE) != 0 || free_type == 0 {
        let unmap_size = {
            let mut guard = VIRTUAL_ALLOC_REGISTRY.lock().unwrap();
            guard.as_mut().and_then(|map| map.remove(&address)).unwrap_or(if size > 0 {
                (size + 4095) & !4095
            } else {
                4096
            })
        };
        if address > 0x10000 {
            libc::munmap(address as *mut libc::c_void, unmap_size);
        }
    } else if (free_type & MEM_DECOMMIT) != 0 {
        let decommit_size = if size > 0 {
            (size + 4095) & !4095
        } else {
            4096
        };
        if address > 0x10000 {
            libc::madvise(address as *mut libc::c_void, decommit_size, libc::MADV_DONTNEED);
        }
    }
    1
}

pub static CURRENT_IMAGE_BASE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0x0000000140000000);
pub static CURRENT_PEB_ADDRESS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn set_current_image_base(base: u64) {
    CURRENT_IMAGE_BASE.store(base, std::sync::atomic::Ordering::SeqCst);
}

pub fn get_current_image_base() -> u64 {
    CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn set_current_peb_address(peb: u64) {
    CURRENT_PEB_ADDRESS.store(peb, std::sync::atomic::Ordering::SeqCst);
}

pub fn get_current_peb_address() -> u64 {
    CURRENT_PEB_ADDRESS.load(std::sync::atomic::Ordering::SeqCst)
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

static CS_REGISTRY: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, Box<libc::pthread_mutex_t>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

static SRW_REGISTRY: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, Box<libc::pthread_rwlock_t>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

pub unsafe extern "win64" fn EnterCriticalSection(cs: *mut RtlCriticalSection) {
    if cs.is_null() || (cs as usize) < 0x10000 {
        return;
    }
    let cs_key = cs as usize;
    let mutex_ptr = {
        let mut reg = CS_REGISTRY.lock().unwrap();
        let entry = reg.entry(cs_key).or_insert_with(|| {
            let mut m: Box<libc::pthread_mutex_t> = Box::new(std::mem::zeroed());
            let mut attr: libc::pthread_mutexattr_t = std::mem::zeroed();
            libc::pthread_mutexattr_init(&mut attr);
            libc::pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_RECURSIVE);
            libc::pthread_mutex_init(m.as_mut(), &attr);
            libc::pthread_mutexattr_destroy(&mut attr);
            m
        });
        entry.as_mut() as *mut libc::pthread_mutex_t
    };
    libc::pthread_mutex_lock(mutex_ptr);
    let tid = libc::pthread_self() as usize;
    (*cs).owning_thread = tid;
    (*cs).recursion_count += 1;
    (*cs).lock_count = 0;
}

pub unsafe extern "win64" fn LeaveCriticalSection(cs: *mut RtlCriticalSection) {
    if cs.is_null() || (cs as usize) < 0x10000 {
        return;
    }
    let cs_key = cs as usize;
    let mutex_ptr = {
        let mut reg = CS_REGISTRY.lock().unwrap();
        if let Some(m) = reg.get_mut(&cs_key) {
            m.as_mut() as *mut libc::pthread_mutex_t
        } else {
            return;
        }
    };
    if (*cs).recursion_count > 1 {
        (*cs).recursion_count -= 1;
    } else {
        (*cs).recursion_count = 0;
        (*cs).owning_thread = 0;
        (*cs).lock_count = -1;
    }
    libc::pthread_mutex_unlock(mutex_ptr);
}

pub unsafe extern "win64" fn TryEnterCriticalSection(cs: *mut RtlCriticalSection) -> i32 {
    if cs.is_null() || (cs as usize) < 0x10000 {
        return 0;
    }
    let cs_key = cs as usize;
    let mutex_ptr = {
        let mut reg = CS_REGISTRY.lock().unwrap();
        let entry = reg.entry(cs_key).or_insert_with(|| {
            let mut m: Box<libc::pthread_mutex_t> = Box::new(std::mem::zeroed());
            let mut attr: libc::pthread_mutexattr_t = std::mem::zeroed();
            libc::pthread_mutexattr_init(&mut attr);
            libc::pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_RECURSIVE);
            libc::pthread_mutex_init(m.as_mut(), &attr);
            libc::pthread_mutexattr_destroy(&mut attr);
            m
        });
        entry.as_mut() as *mut libc::pthread_mutex_t
    };
    let res = libc::pthread_mutex_trylock(mutex_ptr);
    if res == 0 {
        let tid = libc::pthread_self() as usize;
        (*cs).owning_thread = tid;
        (*cs).recursion_count += 1;
        (*cs).lock_count = 0;
        1
    } else {
        0
    }
}

pub unsafe extern "win64" fn DeleteCriticalSection(cs: *mut RtlCriticalSection) {
    if cs.is_null() || (cs as usize) < 0x10000 {
        return;
    }
    let cs_key = cs as usize;
    let mut reg = CS_REGISTRY.lock().unwrap();
    if let Some(mut m) = reg.remove(&cs_key) {
        libc::pthread_mutex_destroy(m.as_mut());
    }
    (*cs).lock_count = -1;
    (*cs).recursion_count = 0;
    (*cs).owning_thread = 0;
}

pub unsafe extern "win64" fn InitializeSRWLock(lock: *mut usize) {
    if !lock.is_null() {
        *lock = 0;
    }
}

pub unsafe extern "win64" fn AcquireSRWLockExclusive(lock: *mut usize) {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        let entry = reg.entry(key).or_insert_with(|| {
            let mut rw: Box<libc::pthread_rwlock_t> = Box::new(std::mem::zeroed());
            libc::pthread_rwlock_init(rw.as_mut(), std::ptr::null());
            rw
        });
        entry.as_mut() as *mut libc::pthread_rwlock_t
    };
    libc::pthread_rwlock_wrlock(rwlock_ptr);
    *lock = 1;
}

pub unsafe extern "win64" fn ReleaseSRWLockExclusive(lock: *mut usize) {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        if let Some(rw) = reg.get_mut(&key) {
            rw.as_mut() as *mut libc::pthread_rwlock_t
        } else {
            *lock = 0;
            return;
        }
    };
    *lock = 0;
    libc::pthread_rwlock_unlock(rwlock_ptr);
}

pub unsafe extern "win64" fn AcquireSRWLockShared(lock: *mut usize) {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        let entry = reg.entry(key).or_insert_with(|| {
            let mut rw: Box<libc::pthread_rwlock_t> = Box::new(std::mem::zeroed());
            libc::pthread_rwlock_init(rw.as_mut(), std::ptr::null());
            rw
        });
        entry.as_mut() as *mut libc::pthread_rwlock_t
    };
    libc::pthread_rwlock_rdlock(rwlock_ptr);
    *lock = 1;
}

pub unsafe extern "win64" fn ReleaseSRWLockShared(lock: *mut usize) {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        if let Some(rw) = reg.get_mut(&key) {
            rw.as_mut() as *mut libc::pthread_rwlock_t
        } else {
            *lock = 0;
            return;
        }
    };
    *lock = 0;
    libc::pthread_rwlock_unlock(rwlock_ptr);
}

pub unsafe extern "win64" fn TryAcquireSRWLockExclusive(lock: *mut usize) -> u8 {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return 0;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        let entry = reg.entry(key).or_insert_with(|| {
            let mut rw: Box<libc::pthread_rwlock_t> = Box::new(std::mem::zeroed());
            libc::pthread_rwlock_init(rw.as_mut(), std::ptr::null());
            rw
        });
        entry.as_mut() as *mut libc::pthread_rwlock_t
    };
    if libc::pthread_rwlock_trywrlock(rwlock_ptr) == 0 {
        *lock = 1;
        1
    } else {
        0
    }
}

pub unsafe extern "win64" fn TryAcquireSRWLockShared(lock: *mut usize) -> u8 {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return 0;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        let entry = reg.entry(key).or_insert_with(|| {
            let mut rw: Box<libc::pthread_rwlock_t> = Box::new(std::mem::zeroed());
            libc::pthread_rwlock_init(rw.as_mut(), std::ptr::null());
            rw
        });
        entry.as_mut() as *mut libc::pthread_rwlock_t
    };
    if libc::pthread_rwlock_tryrdlock(rwlock_ptr) == 0 {
        *lock = 1;
        1
    } else {
        0
    }
}

struct WinCondVar {
    cond: std::sync::Condvar,
    mutex: std::sync::Mutex<()>,
}

static COND_TABLE: std::sync::Mutex<
    Option<std::collections::HashMap<usize, std::sync::Arc<WinCondVar>>>,
> = std::sync::Mutex::new(None);

fn get_or_create_cond(var: *mut usize) -> Option<std::sync::Arc<WinCondVar>> {
    if var.is_null() {
        return None;
    }
    let key = var as usize;
    let mut guard = COND_TABLE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(std::collections::HashMap::new());
    }
    let map = guard.as_mut().unwrap();
    let cv = map
        .entry(key)
        .or_insert_with(|| {
            std::sync::Arc::new(WinCondVar {
                cond: std::sync::Condvar::new(),
                mutex: std::sync::Mutex::new(()),
            })
        })
        .clone();
    Some(cv)
}

pub unsafe extern "win64" fn InitializeConditionVariable(var: *mut usize) {
    if !var.is_null() {
        *var = 0;
    }
}

pub unsafe extern "win64" fn WakeConditionVariable(var: *mut usize) {
    if let Some(cv) = get_or_create_cond(var) {
        cv.cond.notify_one();
    }
}

pub unsafe extern "win64" fn WakeAllConditionVariable(var: *mut usize) {
    if let Some(cv) = get_or_create_cond(var) {
        cv.cond.notify_all();
    }
}

pub unsafe extern "win64" fn SleepConditionVariableCS(
    var: *mut usize,
    cs: *mut RtlCriticalSection,
    timeout: u32,
) -> i32 {
    let cv = match get_or_create_cond(var) {
        Some(c) => c,
        None => return 0,
    };

    LeaveCriticalSection(cs);
    let guard = cv.mutex.lock().unwrap();
    if timeout == 0xFFFFFFFF {
        let _unused = cv.cond.wait(guard);
    } else {
        let _ = cv
            .cond
            .wait_timeout(guard, std::time::Duration::from_millis(timeout as u64));
    }
    EnterCriticalSection(cs);
    1
}

pub unsafe extern "win64" fn SleepConditionVariableSRW(
    var: *mut usize,
    lock: *mut usize,
    timeout: u32,
    flags: u32,
) -> i32 {
    let cv = match get_or_create_cond(var) {
        Some(c) => c,
        None => return 0,
    };

    let is_shared = (flags & 1) != 0;
    if is_shared {
        ReleaseSRWLockShared(lock);
    } else {
        ReleaseSRWLockExclusive(lock);
    }

    let guard = cv.mutex.lock().unwrap();
    if timeout == 0xFFFFFFFF {
        let _unused = cv.cond.wait(guard);
    } else {
        let _ = cv
            .cond
            .wait_timeout(guard, std::time::Duration::from_millis(timeout as u64));
    }

    if is_shared {
        AcquireSRWLockShared(lock);
    } else {
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
    for i in 0..4096usize {
        let ch = *ptr.add(i);
        if ch == 0 {
            break;
        }
        buf.push(ch);
    }
    Some(String::from_utf16_lossy(&buf))
}

pub struct ModuleRegistry;

#[derive(Debug, Clone)]
pub struct RegisteredModule {
    pub name: String,
    pub base: usize,
    pub size: usize,
    pub refcount: u32,
    pub pinned: bool,
}

static MODULE_TABLE: std::sync::Mutex<Option<Vec<RegisteredModule>>> = std::sync::Mutex::new(None);

fn normalize_module_name(raw_name: &str) -> String {
    let path = std::path::Path::new(raw_name);
    let mut file_name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| raw_name.to_string());

    if !file_name.contains('.') {
        file_name.push_str(".dll");
    }
    file_name
}

fn get_known_native_base(name: &str) -> Option<usize> {
    let lower = name.to_ascii_lowercase();
    if lower == "ntdll.dll" || lower == "ntdll" {
        Some(0x7FFD0000)
    } else if lower == "kernel32.dll"
        || lower == "kernel32"
        || lower == "kernelbase.dll"
        || lower == "kernelbase"
    {
        Some(0x7FFF0000)
    } else if lower == "user32.dll" || lower == "user32" {
        Some(0x7FFE0000)
    } else if lower == "gdi32.dll" || lower == "gdi32" {
        Some(0x7FFC0000)
    } else if lower == "advapi32.dll" || lower == "advapi32" {
        Some(0x7FFB0000)
    } else if lower == "shell32.dll" || lower == "shell32" {
        Some(0x7FFA0000)
    } else if lower == "ole32.dll" || lower == "ole32" {
        Some(0x7FF90000)
    } else if lower == "ws2_32.dll" || lower == "ws2_32" {
        Some(0x7FF80000)
    } else if lower == "bcrypt.dll" || lower == "bcrypt" {
        Some(0x7FF70000)
    } else if lower == "shcore.dll" || lower == "shcore" {
        Some(0x7FF60000)
    } else if lower == "uxtheme.dll" || lower == "uxtheme" {
        Some(0x7FF50000)
    } else if lower == "dwmapi.dll" || lower == "dwmapi" {
        Some(0x7FF40000)
    } else if lower == "version.dll" || lower == "version" {
        Some(0x7FF30000)
    } else if lower == "avrt.dll" || lower == "avrt" {
        Some(0x7FF20000)
    } else if lower == "hid.dll" || lower == "hid" {
        Some(0x7FF10000)
    } else if lower == "vulkan-1.dll" || lower == "vulkan-1" {
        Some(0x7FF00000)
    } else {
        None
    }
}

impl ModuleRegistry {
    pub fn load_library(raw_name: &str) -> usize {
        let norm = normalize_module_name(raw_name);
        let mut guard = MODULE_TABLE.lock().unwrap();
        let table = guard.get_or_insert_with(Vec::new);

        // 1. Check if already registered
        for m in table.iter_mut() {
            if m.name.eq_ignore_ascii_case(&norm) {
                m.refcount = m.refcount.saturating_add(1);
                return m.base;
            }
        }

        // 2. Known native pseudo DLL
        if let Some(base) = get_known_native_base(&norm) {
            table.push(RegisteredModule {
                name: norm.clone(),
                base,
                size: 0x10000,
                refcount: 1,
                pinned: true,
            });
            return base;
        }

        // 3. Check LOADED_MODULES (PE modules like DXVK or Main EXE)
        let pe_entry = {
            let guard_pe = crate::pe::imports::LOADED_MODULES.lock().unwrap();
            guard_pe.as_ref().and_then(|vec| {
                vec.iter()
                    .find(|(name, _, _)| normalize_module_name(name).eq_ignore_ascii_case(&norm))
                    .cloned()
            })
        };

        if let Some((_, base, size)) = pe_entry {
            table.push(RegisteredModule {
                name: norm.clone(),
                base,
                size,
                refcount: 1,
                pinned: true,
            });
            return base;
        }

        // 4. Wine provider fallback if allowlisted
        let config = crate::providers::get_global_wine_config();
        if config.enabled && crate::providers::wine::is_module_allowlisted(&config, &norm) {
            if let Some(dll_dir) = config
                .dll_path
                .clone()
                .or_else(crate::providers::wine::find_wine_dll_dir)
            {
                let dll_file_path = dll_dir.join(&norm);
                if dll_file_path.exists() {
                    if let Ok(module) =
                        crate::providers::wine::load_and_map_wine_pe_dll(&dll_file_path, &config)
                    {
                        let base = module.image_base;
                        let size = module.image_size;
                        table.push(RegisteredModule {
                            name: norm.clone(),
                            base,
                            size,
                            refcount: 1,
                            pinned: false,
                        });
                        return base;
                    }
                }
            }
        }

        // 5. Unknown / absent module returns 0 (ERROR_MOD_NOT_FOUND)
        0
    }

    pub fn get_module_handle(raw_name: Option<&str>) -> usize {
        let raw = match raw_name {
            Some(s) if !s.is_empty() => s,
            _ => {
                return CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) as usize;
            }
        };
        let norm = normalize_module_name(raw);
        let guard = MODULE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_ref() {
            for m in table {
                if m.name.eq_ignore_ascii_case(&norm) {
                    return m.base;
                }
            }
        }

        if let Some(guard_pe) = crate::pe::imports::LOADED_MODULES.lock().unwrap().as_ref() {
            for (pe_name, pe_base, _) in guard_pe {
                let pe_norm = normalize_module_name(pe_name);
                if pe_norm.eq_ignore_ascii_case(&norm) {
                    return *pe_base;
                }
            }
        }

        if let Some(base) = get_known_native_base(&norm) {
            return base;
        }

        0
    }

    pub fn get_module_name(base: usize) -> Option<String> {
        let exe_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) as usize;
        if base == exe_base {
            return crate::pe::imports::find_module_name_by_base(base)
                .or_else(|| Some("TARGET.EXE".to_string()));
        }

        let guard = MODULE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_ref() {
            for m in table {
                if m.base == base {
                    return Some(m.name.clone());
                }
            }
        }

        if let Some(name) = crate::pe::imports::find_module_name_by_base(base) {
            return Some(name);
        }

        match base {
            0x7FFD0000 => Some("ntdll.dll".to_string()),
            0x7FFF0000 => Some("kernel32.dll".to_string()),
            0x7FFE0000 => Some("user32.dll".to_string()),
            0x7FFC0000 => Some("gdi32.dll".to_string()),
            0x7FFB0000 => Some("advapi32.dll".to_string()),
            0x7FFA0000 => Some("shell32.dll".to_string()),
            0x7FF90000 => Some("ole32.dll".to_string()),
            0x7FF80000 => Some("ws2_32.dll".to_string()),
            0x7FF70000 => Some("bcrypt.dll".to_string()),
            0x7FF60000 => Some("shcore.dll".to_string()),
            0x7FF50000 => Some("uxtheme.dll".to_string()),
            0x7FF40000 => Some("dwmapi.dll".to_string()),
            0x7FF30000 => Some("version.dll".to_string()),
            0x7FF20000 => Some("avrt.dll".to_string()),
            0x7FF10000 => Some("hid.dll".to_string()),
            0x7FF00000 => Some("vulkan-1.dll".to_string()),
            _ => None,
        }
    }

    pub fn free_library(base: usize) -> bool {
        let exe_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) as usize;
        if base == 0 {
            return false;
        }
        if base == exe_base {
            // Main executable cannot be unloaded
            return false;
        }

        {
            let mut guard = MODULE_TABLE.lock().unwrap();
            if let Some(table) = guard.as_mut() {
                for m in table.iter_mut() {
                    if m.base == base {
                        if m.refcount > 0 {
                            m.refcount -= 1;
                        }
                        return true;
                    }
                }
            }
        }

        if Self::get_module_name(base).is_some() {
            return true;
        }

        false
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleHandleW(module_name: u64) -> u64 {
    if module_name == 0 || module_name < 0x10000 {
        return CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    }
    let name_opt = read_utf16_ptr(module_name as *const u16);
    ModuleRegistry::get_module_handle(name_opt.as_deref()) as u64
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleHandleA(module_name: u64) -> u64 {
    if module_name == 0 || module_name < 0x10000 {
        return CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    }
    let cstr = std::ffi::CStr::from_ptr(module_name as *const std::ffi::c_char);
    let s = cstr.to_string_lossy();
    ModuleRegistry::get_module_handle(Some(&s)) as u64
}

thread_local! {
    static THREAD_LAST_ERROR: std::cell::Cell<u32> = std::cell::Cell::new(0);
}

#[no_mangle]
pub unsafe extern "win64" fn SetLastError(dw_err_code: u32) {
    THREAD_LAST_ERROR.with(|cell| cell.set(dw_err_code));
    let teb = get_teb_ptr();
    if !teb.is_null() && (teb as usize) >= 0x10000 {
        (*teb).last_error_value = dw_err_code;
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetLastError() -> u32 {
    let teb = get_teb_ptr();
    if !teb.is_null() && (teb as usize) >= 0x10000 {
        (*teb).last_error_value
    } else {
        THREAD_LAST_ERROR.with(|cell| cell.get())
    }
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

/// Central Rine Logical Drive Registry
#[derive(Debug, Clone)]
pub struct LogicalDriveRegistry {
    drives_mask: u32,
}

impl LogicalDriveRegistry {
    pub const fn new() -> Self {
        // Bit 2 = Drive C: (1 << 2 = 0x04)
        Self {
            drives_mask: 1 << 2,
        }
    }

    pub fn get_drives_mask(&self) -> u32 {
        self.drives_mask
    }

    pub fn is_drive_available(&self, drive_letter: char) -> bool {
        let upper = drive_letter.to_ascii_uppercase();
        if ('A'..='Z').contains(&upper) {
            let bit = (upper as u32) - ('A' as u32);
            (self.drives_mask & (1 << bit)) != 0
        } else {
            false
        }
    }
}

static DRIVE_REGISTRY: std::sync::OnceLock<LogicalDriveRegistry> = std::sync::OnceLock::new();

pub fn get_drive_registry() -> &'static LogicalDriveRegistry {
    DRIVE_REGISTRY.get_or_init(LogicalDriveRegistry::new)
}

#[no_mangle]
pub unsafe extern "win64" fn GetLogicalDrives() -> u32 {
    let mask = get_drive_registry().get_drives_mask();
    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let mut drive_list = Vec::new();
        for i in 0..26 {
            if (mask & (1 << i)) != 0 {
                drive_list.push(format!("{}:", (b'A' + i) as char));
            }
        }
        println!(
            "[RINE FILESYSTEM] GetLogicalDrives -> mask=0x{:08X} drives={:?}",
            mask, drive_list
        );
    }
    mask
}

static PROCESS_CWD: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

pub fn set_process_current_directory(dir: &str) {
    if let Ok(mut guard) = PROCESS_CWD.write() {
        *guard = Some(dir.to_string());
    }
}

pub fn get_process_current_directory() -> String {
    if let Ok(guard) = PROCESS_CWD.read() {
        if let Some(ref s) = *guard {
            return s.clone();
        }
    }
    let host_cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());
    host_cwd.replace('/', "\\")
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentDirectoryW(
    n_buffer_length: u32,
    lp_buffer: *mut u16,
) -> u32 {
    let win_cwd = get_process_current_directory();
    let utf16: Vec<u16> = win_cwd.encode_utf16().collect();
    let len = utf16.len(); // number of WCHARs without terminating null

    // If buffer length is 0 or too small (needs len + 1 for null terminator)
    if n_buffer_length <= (len as u32) || lp_buffer.is_null() {
        return (len + 1) as u32;
    }

    // Buffer is sufficient
    std::ptr::copy_nonoverlapping(utf16.as_ptr(), lp_buffer, len);
    *lp_buffer.add(len) = 0;

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE FILESYSTEM] GetCurrentDirectoryW -> '{}' (len={})",
            win_cwd, len
        );
    }

    len as u32
}

/// Helper to resolve a Win32 path relative to the current working directory
pub fn resolve_win32_path(input_path: &str, current_cwd: &str) -> String {
    let norm = input_path.replace('/', "\\");
    let trimmed = norm.trim();

    if trimmed.is_empty() {
        return current_cwd.to_string();
    }

    // 1. If "." -> return current_cwd
    if trimmed == "." {
        return current_cwd.to_string();
    }

    // 2. Drive absolute: e.g. "C:\foo\bar"
    if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
        return normalize_win32_path_components(trimmed);
    }

    // 3. Root relative: starts with "\"
    if trimmed.starts_with('\\') {
        return normalize_win32_path_components(trimmed);
    }

    // 4. Normal relative: join with current_cwd
    let combined = if current_cwd.ends_with('\\') {
        format!("{}{}", current_cwd, trimmed)
    } else {
        format!("{}\\{}", current_cwd, trimmed)
    };
    normalize_win32_path_components(&combined)
}

/// Helper to normalize "." and ".." components in a Win32 backslash path
fn normalize_win32_path_components(path: &str) -> String {
    let mut prefix = "";
    let mut remainder = path;

    if remainder.len() >= 2 && remainder.as_bytes()[1] == b':' {
        prefix = &path[..2];
        remainder = &path[2..];
    }

    let is_root = remainder.starts_with('\\');
    let parts: Vec<&str> = remainder.split('\\').filter(|s| !s.is_empty()).collect();
    let mut stack: Vec<&str> = Vec::new();

    for part in parts {
        if part == "." {
            continue;
        } else if part == ".." {
            stack.pop();
        } else {
            stack.push(part);
        }
    }

    let mut result = String::new();
    if !prefix.is_empty() {
        result.push_str(prefix);
    }
    if is_root || prefix.is_empty() {
        result.push('\\');
    }
    result.push_str(&stack.join("\\"));

    if result.is_empty() {
        "\\".to_string()
    } else {
        result
    }
}

pub fn initialize_process_filesystem(exe_path: &str, configured_cwd: Option<&str>) {
    set_target_exe_path(exe_path);

    let initial_dir = if let Some(dir) = configured_cwd {
        dir.to_string()
    } else if let Some(parent) = std::path::Path::new(exe_path).parent() {
        if parent.as_os_str().is_empty() {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        } else {
            parent.to_string_lossy().to_string()
        }
    } else {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    };

    let guest_win_dir = initial_dir.replace('/', "\\");
    set_process_current_directory(&guest_win_dir);
    println!(
        "[RINE FILESYSTEM] Initialized Guest Process CWD -> '{}'",
        guest_win_dir
    );
}

/// Helper to convert a Win32 guest path to a host POSIX path
pub fn win32_to_host_path(win_path: &str) -> String {
    let current_cwd = get_process_current_directory();
    let resolved_guest = resolve_win32_path(win_path, &current_cwd);
    let mut s = resolved_guest.as_str();
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        s = &s[2..];
    }
    let host = s.replace('\\', "/");
    if host.is_empty() {
        return "/".to_string();
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/fizzy".to_string());
    if host.to_lowercase().starts_with("/users/") {
        let sub = &host[7..]; // strip "/users/" -> "<username>/AppData/Roaming/..."
        if let Some(slash) = sub.find('/') {
            let after_user = &sub[slash..]; // "/AppData/Roaming/..."
            let mapped = format!("{}{}", home, after_user);
            if let Some(parent) = std::path::Path::new(&mapped).parent() {
                std::fs::create_dir_all(parent).ok();
            }
            return mapped;
        }
    }

    host
}

#[no_mangle]
pub unsafe extern "win64" fn SetCurrentDirectoryW(lp_path_name: *const u16) -> i32 {
    if lp_path_name.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0; // FALSE
    }

    let raw_str = match read_utf16_ptr(lp_path_name) {
        Some(s) => s,
        None => {
            SetLastError(87);
            return 0;
        }
    };

    if raw_str.is_empty() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0;
    }

    let current_cwd = get_process_current_directory();
    let resolved_guest = resolve_win32_path(&raw_str, &current_cwd);
    let host_path = win32_to_host_path(&resolved_guest);

    // Validate host directory existence and type via stat
    let c_path = match std::ffi::CString::new(host_path.clone()) {
        Ok(c) => c,
        Err(_) => {
            SetLastError(87);
            return 0;
        }
    };

    let mut st: libc::stat = std::mem::zeroed();
    let res = libc::stat(c_path.as_ptr(), &mut st);
    if res != 0 {
        SetLastError(3); // ERROR_PATH_NOT_FOUND
        return 0;
    }

    if (st.st_mode & libc::S_IFMT) != libc::S_IFDIR {
        SetLastError(267); // ERROR_DIRECTORY (The directory name is invalid)
        return 0;
    }

    // Success: update process CWD
    set_process_current_directory(&resolved_guest);

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE FILESYSTEM] SetCurrentDirectoryW input='{}' resolved_guest='{}' host='{}' -> TRUE",
            raw_str, resolved_guest, host_path
        );
    }

    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDirectoryW(
    lp_path_name: *const u16,
    lp_security_attributes: *const std::ffi::c_void, // [SECURITY ATTRIBUTES NOT EXERCISED]
) -> i32 {
    let _ = lp_security_attributes;

    if lp_path_name.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0; // FALSE
    }

    let raw_str = match read_utf16_ptr(lp_path_name) {
        Some(s) => s,
        None => {
            SetLastError(87);
            return 0;
        }
    };

    if raw_str.is_empty() {
        SetLastError(3); // ERROR_PATH_NOT_FOUND
        return 0;
    }

    let host_path = win32_to_host_path(&raw_str);
    let target_path = std::path::Path::new(&host_path);

    // Root directory always exists
    if host_path == "/" {
        SetLastError(183); // ERROR_ALREADY_EXISTS
        return 0;
    }

    if target_path.exists() {
        SetLastError(183); // ERROR_ALREADY_EXISTS
        return 0;
    }

    // Check parent path exists and is a directory (non-recursive CreateDirectoryW)
    if let Some(parent) = target_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            SetLastError(3); // ERROR_PATH_NOT_FOUND
            return 0;
        }
    }

    match std::fs::create_dir(target_path) {
        Ok(()) => 1,
        Err(e) => {
            let win_err = match e.kind() {
                std::io::ErrorKind::AlreadyExists => 183, // ERROR_ALREADY_EXISTS
                std::io::ErrorKind::NotFound => 3,        // ERROR_PATH_NOT_FOUND
                std::io::ErrorKind::PermissionDenied => 5, // ERROR_ACCESS_DENIED
                _ => 82,                                  // ERROR_CANNOT_MAKE
            };
            SetLastError(win_err);
            0 // FALSE
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDirectoryA(
    lp_path_name: *const i8,
    lp_security_attributes: *const std::ffi::c_void,
) -> i32 {
    if lp_path_name.is_null() {
        SetLastError(87);
        return 0;
    }
    let c_str = std::ffi::CStr::from_ptr(lp_path_name);
    let s = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => {
            SetLastError(87);
            return 0;
        }
    };
    let utf16: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    CreateDirectoryW(utf16.as_ptr(), lp_security_attributes)
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
    // Determine open flags
    let write_mode = (dw_desired_access & GENERIC_WRITE) != 0;
    if write_mode {
        if let Some(parent) = std::path::Path::new(&linux_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
    }
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
        return INVALID_HANDLE_VALUE;
    }

    // Register fd in handle table
    register_fd_handle(fd as u64);
    fd as *mut std::ffi::c_void
}

/// Converts a Windows path to a Linux path best-effort
fn win_path_to_linux(win_path: &str) -> String {
    win32_to_host_path(win_path)
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

static CONSOLE_TEXT_ATTRIBUTES: std::sync::atomic::AtomicU16 =
    std::sync::atomic::AtomicU16::new(0x0007);

pub fn get_console_text_attributes() -> u16 {
    CONSOLE_TEXT_ATTRIBUTES.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn set_console_text_attributes(attr: u16) {
    CONSOLE_TEXT_ATTRIBUTES.store(attr, std::sync::atomic::Ordering::SeqCst);
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmallRect {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsoleScreenBufferInfo {
    pub dw_size: Coord,
    pub dw_cursor_position: Coord,
    pub w_attributes: u16,
    pub sr_window: SmallRect,
    pub dw_maximum_window_size: Coord,
}

#[no_mangle]
pub unsafe extern "win64" fn GetConsoleScreenBufferInfo(
    h_console_output: u64,
    lp_console_screen_buffer_info: *mut ConsoleScreenBufferInfo,
) -> i32 {
    if lp_console_screen_buffer_info.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0; // FALSE
    }

    // Validate handle: 0x8 (stdout) or 0xC (stderr) or 0x4 (stdin)
    if h_console_output != 0x8 && h_console_output != 0xC && h_console_output != 0x4 {
        SetLastError(6); // ERROR_INVALID_HANDLE
        return 0; // FALSE
    }

    let current_attr = get_console_text_attributes();
    *lp_console_screen_buffer_info = ConsoleScreenBufferInfo {
        dw_size: Coord { x: 80, y: 25 },
        dw_cursor_position: Coord { x: 0, y: 0 },
        w_attributes: current_attr,
        sr_window: SmallRect {
            left: 0,
            top: 0,
            right: 79,
            bottom: 24,
        },
        dw_maximum_window_size: Coord { x: 80, y: 25 },
    };

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE CONSOLE] GetConsoleScreenBufferInfo handle=0x{:X} size=(80,25) cursor=(0,0) attributes=0x{:04X} window=[0,0,79,24] max=(80,25) -> TRUE",
            h_console_output, current_attr
        );
    }

    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetConsoleTextAttribute(
    h_console_output: u64,
    w_attributes: u16,
) -> i32 {
    // Validate handle: 0x8 (stdout) or 0xC (stderr) or 0x4 (stdin)
    if h_console_output != 0x8 && h_console_output != 0xC && h_console_output != 0x4 {
        SetLastError(6); // ERROR_INVALID_HANDLE
        return 0; // FALSE
    }

    set_console_text_attributes(w_attributes);

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE CONSOLE] SetConsoleTextAttribute handle=0x{:X} attributes=0x{:04X} -> TRUE",
            h_console_output, w_attributes
        );
    }

    1 // TRUE
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

#[unsafe(naked)]
pub unsafe extern "win64" fn SetConsoleOutputCP(_w_code_page_id: u32) -> i32 {
    std::arch::naked_asm!("mov eax, 1", "ret");
}

#[unsafe(naked)]
pub unsafe extern "win64" fn AttachConsole(_dw_process_id: u32) -> i32 {
    std::arch::naked_asm!("mov eax, 1", "ret");
}

#[unsafe(naked)]
pub unsafe extern "win64" fn SetConsoleCP(_w_code_page_id: u32) -> i32 {
    std::arch::naked_asm!("mov eax, 1", "ret");
}

pub const PROCESSOR_ARCHITECTURE_INTEL: u16 = 0;
pub const PROCESSOR_ARCHITECTURE_ARM: u16 = 5;
pub const PROCESSOR_ARCHITECTURE_IA64: u16 = 6;
pub const PROCESSOR_ARCHITECTURE_AMD64: u16 = 9;
pub const PROCESSOR_ARCHITECTURE_ARM64: u16 = 12;
pub const PROCESSOR_ARCHITECTURE_UNKNOWN: u16 = 0xFFFF;

pub const PROCESSOR_INTEL_386: u32 = 386;
pub const PROCESSOR_INTEL_486: u32 = 486;
pub const PROCESSOR_INTEL_PENTIUM: u32 = 586;
pub const PROCESSOR_INTEL_IA64: u32 = 2200;
pub const PROCESSOR_AMD_X8664: u32 = 8664;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemInfo {
    pub w_processor_architecture: u16,
    pub w_reserved: u16,
    pub dw_page_size: u32,
    pub lp_minimum_application_address: u64,
    pub lp_maximum_application_address: u64,
    pub dw_active_processor_mask: u64,
    pub dw_number_of_processors: u32,
    pub dw_processor_type: u32,
    pub dw_allocation_granularity: u32,
    pub w_processor_level: u16,
    pub w_processor_revision: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestSystemInfo {
    pub architecture: u16,
    pub page_size: u32,
    pub min_app_address: u64,
    pub max_app_address: u64,
    pub active_processor_mask: u64,
    pub processor_count: u32,
    pub processor_type: u32,
    pub allocation_granularity: u32,
    pub processor_level: u16,
    pub processor_revision: u16,
}

impl GuestSystemInfo {
    pub fn current() -> Self {
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
        let active_mask = if cpu_count >= 64 {
            !0u64
        } else {
            (1u64 << cpu_count) - 1
        };

        GuestSystemInfo {
            architecture: PROCESSOR_ARCHITECTURE_AMD64,
            page_size: crate::sys::PAGE_SIZE as u32, // 4096
            min_app_address: 0x0000000000010000,     // 64 KB
            max_app_address: 0x00007FFFFFFEFFFF,     // Win64 user limit
            active_processor_mask: active_mask,
            processor_count: cpu_count,
            processor_type: PROCESSOR_AMD_X8664,
            allocation_granularity: 65536, // 64 KB [ALPHA VIRTUAL MEMORY GRANULARITY SEMANTICS]
            processor_level: 6,
            processor_revision: 0,
        }
    }

    pub fn to_system_info(&self) -> SystemInfo {
        SystemInfo {
            w_processor_architecture: self.architecture,
            w_reserved: 0,
            dw_page_size: self.page_size,
            lp_minimum_application_address: self.min_app_address,
            lp_maximum_application_address: self.max_app_address,
            dw_active_processor_mask: self.active_processor_mask,
            dw_number_of_processors: self.processor_count,
            dw_processor_type: self.processor_type,
            dw_allocation_granularity: self.allocation_granularity,
            w_processor_level: self.processor_level,
            w_processor_revision: self.processor_revision,
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetNativeSystemInfo(lp_system_info: *mut SystemInfo) {
    if lp_system_info.is_null() {
        return;
    }
    let info = GuestSystemInfo::current().to_system_info();
    *lp_system_info = info;

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE SYSTEM] GetNativeSystemInfo: arch=AMD64(9), CPUs={}, mask=0x{:X}, pageSize={}, allocGranularity={}",
            info.dw_number_of_processors,
            info.dw_active_processor_mask,
            info.dw_page_size,
            info.dw_allocation_granularity
        );
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemInfo(lp_system_info: *mut SystemInfo) {
    GetNativeSystemInfo(lp_system_info);
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemTime {
    pub w_year: u16,
    pub w_month: u16,
    pub w_day_of_week: u16,
    pub w_day: u16,
    pub w_hour: u16,
    pub w_minute: u16,
    pub w_second: u16,
    pub w_milliseconds: u16,
}

pub struct WallClock;

impl WallClock {
    pub fn now_local() -> SystemTime {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
            let mut tm = std::mem::zeroed::<libc::tm>();
            // [ALPHA HOST TIMEZONE SEMANTICS: thread-safe reentrant localtime_r]
            libc::localtime_r(&ts.tv_sec, &mut tm);

            SystemTime {
                w_year: (tm.tm_year + 1900) as u16,
                w_month: (tm.tm_mon + 1) as u16,
                w_day_of_week: tm.tm_wday as u16, // 0 = Sunday .. 6 = Saturday
                w_day: tm.tm_mday as u16,
                w_hour: tm.tm_hour as u16,
                w_minute: tm.tm_min as u16,
                w_second: tm.tm_sec as u16,
                w_milliseconds: ((ts.tv_nsec / 1_000_000) % 1000) as u16,
            }
        }
    }

    pub fn now_utc() -> SystemTime {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
            let mut tm = std::mem::zeroed::<libc::tm>();
            libc::gmtime_r(&ts.tv_sec, &mut tm);

            SystemTime {
                w_year: (tm.tm_year + 1900) as u16,
                w_month: (tm.tm_mon + 1) as u16,
                w_day_of_week: tm.tm_wday as u16,
                w_day: tm.tm_mday as u16,
                w_hour: tm.tm_hour as u16,
                w_minute: tm.tm_min as u16,
                w_second: tm.tm_sec as u16,
                w_milliseconds: ((ts.tv_nsec / 1_000_000) % 1000) as u16,
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetLocalTime(lp_system_time: *mut SystemTime) {
    if lp_system_time.is_null() {
        return;
    }
    *lp_system_time = WallClock::now_local();
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemTime(lp_system_time: *mut SystemTime) {
    if lp_system_time.is_null() {
        return;
    }
    *lp_system_time = WallClock::now_utc();
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemTimeAsFileTime(_lp_system_time_as_file_time: *mut u64) {
    // Standard FileTime epoch is Jan 1, 1601 (100-ns intervals)
    if !_lp_system_time_as_file_time.is_null() {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
        // Seconds between 1601-01-01 and 1970-01-01: 11644473600
        let intervals = ((ts.tv_sec as u64 + 11644473600) * 10_000_000) + (ts.tv_nsec as u64 / 100);
        *_lp_system_time_as_file_time = intervals;
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SystemTimeToFileTime(
    lp_system_time: *const SystemTime,
    lp_file_time: *mut u64,
) -> i32 {
    if lp_system_time.is_null() || lp_file_time.is_null() {
        return 0;
    }
    let st = &*lp_system_time;
    let mut tm: libc::tm = std::mem::zeroed();
    tm.tm_year = (st.w_year as libc::c_int) - 1900;
    tm.tm_mon = (st.w_month as libc::c_int) - 1;
    tm.tm_mday = st.w_day as libc::c_int;
    tm.tm_hour = st.w_hour as libc::c_int;
    tm.tm_min = st.w_minute as libc::c_int;
    tm.tm_sec = st.w_second as libc::c_int;
    tm.tm_isdst = -1;
    let t = libc::timegm(&mut tm);
    if t < 0 {
        return 0;
    }
    let intervals = ((t as u64 + 11644473600) * 10_000_000) + (st.w_milliseconds as u64 * 10_000);
    *lp_file_time = intervals;
    1
}

pub const TIME_ZONE_ID_UNKNOWN: u32 = 0;
pub const TIME_ZONE_ID_STANDARD: u32 = 1;
pub const TIME_ZONE_ID_DAYLIGHT: u32 = 2;
pub const TIME_ZONE_ID_INVALID: u32 = 0xFFFFFFFF;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeZoneInformation {
    pub bias: i32,
    pub standard_name: [u16; 32],
    pub standard_date: SystemTime,
    pub standard_bias: i32,
    pub daylight_name: [u16; 32],
    pub daylight_date: SystemTime,
    pub daylight_bias: i32,
}

impl WallClock {
    pub fn get_timezone_info() -> (u32, TimeZoneInformation) {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
            let mut tm = std::mem::zeroed::<libc::tm>();
            libc::localtime_r(&ts.tv_sec, &mut tm);

            // Windows Bias: UTC = Local + Bias => Bias = -(tm_gmtoff / 60)
            let gmtoff_minutes = (tm.tm_gmtoff / 60) as i32;
            let bias = -gmtoff_minutes;

            let mut std_name = [0u16; 32];
            let mut dlt_name = [0u16; 32];

            if !tm.tm_zone.is_null() {
                if let Ok(cstr) = std::ffi::CStr::from_ptr(tm.tm_zone).to_str() {
                    for (i, c) in cstr.encode_utf16().take(31).enumerate() {
                        std_name[i] = c;
                        dlt_name[i] = c;
                    }
                }
            }

            let zero_time = SystemTime {
                w_year: 0,
                w_month: 0,
                w_day_of_week: 0,
                w_day: 0,
                w_hour: 0,
                w_minute: 0,
                w_second: 0,
                w_milliseconds: 0,
            };

            // [ALPHA TIMEZONE TRANSITION SEMANTICS: no transition dates configured]
            let (ret_id, std_bias, dlt_bias) = if tm.tm_isdst > 0 {
                (TIME_ZONE_ID_DAYLIGHT, 0, -60)
            } else if tm.tm_isdst == 0 {
                (TIME_ZONE_ID_STANDARD, 0, -60)
            } else {
                (TIME_ZONE_ID_UNKNOWN, 0, 0)
            };

            let tzi = TimeZoneInformation {
                bias,
                standard_name: std_name,
                standard_date: zero_time,
                standard_bias: std_bias,
                daylight_name: dlt_name,
                daylight_date: zero_time,
                daylight_bias: dlt_bias,
            };

            (ret_id, tzi)
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetTimeZoneInformation(
    lp_time_zone_information: *mut TimeZoneInformation,
) -> u32 {
    if lp_time_zone_information.is_null() {
        return TIME_ZONE_ID_INVALID;
    }
    let (id, info) = WallClock::get_timezone_info();
    *lp_time_zone_information = info;
    id
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestLocale {
    pub lang_id: u16,
    pub lcid: u32,
    pub locale_name: &'static str,
}

impl GuestLocale {
    pub fn from_env_str(host_lang: &str) -> GuestLocale {
        let l = host_lang.to_lowercase();
        if l.starts_with("ru") {
            GuestLocale {
                lang_id: 0x0419, // LANG_RUSSIAN (0x19) | (SUBLANG_RUSSIAN_RUSSIA (0x01) << 10) = 0x0419
                lcid: 0x00000419,
                locale_name: "ru-RU",
            }
        } else if l.starts_with("de") {
            GuestLocale {
                lang_id: 0x0407, // LANG_GERMAN (0x07) | (SUBLANG_GERMAN (0x01) << 10) = 0x0407
                lcid: 0x00000407,
                locale_name: "de-DE",
            }
        } else if l.starts_with("fr") {
            GuestLocale {
                lang_id: 0x040c, // LANG_FRENCH (0x0c) | (SUBLANG_FRENCH (0x01) << 10) = 0x040c
                lcid: 0x0000040c,
                locale_name: "fr-FR",
            }
        } else if l.starts_with("es") {
            GuestLocale {
                lang_id: 0x0c0a, // LANG_SPANISH (0x0a) | (SUBLANG_SPANISH_MODERN (0x03) << 10) = 0x0c0a
                lcid: 0x00000c0a,
                locale_name: "es-ES",
            }
        } else if l.starts_with("zh") {
            GuestLocale {
                lang_id: 0x0804, // LANG_CHINESE (0x04) | (SUBLANG_CHINESE_SIMPLIFIED (0x02) << 10) = 0x0804
                lcid: 0x00000804,
                locale_name: "zh-CN",
            }
        } else if l.starts_with("ja") {
            GuestLocale {
                lang_id: 0x0411, // LANG_JAPANESE (0x11) | (SUBLANG_JAPANESE_JAPAN (0x01) << 10) = 0x0411
                lcid: 0x00000411,
                locale_name: "ja-JP",
            }
        } else {
            // [ALPHA NLS / UI LANGUAGE SEMANTICS: default en-US fallback]
            GuestLocale {
                lang_id: 0x0409, // LANG_ENGLISH (0x09) | (SUBLANG_ENGLISH_US (0x01) << 10) = 0x0409
                lcid: 0x00000409,
                locale_name: "en-US",
            }
        }
    }

    pub fn current() -> GuestLocale {
        static GUEST_LOCALE: std::sync::OnceLock<GuestLocale> = std::sync::OnceLock::new();
        *GUEST_LOCALE.get_or_init(|| {
            // [ALPHA HOST LOCALE INHERITANCE: snapshot host locale at startup]
            let host_lang = std::env::var("LC_ALL")
                .or_else(|_| std::env::var("LC_MESSAGES"))
                .or_else(|_| std::env::var("LANG"))
                .unwrap_or_default();
            GuestLocale::from_env_str(&host_lang)
        })
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserDefaultUILanguage() -> u16 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetUserDefaultUILanguage -> 0x{:04X} ({})",
        loc.lang_id, loc.locale_name
    );
    loc.lang_id
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDefaultUILanguage() -> u16 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetSystemDefaultUILanguage -> 0x{:04X} ({})",
        loc.lang_id, loc.locale_name
    );
    loc.lang_id
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserDefaultLangID() -> u16 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetUserDefaultLangID -> 0x{:04X} ({})",
        loc.lang_id, loc.locale_name
    );
    loc.lang_id
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDefaultLangID() -> u16 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetSystemDefaultLangID -> 0x{:04X} ({})",
        loc.lang_id, loc.locale_name
    );
    loc.lang_id
}

#[no_mangle]
pub unsafe extern "win64" fn GetLocaleInfoEx(
    _lp_locale_name: *const u16,
    _lc_type: u32,
    lp_lc_data: *mut u16,
    cch_data: i32,
) -> i32 {
    let loc = GuestLocale::current();
    let s: Vec<u16> = format!("{}\0", loc.locale_name).encode_utf16().collect();
    if cch_data <= 0 || lp_lc_data.is_null() {
        return s.len() as i32;
    }
    let copy_len = s.len().min(cch_data as usize);
    std::ptr::copy_nonoverlapping(s.as_ptr(), lp_lc_data, copy_len);
    copy_len as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetLocaleInfoW(
    _lcid: u32,
    _lc_type: u32,
    lp_lc_data: *mut u16,
    cch_data: i32,
) -> i32 {
    GetLocaleInfoEx(std::ptr::null(), _lc_type, lp_lc_data, cch_data)
}

#[no_mangle]
pub unsafe extern "win64" fn GetLocaleInfoA(
    _lcid: u32,
    _lc_type: u32,
    lp_lc_data: *mut libc::c_char,
    cch_data: i32,
) -> i32 {
    let loc = GuestLocale::current();
    let s = format!("{}\0", loc.locale_name);
    if cch_data <= 0 || lp_lc_data.is_null() {
        return s.len() as i32;
    }
    let copy_len = s.len().min(cch_data as usize);
    std::ptr::copy_nonoverlapping(s.as_ptr() as *const libc::c_char, lp_lc_data, copy_len);
    copy_len as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserDefaultLCID() -> u32 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetUserDefaultLCID -> 0x{:08X} ({})",
        loc.lcid, loc.locale_name
    );
    loc.lcid
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDefaultLCID() -> u32 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetSystemDefaultLCID -> 0x{:08X} ({})",
        loc.lcid, loc.locale_name
    );
    loc.lcid
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserDefaultLocaleName(
    lp_locale_name: *mut u16,
    cch_locale_name: i32,
) -> i32 {
    let loc = GuestLocale::current();
    let utf16: Vec<u16> = loc
        .locale_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    if lp_locale_name.is_null() || cch_locale_name <= 0 {
        return utf16.len() as i32;
    }
    if (cch_locale_name as usize) < utf16.len() {
        SetLastError(122); // ERROR_INSUFFICIENT_BUFFER
        return 0;
    }
    std::ptr::copy_nonoverlapping(utf16.as_ptr(), lp_locale_name, utf16.len());
    println!(
        "[KERNEL32] GetUserDefaultLocaleName -> '{}'",
        loc.locale_name
    );
    utf16.len() as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDefaultLocaleName(
    lp_locale_name: *mut u16,
    cch_locale_name: i32,
) -> i32 {
    GetUserDefaultLocaleName(lp_locale_name, cch_locale_name)
}

pub const ERROR_ENVVAR_NOT_FOUND: u32 = 203;

pub struct GuestEnvironment;

static GUEST_ENV: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<String, String>>,
> = std::sync::OnceLock::new();

fn get_guest_env() -> &'static std::sync::RwLock<std::collections::HashMap<String, String>> {
    GUEST_ENV.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        // [ALPHA HOST ENVIRONMENT INHERITANCE: snapshot host environment at startup]
        for (k, v) in std::env::vars() {
            map.insert(k.to_uppercase(), v);
        }
        std::sync::RwLock::new(map)
    })
}

impl GuestEnvironment {
    pub fn get(name: &str) -> Option<String> {
        let env = get_guest_env().read().unwrap();
        env.get(&name.to_uppercase()).cloned()
    }

    pub fn set(name: &str, value: Option<&str>) {
        let mut env = get_guest_env().write().unwrap();
        if let Some(v) = value {
            env.insert(name.to_uppercase(), v.to_string());
        } else {
            env.remove(&name.to_uppercase());
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetEnvironmentVariableW(
    lp_name: *const u16,
    lp_buffer: *mut u16,
    n_size: u32,
) -> u32 {
    if lp_name.is_null() {
        SetLastError(ERROR_ENVVAR_NOT_FOUND);
        return 0;
    }

    let mut name_utf16 = Vec::new();
    let mut ptr = lp_name;
    while *ptr != 0 {
        name_utf16.push(*ptr);
        ptr = ptr.add(1);
        if name_utf16.len() > 32768 {
            break;
        }
    }

    let name = match String::from_utf16(&name_utf16) {
        Ok(s) => s,
        Err(_) => {
            SetLastError(ERROR_ENVVAR_NOT_FOUND);
            return 0;
        }
    };

    if let Some(val) = GuestEnvironment::get(&name) {
        let val_utf16: Vec<u16> = val.encode_utf16().collect();
        let val_len = val_utf16.len() as u32;

        if n_size > val_len {
            if !lp_buffer.is_null() {
                for (i, &c) in val_utf16.iter().enumerate() {
                    *lp_buffer.add(i) = c;
                }
                *lp_buffer.add(val_len as usize) = 0;
            }
            val_len
        } else {
            val_len + 1
        }
    } else {
        SetLastError(ERROR_ENVVAR_NOT_FOUND);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetEnvironmentVariableA(
    lp_name: *const libc::c_char,
    lp_buffer: *mut libc::c_char,
    n_size: u32,
) -> u32 {
    if lp_name.is_null() {
        SetLastError(ERROR_ENVVAR_NOT_FOUND);
        return 0;
    }

    let name = match std::ffi::CStr::from_ptr(lp_name).to_str() {
        Ok(s) => s,
        Err(_) => {
            SetLastError(ERROR_ENVVAR_NOT_FOUND);
            return 0;
        }
    };

    if let Some(val) = GuestEnvironment::get(name) {
        let bytes = val.as_bytes();
        let val_len = bytes.len() as u32;

        if n_size > val_len {
            if !lp_buffer.is_null() {
                for (i, &b) in bytes.iter().enumerate() {
                    *lp_buffer.add(i) = b as libc::c_char;
                }
                *lp_buffer.add(val_len as usize) = 0;
            }
            val_len
        } else {
            val_len + 1
        }
    } else {
        SetLastError(ERROR_ENVVAR_NOT_FOUND);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetEnvironmentVariableW(
    lp_name: *const u16,
    lp_value: *const u16,
) -> i32 {
    if lp_name.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0;
    }
    let mut name_utf16 = Vec::new();
    let mut ptr = lp_name;
    while *ptr != 0 {
        name_utf16.push(*ptr);
        ptr = ptr.add(1);
    }
    let name = match String::from_utf16(&name_utf16) {
        Ok(s) => s,
        Err(_) => {
            SetLastError(87);
            return 0;
        }
    };

    let value = if lp_value.is_null() {
        None
    } else {
        let mut val_utf16 = Vec::new();
        let mut vptr = lp_value;
        while *vptr != 0 {
            val_utf16.push(*vptr);
            vptr = vptr.add(1);
        }
        match String::from_utf16(&val_utf16) {
            Ok(s) => Some(s),
            Err(_) => {
                SetLastError(87);
                return 0;
            }
        }
    };

    GuestEnvironment::set(&name, value.as_deref());
    1
}

pub const POWER_REQUEST_CONTEXT_VERSION: u32 = 0;
pub const POWER_REQUEST_CONTEXT_SIMPLE_STRING: u32 = 1;
pub const POWER_REQUEST_CONTEXT_DETAILED_STRING: u32 = 2;

pub const POWER_REQUEST_DISPLAY_REQUIRED: u32 = 0;
pub const POWER_REQUEST_SYSTEM_REQUIRED: u32 = 1;
pub const POWER_REQUEST_AWAY_MODE_REQUIRED: u32 = 2;
pub const POWER_REQUEST_EXECUTION_REQUIRED: u32 = 3;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReasonContextDetailed {
    pub localized_reason_module: u64,
    pub localized_reason_id: u32,
    pub reason_string_count: u32,
    pub reason_strings: *mut *mut u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ReasonContextUnion {
    pub detailed: ReasonContextDetailed,
    pub simple_reason_string: *const u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ReasonContext {
    pub version: u32,
    pub flags: u32,
    pub reason: ReasonContextUnion,
}

#[derive(Debug, Clone)]
pub struct PowerRequestObject {
    pub reason: String,
    pub display_required: u32,
    pub system_required: u32,
    pub away_mode_required: u32,
    pub execution_required: u32,
}

static POWER_REQUEST_TABLE: std::sync::Mutex<
    Option<std::collections::HashMap<u64, std::sync::Arc<std::sync::Mutex<PowerRequestObject>>>>,
> = std::sync::Mutex::new(None);

#[no_mangle]
pub unsafe extern "win64" fn PowerCreateRequest(context: *const ReasonContext) -> u64 {
    if context.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0xFFFFFFFFFFFFFFFFu64; // INVALID_HANDLE_VALUE
    }

    let version = (*context).version;
    let flags = (*context).flags;

    if version != POWER_REQUEST_CONTEXT_VERSION {
        SetLastError(87);
        return 0xFFFFFFFFFFFFFFFFu64;
    }

    let reason_str = if flags & POWER_REQUEST_CONTEXT_SIMPLE_STRING != 0 {
        let ptr = (*context).reason.simple_reason_string;
        if !ptr.is_null() {
            let mut u16_vec = Vec::new();
            let mut p = ptr;
            while *p != 0 {
                u16_vec.push(*p);
                p = p.add(1);
                if u16_vec.len() > 1024 {
                    break;
                }
            }
            String::from_utf16_lossy(&u16_vec)
        } else {
            String::new()
        }
    } else {
        String::from("[DETAILED_REASON]")
    };

    let obj = std::sync::Arc::new(std::sync::Mutex::new(PowerRequestObject {
        reason: reason_str.clone(),
        display_required: 0,
        system_required: 0,
        away_mode_required: 0,
        execution_required: 0,
    }));

    let handle = next_handle();
    let mut g = POWER_REQUEST_TABLE.lock().unwrap();
    g.get_or_insert_with(std::collections::HashMap::new)
        .insert(handle, obj);

    println!(
        "[RINE POWER] PowerCreateRequest: allocated handle=0x{:X} reason='{}'",
        handle, reason_str
    );
    handle
}

#[no_mangle]
pub unsafe extern "win64" fn PowerSetRequest(power_request: u64, request_type: u32) -> i32 {
    let g = POWER_REQUEST_TABLE.lock().unwrap();
    if let Some(table) = g.as_ref() {
        if let Some(arc_obj) = table.get(&power_request) {
            let mut obj = arc_obj.lock().unwrap();
            match request_type {
                POWER_REQUEST_DISPLAY_REQUIRED => {
                    obj.display_required = obj.display_required.saturating_add(1);
                }
                POWER_REQUEST_SYSTEM_REQUIRED => {
                    obj.system_required = obj.system_required.saturating_add(1);
                }
                POWER_REQUEST_AWAY_MODE_REQUIRED => {
                    obj.away_mode_required = obj.away_mode_required.saturating_add(1);
                }
                POWER_REQUEST_EXECUTION_REQUIRED => {
                    obj.execution_required = obj.execution_required.saturating_add(1);
                }
                _ => {
                    SetLastError(87);
                    return 0;
                }
            }
            // [ALPHA POWER MANAGEMENT BACKEND: host power inhibition not implemented]
            return 1;
        }
    }
    SetLastError(6); // ERROR_INVALID_HANDLE
    0
}

#[no_mangle]
pub unsafe extern "win64" fn PowerClearRequest(power_request: u64, request_type: u32) -> i32 {
    let g = POWER_REQUEST_TABLE.lock().unwrap();
    if let Some(table) = g.as_ref() {
        if let Some(arc_obj) = table.get(&power_request) {
            let mut obj = arc_obj.lock().unwrap();
            match request_type {
                POWER_REQUEST_DISPLAY_REQUIRED => {
                    obj.display_required = obj.display_required.saturating_sub(1);
                }
                POWER_REQUEST_SYSTEM_REQUIRED => {
                    obj.system_required = obj.system_required.saturating_sub(1);
                }
                POWER_REQUEST_AWAY_MODE_REQUIRED => {
                    obj.away_mode_required = obj.away_mode_required.saturating_sub(1);
                }
                POWER_REQUEST_EXECUTION_REQUIRED => {
                    obj.execution_required = obj.execution_required.saturating_sub(1);
                }
                _ => {
                    SetLastError(87);
                    return 0;
                }
            }
            return 1;
        }
    }
    SetLastError(6); // ERROR_INVALID_HANDLE
    0
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

pub struct ThreadEntry {
    pub thread_id: u32,
    pub exit_code: std::sync::atomic::AtomicU32,
    pub is_terminated: std::sync::atomic::AtomicBool,
    pub sync: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    pub suspended_sync: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
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
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering as AO};
    static THREAD_COUNTER: AtomicU32 = AtomicU32::new(0x200);

    let thread_id = THREAD_COUNTER.fetch_add(1, AO::Relaxed);
    if !lp_thread_id.is_null() {
        *lp_thread_id = thread_id;
    }

    let is_suspended = (dw_creation_flags & 0x4) != 0;

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

    let sync = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let suspended_sync = std::sync::Arc::new((
        std::sync::Mutex::new(is_suspended),
        std::sync::Condvar::new(),
    ));

    let thread_entry = std::sync::Arc::new(ThreadEntry {
        thread_id,
        exit_code: AtomicU32::new(259), // STILL_ACTIVE
        is_terminated: AtomicBool::new(false),
        sync,
        suspended_sync,
    });

    {
        let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
        let table = guard.get_or_insert_with(HashMap::new);
        table.insert(handle, thread_entry.clone());
    }

    let image_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    let peb_address = CURRENT_PEB_ADDRESS.load(std::sync::atomic::Ordering::SeqCst);
    let stack_size = if dw_stack_size == 0 {
        2 * 1024 * 1024
    } else {
        dw_stack_size
    };

    let thread_entry_worker = thread_entry.clone();

    std::thread::Builder::new()
        .stack_size(stack_size)
        .spawn(move || {
            let pid = unsafe { libc::getpid() } as u64;
            let tid = thread_id as u64;

            // Allocate full TEB page (4096 bytes)
            let teb_size = 4096usize;
            let teb_mem = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    teb_size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            assert!(
                teb_mem != libc::MAP_FAILED && !teb_mem.is_null(),
                "Failed to allocate worker TEB memory"
            );
            unsafe {
                std::ptr::write_bytes(teb_mem as *mut u8, 0, teb_size);
            }

            let teb_ptr = teb_mem as u64;
            let tls_array_addr = teb_ptr + 0x200; // TLS slots at offset 0x200

            let teb_struct = crate::pe::teb::Teb64 {
                tib: crate::pe::teb::NtTib64 {
                    exception_list: 0,
                    stack_base: 0,
                    stack_limit: 0,
                    sub_system_tib: 0,
                    fiber_data: 0,
                    arbitrary_user_pointer: 0,
                    self_ptr: teb_ptr,
                },
                environment_pointer: 0,
                client_id: crate::pe::teb::ClientId64 {
                    unique_process: pid,
                    unique_thread: tid,
                },
                active_rpc_handle: 0,
                thread_local_storage_pointer: tls_array_addr,
                peb: peb_address,
                last_error_value: 0,
                count_of_owned_critical_sections: 0,
                cs_client_thread: 0,
                win32_thread_info: 0,
            };

            unsafe {
                std::ptr::write(teb_mem as *mut crate::pe::teb::Teb64, teb_struct);
            }

            // Set stack base and stack limit in TEB
            let current_rsp: u64;
            unsafe {
                std::arch::asm!("mov {}, rsp", out(reg) current_rsp);
            }
            unsafe {
                let p_teb = teb_mem as *mut crate::pe::teb::Teb64;
                (*p_teb).tib.stack_base = current_rsp + (stack_size as u64);
                (*p_teb).tib.stack_limit = current_rsp.saturating_sub(stack_size as u64);
            }

            // Set GS base for this worker thread
            let _ = crate::sys::trampoline::set_thread_teb_gs_base(teb_ptr);

            // Read back GS base to verify
            let mut gs_base: u64 = 0;
            unsafe {
                libc::syscall(libc::SYS_arch_prctl, 0x1004, &raw mut gs_base); // ARCH_GET_GS
            }
            let host_tid = unsafe { libc::syscall(libc::SYS_gettid) as u64 };
            let rva = if image_base > 0 && lp_start_address >= image_base {
                lp_start_address - image_base
            } else {
                0
            };

            println!(
                "[RINE GUEST THREAD START] guest_start_va=0x{:016X} guest_start_rva=0x{:08X} thread_handle=0x{:016X} thread_id={} host_thread_id={} argument=0x{:016X} gs_base=0x{:016X} teb=0x{:016X} match={}",
                lp_start_address,
                rva,
                handle,
                thread_id,
                host_tid,
                lp_parameter,
                gs_base,
                teb_ptr,
                if gs_base == teb_ptr { "yes" } else { "no" }
            );

            if is_suspended {
                let (lock, cvar) = &*thread_entry_worker.suspended_sync;
                let mut guard = lock.lock().unwrap();
                while *guard {
                    guard = cvar.wait(guard).unwrap();
                }
            }

            let thread_fn: unsafe extern "win64" fn(u64) -> u32 =
                unsafe { std::mem::transmute(lp_start_address) };
            let ret = unsafe { thread_fn(lp_parameter) };

            println!(
                "[RINE GUEST THREAD EXIT] guest_start_va=0x{:016X} thread_handle=0x{:016X} thread_id={} exit_code={}",
                lp_start_address, handle, thread_id, ret
            );

            thread_entry_worker
                .exit_code
                .store(ret, std::sync::atomic::Ordering::SeqCst);
            thread_entry_worker
                .is_terminated
                .store(true, std::sync::atomic::Ordering::SeqCst);

            {
                let (lock, cvar) = &*thread_entry_worker.sync;
                let mut done = lock.lock().unwrap();
                *done = true;
                cvar.notify_all();
            }
        })
        .expect("Failed to spawn guest thread");

    handle
}

static THREAD_HANDLE_TABLE: std::sync::Mutex<Option<HashMap<u64, std::sync::Arc<ThreadEntry>>>> =
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
        let thread_entry = {
            let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
            if let Some(table) = guard.as_mut() {
                table.get(&h_handle).cloned()
            } else {
                None
            }
        };
        if let Some(entry) = thread_entry {
            if entry
                .is_terminated
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                return WAIT_OBJECT_0;
            }
            let (lock, cvar) = &*entry.sync;
            let mut done = lock.lock().unwrap();
            if *done
                || entry
                    .is_terminated
                    .load(std::sync::atomic::Ordering::Relaxed)
            {
                return WAIT_OBJECT_0;
            }
            if dw_milliseconds == 0xFFFFFFFF {
                while !*done
                    && !entry
                        .is_terminated
                        .load(std::sync::atomic::Ordering::Relaxed)
                {
                    done = cvar.wait(done).unwrap();
                }
                return WAIT_OBJECT_0;
            } else {
                let timeout = std::time::Duration::from_millis(dw_milliseconds as u64);
                let result = cvar.wait_timeout(done, timeout).unwrap();
                return if *result.0
                    || entry
                        .is_terminated
                        .load(std::sync::atomic::Ordering::Relaxed)
                {
                    WAIT_OBJECT_0
                } else {
                    WAIT_TIMEOUT
                };
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
    // Remove from power request table
    {
        let mut guard = POWER_REQUEST_TABLE.lock().unwrap();
        if let Some(table) = guard.as_mut() {
            table.remove(&h_object);
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetHandleInformation(
    _h_object: u64,
    lpdw_flags: *mut u32,
) -> i32 {
    if !lpdw_flags.is_null() {
        *lpdw_flags = 0;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetHandleInformation(
    _h_object: u64,
    _dw_mask: u32,
    _dw_flags: u32,
) -> i32 {
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
pub unsafe extern "win64" fn GetThreadId(h_thread: u64) -> u32 {
    if h_thread == 0xFFFFFFFFFFFFFFFEu64 || h_thread == 0xFFFFFFFFFFFFFFFFu64 {
        return GetCurrentThreadId();
    }
    let guard = THREAD_HANDLE_TABLE.lock().unwrap();
    if let Some(table) = guard.as_ref() {
        if let Some(entry) = table.get(&h_thread) {
            return entry.thread_id;
        }
    }
    if (h_thread & 0xF) == 0x3 {
        return (h_thread >> 4) as u32;
    }
    GetCurrentThreadId()
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentProcessId() -> u32 {
    unsafe { libc::getpid() as u32 }
}

#[no_mangle]
pub unsafe extern "win64" fn GetExitCodeThread(h_thread: u64, lp_exit_code: *mut u32) -> i32 {
    if !lp_exit_code.is_null() {
        let guard = THREAD_HANDLE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_ref() {
            if let Some(entry) = table.get(&h_thread) {
                if entry
                    .is_terminated
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    *lp_exit_code = entry.exit_code.load(std::sync::atomic::Ordering::Relaxed);
                } else {
                    *lp_exit_code = 259; // STILL_ACTIVE
                }
                return 1;
            }
        }
        *lp_exit_code = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetPriorityClass(_h_process: usize, dw_priority_class: u32) -> i32 {
    println!(
        "[KERNEL32] SetPriorityClass(dwPriorityClass=0x{:X}) -> TRUE",
        dw_priority_class
    );
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetPriorityClass(_h_process: usize) -> u32 {
    0x00000020 // NORMAL_PRIORITY_CLASS
}

#[no_mangle]
pub unsafe extern "win64" fn SetThreadPriority(_h_thread: u64, _n_priority: i32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SuspendThread(h_thread: u64) -> u32 {
    let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
    if let Some(table) = guard.as_mut() {
        if let Some(entry) = table.get(&h_thread) {
            let (lock, _cvar) = &*entry.suspended_sync;
            let mut g = lock.lock().unwrap();
            let prev = if *g { 1 } else { 0 };
            *g = true;
            return prev;
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ResumeThread(h_thread: u64) -> u32 {
    let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
    if let Some(table) = guard.as_mut() {
        if let Some(entry) = table.get(&h_thread) {
            let (lock, cvar) = &*entry.suspended_sync;
            let mut g = lock.lock().unwrap();
            if *g {
                *g = false;
                cvar.notify_all();
                println!(
                    "[KERNEL32] ResumeThread(handle=0x{:X}) -> 1 (resumed)",
                    h_thread
                );
                return 1;
            }
        }
    }
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

    let module_name = ModuleRegistry::get_module_name(h_module)
        .unwrap_or_else(|| format!("HMODULE_0x{:016X}", h_module));

    if module_name.eq_ignore_ascii_case("d3d11.dll") || module_name.eq_ignore_ascii_case("d3d11") {
        if let Some(addr) = crate::dlls::dxvk_loader::resolve_dxvk_symbol("d3d11.dll", &func_name) {
            println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
            return addr;
        }
    }
    if module_name.eq_ignore_ascii_case("dxgi.dll") || module_name.eq_ignore_ascii_case("dxgi") {
        if let Some(addr) = crate::dlls::dxvk_loader::resolve_dxvk_symbol("dxgi.dll", &func_name) {
            println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
            return addr;
        }
    }
    if module_name.eq_ignore_ascii_case("vulkan-1.dll")
        || module_name.eq_ignore_ascii_case("vulkan-1")
    {
        if let Some(addr) = crate::dlls::dxvk_loader::resolve_vulkan_symbol(&func_name) {
            println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
            return addr;
        }
    }

    if let Some(resolved) =
        crate::providers::RineApiResolver::resolve_symbol(&module_name, &func_name)
    {
        println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} provider={:?}", h_module, module_name, func_name, resolved.address, resolved.provider);
        return resolved.address as usize;
    }

    for fallback_dll in &[
        "MSVCRT.DLL",
        "KERNEL32.DLL",
        "USER32.DLL",
        "NTDLL.DLL",
        "VCRUNTIME140.DLL",
        "UCRTBASE.DLL",
    ] {
        if let Some(resolved) =
            crate::providers::RineApiResolver::resolve_symbol(fallback_dll, &func_name)
        {
            println!("[RINE GETPROCADDRESS FALLBACK BIND] hmodule=0x{:016X} fallback_dll='{}' symbol='{}' returned_addr=0x{:016X}", h_module, fallback_dll, func_name, resolved.address);
            return resolved.address as usize;
        }
    }

    println!(
        "[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x0 stub_id=0 (UNRESOLVED)",
        h_module, module_name, func_name
    );
    SetLastError(127); // ERROR_PROC_NOT_FOUND
    0
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryW(lp_lib_name: *const u16) -> usize {
    if lp_lib_name.is_null() {
        SetLastError(126); // ERROR_MOD_NOT_FOUND
        return 0;
    }
    let name_opt = read_utf16_ptr(lp_lib_name);
    let name = match name_opt {
        Some(s) => s,
        None => {
            SetLastError(126);
            return 0;
        }
    };
    println!("[KERNEL32] LoadLibraryW('{}')", name);
    let base = ModuleRegistry::load_library(&name);
    if base == 0 {
        SetLastError(126);
    }
    base
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryA(lp_lib_name: *const libc::c_char) -> usize {
    if lp_lib_name.is_null() {
        SetLastError(126);
        return 0;
    }
    let name = std::ffi::CStr::from_ptr(lp_lib_name)
        .to_string_lossy()
        .to_string();
    println!("[KERNEL32] LoadLibraryA('{}')", name);
    let base = ModuleRegistry::load_library(&name);
    if base == 0 {
        SetLastError(126);
    }
    base
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryExW(
    lp_lib_name: *const u16,
    _h_file: usize,
    _flags: u32,
) -> usize {
    LoadLibraryW(lp_lib_name)
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryExA(
    lp_lib_name: *const libc::c_char,
    _h_file: usize,
    _flags: u32,
) -> usize {
    LoadLibraryA(lp_lib_name)
}

#[no_mangle]
pub unsafe extern "win64" fn FreeLibrary(h_lib_module: usize) -> i32 {
    println!("[KERNEL32] FreeLibrary(0x{:016X})", h_lib_module);
    if ModuleRegistry::free_library(h_lib_module) {
        1
    } else {
        SetLastError(6); // ERROR_INVALID_HANDLE
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn DisableThreadLibraryCalls(_h_lib_module: usize) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetTickCount() -> u32 {
    static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START_TIME.get_or_init(std::time::Instant::now);
    start.elapsed().as_millis() as u32
}

#[no_mangle]
pub unsafe extern "win64" fn IsBadStringPtrW(_lpsz: *const u16, _ucch_max: usize) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn RaiseException(
    dw_exception_code: u32,
    dw_exception_flags: u32,
    n_number_of_arguments: u32,
    _lp_arguments: *const usize,
) {
    println!(
        "[KERNEL32] RaiseException: code=0x{:08X}, flags=0x{:X}, num_args={}",
        dw_exception_code, dw_exception_flags, n_number_of_arguments
    );
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

    if cb_multi_byte == 0 || lp_multi_byte_str.is_null() || (lp_multi_byte_str as usize) < 0x10000 {
        out_bytes.len() as i32
    } else if (cb_multi_byte as usize) < out_bytes.len() {
        SetLastError(122); // ERROR_INSUFFICIENT_BUFFER
        0
    } else {
        std::ptr::copy_nonoverlapping(out_bytes.as_ptr(), lp_multi_byte_str, out_bytes.len());
        out_bytes.len() as i32
    }
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
    if lp_multi_byte_str.is_null() || (lp_multi_byte_str as usize) < 0x10000 {
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

    if cch_wide_char == 0 || lp_wide_char_str.is_null() || (lp_wide_char_str as usize) < 0x10000 {
        return out_u16.len() as i32;
    }

    if (cch_wide_char as usize) < out_u16.len() {
        SetLastError(122); // ERROR_INSUFFICIENT_BUFFER
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

    #[test]
    fn executable_set_console_output_cp_preserves_win64_nonvolatile_gprs() {
        let fn_ptr = SetConsoleOutputCP as unsafe extern "win64" fn(u32) -> i32;

        let res: i32;
        let mut saved_regs = [0u64; 7];
        let ptr_val = saved_regs.as_mut_ptr();

        unsafe {
            std::arch::asm!(
                "mov rax, {fn_ptr}",
                "mov r10, {ptr}",

                "push rbx",
                "push rbp",
                "push rsi",
                "push rdi",
                "push r12",
                "push r13",
                "push r14",
                "push r15",

                "mov rbp, 0x5555666677778888",
                "mov rsi, 0x9999AAAABBBBCCCC",
                "mov rdi, 0xDDDDEEEEFFFF0000",
                "mov r12, 0x123456789ABCDEF0",
                "mov r13, 0x0FEDCBA987654321",
                "mov r14, 0xA1B2C3D4E5F60718",
                "mov r15, 0x9F8E7D6C5B4A3928",

                "mov ecx, 65001",
                "sub rsp, 0x20",
                "call rax",
                "add rsp, 0x20",

                "mov {res:e}, eax",

                "mov [r10 + 0x00], rbp",
                "mov [r10 + 0x08], rsi",
                "mov [r10 + 0x10], rdi",
                "mov [r10 + 0x18], r12",
                "mov [r10 + 0x20], r13",
                "mov [r10 + 0x28], r14",
                "mov [r10 + 0x30], r15",

                "pop r15",
                "pop r14",
                "pop r13",
                "pop r12",
                "pop rdi",
                "pop rsi",
                "pop rbp",
                "pop rbx",
                fn_ptr = in(reg) fn_ptr,
                ptr = in(reg) ptr_val,
                res = out(reg) res,
                out("rax") _,
                out("r10") _,
            );
        }

        assert_eq!(res, 1, "SetConsoleOutputCP must return Win32 BOOL TRUE (1)");
        assert_eq!(saved_regs[0], 0x5555666677778888, "RBP clobbered!");
        assert_eq!(saved_regs[1], 0x9999AAAABBBBCCCC, "RSI clobbered!");
        assert_eq!(saved_regs[2], 0xDDDDEEEEFFFF0000, "RDI clobbered!");
        assert_eq!(saved_regs[3], 0x123456789ABCDEF0, "R12 clobbered!");
        assert_eq!(saved_regs[4], 0x0FEDCBA987654321, "R13 clobbered!");
        assert_eq!(saved_regs[5], 0xA1B2C3D4E5F60718, "R14 clobbered!");
        assert_eq!(saved_regs[6], 0x9F8E7D6C5B4A3928, "R15 clobbered!");
    }

    #[test]
    fn test_wide_char_to_multi_byte_contract_suite() {
        unsafe {
            // A. ASCII + cchWideChar = -1
            let wstr_ascii: Vec<u16> = "abc\0".encode_utf16().collect();
            let req_ascii = WideCharToMultiByte(
                65001,
                0,
                wstr_ascii.as_ptr(),
                -1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(req_ascii, 4, "ASCII 'abc\\0' sizing must equal 4 bytes");

            // B. Non-ASCII UTF-8 (é is U+00E9 -> 2 UTF-8 bytes)
            let wstr_non_ascii: Vec<u16> = "A\u{00E9}\0".encode_utf16().collect();
            let req_non_ascii = WideCharToMultiByte(
                65001,
                0,
                wstr_non_ascii.as_ptr(),
                -1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(
                req_non_ascii, 4,
                "Non-ASCII 'A\\u{{00E9}}\\0' (1+2+1=4 bytes) sizing"
            );
            assert_ne!(
                wstr_non_ascii.len() as i32,
                req_non_ascii,
                "UTF-16 code units (3) != UTF-8 byte count (4)"
            );

            // C. Exact size conversion (cb_multi_byte == required)
            let mut dst_exact = vec![0u8; req_ascii as usize];
            let conv_exact = WideCharToMultiByte(
                65001,
                0,
                wstr_ascii.as_ptr(),
                -1,
                dst_exact.as_mut_ptr(),
                req_ascii,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(conv_exact, req_ascii);
            assert_eq!(&dst_exact[..], b"abc\0");

            // D. Insufficient buffer by one
            SetLastError(0);
            let mut dst_short = vec![0u8; (req_ascii - 1) as usize];
            let conv_short = WideCharToMultiByte(
                65001,
                0,
                wstr_ascii.as_ptr(),
                -1,
                dst_short.as_mut_ptr(),
                req_ascii - 1,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(conv_short, 0, "Insufficient buffer must return 0");
            assert_eq!(
                GetLastError(),
                122,
                "LastError must set ERROR_INSUFFICIENT_BUFFER (122)"
            );

            // E. Explicit cchWideChar excluding NUL
            let mut dst_no_nul = vec![0u8; 3];
            let conv_no_nul = WideCharToMultiByte(
                65001,
                0,
                wstr_ascii.as_ptr(),
                3,
                dst_no_nul.as_mut_ptr(),
                3,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(conv_no_nul, 3, "Explicit cch=3 excluding NUL must return 3");
            assert_eq!(&dst_no_nul[..], b"abc");

            // F. Empty NUL-terminated string L"\0" with cch = -1
            let wstr_empty: Vec<u16> = "\0".encode_utf16().collect();
            let req_empty = WideCharToMultiByte(
                65001,
                0,
                wstr_empty.as_ptr(),
                -1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(
                req_empty, 1,
                "Empty NUL-terminated string L'\\0' sizing must equal 1"
            );
        }
    }

    static WORKER_EXECUTED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    unsafe extern "win64" fn test_worker_func(_param: u64) -> u32 {
        WORKER_EXECUTED.store(true, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_guest_worker_start_executes() {
        WORKER_EXECUTED.store(false, std::sync::atomic::Ordering::SeqCst);
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_worker_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        assert!(WORKER_EXECUTED.load(std::sync::atomic::Ordering::SeqCst));
    }

    static STACK_ALIGN_RSP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    unsafe extern "win64" fn test_stack_align_func(_param: u64) -> u32 {
        let rsp: u64;
        std::arch::asm!("mov {}, rsp", out(reg) rsp);
        STACK_ALIGN_RSP.store(rsp, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_worker_stack_alignment() {
        STACK_ALIGN_RSP.store(0, std::sync::atomic::Ordering::SeqCst);
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_stack_align_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        let rsp = STACK_ALIGN_RSP.load(std::sync::atomic::Ordering::SeqCst);
        assert_ne!(rsp, 0);
        assert_eq!(rsp % 8, 0, "Stack pointer must be 8-byte aligned");
    }

    static WORKER_TEB_1: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    unsafe extern "win64" fn test_teb_func_1(_param: u64) -> u32 {
        let teb = get_teb_ptr() as u64;
        WORKER_TEB_1.store(teb, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_worker_has_distinct_teb() {
        let main_teb = unsafe { get_teb_ptr() as u64 };
        WORKER_TEB_1.store(0, std::sync::atomic::Ordering::SeqCst);
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_teb_func_1 as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        let worker_teb = WORKER_TEB_1.load(std::sync::atomic::Ordering::SeqCst);
        assert_ne!(worker_teb, 0, "Worker TEB must not be NULL");
        assert_ne!(
            worker_teb, main_teb,
            "Worker TEB must be distinct from main thread TEB"
        );
    }

    static WORKER_TEB_2: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static WORKER_GS_2: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    unsafe extern "win64" fn test_teb_func_2(_param: u64) -> u32 {
        let teb = get_teb_ptr() as u64;
        WORKER_TEB_2.store(teb, std::sync::atomic::Ordering::SeqCst);
        let mut gs_base: u64 = 0;
        libc::syscall(libc::SYS_arch_prctl, 0x1004, &raw mut gs_base);
        WORKER_GS_2.store(gs_base, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_worker_gs_points_to_teb() {
        WORKER_TEB_2.store(0, std::sync::atomic::Ordering::SeqCst);
        WORKER_GS_2.store(0, std::sync::atomic::Ordering::SeqCst);
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_teb_func_2 as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        let worker_teb = WORKER_TEB_2.load(std::sync::atomic::Ordering::SeqCst);
        let worker_gs = WORKER_GS_2.load(std::sync::atomic::Ordering::SeqCst);
        assert_ne!(worker_teb, 0);
        assert_eq!(
            worker_gs, worker_teb,
            "GS base must match worker TEB pointer"
        );
    }

    static WORKER_LAST_ERR0: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    static WORKER_LAST_ERR1: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    unsafe extern "win64" fn test_last_error_func(_param: u64) -> u32 {
        let err0 = GetLastError();
        WORKER_LAST_ERR0.store(err0, std::sync::atomic::Ordering::SeqCst);
        SetLastError(222);
        let err1 = GetLastError();
        WORKER_LAST_ERR1.store(err1, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_last_error_thread_isolation() {
        unsafe {
            SetLastError(111);
        }
        assert_eq!(unsafe { GetLastError() }, 111);

        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_last_error_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }

        assert_eq!(
            WORKER_LAST_ERR0.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(
            WORKER_LAST_ERR1.load(std::sync::atomic::Ordering::SeqCst),
            222
        );
        assert_eq!(
            unsafe { GetLastError() },
            111,
            "Main thread LastError must remain 111 after worker execution"
        );
    }

    static WORKER_TLS_VAL0: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static WORKER_TLS_VAL1: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static TEST_TLS_SLOT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    unsafe extern "win64" fn test_tls_func(_param: u64) -> u32 {
        let slot = TEST_TLS_SLOT.load(std::sync::atomic::Ordering::SeqCst);
        let v0 = TlsGetValue(slot);
        WORKER_TLS_VAL0.store(v0, std::sync::atomic::Ordering::SeqCst);
        TlsSetValue(slot, 0xBBBB);
        let v1 = TlsGetValue(slot);
        WORKER_TLS_VAL1.store(v1, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_tls_thread_isolation() {
        let slot = unsafe { TlsAlloc() };
        TEST_TLS_SLOT.store(slot, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            TlsSetValue(slot, 0xAAAA);
        }
        assert_eq!(unsafe { TlsGetValue(slot) }, 0xAAAA);

        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_tls_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }

        assert_eq!(WORKER_TLS_VAL0.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(
            WORKER_TLS_VAL1.load(std::sync::atomic::Ordering::SeqCst),
            0xBBBB
        );
        assert_eq!(
            unsafe { TlsGetValue(slot) },
            0xAAAA,
            "Main thread TLS value must remain 0xAAAA"
        );
        unsafe {
            TlsFree(slot);
        }
    }

    unsafe extern "win64" fn test_clean_return_func(_param: u64) -> u32 {
        0xDEADBEEF
    }

    #[test]
    fn test_guest_worker_clean_return() {
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_clean_return_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        let mut exit_code: u32 = 0;
        let res = unsafe { GetExitCodeThread(handle, &raw mut exit_code) };
        assert_eq!(res, 1);
        assert_eq!(exit_code, 0xDEADBEEF);
        unsafe {
            CloseHandle(handle);
        }
    }

    #[test]
    fn test_get_logical_drives_mask() {
        let mask = unsafe { GetLogicalDrives() };
        assert_ne!(mask, 0, "Logical drive mask must be non-zero");
        assert_eq!(
            mask & (1 << 2),
            1 << 2,
            "Bit 2 (Drive C:) must be set in logical drive mask"
        );
        assert_eq!(
            mask,
            1 << 2,
            "Alpha virtual drive policy must only set Bit 2 (Drive C:)"
        );
        let registry = get_drive_registry();
        assert!(registry.is_drive_available('C'));
        assert!(registry.is_drive_available('c'));
        assert!(!registry.is_drive_available('A'));
        assert!(!registry.is_drive_available('Z'));
    }

    #[test]
    fn test_get_logical_drives_win64_abi() {
        let func_ptr: unsafe extern "win64" fn() -> u32 = GetLogicalDrives;
        let mask = unsafe { func_ptr() };
        assert_eq!(mask, 0x00000004);
    }

    static CWD_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_get_current_directory_w_contract_suite() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("C:\\foo");
        let expected_len = 6u32; // "C:\foo" has 6 WCHARs

        // Test A: Sufficient buffer
        let mut buf = [0u16; 64];
        let ret = unsafe { GetCurrentDirectoryW(64, buf.as_mut_ptr()) };
        assert_eq!(ret, expected_len, "Must return WCHAR count excluding null");
        assert_eq!(
            String::from_utf16_lossy(&buf[..expected_len as usize]),
            "C:\\foo"
        );
        assert_eq!(buf[expected_len as usize], 0, "Must be null-terminated");

        // Test B: Exact capacity (len + 1 = 7)
        let mut exact_buf = [0u16; 7];
        let ret_exact = unsafe { GetCurrentDirectoryW(7, exact_buf.as_mut_ptr()) };
        assert_eq!(ret_exact, expected_len);
        assert_eq!(
            String::from_utf16_lossy(&exact_buf[..expected_len as usize]),
            "C:\\foo"
        );
        assert_eq!(exact_buf[expected_len as usize], 0);

        // Test C: One WCHAR too small (nBufferLength = len = 6)
        let mut small_buf = [0xAAAAu16; 6];
        let ret_small = unsafe { GetCurrentDirectoryW(6, small_buf.as_mut_ptr()) };
        assert_eq!(
            ret_small,
            expected_len + 1,
            "Must return required capacity (len + 1) when too small"
        );
        assert_eq!(
            small_buf, [0xAAAAu16; 6],
            "Small buffer must not be overwritten"
        );

        // Test D: nBufferLength = 0 / NULL buffer
        let ret_zero = unsafe { GetCurrentDirectoryW(0, std::ptr::null_mut()) };
        assert_eq!(ret_zero, expected_len + 1);
    }

    #[test]
    fn test_get_current_directory_w_unicode() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        let unicode_path = "C:\\Тест\\Проект";
        set_process_current_directory(unicode_path);
        let expected_utf16: Vec<u16> = unicode_path.encode_utf16().collect();
        let expected_len = expected_utf16.len() as u32;

        let mut buf = [0u16; 128];
        let ret = unsafe { GetCurrentDirectoryW(128, buf.as_mut_ptr()) };
        assert_eq!(ret, expected_len);
        assert_eq!(&buf[..expected_len as usize], expected_utf16.as_slice());
        assert_eq!(buf[expected_len as usize], 0);
        assert_eq!(
            String::from_utf16_lossy(&buf[..expected_len as usize]),
            unicode_path
        );
    }

    #[test]
    fn test_get_current_directory_w_buffer_bounds() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("C:\\Games\\Neighbor");
        let utf16: Vec<u16> = "C:\\Games\\Neighbor".encode_utf16().collect();
        let len = utf16.len();

        #[repr(C)]
        struct Guarded {
            canary_before: [u16; 8],
            buffer: [u16; 32],
            canary_after: [u16; 8],
        }

        let mut g = Guarded {
            canary_before: [0xCAFE; 8],
            buffer: [0; 32],
            canary_after: [0xBEEF; 8],
        };

        let ret = unsafe { GetCurrentDirectoryW(32, g.buffer.as_mut_ptr()) };
        assert_eq!(ret, len as u32);
        assert_eq!(g.canary_before, [0xCAFE; 8]);
        assert_eq!(g.canary_after, [0xBEEF; 8]);
        assert_eq!(g.buffer[len], 0);
    }

    #[test]
    fn test_set_current_directory_w_dot() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("\\tmp");
        let dot_utf16: Vec<u16> = ".\0".encode_utf16().collect();
        let res = unsafe { SetCurrentDirectoryW(dot_utf16.as_ptr()) };
        assert_eq!(res, 1, "SetCurrentDirectoryW('.') must succeed");
        assert_eq!(get_process_current_directory(), "\\tmp");
    }

    #[test]
    fn test_set_current_directory_w_nonexistent_does_not_change_state() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("\\tmp");
        let bad_utf16: Vec<u16> = "C:\\nonexistent_dir_987654321\0".encode_utf16().collect();
        let res = unsafe { SetCurrentDirectoryW(bad_utf16.as_ptr()) };
        assert_eq!(
            res, 0,
            "SetCurrentDirectoryW must fail on non-existent path"
        );
        assert_eq!(
            get_process_current_directory(),
            "\\tmp",
            "CWD must not change on failure"
        );
        let err = unsafe { GetLastError() };
        assert_ne!(err, 0, "LastError must be set on failure");
    }

    #[test]
    fn test_set_then_get_current_directory_roundtrip() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        let target_utf16: Vec<u16> = "C:\\tmp\0".encode_utf16().collect();
        let res = unsafe { SetCurrentDirectoryW(target_utf16.as_ptr()) };
        assert_eq!(res, 1);

        let mut buf = [0u16; 64];
        let len = unsafe { GetCurrentDirectoryW(64, buf.as_mut_ptr()) };
        assert!(len > 0);
        let read_back = String::from_utf16_lossy(&buf[..len as usize]);
        assert_eq!(read_back, "C:\\tmp");
    }

    #[test]
    fn test_process_cwd_shared_between_threads() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("\\tmp");
        let handle = std::thread::spawn(|| {
            assert_eq!(get_process_current_directory(), "\\tmp");
            set_process_current_directory("\\proc");
        });
        handle.join().unwrap();
        assert_eq!(get_process_current_directory(), "\\proc");
    }

    #[test]
    fn test_console_screen_buffer_info_layout_and_bounds() {
        assert_eq!(
            std::mem::size_of::<ConsoleScreenBufferInfo>(),
            22,
            "CONSOLE_SCREEN_BUFFER_INFO must be 22 bytes in Win32/Win64 ABI"
        );
        assert_eq!(
            std::mem::align_of::<ConsoleScreenBufferInfo>(),
            2,
            "CONSOLE_SCREEN_BUFFER_INFO alignment must be 2"
        );

        let dummy: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let base = &dummy as *const _ as usize;
        let off_dw_size = (&dummy.dw_size as *const _ as usize) - base;
        let off_cursor = (&dummy.dw_cursor_position as *const _ as usize) - base;
        let off_attr = (&dummy.w_attributes as *const _ as usize) - base;
        let off_window = (&dummy.sr_window as *const _ as usize) - base;
        let off_max = (&dummy.dw_maximum_window_size as *const _ as usize) - base;

        assert_eq!(off_dw_size, 0, "dwSize offset must be 0");
        assert_eq!(off_cursor, 4, "dwCursorPosition offset must be 4");
        assert_eq!(off_attr, 8, "wAttributes offset must be 8");
        assert_eq!(off_window, 10, "srWindow offset must be 10");
        assert_eq!(off_max, 18, "dwMaximumWindowSize offset must be 18");

        #[repr(C)]
        struct GuardedInfo {
            canary_before: [u32; 4],
            info: ConsoleScreenBufferInfo,
            canary_after: [u32; 4],
        }

        let mut guarded = GuardedInfo {
            canary_before: [0xDEADBEEF; 4],
            info: unsafe { std::mem::zeroed() },
            canary_after: [0xCAFEBABE; 4],
        };

        let res = unsafe { GetConsoleScreenBufferInfo(0x8, &mut guarded.info) };
        assert_eq!(res, 1);
        assert_eq!(guarded.canary_before, [0xDEADBEEF; 4]);
        assert_eq!(guarded.canary_after, [0xCAFEBABE; 4]);
    }

    #[test]
    fn test_get_console_screen_buffer_info_valid_console_handle() {
        let mut info: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let res = unsafe { GetConsoleScreenBufferInfo(0x8, &mut info) };
        assert_eq!(res, 1);
        assert_eq!(info.dw_size, Coord { x: 80, y: 25 });
        assert_eq!(info.dw_cursor_position, Coord { x: 0, y: 0 });
        assert_eq!(info.w_attributes, 0x0007);
        assert_eq!(
            info.sr_window,
            SmallRect {
                left: 0,
                top: 0,
                right: 79,
                bottom: 24,
            }
        );
        assert_eq!(info.dw_maximum_window_size, Coord { x: 80, y: 25 });
    }

    #[test]
    fn test_get_console_screen_buffer_info_invalid_handle() {
        let mut info: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let res = unsafe { GetConsoleScreenBufferInfo(0x9999, &mut info) };
        assert_eq!(res, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 6, "Must set ERROR_INVALID_HANDLE (6)");
    }

    #[test]
    fn test_get_console_screen_buffer_info_null_output() {
        let res = unsafe { GetConsoleScreenBufferInfo(0x8, std::ptr::null_mut()) };
        assert_eq!(res, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 87, "Must set ERROR_INVALID_PARAMETER (87)");
    }

    #[test]
    fn executable_get_console_screen_buffer_info_preserves_win64_nonvolatile_gprs() {
        let func_ptr: unsafe extern "win64" fn(u64, *mut ConsoleScreenBufferInfo) -> i32 =
            GetConsoleScreenBufferInfo;
        let mut info: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let res = unsafe { func_ptr(0x8, &mut info) };
        assert_eq!(res, 1);
    }

    #[test]
    fn test_set_console_text_attribute_state_roundtrip() {
        set_console_text_attributes(0x0007);
        let mut info: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let res1 = unsafe { GetConsoleScreenBufferInfo(0x8, &mut info) };
        assert_eq!(res1, 1);
        assert_eq!(info.w_attributes, 0x0007);

        let res2 = unsafe { SetConsoleTextAttribute(0x8, 0x000C) };
        assert_eq!(res2, 1);

        let res3 = unsafe { GetConsoleScreenBufferInfo(0x8, &mut info) };
        assert_eq!(res3, 1);
        assert_eq!(info.w_attributes, 0x000C);

        let res4 = unsafe { SetConsoleTextAttribute(0x8, 0x0007) };
        assert_eq!(res4, 1);

        let res5 = unsafe { GetConsoleScreenBufferInfo(0x8, &mut info) };
        assert_eq!(res5, 1);
        assert_eq!(info.w_attributes, 0x0007);
    }

    #[test]
    fn test_set_console_text_attribute_invalid_handle() {
        let res = unsafe { SetConsoleTextAttribute(0x9999, 0x000C) };
        assert_eq!(res, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 6, "Must set ERROR_INVALID_HANDLE (6)");
    }

    #[test]
    fn test_set_console_text_attribute_word_width() {
        let res = unsafe { SetConsoleTextAttribute(0x8, 0x002F) };
        assert_eq!(res, 1);
        assert_eq!(get_console_text_attributes(), 0x002F);

        // Reset to default
        set_console_text_attributes(0x0007);
    }

    #[test]
    fn executable_set_console_text_attribute_preserves_win64_nonvolatile_gprs() {
        let func_ptr: unsafe extern "win64" fn(u64, u16) -> i32 = SetConsoleTextAttribute;
        let res = unsafe { func_ptr(0x8, 0x0007) };
        assert_eq!(res, 1);
    }

    #[test]
    fn test_initial_guest_cwd_policy() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();

        // 1. Explicit configured CWD
        initialize_process_filesystem("/opt/games/test/game.exe", Some("/custom/cwd"));
        assert_eq!(get_process_current_directory(), "\\custom\\cwd");

        // 2. Default to parent directory of executable
        initialize_process_filesystem("/opt/games/test/game.exe", None);
        assert_eq!(get_process_current_directory(), "\\opt\\games\\test");

        // 3. Project data relative path resolution
        let rel_pck = win32_to_host_path("game.pck");
        assert_eq!(rel_pck, "/opt/games/test/game.pck");

        // 4. Absolute path preserves target
        let abs_path = win32_to_host_path("C:\\opt\\games\\test\\game.exe");
        assert_eq!(abs_path, "/opt/games/test/game.exe");
    }

    #[test]
    fn test_system_info_win64_layout() {
        assert_eq!(
            std::mem::size_of::<SystemInfo>(),
            48,
            "Win64 SYSTEM_INFO size must be exactly 48 bytes"
        );
        assert_eq!(
            std::mem::align_of::<SystemInfo>(),
            8,
            "Win64 SYSTEM_INFO alignment must be 8 bytes"
        );

        let dummy = SystemInfo {
            w_processor_architecture: 0,
            w_reserved: 0,
            dw_page_size: 0,
            lp_minimum_application_address: 0,
            lp_maximum_application_address: 0,
            dw_active_processor_mask: 0,
            dw_number_of_processors: 0,
            dw_processor_type: 0,
            dw_allocation_granularity: 0,
            w_processor_level: 0,
            w_processor_revision: 0,
        };
        let base = &raw const dummy as usize;
        assert_eq!(
            (&raw const dummy.w_processor_architecture as usize) - base,
            0
        );
        assert_eq!((&raw const dummy.w_reserved as usize) - base, 2);
        assert_eq!((&raw const dummy.dw_page_size as usize) - base, 4);
        assert_eq!(
            (&raw const dummy.lp_minimum_application_address as usize) - base,
            8
        );
        assert_eq!(
            (&raw const dummy.lp_maximum_application_address as usize) - base,
            16
        );
        assert_eq!(
            (&raw const dummy.dw_active_processor_mask as usize) - base,
            24
        );
        assert_eq!(
            (&raw const dummy.dw_number_of_processors as usize) - base,
            32
        );
        assert_eq!((&raw const dummy.dw_processor_type as usize) - base, 36);
        assert_eq!(
            (&raw const dummy.dw_allocation_granularity as usize) - base,
            40
        );
        assert_eq!((&raw const dummy.w_processor_level as usize) - base, 44);
        assert_eq!((&raw const dummy.w_processor_revision as usize) - base, 46);
    }

    #[test]
    fn test_get_native_system_info_buffer_bounds() {
        #[repr(C)]
        struct GuardedSystemInfo {
            guard_before: [u8; 16],
            info: SystemInfo,
            guard_after: [u8; 16],
        }

        let mut guarded = GuardedSystemInfo {
            guard_before: [0xAA; 16],
            info: unsafe { std::mem::zeroed() },
            guard_after: [0xBB; 16],
        };

        unsafe {
            GetNativeSystemInfo(&mut guarded.info);
        }

        assert_eq!(
            guarded.guard_before, [0xAA; 16],
            "guard_before was overwritten!"
        );
        assert_eq!(
            guarded.guard_after, [0xBB; 16],
            "guard_after was overwritten!"
        );
        assert_eq!(
            guarded.info.w_processor_architecture,
            PROCESSOR_ARCHITECTURE_AMD64
        );
        assert_eq!(guarded.info.dw_page_size, 4096);
        assert_eq!(guarded.info.dw_allocation_granularity, 65536);
        assert!(guarded.info.dw_number_of_processors >= 1);
        assert_ne!(guarded.info.dw_active_processor_mask, 0);
    }

    #[test]
    fn test_get_native_system_info_fields() {
        let mut info: SystemInfo = unsafe { std::mem::zeroed() };
        unsafe {
            GetNativeSystemInfo(&mut info);
        }

        assert_eq!(info.w_processor_architecture, PROCESSOR_ARCHITECTURE_AMD64);
        assert_eq!(info.w_reserved, 0);
        assert_eq!(info.dw_page_size, 4096);
        assert_eq!(info.lp_minimum_application_address, 0x10000);
        assert_eq!(info.lp_maximum_application_address, 0x00007FFFFFFEFFFF);
        assert_eq!(info.dw_processor_type, PROCESSOR_AMD_X8664);
        assert_eq!(info.dw_allocation_granularity, 65536);
        assert_eq!(info.w_processor_level, 6);
        assert_eq!(info.w_processor_revision, 0);
        assert!(info.dw_number_of_processors >= 1);
    }

    #[test]
    fn test_system_info_native_and_process_consistency() {
        let mut native_info: SystemInfo = unsafe { std::mem::zeroed() };
        let mut sys_info: SystemInfo = unsafe { std::mem::zeroed() };

        unsafe {
            GetNativeSystemInfo(&mut native_info);
            GetSystemInfo(&mut sys_info);
        }

        assert_eq!(native_info, sys_info);
    }

    #[test]
    fn test_get_native_system_info_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*mut SystemInfo) = GetNativeSystemInfo;
        let mut info: SystemInfo = unsafe { std::mem::zeroed() };
        unsafe {
            func_ptr(&mut info);
        }
        assert_eq!(info.w_processor_architecture, PROCESSOR_ARCHITECTURE_AMD64);
    }

    #[test]
    fn test_create_directory_w_contract_suite() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();

        let temp_base = std::env::temp_dir().join(format!(
            "rine_test_mkdir_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_base).unwrap();

        let temp_base_str = temp_base.to_string_lossy().to_string();
        initialize_process_filesystem("/tmp/dummy.exe", Some(&temp_base_str));
        let initial_cwd = get_process_current_directory();

        // 1. Success creating single directory
        let dir1_utf16: Vec<u16> = "new_folder"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let res = unsafe { CreateDirectoryW(dir1_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(res, 1, "CreateDirectoryW must return 1 on success");
        assert!(temp_base.join("new_folder").is_dir());

        // 2. Failure: already exists
        let res_dup = unsafe { CreateDirectoryW(dir1_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(
            res_dup, 0,
            "CreateDirectoryW must return 0 if already exists"
        );
        let err = unsafe { GetLastError() };
        assert_eq!(err, 183, "Must set ERROR_ALREADY_EXISTS (183)");

        // 3. Success creating Unicode directory
        let u_name = "Юникод_Папка_Тест";
        let u_utf16: Vec<u16> = u_name.encode_utf16().chain(std::iter::once(0)).collect();
        let res_u = unsafe { CreateDirectoryW(u_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(res_u, 1, "CreateDirectoryW must create Unicode directories");
        assert!(temp_base.join(u_name).is_dir());

        // 4. Failure: missing parent (non-recursive)
        let deep_utf16: Vec<u16> = "missing_parent\\child"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let res_deep = unsafe { CreateDirectoryW(deep_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(
            res_deep, 0,
            "Non-recursive mkdir must fail if parent missing"
        );
        let err_deep = unsafe { GetLastError() };
        assert_eq!(err_deep, 3, "Must set ERROR_PATH_NOT_FOUND (3)");

        // 5. Failure: file collision
        let file_path = temp_base.join("colliding_file.txt");
        std::fs::write(&file_path, b"dummy").unwrap();
        let col_utf16: Vec<u16> = "colliding_file.txt"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let res_col = unsafe { CreateDirectoryW(col_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(res_col, 0, "Must fail if path exists as file");
        let err_col = unsafe { GetLastError() };
        assert_eq!(err_col, 183, "Must set ERROR_ALREADY_EXISTS (183)");

        // 6. Invariant: Process CWD was NOT altered by CreateDirectoryW
        assert_eq!(get_process_current_directory(), initial_cwd);

        // 7. Executable Win64 calling convention test
        let func_ptr: unsafe extern "win64" fn(*const u16, *const std::ffi::c_void) -> i32 =
            CreateDirectoryW;
        let win64_dir_utf16: Vec<u16> = "win64_call_dir"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let res_win64 = unsafe { func_ptr(win64_dir_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(res_win64, 1);
        assert!(temp_base.join("win64_call_dir").is_dir());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_base);
    }

    #[test]
    fn test_systemtime_win64_layout() {
        assert_eq!(
            std::mem::size_of::<SystemTime>(),
            16,
            "SYSTEMTIME size must be exactly 16 bytes"
        );
        assert_eq!(
            std::mem::align_of::<SystemTime>(),
            2,
            "SYSTEMTIME alignment must be 2 bytes"
        );

        let dummy = SystemTime {
            w_year: 0,
            w_month: 0,
            w_day_of_week: 0,
            w_day: 0,
            w_hour: 0,
            w_minute: 0,
            w_second: 0,
            w_milliseconds: 0,
        };
        let base = &raw const dummy as usize;
        assert_eq!((&raw const dummy.w_year as usize) - base, 0);
        assert_eq!((&raw const dummy.w_month as usize) - base, 2);
        assert_eq!((&raw const dummy.w_day_of_week as usize) - base, 4);
        assert_eq!((&raw const dummy.w_day as usize) - base, 6);
        assert_eq!((&raw const dummy.w_hour as usize) - base, 8);
        assert_eq!((&raw const dummy.w_minute as usize) - base, 10);
        assert_eq!((&raw const dummy.w_second as usize) - base, 12);
        assert_eq!((&raw const dummy.w_milliseconds as usize) - base, 14);
    }

    #[test]
    fn test_get_local_time_buffer_bounds() {
        #[repr(C)]
        struct GuardedSystemTime {
            guard_before: [u8; 16],
            time: SystemTime,
            guard_after: [u8; 16],
        }

        let mut guarded = GuardedSystemTime {
            guard_before: [0xAA; 16],
            time: unsafe { std::mem::zeroed() },
            guard_after: [0xBB; 16],
        };

        unsafe {
            GetLocalTime(&mut guarded.time);
        }

        assert_eq!(
            guarded.guard_before, [0xAA; 16],
            "guard_before was overwritten!"
        );
        assert_eq!(
            guarded.guard_after, [0xBB; 16],
            "guard_after was overwritten!"
        );
        assert!(guarded.time.w_year >= 2024);
        assert!(guarded.time.w_month >= 1 && guarded.time.w_month <= 12);
        assert!(guarded.time.w_day >= 1 && guarded.time.w_day <= 31);
        assert!(guarded.time.w_day_of_week <= 6);
        assert!(guarded.time.w_hour <= 23);
        assert!(guarded.time.w_minute <= 59);
        assert!(guarded.time.w_second <= 59);
        assert!(guarded.time.w_milliseconds <= 999);
    }

    #[test]
    fn test_get_local_time_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*mut SystemTime) = GetLocalTime;
        let mut st: SystemTime = unsafe { std::mem::zeroed() };
        unsafe {
            func_ptr(&mut st);
        }
        assert!(st.w_year >= 2024);
        assert!(st.w_month >= 1 && st.w_month <= 12);
    }

    #[test]
    fn test_time_zone_information_win64_layout() {
        assert_eq!(
            std::mem::size_of::<TimeZoneInformation>(),
            172,
            "TIME_ZONE_INFORMATION size must be exactly 172 bytes"
        );
        assert_eq!(
            std::mem::align_of::<TimeZoneInformation>(),
            4,
            "TIME_ZONE_INFORMATION alignment must be 4 bytes"
        );

        let dummy = TimeZoneInformation {
            bias: 0,
            standard_name: [0u16; 32],
            standard_date: SystemTime {
                w_year: 0,
                w_month: 0,
                w_day_of_week: 0,
                w_day: 0,
                w_hour: 0,
                w_minute: 0,
                w_second: 0,
                w_milliseconds: 0,
            },
            standard_bias: 0,
            daylight_name: [0u16; 32],
            daylight_date: SystemTime {
                w_year: 0,
                w_month: 0,
                w_day_of_week: 0,
                w_day: 0,
                w_hour: 0,
                w_minute: 0,
                w_second: 0,
                w_milliseconds: 0,
            },
            daylight_bias: 0,
        };
        let base = &raw const dummy as usize;
        assert_eq!((&raw const dummy.bias as usize) - base, 0);
        assert_eq!((&raw const dummy.standard_name as usize) - base, 4);
        assert_eq!((&raw const dummy.standard_date as usize) - base, 68);
        assert_eq!((&raw const dummy.standard_bias as usize) - base, 84);
        assert_eq!((&raw const dummy.daylight_name as usize) - base, 88);
        assert_eq!((&raw const dummy.daylight_date as usize) - base, 152);
        assert_eq!((&raw const dummy.daylight_bias as usize) - base, 168);
    }

    #[test]
    fn test_get_time_zone_information_buffer_bounds() {
        #[repr(C)]
        struct GuardedTzi {
            guard_before: [u8; 16],
            info: TimeZoneInformation,
            guard_after: [u8; 16],
        }

        let mut guarded = GuardedTzi {
            guard_before: [0xCC; 16],
            info: unsafe { std::mem::zeroed() },
            guard_after: [0xDD; 16],
        };

        let ret = unsafe { GetTimeZoneInformation(&mut guarded.info) };
        assert_ne!(
            ret, TIME_ZONE_ID_INVALID,
            "GetTimeZoneInformation must not return INVALID"
        );
        assert_eq!(
            guarded.guard_before, [0xCC; 16],
            "guard_before was overwritten!"
        );
        assert_eq!(
            guarded.guard_after, [0xDD; 16],
            "guard_after was overwritten!"
        );
    }

    #[test]
    fn test_get_time_zone_information_bias_sign() {
        let mut tzi: TimeZoneInformation = unsafe { std::mem::zeroed() };
        let ret = unsafe { GetTimeZoneInformation(&mut tzi) };
        assert!(
            ret == TIME_ZONE_ID_UNKNOWN
                || ret == TIME_ZONE_ID_STANDARD
                || ret == TIME_ZONE_ID_DAYLIGHT
        );

        // Verification of Bias sign:
        // Host local time and UTC time check
        let local_st = WallClock::now_local();
        let utc_st = WallClock::now_utc();

        // Convert hours & minutes to total minutes within day
        let local_mins = (local_st.w_hour as i32 * 60) + local_st.w_minute as i32;
        let utc_mins = (utc_st.w_hour as i32 * 60) + utc_st.w_minute as i32;

        // Difference modulo 1440 minutes (24h)
        let diff_mins = (utc_mins - local_mins + 1440) % 1440;
        let expected_bias = if diff_mins > 720 {
            diff_mins - 1440
        } else {
            diff_mins
        };

        assert_eq!(
            tzi.bias, expected_bias,
            "Windows Bias must satisfy UTC = Local + Bias"
        );
    }

    #[test]
    fn test_get_time_zone_information_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*mut TimeZoneInformation) -> u32 =
            GetTimeZoneInformation;
        let mut tzi: TimeZoneInformation = unsafe { std::mem::zeroed() };
        let res = unsafe { func_ptr(&mut tzi) };
        assert_ne!(res, TIME_ZONE_ID_INVALID);
    }

    #[test]
    fn test_get_environment_variable_w_found_and_not_found() {
        GuestEnvironment::set("RINE_TEST_KEY", Some("TestValue123"));

        // 1. Found with sufficient buffer
        let key_utf16: Vec<u16> = "RINE_TEST_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut buf = [0u16; 64];
        let len = unsafe { GetEnvironmentVariableW(key_utf16.as_ptr(), buf.as_mut_ptr(), 64) };
        assert_eq!(len, 12);
        let val = String::from_utf16(&buf[..12]).unwrap();
        assert_eq!(val, "TestValue123");
        assert_eq!(buf[12], 0);

        // 2. Case-insensitivity
        let key_lower_utf16: Vec<u16> = "rine_test_key"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let len_lower =
            unsafe { GetEnvironmentVariableW(key_lower_utf16.as_ptr(), buf.as_mut_ptr(), 64) };
        assert_eq!(len_lower, 12);

        // 3. Not found
        let key_missing: Vec<u16> = "NONEXISTENT_ENV_VAR"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let len_missing =
            unsafe { GetEnvironmentVariableW(key_missing.as_ptr(), buf.as_mut_ptr(), 64) };
        assert_eq!(len_missing, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 203, "Must set ERROR_ENVVAR_NOT_FOUND (203)");
    }

    #[test]
    fn test_get_environment_variable_w_insufficient_buffer() {
        GuestEnvironment::set("SHORT_KEY", Some("abcd"));
        let key_utf16: Vec<u16> = "SHORT_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut buf = [0u16; 4]; // capacity is 4, but required capacity with null is 5
        let req_len = unsafe { GetEnvironmentVariableW(key_utf16.as_ptr(), buf.as_mut_ptr(), 4) };
        assert_eq!(
            req_len, 5,
            "Must return required capacity including null terminator"
        );
    }

    #[test]
    fn test_get_environment_variable_w_unicode() {
        GuestEnvironment::set("UNICODE_KEY", Some("ТестовоеЗначение"));
        let key_utf16: Vec<u16> = "UNICODE_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut buf = [0u16; 64];
        let len = unsafe { GetEnvironmentVariableW(key_utf16.as_ptr(), buf.as_mut_ptr(), 64) };
        assert_eq!(len, 16);
        let val = String::from_utf16(&buf[..16]).unwrap();
        assert_eq!(val, "ТестовоеЗначение");
    }

    #[test]
    fn test_get_environment_variable_w_buffer_bounds() {
        GuestEnvironment::set("BOUNDS_KEY", Some("hello"));
        let key_utf16: Vec<u16> = "BOUNDS_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        #[repr(C)]
        struct GuardedBuf {
            guard_before: [u16; 8],
            data: [u16; 6], // "hello\0"
            guard_after: [u16; 8],
        }

        let mut guarded = GuardedBuf {
            guard_before: [0xAAAA; 8],
            data: [0; 6],
            guard_after: [0xBBBB; 8],
        };

        let ret =
            unsafe { GetEnvironmentVariableW(key_utf16.as_ptr(), guarded.data.as_mut_ptr(), 6) };
        assert_eq!(ret, 5);
        assert_eq!(guarded.guard_before, [0xAAAA; 8]);
        assert_eq!(guarded.guard_after, [0xBBBB; 8]);
    }

    #[test]
    fn test_get_environment_variable_w_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*const u16, *mut u16, u32) -> u32 =
            GetEnvironmentVariableW;
        GuestEnvironment::set("WIN64_ENV_KEY", Some("win64_val"));
        let key_utf16: Vec<u16> = "WIN64_ENV_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut buf = [0u16; 32];
        let ret = unsafe { func_ptr(key_utf16.as_ptr(), buf.as_mut_ptr(), 32) };
        assert_eq!(ret, 9);
    }

    #[test]
    fn test_reason_context_win64_layout() {
        assert_eq!(
            std::mem::size_of::<ReasonContext>(),
            32,
            "REASON_CONTEXT size must be exactly 32 bytes"
        );
        assert_eq!(
            std::mem::align_of::<ReasonContext>(),
            8,
            "REASON_CONTEXT alignment must be 8 bytes"
        );

        let dummy = ReasonContext {
            version: 0,
            flags: 0,
            reason: ReasonContextUnion {
                simple_reason_string: std::ptr::null(),
            },
        };
        let base = &raw const dummy as usize;
        assert_eq!((&raw const dummy.version as usize) - base, 0);
        assert_eq!((&raw const dummy.flags as usize) - base, 4);
        assert_eq!((&raw const dummy.reason as usize) - base, 8);
    }

    #[test]
    fn test_power_request_lifecycle_and_counting() {
        let reason_utf16: Vec<u16> = "Test Reason"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let ctx = ReasonContext {
            version: POWER_REQUEST_CONTEXT_VERSION,
            flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            reason: ReasonContextUnion {
                simple_reason_string: reason_utf16.as_ptr(),
            },
        };

        // 1. Create valid power request
        let handle = unsafe { PowerCreateRequest(&ctx) };
        assert_ne!(handle, 0);
        assert_ne!(handle, 0xFFFFFFFFFFFFFFFFu64);

        // 2. Set requests and check counting
        let res_sys1 = unsafe { PowerSetRequest(handle, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_sys1, 1);
        let res_sys2 = unsafe { PowerSetRequest(handle, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_sys2, 1);
        let res_disp = unsafe { PowerSetRequest(handle, POWER_REQUEST_DISPLAY_REQUIRED) };
        assert_eq!(res_disp, 1);

        {
            let g = POWER_REQUEST_TABLE.lock().unwrap();
            let table = g.as_ref().unwrap();
            let obj_arc = table.get(&handle).unwrap();
            let obj = obj_arc.lock().unwrap();
            assert_eq!(obj.system_required, 2);
            assert_eq!(obj.display_required, 1);
            assert_eq!(obj.reason, "Test Reason");
        }

        // 3. Clear requests
        let res_clr_sys = unsafe { PowerClearRequest(handle, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_clr_sys, 1);
        {
            let g = POWER_REQUEST_TABLE.lock().unwrap();
            let table = g.as_ref().unwrap();
            let obj_arc = table.get(&handle).unwrap();
            let obj = obj_arc.lock().unwrap();
            assert_eq!(obj.system_required, 1);
        }

        // 4. Invalid handle rejection
        let res_invalid = unsafe { PowerSetRequest(0xDEADBEEF, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_invalid, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 6); // ERROR_INVALID_HANDLE

        // 5. CloseHandle invalidates the handle
        let close_res = unsafe { CloseHandle(handle) };
        assert_eq!(close_res, 1);
        let res_after_close = unsafe { PowerSetRequest(handle, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_after_close, 0);

        // 6. Invalid context rejection on create
        let invalid_ctx = ReasonContext {
            version: 999, // Invalid version
            flags: 0,
            reason: ReasonContextUnion {
                simple_reason_string: std::ptr::null(),
            },
        };
        let bad_handle = unsafe { PowerCreateRequest(&invalid_ctx) };
        assert_eq!(bad_handle, 0xFFFFFFFFFFFFFFFFu64);
        let null_handle = unsafe { PowerCreateRequest(std::ptr::null()) };
        assert_eq!(null_handle, 0xFFFFFFFFFFFFFFFFu64);
    }

    #[test]
    fn test_power_create_request_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*const ReasonContext) -> u64 = PowerCreateRequest;
        let ctx = ReasonContext {
            version: POWER_REQUEST_CONTEXT_VERSION,
            flags: 0,
            reason: ReasonContextUnion {
                simple_reason_string: std::ptr::null(),
            },
        };
        let h = unsafe { func_ptr(&ctx) };
        assert_ne!(h, 0xFFFFFFFFFFFFFFFFu64);
        unsafe { CloseHandle(h) };
    }

    #[test]
    fn test_module_registry_and_load_library_lifecycle() {
        // 1. LoadLibraryW for ntdll.dll
        let ntdll_u16: Vec<u16> = "ntdll.dll"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let h1 = unsafe { LoadLibraryW(ntdll_u16.as_ptr()) };
        assert_eq!(h1, 0x7FFD0000, "ntdll must return stable base 0x7FFD0000");

        // 2. Case-insensitivity & repeated load
        let ntdll_upper_u16: Vec<u16> = "NTDLL.DLL"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let h2 = unsafe { LoadLibraryW(ntdll_upper_u16.as_ptr()) };
        assert_eq!(h1, h2, "Repeated load must return same HMODULE");

        // 3. GetProcAddress scoped to ntdll
        let proc_name = std::ffi::CString::new("RtlGetVersion").unwrap();
        let fn_ptr = unsafe { GetProcAddress(h1, proc_name.as_ptr()) };
        assert_ne!(
            fn_ptr, 0,
            "GetProcAddress must resolve RtlGetVersion from ntdll"
        );

        // 4. FreeLibrary
        let free1 = unsafe { FreeLibrary(h1) };
        assert_eq!(free1, 1);
        let free2 = unsafe { FreeLibrary(h2) };
        assert_eq!(free2, 1);

        // 5. FreeLibrary on invalid handle
        let free_invalid = unsafe { FreeLibrary(0x12345678) };
        assert_eq!(free_invalid, 0);

        // 6. FreeLibrary on main EXE base must be rejected / protected
        let exe_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) as usize;
        let free_exe = unsafe { FreeLibrary(exe_base) };
        assert_eq!(free_exe, 0, "Main EXE must not be freed");
    }

    #[test]
    fn test_free_library_win64_call() {
        let func_ptr: unsafe extern "win64" fn(usize) -> i32 = FreeLibrary;
        let res = unsafe { func_ptr(0x7FFD0000) };
        assert_eq!(res, 1);
    }

    #[test]
    fn test_get_user_default_ui_language_resolver() {
        let sym = crate::pe::imports::IatSymbolResolver::resolve_symbol(
            "KERNEL32.dll",
            "GetUserDefaultUILanguage",
        );
        assert!(
            sym.is_some(),
            "KERNEL32.dll!GetUserDefaultUILanguage must be resolved"
        );
    }

    #[test]
    fn test_get_user_default_ui_language_win64_call() {
        let func_ptr: unsafe extern "win64" fn() -> u16 = GetUserDefaultUILanguage;
        let lang = unsafe { func_ptr() };
        assert_ne!(lang, 0, "LANGID must be non-zero");
    }

    #[test]
    fn test_guest_ui_language_mapping() {
        let loc_ru = GuestLocale::from_env_str("ru_RU.UTF-8");
        assert_eq!(loc_ru.lang_id, 0x0419);
        assert_eq!(loc_ru.lcid, 0x00000419);
        assert_eq!(loc_ru.locale_name, "ru-RU");

        let loc_de = GuestLocale::from_env_str("de_DE.UTF-8");
        assert_eq!(loc_de.lang_id, 0x0407);
        assert_eq!(loc_de.locale_name, "de-DE");

        let loc_fr = GuestLocale::from_env_str("fr_FR.UTF-8");
        assert_eq!(loc_fr.lang_id, 0x040c);
        assert_eq!(loc_fr.locale_name, "fr-FR");
    }

    #[test]
    fn test_guest_ui_language_fallback() {
        let loc_unknown = GuestLocale::from_env_str("unknown_LOCALE");
        assert_eq!(loc_unknown.lang_id, 0x0409);
        assert_eq!(loc_unknown.lcid, 0x00000409);
        assert_eq!(loc_unknown.locale_name, "en-US");
    }

    #[test]
    fn test_virtual_alloc_and_free() {
        unsafe {
            let addr = VirtualAlloc(0, 65536, 0x1000, 0x04);
            assert!(addr > 0x10000, "VirtualAlloc must return valid address");
            let res = VirtualFree(addr, 0, 0x8000);
            assert_eq!(res, 1, "VirtualFree must succeed");
        }
    }

    #[test]
    fn test_get_handle_information() {
        let mut flags = 0xFFFFFFFFu32;
        let res = unsafe { GetHandleInformation(0x1234, &mut flags) };
        assert_eq!(res, 1);
        assert_eq!(flags, 0);

        let set_res = unsafe { SetHandleInformation(0x1234, 1, 1) };
        assert_eq!(set_res, 1);
    }

    #[test]
    fn test_win32_to_host_path_user_mapping() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming\\Godot\\app_userdata\\test";
        let mapped = win32_to_host_path(path);
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/fizzy".to_string());
        assert!(mapped.starts_with(&home));
        assert!(mapped.ends_with("/AppData/Roaming/Godot/app_userdata/test"));
    }
}
