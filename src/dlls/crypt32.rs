//! Implementation of CRYPT32.DLL APIs for Godot Engine HTTPS/SSL/crypto support.

#[no_mangle]
pub unsafe extern "win64" fn CertOpenSystemStoreA(
    _h_prov: usize,
    sz_subsystem_protocol: *const libc::c_char,
) -> usize {
    let name = if !sz_subsystem_protocol.is_null() {
        std::ffi::CStr::from_ptr(sz_subsystem_protocol).to_string_lossy()
    } else {
        std::borrow::Cow::Borrowed("ROOT")
    };
    println!("[CRYPT32] CertOpenSystemStoreA('{}') -> 0x00010001", name);
    0x00010001
}

#[no_mangle]
pub unsafe extern "win64" fn CertOpenSystemStoreW(
    _h_prov: usize,
    _sz_subsystem_protocol: *const u16,
) -> usize {
    println!("[CRYPT32] CertOpenSystemStoreW -> 0x00010001");
    0x00010001
}

#[no_mangle]
pub unsafe extern "win64" fn CertCloseStore(_h_cert_store: usize, _dw_flags: u32) -> i32 {
    println!("[CRYPT32] CertCloseStore -> TRUE");
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CertFreeCertificateContext(
    _p_cert_context: *const std::ffi::c_void,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CertEnumCertificatesInStore(
    _h_cert_store: usize,
    _p_prev_cert_context: *const std::ffi::c_void,
) -> *mut std::ffi::c_void {
    std::ptr::null_mut()
}
