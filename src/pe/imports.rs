use crate::dlls;
use std::collections::HashMap;
use std::sync::Mutex;

pub static STUB_ADDRESS_TO_NAME: Mutex<Option<HashMap<u64, String>>> = Mutex::new(None);

pub fn register_stub_target(stub_fn_ptr: u64, full_name: String) {
    let mut guard = STUB_ADDRESS_TO_NAME.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(stub_fn_ptr, full_name);
}

pub fn get_stub_name(stub_fn_ptr: u64) -> Option<String> {
    let guard = STUB_ADDRESS_TO_NAME.lock().unwrap();
    if let Some(map) = guard.as_ref() {
        map.get(&stub_fn_ptr).cloned()
    } else {
        None
    }
}

pub fn create_dynamic_stub(full_name: &str) -> u64 {
    let parts: Vec<&str> = full_name.split('!').collect();
    let (dll, sym) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        ("UnknownDLL", full_name)
    };
    create_unresolved_stub("LegacyDynamic", dll, sym, 0, 0, "dynamic_stub") as u64
}

#[derive(Debug, Clone)]
pub struct StubMetadata {
    pub stub_id: u32,
    pub importer: String,
    pub dll: String,
    pub symbol: String,
    pub iat_rva: usize,
    pub iat_va: usize,
    pub resolver: String,
}

pub static STUB_METADATA_REGISTRY: Mutex<Option<HashMap<u32, StubMetadata>>> = Mutex::new(None);
static NEXT_STUB_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

pub fn get_stub_metadata(id: u32) -> Option<StubMetadata> {
    let guard = STUB_METADATA_REGISTRY.lock().unwrap();
    guard.as_ref()?.get(&id).cloned()
}

pub fn create_unresolved_stub(
    importer: &str,
    dll: &str,
    symbol: &str,
    iat_rva: usize,
    iat_va: usize,
    resolver: &str,
) -> usize {
    static STUB_REGION: Mutex<Option<(usize, usize)>> = Mutex::new(None);
    let mut guard = STUB_REGION.lock().unwrap();

    let (base_ptr, offset) = match *guard {
        Some((ptr, off)) => (ptr, off),
        None => {
            let region = crate::mem::mmap::MmapRegion::allocate_anonymous(
                None,
                0x40000,
                libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
            )
            .expect("Failed to allocate dynamic stub pool");
            let ptr = region.as_ptr() as usize;
            std::mem::forget(region);
            (ptr, 0)
        }
    };

    let stub_addr = (base_ptr + offset) as u64;
    *guard = Some((base_ptr, offset + 32));

    let stub_id = NEXT_STUB_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let meta = StubMetadata {
        stub_id,
        importer: importer.to_string(),
        dll: dll.to_string(),
        symbol: symbol.to_string(),
        iat_rva,
        iat_va,
        resolver: resolver.to_string(),
    };

    {
        let mut reg_guard = STUB_METADATA_REGISTRY.lock().unwrap();
        let map = reg_guard.get_or_insert_with(HashMap::new);
        map.insert(stub_id, meta);
    }

    println!(
        "[RINE UNRESOLVED BIND]\n\
         importer={}\n\
         dll={}\n\
         symbol={}\n\
         iat_rva=0x{:X}\n\
         iat_va=0x{:X}\n\
         stub_id={}\n\
         resolver={}",
        importer, dll, symbol, iat_rva, iat_va, stub_id, resolver
    );

    let full_name = format!("{}!{}", dll, symbol);
    register_stub_target(stub_addr, full_name.clone());
    if iat_va != 0 {
        register_iat_symbol(iat_va as u64, full_name.clone());
    }

    let target_fn = crate::dlls::registry::universal_stub_thunk as *const () as u64;
    let tagged_id: u64 = 0x52494E4500000000 | (stub_id as u64);

    unsafe {
        let ptr = stub_addr as *mut u8;
        // mov r11, [rsp] (4 bytes: 4C 8B 1C 24)
        *ptr.add(0) = 0x4C;
        *ptr.add(1) = 0x8B;
        *ptr.add(2) = 0x1C;
        *ptr.add(3) = 0x24;
        // mov r10, tagged_id (10 bytes: 49 BA <u64>)
        *ptr.add(4) = 0x49;
        *ptr.add(5) = 0xBA;
        std::ptr::write_unaligned(ptr.add(6) as *mut u64, tagged_id);
        // mov rax, target_fn (10 bytes: 48 B8 <u64>)
        *ptr.add(14) = 0x48;
        *ptr.add(15) = 0xB8;
        std::ptr::write_unaligned(ptr.add(16) as *mut u64, target_fn);
        // jmp rax (2 bytes: FF E0)
        *ptr.add(24) = 0xFF;
        *ptr.add(25) = 0xE0;
    }

    stub_addr as usize
}

pub static LOADED_MODULES: Mutex<Option<Vec<(String, usize, usize)>>> = Mutex::new(None);

pub fn register_loaded_module(name: String, base: usize, size: usize) {
    let mut guard = LOADED_MODULES.lock().unwrap();
    let vec = guard.get_or_insert_with(Vec::new);
    vec.retain(|(_, b, _)| *b != base);
    vec.push((name, base, size));
}

pub fn find_module_by_addr(addr: usize) -> Option<(String, usize, usize)> {
    let guard = LOADED_MODULES.lock().unwrap();
    if let Some(vec) = guard.as_ref() {
        for (name, base, size) in vec {
            if addr >= *base && addr < (*base + *size) {
                return Some((name.clone(), *base, *size));
            }
        }
    }
    None
}

pub fn find_module_name_by_base(base_addr: usize) -> Option<String> {
    let guard = LOADED_MODULES.lock().unwrap();
    if let Some(vec) = guard.as_ref() {
        for (name, base, _) in vec {
            if *base == base_addr {
                return Some(name.clone());
            }
        }
    }
    None
}

static IAT_LOOKUP_MAP: Mutex<Option<HashMap<u64, String>>> = Mutex::new(None);

pub fn register_iat_symbol(slot_addr: u64, name: String) {
    let mut guard = IAT_LOOKUP_MAP.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(slot_addr, name);
}

pub fn lookup_function_by_ret_addr(ret_addr: u64) -> Option<String> {
    if ret_addr < 0x10000 || ret_addr > 0x7FFFFFFFFFFF {
        return None;
    }

    unsafe {
        let code_ptr = ret_addr as *const u8;
        let op_ptr = code_ptr.sub(6);
        if !op_ptr.is_null() {
            let op = std::ptr::read_unaligned(op_ptr as *const u16);
            if op == 0x15FF {
                let disp = std::ptr::read_unaligned(code_ptr.sub(4) as *const i32);
                let target_slot = (ret_addr as i64 + disp as i64) as u64;

                let guard = IAT_LOOKUP_MAP.lock().unwrap();
                if let Some(map) = guard.as_ref() {
                    if let Some(name) = map.get(&target_slot) {
                        return Some(name.clone());
                    }
                }
            }
        }
    }

    let guard = IAT_LOOKUP_MAP.lock().unwrap();
    if let Some(map) = guard.as_ref() {
        if let Some(name) = map.get(&ret_addr) {
            return Some(name.clone());
        }
    }
    None
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageImportDescriptor {
    pub original_first_thunk: u32, // RVA to Original First Thunk (Import Name Table)
    pub time_date_stamp: u32,      // 0 if not bound
    pub forwarder_chain: u32,      // -1 if no forwarders
    pub name: u32,                 // RVA to DLL name string
    pub first_thunk: u32,          // RVA to First Thunk (Import Address Table - IAT)
}

impl ImageImportDescriptor {
    pub fn is_null(&self) -> bool {
        self.original_first_thunk == 0 && self.first_thunk == 0 && self.name == 0
    }

    #[inline]
    pub fn original_first_thunk(&self) -> u32 {
        self.original_first_thunk
    }
    #[inline]
    pub fn name(&self) -> u32 {
        self.name
    }
    #[inline]
    pub fn first_thunk(&self) -> u32 {
        self.first_thunk
    }
}

pub unsafe extern "win64" fn wine_dbg_noop_stub() -> i32 {
    0
}

pub struct IatSymbolResolver;

impl IatSymbolResolver {
    /// Resolves an imported function symbol by DLL name and function name to a function pointer.
    pub fn resolve_symbol(dll_name: &str, func_name: &str) -> Option<usize> {
        if func_name.starts_with("__wine_") {
            return Some(wine_dbg_noop_stub as *const () as usize);
        }

        let dll_upper = dll_name.to_uppercase();

        match dll_upper.as_str() {
            dll if dll.starts_with("KERNEL32")
                || dll.starts_with("KERNELBASE")
                || dll.starts_with("API-MS-WIN-CORE-")
                || dll.starts_with("API-MS-WIN-BASE-") =>
            {
                match func_name {
                    "GetProcAddress" => Some(dlls::kernel32::GetProcAddress as *const () as usize),
                    "LoadLibraryW" => Some(dlls::kernel32::LoadLibraryW as *const () as usize),
                    "LoadLibraryA" => Some(dlls::kernel32::LoadLibraryA as *const () as usize),
                    "LoadLibraryExW" | "LoadLibraryExA" => {
                        Some(dlls::kernel32::LoadLibraryExW as *const () as usize)
                    }
                    "FreeLibrary" => Some(dlls::kernel32::FreeLibrary as *const () as usize),
                    "GetModuleHandleW" | "GetModuleHandleA" => {
                        Some(dlls::kernel32::GetModuleHandleW as *const () as usize)
                    }
                    "GetModuleHandleExW" | "GetModuleHandleExA" => {
                        Some(dlls::kernel32::GetModuleHandleExW as *const () as usize)
                    }
                    "GetModuleFileNameW" => {
                        Some(dlls::kernel32::GetModuleFileNameW as *const () as usize)
                    }
                    "GetModuleFileNameA" => {
                        Some(dlls::kernel32::GetModuleFileNameA as *const () as usize)
                    }
                    "GetLogicalDrives" => {
                        Some(dlls::kernel32::GetLogicalDrives as *const () as usize)
                    }
                    "GetCurrentDirectoryW" => {
                        Some(dlls::kernel32::GetCurrentDirectoryW as *const () as usize)
                    }
                    "SetCurrentDirectoryW" => {
                        Some(dlls::kernel32::SetCurrentDirectoryW as *const () as usize)
                    }
                    "CreateDirectoryW" => {
                        Some(dlls::kernel32::CreateDirectoryW as *const () as usize)
                    }
                    "CreateDirectoryA" => {
                        Some(dlls::kernel32::CreateDirectoryA as *const () as usize)
                    }
                    "GetNativeSystemInfo" => {
                        Some(dlls::kernel32::GetNativeSystemInfo as *const () as usize)
                    }
                    "GetSystemInfo" => Some(dlls::kernel32::GetSystemInfo as *const () as usize),
                    "GetLocalTime" => Some(dlls::kernel32::GetLocalTime as *const () as usize),
                    "GetSystemTime" => Some(dlls::kernel32::GetSystemTime as *const () as usize),
                    "GetSystemTimeAsFileTime" => {
                        Some(dlls::kernel32::GetSystemTimeAsFileTime as *const () as usize)
                    }
                    "SystemTimeToFileTime" => {
                        Some(dlls::kernel32::SystemTimeToFileTime as *const () as usize)
                    }
                    "GetLocaleInfoEx" => {
                        Some(dlls::kernel32::GetLocaleInfoEx as *const () as usize)
                    }
                    "GetLocaleInfoW" => Some(dlls::kernel32::GetLocaleInfoW as *const () as usize),
                    "GetLocaleInfoA" => Some(dlls::kernel32::GetLocaleInfoA as *const () as usize),
                    "GetTimeZoneInformation" => {
                        Some(dlls::kernel32::GetTimeZoneInformation as *const () as usize)
                    }
                    "GetEnvironmentVariableW" => {
                        Some(dlls::kernel32::GetEnvironmentVariableW as *const () as usize)
                    }
                    "GetEnvironmentVariableA" => {
                        Some(dlls::kernel32::GetEnvironmentVariableA as *const () as usize)
                    }
                    "SetEnvironmentVariableW" => {
                        Some(dlls::kernel32::SetEnvironmentVariableW as *const () as usize)
                    }
                    "PowerCreateRequest" => {
                        Some(dlls::kernel32::PowerCreateRequest as *const () as usize)
                    }
                    "PowerSetRequest" => {
                        Some(dlls::kernel32::PowerSetRequest as *const () as usize)
                    }
                    "PowerClearRequest" => {
                        Some(dlls::kernel32::PowerClearRequest as *const () as usize)
                    }
                    "GetLastError" => Some(dlls::kernel32::GetLastError as *const () as usize),
                    "SetLastError" => Some(dlls::kernel32::SetLastError as *const () as usize),
                    "DisableThreadLibraryCalls" => {
                        Some(dlls::kernel32::DisableThreadLibraryCalls as *const () as usize)
                    }
                    "GetTickCount" => Some(dlls::kernel32::GetTickCount as *const () as usize),
                    "IsBadStringPtrW" => {
                        Some(dlls::kernel32::IsBadStringPtrW as *const () as usize)
                    }
                    "RaiseException" => Some(dlls::kernel32::RaiseException as *const () as usize),
                    "ExitProcess" => Some(dlls::kernel32::ExitProcess as *const () as usize),
                    "TerminateProcess" => {
                        Some(dlls::kernel32::TerminateProcess as *const () as usize)
                    }
                    "OutputDebugStringA" => {
                        Some(dlls::kernel32::OutputDebugStringA as *const () as usize)
                    }
                    "OutputDebugStringW" => {
                        Some(dlls::kernel32::OutputDebugStringW as *const () as usize)
                    }
                    "SetUnhandledExceptionFilter" => {
                        Some(dlls::kernel32::SetUnhandledExceptionFilter as *const () as usize)
                    }
                    "GetCurrentProcess" => {
                        Some(dlls::kernel32::GetCurrentProcess as *const () as usize)
                    }
                    "GetThreadPriority" => {
                        Some(dlls::kernel32::GetThreadPriority as *const () as usize)
                    }
                    "DuplicateHandle" => {
                        Some(dlls::kernel32::DuplicateHandle as *const () as usize)
                    }
                    "WideCharToMultiByte" => {
                        Some(dlls::kernel32::WideCharToMultiByte as *const () as usize)
                    }
                    "MultiByteToWideChar" => {
                        Some(dlls::kernel32::MultiByteToWideChar as *const () as usize)
                    }
                    "GetCommandLineW" | "GetCommandLineA" => {
                        Some(dlls::kernel32::GetCommandLineW as *const () as usize)
                    }
                    "GetStdHandle" => Some(dlls::kernel32::GetStdHandle as *const () as usize),
                    "GetConsoleScreenBufferInfo" => {
                        Some(dlls::kernel32::GetConsoleScreenBufferInfo as *const () as usize)
                    }
                    "SetConsoleTextAttribute" => {
                        Some(dlls::kernel32::SetConsoleTextAttribute as *const () as usize)
                    }
                    "GetFileType" => Some(dlls::kernel32::GetFileType as *const () as usize),
                    "GetConsoleMode" => Some(dlls::kernel32::GetConsoleMode as *const () as usize),
                    "SetConsoleMode" => Some(dlls::kernel32::SetConsoleMode as *const () as usize),
                    "SetConsoleOutputCP" => {
                        Some(dlls::kernel32::SetConsoleOutputCP as *const () as usize)
                    }
                    "AttachConsole" => Some(dlls::kernel32::AttachConsole as *const () as usize),
                    "SetConsoleCP" => Some(dlls::kernel32::SetConsoleCP as *const () as usize),
                    "WriteConsoleW" | "WriteConsoleA" => {
                        Some(dlls::kernel32::WriteConsoleW as *const () as usize)
                    }
                    "FindResourceW" => Some(dlls::kernel32::FindResourceW as *const () as usize),
                    "FindResourceA" => Some(dlls::kernel32::FindResourceA as *const () as usize),
                    "LoadResource" => Some(dlls::kernel32::LoadResource as *const () as usize),
                    "LockResource" => Some(dlls::kernel32::LockResource as *const () as usize),
                    "SizeofResource" => Some(dlls::kernel32::SizeofResource as *const () as usize),
                    "GetStartupInfoW" | "GetStartupInfoA" => {
                        Some(dlls::kernel32::GetStartupInfoW as *const () as usize)
                    }
                    "GetProcessHeap" => Some(dlls::kernel32::GetProcessHeap as *const () as usize),
                    "HeapAlloc" => Some(dlls::kernel32::HeapAlloc as *const () as usize),
                    "HeapFree" => Some(dlls::kernel32::HeapFree as *const () as usize),
                    "HeapReAlloc" => Some(dlls::kernel32::HeapReAlloc as *const () as usize),
                    "HeapSize" => Some(dlls::kernel32::HeapSize as *const () as usize),
                    "HeapCreate" => Some(dlls::kernel32::HeapCreate as *const () as usize),
                    "HeapDestroy" => Some(dlls::kernel32::HeapDestroy as *const () as usize),
                    "HeapValidate" => Some(dlls::kernel32::HeapValidate as *const () as usize),
                    "LocalAlloc" => Some(dlls::kernel32::LocalAlloc as *const () as usize),
                    "LocalFree" => Some(dlls::kernel32::LocalFree as *const () as usize),
                    "LocalReAlloc" => Some(dlls::kernel32::LocalReAlloc as *const () as usize),
                    "LocalSize" => Some(dlls::kernel32::LocalSize as *const () as usize),
                    "LocalLock" => Some(dlls::kernel32::LocalLock as *const () as usize),
                    "LocalUnlock" => Some(dlls::kernel32::LocalUnlock as *const () as usize),
                    "LocalHandle" => Some(dlls::kernel32::LocalHandle as *const () as usize),
                    "GlobalAlloc" => Some(dlls::kernel32::GlobalAlloc as *const () as usize),
                    "GlobalFree" => Some(dlls::kernel32::GlobalFree as *const () as usize),
                    "GlobalReAlloc" => Some(dlls::kernel32::GlobalReAlloc as *const () as usize),
                    "GlobalSize" => Some(dlls::kernel32::GlobalSize as *const () as usize),
                    "GlobalLock" => Some(dlls::kernel32::GlobalLock as *const () as usize),
                    "GlobalUnlock" => Some(dlls::kernel32::GlobalUnlock as *const () as usize),
                    "GlobalHandle" => Some(dlls::kernel32::GlobalHandle as *const () as usize),
                    "VirtualAlloc" => Some(dlls::kernel32::VirtualAlloc as *const () as usize),
                    "VirtualFree" => Some(dlls::kernel32::VirtualFree as *const () as usize),
                    "InitializeCriticalSection" => {
                        Some(dlls::kernel32::InitializeCriticalSection as *const () as usize)
                    }
                    "InitializeCriticalSectionAndSpinCount" => Some(
                        dlls::kernel32::InitializeCriticalSectionAndSpinCount as *const () as usize,
                    ),
                    "InitializeCriticalSectionEx" => {
                        Some(dlls::kernel32::InitializeCriticalSectionEx as *const () as usize)
                    }
                    "EnterCriticalSection" => {
                        Some(dlls::kernel32::EnterCriticalSection as *const () as usize)
                    }
                    "LeaveCriticalSection" => {
                        Some(dlls::kernel32::LeaveCriticalSection as *const () as usize)
                    }
                    "TryEnterCriticalSection" => {
                        Some(dlls::kernel32::TryEnterCriticalSection as *const () as usize)
                    }
                    "DeleteCriticalSection" => {
                        Some(dlls::kernel32::DeleteCriticalSection as *const () as usize)
                    }
                    "InitializeSRWLock" => {
                        Some(dlls::kernel32::InitializeSRWLock as *const () as usize)
                    }
                    "AcquireSRWLockExclusive" => {
                        Some(dlls::kernel32::AcquireSRWLockExclusive as *const () as usize)
                    }
                    "ReleaseSRWLockExclusive" => {
                        Some(dlls::kernel32::ReleaseSRWLockExclusive as *const () as usize)
                    }
                    "AcquireSRWLockShared" => {
                        Some(dlls::kernel32::AcquireSRWLockShared as *const () as usize)
                    }
                    "ReleaseSRWLockShared" => {
                        Some(dlls::kernel32::ReleaseSRWLockShared as *const () as usize)
                    }
                    "TryAcquireSRWLockExclusive" => {
                        Some(dlls::kernel32::TryAcquireSRWLockExclusive as *const () as usize)
                    }
                    "TryAcquireSRWLockShared" => {
                        Some(dlls::kernel32::TryAcquireSRWLockShared as *const () as usize)
                    }
                    "InitializeConditionVariable" => {
                        Some(dlls::kernel32::InitializeConditionVariable as *const () as usize)
                    }
                    "WakeConditionVariable" => {
                        Some(dlls::kernel32::WakeConditionVariable as *const () as usize)
                    }
                    "WakeAllConditionVariable" => {
                        Some(dlls::kernel32::WakeAllConditionVariable as *const () as usize)
                    }
                    "SleepConditionVariableCS" => {
                        Some(dlls::kernel32::SleepConditionVariableCS as *const () as usize)
                    }
                    "SleepConditionVariableSRW" => {
                        Some(dlls::kernel32::SleepConditionVariableSRW as *const () as usize)
                    }
                    "CreateEventW" => Some(dlls::kernel32::CreateEventW as *const () as usize),
                    "CreateEventA" => Some(dlls::kernel32::CreateEventA as *const () as usize),
                    "OpenEventW" => Some(dlls::kernel32::OpenEventW as *const () as usize),
                    "OpenEventA" => Some(dlls::kernel32::OpenEventA as *const () as usize),
                    "CreateMutexW" => Some(dlls::kernel32::CreateMutexW as *const () as usize),
                    "CreateMutexA" => Some(dlls::kernel32::CreateMutexA as *const () as usize),
                    "OpenMutexW" => Some(dlls::kernel32::OpenMutexW as *const () as usize),
                    "OpenMutexA" => Some(dlls::kernel32::OpenMutexA as *const () as usize),
                    "CreateSemaphoreW" => {
                        Some(dlls::kernel32::CreateSemaphoreW as *const () as usize)
                    }
                    "CreateSemaphoreA" => {
                        Some(dlls::kernel32::CreateSemaphoreA as *const () as usize)
                    }
                    "ReleaseSemaphore" => {
                        Some(dlls::kernel32::ReleaseSemaphore as *const () as usize)
                    }
                    "InitOnceInitialize" => {
                        Some(dlls::kernel32::InitOnceInitialize as *const () as usize)
                    }
                    "InitOnceBeginInitialize" => {
                        Some(dlls::kernel32::InitOnceBeginInitialize as *const () as usize)
                    }
                    "InitOnceComplete" => {
                        Some(dlls::kernel32::InitOnceComplete as *const () as usize)
                    }
                    "InitOnceExecuteOnce" => {
                        Some(dlls::kernel32::InitOnceExecuteOnce as *const () as usize)
                    }
                    "QueryPerformanceCounter" => {
                        Some(dlls::kernel32::QueryPerformanceCounter as *const () as usize)
                    }
                    "QueryPerformanceFrequency" => {
                        Some(dlls::kernel32::QueryPerformanceFrequency as *const () as usize)
                    }
                    "GetUserDefaultUILanguage" => {
                        Some(dlls::kernel32::GetUserDefaultUILanguage as *const () as usize)
                    }
                    "GetSystemDefaultUILanguage" => {
                        Some(dlls::kernel32::GetSystemDefaultUILanguage as *const () as usize)
                    }
                    "GetUserDefaultLangID" => {
                        Some(dlls::kernel32::GetUserDefaultLangID as *const () as usize)
                    }
                    "GetSystemDefaultLangID" => {
                        Some(dlls::kernel32::GetSystemDefaultLangID as *const () as usize)
                    }
                    "GetUserDefaultLCID" => {
                        Some(dlls::kernel32::GetUserDefaultLCID as *const () as usize)
                    }
                    "GetSystemDefaultLCID" => {
                        Some(dlls::kernel32::GetSystemDefaultLCID as *const () as usize)
                    }
                    "GetUserDefaultLocaleName" => {
                        Some(dlls::kernel32::GetUserDefaultLocaleName as *const () as usize)
                    }
                    "GetSystemDefaultLocaleName" => {
                        Some(dlls::kernel32::GetSystemDefaultLocaleName as *const () as usize)
                    }
                    "EncodePointer" => Some(dlls::kernel32::EncodePointer as *const () as usize),
                    "DecodePointer" => Some(dlls::kernel32::DecodePointer as *const () as usize),
                    "EncodeSystemPointer" => {
                        Some(dlls::kernel32::EncodeSystemPointer as *const () as usize)
                    }
                    "DecodeSystemPointer" => {
                        Some(dlls::kernel32::DecodeSystemPointer as *const () as usize)
                    }
                    "TlsAlloc" => Some(dlls::kernel32::TlsAlloc as *const () as usize),
                    "TlsGetValue" => Some(dlls::kernel32::TlsGetValue as *const () as usize),
                    "TlsSetValue" => Some(dlls::kernel32::TlsSetValue as *const () as usize),
                    "TlsFree" => Some(dlls::kernel32::TlsFree as *const () as usize),
                    "FlsAlloc" => Some(dlls::kernel32::FlsAlloc as *const () as usize),
                    "FlsGetValue" => Some(dlls::kernel32::FlsGetValue as *const () as usize),
                    "FlsSetValue" => Some(dlls::kernel32::FlsSetValue as *const () as usize),
                    "FlsFree" => Some(dlls::kernel32::FlsFree as *const () as usize),
                    "CreateFileW" | "CreateFileA" => {
                        Some(dlls::kernel32::CreateFileW as *const () as usize)
                    }
                    "ReadFile" => Some(dlls::kernel32::ReadFile as *const () as usize),
                    "WriteFile" => Some(dlls::kernel32::WriteFile as *const () as usize),
                    "GetFileSize" => Some(dlls::kernel32::GetFileSize as *const () as usize),
                    "GetFileSizeEx" => Some(dlls::kernel32::GetFileSizeEx as *const () as usize),
                    "SetFilePointer" => Some(dlls::kernel32::SetFilePointer as *const () as usize),
                    "SetFilePointerEx" => {
                        Some(dlls::kernel32::SetFilePointerEx as *const () as usize)
                    }
                    // Threading
                    "CreateThread" => Some(dlls::kernel32::CreateThread as *const () as usize),
                    "GetCurrentThread" => {
                        Some(dlls::kernel32::GetCurrentThread as *const () as usize)
                    }
                    "GetCurrentThreadId" => {
                        Some(dlls::kernel32::GetCurrentThreadId as *const () as usize)
                    }
                    "GetCurrentProcessId" => {
                        Some(dlls::kernel32::GetCurrentProcessId as *const () as usize)
                    }
                    "OpenThread" => Some(dlls::kernel32::OpenThread as *const () as usize),
                    "SetPriorityClass" => {
                        Some(dlls::kernel32::SetPriorityClass as *const () as usize)
                    }
                    "GetPriorityClass" => {
                        Some(dlls::kernel32::GetPriorityClass as *const () as usize)
                    }
                    "SetThreadPriority" => {
                        Some(dlls::kernel32::SetThreadPriority as *const () as usize)
                    }
                    "SuspendThread" => Some(dlls::kernel32::SuspendThread as *const () as usize),
                    "ResumeThread" => Some(dlls::kernel32::ResumeThread as *const () as usize),
                    "GetExitCodeThread" => {
                        Some(dlls::kernel32::GetExitCodeThread as *const () as usize)
                    }
                    "GetThreadId" => Some(dlls::kernel32::GetThreadId as *const () as usize),
                    "Sleep" => Some(dlls::kernel32::Sleep as *const () as usize),
                    "SleepEx" => Some(dlls::kernel32::SleepEx as *const () as usize),
                    // Synchronization primitives
                    "CreateEventExW" | "CreateEventExA" => {
                        Some(dlls::kernel32::CreateEventW as *const () as usize)
                    }
                    "SetEvent" => Some(dlls::kernel32::SetEvent as *const () as usize),
                    "ResetEvent" => Some(dlls::kernel32::ResetEvent as *const () as usize),
                    "WaitForSingleObject" | "WaitForSingleObjectEx" => {
                        Some(dlls::kernel32::WaitForSingleObject as *const () as usize)
                    }
                    "WaitForMultipleObjects" | "WaitForMultipleObjectsEx" => {
                        Some(dlls::kernel32::WaitForMultipleObjects as *const () as usize)
                    }
                    "CloseHandle" => Some(dlls::kernel32::CloseHandle as *const () as usize),
                    "GetHandleInformation" => {
                        Some(dlls::kernel32::GetHandleInformation as *const () as usize)
                    }
                    "SetHandleInformation" => {
                        Some(dlls::kernel32::SetHandleInformation as *const () as usize)
                    }
                    "CreateMutexExW" | "CreateMutexExA" => {
                        Some(dlls::kernel32::CreateMutexExW as *const () as usize)
                    }
                    "ReleaseMutex" => Some(dlls::kernel32::ReleaseMutex as *const () as usize),
                    "CreateSemaphoreExW" => {
                        Some(dlls::kernel32::CreateSemaphoreW as *const () as usize)
                    }
                    _ => None,
                }
            }
            "USER32.DLL" | "USER32" | "user32.dll" | "WIN32U.DLL" => match func_name {
                "RegisterClassExW" => Some(dlls::user32::RegisterClassExW as *const () as usize),
                "RegisterClassExA" => Some(dlls::user32::RegisterClassExA as *const () as usize),
                "RegisterClassW" => Some(dlls::user32::RegisterClassW as *const () as usize),
                "RegisterClassA" => Some(dlls::user32::RegisterClassA as *const () as usize),
                "CreateWindowExW" | "CreateWindowExA" | "CreateWindowW" | "CreateWindowA" => {
                    Some(dlls::user32::CreateWindowExW_System as *const () as usize)
                }
                "GetMessageW" | "GetMessageA" => {
                    Some(dlls::user32::GetMessageW as *const () as usize)
                }
                "PeekMessageW" | "PeekMessageA" => {
                    Some(dlls::user32::PeekMessageW as *const () as usize)
                }
                "DispatchMessageW" | "DispatchMessageA" => {
                    Some(dlls::user32::DispatchMessageW_System as *const () as usize)
                }
                "GetWindowLongPtrW" => Some(dlls::user32::GetWindowLongPtrW as *const () as usize),
                "GetWindowLongPtrA" => Some(dlls::user32::GetWindowLongPtrA as *const () as usize),
                "SetWindowLongPtrW" => Some(dlls::user32::SetWindowLongPtrW as *const () as usize),
                "SetWindowLongPtrA" => Some(dlls::user32::SetWindowLongPtrA as *const () as usize),
                "GetWindowLongW" => Some(dlls::user32::GetWindowLongW as *const () as usize),
                "GetWindowLongA" => Some(dlls::user32::GetWindowLongA as *const () as usize),
                "SetWindowLongW" => Some(dlls::user32::SetWindowLongW as *const () as usize),
                "SetWindowLongA" => Some(dlls::user32::SetWindowLongA as *const () as usize),
                "SendMessageW" => Some(dlls::user32::SendMessageW as *const () as usize),
                "SendMessageA" => Some(dlls::user32::SendMessageA as *const () as usize),
                "PostMessageW" => Some(dlls::user32::PostMessageW as *const () as usize),
                "PostMessageA" => Some(dlls::user32::PostMessageA as *const () as usize),
                "GetMessageExtraInfo" => {
                    Some(dlls::user32::GetMessageExtraInfo as *const () as usize)
                }
                "SetMessageExtraInfo" => {
                    Some(dlls::user32::SetMessageExtraInfo as *const () as usize)
                }
                "DefWindowProcW" => Some(dlls::user32::DefWindowProcW as *const () as usize),
                "DefWindowProcA" => Some(dlls::user32::DefWindowProcA as *const () as usize),
                "MessageBoxA" => Some(dlls::user32::MessageBoxA as *const () as usize),
                "MessageBoxW" => Some(dlls::user32::MessageBoxW as *const () as usize),
                "SetWindowTextW" => Some(dlls::user32::SetWindowTextW as *const () as usize),
                "SetWindowTextA" => Some(dlls::user32::SetWindowTextA as *const () as usize),
                "TranslateMessage" => Some(dlls::user32::TranslateMessage as *const () as usize),
                "GetSystemMetrics" => Some(dlls::user32::GetSystemMetrics as *const () as usize),
                "LoadIconA" => Some(dlls::user32::LoadIconA as *const () as usize),
                "LoadIconW" => Some(dlls::user32::LoadIconW as *const () as usize),
                "LoadCursorA" => Some(dlls::user32::LoadCursorA as *const () as usize),
                "LoadCursorW" => Some(dlls::user32::LoadCursorW as *const () as usize),
                "SetCursor" => Some(dlls::user32::SetCursor as *const () as usize),
                "TrackMouseEvent" | "_TrackMouseEvent" => {
                    Some(dlls::user32::TrackMouseEvent as *const () as usize)
                }
                "GetDC" => Some(dlls::user32::GetDC as *const () as usize),
                "ReleaseDC" => Some(dlls::user32::ReleaseDC as *const () as usize),
                "CreateIconIndirect" => {
                    Some(dlls::user32::CreateIconIndirect as *const () as usize)
                }
                "CreateIconFromResource" => {
                    Some(dlls::user32::CreateIconFromResource as *const () as usize)
                }
                "CreateIconFromResourceEx" => {
                    Some(dlls::user32::CreateIconFromResourceEx as *const () as usize)
                }
                "DestroyIcon" => Some(dlls::user32::DestroyIcon as *const () as usize),
                "RegisterRawInputDevices" => {
                    Some(dlls::user32::RegisterRawInputDevices as *const () as usize)
                }
                "GetPointerType" => Some(dlls::user32::GetPointerType as *const () as usize),
                "GetPointerPenInfo" => Some(dlls::user32::GetPointerPenInfo as *const () as usize),
                "LogicalToPhysicalPointForPerMonitorDPI" => {
                    Some(dlls::user32::LogicalToPhysicalPointForPerMonitorDPI as *const () as usize)
                }
                "PhysicalToLogicalPointForPerMonitorDPI" => {
                    Some(dlls::user32::PhysicalToLogicalPointForPerMonitorDPI as *const () as usize)
                }
                "WindowFromPoint" => Some(dlls::user32::WindowFromPoint as *const () as usize),
                "ChildWindowFromPoint" => {
                    Some(dlls::user32::ChildWindowFromPoint as *const () as usize)
                }
                "ChildWindowFromPointEx" => {
                    Some(dlls::user32::ChildWindowFromPointEx as *const () as usize)
                }
                "SetWindowsHookExA" => Some(dlls::user32::SetWindowsHookExA as *const () as usize),
                "SetWindowsHookExW" => Some(dlls::user32::SetWindowsHookExW as *const () as usize),
                "CallNextHookEx" => Some(dlls::user32::CallNextHookEx as *const () as usize),
                "UnhookWindowsHookEx" => {
                    Some(dlls::user32::UnhookWindowsHookEx as *const () as usize)
                }
                "EnumDisplayMonitors" => {
                    Some(dlls::user32::EnumDisplayMonitors as *const () as usize)
                }
                "GetMonitorInfoW" => Some(dlls::user32::GetMonitorInfoW as *const () as usize),
                "GetMonitorInfoA" => Some(dlls::user32::GetMonitorInfoA as *const () as usize),
                "EnumDisplaySettingsW" => {
                    Some(dlls::user32::EnumDisplaySettingsW as *const () as usize)
                }
                "EnumDisplaySettingsA" => {
                    Some(dlls::user32::EnumDisplaySettingsA as *const () as usize)
                }
                "EnumDisplaySettingsExW" => {
                    Some(dlls::user32::EnumDisplaySettingsExW as *const () as usize)
                }
                "EnumDisplaySettingsExA" => {
                    Some(dlls::user32::EnumDisplaySettingsExA as *const () as usize)
                }
                "GetSystemMetricsForDpi" => {
                    Some(dlls::user32::GetSystemMetricsForDpi as *const () as usize)
                }
                "SetProcessDPIAware" => {
                    Some(dlls::user32::SetProcessDPIAware as *const () as usize)
                }
                "GetDpiForWindow" => Some(dlls::user32::GetDpiForWindow as *const () as usize),
                "GetDpiForSystem" => Some(dlls::user32::GetDpiForSystem as *const () as usize),
                "GetSystemDpiForProcess" => {
                    Some(dlls::user32::GetSystemDpiForProcess as *const () as usize)
                }
                "GetDpiForMonitor" => Some(dlls::user32::GetDpiForMonitor as *const () as usize),
                "GetWindowPlacement" => {
                    Some(dlls::user32::GetWindowPlacement as *const () as usize)
                }
                "SetWindowPlacement" => {
                    Some(dlls::user32::SetWindowPlacement as *const () as usize)
                }
                "MonitorFromWindow" => Some(dlls::user32::MonitorFromWindow as *const () as usize),
                "MonitorFromPoint" => Some(dlls::user32::MonitorFromPoint as *const () as usize),
                "MonitorFromRect" => Some(dlls::user32::MonitorFromRect as *const () as usize),
                "RegisterTouchWindow" => {
                    Some(dlls::user32::RegisterTouchWindow as *const () as usize)
                }
                "UnregisterTouchWindow" => {
                    Some(dlls::user32::UnregisterTouchWindow as *const () as usize)
                }
                "IsTouchWindow" => Some(dlls::user32::IsTouchWindow as *const () as usize),
                "GetTouchInputInfo" => Some(dlls::user32::GetTouchInputInfo as *const () as usize),
                "CloseTouchInputHandle" => {
                    Some(dlls::user32::CloseTouchInputHandle as *const () as usize)
                }
                "RegisterPointerInputTarget" => {
                    Some(dlls::user32::RegisterPointerInputTarget as *const () as usize)
                }
                "UnregisterPointerInputTarget" => {
                    Some(dlls::user32::UnregisterPointerInputTarget as *const () as usize)
                }
                "GetClientRect" => Some(dlls::user32::GetClientRect as *const () as usize),
                "GetWindowRect" => Some(dlls::user32::GetWindowRect as *const () as usize),
                "AdjustWindowRectEx" => {
                    Some(dlls::user32::AdjustWindowRectEx as *const () as usize)
                }
                "AdjustWindowRect" => Some(dlls::user32::AdjustWindowRect as *const () as usize),
                "SetWindowPos" => Some(dlls::user32::SetWindowPos as *const () as usize),
                "MoveWindow" => Some(dlls::user32::MoveWindow as *const () as usize),
                "ShowWindow" => Some(dlls::user32::ShowWindow as *const () as usize),
                "UpdateWindow" => Some(dlls::user32::UpdateWindow as *const () as usize),
                "ClientToScreen" => Some(dlls::user32::ClientToScreen as *const () as usize),
                "ScreenToClient" => Some(dlls::user32::ScreenToClient as *const () as usize),
                "MapWindowPoints" => Some(dlls::user32::MapWindowPoints as *const () as usize),
                "GetCursorPos" => Some(dlls::user32::GetCursorPos as *const () as usize),
                "SetCursorPos" => Some(dlls::user32::SetCursorPos as *const () as usize),
                "ClipCursor" => Some(dlls::user32::ClipCursor as *const () as usize),
                "GetClipCursor" => Some(dlls::user32::GetClipCursor as *const () as usize),
                "SetForegroundWindow" => {
                    Some(dlls::user32::SetForegroundWindow as *const () as usize)
                }
                "GetForegroundWindow" => {
                    Some(dlls::user32::GetForegroundWindow as *const () as usize)
                }
                "SetActiveWindow" => Some(dlls::user32::SetActiveWindow as *const () as usize),
                "GetActiveWindow" => Some(dlls::user32::GetActiveWindow as *const () as usize),
                "SetFocus" => Some(dlls::user32::SetFocus as *const () as usize),
                "GetFocus" => Some(dlls::user32::GetFocus as *const () as usize),
                "BringWindowToTop" => Some(dlls::user32::BringWindowToTop as *const () as usize),
                "IsWindowVisible" => Some(dlls::user32::IsWindowVisible as *const () as usize),
                "IsIconic" => Some(dlls::user32::IsIconic as *const () as usize),
                "IsZoomed" => Some(dlls::user32::IsZoomed as *const () as usize),
                "IsWindow" => Some(dlls::user32::IsWindow as *const () as usize),
                "EnableWindow" => Some(dlls::user32::EnableWindow as *const () as usize),
                "IsWindowEnabled" => Some(dlls::user32::IsWindowEnabled as *const () as usize),
                "SetCapture" => Some(dlls::user32::SetCapture as *const () as usize),
                "ReleaseCapture" => Some(dlls::user32::ReleaseCapture as *const () as usize),
                "GetCapture" => Some(dlls::user32::GetCapture as *const () as usize),
                "GetAsyncKeyState" => Some(dlls::user32::GetAsyncKeyState as *const () as usize),
                "GetKeyState" => Some(dlls::user32::GetKeyState as *const () as usize),
                "GetKeyboardState" => Some(dlls::user32::GetKeyboardState as *const () as usize),
                "GetKeyboardLayout" => Some(dlls::user32::GetKeyboardLayout as *const () as usize),
                "MapVirtualKeyA" => Some(dlls::user32::MapVirtualKeyA as *const () as usize),
                "MapVirtualKeyW" => Some(dlls::user32::MapVirtualKeyW as *const () as usize),
                "ToUnicode" => Some(dlls::user32::ToUnicode as *const () as usize),
                "ToUnicodeEx" => Some(dlls::user32::ToUnicodeEx as *const () as usize),
                "ToAscii" => Some(dlls::user32::ToAscii as *const () as usize),
                "ToAsciiEx" => Some(dlls::user32::ToAsciiEx as *const () as usize),
                "VkKeyScanA" => Some(dlls::user32::VkKeyScanA as *const () as usize),
                "VkKeyScanW" => Some(dlls::user32::VkKeyScanW as *const () as usize),
                _ => None,
            },
            "GDI32.DLL" | "OPENGL32.DLL" => match func_name {
                "GetDC" => Some(dlls::gdi32::GetDC as *const () as usize),
                "ReleaseDC" => Some(dlls::gdi32::ReleaseDC as *const () as usize),
                "CreateCompatibleDC" => Some(dlls::gdi32::CreateCompatibleDC as *const () as usize),
                "CreateCompatibleBitmap" => {
                    Some(dlls::gdi32::CreateCompatibleBitmap as *const () as usize)
                }
                "CreateBitmap" => Some(dlls::gdi32::CreateBitmap as *const () as usize),
                "SelectObject" => Some(dlls::gdi32::SelectObject as *const () as usize),
                "DeleteObject" => Some(dlls::gdi32::DeleteObject as *const () as usize),
                "CreateDIBSection" => Some(dlls::gdi32::CreateDIBSection as *const () as usize),
                "SwapBuffers" => Some(dlls::gdi32::SwapBuffers as *const () as usize),
                "BitBlt" => Some(dlls::gdi32::BitBlt as *const () as usize),
                "StretchDIBits" => Some(dlls::gdi32::StretchDIBits as *const () as usize),
                "ChoosePixelFormat" => Some(dlls::gdi32::ChoosePixelFormat as *const () as usize),
                "SetPixelFormat" => Some(dlls::gdi32::SetPixelFormat as *const () as usize),
                "wglCreateContext" => Some(dlls::gdi32::wglCreateContext as *const () as usize),
                "wglMakeCurrent" => Some(dlls::gdi32::wglMakeCurrent as *const () as usize),
                "wglGetProcAddress" => Some(dlls::gdi32::wglGetProcAddress as *const () as usize),
                "wglDeleteContext" => Some(dlls::gdi32::wglDeleteContext as *const () as usize),
                "GetDeviceCaps" => Some(dlls::gdi32::GetDeviceCaps as *const () as usize),
                _ => None,
            },
            "DXGI.DLL" | "DXGI" | "dxgi.dll" => {
                // First try DXVK's real implementation (D3D11 → Vulkan)
                if let Some(addr) =
                    crate::dlls::dxvk_loader::resolve_dxvk_symbol("dxgi.dll", func_name)
                {
                    return Some(addr);
                }
                // Fallback stubs
                match func_name {
                    "CreateDXGIFactory" | "CreateDXGIFactory1" | "CreateDXGIFactory2" => {
                        Some(dlls::dxgi::CreateDXGIFactory1 as *const () as usize)
                    }
                    "EnumAdapters" | "EnumAdapters1" => {
                        Some(dlls::dxgi::EnumAdapters1 as *const () as usize)
                    }
                    _ => None,
                }
            }
            "D3D11.DLL" | "D3D11" | "d3d11.dll" => {
                // Route through DXVK's real D3D11 implementation
                if let Some(addr) =
                    crate::dlls::dxvk_loader::resolve_dxvk_symbol("d3d11.dll", func_name)
                {
                    return Some(addr);
                }
                match func_name {
                    "D3D11CreateDevice" => {
                        Some(dlls::d3d11::D3D11CreateDevice as *const () as usize)
                    }
                    "D3D11CreateDeviceAndSwapChain" => {
                        Some(dlls::d3d11::D3D11CreateDeviceAndSwapChain as *const () as usize)
                    }
                    _ => None,
                }
            }
            "D3D10CORE.DLL" | "D3D10CORE" | "d3d10core.dll" => {
                if let Some(addr) =
                    crate::dlls::dxvk_loader::resolve_dxvk_symbol("d3d10core.dll", func_name)
                {
                    return Some(addr);
                }
                None
            }
            dll if dll.starts_with("OLE32")
                || dll.starts_with("OLEAUT32")
                || dll.starts_with("COMBASE")
                || dll.starts_with("SHELL32") =>
            {
                match func_name {
                    "CoInitializeEx" | "CoInitialize" => {
                        Some(dlls::shell::CoInitializeEx as *const () as usize)
                    }
                    "CoUninitialize" => Some(dlls::shell::CoUninitialize as *const () as usize),
                    "CoCreateInstance" => Some(dlls::shell::CoCreateInstance as *const () as usize),
                    "CoTaskMemAlloc" => Some(dlls::shell::CoTaskMemAlloc as *const () as usize),
                    "CoTaskMemFree" => Some(dlls::shell::CoTaskMemFree as *const () as usize),
                    "PropVariantClear" => Some(dlls::shell::PropVariantClear as *const () as usize),
                    "VariantInit" => Some(dlls::shell::VariantInit as *const () as usize),
                    "SysAllocString" => Some(dlls::shell::SysAllocString as *const () as usize),
                    "SysFreeString" => Some(dlls::shell::SysFreeString as *const () as usize),
                    "SHGetFolderPathW" => Some(dlls::shell::SHGetFolderPathW as *const () as usize),
                    "SHGetFolderPathA" => Some(dlls::shell::SHGetFolderPathA as *const () as usize),
                    "SHGetKnownFolderPath" => {
                        Some(dlls::shell::SHGetKnownFolderPath as *const () as usize)
                    }
                    "SHGetSpecialFolderPathW" => {
                        Some(dlls::shell::SHGetSpecialFolderPathW as *const () as usize)
                    }
                    "SHGetSpecialFolderPathA" => {
                        Some(dlls::shell::SHGetSpecialFolderPathA as *const () as usize)
                    }
                    "CommandLineToArgvW" => {
                        Some(dlls::shell::CommandLineToArgvW as *const () as usize)
                    }
                    "DragAcceptFiles" => Some(dlls::shell::DragAcceptFiles as *const () as usize),
                    "DragFinish" => Some(dlls::shell::DragFinish as *const () as usize),
                    "DragQueryFileW" => Some(dlls::shell::DragQueryFileW as *const () as usize),
                    "DragQueryFileA" => Some(dlls::shell::DragQueryFileA as *const () as usize),
                    "DragQueryPoint" => Some(dlls::shell::DragQueryPoint as *const () as usize),
                    _ => None,
                }
            }
            "WINMM.DLL" => match func_name {
                "timeGetTime" => Some(dlls::winmm::timeGetTime as *const () as usize),
                "timeBeginPeriod" => Some(dlls::winmm::timeBeginPeriod as *const () as usize),
                "timeEndPeriod" => Some(dlls::winmm::timeEndPeriod as *const () as usize),
                "waveOutOpen" => Some(dlls::winmm::waveOutOpen as *const () as usize),
                "waveOutWrite" => Some(dlls::winmm::waveOutWrite as *const () as usize),
                "waveOutPrepareHeader" => {
                    Some(dlls::winmm::waveOutPrepareHeader as *const () as usize)
                }
                "waveOutUnprepareHeader" => {
                    Some(dlls::winmm::waveOutUnprepareHeader as *const () as usize)
                }
                "waveOutReset" => Some(dlls::winmm::waveOutReset as *const () as usize),
                "waveOutClose" => Some(dlls::winmm::waveOutClose as *const () as usize),
                _ => None,
            },
            "CRYPT32.DLL" | "CRYPT32" | "crypt32.dll" => match func_name {
                "CertOpenSystemStoreA" => {
                    Some(dlls::crypt32::CertOpenSystemStoreA as *const () as usize)
                }
                "CertOpenSystemStoreW" => {
                    Some(dlls::crypt32::CertOpenSystemStoreW as *const () as usize)
                }
                "CertCloseStore" => Some(dlls::crypt32::CertCloseStore as *const () as usize),
                "CertFreeCertificateContext" => {
                    Some(dlls::crypt32::CertFreeCertificateContext as *const () as usize)
                }
                "CertEnumCertificatesInStore" => {
                    Some(dlls::crypt32::CertEnumCertificatesInStore as *const () as usize)
                }
                _ => None,
            },
            "ADVAPI32.DLL" => match func_name {
                "RegOpenKeyExW" => Some(dlls::advapi32::RegOpenKeyExW as *const () as usize),
                "RegQueryValueExW" => Some(dlls::advapi32::RegQueryValueExW as *const () as usize),
                "RegCloseKey" => Some(dlls::advapi32::RegCloseKey as *const () as usize),
                "AdjustTokenPrivileges" => {
                    Some(dlls::advapi32::AdjustTokenPrivileges as *const () as usize)
                }
                "GetTokenInformation" => {
                    Some(dlls::advapi32::GetTokenInformation as *const () as usize)
                }
                "OpenProcessToken" => Some(dlls::advapi32::OpenProcessToken as *const () as usize),
                "GetSidSubAuthority" => {
                    Some(dlls::advapi32::GetSidSubAuthority as *const () as usize)
                }
                "GetSidSubAuthorityCount" => {
                    Some(dlls::advapi32::GetSidSubAuthorityCount as *const () as usize)
                }
                "LookupPrivilegeValueW" => {
                    Some(dlls::advapi32::LookupPrivilegeValueW as *const () as usize)
                }
                _ => None,
            },
            "WS2_32.DLL" | "WSOCK32.DLL" => match func_name {
                "WSAStartup" => Some(dlls::ws2_32::WSAStartup as *const () as usize),
                "WSACleanup" => Some(dlls::ws2_32::WSACleanup as *const () as usize),
                "socket" => Some(dlls::ws2_32::socket as *const () as usize),
                "closesocket" => Some(dlls::ws2_32::closesocket as *const () as usize),
                _ => None,
            },
            "NTDLL.DLL" | "NTDLL" => match func_name {
                "NtCreateEvent" => Some(dlls::ntdll::NtCreateEvent as *const () as usize),
                "NtSetEvent" => Some(dlls::ntdll::NtSetEvent as *const () as usize),
                "NtResetEvent" => Some(dlls::ntdll::NtResetEvent as *const () as usize),
                "NtCreateMutant" => Some(dlls::ntdll::NtCreateMutant as *const () as usize),
                "NtReleaseMutant" => Some(dlls::ntdll::NtReleaseMutant as *const () as usize),
                "NtAllocateVirtualMemory" => {
                    Some(dlls::ntdll::NtAllocateVirtualMemory as *const () as usize)
                }
                "NtProtectVirtualMemory" => {
                    Some(dlls::ntdll::NtProtectVirtualMemory as *const () as usize)
                }
                "NtClose" => Some(dlls::ntdll::NtClose as *const () as usize),
                "RtlLookupFunctionEntry" => {
                    Some(dlls::ntdll::RtlLookupFunctionEntry as *const () as usize)
                }
                "RtlVirtualUnwind" => Some(dlls::ntdll::RtlVirtualUnwind as *const () as usize),
                "RtlAllocateHeap" => Some(dlls::ntdll::RtlAllocateHeap as *const () as usize),
                "RtlFreeHeap" => Some(dlls::ntdll::RtlFreeHeap as *const () as usize),
                "RtlReAllocateHeap" => Some(dlls::ntdll::RtlReAllocateHeap as *const () as usize),
                "RtlSizeHeap" => Some(dlls::ntdll::RtlSizeHeap as *const () as usize),
                "RtlInitializeCriticalSection" => {
                    Some(dlls::ntdll::RtlInitializeCriticalSection as *const () as usize)
                }
                "RtlInitializeCriticalSectionAndSpinCount" => Some(
                    dlls::ntdll::RtlInitializeCriticalSectionAndSpinCount as *const () as usize,
                ),
                "RtlInitializeCriticalSectionEx" => {
                    Some(dlls::ntdll::RtlInitializeCriticalSectionEx as *const () as usize)
                }
                "RtlEnterCriticalSection" => {
                    Some(dlls::ntdll::RtlEnterCriticalSection as *const () as usize)
                }
                "RtlLeaveCriticalSection" => {
                    Some(dlls::ntdll::RtlLeaveCriticalSection as *const () as usize)
                }
                "RtlDeleteCriticalSection" => {
                    Some(dlls::ntdll::RtlDeleteCriticalSection as *const () as usize)
                }
                "RtlTryEnterCriticalSection" => {
                    Some(dlls::ntdll::RtlTryEnterCriticalSection as *const () as usize)
                }
                "RtlInitializeSRWLock" => {
                    Some(dlls::ntdll::RtlInitializeSRWLock as *const () as usize)
                }
                "RtlAcquireSRWLockExclusive" => {
                    Some(dlls::ntdll::RtlAcquireSRWLockExclusive as *const () as usize)
                }
                "RtlReleaseSRWLockExclusive" => {
                    Some(dlls::ntdll::RtlReleaseSRWLockExclusive as *const () as usize)
                }
                "RtlAcquireSRWLockShared" => {
                    Some(dlls::ntdll::RtlAcquireSRWLockShared as *const () as usize)
                }
                "RtlReleaseSRWLockShared" => {
                    Some(dlls::ntdll::RtlReleaseSRWLockShared as *const () as usize)
                }
                "RtlTryAcquireSRWLockExclusive" => {
                    Some(dlls::ntdll::RtlTryAcquireSRWLockExclusive as *const () as usize)
                }
                "RtlTryAcquireSRWLockShared" => {
                    Some(dlls::ntdll::RtlTryAcquireSRWLockShared as *const () as usize)
                }
                "RtlEncodePointer" => Some(dlls::ntdll::RtlEncodePointer as *const () as usize),
                "RtlDecodePointer" => Some(dlls::ntdll::RtlDecodePointer as *const () as usize),
                "RtlEncodeSystemPointer" => {
                    Some(dlls::ntdll::RtlEncodeSystemPointer as *const () as usize)
                }
                "RtlDecodeSystemPointer" => {
                    Some(dlls::ntdll::RtlDecodeSystemPointer as *const () as usize)
                }
                "RtlGetVersion" => Some(dlls::ntdll::RtlGetVersion as *const () as usize),
                _ => None,
            },
            "SHCORE.DLL" | "SHCORE" | "shcore.dll" => match func_name {
                "SetProcessDpiAwareness" => {
                    Some(dlls::user32::SetProcessDpiAwareness as *const () as usize)
                }
                "GetDpiForMonitor" => Some(dlls::user32::GetDpiForMonitor as *const () as usize),
                _ => None,
            },
            // Universal CRT: Map ALL api-ms-win-crt-* DLL names to our CRT implementations.
            // DXVK 2.7.1 uses UCRT instead of legacy msvcrt.dll.
            "MSVCRT.DLL"
            | "UCRTBASE.DLL"
            | "VCRUNTIME140.DLL"
            | "VCRUNTIME140_1.DLL"
            | "API-MS-WIN-CRT-RUNTIME-L1-1-0.DLL"
            | "API-MS-WIN-CRT-STDIO-L1-1-0.DLL"
            | "API-MS-WIN-CRT-HEAP-L1-1-0.DLL"
            | "API-MS-WIN-CRT-MATH-L1-1-0.DLL"
            | "API-MS-WIN-CRT-STRING-L1-1-0.DLL"
            | "API-MS-WIN-CRT-CONVERT-L1-1-0.DLL"
            | "API-MS-WIN-CRT-ENVIRONMENT-L1-1-0.DLL"
            | "API-MS-WIN-CRT-FILESYSTEM-L1-1-0.DLL"
            | "API-MS-WIN-CRT-LOCALE-L1-1-0.DLL"
            | "API-MS-WIN-CRT-PRIVATE-L1-1-0.DLL"
            | "API-MS-WIN-CRT-TIME-L1-1-0.DLL"
            | "API-MS-WIN-CRT-UTILITY-L1-1-0.DLL"
            | "API-MS-WIN-CRT-MULTIBYTE-L1-1-0.DLL"
            | "API-MS-WIN-CRT-CONIO-L1-1-0.DLL"
            | "API-MS-WIN-CRT-PROCESS-L1-1-0.DLL" => match func_name {
                // Memory management
                "malloc" => Some(dlls::crt::msvcrt_malloc as *const () as usize),
                "free" => Some(dlls::crt::msvcrt_free as *const () as usize),
                "calloc" => Some(dlls::crt::msvcrt_calloc as *const () as usize),
                "realloc" => Some(dlls::crt::msvcrt_realloc as *const () as usize),
                "_aligned_malloc" => Some(dlls::crt::msvcrt_aligned_malloc as *const () as usize),
                "_aligned_free" => Some(dlls::crt::msvcrt_aligned_free as *const () as usize),
                "_aligned_realloc" => Some(dlls::crt::msvcrt_aligned_realloc as *const () as usize),
                "_msize" => Some(dlls::crt::msvcrt_msize as *const () as usize),
                // String/memory operations
                "memset" => Some(dlls::crt::msvcrt_memset as *const () as usize),
                "memcpy" => Some(dlls::crt::msvcrt_memcpy as *const () as usize),
                "memcpy_s" => Some(dlls::crt::msvcrt_memcpy_s as *const () as usize),
                "memmove" => Some(dlls::crt::msvcrt_memmove as *const () as usize),
                "memmove_s" => Some(dlls::crt::msvcrt_memmove_s as *const () as usize),
                "memchr" => Some(dlls::crt::msvcrt_memchr as *const () as usize),
                "memcmp" => Some(dlls::crt::msvcrt_memcmp as *const () as usize),
                "strlen" => Some(dlls::crt::msvcrt_strlen as *const () as usize),
                "wcslen" => Some(dlls::crt::msvcrt_wcslen as *const () as usize),
                "strcmp" => Some(dlls::crt::msvcrt_strcmp as *const () as usize),
                "strncmp" => Some(dlls::crt::msvcrt_strncmp as *const () as usize),
                "strcpy" => Some(dlls::crt::msvcrt_strcpy as *const () as usize),
                "strcpy_s" => Some(dlls::crt::msvcrt_strcpy_s as *const () as usize),
                "strncpy" => Some(dlls::crt::msvcrt_strncpy as *const () as usize),
                "strncpy_s" => Some(dlls::crt::msvcrt_strncpy_s as *const () as usize),
                "strcat" => Some(dlls::crt::msvcrt_strcat as *const () as usize),
                "strcat_s" => Some(dlls::crt::msvcrt_strcat_s as *const () as usize),
                "strchr" => Some(dlls::crt::msvcrt_strchr as *const () as usize),
                "strrchr" => Some(dlls::crt::msvcrt_strrchr as *const () as usize),
                "strstr" => Some(dlls::crt::msvcrt_strstr as *const () as usize),
                "_stricmp" | "_strcmpi" => Some(dlls::crt::msvcrt_stricmp as *const () as usize),
                "_strnicmp" => Some(dlls::crt::msvcrt_strnicmp as *const () as usize),
                "sprintf" => Some(dlls::crt::msvcrt_sprintf_stub as *const () as usize),
                "sprintf_s" | "_o_sprintf_s" => {
                    Some(dlls::crt::msvcrt_sprintf_s as *const () as usize)
                }
                "snprintf" | "_snprintf" => {
                    Some(dlls::crt::msvcrt_sprintf_stub as *const () as usize)
                }
                "_snprintf_s" => Some(dlls::crt::msvcrt_snprintf_s as *const () as usize),
                "sscanf" | "sscanf_s" => Some(dlls::crt::msvcrt_sscanf_stub as *const () as usize),
                "wcscmp" => Some(dlls::crt::msvcrt_wcscmp as *const () as usize),
                "wcsncmp" => Some(dlls::crt::msvcrt_wcsncmp as *const () as usize),
                "wcscpy" => Some(dlls::crt::msvcrt_wcscpy as *const () as usize),
                "wcscpy_s" => Some(dlls::crt::msvcrt_wcscpy_s as *const () as usize),
                "wcsncpy" => Some(dlls::crt::msvcrt_wcsncpy as *const () as usize),
                "wcsncpy_s" => Some(dlls::crt::msvcrt_wcsncpy_s as *const () as usize),
                "wcscat" => Some(dlls::crt::msvcrt_wcscat as *const () as usize),
                "wcscat_s" => Some(dlls::crt::msvcrt_wcscat_s as *const () as usize),
                "wcschr" => Some(dlls::crt::msvcrt_wcschr as *const () as usize),
                "wcsrchr" => Some(dlls::crt::msvcrt_wcsrchr as *const () as usize),
                "_wcsicmp" => Some(dlls::crt::msvcrt_wcsicmp as *const () as usize),
                "towlower" => Some(dlls::crt::msvcrt_towlower as *const () as usize),
                "towupper" => Some(dlls::crt::msvcrt_towupper as *const () as usize),
                "tolower" => Some(dlls::crt::msvcrt_tolower as *const () as usize),
                "toupper" => Some(dlls::crt::msvcrt_toupper as *const () as usize),
                "isdigit" => Some(dlls::crt::msvcrt_isdigit as *const () as usize),
                "isalpha" => Some(dlls::crt::msvcrt_isalpha as *const () as usize),
                "isspace" => Some(dlls::crt::msvcrt_isspace as *const () as usize),
                "iswdigit" => Some(dlls::crt::msvcrt_iswdigit as *const () as usize),
                // Conversion
                "strtoul" | "_o_strtoul" => Some(dlls::crt::msvcrt_strtoul as *const () as usize),
                "strtol" | "_o_strtol" => Some(dlls::crt::msvcrt_strtol as *const () as usize),
                "strtod" | "_o_strtod" => Some(dlls::crt::msvcrt_strtod as *const () as usize),
                "atoi" => Some(dlls::crt::msvcrt_atoi as *const () as usize),
                "atof" => Some(dlls::crt::msvcrt_atof as *const () as usize),
                "_strtoi64" => Some(dlls::crt::msvcrt_strtoi64 as *const () as usize),
                "_strtoui64" => Some(dlls::crt::msvcrt_strtoui64 as *const () as usize),
                "btowc" | "_o_btowc" => Some(dlls::crt::msvcrt_btowc as *const () as usize),
                "wctob" | "_o_wctob" => Some(dlls::crt::msvcrt_wctob as *const () as usize),
                "mbrtowc" | "_o_mbrtowc" => Some(dlls::crt::msvcrt_mbrtowc as *const () as usize),
                "wcrtomb" | "_o_wcrtomb" => Some(dlls::crt::msvcrt_wcrtomb as *const () as usize),
                "mbsrtowcs" | "_o_mbsrtowcs" => {
                    Some(dlls::crt::msvcrt_mbsrtowcs as *const () as usize)
                }
                // Environment
                "getenv" | "_o_getenv" => Some(dlls::crt::msvcrt_getenv as *const () as usize),
                "_wgetenv" => Some(dlls::crt::msvcrt_wgetenv as *const () as usize),
                "_putenv_s" => Some(dlls::crt::msvcrt_putenv_s as *const () as usize),
                // Locale
                "setlocale" | "_o_setlocale" => {
                    Some(dlls::crt::msvcrt_setlocale as *const () as usize)
                }
                "localeconv" | "_o_localeconv" => {
                    Some(dlls::crt::msvcrt_localeconv as *const () as usize)
                }
                "___mb_cur_max_func" | "__mb_cur_max" => {
                    Some(dlls::crt::msvcrt_mb_cur_max_func as *const () as usize)
                }
                // File I/O
                "fopen" | "_o_fopen" => Some(dlls::crt::msvcrt_fopen as *const () as usize),
                "fopen_s" | "_o_fopen_s" => Some(dlls::crt::msvcrt_fopen_s as *const () as usize),
                "_wfopen" | "_o__wfopen" => Some(dlls::crt::msvcrt_wfopen as *const () as usize),
                "_wfopen_s" | "_o__wfopen_s" => {
                    Some(dlls::crt::msvcrt_wfopen_s as *const () as usize)
                }
                "_wfsopen" => Some(dlls::crt::msvcrt_wfsopen as *const () as usize),
                "_filelengthi64" => Some(dlls::crt::msvcrt_filelengthi64 as *const () as usize),
                "_fileno" => Some(dlls::crt::msvcrt_fileno as *const () as usize),
                "_lseeki64" | "_lseek" => Some(dlls::crt::msvcrt_lseeki64 as *const () as usize),
                "fclose" | "_o_fclose" => Some(dlls::crt::msvcrt_fclose as *const () as usize),
                "fread" | "_o_fread" | "_fread_nolock" => {
                    Some(dlls::crt::msvcrt_fread as *const () as usize)
                }
                "fseek" | "_o_fseek" | "_fseeki64" => {
                    Some(dlls::crt::msvcrt_fseek as *const () as usize)
                }
                "ftell" | "_o_ftell" | "_ftelli64" => {
                    Some(dlls::crt::msvcrt_ftell as *const () as usize)
                }
                "fgetpos" | "_o_fgetpos" => Some(dlls::crt::msvcrt_fgetpos as *const () as usize),
                "fsetpos" | "_o_fsetpos" => Some(dlls::crt::msvcrt_fsetpos as *const () as usize),
                "fwrite" | "_o_fwrite" | "_fwrite_nolock" => {
                    Some(dlls::crt::fwrite as *const () as usize)
                }
                "fflush" => Some(dlls::crt::fflush as *const () as usize),
                "feof" => Some(dlls::crt::msvcrt_feof as *const () as usize),
                "ferror" => Some(dlls::crt::msvcrt_ferror as *const () as usize),
                "remove" | "_o_remove" => Some(dlls::crt::msvcrt_remove as *const () as usize),
                "_lock_file" => Some(dlls::crt::msvcrt_lock_file as *const () as usize),
                "_unlock_file" => Some(dlls::crt::msvcrt_unlock_file as *const () as usize),
                "fputs" | "_o_fputs" => Some(dlls::crt::msvcrt_fputs as *const () as usize),
                "fgets" | "_o_fgets" => Some(dlls::crt::msvcrt_fgets as *const () as usize),
                "fputc" | "_o_fputc" => Some(dlls::crt::msvcrt_fputc as *const () as usize),
                "fgetc" | "_o_fgetc" => Some(dlls::crt::msvcrt_fgetc as *const () as usize),
                "ungetc" | "_o_ungetc" => Some(dlls::crt::msvcrt_ungetc as *const () as usize),
                // Time
                "strftime" | "_o_strftime" => {
                    Some(dlls::crt::msvcrt_strftime as *const () as usize)
                }
                "wcsftime" | "_o_wcsftime" => {
                    Some(dlls::crt::msvcrt_wcsftime as *const () as usize)
                }
                "time" | "_time64" => Some(dlls::crt::msvcrt_time as *const () as usize),
                "clock" => Some(dlls::crt::msvcrt_clock as *const () as usize),
                "localtime" | "_localtime64" => {
                    Some(dlls::crt::msvcrt_localtime as *const () as usize)
                }
                "gmtime" | "_gmtime64" => Some(dlls::crt::msvcrt_gmtime as *const () as usize),
                // Utility
                "rand_s" => Some(dlls::crt::msvcrt_rand_s as *const () as usize),
                "rand" => Some(dlls::crt::msvcrt_rand as *const () as usize),
                "srand" => Some(dlls::crt::msvcrt_srand as *const () as usize),
                "qsort" | "qsort_s" => Some(dlls::crt::msvcrt_qsort as *const () as usize),
                "bsearch" => Some(dlls::crt::msvcrt_bsearch as *const () as usize),
                "abs" => Some(dlls::crt::msvcrt_abs as *const () as usize),
                // longjmp/setjmp
                "longjmp" | "_longjmpex" => {
                    Some(dlls::crt::msvcrt_longjmp as *const () as usize)
                }
                "__intrinsic_setjmpex" | "_setjmpex" | "_setjmp" | "setjmp" => {
                    Some(dlls::crt::msvcrt_setjmp as *const () as usize)
                }
                "__CxxFrameHandler3" | "__CxxFrameHandler4" => {
                    Some(dlls::crt::msvcrt_cxx_frame_handler as *const () as usize)
                }
                "_CxxThrowException" => {
                    Some(dlls::crt::msvcrt_cxx_throw_exception as *const () as usize)
                }
                // Math
                "sqrt" | "sqrtf" => Some(dlls::crt::msvcrt_sqrt as *const () as usize),
                "sin" | "sinf" => Some(dlls::crt::msvcrt_sin as *const () as usize),
                "cos" | "cosf" => Some(dlls::crt::msvcrt_cos as *const () as usize),
                "tan" | "tanf" => Some(dlls::crt::msvcrt_tan as *const () as usize),
                "atan2" | "atan2f" => Some(dlls::crt::msvcrt_atan2 as *const () as usize),
                "pow" | "powf" => Some(dlls::crt::msvcrt_pow as *const () as usize),
                "log" | "logf" => Some(dlls::crt::msvcrt_log as *const () as usize),
                "log2" | "log2f" => Some(dlls::crt::msvcrt_log2 as *const () as usize),
                "log10" | "log10f" => Some(dlls::crt::msvcrt_log10 as *const () as usize),
                "exp" | "expf" => Some(dlls::crt::msvcrt_exp as *const () as usize),
                "floor" | "floorf" => Some(dlls::crt::msvcrt_floor as *const () as usize),
                "ceil" | "ceilf" => Some(dlls::crt::msvcrt_ceil as *const () as usize),
                "fabs" | "fabsf" => Some(dlls::crt::msvcrt_fabs as *const () as usize),
                "fmod" | "fmodf" => Some(dlls::crt::msvcrt_fmod as *const () as usize),
                "round" | "roundf" => Some(dlls::crt::msvcrt_round as *const () as usize),
                "trunc" | "truncf" => Some(dlls::crt::msvcrt_trunc as *const () as usize),
                "ldexp" => Some(dlls::crt::msvcrt_ldexp as *const () as usize),
                "frexp" => Some(dlls::crt::msvcrt_frexp as *const () as usize),
                // Existing CRT init/exit
                "__p___argc" | "__getmainargs" => Some(dlls::crt::__p___argc as *const () as usize),
                "__p___argv" | "__wgetmainargs" => {
                    Some(dlls::crt::__p___argv as *const () as usize)
                }
                "__p__commode" => Some(dlls::crt::__p__commode as *const () as usize),
                "__p__fmode" => Some(dlls::crt::__p__fmode as *const () as usize),
                "_fmode" => Some(unsafe { dlls::crt::get_fmode_ptr() } as usize),
                "_commode" => Some(unsafe { dlls::crt::get_commode_ptr() } as usize),
                "__argc" => Some(unsafe { dlls::crt::get_argc_ptr() } as usize),
                "__argv" => Some(unsafe { dlls::crt::get_argv_ptr() } as usize),
                "_environ" => Some(unsafe { dlls::crt::get_environ_ptr() } as usize),
                "_initialize_onexit_table" => {
                    Some(dlls::crt::_initialize_onexit_table as *const () as usize)
                }
                "_register_onexit_function" => {
                    Some(dlls::crt::_register_onexit_function as *const () as usize)
                }
                "_execute_onexit_table" => {
                    Some(dlls::crt::_execute_onexit_table as *const () as usize)
                }
                "_beginthread" | "_o__beginthread" => {
                    Some(dlls::crt::msvcrt_beginthread as *const () as usize)
                }
                "_beginthreadex" | "_o__beginthreadex" => {
                    Some(dlls::crt::msvcrt_beginthreadex as *const () as usize)
                }
                "_initterm" => Some(dlls::crt::_initterm as *const () as usize),
                "_initterm_e" => Some(dlls::crt::_initterm_e as *const () as usize),
                "_configthreadlocale" => Some(dlls::crt::_configthreadlocale as *const () as usize),
                "_set_fmode" => Some(dlls::crt::_set_fmode as *const () as usize),
                "_lock" => Some(dlls::crt::_lock as *const () as usize),
                "_unlock" => Some(dlls::crt::_unlock as *const () as usize),
                "_errno" => Some(dlls::crt::_errno as *const () as usize),
                "__initenv" => Some(unsafe { dlls::crt::__initenv() } as usize),
                "__iob_func" => Some(dlls::crt::__iob_func as *const () as usize),
                "__set_app_type" => Some(dlls::crt::__set_app_type as *const () as usize),
                "_amsg_exit" => Some(dlls::crt::_amsg_exit as *const () as usize),
                "exit" => Some(dlls::crt::msvcrt_exit as *const () as usize),
                "_exit" => Some(dlls::crt::msvcrt_exit_underscore as *const () as usize),
                "abort" => Some(dlls::crt::msvcrt_abort as *const () as usize),
                "_cexit" => Some(dlls::crt::_cexit as *const () as usize),
                "_c_exit" => Some(dlls::crt::_c_exit as *const () as usize),
                "quick_exit" => Some(dlls::crt::quick_exit as *const () as usize),
                "_strdup" | "strdup" => Some(dlls::crt::_strdup as *const () as usize),
                "_wcsdup" => Some(dlls::crt::_wcsdup as *const () as usize),
                "_get_osfhandle" => Some(dlls::crt::_get_osfhandle as *const () as usize),
                "_fstat64" | "_fstat64i32" => Some(dlls::crt::_fstat64 as *const () as usize),
                "_wstat64" | "_wstat64i32" => Some(dlls::crt::_wstat64 as *const () as usize),
                "__acrt_iob_func" => Some(dlls::crt::__acrt_iob_func as *const () as usize),
                "vfprintf" | "__stdio_common_vfprintf" => {
                    Some(dlls::crt::vfprintf as *const () as usize)
                }
                "__stdio_common_vfscanf" => {
                    Some(dlls::crt::msvcrt_sscanf_stub as *const () as usize)
                }
                "vsprintf_s" => Some(dlls::crt::vsprintf_s as *const () as usize),
                "_vsprintf_s_l" => Some(dlls::crt::_vsprintf_s_l as *const () as usize),
                "_vsnprintf_s" => Some(dlls::crt::_vsnprintf_s as *const () as usize),
                "_vsnprintf_s_l" => Some(dlls::crt::_vsnprintf_s_l as *const () as usize),
                "vsprintf" => Some(dlls::crt::vsprintf as *const () as usize),
                "_vsnprintf" => Some(dlls::crt::_vsnprintf as *const () as usize),
                "__stdio_common_vsprintf_s" | "__stdio_common_vsnprintf_s" => {
                    Some(dlls::crt::__stdio_common_vsprintf_s as *const () as usize)
                }
                "__stdio_common_vsprintf" => {
                    Some(dlls::crt::__stdio_common_vsprintf as *const () as usize)
                }
                "__stdio_common_vswprintf" | "__stdio_common_vswprintf_s" => {
                    Some(dlls::crt::msvcrt_sprintf_stub as *const () as usize)
                }
                "EncodePointer" | "_encode_pointer" => {
                    Some(dlls::kernel32::EncodePointer as *const () as usize)
                }
                "DecodePointer" | "_decode_pointer" => {
                    Some(dlls::kernel32::DecodePointer as *const () as usize)
                }
                // Catch-all for unimplemented CRT functions: return a safe no-op stub
                // rather than leaving them unresolved (which causes SIGSEGV)
                _ => {
                    println!("[CRT STUB] Providing no-op stub for: {}", func_name);
                    Some(dlls::crt::msvcrt_generic_stub as *const () as usize)
                }
            },
            "OLE32.DLL" | "COMBASE.DLL" => match func_name {
                "CoInitializeEx" => Some(dlls::shell::CoInitializeEx as *const () as usize),
                "CoUninitialize" => Some(dlls::shell::CoUninitialize as *const () as usize),
                "CoCreateInstance" => Some(dlls::shell::CoCreateInstance as *const () as usize),
                "CoTaskMemFree" => Some(dlls::shell::CoTaskMemFree as *const () as usize),
                "PropVariantClear" => Some(dlls::shell::PropVariantClear as *const () as usize),
                _ => None,
            },
            "OLEAUT32.DLL" => match func_name {
                "SysAllocString" => Some(dlls::shell::SysAllocString as *const () as usize),
                "SysFreeString" => Some(dlls::shell::SysFreeString as *const () as usize),
                "VariantInit" => Some(dlls::shell::VariantInit as *const () as usize),
                _ => None,
            },
            "SHELL32.DLL" | "SHELL32" | "SHELL32.dll" | "shell32.dll" | "SHLWAPI.DLL"
            | "SHLWAPI" | "SHLWAPI.dll" | "shlwapi.dll" => match func_name {
                "CommandLineToArgvW" => Some(dlls::shell::CommandLineToArgvW as *const () as usize),
                _ => Some(dlls::shell::rine_shell_folder_stub as *const () as usize),
            },
            "BCRYPT.DLL" | "BCRYPT" | "bcrypt.dll" => match func_name {
                "BCryptGenRandom" => Some(dlls::advapi32::BCryptGenRandom as *const () as usize),
                "BCryptOpenAlgorithmProvider" => {
                    Some(dlls::net::BCryptOpenAlgorithmProvider as *const () as usize)
                }
                "BCryptCloseAlgorithmProvider" => {
                    Some(dlls::net::BCryptCloseAlgorithmProvider as *const () as usize)
                }
                _ => None,
            },
            // VULKAN-1.DLL: DXVK imports this — forward to system libvulkan.so.1
            "VULKAN-1.DLL" | "VULKAN-1" | "vulkan-1.dll" => {
                crate::dlls::dxvk_loader::resolve_vulkan_symbol(func_name)
            }
            "DSOUND.DLL" => match func_name {
                "DirectSoundCreate" => Some(dlls::dsound::DirectSoundCreate as *const () as usize),
                "DirectSoundCreate8" => {
                    Some(dlls::dsound::DirectSoundCreate8 as *const () as usize)
                }
                _ => None,
            },
            "AVRT.DLL" | "AVRT" | "avrt.dll" => match func_name {
                "AvSetMmThreadCharacteristicsW" => {
                    Some(dlls::avrt::AvSetMmThreadCharacteristicsW as *const () as usize)
                }
                "AvSetMmThreadCharacteristicsA" => {
                    Some(dlls::avrt::AvSetMmThreadCharacteristicsA as *const () as usize)
                }
                "AvRevertMmThreadCharacteristics" => {
                    Some(dlls::avrt::AvRevertMmThreadCharacteristics as *const () as usize)
                }
                "AvSetMmThreadPriority" => {
                    Some(dlls::avrt::AvSetMmThreadPriority as *const () as usize)
                }
                _ => None,
            },
            "DWRITE.DLL" => match func_name {
                "DWriteCreateFactory" => {
                    Some(dlls::dwrite::DWriteCreateFactory as *const () as usize)
                }
                _ => None,
            },
            "IMM32.DLL" => match func_name {
                "ImmGetContext" => Some(dlls::imm32::ImmGetContext as *const () as usize),
                "ImmReleaseContext" => Some(dlls::imm32::ImmReleaseContext as *const () as usize),
                "ImmAssociateContext" => {
                    Some(dlls::imm32::ImmAssociateContext as *const () as usize)
                }
                "ImmGetCompositionStringW" => {
                    Some(dlls::imm32::ImmGetCompositionStringW as *const () as usize)
                }
                "ImmSetCandidateWindow" => {
                    Some(dlls::imm32::ImmSetCandidateWindow as *const () as usize)
                }
                "ImmSetCompositionWindow" => {
                    Some(dlls::imm32::ImmSetCompositionWindow as *const () as usize)
                }
                _ => None,
            },
            "DINPUT8.DLL" => match func_name {
                "DirectInput8Create" => {
                    Some(dlls::dinput::DirectInput8Create as *const () as usize)
                }
                _ => None,
            },
            "DWMAPI.DLL" => match func_name {
                "DwmEnableBlurBehindWindow" => {
                    Some(dlls::dwmapi::DwmEnableBlurBehindWindow as *const () as usize)
                }
                "DwmGetWindowAttribute" => {
                    Some(dlls::dwmapi::DwmGetWindowAttribute as *const () as usize)
                }
                "DwmSetWindowAttribute" => {
                    Some(dlls::dwmapi::DwmSetWindowAttribute as *const () as usize)
                }
                "DwmIsCompositionEnabled" => {
                    Some(dlls::dwmapi::DwmIsCompositionEnabled as *const () as usize)
                }
                "DwmFlush" => Some(dlls::dwmapi::DwmFlush as *const () as usize),
                _ => None,
            },
            "SETUPAPI.DLL" | "SETUPAPI" => match func_name {
                "SetupDiGetClassDevsW" => {
                    Some(dlls::setupapi::SetupDiGetClassDevsW as *const () as usize)
                }
                "SetupDiGetClassDevsA" => {
                    Some(dlls::setupapi::SetupDiGetClassDevsA as *const () as usize)
                }
                "SetupDiEnumDeviceInterfaces" => {
                    Some(dlls::setupapi::SetupDiEnumDeviceInterfaces as *const () as usize)
                }
                "SetupDiGetDeviceInterfaceDetailW" => {
                    Some(dlls::setupapi::SetupDiGetDeviceInterfaceDetailW as *const () as usize)
                }
                "SetupDiGetDeviceInterfaceDetailA" => {
                    Some(dlls::setupapi::SetupDiGetDeviceInterfaceDetailA as *const () as usize)
                }
                "SetupDiOpenDevRegKey" => {
                    Some(dlls::setupapi::SetupDiOpenDevRegKey as *const () as usize)
                }
                "SetupDiDestroyDeviceInfoList" => {
                    Some(dlls::setupapi::SetupDiDestroyDeviceInfoList as *const () as usize)
                }
                "SetupDiEnumDeviceInfo" => {
                    Some(dlls::setupapi::SetupDiEnumDeviceInfo as *const () as usize)
                }
                _ => None,
            },
            _ => {
                println!(
                    "[IAT UNRESOLVED] DLL='{}', Function='{}'",
                    dll_name, func_name
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iat_symbol_resolution() {
        let k32_alloc = IatSymbolResolver::resolve_symbol("kernel32.dll", "HeapAlloc");
        assert!(k32_alloc.is_some());

        let user32_msg = IatSymbolResolver::resolve_symbol("USER32.DLL", "GetMessageW");
        assert!(user32_msg.is_some());

        let ntdll_alloc = IatSymbolResolver::resolve_symbol("ntdll.dll", "NtAllocateVirtualMemory");
        assert!(ntdll_alloc.is_some());

        let d3d11_dev = IatSymbolResolver::resolve_symbol("d3d11.dll", "D3D11CreateDevice");
        assert!(d3d11_dev.is_some());

        let winmm_stub = IatSymbolResolver::resolve_symbol("winmm.dll", "timeGetTime");
        assert!(winmm_stub.is_some());

        let dwmapi_stub =
            IatSymbolResolver::resolve_symbol("dwmapi.dll", "DwmIsCompositionEnabled");
        assert!(dwmapi_stub.is_some());

        let k32_cp = IatSymbolResolver::resolve_symbol("KERNEL32.dll", "SetConsoleOutputCP");
        assert!(k32_cp.is_some());
        assert_eq!(
            k32_cp.unwrap(),
            crate::dlls::kernel32::SetConsoleOutputCP as *const () as usize
        );

        let ole32_coinit = IatSymbolResolver::resolve_symbol("ole32.dll", "CoInitializeEx");
        assert!(ole32_coinit.is_some());
        assert_eq!(
            ole32_coinit.unwrap(),
            crate::dlls::shell::CoInitializeEx as *const () as usize
        );

        let crt_errno = IatSymbolResolver::resolve_symbol("msvcrt.dll", "_errno");
        assert!(crt_errno.is_some());
        assert_eq!(
            crt_errno.unwrap(),
            crate::dlls::crt::_errno as *const () as usize
        );

        let crt_wstat = IatSymbolResolver::resolve_symbol("msvcrt.dll", "_wstat64");
        assert!(crt_wstat.is_some());
        assert_eq!(
            crt_wstat.unwrap(),
            crate::dlls::crt::_wstat64 as *const () as usize
        );

        let k32_drives = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "GetLogicalDrives");
        assert!(k32_drives.is_some());
        assert_eq!(
            k32_drives.unwrap(),
            crate::dlls::kernel32::GetLogicalDrives as *const () as usize
        );

        let k32_cwd = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "GetCurrentDirectoryW");
        assert!(k32_cwd.is_some());
        assert_eq!(
            k32_cwd.unwrap(),
            crate::dlls::kernel32::GetCurrentDirectoryW as *const () as usize
        );

        let k32_set_cwd = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "SetCurrentDirectoryW");
        assert!(k32_set_cwd.is_some());
        assert_eq!(
            k32_set_cwd.unwrap(),
            crate::dlls::kernel32::SetCurrentDirectoryW as *const () as usize
        );

        let k32_csbi =
            IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "GetConsoleScreenBufferInfo");
        assert!(k32_csbi.is_some());
        assert_eq!(
            k32_csbi.unwrap(),
            crate::dlls::kernel32::GetConsoleScreenBufferInfo as *const () as usize
        );

        let k32_scta = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "SetConsoleTextAttribute");
        assert!(k32_scta.is_some());
        assert_eq!(
            k32_scta.unwrap(),
            crate::dlls::kernel32::SetConsoleTextAttribute as *const () as usize
        );

        let crt_wfsopen = IatSymbolResolver::resolve_symbol("MSVCRT.DLL", "_wfsopen");
        assert!(crt_wfsopen.is_some());
        assert_eq!(
            crt_wfsopen.unwrap(),
            crate::dlls::crt::msvcrt_wfsopen as *const () as usize
        );

        let crt_filelen = IatSymbolResolver::resolve_symbol("MSVCRT.DLL", "_filelengthi64");
        assert!(crt_filelen.is_some());
        assert_eq!(
            crt_filelen.unwrap(),
            crate::dlls::crt::msvcrt_filelengthi64 as *const () as usize
        );

        let k32_gnsi = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "GetNativeSystemInfo");
        assert!(k32_gnsi.is_some());
        assert_eq!(
            k32_gnsi.unwrap(),
            crate::dlls::kernel32::GetNativeSystemInfo as *const () as usize
        );

        let k32_gsi = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "GetSystemInfo");
        assert!(k32_gsi.is_some());
        assert_eq!(
            k32_gsi.unwrap(),
            crate::dlls::kernel32::GetSystemInfo as *const () as usize
        );

        let k32_cdw = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "CreateDirectoryW");
        assert!(k32_cdw.is_some());
        assert_eq!(
            k32_cdw.unwrap(),
            crate::dlls::kernel32::CreateDirectoryW as *const () as usize
        );

        let k32_cda = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "CreateDirectoryA");
        assert!(k32_cda.is_some());
        assert_eq!(
            k32_cda.unwrap(),
            crate::dlls::kernel32::CreateDirectoryA as *const () as usize
        );

        let k32_glt = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "GetLocalTime");
        assert!(k32_glt.is_some());
        assert_eq!(
            k32_glt.unwrap(),
            crate::dlls::kernel32::GetLocalTime as *const () as usize
        );

        let k32_gtzi = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "GetTimeZoneInformation");
        assert!(k32_gtzi.is_some());
        assert_eq!(
            k32_gtzi.unwrap(),
            crate::dlls::kernel32::GetTimeZoneInformation as *const () as usize
        );

        let k32_genv = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "GetEnvironmentVariableW");
        assert!(k32_genv.is_some());
        assert_eq!(
            k32_genv.unwrap(),
            crate::dlls::kernel32::GetEnvironmentVariableW as *const () as usize
        );

        let k32_pcr = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "PowerCreateRequest");
        assert!(k32_pcr.is_some());
        assert_eq!(
            k32_pcr.unwrap(),
            crate::dlls::kernel32::PowerCreateRequest as *const () as usize
        );

        let k32_psr = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "PowerSetRequest");
        assert!(k32_psr.is_some());
        assert_eq!(
            k32_psr.unwrap(),
            crate::dlls::kernel32::PowerSetRequest as *const () as usize
        );

        let k32_pclr = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "PowerClearRequest");
        assert!(k32_pclr.is_some());
        assert_eq!(
            k32_pclr.unwrap(),
            crate::dlls::kernel32::PowerClearRequest as *const () as usize
        );

        let k32_freelib = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "FreeLibrary");
        assert!(k32_freelib.is_some());
        assert_eq!(
            k32_freelib.unwrap(),
            crate::dlls::kernel32::FreeLibrary as *const () as usize
        );

        let ntdll_ver = IatSymbolResolver::resolve_symbol("NTDLL.DLL", "RtlGetVersion");
        assert!(ntdll_ver.is_some());
        assert_eq!(
            ntdll_ver.unwrap(),
            crate::dlls::ntdll::RtlGetVersion as *const () as usize
        );

        let u32_hook = IatSymbolResolver::resolve_symbol("USER32.DLL", "SetWindowsHookExA");
        assert!(u32_hook.is_some());
        assert_eq!(
            u32_hook.unwrap(),
            crate::dlls::user32::SetWindowsHookExA as *const () as usize
        );

        let u32_callnext = IatSymbolResolver::resolve_symbol("USER32.DLL", "CallNextHookEx");
        assert!(u32_callnext.is_some());
        assert_eq!(
            u32_callnext.unwrap(),
            crate::dlls::user32::CallNextHookEx as *const () as usize
        );

        let u32_unhook = IatSymbolResolver::resolve_symbol("USER32.DLL", "UnhookWindowsHookEx");
        assert!(u32_unhook.is_some());
        assert_eq!(
            u32_unhook.unwrap(),
            crate::dlls::user32::UnhookWindowsHookEx as *const () as usize
        );
    }
}
