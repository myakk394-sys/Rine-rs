#![allow(non_snake_case)]
use std::ffi::c_void;

pub unsafe extern "win64" fn msvcrt_malloc(size: usize) -> *mut c_void {
    libc::malloc(size)
}

pub unsafe extern "win64" fn msvcrt_free(ptr: *mut c_void) {
    if !ptr.is_null() && (ptr as usize) < 0x700000000000 {
        libc::free(ptr);
    }
}

pub unsafe extern "win64" fn msvcrt_memset(dest: *mut c_void, c: i32, n: usize) -> *mut c_void {
    libc::memset(dest, c, n)
}

static G_COMMODE_PTR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
static G_FMODE_PTR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
static G_ARGC_PTR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
static G_ARGV_PTR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
static G_ERRNO_PTR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

pub unsafe extern "win64" fn __p___argc() -> *mut i32 {
    let ptr = *G_ARGC_PTR.get_or_init(|| unsafe {
        let p = libc::calloc(1, 64) as *mut i32;
        *p = 1;
        p as usize
    });
    ptr as *mut i32
}

pub unsafe extern "win64" fn __p___argv() -> *mut usize {
    let ptr = *G_ARGV_PTR.get_or_init(|| unsafe {
        let p = libc::calloc(2, std::mem::size_of::<usize>()) as *mut usize;
        p as usize
    });
    ptr as *mut usize
}

pub unsafe extern "win64" fn _errno() -> *mut i32 {
    let ptr = *G_ERRNO_PTR.get_or_init(|| unsafe { libc::calloc(1, 64) as usize });
    ptr as *mut i32
}

const TOTAL_LOCKS: usize = 256;

static mut CRT_LOCK_INIT: [bool; TOTAL_LOCKS] = [false; TOTAL_LOCKS];
static mut CRT_LOCKS: [crate::dlls::kernel32::RtlCriticalSection; TOTAL_LOCKS] =
    unsafe { std::mem::zeroed() };

#[no_mangle]
pub unsafe extern "win64" fn _lock(lock_num: i32) {
    if lock_num < 0 || lock_num as usize >= TOTAL_LOCKS {
        return;
    }
    let idx = lock_num as usize;
    if !CRT_LOCK_INIT[idx] {
        crate::dlls::kernel32::InitializeCriticalSection(&raw mut CRT_LOCKS[idx]);
        CRT_LOCK_INIT[idx] = true;
    }
    crate::dlls::kernel32::EnterCriticalSection(&raw mut CRT_LOCKS[idx]);
}

#[no_mangle]
pub unsafe extern "win64" fn _unlock(lock_num: i32) {
    if lock_num < 0 || lock_num as usize >= TOTAL_LOCKS {
        return;
    }
    let idx = lock_num as usize;
    if CRT_LOCK_INIT[idx] {
        crate::dlls::kernel32::LeaveCriticalSection(&raw mut CRT_LOCKS[idx]);
    }
}

pub unsafe extern "win64" fn __initenv() -> *mut *mut libc::c_char {
    static mut ENV_PTRS: [*mut libc::c_char; 2] = [std::ptr::null_mut(), std::ptr::null_mut()];
    &raw mut ENV_PTRS as *mut *mut libc::c_char
}

pub unsafe extern "win64" fn __p__commode() -> *mut i32 {
    let ptr = *G_COMMODE_PTR.get_or_init(|| unsafe { libc::calloc(1, 64) as usize });
    ptr as *mut i32
}

pub unsafe extern "win64" fn __p__fmode() -> *mut i32 {
    let ptr = *G_FMODE_PTR.get_or_init(|| unsafe { libc::calloc(1, 64) as usize });
    ptr as *mut i32
}

pub unsafe extern "win64" fn get_fmode_ptr() -> *mut i32 {
    __p__fmode()
}

pub unsafe extern "win64" fn get_commode_ptr() -> *mut i32 {
    __p__commode()
}

pub unsafe extern "win64" fn get_argc_ptr() -> *mut i32 {
    __p___argc()
}

pub unsafe extern "win64" fn get_argv_ptr() -> *mut usize {
    __p___argv()
}

pub unsafe extern "win64" fn get_environ_ptr() -> *mut *mut libc::c_char {
    __initenv()
}

#[repr(C)]
pub struct FILE {
    pub _ptr: *mut u8,
    pub _cnt: i32,
    pub _base: *mut u8,
    pub _flag: i32,
    pub _file: i32,
    pub _charbuf: i32,
    pub _bufsiz: i32,
    pub _tmpfname: *mut u8,
}

static mut DUMMY_IOB: [FILE; 3] = [
    FILE {
        _ptr: std::ptr::null_mut(),
        _cnt: 0,
        _base: std::ptr::null_mut(),
        _flag: 1,
        _file: 0,
        _charbuf: 0,
        _bufsiz: 0,
        _tmpfname: std::ptr::null_mut(),
    },
    FILE {
        _ptr: std::ptr::null_mut(),
        _cnt: 0,
        _base: std::ptr::null_mut(),
        _flag: 2,
        _file: 1,
        _charbuf: 0,
        _bufsiz: 0,
        _tmpfname: std::ptr::null_mut(),
    },
    FILE {
        _ptr: std::ptr::null_mut(),
        _cnt: 0,
        _base: std::ptr::null_mut(),
        _flag: 2,
        _file: 2,
        _charbuf: 0,
        _bufsiz: 0,
        _tmpfname: std::ptr::null_mut(),
    },
];

#[no_mangle]
pub unsafe extern "win64" fn __iob_func() -> *mut FILE {
    unsafe { &raw mut DUMMY_IOB as *mut FILE }
}

#[no_mangle]
pub unsafe extern "win64" fn __set_app_type(_app_type: i32) {}

#[no_mangle]
pub unsafe extern "win64" fn _amsg_exit(_err_code: i32) {}

#[no_mangle]
pub unsafe extern "win64" fn _strdup(str_ptr: *const libc::c_char) -> *mut libc::c_char {
    if str_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = std::ffi::CStr::from_ptr(str_ptr);
    let bytes = c_str.to_bytes_with_nul();
    let mem = libc::malloc(bytes.len()) as *mut u8;
    if !mem.is_null() {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), mem, bytes.len());
    }
    mem as *mut libc::c_char
}

#[no_mangle]
pub unsafe extern "win64" fn _wcsdup(str_ptr: *const u16) -> *mut u16 {
    if str_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let mut len = 0;
    while *str_ptr.add(len) != 0 {
        len += 1;
    }
    let bytes_len = (len + 1) * 2;
    let mem = libc::malloc(bytes_len) as *mut u16;
    if !mem.is_null() {
        std::ptr::copy_nonoverlapping(str_ptr, mem, len + 1);
    }
    mem
}

#[no_mangle]
pub unsafe extern "win64" fn _get_osfhandle(fd: i32) -> u64 {
    match fd {
        0 => 0x0000000000000004,
        1 => 0x0000000000000008,
        2 => 0x000000000000000C,
        _ => fd as u64 + 0x100,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn _fileno(stream: *mut std::ffi::c_void) -> i32 {
    if stream.is_null() {
        return -1;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn _fstat64(_fd: i32, buf: *mut u8) -> i32 {
    if !buf.is_null() {
        std::ptr::write_bytes(buf, 0, 128);
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn _wstat64(_path: *const u16, buf: *mut u8) -> i32 {
    if !buf.is_null() {
        std::ptr::write_bytes(buf, 0, 128);
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn vfprintf(
    _stream: *mut std::ffi::c_void,
    format: *const libc::c_char,
    _argptr: *mut u8,
) -> i32 {
    if format.is_null() {
        return -1;
    }
    let c_str = std::ffi::CStr::from_ptr(format);
    if let Ok(s) = c_str.to_str() {
        eprintln!("[GODOT STDERR/LOG] {}", s);
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn fwrite(
    ptr: *const std::ffi::c_void,
    size: usize,
    count: usize,
    _stream: *mut std::ffi::c_void,
) -> usize {
    if ptr.is_null() || size == 0 || count == 0 {
        return 0;
    }
    let total_bytes = size * count;
    let slice = std::slice::from_raw_parts(ptr as *const u8, total_bytes);
    if let Ok(s) = std::str::from_utf8(slice) {
        print!("{}", s);
    }
    count
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_fopen(
    filename: *const libc::c_char,
    mode: *const libc::c_char,
) -> *mut libc::FILE {
    if filename.is_null() || mode.is_null() {
        return std::ptr::null_mut();
    }
    let c_name = std::ffi::CStr::from_ptr(filename).to_string_lossy();
    let linux_path = c_name.replace('\\', "/");
    let c_mode = std::ffi::CStr::from_ptr(mode);
    println!("[MSVCRT FILE] fopen: '{}'", linux_path);
    if let Ok(c_path) = std::ffi::CString::new(linux_path.as_bytes()) {
        libc::fopen(c_path.as_ptr(), c_mode.as_ptr())
    } else {
        std::ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_wfopen(
    filename: *const u16,
    mode: *const u16,
) -> *mut libc::FILE {
    if filename.is_null() || mode.is_null() {
        return std::ptr::null_mut();
    }
    let mut len = 0usize;
    while *filename.add(len) != 0 {
        len += 1;
    }
    let win_path = String::from_utf16_lossy(std::slice::from_raw_parts(filename, len));
    let linux_path = win_path.replace('\\', "/");

    let mut mlen = 0usize;
    while *mode.add(mlen) != 0 {
        mlen += 1;
    }
    let mode_str = String::from_utf16_lossy(std::slice::from_raw_parts(mode, mlen));

    println!(
        "[MSVCRT FILE] _wfopen: '{}' (mode '{}')",
        linux_path, mode_str
    );

    let c_path = match std::ffi::CString::new(linux_path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };
    let c_mode = match std::ffi::CString::new(mode_str.as_bytes()) {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };

    libc::fopen(c_path.as_ptr(), c_mode.as_ptr())
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_fclose(stream: *mut libc::FILE) -> i32 {
    if !stream.is_null() {
        libc::fclose(stream)
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_fread(
    ptr: *mut libc::c_void,
    size: usize,
    count: usize,
    stream: *mut libc::FILE,
) -> usize {
    if stream.is_null() || ptr.is_null() || size == 0 || count == 0 {
        return 0;
    }
    libc::fread(ptr, size, count, stream)
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_fseek(
    stream: *mut libc::FILE,
    offset: i64,
    origin: i32,
) -> i32 {
    if stream.is_null() {
        return -1;
    }
    libc::fseek(stream, offset as libc::c_long, origin)
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_ftell(stream: *mut libc::FILE) -> i64 {
    if stream.is_null() {
        return -1;
    }
    libc::ftell(stream) as i64
}

#[no_mangle]
pub unsafe extern "win64" fn fflush(_stream: *mut std::ffi::c_void) -> i32 {
    0
}

#[repr(C)]
pub struct OnExitTable {
    pub first: u64,
    pub last: u64,
    pub end: u64,
}

pub unsafe extern "win64" fn _initialize_onexit_table(table: *mut OnExitTable) -> i32 {
    if !table.is_null() {
        let buf = libc::mmap(
            std::ptr::null_mut(),
            4096,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        ) as u64;
        (*table).first = buf;
        (*table).last = buf;
        (*table).end = buf + 4096;
    }
    0
}

pub unsafe extern "win64" fn _register_onexit_function(
    table: *mut OnExitTable,
    fn_ptr: u64,
) -> i32 {
    if !table.is_null() && (*table).last < (*table).end {
        let slot = (*table).last as *mut u64;
        *slot = fn_ptr;
        (*table).last += 8;
    }
    0
}

pub unsafe extern "win64" fn _execute_onexit_table(table: *mut OnExitTable) -> i32 {
    if !table.is_null() {
        let mut p = (*table).first as *const u64;
        let limit = (*table).last as *const u64;
        while p < limit {
            let fn_ptr = *p;
            if fn_ptr > 0x10000 {
                let func: unsafe extern "win64" fn() = std::mem::transmute(fn_ptr);
                func();
            }
            p = p.add(1);
        }
    }
    0
}

pub unsafe extern "win64" fn _crt_atexit(_fn_ptr: u64) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_exit(status: i32) -> ! {
    eprintln!(
        "[CRT EXIT] MSVCRT!exit called with status={} (0x{:08X})",
        status, status
    );
    eprintln!("[GUEST EXITED] MSVCRT!exit invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(status);
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_exit_underscore(status: i32) -> ! {
    eprintln!(
        "[CRT EXIT] MSVCRT!_exit called with status={} (0x{:08X})",
        status, status
    );
    eprintln!("[GUEST EXITED] MSVCRT!_exit invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(status);
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_abort() -> ! {
    eprintln!("[CRT ABORT] MSVCRT!abort called");
    eprintln!("[GUEST EXITED] MSVCRT!abort invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(3);
}

#[no_mangle]
pub unsafe extern "win64" fn _cexit() {
    println!("[CRT EXIT] MSVCRT!_cexit called");
}

#[no_mangle]
pub unsafe extern "win64" fn _c_exit() {
    println!("[CRT EXIT] MSVCRT!_c_exit called");
}

#[no_mangle]
pub unsafe extern "win64" fn quick_exit(status: i32) -> ! {
    eprintln!("[CRT EXIT] UCRT!quick_exit called with status={}", status);
    eprintln!("[GUEST EXITED] UCRT!quick_exit invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(status);
}

pub type Pvfv = unsafe extern "win64" fn();
pub type Pifv = unsafe extern "win64" fn() -> i32;

pub unsafe extern "win64" fn _initterm(start: u64, end: u64) {
    if start == 0 || end == 0 || start >= end || (end - start) % 8 != 0 {
        println!(
            "[CRT INITTERM REJECTED] Invalid range: start=0x{:016X}, end=0x{:016X}",
            start, end
        );
        return;
    }
    let count = (end - start) / 8;
    println!(
        "[CRT INITTERM ENTER] first=0x{:016X}, last=0x{:016X}, count={}",
        start, end, count
    );

    let mut p = start as *const u64;
    let limit = end as *const u64;
    let mut idx = 0;
    while p < limit {
        let fn_ptr = *p;
        println!(
            "[CRT INITTERM ENTRY] index={} slot_address=0x{:016X} callback_value=0x{:016X}",
            idx, p as usize, fn_ptr
        );
        if fn_ptr != 0 {
            if fn_ptr < 0x10000 {
                println!(
                    "[CRT INITTERM SKIP] Invalid low callback pointer 0x{:016X}",
                    fn_ptr
                );
            } else {
                let func: Pvfv = std::mem::transmute(fn_ptr);
                println!(
                    "[CRT INITTERM EXEC] Executing static initializer #{} @ 0x{:016X}",
                    idx, fn_ptr
                );
                func();
                println!(
                    "[CRT INITTERM SUCCESS] Static initializer #{} @ 0x{:016X} completed",
                    idx, fn_ptr
                );
            }
        } else {
            println!("[CRT INITTERM SKIP] Skipping NULL callback entry #{}", idx);
        }
        p = p.add(1);
        idx += 1;
    }
}

pub unsafe extern "win64" fn _initterm_e(start: u64, end: u64) -> i32 {
    if start == 0 || end == 0 || start >= end || (end - start) % 8 != 0 {
        println!(
            "[CRT INITTERM_E REJECTED] Invalid range: start=0x{:016X}, end=0x{:016X}",
            start, end
        );
        return 0;
    }
    let count = (end - start) / 8;
    println!(
        "[CRT INITTERM_E ENTER] first=0x{:016X}, last=0x{:016X}, count={}",
        start, end, count
    );

    let mut p = start as *const u64;
    let limit = end as *const u64;
    let mut idx = 0;
    while p < limit {
        let fn_ptr = *p;
        println!(
            "[CRT INITTERM_E ENTRY] index={} slot_address=0x{:016X} callback_value=0x{:016X}",
            idx, p as usize, fn_ptr
        );
        if fn_ptr != 0 {
            if fn_ptr < 0x10000 {
                println!(
                    "[CRT INITTERM_E SKIP] Invalid low callback pointer 0x{:016X}",
                    fn_ptr
                );
            } else {
                let func: Pifv = std::mem::transmute(fn_ptr);
                println!(
                    "[CRT INITTERM_E EXEC] Executing static initializer #{} @ 0x{:016X}",
                    idx, fn_ptr
                );
                let res = func();
                println!(
                    "[CRT INITTERM_E RETURN] Static initializer #{} @ 0x{:016X} returned {}",
                    idx, fn_ptr, res
                );
                if res != 0 {
                    println!(
                        "[CRT INITTERM_E STOP] Early exit due to non-zero status {}",
                        res
                    );
                    return res;
                }
            }
        } else {
            println!(
                "[CRT INITTERM_E SKIP] Skipping NULL callback entry #{}",
                idx
            );
        }
        p = p.add(1);
        idx += 1;
    }
    0
}

type Win64Initterm = unsafe extern "win64" fn(u64, u64);
type Win64InittermE = unsafe extern "win64" fn(u64, u64) -> i32;
type Win64Memcpy = unsafe extern "win64" fn(*mut c_void, *const c_void, usize) -> *mut c_void;
type Win64Memcmp = unsafe extern "win64" fn(*const c_void, *const c_void, usize) -> i32;
type Win64Memchr = unsafe extern "win64" fn(*const c_void, i32, usize) -> *mut c_void;
type Win64Strlen = unsafe extern "win64" fn(*const libc::c_char) -> usize;
type Win64Wcslen = unsafe extern "win64" fn(*const u16) -> usize;

const _: Win64Initterm = _initterm;
const _: Win64InittermE = _initterm_e;
const _: Win64Memcpy = msvcrt_memcpy;
const _: Win64Memcmp = msvcrt_memcmp;
const _: Win64Memchr = msvcrt_memchr;
const _: Win64Strlen = msvcrt_strlen;
const _: Win64Wcslen = msvcrt_wcslen;

pub unsafe extern "win64" fn _configthreadlocale(_flag: i32) -> i32 {
    0
}

pub unsafe extern "win64" fn _set_fmode(_mode: i32) -> i32 {
    0
}

pub unsafe extern "win64" fn msvcrt_memcpy(
    dest: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    libc::memcpy(dest, src, n)
}

pub unsafe extern "win64" fn msvcrt_memchr(s: *const c_void, c: i32, n: usize) -> *mut c_void {
    if s.is_null() || n == 0 {
        std::ptr::null_mut()
    } else {
        libc::memchr(s, c, n)
    }
}

pub unsafe extern "win64" fn msvcrt_memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    libc::memcmp(s1, s2, n)
}

pub unsafe extern "win64" fn msvcrt_strlen(s: *const libc::c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    libc::strlen(s)
}

pub unsafe extern "win64" fn msvcrt_wcslen(s: *const u16) -> usize {
    let mut len = 0;
    let mut ptr = s;
    while !ptr.is_null() && *ptr != 0 {
        len += 1;
        ptr = ptr.add(1);
    }
    len
}

pub unsafe extern "win64" fn msvcrt_calloc(nmemb: usize, size: usize) -> *mut c_void {
    libc::calloc(nmemb, size)
}

pub unsafe extern "win64" fn msvcrt_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() || (ptr as usize) < 0x700000000000 {
        return libc::realloc(ptr, size);
    }
    std::ptr::null_mut()
}

pub unsafe extern "win64" fn msvcrt_aligned_malloc(size: usize, alignment: usize) -> *mut c_void {
    let mut ptr: *mut libc::c_void = std::ptr::null_mut();
    let align = if alignment < 8 { 8 } else { alignment };
    if libc::posix_memalign(&mut ptr, align, size) == 0 {
        ptr
    } else {
        std::ptr::null_mut()
    }
}

pub unsafe extern "win64" fn msvcrt_aligned_free(ptr: *mut c_void) {
    msvcrt_free(ptr)
}

pub unsafe extern "win64" fn msvcrt_aligned_realloc(
    ptr: *mut c_void,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    let new_ptr = msvcrt_aligned_malloc(size, alignment);
    if !new_ptr.is_null() && !ptr.is_null() {
        libc::memcpy(new_ptr, ptr, size);
        msvcrt_free(ptr);
    }
    new_ptr
}

pub unsafe extern "win64" fn msvcrt_msize(ptr: *mut c_void) -> usize {
    if ptr.is_null() {
        0
    } else {
        libc::malloc_usable_size(ptr)
    }
}

pub unsafe extern "win64" fn msvcrt_memmove(
    dest: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    libc::memmove(dest, src, n)
}

pub unsafe extern "win64" fn msvcrt_strcmp(
    s1: *const libc::c_char,
    s2: *const libc::c_char,
) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    libc::strcmp(s1, s2)
}

pub unsafe extern "win64" fn msvcrt_strncmp(
    s1: *const libc::c_char,
    s2: *const libc::c_char,
    n: usize,
) -> i32 {
    if s1.is_null() || s2.is_null() || n == 0 {
        return 0;
    }
    libc::strncmp(s1, s2, n)
}

pub unsafe extern "win64" fn msvcrt_strcpy(
    dest: *mut libc::c_char,
    src: *const libc::c_char,
) -> *mut libc::c_char {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    libc::strcpy(dest, src)
}

pub unsafe extern "win64" fn msvcrt_strncpy(
    dest: *mut libc::c_char,
    src: *const libc::c_char,
    n: usize,
) -> *mut libc::c_char {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }
    libc::strncpy(dest, src, n)
}

pub unsafe extern "win64" fn msvcrt_strcat(
    dest: *mut libc::c_char,
    src: *const libc::c_char,
) -> *mut libc::c_char {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    libc::strcat(dest, src)
}

pub unsafe extern "win64" fn msvcrt_strchr(s: *const libc::c_char, c: i32) -> *mut libc::c_char {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    libc::strchr(s, c)
}

pub unsafe extern "win64" fn msvcrt_strrchr(s: *const libc::c_char, c: i32) -> *mut libc::c_char {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    libc::strrchr(s, c)
}

pub unsafe extern "win64" fn msvcrt_strstr(
    haystack: *const libc::c_char,
    needle: *const libc::c_char,
) -> *mut libc::c_char {
    if haystack.is_null() || needle.is_null() {
        return std::ptr::null_mut();
    }
    libc::strstr(haystack, needle)
}

pub unsafe extern "win64" fn msvcrt_stricmp(
    s1: *const libc::c_char,
    s2: *const libc::c_char,
) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    libc::strcasecmp(s1, s2)
}

pub unsafe extern "win64" fn msvcrt_strnicmp(
    s1: *const libc::c_char,
    s2: *const libc::c_char,
    n: usize,
) -> i32 {
    if s1.is_null() || s2.is_null() || n == 0 {
        return 0;
    }
    libc::strncasecmp(s1, s2, n)
}

pub unsafe extern "win64" fn msvcrt_sprintf_stub() -> i32 {
    0
}
pub unsafe extern "win64" fn msvcrt_sscanf_stub() -> i32 {
    0
}

pub unsafe extern "win64" fn msvcrt_wcscmp(s1: *const u16, s2: *const u16) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    let mut p1 = s1;
    let mut p2 = s2;
    while *p1 != 0 && *p1 == *p2 {
        p1 = p1.add(1);
        p2 = p2.add(1);
    }
    (*p1 as i32) - (*p2 as i32)
}

pub unsafe extern "win64" fn msvcrt_wcsncmp(s1: *const u16, s2: *const u16, n: usize) -> i32 {
    if s1.is_null() || s2.is_null() || n == 0 {
        return 0;
    }
    for i in 0..n {
        let c1 = *s1.add(i);
        let c2 = *s2.add(i);
        if c1 != c2 || c1 == 0 {
            return (c1 as i32) - (c2 as i32);
        }
    }
    0
}

pub unsafe extern "win64" fn msvcrt_wcscpy(dest: *mut u16, src: *const u16) -> *mut u16 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    let mut i = 0;
    loop {
        let c = *src.add(i);
        *dest.add(i) = c;
        if c == 0 {
            break;
        }
        i += 1;
    }
    dest
}

pub unsafe extern "win64" fn msvcrt_wcsncpy(dest: *mut u16, src: *const u16, n: usize) -> *mut u16 {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }
    for i in 0..n {
        let c = *src.add(i);
        *dest.add(i) = c;
        if c == 0 {
            break;
        }
    }
    dest
}

pub unsafe extern "win64" fn msvcrt_wcscat(dest: *mut u16, src: *const u16) -> *mut u16 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    let len = msvcrt_wcslen(dest);
    msvcrt_wcscpy(dest.add(len), src);
    dest
}

pub unsafe extern "win64" fn msvcrt_wcschr(s: *const u16, c: u16) -> *mut u16 {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    let mut p = s;
    while *p != 0 {
        if *p == c {
            return p as *mut u16;
        }
        p = p.add(1);
    }
    if c == 0 {
        p as *mut u16
    } else {
        std::ptr::null_mut()
    }
}

pub unsafe extern "win64" fn msvcrt_wcsrchr(s: *const u16, c: u16) -> *mut u16 {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    let len = msvcrt_wcslen(s);
    let mut p = s.add(len);
    loop {
        if *p == c {
            return p as *mut u16;
        }
        if p == s {
            break;
        }
        p = p.sub(1);
    }
    std::ptr::null_mut()
}

pub unsafe extern "win64" fn msvcrt_wcsicmp(s1: *const u16, s2: *const u16) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    let mut p1 = s1;
    let mut p2 = s2;
    let to_lower = |c: u16| -> i32 {
        if c >= 0x41 && c <= 0x5A {
            (c + 0x20) as i32
        } else {
            c as i32
        }
    };
    while *p1 != 0 {
        let c1 = to_lower(*p1);
        let c2 = to_lower(*p2);
        if c1 != c2 {
            return c1 - c2;
        }
        p1 = p1.add(1);
        p2 = p2.add(1);
    }
    to_lower(*p1) - to_lower(*p2)
}

pub unsafe extern "win64" fn msvcrt_towlower(c: u16) -> u16 {
    (c as u8).to_ascii_lowercase() as u16
}

pub unsafe extern "win64" fn msvcrt_towupper(c: u16) -> u16 {
    (c as u8).to_ascii_uppercase() as u16
}

pub unsafe extern "win64" fn msvcrt_tolower(c: i32) -> i32 {
    libc::tolower(c)
}

pub unsafe extern "win64" fn msvcrt_toupper(c: i32) -> i32 {
    libc::toupper(c)
}

pub unsafe extern "win64" fn msvcrt_isdigit(c: i32) -> i32 {
    libc::isdigit(c)
}

pub unsafe extern "win64" fn msvcrt_isalpha(c: i32) -> i32 {
    libc::isalpha(c)
}

pub unsafe extern "win64" fn msvcrt_isspace(c: i32) -> i32 {
    libc::isspace(c)
}

pub unsafe extern "win64" fn msvcrt_iswdigit(c: u16) -> i32 {
    if c >= 0x30 && c <= 0x39 {
        1
    } else {
        0
    }
}

pub unsafe extern "win64" fn msvcrt_strtoul(
    nptr: *const libc::c_char,
    endptr: *mut *mut libc::c_char,
    base: i32,
) -> libc::c_ulong {
    if nptr.is_null() {
        return 0;
    }
    libc::strtoul(nptr, endptr, base)
}

pub unsafe extern "win64" fn msvcrt_strtol(
    nptr: *const libc::c_char,
    endptr: *mut *mut libc::c_char,
    base: i32,
) -> libc::c_long {
    if nptr.is_null() {
        return 0;
    }
    libc::strtol(nptr, endptr, base)
}

pub unsafe extern "win64" fn msvcrt_strtod(
    nptr: *const libc::c_char,
    endptr: *mut *mut libc::c_char,
) -> libc::c_double {
    if nptr.is_null() {
        return 0.0;
    }
    libc::strtod(nptr, endptr)
}

pub unsafe extern "win64" fn msvcrt_atoi(nptr: *const libc::c_char) -> i32 {
    if nptr.is_null() {
        return 0;
    }
    libc::atoi(nptr)
}

pub unsafe extern "win64" fn msvcrt_atof(nptr: *const libc::c_char) -> f64 {
    if nptr.is_null() {
        return 0.0;
    }
    libc::atof(nptr)
}

pub unsafe extern "win64" fn msvcrt_strtoi64(
    nptr: *const libc::c_char,
    endptr: *mut *mut libc::c_char,
    base: i32,
) -> i64 {
    if nptr.is_null() {
        return 0;
    }
    libc::strtoll(nptr, endptr, base) as i64
}

pub unsafe extern "win64" fn msvcrt_strtoui64(
    nptr: *const libc::c_char,
    endptr: *mut *mut libc::c_char,
    base: i32,
) -> u64 {
    if nptr.is_null() {
        return 0;
    }
    libc::strtoull(nptr, endptr, base) as u64
}

pub unsafe extern "win64" fn msvcrt_btowc(c: i32) -> u32 {
    if c == -1 {
        0xFFFFFFFF
    } else {
        c as u32
    }
}

pub unsafe extern "win64" fn msvcrt_wctob(c: u32) -> i32 {
    if c > 0x7F {
        -1
    } else {
        c as i32
    }
}

pub unsafe extern "win64" fn msvcrt_mbrtowc(
    pwc: *mut u16,
    s: *const libc::c_char,
    _n: usize,
    _ps: *mut c_void,
) -> usize {
    if s.is_null() {
        return 0;
    }
    let val = (*s as u8) as u16;
    if !pwc.is_null() {
        *pwc = val;
    }
    if val == 0 {
        0
    } else {
        1
    }
}

pub unsafe extern "win64" fn msvcrt_wcrtomb(
    s: *mut libc::c_char,
    wc: u16,
    _ps: *mut c_void,
) -> usize {
    if s.is_null() {
        return 1;
    }
    *s = (wc as u8) as libc::c_char;
    1
}

pub unsafe extern "win64" fn msvcrt_mbsrtowcs(
    dst: *mut u16,
    src: *const *const libc::c_char,
    len: usize,
    _ps: *mut c_void,
) -> usize {
    if src.is_null() || (*src).is_null() {
        return 0;
    }
    let mut count = 0;
    let s = *src;
    while count < len && *s.add(count) != 0 {
        if !dst.is_null() {
            *dst.add(count) = (*s.add(count) as u8) as u16;
        }
        count += 1;
    }
    if !dst.is_null() && count < len {
        *dst.add(count) = 0;
    }
    count
}

pub unsafe extern "win64" fn msvcrt_getenv(name: *const libc::c_char) -> *mut libc::c_char {
    if name.is_null() {
        return std::ptr::null_mut();
    }
    libc::getenv(name)
}

pub unsafe extern "win64" fn msvcrt_wgetenv(_name: *const u16) -> *mut u16 {
    std::ptr::null_mut()
}

pub unsafe extern "win64" fn msvcrt_putenv_s(
    name: *const libc::c_char,
    val: *const libc::c_char,
) -> i32 {
    if name.is_null() || val.is_null() {
        return -1;
    }
    libc::setenv(name, val, 1)
}

pub unsafe extern "win64" fn msvcrt_setlocale(
    _category: i32,
    locale: *const libc::c_char,
) -> *mut libc::c_char {
    if locale.is_null() {
        return std::ptr::null_mut();
    }
    locale as *mut libc::c_char
}

pub unsafe extern "win64" fn msvcrt_localeconv() -> *mut c_void {
    libc::localeconv() as *mut c_void
}

pub unsafe extern "win64" fn msvcrt_mb_cur_max_func() -> i32 {
    1
}

pub unsafe extern "win64" fn msvcrt_feof(stream: *mut libc::FILE) -> i32 {
    if stream.is_null() {
        1
    } else {
        libc::feof(stream)
    }
}

pub unsafe extern "win64" fn msvcrt_ferror(stream: *mut libc::FILE) -> i32 {
    if stream.is_null() {
        1
    } else {
        libc::ferror(stream)
    }
}

pub unsafe extern "win64" fn msvcrt_remove(path: *const libc::c_char) -> i32 {
    if path.is_null() {
        -1
    } else {
        libc::remove(path)
    }
}

pub unsafe extern "win64" fn msvcrt_lock_file(_file: *mut libc::FILE) {}
pub unsafe extern "win64" fn msvcrt_unlock_file(_file: *mut libc::FILE) {}

pub unsafe extern "win64" fn msvcrt_fputs(s: *const libc::c_char, stream: *mut libc::FILE) -> i32 {
    if s.is_null() || stream.is_null() {
        -1
    } else {
        libc::fputs(s, stream)
    }
}

pub unsafe extern "win64" fn msvcrt_fgets(
    s: *mut libc::c_char,
    size: i32,
    stream: *mut libc::FILE,
) -> *mut libc::c_char {
    if s.is_null() || stream.is_null() || size <= 0 {
        std::ptr::null_mut()
    } else {
        libc::fgets(s, size, stream)
    }
}

pub unsafe extern "win64" fn msvcrt_fputc(c: i32, stream: *mut libc::FILE) -> i32 {
    if stream.is_null() {
        -1
    } else {
        libc::fputc(c, stream)
    }
}

pub unsafe extern "win64" fn msvcrt_fgetc(stream: *mut libc::FILE) -> i32 {
    if stream.is_null() {
        -1
    } else {
        libc::fgetc(stream)
    }
}

pub unsafe extern "win64" fn msvcrt_ungetc(c: i32, stream: *mut libc::FILE) -> i32 {
    if stream.is_null() {
        -1
    } else {
        libc::ungetc(c, stream)
    }
}

pub unsafe extern "win64" fn msvcrt_strftime(
    s: *mut libc::c_char,
    maxsize: usize,
    format: *const libc::c_char,
    _timeptr: *const c_void,
) -> usize {
    if s.is_null() || format.is_null() || maxsize == 0 {
        return 0;
    }
    *s = 0;
    0
}

pub unsafe extern "win64" fn msvcrt_wcsftime(
    s: *mut u16,
    maxsize: usize,
    format: *const u16,
    _timeptr: *const c_void,
) -> usize {
    if s.is_null() || format.is_null() || maxsize == 0 {
        return 0;
    }
    *s = 0;
    0
}

pub unsafe extern "win64" fn msvcrt_time(timer: *mut i64) -> i64 {
    let t = libc::time(std::ptr::null_mut()) as i64;
    if !timer.is_null() {
        *timer = t;
    }
    t
}

pub unsafe extern "win64" fn msvcrt_clock() -> libc::clock_t {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &raw mut ts);
    ((ts.tv_sec * 1000) + (ts.tv_nsec / 1_000_000)) as libc::clock_t
}

pub unsafe extern "win64" fn msvcrt_localtime(_timer: *const i64) -> *mut c_void {
    static mut LOCAL_TM: libc::tm = unsafe { std::mem::zeroed() };
    &raw mut LOCAL_TM as *mut _ as *mut c_void
}

pub unsafe extern "win64" fn msvcrt_gmtime(_timer: *const i64) -> *mut c_void {
    static mut GM_TM: libc::tm = unsafe { std::mem::zeroed() };
    &raw mut GM_TM as *mut _ as *mut c_void
}

pub unsafe extern "win64" fn msvcrt_rand_s(val: *mut u32) -> i32 {
    if !val.is_null() {
        *val = libc::rand() as u32;
    }
    0
}

pub unsafe extern "win64" fn msvcrt_rand() -> i32 {
    libc::rand()
}

pub unsafe extern "win64" fn msvcrt_srand(seed: u32) {
    libc::srand(seed as libc::c_uint)
}

pub unsafe extern "win64" fn msvcrt_qsort(
    base: *mut c_void,
    num: usize,
    size: usize,
    compar: unsafe extern "win64" fn(*const c_void, *const c_void) -> i32,
) {
    if base.is_null() || num <= 1 || size == 0 {
        return;
    }
    let buf = libc::malloc(size);
    if buf.is_null() {
        return;
    }
    for i in 1..num {
        let mut j = i;
        while j > 0 {
            let p1 = (base as *mut u8).add((j - 1) * size) as *const c_void;
            let p2 = (base as *mut u8).add(j * size) as *const c_void;
            if compar(p1, p2) > 0 {
                libc::memcpy(buf, p1, size);
                libc::memcpy(p1 as *mut c_void, p2, size);
                libc::memcpy(p2 as *mut c_void, buf, size);
                j -= 1;
            } else {
                break;
            }
        }
    }
    libc::free(buf);
}

pub unsafe extern "win64" fn msvcrt_bsearch(
    key: *const c_void,
    base: *const c_void,
    num: usize,
    size: usize,
    compar: unsafe extern "win64" fn(*const c_void, *const c_void) -> i32,
) -> *mut c_void {
    if key.is_null() || base.is_null() || num == 0 || size == 0 {
        return std::ptr::null_mut();
    }
    let mut low = 0;
    let mut high = num;
    while low < high {
        let mid = low + (high - low) / 2;
        let mid_ptr = (base as *const u8).add(mid * size) as *const c_void;
        let res = compar(key, mid_ptr);
        if res == 0 {
            return mid_ptr as *mut c_void;
        } else if res < 0 {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    std::ptr::null_mut()
}

pub unsafe extern "win64" fn msvcrt_abs(j: i32) -> i32 {
    j.abs()
}

pub unsafe extern "win64" fn msvcrt_longjmp_stub(_env: *const c_void, _val: i32) {
    println!("[CRT STUB] msvcrt_longjmp_stub called");
}

pub unsafe extern "win64" fn msvcrt_setjmp_stub(_env: *mut c_void) -> i32 {
    0
}

pub unsafe extern "win64" fn msvcrt_cxx_frame_handler() -> i32 {
    0
}

pub unsafe extern "win64" fn msvcrt_cxx_throw_exception(
    _p_exception_object: *mut c_void,
    _p_throw_info: *const c_void,
) {
    println!("[CRT STUB] _CxxThrowException called");
}

pub unsafe extern "win64" fn msvcrt_sqrt(x: f64) -> f64 {
    x.sqrt()
}
pub unsafe extern "win64" fn msvcrt_sin(x: f64) -> f64 {
    x.sin()
}
pub unsafe extern "win64" fn msvcrt_cos(x: f64) -> f64 {
    x.cos()
}
pub unsafe extern "win64" fn msvcrt_tan(x: f64) -> f64 {
    x.tan()
}
pub unsafe extern "win64" fn msvcrt_atan2(y: f64, x: f64) -> f64 {
    y.atan2(x)
}
pub unsafe extern "win64" fn msvcrt_pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}
pub unsafe extern "win64" fn msvcrt_log(x: f64) -> f64 {
    x.ln()
}
pub unsafe extern "win64" fn msvcrt_log2(x: f64) -> f64 {
    x.log2()
}
pub unsafe extern "win64" fn msvcrt_log10(x: f64) -> f64 {
    x.log10()
}
pub unsafe extern "win64" fn msvcrt_exp(x: f64) -> f64 {
    x.exp()
}
pub unsafe extern "win64" fn msvcrt_floor(x: f64) -> f64 {
    x.floor()
}
pub unsafe extern "win64" fn msvcrt_ceil(x: f64) -> f64 {
    x.ceil()
}
pub unsafe extern "win64" fn msvcrt_fabs(x: f64) -> f64 {
    x.abs()
}
pub unsafe extern "win64" fn msvcrt_fmod(x: f64, y: f64) -> f64 {
    x % y
}
pub unsafe extern "win64" fn msvcrt_round(x: f64) -> f64 {
    x.round()
}
pub unsafe extern "win64" fn msvcrt_trunc(x: f64) -> f64 {
    x.trunc()
}
pub unsafe extern "win64" fn msvcrt_ldexp(x: f64, exp: i32) -> f64 {
    x * (2.0_f64).powi(exp)
}
pub unsafe extern "win64" fn msvcrt_frexp(x: f64, exp: *mut i32) -> f64 {
    if !exp.is_null() {
        *exp = 0;
    }
    x
}
pub unsafe extern "win64" fn msvcrt_generic_stub() -> usize {
    0
}

mod tests {
    use super::*;

    #[test]
    fn test_crt_memory_management() {
        unsafe {
            let ptr = msvcrt_malloc(64);
            assert!(!ptr.is_null());

            msvcrt_memset(ptr, 0xAB, 64);
            let slice = std::slice::from_raw_parts(ptr as *const u8, 64);
            assert!(slice.iter().all(|&b| b == 0xAB));

            msvcrt_free(ptr);
        }
    }

    #[test]
    fn test_msvcrt_strlen_abi() {
        unsafe {
            let f: unsafe extern "win64" fn(*const libc::c_char) -> usize = msvcrt_strlen;
            let cstr = b"Hello Rine\0";
            let len = f(cstr.as_ptr() as *const libc::c_char);
            assert_eq!(len, 10);

            let null_len = f(std::ptr::null());
            assert_eq!(null_len, 0);
        }
    }

    static mut TEST_COUNTER: i32 = 0;
    static mut TEST_ORDER: Vec<i32> = Vec::new();

    unsafe extern "win64" fn dummy_cb_1() {
        TEST_COUNTER += 1;
        TEST_ORDER.push(1);
    }

    unsafe extern "win64" fn dummy_cb_2() {
        TEST_COUNTER += 10;
        TEST_ORDER.push(2);
    }

    unsafe extern "win64" fn dummy_cbe_ok() -> i32 {
        0
    }

    unsafe extern "win64" fn dummy_cbe_err() -> i32 {
        42
    }

    #[test]
    fn test_initterm_null_skipping_and_order() {
        unsafe {
            TEST_COUNTER = 0;
            TEST_ORDER.clear();

            let callbacks: [u64; 4] = [
                0, // NULL sentinel 0 (skipped)
                dummy_cb_1 as *const () as u64,
                0, // NULL sentinel 1 (skipped)
                dummy_cb_2 as *const () as u64,
            ];

            let start = callbacks.as_ptr() as u64;
            let end = start + 32;

            let f: unsafe extern "win64" fn(u64, u64) = _initterm;
            f(start, end);

            assert_eq!(TEST_COUNTER, 11);
            assert_eq!(TEST_ORDER, vec![1, 2]);
        }
    }

    #[test]
    fn test_initterm_e_early_exit_and_success() {
        unsafe {
            let ok_table: [u64; 2] = [
                dummy_cbe_ok as *const () as u64,
                dummy_cbe_ok as *const () as u64,
            ];
            let fe: unsafe extern "win64" fn(u64, u64) -> i32 = _initterm_e;
            let res1 = fe(ok_table.as_ptr() as u64, ok_table.as_ptr() as u64 + 16);
            assert_eq!(res1, 0);

            let err_table: [u64; 3] = [
                dummy_cbe_ok as *const () as u64,
                dummy_cbe_err as *const () as u64,
                dummy_cbe_ok as *const () as u64,
            ];
            let res2 = fe(err_table.as_ptr() as u64, err_table.as_ptr() as u64 + 24);
            assert_eq!(res2, 42);
        }
    }

    #[test]
    fn test_initterm_invalid_range_rejection() {
        unsafe {
            let f: unsafe extern "win64" fn(u64, u64) = _initterm;
            f(0, 0);
            f(0x1000, 0x1000);
            f(0x2000, 0x1000);
            f(0x1000, 0x1005); // not multiple of 8

            let fe: unsafe extern "win64" fn(u64, u64) -> i32 = _initterm_e;
            assert_eq!(fe(0, 0), 0);
            assert_eq!(fe(0x1000, 0x1005), 0);
        }
    }
}
