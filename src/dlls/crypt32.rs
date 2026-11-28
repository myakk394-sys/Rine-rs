//! Implementation of CRYPT32.DLL APIs for Godot Engine / Unity / .NET HTTPS/SSL/crypto support.
#![allow(non_snake_case)]

use std::ffi::c_void;

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
pub unsafe extern "win64" fn CertOpenStore(
    _store_provider: usize,
    _msg_and_cert_encoding_type: u32,
    _crypt_prov: usize,
    _flags: u32,
    _parameters: *const c_void,
) -> usize {
    println!("[CRYPT32] CertOpenStore -> 0x00010002");
    0x00010002
}

#[no_mangle]
pub unsafe extern "win64" fn CertCloseStore(_h_cert_store: usize, _dw_flags: u32) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CertFreeCertificateContext(
    _p_cert_context: *const c_void,
) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CertFreeCertificateChain(
    _p_chain_context: *const c_void,
) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CertVerifyCertificateChainPolicy(
    _policy_oid: usize,
    _p_chain_context: *const c_void,
    _p_policy_para: *const c_void,
    p_policy_status: *mut c_void,
) -> i32 {
    if !p_policy_status.is_null() {
        // Clear dwError field in CERT_CHAIN_POLICY_STATUS (offset 4 is dwError)
        unsafe {
            *(p_policy_status.add(4) as *mut u32) = 0; // ERROR_SUCCESS
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CertAddEncodedCertificateToStore(
    _h_cert_store: usize,
    _cert_encoding_type: u32,
    _cert_encoded: *const u8,
    _cert_encoded_size: u32,
    _add_disposition: u32,
    pp_cert_context: *mut *mut c_void,
) -> i32 {
    if !pp_cert_context.is_null() {
        *pp_cert_context = std::ptr::null_mut();
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CertGetCertificateChain(
    _chain_engine: usize,
    _cert_context: *const c_void,
    _time: *const c_void,
    _additional_store: usize,
    _chain_para: *const c_void,
    _flags: u32,
    _reserved: *const c_void,
    pp_chain_context: *mut *mut c_void,
) -> i32 {
    if !pp_chain_context.is_null() {
        *pp_chain_context = std::ptr::null_mut();
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CertGetCertificateContextProperty(
    _cert_context: *const c_void,
    _prop_id: u32,
    _data: *mut c_void,
    pcb_data: *mut u32,
) -> i32 {
    if !pcb_data.is_null() {
        *pcb_data = 0;
    }
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn CertEnumCertificatesInStore(
    _h_cert_store: usize,
    _p_prev_cert_context: *const c_void,
) -> *mut c_void {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "win64" fn CryptBinaryToStringA(
    pb_binary: *const u8,
    cb_binary: u32,
    _flags: u32,
    psz_string: *mut u8,
    pcch_string: *mut u32,
) -> i32 {
    if pcch_string.is_null() {
        return 0;
    }
    let req = cb_binary * 2 + 1;
    if psz_string.is_null() {
        *pcch_string = req;
        return 1;
    }
    if *pcch_string < req {
        *pcch_string = req;
        return 0;
    }
    // Hex encode
    let hex_chars = b"0123456789ABCDEF";
    for i in 0..cb_binary as usize {
        let b = *pb_binary.add(i);
        *psz_string.add(i * 2) = hex_chars[(b >> 4) as usize];
        *psz_string.add(i * 2 + 1) = hex_chars[(b & 0xF) as usize];
    }
    *psz_string.add(cb_binary as usize * 2) = 0;
    *pcch_string = cb_binary * 2;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CryptBinaryToStringW(
    pb_binary: *const u8,
    cb_binary: u32,
    flags: u32,
    psz_string: *mut u16,
    pcch_string: *mut u32,
) -> i32 {
    let mut temp = vec![0u8; (cb_binary * 2 + 2) as usize];
    let mut len = temp.len() as u32;
    let ok = CryptBinaryToStringA(pb_binary, cb_binary, flags, temp.as_mut_ptr(), &mut len);
    if ok == 0 {
        return 0;
    }
    if pcch_string.is_null() {
        return 0;
    }
    if psz_string.is_null() {
        *pcch_string = len;
        return 1;
    }
    let copy_len = (*pcch_string).min(len) as usize;
    for i in 0..copy_len {
        *psz_string.add(i) = temp[i] as u16;
    }
    *pcch_string = copy_len as u32;
    1
}

#[repr(C)]
pub struct CryptoApiBlob {
    pub cb_data: u32,
    pub pb_data: *mut u8,
}

#[no_mangle]
pub unsafe extern "win64" fn CryptProtectData(
    p_data_in: *const CryptoApiBlob,
    _sz_data_descr: *const u16,
    _p_optional_entropy: *const CryptoApiBlob,
    _pv_reserved: *mut u8,
    _p_prompt_struct: *mut u8,
    _dw_flags: u32,
    p_data_out: *mut CryptoApiBlob,
) -> i32 {
    if p_data_in.is_null() || p_data_out.is_null() {
        return 0;
    }
    let in_len = (*p_data_in).cb_data as usize;
    let in_ptr = (*p_data_in).pb_data;
    if in_len > 0 && in_ptr.is_null() {
        return 0;
    }
    let out_len = in_len + 4;
    let out_ptr = crate::dlls::kernel32::LocalAlloc(0x40, out_len) as *mut u8;
    if out_ptr.is_null() {
        return 0;
    }
    std::ptr::copy_nonoverlapping(b"RINE".as_ptr(), out_ptr, 4);
    for i in 0..in_len {
        *out_ptr.add(4 + i) = (*in_ptr.add(i)) ^ 0x5A;
    }
    (*p_data_out).cb_data = out_len as u32;
    (*p_data_out).pb_data = out_ptr;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CryptUnprotectData(
    p_data_in: *const CryptoApiBlob,
    ppsz_data_descr: *mut *mut u16,
    _p_optional_entropy: *const CryptoApiBlob,
    _pv_reserved: *mut u8,
    _p_prompt_struct: *mut u8,
    _dw_flags: u32,
    p_data_out: *mut CryptoApiBlob,
) -> i32 {
    if p_data_in.is_null() || p_data_out.is_null() {
        return 0;
    }
    let in_len = (*p_data_in).cb_data as usize;
    let in_ptr = (*p_data_in).pb_data;
    if in_len == 0 || in_ptr.is_null() {
        return 0;
    }
    let input = std::slice::from_raw_parts(in_ptr, in_len);
    let (plain_len, xor_mask, offset) = if in_len >= 4 && &input[..4] == b"RINE" {
        (in_len - 4, 0x5Au8, 4usize)
    } else {
        (in_len, 0u8, 0usize)
    };
    let out_ptr = crate::dlls::kernel32::LocalAlloc(0x40, plain_len.max(1)) as *mut u8;
    if out_ptr.is_null() {
        return 0;
    }
    for i in 0..plain_len {
        *out_ptr.add(i) = input[offset + i] ^ xor_mask;
    }
    if !ppsz_data_descr.is_null() {
        *ppsz_data_descr = std::ptr::null_mut();
    }
    (*p_data_out).cb_data = plain_len as u32;
    (*p_data_out).pb_data = out_ptr;
    1
}
