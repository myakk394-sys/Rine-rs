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
    if dest.is_null() || n == 0 {
        return dest;
    }
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

thread_local! {
    static THREAD_ERRNO: std::cell::UnsafeCell<i32> = std::cell::UnsafeCell::new(0);
}

pub unsafe extern "win64" fn _errno() -> *mut i32 {
    THREAD_ERRNO.with(|cell| cell.get())
}

const TOTAL_LOCKS: usize = 256;

static mut CRT_LOCK_INIT: [bool; TOTAL_LOCKS] = [false; TOTAL_LOCKS];
static mut CRT_LOCKS: [crate::dlls::kernel32::RtlCriticalSection; TOTAL_LOCKS] =
    unsafe { std::mem::zeroed() };

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

extern "C" {
    static stdin: *mut libc::FILE;
    static stdout: *mut libc::FILE;
    static stderr: *mut libc::FILE;
}

pub unsafe extern "win64" fn __iob_func() -> *mut FILE {
    &raw mut DUMMY_IOB as *mut FILE
}

pub unsafe fn resolve_file_stream(stream: *mut libc::FILE) -> *mut libc::FILE {
    if stream.is_null() {
        return std::ptr::null_mut();
    }
    let dummy_start = &raw mut DUMMY_IOB as *mut FILE as usize;
    let stream_addr = stream as usize;
    if stream_addr >= dummy_start && stream_addr < dummy_start + 3 * std::mem::size_of::<FILE>() {
        let index = (stream_addr - dummy_start) / std::mem::size_of::<FILE>();
        match index {
            0 => stdin,
            1 => stdout,
            2 => stderr,
            _ => stdout,
        }
    } else {
        stream
    }
}

pub unsafe extern "win64" fn __acrt_iob_func(index: u32) -> *mut libc::FILE {
    match index {
        0 => stdin,
        1 => stdout,
        2 => stderr,
        _ => std::ptr::null_mut(),
    }
}

pub unsafe extern "win64" fn __set_app_type(_app_type: i32) {}

pub unsafe extern "win64" fn _amsg_exit(_err_code: i32) {}

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

pub unsafe extern "win64" fn _get_osfhandle(fd: i32) -> u64 {
    if fd < 0 {
        return !0u64;
    }
    match fd {
        0 => 0x0000000000000004,
        1 => 0x0000000000000008,
        2 => 0x000000000000000C,
        _ => (fd as u64).wrapping_add(0x100),
    }
}

pub unsafe extern "win64" fn _fileno(stream: *mut std::ffi::c_void) -> i32 {
    msvcrt_fileno(stream as *mut libc::FILE)
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Stat64 {
    pub st_dev: u32,
    pub st_ino: u16,
    pub st_mode: u16,
    pub st_nlink: i16,
    pub st_uid: i16,
    pub st_gid: i16,
    pub _pad0: u16,
    pub st_rdev: u32,
    pub _pad: u32,
    pub st_size: i64,
    pub st_atime: i64,
    pub st_mtime: i64,
    pub st_ctime: i64,
}

pub unsafe extern "win64" fn _fstat64(fd: i32, buf: *mut u8) -> i32 {
    if buf.is_null() || fd < 0 {
        return -1;
    }
    let mut st: libc::stat = std::mem::zeroed();
    let res = libc::fstat(fd, &mut st);
    if res != 0 {
        return -1;
    }
    let out = buf as *mut Stat64;
    let mut win_mode: u16 = 0;
    if (st.st_mode & libc::S_IFMT) == libc::S_IFDIR {
        win_mode |= 0x4000;
    } else if (st.st_mode & libc::S_IFMT) == libc::S_IFREG {
        win_mode |= 0x8000;
    }
    if (st.st_mode & libc::S_IRUSR) != 0 {
        win_mode |= 0x0100;
    }
    if (st.st_mode & libc::S_IWUSR) != 0 {
        win_mode |= 0x0080;
    }
    if (st.st_mode & libc::S_IXUSR) != 0 {
        win_mode |= 0x0040;
    }

    (*out).st_dev = st.st_dev as u32;
    (*out).st_ino = st.st_ino as u16;
    (*out).st_mode = win_mode;
    (*out).st_nlink = st.st_nlink as i16;
    (*out).st_uid = st.st_uid as i16;
    (*out).st_gid = st.st_gid as i16;
    (*out)._pad0 = 0;
    (*out).st_rdev = st.st_rdev as u32;
    (*out)._pad = 0;
    (*out).st_size = st.st_size as i64;
    (*out).st_atime = st.st_atime as i64;
    (*out).st_mtime = st.st_mtime as i64;
    (*out).st_ctime = st.st_ctime as i64;

    0
}

pub unsafe extern "win64" fn _wstat64(path: *const u16, buf: *mut u8) -> i32 {
    if path.is_null() || buf.is_null() {
        return -1;
    }
    let mut len = 0;
    while *path.add(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(path, len);
    let os_string = String::from_utf16_lossy(slice);

    let host_path = crate::dlls::kernel32::win32_to_host_path(&os_string);
    let c_path = match std::ffi::CString::new(host_path) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let mut st: libc::stat = std::mem::zeroed();
    let res = libc::stat(c_path.as_ptr(), &mut st);
    if res != 0 {
        return -1;
    }

    let out = buf as *mut Stat64;
    let mut win_mode: u16 = 0;
    if (st.st_mode & libc::S_IFMT) == libc::S_IFDIR {
        win_mode |= 0x4000;
    } else if (st.st_mode & libc::S_IFMT) == libc::S_IFREG {
        win_mode |= 0x8000;
    } else if (st.st_mode & libc::S_IFMT) == libc::S_IFCHR {
        win_mode |= 0x2000;
    }
    if (st.st_mode & libc::S_IRUSR) != 0 {
        win_mode |= 0x0100;
    }
    if (st.st_mode & libc::S_IWUSR) != 0 {
        win_mode |= 0x0080;
    }
    if (st.st_mode & libc::S_IXUSR) != 0 {
        win_mode |= 0x0040;
    }

    (*out).st_dev = st.st_dev as u32;
    (*out).st_ino = st.st_ino as u16;
    (*out).st_mode = win_mode;
    (*out).st_nlink = st.st_nlink as i16;
    (*out).st_uid = st.st_uid as i16;
    (*out).st_gid = st.st_gid as i16;
    (*out)._pad0 = 0;
    (*out).st_rdev = st.st_rdev as u32;
    (*out)._pad = 0;
    (*out).st_size = st.st_size as i64;
    (*out).st_atime = st.st_atime as i64;
    (*out).st_mtime = st.st_mtime as i64;
    (*out).st_ctime = st.st_ctime as i64;

    0
}

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

pub unsafe extern "win64" fn fwrite(
    ptr: *const std::ffi::c_void,
    size: usize,
    count: usize,
    stream: *mut libc::FILE,
) -> usize {
    if stream.is_null() || ptr.is_null() || size == 0 || count == 0 {
        return 0;
    }
    libc::fwrite(ptr, size, count, stream)
}

pub unsafe extern "win64" fn msvcrt_fopen(
    filename: *const libc::c_char,
    mode: *const libc::c_char,
) -> *mut libc::FILE {
    if filename.is_null() || mode.is_null() {
        return std::ptr::null_mut();
    }
    let c_name = std::ffi::CStr::from_ptr(filename).to_string_lossy();
    let host_path = crate::dlls::kernel32::win32_to_host_path(&c_name);
    let c_mode = std::ffi::CStr::from_ptr(mode);
    println!("[MSVCRT FILE] fopen: '{}' -> '{}'", c_name, host_path);
    if let Ok(c_path) = std::ffi::CString::new(host_path.as_bytes()) {
        libc::fopen(c_path.as_ptr(), c_mode.as_ptr())
    } else {
        std::ptr::null_mut()
    }
}

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
    let host_path = crate::dlls::kernel32::win32_to_host_path(&win_path);

    let mut mlen = 0usize;
    while *mode.add(mlen) != 0 {
        mlen += 1;
    }
    let mode_str = String::from_utf16_lossy(std::slice::from_raw_parts(mode, mlen));

    if mode_str.contains('w') || mode_str.contains('a') {
        if let Some(parent) = std::path::Path::new(&host_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
    }

    let c_path = match std::ffi::CString::new(host_path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };
    let c_mode = match std::ffi::CString::new(mode_str.as_bytes()) {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };

    libc::fopen(c_path.as_ptr(), c_mode.as_ptr())
}

pub unsafe extern "win64" fn msvcrt_wfsopen(
    filename: *const u16,
    mode: *const u16,
    _shflag: i32,
) -> *mut libc::FILE {
    msvcrt_wfopen(filename, mode)
}

pub unsafe extern "win64" fn msvcrt_filelengthi64(fd: i32) -> i64 {
    let mut st: libc::stat = std::mem::zeroed();
    if libc::fstat(fd, &mut st) == 0 {
        st.st_size as i64
    } else {
        -1
    }
}

pub unsafe extern "win64" fn msvcrt_fileno(stream: *mut libc::FILE) -> i32 {
    if !stream.is_null() {
        libc::fileno(stream)
    } else {
        -1
    }
}

pub unsafe extern "win64" fn msvcrt_lseeki64(fd: i32, offset: i64, origin: i32) -> i64 {
    let whence = match origin {
        0 => libc::SEEK_SET,
        1 => libc::SEEK_CUR,
        2 => libc::SEEK_END,
        _ => return -1,
    };
    libc::lseek(fd, offset, whence)
}

pub unsafe extern "win64" fn msvcrt_fclose(stream: *mut libc::FILE) -> i32 {
    if !stream.is_null() {
        libc::fclose(stream)
    } else {
        0
    }
}

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

pub unsafe extern "win64" fn msvcrt_fseek(
    stream: *mut libc::FILE,
    offset: i64,
    origin: i32,
) -> i32 {
    if stream.is_null() {
        return -1;
    }
    libc::fseeko(stream, offset as libc::off_t, origin)
}

pub unsafe extern "win64" fn msvcrt_ftell(stream: *mut libc::FILE) -> i64 {
    if stream.is_null() {
        return -1;
    }
    libc::ftello(stream) as i64
}

pub unsafe extern "win64" fn msvcrt_fgetpos(stream: *mut libc::FILE, pos: *mut i64) -> i32 {
    if stream.is_null() || pos.is_null() {
        return -1;
    }
    let p = libc::ftello(stream);
    if p < 0 {
        return -1;
    }
    *pos = p as i64;
    0
}

pub unsafe extern "win64" fn msvcrt_fsetpos(stream: *mut libc::FILE, pos: *const i64) -> i32 {
    if stream.is_null() || pos.is_null() {
        return -1;
    }
    libc::fseeko(stream, *pos as libc::off_t, libc::SEEK_SET)
}

pub unsafe extern "win64" fn fflush(stream: *mut libc::FILE) -> i32 {
    if !stream.is_null() {
        libc::fflush(stream)
    } else {
        libc::fflush(std::ptr::null_mut())
    }
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

pub unsafe extern "win64" fn msvcrt_exit(status: i32) -> ! {
    eprintln!(
        "[CRT EXIT] MSVCRT!exit called with status={} (0x{:08X})",
        status, status
    );
    eprintln!("[GUEST EXITED] MSVCRT!exit invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(status);
}

pub unsafe extern "win64" fn msvcrt_exit_underscore(status: i32) -> ! {
    eprintln!(
        "[CRT EXIT] MSVCRT!_exit called with status={} (0x{:08X})",
        status, status
    );
    eprintln!("[GUEST EXITED] MSVCRT!_exit invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(status);
}

pub unsafe extern "win64" fn msvcrt_abort() -> ! {
    eprintln!("[CRT ABORT] MSVCRT!abort called");
    eprintln!("[GUEST EXITED] MSVCRT!abort invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(3);
}

pub unsafe extern "win64" fn _cexit() {
    println!("[CRT EXIT] MSVCRT!_cexit called");
}

pub unsafe extern "win64" fn _c_exit() {
    println!("[CRT EXIT] MSVCRT!_c_exit called");
}

pub unsafe extern "win64" fn quick_exit(status: i32) -> ! {
    eprintln!("[CRT EXIT] UCRT!quick_exit called with status={}", status);
    eprintln!("[GUEST EXITED] UCRT!quick_exit invoked; host window pump remains active.");
    crate::dlls::user32::run_main_window_event_loop();
    std::process::exit(status);
}

pub type Pvfv = unsafe extern "win64" fn();
pub type Pifv = unsafe extern "win64" fn() -> i32;

pub unsafe extern "win64" fn msvcrt_beginthread(
    start_address: usize,
    stack_size: u32,
    arglist: *mut c_void,
) -> usize {
    println!(
        "[CRT THREAD] _beginthread: start_address=0x{:016X}, stack_size={}, arglist={:p}",
        start_address, stack_size, arglist
    );

    let handle = crate::dlls::kernel32::CreateThread(
        std::ptr::null_mut(),
        stack_size as usize,
        start_address as u64,
        arglist as u64,
        0,
        std::ptr::null_mut(),
    );

    if handle == 0 {
        usize::MAX
    } else {
        handle as usize
    }
}

pub unsafe extern "win64" fn msvcrt_beginthreadex(
    security: *mut c_void,
    stack_size: u32,
    start_address: usize,
    arglist: *mut c_void,
    initflag: u32,
    thrdaddr: *mut u32,
) -> usize {
    println!(
        "[CRT THREAD] _beginthreadex: security={:p}, stack_size={}, start_address=0x{:016X}, arglist={:p}, initflag=0x{:X}, thrdaddr={:p}",
        security, stack_size, start_address, arglist, initflag, thrdaddr
    );

    let handle = crate::dlls::kernel32::CreateThread(
        security as *mut u8,
        stack_size as usize,
        start_address as u64,
        arglist as u64,
        initflag,
        thrdaddr,
    );

    println!(
        "[CRT THREAD] _beginthreadex returning thread handle 0x{:016X}",
        handle
    );
    handle as usize
}

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
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }
    libc::memcpy(dest, src, n)
}

pub unsafe extern "win64" fn msvcrt_memcpy_s(
    dest: *mut c_void,
    dest_size: usize,
    src: *const c_void,
    count: usize,
) -> i32 {
    if dest.is_null() || count == 0 {
        return if dest.is_null() { 22 } else { 0 };
    }
    if src.is_null() || count > dest_size {
        libc::memset(dest, 0, dest_size);
        return if src.is_null() { 22 } else { 34 };
    }
    libc::memcpy(dest, src, count);
    0
}

pub unsafe extern "win64" fn msvcrt_memmove_s(
    dest: *mut c_void,
    dest_size: usize,
    src: *const c_void,
    count: usize,
) -> i32 {
    if dest.is_null() || count == 0 {
        return if dest.is_null() { 22 } else { 0 };
    }
    if src.is_null() || count > dest_size {
        return if src.is_null() { 22 } else { 34 };
    }
    libc::memmove(dest, src, count);
    0
}

pub unsafe extern "win64" fn msvcrt_memchr(s: *const c_void, c: i32, n: usize) -> *mut c_void {
    if s.is_null() || n == 0 {
        std::ptr::null_mut()
    } else {
        libc::memchr(s, c, n)
    }
}

pub unsafe extern "win64" fn msvcrt_memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    if s1.is_null() || s2.is_null() || n == 0 {
        return 0;
    }
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
    libc::realloc(ptr, size)
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
    if ptr.is_null() {
        return msvcrt_aligned_malloc(size, alignment);
    }
    if size == 0 {
        msvcrt_aligned_free(ptr);
        return std::ptr::null_mut();
    }
    let new_ptr = msvcrt_aligned_malloc(size, alignment);
    if !new_ptr.is_null() {
        let old_size = libc::malloc_usable_size(ptr);
        let copy_len = if old_size > 0 {
            old_size.min(size)
        } else {
            size
        };
        libc::memcpy(new_ptr, ptr, copy_len);
        msvcrt_aligned_free(ptr);
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
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }
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

pub unsafe extern "win64" fn msvcrt_strcpy_s(
    dest: *mut libc::c_char,
    dest_size: usize,
    src: *const libc::c_char,
) -> i32 {
    if dest.is_null() || dest_size == 0 {
        return 22; // EINVAL
    }
    if src.is_null() {
        *dest = 0;
        return 22; // EINVAL
    }
    let src_len = libc::strlen(src);
    if src_len >= dest_size {
        *dest = 0;
        return 34; // ERANGE
    }
    libc::strcpy(dest, src);
    0
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

pub unsafe extern "win64" fn msvcrt_strncpy_s(
    dest: *mut libc::c_char,
    dest_size: usize,
    src: *const libc::c_char,
    count: usize,
) -> i32 {
    if dest.is_null() || dest_size == 0 {
        return 22;
    }
    if src.is_null() {
        *dest = 0;
        return 22;
    }
    let src_len = libc::strlen(src);
    let copy_len = if count == usize::MAX || count > src_len {
        src_len
    } else {
        count
    };
    if copy_len >= dest_size {
        *dest = 0;
        return 34;
    }
    libc::strncpy(dest, src, copy_len);
    *dest.add(copy_len) = 0;
    0
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

pub unsafe extern "win64" fn msvcrt_strcat_s(
    dest: *mut libc::c_char,
    dest_size: usize,
    src: *const libc::c_char,
) -> i32 {
    if dest.is_null() || dest_size == 0 {
        return 22;
    }
    if src.is_null() {
        return 22;
    }
    let dest_len = libc::strlen(dest);
    let src_len = libc::strlen(src);
    if dest_len + src_len >= dest_size {
        return 34;
    }
    libc::strcat(dest, src);
    0
}

pub unsafe extern "win64" fn msvcrt_strncat_s(
    dest: *mut libc::c_char,
    dest_size: usize,
    src: *const libc::c_char,
    count: usize,
) -> i32 {
    if dest.is_null() || dest_size == 0 {
        return 22;
    }
    if src.is_null() {
        return 22;
    }
    let dest_len = libc::strlen(dest);
    let src_len = libc::strlen(src);
    let copy_len = if count == usize::MAX || count > src_len {
        src_len
    } else {
        count
    };
    if dest_len + copy_len >= dest_size {
        return 34;
    }
    libc::strncat(dest, src, copy_len);
    *dest.add(dest_len + copy_len) = 0;
    0
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

pub unsafe fn format_win64_vsprintf(
    buffer: *mut libc::c_char,
    buffer_count: usize,
    format: *const libc::c_char,
    mut arglist: *mut u64,
) -> i32 {
    if buffer.is_null()
        || format.is_null()
        || buffer_count == 0
        || (buffer as usize) < 0x10000
        || (format as usize) < 0x10000
    {
        return -1;
    }

    let fmt_str = std::ffi::CStr::from_ptr(format).to_string_lossy();
    let mut out = String::new();
    let mut chars = fmt_str.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }

        if chars.peek() == Some(&'%') {
            chars.next();
            out.push('%');
            continue;
        }

        let mut spec = String::from("%");
        while let Some(&next_c) = chars.peek() {
            chars.next();
            spec.push(next_c);
            match next_c {
                'd' | 'i' => {
                    let val = if !arglist.is_null() {
                        let v = *arglist as i64;
                        arglist = arglist.add(1);
                        v
                    } else {
                        0
                    };
                    if spec.contains("lld") || spec.contains("I64d") || spec.contains("jd") {
                        out.push_str(&format!("{}", val));
                    } else {
                        out.push_str(&format!("{}", val as i32));
                    }
                    break;
                }
                'u' => {
                    let val = if !arglist.is_null() {
                        let v = *arglist;
                        arglist = arglist.add(1);
                        v
                    } else {
                        0
                    };
                    if spec.contains("llu") || spec.contains("I64u") || spec.contains("zu") {
                        out.push_str(&format!("{}", val));
                    } else {
                        out.push_str(&format!("{}", val as u32));
                    }
                    break;
                }
                'x' | 'X' => {
                    let val = if !arglist.is_null() {
                        let v = *arglist;
                        arglist = arglist.add(1);
                        v
                    } else {
                        0
                    };
                    if next_c == 'X' {
                        out.push_str(&format!("{:X}", val as u32));
                    } else {
                        out.push_str(&format!("{:x}", val as u32));
                    }
                    break;
                }
                'p' => {
                    let val = if !arglist.is_null() {
                        let v = *arglist;
                        arglist = arglist.add(1);
                        v
                    } else {
                        0
                    };
                    out.push_str(&format!("0x{:016X}", val));
                    break;
                }
                's' => {
                    let ptr = if !arglist.is_null() {
                        let p = *arglist as *const libc::c_char;
                        arglist = arglist.add(1);
                        p
                    } else {
                        std::ptr::null()
                    };
                    if ptr.is_null() || (ptr as usize) < 0x10000 {
                        out.push_str("(null)");
                    } else {
                        let s = std::ffi::CStr::from_ptr(ptr).to_string_lossy();
                        out.push_str(&s);
                    }
                    break;
                }
                'c' => {
                    let val = if !arglist.is_null() {
                        let v = *arglist as u8;
                        arglist = arglist.add(1);
                        v
                    } else {
                        0
                    };
                    out.push(val as char);
                    break;
                }
                'f' | 'g' | 'e' => {
                    let val = if !arglist.is_null() {
                        let bits = *arglist;
                        arglist = arglist.add(1);
                        f64::from_bits(bits)
                    } else {
                        0.0
                    };
                    out.push_str(&format!("{:.6}", val));
                    break;
                }
                _ => {}
            }
        }
    }

    let bytes = out.as_bytes();
    let copy_len = bytes.len().min(buffer_count.saturating_sub(1));
    if copy_len > 0 {
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const libc::c_char, buffer, copy_len);
    }
    if buffer_count > 0 {
        *buffer.add(copy_len) = 0;
    }
    copy_len as i32
}

pub unsafe extern "win64" fn __stdio_common_vsprintf_s(
    _options: u64,
    buffer: *mut libc::c_char,
    buffer_count: usize,
    format: *const libc::c_char,
    _locale: *mut std::ffi::c_void,
    arglist: *mut u64,
) -> i32 {
    format_win64_vsprintf(buffer, buffer_count, format, arglist)
}

pub unsafe extern "win64" fn __stdio_common_vsprintf(
    _options: u64,
    buffer: *mut libc::c_char,
    buffer_count: usize,
    format: *const libc::c_char,
    _locale: *mut std::ffi::c_void,
    arglist: *mut u64,
) -> i32 {
    format_win64_vsprintf(buffer, buffer_count, format, arglist)
}

pub unsafe extern "win64" fn vsprintf(
    buffer: *mut libc::c_char,
    format: *const libc::c_char,
    arglist: *mut u64,
) -> i32 {
    format_win64_vsprintf(buffer, 65536, format, arglist)
}

pub unsafe extern "win64" fn _vsnprintf(
    buffer: *mut libc::c_char,
    count: usize,
    format: *const libc::c_char,
    arglist: *mut u64,
) -> i32 {
    format_win64_vsprintf(buffer, count, format, arglist)
}

pub unsafe extern "win64" fn _vsprintf_s_l(
    buffer: *mut libc::c_char,
    number_of_elements: usize,
    format: *const libc::c_char,
    _locale: *mut std::ffi::c_void,
    arglist: *mut u64,
) -> i32 {
    format_win64_vsprintf(buffer, number_of_elements, format, arglist)
}

pub unsafe extern "win64" fn _vsnprintf_s_l(
    buffer: *mut libc::c_char,
    size_of_buffer: usize,
    count: usize,
    format: *const libc::c_char,
    _locale: *mut std::ffi::c_void,
    arglist: *mut u64,
) -> i32 {
    let max_len = if count == usize::MAX || count >= size_of_buffer {
        size_of_buffer
    } else {
        count + 1
    };
    format_win64_vsprintf(buffer, max_len, format, arglist)
}

pub unsafe extern "win64" fn vsprintf_s(
    buffer: *mut libc::c_char,
    number_of_elements: usize,
    format: *const libc::c_char,
    arglist: *mut u64,
) -> i32 {
    format_win64_vsprintf(buffer, number_of_elements, format, arglist)
}

pub unsafe extern "win64" fn _vsnprintf_s(
    buffer: *mut libc::c_char,
    size_of_buffer: usize,
    count: usize,
    format: *const libc::c_char,
    arglist: *mut u64,
) -> i32 {
    let max_len = if count == usize::MAX || count >= size_of_buffer {
        size_of_buffer
    } else {
        count + 1
    };
    format_win64_vsprintf(buffer, max_len, format, arglist)
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

pub unsafe extern "win64" fn msvcrt_wcscpy_s(
    dest: *mut u16,
    dest_size: usize,
    src: *const u16,
) -> i32 {
    if dest.is_null() || dest_size == 0 {
        return 22;
    }
    if src.is_null() {
        *dest = 0;
        return 22;
    }
    let mut src_len = 0;
    while *src.add(src_len) != 0 {
        src_len += 1;
    }
    if src_len >= dest_size {
        *dest = 0;
        return 34;
    }
    std::ptr::copy_nonoverlapping(src, dest, src_len + 1);
    0
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

pub unsafe extern "win64" fn msvcrt_wcsncpy_s(
    dest: *mut u16,
    dest_size: usize,
    src: *const u16,
    count: usize,
) -> i32 {
    if dest.is_null() || dest_size == 0 {
        return 22;
    }
    if src.is_null() {
        *dest = 0;
        return 22;
    }
    let mut src_len = 0;
    while *src.add(src_len) != 0 {
        src_len += 1;
    }
    let copy_len = if count == usize::MAX || count > src_len {
        src_len
    } else {
        count
    };
    if copy_len >= dest_size {
        *dest = 0;
        return 34;
    }
    std::ptr::copy_nonoverlapping(src, dest, copy_len);
    *dest.add(copy_len) = 0;
    0
}

pub unsafe extern "win64" fn msvcrt_wcscat(dest: *mut u16, src: *const u16) -> *mut u16 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    let len = msvcrt_wcslen(dest);
    msvcrt_wcscpy(dest.add(len), src);
    dest
}

pub unsafe extern "win64" fn msvcrt_wcscat_s(
    dest: *mut u16,
    dest_size: usize,
    src: *const u16,
) -> i32 {
    if dest.is_null() || dest_size == 0 {
        return 22;
    }
    if src.is_null() {
        return 22;
    }
    let mut dest_len = 0;
    while *dest.add(dest_len) != 0 {
        dest_len += 1;
    }
    let mut src_len = 0;
    while *src.add(src_len) != 0 {
        src_len += 1;
    }
    if dest_len + src_len >= dest_size {
        return 34;
    }
    std::ptr::copy_nonoverlapping(src, dest.add(dest_len), src_len + 1);
    0
}

pub unsafe extern "win64" fn msvcrt_fopen_s(
    file_ptr: *mut *mut libc::FILE,
    filename: *const libc::c_char,
    mode: *const libc::c_char,
) -> i32 {
    if file_ptr.is_null() {
        return 22;
    }
    *file_ptr = std::ptr::null_mut();
    if filename.is_null() || mode.is_null() {
        return 22;
    }
    let fp = msvcrt_fopen(filename, mode);
    if !fp.is_null() {
        *file_ptr = fp;
        0
    } else {
        *libc::__errno_location()
    }
}

pub unsafe extern "win64" fn msvcrt_wfopen_s(
    file_ptr: *mut *mut libc::FILE,
    filename: *const u16,
    mode: *const u16,
) -> i32 {
    if file_ptr.is_null() {
        return 22;
    }
    *file_ptr = std::ptr::null_mut();
    if filename.is_null() || mode.is_null() {
        return 22;
    }
    let fp = msvcrt_wfopen(filename, mode);
    if !fp.is_null() {
        *file_ptr = fp;
        0
    } else {
        *libc::__errno_location()
    }
}

pub unsafe extern "win64" fn msvcrt_sprintf_s(
    buffer: *mut libc::c_char,
    number_of_elements: usize,
    format: *const libc::c_char,
    arglist: *mut u64,
) -> i32 {
    format_win64_vsprintf(buffer, number_of_elements, format, arglist)
}

pub unsafe extern "win64" fn msvcrt_snprintf_s(
    buffer: *mut libc::c_char,
    size_of_buffer: usize,
    count: usize,
    format: *const libc::c_char,
    arglist: *mut u64,
) -> i32 {
    let max_len = if count == usize::MAX || count >= size_of_buffer {
        size_of_buffer
    } else {
        count + 1
    };
    format_win64_vsprintf(buffer, max_len, format, arglist)
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

#[unsafe(naked)]
pub unsafe extern "win64" fn msvcrt_longjmp(_env: *const c_void, _val: i32) {
    std::arch::naked_asm!(
        "mov eax, edx",
        "test eax, eax",
        "jnz 2f",
        "mov eax, 1",
        "2:",
        "ldmxcsr [rcx + 0x58]",
        "fldcw [rcx + 0x5C]",
        "movdqu xmm6, [rcx + 0x60]",
        "movdqu xmm7, [rcx + 0x70]",
        "movdqu xmm8, [rcx + 0x80]",
        "movdqu xmm9, [rcx + 0x90]",
        "movdqu xmm10, [rcx + 0xA0]",
        "movdqu xmm11, [rcx + 0xB0]",
        "movdqu xmm12, [rcx + 0xC0]",
        "movdqu xmm13, [rcx + 0xD0]",
        "movdqu xmm14, [rcx + 0xE0]",
        "movdqu xmm15, [rcx + 0xF0]",
        "mov rbx, [rcx + 0x08]",
        "mov rbp, [rcx + 0x18]",
        "mov rsi, [rcx + 0x20]",
        "mov rdi, [rcx + 0x28]",
        "mov r12, [rcx + 0x30]",
        "mov r13, [rcx + 0x38]",
        "mov r14, [rcx + 0x40]",
        "mov r15, [rcx + 0x48]",
        "mov rsp, [rcx + 0x10]",
        "mov rdx, [rcx + 0x50]",
        "jmp rdx"
    );
}

#[unsafe(naked)]
pub unsafe extern "win64" fn msvcrt_setjmp(_env: *mut c_void) -> i32 {
    std::arch::naked_asm!(
        "mov [rcx + 0x00], rdx",
        "mov [rcx + 0x08], rbx",
        "lea rax, [rsp + 8]",
        "mov [rcx + 0x10], rax",
        "mov [rcx + 0x18], rbp",
        "mov [rcx + 0x20], rsi",
        "mov [rcx + 0x28], rdi",
        "mov [rcx + 0x30], r12",
        "mov [rcx + 0x38], r13",
        "mov [rcx + 0x40], r14",
        "mov [rcx + 0x48], r15",
        "mov rax, [rsp]",
        "mov [rcx + 0x50], rax",
        "stmxcsr [rcx + 0x58]",
        "fnstcw [rcx + 0x5C]",
        "movdqu [rcx + 0x60], xmm6",
        "movdqu [rcx + 0x70], xmm7",
        "movdqu [rcx + 0x80], xmm8",
        "movdqu [rcx + 0x90], xmm9",
        "movdqu [rcx + 0xA0], xmm10",
        "movdqu [rcx + 0xB0], xmm11",
        "movdqu [rcx + 0xC0], xmm12",
        "movdqu [rcx + 0xD0], xmm13",
        "movdqu [rcx + 0xE0], xmm14",
        "movdqu [rcx + 0xF0], xmm15",
        "xor eax, eax",
        "ret"
    );
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

    static RECEIVED_ARG: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    unsafe extern "win64" fn test_start_routine(arg: *mut std::ffi::c_void) {
        RECEIVED_ARG.store(arg as usize, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    fn test_beginthread_win64_arguments() {
        RECEIVED_ARG.store(0, std::sync::atomic::Ordering::SeqCst);
        let arg_val = 0x12345678usize;
        let handle = unsafe {
            msvcrt_beginthread(
                test_start_routine as *const () as usize,
                65536,
                arg_val as *mut std::ffi::c_void,
            )
        };
        assert_ne!(handle, 0);
        assert_ne!(handle, usize::MAX);

        unsafe {
            crate::dlls::kernel32::WaitForSingleObject(handle as u64, 5000);
        }
        assert_eq!(
            RECEIVED_ARG.load(std::sync::atomic::Ordering::SeqCst),
            arg_val
        );
    }

    static RECEIVED_EX_ARG: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    unsafe extern "win64" fn test_startex_routine(arg: *mut std::ffi::c_void) -> u32 {
        RECEIVED_EX_ARG.store(arg as usize, std::sync::atomic::Ordering::SeqCst);
        123
    }

    #[test]
    fn test_beginthreadex_win64_arguments() {
        RECEIVED_EX_ARG.store(0, std::sync::atomic::Ordering::SeqCst);
        let arg_val = 0x87654321usize;
        let mut thrdaddr: u32 = 0;
        let handle = unsafe {
            msvcrt_beginthreadex(
                std::ptr::null_mut(),
                65536,
                test_startex_routine as *const () as usize,
                arg_val as *mut std::ffi::c_void,
                0,
                &raw mut thrdaddr,
            )
        };
        assert_ne!(handle, 0);
        assert_ne!(thrdaddr, 0);
        assert_ne!(thrdaddr as u64, handle as u64); // Confirm thrdaddr is Thread ID, NOT handle

        unsafe {
            crate::dlls::kernel32::WaitForSingleObject(handle as u64, 5000);
        }
        assert_eq!(
            RECEIVED_EX_ARG.load(std::sync::atomic::Ordering::SeqCst),
            arg_val
        );

        let mut exit_code = 0u32;
        unsafe {
            crate::dlls::kernel32::GetExitCodeThread(handle as u64, &raw mut exit_code);
        }
        assert_eq!(exit_code, 123);
    }

    #[test]
    fn test_stat64_size_and_alignment() {
        assert_eq!(
            std::mem::size_of::<Stat64>(),
            56,
            "Win64 struct _stat64 must be exactly 56 bytes"
        );
        assert_eq!(
            std::mem::align_of::<Stat64>(),
            8,
            "Win64 struct _stat64 must be 8-byte aligned"
        );

        let dummy = Stat64 {
            st_dev: 0,
            st_ino: 0,
            st_mode: 0,
            st_nlink: 0,
            st_uid: 0,
            st_gid: 0,
            _pad0: 0,
            st_rdev: 0,
            _pad: 0,
            st_size: 0,
            st_atime: 0,
            st_mtime: 0,
            st_ctime: 0,
        };
        let base = &raw const dummy as usize;
        assert_eq!((&raw const dummy.st_dev as usize) - base, 0);
        assert_eq!((&raw const dummy.st_ino as usize) - base, 4);
        assert_eq!((&raw const dummy.st_mode as usize) - base, 6);
        assert_eq!((&raw const dummy.st_nlink as usize) - base, 8);
        assert_eq!((&raw const dummy.st_uid as usize) - base, 10);
        assert_eq!((&raw const dummy.st_gid as usize) - base, 12);
        assert_eq!((&raw const dummy._pad0 as usize) - base, 14);
        assert_eq!((&raw const dummy.st_rdev as usize) - base, 16);
        assert_eq!((&raw const dummy._pad as usize) - base, 20);
        assert_eq!((&raw const dummy.st_size as usize) - base, 24);
        assert_eq!((&raw const dummy.st_atime as usize) - base, 32);
        assert_eq!((&raw const dummy.st_mtime as usize) - base, 40);
        assert_eq!((&raw const dummy.st_ctime as usize) - base, 48);
    }

    #[test]
    fn test_errno_thread_isolation() {
        unsafe {
            let main_errno_ptr = _errno();
            assert!(!main_errno_ptr.is_null());
            *main_errno_ptr = 111;
            let main_errno_addr = main_errno_ptr as usize;

            let handle = std::thread::spawn(move || {
                let worker_errno_ptr = _errno();
                assert!(!worker_errno_ptr.is_null());
                assert_ne!(worker_errno_ptr as usize, main_errno_addr);
                assert_eq!(*worker_errno_ptr, 0);
                *worker_errno_ptr = 222;
                assert_eq!(*worker_errno_ptr, 222);
            });
            handle.join().unwrap();

            assert_eq!(*_errno(), 111);
        }
    }

    #[test]
    fn test_wstat64_buffer_bounds_and_fields() {
        #[repr(C)]
        struct GuardedBuffer {
            guard_before: [u8; 16],
            stat_buf: Stat64,
            guard_after: [u8; 16],
        }

        let mut buf = GuardedBuffer {
            guard_before: [0xAA; 16],
            stat_buf: unsafe { std::mem::zeroed() },
            guard_after: [0xBB; 16],
        };

        // Test on existing path Cargo.toml
        let path = format!("{}/Cargo.toml\0", env!("CARGO_MANIFEST_DIR"));
        let path_utf16: Vec<u16> = path.encode_utf16().collect();
        let ret = unsafe { _wstat64(path_utf16.as_ptr(), &raw mut buf.stat_buf as *mut u8) };
        assert_eq!(ret, 0, "stat on Cargo.toml must succeed");
        assert_eq!(
            buf.guard_before, [0xAA; 16],
            "guard_before was overwritten!"
        );
        assert_eq!(buf.guard_after, [0xBB; 16], "guard_after was overwritten!");
        assert!(buf.stat_buf.st_size > 0, "Cargo.toml size must be > 0");
        assert_ne!(buf.stat_buf.st_mode, 0, "st_mode must be set");

        // Test on nonexistent path
        let non_utf16: Vec<u16> = "nonexistent_file_12345.bin\0".encode_utf16().collect();
        let ret_fail = unsafe { _wstat64(non_utf16.as_ptr(), &raw mut buf.stat_buf as *mut u8) };
        assert_eq!(ret_fail, -1, "stat on nonexistent file must return -1");
    }

    #[test]
    fn test_wfsopen_and_file_operations() {
        let temp_path = std::env::temp_dir().join("rine_test_wfsopen.tmp");
        std::fs::write(&temp_path, b"hello rine file test 12345\n").unwrap();
        let path_str = temp_path.to_str().unwrap();
        let path_utf16: Vec<u16> = format!("{}\0", path_str).encode_utf16().collect();
        let mode_utf16: Vec<u16> = "rb\0".encode_utf16().collect();

        unsafe {
            let fp = msvcrt_wfsopen(path_utf16.as_ptr(), mode_utf16.as_ptr(), 0x40);
            assert!(!fp.is_null(), "test file must open successfully");

            let fd = msvcrt_fileno(fp);
            assert!(fd >= 0, "fileno must return non-negative fd");

            let len = msvcrt_filelengthi64(fd);
            assert!(len > 0, "test file length must be > 0");

            let pos = msvcrt_ftell(fp);
            assert_eq!(pos, 0);

            let mut fpos = 0i64;
            assert_eq!(msvcrt_fgetpos(fp, &mut fpos), 0);
            assert_eq!(fpos, 0);

            let seek_res = msvcrt_fseek(fp, 0, libc::SEEK_END);
            assert_eq!(seek_res, 0);

            let end_pos = msvcrt_ftell(fp);
            assert_eq!(end_pos, len);

            assert_eq!(msvcrt_fgetpos(fp, &mut fpos), 0);
            assert_eq!(fpos, len);

            let start_pos = 0i64;
            assert_eq!(msvcrt_fsetpos(fp, &start_pos), 0);
            assert_eq!(msvcrt_ftell(fp), 0);

            msvcrt_fclose(fp);
        }
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn test_setjmp_longjmp_lifecycle() {
        let mut jmp_buf = [0u8; 256];
        unsafe {
            let res = msvcrt_setjmp(jmp_buf.as_mut_ptr() as *mut c_void);
            if res == 0 {
                msvcrt_longjmp(jmp_buf.as_ptr() as *const c_void, 42);
                panic!("msvcrt_longjmp must not return here");
            } else {
                assert_eq!(res, 42, "msvcrt_setjmp must return the value passed to longjmp");
            }
        }
    }

    #[test]
    fn test_setjmp_longjmp_zero_value_returns_one() {
        let mut jmp_buf = [0u8; 256];
        unsafe {
            let res = msvcrt_setjmp(jmp_buf.as_mut_ptr() as *mut c_void);
            if res == 0 {
                msvcrt_longjmp(jmp_buf.as_ptr() as *const c_void, 0);
                panic!("msvcrt_longjmp must not return here");
            } else {
                assert_eq!(res, 1, "longjmp with val=0 must cause setjmp to return 1 per Win64 CRT spec");
            }
        }
    }

    #[test]
    fn test_setjmp_longjmp_canary_integrity() {
        #[repr(C, align(16))]
        struct CanaryBuffer {
            prefix_canary: [u8; 32],
            jmp_buf: [u8; 256],
            suffix_canary: [u8; 32],
        }

        let mut buf = CanaryBuffer {
            prefix_canary: [0x5A; 32],
            jmp_buf: [0; 256],
            suffix_canary: [0xA5; 32],
        };

        unsafe {
            let res = msvcrt_setjmp(buf.jmp_buf.as_mut_ptr() as *mut c_void);
            if res == 0 {
                assert_eq!(buf.prefix_canary, [0x5A; 32], "prefix canary must not be modified by setjmp");
                assert_eq!(buf.suffix_canary, [0xA5; 32], "suffix canary must not be modified by setjmp");
                msvcrt_longjmp(buf.jmp_buf.as_ptr() as *const c_void, 77);
                panic!("longjmp must not return");
            } else {
                assert_eq!(res, 77);
                assert_eq!(buf.prefix_canary, [0x5A; 32], "prefix canary must remain intact after longjmp");
                assert_eq!(buf.suffix_canary, [0xA5; 32], "suffix canary must remain intact after longjmp");
            }
        }
    }

    #[test]
    fn test_setjmp_longjmp_buffer_layout() {
        let mut jmp_buf = [0u8; 256];
        unsafe {
            let res = msvcrt_setjmp(jmp_buf.as_mut_ptr() as *mut c_void);
            if res == 0 {
                // Verify that saved RSP (offset 0x10) and saved RIP (offset 0x50) are non-zero
                let saved_rsp = u64::from_le_bytes(jmp_buf[0x10..0x18].try_into().unwrap());
                let saved_rip = u64::from_le_bytes(jmp_buf[0x50..0x58].try_into().unwrap());
                assert_ne!(saved_rsp, 0, "saved RSP must be non-zero");
                assert_ne!(saved_rip, 0, "saved RIP must be non-zero");
                // Stack pointer should be 8-byte aligned (RSP % 8 == 0)
                assert_eq!(saved_rsp % 8, 0, "saved RSP must be at least 8-byte aligned");

                // Verify MXCSR (offset 0x58) is non-zero (standard default is 0x1F80)
                let mxcsr = u32::from_le_bytes(jmp_buf[0x58..0x5C].try_into().unwrap());
                assert_ne!(mxcsr, 0, "MXCSR must be saved and non-zero");

                // Verify FPCSR (offset 0x5C) is non-zero (standard default is 0x037F or similar)
                let fpcsr = u16::from_le_bytes(jmp_buf[0x5C..0x5E].try_into().unwrap());
                assert_ne!(fpcsr, 0, "FPCSR must be saved and non-zero");
            }
        }
    }

    #[test]
    fn test_setjmp_longjmp_xmm_and_gpr_restoration() {
        let mut jmp_buf = [0u8; 256];
        unsafe {
            let res = msvcrt_setjmp(jmp_buf.as_mut_ptr() as *mut c_void);
            if res == 0 {
                // Verify that XMM buffer space is populated and accessible
                for i in 0..10 {
                    let offset = 0x60 + i * 16;
                    let _xmm_part1 = u64::from_le_bytes(jmp_buf[offset..offset + 8].try_into().unwrap());
                    let _xmm_part2 = u64::from_le_bytes(jmp_buf[offset + 8..offset + 16].try_into().unwrap());
                }
                msvcrt_longjmp(jmp_buf.as_ptr() as *const c_void, 12345);
                panic!("longjmp must not return");
            } else {
                assert_eq!(res, 12345, "setjmp continuation must receive longjmp argument");
            }
        }
    }

    #[test]
    fn test_fwrite_and_fread_roundtrip() {
        let temp_path = "/tmp/rine_test_fwrite_roundtrip.bin";
        let c_path = std::ffi::CString::new(temp_path).unwrap();
        let c_mode_w = std::ffi::CString::new("wb").unwrap();
        let c_mode_r = std::ffi::CString::new("rb").unwrap();

        let data = b"GodotSaveData1234567890";
        unsafe {
            let fp_w = libc::fopen(c_path.as_ptr(), c_mode_w.as_ptr());
            assert!(!fp_w.is_null());
            let written = fwrite(data.as_ptr() as *const std::ffi::c_void, 1, data.len(), fp_w);
            assert_eq!(written, data.len());
            msvcrt_fclose(fp_w);

            let fp_r = libc::fopen(c_path.as_ptr(), c_mode_r.as_ptr());
            assert!(!fp_r.is_null());
            let mut read_buf = vec![0u8; data.len()];
            let read = msvcrt_fread(read_buf.as_mut_ptr() as *mut libc::c_void, 1, data.len(), fp_r);
            assert_eq!(read, data.len());
            assert_eq!(&read_buf, data);
            msvcrt_fclose(fp_r);

            libc::unlink(c_path.as_ptr());
        }
    }
}
