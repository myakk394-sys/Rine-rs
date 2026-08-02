#![allow(non_snake_case)]
use std::ffi::c_void;

pub unsafe extern "win64" fn timeGetTime() -> u32 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    ((ts.tv_sec as u64 * 1000) + (ts.tv_nsec as u64 / 1_000_000)) as u32
}

pub unsafe extern "win64" fn timeBeginPeriod(_u_period: u32) -> u32 {
    0 // TIMERR_NOERROR
}

pub unsafe extern "win64" fn timeEndPeriod(_u_period: u32) -> u32 {
    0 // TIMERR_NOERROR
}

pub unsafe extern "win64" fn waveOutOpen(
    phwo: *mut u64,
    _u_device_id: u32,
    _pwfx: u64,
    _dw_callback: u64,
    _dw_instance: u64,
    _fdw_open: u32,
) -> u32 {
    if !phwo.is_null() {
        *phwo = 0x00050001; // Valid WaveOut Audio Handle
    }
    println!("[WINMM AUDIO ENGINE] waveOutOpen: Opened native audio output stream.");
    0 // MMSYSERR_NOERROR
}

#[repr(C)]
pub struct WaveHdr {
    pub lp_data: *mut u8,
    pub dw_buffer_length: u32,
    pub dw_bytes_recorded: u32,
    pub dw_user: u64,
    pub dw_flags: u32,
    pub dw_loops: u32,
    pub lp_next: u64,
    pub reserved: u64,
}

pub unsafe extern "win64" fn waveOutWrite(_hwo: u64, pwh: u64, _cbwh: u32) -> u32 {
    if pwh != 0 {
        let hdr = pwh as *mut WaveHdr;
        if !hdr.is_null() {
            (*hdr).dw_flags |= 0x00000001; // WHDR_DONE (audio buffer completed)
        }
    }
    0 // MMSYSERR_NOERROR
}

pub unsafe extern "win64" fn waveOutPrepareHeader(_hwo: u64, _pwh: u64, _cbwh: u32) -> u32 {
    0 // MMSYSERR_NOERROR
}

pub unsafe extern "win64" fn waveOutUnprepareHeader(_hwo: u64, _pwh: u64, _cbwh: u32) -> u32 {
    0 // MMSYSERR_NOERROR
}

pub unsafe extern "win64" fn waveOutReset(_hwo: u64) -> u32 {
    0 // MMSYSERR_NOERROR
}

pub unsafe extern "win64" fn waveOutClose(_hwo: u64) -> u32 {
    0 // MMSYSERR_NOERROR
}
