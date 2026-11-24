//! SteamStub DRM Auto-Unpacker for Rine
//!
//! Automatically detects SteamStub Variant 3.1 protection, decrypts the original
//! .text code section in memory on-the-fly, and restores the Original Entry Point (OEP).

use std::ffi::c_void;

type EvpCipherCtxNewFn = unsafe extern "C" fn() -> *mut c_void;
type EvpCipherCtxFreeFn = unsafe extern "C" fn(*mut c_void);
type EvpCipherFn = unsafe extern "C" fn() -> *const c_void;
type EvpDecryptInitExFn = unsafe extern "C" fn(
    ctx: *mut c_void,
    cipher: *const c_void,
    engine: *mut c_void,
    key: *const u8,
    iv: *const u8,
) -> i32;
type EvpCipherCtxSetPaddingFn = unsafe extern "C" fn(ctx: *mut c_void, pad: i32) -> i32;
type EvpDecryptUpdateFn = unsafe extern "C" fn(
    ctx: *mut c_void,
    out: *mut u8,
    outl: *mut i32,
    in_buf: *const u8,
    inl: i32,
) -> i32;
type EvpDecryptFinalExFn =
    unsafe extern "C" fn(ctx: *mut c_void, out: *mut u8, outl: *mut i32) -> i32;

struct CryptoLib {
    _handle: *mut c_void,
    ctx_new: EvpCipherCtxNewFn,
    ctx_free: EvpCipherCtxFreeFn,
    aes_256_ecb: EvpCipherFn,
    aes_256_cbc: EvpCipherFn,
    decrypt_init_ex: EvpDecryptInitExFn,
    ctx_set_padding: EvpCipherCtxSetPaddingFn,
    decrypt_update: EvpDecryptUpdateFn,
    decrypt_final_ex: EvpDecryptFinalExFn,
}

impl CryptoLib {
    fn load() -> Option<Self> {
        let candidate_libs = ["libcrypto.so.3\0", "libcrypto.so\0"];
        for lib in candidate_libs {
            let handle = unsafe { libc::dlopen(lib.as_ptr() as *const libc::c_char, libc::RTLD_NOW) };
            if handle.is_null() {
                continue;
            }
            unsafe {
                let ctx_new: EvpCipherCtxNewFn = std::mem::transmute(libc::dlsym(handle, b"EVP_CIPHER_CTX_new\0".as_ptr() as *const libc::c_char));
                let ctx_free: EvpCipherCtxFreeFn = std::mem::transmute(libc::dlsym(handle, b"EVP_CIPHER_CTX_free\0".as_ptr() as *const libc::c_char));
                let aes_256_ecb: EvpCipherFn = std::mem::transmute(libc::dlsym(handle, b"EVP_aes_256_ecb\0".as_ptr() as *const libc::c_char));
                let aes_256_cbc: EvpCipherFn = std::mem::transmute(libc::dlsym(handle, b"EVP_aes_256_cbc\0".as_ptr() as *const libc::c_char));
                let decrypt_init_ex: EvpDecryptInitExFn = std::mem::transmute(libc::dlsym(handle, b"EVP_DecryptInit_ex\0".as_ptr() as *const libc::c_char));
                let ctx_set_padding: EvpCipherCtxSetPaddingFn = std::mem::transmute(libc::dlsym(handle, b"EVP_CIPHER_CTX_set_padding\0".as_ptr() as *const libc::c_char));
                let decrypt_update: EvpDecryptUpdateFn = std::mem::transmute(libc::dlsym(handle, b"EVP_DecryptUpdate\0".as_ptr() as *const libc::c_char));
                let decrypt_final_ex: EvpDecryptFinalExFn = std::mem::transmute(libc::dlsym(handle, b"EVP_DecryptFinal_ex\0".as_ptr() as *const libc::c_char));

                return Some(Self {
                    _handle: handle,
                    ctx_new,
                    ctx_free,
                    aes_256_ecb,
                    aes_256_cbc,
                    decrypt_init_ex,
                    ctx_set_padding,
                    decrypt_update,
                    decrypt_final_ex,
                });
            }
        }
        None
    }

    fn decrypt_aes_256_ecb(&self, key: &[u8], in_data: &[u8]) -> Option<Vec<u8>> {
        if key.len() != 32 || in_data.len() % 16 != 0 {
            return None;
        }
        unsafe {
            let ctx = (self.ctx_new)();
            if ctx.is_null() {
                return None;
            }
            let cipher = (self.aes_256_ecb)();
            if (self.decrypt_init_ex)(ctx, cipher, std::ptr::null_mut(), key.as_ptr(), std::ptr::null()) != 1 {
                (self.ctx_free)(ctx);
                return None;
            }
            (self.ctx_set_padding)(ctx, 0);

            let mut out = vec![0u8; in_data.len() + 16];
            let mut outl1 = 0i32;
            let mut outl2 = 0i32;

            if (self.decrypt_update)(ctx, out.as_mut_ptr(), &mut outl1, in_data.as_ptr(), in_data.len() as i32) != 1 {
                (self.ctx_free)(ctx);
                return None;
            }
            let _ = (self.decrypt_final_ex)(ctx, out.as_mut_ptr().add(outl1 as usize), &mut outl2);
            (self.ctx_free)(ctx);
            out.truncate((outl1 + outl2) as usize);
            Some(out)
        }
    }

    fn decrypt_aes_256_cbc(&self, key: &[u8], iv: &[u8], in_data: &[u8]) -> Option<Vec<u8>> {
        if key.len() != 32 || iv.len() != 16 || in_data.len() % 16 != 0 {
            return None;
        }
        unsafe {
            let ctx = (self.ctx_new)();
            if ctx.is_null() {
                return None;
            }
            let cipher = (self.aes_256_cbc)();
            if (self.decrypt_init_ex)(ctx, cipher, std::ptr::null_mut(), key.as_ptr(), iv.as_ptr()) != 1 {
                (self.ctx_free)(ctx);
                return None;
            }
            (self.ctx_set_padding)(ctx, 0);

            let mut out = vec![0u8; in_data.len() + 16];
            let mut outl1 = 0i32;
            let mut outl2 = 0i32;

            if (self.decrypt_update)(ctx, out.as_mut_ptr(), &mut outl1, in_data.as_ptr(), in_data.len() as i32) != 1 {
                (self.ctx_free)(ctx);
                return None;
            }
            let _ = (self.decrypt_final_ex)(ctx, out.as_mut_ptr().add(outl1 as usize), &mut outl2);
            (self.ctx_free)(ctx);
            out.truncate((outl1 + outl2) as usize);
            Some(out)
        }
    }
}

/// Rolling XOR decryption function used by SteamStub
pub fn steam_xor(data: &mut [u8]) -> u32 {
    if data.len() < 4 {
        return 0;
    }
    let mut key = u32::from_le_bytes(data[0..4].try_into().unwrap());
    for chunk in data[4..].chunks_exact_mut(4) {
        let val = u32::from_le_bytes(chunk.try_into().unwrap());
        let dec = val ^ key;
        chunk.copy_from_slice(&dec.to_le_bytes());
        key = val;
    }
    key
}

/// Checks if the image is protected by SteamStub Variant 3.1, decrypts the code section,
/// and returns the Original Entry Point (OEP) RVA.
pub fn try_unpack_steamstub(
    base_ptr: *mut u8,
    image_size: usize,
    _raw_pe_bytes: &[u8],
    ep_rva: usize,
) -> Option<usize> {
    if ep_rva < 0xF0 || ep_rva >= image_size {
        return None;
    }

    // Read 0xF0 bytes right before EntryPoint
    let mut header_bytes = [0u8; 0xF0];
    unsafe {
        std::ptr::copy_nonoverlapping(base_ptr.add(ep_rva - 0xF0), header_bytes.as_mut_ptr(), 0xF0);
    }

    steam_xor(&mut header_bytes);

    // Verify signature at offset 0x04: 0xC0DEC0DF
    let sig = u32::from_le_bytes(header_bytes[4..8].try_into().unwrap());
    println!("[STEAMSTUB CHECK] ep_rva=0x{:X}, sig=0x{:X}", ep_rva, sig);
    if sig != 0xC0DEC0DF {
        return None;
    }

    let oep = u64::from_le_bytes(header_bytes[0x20..0x28].try_into().unwrap());
    let app_id = u32::from_le_bytes(header_bytes[0x38..0x3C].try_into().unwrap());
    let code_va = u64::from_le_bytes(header_bytes[0x48..0x50].try_into().unwrap()) as usize;
    let code_raw_size = u64::from_le_bytes(header_bytes[0x50..0x58].try_into().unwrap()) as usize;

    let aes_key = &header_bytes[0x58..0x78]; // 32 bytes
    let aes_iv = &header_bytes[0x78..0x88];  // 16 bytes
    let stolen = &header_bytes[0x88..0x98];  // 16 bytes

    println!(
        "[RINE STEAMSTUB] Found SteamStub Variant 3.1 header! AppId={}, OEP=0x{:X}, CodeVA=0x{:X}, CodeSize={}",
        app_id, oep, code_va, code_raw_size
    );

    let crypto = match CryptoLib::load() {
        Some(c) => c,
        None => {
            eprintln!("[RINE STEAMSTUB] Failed to load libcrypto.so for AES decryption!");
            return None;
        }
    };

    // 1. Rebuild IV: Decrypt aes_iv with AES-256-ECB using aes_key
    let rebuilt_iv = match crypto.decrypt_aes_256_ecb(aes_key, aes_iv) {
        Some(iv) if iv.len() >= 16 => iv[0..16].to_vec(),
        _ => {
            eprintln!("[RINE STEAMSTUB] Failed to rebuild AES IV!");
            return None;
        }
    };

    // 2. Prepare code input: stolen (16 bytes) + raw code from memory at base_ptr + code_va
    if code_va + code_raw_size > image_size {
        eprintln!("[RINE STEAMSTUB] Invalid code section bounds in header!");
        return None;
    }

    let mut code_input = Vec::with_capacity(16 + code_raw_size);
    code_input.extend_from_slice(stolen);

    // Read code from raw_pe_bytes or memory
    unsafe {
        let code_slice = std::slice::from_raw_parts(base_ptr.add(code_va), code_raw_size);
        code_input.extend_from_slice(code_slice);
    }

    // 3. Decrypt code using AES-256-CBC
    let decrypted_code = match crypto.decrypt_aes_256_cbc(aes_key, &rebuilt_iv, &code_input) {
        Some(dec) => dec,
        None => {
            eprintln!("[RINE STEAMSTUB] AES-256-CBC decryption of code section failed!");
            return None;
        }
    };

    if decrypted_code.len() < code_raw_size {
        eprintln!(
            "[RINE STEAMSTUB] Decrypted code length ({}) is less than code raw size ({})!",
            decrypted_code.len(),
            code_raw_size
        );
        return None;
    }

    // 4. Overwrite code section in memory with decrypted code
    unsafe {
        std::ptr::copy_nonoverlapping(
            decrypted_code.as_ptr(),
            base_ptr.add(code_va),
            code_raw_size,
        );
    }

    println!(
        "[RINE STEAMSTUB] Successfully decrypted .text section ({} bytes)! Original Entry Point restored to 0x{:08X}",
        code_raw_size, oep
    );

    Some(oep as usize)
}
