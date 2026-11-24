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

#[repr(C)]
pub struct TIMECAPS {
    pub wPeriodMin: u32,
    pub wPeriodMax: u32,
}

pub unsafe extern "win64" fn timeGetDevCaps(ptc: *mut TIMECAPS, cbtc: u32) -> u32 {
    if !ptc.is_null() && cbtc >= std::mem::size_of::<TIMECAPS>() as u32 {
        (*ptc).wPeriodMin = 1;
        (*ptc).wPeriodMax = 1000000;
        0 // TIMERR_NOERROR
    } else {
        4 // TIMERR_NOCANDO
    }
}

pub unsafe extern "win64" fn timeSetEvent(
    _u_delay: u32,
    _u_resolution: u32,
    _lp_time_proc: *const c_void,
    _dw_user: usize,
    _fu_event: u32,
) -> u32 {
    1 // Event ID
}

pub unsafe extern "win64" fn timeKillEvent(_u_timer_id: u32) -> u32 {
    0 // TIMERR_NOERROR
}

pub unsafe extern "win64" fn waveOutGetNumDevs() -> u32 {
    1
}

pub unsafe extern "win64" fn waveInGetNumDevs() -> u32 {
    0
}

pub unsafe extern "win64" fn waveOutGetDevCapsW(
    _u_device_id: usize,
    pwoc: *mut u8,
    cbwoc: u32,
) -> u32 {
    if !pwoc.is_null() && cbwoc > 0 {
        let size = (cbwoc as usize).min(std::mem::size_of::<[u8; 84]>());
        std::ptr::write_bytes(pwoc, 0, size);
        let name: Vec<u16> = "Rine WaveOut\0".encode_utf16().collect();
        let name_bytes = name.len() * 2;
        if size >= 8 + name_bytes {
            std::ptr::copy_nonoverlapping(
                name.as_ptr() as *const u8,
                pwoc.add(8),
                name_bytes,
            );
        }
    }
    0 // MMSYSERR_NOERROR
}

pub unsafe extern "win64" fn waveOutGetDevCapsA(
    _u_device_id: usize,
    pwoc: *mut u8,
    cbwoc: u32,
) -> u32 {
    if !pwoc.is_null() && cbwoc > 0 {
        let size = (cbwoc as usize).min(std::mem::size_of::<[u8; 52]>());
        std::ptr::write_bytes(pwoc, 0, size);
        let name = b"Rine WaveOut\0";
        if size >= 8 + name.len() {
            std::ptr::copy_nonoverlapping(name.as_ptr(), pwoc.add(8), name.len());
        }
    }
    0 // MMSYSERR_NOERROR
}

pub unsafe extern "win64" fn waveInGetDevCapsW(
    _u_device_id: usize,
    pwic: *mut u8,
    cbwic: u32,
) -> u32 {
    if !pwic.is_null() && cbwic > 0 {
        std::ptr::write_bytes(pwic, 0, cbwic as usize);
    }
    0
}

pub unsafe extern "win64" fn waveInGetDevCapsA(
    _u_device_id: usize,
    pwic: *mut u8,
    cbwic: u32,
) -> u32 {
    if !pwic.is_null() && cbwic > 0 {
        std::ptr::write_bytes(pwic, 0, cbwic as usize);
    }
    0
}

pub unsafe extern "win64" fn waveOutGetPosition(_hwo: u64, pmmt: *mut u8, cbmmt: u32) -> u32 {
    if !pmmt.is_null() && cbmmt >= 8 {
        std::ptr::write_bytes(pmmt, 0, cbmmt as usize);
        *(pmmt as *mut u32) = 1; // TIME_MS
        *((pmmt.add(4)) as *mut u32) = timeGetTime();
    }
    0 // MMSYSERR_NOERROR
}

pub unsafe extern "win64" fn waveInOpen(
    phwi: *mut u64,
    _u_device_id: u32,
    _pwfx: u64,
    _dw_callback: u64,
    _dw_instance: u64,
    _fdw_open: u32,
) -> u32 {
    if !phwi.is_null() {
        *phwi = 0x00060001;
    }
    0
}

pub unsafe extern "win64" fn waveInClose(_hwi: u64) -> u32 {
    0
}

pub unsafe extern "win64" fn waveInStart(_hwi: u64) -> u32 {
    0
}

pub unsafe extern "win64" fn waveInReset(_hwi: u64) -> u32 {
    0
}

pub unsafe extern "win64" fn waveInPrepareHeader(_hwi: u64, _pwh: u64, _cbwh: u32) -> u32 {
    0
}

pub unsafe extern "win64" fn waveInUnprepareHeader(_hwi: u64, _pwh: u64, _cbwh: u32) -> u32 {
    0
}

pub unsafe extern "win64" fn waveInAddBuffer(_hwi: u64, pwh: u64, _cbwh: u32) -> u32 {
    if pwh != 0 {
        let hdr = pwh as *mut WaveHdr;
        if !hdr.is_null() {
            (*hdr).dw_flags |= 0x00000001; // WHDR_DONE
        }
    }
    0
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
