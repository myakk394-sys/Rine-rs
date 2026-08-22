#![allow(non_snake_case)]
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

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

// -------------------------------------------------------------
// SDL Audio Host Backend
// -------------------------------------------------------------

pub static SDL_AUDIO_DEVICE_ID: AtomicU32 = AtomicU32::new(0);
pub static SDL_AUDIO_INITIALIZED: AtomicBool = AtomicBool::new(false);
static AUDIO_BUFFER: Mutex<Option<Vec<u8>>> = Mutex::new(None);
static AUDIO_EVENT_HANDLE: AtomicU64 = AtomicU64::new(0);

pub static AUDIO_SAMPLE_RATE: AtomicU32 = AtomicU32::new(44100);
pub static AUDIO_CHANNELS: AtomicU32 = AtomicU32::new(2);
pub static AUDIO_BITS_PER_SAMPLE: AtomicU32 = AtomicU32::new(32);
pub static AUDIO_BLOCK_ALIGN: AtomicU32 = AtomicU32::new(8);
pub static AUDIO_BUFFER_SIZE_FRAMES: AtomicU32 = AtomicU32::new(2048);

pub static AUDIO_GET_BUFFER_COUNT: AtomicU64 = AtomicU64::new(0);
pub static AUDIO_RELEASE_BUFFER_COUNT: AtomicU64 = AtomicU64::new(0);
pub static AUDIO_BYTES_SUBMITTED_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static AUDIO_NONZERO_SAMPLES_COUNT: AtomicU64 = AtomicU64::new(0);
pub static AUDIO_PEAK_AMPLITUDE_MILLIS: AtomicU32 = AtomicU32::new(0);

pub fn get_max_queue_bytes() -> u32 {
    let rate = AUDIO_SAMPLE_RATE.load(Ordering::Relaxed).max(8000);
    let align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(1);
    rate * align / 2 // 500 ms
}

pub fn get_replenish_target_bytes() -> u32 {
    let rate = AUDIO_SAMPLE_RATE.load(Ordering::Relaxed).max(8000);
    let align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(1);
    rate * align / 10 // 100 ms
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
                std::ffi::CString::new("SDL_GetAudioDeviceStatus").unwrap().as_ptr(),
            );
            if !sym_status.is_null() {
                let fn_status: SdlGetAudioDeviceStatusFn = std::mem::transmute(sym_status);
                return fn_status(dev_id);
            }
        }
    }
    0
}

pub fn get_audio_telemetry_snapshot() -> (u32, u32, u32, u32, u32, u32, u32, u64, u64, u64, u64, u32) {
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

fn ensure_sdl_audio_init(sample_rate: u32, channels: u16) -> u32 {
    let dev_id = SDL_AUDIO_DEVICE_ID.load(Ordering::SeqCst);
    if dev_id != 0 {
        return dev_id;
    }

    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if sdl_handle.is_null() {
        return 0;
    }

    AUDIO_SAMPLE_RATE.store(sample_rate, Ordering::SeqCst);
    AUDIO_CHANNELS.store(channels as u32, Ordering::SeqCst);
    AUDIO_BITS_PER_SAMPLE.store(32, Ordering::SeqCst);
    AUDIO_BLOCK_ALIGN.store((channels as u32) * 4, Ordering::SeqCst);

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
            let desired = SdlAudioSpec {
                freq: sample_rate as i32,
                format: 0x8120, // AUDIO_F32SYS (32-bit float)
                channels: channels as u8,
                silence: 0,
                samples: 256,
                padding: 0,
                size: 0,
                callback: std::ptr::null_mut(),
                userdata: std::ptr::null_mut(),
            };
            let mut obtained: SdlAudioSpec = std::mem::zeroed();
            let opened_id = fn_open(std::ptr::null(), 0, &desired, &mut obtained, 0);
            if opened_id > 0 {
                println!(
                    "[WASAPI AUDIO BACKEND] Successfully opened SDL Audio Device id={} freq={} channels={}",
                    opened_id, obtained.freq, obtained.channels
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
                return opened_id;
            }
        }
    }
    0
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
                std::ffi::CString::new("SDL_GetQueuedAudioSize").unwrap().as_ptr(),
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

fn ensure_audio_clock_thread() {
    if AUDIO_CLOCK_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("rine_audio_clock".into())
        .spawn(|| {
            while SDL_AUDIO_INITIALIZED.load(Ordering::Relaxed) {
                let h_event = AUDIO_EVENT_HANDLE.load(Ordering::Relaxed);
                if h_event != 0 {
                    let queued = get_sdl_queued_audio_bytes();
                    let replenish_target = get_replenish_target_bytes();
                    // Maintain a healthy ~100-200ms audio buffer (~35-70 KB) to prevent underruns
                    if queued < (replenish_target * 2) {
                        unsafe {
                            crate::dlls::kernel32::SetEvent(h_event);
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(3));
            }
        })
        .ok();
}

fn queue_sdl_audio(data: *const u8, len: usize) {
    let dev_id = SDL_AUDIO_DEVICE_ID.load(Ordering::SeqCst);
    if dev_id == 0 || data.is_null() || len == 0 {
        return;
    }
    let queued = get_sdl_queued_audio_bytes();
    let max_queue = get_max_queue_bytes();
    // Cap audio queue at 500ms to prevent unbounded memory growth
    if queued >= max_queue {
        return;
    }
    let sdl_handle = crate::windowing::x11::get_sdl_handle();
    if !sdl_handle.is_null() {
        unsafe {
            type SdlQueueAudioFn = unsafe extern "C" fn(u32, *const u8, u32) -> i32;
            let sym_queue = libc::dlsym(
                sdl_handle,
                std::ffi::CString::new("SDL_QueueAudio").unwrap().as_ptr(),
            );
            if !sym_queue.is_null() {
                let fn_queue: SdlQueueAudioFn = std::mem::transmute(sym_queue);
                let res = fn_queue(dev_id, data, len as u32);
                if res == 0 {
                    AUDIO_BYTES_SUBMITTED_TOTAL.fetch_add(len as u64, Ordering::Relaxed);
                }
            }
        }
    }
}

// -------------------------------------------------------------
// COM Object Representations
// -------------------------------------------------------------

#[repr(C)]
pub struct MockComObject {
    pub lpVtbl: *const c_void,
    pub refcount: u32,
    pub extra_data: usize,
}

// -------------------------------------------------------------
// IMMDeviceEnumerator
// -------------------------------------------------------------

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

pub unsafe extern "win64" fn create_mm_device_enumerator(
    ppv: *mut *mut c_void,
) -> HRESULT {
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
    println!("[WASAPI] CoCreateInstance(CLSID_MMDeviceEnumerator) -> S_OK (obj=0x{:016X})", obj as usize);
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

unsafe extern "win64" fn mm_enum_enum_audio_endpoints(
    _this: *mut c_void,
    _data_flow: u32,
    _dw_state_mask: u32,
    pp_devices: *mut *mut c_void,
) -> HRESULT {
    if !pp_devices.is_null() {
        *pp_devices = std::ptr::null_mut();
    }
    S_OK
}

unsafe extern "win64" fn mm_enum_get_default_audio_endpoint(
    _this: *mut c_void,
    data_flow: u32,
    role: u32,
    pp_endpoint: *mut *mut c_void,
) -> HRESULT {
    println!("[WASAPI] IMMDeviceEnumerator::GetDefaultAudioEndpoint(flow={}, role={})", data_flow, role);
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

// -------------------------------------------------------------
// IMMDevice
// -------------------------------------------------------------

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

unsafe extern "win64" fn mm_dev_activate(
    _this: *mut c_void,
    _riid: *const u8,
    _dw_cls_ctx: u32,
    _p_activation_params: *mut c_void,
    pp_interface: *mut *mut c_void,
) -> HRESULT {
    println!("[WASAPI] IMMDevice::Activate(IAudioClient)");
    create_audio_client(pp_interface)
}

unsafe extern "win64" fn mm_dev_open_property_store(
    _this: *mut c_void,
    _stgm_access: u32,
    pp_properties: *mut *mut c_void,
) -> HRESULT {
    if !pp_properties.is_null() {
        *pp_properties = std::ptr::null_mut();
    }
    S_OK
}

unsafe extern "win64" fn mm_dev_get_id(
    _this: *mut c_void,
    ppstr_id: *mut *mut u16,
) -> HRESULT {
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

unsafe extern "win64" fn mm_dev_get_state(
    _this: *mut c_void,
    pdw_state: *mut u32,
) -> HRESULT {
    if !pdw_state.is_null() {
        *pdw_state = 1; // DEVICE_STATE_ACTIVE
    }
    S_OK
}

// -------------------------------------------------------------
// IAudioClient
// -------------------------------------------------------------

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
    (*obj).lpVtbl = AUDIO_CLIENT_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    *ppv = obj as *mut c_void;
    println!("[WASAPI] Created IAudioClient (obj=0x{:016X})", obj as usize);
    S_OK
}

unsafe extern "win64" fn audio_client_query_interface(
    this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
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
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn audio_client_initialize(
    _this: *mut c_void,
    share_mode: u32,
    stream_flags: u32,
    hns_buffer_duration: i64,
    _hns_periodicity: i64,
    p_format: *const WAVEFORMATEX,
    _audio_session_guid: *const GUID,
) -> HRESULT {
    let (rate, channels) = if !p_format.is_null() {
        ((*p_format).nSamplesPerSec, (*p_format).nChannels)
    } else {
        (48000, 2)
    };
    println!(
        "[WASAPI] IAudioClient::Initialize(shareMode={}, flags=0x{:X}, duration={}, rate={}, channels={})",
        share_mode, stream_flags, hns_buffer_duration, rate, channels
    );
    ensure_sdl_audio_init(rate, channels);
    let requested_frames = (hns_buffer_duration * (rate as i64) / 10_000_000).max(4096) as u32;
    let buffer_frames = requested_frames.max(4096);
    AUDIO_BUFFER_SIZE_FRAMES.store(buffer_frames, Ordering::SeqCst);

    let mut buf_guard = AUDIO_BUFFER.lock().unwrap();
    *buf_guard = Some(vec![0u8; (buffer_frames as usize) * (channels as usize) * 4]);
    S_OK
}

unsafe extern "win64" fn audio_client_get_buffer_size(
    _this: *mut c_void,
    p_num_buffer_frames: *mut u32,
) -> HRESULT {
    if !p_num_buffer_frames.is_null() {
        *p_num_buffer_frames = AUDIO_BUFFER_SIZE_FRAMES.load(Ordering::Relaxed).max(2048);
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
    _this: *mut c_void,
    p_num_padding_frames: *mut u32,
) -> HRESULT {
    if !p_num_padding_frames.is_null() {
        let queued = get_sdl_queued_audio_bytes();
        let align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(1);
        let buf_frames = AUDIO_BUFFER_SIZE_FRAMES.load(Ordering::Relaxed).max(512);
        let target_prefill_frames = 3000u32; // ~68ms preload
        let queued_frames = queued / align;
        *p_num_padding_frames = queued_frames.saturating_sub(target_prefill_frames).min(buf_frames);
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
    let size = std::mem::size_of::<WAVEFORMATEX>();
    let mem = libc::calloc(1, size) as *mut WAVEFORMATEX;
    if mem.is_null() {
        return E_FAIL;
    }
    (*mem).wFormatTag = 0x0003; // WAVE_FORMAT_IEEE_FLOAT
    (*mem).nChannels = 2;
    (*mem).nSamplesPerSec = 44100;
    (*mem).wBitsPerSample = 32;
    (*mem).nBlockAlign = 8;
    (*mem).nAvgBytesPerSec = 44100 * 8;
    (*mem).cbSize = 0;
    *pp_device_format = mem;
    println!("[WASAPI] IAudioClient::GetMixFormat -> 44100Hz 32-bit Float Stereo (WAVE_FORMAT_IEEE_FLOAT)");
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

unsafe extern "win64" fn audio_client_start(_this: *mut c_void) -> HRESULT {
    println!("[WASAPI] IAudioClient::Start()");
    ensure_audio_clock_thread();
    let h_event = AUDIO_EVENT_HANDLE.load(Ordering::SeqCst);
    if h_event != 0 {
        crate::dlls::kernel32::SetEvent(h_event);
    }
    S_OK
}

unsafe extern "win64" fn audio_client_stop(_this: *mut c_void) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn audio_client_reset(_this: *mut c_void) -> HRESULT {
    S_OK
}

unsafe extern "win64" fn audio_client_set_event_handle(
    _this: *mut c_void,
    event_handle: u64,
) -> HRESULT {
    println!("[WASAPI] IAudioClient::SetEventHandle(0x{:X})", event_handle);
    AUDIO_EVENT_HANDLE.store(event_handle, Ordering::SeqCst);
    ensure_audio_clock_thread();
    if event_handle != 0 {
        crate::dlls::kernel32::SetEvent(event_handle);
    }
    S_OK
}

unsafe extern "win64" fn audio_client_get_service(
    _this: *mut c_void,
    _riid: *const u8,
    ppv: *mut *mut c_void,
) -> HRESULT {
    println!("[WASAPI] IAudioClient::GetService(IAudioRenderClient)");
    create_audio_render_client(ppv)
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

// -------------------------------------------------------------
// IAudioRenderClient
// -------------------------------------------------------------

static mut AUDIO_RENDER_CLIENT_VTBL: [*const c_void; 5] = [
    audio_render_query_interface as *const c_void,
    audio_render_add_ref as *const c_void,
    audio_render_release as *const c_void,
    audio_render_get_buffer as *const c_void,
    audio_render_release_buffer as *const c_void,
];

pub unsafe extern "win64" fn create_audio_render_client(ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    let obj = libc::calloc(1, std::mem::size_of::<MockComObject>()) as *mut MockComObject;
    if obj.is_null() {
        return E_FAIL;
    }
    (*obj).lpVtbl = AUDIO_RENDER_CLIENT_VTBL.as_ptr() as *const c_void;
    (*obj).refcount = 1;
    *ppv = obj as *mut c_void;
    println!("[WASAPI] Created IAudioRenderClient (obj=0x{:016X})", obj as usize);
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
        libc::free(this);
        0
    }
}

unsafe extern "win64" fn audio_render_get_buffer(
    _this: *mut c_void,
    num_frames_requested: u32,
    pp_data: *mut *mut u8,
) -> HRESULT {
    if pp_data.is_null() {
        return E_POINTER;
    }
    AUDIO_GET_BUFFER_COUNT.fetch_add(1, Ordering::Relaxed);
    let align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(1) as usize;
    let mut buf_guard = AUDIO_BUFFER.lock().unwrap();
    if buf_guard.is_none() {
        *buf_guard = Some(vec![0u8; 4096 * align]);
    }
    if let Some(buf) = buf_guard.as_mut() {
        let needed_bytes = (num_frames_requested as usize) * align;
        if buf.len() < needed_bytes {
            buf.resize(needed_bytes, 0);
        }
        *pp_data = buf.as_mut_ptr();
        return S_OK;
    }
    E_FAIL
}

unsafe extern "win64" fn audio_render_release_buffer(
    _this: *mut c_void,
    num_frames_written: u32,
    dw_flags: u32,
) -> HRESULT {
    AUDIO_RELEASE_BUFFER_COUNT.fetch_add(1, Ordering::Relaxed);
    let align = AUDIO_BLOCK_ALIGN.load(Ordering::Relaxed).max(1) as usize;
    let mut buf_guard = AUDIO_BUFFER.lock().unwrap();
    if let Some(buf) = buf_guard.as_mut() {
        let bytes_written = (num_frames_written as usize) * align;
        if bytes_written > 0 && bytes_written <= buf.len() {
            const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 0x2;
            if (dw_flags & AUDCLNT_BUFFERFLAGS_SILENT) != 0 {
                buf[..bytes_written].fill(0);
            } else {
                let floats = std::slice::from_raw_parts(buf.as_ptr() as *const f32, bytes_written / 4);
                let mut max_amp: f32 = 0.0;
                let mut nonzero = 0u64;
                for &sample in floats {
                    let abs = sample.abs();
                    if abs > 1e-4 {
                        nonzero += 1;
                        if abs > max_amp {
                            max_amp = abs;
                        }
                    }
                }
                if nonzero > 0 {
                    AUDIO_NONZERO_SAMPLES_COUNT.fetch_add(nonzero, Ordering::Relaxed);
                    let amp_int = (max_amp * 1000.0) as u32;
                    AUDIO_PEAK_AMPLITUDE_MILLIS.fetch_max(amp_int, Ordering::Relaxed);
                }
            }
            queue_sdl_audio(buf.as_ptr(), bytes_written);
        }
    }
    let queued = get_sdl_queued_audio_bytes();
    let target = get_replenish_target_bytes();
    if queued < target {
        let h_event = AUDIO_EVENT_HANDLE.load(Ordering::SeqCst);
        if h_event != 0 {
            unsafe {
                crate::dlls::kernel32::SetEvent(h_event);
            }
        }
    }
    S_OK
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
}
