//! Windows Media Foundation implementation (`mf.dll`, `mfplat.dll`, `mfreadwrite.dll`, `mfplay.dll`)
//! Uses host `ffprobe` / `ffmpeg` for `.mp4` (and other media) decoding so UE4 `WmfMedia`
//! and `WindowsMoviePlayer` can play menu backgrounds and cutscenes via software
//! `IMFSampleGrabberSinkCallback` and `MFCreateAudioRendererActivate` / WASAPI.
#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// HRESULT & MF Constants
// ============================================================================

pub const S_OK: i32 = 0;
pub const S_FALSE: i32 = 1;
pub const E_NOTIMPL: i32 = 0x80004001_u32 as i32;
pub const E_NOINTERFACE: i32 = 0x80004002_u32 as i32;
pub const E_POINTER: i32 = 0x80004003_u32 as i32;
pub const E_FAIL: i32 = 0x80004005_u32 as i32;
pub const E_INVALIDARG: i32 = 0x80070057_u32 as i32;
pub const HRESULT_FILE_NOT_FOUND: i32 = 0x80070002_u32 as i32;

pub const MF_E_ATTRIBUTENOTFOUND: i32 = 0xC00D36E6_u32 as i32;
pub const MF_E_INVALIDTYPE: i32 = 0xC00D36E7_u32 as i32;
pub const MF_E_BUFFERTOOSMALL: i32 = 0xC00D36F0_u32 as i32;
pub const MF_E_NO_EVENTS_AVAILABLE: i32 = 0xC00D3E80_u32 as i32;
pub const MF_E_SHUTDOWN: i32 = 0xC00D3E85_u32 as i32;
pub const MF_E_UNSUPPORTED_SERVICE: i32 = 0xC00D36BA_u32 as i32;
pub const MF_E_INVALIDREQUEST: i32 = 0xC00D36B2_u32 as i32;

// Media Event Types
const MEUnknown: u32 = 0;
const MESessionTopologySet: u32 = 101; // 0x65
const MESessionStarted: u32 = 103; // 0x67
const MESessionPaused: u32 = 104; // 0x68
const MESessionStopped: u32 = 105; // 0x69
const MESessionClosed: u32 = 106; // 0x6a
const MESessionEnded: u32 = 107; // 0x6b
const MESessionTopologyStatus: u32 = 111; // 0x6f

// Topology Status
const MF_TOPOSTATUS_READY: u32 = 100;
const MF_TOPOSTATUS_STARTED_SOURCE: u32 = 200;

// Topology Node Types
const MF_TOPOLOGY_OUTPUT_NODE: u32 = 0;
const MF_TOPOLOGY_SOURCESTREAM_NODE: u32 = 1;

// Event Flags
const MF_EVENT_FLAG_NO_WAIT: u32 = 1;

// Clock States
const MFCLOCK_STATE_INVALID: u32 = 0;
const MFCLOCK_STATE_RUNNING: u32 = 1;
const MFCLOCK_STATE_STOPPED: u32 = 2;
const MFCLOCK_STATE_PAUSED: u32 = 3;

// Object types
const MF_OBJECT_MEDIASOURCE: u32 = 1;

// PROPVARIANT VARTYPEs
const VT_EMPTY: u16 = 0;
const VT_R8: u16 = 5;
const VT_UNKNOWN: u16 = 13;
const VT_UI4: u16 = 19;
const VT_I8: u16 = 20;
const VT_UI8: u16 = 21;
const VT_LPWSTR: u16 = 31;
const VT_CLSID: u16 = 72;
const VT_VECTOR_UI1: u16 = 0x1011;

// ============================================================================
// GUID Helpers & Definitions
// ============================================================================

const fn guid(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> [u8; 16] {
    let b1 = d1.to_le_bytes();
    let b2 = d2.to_le_bytes();
    let b3 = d3.to_le_bytes();
    [
        b1[0], b1[1], b1[2], b1[3], b2[0], b2[1], b3[0], b3[1], d4[0], d4[1], d4[2], d4[3], d4[4],
        d4[5], d4[6], d4[7],
    ]
}

const GUID_NULL: [u8; 16] = [0u8; 16];
const IID_IUnknown: [u8; 16] = guid(0x00000000, 0x0000, 0x0000, [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]);
const IID_IMFAttributes: [u8; 16] = guid(0x2CD2D921, 0xC447, 0x44A7, [0xA1, 0x3C, 0x4A, 0xDA, 0xBF, 0xC2, 0x47, 0xE3]);
const IID_IMFMediaType: [u8; 16] = guid(0x44AE0FA8, 0xEA31, 0x4109, [0x8D, 0x2E, 0x4C, 0xAE, 0x49, 0x97, 0xC5, 0x55]);
const IID_IMFMediaEvent: [u8; 16] = guid(0xDF598932, 0xF10C, 0x4E39, [0xBB, 0xA2, 0xC3, 0x08, 0xF1, 0x01, 0xDA, 0xA3]);
const IID_IMFMediaEventGenerator: [u8; 16] = guid(0x2CD0BD52, 0xBCD5, 0x4B89, [0xB6, 0x2C, 0xEA, 0xDC, 0x0C, 0x03, 0x1E, 0x7D]);
const IID_IMFMediaEventQueue: [u8; 16] = guid(0x36F846FC, 0x2256, 0x48B6, [0xB5, 0x8E, 0xE2, 0xB6, 0x38, 0x31, 0x65, 0x81]);
const IID_IMFAsyncResult: [u8; 16] = guid(0xAC6B7889, 0x0740, 0x4D51, [0x86, 0x19, 0x90, 0x59, 0x94, 0xA5, 0x5C, 0xC6]);
const IID_IMFMediaBuffer: [u8; 16] = guid(0x045FA593, 0x8799, 0x42B8, [0xBC, 0x8D, 0x89, 0x68, 0xC6, 0x45, 0x35, 0x07]);
const IID_IMFSample: [u8; 16] = guid(0xC40A00F2, 0xB93A, 0x4D80, [0xAE, 0x8C, 0x5A, 0x1C, 0x63, 0x4F, 0x58, 0xE4]);
const IID_IMFSourceResolver: [u8; 16] = guid(0xFBE5A32D, 0xA497, 0x4B61, [0xBB, 0x85, 0x97, 0xB1, 0xA8, 0x48, 0xA6, 0xE3]);
const IID_IMFMediaSource: [u8; 16] = guid(0x279A808D, 0xAEC7, 0x40C8, [0x9C, 0x6B, 0xA6, 0xB4, 0x92, 0xC7, 0x8A, 0x66]);
const IID_IMFPresentationDescriptor: [u8; 16] = guid(0x03CB2711, 0x24D7, 0x4DB6, [0xA1, 0x7F, 0xF3, 0xA7, 0xA4, 0x79, 0xA5, 0x36]);
const IID_IMFStreamDescriptor: [u8; 16] = guid(0x56C03D9C, 0x9DBB, 0x45F5, [0xAB, 0x4B, 0xD8, 0x0F, 0x47, 0xC0, 0x59, 0x38]);
const IID_IMFMediaTypeHandler: [u8; 16] = guid(0xE93DCF6C, 0x4B07, 0x4E1E, [0x81, 0x23, 0xAA, 0x16, 0xED, 0x6E, 0xAD, 0xF5]);
const IID_IMFTopology: [u8; 16] = guid(0x83CF873A, 0xF6DA, 0x4BC8, [0x82, 0x3F, 0xBA, 0xCF, 0xD5, 0x5D, 0xC4, 0x33]);
const IID_IMFTopologyNode: [u8; 16] = guid(0x83CF873A, 0xF6DA, 0x4BC8, [0x82, 0x3F, 0xBA, 0xCF, 0xD5, 0x5D, 0xC4, 0x30]);
const IID_IMFActivate: [u8; 16] = guid(0x7FEE9E9A, 0x4A89, 0x47A6, [0x89, 0x9C, 0xB6, 0xA5, 0x3A, 0x70, 0xFB, 0x67]);
const IID_IMFMediaSession: [u8; 16] = guid(0x90377834, 0x21D0, 0x4DEE, [0x82, 0x14, 0xBA, 0x2E, 0x3E, 0x6C, 0x11, 0x27]);
const IID_IMFClock: [u8; 16] = guid(0x2EB1E945, 0x18B8, 0x4139, [0x9B, 0x1A, 0xD5, 0xD5, 0x84, 0x81, 0x85, 0x30]);
const IID_IMFPresentationClock: [u8; 16] = guid(0x868CE85C, 0x8EA9, 0x4F55, [0xAB, 0x82, 0xB0, 0x09, 0xA9, 0x10, 0xA8, 0x05]);
const IID_IMFRateControl: [u8; 16] = guid(0x88DDCD21, 0x03C3, 0x4275, [0x91, 0xED, 0x55, 0xEE, 0x39, 0x29, 0x32, 0x8F]);
const IID_IMFRateSupport: [u8; 16] = guid(0x0A9CCDBC, 0xD797, 0x4563, [0x96, 0x67, 0x94, 0xEC, 0x5D, 0x79, 0x29, 0x2D]);
const IID_IMFGetService: [u8; 16] = guid(0xFA993888, 0x4383, 0x415A, [0xA9, 0x30, 0xDD, 0x47, 0x2A, 0x8C, 0xF6, 0xF7]);

const MF_RATE_CONTROL_SERVICE: [u8; 16] = guid(0x866FA297, 0xB802, 0x4BF8, [0x9D, 0xC9, 0x5E, 0x3B, 0x6A, 0x9F, 0x53, 0xC9]);
const MR_VIDEO_ACCELERATION_SERVICE: [u8; 16] = guid(0xEF114C90, 0xCD1D, 0x484E, [0x96, 0xE5, 0x09, 0xCF, 0xAF, 0x91, 0x2A, 0x21]);

// Media types & formats
const MFMediaType_Audio: [u8; 16] = guid(0x73647561, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
const MFMediaType_Video: [u8; 16] = guid(0x73646976, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
const MFVideoFormat_H264: [u8; 16] = guid(0x34363248, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
const MFVideoFormat_YUY2: [u8; 16] = guid(0x32595559, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
const MFVideoFormat_NV12: [u8; 16] = guid(0x3231564E, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
const MFVideoFormat_RGB32: [u8; 16] = guid(0x00000016, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
const MFVideoFormat_ARGB32: [u8; 16] = guid(0x00000015, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
const MFAudioFormat_PCM: [u8; 16] = guid(0x00000001, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);

// Attribute keys
const MF_MT_MAJOR_TYPE: [u8; 16] = guid(0x48EBA18E, 0xF8C9, 0x4687, [0xBF, 0x11, 0x0A, 0x74, 0xC9, 0xF9, 0x6A, 0x8F]);
const MF_MT_SUBTYPE: [u8; 16] = guid(0xF7E34C9A, 0x42E8, 0x4714, [0xB7, 0x4B, 0xCB, 0x29, 0xD7, 0x2C, 0x35, 0xE5]);
const MF_MT_ALL_SAMPLES_INDEPENDENT: [u8; 16] = guid(0xC9173739, 0x5E56, 0x461C, [0xB7, 0x13, 0x46, 0xFB, 0x99, 0x5C, 0xB9, 0x5F]);
const MF_MT_FIXED_SIZE_SAMPLES: [u8; 16] = guid(0xB8EBEFAF, 0xB718, 0x4E04, [0xB0, 0xA9, 0x11, 0x67, 0x75, 0xE3, 0x32, 0x1B]);
const MF_MT_FRAME_SIZE: [u8; 16] = guid(0x1652C33D, 0xD6B2, 0x4012, [0xB8, 0x34, 0x72, 0x03, 0x08, 0x49, 0xA3, 0x7D]);
const MF_MT_DEFAULT_STRIDE: [u8; 16] = guid(0x644B4E48, 0x1E02, 0x4516, [0xB0, 0xEB, 0xC0, 0x1C, 0xA9, 0xD4, 0x9A, 0xC6]);
const MF_MT_FRAME_RATE: [u8; 16] = guid(0xC459A2E8, 0x3D2C, 0x4E44, [0xB1, 0x32, 0xFE, 0xE5, 0x15, 0x6C, 0x7B, 0xB0]);
const MF_MT_PIXEL_ASPECT_RATIO: [u8; 16] = guid(0xC6376A1E, 0x8D0A, 0x4027, [0xBE, 0x45, 0x6D, 0x9A, 0x0A, 0xD3, 0x9B, 0xB6]);
const MF_MT_INTERLACE_MODE: [u8; 16] = guid(0xE2724BB8, 0xE676, 0x4806, [0xB4, 0xB2, 0xA8, 0xD6, 0xEF, 0xB4, 0x4C, 0xCD]);
const MF_MT_AUDIO_NUM_CHANNELS: [u8; 16] = guid(0x37E48BF5, 0x645E, 0x4C5B, [0x89, 0xDE, 0xAD, 0xA9, 0xE2, 0x9B, 0x69, 0x6A]);
const MF_MT_AUDIO_SAMPLES_PER_SECOND: [u8; 16] = guid(0x5FAEEAE7, 0x0290, 0x4C31, [0x9E, 0x8A, 0xC5, 0x34, 0xF6, 0x8D, 0x9D, 0xBA]);
const MF_MT_AUDIO_BITS_PER_SAMPLE: [u8; 16] = guid(0xF2DEB57F, 0x40FA, 0x4764, [0xAA, 0x33, 0xED, 0x4F, 0x2D, 0x1F, 0xF6, 0x69]);
const MF_MT_AUDIO_AVG_BYTES_PER_SECOND: [u8; 16] = guid(0x1AAB75C8, 0xCFEF, 0x451C, [0xAB, 0x95, 0xAC, 0x03, 0x4B, 0x8E, 0x17, 0x31]);
const MF_MT_AUDIO_BLOCK_ALIGNMENT: [u8; 16] = guid(0x322DE230, 0x9EEB, 0x43BD, [0xAB, 0x7A, 0xFF, 0x41, 0x22, 0x51, 0x54, 0x1D]);
const MF_PD_DURATION: [u8; 16] = guid(0x6C990D33, 0xBB8E, 0x477A, [0x85, 0x98, 0x0D, 0x5D, 0x96, 0xFC, 0xD8, 0x8A]);
const MF_EVENT_TOPOLOGY_STATUS: [u8; 16] = guid(0x30C5018D, 0x9A53, 0x454B, [0xAD, 0x9E, 0x6D, 0x5F, 0x8F, 0xA7, 0xC4, 0x3B]);

// ============================================================================
// COM Helpers
// ============================================================================

unsafe fn com_add_ref(obj: usize) {
    if obj != 0 {
        let vtbl = *(obj as *const *const usize);
        let f: unsafe extern "win64" fn(usize) -> u32 = std::mem::transmute(*vtbl.add(1));
        f(obj);
    }
}

unsafe fn com_release(obj: usize) {
    if obj != 0 {
        let vtbl = *(obj as *const *const usize);
        let f: unsafe extern "win64" fn(usize) -> u32 = std::mem::transmute(*vtbl.add(2));
        f(obj);
    }
}

unsafe fn com_query_interface(obj: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if obj == 0 || riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let vtbl = *(obj as *const *const usize);
    let f: unsafe extern "win64" fn(usize, *const [u8; 16], *mut usize) -> i32 =
        std::mem::transmute(*vtbl.add(0));
    f(obj, riid, ppv)
}

unsafe fn read_utf16_z(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PropVariant {
    vt: u16,
    reserved1: u16,
    reserved2: u16,
    reserved3: u16,
    val_lo: u64,
    val_hi: u64,
}

unsafe extern "win64" fn generic_add_ref<T: HasRefCount>(this: usize) -> u32 {
    let obj = &*(this as *const T);
    obj.ref_count().fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "win64" fn generic_release<T: HasRefCount>(this: usize) -> u32 {
    let obj = &*(this as *const T);
    let prev = obj.ref_count().fetch_sub(1, Ordering::SeqCst);
    if prev == 1 {
        drop(Box::from_raw(this as *mut T));
    }
    prev.saturating_sub(1)
}

trait HasRefCount {
    fn ref_count(&self) -> &AtomicU32;
}

// ============================================================================
// IMFAttributes Shared Header & Implementation
// ============================================================================

#[derive(Clone)]
enum AttrValue {
    Uint32(u32),
    Uint64(u64),
    Double(f64),
    Guid([u8; 16]),
    String(Vec<u16>),
    Blob(Vec<u8>),
    Unknown(usize),
}

impl AttrValue {
    fn mf_type(&self) -> u32 {
        match self {
            AttrValue::Uint32(_) => 19,  // MF_ATTRIBUTE_UINT32
            AttrValue::Uint64(_) => 21,  // MF_ATTRIBUTE_UINT64
            AttrValue::Double(_) => 5,   // MF_ATTRIBUTE_DOUBLE
            AttrValue::Guid(_) => 72,    // MF_ATTRIBUTE_GUID
            AttrValue::String(_) => 31,  // MF_ATTRIBUTE_STRING
            AttrValue::Blob(_) => 0x1011, // MF_ATTRIBUTE_BLOB
            AttrValue::Unknown(_) => 13, // MF_ATTRIBUTE_IUNKNOWN
        }
    }
}

struct AttrMap {
    map: Mutex<HashMap<[u8; 16], AttrValue>>,
}

impl AttrMap {
    fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    fn set(&self, key: [u8; 16], val: AttrValue) {
        if let AttrValue::Unknown(ptr) = val {
            unsafe { com_add_ref(ptr) };
        }
        let old = self.map.lock().unwrap().insert(key, val);
        if let Some(AttrValue::Unknown(old_ptr)) = old {
            unsafe { com_release(old_ptr) };
        }
    }

    fn delete(&self, key: &[u8; 16]) {
        let old = self.map.lock().unwrap().remove(key);
        if let Some(AttrValue::Unknown(old_ptr)) = old {
            unsafe { com_release(old_ptr) };
        }
    }

    fn clear(&self) {
        let old_vals: Vec<AttrValue> = {
            let mut g = self.map.lock().unwrap();
            g.drain().map(|(_, v)| v).collect()
        };
        for v in old_vals {
            if let AttrValue::Unknown(ptr) = v {
                unsafe { com_release(ptr) };
            }
        }
    }
}

impl Drop for AttrMap {
    fn drop(&mut self) {
        self.clear();
    }
}

#[repr(C)]
struct MfAttrHeader {
    vtable: *const usize,
    ref_count: AtomicU32,
    attrs: AttrMap,
}

impl MfAttrHeader {
    fn new(vtable: *const usize) -> Self {
        Self {
            vtable,
            ref_count: AtomicU32::new(1),
            attrs: AttrMap::new(),
        }
    }
}

impl HasRefCount for MfAttrHeader {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

unsafe extern "win64" fn attr_get_item(
    this: usize,
    guid_key: *const [u8; 16],
    p_value: *mut PropVariant,
) -> i32 {
    if guid_key.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    let Some(val) = g.get(&*guid_key) else {
        return MF_E_ATTRIBUTENOTFOUND;
    };
    if p_value.is_null() {
        return S_OK;
    }
    std::ptr::write_bytes(p_value, 0, 1);
    match val {
        AttrValue::Uint32(v) => {
            (*p_value).vt = VT_UI4;
            (*p_value).val_lo = *v as u64;
        }
        AttrValue::Uint64(v) => {
            (*p_value).vt = VT_UI8;
            (*p_value).val_lo = *v;
        }
        AttrValue::Double(v) => {
            (*p_value).vt = VT_R8;
            (*p_value).val_lo = v.to_bits();
        }
        AttrValue::Guid(guid_val) => {
            let mem = crate::dlls::shell::CoTaskMemAlloc(16) as *mut [u8; 16];
            if !mem.is_null() {
                *mem = *guid_val;
            }
            (*p_value).vt = VT_CLSID;
            (*p_value).val_lo = mem as u64;
        }
        AttrValue::String(s) => {
            let bytes = s.len() * 2;
            let mem = crate::dlls::shell::CoTaskMemAlloc(bytes) as *mut u16;
            if !mem.is_null() {
                std::ptr::copy_nonoverlapping(s.as_ptr(), mem, s.len());
            }
            (*p_value).vt = VT_LPWSTR;
            (*p_value).val_lo = mem as u64;
        }
        AttrValue::Blob(b) => {
            let mem = crate::dlls::shell::CoTaskMemAlloc(b.len().max(1)) as *mut u8;
            if !mem.is_null() && !b.is_empty() {
                std::ptr::copy_nonoverlapping(b.as_ptr(), mem, b.len());
            }
            (*p_value).vt = VT_VECTOR_UI1;
            (*p_value).val_lo = b.len() as u64;
            (*p_value).val_hi = mem as u64;
        }
        AttrValue::Unknown(ptr) => {
            com_add_ref(*ptr);
            (*p_value).vt = VT_UNKNOWN;
            (*p_value).val_lo = *ptr as u64;
        }
    }
    S_OK
}

unsafe extern "win64" fn attr_get_item_type(
    this: usize,
    guid_key: *const [u8; 16],
    p_type: *mut u32,
) -> i32 {
    if guid_key.is_null() || p_type.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    if let Some(v) = g.get(&*guid_key) {
        *p_type = v.mf_type();
        S_OK
    } else {
        MF_E_ATTRIBUTENOTFOUND
    }
}

unsafe extern "win64" fn attr_compare_item(
    this: usize,
    guid_key: *const [u8; 16],
    value: *const PropVariant,
    pb_result: *mut i32,
) -> i32 {
    if guid_key.is_null() || value.is_null() || pb_result.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    let eq = if let Some(v) = g.get(&*guid_key) {
        match (v, (*value).vt) {
            (AttrValue::Uint32(a), VT_UI4) => *a == (*value).val_lo as u32,
            (AttrValue::Uint64(a), VT_UI8) | (AttrValue::Uint64(a), VT_I8) => *a == (*value).val_lo,
            (AttrValue::Guid(a), VT_CLSID) => {
                let p = (*value).val_lo as *const [u8; 16];
                !p.is_null() && *a == *p
            }
            _ => false,
        }
    } else {
        false
    };
    *pb_result = if eq { 1 } else { 0 };
    S_OK
}

unsafe extern "win64" fn attr_compare(
    _this: usize,
    _p_theirs: usize,
    _match_type: u32,
    pb_result: *mut i32,
) -> i32 {
    if !pb_result.is_null() {
        *pb_result = 0;
    }
    S_OK
}

unsafe extern "win64" fn attr_get_uint32(
    this: usize,
    guid_key: *const [u8; 16],
    pun_value: *mut u32,
) -> i32 {
    if guid_key.is_null() || pun_value.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::Uint32(v)) => {
            *pun_value = *v;
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_uint64(
    this: usize,
    guid_key: *const [u8; 16],
    pun_value: *mut u64,
) -> i32 {
    if guid_key.is_null() || pun_value.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::Uint64(v)) => {
            *pun_value = *v;
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_double(
    this: usize,
    guid_key: *const [u8; 16],
    pf_value: *mut f64,
) -> i32 {
    if guid_key.is_null() || pf_value.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::Double(v)) => {
            *pf_value = *v;
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_guid(
    this: usize,
    guid_key: *const [u8; 16],
    pguid_value: *mut [u8; 16],
) -> i32 {
    if guid_key.is_null() || pguid_value.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::Guid(v)) => {
            *pguid_value = *v;
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_string_length(
    this: usize,
    guid_key: *const [u8; 16],
    pcch_length: *mut u32,
) -> i32 {
    if guid_key.is_null() || pcch_length.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::String(v)) => {
            *pcch_length = v.len().saturating_sub(1) as u32;
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_string(
    this: usize,
    guid_key: *const [u8; 16],
    pwsz_value: *mut u16,
    cch_buf_size: u32,
    pcch_length: *mut u32,
) -> i32 {
    if guid_key.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::String(v)) => {
            let char_len = v.len().saturating_sub(1);
            if !pcch_length.is_null() {
                *pcch_length = char_len as u32;
            }
            if pwsz_value.is_null() || (cch_buf_size as usize) < v.len() {
                return MF_E_BUFFERTOOSMALL;
            }
            std::ptr::copy_nonoverlapping(v.as_ptr(), pwsz_value, v.len());
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_allocated_string(
    this: usize,
    guid_key: *const [u8; 16],
    ppwsz_value: *mut *mut u16,
    pcch_length: *mut u32,
) -> i32 {
    if guid_key.is_null() || ppwsz_value.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::String(v)) => {
            let mem = crate::dlls::shell::CoTaskMemAlloc(v.len() * 2) as *mut u16;
            if mem.is_null() {
                return E_FAIL;
            }
            std::ptr::copy_nonoverlapping(v.as_ptr(), mem, v.len());
            *ppwsz_value = mem;
            if !pcch_length.is_null() {
                *pcch_length = v.len().saturating_sub(1) as u32;
            }
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_blob_size(
    this: usize,
    guid_key: *const [u8; 16],
    pcb_blob_size: *mut u32,
) -> i32 {
    if guid_key.is_null() || pcb_blob_size.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::Blob(b)) => {
            *pcb_blob_size = b.len() as u32;
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_blob(
    this: usize,
    guid_key: *const [u8; 16],
    p_buf: *mut u8,
    cb_buf_size: u32,
    pcb_blob_size: *mut u32,
) -> i32 {
    if guid_key.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::Blob(b)) => {
            if !pcb_blob_size.is_null() {
                *pcb_blob_size = b.len() as u32;
            }
            if p_buf.is_null() || (cb_buf_size as usize) < b.len() {
                return MF_E_BUFFERTOOSMALL;
            }
            std::ptr::copy_nonoverlapping(b.as_ptr(), p_buf, b.len());
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_allocated_blob(
    this: usize,
    guid_key: *const [u8; 16],
    pp_buf: *mut *mut u8,
    pcb_size: *mut u32,
) -> i32 {
    if guid_key.is_null() || pp_buf.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let g = hdr.attrs.map.lock().unwrap();
    match g.get(&*guid_key) {
        Some(AttrValue::Blob(b)) => {
            let mem = crate::dlls::shell::CoTaskMemAlloc(b.len().max(1)) as *mut u8;
            if mem.is_null() {
                return E_FAIL;
            }
            if !b.is_empty() {
                std::ptr::copy_nonoverlapping(b.as_ptr(), mem, b.len());
            }
            *pp_buf = mem;
            if !pcb_size.is_null() {
                *pcb_size = b.len() as u32;
            }
            S_OK
        }
        Some(_) => MF_E_INVALIDTYPE,
        None => MF_E_ATTRIBUTENOTFOUND,
    }
}

unsafe extern "win64" fn attr_get_unknown(
    this: usize,
    guid_key: *const [u8; 16],
    riid: *const [u8; 16],
    ppv: *mut usize,
) -> i32 {
    if guid_key.is_null() || riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let ptr = {
        let g = hdr.attrs.map.lock().unwrap();
        match g.get(&*guid_key) {
            Some(AttrValue::Unknown(p)) => *p,
            Some(_) => return MF_E_INVALIDTYPE,
            None => return MF_E_ATTRIBUTENOTFOUND,
        }
    };
    if ptr == 0 {
        return E_NOINTERFACE;
    }
    com_query_interface(ptr, riid, ppv)
}

unsafe extern "win64" fn attr_set_item(
    this: usize,
    guid_key: *const [u8; 16],
    value: *const PropVariant,
) -> i32 {
    if guid_key.is_null() || value.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let pv = &*value;
    let val = match pv.vt {
        VT_UI4 => AttrValue::Uint32(pv.val_lo as u32),
        VT_UI8 | VT_I8 => AttrValue::Uint64(pv.val_lo),
        VT_R8 => AttrValue::Double(f64::from_bits(pv.val_lo)),
        VT_CLSID => {
            let p = pv.val_lo as *const [u8; 16];
            if p.is_null() {
                return E_POINTER;
            }
            AttrValue::Guid(*p)
        }
        VT_LPWSTR => {
            let p = pv.val_lo as *const u16;
            if p.is_null() {
                return E_POINTER;
            }
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            let mut v = std::slice::from_raw_parts(p, len + 1).to_vec();
            if *v.last().unwrap_or(&1) != 0 {
                v.push(0);
            }
            AttrValue::String(v)
        }
        VT_UNKNOWN => AttrValue::Unknown(pv.val_lo as usize),
        VT_VECTOR_UI1 => {
            let len = pv.val_lo as usize;
            let p = pv.val_hi as *const u8;
            let slice = if p.is_null() || len == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(p, len)
            };
            AttrValue::Blob(slice.to_vec())
        }
        _ => return E_INVALIDARG,
    };
    hdr.attrs.set(*guid_key, val);
    S_OK
}

unsafe extern "win64" fn attr_delete_item(this: usize, guid_key: *const [u8; 16]) -> i32 {
    if guid_key.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    hdr.attrs.delete(&*guid_key);
    S_OK
}

unsafe extern "win64" fn attr_delete_all_items(this: usize) -> i32 {
    let hdr = &*(this as *const MfAttrHeader);
    hdr.attrs.clear();
    S_OK
}

unsafe extern "win64" fn attr_set_uint32(
    this: usize,
    guid_key: *const [u8; 16],
    un_value: u32,
) -> i32 {
    if guid_key.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    hdr.attrs.set(*guid_key, AttrValue::Uint32(un_value));
    S_OK
}

unsafe extern "win64" fn attr_set_uint64(
    this: usize,
    guid_key: *const [u8; 16],
    un_value: u64,
) -> i32 {
    if guid_key.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    hdr.attrs.set(*guid_key, AttrValue::Uint64(un_value));
    S_OK
}

unsafe extern "win64" fn attr_set_double(
    this: usize,
    guid_key: *const [u8; 16],
    f_value: f64,
) -> i32 {
    if guid_key.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    hdr.attrs.set(*guid_key, AttrValue::Double(f_value));
    S_OK
}

unsafe extern "win64" fn attr_set_guid(
    this: usize,
    guid_key: *const [u8; 16],
    guid_value: *const [u8; 16],
) -> i32 {
    if guid_key.is_null() || guid_value.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    hdr.attrs.set(*guid_key, AttrValue::Guid(*guid_value));
    S_OK
}

unsafe extern "win64" fn attr_set_string(
    this: usize,
    guid_key: *const [u8; 16],
    wsz_value: *const u16,
) -> i32 {
    if guid_key.is_null() || wsz_value.is_null() {
        return E_POINTER;
    }
    let mut len = 0;
    while *wsz_value.add(len) != 0 {
        len += 1;
    }
    let v = std::slice::from_raw_parts(wsz_value, len + 1).to_vec();
    let hdr = &*(this as *const MfAttrHeader);
    hdr.attrs.set(*guid_key, AttrValue::String(v));
    S_OK
}

unsafe extern "win64" fn attr_set_blob(
    this: usize,
    guid_key: *const [u8; 16],
    p_buf: *const u8,
    cb_buf_size: u32,
) -> i32 {
    if guid_key.is_null() {
        return E_POINTER;
    }
    let slice = if p_buf.is_null() || cb_buf_size == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(p_buf, cb_buf_size as usize)
    };
    let hdr = &*(this as *const MfAttrHeader);
    hdr.attrs.set(*guid_key, AttrValue::Blob(slice.to_vec()));
    S_OK
}

unsafe extern "win64" fn attr_set_unknown(
    this: usize,
    guid_key: *const [u8; 16],
    p_unknown: usize,
) -> i32 {
    if guid_key.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    hdr.attrs.set(*guid_key, AttrValue::Unknown(p_unknown));
    S_OK
}

unsafe extern "win64" fn attr_lock_store(_this: usize) -> i32 {
    S_OK
}
unsafe extern "win64" fn attr_unlock_store(_this: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn attr_get_count(this: usize, pc_items: *mut u32) -> i32 {
    if pc_items.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    *pc_items = hdr.attrs.map.lock().unwrap().len() as u32;
    S_OK
}

unsafe extern "win64" fn attr_get_item_by_index(
    this: usize,
    un_index: u32,
    pguid_key: *mut [u8; 16],
    p_value: *mut PropVariant,
) -> i32 {
    if pguid_key.is_null() {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let key_opt = {
        let g = hdr.attrs.map.lock().unwrap();
        g.keys().nth(un_index as usize).copied()
    };
    let Some(k) = key_opt else {
        return E_INVALIDARG;
    };
    *pguid_key = k;
    if !p_value.is_null() {
        attr_get_item(this, &k, p_value)
    } else {
        S_OK
    }
}

unsafe extern "win64" fn attr_copy_all_items(this: usize, p_dest: usize) -> i32 {
    if p_dest == 0 {
        return E_POINTER;
    }
    let hdr = &*(this as *const MfAttrHeader);
    let items: Vec<([u8; 16], AttrValue)> = {
        let g = hdr.attrs.map.lock().unwrap();
        g.iter().map(|(k, v)| (*k, v.clone())).collect()
    };
    let dest_vtbl = *(p_dest as *const *const usize);
    for (k, v) in items {
        match v {
            AttrValue::Uint32(x) => {
                let f: unsafe extern "win64" fn(usize, *const [u8; 16], u32) -> i32 =
                    std::mem::transmute(*dest_vtbl.add(20));
                f(p_dest, &k, x);
            }
            AttrValue::Uint64(x) => {
                let f: unsafe extern "win64" fn(usize, *const [u8; 16], u64) -> i32 =
                    std::mem::transmute(*dest_vtbl.add(21));
                f(p_dest, &k, x);
            }
            AttrValue::Double(x) => {
                let f: unsafe extern "win64" fn(usize, *const [u8; 16], f64) -> i32 =
                    std::mem::transmute(*dest_vtbl.add(22));
                f(p_dest, &k, x);
            }
            AttrValue::Guid(x) => {
                let f: unsafe extern "win64" fn(usize, *const [u8; 16], *const [u8; 16]) -> i32 =
                    std::mem::transmute(*dest_vtbl.add(23));
                f(p_dest, &k, &x);
            }
            AttrValue::String(x) => {
                let f: unsafe extern "win64" fn(usize, *const [u8; 16], *const u16) -> i32 =
                    std::mem::transmute(*dest_vtbl.add(24));
                f(p_dest, &k, x.as_ptr());
            }
            AttrValue::Blob(x) => {
                let f: unsafe extern "win64" fn(usize, *const [u8; 16], *const u8, u32) -> i32 =
                    std::mem::transmute(*dest_vtbl.add(25));
                f(p_dest, &k, x.as_ptr(), x.len() as u32);
            }
            AttrValue::Unknown(x) => {
                let f: unsafe extern "win64" fn(usize, *const [u8; 16], usize) -> i32 =
                    std::mem::transmute(*dest_vtbl.add(26));
                f(p_dest, &k, x);
            }
        }
    }
    S_OK
}

#[repr(transparent)]
struct SyncVtbl<const N: usize>([*const (); N]);
unsafe impl<const N: usize> Sync for SyncVtbl<N> {}

impl<const N: usize> SyncVtbl<N> {
    const fn as_ptr(&self) -> *const usize {
        self.0.as_ptr() as *const usize
    }
}

macro_rules! make_attr_vtbl {
    ($n:expr, $qi:expr, $addref:expr, $release:expr $(, $extra:expr)* $(,)?) => {
        SyncVtbl([
            $qi as *const (),
            $addref as *const (),
            $release as *const (),
            attr_get_item as *const (),
            attr_get_item_type as *const (),
            attr_compare_item as *const (),
            attr_compare as *const (),
            attr_get_uint32 as *const (),
            attr_get_uint64 as *const (),
            attr_get_double as *const (),
            attr_get_guid as *const (),
            attr_get_string_length as *const (),
            attr_get_string as *const (),
            attr_get_allocated_string as *const (),
            attr_get_blob_size as *const (),
            attr_get_blob as *const (),
            attr_get_allocated_blob as *const (),
            attr_get_unknown as *const (),
            attr_set_item as *const (),
            attr_delete_item as *const (),
            attr_delete_all_items as *const (),
            attr_set_uint32 as *const (),
            attr_set_uint64 as *const (),
            attr_set_double as *const (),
            attr_set_guid as *const (),
            attr_set_string as *const (),
            attr_set_blob as *const (),
            attr_set_unknown as *const (),
            attr_lock_store as *const (),
            attr_unlock_store as *const (),
            attr_get_count as *const (),
            attr_get_item_by_index as *const (),
            attr_copy_all_items as *const (),
            $( $extra as *const () ),*
        ])
    };
}

// ============================================================================
// IMFAttributes Concrete Object
// ============================================================================

unsafe extern "win64" fn attrs_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAttributes {
        generic_add_ref::<MfAttrHeader>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

static MF_ATTRS_VTBL: SyncVtbl<33> = make_attr_vtbl!(
    33,
    attrs_qi,
    generic_add_ref::<MfAttrHeader>,
    generic_release::<MfAttrHeader>,
);

fn create_attributes() -> usize {
    Box::into_raw(Box::new(MfAttrHeader::new(MF_ATTRS_VTBL.as_ptr()))) as usize
}

// ============================================================================
// IMFMediaType
// ============================================================================

#[repr(C)]
struct MfMediaType {
    header: MfAttrHeader,
}

impl HasRefCount for MfMediaType {
    fn ref_count(&self) -> &AtomicU32 {
        &self.header.ref_count
    }
}

unsafe extern "win64" fn mt_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAttributes || id == IID_IMFMediaType {
        generic_add_ref::<MfMediaType>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn mt_get_major_type(this: usize, pguid_major_type: *mut [u8; 16]) -> i32 {
    attr_get_guid(this, &MF_MT_MAJOR_TYPE, pguid_major_type)
}

unsafe extern "win64" fn mt_is_compressed_format(this: usize, pf_compressed: *mut i32) -> i32 {
    if pf_compressed.is_null() {
        return E_POINTER;
    }
    let mut sub = GUID_NULL;
    if attr_get_guid(this, &MF_MT_SUBTYPE, &mut sub) == S_OK && sub == MFVideoFormat_H264 {
        *pf_compressed = 1;
    } else {
        *pf_compressed = 0;
    }
    S_OK
}

unsafe extern "win64" fn mt_is_equal(
    this: usize,
    p_media_type: usize,
    pdw_flags: *mut u32,
) -> i32 {
    if p_media_type == 0 {
        return E_POINTER;
    }
    let mut maj1 = GUID_NULL;
    let mut maj2 = GUID_NULL;
    let mut sub1 = GUID_NULL;
    let mut sub2 = GUID_NULL;
    attr_get_guid(this, &MF_MT_MAJOR_TYPE, &mut maj1);
    attr_get_guid(this, &MF_MT_SUBTYPE, &mut sub1);
    let vtbl2 = *(p_media_type as *const *const usize);
    let get_guid2: unsafe extern "win64" fn(usize, *const [u8; 16], *mut [u8; 16]) -> i32 =
        std::mem::transmute(*vtbl2.add(10));
    get_guid2(p_media_type, &MF_MT_MAJOR_TYPE, &mut maj2);
    get_guid2(p_media_type, &MF_MT_SUBTYPE, &mut sub2);
    if !pdw_flags.is_null() {
        *pdw_flags = if maj1 == maj2 && sub1 == sub2 { 1 } else { 0 };
    }
    if maj1 == maj2 && sub1 == sub2 {
        S_OK
    } else {
        S_FALSE
    }
}

unsafe extern "win64" fn mt_get_representation(
    _this: usize,
    _guid_rep: [u8; 16],
    _ppv_rep: *mut usize,
) -> i32 {
    E_NOTIMPL
}

unsafe extern "win64" fn mt_free_representation(
    _this: usize,
    _guid_rep: [u8; 16],
    _pv_rep: usize,
) -> i32 {
    S_OK
}

static MF_MEDIA_TYPE_VTBL: SyncVtbl<38> = make_attr_vtbl!(
    38,
    mt_qi,
    generic_add_ref::<MfMediaType>,
    generic_release::<MfMediaType>,
    mt_get_major_type,
    mt_is_compressed_format,
    mt_is_equal,
    mt_get_representation,
    mt_free_representation,
);

fn create_media_type() -> usize {
    Box::into_raw(Box::new(MfMediaType {
        header: MfAttrHeader::new(MF_MEDIA_TYPE_VTBL.as_ptr()),
    })) as usize
}

// ============================================================================
// IMFMediaBuffer
// ============================================================================

#[repr(C)]
struct MfMediaBuffer {
    vtable: *const usize,
    ref_count: AtomicU32,
    data: Mutex<Vec<u8>>,
    cur_len: AtomicU32,
    max_len: u32,
}

impl HasRefCount for MfMediaBuffer {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

unsafe extern "win64" fn buf_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFMediaBuffer {
        generic_add_ref::<MfMediaBuffer>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn buf_lock(
    this: usize,
    ppb_buffer: *mut *mut u8,
    pcb_max_length: *mut u32,
    pcb_current_length: *mut u32,
) -> i32 {
    let b = &*(this as *const MfMediaBuffer);
    let mut g = b.data.lock().unwrap();
    if !ppb_buffer.is_null() {
        *ppb_buffer = g.as_mut_ptr();
    }
    if !pcb_max_length.is_null() {
        *pcb_max_length = b.max_len;
    }
    if !pcb_current_length.is_null() {
        *pcb_current_length = b.cur_len.load(Ordering::SeqCst);
    }
    S_OK
}

unsafe extern "win64" fn buf_unlock(_this: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn buf_get_current_length(
    this: usize,
    pcb_current_length: *mut u32,
) -> i32 {
    if pcb_current_length.is_null() {
        return E_POINTER;
    }
    let b = &*(this as *const MfMediaBuffer);
    *pcb_current_length = b.cur_len.load(Ordering::SeqCst);
    S_OK
}

unsafe extern "win64" fn buf_set_current_length(this: usize, cb_current_length: u32) -> i32 {
    let b = &*(this as *const MfMediaBuffer);
    if cb_current_length > b.max_len {
        return E_INVALIDARG;
    }
    b.cur_len.store(cb_current_length, Ordering::SeqCst);
    S_OK
}

unsafe extern "win64" fn buf_get_max_length(this: usize, pcb_max_length: *mut u32) -> i32 {
    if pcb_max_length.is_null() {
        return E_POINTER;
    }
    let b = &*(this as *const MfMediaBuffer);
    *pcb_max_length = b.max_len;
    S_OK
}

static MF_MEDIA_BUFFER_VTBL: SyncVtbl<8> = SyncVtbl([
    buf_qi as *const (),
    generic_add_ref::<MfMediaBuffer> as *const (),
    generic_release::<MfMediaBuffer> as *const (),
    buf_lock as *const (),
    buf_unlock as *const (),
    buf_get_current_length as *const (),
    buf_set_current_length as *const (),
    buf_get_max_length as *const (),
]);

fn create_memory_buffer(max_len: u32) -> usize {
    Box::into_raw(Box::new(MfMediaBuffer {
        vtable: MF_MEDIA_BUFFER_VTBL.as_ptr(),
        ref_count: AtomicU32::new(1),
        data: Mutex::new(vec![0u8; max_len as usize]),
        cur_len: AtomicU32::new(0),
        max_len,
    })) as usize
}

// ============================================================================
// IMFSample
// ============================================================================

#[repr(C)]
struct MfSample {
    header: MfAttrHeader,
    flags: AtomicU32,
    sample_time: Mutex<i64>,
    sample_duration: Mutex<i64>,
    buffers: Mutex<Vec<usize>>,
}

impl HasRefCount for MfSample {
    fn ref_count(&self) -> &AtomicU32 {
        &self.header.ref_count
    }
}

impl Drop for MfSample {
    fn drop(&mut self) {
        let bufs: Vec<usize> = self.buffers.lock().unwrap().drain(..).collect();
        for b in bufs {
            unsafe { com_release(b) };
        }
    }
}

unsafe extern "win64" fn sample_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAttributes || id == IID_IMFSample {
        generic_add_ref::<MfSample>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn sample_get_flags(this: usize, pdw_flags: *mut u32) -> i32 {
    if pdw_flags.is_null() {
        return E_POINTER;
    }
    let s = &*(this as *const MfSample);
    *pdw_flags = s.flags.load(Ordering::SeqCst);
    S_OK
}

unsafe extern "win64" fn sample_set_flags(this: usize, dw_flags: u32) -> i32 {
    let s = &*(this as *const MfSample);
    s.flags.store(dw_flags, Ordering::SeqCst);
    S_OK
}

unsafe extern "win64" fn sample_get_time(this: usize, phns_time: *mut i64) -> i32 {
    if phns_time.is_null() {
        return E_POINTER;
    }
    let s = &*(this as *const MfSample);
    *phns_time = *s.sample_time.lock().unwrap();
    S_OK
}

unsafe extern "win64" fn sample_set_time(this: usize, hns_time: i64) -> i32 {
    let s = &*(this as *const MfSample);
    *s.sample_time.lock().unwrap() = hns_time;
    S_OK
}

unsafe extern "win64" fn sample_get_duration(this: usize, phns_dur: *mut i64) -> i32 {
    if phns_dur.is_null() {
        return E_POINTER;
    }
    let s = &*(this as *const MfSample);
    *phns_dur = *s.sample_duration.lock().unwrap();
    S_OK
}

unsafe extern "win64" fn sample_set_duration(this: usize, hns_dur: i64) -> i32 {
    let s = &*(this as *const MfSample);
    *s.sample_duration.lock().unwrap() = hns_dur;
    S_OK
}

unsafe extern "win64" fn sample_get_buffer_count(this: usize, pdw_count: *mut u32) -> i32 {
    if pdw_count.is_null() {
        return E_POINTER;
    }
    let s = &*(this as *const MfSample);
    *pdw_count = s.buffers.lock().unwrap().len() as u32;
    S_OK
}

unsafe extern "win64" fn sample_get_buffer_by_index(
    this: usize,
    dw_index: u32,
    pp_buffer: *mut usize,
) -> i32 {
    if pp_buffer.is_null() {
        return E_POINTER;
    }
    let s = &*(this as *const MfSample);
    let g = s.buffers.lock().unwrap();
    if let Some(&b) = g.get(dw_index as usize) {
        com_add_ref(b);
        *pp_buffer = b;
        S_OK
    } else {
        E_INVALIDARG
    }
}

unsafe extern "win64" fn sample_convert_to_contiguous_buffer(
    this: usize,
    pp_buffer: *mut usize,
) -> i32 {
    if pp_buffer.is_null() {
        return E_POINTER;
    }
    let s = &*(this as *const MfSample);
    let g = s.buffers.lock().unwrap();
    if g.is_empty() {
        return E_FAIL;
    }
    if g.len() == 1 {
        com_add_ref(g[0]);
        *pp_buffer = g[0];
        return S_OK;
    }
    let mut total = 0u32;
    for &b in g.iter() {
        let mut len = 0u32;
        buf_get_current_length(b, &mut len);
        total += len;
    }
    let new_buf = create_memory_buffer(total);
    let mut dst_ptr: *mut u8 = std::ptr::null_mut();
    buf_lock(new_buf, &mut dst_ptr, std::ptr::null_mut(), std::ptr::null_mut());
    let mut offset = 0usize;
    for &b in g.iter() {
        let mut src_ptr: *mut u8 = std::ptr::null_mut();
        let mut len = 0u32;
        buf_lock(b, &mut src_ptr, std::ptr::null_mut(), &mut len);
        if !src_ptr.is_null() && len > 0 {
            std::ptr::copy_nonoverlapping(src_ptr, dst_ptr.add(offset), len as usize);
            offset += len as usize;
        }
        buf_unlock(b);
    }
    buf_unlock(new_buf);
    buf_set_current_length(new_buf, total);
    *pp_buffer = new_buf;
    S_OK
}

unsafe extern "win64" fn sample_add_buffer(this: usize, p_buffer: usize) -> i32 {
    if p_buffer == 0 {
        return E_POINTER;
    }
    let s = &*(this as *const MfSample);
    com_add_ref(p_buffer);
    s.buffers.lock().unwrap().push(p_buffer);
    S_OK
}

unsafe extern "win64" fn sample_remove_buffer_by_index(this: usize, dw_index: u32) -> i32 {
    let s = &*(this as *const MfSample);
    let mut g = s.buffers.lock().unwrap();
    if (dw_index as usize) >= g.len() {
        return E_INVALIDARG;
    }
    let b = g.remove(dw_index as usize);
    com_release(b);
    S_OK
}

unsafe extern "win64" fn sample_remove_all_buffers(this: usize) -> i32 {
    let s = &*(this as *const MfSample);
    let bufs: Vec<usize> = s.buffers.lock().unwrap().drain(..).collect();
    for b in bufs {
        com_release(b);
    }
    S_OK
}

unsafe extern "win64" fn sample_get_total_length(this: usize, pcb_total: *mut u32) -> i32 {
    if pcb_total.is_null() {
        return E_POINTER;
    }
    let s = &*(this as *const MfSample);
    let g = s.buffers.lock().unwrap();
    let mut total = 0u32;
    for &b in g.iter() {
        let vtbl = *(b as *const *const usize);
        let f: unsafe extern "win64" fn(usize, *mut u32) -> i32 = std::mem::transmute(*vtbl.add(5));
        let mut len = 0u32;
        if f(b, &mut len) == S_OK {
            total += len;
        }
    }
    *pcb_total = total;
    S_OK
}

unsafe extern "win64" fn sample_copy_to_buffer(this: usize, p_buffer: usize) -> i32 {
    if p_buffer == 0 {
        return E_POINTER;
    }
    let mut dst_ptr: *mut u8 = std::ptr::null_mut();
    let mut dst_max = 0u32;
    let vtbl_dst = *(p_buffer as *const *const usize);
    let lock_dst: unsafe extern "win64" fn(usize, *mut *mut u8, *mut u32, *mut u32) -> i32 =
        std::mem::transmute(*vtbl_dst.add(3));
    let unlock_dst: unsafe extern "win64" fn(usize) -> i32 = std::mem::transmute(*vtbl_dst.add(4));
    let set_len_dst: unsafe extern "win64" fn(usize, u32) -> i32 =
        std::mem::transmute(*vtbl_dst.add(6));
    lock_dst(p_buffer, &mut dst_ptr, &mut dst_max, std::ptr::null_mut());

    let s = &*(this as *const MfSample);
    let g = s.buffers.lock().unwrap();
    let mut off = 0u32;
    for &b in g.iter() {
        let vtbl_src = *(b as *const *const usize);
        let lock_src: unsafe extern "win64" fn(usize, *mut *mut u8, *mut u32, *mut u32) -> i32 =
            std::mem::transmute(*vtbl_src.add(3));
        let unlock_src: unsafe extern "win64" fn(usize) -> i32 =
            std::mem::transmute(*vtbl_src.add(4));
        let mut src_ptr: *mut u8 = std::ptr::null_mut();
        let mut src_len = 0u32;
        lock_src(b, &mut src_ptr, std::ptr::null_mut(), &mut src_len);
        if off + src_len > dst_max {
            unlock_src(b);
            unlock_dst(p_buffer);
            return MF_E_BUFFERTOOSMALL;
        }
        if !src_ptr.is_null() && !dst_ptr.is_null() && src_len > 0 {
            std::ptr::copy_nonoverlapping(src_ptr, dst_ptr.add(off as usize), src_len as usize);
            off += src_len;
        }
        unlock_src(b);
    }
    unlock_dst(p_buffer);
    set_len_dst(p_buffer, off);
    S_OK
}

static MF_SAMPLE_VTBL: SyncVtbl<47> = make_attr_vtbl!(
    47,
    sample_qi,
    generic_add_ref::<MfSample>,
    generic_release::<MfSample>,
    sample_get_flags,
    sample_set_flags,
    sample_get_time,
    sample_set_time,
    sample_get_duration,
    sample_set_duration,
    sample_get_buffer_count,
    sample_get_buffer_by_index,
    sample_convert_to_contiguous_buffer,
    sample_add_buffer,
    sample_remove_buffer_by_index,
    sample_remove_all_buffers,
    sample_get_total_length,
    sample_copy_to_buffer,
);

fn create_sample() -> usize {
    Box::into_raw(Box::new(MfSample {
        header: MfAttrHeader::new(MF_SAMPLE_VTBL.as_ptr()),
        flags: AtomicU32::new(0),
        sample_time: Mutex::new(0),
        sample_duration: Mutex::new(0),
        buffers: Mutex::new(Vec::new()),
    })) as usize
}

// ============================================================================
// IMFAsyncResult
// ============================================================================

#[repr(C)]
struct MfAsyncResult {
    vtable: *const usize,
    ref_count: AtomicU32,
    object: usize,
    callback: usize,
    state: usize,
    status: AtomicU32,
}

impl HasRefCount for MfAsyncResult {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

impl Drop for MfAsyncResult {
    fn drop(&mut self) {
        unsafe {
            com_release(self.object);
            com_release(self.callback);
            com_release(self.state);
        }
    }
}

unsafe extern "win64" fn ar_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAsyncResult {
        generic_add_ref::<MfAsyncResult>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn ar_get_state(this: usize, ppunk_state: *mut usize) -> i32 {
    if ppunk_state.is_null() {
        return E_POINTER;
    }
    let ar = &*(this as *const MfAsyncResult);
    if ar.state == 0 {
        *ppunk_state = 0;
        return E_POINTER;
    }
    com_add_ref(ar.state);
    *ppunk_state = ar.state;
    S_OK
}

unsafe extern "win64" fn ar_get_status(this: usize) -> i32 {
    let ar = &*(this as *const MfAsyncResult);
    ar.status.load(Ordering::SeqCst) as i32
}

unsafe extern "win64" fn ar_set_status(this: usize, hr_status: i32) -> i32 {
    let ar = &*(this as *const MfAsyncResult);
    ar.status.store(hr_status as u32, Ordering::SeqCst);
    S_OK
}

unsafe extern "win64" fn ar_get_object(this: usize, pp_object: *mut usize) -> i32 {
    if pp_object.is_null() {
        return E_POINTER;
    }
    let ar = &*(this as *const MfAsyncResult);
    if ar.object == 0 {
        *pp_object = 0;
        return E_POINTER;
    }
    com_add_ref(ar.object);
    *pp_object = ar.object;
    S_OK
}

unsafe extern "win64" fn ar_get_state_no_add_ref(this: usize) -> usize {
    let ar = &*(this as *const MfAsyncResult);
    ar.state
}

static MF_ASYNC_RESULT_VTBL: SyncVtbl<8> = SyncVtbl([
    ar_qi as *const (),
    generic_add_ref::<MfAsyncResult> as *const (),
    generic_release::<MfAsyncResult> as *const (),
    ar_get_state as *const (),
    ar_get_status as *const (),
    ar_set_status as *const (),
    ar_get_object as *const (),
    ar_get_state_no_add_ref as *const (),
]);

fn create_async_result(object: usize, callback: usize, state: usize) -> usize {
    unsafe {
        com_add_ref(object);
        com_add_ref(callback);
        com_add_ref(state);
    }
    Box::into_raw(Box::new(MfAsyncResult {
        vtable: MF_ASYNC_RESULT_VTBL.as_ptr(),
        ref_count: AtomicU32::new(1),
        object,
        callback,
        state,
        status: AtomicU32::new(S_OK as u32),
    })) as usize
}

unsafe fn invoke_async_result(ar_ptr: usize) -> i32 {
    if ar_ptr == 0 {
        return E_POINTER;
    }
    let ar = &*(ar_ptr as *const MfAsyncResult);
    let cb = ar.callback;
    if cb == 0 {
        return E_POINTER;
    }
    let vtbl = *(cb as *const *const usize);
    let invoke: unsafe extern "win64" fn(usize, usize) -> i32 =
        std::mem::transmute(*vtbl.add(4));
    println!(
        "[MF] invoke_async_result(ar=0x{:X}, cb=0x{:X}, obj=0x{:X}, status=0x{:08X})",
        ar_ptr,
        cb,
        ar.object,
        ar.status.load(Ordering::SeqCst)
    );
    let hr = invoke(cb, ar_ptr);
    println!("[MF] invoke_async_result(ar=0x{:X}) -> 0x{:08X}", ar_ptr, hr as u32);
    hr
}

// ============================================================================
// IMFMediaEvent
// ============================================================================

#[repr(C)]
struct MfMediaEvent {
    header: MfAttrHeader,
    event_type: u32,
    extended_type: [u8; 16],
    status: i32,
    value: PropVariant,
}

impl HasRefCount for MfMediaEvent {
    fn ref_count(&self) -> &AtomicU32 {
        &self.header.ref_count
    }
}

impl Drop for MfMediaEvent {
    fn drop(&mut self) {
        if self.value.vt == VT_UNKNOWN && self.value.val_lo != 0 {
            unsafe { com_release(self.value.val_lo as usize) };
        }
    }
}

unsafe extern "win64" fn me_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAttributes || id == IID_IMFMediaEvent {
        generic_add_ref::<MfMediaEvent>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn me_get_type(this: usize, pmet: *mut u32) -> i32 {
    if pmet.is_null() {
        return E_POINTER;
    }
    let ev = &*(this as *const MfMediaEvent);
    *pmet = ev.event_type;
    S_OK
}

unsafe extern "win64" fn me_get_extended_type(this: usize, pguid: *mut [u8; 16]) -> i32 {
    if pguid.is_null() {
        return E_POINTER;
    }
    let ev = &*(this as *const MfMediaEvent);
    *pguid = ev.extended_type;
    S_OK
}

unsafe extern "win64" fn me_get_status(this: usize, phr_status: *mut i32) -> i32 {
    if phr_status.is_null() {
        return E_POINTER;
    }
    let ev = &*(this as *const MfMediaEvent);
    *phr_status = ev.status;
    S_OK
}

unsafe extern "win64" fn me_get_value(this: usize, pv_value: *mut PropVariant) -> i32 {
    if pv_value.is_null() {
        return E_POINTER;
    }
    let ev = &*(this as *const MfMediaEvent);
    *pv_value = ev.value;
    if ev.value.vt == VT_UNKNOWN && ev.value.val_lo != 0 {
        com_add_ref(ev.value.val_lo as usize);
    }
    S_OK
}

static MF_MEDIA_EVENT_VTBL: SyncVtbl<37> = make_attr_vtbl!(
    37,
    me_qi,
    generic_add_ref::<MfMediaEvent>,
    generic_release::<MfMediaEvent>,
    me_get_type,
    me_get_extended_type,
    me_get_status,
    me_get_value,
);

fn create_media_event(
    event_type: u32,
    extended_type: [u8; 16],
    status: i32,
    value: PropVariant,
) -> usize {
    if value.vt == VT_UNKNOWN && value.val_lo != 0 {
        unsafe { com_add_ref(value.val_lo as usize) };
    }
    Box::into_raw(Box::new(MfMediaEvent {
        header: MfAttrHeader::new(MF_MEDIA_EVENT_VTBL.as_ptr()),
        event_type,
        extended_type,
        status,
        value,
    })) as usize
}

// ============================================================================
// IMFMediaEventQueue
// ============================================================================

struct EventQueueState {
    events: VecDeque<usize>,
    pending_callbacks: VecDeque<(usize, usize)>,
    shutdown: bool,
}

#[repr(C)]
struct MfMediaEventQueue {
    vtable: *const usize,
    ref_count: AtomicU32,
    state: Mutex<EventQueueState>,
    cvar: Condvar,
}

impl HasRefCount for MfMediaEventQueue {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

impl Drop for MfMediaEventQueue {
    fn drop(&mut self) {
        let mut g = self.state.lock().unwrap();
        for ev in g.events.drain(..) {
            unsafe { com_release(ev) };
        }
        for (cb, st) in g.pending_callbacks.drain(..) {
            unsafe {
                com_release(cb);
                com_release(st);
            }
        }
    }
}

unsafe extern "win64" fn eq_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFMediaEventQueue {
        generic_add_ref::<MfMediaEventQueue>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn eq_get_event(this: usize, dw_flags: u32, pp_event: *mut usize) -> i32 {
    if pp_event.is_null() {
        return E_POINTER;
    }
    let eq = &*(this as *const MfMediaEventQueue);
    let mut g = eq.state.lock().unwrap();
    if (dw_flags & MF_EVENT_FLAG_NO_WAIT) != 0 {
        if let Some(ev) = g.events.pop_front() {
            *pp_event = ev;
            return S_OK;
        }
        *pp_event = 0;
        return if g.shutdown {
            MF_E_SHUTDOWN
        } else {
            MF_E_NO_EVENTS_AVAILABLE
        };
    }
    while g.events.is_empty() && !g.shutdown {
        g = eq.cvar.wait(g).unwrap();
    }
    if let Some(ev) = g.events.pop_front() {
        *pp_event = ev;
        S_OK
    } else {
        *pp_event = 0;
        MF_E_SHUTDOWN
    }
}

fn dispatch_event_callback(ev: usize, cb: usize, st: usize) {
    let ar = create_async_result(ev, cb, st);
    println!(
        "[MF] dispatch_event_callback(ev=0x{:X}, cb=0x{:X}, st=0x{:X}, ar=0x{:X})",
        ev, cb, st, ar
    );
    unsafe {
        com_release(ev);
        com_release(cb);
        com_release(st);
    }
    std::thread::spawn(move || {
        crate::dlls::kernel32::ensure_current_thread_teb();
        unsafe {
            invoke_async_result(ar);
            com_release(ar);
        }
    });
}

unsafe extern "win64" fn eq_begin_get_event(
    this: usize,
    p_callback: usize,
    punk_state: usize,
) -> i32 {
    if p_callback == 0 {
        return E_POINTER;
    }
    let eq = &*(this as *const MfMediaEventQueue);
    let mut g = eq.state.lock().unwrap();
    if g.shutdown {
        return MF_E_SHUTDOWN;
    }
    com_add_ref(p_callback);
    com_add_ref(punk_state);
    if let Some(ev) = g.events.pop_front() {
        drop(g);
        dispatch_event_callback(ev, p_callback, punk_state);
        return S_OK;
    }
    g.pending_callbacks.push_back((p_callback, punk_state));
    S_OK
}

unsafe extern "win64" fn eq_end_get_event(
    _this: usize,
    p_result: usize,
    pp_event: *mut usize,
) -> i32 {
    if p_result == 0 || pp_event.is_null() {
        return E_POINTER;
    }
    ar_get_object(p_result, pp_event)
}

unsafe extern "win64" fn eq_queue_event(this: usize, p_event: usize) -> i32 {
    if p_event == 0 {
        return E_POINTER;
    }
    let eq = &*(this as *const MfMediaEventQueue);
    let mut g = eq.state.lock().unwrap();
    if g.shutdown {
        return MF_E_SHUTDOWN;
    }
    com_add_ref(p_event);
    if let Some((cb, st)) = g.pending_callbacks.pop_front() {
        drop(g);
        dispatch_event_callback(p_event, cb, st);
        return S_OK;
    }
    g.events.push_back(p_event);
    eq.cvar.notify_one();
    S_OK
}

unsafe extern "win64" fn eq_queue_event_param_var(
    this: usize,
    met: u32,
    guid_ext: *const [u8; 16],
    hr_status: i32,
    pv_value: *const PropVariant,
) -> i32 {
    let ext = if guid_ext.is_null() {
        GUID_NULL
    } else {
        *guid_ext
    };
    let val = if pv_value.is_null() {
        PropVariant {
            vt: VT_EMPTY,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
            val_lo: 0,
            val_hi: 0,
        }
    } else {
        *pv_value
    };
    let ev = create_media_event(met, ext, hr_status, val);
    let hr = eq_queue_event(this, ev);
    com_release(ev);
    hr
}

unsafe extern "win64" fn eq_queue_event_param_unk(
    this: usize,
    met: u32,
    guid_ext: *const [u8; 16],
    hr_status: i32,
    p_unk: usize,
) -> i32 {
    let pv = PropVariant {
        vt: if p_unk != 0 { VT_UNKNOWN } else { VT_EMPTY },
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
        val_lo: p_unk as u64,
        val_hi: 0,
    };
    eq_queue_event_param_var(this, met, guid_ext, hr_status, &pv)
}

unsafe extern "win64" fn eq_shutdown(this: usize) -> i32 {
    let eq = &*(this as *const MfMediaEventQueue);
    let mut g = eq.state.lock().unwrap();
    g.shutdown = true;
    for ev in g.events.drain(..) {
        com_release(ev);
    }
    for (cb, st) in g.pending_callbacks.drain(..) {
        com_release(cb);
        com_release(st);
    }
    eq.cvar.notify_all();
    S_OK
}

static MF_EVENT_QUEUE_VTBL: SyncVtbl<10> = SyncVtbl([
    eq_qi as *const (),
    generic_add_ref::<MfMediaEventQueue> as *const (),
    generic_release::<MfMediaEventQueue> as *const (),
    eq_get_event as *const (),
    eq_begin_get_event as *const (),
    eq_end_get_event as *const (),
    eq_queue_event as *const (),
    eq_queue_event_param_var as *const (),
    eq_queue_event_param_unk as *const (),
    eq_shutdown as *const (),
]);

fn create_event_queue() -> usize {
    Box::into_raw(Box::new(MfMediaEventQueue {
        vtable: MF_EVENT_QUEUE_VTBL.as_ptr(),
        ref_count: AtomicU32::new(1),
        state: Mutex::new(EventQueueState {
            events: VecDeque::new(),
            pending_callbacks: VecDeque::new(),
            shutdown: false,
        }),
        cvar: Condvar::new(),
    })) as usize
}

// ============================================================================
// IMFMediaTypeHandler, IMFStreamDescriptor, IMFPresentationDescriptor, IMFMediaSource
// ============================================================================

#[repr(C)]
struct MfMediaTypeHandler {
    vtable: *const usize,
    ref_count: AtomicU32,
    media_types: Mutex<Vec<usize>>,
    current_type: Mutex<usize>,
    major_type: [u8; 16],
}

impl HasRefCount for MfMediaTypeHandler {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

impl Drop for MfMediaTypeHandler {
    fn drop(&mut self) {
        let cur = *self.current_type.lock().unwrap();
        unsafe { com_release(cur) };
        for mt in self.media_types.lock().unwrap().drain(..) {
            unsafe { com_release(mt) };
        }
    }
}

unsafe extern "win64" fn mth_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFMediaTypeHandler {
        generic_add_ref::<MfMediaTypeHandler>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn mth_is_supported(
    this: usize,
    p_media_type: usize,
    pp_media_type: *mut usize,
) -> i32 {
    if p_media_type == 0 {
        return E_POINTER;
    }
    if !pp_media_type.is_null() {
        *pp_media_type = 0;
    }
    let mth = &*(this as *const MfMediaTypeHandler);
    let mut maj = GUID_NULL;
    let vtbl = *(p_media_type as *const *const usize);
    let get_guid: unsafe extern "win64" fn(usize, *const [u8; 16], *mut [u8; 16]) -> i32 =
        std::mem::transmute(*vtbl.add(10));
    if get_guid(p_media_type, &MF_MT_MAJOR_TYPE, &mut maj) == S_OK && maj == mth.major_type {
        S_OK
    } else {
        0xC00D36B4_u32 as i32 // MF_E_INVALIDMEDIATYPE
    }
}

unsafe extern "win64" fn mth_get_count(this: usize, pdw_count: *mut u32) -> i32 {
    if pdw_count.is_null() {
        return E_POINTER;
    }
    let mth = &*(this as *const MfMediaTypeHandler);
    *pdw_count = mth.media_types.lock().unwrap().len() as u32;
    S_OK
}

unsafe extern "win64" fn mth_get_by_index(
    this: usize,
    dw_index: u32,
    pp_type: *mut usize,
) -> i32 {
    if pp_type.is_null() {
        return E_POINTER;
    }
    let mth = &*(this as *const MfMediaTypeHandler);
    let g = mth.media_types.lock().unwrap();
    if let Some(&mt) = g.get(dw_index as usize) {
        com_add_ref(mt);
        *pp_type = mt;
        S_OK
    } else {
        *pp_type = 0;
        0xC00D36B5_u32 as i32 // MF_E_NO_MORE_TYPES
    }
}

unsafe extern "win64" fn mth_set_current(this: usize, p_media_type: usize) -> i32 {
    if p_media_type == 0 {
        return E_POINTER;
    }
    let mth = &*(this as *const MfMediaTypeHandler);
    com_add_ref(p_media_type);
    let mut cur = mth.current_type.lock().unwrap();
    let old = *cur;
    *cur = p_media_type;
    drop(cur);
    com_release(old);
    S_OK
}

unsafe extern "win64" fn mth_get_current(this: usize, pp_media_type: *mut usize) -> i32 {
    if pp_media_type.is_null() {
        return E_POINTER;
    }
    let mth = &*(this as *const MfMediaTypeHandler);
    let cur = *mth.current_type.lock().unwrap();
    if cur == 0 {
        *pp_media_type = 0;
        return MF_E_ATTRIBUTENOTFOUND;
    }
    com_add_ref(cur);
    *pp_media_type = cur;
    S_OK
}

unsafe extern "win64" fn mth_get_major_type(
    this: usize,
    pguid_major_type: *mut [u8; 16],
) -> i32 {
    if pguid_major_type.is_null() {
        return E_POINTER;
    }
    let mth = &*(this as *const MfMediaTypeHandler);
    *pguid_major_type = mth.major_type;
    S_OK
}

static MF_MEDIA_TYPE_HANDLER_VTBL: SyncVtbl<9> = SyncVtbl([
    mth_qi as *const (),
    generic_add_ref::<MfMediaTypeHandler> as *const (),
    generic_release::<MfMediaTypeHandler> as *const (),
    mth_is_supported as *const (),
    mth_get_count as *const (),
    mth_get_by_index as *const (),
    mth_set_current as *const (),
    mth_get_current as *const (),
    mth_get_major_type as *const (),
]);

fn create_media_type_handler(major_type: [u8; 16], media_types: Vec<usize>) -> usize {
    let cur = media_types.first().copied().unwrap_or(0);
    unsafe { com_add_ref(cur) };
    Box::into_raw(Box::new(MfMediaTypeHandler {
        vtable: MF_MEDIA_TYPE_HANDLER_VTBL.as_ptr(),
        ref_count: AtomicU32::new(1),
        media_types: Mutex::new(media_types),
        current_type: Mutex::new(cur),
        major_type,
    })) as usize
}

// IMFStreamDescriptor
#[repr(C)]
struct MfStreamDescriptor {
    header: MfAttrHeader,
    stream_id: u32,
    type_handler: usize,
}

impl HasRefCount for MfStreamDescriptor {
    fn ref_count(&self) -> &AtomicU32 {
        &self.header.ref_count
    }
}

impl Drop for MfStreamDescriptor {
    fn drop(&mut self) {
        unsafe { com_release(self.type_handler) };
    }
}

unsafe extern "win64" fn sd_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAttributes || id == IID_IMFStreamDescriptor {
        generic_add_ref::<MfStreamDescriptor>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn sd_get_stream_identifier(
    this: usize,
    pdw_stream_id: *mut u32,
) -> i32 {
    if pdw_stream_id.is_null() {
        return E_POINTER;
    }
    let sd = &*(this as *const MfStreamDescriptor);
    *pdw_stream_id = sd.stream_id;
    S_OK
}

unsafe extern "win64" fn sd_get_media_type_handler(
    this: usize,
    pp_handler: *mut usize,
) -> i32 {
    if pp_handler.is_null() {
        return E_POINTER;
    }
    let sd = &*(this as *const MfStreamDescriptor);
    com_add_ref(sd.type_handler);
    *pp_handler = sd.type_handler;
    S_OK
}

static MF_STREAM_DESCRIPTOR_VTBL: SyncVtbl<35> = make_attr_vtbl!(
    35,
    sd_qi,
    generic_add_ref::<MfStreamDescriptor>,
    generic_release::<MfStreamDescriptor>,
    sd_get_stream_identifier,
    sd_get_media_type_handler,
);

fn create_stream_descriptor(stream_id: u32, type_handler: usize) -> usize {
    Box::into_raw(Box::new(MfStreamDescriptor {
        header: MfAttrHeader::new(MF_STREAM_DESCRIPTOR_VTBL.as_ptr()),
        stream_id,
        type_handler,
    })) as usize
}

// IMFPresentationDescriptor
#[repr(C)]
struct MfPresentationDescriptor {
    header: MfAttrHeader,
    streams: Mutex<Vec<(usize, bool)>>,
}

impl HasRefCount for MfPresentationDescriptor {
    fn ref_count(&self) -> &AtomicU32 {
        &self.header.ref_count
    }
}

impl Drop for MfPresentationDescriptor {
    fn drop(&mut self) {
        for (sd, _) in self.streams.lock().unwrap().drain(..) {
            unsafe { com_release(sd) };
        }
    }
}

unsafe extern "win64" fn pd_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAttributes || id == IID_IMFPresentationDescriptor {
        generic_add_ref::<MfPresentationDescriptor>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn pd_get_stream_count(this: usize, pdw_count: *mut u32) -> i32 {
    if pdw_count.is_null() {
        return E_POINTER;
    }
    let pd = &*(this as *const MfPresentationDescriptor);
    *pdw_count = pd.streams.lock().unwrap().len() as u32;
    S_OK
}

unsafe extern "win64" fn pd_get_stream_by_index(
    this: usize,
    dw_index: u32,
    pf_selected: *mut i32,
    pp_descriptor: *mut usize,
) -> i32 {
    if pp_descriptor.is_null() {
        return E_POINTER;
    }
    let pd = &*(this as *const MfPresentationDescriptor);
    let g = pd.streams.lock().unwrap();
    if let Some(&(sd, sel)) = g.get(dw_index as usize) {
        if !pf_selected.is_null() {
            *pf_selected = if sel { 1 } else { 0 };
        }
        com_add_ref(sd);
        *pp_descriptor = sd;
        S_OK
    } else {
        *pp_descriptor = 0;
        E_INVALIDARG
    }
}

unsafe extern "win64" fn pd_select_stream(this: usize, dw_index: u32) -> i32 {
    let pd = &*(this as *const MfPresentationDescriptor);
    let mut g = pd.streams.lock().unwrap();
    if let Some(entry) = g.get_mut(dw_index as usize) {
        entry.1 = true;
        S_OK
    } else {
        E_INVALIDARG
    }
}

unsafe extern "win64" fn pd_deselect_stream(this: usize, dw_index: u32) -> i32 {
    let pd = &*(this as *const MfPresentationDescriptor);
    let mut g = pd.streams.lock().unwrap();
    if let Some(entry) = g.get_mut(dw_index as usize) {
        entry.1 = false;
        S_OK
    } else {
        E_INVALIDARG
    }
}

unsafe extern "win64" fn pd_clone(this: usize, pp_pd: *mut usize) -> i32 {
    if pp_pd.is_null() {
        return E_POINTER;
    }
    let pd = &*(this as *const MfPresentationDescriptor);
    let streams: Vec<(usize, bool)> = {
        let g = pd.streams.lock().unwrap();
        for &(sd, _) in g.iter() {
            com_add_ref(sd);
        }
        g.clone()
    };
    let cloned = Box::into_raw(Box::new(MfPresentationDescriptor {
        header: MfAttrHeader::new(MF_PRESENTATION_DESCRIPTOR_VTBL.as_ptr()),
        streams: Mutex::new(streams),
    })) as usize;
    attr_copy_all_items(this, cloned);
    *pp_pd = cloned;
    S_OK
}

static MF_PRESENTATION_DESCRIPTOR_VTBL: SyncVtbl<38> = make_attr_vtbl!(
    38,
    pd_qi,
    generic_add_ref::<MfPresentationDescriptor>,
    generic_release::<MfPresentationDescriptor>,
    pd_get_stream_count,
    pd_get_stream_by_index,
    pd_select_stream,
    pd_deselect_stream,
    pd_clone,
);

// ============================================================================
// MediaInfo & IMFMediaSource
// ============================================================================

#[derive(Clone, Debug)]
struct MediaInfo {
    host_path: String,
    duration_100ns: u64,
    has_video: bool,
    width: u32,
    height: u32,
    fps_num: u32,
    fps_den: u32,
    has_audio: bool,
    channels: u32,
    sample_rate: u32,
}

fn probe_media_file(host_path: &str) -> Option<MediaInfo> {
    if !std::path::Path::new(host_path).exists() {
        return None;
    }
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            host_path,
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let dur_sec: f64 = json
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let duration_100ns = (dur_sec * 10_000_000.0).max(0.0) as u64;

    let mut info = MediaInfo {
        host_path: host_path.to_string(),
        duration_100ns,
        has_video: false,
        width: 0,
        height: 0,
        fps_num: 30,
        fps_den: 1,
        has_audio: false,
        channels: 2,
        sample_rate: 48000,
    };

    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
        for st in streams {
            let ctype = st.get("codec_type").and_then(|v| v.as_str()).unwrap_or("");
            if ctype == "video" && !info.has_video {
                info.has_video = true;
                info.width = st.get("width").and_then(|v| v.as_u64()).unwrap_or(1280) as u32;
                info.height = st.get("height").and_then(|v| v.as_u64()).unwrap_or(720) as u32;
                let fps_str = st
                    .get("r_frame_rate")
                    .and_then(|v| v.as_str())
                    .unwrap_or("30/1");
                if let Some((n, d)) = fps_str.split_once('/') {
                    let num = n.parse::<u32>().unwrap_or(30).max(1);
                    let den = d.parse::<u32>().unwrap_or(1).max(1);
                    info.fps_num = num;
                    info.fps_den = den;
                }
            } else if ctype == "audio" && !info.has_audio {
                info.has_audio = true;
                info.channels = st.get("channels").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
                info.sample_rate = st
                    .get("sample_rate")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(48000);
            }
        }
    }
    Some(info)
}

#[repr(C)]
struct MfMediaSource {
    vtable: *const usize,
    ref_count: AtomicU32,
    event_queue: usize,
    info: MediaInfo,
}

impl HasRefCount for MfMediaSource {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

impl Drop for MfMediaSource {
    fn drop(&mut self) {
        unsafe { com_release(self.event_queue) };
    }
}

unsafe extern "win64" fn ms_src_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFMediaEventGenerator || id == IID_IMFMediaSource {
        generic_add_ref::<MfMediaSource>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn ms_src_get_event(
    this: usize,
    dw_flags: u32,
    pp_event: *mut usize,
) -> i32 {
    let src = &*(this as *const MfMediaSource);
    eq_get_event(src.event_queue, dw_flags, pp_event)
}

unsafe extern "win64" fn ms_src_begin_get_event(
    this: usize,
    p_callback: usize,
    punk_state: usize,
) -> i32 {
    let src = &*(this as *const MfMediaSource);
    eq_begin_get_event(src.event_queue, p_callback, punk_state)
}

unsafe extern "win64" fn ms_src_end_get_event(
    this: usize,
    p_result: usize,
    pp_event: *mut usize,
) -> i32 {
    let src = &*(this as *const MfMediaSource);
    eq_end_get_event(src.event_queue, p_result, pp_event)
}

unsafe extern "win64" fn ms_src_queue_event(
    this: usize,
    met: u32,
    guid_ext: *const [u8; 16],
    hr_status: i32,
    pv_value: *const PropVariant,
) -> i32 {
    let src = &*(this as *const MfMediaSource);
    eq_queue_event_param_var(src.event_queue, met, guid_ext, hr_status, pv_value)
}

unsafe extern "win64" fn ms_src_get_characteristics(
    _this: usize,
    pdw_chars: *mut u32,
) -> i32 {
    if pdw_chars.is_null() {
        return E_POINTER;
    }
    // MFMEDIASOURCE_CAN_SEEK (0x2) | MFMEDIASOURCE_CAN_PAUSE (0x8)
    *pdw_chars = 0x2 | 0x8;
    S_OK
}

unsafe extern "win64" fn ms_src_create_pd(this: usize, pp_pd: *mut usize) -> i32 {
    if pp_pd.is_null() {
        return E_POINTER;
    }
    let src = &*(this as *const MfMediaSource);
    let mut streams = Vec::new();
    let mut sid = 0u32;

    if src.info.has_video {
        let mt = create_media_type();
        attr_set_guid(mt, &MF_MT_MAJOR_TYPE, &MFMediaType_Video);
        attr_set_guid(mt, &MF_MT_SUBTYPE, &MFVideoFormat_H264);
        attr_set_uint64(
            mt,
            &MF_MT_FRAME_SIZE,
            ((src.info.width as u64) << 32) | (src.info.height as u64),
        );
        attr_set_uint64(
            mt,
            &MF_MT_FRAME_RATE,
            ((src.info.fps_num as u64) << 32) | (src.info.fps_den as u64),
        );
        attr_set_uint64(mt, &MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1);
        attr_set_uint32(mt, &MF_MT_INTERLACE_MODE, 2);
        attr_set_uint32(mt, &MF_MT_DEFAULT_STRIDE, src.info.width * 2);

        let mt_yuy2 = create_media_type();
        attr_copy_all_items(mt, mt_yuy2);
        attr_set_guid(mt_yuy2, &MF_MT_SUBTYPE, &MFVideoFormat_YUY2);
        attr_set_uint32(mt_yuy2, &MF_MT_DEFAULT_STRIDE, src.info.width * 2);

        let handler = create_media_type_handler(MFMediaType_Video, vec![mt, mt_yuy2]);
        let sd = create_stream_descriptor(sid, handler);
        streams.push((sd, true));
        sid += 1;
    }

    if src.info.has_audio {
        let mt = create_media_type();
        attr_set_guid(mt, &MF_MT_MAJOR_TYPE, &MFMediaType_Audio);
        attr_set_guid(mt, &MF_MT_SUBTYPE, &MFAudioFormat_PCM);
        attr_set_uint32(mt, &MF_MT_AUDIO_NUM_CHANNELS, src.info.channels);
        attr_set_uint32(mt, &MF_MT_AUDIO_SAMPLES_PER_SECOND, src.info.sample_rate);
        attr_set_uint32(mt, &MF_MT_AUDIO_BITS_PER_SAMPLE, 16);
        let block_align = src.info.channels * 2;
        attr_set_uint32(mt, &MF_MT_AUDIO_BLOCK_ALIGNMENT, block_align);
        attr_set_uint32(
            mt,
            &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
            src.info.sample_rate * block_align,
        );
        attr_set_uint32(mt, &MF_MT_ALL_SAMPLES_INDEPENDENT, 1);
        attr_set_uint32(mt, &MF_MT_FIXED_SIZE_SAMPLES, 1);

        let handler = create_media_type_handler(MFMediaType_Audio, vec![mt]);
        let sd = create_stream_descriptor(sid, handler);
        streams.push((sd, true));
    }

    let pd = Box::into_raw(Box::new(MfPresentationDescriptor {
        header: MfAttrHeader::new(MF_PRESENTATION_DESCRIPTOR_VTBL.as_ptr()),
        streams: Mutex::new(streams),
    })) as usize;
    attr_set_uint64(pd, &MF_PD_DURATION, src.info.duration_100ns);
    *pp_pd = pd;
    S_OK
}

unsafe extern "win64" fn ms_src_start(
    _this: usize,
    _p_pd: usize,
    _pguid_tf: *const [u8; 16],
    _pvar_pos: *const PropVariant,
) -> i32 {
    S_OK
}

unsafe extern "win64" fn ms_src_stop(_this: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn ms_src_pause(_this: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn ms_src_shutdown(this: usize) -> i32 {
    let src = &*(this as *const MfMediaSource);
    eq_shutdown(src.event_queue)
}

static MF_MEDIA_SOURCE_VTBL: SyncVtbl<13> = SyncVtbl([
    ms_src_qi as *const (),
    generic_add_ref::<MfMediaSource> as *const (),
    generic_release::<MfMediaSource> as *const (),
    ms_src_get_event as *const (),
    ms_src_begin_get_event as *const (),
    ms_src_end_get_event as *const (),
    ms_src_queue_event as *const (),
    ms_src_get_characteristics as *const (),
    ms_src_create_pd as *const (),
    ms_src_start as *const (),
    ms_src_stop as *const (),
    ms_src_pause as *const (),
    ms_src_shutdown as *const (),
]);

fn create_media_source(info: MediaInfo) -> usize {
    Box::into_raw(Box::new(MfMediaSource {
        vtable: MF_MEDIA_SOURCE_VTBL.as_ptr(),
        ref_count: AtomicU32::new(1),
        event_queue: create_event_queue(),
        info,
    })) as usize
}

// ============================================================================
// IMFSourceResolver
// ============================================================================

#[repr(C)]
struct MfSourceResolver {
    vtable: *const usize,
    ref_count: AtomicU32,
}

impl HasRefCount for MfSourceResolver {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

unsafe extern "win64" fn sr_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFSourceResolver {
        generic_add_ref::<MfSourceResolver>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

fn url_to_host_path(raw_url: &str) -> String {
    let mut win_path = raw_url.trim().to_string();
    if let Some(stripped) = win_path.strip_prefix("file:///") {
        win_path = stripped.to_string();
    } else if let Some(stripped) = win_path.strip_prefix("file://") {
        win_path = stripped.to_string();
    }
    crate::dlls::kernel32::win32_to_host_path(&win_path)
}

unsafe extern "win64" fn sr_create_object_from_url(
    _this: usize,
    pwsz_url: *const u16,
    _dw_flags: u32,
    _p_props: usize,
    p_object_type: *mut u32,
    pp_object: *mut usize,
) -> i32 {
    if pwsz_url.is_null() || pp_object.is_null() {
        return E_POINTER;
    }
    let url = read_utf16_z(pwsz_url);
    let host_path = url_to_host_path(&url);
    println!(
        "[MF] CreateObjectFromURL url='{}' -> host='{}'",
        url, host_path
    );
    let Some(info) = probe_media_file(&host_path) else {
        if !p_object_type.is_null() {
            *p_object_type = 0;
        }
        *pp_object = 0;
        return HRESULT_FILE_NOT_FOUND;
    };
    let src = create_media_source(info);
    if !p_object_type.is_null() {
        *p_object_type = MF_OBJECT_MEDIASOURCE;
    }
    *pp_object = src;
    S_OK
}

unsafe extern "win64" fn sr_create_object_from_bytestream(
    _this: usize,
    p_bs: usize,
    pwsz_url: *const u16,
    _dw_flags: u32,
    _p_props: usize,
    p_object_type: *mut u32,
    pp_object: *mut usize,
) -> i32 {
    if pp_object.is_null() {
        return E_POINTER;
    }
    *pp_object = 0;
    if !p_object_type.is_null() {
        *p_object_type = 0;
    }

    let url = if !pwsz_url.is_null() {
        read_utf16_z(pwsz_url)
    } else {
        String::new()
    };
    println!(
        "[MF] CreateObjectFromByteStream(p_bs=0x{:X}, url='{}')",
        p_bs, url
    );

    if !url.is_empty() {
        let host_path = url_to_host_path(&url);
        if std::path::Path::new(&host_path).is_file() {
            if let Some(info) = probe_media_file(&host_path) {
                let src = create_media_source(info);
                if !p_object_type.is_null() {
                    *p_object_type = MF_OBJECT_MEDIASOURCE;
                }
                *pp_object = src;
                return S_OK;
            }
        }
    }

    if p_bs == 0 {
        return if !url.is_empty() {
            HRESULT_FILE_NOT_FOUND
        } else {
            E_POINTER
        };
    }

    let vtbl = *(p_bs as *const *const usize);
    if vtbl.is_null() {
        return E_FAIL;
    }
    // IMFByteStream vtable slots: 4=GetLength, 7=SetCurrentPosition, 9=Read
    let get_length: unsafe extern "win64" fn(usize, *mut u64) -> i32 =
        std::mem::transmute(*vtbl.add(4));
    let set_pos: unsafe extern "win64" fn(usize, u64) -> i32 =
        std::mem::transmute(*vtbl.add(7));
    let read_fn: unsafe extern "win64" fn(usize, *mut u8, u32, *mut u32) -> i32 =
        std::mem::transmute(*vtbl.add(9));

    let _ = set_pos(p_bs, 0);
    let mut total_len: u64 = 0;
    let has_len = get_length(p_bs, &mut total_len) >= 0 && total_len > 0;

    static BS_TMP_COUNTER: AtomicU64 = AtomicU64::new(1);
    let tmp_id = BS_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = format!("/tmp/rine_mf_bs_{}.mp4", tmp_id);

    let Ok(mut file) = std::fs::File::create(&tmp_path) else {
        return E_FAIL;
    };

    const CHUNK_SIZE: usize = 1024 * 1024;
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut total_read: u64 = 0;

    loop {
        let to_read = if has_len {
            let rem = total_len.saturating_sub(total_read);
            if rem == 0 {
                break;
            }
            rem.min(CHUNK_SIZE as u64) as u32
        } else {
            CHUNK_SIZE as u32
        };

        let mut cb_read: u32 = 0;
        let hr = read_fn(p_bs, buf.as_mut_ptr(), to_read, &mut cb_read);
        if hr < 0 || cb_read == 0 {
            break;
        }
        if file.write_all(&buf[..cb_read as usize]).is_err() {
            drop(file);
            let _ = std::fs::remove_file(&tmp_path);
            return E_FAIL;
        }
        total_read += cb_read as u64;
        if has_len && total_read >= total_len {
            break;
        }
    }
    drop(file);

    if total_read == 0 {
        let _ = std::fs::remove_file(&tmp_path);
        return E_FAIL;
    }

    let Some(info) = probe_media_file(&tmp_path) else {
        let _ = std::fs::remove_file(&tmp_path);
        return E_FAIL;
    };

    let src = create_media_source(info);
    if !p_object_type.is_null() {
        *p_object_type = MF_OBJECT_MEDIASOURCE;
    }
    *pp_object = src;
    S_OK
}

unsafe extern "win64" fn sr_begin_create_from_url(
    this: usize,
    pwsz_url: *const u16,
    dw_flags: u32,
    p_props: usize,
    pp_cancel_cookie: *mut usize,
    p_callback: usize,
    punk_state: usize,
) -> i32 {
    if !pp_cancel_cookie.is_null() {
        *pp_cancel_cookie = 0;
    }
    if p_callback == 0 {
        return E_POINTER;
    }
    let mut obj_type = 0u32;
    let mut obj = 0usize;
    let hr = sr_create_object_from_url(this, pwsz_url, dw_flags, p_props, &mut obj_type, &mut obj);
    let ar = create_async_result(obj, p_callback, punk_state);
    ar_set_status(ar, hr);
    com_release(obj);
    std::thread::spawn(move || {
        crate::dlls::kernel32::ensure_current_thread_teb();
        invoke_async_result(ar);
        com_release(ar);
    });
    S_OK
}

unsafe extern "win64" fn sr_end_create_from_url(
    _this: usize,
    p_result: usize,
    p_object_type: *mut u32,
    pp_object: *mut usize,
) -> i32 {
    if p_result == 0 || pp_object.is_null() {
        return E_POINTER;
    }
    let hr = ar_get_status(p_result);
    if hr < 0 {
        if !p_object_type.is_null() {
            *p_object_type = 0;
        }
        *pp_object = 0;
        return hr;
    }
    if !p_object_type.is_null() {
        *p_object_type = MF_OBJECT_MEDIASOURCE;
    }
    ar_get_object(p_result, pp_object)
}

unsafe extern "win64" fn sr_begin_create_from_bs(
    this: usize,
    p_bs: usize,
    pwsz_url: *const u16,
    dw_flags: u32,
    p_props: usize,
    pp_cancel_cookie: *mut usize,
    p_callback: usize,
    punk_state: usize,
) -> i32 {
    if !pp_cancel_cookie.is_null() {
        *pp_cancel_cookie = 0;
    }
    if p_callback == 0 {
        return E_POINTER;
    }
    let mut obj_type = 0u32;
    let mut obj = 0usize;
    let hr = sr_create_object_from_bytestream(
        this,
        p_bs,
        pwsz_url,
        dw_flags,
        p_props,
        &mut obj_type,
        &mut obj,
    );
    let ar = create_async_result(obj, p_callback, punk_state);
    ar_set_status(ar, hr);
    com_release(obj);
    std::thread::spawn(move || {
        crate::dlls::kernel32::ensure_current_thread_teb();
        invoke_async_result(ar);
        com_release(ar);
    });
    S_OK
}

unsafe extern "win64" fn sr_end_create_from_bs(
    this: usize,
    p_result: usize,
    p_object_type: *mut u32,
    pp_object: *mut usize,
) -> i32 {
    sr_end_create_from_url(this, p_result, p_object_type, pp_object)
}

unsafe extern "win64" fn sr_cancel_object_creation(_this: usize, _p_cookie: usize) -> i32 {
    S_OK
}

static MF_SOURCE_RESOLVER_VTBL: SyncVtbl<10> = SyncVtbl([
    sr_qi as *const (),
    generic_add_ref::<MfSourceResolver> as *const (),
    generic_release::<MfSourceResolver> as *const (),
    sr_create_object_from_url as *const (),
    sr_create_object_from_bytestream as *const (),
    sr_begin_create_from_url as *const (),
    sr_end_create_from_url as *const (),
    sr_begin_create_from_bs as *const (),
    sr_end_create_from_bs as *const (),
    sr_cancel_object_creation as *const (),
]);

// ============================================================================
// IMFTopology & IMFTopologyNode
// ============================================================================

#[repr(C)]
struct MfTopologyNode {
    header: MfAttrHeader,
    node_type: u32,
    topo_id: AtomicU64,
    object: Mutex<usize>,
    outputs: Mutex<Vec<(usize, u32)>>,
}

impl HasRefCount for MfTopologyNode {
    fn ref_count(&self) -> &AtomicU32 {
        &self.header.ref_count
    }
}

impl Drop for MfTopologyNode {
    fn drop(&mut self) {
        let obj = *self.object.lock().unwrap();
        unsafe { com_release(obj) };
        for (dn, _) in self.outputs.lock().unwrap().drain(..) {
            unsafe { com_release(dn) };
        }
    }
}

unsafe extern "win64" fn tn_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAttributes || id == IID_IMFTopologyNode {
        generic_add_ref::<MfTopologyNode>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn tn_set_object(this: usize, p_object: usize) -> i32 {
    let tn = &*(this as *const MfTopologyNode);
    com_add_ref(p_object);
    let mut g = tn.object.lock().unwrap();
    let old = *g;
    *g = p_object;
    drop(g);
    com_release(old);
    S_OK
}

unsafe extern "win64" fn tn_get_object(this: usize, pp_object: *mut usize) -> i32 {
    if pp_object.is_null() {
        return E_POINTER;
    }
    let tn = &*(this as *const MfTopologyNode);
    let obj = *tn.object.lock().unwrap();
    if obj == 0 {
        *pp_object = 0;
        return E_NOINTERFACE;
    }
    com_add_ref(obj);
    *pp_object = obj;
    S_OK
}

unsafe extern "win64" fn tn_get_node_type(this: usize, p_type: *mut u32) -> i32 {
    if p_type.is_null() {
        return E_POINTER;
    }
    let tn = &*(this as *const MfTopologyNode);
    *p_type = tn.node_type;
    S_OK
}

unsafe extern "win64" fn tn_get_id(this: usize, p_id: *mut u64) -> i32 {
    if p_id.is_null() {
        return E_POINTER;
    }
    let tn = &*(this as *const MfTopologyNode);
    *p_id = tn.topo_id.load(Ordering::SeqCst);
    S_OK
}

unsafe extern "win64" fn tn_set_id(this: usize, id: u64) -> i32 {
    let tn = &*(this as *const MfTopologyNode);
    tn.topo_id.store(id, Ordering::SeqCst);
    S_OK
}

unsafe extern "win64" fn tn_get_input_count(_this: usize, pc: *mut u32) -> i32 {
    if !pc.is_null() {
        *pc = 0;
    }
    S_OK
}

unsafe extern "win64" fn tn_get_output_count(this: usize, pc: *mut u32) -> i32 {
    if pc.is_null() {
        return E_POINTER;
    }
    let tn = &*(this as *const MfTopologyNode);
    *pc = tn.outputs.lock().unwrap().len() as u32;
    S_OK
}

unsafe extern "win64" fn tn_connect_output(
    this: usize,
    dw_output_idx: u32,
    p_downstream: usize,
    dw_input_idx: u32,
) -> i32 {
    if p_downstream == 0 {
        return E_POINTER;
    }
    let tn = &*(this as *const MfTopologyNode);
    let mut g = tn.outputs.lock().unwrap();
    let idx = dw_output_idx as usize;
    if idx >= g.len() {
        g.resize(idx + 1, (0, 0));
    }
    com_add_ref(p_downstream);
    let old = g[idx].0;
    g[idx] = (p_downstream, dw_input_idx);
    drop(g);
    com_release(old);
    S_OK
}

unsafe extern "win64" fn tn_disconnect_output(this: usize, dw_output_idx: u32) -> i32 {
    let tn = &*(this as *const MfTopologyNode);
    let mut g = tn.outputs.lock().unwrap();
    if let Some(slot) = g.get_mut(dw_output_idx as usize) {
        let old = slot.0;
        *slot = (0, 0);
        drop(g);
        com_release(old);
    }
    S_OK
}

unsafe extern "win64" fn tn_get_input(
    _this: usize,
    _dw_idx: u32,
    pp_up: *mut usize,
    _pdw_out: *mut u32,
) -> i32 {
    if !pp_up.is_null() {
        *pp_up = 0;
    }
    E_NOTIMPL
}

unsafe extern "win64" fn tn_get_output(
    this: usize,
    dw_idx: u32,
    pp_down: *mut usize,
    pdw_in: *mut u32,
) -> i32 {
    if pp_down.is_null() {
        return E_POINTER;
    }
    let tn = &*(this as *const MfTopologyNode);
    let g = tn.outputs.lock().unwrap();
    if let Some(&(dn, in_idx)) = g.get(dw_idx as usize) {
        if dn != 0 {
            com_add_ref(dn);
            *pp_down = dn;
            if !pdw_in.is_null() {
                *pdw_in = in_idx;
            }
            return S_OK;
        }
    }
    *pp_down = 0;
    E_INVALIDARG
}

unsafe extern "win64" fn tn_stub_pref(_this: usize, _idx: u32, _p: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn tn_clone_from(_this: usize, _p_node: usize) -> i32 {
    S_OK
}

static MF_TOPOLOGY_NODE_VTBL: SyncVtbl<49> = make_attr_vtbl!(
    49,
    tn_qi,
    generic_add_ref::<MfTopologyNode>,
    generic_release::<MfTopologyNode>,
    tn_set_object,
    tn_get_object,
    tn_get_node_type,
    tn_get_id,
    tn_set_id,
    tn_get_input_count,
    tn_get_output_count,
    tn_connect_output,
    tn_disconnect_output,
    tn_get_input,
    tn_get_output,
    tn_stub_pref,
    tn_stub_pref,
    tn_stub_pref,
    tn_stub_pref,
    tn_clone_from,
);

#[repr(C)]
struct MfTopology {
    header: MfAttrHeader,
    topo_id: AtomicU64,
    nodes: Mutex<Vec<usize>>,
}

impl HasRefCount for MfTopology {
    fn ref_count(&self) -> &AtomicU32 {
        &self.header.ref_count
    }
}

impl Drop for MfTopology {
    fn drop(&mut self) {
        for n in self.nodes.lock().unwrap().drain(..) {
            unsafe { com_release(n) };
        }
    }
}

unsafe extern "win64" fn topo_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAttributes || id == IID_IMFTopology {
        generic_add_ref::<MfTopology>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn topo_get_id(this: usize, p_id: *mut u64) -> i32 {
    if p_id.is_null() {
        return E_POINTER;
    }
    let t = &*(this as *const MfTopology);
    *p_id = t.topo_id.load(Ordering::SeqCst);
    S_OK
}

unsafe extern "win64" fn topo_add_node(this: usize, p_node: usize) -> i32 {
    if p_node == 0 {
        return E_POINTER;
    }
    let t = &*(this as *const MfTopology);
    com_add_ref(p_node);
    t.nodes.lock().unwrap().push(p_node);
    S_OK
}

unsafe extern "win64" fn topo_remove_node(this: usize, p_node: usize) -> i32 {
    let t = &*(this as *const MfTopology);
    let mut g = t.nodes.lock().unwrap();
    if let Some(pos) = g.iter().position(|&n| n == p_node) {
        let removed = g.remove(pos);
        drop(g);
        com_release(removed);
        S_OK
    } else {
        E_INVALIDARG
    }
}

unsafe extern "win64" fn topo_get_node_count(this: usize, pw_nodes: *mut u16) -> i32 {
    if pw_nodes.is_null() {
        return E_POINTER;
    }
    let t = &*(this as *const MfTopology);
    *pw_nodes = t.nodes.lock().unwrap().len() as u16;
    S_OK
}

unsafe extern "win64" fn topo_get_node(
    this: usize,
    w_index: u16,
    pp_node: *mut usize,
) -> i32 {
    if pp_node.is_null() {
        return E_POINTER;
    }
    let t = &*(this as *const MfTopology);
    let g = t.nodes.lock().unwrap();
    if let Some(&n) = g.get(w_index as usize) {
        com_add_ref(n);
        *pp_node = n;
        S_OK
    } else {
        *pp_node = 0;
        E_INVALIDARG
    }
}

unsafe extern "win64" fn topo_clear(this: usize) -> i32 {
    let t = &*(this as *const MfTopology);
    let nodes: Vec<usize> = t.nodes.lock().unwrap().drain(..).collect();
    for n in nodes {
        com_release(n);
    }
    S_OK
}

unsafe extern "win64" fn topo_clone_from(_this: usize, _p_topo: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn topo_get_node_by_id(
    this: usize,
    id: u64,
    pp_node: *mut usize,
) -> i32 {
    if pp_node.is_null() {
        return E_POINTER;
    }
    let t = &*(this as *const MfTopology);
    let g = t.nodes.lock().unwrap();
    for &n in g.iter() {
        let tn = &*(n as *const MfTopologyNode);
        if tn.topo_id.load(Ordering::SeqCst) == id {
            com_add_ref(n);
            *pp_node = n;
            return S_OK;
        }
    }
    *pp_node = 0;
    E_INVALIDARG
}

unsafe extern "win64" fn topo_get_collection(_this: usize, pp_col: *mut usize) -> i32 {
    if !pp_col.is_null() {
        *pp_col = 0;
    }
    E_NOTIMPL
}

static MF_TOPOLOGY_VTBL: SyncVtbl<43> = make_attr_vtbl!(
    43,
    topo_qi,
    generic_add_ref::<MfTopology>,
    generic_release::<MfTopology>,
    topo_get_id,
    topo_add_node,
    topo_remove_node,
    topo_get_node_count,
    topo_get_node,
    topo_clear,
    topo_clone_from,
    topo_get_node_by_id,
    topo_get_collection,
    topo_get_collection,
);

// ============================================================================
// IMFActivate (SampleGrabber & AudioRenderer)
// ============================================================================

#[repr(C)]
struct MfActivate {
    header: MfAttrHeader,
    is_audio_renderer: bool,
    major_type: [u8; 16],
    subtype: [u8; 16],
    callback: usize,
}

impl HasRefCount for MfActivate {
    fn ref_count(&self) -> &AtomicU32 {
        &self.header.ref_count
    }
}

impl Drop for MfActivate {
    fn drop(&mut self) {
        unsafe { com_release(self.callback) };
    }
}

unsafe extern "win64" fn act_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFAttributes || id == IID_IMFActivate {
        generic_add_ref::<MfActivate>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn act_activate_object(
    this: usize,
    _riid: *const [u8; 16],
    ppv: *mut usize,
) -> i32 {
    if ppv.is_null() {
        return E_POINTER;
    }
    generic_add_ref::<MfActivate>(this);
    *ppv = this;
    S_OK
}

unsafe extern "win64" fn act_shutdown_object(_this: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn act_detach_object(_this: usize) -> i32 {
    S_OK
}

static MF_ACTIVATE_VTBL: SyncVtbl<36> = make_attr_vtbl!(
    36,
    act_qi,
    generic_add_ref::<MfActivate>,
    generic_release::<MfActivate>,
    act_activate_object,
    act_shutdown_object,
    act_detach_object,
);

// ============================================================================
// IMFPresentationClock, IMFRateControl, IMFRateSupport
// ============================================================================

struct ClockInner {
    state: u32,
    start_offset_100ns: i64,
    accumulated_100ns: i64,
    started_at: Option<Instant>,
}

impl ClockInner {
    fn current_time_100ns(&self) -> i64 {
        match self.state {
            MFCLOCK_STATE_RUNNING => {
                let elapsed = self
                    .started_at
                    .map(|t| (t.elapsed().as_nanos() / 100) as i64)
                    .unwrap_or(0);
                self.start_offset_100ns + self.accumulated_100ns + elapsed
            }
            MFCLOCK_STATE_PAUSED => self.start_offset_100ns + self.accumulated_100ns,
            _ => 0,
        }
    }
}

#[repr(C)]
struct MfPresentationClock {
    vtable: *const usize,
    ref_count: AtomicU32,
    inner: Mutex<ClockInner>,
}

impl HasRefCount for MfPresentationClock {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

unsafe extern "win64" fn clk_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFClock || id == IID_IMFPresentationClock {
        generic_add_ref::<MfPresentationClock>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn clk_get_characteristics(_this: usize, pdw_chars: *mut u32) -> i32 {
    if !pdw_chars.is_null() {
        *pdw_chars = 0x1; // MFCLOCK_CHARACTERISTICS_FLAG_FREQUENCY_10MHZ
    }
    S_OK
}

unsafe extern "win64" fn clk_get_correlated_time(
    this: usize,
    _dw_reserved: u32,
    pll_clock_time: *mut i64,
    phns_system_time: *mut i64,
) -> i32 {
    let clk = &*(this as *const MfPresentationClock);
    let t = clk.inner.lock().unwrap().current_time_100ns();
    static CNT: AtomicU32 = AtomicU32::new(0);
    let c = CNT.fetch_add(1, Ordering::Relaxed);
    if c < 5 {
        println!("[MF] clk_get_correlated_time #{} -> {}", c, t);
    }
    if !pll_clock_time.is_null() {
        *pll_clock_time = t;
    }
    if !phns_system_time.is_null() {
        *phns_system_time = t;
    }
    S_OK
}

unsafe extern "win64" fn clk_get_continuity_key(_this: usize, pdw_key: *mut u32) -> i32 {
    if !pdw_key.is_null() {
        *pdw_key = 0;
    }
    S_OK
}

unsafe extern "win64" fn clk_get_state(this: usize, _dw_reserved: u32, pe_state: *mut u32) -> i32 {
    if pe_state.is_null() {
        return E_POINTER;
    }
    let clk = &*(this as *const MfPresentationClock);
    *pe_state = clk.inner.lock().unwrap().state;
    S_OK
}

unsafe extern "win64" fn clk_get_properties(_this: usize, p_props: *mut u8) -> i32 {
    if !p_props.is_null() {
        std::ptr::write_bytes(p_props, 0, 48);
    }
    S_OK
}

unsafe extern "win64" fn clk_set_time_source(_this: usize, _p_ts: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn clk_get_time_source(_this: usize, pp_ts: *mut usize) -> i32 {
    if !pp_ts.is_null() {
        *pp_ts = 0;
    }
    E_NOTIMPL
}

unsafe extern "win64" fn clk_get_time(this: usize, phns_time: *mut i64) -> i32 {
    if phns_time.is_null() {
        return E_POINTER;
    }
    let clk = &*(this as *const MfPresentationClock);
    let t = clk.inner.lock().unwrap().current_time_100ns();
    static CNT: AtomicU32 = AtomicU32::new(0);
    let c = CNT.fetch_add(1, Ordering::Relaxed);
    if c < 5 {
        println!("[MF] clk_get_time #{} -> {}", c, t);
    }
    *phns_time = t;
    S_OK
}

unsafe extern "win64" fn clk_add_sink(_this: usize, _p_sink: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn clk_remove_sink(_this: usize, _p_sink: usize) -> i32 {
    S_OK
}

unsafe extern "win64" fn clk_start(this: usize, ll_offset: i64) -> i32 {
    let clk = &*(this as *const MfPresentationClock);
    let mut g = clk.inner.lock().unwrap();
    g.start_offset_100ns = ll_offset;
    g.accumulated_100ns = 0;
    g.started_at = Some(Instant::now());
    g.state = MFCLOCK_STATE_RUNNING;
    S_OK
}

unsafe extern "win64" fn clk_stop(this: usize) -> i32 {
    let clk = &*(this as *const MfPresentationClock);
    let mut g = clk.inner.lock().unwrap();
    g.state = MFCLOCK_STATE_STOPPED;
    g.accumulated_100ns = 0;
    g.started_at = None;
    S_OK
}

unsafe extern "win64" fn clk_pause(this: usize) -> i32 {
    let clk = &*(this as *const MfPresentationClock);
    let mut g = clk.inner.lock().unwrap();
    if g.state == MFCLOCK_STATE_RUNNING {
        if let Some(t) = g.started_at.take() {
            g.accumulated_100ns += (t.elapsed().as_nanos() / 100) as i64;
        }
        g.state = MFCLOCK_STATE_PAUSED;
    }
    S_OK
}

static MF_CLOCK_VTBL: SyncVtbl<16> = SyncVtbl([
    clk_qi as *const (),
    generic_add_ref::<MfPresentationClock> as *const (),
    generic_release::<MfPresentationClock> as *const (),
    clk_get_characteristics as *const (),
    clk_get_correlated_time as *const (),
    clk_get_continuity_key as *const (),
    clk_get_state as *const (),
    clk_get_properties as *const (),
    clk_set_time_source as *const (),
    clk_get_time_source as *const (),
    clk_get_time as *const (),
    clk_add_sink as *const (),
    clk_remove_sink as *const (),
    clk_start as *const (),
    clk_stop as *const (),
    clk_pause as *const (),
]);

fn create_presentation_clock() -> usize {
    Box::into_raw(Box::new(MfPresentationClock {
        vtable: MF_CLOCK_VTBL.as_ptr(),
        ref_count: AtomicU32::new(1),
        inner: Mutex::new(ClockInner {
            state: MFCLOCK_STATE_STOPPED,
            start_offset_100ns: 0,
            accumulated_100ns: 0,
            started_at: None,
        }),
    })) as usize
}

// IMFRateControl & IMFRateSupport
#[repr(C)]
struct MfRateControl {
    vtable: *const usize,
    ref_count: AtomicU32,
    rate: Mutex<f32>,
    thin: AtomicBool,
}

impl HasRefCount for MfRateControl {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

unsafe extern "win64" fn rc_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFRateControl {
        generic_add_ref::<MfRateControl>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn rc_set_rate(this: usize, f_thin: i32, fl_rate: f32) -> i32 {
    let rc = &*(this as *const MfRateControl);
    rc.thin.store(f_thin != 0, Ordering::SeqCst);
    *rc.rate.lock().unwrap() = fl_rate;
    S_OK
}

unsafe extern "win64" fn rc_get_rate(this: usize, pf_thin: *mut i32, pfl_rate: *mut f32) -> i32 {
    let rc = &*(this as *const MfRateControl);
    if !pf_thin.is_null() {
        *pf_thin = if rc.thin.load(Ordering::SeqCst) { 1 } else { 0 };
    }
    if !pfl_rate.is_null() {
        *pfl_rate = *rc.rate.lock().unwrap();
    }
    S_OK
}

static MF_RATE_CONTROL_VTBL: SyncVtbl<5> = SyncVtbl([
    rc_qi as *const (),
    generic_add_ref::<MfRateControl> as *const (),
    generic_release::<MfRateControl> as *const (),
    rc_set_rate as *const (),
    rc_get_rate as *const (),
]);

#[repr(C)]
struct MfRateSupport {
    vtable: *const usize,
    ref_count: AtomicU32,
}

impl HasRefCount for MfRateSupport {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

unsafe extern "win64" fn rs_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFRateSupport {
        generic_add_ref::<MfRateSupport>(this);
        *ppv = this;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn rs_get_slowest(
    _this: usize,
    _dir: u32,
    _thin: i32,
    pfl_rate: *mut f32,
) -> i32 {
    if !pfl_rate.is_null() {
        *pfl_rate = 0.0;
    }
    S_OK
}

unsafe extern "win64" fn rs_get_fastest(
    _this: usize,
    dir: u32,
    _thin: i32,
    pfl_rate: *mut f32,
) -> i32 {
    if !pfl_rate.is_null() {
        *pfl_rate = if dir == 0 { 1.0 } else { -1.0 };
    }
    S_OK
}

unsafe extern "win64" fn rs_is_supported(
    _this: usize,
    _thin: i32,
    fl_rate: f32,
    pfl_nearest: *mut f32,
) -> i32 {
    if !pfl_nearest.is_null() {
        *pfl_nearest = fl_rate;
    }
    S_OK
}

static MF_RATE_SUPPORT_VTBL: SyncVtbl<6> = SyncVtbl([
    rs_qi as *const (),
    generic_add_ref::<MfRateSupport> as *const (),
    generic_release::<MfRateSupport> as *const (),
    rs_get_slowest as *const (),
    rs_get_fastest as *const (),
    rs_is_supported as *const (),
]);

// ============================================================================
// IMFMediaSession + Playback Engine
// ============================================================================

struct PlaybackControl {
    stop: AtomicBool,
    pause: AtomicBool,
    video_child: Mutex<Option<Child>>,
    audio_child: Mutex<Option<Child>>,
    threads: Mutex<Vec<std::thread::JoinHandle<()>>>,
    callbacks: Mutex<Vec<usize>>,
}

impl PlaybackControl {
    fn new() -> Self {
        Self {
            stop: AtomicBool::new(false),
            pause: AtomicBool::new(false),
            video_child: Mutex::new(None),
            audio_child: Mutex::new(None),
            threads: Mutex::new(Vec::new()),
            callbacks: Mutex::new(Vec::new()),
        }
    }

    fn stop_all(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.pause.store(false, Ordering::SeqCst);
        if let Some(mut c) = self.video_child.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(mut c) = self.audio_child.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        let handles: Vec<_> = self.threads.lock().unwrap().drain(..).collect();
        for h in handles {
            let _ = h.join();
        }
        for cb in self.callbacks.lock().unwrap().drain(..) {
            unsafe { com_release(cb) };
        }
    }
}

#[repr(C)]
struct MfSessionGetService {
    vtable: *const usize,
    session: usize,
}

#[repr(C)]
struct MfMediaSession {
    vtable: *const usize,
    ref_count: AtomicU32,
    get_service: MfSessionGetService,
    event_queue: usize,
    clock: usize,
    topology: Mutex<usize>,
    playback: Mutex<Option<Arc<PlaybackControl>>>,
}

impl HasRefCount for MfMediaSession {
    fn ref_count(&self) -> &AtomicU32 {
        &self.ref_count
    }
}

impl Drop for MfMediaSession {
    fn drop(&mut self) {
        if let Some(pb) = self.playback.lock().unwrap().take() {
            pb.stop_all();
        }
        let topo = *self.topology.lock().unwrap();
        unsafe {
            com_release(topo);
            com_release(self.clock);
            com_release(self.event_queue);
        }
    }
}

unsafe extern "win64" fn sess_gs_qi(
    this: usize,
    riid: *const [u8; 16],
    ppv: *mut usize,
) -> i32 {
    let gs = &*(this as *const MfSessionGetService);
    sess_qi(gs.session, riid, ppv)
}

unsafe extern "win64" fn sess_gs_add_ref(this: usize) -> u32 {
    let gs = &*(this as *const MfSessionGetService);
    generic_add_ref::<MfMediaSession>(gs.session)
}

unsafe extern "win64" fn sess_gs_release(this: usize) -> u32 {
    let gs = &*(this as *const MfSessionGetService);
    generic_release::<MfMediaSession>(gs.session)
}

unsafe extern "win64" fn sess_gs_get_service(
    _this: usize,
    guid_service: *const [u8; 16],
    riid: *const [u8; 16],
    ppv_object: *mut usize,
) -> i32 {
    if guid_service.is_null() || riid.is_null() || ppv_object.is_null() {
        return E_POINTER;
    }
    let svc = *guid_service;
    let id = *riid;
    if svc == MF_RATE_CONTROL_SERVICE {
        if id == IID_IMFRateControl || id == IID_IUnknown {
            *ppv_object = Box::into_raw(Box::new(MfRateControl {
                vtable: MF_RATE_CONTROL_VTBL.as_ptr(),
                ref_count: AtomicU32::new(1),
                rate: Mutex::new(1.0),
                thin: AtomicBool::new(false),
            })) as usize;
            return S_OK;
        }
        if id == IID_IMFRateSupport {
            *ppv_object = Box::into_raw(Box::new(MfRateSupport {
                vtable: MF_RATE_SUPPORT_VTBL.as_ptr(),
                ref_count: AtomicU32::new(1),
            })) as usize;
            return S_OK;
        }
        *ppv_object = 0;
        return E_NOINTERFACE;
    }
    if svc == MR_VIDEO_ACCELERATION_SERVICE {
        *ppv_object = 0;
        return E_NOINTERFACE;
    }
    *ppv_object = 0;
    MF_E_UNSUPPORTED_SERVICE
}

static MF_SESSION_GET_SERVICE_VTBL: SyncVtbl<4> = SyncVtbl([
    sess_gs_qi as *const (),
    sess_gs_add_ref as *const (),
    sess_gs_release as *const (),
    sess_gs_get_service as *const (),
]);

unsafe extern "win64" fn sess_qi(this: usize, riid: *const [u8; 16], ppv: *mut usize) -> i32 {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let id = *riid;
    if id == IID_IUnknown || id == IID_IMFMediaEventGenerator || id == IID_IMFMediaSession {
        generic_add_ref::<MfMediaSession>(this);
        *ppv = this;
        return S_OK;
    }
    if id == IID_IMFGetService {
        generic_add_ref::<MfMediaSession>(this);
        let sess = &mut *(this as *mut MfMediaSession);
        *ppv = &mut sess.get_service as *mut MfSessionGetService as usize;
        return S_OK;
    }
    *ppv = 0;
    E_NOINTERFACE
}

unsafe extern "win64" fn sess_get_event(
    this: usize,
    dw_flags: u32,
    pp_event: *mut usize,
) -> i32 {
    let sess = &*(this as *const MfMediaSession);
    eq_get_event(sess.event_queue, dw_flags, pp_event)
}

unsafe extern "win64" fn sess_begin_get_event(
    this: usize,
    p_callback: usize,
    punk_state: usize,
) -> i32 {
    println!(
        "[MF] sess_begin_get_event(sess=0x{:X}, cb=0x{:X}, state=0x{:X})",
        this, p_callback, punk_state
    );
    let sess = &*(this as *const MfMediaSession);
    eq_begin_get_event(sess.event_queue, p_callback, punk_state)
}

unsafe extern "win64" fn sess_end_get_event(
    this: usize,
    p_result: usize,
    pp_event: *mut usize,
) -> i32 {
    let sess = &*(this as *const MfMediaSession);
    let hr = eq_end_get_event(sess.event_queue, p_result, pp_event);
    let ev = if !pp_event.is_null() { *pp_event } else { 0 };
    println!(
        "[MF] sess_end_get_event(sess=0x{:X}, ar=0x{:X}) -> hr=0x{:08X}, ev=0x{:X}",
        this, p_result, hr as u32, ev
    );
    hr
}

unsafe extern "win64" fn sess_queue_event(
    this: usize,
    met: u32,
    guid_ext: *const [u8; 16],
    hr_status: i32,
    pv_value: *const PropVariant,
) -> i32 {
    let sess = &*(this as *const MfMediaSession);
    eq_queue_event_param_var(sess.event_queue, met, guid_ext, hr_status, pv_value)
}

unsafe fn queue_topology_status(event_queue: usize, topo: usize, status: u32) {
    let pv_topo = PropVariant {
        vt: if topo != 0 { VT_UNKNOWN } else { VT_EMPTY },
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
        val_lo: topo as u64,
        val_hi: 0,
    };
    let ev = create_media_event(MESessionTopologyStatus, GUID_NULL, S_OK, pv_topo);
    attr_set_uint32(ev, &MF_EVENT_TOPOLOGY_STATUS, status);
    eq_queue_event(event_queue, ev);
    com_release(ev);
}

unsafe extern "win64" fn sess_set_topology(
    this: usize,
    _dw_flags: u32,
    p_topology: usize,
) -> i32 {
    println!("[MF] sess_set_topology(sess=0x{:X}, topo=0x{:X})", this, p_topology);
    let sess = &*(this as *const MfMediaSession);
    com_add_ref(p_topology);
    let mut g = sess.topology.lock().unwrap();
    let old = *g;
    *g = p_topology;
    drop(g);
    com_release(old);

    eq_queue_event_param_unk(sess.event_queue, MESessionTopologySet, &GUID_NULL, S_OK, p_topology);
    queue_topology_status(sess.event_queue, p_topology, MF_TOPOSTATUS_READY);
    S_OK
}

unsafe extern "win64" fn sess_clear_topologies(this: usize) -> i32 {
    let sess = &*(this as *const MfMediaSession);
    let mut g = sess.topology.lock().unwrap();
    let old = *g;
    *g = 0;
    drop(g);
    com_release(old);
    S_OK
}

unsafe fn call_grabber_clock_start(cb: usize, sys_time: i64, offset: i64) {
    if cb != 0 {
        let vtbl = *(cb as *const *const usize);
        let f: unsafe extern "win64" fn(usize, i64, i64) -> i32 = std::mem::transmute(*vtbl.add(3));
        f(cb, sys_time, offset);
    }
}

unsafe fn call_grabber_clock_stop(cb: usize, sys_time: i64) {
    if cb != 0 {
        let vtbl = *(cb as *const *const usize);
        let f: unsafe extern "win64" fn(usize, i64) -> i32 = std::mem::transmute(*vtbl.add(4));
        f(cb, sys_time);
    }
}

unsafe fn call_grabber_clock_pause(cb: usize, sys_time: i64) {
    if cb != 0 {
        let vtbl = *(cb as *const *const usize);
        let f: unsafe extern "win64" fn(usize, i64) -> i32 = std::mem::transmute(*vtbl.add(5));
        f(cb, sys_time);
    }
}

unsafe fn call_grabber_set_presentation_clock(cb: usize, clock: usize) {
    if cb != 0 {
        let vtbl = *(cb as *const *const usize);
        let f: unsafe extern "win64" fn(usize, usize) -> i32 = std::mem::transmute(*vtbl.add(8));
        f(cb, clock);
    }
}

unsafe fn call_grabber_process_sample(
    cb: usize,
    guid_major: *const [u8; 16],
    flags: u32,
    sample_time: i64,
    sample_dur: i64,
    buf: *const u8,
    buf_len: u32,
) -> i32 {
    if cb == 0 {
        return E_POINTER;
    }
    let vtbl = *(cb as *const *const usize);
    let f: unsafe extern "win64" fn(usize, *const [u8; 16], u32, i64, i64, *const u8, u32) -> i32 =
        std::mem::transmute(*vtbl.add(9));
    f(cb, guid_major, flags, sample_time, sample_dur, buf, buf_len)
}

unsafe fn call_grabber_shutdown(cb: usize) {
    if cb != 0 {
        let vtbl = *(cb as *const *const usize);
        let f: unsafe extern "win64" fn(usize) -> i32 = std::mem::transmute(*vtbl.add(10));
        f(cb);
    }
}

fn pix_fmt_for_subtype(sub: [u8; 16]) -> (&'static str, usize) {
    if sub == MFVideoFormat_NV12 {
        ("nv12", 12) // bits per pixel
    } else if sub == MFVideoFormat_RGB32 || sub == MFVideoFormat_ARGB32 {
        ("bgra", 32)
    } else {
        ("yuyv422", 16)
    }
}

unsafe extern "win64" fn sess_start(
    this: usize,
    _pguid_tf: *const [u8; 16],
    pvar_pos: *const PropVariant,
) -> i32 {
    let sess = &*(this as *const MfMediaSession);
    let start_100ns = if !pvar_pos.is_null() {
        let pv = &*pvar_pos;
        if pv.vt == VT_I8 || pv.vt == VT_UI8 {
            pv.val_lo as i64
        } else {
            0
        }
    } else {
        0
    };
    println!("[MF] sess_start(sess=0x{:X}, start_100ns={})", this, start_100ns);

    // If paused and no explicit new position, resume
    if let Some(pb) = sess.playback.lock().unwrap().as_ref() {
        if pb.pause.load(Ordering::SeqCst) && pvar_pos.is_null() {
            pb.pause.store(false, Ordering::SeqCst);
            let clk = &*(sess.clock as *const MfPresentationClock);
            let mut cg = clk.inner.lock().unwrap();
            cg.started_at = Some(Instant::now());
            cg.state = MFCLOCK_STATE_RUNNING;
            drop(cg);
            for &cb in pb.callbacks.lock().unwrap().iter() {
                call_grabber_clock_start(cb, 0, start_100ns);
            }
            eq_queue_event_param_var(sess.event_queue, MESessionStarted, &GUID_NULL, S_OK, std::ptr::null());
            return S_OK;
        }
    }

    // Stop any previous playback
    if let Some(old_pb) = sess.playback.lock().unwrap().take() {
        old_pb.stop_all();
    }

    let topo_ptr = *sess.topology.lock().unwrap();
    if topo_ptr == 0 {
        return MF_E_INVALIDREQUEST;
    }

    // Inspect topology nodes
    let mut media_info: Option<MediaInfo> = None;
    let mut video_sink: Option<(usize, [u8; 16])> = None;
    let mut audio_sink: Option<usize> = None;
    let mut has_audio_renderer = false;

    let topo = &*(topo_ptr as *const MfTopology);
    let nodes: Vec<usize> = topo.nodes.lock().unwrap().clone();
    for &n_ptr in &nodes {
        let tn = &*(n_ptr as *const MfTopologyNode);
        let obj = *tn.object.lock().unwrap();
        if tn.node_type == MF_TOPOLOGY_SOURCESTREAM_NODE {
            let mut candidates = Vec::new();
            if obj != 0 {
                candidates.push(obj);
            }
            if let Ok(map) = tn.header.attrs.map.lock() {
                for val in map.values() {
                    if let AttrValue::Unknown(u) = *val {
                        if u != 0 {
                            candidates.push(u);
                        }
                    }
                }
            }
            for cand in candidates {
                let mut src_ptr = 0usize;
                if com_query_interface(cand, &IID_IMFMediaSource, &mut src_ptr) == S_OK && src_ptr != 0 {
                    let ms = &*(src_ptr as *const MfMediaSource);
                    media_info = Some(ms.info.clone());
                    com_release(src_ptr);
                    break;
                }
            }
        } else if tn.node_type == MF_TOPOLOGY_OUTPUT_NODE && obj != 0 {
            let mut act_ptr = 0usize;
            if com_query_interface(obj, &IID_IMFActivate, &mut act_ptr) == S_OK && act_ptr != 0 {
                let act = &*(act_ptr as *const MfActivate);
                if act.is_audio_renderer {
                    has_audio_renderer = true;
                } else if act.callback != 0 {
                    if act.major_type == MFMediaType_Video {
                        com_add_ref(act.callback);
                        video_sink = Some((act.callback, act.subtype));
                    } else if act.major_type == MFMediaType_Audio {
                        com_add_ref(act.callback);
                        audio_sink = Some(act.callback);
                    }
                }
                com_release(act_ptr);
            }
        }
    }
    println!(
        "[MF] sess_start inspected {} nodes: media_info={}, video_sink={}, audio_sink={}, audio_renderer={}",
        nodes.len(),
        media_info.is_some(),
        video_sink.is_some(),
        audio_sink.is_some(),
        has_audio_renderer
    );

    queue_topology_status(sess.event_queue, topo_ptr, MF_TOPOSTATUS_STARTED_SOURCE);

    clk_start(sess.clock, start_100ns);
    eq_queue_event_param_var(sess.event_queue, MESessionStarted, &GUID_NULL, S_OK, std::ptr::null());

    let Some(info) = media_info else {
        return S_OK;
    };

    let pb = Arc::new(PlaybackControl::new());
    if let Some((vcb, _)) = video_sink {
        com_add_ref(vcb);
        pb.callbacks.lock().unwrap().push(vcb);
        call_grabber_set_presentation_clock(vcb, sess.clock);
        call_grabber_clock_start(vcb, 0, start_100ns);
    }
    if let Some(acb) = audio_sink {
        com_add_ref(acb);
        pb.callbacks.lock().unwrap().push(acb);
        call_grabber_set_presentation_clock(acb, sess.clock);
        call_grabber_clock_start(acb, 0, start_100ns);
    }

    let start_sec = (start_100ns.max(0) as f64) / 10_000_000.0;

    // Spawn video decoding thread
    if let Some((vcb, sub)) = video_sink {
        if info.has_video && info.width > 0 && info.height > 0 {
            let (pix_fmt, bpp) = pix_fmt_for_subtype(sub);
            let frame_bytes = (info.width as usize) * (info.height as usize) * bpp / 8;
            let frame_dur_100ns =
                (10_000_000i64 * (info.fps_den.max(1) as i64)) / (info.fps_num.max(1) as i64);
            let host_path = info.host_path.clone();
            let pb_v = Arc::clone(&pb);
            let eq_ptr = sess.event_queue;
            com_add_ref(eq_ptr);

            let child_res = Command::new("ffmpeg")
                .args([
                    "-v",
                    "quiet",
                    "-ss",
                    &format!("{:.3}", start_sec),
                    "-i",
                    &host_path,
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    pix_fmt,
                    "-",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();

            match child_res {
                Ok(mut child) => {
                    println!(
                        "[MF] sess_start: spawned video ffmpeg pid={} ({}x{}, fmt={}, path={})",
                        child.id(),
                        info.width,
                        info.height,
                        pix_fmt,
                        host_path
                    );
                    let stdout = child.stdout.take();
                    *pb.video_child.lock().unwrap() = Some(child);
                    let jh = std::thread::spawn(move || {
                        crate::dlls::kernel32::ensure_current_thread_teb();
                        let mut buf = vec![0u8; frame_bytes];
                        let t0 = Instant::now();
                        let mut frame_idx: i64 = 0;
                        let mut paused_dur = Duration::ZERO;
                        if let Some(mut out) = stdout {
                            loop {
                                if pb_v.stop.load(Ordering::SeqCst) {
                                    break;
                                }
                                if pb_v.pause.load(Ordering::SeqCst) {
                                    let p0 = Instant::now();
                                    while pb_v.pause.load(Ordering::SeqCst)
                                        && !pb_v.stop.load(Ordering::SeqCst)
                                    {
                                        std::thread::sleep(Duration::from_millis(10));
                                    }
                                    paused_dur += p0.elapsed();
                                    continue;
                                }
                                if out.read_exact(&mut buf).is_err() {
                                    break;
                                }
                                let pts_100ns = start_100ns + frame_idx * frame_dur_100ns;
                                let target_elapsed = Duration::from_nanos(
                                    (frame_idx as u64) * (frame_dur_100ns as u64) * 100,
                                );
                                let actual_elapsed = t0.elapsed().saturating_sub(paused_dur);
                                if target_elapsed > actual_elapsed {
                                    std::thread::sleep(target_elapsed - actual_elapsed);
                                }
                                if pb_v.stop.load(Ordering::SeqCst) {
                                    break;
                                }
                                let hr = call_grabber_process_sample(
                                    vcb,
                                    &MFMediaType_Video,
                                    0,
                                    pts_100ns,
                                    frame_dur_100ns,
                                    buf.as_ptr(),
                                    buf.len() as u32,
                                );
                                if frame_idx < 5 {
                                    println!(
                                        "[MF] video frame #{} pts_100ns={} bytes={} hr=0x{:08X}",
                                        frame_idx,
                                        pts_100ns,
                                        buf.len(),
                                        hr as u32
                                    );
                                }
                                frame_idx += 1;
                            }
                        }
                        if !pb_v.stop.load(Ordering::SeqCst) {
                            eq_queue_event_param_var(
                                eq_ptr,
                                MESessionEnded,
                                &GUID_NULL,
                                S_OK,
                                std::ptr::null(),
                            );
                        }
                        com_release(vcb);
                        com_release(eq_ptr);
                    });
                    pb.threads.lock().unwrap().push(jh);
                }
                Err(e) => {
                    println!("[MF] sess_start: failed to spawn video ffmpeg: {}", e);
                    com_release(vcb);
                    com_release(eq_ptr);
                }
            }
        } else {
            com_release(vcb);
        }
    }

    // Spawn audio decoding thread
    if info.has_audio && (audio_sink.is_some() || has_audio_renderer) {
        let ch = info.channels.max(1);
        let rate = info.sample_rate.max(8000);
        let bytes_per_sec = (rate * ch * 2) as usize;
        let chunk_bytes = (bytes_per_sec / 50).max(1024); // ~20ms
        let host_path = info.host_path.clone();
        let pb_a = Arc::clone(&pb);

        let child_res = Command::new("ffmpeg")
            .args([
                "-v",
                "quiet",
                "-ss",
                &format!("{:.3}", start_sec),
                "-i",
                &host_path,
                "-f",
                "s16le",
                "-ac",
                &ch.to_string(),
                "-ar",
                &rate.to_string(),
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        match child_res {
            Ok(mut child) => {
                println!(
                    "[MF] sess_start: spawned audio ffmpeg pid={} (ch={}, rate={}, path={})",
                    child.id(),
                    ch,
                    rate,
                    host_path
                );
                let stdout = child.stdout.take();
                *pb.audio_child.lock().unwrap() = Some(child);
                let jh = std::thread::spawn(move || {
                    crate::dlls::kernel32::ensure_current_thread_teb();
                    let mut buf = vec![0u8; chunk_bytes];
                let mut total_bytes: u64 = 0;
                let t0 = Instant::now();
                if let Some(mut out) = stdout {
                    loop {
                        if pb_a.stop.load(Ordering::SeqCst) {
                            break;
                        }
                        if pb_a.pause.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        let Ok(n) = out.read(&mut buf) else { break };
                        if n == 0 {
                            break;
                        }
                        let pts_100ns =
                            start_100ns + ((total_bytes * 10_000_000) / bytes_per_sec as u64) as i64;
                        let dur_100ns = ((n as u64 * 10_000_000) / bytes_per_sec as u64) as i64;
                        if let Some(acb) = audio_sink {
                            call_grabber_process_sample(
                                acb,
                                &MFMediaType_Audio,
                                0,
                                pts_100ns,
                                dur_100ns,
                                buf.as_ptr(),
                                n as u32,
                            );
                        } else if has_audio_renderer {
                            crate::dlls::wasapi::queue_sdl_audio(buf.as_ptr(), n);
                        }
                        total_bytes += n as u64;
                        let target = Duration::from_nanos(
                            (total_bytes * 1_000_000_000) / bytes_per_sec as u64,
                        );
                        let elapsed = t0.elapsed();
                        if target > elapsed {
                            std::thread::sleep(target - elapsed);
                        }
                    }
                }
                if let Some(acb) = audio_sink {
                    com_release(acb);
                }
            });
            pb.threads.lock().unwrap().push(jh);
            }
            Err(e) => {
                println!("[MF] sess_start: failed to spawn audio ffmpeg: {}", e);
                if let Some(acb) = audio_sink {
                    com_release(acb);
                }
            }
        }
    } else if let Some(acb) = audio_sink {
        com_release(acb);
    }

    *sess.playback.lock().unwrap() = Some(pb);
    S_OK
}

unsafe extern "win64" fn sess_pause(this: usize) -> i32 {
    let sess = &*(this as *const MfMediaSession);
    if let Some(pb) = sess.playback.lock().unwrap().as_ref() {
        pb.pause.store(true, Ordering::SeqCst);
        for &cb in pb.callbacks.lock().unwrap().iter() {
            call_grabber_clock_pause(cb, 0);
        }
    }
    clk_pause(sess.clock);
    eq_queue_event_param_var(sess.event_queue, MESessionPaused, &GUID_NULL, S_OK, std::ptr::null());
    S_OK
}

unsafe extern "win64" fn sess_stop(this: usize) -> i32 {
    let sess = &*(this as *const MfMediaSession);
    if let Some(pb) = sess.playback.lock().unwrap().take() {
        for &cb in pb.callbacks.lock().unwrap().iter() {
            call_grabber_clock_stop(cb, 0);
        }
        pb.stop_all();
    }
    clk_stop(sess.clock);
    eq_queue_event_param_var(sess.event_queue, MESessionStopped, &GUID_NULL, S_OK, std::ptr::null());
    S_OK
}

unsafe extern "win64" fn sess_close(this: usize) -> i32 {
    let sess = &*(this as *const MfMediaSession);
    if let Some(pb) = sess.playback.lock().unwrap().take() {
        pb.stop_all();
    }
    clk_stop(sess.clock);
    eq_queue_event_param_var(sess.event_queue, MESessionClosed, &GUID_NULL, S_OK, std::ptr::null());
    S_OK
}

unsafe extern "win64" fn sess_shutdown(this: usize) -> i32 {
    let sess = &*(this as *const MfMediaSession);
    if let Some(pb) = sess.playback.lock().unwrap().take() {
        for &cb in pb.callbacks.lock().unwrap().iter() {
            call_grabber_shutdown(cb);
        }
        pb.stop_all();
    }
    clk_stop(sess.clock);
    eq_shutdown(sess.event_queue)
}

unsafe extern "win64" fn sess_get_clock(this: usize, pp_clock: *mut usize) -> i32 {
    if pp_clock.is_null() {
        return E_POINTER;
    }
    let sess = &*(this as *const MfMediaSession);
    com_add_ref(sess.clock);
    *pp_clock = sess.clock;
    println!("[MF] sess_get_clock(sess=0x{:X}) -> 0x{:X}", this, sess.clock);
    S_OK
}

unsafe extern "win64" fn sess_get_caps(_this: usize, pdw_caps: *mut u32) -> i32 {
    if !pdw_caps.is_null() {
        *pdw_caps = 0x1 | 0x2 | 0x4; // SEEK | PAUSE | RATE_FORWARD
    }
    S_OK
}

unsafe extern "win64" fn sess_get_full_topology(
    this: usize,
    _dw_flags: u32,
    _topo_id: u64,
    pp_topo: *mut usize,
) -> i32 {
    if pp_topo.is_null() {
        return E_POINTER;
    }
    let sess = &*(this as *const MfMediaSession);
    let t = *sess.topology.lock().unwrap();
    if t == 0 {
        *pp_topo = 0;
        return MF_E_INVALIDREQUEST;
    }
    com_add_ref(t);
    *pp_topo = t;
    S_OK
}

static MF_MEDIA_SESSION_VTBL: SyncVtbl<17> = SyncVtbl([
    sess_qi as *const (),
    generic_add_ref::<MfMediaSession> as *const (),
    generic_release::<MfMediaSession> as *const (),
    sess_get_event as *const (),
    sess_begin_get_event as *const (),
    sess_end_get_event as *const (),
    sess_queue_event as *const (),
    sess_set_topology as *const (),
    sess_clear_topologies as *const (),
    sess_start as *const (),
    sess_pause as *const (),
    sess_stop as *const (),
    sess_close as *const (),
    sess_shutdown as *const (),
    sess_get_clock as *const (),
    sess_get_caps as *const (),
    sess_get_full_topology as *const (),
]);

// ============================================================================
// Exported Win64 Media Foundation APIs
// ============================================================================

#[no_mangle]
pub unsafe extern "win64" fn MFStartup(_version: u32, _dw_flags: u32) -> i32 {
    println!("[MF] MFStartup()");
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFShutdown() -> i32 {
    println!("[MF] MFShutdown()");
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateAttributes(
    pp_attrs: *mut usize,
    _c_initial_size: u32,
) -> i32 {
    if pp_attrs.is_null() {
        return E_POINTER;
    }
    *pp_attrs = create_attributes();
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateMediaType(pp_type: *mut usize) -> i32 {
    if pp_type.is_null() {
        return E_POINTER;
    }
    *pp_type = create_media_type();
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateAlignedMemoryBuffer(
    cb_max_len: u32,
    _f_alignment: u32,
    pp_buffer: *mut usize,
) -> i32 {
    if pp_buffer.is_null() {
        return E_POINTER;
    }
    *pp_buffer = create_memory_buffer(cb_max_len);
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateSample(pp_sample: *mut usize) -> i32 {
    if pp_sample.is_null() {
        return E_POINTER;
    }
    *pp_sample = create_sample();
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateEventQueue(pp_eq: *mut usize) -> i32 {
    if pp_eq.is_null() {
        return E_POINTER;
    }
    *pp_eq = create_event_queue();
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateAsyncResult(
    punk_object: usize,
    p_callback: usize,
    punk_state: usize,
    pp_async_result: *mut usize,
) -> i32 {
    if pp_async_result.is_null() {
        return E_POINTER;
    }
    *pp_async_result = create_async_result(punk_object, p_callback, punk_state);
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFInvokeCallback(p_async_result: usize) -> i32 {
    invoke_async_result(p_async_result)
}

#[no_mangle]
pub unsafe extern "win64" fn MFPutWorkItem(
    _dw_queue: u32,
    p_callback: usize,
    p_state: usize,
) -> i32 {
    if p_callback == 0 {
        return E_POINTER;
    }
    let ar = create_async_result(0, p_callback, p_state);
    std::thread::spawn(move || {
        crate::dlls::kernel32::ensure_current_thread_teb();
        invoke_async_result(ar);
        com_release(ar);
    });
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFPutWaitingWorkItem(
    h_event: usize,
    _l_priority: i32,
    p_result: usize,
    p_key: *mut u64,
) -> i32 {
    if p_result == 0 {
        return E_POINTER;
    }
    if !p_key.is_null() {
        *p_key = 1;
    }
    com_add_ref(p_result);
    std::thread::spawn(move || {
        crate::dlls::kernel32::ensure_current_thread_teb();
        if h_event != 0 {
            crate::dlls::kernel32::WaitForSingleObject(h_event as u64, 0xFFFFFFFF);
        }
        invoke_async_result(p_result);
        com_release(p_result);
    });
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateDXGIDeviceManager(
    p_reset_token: *mut u32,
    pp_mgr: *mut usize,
) -> i32 {
    if !p_reset_token.is_null() {
        *p_reset_token = 0;
    }
    if !pp_mgr.is_null() {
        *pp_mgr = 0;
    }
    E_NOTIMPL
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateSourceResolver(pp_resolver: *mut usize) -> i32 {
    if pp_resolver.is_null() {
        return E_POINTER;
    }
    *pp_resolver = Box::into_raw(Box::new(MfSourceResolver {
        vtable: MF_SOURCE_RESOLVER_VTBL.as_ptr(),
        ref_count: AtomicU32::new(1),
    })) as usize;
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateMediaSession(
    _p_config: usize,
    pp_session: *mut usize,
) -> i32 {
    if pp_session.is_null() {
        return E_POINTER;
    }
    let boxed = Box::new(MfMediaSession {
        vtable: MF_MEDIA_SESSION_VTBL.as_ptr(),
        ref_count: AtomicU32::new(1),
        get_service: MfSessionGetService {
            vtable: MF_SESSION_GET_SERVICE_VTBL.as_ptr(),
            session: 0,
        },
        event_queue: create_event_queue(),
        clock: create_presentation_clock(),
        topology: Mutex::new(0),
        playback: Mutex::new(None),
    });
    let ptr = Box::into_raw(boxed);
    (*ptr).get_service.session = ptr as usize;
    *pp_session = ptr as usize;
    println!("[MF] MFCreateMediaSession -> 0x{:X}", ptr as usize);
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateTopology(pp_topology: *mut usize) -> i32 {
    if pp_topology.is_null() {
        return E_POINTER;
    }
    *pp_topology = Box::into_raw(Box::new(MfTopology {
        header: MfAttrHeader::new(MF_TOPOLOGY_VTBL.as_ptr()),
        topo_id: AtomicU64::new(1),
        nodes: Mutex::new(Vec::new()),
    })) as usize;
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateTopologyNode(node_type: u32, pp_node: *mut usize) -> i32 {
    if pp_node.is_null() {
        return E_POINTER;
    }
    static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);
    *pp_node = Box::into_raw(Box::new(MfTopologyNode {
        header: MfAttrHeader::new(MF_TOPOLOGY_NODE_VTBL.as_ptr()),
        node_type,
        topo_id: AtomicU64::new(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed)),
        object: Mutex::new(0),
        outputs: Mutex::new(Vec::new()),
    })) as usize;
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateSampleGrabberSinkActivate(
    p_media_type: usize,
    p_callback: usize,
    pp_activate: *mut usize,
) -> i32 {
    if pp_activate.is_null() {
        return E_POINTER;
    }
    let mut major = GUID_NULL;
    let mut sub = GUID_NULL;
    if p_media_type != 0 {
        let vtbl = *(p_media_type as *const *const usize);
        let get_guid: unsafe extern "win64" fn(usize, *const [u8; 16], *mut [u8; 16]) -> i32 =
            std::mem::transmute(*vtbl.add(10));
        get_guid(p_media_type, &MF_MT_MAJOR_TYPE, &mut major);
        get_guid(p_media_type, &MF_MT_SUBTYPE, &mut sub);
    }
    com_add_ref(p_callback);
    println!(
        "[MF] MFCreateSampleGrabberSinkActivate(cb=0x{:X}, major={:02X?}, sub={:02X?})",
        p_callback,
        &major[0..4],
        &sub[0..4]
    );
    *pp_activate = Box::into_raw(Box::new(MfActivate {
        header: MfAttrHeader::new(MF_ACTIVATE_VTBL.as_ptr()),
        is_audio_renderer: false,
        major_type: major,
        subtype: sub,
        callback: p_callback,
    })) as usize;
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateAudioRendererActivate(pp_activate: *mut usize) -> i32 {
    if pp_activate.is_null() {
        return E_POINTER;
    }
    *pp_activate = Box::into_raw(Box::new(MfActivate {
        header: MfAttrHeader::new(MF_ACTIVATE_VTBL.as_ptr()),
        is_audio_renderer: true,
        major_type: MFMediaType_Audio,
        subtype: MFAudioFormat_PCM,
        callback: 0,
    })) as usize;
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFGetService(
    punk_object: usize,
    guid_service: *const [u8; 16],
    riid: *const [u8; 16],
    ppv_object: *mut usize,
) -> i32 {
    if punk_object == 0 || guid_service.is_null() || riid.is_null() || ppv_object.is_null() {
        return E_POINTER;
    }
    let mut gs_ptr = 0usize;
    if com_query_interface(punk_object, &IID_IMFGetService, &mut gs_ptr) == S_OK && gs_ptr != 0 {
        let vtbl = *(gs_ptr as *const *const usize);
        let f: unsafe extern "win64" fn(
            usize,
            *const [u8; 16],
            *const [u8; 16],
            *mut usize,
        ) -> i32 = std::mem::transmute(*vtbl.add(3));
        let hr = f(gs_ptr, guid_service, riid, ppv_object);
        com_release(gs_ptr);
        return hr;
    }
    *ppv_object = 0;
    E_NOINTERFACE
}

#[no_mangle]
pub unsafe extern "win64" fn MFEnumDeviceSources(
    _p_attrs: usize,
    ppp_sources: *mut usize,
    pc_sources: *mut u32,
) -> i32 {
    if !pc_sources.is_null() {
        *pc_sources = 0;
    }
    if !ppp_sources.is_null() {
        *ppp_sources = 0;
    }
    S_OK
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateDeviceSource(
    _p_attrs: usize,
    pp_source: *mut usize,
) -> i32 {
    if !pp_source.is_null() {
        *pp_source = 0;
    }
    E_NOTIMPL
}

#[no_mangle]
pub unsafe extern "win64" fn MFCreateSinkWriterFromURL(
    _pwsz_url: *const u16,
    _p_bs: usize,
    _p_attrs: usize,
    pp_writer: *mut usize,
) -> i32 {
    if !pp_writer.is_null() {
        *pp_writer = 0;
    }
    E_NOTIMPL
}

#[no_mangle]
pub unsafe extern "win64" fn MFPCreateMediaPlayer(
    _pwsz_url: *const u16,
    _f_start_playback: i32,
    _creation_options: u32,
    _p_callback: usize,
    _hwnd: usize,
    pp_media_player: *mut usize,
) -> i32 {
    if !pp_media_player.is_null() {
        *pp_media_player = 0;
    }
    E_NOTIMPL
}

#[no_mangle]
pub unsafe extern "win64" fn MFPCreateMediaPlayerEx(
    _pwsz_url: *const u16,
    _f_start_playback: i32,
    _creation_options: u32,
    _p_callback: usize,
    _hwnd: usize,
    _p_create_attrs: usize,
    pp_media_player: *mut usize,
) -> i32 {
    if !pp_media_player.is_null() {
        *pp_media_player = 0;
    }
    E_NOTIMPL
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mf_attributes_and_media_type() {
        unsafe {
            assert_eq!(MFStartup(0x00020070, 0), S_OK);

            let mut attrs = 0usize;
            assert_eq!(MFCreateAttributes(&mut attrs, 4), S_OK);
            assert_ne!(attrs, 0);

            assert_eq!(attr_set_uint32(attrs, &MF_MT_AUDIO_NUM_CHANNELS, 2), S_OK);
            let mut ch = 0u32;
            assert_eq!(attr_get_uint32(attrs, &MF_MT_AUDIO_NUM_CHANNELS, &mut ch), S_OK);
            assert_eq!(ch, 2);

            assert_eq!(attr_set_uint64(attrs, &MF_PD_DURATION, 123_456_789), S_OK);
            let mut dur = 0u64;
            assert_eq!(attr_get_uint64(attrs, &MF_PD_DURATION, &mut dur), S_OK);
            assert_eq!(dur, 123_456_789);

            let mut mt = 0usize;
            assert_eq!(MFCreateMediaType(&mut mt), S_OK);
            assert_eq!(attr_set_guid(mt, &MF_MT_MAJOR_TYPE, &MFMediaType_Video), S_OK);
            assert_eq!(attr_set_guid(mt, &MF_MT_SUBTYPE, &MFVideoFormat_YUY2), S_OK);
            let mut maj = GUID_NULL;
            assert_eq!(mt_get_major_type(mt, &mut maj), S_OK);
            assert_eq!(maj, MFMediaType_Video);

            com_release(mt);
            com_release(attrs);
            assert_eq!(MFShutdown(), S_OK);
        }
    }

    #[test]
    fn test_mf_sample_and_buffer() {
        unsafe {
            let mut buf = 0usize;
            assert_eq!(MFCreateAlignedMemoryBuffer(64, 16, &mut buf), S_OK);
            assert_ne!(buf, 0);

            let mut ptr: *mut u8 = std::ptr::null_mut();
            let mut max_len = 0u32;
            let mut cur_len = 0u32;
            assert_eq!(buf_lock(buf, &mut ptr, &mut max_len, &mut cur_len), S_OK);
            assert_eq!(max_len, 64);
            assert_eq!(cur_len, 0);
            *ptr = 0xAB;
            assert_eq!(buf_unlock(buf), S_OK);
            assert_eq!(buf_set_current_length(buf, 16), S_OK);

            let mut sample = 0usize;
            assert_eq!(MFCreateSample(&mut sample), S_OK);
            assert_eq!(sample_add_buffer(sample, buf), S_OK);
            assert_eq!(sample_set_time(sample, 1000), S_OK);
            assert_eq!(sample_set_duration(sample, 333333), S_OK);

            let mut total = 0u32;
            assert_eq!(sample_get_total_length(sample, &mut total), S_OK);
            assert_eq!(total, 16);

            com_release(sample);
            com_release(buf);
        }
    }

    #[test]
    fn test_mf_event_queue_and_source_resolver_missing_file() {
        unsafe {
            let mut eq = 0usize;
            assert_eq!(MFCreateEventQueue(&mut eq), S_OK);

            assert_eq!(
                eq_queue_event_param_var(eq, MESessionStarted, &GUID_NULL, S_OK, std::ptr::null()),
                S_OK
            );
            let mut ev = 0usize;
            assert_eq!(eq_get_event(eq, MF_EVENT_FLAG_NO_WAIT, &mut ev), S_OK);
            assert_ne!(ev, 0);
            let mut met = 0u32;
            assert_eq!(me_get_type(ev, &mut met), S_OK);
            assert_eq!(met, MESessionStarted);
            com_release(ev);

            let mut ev2 = 0usize;
            assert_eq!(
                eq_get_event(eq, MF_EVENT_FLAG_NO_WAIT, &mut ev2),
                MF_E_NO_EVENTS_AVAILABLE
            );
            com_release(eq);

            let mut sr = 0usize;
            assert_eq!(MFCreateSourceResolver(&mut sr), S_OK);
            let missing: Vec<u16> = "C:\\nonexistent_video_12345.mp4\0".encode_utf16().collect();
            let mut obj_type = 0u32;
            let mut obj = 0usize;
            let hr = sr_create_object_from_url(
                sr,
                missing.as_ptr(),
                0,
                0,
                &mut obj_type,
                &mut obj,
            );
            assert_eq!(hr, HRESULT_FILE_NOT_FOUND);
            assert_eq!(obj, 0);
            com_release(sr);
        }
    }

    #[test]
    fn test_mf_source_resolver_bytestream() {
        #[repr(C)]
        struct MockByteStream {
            vtable: *const *const (),
            data: Vec<u8>,
            pos: usize,
            seek_called: bool,
        }

        unsafe extern "win64" fn bs_qi(_this: usize, _riid: *const [u8; 16], ppv: *mut usize) -> i32 {
            *ppv = 0;
            E_NOINTERFACE
        }
        unsafe extern "win64" fn bs_add_ref(_this: usize) -> u32 {
            1
        }
        unsafe extern "win64" fn bs_release(_this: usize) -> u32 {
            1
        }
        unsafe extern "win64" fn bs_get_caps(_this: usize, pdw: *mut u32) -> i32 {
            *pdw = 0;
            S_OK
        }
        unsafe extern "win64" fn bs_get_length(this: usize, pqw: *mut u64) -> i32 {
            let bs = &*(this as *const MockByteStream);
            *pqw = bs.data.len() as u64;
            S_OK
        }
        unsafe extern "win64" fn bs_set_length(_this: usize, _qw: u64) -> i32 {
            E_NOTIMPL
        }
        unsafe extern "win64" fn bs_get_pos(this: usize, pqw: *mut u64) -> i32 {
            let bs = &*(this as *const MockByteStream);
            *pqw = bs.pos as u64;
            S_OK
        }
        unsafe extern "win64" fn bs_set_pos(this: usize, qw: u64) -> i32 {
            let bs = &mut *(this as *mut MockByteStream);
            bs.seek_called = true;
            bs.pos = (qw as usize).min(bs.data.len());
            S_OK
        }
        unsafe extern "win64" fn bs_is_eos(this: usize, pf: *mut i32) -> i32 {
            let bs = &*(this as *const MockByteStream);
            *pf = if bs.pos >= bs.data.len() { 1 } else { 0 };
            S_OK
        }
        unsafe extern "win64" fn bs_read(
            this: usize,
            pb: *mut u8,
            cb: u32,
            pcb_read: *mut u32,
        ) -> i32 {
            let bs = &mut *(this as *mut MockByteStream);
            let avail = bs.data.len().saturating_sub(bs.pos);
            let n = avail.min(cb as usize);
            if n > 0 {
                std::ptr::copy_nonoverlapping(bs.data.as_ptr().add(bs.pos), pb, n);
                bs.pos += n;
            }
            *pcb_read = n as u32;
            S_OK
        }

        static MOCK_BS_VTBL: SyncVtbl<10> = SyncVtbl([
            bs_qi as *const (),
            bs_add_ref as *const (),
            bs_release as *const (),
            bs_get_caps as *const (),
            bs_get_length as *const (),
            bs_set_length as *const (),
            bs_get_pos as *const (),
            bs_set_pos as *const (),
            bs_is_eos as *const (),
            bs_read as *const (),
        ]);

        unsafe {
            let mut sr = 0usize;
            assert_eq!(MFCreateSourceResolver(&mut sr), S_OK);

            // 1. Null p_bs with missing URL returns HRESULT_FILE_NOT_FOUND
            let missing: Vec<u16> = "C:\\nonexistent_bs_12345.mp4\0".encode_utf16().collect();
            let mut obj_type = 99u32;
            let mut obj = 99usize;
            let hr = sr_create_object_from_bytestream(
                sr,
                0,
                missing.as_ptr(),
                0,
                0,
                &mut obj_type,
                &mut obj,
            );
            assert_eq!(hr, HRESULT_FILE_NOT_FOUND);
            assert_eq!(obj, 0);
            assert_eq!(obj_type, 0);

            // 2. Mock IMFByteStream with invalid media bytes: seeks to 0, reads all bytes,
            //    fails ffprobe, cleans up temp file, returns E_FAIL.
            let mut mock_bs = MockByteStream {
                vtable: MOCK_BS_VTBL.0.as_ptr(),
                data: b"not a real mp4 file".to_vec(),
                pos: 5,
                seek_called: false,
            };
            let bs_ptr = (&mut mock_bs as *mut MockByteStream) as usize;
            let hr = sr_create_object_from_bytestream(
                sr,
                bs_ptr,
                std::ptr::null(),
                0,
                0,
                &mut obj_type,
                &mut obj,
            );
            assert!(mock_bs.seek_called);
            assert_eq!(mock_bs.pos, mock_bs.data.len());
            assert_eq!(hr, E_FAIL);
            assert_eq!(obj, 0);

            // 3. sr_begin_create_from_bs / sr_end_create_from_bs null checks
            assert_eq!(
                sr_begin_create_from_bs(sr, 0, std::ptr::null(), 0, 0, std::ptr::null_mut(), 0, 0),
                E_POINTER
            );
            assert_eq!(
                sr_end_create_from_bs(sr, 0, &mut obj_type, &mut obj),
                E_POINTER
            );

            com_release(sr);
        }
    }

    #[test]
    fn test_mf_get_allocated_blob_and_vtbl_slots() {
        unsafe {
            let mut attrs = 0usize;
            assert_eq!(MFCreateAttributes(&mut attrs, 2), S_OK);

            let key: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
            let payload = [0xDEu8, 0xAD, 0xBE, 0xEF];

            let mut out_ptr: *mut u8 = std::ptr::null_mut();
            let mut out_len = 0u32;
            assert_eq!(
                attr_get_allocated_blob(attrs, &key, &mut out_ptr, &mut out_len),
                MF_E_ATTRIBUTENOTFOUND
            );

            assert_eq!(attr_set_blob(attrs, &key, payload.as_ptr(), payload.len() as u32), S_OK);
            assert_eq!(
                attr_get_allocated_blob(attrs, &key, &mut out_ptr, &mut out_len),
                S_OK
            );
            assert_eq!(out_len as usize, payload.len());
            assert!(!out_ptr.is_null());
            assert_eq!(std::slice::from_raw_parts(out_ptr, out_len as usize), &payload);
            crate::dlls::shell::CoTaskMemFree(out_ptr as *mut std::ffi::c_void);

            // Verify IMFAttributes has 33 slots (0..32) with GetAllocatedBlob at slot 16 (+0x80)
            // and IMFPresentationDescriptor::GetStreamDescriptorCount at slot 33 (+0x108).
            assert_eq!(MF_ATTRS_VTBL.0[16], attr_get_allocated_blob as *const ());
            assert_eq!(MF_PRESENTATION_DESCRIPTOR_VTBL.0[33], pd_get_stream_count as *const ());
            assert_eq!(MF_PRESENTATION_DESCRIPTOR_VTBL.0[34], pd_get_stream_by_index as *const ());

            com_release(attrs);
        }
    }
}
