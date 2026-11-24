//! Dedicated Rine Rust implementation module for xaudio2_7
#![allow(non_snake_case)]

pub fn stub_export() -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn XAudio2Create(
    pp_xaudio2: *mut *mut std::ffi::c_void,
    _flags: u32,
    _x_audio2_processor: u32,
) -> i32 {
    println!("[XAUDIO2] XAudio2Create called");
    if !pp_xaudio2.is_null() {
        // Return a mock COM interface so games can query interface or initialize
        *pp_xaudio2 = crate::dlls::steam::get_mock_steam_interface();
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn CreateAudioReverb(pp_effect: *mut *mut std::ffi::c_void) -> i32 {
    if !pp_effect.is_null() {
        *pp_effect = crate::dlls::steam::get_mock_steam_interface();
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn CreateAudioVolumeMeter(pp_effect: *mut *mut std::ffi::c_void) -> i32 {
    if !pp_effect.is_null() {
        *pp_effect = crate::dlls::steam::get_mock_steam_interface();
    }
    0 // S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn CreateFX(
    _clsid: *const std::ffi::c_void,
    pp_effect: *mut *mut std::ffi::c_void,
    _p_init_data: *const std::ffi::c_void,
    _init_data_byte_size: u32,
) -> i32 {
    if !pp_effect.is_null() {
        *pp_effect = crate::dlls::steam::get_mock_steam_interface();
    }
    0 // S_OK
}
