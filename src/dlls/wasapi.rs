#![allow(non_snake_case)]
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

pub type HRESULT = i32;
pub const S_OK: HRESULT = 0;
pub const E_NOINTERFACE: HRESULT = 0x8000_4002u32 as i32;
pub const E_POINTER: HRESULT = 0x8000_4003u32 as i32;
pub const E_INVALIDARG: HRESULT = 0x8007_0057u32 as i32;
pub const E_FAIL: HRESULT = 0x8000_4005u32 as i32;
pub const AUDCLNT_E_NOT_INITIALIZED: HRESULT = 0x8889_0001u32 as i32;

pub const CLSID_MM_DEVICE_ENUMERATOR: [u8; 16] = [
    0x95, 0x03, 0xDE, 0xBC, 0x2F, 0xE5, 0x7C, 0x46, 0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E,
];
pub const IID_IMM_DEVICE_ENUMERATOR: [u8; 16] = [
    0xD2, 0x64, 0x56, 0xA9, 0x14, 0x96, 0x35, 0x4F, 0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6,
];
pub const IID_IMM_DEVICE: [u8; 16] = [
    0x3F, 0x06, 0x66, 0xD6, 0x87, 0x15, 0x43, 0x4E, 0x81, 0xF1, 0xB9, 0x48, 0xE8, 0x07, 0x36, 0x3F,
];
pub const IID_IAUDIO_CLIENT: [u8; 16] = [
    0x4C, 0xAD, 0xB9, 0x1C, 0xFA, 0xDB, 0x32, 0x4C, 0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2,
];
pub const IID_IAUDIO_RENDER_CLIENT: [u8; 16] = [
    0x80, 0xAC, 0x94, 0xF2, 0x86, 0x31, 0x88, 0x42, 0x8E, 0x1D, 0x07, 0xB4, 0xE1, 0xB3, 0xE2, 0x70,
];

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WAVEFORMATEX {
    pub wFormatTag: u16,
    pub nChannels: u16,
    pub nSamplesPerSec: u32,
    pub nAvgBytesPerSec: u32,
    pub nBlockAlign: u16,
    pub wBitsPerSample: u16,
    pub cbSize: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GUID {
    pub Data1: u32,
    pub Data2: u16,
    pub Data3: u16,
    pub Data4: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WAVEFORMATEXTENSIBLE {
    pub Format: WAVEFORMATEX,
    pub Samples: u16, // wValidBitsPerSample
    pub dwChannelMask: u32,
    pub SubFormat: GUID,
}

// SDL audio host backend
// Аудио-бэкенд хоста на базе SDL2

pub static SDL_AUDIO_DEVICE_ID: AtomicU32 = AtomicU32::new(0);
pub static SDL_AUDIO_INITIALIZED: AtomicBool = AtomicBool::new(false);
static SDL_AUDIO_INIT_STARTED: AtomicBool = AtomicBool::new(false);
pub static AUDIO_IS_FLOAT: AtomicBool = AtomicBool::new(false);

pub static AUDIO_SAMPLE_RATE: AtomicU32 = AtomicU32::new(48000);
pub static AUDIO_CHANNELS: AtomicU32 = AtomicU32::new(2);
pub static AUDIO_BITS_PER_SAMPLE: AtomicU32 = AtomicU32::new(16);
pub static AUDIO_BLOCK_ALIGN: AtomicU32 = AtomicU32::new(4);
pub static AUDIO_BUFFER_SIZE_FRAMES: AtomicU32 = AtomicU32::new(2048);

pub static AUDIO_GET_BUFFER_COUNT: AtomicU64 = AtomicU64::new(0);
pub static AUDIO_RELEASE_BUFFER_COUNT: AtomicU64 = AtomicU64::new(0);
pub static AUDIO_BYTES_SUBMITTED_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static AUDIO_NONZERO_SAMPLES_COUNT: AtomicU64 = AtomicU64::new(0);
pub static AUDIO_PEAK_AMPLITUDE_MILLIS: AtomicU32 = AtomicU32::new(0);

static NEXT_AUDIO_CLIENT_ID: AtomicU64 = AtomicU64::new(1);
static AUDIO_STREAMS: Mutex<Vec<Weak<Mutex<AudioStreamState>>>> = Mutex::new(Vec::new());

pub struct AudioStreamState {
    pub id: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub block_align: u32,
    pub is_float: bool,
    pub buffer_size_frames: u32,
    pub event_handle: u64,
    pub started: bool,
    pub initialized: bool,
    pub staging_buf: Vec<u8>,
    /// Interleaved stereo f32 samples (len = queued_frames * 2) at `sample_rate`.
    pub queued_stereo_f32: VecDeque<f32>,
    pub frames_played: u64,
}

impl AudioStreamState {
    fn new(id: u64) -> Self {
        Self {
            id,
            sample_rate: 48000,
            channels: 2,
            bits_per_sample: 32,
            block_align: 8,
            is_float: true,
            buffer_size_frames: 2048,
            event_handle: 0,
            started: false,
            initialized: false,
            staging_buf: Vec::new(),
            queued_stereo_f32: VecDeque::new(),
            frames_played: 0,
        }
    }

    fn queued_frames(&self) -> u32 {
        (self.queued_stereo_f32.len() / 2) as u32
    }
}

fn register_audio_stream() -> Arc<Mutex<AudioStreamState>> {
    let id = NEXT_AUDIO_CLIENT_ID.fetch_add(1, Ordering::SeqCst);
    let state = Arc::new(Mutex::new(AudioStreamState::new(id)));
    if let Ok(mut list) = AUDIO_STREAMS.lock() {
        list.retain(|w| w.strong_count() > 0);
        list.push(Arc::downgrade(&state));
    }
    state
}

unsafe fn set_obj_stream_state(obj: *mut MockComObject, state: Option<Arc<Mutex<AudioStreamState>>>) {
    if let Some(arc) = state {
        (*obj).extra_data = Arc::into_raw(arc) as usize;
    } else {
        (*obj).extra_data = 0;
    }
}

unsafe fn get_obj_stream_state(this: *mut c_void) -> Option<Arc<Mutex<AudioStreamState>>> {
    if this.is_null() {
        return None;
    }
    let obj = this as *const MockComObject;
    let ptr = (*obj).extra_data as *const Mutex<AudioStreamState>;
    if ptr.is_null() {
        return None;
    }
    Arc::increment_strong_count(ptr);
    Some(Arc::from_raw(ptr))
}

unsafe fn drop_obj_stream_state(this: *mut c_void) {
    if this.is_null() {
        return;
    }
    let obj = this as *mut MockComObject;
    let ptr = (*obj).extra_data as *const Mutex<AudioStreamState>;
    if !ptr.is_null() {
        (*obj).extra_data = 0;
        drop(Arc::from_raw(ptr));
    }
}

pub fn get_max_queue_bytes() -> u32 {
    let rate = AUDIO_SAMPLE_RATE.load(Ordering::Relaxed).max(8000);
    let align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(1);
    let buf_frames = AUDIO_BUFFER_SIZE_FRAMES.load(Ordering::Relaxed).max(512);
    let by_buf = buf_frames * align * 4;
    let by_time = rate * align * 50 / 100; // ~500 ms
    by_buf.max(by_time).max(96000)
}

pub fn get_replenish_target_bytes() -> u32 {
    let rate = AUDIO_SAMPLE_RATE.load(Ordering::Relaxed).max(8000);
    let align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(1);
    let buf_frames = AUDIO_BUFFER_SIZE_FRAMES.load(Ordering::Relaxed).max(512);
    (buf_frames * align / 2).max(rate * align / 20) // 50 ms
}

pub fn get_sdl_audio_device_status() -> u32 {
    let dev_id = SDL_AUDIO_DEVICE_ID.load(Ordering::SeqCst);
    if dev_id == 0 {
        return 0;
    }
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if !sdl_handle.is_null() {
        unsafe {
            type SdlGetAudioDeviceStatusFn = unsafe extern "C" fn(u32) -> u32;
            let sym_status = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetAudioDeviceStatus")
                    .unwrap()
                    .as_ptr(),
            );
            if !sym_status.is_null() {
                let fn_status: SdlGetAudioDeviceStatusFn = std::mem::transmute(sym_status);
                return fn_status(dev_id);
            }
        }
    }
    0
}

pub fn get_audio_telemetry_snapshot() -> (u32, u32, u32, u32, u32, u32, u32, u64, u64, u64, u64, u32)
{
    let dev_id = SDL_AUDIO_DEVICE_ID.load(Ordering::Relaxed);
    let status = get_sdl_audio_device_status();
    let queued_bytes = get_sdl_queued_audio_bytes();
    let align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(1);
    let buf_frames = AUDIO_BUFFER_SIZE_FRAMES.load(Ordering::Relaxed).max(512);
    let queued_frames = queued_bytes / align;
    let padding_frames = queued_frames.min(buf_frames);
    let available_frames = buf_frames.saturating_sub(padding_frames);
    let get_count = AUDIO_GET_BUFFER_COUNT.load(Ordering::Relaxed);
    let rel_count = AUDIO_RELEASE_BUFFER_COUNT.load(Ordering::Relaxed);
    let bytes_sub = AUDIO_BYTES_SUBMITTED_TOTAL.load(Ordering::Relaxed);
    let nonzero_samples = AUDIO_NONZERO_SAMPLES_COUNT.load(Ordering::Relaxed);
    let peak_amp = AUDIO_PEAK_AMPLITUDE_MILLIS.load(Ordering::Relaxed);
    (
        dev_id,
        status,
        queued_bytes,
        queued_frames,
        buf_frames,
        padding_frames,
        available_frames,
        get_count,
        rel_count,
        bytes_sub,
        nonzero_samples,
        peak_amp,
    )
}

fn ensure_sdl_audio_init(sample_rate: u32, channels: u16, bits: u16, is_float: bool) {
    if SDL_AUDIO_INITIALIZED.load(Ordering::SeqCst) && SDL_AUDIO_DEVICE_ID.load(Ordering::SeqCst) > 0
    {
        return;
    }
    println!(
        "[WASAPI] ensure_sdl_audio_init: rate={} channels={} bits={} is_float={}",
        sample_rate, channels, bits, is_float
    );
    let bytes_per_sample = ((bits as u32) / 8).max(2);
    let block_align = (channels as u32) * bytes_per_sample;

    AUDIO_SAMPLE_RATE.store(sample_rate, Ordering::SeqCst);
    AUDIO_CHANNELS.store(channels as u32, Ordering::SeqCst);
    AUDIO_BITS_PER_SAMPLE.store(bits as u32, Ordering::SeqCst);
    AUDIO_BLOCK_ALIGN.store(block_align, Ordering::SeqCst);
    AUDIO_IS_FLOAT.store(is_float, Ordering::SeqCst);

    let mut sdl_handle = crate::windowing::x11::get_sdl_handle();
    if sdl_handle.is_null() {
        unsafe {
            for lib in &["libSDL2-2.0.so.0", "libSDL2-2.0.so", "libSDL2.so"] {
                let cname = std::ffi::CString::new(*lib).unwrap();
                let handle = libc::dlopen(cname.as_ptr(), libc::RTLD_LAZY | libc::RTLD_GLOBAL);
                if !handle.is_null() {
                    sdl_handle = handle;
                    break;
                }
            }
        }
    }
    if sdl_handle.is_null() {
        println!("[WASAPI AUDIO BACKEND] ERROR: get_sdl_handle() is NULL!");
        return;
    }

    unsafe {
        type SdlInitSubsystemFn = unsafe extern "C" fn(u32) -> i32;
        let sym_init = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_InitSubSystem").unwrap().as_ptr(),
        );
        if !sym_init.is_null() {
            let fn_init: SdlInitSubsystemFn = std::mem::transmute(sym_init);
            fn_init(0x00000010); // SDL_INIT_AUDIO
        }

        #[repr(C)]
        struct SdlAudioSpec {
            freq: i32,
            format: u16,
            channels: u8,
            silence: u8,
            samples: u16,
            padding: u16,
            size: u32,
            callback: *mut c_void,
            userdata: *mut c_void,
        }

        type SdlOpenAudioDeviceFn = unsafe extern "C" fn(
            *const std::ffi::c_char,
            i32,
            *const SdlAudioSpec,
            *mut SdlAudioSpec,
            i32,
        ) -> u32;

        let sym_open = libc::dlsym(
            sdl_handle,
            std::ffi::CString::new("SDL_OpenAudioDevice").unwrap().as_ptr(),
        );
        if !sym_open.is_null() {
            let fn_open: SdlOpenAudioDeviceFn = std::mem::transmute(sym_open);
            let sdl_format: u16 = if is_float && bits == 32 {
                0x8120 // AUDIO_F32SYS (32-bit float)
            } else if bits == 16 {
                0x8010 // AUDIO_S16SYS (16-bit signed integer)
            } else if bits == 32 {
                0x8020 // AUDIO_S32SYS (32-bit signed integer)
            } else {
                0x8010 // default to AUDIO_S16SYS
            };

            let desired = SdlAudioSpec {
                freq: sample_rate as i32,
                format: sdl_format,
                channels: channels as u8,
                silence: 0,
                samples: 512,
                padding: 0,
                size: 0,
                callback: std::ptr::null_mut(),
                userdata: std::ptr::null_mut(),
            };
            let mut obtained: SdlAudioSpec = std::mem::zeroed();
            let opened_id = fn_open(std::ptr::null(), 0, &desired, &mut obtained, 0);
            if opened_id > 0 {
                type SdlGetCurrentAudioDriverFn = unsafe extern "C" fn() -> *const std::ffi::c_char;
                let sym_drv = libc::dlsym(
                    sdl_handle,
                    std::ffi::CString::new("SDL_GetCurrentAudioDriver").unwrap().as_ptr(),
                );
                let drv_str = if !sym_drv.is_null() {
                    let fn_drv: SdlGetCurrentAudioDriverFn = std::mem::transmute(sym_drv);
                    let p = fn_drv();
                    if !p.is_null() {
                        std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
                    } else {
                        "unknown".into()
                    }
                } else {
                    "unknown".into()
                };
                println!(
                    "[WASAPI AUDIO BACKEND] Successfully opened SDL Audio Device id={} freq={} channels={} format=0x{:04X} driver={}",
                    opened_id, obtained.freq, obtained.channels, obtained.format, drv_str
                );
                SDL_AUDIO_DEVICE_ID.store(opened_id, Ordering::SeqCst);
                SDL_AUDIO_INITIALIZED.store(true, Ordering::SeqCst);

                type SdlPauseAudioDeviceFn = unsafe extern "C" fn(u32, i32);
                let sym_pause = libc::dlsym(
                    sdl_handle,
                    std::ffi::CString::new("SDL_PauseAudioDevice").unwrap().as_ptr(),
                );
                if !sym_pause.is_null() {
                    let fn_pause: SdlPauseAudioDeviceFn = std::mem::transmute(sym_pause);
                    fn_pause(opened_id, 0); // Unpause
                }
            } else {
                println!("[WASAPI AUDIO BACKEND] ERROR: SDL_OpenAudioDevice returned 0!");
            }
        }
    }
}

fn get_sdl_queued_audio_bytes() -> u32 {
    let dev_id = SDL_AUDIO_DEVICE_ID.load(Ordering::SeqCst);
    if dev_id == 0 {
        return 0;
    }
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if !sdl_handle.is_null() {
        unsafe {
            type SdlGetQueuedAudioSizeFn = unsafe extern "C" fn(u32) -> u32;
            let sym_queued = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_GetQueuedAudioSize")
                    .unwrap()
                    .as_ptr(),
            );
            if !sym_queued.is_null() {
                let fn_queued: SdlGetQueuedAudioSizeFn = std::mem::transmute(sym_queued);
                return fn_queued(dev_id);
            }
        }
    }
    0
}

static AUDIO_CLOCK_STARTED: AtomicBool = AtomicBool::new(false);

fn mix_and_submit_audio_tick() {
    let sdl_rate = AUDIO_SAMPLE_RATE.load(Ordering::Relaxed).max(8000);
    let sdl_align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(4);
    let max_sdl_bytes = (sdl_rate as u64 * sdl_align as u64 * 40 / 1000) as u32;
    let sdl_has_room = get_sdl_queued_audio_bytes() < max_sdl_bytes;
    let mix_frames = (sdl_rate / 100).max(1) as usize; // ~10ms (480 frames at 48kHz)

    let streams: Vec<Arc<Mutex<AudioStreamState>>> = {
        let Ok(mut guard) = AUDIO_STREAMS.lock() else {
            return;
        };
        let mut active = Vec::with_capacity(guard.len());
        guard.retain(|w| {
            if let Some(arc) = w.upgrade() {
                active.push(arc);
                true
            } else {
                false
            }
        });
        active
    };

    if streams.is_empty() {
        return;
    }

    let mut mix_buf = vec![0.0f32; mix_frames * 2];
    let mut max_drained_frames = 0usize;
    let mut any_started = false;
    let mut events_to_signal: Vec<u64> = Vec::new();

    for stream_arc in streams {
        let Ok(mut st) = stream_arc.lock() else {
            continue;
        };
        if !st.started {
            continue;
        }
        any_started = true;

        if sdl_has_room {
            let avail = st.queued_stereo_f32.len() / 2;
            if st.sample_rate == sdl_rate || st.sample_rate == 0 {
                let drain = avail.min(mix_frames);
                for i in 0..drain {
                    mix_buf[i * 2] += st.queued_stereo_f32.pop_front().unwrap_or(0.0);
                    mix_buf[i * 2 + 1] += st.queued_stereo_f32.pop_front().unwrap_or(0.0);
                }
                st.frames_played += drain as u64;
                if drain > max_drained_frames {
                    max_drained_frames = drain;
                }
            } else {
                let src_needed = ((mix_frames as u64 * st.sample_rate as u64) / sdl_rate as u64)
                    .max(1) as usize;
                let drain = avail.min(src_needed);
                if drain > 0 {
                    let mut src = Vec::with_capacity(drain * 2);
                    for _ in 0..drain {
                        src.push(st.queued_stereo_f32.pop_front().unwrap_or(0.0));
                        src.push(st.queued_stereo_f32.pop_front().unwrap_or(0.0));
                    }
                    let out_frames = ((drain as u64 * sdl_rate as u64) / st.sample_rate as u64)
                        .min(mix_frames as u64) as usize;
                    for i in 0..out_frames {
                        let pos = (i as f32) * (drain as f32) / (out_frames.max(1) as f32);
                        let idx0 = (pos as usize).min(drain - 1);
                        let idx1 = (idx0 + 1).min(drain - 1);
                        let frac = pos - (idx0 as f32);
                        let l = src[idx0 * 2] * (1.0 - frac) + src[idx1 * 2] * frac;
                        let r = src[idx0 * 2 + 1] * (1.0 - frac) + src[idx1 * 2 + 1] * frac;
                        mix_buf[i * 2] += l;
                        mix_buf[i * 2 + 1] += r;
                    }
                    st.frames_played += drain as u64;
                    if out_frames > max_drained_frames {
                        max_drained_frames = out_frames;
                    }
                }
            }
        }

        if st.event_handle != 0 && st.queued_frames() < st.buffer_size_frames {
            events_to_signal.push(st.event_handle);
        }
    }

    if any_started && sdl_has_room && max_drained_frames > 0 {
        let sample_count = max_drained_frames * 2;
        for s in &mut mix_buf[..sample_count] {
            *s = s.clamp(-1.0, 1.0);
        }
        if AUDIO_IS_FLOAT.load(Ordering::Relaxed) {
            queue_sdl_audio(mix_buf.as_ptr() as *const u8, sample_count * 4);
        } else {
            let pcm16: Vec<i16> = mix_buf[..sample_count]
                .iter()
                .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
                .collect();
            queue_sdl_audio(pcm16.as_ptr() as *const u8, pcm16.len() * 2);
        }
    }

    for h_event in events_to_signal {
        unsafe {
            crate::dlls::kernel32::SetEvent(h_event);
        }
    }
}

fn ensure_audio_clock_thread() {
    if AUDIO_CLOCK_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("rine_audio_clock".into())
        .spawn(|| {
            while AUDIO_CLOCK_STARTED.load(Ordering::Relaxed) {
                mix_and_submit_audio_tick();
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        })
        .ok();
}

pub(crate) fn queue_sdl_audio(data: *const u8, len: usize) {
    let mut dev_id = SDL_AUDIO_DEVICE_ID.load(Ordering::SeqCst);
    if dev_id == 0 {
        let rate = AUDIO_SAMPLE_RATE.load(Ordering::Relaxed).max(44100);
        let channels = AUDIO_CHANNELS.load(Ordering::Relaxed).max(2) as u16;
        let bits = AUDIO_BITS_PER_SAMPLE.load(Ordering::Relaxed).max(16) as u16;
        let is_float = AUDIO_IS_FLOAT.load(Ordering::Relaxed);
        ensure_sdl_audio_init(rate, channels, bits, is_float);
        dev_id = SDL_AUDIO_DEVICE_ID.load(Ordering::SeqCst);
        if dev_id == 0 {
            return;
        }
    }
    if data.is_null() || len == 0 {
        return;
    }
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if !sdl_handle.is_null() {
        unsafe {
            type SdlPauseAudioDeviceFn = unsafe extern "C" fn(u32, i32);
            let sym_pause = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_PauseAudioDevice").unwrap().as_ptr(),
            );
            if !sym_pause.is_null() {
                let fn_pause: SdlPauseAudioDeviceFn = std::mem::transmute(sym_pause);
                fn_pause(dev_id, 0);
            }

            type SdlQueueAudioFn = unsafe extern "C" fn(u32, *const u8, u32) -> i32;
            let sym_queue = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_QueueAudio").unwrap().as_ptr(),
            );
            if !sym_queue.is_null() {
                let fn_queue: SdlQueueAudioFn = std::mem::transmute(sym_queue);
                let vol_u32 = AUDIO_MASTER_VOLUME.load(Ordering::Relaxed);
                let res = if vol_u32 == 0 {
                    0 // Muted, skip submission
                } else if vol_u32 < 1000 {
                    let is_float = AUDIO_IS_FLOAT.load(Ordering::Relaxed);
                    if is_float && len >= 4 {
                        let float_count = len / 4;
                        let vol_scalar = vol_u32 as f32 / 1000.0;
                        let src_floats = std::slice::from_raw_parts(data as *const f32, float_count);
                        let scaled_floats: Vec<f32> = src_floats.iter().map(|&s| s * vol_scalar).collect();
                        fn_queue(dev_id, scaled_floats.as_ptr() as *const u8, len as u32)
                    } else if !is_float && len >= 2 {
                        let sample_count = len / 2;
                        let src_samples = std::slice::from_raw_parts(data as *const i16, sample_count);
                        let scaled_samples: Vec<i16> = src_samples.iter().map(|&s| {
                            ((s as i32 * vol_u32 as i32) / 1000).clamp(-32768, 32767) as i16
                        }).collect();
                        fn_queue(dev_id, scaled_samples.as_ptr() as *const u8, len as u32)
                    } else {
                        fn_queue(dev_id, data, len as u32)
                    }
                } else {
                    fn_queue(dev_id, data, len as u32)
                };
                if res == 0 {
                    AUDIO_BYTES_SUBMITTED_TOTAL.fetch_add(len as u64, Ordering::Relaxed);
                }
            }
        }
    }
}

// COM object definitions
// Описания структур COM-объектов
#[repr(C)]
pub struct MockComObject {
    pub lpVtbl: *const c_void,
    pub refcount: u32,
    pub extra_data: usize,
}

// IMMDeviceEnumerator COM interface
// COM-интерфейс IMMDeviceEnumerator

static mut MM_DEVICE_ENUMERATOR_VTBL: [*const c_void; 8] = [
    mm_enum_query_interface as *const c_void,
    mm_enum_add_ref as *const c_void,
    mm_enum_release as *const c_void,
    mm_enum_enum_audio_endpoints as *const c_void,
    mm_enum_get_default_audio_endpoint as *const c_void,
    mm_enum_get_device as *const c_void,
    mm_enum_register_endpoint_notification_callback as *const c_void,
    mm_enum_unregister_endpoint_notification_callback as *const c_void,
];

pub unsafe extern "win64" fn create_mm_device_enumerator(ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = MM_DEVICE_ENUMERATOR_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    *ppv = obj as *mut c_void;
    println!(
        "[WASAPI] CoCreateInstance(CLSID_MMDeviceEnumerator) -> S_OK (obj=0x{:016X})",
        obj as usize
    );
    S_OK
}

unsafe extern "win64" fn mm_enum_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = this;
    mm_enum_add_ref(this);
    S_OK
}

unsafe extern "win64" fn mm_enum_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn mm_enum_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        libc::free(this);
        0
    }
}

static mut MM_DEVICE_COLLECTION_VTBL: [*const c_void; 5] = [
    mm_coll_query_interface as *const c_void,
    mm_coll_add_ref as *const c_void,
    mm_coll_release as *const c_void,
    mm_coll_get_count as *const c_void,
    mm_coll_item as *const c_void,
];

unsafe extern "win64" fn mm_coll_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = this;
    mm_coll_add_ref(this);
    S_OK
}

unsafe extern "win64" fn mm_coll_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn mm_coll_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn mm_coll_get_count(_this: *mut c_void, pc_devices: *mut u32) -> HRESULT {
    if !pc_devices.is_null() {
        *pc_devices = 1;
    }
    S_OK
}

unsafe extern "win64" fn mm_coll_item(
    _this: *mut c_void,
    n_device: u32,
    pp_device: *mut *mut c_void,
) -> HRESULT {
    if n_device == 0 {
        create_mm_device(pp_device)
    } else {
        0x80070057u32 as i32 // E_INVALIDARG
    }
}

pub unsafe extern "win64" fn create_mm_device_collection(ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = MM_DEVICE_COLLECTION_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    *ppv = obj as *mut c_void;
    S_OK
}

unsafe extern "win64" fn mm_enum_enum_audio_endpoints(
    _this: *mut c_void,
    _data_flow: u32,
    _dw_state_mask: u32,
    pp_devices: *mut *mut c_void,
) -> HRESULT {
    println!("[WASAPI] IMMDeviceEnumerator::EnumAudioEndpoints");
    create_mm_device_collection(pp_devices)
}

unsafe extern "win64" fn mm_enum_get_default_audio_endpoint(
    _this: *mut c_void,
    data_flow: u32,
    role: u32,
    pp_endpoint: *mut *mut c_void,
) -> HRESULT {
    println!(
        "[WASAPI] IMMDeviceEnumerator::GetDefaultAudioEndpoint(flow={}, role={})",
        data_flow, role
    );
    create_mm_device(pp_endpoint)
}

unsafe extern "win64" fn mm_enum_get_device(
    _this: *mut c_void,
    _pwstr_id: *const u16,
    pp_device: *mut *mut c_void,
) -> HRESULT {
    create_mm_device(pp_device)
}

unsafe extern "win64" fn mm_enum_register_endpoint_notification_callback(
    _this: *mut c_void,
    _p_client: *mut c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn mm_enum_unregister_endpoint_notification_callback(
    _this: *mut c_void,
    _p_client: *mut c_void,
) -> HRESULT {
    S_OK
}

// IMMDevice COM interface
// COM-интерфейс IMMDevice

static mut MM_DEVICE_VTBL: [*const c_void; 7] = [
    mm_dev_query_interface as *const c_void,
    mm_dev_add_ref as *const c_void,
    mm_dev_release as *const c_void,
    mm_dev_activate as *const c_void,
    mm_dev_open_property_store as *const c_void,
    mm_dev_get_id as *const c_void,
    mm_dev_get_state as *const c_void,
];

pub unsafe extern "win64" fn create_mm_device(ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = MM_DEVICE_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    *ppv = obj as *mut c_void;
    println!("[WASAPI] Created IMMDevice (obj=0x{:016X})", obj as usize);
    S_OK
}

unsafe extern "win64" fn mm_dev_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = this;
    mm_dev_add_ref(this);
    S_OK
}

unsafe extern "win64" fn mm_dev_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn mm_dev_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        libc::free(this);
        0
    }
}

// IAudioEndpointVolume COM interface
// COM-интерфейс IAudioEndpointVolume

static mut AUDIO_ENDPOINT_VOLUME_VTBL: [*const c_void; 21] = [
    ep_vol_query_interface as *const c_void,
    ep_vol_add_ref as *const c_void,
    ep_vol_release as *const c_void,
    ep_vol_register_control_change_notify as *const c_void,
    ep_vol_unregister_control_change_notify as *const c_void,
    ep_vol_get_channel_count as *const c_void,
    ep_vol_set_master_volume_level as *const c_void,
    ep_vol_set_master_volume_level_scalar as *const c_void,
    ep_vol_get_master_volume_level as *const c_void,
    ep_vol_get_master_volume_level_scalar as *const c_void,
    ep_vol_set_channel_volume_level as *const c_void,
    ep_vol_set_channel_volume_level_scalar as *const c_void,
    ep_vol_get_channel_volume_level as *const c_void,
    ep_vol_get_channel_volume_level_scalar as *const c_void,
    ep_vol_set_mute as *const c_void,
    ep_vol_get_mute as *const c_void,
    ep_vol_get_volume_step_info as *const c_void,
    ep_vol_volume_step_up as *const c_void,
    ep_vol_volume_step_down as *const c_void,
    ep_vol_query_hardware_support as *const c_void,
    ep_vol_get_volume_range as *const c_void,
];

pub unsafe extern "win64" fn create_audio_endpoint_volume(ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = AUDIO_ENDPOINT_VOLUME_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    *ppv = obj as *mut c_void;
    println!("[WASAPI] Created IAudioEndpointVolume (obj=0x{:016X})", obj as usize);
    S_OK
}

unsafe extern "win64" fn ep_vol_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = this;
    ep_vol_add_ref(this);
    S_OK
}

unsafe extern "win64" fn ep_vol_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn ep_vol_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn ep_vol_register_control_change_notify(
    _this: *mut c_void,
    _p_notify: *mut c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn ep_vol_unregister_control_change_notify(
    _this: *mut c_void,
    _p_notify: *mut c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn ep_vol_get_channel_count(
    _this: *mut c_void,
    pn_channel_count: *mut u32,
) -> HRESULT {
    if !pn_channel_count.is_null() {
        *pn_channel_count = 2;
    }
    S_OK
}

unsafe extern "win64" fn ep_vol_set_master_volume_level(
    _this: *mut c_void,
    f_level_db: f32,
    _pguid_event_context: *const GUID,
) -> HRESULT {
    let scalar = if f_level_db <= -65.0 {
        0.0
    } else {
        10.0f32.powf(f_level_db / 20.0).clamp(0.0, 1.0)
    };
    AUDIO_MASTER_VOLUME.store((scalar * 1000.0) as u32, Ordering::Relaxed);
    println!("[WASAPI] IAudioEndpointVolume::SetMasterVolumeLevel: {} dB (scalar: {})", f_level_db, scalar);
    S_OK
}

unsafe extern "win64" fn ep_vol_set_master_volume_level_scalar(
    _this: *mut c_void,
    f_level: f32,
    _pguid_event_context: *const GUID,
) -> HRESULT {
    let scaled = (f_level.clamp(0.0, 1.0) * 1000.0) as u32;
    AUDIO_MASTER_VOLUME.store(scaled, Ordering::Relaxed);
    println!("[WASAPI] IAudioEndpointVolume::SetMasterVolumeLevelScalar: {}", f_level);
    S_OK
}

unsafe extern "win64" fn ep_vol_get_master_volume_level(
    _this: *mut c_void,
    pf_level_db: *mut f32,
) -> HRESULT {
    if !pf_level_db.is_null() {
        let s = AUDIO_MASTER_VOLUME.load(Ordering::Relaxed) as f32 / 1000.0;
        *pf_level_db = if s > 0.0001 { 20.0 * s.log10() } else { -65.0 };
    }
    S_OK
}

unsafe extern "win64" fn ep_vol_get_master_volume_level_scalar(
    _this: *mut c_void,
    pf_level: *mut f32,
) -> HRESULT {
    if !pf_level.is_null() {
        let scaled = AUDIO_MASTER_VOLUME.load(Ordering::Relaxed) as f32;
        *pf_level = (scaled / 1000.0).clamp(0.0, 1.0);
    }
    S_OK
}

unsafe extern "win64" fn ep_vol_set_channel_volume_level(
    _this: *mut c_void,
    _n_channel: u32,
    _f_level_db: f32,
    _pguid_event_context: *const GUID,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn ep_vol_set_channel_volume_level_scalar(
    _this: *mut c_void,
    _n_channel: u32,
    _f_level: f32,
    _pguid_event_context: *const GUID,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn ep_vol_get_channel_volume_level(
    _this: *mut c_void,
    _n_channel: u32,
    pf_level_db: *mut f32,
) -> HRESULT {
    if !pf_level_db.is_null() {
        *pf_level_db = 0.0;
    }
    S_OK
}

unsafe extern "win64" fn ep_vol_get_channel_volume_level_scalar(
    _this: *mut c_void,
    _n_channel: u32,
    pf_level: *mut f32,
) -> HRESULT {
    if !pf_level.is_null() {
        *pf_level = 1.0;
    }
    S_OK
}

unsafe extern "win64" fn ep_vol_set_mute(
    _this: *mut c_void,
    _b_mute: i32,
    _pguid_event_context: *const GUID,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn ep_vol_get_mute(_this: *mut c_void, pb_mute: *mut i32) -> HRESULT {
    if !pb_mute.is_null() {
        *pb_mute = 0;
    }
    S_OK
}

unsafe extern "win64" fn ep_vol_get_volume_step_info(
    _this: *mut c_void,
    pn_step: *mut u32,
    pn_step_count: *mut u32,
) -> HRESULT {
    if !pn_step.is_null() {
        *pn_step = 0;
    }
    if !pn_step_count.is_null() {
        *pn_step_count = 100;
    }
    S_OK
}

unsafe extern "win64" fn ep_vol_volume_step_up(
    _this: *mut c_void,
    _pguid_event_context: *const GUID,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn ep_vol_volume_step_down(
    _this: *mut c_void,
    _pguid_event_context: *const GUID,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn ep_vol_query_hardware_support(
    _this: *mut c_void,
    pdw_hardware_support_mask: *mut u32,
) -> HRESULT {
    if !pdw_hardware_support_mask.is_null() {
        *pdw_hardware_support_mask = 0;
    }
    S_OK
}

unsafe extern "win64" fn ep_vol_get_volume_range(
    _this: *mut c_void,
    pfl_volume_mindb: *mut f32,
    pfl_volume_maxdb: *mut f32,
    pfl_volume_incrementdb: *mut f32,
) -> HRESULT {
    if !pfl_volume_mindb.is_null() {
        *pfl_volume_mindb = -65.0;
    }
    if !pfl_volume_maxdb.is_null() {
        *pfl_volume_maxdb = 0.0;
    }
    if !pfl_volume_incrementdb.is_null() {
        *pfl_volume_incrementdb = 0.5;
    }
    S_OK
}

unsafe extern "win64" fn mm_dev_activate(
    _this: *mut c_void,
    riid: *const u8,
    _dw_cls_ctx: u32,
    _p_activation_params: *mut c_void,
    pp_interface: *mut *mut c_void,
) -> HRESULT {
    if pp_interface.is_null() {
        return E_POINTER;
    }
    if !riid.is_null() {
        let g = *(riid as *const GUID);
        if g.Data1 == 0x5CDF2C82 || g.Data1 == 0x5BC64874 {
            println!("[WASAPI] IMMDevice::Activate(IAudioEndpointVolume)");
            return create_audio_endpoint_volume(pp_interface);
        }
        if g.Data1 == 0x87CE5498 {
            println!("[WASAPI] IMMDevice::Activate(ISimpleAudioVolume)");
            return create_simple_audio_volume(pp_interface);
        }
        if g.Data1 == 0x7ED4EE07 {
            println!("[WASAPI] IMMDevice::Activate(IAudioClient3) -> E_NOINTERFACE (triggering graceful fallback to IAudioClient)");
            return E_NOINTERFACE;
        }
        if g.Data1 != 0x1CB9AD4C && g.Data1 != 0x726778CD {
            println!("[WASAPI] IMMDevice::Activate(unknown IID Data1=0x{:08X}) -> E_NOINTERFACE", g.Data1);
            *pp_interface = std::ptr::null_mut();
            return E_NOINTERFACE;
        }
    }
    println!("[WASAPI] IMMDevice::Activate(IAudioClient)");
    create_audio_client(pp_interface)
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PROPERTYKEY {
    pub fmtid: [u8; 16],
    pub pid: u32,
}

static mut PROP_STORE_VTBL: [*const c_void; 8] = [
    prop_store_query_interface as *const c_void,
    prop_store_add_ref as *const c_void,
    prop_store_release as *const c_void,
    prop_store_get_count as *const c_void,
    prop_store_get_at as *const c_void,
    prop_store_get_value as *const c_void,
    prop_store_set_value as *const c_void,
    prop_store_commit as *const c_void,
];

unsafe extern "win64" fn prop_store_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = this;
    prop_store_add_ref(this);
    S_OK
}

unsafe extern "win64" fn prop_store_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn prop_store_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn prop_store_get_count(_this: *mut c_void, c_props: *mut u32) -> HRESULT {
    if !c_props.is_null() {
        *c_props = 2;
    }
    S_OK
}

unsafe extern "win64" fn prop_store_get_at(
    _this: *mut c_void,
    _i_prop: u32,
    _pkey: *mut PROPERTYKEY,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn prop_store_get_value(
    _this: *mut c_void,
    key: *const PROPERTYKEY,
    pv: *mut c_void,
) -> HRESULT {
    if pv.is_null() {
        return E_POINTER;
    }
    std::ptr::write_bytes(pv as *mut u8, 0, 24);

    if !key.is_null() {
        let pid = (*key).pid;
        let data1 = u32::from_le_bytes([
            (*key).fmtid[0],
            (*key).fmtid[1],
            (*key).fmtid[2],
            (*key).fmtid[3],
        ]);
        if pid == 14 {
            // PKEY_Device_FriendlyName (pid 14) -> VT_LPWSTR (31)
            let name: Vec<u16> = "Rine Virtual Audio Device\0".encode_utf16().collect();
            let bytes = name.len() * 2;
            let mem = libc::calloc(1, bytes) as *mut u16;
            if !mem.is_null() {
                std::ptr::copy_nonoverlapping(name.as_ptr(), mem, name.len());
                *(pv as *mut u16) = 31; // VT_LPWSTR
                *(((pv as *mut u8).add(8)) as *mut *mut u16) = mem;
            }
            return S_OK;
        } else if data1 == 0xf19f_064d && pid == 0 {
            // PKEY_AudioEngine_DeviceFormat -> VT_BLOB (65) with valid 18-byte WAVEFORMATEX
            let wfx = libc::calloc(1, 18) as *mut u8;
            if !wfx.is_null() {
                *(wfx.add(0) as *mut u16) = 1; // WAVE_FORMAT_PCM
                *(wfx.add(2) as *mut u16) = 2; // nChannels
                *(wfx.add(4) as *mut u32) = 48000; // nSamplesPerSec
                *(wfx.add(8) as *mut u32) = 192000; // nAvgBytesPerSec
                *(wfx.add(12) as *mut u16) = 4; // nBlockAlign
                *(wfx.add(14) as *mut u16) = 16; // wBitsPerSample
                *(wfx.add(16) as *mut u16) = 0; // cbSize
                *(pv as *mut u16) = 65; // VT_BLOB
                *((pv as *mut u8).add(8) as *mut u32) = 18; // cbSize
                *((pv as *mut u8).add(16) as *mut *mut u8) = wfx; // pBlobData
                return S_OK;
            }
        } else if data1 == 0x1da5_d803 && pid == 0 {
            // PKEY_AudioEndpoint_FormFactor (pid 0) -> VT_UI4 (19)
            *(pv as *mut u16) = 19; // VT_UI4
            *(((pv as *mut u8).add(8)) as *mut u32) = 0; // Speakers
            return S_OK;
        }
    }

    *(pv as *mut u16) = 0; // VT_EMPTY
    S_OK
}

unsafe extern "win64" fn prop_store_set_value(
    _this: *mut c_void,
    _key: *const PROPERTYKEY,
    _pv: *const c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn prop_store_commit(_this: *mut c_void) -> HRESULT {
    S_OK
}

pub unsafe extern "win64" fn create_property_store(ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = PROP_STORE_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    *ppv = obj as *mut c_void;
    S_OK
}

unsafe extern "win64" fn mm_dev_open_property_store(
    _this: *mut c_void,
    _stgm_access: u32,
    pp_properties: *mut *mut c_void,
) -> HRESULT {
    create_property_store(pp_properties)
}

unsafe extern "win64" fn mm_dev_get_id(_this: *mut c_void, ppstr_id: *mut *mut u16) -> HRESULT {
    if !ppstr_id.is_null() {
        let name: Vec<u16> = "Rine Audio Output Device\0".encode_utf16().collect();
        let bytes = name.len() * 2;
        let mem = libc::calloc(1, bytes) as *mut u16;
        if !mem.is_null() {
            std::ptr::copy_nonoverlapping(name.as_ptr(), mem, name.len());
            *ppstr_id = mem;
        }
    }
    S_OK
}

unsafe extern "win64" fn mm_dev_get_state(_this: *mut c_void, pdw_state: *mut u32) -> HRESULT {
    if !pdw_state.is_null() {
        *pdw_state = 1; // DEVICE_STATE_ACTIVE
    }
    S_OK
}

// IAudioClient COM interface
// COM-интерфейс IAudioClient

static mut AUDIO_CLIENT_VTBL: [*const c_void; 21] = [
    audio_client_query_interface as *const c_void,
    audio_client_add_ref as *const c_void,
    audio_client_release as *const c_void,
    audio_client_initialize as *const c_void,
    audio_client_get_buffer_size as *const c_void,
    audio_client_get_stream_latency as *const c_void,
    audio_client_get_current_padding as *const c_void,
    audio_client_is_format_supported as *const c_void,
    audio_client_get_mix_format as *const c_void,
    audio_client_get_device_period as *const c_void,
    audio_client_start as *const c_void,
    audio_client_stop as *const c_void,
    audio_client_reset as *const c_void,
    audio_client_set_event_handle as *const c_void,
    audio_client_get_service as *const c_void,
    audio_client_is_offload_capable as *const c_void,
    audio_client_set_client_properties as *const c_void,
    audio_client_get_buffer_size_limits as *const c_void,
    audio_client_get_shared_mode_engine_period as *const c_void,
    audio_client_get_current_shared_mode_engine_period as *const c_void,
    audio_client_initialize_shared_audio_stream as *const c_void,
];

pub unsafe extern "win64" fn create_audio_client(ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    let state = register_audio_stream();
    let client_id = state.lock().map(|s| s.id).unwrap_or(0);
    (*obj).lpVtbl = AUDIO_CLIENT_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    set_obj_stream_state(obj, Some(state));
    *ppv = obj as *mut c_void;
    println!(
        "[WASAPI] Created IAudioClient #{} (obj=0x{:016X})",
        client_id, obj as usize
    );
    S_OK
}

unsafe extern "win64" fn audio_client_query_interface(
    this: *mut c_void,
    riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    if !riid.is_null() {
        let g = *(riid as *const GUID);
        if g.Data1 == 0x7ED4EE07 {
            return E_NOINTERFACE;
        }
        if g.Data1 == 0x5BC64874 || g.Data1 == 0x5CDF2C82 {
            return create_audio_endpoint_volume(ppv);
        }
        if g.Data1 == 0x87CE5498 {
            return create_simple_audio_volume(ppv);
        }
    }
    *ppv = this;
    audio_client_add_ref(this);
    S_OK
}

unsafe extern "win64" fn audio_client_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn audio_client_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        drop_obj_stream_state(this);
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn audio_client_initialize(
    this: *mut c_void,
    share_mode: u32,
    stream_flags: u32,
    hns_buffer_duration: i64,
    _hns_periodicity: i64,
    p_format: *const WAVEFORMATEX,
    _audio_session_guid: *const GUID,
) -> HRESULT {
    let (rate, channels, bits, block_align, is_float) = if !p_format.is_null() {
        let tag = (*p_format).wFormatTag;
        let subformat_data1 = if tag == 0xFFFE {
            // In Windows WAVEFORMATEXTENSIBLE, SubFormat GUID Data1 is at exact byte offset 24
            let raw = p_format as *const u8;
            std::ptr::read_unaligned(raw.add(24) as *const u32)
        } else {
            0
        };
        let is_float = tag == 0x0003 || subformat_data1 == 0x00000003 || (tag == 0xFFFE && (*p_format).wBitsPerSample == 32);
        let bits = (*p_format).wBitsPerSample;
        let rate = (*p_format).nSamplesPerSec;
        let channels = (*p_format).nChannels;
        if rate < 8000 || rate > 384_000 || channels == 0 || channels > 32 || bits == 0 || bits > 64 {
            (48000, 2, 16, 4, false)
        } else {
            let align = if (*p_format).nBlockAlign > 0 && (*p_format).nBlockAlign <= 256 {
                (*p_format).nBlockAlign as u32
            } else {
                (channels as u32) * ((bits as u32) / 8).max(2)
            };
            (rate, channels, bits, align, is_float)
        }
    } else {
        (48000, 2, 16, 4, false)
    };
    let requested_frames = (hns_buffer_duration.clamp(0, 10_000_000) * (rate as i64) / 10_000_000).max(2048) as u32;
    let buffer_frames = requested_frames.clamp(2048, 48000 * 4);

    let mut client_id = 0u64;
    if let Some(state_arc) = get_obj_stream_state(this) {
        if let Ok(mut st) = state_arc.lock() {
            client_id = st.id;
            st.sample_rate = rate;
            st.channels = channels;
            st.bits_per_sample = bits;
            st.block_align = block_align;
            st.is_float = is_float;
            st.buffer_size_frames = buffer_frames;
            st.staging_buf = vec![0u8; (buffer_frames as usize) * (block_align as usize)];
            st.queued_stereo_f32.clear();
            st.frames_played = 0;
            st.initialized = true;
        }
    }
    println!(
        "[WASAPI] IAudioClient #{}::Initialize(shareMode={}, flags=0x{:X}, duration={}, rate={}, channels={}, bits={}, align={}, isFloat={}, bufFrames={})",
        client_id, share_mode, stream_flags, hns_buffer_duration, rate, channels, bits, block_align, is_float, buffer_frames
    );
    // Open the single shared SDL2 mixer device once (48kHz, stereo, 32-bit float) if not yet initialized.
    if !SDL_AUDIO_INITIALIZED.load(Ordering::SeqCst) {
        ensure_sdl_audio_init(48000, 2, 32, true);
    }
    AUDIO_BUFFER_SIZE_FRAMES.store(buffer_frames, Ordering::SeqCst);
    S_OK
}

unsafe extern "win64" fn audio_client_get_buffer_size(
    this: *mut c_void,
    p_num_buffer_frames: *mut u32,
) -> HRESULT {
    if !p_num_buffer_frames.is_null() {
        let frames = get_obj_stream_state(this)
            .and_then(|arc| arc.lock().ok().map(|st| st.buffer_size_frames.max(2048)))
            .unwrap_or_else(|| AUDIO_BUFFER_SIZE_FRAMES.load(Ordering::Relaxed).max(2048));
        *p_num_buffer_frames = frames;
    }
    S_OK
}

unsafe extern "win64" fn audio_client_get_stream_latency(
    _this: *mut c_void,
    phns_latency: *mut i64,
) -> HRESULT {
    if !phns_latency.is_null() {
        *phns_latency = 200000; // 20ms
    }
    S_OK
}

unsafe extern "win64" fn audio_client_get_current_padding(
    this: *mut c_void,
    p_num_padding_frames: *mut u32,
) -> HRESULT {
    if !p_num_padding_frames.is_null() {
        let (client_id, queued_frames, buf_frames) = if let Some(state_arc) = get_obj_stream_state(this) {
            if let Ok(st) = state_arc.lock() {
                (st.id, st.queued_frames(), st.buffer_size_frames.max(512))
            } else {
                (0, 0, 2048)
            }
        } else {
            (0, 0, AUDIO_BUFFER_SIZE_FRAMES.load(Ordering::Relaxed).max(512))
        };
        *p_num_padding_frames = queued_frames.min(buf_frames);
        static PAD_C: AtomicU64 = AtomicU64::new(0);
        let c = PAD_C.fetch_add(1, Ordering::Relaxed);
        if c < 20 || c % 100 == 0 {
            println!(
                "[WASAPI] GetCurrentPadding #{} (client={}) -> queued_frames={}, padding={}",
                c, client_id, queued_frames, *p_num_padding_frames
            );
        }
    }
    S_OK
}

unsafe extern "win64" fn audio_client_is_format_supported(
    _this: *mut c_void,
    share_mode: u32,
    p_format: *const WAVEFORMATEX,
    pp_closest_match: *mut *mut WAVEFORMATEX,
) -> HRESULT {
    println!(
        "[WASAPI] IAudioClient::IsFormatSupported(shareMode={}, pFormat=0x{:X}) -> S_OK",
        share_mode, p_format as usize
    );
    if !pp_closest_match.is_null() {
        *pp_closest_match = std::ptr::null_mut();
    }
    S_OK
}

unsafe extern "win64" fn audio_client_get_mix_format(
    _this: *mut c_void,
    pp_device_format: *mut *mut WAVEFORMATEX,
) -> HRESULT {
    if pp_device_format.is_null() {
        return E_POINTER;
    }
    // Allocate 40 bytes for standard Windows WAVEFORMATEXTENSIBLE
    let size = 40usize;
    let mem = libc::calloc(1, size) as *mut u8;
    if mem.is_null() {
        return E_FAIL;
    }
    let wfx = mem as *mut WAVEFORMATEX;
    (*wfx).wFormatTag = 0xFFFE; // WAVE_FORMAT_EXTENSIBLE
    (*wfx).nChannels = 2;
    (*wfx).nSamplesPerSec = 48000;
    (*wfx).wBitsPerSample = 32;
    (*wfx).nBlockAlign = 8;
    (*wfx).nAvgBytesPerSec = 48000 * 8; // 384000
    (*wfx).cbSize = 22; // 22 bytes follow WAVEFORMATEX

    *(mem.add(18) as *mut u16) = 32; // wValidBitsPerSample
    *(mem.add(20) as *mut u32) = 0x00000003; // SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT
    let subformat = GUID {
        Data1: 0x00000003, // KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
        Data2: 0x0000,
        Data3: 0x0010,
        Data4: [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
    };
    std::ptr::write_unaligned(mem.add(24) as *mut GUID, subformat);

    *pp_device_format = wfx;
    println!("[WASAPI] IAudioClient::GetMixFormat -> 48000Hz 32-bit Float Stereo (WAVE_FORMAT_EXTENSIBLE)");
    S_OK
}

unsafe extern "win64" fn audio_client_get_device_period(
    _this: *mut c_void,
    phns_default: *mut i64,
    phns_minimum: *mut i64,
) -> HRESULT {
    if !phns_default.is_null() {
        *phns_default = 100000; // 10ms
    }
    if !phns_minimum.is_null() {
        *phns_minimum = 50000; // 5ms
    }
    S_OK
}

unsafe extern "win64" fn audio_client_start(this: *mut c_void) -> HRESULT {
    let mut client_id = 0u64;
    let mut signal_event = 0u64;
    if let Some(state_arc) = get_obj_stream_state(this) {
        if let Ok(mut st) = state_arc.lock() {
            client_id = st.id;
            st.started = true;
            if st.event_handle != 0 && st.queued_frames() < st.buffer_size_frames {
                signal_event = st.event_handle;
            }
        }
    }
    println!("[WASAPI] IAudioClient #{}::Start()", client_id);
    if !SDL_AUDIO_INITIALIZED.load(Ordering::SeqCst) {
        ensure_sdl_audio_init(48000, 2, 32, true);
    }
    let dev_id = SDL_AUDIO_DEVICE_ID.load(Ordering::SeqCst);
    if dev_id > 0 {
        let sdl_handle = crate::windowing::x11::get_sdl_handle();
        if !sdl_handle.is_null() {
            type SdlPauseAudioDeviceFn = unsafe extern "C" fn(u32, i32);
            let sym_pause = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_PauseAudioDevice").unwrap().as_ptr(),
            );
            if !sym_pause.is_null() {
                let fn_pause: SdlPauseAudioDeviceFn = std::mem::transmute(sym_pause);
                fn_pause(dev_id, 0); // 0 = unpause
            }
        }
    }
    ensure_audio_clock_thread();
    if signal_event != 0 {
        crate::dlls::kernel32::SetEvent(signal_event);
    }
    S_OK
}

unsafe extern "win64" fn audio_client_stop(this: *mut c_void) -> HRESULT {
    if let Some(state_arc) = get_obj_stream_state(this) {
        if let Ok(mut st) = state_arc.lock() {
            st.started = false;
            println!("[WASAPI] IAudioClient #{}::Stop()", st.id);
        }
    }
    S_OK
}

unsafe extern "win64" fn audio_client_reset(this: *mut c_void) -> HRESULT {
    if let Some(state_arc) = get_obj_stream_state(this) {
        if let Ok(mut st) = state_arc.lock() {
            st.queued_stereo_f32.clear();
            st.frames_played = 0;
            println!("[WASAPI] IAudioClient #{}::Reset()", st.id);
        }
    }
    S_OK
}

unsafe extern "win64" fn audio_client_set_event_handle(
    this: *mut c_void,
    event_handle: u64,
) -> HRESULT {
    let mut client_id = 0u64;
    if let Some(state_arc) = get_obj_stream_state(this) {
        if let Ok(mut st) = state_arc.lock() {
            client_id = st.id;
            st.event_handle = event_handle;
        }
    }
    println!(
        "[WASAPI] IAudioClient #{}::SetEventHandle(0x{:X})",
        client_id, event_handle
    );
    ensure_audio_clock_thread();
    S_OK
}

unsafe extern "win64" fn audio_client_get_service(
    this: *mut c_void,
    riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let state = get_obj_stream_state(this);
    if riid.is_null() {
        return create_audio_render_client_with_state(ppv, state);
    }
    let g = *(riid as *const GUID);
    println!("[WASAPI] IAudioClient::GetService(GUID: 0x{:08X})", g.Data1);
    match g.Data1 {
        0xF294ACFC => {
            // IID_IAudioRenderClient
            create_audio_render_client_with_state(ppv, state)
        }
        0xCD63314F | 0x6F49FF73 => {
            // IID_IAudioClock, IID_IAudioClock2
            create_audio_clock_with_state(ppv, state)
        }
        0x87CE5498 => {
            // IID_ISimpleAudioVolume
            create_simple_audio_volume(ppv)
        }
        0xF4B1A599 | 0xBFB7FF88 => {
            // IID_IAudioSessionControl, IID_IAudioSessionControl2
            create_audio_session_control_with_state(ppv, state)
        }
        _ => {
            create_audio_render_client_with_state(ppv, state)
        }
    }
}

unsafe extern "win64" fn audio_client_is_offload_capable(
    _this: *mut c_void,
    _category: u32,
    pb_offload_capable: *mut i32,
) -> HRESULT {
    if !pb_offload_capable.is_null() {
        *pb_offload_capable = 0; // FALSE
    }
    S_OK
}

unsafe extern "win64" fn audio_client_set_client_properties(
    _this: *mut c_void,
    _p_properties: *const c_void,
) -> HRESULT {
    println!("[WASAPI] IAudioClient2::SetClientProperties -> S_OK");
    S_OK
}

unsafe extern "win64" fn audio_client_get_buffer_size_limits(
    _this: *mut c_void,
    _p_format: *const WAVEFORMATEX,
    _b_event_driven: i32,
    phns_min_buffer_duration: *mut i64,
    phns_max_buffer_duration: *mut i64,
) -> HRESULT {
    if !phns_min_buffer_duration.is_null() {
        *phns_min_buffer_duration = 50000;
    }
    if !phns_max_buffer_duration.is_null() {
        *phns_max_buffer_duration = 5000000;
    }
    S_OK
}

unsafe extern "win64" fn audio_client_get_shared_mode_engine_period(
    _this: *mut c_void,
    _p_format: *const WAVEFORMATEX,
    p_default_period_in_frames: *mut u32,
    p_fundamental_period_in_frames: *mut u32,
    p_min_period_in_frames: *mut u32,
    p_max_period_in_frames: *mut u32,
) -> HRESULT {
    println!("[WASAPI] IAudioClient3::GetSharedModeEnginePeriod");
    if !p_default_period_in_frames.is_null() {
        *p_default_period_in_frames = 480;
    }
    if !p_fundamental_period_in_frames.is_null() {
        *p_fundamental_period_in_frames = 480;
    }
    if !p_min_period_in_frames.is_null() {
        *p_min_period_in_frames = 128;
    }
    if !p_max_period_in_frames.is_null() {
        *p_max_period_in_frames = 4096;
    }
    S_OK
}

unsafe extern "win64" fn audio_client_get_current_shared_mode_engine_period(
    this: *mut c_void,
    pp_format: *mut *mut WAVEFORMATEX,
    p_current_period_in_frames: *mut u32,
) -> HRESULT {
    println!("[WASAPI] IAudioClient3::GetCurrentSharedModeEnginePeriod");
    if !p_current_period_in_frames.is_null() {
        *p_current_period_in_frames = 480;
    }
    if !pp_format.is_null() {
        audio_client_get_mix_format(this, pp_format);
    }
    S_OK
}

unsafe extern "win64" fn audio_client_initialize_shared_audio_stream(
    this: *mut c_void,
    stream_flags: u32,
    period_in_frames: u32,
    p_format: *const WAVEFORMATEX,
    audio_session_guid: *const GUID,
) -> HRESULT {
    println!(
        "[WASAPI] IAudioClient3::InitializeSharedAudioStream(flags=0x{:X}, period={})",
        stream_flags, period_in_frames
    );
    audio_client_initialize(
        this,
        0, // AUDCLNT_SHAREMODE_SHARED
        stream_flags,
        (period_in_frames as i64) * 10000000 / 48000,
        0,
        p_format,
        audio_session_guid,
    )
}

// IAudioRenderClient COM interface
// COM-интерфейс IAudioRenderClient

static mut AUDIO_RENDER_CLIENT_VTBL: [*const c_void; 5] = [
    audio_render_query_interface as *const c_void,
    audio_render_add_ref as *const c_void,
    audio_render_release as *const c_void,
    audio_render_get_buffer as *const c_void,
    audio_render_release_buffer as *const c_void,
];

pub unsafe extern "win64" fn create_audio_render_client(ppv: *mut *mut c_void) -> HRESULT {
    create_audio_render_client_with_state(ppv, Some(register_audio_stream()))
}

unsafe fn create_audio_render_client_with_state(
    ppv: *mut *mut c_void,
    state: Option<Arc<Mutex<AudioStreamState>>>,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = AUDIO_RENDER_CLIENT_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    set_obj_stream_state(obj, state);
    *ppv = obj as *mut c_void;
    println!(
        "[WASAPI] Created IAudioRenderClient (obj=0x{:016X})",
        obj as usize
    );
    S_OK
}

unsafe extern "win64" fn audio_render_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = this;
    audio_render_add_ref(this);
    S_OK
}

unsafe extern "win64" fn audio_render_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn audio_render_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        drop_obj_stream_state(this);
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn audio_render_get_buffer(
    this: *mut c_void,
    num_frames_requested: u32,
    pp_data: *mut *mut u8,
) -> HRESULT {
    if pp_data.is_null() {
        return E_POINTER;
    }
    let get_c = AUDIO_GET_BUFFER_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    if get_c <= 10 || get_c % 100 == 0 {
        println!(
            "[WASAPI AUDIO BACKEND] GetBuffer #{} requested_frames={}",
            get_c, num_frames_requested
        );
    }
    let Some(state_arc) = get_obj_stream_state(this) else {
        return E_FAIL;
    };
    let Ok(mut st) = state_arc.lock() else {
        return E_FAIL;
    };
    let align = st.block_align.max(1) as usize;
    let needed_bytes = (num_frames_requested as usize) * align;
    let min_bytes = needed_bytes.max(align);
    if st.staging_buf.len() < min_bytes {
        st.staging_buf.resize(min_bytes, 0);
    }
    *pp_data = st.staging_buf.as_mut_ptr();
    S_OK
}

fn read_sample_as_f32(frame_bytes: &[u8], ch_idx: usize, bits: u16, is_float: bool) -> f32 {
    if is_float && bits == 32 {
        let off = ch_idx * 4;
        if off + 4 <= frame_bytes.len() {
            return f32::from_le_bytes([
                frame_bytes[off],
                frame_bytes[off + 1],
                frame_bytes[off + 2],
                frame_bytes[off + 3],
            ]);
        }
    } else if !is_float && bits == 16 {
        let off = ch_idx * 2;
        if off + 2 <= frame_bytes.len() {
            let s = i16::from_le_bytes([frame_bytes[off], frame_bytes[off + 1]]);
            return s as f32 / 32768.0;
        }
    } else if !is_float && bits == 32 {
        let off = ch_idx * 4;
        if off + 4 <= frame_bytes.len() {
            let s = i32::from_le_bytes([
                frame_bytes[off],
                frame_bytes[off + 1],
                frame_bytes[off + 2],
                frame_bytes[off + 3],
            ]);
            return s as f32 / 2147483648.0;
        }
    } else if !is_float && bits == 24 {
        let off = ch_idx * 3;
        if off + 3 <= frame_bytes.len() {
            let raw = ((frame_bytes[off] as i32) << 8)
                | ((frame_bytes[off + 1] as i32) << 16)
                | ((frame_bytes[off + 2] as i32) << 24);
            return (raw >> 8) as f32 / 8388608.0;
        }
    } else if !is_float && bits == 8 {
        if ch_idx < frame_bytes.len() {
            return (frame_bytes[ch_idx] as f32 - 128.0) / 128.0;
        }
    }
    0.0
}

unsafe extern "win64" fn audio_render_release_buffer(
    this: *mut c_void,
    num_frames_written: u32,
    dw_flags: u32,
) -> HRESULT {
    let rel_c = AUDIO_RELEASE_BUFFER_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    static LAST_WASAPI_FLAGS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0xFFFF_FFFF);
    let prev_flags = LAST_WASAPI_FLAGS.swap(dw_flags, Ordering::Relaxed);
    if rel_c <= 10 || rel_c % 100 == 0 || (prev_flags != dw_flags && rel_c <= 50) {
        println!(
            "[WASAPI AUDIO BACKEND] ReleaseBuffer #{} written_frames={} flags=0x{:X} (prev=0x{:X})",
            rel_c, num_frames_written, dw_flags, prev_flags
        );
    }
    let Some(state_arc) = get_obj_stream_state(this) else {
        return E_FAIL;
    };
    let Ok(mut st) = state_arc.lock() else {
        return E_FAIL;
    };

    if num_frames_written > 0 {
        let capacity_frames = st.buffer_size_frames as usize;
        let current_frames = st.queued_frames() as usize;
        let free_frames = capacity_frames.saturating_sub(current_frames);
        let frames_to_push = (num_frames_written as usize).min(free_frames);

        const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 0x2;
        if (dw_flags & AUDCLNT_BUFFERFLAGS_SILENT) != 0 {
            st.queued_stereo_f32
                .extend(std::iter::repeat(0.0f32).take(frames_to_push * 2));
        } else {
            let align = st.block_align.max(1) as usize;
            let channels = st.channels.max(1) as usize;
            let bits = st.bits_per_sample;
            let is_float = st.is_float;
            let avail_buf_frames = st.staging_buf.len() / align;
            let actual_frames = frames_to_push.min(avail_buf_frames);

            let mut max_amp: f32 = 0.0;
            let mut nonzero = 0u64;

            for f in 0..actual_frames {
                let frame_slice = &st.staging_buf[f * align..(f + 1) * align];
                let l = read_sample_as_f32(frame_slice, 0, bits, is_float);
                let r = if channels >= 2 {
                    read_sample_as_f32(frame_slice, 1, bits, is_float)
                } else {
                    l
                };
                for &sample in &[l, r] {
                    let abs = sample.abs();
                    if abs > 1e-4 {
                        nonzero += 1;
                        if abs > max_amp {
                            max_amp = abs;
                        }
                    }
                }
                st.queued_stereo_f32.push_back(l);
                st.queued_stereo_f32.push_back(r);
            }
            // If caller wrote more frames than staging_buf held (shouldn't happen), pad silence to match frames_to_push
            if frames_to_push > actual_frames {
                st.queued_stereo_f32
                    .extend(std::iter::repeat(0.0f32).take((frames_to_push - actual_frames) * 2));
            }
            if nonzero > 0 {
                AUDIO_NONZERO_SAMPLES_COUNT.fetch_add(nonzero, Ordering::Relaxed);
                let amp_int = ((max_amp * 1000.0) as u32).min(1000);
                AUDIO_PEAK_AMPLITUDE_MILLIS.fetch_max(amp_int, Ordering::Relaxed);
            }
        }
    }
    // Do NOT call SetEvent inside ReleaseBuffer; the mixer/clock thread paces events.
    S_OK
}

// IAudioClock COM interface
// COM-интерфейс IAudioClock

static mut AUDIO_CLOCK_VTBL: [*const c_void; 6] = [
    audio_clock_query_interface as *const c_void,
    audio_clock_add_ref as *const c_void,
    audio_clock_release as *const c_void,
    audio_clock_get_frequency as *const c_void,
    audio_clock_get_position as *const c_void,
    audio_clock_get_characteristics as *const c_void,
];

pub unsafe extern "win64" fn create_audio_clock(ppv: *mut *mut c_void) -> HRESULT {
    create_audio_clock_with_state(ppv, None)
}

unsafe fn create_audio_clock_with_state(
    ppv: *mut *mut c_void,
    state: Option<Arc<Mutex<AudioStreamState>>>,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = AUDIO_CLOCK_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    set_obj_stream_state(obj, state);
    *ppv = obj as *mut c_void;
    println!("[WASAPI] Created IAudioClock (obj=0x{:016X})", obj as usize);
    S_OK
}

unsafe extern "win64" fn audio_clock_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = this;
    audio_clock_add_ref(this);
    S_OK
}

unsafe extern "win64" fn audio_clock_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn audio_clock_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        drop_obj_stream_state(this);
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn audio_clock_get_frequency(
    this: *mut c_void,
    pu64_frequency: *mut u64,
) -> HRESULT {
    if pu64_frequency.is_null() {
        return E_POINTER;
    }
    let rate = get_obj_stream_state(this)
        .and_then(|arc| arc.lock().ok().map(|st| st.sample_rate.max(8000) as u64))
        .unwrap_or_else(|| AUDIO_SAMPLE_RATE.load(Ordering::Relaxed).max(44100) as u64);
    *pu64_frequency = rate;
    S_OK
}

unsafe extern "win64" fn audio_clock_get_position(
    this: *mut c_void,
    pu64_position: *mut u64,
    pu64_qpc_position: *mut u64,
) -> HRESULT {
    if pu64_position.is_null() {
        return E_POINTER;
    }
    if let Some(state_arc) = get_obj_stream_state(this) {
        let frames_played = state_arc.lock().map(|st| st.frames_played).unwrap_or(0);
        *pu64_position = frames_played;
        static CLK_C: AtomicU64 = AtomicU64::new(0);
        let c = CLK_C.fetch_add(1, Ordering::Relaxed);
        if c < 20 || c % 100 == 0 {
            println!("[WASAPI] GetPosition #{} -> pos={}", c, *pu64_position);
        }
    } else {
        let align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(1) as u64;
        let bytes = AUDIO_BYTES_SUBMITTED_TOTAL.load(Ordering::Relaxed);
        let queued = get_sdl_queued_audio_bytes() as u64;
        let played_bytes = bytes.saturating_sub(queued);
        *pu64_position = played_bytes / align;
        static CLK_C: AtomicU64 = AtomicU64::new(0);
        let c = CLK_C.fetch_add(1, Ordering::Relaxed);
        if c < 20 || c % 100 == 0 {
            println!("[WASAPI] GetPosition #{} -> pos={}, queued={}", c, *pu64_position, queued);
        }
    }
    if !pu64_qpc_position.is_null() {
        let mut qpc = 0i64;
        crate::dlls::kernel32::QueryPerformanceCounter(&mut qpc);
        // MSDN WASAPI: Converts performance counter value to 100-nanosecond units.
        // QueryPerformanceFrequency in Rine is 1_000_000_000 (1 ns per tick).
        // 100 ns = 100 ticks, so qpc / 100.
        *pu64_qpc_position = ((qpc as u64) / 100).max(1);
    }
    S_OK
}

unsafe extern "win64" fn audio_clock_get_characteristics(
    _this: *mut c_void,
    pdw_characteristics: *mut u32,
) -> HRESULT {
    if !pdw_characteristics.is_null() {
        *pdw_characteristics = 0x00000001; // AUDIOCLOCK_CHARACTERISTIC_FIXED_FREQ
    }
    S_OK
}

// ISimpleAudioVolume COM interface
// COM-интерфейс ISimpleAudioVolume

static mut SIMPLE_AUDIO_VOLUME_VTBL: [*const c_void; 7] = [
    simple_audio_volume_query_interface as *const c_void,
    simple_audio_volume_add_ref as *const c_void,
    simple_audio_volume_release as *const c_void,
    simple_audio_volume_set_master_volume as *const c_void,
    simple_audio_volume_get_master_volume as *const c_void,
    simple_audio_volume_set_mute as *const c_void,
    simple_audio_volume_get_mute as *const c_void,
];

pub unsafe extern "win64" fn create_simple_audio_volume(ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = SIMPLE_AUDIO_VOLUME_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    *ppv = obj as *mut c_void;
    println!("[WASAPI] Created ISimpleAudioVolume (obj=0x{:016X})", obj as usize);
    S_OK
}

unsafe extern "win64" fn simple_audio_volume_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = this;
    simple_audio_volume_add_ref(this);
    S_OK
}

unsafe extern "win64" fn simple_audio_volume_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn simple_audio_volume_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        libc::free(this);
        0
    }
}

pub static AUDIO_MASTER_VOLUME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1000);
pub static AUDIO_IS_MUTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe extern "win64" fn simple_audio_volume_set_master_volume(
    _this: *mut c_void,
    f_level_norm: f32,
    _p_context: *const c_void,
) -> HRESULT {
    let scaled = (f_level_norm.clamp(0.0, 1.0) * 1000.0) as u32;
    AUDIO_MASTER_VOLUME.store(scaled, Ordering::Relaxed);
    S_OK
}

unsafe extern "win64" fn simple_audio_volume_get_master_volume(
    _this: *mut c_void,
    pf_level_norm: *mut f32,
) -> HRESULT {
    if pf_level_norm.is_null() {
        return E_POINTER;
    }
    let scaled = AUDIO_MASTER_VOLUME.load(Ordering::Relaxed) as f32;
    *pf_level_norm = scaled / 1000.0;
    S_OK
}

unsafe extern "win64" fn simple_audio_volume_set_mute(
    _this: *mut c_void,
    b_mute: i32,
    _p_context: *const c_void,
) -> HRESULT {
    AUDIO_IS_MUTED.store(b_mute != 0, Ordering::Relaxed);
    S_OK
}

unsafe extern "win64" fn simple_audio_volume_get_mute(
    _this: *mut c_void,
    pb_mute: *mut i32,
) -> HRESULT {
    if pb_mute.is_null() {
        return E_POINTER;
    }
    *pb_mute = if AUDIO_IS_MUTED.load(Ordering::Relaxed) { 1 } else { 0 };
    S_OK
}

// IAudioSessionControl COM interface
// COM-интерфейс IAudioSessionControl

static mut AUDIO_SESSION_CONTROL_VTBL: [*const c_void; 12] = [
    audio_session_control_query_interface as *const c_void,
    audio_session_control_add_ref as *const c_void,
    audio_session_control_release as *const c_void,
    audio_session_control_get_state as *const c_void,
    audio_session_control_get_display_name as *const c_void,
    audio_session_control_set_display_name as *const c_void,
    audio_session_control_get_icon_path as *const c_void,
    audio_session_control_set_icon_path as *const c_void,
    audio_session_control_get_grouping_param as *const c_void,
    audio_session_control_set_grouping_param as *const c_void,
    audio_session_control_register_notification as *const c_void,
    audio_session_control_unregister_notification as *const c_void,
];

pub unsafe extern "win64" fn create_audio_session_control(ppv: *mut *mut c_void) -> HRESULT {
    create_audio_session_control_with_state(ppv, None)
}

unsafe fn create_audio_session_control_with_state(
    ppv: *mut *mut c_void,
    state: Option<Arc<Mutex<AudioStreamState>>>,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = AUDIO_SESSION_CONTROL_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    set_obj_stream_state(obj, state);
    *ppv = obj as *mut c_void;
    println!("[WASAPI] Created IAudioSessionControl (obj=0x{:016X})", obj as usize);
    S_OK
}

unsafe extern "win64" fn audio_session_control_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = this;
    audio_session_control_add_ref(this);
    S_OK
}

unsafe extern "win64" fn audio_session_control_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    (*obj).refcount += 1;
    (*obj).refcount
}

unsafe extern "win64" fn audio_session_control_release(this: *mut c_void) -> u32 {
    let obj = this as *mut MockComObject;
    if (*obj).refcount > 1 {
        (*obj).refcount -= 1;
        (*obj).refcount
    } else {
        drop_obj_stream_state(this);
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn audio_session_control_get_state(
    _this: *mut c_void,
    p_retval: *mut u32,
) -> HRESULT {
    if !p_retval.is_null() {
        *p_retval = 1; // AudioSessionStateActive
    }
    S_OK
}

unsafe extern "win64" fn audio_session_control_get_display_name(
    _this: *mut c_void,
    p_retval: *mut *mut u16,
) -> HRESULT {
    if !p_retval.is_null() {
        *p_retval = std::ptr::null_mut();
    }
    S_OK
}

unsafe extern "win64" fn audio_session_control_set_display_name(
    _this: *mut c_void,
    _value: *const u16,
    _event_context: *const c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn audio_session_control_get_icon_path(
    _this: *mut c_void,
    p_retval: *mut *mut u16,
) -> HRESULT {
    if !p_retval.is_null() {
        *p_retval = std::ptr::null_mut();
    }
    S_OK
}

unsafe extern "win64" fn audio_session_control_set_icon_path(
    _this: *mut c_void,
    _value: *const u16,
    _event_context: *const c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn audio_session_control_get_grouping_param(
    _this: *mut c_void,
    p_retval: *mut GUID,
) -> HRESULT {
    if !p_retval.is_null() {
        *p_retval = GUID { Data1: 0, Data2: 0, Data3: 0, Data4: [0; 8] };
    }
    S_OK
}

unsafe extern "win64" fn audio_session_control_set_grouping_param(
    _this: *mut c_void,
    _override: *const GUID,
    _event_context: *const c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn audio_session_control_register_notification(
    _this: *mut c_void,
    _new_notifications: *const c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn audio_session_control_unregister_notification(
    _this: *mut c_void,
    _new_notifications: *const c_void,
) -> HRESULT {
    S_OK
}

pub unsafe extern "win64" fn X3DAudioInitialize(
    _speaker_channel_mask: u32,
    _speed_of_sound: f32,
    instance: *mut u8,
) -> HRESULT {
    if !instance.is_null() {
        std::ptr::write_bytes(instance, 0, 20);
    }
    S_OK
}

pub unsafe extern "win64" fn X3DAudioCalculate(
    _instance: *const u8,
    _listener: *const c_void,
    _emitter: *const c_void,
    _flags: u32,
    _dsp_settings: *mut c_void,
) {
}

pub unsafe extern "win64" fn CreateFX(
    _clsid: *const c_void,
    p_effect: *mut *mut c_void,
    _p_init_data: *const c_void,
    _init_data_byte_size: u32,
) -> HRESULT {
    if !p_effect.is_null() {
        *p_effect = std::ptr::null_mut();
    }
    0x8000_4001u32 as i32 // E_NOTIMPL
}

pub unsafe extern "win64" fn ActivateAudioInterfaceAsync(
    _device_interface_path: *const u16,
    _riid: *const c_void,
    _activation_params: *const c_void,
    _completion_handler: *mut c_void,
    activation_operation: *mut *mut c_void,
) -> HRESULT {
    if !activation_operation.is_null() {
        *activation_operation = std::ptr::null_mut();
    }
    0x8000_4001u32 as i32 // E_NOTIMPL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasapi_com_lifecycle() {
        unsafe {
            let mut p_enum: *mut c_void = std::ptr::null_mut();
            let hr = create_mm_device_enumerator(&mut p_enum);
            assert_eq!(hr, S_OK);
            assert!(!p_enum.is_null());

            let mut p_device: *mut c_void = std::ptr::null_mut();
            let hr_dev = create_mm_device(&mut p_device);
            assert_eq!(hr_dev, S_OK);
            assert!(!p_device.is_null());

            let mut p_client: *mut c_void = std::ptr::null_mut();
            let hr_client = create_audio_client(&mut p_client);
            assert_eq!(hr_client, S_OK);
            assert!(!p_client.is_null());

            let mut p_render: *mut c_void = std::ptr::null_mut();
            let hr_render = create_audio_render_client(&mut p_render);
            assert_eq!(hr_render, S_OK);
            assert!(!p_render.is_null());

            // Test GetMixFormat
            let mut p_wfx: *mut WAVEFORMATEX = std::ptr::null_mut();
            let hr_mix = audio_client_get_mix_format(p_client, &mut p_wfx);
            assert_eq!(hr_mix, S_OK);
            assert!(!p_wfx.is_null());
            assert_eq!((*p_wfx).nChannels, 2);
            assert_eq!((*p_wfx).wBitsPerSample, 32);
            assert_eq!((*p_wfx).wFormatTag, 0xFFFE);
            libc::free(p_wfx as *mut c_void);

            // Test GetCurrentPadding
            let mut padding = 9999u32;
            let hr_pad = audio_client_get_current_padding(p_client, &mut padding);
            assert_eq!(hr_pad, S_OK);
            assert!(padding <= 2048);
        }
    }

    #[test]
    fn test_audio_queue_bounded() {
        let dummy_data = [0u8; 1024];
        queue_sdl_audio(dummy_data.as_ptr(), dummy_data.len());
    }

    #[test]
    fn test_audio_clock_position_tracking() {
        unsafe {
            let mut p_clock: *mut c_void = std::ptr::null_mut();
            let hr = create_audio_clock(&mut p_clock);
            assert_eq!(hr, S_OK);
            assert!(!p_clock.is_null());

            AUDIO_BLOCK_ALIGN.store(8, Ordering::SeqCst);
            AUDIO_BYTES_SUBMITTED_TOTAL.store(8000, Ordering::SeqCst);
            
            let mut pos = 0u64;
            let mut qpc = 0u64;
            let hr_pos = audio_clock_get_position(p_clock, &mut pos, &mut qpc);
            assert_eq!(hr_pos, S_OK);
            // In unit test without active SDL device, queued is 0, so pos is 8000 / 8 = 1000 frames
            assert_eq!(pos, 1000);
            assert!(qpc > 0);

            libc::free(p_clock);
        }
    }

    #[test]
    fn test_wasapi_multi_client_isolation_and_mixing() {
        unsafe {
            let mut client1: *mut c_void = std::ptr::null_mut();
            let mut client2: *mut c_void = std::ptr::null_mut();
            assert_eq!(create_audio_client(&mut client1), S_OK);
            assert_eq!(create_audio_client(&mut client2), S_OK);

            // Client 1: 48kHz stereo 32-bit float, 100ms buffer (4800 frames)
            let wfx1 = WAVEFORMATEX {
                wFormatTag: 3, // IEEE_FLOAT
                nChannels: 2,
                nSamplesPerSec: 48000,
                nAvgBytesPerSec: 48000 * 8,
                nBlockAlign: 8,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            assert_eq!(
                audio_client_initialize(client1, 0, 0, 1_000_000, 0, &wfx1, std::ptr::null()),
                S_OK
            );

            // Client 2: 48kHz stereo 16-bit PCM, 50ms buffer (2400 frames)
            let wfx2 = WAVEFORMATEX {
                wFormatTag: 1, // PCM
                nChannels: 2,
                nSamplesPerSec: 48000,
                nAvgBytesPerSec: 48000 * 4,
                nBlockAlign: 4,
                wBitsPerSample: 16,
                cbSize: 0,
            };
            assert_eq!(
                audio_client_initialize(client2, 0, 0, 500_000, 0, &wfx2, std::ptr::null()),
                S_OK
            );

            // Verify buffer sizes remain isolated per client
            let mut buf1 = 0u32;
            let mut buf2 = 0u32;
            assert_eq!(audio_client_get_buffer_size(client1, &mut buf1), S_OK);
            assert_eq!(audio_client_get_buffer_size(client2, &mut buf2), S_OK);
            assert_eq!(buf1, 4800);
            assert_eq!(buf2, 2400);

            // GetService for both clients
            let iid_render = GUID {
                Data1: 0xF294ACFC,
                Data2: 0,
                Data3: 0,
                Data4: [0; 8],
            };
            let iid_clock = GUID {
                Data1: 0xCD63314F,
                Data2: 0,
                Data3: 0,
                Data4: [0; 8],
            };
            let mut render1: *mut c_void = std::ptr::null_mut();
            let mut render2: *mut c_void = std::ptr::null_mut();
            let mut clock1: *mut c_void = std::ptr::null_mut();
            let mut clock2: *mut c_void = std::ptr::null_mut();
            assert_eq!(
                audio_client_get_service(client1, &iid_render as *const GUID as *const u8, &mut render1),
                S_OK
            );
            assert_eq!(
                audio_client_get_service(client2, &iid_render as *const GUID as *const u8, &mut render2),
                S_OK
            );
            assert_eq!(
                audio_client_get_service(client1, &iid_clock as *const GUID as *const u8, &mut clock1),
                S_OK
            );
            assert_eq!(
                audio_client_get_service(client2, &iid_clock as *const GUID as *const u8, &mut clock2),
                S_OK
            );

            // Client 1 writes 960 non-silent float frames
            let mut p_data1: *mut u8 = std::ptr::null_mut();
            assert_eq!(audio_render_get_buffer(render1, 960, &mut p_data1), S_OK);
            assert!(!p_data1.is_null());
            let slice1 = std::slice::from_raw_parts_mut(p_data1 as *mut f32, 960 * 2);
            slice1.fill(0.5);
            assert_eq!(audio_render_release_buffer(render1, 960, 0), S_OK);

            // Client 2 writes 600 silent frames (AUDCLNT_BUFFERFLAGS_SILENT = 0x2)
            let mut p_data2: *mut u8 = std::ptr::null_mut();
            assert_eq!(audio_render_get_buffer(render2, 600, &mut p_data2), S_OK);
            assert!(!p_data2.is_null());
            assert_eq!(audio_render_release_buffer(render2, 600, 0x2), S_OK);

            // Padding must advance for both clients independently, including silent frames
            let mut pad1 = 0u32;
            let mut pad2 = 0u32;
            assert_eq!(audio_client_get_current_padding(client1, &mut pad1), S_OK);
            assert_eq!(audio_client_get_current_padding(client2, &mut pad2), S_OK);
            assert_eq!(pad1, 960);
            assert_eq!(pad2, 600);

            // Start Client 1 only; Client 2 is not started yet
            if let Some(st1) = get_obj_stream_state(client1) {
                st1.lock().unwrap().started = true;
            }
            mix_and_submit_audio_tick();

            // Client 1 should have drained 480 frames; Client 2 should still have 600
            assert_eq!(audio_client_get_current_padding(client1, &mut pad1), S_OK);
            assert_eq!(audio_client_get_current_padding(client2, &mut pad2), S_OK);
            assert_eq!(pad1, 480);
            assert_eq!(pad2, 600);

            let mut pos1 = 0u64;
            let mut pos2 = 0u64;
            assert_eq!(audio_clock_get_position(clock1, &mut pos1, std::ptr::null_mut()), S_OK);
            assert_eq!(audio_clock_get_position(clock2, &mut pos2, std::ptr::null_mut()), S_OK);
            assert_eq!(pos1, 480);
            assert_eq!(pos2, 0);

            // Now start Client 2 and run another mixer tick
            if let Some(st2) = get_obj_stream_state(client2) {
                st2.lock().unwrap().started = true;
            }
            mix_and_submit_audio_tick();

            assert_eq!(audio_client_get_current_padding(client1, &mut pad1), S_OK);
            assert_eq!(audio_client_get_current_padding(client2, &mut pad2), S_OK);
            assert_eq!(pad1, 0);
            assert_eq!(pad2, 120);

            assert_eq!(audio_clock_get_position(clock1, &mut pos1, std::ptr::null_mut()), S_OK);
            assert_eq!(audio_clock_get_position(clock2, &mut pos2, std::ptr::null_mut()), S_OK);
            assert_eq!(pos1, 960);
            assert_eq!(pos2, 480);

            // Clean up COM objects
            audio_clock_release(clock1);
            audio_clock_release(clock2);
            audio_render_release(render1);
            audio_render_release(render2);
            audio_client_release(client1);
            audio_client_release(client2);
        }
    }
}
