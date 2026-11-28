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
        "[RINE UNRESOLVED BIND] importer='{}' dll='{}' symbol='{}' iat_rva=0x{:X} iat_va=0x{:X} stub_id={} resolver='{}'",
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
        if func_name == "__wine_unix_call_dispatcher" {
            return Some(dlls::ntdll::get_wine_unix_dispatcher_ptr());
        }
        if func_name.starts_with("__wine_") {
            return Some(wine_dbg_noop_stub as *const () as usize);
        }

        let dll_upper = dll_name.to_uppercase();

        if crate::dlls::steam_bridge::is_bridge_enabled() {
            if dll_upper.starts_with("STEAMCLIENT") || dll_upper.starts_with("LSTEAMCLIENT") {
                if let Some(ptr) = crate::dlls::steam_bridge::resolve_steamclient_symbol(func_name) {
                    return Some(ptr);
                }
            }
            if dll_upper.starts_with("STEAM_API") {
                return None;
            }
        } else if dll_upper.starts_with("STEAM_API")
            || dll_upper.starts_with("STEAMCLIENT")
            || dll_upper.starts_with("LSTEAMCLIENT")
            || dll_upper.starts_with("CSTEAMWORKS")
            || func_name.starts_with("SteamAPI_")
            || func_name.starts_with("SteamInternal_")
            || func_name.starts_with("Steam_")
            || func_name.starts_with("ISteam")
            || func_name == "InitSafe"
            || func_name == "RestartAppIfNecessary"
            || func_name == "IsSteamRunning"
        {
            if let Some(ptr) = crate::dlls::steam::resolve_steam_symbol(func_name) {
                return Some(ptr);
            }
        }

        match dll_upper.as_str() {
            dll if dll.starts_with("OLE32")
                || dll.starts_with("OLEAUT32")
                || dll.starts_with("COMBASE")
                || dll.starts_with("SHELL32")
                || dll.contains("WINRT")
                || dll.contains("API-MS-WIN-CORE-COM-") =>
            {
                match func_name {
                    "CoInitializeEx" | "CoInitialize" => {
                        Some(dlls::shell::CoInitializeEx as *const () as usize)
                    }
                    "CoUninitialize" => Some(dlls::shell::CoUninitialize as *const () as usize),
                    "OleInitialize" => Some(dlls::shell::OleInitialize as *const () as usize),
                    "OleUninitialize" => Some(dlls::shell::OleUninitialize as *const () as usize),
                    "CoCreateInstance" => Some(dlls::shell::CoCreateInstance as *const () as usize),
                    "CoCreateGuid" => Some(dlls::shell::CoCreateGuid as *const () as usize),
                    "StringFromGUID2" => Some(dlls::shell::StringFromGUID2 as *const () as usize),
                    "StringFromIID" => Some(dlls::shell::StringFromIID as *const () as usize),
                    "IIDFromString" => Some(dlls::shell::IIDFromString as *const () as usize),
                    "CLSIDFromString" => Some(dlls::shell::CLSIDFromString as *const () as usize),
                    "PropVariantCopy" => Some(dlls::shell::PropVariantCopy as *const () as usize),
                    "RegisterDragDrop" => Some(dlls::shell::RegisterDragDrop as *const () as usize),
                    "RevokeDragDrop" => Some(dlls::shell::RevokeDragDrop as *const () as usize),
                    "ReleaseStgMedium" => Some(dlls::shell::ReleaseStgMedium as *const () as usize),
                    "CoTaskMemAlloc" => Some(dlls::shell::CoTaskMemAlloc as *const () as usize),
                    "CoTaskMemFree" => Some(dlls::shell::CoTaskMemFree as *const () as usize),
                    "CoTaskMemRealloc" => Some(dlls::shell::CoTaskMemRealloc as *const () as usize),
                    "RoInitialize" => Some(dlls::shell::RoInitialize as *const () as usize),
                    "RoUninitialize" => Some(dlls::shell::RoUninitialize as *const () as usize),
                    "RoActivateInstance" => {
                        Some(dlls::shell::RoActivateInstance as *const () as usize)
                    }
                    "RoGetActivationFactory" => {
                        Some(dlls::shell::RoGetActivationFactory as *const () as usize)
                    }
                    "WindowsCreateString" => {
                        Some(dlls::shell::WindowsCreateString as *const () as usize)
                    }
                    "WindowsDeleteString" => {
                        Some(dlls::shell::WindowsDeleteString as *const () as usize)
                    }
                    "WindowsGetStringRawBuffer" => {
                        Some(dlls::shell::WindowsGetStringRawBuffer as *const () as usize)
                    }
                    "WindowsCreateStringReference" => {
                        Some(dlls::shell::WindowsCreateStringReference as *const () as usize)
                    }
                    "WindowsDuplicateString" => {
                        Some(dlls::shell::WindowsDuplicateString as *const () as usize)
                    }
                    "CoTaskMemAlloc" => Some(dlls::shell::CoTaskMemAlloc as *const () as usize),
                    "CoTaskMemFree" => Some(dlls::shell::CoTaskMemFree as *const () as usize),
                    "PropVariantClear" => Some(dlls::shell::PropVariantClear as *const () as usize),
                    "VariantInit" => Some(dlls::shell::VariantInit as *const () as usize),
                    "VariantClear" => Some(dlls::shell::VariantClear as *const () as usize),
                    "RoSetErrorReportingFlags" => {
                        Some(dlls::shell::RoSetErrorReportingFlags as *const () as usize)
                    }
                    "GetRestrictedErrorInfo" => {
                        Some(dlls::shell::GetRestrictedErrorInfo as *const () as usize)
                    }
                    "RoOriginateError" => Some(dlls::shell::RoOriginateError as *const () as usize),
                    "RoOriginateErrorW" => {
                        Some(dlls::shell::RoOriginateErrorW as *const () as usize)
                    }
                    "RoGetBufferMarshaler" => {
                        Some(dlls::shell::RoGetBufferMarshaler as *const () as usize)
                    }
                    "CoGetApartmentType" => {
                        Some(dlls::shell::CoGetApartmentType as *const () as usize)
                    }
                    "CoCreateFreeThreadedMarshaler" => {
                        Some(dlls::shell::CoCreateFreeThreadedMarshaler as *const () as usize)
                    }
                    "SysStringLen" => Some(dlls::shell::SysStringLen as *const () as usize),
                    "SysStringByteLen" => Some(dlls::shell::SysStringByteLen as *const () as usize),
                    "SafeArrayCreate" => Some(dlls::shell::SafeArrayCreate as *const () as usize),
                    "SafeArrayDestroy" => Some(dlls::shell::SafeArrayDestroy as *const () as usize),
                    "SafeArrayGetDim" => Some(dlls::shell::SafeArrayGetDim as *const () as usize),
                    "SafeArrayGetLBound" => {
                        Some(dlls::shell::SafeArrayGetLBound as *const () as usize)
                    }
                    "SafeArrayGetUBound" => {
                        Some(dlls::shell::SafeArrayGetUBound as *const () as usize)
                    }
                    "SafeArrayGetVartype" => {
                        Some(dlls::shell::SafeArrayGetVartype as *const () as usize)
                    }
                    "SafeArrayAccessData" => {
                        Some(dlls::shell::SafeArrayAccessData as *const () as usize)
                    }
                    "SafeArrayUnaccessData" => {
                        Some(dlls::shell::SafeArrayUnaccessData as *const () as usize)
                    }
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
                    "ShellExecuteW" => Some(dlls::shell::ShellExecuteW as *const () as usize),
                    "ShellExecuteA" => Some(dlls::shell::ShellExecuteA as *const () as usize),
                    "DragAcceptFiles" => Some(dlls::shell::DragAcceptFiles as *const () as usize),
                    "DragFinish" => Some(dlls::shell::DragFinish as *const () as usize),
                    "DragQueryFileW" => Some(dlls::shell::DragQueryFileW as *const () as usize),
                    "DragQueryFileA" => Some(dlls::shell::DragQueryFileA as *const () as usize),
                    "DragQueryPoint" => Some(dlls::shell::DragQueryPoint as *const () as usize),
                    _ => None,
                }
            }
            dll if dll.starts_with("DBGHELP") => match func_name {
                "SymInitialize" => Some(dlls::shell::SymInitialize as *const () as usize),
                "SymInitializeW" => Some(dlls::shell::SymInitializeW as *const () as usize),
                "SymFromAddr" => Some(dlls::shell::SymFromAddr as *const () as usize),
                "SymFromAddrW" => Some(dlls::shell::SymFromAddrW as *const () as usize),
                "SymCleanup" => Some(dlls::shell::SymCleanup as *const () as usize),
                "SymSetOptions" => Some(dlls::shell::SymSetOptions as *const () as usize),
                "SymGetOptions" => Some(dlls::shell::SymGetOptions as *const () as usize),
                "SymGetLineFromAddr64" => Some(dlls::shell::SymGetLineFromAddr64 as *const () as usize),
                "SymGetLineFromAddrW64" => Some(dlls::shell::SymGetLineFromAddrW64 as *const () as usize),
                "SymGetModuleInfo64" => Some(dlls::shell::SymGetModuleInfo64 as *const () as usize),
                "SymGetModuleInfoW64" => Some(dlls::shell::SymGetModuleInfoW64 as *const () as usize),
                "SymLoadModuleEx" => Some(dlls::shell::SymLoadModuleEx as *const () as usize),
                "SymLoadModuleExW" => Some(dlls::shell::SymLoadModuleExW as *const () as usize),
                "StackWalk64" => Some(dlls::shell::StackWalk64 as *const () as usize),
                "SymFunctionTableAccess64" => Some(dlls::shell::SymFunctionTableAccess64 as *const () as usize),
                "SymGetModuleBase64" => Some(dlls::shell::SymGetModuleBase64 as *const () as usize),
                "SymGetSearchPathW" => Some(dlls::shell::SymGetSearchPathW as *const () as usize),
                "SymSetSearchPathW" => Some(dlls::shell::SymSetSearchPathW as *const () as usize),
                "MiniDumpWriteDump" => Some(dlls::shell::MiniDumpWriteDump as *const () as usize),
                _ => None,
            },
            dll if dll.starts_with("WS2_32") || dll.starts_with("WSOCK32") => match func_name {
                "WSAStartup" => Some(dlls::ws2_32::WSAStartup as *const () as usize),
                "GetHostNameW" => Some(dlls::ws2_32::GetHostNameW as *const () as usize),
                "WSALookupServiceBeginW" => {
                    Some(dlls::ws2_32::WSALookupServiceBeginW as *const () as usize)
                }
                "WSALookupServiceBeginA" => {
                    Some(dlls::ws2_32::WSALookupServiceBeginA as *const () as usize)
                }
                "WSALookupServiceNextW" => {
                    Some(dlls::ws2_32::WSALookupServiceNextW as *const () as usize)
                }
                "WSALookupServiceNextA" => {
                    Some(dlls::ws2_32::WSALookupServiceNextA as *const () as usize)
                }
                "WSALookupServiceEnd" => {
                    Some(dlls::ws2_32::WSALookupServiceEnd as *const () as usize)
                }
                "WSANSPIoctl" => Some(dlls::ws2_32::WSANSPIoctl as *const () as usize),
                "WSACleanup" => Some(dlls::ws2_32::WSACleanup as *const () as usize),
                "WSAGetLastError" => Some(dlls::ws2_32::WSAGetLastError as *const () as usize),
                "WSASetLastError" => Some(dlls::ws2_32::WSASetLastError as *const () as usize),
                "socket" => Some(dlls::ws2_32::socket as *const () as usize),
                "closesocket" => Some(dlls::ws2_32::closesocket as *const () as usize),
                "bind" => Some(dlls::ws2_32::bind as *const () as usize),
                "connect" => Some(dlls::ws2_32::connect as *const () as usize),
                "listen" => Some(dlls::ws2_32::listen as *const () as usize),
                "accept" => Some(dlls::ws2_32::accept as *const () as usize),
                "send" => Some(dlls::ws2_32::send as *const () as usize),
                "recv" => Some(dlls::ws2_32::recv as *const () as usize),
                "sendto" => Some(dlls::ws2_32::sendto as *const () as usize),
                "recvfrom" => Some(dlls::ws2_32::recvfrom as *const () as usize),
                "shutdown" => Some(dlls::ws2_32::shutdown as *const () as usize),
                "htons" => Some(dlls::ws2_32::htons as *const () as usize),
                "ntohs" => Some(dlls::ws2_32::ntohs as *const () as usize),
                "htonl" => Some(dlls::ws2_32::htonl as *const () as usize),
                "ntohl" => Some(dlls::ws2_32::ntohl as *const () as usize),
                "inet_addr" => Some(dlls::ws2_32::inet_addr as *const () as usize),
                "inet_ntoa" => Some(dlls::ws2_32::inet_ntoa as *const () as usize),
                "ioctlsocket" => Some(dlls::ws2_32::ioctlsocket as *const () as usize),
                "setsockopt" => Some(dlls::ws2_32::setsockopt as *const () as usize),
                "getsockopt" => Some(dlls::ws2_32::getsockopt as *const () as usize),
                "gethostname" => Some(dlls::ws2_32::gethostname as *const () as usize),
                "__WSAFDIsSet" => Some(dlls::ws2_32::__WSAFDIsSet as *const () as usize),
                "WSACreateEvent" => Some(dlls::ws2_32::WSACreateEvent as *const () as usize),
                "WSACloseEvent" => Some(dlls::ws2_32::WSACloseEvent as *const () as usize),
                "WSASetEvent" => Some(dlls::ws2_32::WSASetEvent as *const () as usize),
                "WSAResetEvent" => Some(dlls::ws2_32::WSAResetEvent as *const () as usize),
                "WSAWaitForMultipleEvents" => {
                    Some(dlls::ws2_32::WSAWaitForMultipleEvents as *const () as usize)
                }
                "WSASocketA" => Some(dlls::ws2_32::WSASocketA as *const () as usize),
                "WSASocketW" => Some(dlls::ws2_32::WSASocketW as *const () as usize),
                "WSAEventSelect" => Some(dlls::ws2_32::WSAEventSelect as *const () as usize),
                "WSAEnumNetworkEvents" => {
                    Some(dlls::ws2_32::WSAEnumNetworkEvents as *const () as usize)
                }
                "WSAIoctl" => Some(dlls::ws2_32::WSAIoctl as *const () as usize),
                "WSASend" => Some(dlls::ws2_32::WSASend as *const () as usize),
                "WSARecv" => Some(dlls::ws2_32::WSARecv as *const () as usize),
                "WSASendDisconnect" => {
                    Some(dlls::ws2_32::WSASendDisconnect as *const () as usize)
                }
                "select" => Some(dlls::ws2_32::select as *const () as usize),
                "getpeername" => Some(dlls::ws2_32::getpeername as *const () as usize),
                "getsockname" => Some(dlls::ws2_32::getsockname as *const () as usize),
                "inet_ntop" => Some(dlls::ws2_32::inet_ntop as *const () as usize),
                "inet_pton" => Some(dlls::ws2_32::inet_pton as *const () as usize),
                "getaddrinfo" => Some(dlls::ws2_32::getaddrinfo as *const () as usize),
                "freeaddrinfo" => Some(dlls::ws2_32::freeaddrinfo as *const () as usize),
                "GetAddrInfoW" => Some(dlls::ws2_32::GetAddrInfoW as *const () as usize),
                "FreeAddrInfoW" => Some(dlls::ws2_32::FreeAddrInfoW as *const () as usize),
                "GetAddrInfoExW" => Some(dlls::ws2_32::GetAddrInfoExW as *const () as usize),
                "FreeAddrInfoExW" => Some(dlls::ws2_32::FreeAddrInfoExW as *const () as usize),
                "GetAddrInfoExCancel" => {
                    Some(dlls::ws2_32::GetAddrInfoExCancel as *const () as usize)
                }
                "getnameinfo" => Some(dlls::ws2_32::getnameinfo as *const () as usize),
                "GetNameInfoW" => Some(dlls::ws2_32::GetNameInfoW as *const () as usize),
                "InetNtopW" => Some(dlls::ws2_32::InetNtopW as *const () as usize),
                "InetPtonW" => Some(dlls::ws2_32::InetPtonW as *const () as usize),
                "gethostbyname" => Some(dlls::ws2_32::gethostbyname as *const () as usize),
                "gethostbyaddr" => Some(dlls::ws2_32::gethostbyaddr as *const () as usize),
                "getprotobyname" => Some(dlls::ws2_32::getprotobyname as *const () as usize),
                "getprotobynumber" => {
                    Some(dlls::ws2_32::getprotobynumber as *const () as usize)
                }
                "getservbyname" => Some(dlls::ws2_32::getservbyname as *const () as usize),
                "getservbyport" => Some(dlls::ws2_32::getservbyport as *const () as usize),
                _ => None,
            },
            dll if dll.starts_with("ADVAPI32")
                || dll.starts_with("API-MS-WIN-CORE-REGISTRY")
                || dll.starts_with("API-MS-WIN-CORE-LOCALREGISTRY") => match func_name {
                "RegOpenKeyExW" => Some(dlls::advapi32::RegOpenKeyExW as *const () as usize),
                "RegOpenKeyExA" => Some(dlls::advapi32::RegOpenKeyExA as *const () as usize),
                "RegOpenKeyW" => Some(dlls::advapi32::RegOpenKeyW as *const () as usize),
                "RegOpenKeyA" => Some(dlls::advapi32::RegOpenKeyA as *const () as usize),
                "RegCreateKeyW" => Some(dlls::advapi32::RegCreateKeyW as *const () as usize),
                "RegCreateKeyA" => Some(dlls::advapi32::RegCreateKeyA as *const () as usize),
                "RegCreateKeyExW" => Some(dlls::advapi32::RegCreateKeyExW as *const () as usize),
                "RegCreateKeyExA" => Some(dlls::advapi32::RegCreateKeyExA as *const () as usize),
                "RegQueryValueExW" => Some(dlls::advapi32::RegQueryValueExW as *const () as usize),
                "RegQueryValueExA" => Some(dlls::advapi32::RegQueryValueExA as *const () as usize),
                "RegSetValueExW" => Some(dlls::advapi32::RegSetValueExW as *const () as usize),
                "RegSetValueExA" => Some(dlls::advapi32::RegSetValueExA as *const () as usize),
                "RegCloseKey" => Some(dlls::advapi32::RegCloseKey as *const () as usize),
                "RegDeleteKeyW" => Some(dlls::advapi32::RegDeleteKeyW as *const () as usize),
                "RegDeleteKeyA" => Some(dlls::advapi32::RegDeleteKeyA as *const () as usize),
                "RegDeleteValueW" => Some(dlls::advapi32::RegDeleteValueW as *const () as usize),
                "RegDeleteValueA" => Some(dlls::advapi32::RegDeleteValueA as *const () as usize),
                "RegFlushKey" => Some(dlls::advapi32::RegFlushKey as *const () as usize),
                "RegNotifyChangeKeyValue" => {
                    Some(dlls::advapi32::RegNotifyChangeKeyValue as *const () as usize)
                }
                "RegQueryInfoKeyW" => {
                    Some(dlls::advapi32::RegQueryInfoKeyW as *const () as usize)
                }
                "RegQueryInfoKeyA" => {
                    Some(dlls::advapi32::RegQueryInfoKeyA as *const () as usize)
                }
                "RegGetValueW" => Some(dlls::advapi32::RegGetValueW as *const () as usize),
                "RegGetValueA" => Some(dlls::advapi32::RegGetValueA as *const () as usize),
                "RegEnumKeyExW" => Some(dlls::advapi32::RegEnumKeyExW as *const () as usize),
                "RegEnumValueW" => Some(dlls::advapi32::RegEnumValueW as *const () as usize),
                "GetUserNameW" => Some(dlls::advapi32::GetUserNameW as *const () as usize),
                "GetUserNameA" => Some(dlls::advapi32::GetUserNameA as *const () as usize),
                "LookupPrivilegeValueA" => {
                    Some(dlls::advapi32::LookupPrivilegeValueA as *const () as usize)
                }
                "RegisterEventSourceW" => {
                    Some(dlls::advapi32::RegisterEventSourceW as *const () as usize)
                }
                "ReportEventW" => Some(dlls::advapi32::ReportEventW as *const () as usize),
                "DeregisterEventSource" => {
                    Some(dlls::advapi32::DeregisterEventSource as *const () as usize)
                }
                "AdjustTokenPrivileges" => {
                    Some(dlls::advapi32::AdjustTokenPrivileges as *const () as usize)
                }
                "GetTokenInformation" => {
                    Some(dlls::advapi32::GetTokenInformation as *const () as usize)
                }
                "OpenProcessToken" => Some(dlls::advapi32::OpenProcessToken as *const () as usize),
                "GetSecurityInfo" => Some(dlls::advapi32::GetSecurityInfo as *const () as usize),
                "SetSecurityInfo" => Some(dlls::advapi32::SetSecurityInfo as *const () as usize),
                "GetSidSubAuthority" => {
                    Some(dlls::advapi32::GetSidSubAuthority as *const () as usize)
                }
                "GetSidSubAuthorityCount" => {
                    Some(dlls::advapi32::GetSidSubAuthorityCount as *const () as usize)
                }
                "LookupPrivilegeValueW" => {
                    Some(dlls::advapi32::LookupPrivilegeValueW as *const () as usize)
                }
                "AllocateLocallyUniqueId" => {
                    Some(dlls::advapi32::AllocateLocallyUniqueId as *const () as usize)
                }
                "InitializeSecurityDescriptor" => {
                    Some(dlls::advapi32::InitializeSecurityDescriptor as *const () as usize)
                }
                "SetSecurityDescriptorDacl" => {
                    Some(dlls::advapi32::SetSecurityDescriptorDacl as *const () as usize)
                }
                "SetSecurityDescriptorOwner" => {
                    Some(dlls::advapi32::SetSecurityDescriptorOwner as *const () as usize)
                }
                "SetSecurityDescriptorGroup" => {
                    Some(dlls::advapi32::SetSecurityDescriptorGroup as *const () as usize)
                }
                "IsValidSecurityDescriptor" => {
                    Some(dlls::advapi32::IsValidSecurityDescriptor as *const () as usize)
                }
                "GetSecurityDescriptorDacl" => {
                    Some(dlls::advapi32::GetSecurityDescriptorDacl as *const () as usize)
                }
                "InitializeAcl" => Some(dlls::advapi32::InitializeAcl as *const () as usize),
                "AddAccessAllowedAce" => {
                    Some(dlls::advapi32::AddAccessAllowedAce as *const () as usize)
                }
                "IsValidSid" => Some(dlls::advapi32::IsValidSid as *const () as usize),
                "GetLengthSid" => Some(dlls::advapi32::GetLengthSid as *const () as usize),
                "CreateWellKnownSid" => {
                    Some(dlls::advapi32::CreateWellKnownSid as *const () as usize)
                }
                "CopySid" => Some(dlls::advapi32::CopySid as *const () as usize),
                "EqualSid" => Some(dlls::advapi32::EqualSid as *const () as usize),
                "EventRegister" => Some(dlls::advapi32::EventRegister as *const () as usize),
                "EventUnregister" => Some(dlls::advapi32::EventUnregister as *const () as usize),
                "EventWriteTransfer" => Some(dlls::advapi32::EventWriteTransfer as *const () as usize),
                "CryptAcquireContextW" => Some(dlls::advapi32::CryptAcquireContextW as *const () as usize),
                "CryptAcquireContextA" => Some(dlls::advapi32::CryptAcquireContextA as *const () as usize),
                "CryptReleaseContext" => Some(dlls::advapi32::CryptReleaseContext as *const () as usize),
                "CryptCreateHash" => Some(dlls::advapi32::CryptCreateHash as *const () as usize),
                "CryptDestroyHash" => Some(dlls::advapi32::CryptDestroyHash as *const () as usize),
                "CryptHashData" => Some(dlls::advapi32::CryptHashData as *const () as usize),
                "CryptGetHashParam" => Some(dlls::advapi32::CryptGetHashParam as *const () as usize),
                "CryptGenRandom" => Some(dlls::advapi32::CryptGenRandom as *const () as usize),
                "SystemFunction036" => Some(dlls::advapi32::SystemFunction036 as *const () as usize),
                "EventSetInformation" => Some(dlls::advapi32::EventSetInformation as *const () as usize),
                _ => None,
            },
            dll if dll.starts_with("CRYPTBASE") => match func_name {
                "SystemFunction036" => Some(dlls::advapi32::SystemFunction036 as *const () as usize),
                _ => None,
            },
            dll if dll.starts_with("CFGMGR32") => crate::dlls::cfgmgr32::resolve_cfgmgr32_symbol(func_name),
            dll if dll.starts_with("PSAPI") => match func_name {
                "GetModuleInformation" => Some(dlls::kernel32::GetModuleInformation as *const () as usize),
                "GetModuleBaseNameA" => Some(dlls::kernel32::GetModuleBaseNameA as *const () as usize),
                "GetModuleBaseNameW" => Some(dlls::kernel32::GetModuleBaseNameW as *const () as usize),
                "GetModuleFileNameExA" => Some(dlls::kernel32::GetModuleFileNameExA as *const () as usize),
                "GetModuleFileNameExW" => Some(dlls::kernel32::GetModuleFileNameExW as *const () as usize),
                "EnumProcessModules" => Some(dlls::kernel32::EnumProcessModules as *const () as usize),
                "EnumProcessModulesEx" => Some(dlls::kernel32::EnumProcessModulesEx as *const () as usize),
                "EnumProcesses" => Some(dlls::kernel32::EnumProcesses as *const () as usize),
                "GetProcessMemoryInfo" => Some(dlls::kernel32::K32GetProcessMemoryInfo as *const () as usize),
                "K32EnumProcessModules" => Some(dlls::kernel32::K32EnumProcessModules as *const () as usize),
                "K32EnumProcessModulesEx" => Some(dlls::kernel32::K32EnumProcessModulesEx as *const () as usize),
                "K32EnumProcesses" => Some(dlls::kernel32::K32EnumProcesses as *const () as usize),
                "K32GetModuleBaseNameA" => Some(dlls::kernel32::K32GetModuleBaseNameA as *const () as usize),
                "K32GetModuleBaseNameW" => Some(dlls::kernel32::K32GetModuleBaseNameW as *const () as usize),
                "K32GetModuleFileNameExA" => Some(dlls::kernel32::K32GetModuleFileNameExA as *const () as usize),
                "K32GetModuleFileNameExW" => Some(dlls::kernel32::K32GetModuleFileNameExW as *const () as usize),
                "K32GetModuleInformation" => Some(dlls::kernel32::K32GetModuleInformation as *const () as usize),
                "K32GetProcessMemoryInfo" => Some(dlls::kernel32::K32GetProcessMemoryInfo as *const () as usize),
                _ => None,
            },
            dll if dll.starts_with("KERNEL32")
                || dll.starts_with("KERNELBASE")
                || dll.starts_with("API-MS-WIN-CORE-")
                || dll.starts_with("API-MS-WIN-BASE-") =>
            {
                match func_name {
                    "RegOpenKeyExW" => Some(dlls::advapi32::RegOpenKeyExW as *const () as usize),
                    "RegOpenKeyExA" => Some(dlls::advapi32::RegOpenKeyExA as *const () as usize),
                    "RegOpenKeyW" => Some(dlls::advapi32::RegOpenKeyW as *const () as usize),
                    "RegOpenKeyA" => Some(dlls::advapi32::RegOpenKeyA as *const () as usize),
                    "RegCreateKeyW" => Some(dlls::advapi32::RegCreateKeyW as *const () as usize),
                    "RegCreateKeyA" => Some(dlls::advapi32::RegCreateKeyA as *const () as usize),
                    "RegCreateKeyExW" => Some(dlls::advapi32::RegCreateKeyExW as *const () as usize),
                    "RegCreateKeyExA" => Some(dlls::advapi32::RegCreateKeyExA as *const () as usize),
                    "RegQueryValueExW" => Some(dlls::advapi32::RegQueryValueExW as *const () as usize),
                    "RegQueryValueExA" => Some(dlls::advapi32::RegQueryValueExA as *const () as usize),
                    "RegSetValueExW" => Some(dlls::advapi32::RegSetValueExW as *const () as usize),
                    "RegSetValueExA" => Some(dlls::advapi32::RegSetValueExA as *const () as usize),
                    "RegCloseKey" => Some(dlls::advapi32::RegCloseKey as *const () as usize),
                    "RegDeleteKeyW" => Some(dlls::advapi32::RegDeleteKeyW as *const () as usize),
                    "RegDeleteKeyA" => Some(dlls::advapi32::RegDeleteKeyA as *const () as usize),
                    "RegDeleteValueW" => Some(dlls::advapi32::RegDeleteValueW as *const () as usize),
                    "RegDeleteValueA" => Some(dlls::advapi32::RegDeleteValueA as *const () as usize),
                    "RegFlushKey" => Some(dlls::advapi32::RegFlushKey as *const () as usize),
                    "RegNotifyChangeKeyValue" => {
                        Some(dlls::advapi32::RegNotifyChangeKeyValue as *const () as usize)
                    }
                    "RegQueryInfoKeyW" => {
                        Some(dlls::advapi32::RegQueryInfoKeyW as *const () as usize)
                    }
                    "RegQueryInfoKeyA" => {
                        Some(dlls::advapi32::RegQueryInfoKeyA as *const () as usize)
                    }
                    "RegGetValueW" => Some(dlls::advapi32::RegGetValueW as *const () as usize),
                    "RegGetValueA" => Some(dlls::advapi32::RegGetValueA as *const () as usize),
                    "RegEnumValueW" => Some(dlls::advapi32::RegEnumValueW as *const () as usize),
                    "RegEnumValueA" => Some(dlls::advapi32::RegEnumValueA as *const () as usize),
                    "IsDBCSLeadByteEx" => Some(dlls::kernel32::IsDBCSLeadByteEx as *const () as usize),
                    "IsWow64Process" => Some(dlls::kernel32::IsWow64Process as *const () as usize),
                    "IsWow64Process2" => {
                        Some(dlls::kernel32::IsWow64Process2 as *const () as usize)
                    }
                    "SetThreadInformation" => {
                        Some(dlls::kernel32::SetThreadInformation as *const () as usize)
                    }
                    "SetProcessInformation" => {
                        Some(dlls::kernel32::SetProcessInformation as *const () as usize)
                    }
                    "DiscardVirtualMemory" => {
                        Some(dlls::kernel32::DiscardVirtualMemory as *const () as usize)
                    }
                    "PrefetchVirtualMemory" => {
                        Some(dlls::kernel32::PrefetchVirtualMemory as *const () as usize)
                    }
                    "GetConsoleTitleW" => {
                        Some(dlls::kernel32::GetConsoleTitleW as *const () as usize)
                    }
                    "GetConsoleTitleA" => {
                        Some(dlls::kernel32::GetConsoleTitleA as *const () as usize)
                    }
                    "SetConsoleTitleW" => {
                        Some(dlls::kernel32::SetConsoleTitleW as *const () as usize)
                    }
                    "SetConsoleTitleA" => {
                        Some(dlls::kernel32::SetConsoleTitleA as *const () as usize)
                    }
                    "GetPackageFamilyName" => {
                        Some(dlls::kernel32::GetPackageFamilyName as *const () as usize)
                    }
                    "RegisterWaitForSingleObject" => {
                        Some(dlls::kernel32::RegisterWaitForSingleObject as *const () as usize)
                    }
                    "UnregisterWait" => {
                        Some(dlls::kernel32::UnregisterWait as *const () as usize)
                    }
                    "UnregisterWaitEx" => {
                        Some(dlls::kernel32::UnregisterWaitEx as *const () as usize)
                    }
                    "GetCurrentPackageFamilyName" => {
                        Some(dlls::kernel32::GetCurrentPackageFamilyName as *const () as usize)
                    }
                    "GetPackageFullName" => {
                        Some(dlls::kernel32::GetPackageFullName as *const () as usize)
                    }
                    "GetCurrentPackageFullName" => {
                        Some(dlls::kernel32::GetCurrentPackageFullName as *const () as usize)
                    }
                    "GetProcessIdOfThread" => Some(dlls::kernel32::GetProcessIdOfThread as *const () as usize),
                    "GetSystemWindowsDirectoryW" => Some(dlls::kernel32::GetSystemWindowsDirectoryW as *const () as usize),
                    "Module32First" => Some(dlls::kernel32::Module32First as *const () as usize),
                    "Module32Next" => Some(dlls::kernel32::Module32Next as *const () as usize),
                    "ConnectNamedPipe" => Some(dlls::kernel32::ConnectNamedPipe as *const () as usize),
                    "DisconnectNamedPipe" => Some(dlls::kernel32::DisconnectNamedPipe as *const () as usize),
                    "CreateNamedPipeA" => Some(dlls::kernel32::CreateNamedPipeA as *const () as usize),
                    "CreateNamedPipeW" => Some(dlls::kernel32::CreateNamedPipeW as *const () as usize),
                    "WaitNamedPipeW" => Some(dlls::kernel32::WaitNamedPipeW as *const () as usize),
                    "SetNamedPipeHandleState" => Some(dlls::kernel32::SetNamedPipeHandleState as *const () as usize),
                    "WriteProcessMemory" => Some(dlls::kernel32::WriteProcessMemory as *const () as usize),
                    "VirtualAllocEx" => Some(dlls::kernel32::VirtualAllocEx as *const () as usize),
                    "lstrcmpA" => Some(dlls::kernel32::lstrcmpA as *const () as usize),
                    "lstrcmpW" => Some(dlls::kernel32::lstrcmpW as *const () as usize),
                    "lstrcpyW" => Some(dlls::kernel32::lstrcpyW as *const () as usize),
                    "K32GetPerformanceInfo" => Some(dlls::kernel32::K32GetPerformanceInfo as *const () as usize),
                    "LCIDToLocaleName" => Some(dlls::kernel32::LCIDToLocaleName as *const () as usize),
                    "LocaleNameToLCID" => Some(dlls::kernel32::LocaleNameToLCID as *const () as usize),
                    "GetProcessAffinityMask" => Some(dlls::kernel32::GetProcessAffinityMask as *const () as usize),
                    "SetProcessAffinityMask" => Some(dlls::kernel32::SetProcessAffinityMask as *const () as usize),
                    "SetThreadIdealProcessor" | "SetThreadIdealProcessorEx" => Some(dlls::kernel32::SetThreadIdealProcessor as *const () as usize),
                    "GetLargePageMinimum" => Some(dlls::kernel32::GetLargePageMinimum as *const () as usize),
                    "GetDynamicTimeZoneInformation" => Some(dlls::kernel32::GetDynamicTimeZoneInformation as *const () as usize),
                    "GetGeoInfoW" => Some(dlls::kernel32::GetGeoInfoW as *const () as usize),
                    "GetUserGeoID" => Some(dlls::kernel32::GetUserGeoID as *const () as usize),
                    "TerminateThread" => Some(dlls::kernel32::TerminateThread as *const () as usize),
                    "ProcessIdToSessionId" => Some(dlls::kernel32::ProcessIdToSessionId as *const () as usize),
                    "DeviceIoControl" => Some(dlls::kernel32::DeviceIoControl as *const () as usize),
                    "GetProcAddress" => Some(dlls::kernel32::GetProcAddress as *const () as usize),
                    "LoadLibraryW" => Some(dlls::kernel32::LoadLibraryW as *const () as usize),
                    "LoadLibraryA" => Some(dlls::kernel32::LoadLibraryA as *const () as usize),
                    "LoadLibraryExW" => {
                        Some(dlls::kernel32::LoadLibraryExW as *const () as usize)
                    }
                    "LoadLibraryExA" => {
                        Some(dlls::kernel32::LoadLibraryExA as *const () as usize)
                    }
                    "FreeLibrary" | "FreeLibraryW" | "FreeLibraryA" => {
                        Some(dlls::kernel32::FreeLibrary as *const () as usize)
                    }
                    "SetDllDirectoryW" => {
                        Some(dlls::kernel32::SetDllDirectoryW as *const () as usize)
                    }
                    "SetDllDirectoryA" => {
                        Some(dlls::kernel32::SetDllDirectoryA as *const () as usize)
                    }
                    "GetDllDirectoryW" => {
                        Some(dlls::kernel32::GetDllDirectoryW as *const () as usize)
                    }
                    "GetDllDirectoryA" => {
                        Some(dlls::kernel32::GetDllDirectoryA as *const () as usize)
                    }
                    "AddDllDirectory" => {
                        Some(dlls::kernel32::AddDllDirectory as *const () as usize)
                    }
                    "RemoveDllDirectory" => {
                        Some(dlls::kernel32::RemoveDllDirectory as *const () as usize)
                    }
                    "SetDefaultDllDirectories" => {
                        Some(dlls::kernel32::SetDefaultDllDirectories as *const () as usize)
                    }
                    "GetModuleHandleW" => {
                        Some(dlls::kernel32::GetModuleHandleW as *const () as usize)
                    }
                    "GetModuleHandleA" => {
                        Some(dlls::kernel32::GetModuleHandleA as *const () as usize)
                    }
                    "GetModuleHandleExW" => {
                        Some(dlls::kernel32::GetModuleHandleExW as *const () as usize)
                    }
                    "GetModuleHandleExA" => {
                        Some(dlls::kernel32::GetModuleHandleExA as *const () as usize)
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
                    "GetLogicalProcessorInformation" => {
                        Some(dlls::kernel32::GetLogicalProcessorInformation as *const () as usize)
                    }
                    "GetLogicalProcessorInformationEx" => {
                        Some(dlls::kernel32::GetLogicalProcessorInformationEx as *const () as usize)
                    }
                    "GlobalMemoryStatus" => {
                        Some(dlls::kernel32::GlobalMemoryStatus as *const () as usize)
                    }
                    "GlobalMemoryStatusEx" => {
                        Some(dlls::kernel32::GlobalMemoryStatusEx as *const () as usize)
                    }
                    "GetDiskFreeSpaceExW" => {
                        Some(dlls::kernel32::GetDiskFreeSpaceExW as *const () as usize)
                    }
                    "GetDiskFreeSpaceExA" => {
                        Some(dlls::kernel32::GetDiskFreeSpaceExA as *const () as usize)
                    }
                    "GetLongPathNameW" => {
                        Some(dlls::kernel32::GetLongPathNameW as *const () as usize)
                    }
                    "GetLongPathNameA" => {
                        Some(dlls::kernel32::GetLongPathNameA as *const () as usize)
                    }
                    "GetShortPathNameW" => {
                        Some(dlls::kernel32::GetShortPathNameW as *const () as usize)
                    }
                    "GetShortPathNameA" => {
                        Some(dlls::kernel32::GetShortPathNameA as *const () as usize)
                    }
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
                    "IsValidLocale" => Some(dlls::kernel32::IsValidLocale as *const () as usize),
                    "IsValidLocaleName" => {
                        Some(dlls::kernel32::IsValidLocaleName as *const () as usize)
                    }
                    "GetUserPreferredUILanguages" => {
                        Some(dlls::kernel32::GetUserPreferredUILanguages as *const () as usize)
                    }
                    "GetThreadPreferredUILanguages" => {
                        Some(dlls::kernel32::GetThreadPreferredUILanguages as *const () as usize)
                    }
                    "GetStringTypeW" => Some(dlls::kernel32::GetStringTypeW as *const () as usize),
                    "GetStringTypeA" => Some(dlls::kernel32::GetStringTypeA as *const () as usize),
                    "GetStringTypeExW" => {
                        Some(dlls::kernel32::GetStringTypeW as *const () as usize)
                    }
                    "LCMapStringW" => Some(dlls::kernel32::LCMapStringW as *const () as usize),
                    "LCMapStringA" => Some(dlls::kernel32::LCMapStringW as *const () as usize),
                    "LCMapStringEx" => Some(dlls::kernel32::LCMapStringEx as *const () as usize),
                    "CompareStringW" => Some(dlls::kernel32::CompareStringW as *const () as usize),
                    "CompareStringA" => Some(dlls::kernel32::CompareStringA as *const () as usize),
                    "CompareStringEx" => {
                        Some(dlls::kernel32::CompareStringEx as *const () as usize)
                    }
                    "FormatMessageW" => Some(dlls::kernel32::FormatMessageW as *const () as usize),
                    "FormatMessageA" => Some(dlls::kernel32::FormatMessageA as *const () as usize),
                    "GetThreadLocale" => {
                        Some(dlls::kernel32::GetThreadLocale as *const () as usize)
                    }
                    "SetThreadLocale" => {
                        Some(dlls::kernel32::SetThreadLocale as *const () as usize)
                    }
                    "GetTimeZoneInformation" => {
                        Some(dlls::kernel32::GetTimeZoneInformation as *const () as usize)
                    }
                    "GetEnvironmentVariableW" => {
                        Some(dlls::kernel32::GetEnvironmentVariableW as *const () as usize)
                    }
                    "GetEnvironmentVariableA" => {
                        Some(dlls::kernel32::GetEnvironmentVariableA as *const () as usize)
                    }
                    "GetEnvironmentStringsW" | "GetEnvironmentStrings" => {
                        Some(dlls::kernel32::GetEnvironmentStringsW as *const () as usize)
                    }
                    "GetEnvironmentStringsA" => {
                        Some(dlls::kernel32::GetEnvironmentStringsA as *const () as usize)
                    }
                    "FreeEnvironmentStringsW" => {
                        Some(dlls::kernel32::FreeEnvironmentStringsW as *const () as usize)
                    }
                    "FreeEnvironmentStringsA" => {
                        Some(dlls::kernel32::FreeEnvironmentStringsA as *const () as usize)
                    }
                    "SetEnvironmentVariableW" => {
                        Some(dlls::kernel32::SetEnvironmentVariableW as *const () as usize)
                    }
                    "SetEnvironmentVariableA" => {
                        Some(dlls::kernel32::SetEnvironmentVariableA as *const () as usize)
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
                    "SetErrorMode" => Some(dlls::kernel32::SetErrorMode as *const () as usize),
                    "GetErrorMode" => Some(dlls::kernel32::GetErrorMode as *const () as usize),
                    "SetThreadErrorMode" => {
                        Some(dlls::kernel32::SetThreadErrorMode as *const () as usize)
                    }
                    "GetThreadErrorMode" => {
                        Some(dlls::kernel32::GetThreadErrorMode as *const () as usize)
                    }
                    "SetThreadDescription" => {
                        Some(dlls::kernel32::SetThreadDescription as *const () as usize)
                    }
                    "RtlPcToFileHeader" => {
                        Some(dlls::kernel32::RtlPcToFileHeader as *const () as usize)
                    }
                    "RtlLookupFunctionEntry" => {
                        Some(dlls::kernel32::RtlLookupFunctionEntry as *const () as usize)
                    }
                    "RtlLookupFunctionTable" => {
                        Some(dlls::kernel32::RtlLookupFunctionTable as *const () as usize)
                    }
                    "RtlAddGrowableFunctionTable" => {
                        Some(dlls::kernel32::RtlAddGrowableFunctionTable as *const () as usize)
                    }
                    "RtlGrowFunctionTable" => {
                        Some(dlls::kernel32::RtlGrowFunctionTable as *const () as usize)
                    }
                    "RtlDeleteGrowableFunctionTable" => {
                        Some(dlls::kernel32::RtlDeleteGrowableFunctionTable as *const () as usize)
                    }
                    "RtlAddFunctionTable" => {
                        Some(dlls::kernel32::RtlAddFunctionTable as *const () as usize)
                    }
                    "RtlDeleteFunctionTable" => {
                        Some(dlls::kernel32::RtlDeleteFunctionTable as *const () as usize)
                    }
                    "RtlInstallFunctionTableCallback" => {
                        Some(dlls::kernel32::RtlInstallFunctionTableCallback as *const () as usize)
                    }
                    "RtlVirtualUnwind" => {
                        Some(dlls::kernel32::RtlVirtualUnwind as *const () as usize)
                    }
                    "RtlCaptureContext" => {
                        Some(dlls::kernel32::RtlCaptureContext as *const () as usize)
                    }
                    "RtlRestoreContext" => {
                        Some(dlls::kernel32::RtlRestoreContext as *const () as usize)
                    }
                    "RtlCaptureStackBackTrace" => {
                        Some(dlls::kernel32::RtlCaptureStackBackTrace as *const () as usize)
                    }
                    "RtlUnwind" => Some(dlls::kernel32::RtlUnwind as *const () as usize),
                    "RtlUnwindEx" => Some(dlls::kernel32::RtlUnwindEx as *const () as usize),
                    "GetTempPathW" => Some(dlls::kernel32::GetTempPathW as *const () as usize),
                    "GetTempPathA" => Some(dlls::kernel32::GetTempPathA as *const () as usize),
                    "GetTempPath2W" => Some(dlls::kernel32::GetTempPath2W as *const () as usize),
                    "GetTempPath2A" => Some(dlls::kernel32::GetTempPath2A as *const () as usize),
                    "GetTempFileNameW" => {
                        Some(dlls::kernel32::GetTempFileNameW as *const () as usize)
                    }
                    "EnumSystemLocalesEx" => {
                        Some(dlls::kernel32::EnumSystemLocalesEx as *const () as usize)
                    }
                    "GetDateFormatEx" => {
                        Some(dlls::kernel32::GetDateFormatEx as *const () as usize)
                    }
                    "GetTimeFormatEx" => {
                        Some(dlls::kernel32::GetTimeFormatEx as *const () as usize)
                    }
                    "VerifyVersionInfoW" => {
                        Some(dlls::kernel32::VerifyVersionInfoW as *const () as usize)
                    }
                    "VerifyVersionInfoA" => {
                        Some(dlls::kernel32::VerifyVersionInfoA as *const () as usize)
                    }
                    "SetConsoleCtrlHandler" => {
                        Some(dlls::kernel32::SetConsoleCtrlHandler as *const () as usize)
                    }
                    "QueueUserWorkItem" => {
                        Some(dlls::kernel32::QueueUserWorkItem as *const () as usize)
                    }
                    "GetProductInfo" => {
                        Some(dlls::kernel32::GetProductInfo as *const () as usize)
                    }
                    "GetComputerNameExW" => {
                        Some(dlls::kernel32::GetComputerNameExW as *const () as usize)
                    }
                    "CreateIoCompletionPort" => {
                        Some(dlls::kernel32::CreateIoCompletionPort as *const () as usize)
                    }
                    "PostQueuedCompletionStatus" => {
                        Some(dlls::kernel32::PostQueuedCompletionStatus as *const () as usize)
                    }
                    "GetQueuedCompletionStatus" => {
                        Some(dlls::kernel32::GetQueuedCompletionStatus as *const () as usize)
                    }
                    "GetQueuedCompletionStatusEx" => {
                        Some(dlls::kernel32::GetQueuedCompletionStatusEx as *const () as usize)
                    }
                    "VerSetConditionMask" => {
                        Some(dlls::kernel32::VerSetConditionMask as *const () as usize)
                    }
                    "OpenThread" => Some(dlls::kernel32::OpenThread as *const () as usize),
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
                    "IsDebuggerPresent" => {
                        Some(dlls::kernel32::IsDebuggerPresent as *const () as usize)
                    }
                    "CheckRemoteDebuggerPresent" => {
                        Some(dlls::kernel32::CheckRemoteDebuggerPresent as *const () as usize)
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
                    "GetCommandLineW" => {
                        Some(dlls::kernel32::GetCommandLineW as *const () as usize)
                    }
                    "GetCommandLineA" => {
                        Some(dlls::kernel32::GetCommandLineA as *const () as usize)
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
                    "GetConsoleOutputCP" => {
                        Some(dlls::kernel32::GetConsoleOutputCP as *const () as usize)
                    }
                    "AttachConsole" => Some(dlls::kernel32::AttachConsole as *const () as usize),
                    "SetConsoleCP" => Some(dlls::kernel32::SetConsoleCP as *const () as usize),
                    "GetConsoleCP" => Some(dlls::kernel32::GetConsoleCP as *const () as usize),
                    "WriteConsoleW" | "WriteConsoleA" => {
                        Some(dlls::kernel32::WriteConsoleW as *const () as usize)
                    }
                    "FindResourceW" => Some(dlls::kernel32::FindResourceW as *const () as usize),
                    "FindResourceA" => Some(dlls::kernel32::FindResourceA as *const () as usize),
                    "LoadResource" => Some(dlls::kernel32::LoadResource as *const () as usize),
                    "LockResource" => Some(dlls::kernel32::LockResource as *const () as usize),
                    "SizeofResource" => Some(dlls::kernel32::SizeofResource as *const () as usize),
                    "GetStartupInfoW" => {
                        Some(dlls::kernel32::GetStartupInfoW as *const () as usize)
                    }
                    "GetStartupInfoA" => {
                        Some(dlls::kernel32::GetStartupInfoA as *const () as usize)
                    }
                    "GetProcessHeap" => Some(dlls::kernel32::GetProcessHeap as *const () as usize),
                    "HeapAlloc" => Some(dlls::kernel32::HeapAlloc as *const () as usize),
                    "HeapFree" => Some(dlls::kernel32::HeapFree as *const () as usize),
                    "HeapReAlloc" => Some(dlls::kernel32::HeapReAlloc as *const () as usize),
                    "HeapSize" => Some(dlls::kernel32::HeapSize as *const () as usize),
                    "HeapCreate" => Some(dlls::kernel32::HeapCreate as *const () as usize),
                    "HeapDestroy" => Some(dlls::kernel32::HeapDestroy as *const () as usize),
                    "HeapValidate" => Some(dlls::kernel32::HeapValidate as *const () as usize),
                    "GetProcessHeaps" => Some(dlls::kernel32::GetProcessHeaps as *const () as usize),
                    "HeapCompact" => Some(dlls::kernel32::HeapCompact as *const () as usize),
                    "HeapWalk" => Some(dlls::kernel32::HeapWalk as *const () as usize),
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
                    "VirtualProtect" => Some(dlls::kernel32::VirtualProtect as *const () as usize),
                    "VirtualProtectEx" => {
                        Some(dlls::kernel32::VirtualProtectEx as *const () as usize)
                    }
                    "VirtualQuery" => Some(dlls::kernel32::VirtualQuery as *const () as usize),
                    "VirtualQueryEx" => Some(dlls::kernel32::VirtualQueryEx as *const () as usize),
                    "InitializeCriticalSection" => {
                        Some(dlls::kernel32::InitializeCriticalSection as *const () as usize)
                    }
                    "InitializeCriticalSectionAndSpinCount" => Some(
                        dlls::kernel32::InitializeCriticalSectionAndSpinCount as *const () as usize,
                    ),
                    "InitializeCriticalSectionEx" => {
                        Some(dlls::kernel32::InitializeCriticalSectionEx as *const () as usize)
                    }
                    "SetCriticalSectionSpinCount" => {
                        Some(dlls::kernel32::SetCriticalSectionSpinCount as *const () as usize)
                    }
                    "InitializeSListHead" => {
                        Some(dlls::kernel32::InitializeSListHead as *const () as usize)
                    }
                    "QueryDepthSList" => {
                        Some(dlls::kernel32::QueryDepthSList as *const () as usize)
                    }
                    "InterlockedFlushSList" => {
                        Some(dlls::kernel32::InterlockedFlushSList as *const () as usize)
                    }
                    "InterlockedPushEntrySList" => {
                        Some(dlls::kernel32::InterlockedPushEntrySList as *const () as usize)
                    }
                    "InterlockedPopEntrySList" => {
                        Some(dlls::kernel32::InterlockedPopEntrySList as *const () as usize)
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
                    "WaitOnAddress" => Some(dlls::kernel32::WaitOnAddress as *const () as usize),
                    "WakeByAddressSingle" => {
                        Some(dlls::kernel32::WakeByAddressSingle as *const () as usize)
                    }
                    "WakeByAddressAll" => {
                        Some(dlls::kernel32::WakeByAddressAll as *const () as usize)
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
                    "GetACP" => Some(dlls::kernel32::GetACP as *const () as usize),
                    "GetOEMCP" => Some(dlls::kernel32::GetOEMCP as *const () as usize),
                    "IsValidCodePage" => {
                        Some(dlls::kernel32::IsValidCodePage as *const () as usize)
                    }
                    "GetCPInfo" => Some(dlls::kernel32::GetCPInfo as *const () as usize),
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
                    "CreateFileW" => Some(dlls::kernel32::CreateFileW as *const () as usize),
                    "CreateFileA" => Some(dlls::kernel32::CreateFileA as *const () as usize),
                    "GetFileAttributesW" => {
                        Some(dlls::kernel32::GetFileAttributesW as *const () as usize)
                    }
                    "GetFileAttributesA" => {
                        Some(dlls::kernel32::GetFileAttributesA as *const () as usize)
                    }
                    "GetFileAttributesExW" => {
                        Some(dlls::kernel32::GetFileAttributesExW as *const () as usize)
                    }
                    "GetFileAttributesExA" => {
                        Some(dlls::kernel32::GetFileAttributesExA as *const () as usize)
                    }
                    "SetFileAttributesW" => {
                        Some(dlls::kernel32::SetFileAttributesW as *const () as usize)
                    }
                    "SetFileAttributesA" => {
                        Some(dlls::kernel32::SetFileAttributesA as *const () as usize)
                    }
                    "FindFirstFileW" => Some(dlls::kernel32::FindFirstFileW as *const () as usize),
                    "FindFirstFileA" => Some(dlls::kernel32::FindFirstFileA as *const () as usize),
                    "FindFirstFileExW" => {
                        Some(dlls::kernel32::FindFirstFileExW as *const () as usize)
                    }
                    "FindFirstFileExA" => {
                        Some(dlls::kernel32::FindFirstFileExA as *const () as usize)
                    }
                    "FindNextFileW" => Some(dlls::kernel32::FindNextFileW as *const () as usize),
                    "FindNextFileA" => Some(dlls::kernel32::FindNextFileA as *const () as usize),
                    "FindClose" => Some(dlls::kernel32::FindClose as *const () as usize),
                    "CreatePipe" => Some(dlls::kernel32::CreatePipe as *const () as usize),
                    "ReadFile" => Some(dlls::kernel32::ReadFile as *const () as usize),
                    "WriteFile" => Some(dlls::kernel32::WriteFile as *const () as usize),
                    "GetFileSize" => Some(dlls::kernel32::GetFileSize as *const () as usize),
                    "GetFileSizeEx" => Some(dlls::kernel32::GetFileSizeEx as *const () as usize),
                    "GetFileInformationByHandle" => {
                        Some(dlls::kernel32::GetFileInformationByHandle as *const () as usize)
                    }
                    "GetFileInformationByHandleEx" => {
                        Some(dlls::kernel32::GetFileInformationByHandleEx as *const () as usize)
                    }
                    "CreateFileMappingW" => {
                        Some(dlls::kernel32::CreateFileMappingW as *const () as usize)
                    }
                    "CreateFileMappingA" => {
                        Some(dlls::kernel32::CreateFileMappingA as *const () as usize)
                    }
                    "OpenFileMappingW" => {
                        Some(dlls::kernel32::OpenFileMappingW as *const () as usize)
                    }
                    "OpenFileMappingA" => {
                        Some(dlls::kernel32::OpenFileMappingA as *const () as usize)
                    }
                    "MapViewOfFile" => Some(dlls::kernel32::MapViewOfFile as *const () as usize),
                    "MapViewOfFileEx" => {
                        Some(dlls::kernel32::MapViewOfFileEx as *const () as usize)
                    }
                    "UnmapViewOfFile" => {
                        Some(dlls::kernel32::UnmapViewOfFile as *const () as usize)
                    }
                    "FlushViewOfFile" => {
                        Some(dlls::kernel32::FlushViewOfFile as *const () as usize)
                    }
                    "SetFilePointer" => Some(dlls::kernel32::SetFilePointer as *const () as usize),
                    "SetFilePointerEx" => {
                        Some(dlls::kernel32::SetFilePointerEx as *const () as usize)
                    }
                    "MoveFileExW" | "MoveFileW" => {
                        Some(dlls::kernel32::MoveFileExW as *const () as usize)
                    }
                    "MoveFileExA" | "MoveFileA" => {
                        Some(dlls::kernel32::MoveFileExA as *const () as usize)
                    }
                    "DeleteFileW" => Some(dlls::kernel32::DeleteFileW as *const () as usize),
                    "DeleteFileA" => Some(dlls::kernel32::DeleteFileA as *const () as usize),
                    "CopyFileW" => Some(dlls::kernel32::CopyFileW as *const () as usize),
                    "CopyFileA" => Some(dlls::kernel32::CopyFileA as *const () as usize),
                    "ReplaceFileW" | "ReplaceFileA" => {
                        Some(dlls::kernel32::ReplaceFileW as *const () as usize)
                    }
                    "FlushFileBuffers" => {
                        Some(dlls::kernel32::FlushFileBuffers as *const () as usize)
                    }
                    "SetEndOfFile" => Some(dlls::kernel32::SetEndOfFile as *const () as usize),
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
                    "CreateProcessW" => Some(dlls::kernel32::CreateProcessW as *const () as usize),
                    "CreateProcessA" => Some(dlls::kernel32::CreateProcessA as *const () as usize),
                    "InitializeProcThreadAttributeList" => Some(
                        dlls::kernel32::InitializeProcThreadAttributeList as *const () as usize,
                    ),
                    "UpdateProcThreadAttribute" => {
                        Some(dlls::kernel32::UpdateProcThreadAttribute as *const () as usize)
                    }
                    "DeleteProcThreadAttributeList" => {
                        Some(dlls::kernel32::DeleteProcThreadAttributeList as *const () as usize)
                    }
                    "SetPriorityClass" => {
                        Some(dlls::kernel32::SetPriorityClass as *const () as usize)
                    }
                    "GetPriorityClass" => {
                        Some(dlls::kernel32::GetPriorityClass as *const () as usize)
                    }
                    "CreateToolhelp32Snapshot" => {
                        Some(dlls::kernel32::CreateToolhelp32Snapshot as *const () as usize)
                    }
                    "Thread32First" => {
                        Some(dlls::kernel32::Thread32First as *const () as usize)
                    }
                    "Thread32Next" => {
                        Some(dlls::kernel32::Thread32Next as *const () as usize)
                    }
                    "Process32FirstW" => {
                        Some(dlls::kernel32::Process32FirstW as *const () as usize)
                    }
                    "Process32NextW" => {
                        Some(dlls::kernel32::Process32NextW as *const () as usize)
                    }
                    "Process32First" => {
                        Some(dlls::kernel32::Process32First as *const () as usize)
                    }
                    "Process32Next" => {
                        Some(dlls::kernel32::Process32Next as *const () as usize)
                    }
                    "GetProcessId" => {
                        Some(dlls::kernel32::GetProcessId as *const () as usize)
                    }
                    "SwitchToThread" => {
                        Some(dlls::kernel32::SwitchToThread as *const () as usize)
                    }
                    "SetThreadAffinityMask" => {
                        Some(dlls::kernel32::SetThreadAffinityMask as *const () as usize)
                    }
                    "GetSystemPowerStatus" => {
                        Some(dlls::kernel32::GetSystemPowerStatus as *const () as usize)
                    }
                    "GetComputerNameW" => {
                        Some(dlls::kernel32::GetComputerNameW as *const () as usize)
                    }
                    "GetSystemDirectoryA" => {
                        Some(dlls::kernel32::GetSystemDirectoryA as *const () as usize)
                    }
                    "GetSystemDirectoryW" => {
                        Some(dlls::kernel32::GetSystemDirectoryW as *const () as usize)
                    }
                    "GetWindowsDirectoryW" => {
                        Some(dlls::kernel32::GetWindowsDirectoryW as *const () as usize)
                    }
                    "GetDriveTypeW" => {
                        Some(dlls::kernel32::GetDriveTypeW as *const () as usize)
                    }
                    "GetDriveTypeA" => {
                        Some(dlls::kernel32::GetDriveTypeA as *const () as usize)
                    }
                    "HeapQueryInformation" => {
                        Some(dlls::kernel32::HeapQueryInformation as *const () as usize)
                    }
                    "HeapSetInformation" => {
                        Some(dlls::kernel32::HeapSetInformation as *const () as usize)
                    }
                    "SetConsoleScreenBufferSize" => {
                        Some(dlls::kernel32::SetConsoleScreenBufferSize as *const () as usize)
                    }
                    "SetConsoleWindowInfo" => {
                        Some(dlls::kernel32::SetConsoleWindowInfo as *const () as usize)
                    }
                    "ReadConsoleA" => {
                        Some(dlls::kernel32::ReadConsoleA as *const () as usize)
                    }
                    "CompareStringOrdinal" => {
                        Some(dlls::kernel32::CompareStringOrdinal as *const () as usize)
                    }
                    "SetHandleCount" => {
                        Some(dlls::kernel32::SetHandleCount as *const () as usize)
                    }
                    "FatalAppExitA" => {
                        Some(dlls::kernel32::FatalAppExitA as *const () as usize)
                    }
                    "ExpandEnvironmentStringsW" => {
                        Some(dlls::kernel32::ExpandEnvironmentStringsW as *const () as usize)
                    }
                    "ExpandEnvironmentStringsA" => {
                        Some(dlls::kernel32::ExpandEnvironmentStringsA as *const () as usize)
                    }
                    "CompareFileTime" => {
                        Some(dlls::kernel32::CompareFileTime as *const () as usize)
                    }
                    "VirtualUnlock" => {
                        Some(dlls::kernel32::VirtualUnlock as *const () as usize)
                    }
                    "GetProcessHandleCount" => {
                        Some(dlls::kernel32::GetProcessHandleCount as *const () as usize)
                    }
                    "AllocConsole" => Some(dlls::kernel32::AllocConsole as *const () as usize),
                    "GetConsoleWindow" => {
                        Some(dlls::kernel32::GetConsoleWindow as *const () as usize)
                    }
                    "GetFinalPathNameByHandleW" => {
                        Some(dlls::kernel32::GetFinalPathNameByHandleW as *const () as usize)
                    }
                    "K32GetProcessMemoryInfo" => {
                        Some(dlls::kernel32::K32GetProcessMemoryInfo as *const () as usize)
                    }
                    "CancelIo" => {
                        Some(dlls::kernel32::CancelIo as *const () as usize)
                    }
                    "SetFileTime" => {
                        Some(dlls::kernel32::SetFileTime as *const () as usize)
                    }
                    "RemoveDirectoryW" => {
                        Some(dlls::kernel32::RemoveDirectoryW as *const () as usize)
                    }
                    "ExitThread" => {
                        Some(dlls::kernel32::ExitThread as *const () as usize)
                    }
                    "FreeLibraryAndExitThread" => {
                        Some(dlls::kernel32::FreeLibraryAndExitThread as *const () as usize)
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
                    "GetThreadContext" => {
                        Some(dlls::kernel32::GetThreadContext as *const () as usize)
                    }
                    "SetThreadContext" => {
                        Some(dlls::kernel32::SetThreadContext as *const () as usize)
                    }
                    "Sleep" => Some(dlls::kernel32::Sleep as *const () as usize),
                    "SleepEx" => Some(dlls::kernel32::SleepEx as *const () as usize),
                    // Synchronization primitives
                    "CreateEventExW" => {
                        Some(dlls::kernel32::CreateEventExW as *const () as usize)
                    }
                    "CreateEventExA" => {
                        Some(dlls::kernel32::CreateEventExA as *const () as usize)
                    }
                    "PulseEvent" => Some(dlls::kernel32::PulseEvent as *const () as usize),
                    "SetEvent" => Some(dlls::kernel32::SetEvent as *const () as usize),
                    "ResetEvent" => Some(dlls::kernel32::ResetEvent as *const () as usize),
                    "WaitForSingleObject" => {
                        Some(dlls::kernel32::WaitForSingleObject as *const () as usize)
                    }
                    "WaitForSingleObjectEx" => {
                        Some(dlls::kernel32::WaitForSingleObjectEx as *const () as usize)
                    }
                    "WaitForMultipleObjects" => {
                        Some(dlls::kernel32::WaitForMultipleObjects as *const () as usize)
                    }
                    "WaitForMultipleObjectsEx" => {
                        Some(dlls::kernel32::WaitForMultipleObjectsEx as *const () as usize)
                    }
                    "CloseHandle" => Some(dlls::kernel32::CloseHandle as *const () as usize),
                    "GetHandleInformation" => {
                        Some(dlls::kernel32::GetHandleInformation as *const () as usize)
                    }
                    "SetHandleInformation" => {
                        Some(dlls::kernel32::SetHandleInformation as *const () as usize)
                    }
                    "CreateMutexExW" => {
                        Some(dlls::kernel32::CreateMutexExW as *const () as usize)
                    }
                    "CreateMutexExA" => {
                        Some(dlls::kernel32::CreateMutexExA as *const () as usize)
                    }
                    "ReleaseMutex" => Some(dlls::kernel32::ReleaseMutex as *const () as usize),
                    "CreateSemaphoreExW" => {
                        Some(dlls::kernel32::CreateSemaphoreExW as *const () as usize)
                    }
                    "CreateSemaphoreExA" => {
                        Some(dlls::kernel32::CreateSemaphoreExA as *const () as usize)
                    }
                    "CreateWaitableTimerW" => {
                        Some(dlls::kernel32::CreateWaitableTimerW as *const () as usize)
                    }
                    "CreateWaitableTimerA" => {
                        Some(dlls::kernel32::CreateWaitableTimerA as *const () as usize)
                    }
                    "CreateWaitableTimerExW" => {
                        Some(dlls::kernel32::CreateWaitableTimerExW as *const () as usize)
                    }
                    "CreateWaitableTimerExA" => {
                        Some(dlls::kernel32::CreateWaitableTimerExA as *const () as usize)
                    }
                    "OpenWaitableTimerW" => {
                        Some(dlls::kernel32::OpenWaitableTimerW as *const () as usize)
                    }
                    "OpenWaitableTimerA" => {
                        Some(dlls::kernel32::OpenWaitableTimerA as *const () as usize)
                    }
                    "SetWaitableTimer" => {
                        Some(dlls::kernel32::SetWaitableTimer as *const () as usize)
                    }
                    "SetWaitableTimerEx" => {
                        Some(dlls::kernel32::SetWaitableTimerEx as *const () as usize)
                    }
                    "CancelWaitableTimer" => {
                        Some(dlls::kernel32::CancelWaitableTimer as *const () as usize)
                    }
                    "GetVersion" => Some(dlls::kernel32::GetVersion as *const () as usize),
                    "GetVersionExW" => Some(dlls::kernel32::GetVersionExW as *const () as usize),
                    "GetVersionExA" => Some(dlls::kernel32::GetVersionExA as *const () as usize),
                    "VerifyVersionInfoW" => {
                        Some(dlls::kernel32::VerifyVersionInfoW as *const () as usize)
                    }
                    "VerifyVersionInfoA" => {
                        Some(dlls::kernel32::VerifyVersionInfoA as *const () as usize)
                    }
                    "IsBadReadPtr" => Some(dlls::kernel32::IsBadReadPtr as *const () as usize),
                    "IsBadWritePtr" => Some(dlls::kernel32::IsBadWritePtr as *const () as usize),
                    "IsBadCodePtr" => Some(dlls::kernel32::IsBadCodePtr as *const () as usize),
                    "IsBadStringPtrA" => Some(dlls::kernel32::IsBadStringPtrA as *const () as usize),
                    "FlushInstructionCache" => {
                        Some(dlls::kernel32::FlushInstructionCache as *const () as usize)
                    }
                    "GetExitCodeProcess" => {
                        Some(dlls::kernel32::GetExitCodeProcess as *const () as usize)
                    }
                    "OpenProcess" => Some(dlls::kernel32::OpenProcess as *const () as usize),
                    "IsProcessorFeaturePresent" => {
                        Some(dlls::kernel32::IsProcessorFeaturePresent as *const () as usize)
                    }
                    "UnhandledExceptionFilter" => {
                        Some(dlls::kernel32::UnhandledExceptionFilter as *const () as usize)
                    }
                    "FlsGetValue2" => Some(dlls::kernel32::FlsGetValue2 as *const () as usize),
                    "AreFileApisANSI" => Some(dlls::kernel32::AreFileApisANSI as *const () as usize),
                    "SetFileApisToOEM" => Some(dlls::kernel32::SetFileApisToOEM as *const () as usize),
                    "SetFileApisToANSI" => Some(dlls::kernel32::SetFileApisToANSI as *const () as usize),
                    "AddVectoredExceptionHandler" => Some(dlls::kernel32::AddVectoredExceptionHandler as *const () as usize),
                    "CancelIoEx" => Some(dlls::kernel32::CancelIoEx as *const () as usize),
                    "CancelSynchronousIo" => Some(dlls::kernel32::CancelSynchronousIo as *const () as usize),
                    "DebugBreak" => Some(dlls::kernel32::DebugBreak as *const () as usize),
                    "EnumProcessModules" => Some(dlls::kernel32::EnumProcessModules as *const () as usize),
                    "EnumProcessModulesEx" => Some(dlls::kernel32::EnumProcessModulesEx as *const () as usize),
                    "EnumProcesses" => Some(dlls::kernel32::EnumProcesses as *const () as usize),
                    "EnumSystemLocalesW" => Some(dlls::kernel32::EnumSystemLocalesW as *const () as usize),
                    "EnumSystemLocalesA" => Some(dlls::kernel32::EnumSystemLocalesA as *const () as usize),
                    "FileTimeToSystemTime" => Some(dlls::kernel32::FileTimeToSystemTime as *const () as usize),
                    "FileTimeToLocalFileTime" => Some(dlls::kernel32::FileTimeToLocalFileTime as *const () as usize),
                    "LocalFileTimeToFileTime" => Some(dlls::kernel32::LocalFileTimeToFileTime as *const () as usize),
                    "FileTimeToDosDateTime" => Some(dlls::kernel32::FileTimeToDosDateTime as *const () as usize),
                    "DosDateTimeToFileTime" => Some(dlls::kernel32::DosDateTimeToFileTime as *const () as usize),
                    "FlushProcessWriteBuffers" => Some(dlls::kernel32::FlushProcessWriteBuffers as *const () as usize),
                    "FreeConsole" => Some(dlls::kernel32::FreeConsole as *const () as usize),
                    "GetCPInfoExW" => Some(dlls::kernel32::GetCPInfoExW as *const () as usize),
                    "GetCurrentDirectoryA" => Some(dlls::kernel32::GetCurrentDirectoryA as *const () as usize),
                    "GetCurrentProcessorNumberEx" => Some(dlls::kernel32::GetCurrentProcessorNumberEx as *const () as usize),
                    "GetDateFormatW" => Some(dlls::kernel32::GetDateFormatW as *const () as usize),
                    "GetFullPathNameW" => Some(dlls::kernel32::GetFullPathNameW as *const () as usize),
                    "GetFullPathNameA" => Some(dlls::kernel32::GetFullPathNameA as *const () as usize),
                    "GetLogicalDriveStringsW" => Some(dlls::kernel32::GetLogicalDriveStringsW as *const () as usize),
                    "GetModuleBaseNameA" => Some(dlls::kernel32::GetModuleBaseNameA as *const () as usize),
                    "GetModuleBaseNameW" => Some(dlls::kernel32::GetModuleBaseNameW as *const () as usize),
                    "GetModuleFileNameExA" => Some(dlls::kernel32::GetModuleFileNameExA as *const () as usize),
                    "GetModuleFileNameExW" => Some(dlls::kernel32::GetModuleFileNameExW as *const () as usize),
                    "GetModuleInformation" => Some(dlls::kernel32::GetModuleInformation as *const () as usize),
                    "GetNumberOfConsoleInputEvents" => Some(dlls::kernel32::GetNumberOfConsoleInputEvents as *const () as usize),
                    "GetOverlappedResult" => Some(dlls::kernel32::GetOverlappedResult as *const () as usize),
                    "GetProcessTimes" => Some(dlls::kernel32::GetProcessTimes as *const () as usize),
                    "GetThreadTimes" => Some(dlls::kernel32::GetThreadTimes as *const () as usize),
                    "GetProcessWorkingSetSize" => Some(dlls::kernel32::GetProcessWorkingSetSize as *const () as usize),
                    "GetSystemTimePreciseAsFileTime" => Some(dlls::kernel32::GetSystemTimePreciseAsFileTime as *const () as usize),
                    "GetSystemTimes" => Some(dlls::kernel32::GetSystemTimes as *const () as usize),
                    "GetTickCount64" => Some(dlls::kernel32::GetTickCount64 as *const () as usize),
                    "GetTimeFormatW" => Some(dlls::kernel32::GetTimeFormatW as *const () as usize),
                    "GetVolumeInformationW" => Some(dlls::kernel32::GetVolumeInformationW as *const () as usize),
                    "K32EnumProcessModules" => Some(dlls::kernel32::K32EnumProcessModules as *const () as usize),
                    "K32EnumProcessModulesEx" => Some(dlls::kernel32::K32EnumProcessModulesEx as *const () as usize),
                    "K32EnumProcesses" => Some(dlls::kernel32::K32EnumProcesses as *const () as usize),
                    "K32GetModuleBaseNameA" => Some(dlls::kernel32::K32GetModuleBaseNameA as *const () as usize),
                    "K32GetModuleBaseNameW" => Some(dlls::kernel32::K32GetModuleBaseNameW as *const () as usize),
                    "K32GetModuleFileNameExA" => Some(dlls::kernel32::K32GetModuleFileNameExA as *const () as usize),
                    "K32GetModuleFileNameExW" => Some(dlls::kernel32::K32GetModuleFileNameExW as *const () as usize),
                    "K32GetModuleInformation" => Some(dlls::kernel32::K32GetModuleInformation as *const () as usize),
                    "LockFile" => Some(dlls::kernel32::LockFile as *const () as usize),
                    "OpenSemaphoreW" => Some(dlls::kernel32::OpenSemaphoreW as *const () as usize),
                    "PeekConsoleInputA" => Some(dlls::kernel32::PeekConsoleInputA as *const () as usize),
                    "PeekNamedPipe" => Some(dlls::kernel32::PeekNamedPipe as *const () as usize),
                    "QueueUserAPC" => Some(dlls::kernel32::QueueUserAPC as *const () as usize),
                    "ReadConsoleInputW" => Some(dlls::kernel32::ReadConsoleInputW as *const () as usize),
                    "ReadConsoleW" => Some(dlls::kernel32::ReadConsoleW as *const () as usize),
                    "ReadProcessMemory" => Some(dlls::kernel32::ReadProcessMemory as *const () as usize),
                    "RemoveVectoredExceptionHandler" => Some(dlls::kernel32::RemoveVectoredExceptionHandler as *const () as usize),
                    "SetProcessWorkingSetSize" => Some(dlls::kernel32::SetProcessWorkingSetSize as *const () as usize),
                    "SetStdHandle" => Some(dlls::kernel32::SetStdHandle as *const () as usize),
                    "SetThreadStackGuarantee" => Some(dlls::kernel32::SetThreadStackGuarantee as *const () as usize),
                    "SignalObjectAndWait" => Some(dlls::kernel32::SignalObjectAndWait as *const () as usize),
                    "SystemTimeToTzSpecificLocalTime" => Some(dlls::kernel32::SystemTimeToTzSpecificLocalTime as *const () as usize),
                    "TzSpecificLocalTimeToSystemTime" => Some(dlls::kernel32::TzSpecificLocalTimeToSystemTime as *const () as usize),
                    "UnlockFile" => Some(dlls::kernel32::UnlockFile as *const () as usize),
                    "VerLanguageNameW" => Some(dlls::kernel32::VerLanguageNameW as *const () as usize),
                    _ => None,
                }
            }
            "USER32.DLL" | "USER32" | "user32.dll" | "WIN32U.DLL" => match func_name {
                "LoadCursorFromFileW" => Some(dlls::user32::LoadCursorFromFileW as *const () as usize),
                "LoadCursorFromFileA" => Some(dlls::user32::LoadCursorFromFileA as *const () as usize),
                "DisableProcessWindowsGhosting" => Some(dlls::user32::DisableProcessWindowsGhosting as *const () as usize),
                "GetProcessWindowStation" => Some(dlls::user32::GetProcessWindowStation as *const () as usize),
                "GetUserObjectInformationW" => Some(dlls::user32::GetUserObjectInformationW as *const () as usize),
                "GetWindowInfo" => Some(dlls::user32::GetWindowInfo as *const () as usize),
                "GetTopWindow" => Some(dlls::user32::GetTopWindow as *const () as usize),
                "BeginPaint" => Some(dlls::user32::BeginPaint as *const () as usize),
                "EndPaint" => Some(dlls::user32::EndPaint as *const () as usize),
                "GetSystemMenu" => Some(dlls::user32::GetSystemMenu as *const () as usize),
                "EnableMenuItem" => Some(dlls::user32::EnableMenuItem as *const () as usize),
                "RegisterHotKey" => Some(dlls::user32::RegisterHotKey as *const () as usize),
                "UnregisterHotKey" => Some(dlls::user32::UnregisterHotKey as *const () as usize),
                "SetLayeredWindowAttributes" => Some(dlls::user32::SetLayeredWindowAttributes as *const () as usize),
                "GetWindowDisplayAffinity" => Some(dlls::user32::GetWindowDisplayAffinity as *const () as usize),
                "SetWindowDisplayAffinity" => Some(dlls::user32::SetWindowDisplayAffinity as *const () as usize),
                "DrawStateW" => Some(dlls::user32::DrawStateW as *const () as usize),
                "keybd_event" => Some(dlls::user32::keybd_event as *const () as usize),
                "SendMessageTimeoutW" => Some(dlls::user32::SendMessageTimeoutW as *const () as usize),
                "MapVirtualKeyExW" => Some(dlls::user32::MapVirtualKeyExW as *const () as usize),
                "GetCursorInfo" => Some(dlls::user32::GetCursorInfo as *const () as usize),
                "EnumChildWindows" => Some(dlls::user32::EnumChildWindows as *const () as usize),
                "GetIconInfo" => Some(dlls::user32::GetIconInfo as *const () as usize),
                "GetWindowThreadProcessId" => Some(dlls::user32::GetWindowThreadProcessId as *const () as usize),
                "SetClassLongPtrW" => Some(dlls::user32::SetClassLongPtrW as *const () as usize),
                "GetClassLongPtrW" => Some(dlls::user32::GetClassLongPtrW as *const () as usize),
                "IsWindowUnicode" => Some(dlls::user32::IsWindowUnicode as *const () as usize),
                "SendInput" => Some(dlls::user32::SendInput as *const () as usize),
                "IsChild" => Some(dlls::user32::IsChild as *const () as usize),
                "CreateCaret" => Some(dlls::user32::CreateCaret as *const () as usize),
                "DestroyCaret" => Some(dlls::user32::DestroyCaret as *const () as usize),
                "SetCaretPos" => Some(dlls::user32::SetCaretPos as *const () as usize),
                "GetKeyboardLayoutList" => Some(dlls::user32::GetKeyboardLayoutList as *const () as usize),
                "ActivateKeyboardLayout" => Some(dlls::user32::ActivateKeyboardLayout as *const () as usize),
                "WindowFromDC" => Some(dlls::user32::WindowFromDC as *const () as usize),
                "VkKeyScanExW" => Some(dlls::user32::VkKeyScanExW as *const () as usize),
                "CharUpperW" => Some(dlls::user32::CharUpperW as *const () as usize),
                "CharLowerW" => Some(dlls::user32::CharLowerW as *const () as usize),
                "CharUpperA" => Some(dlls::user32::CharUpperA as *const () as usize),
                "CharLowerA" => Some(dlls::user32::CharLowerA as *const () as usize),
                "CharUpperBuffW" => Some(dlls::user32::CharUpperBuffW as *const () as usize),
                "CharLowerBuffW" => Some(dlls::user32::CharLowerBuffW as *const () as usize),
                "CharUpperBuffA" => Some(dlls::user32::CharUpperBuffA as *const () as usize),
                "CharLowerBuffA" => Some(dlls::user32::CharLowerBuffA as *const () as usize),
                "RegisterClassExW" => Some(dlls::user32::RegisterClassExW as *const () as usize),
                "RegisterClassExA" => Some(dlls::user32::RegisterClassExA as *const () as usize),
                "RegisterClassW" => Some(dlls::user32::RegisterClassW as *const () as usize),
                "RegisterClassA" => Some(dlls::user32::RegisterClassA as *const () as usize),
                "CreateWindowExW" | "CreateWindowW" => {
                    Some(dlls::user32::CreateWindowExW_System as *const () as usize)
                }
                "CreateWindowExA" | "CreateWindowA" => {
                    Some(dlls::user32::CreateWindowExA as *const () as usize)
                }
                "GetMessageW" | "GetMessageA" => {
                    Some(dlls::user32::GetMessageW as *const () as usize)
                }
                "GetMessagePos" => Some(dlls::user32::GetMessagePos as *const () as usize),
                "GetMessageTime" => Some(dlls::user32::GetMessageTime as *const () as usize),
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
                "SetWinEventHook" => Some(dlls::user32::SetWinEventHook as *const () as usize),
                "UnhookWinEvent" => Some(dlls::user32::UnhookWinEvent as *const () as usize),
                "RegisterSuspendResumeNotification" => {
                    Some(dlls::user32::RegisterSuspendResumeNotification as *const () as usize)
                }
                "UnregisterSuspendResumeNotification" => {
                    Some(dlls::user32::UnregisterSuspendResumeNotification as *const () as usize)
                }
                "RegisterPowerSettingNotification" => {
                    Some(dlls::user32::RegisterPowerSettingNotification as *const () as usize)
                }
                "UnregisterPowerSettingNotification" => {
                    Some(dlls::user32::UnregisterPowerSettingNotification as *const () as usize)
                }
                "LoadIconA" => Some(dlls::user32::LoadIconA as *const () as usize),
                "LoadIconW" => Some(dlls::user32::LoadIconW as *const () as usize),
                "LoadCursorA" => Some(dlls::user32::LoadCursorA as *const () as usize),
                "LoadCursorW" => Some(dlls::user32::LoadCursorW as *const () as usize),
                "SetCursor" => Some(dlls::user32::SetCursor as *const () as usize),
                "TrackMouseEvent" | "_TrackMouseEvent" => {
                    Some(dlls::user32::TrackMouseEvent as *const () as usize)
                }
                "GetDC" => Some(dlls::user32::GetDC as *const () as usize),
                "GetDCEx" => Some(dlls::user32::GetDCEx as *const () as usize),
                "GetWindowDC" => Some(dlls::user32::GetWindowDC as *const () as usize),
                "FillRect" => Some(dlls::user32::FillRect as *const () as usize),
                "FlashWindowEx" => Some(dlls::user32::FlashWindowEx as *const () as usize),
                "GetUpdateRect" => Some(dlls::user32::GetUpdateRect as *const () as usize),
                "SetWindowRgn" => Some(dlls::user32::SetWindowRgn as *const () as usize),
                "RegisterClipboardFormatA" => Some(dlls::user32::RegisterClipboardFormatA as *const () as usize),
                "CallWindowProcW" => Some(dlls::user32::CallWindowProcW as *const () as usize),
                "CallWindowProcA" => Some(dlls::user32::CallWindowProcA as *const () as usize),
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
                "GetRawInputDeviceList" => {
                    Some(dlls::user32::GetRawInputDeviceList as *const () as usize)
                }
                "GetRawInputDeviceInfoA" => {
                    Some(dlls::user32::GetRawInputDeviceInfoA as *const () as usize)
                }
                "GetRawInputDeviceInfoW" => {
                    Some(dlls::user32::GetRawInputDeviceInfoW as *const () as usize)
                }
                "GetRawInputData" => Some(dlls::user32::GetRawInputData as *const () as usize),
                "RegisterDeviceNotificationW" => {
                    Some(dlls::user32::RegisterDeviceNotificationW as *const () as usize)
                }
                "RegisterDeviceNotificationA" => {
                    Some(dlls::user32::RegisterDeviceNotificationA as *const () as usize)
                }
                "UnregisterDeviceNotification" => {
                    Some(dlls::user32::UnregisterDeviceNotification as *const () as usize)
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
                "ChangeDisplaySettingsExW" => {
                    Some(dlls::user32::ChangeDisplaySettingsExW as *const () as usize)
                }
                "ChangeDisplaySettingsW" => {
                    Some(dlls::user32::ChangeDisplaySettingsW as *const () as usize)
                }
                "ChangeDisplaySettingsExA" => {
                    Some(dlls::user32::ChangeDisplaySettingsExA as *const () as usize)
                }
                "ChangeDisplaySettingsA" => {
                    Some(dlls::user32::ChangeDisplaySettingsA as *const () as usize)
                }
                "EnumDisplayDevicesA" => {
                    Some(dlls::user32::EnumDisplayDevicesA as *const () as usize)
                }
                "EnumDisplayDevicesW" => {
                    Some(dlls::user32::EnumDisplayDevicesW as *const () as usize)
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
                "IsMouseInPointerEnabled" => {
                    Some(dlls::user32::IsMouseInPointerEnabled as *const () as usize)
                }
                "EnableMouseInPointer" => {
                    Some(dlls::user32::EnableMouseInPointer as *const () as usize)
                }
                "GetPointerDevices" => {
                    Some(dlls::user32::GetPointerDevices as *const () as usize)
                }
                "GetPointerDevice" => {
                    Some(dlls::user32::GetPointerDevice as *const () as usize)
                }
                "GetPointerDeviceProperties" => {
                    Some(dlls::user32::GetPointerDeviceProperties as *const () as usize)
                }
                "GetPointerDeviceRects" => {
                    Some(dlls::user32::GetPointerDeviceRects as *const () as usize)
                }
                "RegisterPointerDeviceNotifications" => {
                    Some(dlls::user32::RegisterPointerDeviceNotifications as *const () as usize)
                }
                "GetPointerInfo" => {
                    Some(dlls::user32::GetPointerInfo as *const () as usize)
                }
                "GetPointerInfoHistory" => {
                    Some(dlls::user32::GetPointerInfoHistory as *const () as usize)
                }
                "GetPointerTouchInfo" => {
                    Some(dlls::user32::GetPointerTouchInfo as *const () as usize)
                }
                "GetPointerFrameTouchInfo" => {
                    Some(dlls::user32::GetPointerFrameTouchInfo as *const () as usize)
                }
                "SetRect" => Some(dlls::user32::SetRect as *const () as usize),
                "PtInRect" => Some(dlls::user32::PtInRect as *const () as usize),
                "ShowCursor" => Some(dlls::user32::ShowCursor as *const () as usize),
                "DestroyCursor" => Some(dlls::user32::DestroyCursor as *const () as usize),
                "AllowSetForegroundWindow" => Some(dlls::user32::AllowSetForegroundWindow as *const () as usize),
                "GetCaretBlinkTime" => Some(dlls::user32::GetCaretBlinkTime as *const () as usize),
                "GetDoubleClickTime" => Some(dlls::user32::GetDoubleClickTime as *const () as usize),
                "RegisterWindowMessageA" => Some(dlls::user32::RegisterWindowMessageA as *const () as usize),
                "DestroyWindow" => Some(dlls::user32::DestroyWindow as *const () as usize),
                "UnregisterClassW" => Some(dlls::user32::UnregisterClassW as *const () as usize),
                "SetTimer" => Some(dlls::user32::SetTimer as *const () as usize),
                "KillTimer" => Some(dlls::user32::KillTimer as *const () as usize),
                "PostQuitMessage" => Some(dlls::user32::PostQuitMessage as *const () as usize),
                "MsgWaitForMultipleObjects" => Some(dlls::user32::MsgWaitForMultipleObjects as *const () as usize),
                "MsgWaitForMultipleObjectsEx" => Some(dlls::user32::MsgWaitForMultipleObjectsEx as *const () as usize),
                "EnumWindows" => Some(dlls::user32::EnumWindows as *const () as usize),
                "LoadImageW" => Some(dlls::user32::LoadImageW as *const () as usize),
                "OpenClipboard" => Some(dlls::user32::OpenClipboard as *const () as usize),
                "CloseClipboard" => Some(dlls::user32::CloseClipboard as *const () as usize),
                "EmptyClipboard" => Some(dlls::user32::EmptyClipboard as *const () as usize),
                "GetClipboardData" => Some(dlls::user32::GetClipboardData as *const () as usize),
                "SetClipboardData" => Some(dlls::user32::SetClipboardData as *const () as usize),
                "IsClipboardFormatAvailable" => Some(dlls::user32::IsClipboardFormatAvailable as *const () as usize),
                "GetKeyboardLayoutNameW" => Some(dlls::user32::GetKeyboardLayoutNameW as *const () as usize),
                "MapVirtualKeyExA" => Some(dlls::user32::MapVirtualKeyExA as *const () as usize),
                "GetKeyNameTextW" => Some(dlls::user32::GetKeyNameTextW as *const () as usize),
                "GetThreadDesktop" => Some(dlls::user32::GetThreadDesktop as *const () as usize),
                "GetUserObjectInformationA" => Some(dlls::user32::GetUserObjectInformationA as *const () as usize),
                "SendMessageTimeoutA" => Some(dlls::user32::SendMessageTimeoutA as *const () as usize),
                "GetRawInputBuffer" => Some(dlls::user32::GetRawInputBuffer as *const () as usize),
                "GetPointerTouchInfoHistory" => Some(dlls::user32::GetPointerTouchInfoHistory as *const () as usize),
                "GetPointerPenInfoHistory" => Some(dlls::user32::GetPointerPenInfoHistory as *const () as usize),
                "GetClientRect" => Some(dlls::user32::GetClientRect as *const () as usize),
                "GetWindowRect" => Some(dlls::user32::GetWindowRect as *const () as usize),
                "GetWindow" => Some(dlls::user32::GetWindow as *const () as usize),
                "AdjustWindowRectEx" => {
                    Some(dlls::user32::AdjustWindowRectEx as *const () as usize)
                }
                "AdjustWindowRect" => Some(dlls::user32::AdjustWindowRect as *const () as usize),
                "AdjustWindowRectExForDpi" => {
                    Some(dlls::user32::AdjustWindowRectExForDpi as *const () as usize)
                }
                "SystemParametersInfoW" | "SystemParametersInfo" => {
                    Some(dlls::user32::SystemParametersInfoW as *const () as usize)
                }
                "SystemParametersInfoA" => {
                    Some(dlls::user32::SystemParametersInfoA as *const () as usize)
                }
                "SystemParametersInfoForDpi" => {
                    Some(dlls::user32::SystemParametersInfoForDpi as *const () as usize)
                }
                "EnableNonClientDpiScaling" => {
                    Some(dlls::user32::EnableNonClientDpiScaling as *const () as usize)
                }
                "SetWindowPos" => Some(dlls::user32::SetWindowPos as *const () as usize),
                "MoveWindow" => Some(dlls::user32::MoveWindow as *const () as usize),
                "ShowWindow" => Some(dlls::user32::ShowWindow as *const () as usize),
                "UpdateWindow" => Some(dlls::user32::UpdateWindow as *const () as usize),
                "ValidateRect" => Some(dlls::user32::ValidateRect as *const () as usize),
                "InvalidateRect" => Some(dlls::user32::InvalidateRect as *const () as usize),
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
                "DialogBoxParamW" => Some(dlls::user32::DialogBoxParamW as *const () as usize),
                "DialogBoxParamA" => Some(dlls::user32::DialogBoxParamA as *const () as usize),
                "EndDialog" => Some(dlls::user32::EndDialog as *const () as usize),
                "GetDlgItem" => Some(dlls::user32::GetDlgItem as *const () as usize),
                "SetDlgItemTextW" => Some(dlls::user32::SetDlgItemTextW as *const () as usize),
                "SetDlgItemTextA" => Some(dlls::user32::SetDlgItemTextA as *const () as usize),
                "CreateDialogParamW" => {
                    Some(dlls::user32::CreateDialogParamW as *const () as usize)
                }
                "CreateDialogParamA" => {
                    Some(dlls::user32::CreateDialogParamA as *const () as usize)
                }
                "GetParent" => Some(dlls::user32::GetParent as *const () as usize),
                "GetDesktopWindow" => Some(dlls::user32::GetDesktopWindow as *const () as usize),
                "OffsetRect" => Some(dlls::user32::OffsetRect as *const () as usize),
                "CopyRect" => Some(dlls::user32::CopyRect as *const () as usize),
                "SendDlgItemMessageW" => {
                    Some(dlls::user32::SendDlgItemMessageW as *const () as usize)
                }
                "GetDisplayConfigBufferSizes" => {
                    Some(dlls::user32::GetDisplayConfigBufferSizes as *const () as usize)
                }
                "QueryDisplayConfig" => {
                    Some(dlls::user32::QueryDisplayConfig as *const () as usize)
                }
                "DisplayConfigGetDeviceInfo" => {
                    Some(dlls::user32::DisplayConfigGetDeviceInfo as *const () as usize)
                }
                "wsprintfA" => Some(dlls::user32::wsprintfA as *const () as usize),
                "wsprintfW" => Some(dlls::user32::wsprintfW as *const () as usize),
                "WaitForInputIdle" => Some(dlls::user32::WaitForInputIdle as *const () as usize),
                _ => None,
            },
            "GDI32.DLL" | "OPENGL32.DLL" => match func_name {
                "CreateRectRgn" => Some(dlls::gdi32::CreateRectRgn as *const () as usize),
                "CreateRoundRectRgn" => Some(dlls::gdi32::CreateRoundRectRgn as *const () as usize),
                "PtInRegion" => Some(dlls::gdi32::PtInRegion as *const () as usize),
                "GetTextExtentPoint32W" => Some(dlls::gdi32::GetTextExtentPoint32W as *const () as usize),
                "SetBkColor" => Some(dlls::gdi32::SetBkColor as *const () as usize),
                "SetBkMode" => Some(dlls::gdi32::SetBkMode as *const () as usize),
                "SetTextAlign" => Some(dlls::gdi32::SetTextAlign as *const () as usize),
                "SetTextColor" => Some(dlls::gdi32::SetTextColor as *const () as usize),
                "TextOutW" => Some(dlls::gdi32::TextOutW as *const () as usize),
                "GetDC" => Some(dlls::gdi32::GetDC as *const () as usize),
                "ReleaseDC" => Some(dlls::gdi32::ReleaseDC as *const () as usize),
                "GetBitmapBits" => Some(dlls::gdi32::GetBitmapBits as *const () as usize),
                "GetObjectA" => Some(dlls::gdi32::GetObjectA as *const () as usize),
                "DeleteDC" => Some(dlls::gdi32::DeleteDC as *const () as usize),
                "CreateCompatibleDC" => Some(dlls::gdi32::CreateCompatibleDC as *const () as usize),
                "CreateCompatibleBitmap" => {
                    Some(dlls::gdi32::CreateCompatibleBitmap as *const () as usize)
                }
                "CreateBitmap" => Some(dlls::gdi32::CreateBitmap as *const () as usize),
                "SelectObject" => Some(dlls::gdi32::SelectObject as *const () as usize),
                "DeleteObject" => Some(dlls::gdi32::DeleteObject as *const () as usize),
                "GetStockObject" => Some(dlls::gdi32::GetStockObject as *const () as usize),
                "CreateSolidBrush" => Some(dlls::gdi32::CreateSolidBrush as *const () as usize),
                "CreateFontIndirectW" => {
                    Some(dlls::gdi32::CreateFontIndirectW as *const () as usize)
                }
                "GetObjectW" => Some(dlls::gdi32::GetObjectW as *const () as usize),
                "CreateDCW" => Some(dlls::gdi32::CreateDCW as *const () as usize),
                "CreateDCA" => Some(dlls::gdi32::CreateDCA as *const () as usize),
                "CreateICW" => Some(dlls::gdi32::CreateICW as *const () as usize),
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
                "D3DKMTEscape" => Some(dlls::gdi32::D3DKMTEscape as *const () as usize),
                "D3DKMTCreateDevice" => Some(dlls::gdi32::D3DKMTCreateDevice as *const () as usize),
                "D3DKMTOpenAdapterFromHdc" => Some(dlls::gdi32::D3DKMTOpenAdapterFromHdc as *const () as usize),
                "D3DKMTOpenAdapterFromGdiDisplayName" => Some(dlls::gdi32::D3DKMTOpenAdapterFromGdiDisplayName as *const () as usize),
                "D3DKMTOpenAdapterFromLuid" => Some(dlls::gdi32::D3DKMTOpenAdapterFromLuid as *const () as usize),
                "D3DKMTCloseAdapter" => Some(dlls::gdi32::D3DKMTCloseAdapter as *const () as usize),
                "D3DKMTDestroyDevice" => Some(dlls::gdi32::D3DKMTDestroyDevice as *const () as usize),
                "D3DKMTQueryAdapterInfo" => Some(dlls::gdi32::D3DKMTQueryAdapterInfo as *const () as usize),
                "D3DKMTQueryStatistics" => Some(dlls::gdi32::D3DKMTQueryStatistics as *const () as usize),
                "D3DKMTCheckVidPnExclusiveOwnership" => Some(dlls::gdi32::D3DKMTCheckVidPnExclusiveOwnership as *const () as usize),
                "D3DKMTSetVidPnSourceOwner" => Some(dlls::gdi32::D3DKMTSetVidPnSourceOwner as *const () as usize),
                func if func.starts_with("D3DKMT") => Some(dlls::gdi32::D3DKMTEscape as *const () as usize),
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
                    "CreateDXGIFactory" => {
                        Some(dlls::dxgi::CreateDXGIFactory as *const () as usize)
                    }
                    "CreateDXGIFactory1" => {
                        Some(dlls::dxgi::CreateDXGIFactory1 as *const () as usize)
                    }
                    "CreateDXGIFactory2" => {
                        Some(dlls::dxgi::CreateDXGIFactory2 as *const () as usize)
                    }
                    "EnumAdapters" => Some(dlls::dxgi::EnumAdapters as *const () as usize),
                    "EnumAdapters1" => Some(dlls::dxgi::EnumAdapters1 as *const () as usize),
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
                "timeGetDevCaps" => Some(dlls::winmm::timeGetDevCaps as *const () as usize),
                "timeSetEvent" => Some(dlls::winmm::timeSetEvent as *const () as usize),
                "timeKillEvent" => Some(dlls::winmm::timeKillEvent as *const () as usize),
                "waveOutGetNumDevs" => Some(dlls::winmm::waveOutGetNumDevs as *const () as usize),
                "waveInGetNumDevs" => Some(dlls::winmm::waveInGetNumDevs as *const () as usize),
                "waveOutGetDevCapsW" => Some(dlls::winmm::waveOutGetDevCapsW as *const () as usize),
                "waveOutGetDevCapsA" => Some(dlls::winmm::waveOutGetDevCapsA as *const () as usize),
                "waveInGetDevCapsW" => Some(dlls::winmm::waveInGetDevCapsW as *const () as usize),
                "waveInGetDevCapsA" => Some(dlls::winmm::waveInGetDevCapsA as *const () as usize),
                "waveOutGetPosition" => Some(dlls::winmm::waveOutGetPosition as *const () as usize),
                "waveInOpen" => Some(dlls::winmm::waveInOpen as *const () as usize),
                "waveInClose" => Some(dlls::winmm::waveInClose as *const () as usize),
                "waveInStart" => Some(dlls::winmm::waveInStart as *const () as usize),
                "waveInReset" => Some(dlls::winmm::waveInReset as *const () as usize),
                "waveInPrepareHeader" => Some(dlls::winmm::waveInPrepareHeader as *const () as usize),
                "waveInUnprepareHeader" => Some(dlls::winmm::waveInUnprepareHeader as *const () as usize),
                "waveInAddBuffer" => Some(dlls::winmm::waveInAddBuffer as *const () as usize),
                _ => None,
            },
            dll if dll.starts_with("XINPUT") => match func_name {
                "XInputGetState" => Some(crate::dlls::xinput::XInputGetState as *const () as usize),
                "XInputSetState" => Some(crate::dlls::xinput::XInputSetState as *const () as usize),
                "XInputGetCapabilities" => Some(crate::dlls::xinput::XInputGetCapabilities as *const () as usize),
                "XInputEnable" => Some(crate::dlls::xinput::XInputEnable as *const () as usize),
                "XInputGetBatteryInformation" => Some(crate::dlls::xinput::XInputGetBatteryInformation as *const () as usize),
                "XInputGetKeystroke" => Some(crate::dlls::xinput::XInputGetKeystroke as *const () as usize),
                "XInputGetAudioDeviceIds" => Some(crate::dlls::xinput::XInputGetAudioDeviceIds as *const () as usize),
                "XInputGetDSoundAudioDeviceGuids" => Some(crate::dlls::xinput::XInputGetDSoundAudioDeviceGuids as *const () as usize),
                "XInputGetStateEx" => Some(crate::dlls::xinput::XInputGetStateEx as *const () as usize),
                "XInputWaitForGuideButton" => Some(crate::dlls::xinput::XInputWaitForGuideButton as *const () as usize),
                "XInputCancelGuideButtonWait" => Some(crate::dlls::xinput::XInputCancelGuideButtonWait as *const () as usize),
                "XInputPowerOffController" => Some(crate::dlls::xinput::XInputPowerOffController as *const () as usize),
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
                "CertFreeCertificateChain" => {
                    Some(dlls::crypt32::CertFreeCertificateChain as *const () as usize)
                }
                "CertVerifyCertificateChainPolicy" => {
                    Some(dlls::crypt32::CertVerifyCertificateChainPolicy as *const () as usize)
                }
                "CertOpenStore" => Some(dlls::crypt32::CertOpenStore as *const () as usize),
                "CertAddEncodedCertificateToStore" => {
                    Some(dlls::crypt32::CertAddEncodedCertificateToStore as *const () as usize)
                }
                "CertGetCertificateChain" => {
                    Some(dlls::crypt32::CertGetCertificateChain as *const () as usize)
                }
                "CertGetCertificateContextProperty" => {
                    Some(dlls::crypt32::CertGetCertificateContextProperty as *const () as usize)
                }
                "CryptBinaryToStringA" => {
                    Some(dlls::crypt32::CryptBinaryToStringA as *const () as usize)
                }
                "CryptBinaryToStringW" => {
                    Some(dlls::crypt32::CryptBinaryToStringW as *const () as usize)
                }
                "CryptProtectData" => {
                    Some(dlls::crypt32::CryptProtectData as *const () as usize)
                }
                "CryptUnprotectData" => {
                    Some(dlls::crypt32::CryptUnprotectData as *const () as usize)
                }
                _ => None,
            },
            dll if dll.starts_with("CFGMGR32") => crate::dlls::cfgmgr32::resolve_cfgmgr32_symbol(func_name),
            dll if dll.starts_with("PSAPI") => match func_name {
                "GetModuleInformation" => Some(dlls::kernel32::GetModuleInformation as *const () as usize),
                "GetModuleBaseNameA" => Some(dlls::kernel32::GetModuleBaseNameA as *const () as usize),
                "GetModuleBaseNameW" => Some(dlls::kernel32::GetModuleBaseNameW as *const () as usize),
                "GetModuleFileNameExA" => Some(dlls::kernel32::GetModuleFileNameExA as *const () as usize),
                "GetModuleFileNameExW" => Some(dlls::kernel32::GetModuleFileNameExW as *const () as usize),
                "EnumProcessModules" => Some(dlls::kernel32::EnumProcessModules as *const () as usize),
                "EnumProcessModulesEx" => Some(dlls::kernel32::EnumProcessModulesEx as *const () as usize),
                "EnumProcesses" => Some(dlls::kernel32::EnumProcesses as *const () as usize),
                "GetProcessMemoryInfo" => Some(dlls::kernel32::K32GetProcessMemoryInfo as *const () as usize),
                "K32EnumProcessModules" => Some(dlls::kernel32::K32EnumProcessModules as *const () as usize),
                "K32EnumProcessModulesEx" => Some(dlls::kernel32::K32EnumProcessModulesEx as *const () as usize),
                "K32EnumProcesses" => Some(dlls::kernel32::K32EnumProcesses as *const () as usize),
                "K32GetModuleBaseNameA" => Some(dlls::kernel32::K32GetModuleBaseNameA as *const () as usize),
                "K32GetModuleBaseNameW" => Some(dlls::kernel32::K32GetModuleBaseNameW as *const () as usize),
                "K32GetModuleFileNameExA" => Some(dlls::kernel32::K32GetModuleFileNameExA as *const () as usize),
                "K32GetModuleFileNameExW" => Some(dlls::kernel32::K32GetModuleFileNameExW as *const () as usize),
                "K32GetModuleInformation" => Some(dlls::kernel32::K32GetModuleInformation as *const () as usize),
                "K32GetProcessMemoryInfo" => Some(dlls::kernel32::K32GetProcessMemoryInfo as *const () as usize),
                _ => None,
            },
            "WS2_32.DLL" | "WSOCK32.DLL" | "WS2_32" | "WSOCK32" => match func_name {
                "WSAStartup" => Some(dlls::ws2_32::WSAStartup as *const () as usize),
                "GetHostNameW" => Some(dlls::ws2_32::GetHostNameW as *const () as usize),
                "WSALookupServiceBeginW" => {
                    Some(dlls::ws2_32::WSALookupServiceBeginW as *const () as usize)
                }
                "WSALookupServiceBeginA" => {
                    Some(dlls::ws2_32::WSALookupServiceBeginA as *const () as usize)
                }
                "WSALookupServiceNextW" => {
                    Some(dlls::ws2_32::WSALookupServiceNextW as *const () as usize)
                }
                "WSALookupServiceNextA" => {
                    Some(dlls::ws2_32::WSALookupServiceNextA as *const () as usize)
                }
                "WSALookupServiceEnd" => {
                    Some(dlls::ws2_32::WSALookupServiceEnd as *const () as usize)
                }
                "WSANSPIoctl" => Some(dlls::ws2_32::WSANSPIoctl as *const () as usize),
                "WSACleanup" => Some(dlls::ws2_32::WSACleanup as *const () as usize),
                "WSAGetLastError" => Some(dlls::ws2_32::WSAGetLastError as *const () as usize),
                "WSASetLastError" => Some(dlls::ws2_32::WSASetLastError as *const () as usize),
                "socket" => Some(dlls::ws2_32::socket as *const () as usize),
                "closesocket" => Some(dlls::ws2_32::closesocket as *const () as usize),
                "bind" => Some(dlls::ws2_32::bind as *const () as usize),
                "connect" => Some(dlls::ws2_32::connect as *const () as usize),
                "listen" => Some(dlls::ws2_32::listen as *const () as usize),
                "accept" => Some(dlls::ws2_32::accept as *const () as usize),
                "send" => Some(dlls::ws2_32::send as *const () as usize),
                "recv" => Some(dlls::ws2_32::recv as *const () as usize),
                "sendto" => Some(dlls::ws2_32::sendto as *const () as usize),
                "recvfrom" => Some(dlls::ws2_32::recvfrom as *const () as usize),
                "shutdown" => Some(dlls::ws2_32::shutdown as *const () as usize),
                "htons" => Some(dlls::ws2_32::htons as *const () as usize),
                "ntohs" => Some(dlls::ws2_32::ntohs as *const () as usize),
                "htonl" => Some(dlls::ws2_32::htonl as *const () as usize),
                "ntohl" => Some(dlls::ws2_32::ntohl as *const () as usize),
                "inet_addr" => Some(dlls::ws2_32::inet_addr as *const () as usize),
                "inet_ntoa" => Some(dlls::ws2_32::inet_ntoa as *const () as usize),
                "ioctlsocket" => Some(dlls::ws2_32::ioctlsocket as *const () as usize),
                "setsockopt" => Some(dlls::ws2_32::setsockopt as *const () as usize),
                "getsockopt" => Some(dlls::ws2_32::getsockopt as *const () as usize),
                "gethostname" => Some(dlls::ws2_32::gethostname as *const () as usize),
                "__WSAFDIsSet" => Some(dlls::ws2_32::__WSAFDIsSet as *const () as usize),
                "WSACreateEvent" => Some(dlls::ws2_32::WSACreateEvent as *const () as usize),
                "WSACloseEvent" => Some(dlls::ws2_32::WSACloseEvent as *const () as usize),
                "WSASetEvent" => Some(dlls::ws2_32::WSASetEvent as *const () as usize),
                "WSAResetEvent" => Some(dlls::ws2_32::WSAResetEvent as *const () as usize),
                "WSAWaitForMultipleEvents" => {
                    Some(dlls::ws2_32::WSAWaitForMultipleEvents as *const () as usize)
                }
                "WSASocketA" => Some(dlls::ws2_32::WSASocketA as *const () as usize),
                "WSASocketW" => Some(dlls::ws2_32::WSASocketW as *const () as usize),
                "WSAEventSelect" => Some(dlls::ws2_32::WSAEventSelect as *const () as usize),
                "WSAEnumNetworkEvents" => {
                    Some(dlls::ws2_32::WSAEnumNetworkEvents as *const () as usize)
                }
                "WSAIoctl" => Some(dlls::ws2_32::WSAIoctl as *const () as usize),
                "WSASend" => Some(dlls::ws2_32::WSASend as *const () as usize),
                "WSARecv" => Some(dlls::ws2_32::WSARecv as *const () as usize),
                "WSASendDisconnect" => {
                    Some(dlls::ws2_32::WSASendDisconnect as *const () as usize)
                }
                "select" => Some(dlls::ws2_32::select as *const () as usize),
                "getpeername" => Some(dlls::ws2_32::getpeername as *const () as usize),
                "getsockname" => Some(dlls::ws2_32::getsockname as *const () as usize),
                "inet_ntop" => Some(dlls::ws2_32::inet_ntop as *const () as usize),
                "inet_pton" => Some(dlls::ws2_32::inet_pton as *const () as usize),
                "getaddrinfo" => Some(dlls::ws2_32::getaddrinfo as *const () as usize),
                "freeaddrinfo" => Some(dlls::ws2_32::freeaddrinfo as *const () as usize),
                "GetAddrInfoW" => Some(dlls::ws2_32::GetAddrInfoW as *const () as usize),
                "FreeAddrInfoW" => Some(dlls::ws2_32::FreeAddrInfoW as *const () as usize),
                "GetAddrInfoExW" => Some(dlls::ws2_32::GetAddrInfoExW as *const () as usize),
                "FreeAddrInfoExW" => Some(dlls::ws2_32::FreeAddrInfoExW as *const () as usize),
                "GetAddrInfoExCancel" => {
                    Some(dlls::ws2_32::GetAddrInfoExCancel as *const () as usize)
                }
                "getnameinfo" => Some(dlls::ws2_32::getnameinfo as *const () as usize),
                "GetNameInfoW" => Some(dlls::ws2_32::GetNameInfoW as *const () as usize),
                "InetNtopW" => Some(dlls::ws2_32::InetNtopW as *const () as usize),
                "InetPtonW" => Some(dlls::ws2_32::InetPtonW as *const () as usize),
                "gethostbyname" => Some(dlls::ws2_32::gethostbyname as *const () as usize),
                "gethostbyaddr" => Some(dlls::ws2_32::gethostbyaddr as *const () as usize),
                "getprotobyname" => Some(dlls::ws2_32::getprotobyname as *const () as usize),
                "getprotobynumber" => {
                    Some(dlls::ws2_32::getprotobynumber as *const () as usize)
                }
                "getservbyname" => Some(dlls::ws2_32::getservbyname as *const () as usize),
                "getservbyport" => Some(dlls::ws2_32::getservbyport as *const () as usize),
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
                "NtSetInformationThread" | "ZwSetInformationThread" => {
                    Some(dlls::ntdll::NtSetInformationThread as *const () as usize)
                }
                "NtQueryInformationThread" | "ZwQueryInformationThread" => {
                    Some(dlls::ntdll::NtQueryInformationThread as *const () as usize)
                }
                "NtQueryInformationProcess" | "ZwQueryInformationProcess" => {
                    Some(dlls::ntdll::NtQueryInformationProcess as *const () as usize)
                }
                "NtQueryVirtualMemory" | "ZwQueryVirtualMemory" => {
                    Some(dlls::ntdll::NtQueryVirtualMemory as *const () as usize)
                }
                "LdrGetDllHandle" => {
                    Some(dlls::ntdll::LdrGetDllHandle as *const () as usize)
                }
                "RtlImageNtHeader" => {
                    Some(dlls::ntdll::RtlImageNtHeader as *const () as usize)
                }
                "RtlFindExportedRoutineByName" => {
                    Some(dlls::ntdll::RtlFindExportedRoutineByName as *const () as usize)
                }
                "RtlLookupFunctionEntry" => {
                    Some(dlls::ntdll::RtlLookupFunctionEntry as *const () as usize)
                }
                "RtlLookupFunctionTable" => {
                    Some(dlls::ntdll::RtlLookupFunctionTable as *const () as usize)
                }
                "RtlAddGrowableFunctionTable" => {
                    Some(dlls::ntdll::RtlAddGrowableFunctionTable as *const () as usize)
                }
                "RtlGrowFunctionTable" => {
                    Some(dlls::ntdll::RtlGrowFunctionTable as *const () as usize)
                }
                "RtlDeleteGrowableFunctionTable" => {
                    Some(dlls::ntdll::RtlDeleteGrowableFunctionTable as *const () as usize)
                }
                "RtlAddFunctionTable" => {
                    Some(dlls::ntdll::RtlAddFunctionTable as *const () as usize)
                }
                "RtlDeleteFunctionTable" => {
                    Some(dlls::ntdll::RtlDeleteFunctionTable as *const () as usize)
                }
                "RtlInstallFunctionTableCallback" => {
                    Some(dlls::ntdll::RtlInstallFunctionTableCallback as *const () as usize)
                }
                "RtlVirtualUnwind" => Some(dlls::ntdll::RtlVirtualUnwind as *const () as usize),
                "RtlAllocateHeap" => Some(dlls::ntdll::RtlAllocateHeap as *const () as usize),
                "RtlFreeHeap" => Some(dlls::ntdll::RtlFreeHeap as *const () as usize),
                "RtlReAllocateHeap" => Some(dlls::ntdll::RtlReAllocateHeap as *const () as usize),
                "RtlSizeHeap" => Some(dlls::ntdll::RtlSizeHeap as *const () as usize),
                "RtlCreateHeap" => Some(dlls::ntdll::RtlCreateHeap as *const () as usize),
                "RtlDestroyHeap" => Some(dlls::ntdll::RtlDestroyHeap as *const () as usize),
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
                "RtlInitializeConditionVariable" => {
                    Some(dlls::ntdll::RtlInitializeConditionVariable as *const () as usize)
                }
                "RtlWakeConditionVariable" => {
                    Some(dlls::ntdll::RtlWakeConditionVariable as *const () as usize)
                }
                "RtlWakeAllConditionVariable" => {
                    Some(dlls::ntdll::RtlWakeAllConditionVariable as *const () as usize)
                }
                "RtlSleepConditionVariableCS" => {
                    Some(dlls::ntdll::RtlSleepConditionVariableCS as *const () as usize)
                }
                "RtlSleepConditionVariableSRW" => {
                    Some(dlls::ntdll::RtlSleepConditionVariableSRW as *const () as usize)
                }
                "RtlEncodePointer" => Some(dlls::ntdll::RtlEncodePointer as *const () as usize),
                "RtlDecodePointer" => Some(dlls::ntdll::RtlDecodePointer as *const () as usize),
                "RtlEncodeSystemPointer" => {
                    Some(dlls::ntdll::RtlEncodeSystemPointer as *const () as usize)
                }
                "RtlDecodeSystemPointer" => {
                    Some(dlls::ntdll::RtlDecodeSystemPointer as *const () as usize)
                }
                "RtlWaitOnAddress" => Some(dlls::ntdll::RtlWaitOnAddress as *const () as usize),
                "RtlWakeAddressSingle" => {
                    Some(dlls::ntdll::RtlWakeAddressSingle as *const () as usize)
                }
                "RtlWakeAddressAll" => {
                    Some(dlls::ntdll::RtlWakeAddressAll as *const () as usize)
                }
                "RtlGetVersion" => Some(dlls::ntdll::RtlGetVersion as *const () as usize),
                "RtlDllShutdownInProgress" => Some(dlls::ntdll::RtlDllShutdownInProgress as *const () as usize),
                "RtlVerifyVersionInfo" => Some(dlls::ntdll::RtlVerifyVersionInfo as *const () as usize),
                "RtlCaptureStackBackTrace" => {
                    Some(dlls::kernel32::RtlCaptureStackBackTrace as *const () as usize)
                }
                "NtDeviceIoControlFile" | "ZwDeviceIoControlFile" => {
                    Some(dlls::ntdll::NtDeviceIoControlFile as *const () as usize)
                }
                "NtQueryVolumeInformationFile" | "ZwQueryVolumeInformationFile" => {
                    Some(dlls::ntdll::NtQueryVolumeInformationFile as *const () as usize)
                }
                "NtQuerySystemInformation" | "ZwQuerySystemInformation" => {
                    Some(dlls::ntdll::NtQuerySystemInformation as *const () as usize)
                }
                "NtQueryDirectoryFile" | "ZwQueryDirectoryFile" => {
                    Some(dlls::ntdll::NtQueryDirectoryFile as *const () as usize)
                }
                "NtQueryDirectoryFileEx" | "ZwQueryDirectoryFileEx" => {
                    Some(dlls::ntdll::NtQueryDirectoryFileEx as *const () as usize)
                }
                "NtQueryInformationFile" | "ZwQueryInformationFile" => {
                    Some(dlls::ntdll::NtQueryInformationFile as *const () as usize)
                }
                "NtSetInformationFile" | "ZwSetInformationFile" => {
                    Some(dlls::ntdll::NtSetInformationFile as *const () as usize)
                }
                "RtlNtStatusToDosError" | "RtlNtStatusToDosErrorNoTeb" => {
                    Some(dlls::ntdll::RtlNtStatusToDosError as *const () as usize)
                }
                "__wine_dbg_output" => Some(dlls::ntdll::__wine_dbg_output as *const () as usize),
                _ => None,
            },
            dll if dll.starts_with("COMCTL32") => match func_name {
                "InitCommonControls" => {
                    Some(dlls::user32::InitCommonControls as *const () as usize)
                }
                "InitCommonControlsEx" => {
                    Some(dlls::user32::InitCommonControlsEx as *const () as usize)
                }
                "SetWindowSubclass" => {
                    Some(dlls::user32::SetWindowSubclass as *const () as usize)
                }
                "RemoveWindowSubclass" => {
                    Some(dlls::user32::RemoveWindowSubclass as *const () as usize)
                }
                "DefSubclassProc" => Some(dlls::user32::DefSubclassProc as *const () as usize),
                "TaskDialog" => Some(dlls::user32::TaskDialog as *const () as usize),
                "TaskDialogIndirect" => {
                    Some(dlls::user32::TaskDialogIndirect as *const () as usize)
                }
                _ => None,
            },
            dll if dll.starts_with("SHCORE") || dll.starts_with("API-MS-WIN-SHCORE-") => match func_name {
                "SetProcessDpiAwareness" => {
                    Some(dlls::user32::SetProcessDpiAwareness as *const () as usize)
                }
                "GetDpiForMonitor" => Some(dlls::user32::GetDpiForMonitor as *const () as usize),
                "GetProcessDpiAwareness" => Some(dlls::user32::GetProcessDpiAwareness as *const () as usize),
                _ => None,
            },
            "MSVCP140.DLL" | "MSVCP140_1.DLL" | "MSVCP140_2.DLL" => match func_name {
                "_Mtx_init_in_situ" => Some(dlls::crt::_Mtx_init_in_situ as *const () as usize),
                "_Mtx_init" => Some(dlls::crt::_Mtx_init as *const () as usize),
                "_Mtx_destroy_in_situ" => {
                    Some(dlls::crt::_Mtx_destroy_in_situ as *const () as usize)
                }
                "_Mtx_destroy" => Some(dlls::crt::_Mtx_destroy as *const () as usize),
                "_Mtx_lock" => Some(dlls::crt::_Mtx_lock as *const () as usize),
                "_Mtx_trylock" => Some(dlls::crt::_Mtx_trylock as *const () as usize),
                "_Mtx_unlock" => Some(dlls::crt::_Mtx_unlock as *const () as usize),
                "_Mtx_current_owns" => Some(dlls::crt::_Mtx_current_owns as *const () as usize),
                "_Mtx_clear_owner" => Some(dlls::crt::_Mtx_clear_owner as *const () as usize),
                "_Mtx_reset_owner" => Some(dlls::crt::_Mtx_reset_owner as *const () as usize),
                "_Cnd_init_in_situ" => Some(dlls::crt::_Cnd_init_in_situ as *const () as usize),
                "_Cnd_init" => Some(dlls::crt::_Cnd_init as *const () as usize),
                "_Cnd_destroy_in_situ" => {
                    Some(dlls::crt::_Cnd_destroy_in_situ as *const () as usize)
                }
                "_Cnd_destroy" => Some(dlls::crt::_Cnd_destroy as *const () as usize),
                "_Cnd_wait" => Some(dlls::crt::_Cnd_wait as *const () as usize),
                "_Cnd_signal" => Some(dlls::crt::_Cnd_signal as *const () as usize),
                "_Cnd_broadcast" => Some(dlls::crt::_Cnd_broadcast as *const () as usize),
                "_Cnd_do_broadcast_at_thread_exit" => {
                    Some(dlls::crt::_Cnd_do_broadcast_at_thread_exit as *const () as usize)
                }
                "_Thrd_id" => Some(dlls::crt::_Thrd_id as *const () as usize),
                "_Thrd_yield" => Some(dlls::crt::_Thrd_yield as *const () as usize),
                "_Thrd_start" => Some(dlls::crt::_Thrd_start as *const () as usize),
                "_Thrd_join" => Some(dlls::crt::_Thrd_join as *const () as usize),
                "??0_Lockit@std@@QEAA@H@Z" => {
                    Some(dlls::crt::msvcp_lockit_ctor as *const () as usize)
                }
                "??1_Lockit@std@@QEAA@XZ" => {
                    Some(dlls::crt::msvcp_lockit_dtor as *const () as usize)
                }
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
                "_aligned_msize" => Some(dlls::crt::msvcrt_aligned_msize as *const () as usize),
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
                "_wsetlocale" | "_o__wsetlocale" => {
                    Some(dlls::crt::msvcrt_wsetlocale as *const () as usize)
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
                "_open_osfhandle" | "_o__open_osfhandle" => {
                    Some(dlls::crt::msvcrt_open_osfhandle as *const () as usize)
                }
                "_get_osfhandle" | "_o__get_osfhandle" => {
                    Some(dlls::crt::msvcrt_get_osfhandle as *const () as usize)
                }
                "_fdopen" | "fdopen" | "_o__fdopen" => {
                    Some(dlls::crt::msvcrt_fdopen as *const () as usize)
                }
                "_wfdopen" | "_o__wfdopen" => {
                    Some(dlls::crt::msvcrt_wfdopen as *const () as usize)
                }
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
                "_write" | "_o__write" => Some(dlls::crt::msvcrt_write as *const () as usize),
                "_read" | "_o__read" => Some(dlls::crt::msvcrt_read as *const () as usize),
                "_close" | "_o__close" => Some(dlls::crt::msvcrt_close as *const () as usize),
                "_o__fileno" => Some(dlls::crt::msvcrt_fileno as *const () as usize),
                "setvbuf" | "_o_setvbuf" => Some(dlls::crt::msvcrt_setvbuf as *const () as usize),
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
                "longjmp" | "_longjmpex" => Some(dlls::crt::msvcrt_longjmp as *const () as usize),
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
                "vfprintf" => Some(dlls::crt::vfprintf as *const () as usize),
                "__stdio_common_vfprintf"
                | "__stdio_common_vfprintf_s"
                | "__stdio_common_vfprintf_p" => {
                    Some(dlls::crt::__stdio_common_vfprintf as *const () as usize)
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
                "__stdio_common_vswprintf"
                | "__stdio_common_vswprintf_s"
                | "__stdio_common_vsnwprintf_s" => {
                    Some(dlls::crt::__stdio_common_vswprintf as *const () as usize)
                }
                "_vsnwprintf_s" | "_vsnwprintf_s_l" => {
                    Some(dlls::crt::_vsnwprintf_s as *const () as usize)
                }
                "_vsnwprintf" | "_vsnwprintf_l" => {
                    Some(dlls::crt::_vsnwprintf as *const () as usize)
                }
                "vswprintf_s" => Some(dlls::crt::vswprintf_s as *const () as usize),
                "EncodePointer" | "_encode_pointer" => {
                    Some(dlls::kernel32::EncodePointer as *const () as usize)
                }
                "DecodePointer" | "_decode_pointer" => {
                    Some(dlls::kernel32::DecodePointer as *const () as usize)
                }
                "iswalpha" => Some(dlls::crt::crt_iswalpha as *const () as usize),
                "iswalnum" => Some(dlls::crt::crt_iswalnum as *const () as usize),
                "iswdigit" => Some(dlls::crt::crt_iswdigit as *const () as usize),
                "iswlower" => Some(dlls::crt::crt_iswlower as *const () as usize),
                "iswupper" => Some(dlls::crt::crt_iswupper as *const () as usize),
                "iswspace" => Some(dlls::crt::crt_iswspace as *const () as usize),
                "iswxdigit" => Some(dlls::crt::crt_iswxdigit as *const () as usize),
                "iswprint" => Some(dlls::crt::crt_iswprint as *const () as usize),
                "iswgraph" => Some(dlls::crt::crt_iswgraph as *const () as usize),
                "iswctype" => Some(dlls::crt::crt_iswctype as *const () as usize),
                "isalnum" => Some(dlls::crt::crt_isalnum as *const () as usize),
                "islower" => Some(dlls::crt::crt_islower as *const () as usize),
                "isupper" => Some(dlls::crt::crt_isupper as *const () as usize),
                "isxdigit" => Some(dlls::crt::crt_isxdigit as *const () as usize),
                "isprint" => Some(dlls::crt::crt_isprint as *const () as usize),
                "isgraph" => Some(dlls::crt::crt_isgraph as *const () as usize),
                "_wcsnicmp" => Some(dlls::crt::crt_wcsnicmp as *const () as usize),
                "wcsstr" => Some(dlls::crt::crt_wcsstr as *const () as usize),
                "wcstod" => Some(dlls::crt::crt_wcstod as *const () as usize),
                "_wcstoi64" => Some(dlls::crt::crt_wcstoi64 as *const () as usize),
                "_wcstoui64" => Some(dlls::crt::crt_wcstoui64 as *const () as usize),
                "wcstoul" => Some(dlls::crt::crt_wcstoul as *const () as usize),
                "_wtof" => Some(dlls::crt::crt_wtof as *const () as usize),
                "_wtoi" => Some(dlls::crt::crt_wtoi as *const () as usize),
                "_wtoi64" => Some(dlls::crt::crt_wtoi64 as *const () as usize),
                "strnlen" => Some(dlls::crt::crt_strnlen as *const () as usize),
                "acos" => Some(dlls::crt::crt_acos as *const () as usize),
                "acosf" => Some(dlls::crt::crt_acosf as *const () as usize),
                "asin" => Some(dlls::crt::crt_asin as *const () as usize),
                "asinf" => Some(dlls::crt::crt_asinf as *const () as usize),
                "atan" => Some(dlls::crt::crt_atan as *const () as usize),
                "atanf" => Some(dlls::crt::crt_atanf as *const () as usize),
                "exp2" => Some(dlls::crt::crt_exp2 as *const () as usize),
                "sinhf" => Some(dlls::crt::crt_sinhf as *const () as usize),
                "copysignf" => Some(dlls::crt::crt_copysignf as *const () as usize),
                "_isnan" => Some(dlls::crt::crt_isnan as *const () as usize),
                "_finite" => Some(dlls::crt::crt_finite as *const () as usize),
                "modf" => Some(dlls::crt::crt_modf as *const () as usize),
                "modff" => Some(dlls::crt::crt_modff as *const () as usize),
                "strtok" => Some(dlls::crt::crt_strtok as *const () as usize),
                "strpbrk" => Some(dlls::crt::crt_strpbrk as *const () as usize),
                "strcspn" => Some(dlls::crt::crt_strcspn as *const () as usize),
                "strspn" => Some(dlls::crt::crt_strspn as *const () as usize),
                "strncat" => Some(dlls::crt::crt_strncat as *const () as usize),
                "strncat_s" => Some(dlls::crt::crt_strncat_s as *const () as usize),
                "strtoll" => Some(dlls::crt::crt_strtoll as *const () as usize),
                "strcoll" => Some(dlls::crt::crt_strcoll as *const () as usize),
                "strerror" => Some(dlls::crt::crt_strerror as *const () as usize),
                "strerror_s" => Some(dlls::crt::crt_strerror_s as *const () as usize),
                "wcscoll" => Some(dlls::crt::crt_wcscoll as *const () as usize),
                "wcsncat_s" => Some(dlls::crt::crt_wcsncat_s as *const () as usize),
                "__std_type_info_name" => {
                    Some(dlls::crt::crt_std_type_info_name as *const () as usize)
                }
                "__std_type_info_compare" => {
                    Some(dlls::crt::crt_std_type_info_compare as *const () as usize)
                }
                "__tzname" => Some(dlls::crt::crt_tzname as *const () as usize),
                "__timezone" => Some(dlls::crt::crt_timezone as *const () as usize),
                "__daylight" => Some(dlls::crt::crt_daylight as *const () as usize),
                "_tzset" => Some(dlls::crt::crt_tzset as *const () as usize),
                "___lc_codepage_func" => {
                    Some(dlls::crt::crt_lc_codepage_func as *const () as usize)
                }
                "_localtime64_s" => Some(dlls::crt::crt_localtime64_s as *const () as usize),
                "_gmtime64_s" => Some(dlls::crt::crt_gmtime64_s as *const () as usize),
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
                "OleInitialize" => Some(dlls::shell::OleInitialize as *const () as usize),
                "OleUninitialize" => Some(dlls::shell::OleUninitialize as *const () as usize),
                "CoCreateInstance" => Some(dlls::shell::CoCreateInstance as *const () as usize),
                "CoCreateGuid" => Some(dlls::shell::CoCreateGuid as *const () as usize),
                "StringFromGUID2" => Some(dlls::shell::StringFromGUID2 as *const () as usize),
                "CoTaskMemAlloc" => Some(dlls::shell::CoTaskMemAlloc as *const () as usize),
                "CoTaskMemFree" => Some(dlls::shell::CoTaskMemFree as *const () as usize),
                "CoTaskMemRealloc" => Some(dlls::shell::CoTaskMemRealloc as *const () as usize),
                "CoGetApartmentType" => Some(dlls::shell::CoGetApartmentType as *const () as usize),
                "CoCreateFreeThreadedMarshaler" => {
                    Some(dlls::shell::CoCreateFreeThreadedMarshaler as *const () as usize)
                }
                "PropVariantClear" => Some(dlls::shell::PropVariantClear as *const () as usize),
                _ => None,
            },
            "OLEAUT32.DLL" => match func_name {
                "SysAllocString" => Some(dlls::shell::SysAllocString as *const () as usize),
                "SysFreeString" => Some(dlls::shell::SysFreeString as *const () as usize),
                "SysStringLen" => Some(dlls::shell::SysStringLen as *const () as usize),
                "SysStringByteLen" => Some(dlls::shell::SysStringByteLen as *const () as usize),
                "VariantInit" => Some(dlls::shell::VariantInit as *const () as usize),
                "VariantClear" => Some(dlls::shell::VariantClear as *const () as usize),
                "SafeArrayCreate" => Some(dlls::shell::SafeArrayCreate as *const () as usize),
                "SafeArrayDestroy" => Some(dlls::shell::SafeArrayDestroy as *const () as usize),
                "SafeArrayGetDim" => Some(dlls::shell::SafeArrayGetDim as *const () as usize),
                "SafeArrayGetLBound" => Some(dlls::shell::SafeArrayGetLBound as *const () as usize),
                "SafeArrayGetUBound" => Some(dlls::shell::SafeArrayGetUBound as *const () as usize),
                "SafeArrayGetVartype" => {
                    Some(dlls::shell::SafeArrayGetVartype as *const () as usize)
                }
                "SafeArrayAccessData" => {
                    Some(dlls::shell::SafeArrayAccessData as *const () as usize)
                }
                "SafeArrayUnaccessData" => {
                    Some(dlls::shell::SafeArrayUnaccessData as *const () as usize)
                }
                "SafeArrayPtrOfIndex" => {
                    Some(dlls::shell::SafeArrayPtrOfIndex as *const () as usize)
                }
                "SafeArrayPutElement" => {
                    Some(dlls::shell::SafeArrayPutElement as *const () as usize)
                }
                "SysAllocStringLen" => {
                    Some(dlls::shell::SysAllocStringLen as *const () as usize)
                }
                _ => None,
            },
            "VERSION.DLL" | "VERSION" | "version.dll" => match func_name {
                "GetFileVersionInfoSizeW" => {
                    Some(dlls::version::GetFileVersionInfoSizeW as *const () as usize)
                }
                "GetFileVersionInfoSizeA" => {
                    Some(dlls::version::GetFileVersionInfoSizeA as *const () as usize)
                }
                "GetFileVersionInfoW" => {
                    Some(dlls::version::GetFileVersionInfoW as *const () as usize)
                }
                "GetFileVersionInfoA" => {
                    Some(dlls::version::GetFileVersionInfoA as *const () as usize)
                }
                "VerQueryValueW" => {
                    Some(dlls::version::VerQueryValueW as *const () as usize)
                }
                "VerQueryValueA" => {
                    Some(dlls::version::VerQueryValueA as *const () as usize)
                }
                _ => None,
            },
            "DBGHELP.DLL" | "DBGHELP" | "dbghelp.dll" => match func_name {
                "SymInitialize" => Some(dlls::shell::SymInitialize as *const () as usize),
                "SymFromAddr" => Some(dlls::shell::SymFromAddr as *const () as usize),
                "SymCleanup" => Some(dlls::shell::SymCleanup as *const () as usize),
                _ => None,
            },
            "SHELL32.DLL" | "SHELL32" | "SHELL32.dll" | "shell32.dll" | "SHLWAPI.DLL"
            | "SHLWAPI" | "SHLWAPI.dll" | "shlwapi.dll" => match func_name {
                "CommandLineToArgvW" => Some(dlls::shell::CommandLineToArgvW as *const () as usize),
                "SHGetFolderPathW" => Some(dlls::shell::SHGetFolderPathW as *const () as usize),
                "SHGetFolderPathA" => Some(dlls::shell::SHGetFolderPathA as *const () as usize),
                "SHGetKnownFolderPath" => Some(dlls::shell::SHGetKnownFolderPath as *const () as usize),
                "SHGetSpecialFolderPathW" => Some(dlls::shell::SHGetSpecialFolderPathW as *const () as usize),
                "SHGetSpecialFolderPathA" => Some(dlls::shell::SHGetSpecialFolderPathA as *const () as usize),
                "ShellExecuteW" => Some(dlls::shell::ShellExecuteW as *const () as usize),
                "ShellExecuteA" => Some(dlls::shell::ShellExecuteA as *const () as usize),
                "ShellExecuteExW" => Some(dlls::shell::ShellExecuteExW as *const () as usize),
                "ShellExecuteExA" => Some(dlls::shell::ShellExecuteExA as *const () as usize),
                "SHFileOperationW" => Some(dlls::shell::SHFileOperationW as *const () as usize),
                "SHFileOperationA" => Some(dlls::shell::SHFileOperationA as *const () as usize),
                "DragAcceptFiles" => Some(dlls::shell::DragAcceptFiles as *const () as usize),
                "DragFinish" => Some(dlls::shell::DragFinish as *const () as usize),
                "DragQueryFileW" => Some(dlls::shell::DragQueryFileW as *const () as usize),
                "DragQueryFileA" => Some(dlls::shell::DragQueryFileA as *const () as usize),
                "DragQueryPoint" => Some(dlls::shell::DragQueryPoint as *const () as usize),
                "PathFileExistsW" => Some(dlls::shell::PathFileExistsW as *const () as usize),
                "PathFileExistsA" => Some(dlls::shell::PathFileExistsA as *const () as usize),
                "PathCanonicalizeW" => Some(dlls::shell::PathCanonicalizeW as *const () as usize),
                "PathCombineW" => Some(dlls::shell::PathCombineW as *const () as usize),
                "PathCombineA" => Some(dlls::shell::PathCombineA as *const () as usize),
                "SHDeleteKeyW" => Some(dlls::shell::SHDeleteKeyW as *const () as usize),
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
            // VULKAN-1.DLL: Forward DXVK imports to host libvulkan.so.1
            // VULKAN-1.DLL: Перенаправление импортов DXVK в системный libvulkan.so.1
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
            "IMM32.DLL" | "IMM32" | "imm32.dll" => match func_name {
                "ImmGetContext" => Some(dlls::imm32::ImmGetContext as *const () as usize),
                "ImmReleaseContext" => Some(dlls::imm32::ImmReleaseContext as *const () as usize),
                "ImmAssociateContext" => {
                    Some(dlls::imm32::ImmAssociateContext as *const () as usize)
                }
                "ImmAssociateContextEx" => {
                    Some(dlls::imm32::ImmAssociateContextEx as *const () as usize)
                }
                "ImmGetCompositionStringW" => {
                    Some(dlls::imm32::ImmGetCompositionStringW as *const () as usize)
                }
                "ImmSetCompositionStringW" => {
                    Some(dlls::imm32::ImmSetCompositionStringW as *const () as usize)
                }
                "ImmSetCandidateWindow" => {
                    Some(dlls::imm32::ImmSetCandidateWindow as *const () as usize)
                }
                "ImmSetCompositionWindow" => {
                    Some(dlls::imm32::ImmSetCompositionWindow as *const () as usize)
                }
                "ImmSetOpenStatus" => Some(dlls::imm32::ImmSetOpenStatus as *const () as usize),
                "ImmGetProperty" => Some(dlls::imm32::ImmGetProperty as *const () as usize),
                "ImmCreateContext" => Some(dlls::imm32::ImmCreateContext as *const () as usize),
                "ImmNotifyIME" => Some(dlls::imm32::ImmNotifyIME as *const () as usize),
                "ImmDestroyContext" => Some(dlls::imm32::ImmDestroyContext as *const () as usize),
                "ImmGetCandidateListCountW" => {
                    Some(dlls::imm32::ImmGetCandidateListCountW as *const () as usize)
                }
                "ImmGetCandidateListW" => {
                    Some(dlls::imm32::ImmGetCandidateListW as *const () as usize)
                }
                "ImmGetConversionStatus" => {
                    Some(dlls::imm32::ImmGetConversionStatus as *const () as usize)
                }
                "ImmGetOpenStatus" => Some(dlls::imm32::ImmGetOpenStatus as *const () as usize),
                "ImmSetConversionStatus" => {
                    Some(dlls::imm32::ImmSetConversionStatus as *const () as usize)
                }
                _ => None,
            },
            "DINPUT8.DLL" => match func_name {
                "DirectInput8Create" => {
                    Some(dlls::dinput::DirectInput8Create as *const () as usize)
                }
                _ => None,
            },
            "X3DAUDIO1_7.DLL" | "X3DAUDIO1_7" | "x3daudio1_7.dll" => match func_name {
                "X3DAudioInitialize" => Some(dlls::wasapi::X3DAudioInitialize as *const () as usize),
                "X3DAudioCalculate" => Some(dlls::wasapi::X3DAudioCalculate as *const () as usize),
                _ => None,
            },
            "XAPOFX1_5.DLL" | "XAPOFX1_5" | "xapofx1_5.dll" => match func_name {
                "CreateFX" => Some(dlls::wasapi::CreateFX as *const () as usize),
                _ => None,
            },
            "MMDEVAPI.DLL" | "MMDEVAPI" | "mmdevapi.dll" => match func_name {
                "ActivateAudioInterfaceAsync" | "#17" => {
                    Some(dlls::wasapi::ActivateAudioInterfaceAsync as *const () as usize)
                }
                _ => None,
            },
            "DWMAPI.DLL" | "DWMAPI" | "dwmapi.dll" => match func_name {
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
                "DwmGetCompositionTimingInfo" => {
                    Some(dlls::dwmapi::DwmGetCompositionTimingInfo as *const () as usize)
                }
                _ => None,
            },
            "SETUPAPI.DLL" | "SETUPAPI" | "setupapi.dll" | "setupapi" | "CFGMGR32.DLL" | "CFGMGR32" | "cfgmgr32.dll" | "cfgmgr32" => {
                if func_name.starts_with("CM_") || func_name.starts_with("_CM_") {
                    crate::dlls::cfgmgr32::resolve_cfgmgr32_symbol(func_name)
                } else {
                    match func_name {
                        "SetupDiGetClassDevs" | "SetupDiGetClassDevsA" => {
                            Some(dlls::setupapi::SetupDiGetClassDevsA as *const () as usize)
                        }
                        "SetupDiGetClassDevsW" => {
                            Some(dlls::setupapi::SetupDiGetClassDevsW as *const () as usize)
                        }
                        "SetupDiGetClassDevsExW" => {
                            Some(dlls::setupapi::SetupDiGetClassDevsExW as *const () as usize)
                        }
                        "SetupDiGetClassDevsExA" => {
                            Some(dlls::setupapi::SetupDiGetClassDevsExA as *const () as usize)
                        }
                        "SetupGetInfDriverStoreLocationW" => {
                            Some(dlls::setupapi::SetupGetInfDriverStoreLocationW as *const () as usize)
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
                        "SetupDiGetDeviceRegistryPropertyW" => {
                            Some(dlls::setupapi::SetupDiGetDeviceRegistryPropertyW as *const () as usize)
                        }
                        "SetupDiGetDeviceRegistryPropertyA" | "SetupDiGetDeviceRegistryProperty" => {
                            Some(dlls::setupapi::SetupDiGetDeviceRegistryPropertyA as *const () as usize)
                        }
                        "SetupDiGetDeviceInstanceIdW" => {
                            Some(dlls::setupapi::SetupDiGetDeviceInstanceIdW as *const () as usize)
                        }
                        "SetupDiGetDeviceInstanceIdA" | "SetupDiGetDeviceInstanceId" => {
                            Some(dlls::setupapi::SetupDiGetDeviceInstanceIdA as *const () as usize)
                        }
                        "SetupDiGetDevicePropertyW" | "SetupDiGetDeviceProperty" => {
                            Some(dlls::setupapi::SetupDiGetDevicePropertyW as *const () as usize)
                        }
                        _ => None,
                    }
                }
            },
            "HID.DLL" | "HID" | "hid.dll" => match func_name {
                "HidD_GetHidGuid" => Some(dlls::hid::HidD_GetHidGuid as *const () as usize),
                "HidD_GetPreparsedData" => Some(dlls::hid::HidD_GetPreparsedData as *const () as usize),
                "HidD_FreePreparsedData" => Some(dlls::hid::HidD_FreePreparsedData as *const () as usize),
                "HidD_GetAttributes" => Some(dlls::hid::HidD_GetAttributes as *const () as usize),
                "HidD_GetManufacturerString" => Some(dlls::hid::HidD_GetManufacturerString as *const () as usize),
                "HidD_GetProductString" => Some(dlls::hid::HidD_GetProductString as *const () as usize),
                "HidD_GetSerialNumberString" => Some(dlls::hid::HidD_GetSerialNumberString as *const () as usize),
                "HidD_GetIndexedString" => Some(dlls::hid::HidD_GetIndexedString as *const () as usize),
                "HidD_GetFeature" => Some(dlls::hid::HidD_GetFeature as *const () as usize),
                "HidD_SetFeature" => Some(dlls::hid::HidD_SetFeature as *const () as usize),
                "HidD_GetInputReport" => Some(dlls::hid::HidD_GetInputReport as *const () as usize),
                "HidD_SetOutputReport" => Some(dlls::hid::HidD_SetOutputReport as *const () as usize),
                "HidD_GetNumInputBuffers" => Some(dlls::hid::HidD_GetNumInputBuffers as *const () as usize),
                "HidD_SetNumInputBuffers" => Some(dlls::hid::HidD_SetNumInputBuffers as *const () as usize),
                "HidD_FlushQueue" => Some(dlls::hid::HidD_FlushQueue as *const () as usize),
                "HidP_GetCaps" => Some(dlls::hid::HidP_GetCaps as *const () as usize),
                "HidP_GetButtonCaps" => Some(dlls::hid::HidP_GetButtonCaps as *const () as usize),
                "HidP_GetValueCaps" => Some(dlls::hid::HidP_GetValueCaps as *const () as usize),
                "HidP_GetData" => Some(dlls::hid::HidP_GetData as *const () as usize),
                "HidP_MaxDataListLength" => Some(dlls::hid::HidP_MaxDataListLength as *const () as usize),
                "HidP_SetUsages" => Some(dlls::hid::HidP_SetUsages as *const () as usize),
                "HidP_SetUsageValue" => Some(dlls::hid::HidP_SetUsageValue as *const () as usize),
                _ => None,
            },
            "BTHPROPS.CPL" | "BTHPROPS" | "bthprops.cpl" | "BLUETOOTHAPIS.DLL" | "BLUETOOTHAPIS" | "bluetoothapis.dll" => match func_name {
                "BluetoothFindFirstRadio" => Some(crate::dlls::bthprops::BluetoothFindFirstRadio as *const () as usize),
                "BluetoothFindNextRadio" => Some(crate::dlls::bthprops::BluetoothFindNextRadio as *const () as usize),
                "BluetoothFindRadioClose" => Some(crate::dlls::bthprops::BluetoothFindRadioClose as *const () as usize),
                "BluetoothFindFirstDevice" => Some(crate::dlls::bthprops::BluetoothFindFirstDevice as *const () as usize),
                "BluetoothFindNextDevice" => Some(crate::dlls::bthprops::BluetoothFindNextDevice as *const () as usize),
                "BluetoothFindDeviceClose" => Some(crate::dlls::bthprops::BluetoothFindDeviceClose as *const () as usize),
                "BluetoothGetRadioInfo" => Some(crate::dlls::bthprops::BluetoothGetRadioInfo as *const () as usize),
                "BluetoothGetDeviceInfo" => Some(crate::dlls::bthprops::BluetoothGetDeviceInfo as *const () as usize),
                _ => Some(crate::dlls::bthprops::bthprops_generic_stub as *const () as usize),
            },
            "SECUR32.DLL" | "SECUR32" | "secur32.dll" | "SECURITY.DLL" | "SECURITY" | "security.dll" | "SSPICLI.DLL" | "SSPICLI" => match func_name {
                "InitSecurityInterfaceA" => {
                    Some(dlls::secur32::InitSecurityInterfaceA as *const () as usize)
                }
                "InitSecurityInterfaceW" => {
                    Some(dlls::secur32::InitSecurityInterfaceW as *const () as usize)
                }
                "AcquireCredentialsHandleA" => {
                    Some(dlls::secur32::AcquireCredentialsHandleA as *const () as usize)
                }
                "AcquireCredentialsHandleW" => {
                    Some(dlls::secur32::AcquireCredentialsHandleW as *const () as usize)
                }
                "FreeCredentialsHandle" => {
                    Some(dlls::secur32::FreeCredentialsHandle as *const () as usize)
                }
                "InitializeSecurityContextA" => {
                    Some(dlls::secur32::InitializeSecurityContextA as *const () as usize)
                }
                "InitializeSecurityContextW" => {
                    Some(dlls::secur32::InitializeSecurityContextW as *const () as usize)
                }
                "DeleteSecurityContext" => {
                    Some(dlls::secur32::DeleteSecurityContext as *const () as usize)
                }
                "FreeContextBuffer" => {
                    Some(dlls::secur32::FreeContextBuffer as *const () as usize)
                }
                "QueryContextAttributesA" => {
                    Some(dlls::secur32::QueryContextAttributesA as *const () as usize)
                }
                "QueryContextAttributesW" => {
                    Some(dlls::secur32::QueryContextAttributesW as *const () as usize)
                }
                _ => None,
            },
            "D3D12.DLL" | "D3D12" | "d3d12.dll" => match func_name {
                "D3D12CreateDevice" | "#101" => {
                    Some(dlls::d3d12::D3D12CreateDevice as *const () as usize)
                }
                "D3D12GetDebugInterface" | "#102" => {
                    Some(dlls::d3d12::D3D12GetDebugInterface as *const () as usize)
                }
                "D3D12SerializeRootSignature" => {
                    Some(dlls::d3d12::D3D12SerializeRootSignature as *const () as usize)
                }
                _ => None,
            },
            "D3D9.DLL" | "D3D9" | "d3d9.dll" => match func_name {
                "D3DPERF_BeginEvent" => Some(dlls::generated::d3d9::D3DPERF_BeginEvent as *const () as usize),
                "D3DPERF_EndEvent" => Some(dlls::generated::d3d9::D3DPERF_EndEvent as *const () as usize),
                "D3DPERF_SetMarker" => Some(dlls::generated::d3d9::D3DPERF_SetMarker as *const () as usize),
                "D3DPERF_GetStatus" => Some(dlls::generated::d3d9::D3DPERF_GetStatus as *const () as usize),
                "D3DPERF_QueryRepeatFrame" => Some(dlls::generated::d3d9::D3DPERF_QueryRepeatFrame as *const () as usize),
                "D3DPERF_SetOptions" => Some(dlls::generated::d3d9::D3DPERF_SetOptions as *const () as usize),
                "D3DPERF_SetRegion" => Some(dlls::generated::d3d9::D3DPERF_SetRegion as *const () as usize),
                _ => None,
            },
            "WINHTTP.DLL" | "WINHTTP" | "winhttp.dll" => match func_name {
                "WinHttpGetIEProxyConfigForCurrentUser" => {
                    Some(dlls::winhttp::WinHttpGetIEProxyConfigForCurrentUser as *const () as usize)
                }
                "WinHttpGetDefaultProxyConfiguration" => {
                    Some(dlls::winhttp::WinHttpGetDefaultProxyConfiguration as *const () as usize)
                }
                "WinHttpOpen" => {
                    Some(dlls::winhttp::WinHttpOpen as *const () as usize)
                }
                "WinHttpGetProxyForUrl" => {
                    Some(dlls::winhttp::WinHttpGetProxyForUrl as *const () as usize)
                }
                "WinHttpCloseHandle" => {
                    Some(dlls::winhttp::WinHttpCloseHandle as *const () as usize)
                }
                "WinHttpConnect" => {
                    Some(dlls::winhttp::WinHttpConnect as *const () as usize)
                }
                "WinHttpOpenRequest" => {
                    Some(dlls::winhttp::WinHttpOpenRequest as *const () as usize)
                }
                "WinHttpSendRequest" => {
                    Some(dlls::winhttp::WinHttpSendRequest as *const () as usize)
                }
                "WinHttpReceiveResponse" => {
                    Some(dlls::winhttp::WinHttpReceiveResponse as *const () as usize)
                }
                "WinHttpQueryHeaders" => {
                    Some(dlls::winhttp::WinHttpQueryHeaders as *const () as usize)
                }
                "WinHttpReadData" => {
                    Some(dlls::winhttp::WinHttpReadData as *const () as usize)
                }
                "WinHttpSetOption" => {
                    Some(dlls::winhttp::WinHttpSetOption as *const () as usize)
                }
                "WinHttpQueryOption" => {
                    Some(dlls::winhttp::WinHttpQueryOption as *const () as usize)
                }
                "WinHttpSetTimeouts" => {
                    Some(dlls::winhttp::WinHttpSetTimeouts as *const () as usize)
                }
                "WinHttpSetStatusCallback" => {
                    Some(dlls::winhttp::WinHttpSetStatusCallback as *const () as usize)
                }
                "WinHttpCrackUrl" => {
                    Some(dlls::winhttp::WinHttpCrackUrl as *const () as usize)
                }
                "WinHttpCreateUrl" => {
                    Some(dlls::winhttp::WinHttpCreateUrl as *const () as usize)
                }
                _ => None,
            },
            "DSTORAGE.DLL" | "DSTORAGE" | "dstorage.dll" => match func_name {
                "DStorageGetFactory" => Some(dlls::dstorage::DStorageGetFactory as *const () as usize),
                "DStorageGetFactoryCore" => Some(dlls::dstorage::DStorageGetFactoryCore as *const () as usize),
                "DStorageSetConfiguration" => Some(dlls::dstorage::DStorageSetConfiguration as *const () as usize),
                "DStorageSetConfiguration1" => Some(dlls::dstorage::DStorageSetConfiguration1 as *const () as usize),
                "DStorageSetConfigurationCore" => Some(dlls::dstorage::DStorageSetConfigurationCore as *const () as usize),
                "DStorageCreateCompressionCodec" => Some(dlls::dstorage::DStorageCreateCompressionCodec as *const () as usize),
                "DStorageCreateCompressionCodecCore" => Some(dlls::dstorage::DStorageCreateCompressionCodecCore as *const () as usize),
                _ => None,
            },
            "IPHLPAPI.DLL" | "IPHLPAPI" | "iphlpapi.dll" => match func_name {
                "IcmpCreateFile" => Some(dlls::iphlpapi::IcmpCreateFile as *const () as usize),
                "IcmpCloseHandle" => Some(dlls::iphlpapi::IcmpCloseHandle as *const () as usize),
                "IcmpSendEcho" => Some(dlls::iphlpapi::IcmpSendEcho as *const () as usize),
                "Icmp6CreateFile" => Some(dlls::iphlpapi::Icmp6CreateFile as *const () as usize),
                "Icmp6CloseHandle" => Some(dlls::iphlpapi::Icmp6CloseHandle as *const () as usize),
                "Icmp6SendEcho2" => Some(dlls::iphlpapi::Icmp6SendEcho2 as *const () as usize),
                "GetAdaptersInfo" => Some(dlls::iphlpapi::GetAdaptersInfo as *const () as usize),
                "GetAdaptersAddresses" => Some(dlls::iphlpapi::GetAdaptersAddresses as *const () as usize),
                "GetNetworkParams" => Some(dlls::iphlpapi::GetNetworkParams as *const () as usize),
                "GetBestInterfaceEx" => Some(dlls::iphlpapi::GetBestInterfaceEx as *const () as usize),
                "NotifyAddrChange" => Some(dlls::iphlpapi::NotifyAddrChange as *const () as usize),
                "NotifyRouteChange" => Some(dlls::iphlpapi::NotifyRouteChange as *const () as usize),
                "CancelIPChangeNotify" => Some(dlls::iphlpapi::CancelIPChangeNotify as *const () as usize),
                _ => None,
            },
            "UIAUTOMATIONCORE.DLL" | "UIAUTOMATIONCORE" | "uiautomationcore.dll" => match func_name {
                "UiaClientsAreListening" => Some(dlls::uiautomation::UiaClientsAreListening as *const () as usize),
                "UiaHostProviderFromHwnd" => Some(dlls::uiautomation::UiaHostProviderFromHwnd as *const () as usize),
                "UiaReturnRawElementProvider" => Some(dlls::uiautomation::UiaReturnRawElementProvider as *const () as usize),
                "UiaRaiseNotificationEvent" => Some(dlls::uiautomation::UiaRaiseNotificationEvent as *const () as usize),
                "UiaRaiseStructureChangedEvent" => Some(dlls::uiautomation::UiaRaiseStructureChangedEvent as *const () as usize),
                "UiaRaiseAutomationPropertyChangedEvent" => Some(dlls::uiautomation::UiaRaiseAutomationPropertyChangedEvent as *const () as usize),
                "UiaRaiseAutomationEvent" => Some(dlls::uiautomation::UiaRaiseAutomationEvent as *const () as usize),
                "UiaGetReservedMixedAttributeValue" => Some(dlls::uiautomation::UiaGetReservedMixedAttributeValue as *const () as usize),
                "UiaGetReservedNotSupportedValue" => Some(dlls::uiautomation::UiaGetReservedNotSupportedValue as *const () as usize),
                _ => None,
            },
            "MF.DLL" | "MF" | "mf.dll"
            | "MFPLAT.DLL" | "MFPLAT" | "mfplat.dll"
            | "MFREADWRITE.DLL" | "MFREADWRITE" | "mfreadwrite.dll"
            | "MFPLAY.DLL" | "MFPLAY" | "mfplay.dll" => match func_name {
                "MFStartup" => Some(dlls::mf::MFStartup as *const () as usize),
                "MFShutdown" => Some(dlls::mf::MFShutdown as *const () as usize),
                "MFCreateAttributes" => Some(dlls::mf::MFCreateAttributes as *const () as usize),
                "MFCreateMediaType" => Some(dlls::mf::MFCreateMediaType as *const () as usize),
                "MFCreateAlignedMemoryBuffer" => Some(dlls::mf::MFCreateAlignedMemoryBuffer as *const () as usize),
                "MFCreateSample" => Some(dlls::mf::MFCreateSample as *const () as usize),
                "MFCreateEventQueue" => Some(dlls::mf::MFCreateEventQueue as *const () as usize),
                "MFCreateAsyncResult" => Some(dlls::mf::MFCreateAsyncResult as *const () as usize),
                "MFInvokeCallback" => Some(dlls::mf::MFInvokeCallback as *const () as usize),
                "MFPutWorkItem" => Some(dlls::mf::MFPutWorkItem as *const () as usize),
                "MFPutWaitingWorkItem" => Some(dlls::mf::MFPutWaitingWorkItem as *const () as usize),
                "MFCreateDXGIDeviceManager" => Some(dlls::mf::MFCreateDXGIDeviceManager as *const () as usize),
                "MFCreateSourceResolver" => Some(dlls::mf::MFCreateSourceResolver as *const () as usize),
                "MFCreateMediaSession" => Some(dlls::mf::MFCreateMediaSession as *const () as usize),
                "MFCreateTopology" => Some(dlls::mf::MFCreateTopology as *const () as usize),
                "MFCreateTopologyNode" => Some(dlls::mf::MFCreateTopologyNode as *const () as usize),
                "MFCreateSampleGrabberSinkActivate" => Some(dlls::mf::MFCreateSampleGrabberSinkActivate as *const () as usize),
                "MFCreateAudioRendererActivate" => Some(dlls::mf::MFCreateAudioRendererActivate as *const () as usize),
                "MFGetService" => Some(dlls::mf::MFGetService as *const () as usize),
                "MFEnumDeviceSources" => Some(dlls::mf::MFEnumDeviceSources as *const () as usize),
                "MFCreateDeviceSource" => Some(dlls::mf::MFCreateDeviceSource as *const () as usize),
                "MFCreateSinkWriterFromURL" => Some(dlls::mf::MFCreateSinkWriterFromURL as *const () as usize),
                "MFPCreateMediaPlayer" => Some(dlls::mf::MFPCreateMediaPlayer as *const () as usize),
                "MFPCreateMediaPlayerEx" => Some(dlls::mf::MFPCreateMediaPlayerEx as *const () as usize),
                _ => None,
            },
            _ => None,
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

        let ntdll_qdf = IatSymbolResolver::resolve_symbol("ntdll.dll", "NtQueryDirectoryFile");
        assert!(ntdll_qdf.is_some());
        assert_eq!(
            ntdll_qdf.unwrap(),
            crate::dlls::ntdll::NtQueryDirectoryFile as *const () as usize
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

        let ntdll_add_growable = IatSymbolResolver::resolve_symbol("NTDLL.DLL", "RtlAddGrowableFunctionTable");
        assert!(ntdll_add_growable.is_some());
        assert_eq!(
            ntdll_add_growable.unwrap(),
            crate::dlls::ntdll::RtlAddGrowableFunctionTable as *const () as usize
        );

        let k32_install_cb = IatSymbolResolver::resolve_symbol("KERNEL32.DLL", "RtlInstallFunctionTableCallback");
        assert!(k32_install_cb.is_some());
        assert_eq!(
            k32_install_cb.unwrap(),
            crate::dlls::kernel32::RtlInstallFunctionTableCallback as *const () as usize
        );
    }
}
