#![allow(non_snake_case)]

use std::ffi::c_void;

pub const DS_OK: i32 = 0;
pub const DSERR_INVALIDPARAM: i32 = 0x80070057_u32 as i32;

#[repr(C)]
pub struct MockComObject {
    pub lpVtbl: *const c_void,
    pub refcount: u32,
    pub extra: *mut c_void,
}

// IDirectSoundBuffer COM interface
// COM-интерфейс IDirectSoundBuffer

static mut DSOUND_BUFFER_VTBL: [*const c_void; 17] = [
    dsound_buffer_query_interface as *const c_void,
    dsound_buffer_add_ref as *const c_void,
    dsound_buffer_release as *const c_void,
    dsound_buffer_get_caps as *const c_void,
    dsound_buffer_get_current_position as *const c_void,
    dsound_buffer_set_current_position as *const c_void,
    dsound_buffer_get_status as *const c_void,
    dsound_buffer_initialize as *const c_void,
    dsound_buffer_lock as *const c_void,
    dsound_buffer_play as *const c_void,
    dsound_buffer_set_format as *const c_void,
    dsound_buffer_set_volume as *const c_void,
    dsound_buffer_set_pan as *const c_void,
    dsound_buffer_set_frequency as *const c_void,
    dsound_buffer_stop as *const c_void,
    dsound_buffer_unlock as *const c_void,
    dsound_buffer_restore as *const c_void,
];

pub unsafe extern "win64" fn create_dsound_buffer(ppv: *mut *mut c_void) -> i32 {
    if ppv.is_null() {
        return DSERR_INVALIDPARAM;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return 0x80004005_u32 as i32; // E_FAIL
    }
    (*obj).lpVtbl = DSOUND_BUFFER_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    // Allocate 64KB backing buffer for lock/unlock
    (*obj).extra = libc::calloc(1, 65536);
    *ppv = obj as *mut c_void;
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() {
        return DSERR_INVALIDPARAM;
    }
    *ppv = this;
    dsound_buffer_add_ref(this);
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn dsound_buffer_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        if !(*obj).extra.is_null() {
            libc::free((*obj).extra);
        }
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn dsound_buffer_get_caps(_this: *mut c_void, p_caps: *mut u32) -> i32 {
    if !p_caps.is_null() {
        *p_caps = 65536; // dwBufferBytes
    }
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_get_current_position(
    _this: *mut c_void,
    p_play: *mut u32,
    p_write: *mut u32,
) -> i32 {
    static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START_TIME.get_or_init(std::time::Instant::now);
    let elapsed = start.elapsed();
    // Default 44100Hz 16-bit stereo = 176400 bytes/sec
    let bytes_per_sec = 176400u64;
    let total_bytes = (elapsed.as_secs_f64() * bytes_per_sec as f64) as u64;
    let play = (total_bytes % 65536) as u32;
    let write = ((total_bytes + 4096) % 65536) as u32;
    if !p_play.is_null() {
        *p_play = play;
    }
    if !p_write.is_null() {
        *p_write = write;
    }
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_set_current_position(_this: *mut c_void, _pos: u32) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_get_status(_this: *mut c_void, p_status: *mut u32) -> i32 {
    if !p_status.is_null() {
        *p_status = 1; // DSBSTATUS_PLAYING
    }
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_initialize(
    _this: *mut c_void,
    _ds: *mut c_void,
    _desc: *const c_void,
) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_lock(
    this: *mut c_void,
    _offset: u32,
    bytes: u32,
    pp_audio1: *mut *mut c_void,
    p_bytes1: *mut u32,
    pp_audio2: *mut *mut c_void,
    p_bytes2: *mut u32,
    _flags: u32,
) -> i32 {
    let obj = this as *mut MockComObject;
    if !pp_audio1.is_null() {
        *pp_audio1 = (*obj).extra;
    }
    if !p_bytes1.is_null() {
        *p_bytes1 = bytes.min(65536);
    }
    if !pp_audio2.is_null() {
        *pp_audio2 = std::ptr::null_mut();
    }
    if !p_bytes2.is_null() {
        *p_bytes2 = 0;
    }
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_play(
    _this: *mut c_void,
    _res: u32,
    _priority: u32,
    _flags: u32,
) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_set_format(_this: *mut c_void, _fmt: *const c_void) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_set_volume(_this: *mut c_void, _vol: i32) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_set_pan(_this: *mut c_void, _pan: i32) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_set_frequency(_this: *mut c_void, _freq: u32) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_stop(_this: *mut c_void) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_unlock(
    _this: *mut c_void,
    p1: *mut c_void,
    b1: u32,
    p2: *mut c_void,
    b2: u32,
) -> i32 {
    if !p1.is_null() && b1 > 0 {
        crate::dlls::wasapi::queue_sdl_audio(p1 as *const u8, b1 as usize);
    }
    if !p2.is_null() && b2 > 0 {
        crate::dlls::wasapi::queue_sdl_audio(p2 as *const u8, b2 as usize);
    }
    DS_OK
}

unsafe extern "win64" fn dsound_buffer_restore(_this: *mut c_void) -> i32 {
    DS_OK
}

// IDirectSound8 COM interface
// COM-интерфейс IDirectSound8

static mut DSOUND8_VTBL: [*const c_void; 12] = [
    dsound8_query_interface as *const c_void,
    dsound8_add_ref as *const c_void,
    dsound8_release as *const c_void,
    dsound8_create_sound_buffer as *const c_void,
    dsound8_get_caps as *const c_void,
    dsound8_duplicate_sound_buffer as *const c_void,
    dsound8_set_cooperative_level as *const c_void,
    dsound8_compact as *const c_void,
    dsound8_get_speaker_config as *const c_void,
    dsound8_set_speaker_config as *const c_void,
    dsound8_initialize as *const c_void,
    dsound8_verify_certification as *const c_void,
];

pub unsafe extern "win64" fn create_dsound8(ppv: *mut *mut c_void) -> i32 {
    if ppv.is_null() {
        return DSERR_INVALIDPARAM;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return 0x80004005_u32 as i32;
    }
    (*obj).lpVtbl = DSOUND8_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    *ppv = obj as *mut c_void;
    println!("[DSOUND AUDIO] Created IDirectSound8 (obj=0x{:016X})", obj as usize);
    DS_OK
}

unsafe extern "win64" fn dsound8_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() {
        return DSERR_INVALIDPARAM;
    }
    *ppv = this;
    dsound8_add_ref(this);
    DS_OK
}

unsafe extern "win64" fn dsound8_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn dsound8_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn dsound8_create_sound_buffer(
    _this: *mut c_void,
    _desc: *const c_void,
    pp_buffer: *mut *mut c_void,
    _unk: *mut c_void,
) -> i32 {
    create_dsound_buffer(pp_buffer)
}

unsafe extern "win64" fn dsound8_get_caps(_this: *mut c_void, _p_caps: *mut c_void) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound8_duplicate_sound_buffer(
    _this: *mut c_void,
    _original: *mut c_void,
    pp_duplicate: *mut *mut c_void,
) -> i32 {
    create_dsound_buffer(pp_duplicate)
}

unsafe extern "win64" fn dsound8_set_cooperative_level(
    _this: *mut c_void,
    _hwnd: u64,
    _level: u32,
) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound8_compact(_this: *mut c_void) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound8_get_speaker_config(_this: *mut c_void, p_config: *mut u32) -> i32 {
    if !p_config.is_null() {
        *p_config = 2; // DSSPEAKER_STEREO
    }
    DS_OK
}

unsafe extern "win64" fn dsound8_set_speaker_config(_this: *mut c_void, _config: u32) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound8_initialize(_this: *mut c_void, _guid: *const c_void) -> i32 {
    DS_OK
}

unsafe extern "win64" fn dsound8_verify_certification(
    _this: *mut c_void,
    p_certified: *mut u32,
) -> i32 {
    if !p_certified.is_null() {
        *p_certified = 0; // DS_CERTIFIED
    }
    DS_OK
}

// DirectSound DLL exports
// Экспорты DLL DirectSound

#[no_mangle]
pub unsafe extern "win64" fn DirectSoundCreate(
    _guid: *const c_void,
    ds_out: *mut *mut c_void,
    _unk: *mut c_void,
) -> i32 {
    create_dsound8(ds_out)
}

#[no_mangle]
pub unsafe extern "win64" fn DirectSoundCreate8(
    _guid: *const c_void,
    ds_out: *mut *mut c_void,
    _unk: *mut c_void,
) -> i32 {
    create_dsound8(ds_out)
}

#[no_mangle]
pub unsafe extern "win64" fn DirectSoundEnumerateA(
    _callback: *const c_void,
    _context: *mut c_void,
) -> i32 {
    DS_OK
}

#[no_mangle]
pub unsafe extern "win64" fn DirectSoundEnumerateW(
    _callback: *const c_void,
    _context: *mut c_void,
) -> i32 {
    DS_OK
}

#[no_mangle]
pub unsafe extern "win64" fn DirectSoundCaptureCreate(
    _guid: *const c_void,
    ds_out: *mut *mut c_void,
    _unk: *mut c_void,
) -> i32 {
    create_dsound8(ds_out)
}

#[no_mangle]
pub unsafe extern "win64" fn DirectSoundCaptureCreate8(
    _guid: *const c_void,
    ds_out: *mut *mut c_void,
    _unk: *mut c_void,
) -> i32 {
    create_dsound8(ds_out)
}
