#![allow(non_snake_case)]

pub use crate::dlls::generated::secur32::*;

#[repr(C)]
pub struct SecurityFunctionTableA {
    pub dwVersion: u32,
    pub EnumerateSecurityPackagesA: usize,
    pub QueryCredentialsAttributesA: usize,
    pub AcquireCredentialsHandleA: usize,
    pub FreeCredentialsHandle: usize,
    pub Reserved2: usize,
    pub InitializeSecurityContextA: usize,
    pub AcceptSecurityContext: usize,
    pub CompleteAuthToken: usize,
    pub DeleteSecurityContext: usize,
    pub ApplyControlToken: usize,
    pub QueryContextAttributesA: usize,
    pub FreeContextBuffer: usize,
    pub MakeSignature: usize,
    pub VerifySignature: usize,
}

#[repr(C)]
pub struct SecurityFunctionTableW {
    pub dwVersion: u32,
    pub EnumerateSecurityPackagesW: usize,
    pub QueryCredentialsAttributesW: usize,
    pub AcquireCredentialsHandleW: usize,
    pub FreeCredentialsHandle: usize,
    pub Reserved2: usize,
    pub InitializeSecurityContextW: usize,
    pub AcceptSecurityContext: usize,
    pub CompleteAuthToken: usize,
    pub DeleteSecurityContext: usize,
    pub ApplyControlToken: usize,
    pub QueryContextAttributesW: usize,
    pub FreeContextBuffer: usize,
    pub MakeSignature: usize,
    pub VerifySignature: usize,
}

#[no_mangle]
pub unsafe extern "win64" fn AcquireCredentialsHandleA(
    _pszPrincipal: *const libc::c_char,
    _pszPackage: *const libc::c_char,
    _fCredentialUse: u32,
    _pvLogonID: *mut std::ffi::c_void,
    _pAuthData: *mut std::ffi::c_void,
    _pGetKeyFn: *mut std::ffi::c_void,
    _pvGetKeyArgument: *mut std::ffi::c_void,
    _phCredential: *mut u64,
    _ptsExpiry: *mut u64,
) -> i32 {
    0 // SEC_E_OK
}

#[no_mangle]
pub unsafe extern "win64" fn AcquireCredentialsHandleW(
    _pszPrincipal: *const u16,
    _pszPackage: *const u16,
    _fCredentialUse: u32,
    _pvLogonID: *mut std::ffi::c_void,
    _pAuthData: *mut std::ffi::c_void,
    _pGetKeyFn: *mut std::ffi::c_void,
    _pvGetKeyArgument: *mut std::ffi::c_void,
    _phCredential: *mut u64,
    _ptsExpiry: *mut u64,
) -> i32 {
    0 // SEC_E_OK
}

#[no_mangle]
pub unsafe extern "win64" fn FreeCredentialsHandle(_phCredential: *mut u64) -> i32 {
    0 // SEC_E_OK
}

#[no_mangle]
pub unsafe extern "win64" fn InitializeSecurityContextA(
    _phCredential: *mut u64,
    _phContext: *mut u64,
    _pszTargetName: *const libc::c_char,
    _fContextReq: u32,
    _Reserved1: u32,
    _TargetDataRep: u32,
    _pInput: *mut std::ffi::c_void,
    _Reserved2: u32,
    _phNewContext: *mut u64,
    _pOutput: *mut std::ffi::c_void,
    _pfContextAttr: *mut u32,
    _ptsExpiry: *mut u64,
) -> i32 {
    0 // SEC_E_OK
}

#[no_mangle]
pub unsafe extern "win64" fn InitializeSecurityContextW(
    _phCredential: *mut u64,
    _phContext: *mut u64,
    _pszTargetName: *const u16,
    _fContextReq: u32,
    _Reserved1: u32,
    _TargetDataRep: u32,
    _pInput: *mut std::ffi::c_void,
    _Reserved2: u32,
    _phNewContext: *mut u64,
    _pOutput: *mut std::ffi::c_void,
    _pfContextAttr: *mut u32,
    _ptsExpiry: *mut u64,
) -> i32 {
    0 // SEC_E_OK
}

#[no_mangle]
pub unsafe extern "win64" fn DeleteSecurityContext(_phContext: *mut u64) -> i32 {
    0 // SEC_E_OK
}

#[no_mangle]
pub unsafe extern "win64" fn FreeContextBuffer(_pvBuffer: *mut std::ffi::c_void) -> i32 {
    0 // SEC_E_OK
}

#[no_mangle]
pub unsafe extern "win64" fn QueryContextAttributesA(
    _phContext: *mut u64,
    _ulAttribute: u32,
    _pBuffer: *mut std::ffi::c_void,
) -> i32 {
    0 // SEC_E_OK
}

#[no_mangle]
pub unsafe extern "win64" fn QueryContextAttributesW(
    _phContext: *mut u64,
    _ulAttribute: u32,
    _pBuffer: *mut std::ffi::c_void,
) -> i32 {
    0 // SEC_E_OK
}

use std::sync::OnceLock;

static SEC_TABLE_A: OnceLock<SecurityFunctionTableA> = OnceLock::new();
static SEC_TABLE_W: OnceLock<SecurityFunctionTableW> = OnceLock::new();

#[no_mangle]
pub unsafe extern "win64" fn InitSecurityInterfaceA() -> *const SecurityFunctionTableA {
    let tbl = SEC_TABLE_A.get_or_init(|| SecurityFunctionTableA {
        dwVersion: 1,
        EnumerateSecurityPackagesA: 0,
        QueryCredentialsAttributesA: 0,
        AcquireCredentialsHandleA: AcquireCredentialsHandleA as *const () as usize,
        FreeCredentialsHandle: FreeCredentialsHandle as *const () as usize,
        Reserved2: 0,
        InitializeSecurityContextA: InitializeSecurityContextA as *const () as usize,
        AcceptSecurityContext: 0,
        CompleteAuthToken: 0,
        DeleteSecurityContext: DeleteSecurityContext as *const () as usize,
        ApplyControlToken: 0,
        QueryContextAttributesA: QueryContextAttributesA as *const () as usize,
        FreeContextBuffer: FreeContextBuffer as *const () as usize,
        MakeSignature: 0,
        VerifySignature: 0,
    });
    println!("[SECUR32] InitSecurityInterfaceA() -> {:p}", tbl);
    tbl as *const SecurityFunctionTableA
}

#[no_mangle]
pub unsafe extern "win64" fn InitSecurityInterfaceW() -> *const SecurityFunctionTableW {
    let tbl = SEC_TABLE_W.get_or_init(|| SecurityFunctionTableW {
        dwVersion: 1,
        EnumerateSecurityPackagesW: 0,
        QueryCredentialsAttributesW: 0,
        AcquireCredentialsHandleW: AcquireCredentialsHandleW as *const () as usize,
        FreeCredentialsHandle: FreeCredentialsHandle as *const () as usize,
        Reserved2: 0,
        InitializeSecurityContextW: InitializeSecurityContextW as *const () as usize,
        AcceptSecurityContext: 0,
        CompleteAuthToken: 0,
        DeleteSecurityContext: DeleteSecurityContext as *const () as usize,
        ApplyControlToken: 0,
        QueryContextAttributesW: QueryContextAttributesW as *const () as usize,
        FreeContextBuffer: FreeContextBuffer as *const () as usize,
        MakeSignature: 0,
        VerifySignature: 0,
    });
    println!("[SECUR32] InitSecurityInterfaceW() -> {:p}", tbl);
    tbl as *const SecurityFunctionTableW
}
