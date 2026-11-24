//! Dedicated Rine Rust implementation module for XInput (xinput1_3.dll, xinput1_4.dll, xinput9_1_0.dll, xinputuap.dll)
#![allow(non_snake_case)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::ffi::{c_void, CString};

pub const ERROR_SUCCESS: u32 = 0;
pub const ERROR_BAD_ARGUMENTS: u32 = 160;
pub const ERROR_DEVICE_NOT_CONNECTED: u32 = 1167;
pub const ERROR_EMPTY: u32 = 4306;

// Device Types & Subtypes
pub const XINPUT_DEVTYPE_GAMEPAD: u8 = 0x01;
pub const XINPUT_DEVSUBTYPE_GAMEPAD: u8 = 0x01;

// XINPUT_GAMEPAD Buttons bitmasks
pub const XINPUT_GAMEPAD_DPAD_UP: u16 = 0x0001;
pub const XINPUT_GAMEPAD_DPAD_DOWN: u16 = 0x0002;
pub const XINPUT_GAMEPAD_DPAD_LEFT: u16 = 0x0004;
pub const XINPUT_GAMEPAD_DPAD_RIGHT: u16 = 0x0008;
pub const XINPUT_GAMEPAD_START: u16 = 0x0010;
pub const XINPUT_GAMEPAD_BACK: u16 = 0x0020;
pub const XINPUT_GAMEPAD_LEFT_THUMB: u16 = 0x0040;
pub const XINPUT_GAMEPAD_RIGHT_THUMB: u16 = 0x0080;
pub const XINPUT_GAMEPAD_LEFT_SHOULDER: u16 = 0x0100;
pub const XINPUT_GAMEPAD_RIGHT_SHOULDER: u16 = 0x0200;
pub const XINPUT_GAMEPAD_GUIDE: u16 = 0x0400;
pub const XINPUT_GAMEPAD_A: u16 = 0x1000;
pub const XINPUT_GAMEPAD_B: u16 = 0x2000;
pub const XINPUT_GAMEPAD_X: u16 = 0x4000;
pub const XINPUT_GAMEPAD_Y: u16 = 0x8000;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct XINPUT_GAMEPAD {
    pub wButtons: u16,
    pub bLeftTrigger: u8,
    pub bRightTrigger: u8,
    pub sThumbLX: i16,
    pub sThumbLY: i16,
    pub sThumbRX: i16,
    pub sThumbRY: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct XINPUT_STATE {
    pub dwPacketNumber: u32,
    pub Gamepad: XINPUT_GAMEPAD,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct XINPUT_STATE_EX {
    pub dwPacketNumber: u32,
    pub Gamepad: XINPUT_GAMEPAD,
    pub wButtonsEx: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct XINPUT_VIBRATION {
    pub wLeftMotorSpeed: u16,
    pub wRightMotorSpeed: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct XINPUT_CAPABILITIES {
    pub Type: u8,
    pub SubType: u8,
    pub Flags: u16,
    pub Gamepad: XINPUT_GAMEPAD,
    pub Vibration: XINPUT_VIBRATION,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct XINPUT_BATTERY_INFORMATION {
    pub BatteryType: u8,
    pub BatteryLevel: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct XINPUT_KEYSTROKE {
    pub VirtualKey: u16,
    pub Unicode: u16,
    pub Flags: u16,
    pub UserIndex: u8,
    pub HidCode: u8,
}

static XINPUT_ENABLED: AtomicBool = AtomicBool::new(true);

type SdlInitSubSystemFn = unsafe extern "C" fn(u32) -> i32;
type SdlNumJoysticksFn = unsafe extern "C" fn() -> i32;
type SdlIsGameControllerFn = unsafe extern "C" fn(i32) -> i32;
type SdlGameControllerOpenFn = unsafe extern "C" fn(i32) -> *mut c_void;
type SdlGameControllerCloseFn = unsafe extern "C" fn(*mut c_void);
type SdlGameControllerGetButtonFn = unsafe extern "C" fn(*mut c_void, i32) -> u8;
type SdlGameControllerGetAxisFn = unsafe extern "C" fn(*mut c_void, i32) -> i16;
type SdlGameControllerRumbleFn = unsafe extern "C" fn(*mut c_void, u16, u16, u32) -> i32;
type SdlGameControllerGetAttachedFn = unsafe extern "C" fn(*mut c_void) -> i32;
type SdlGameControllerUpdateFn = unsafe extern "C" fn();

struct SdlControllerApi {
    fn_num_joysticks: SdlNumJoysticksFn,
    fn_is_game_controller: SdlIsGameControllerFn,
    fn_open: SdlGameControllerOpenFn,
    fn_close: SdlGameControllerCloseFn,
    fn_get_button: SdlGameControllerGetButtonFn,
    fn_get_axis: SdlGameControllerGetAxisFn,
    fn_rumble: Option<SdlGameControllerRumbleFn>,
    fn_get_attached: SdlGameControllerGetAttachedFn,
    fn_update: SdlGameControllerUpdateFn,
}

static SDL_API: Mutex<Option<Option<SdlControllerApi>>> = Mutex::new(None);
static CONTROLLERS: Mutex<[Option<usize>; 4]> = Mutex::new([None, None, None, None]);
static PACKET_NUMBERS: [std::sync::atomic::AtomicU32; 4] = [
    std::sync::atomic::AtomicU32::new(1),
    std::sync::atomic::AtomicU32::new(1),
    std::sync::atomic::AtomicU32::new(1),
    std::sync::atomic::AtomicU32::new(1),
];

fn get_sdl_controller_api() -> Option<&'static SdlControllerApi> {
    let mut guard = SDL_API.lock().unwrap();
    if guard.is_none() {
        let sdl_handle = crate::windowing::x11::get_sdl_handle();
        let handle = if !sdl_handle.is_null() {
            sdl_handle
        } else {
            let lib_name = CString::new("libSDL2-2.0.so.0").unwrap();
            unsafe { libc::dlopen(lib_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) }
        };

        if handle.is_null() {
            *guard = Some(None);
            return None;
        }

        unsafe {
            let fn_init_subsystem: Option<SdlInitSubSystemFn> = {
                let sym = libc::dlsym(handle, CString::new("SDL_InitSubSystem").unwrap().as_ptr());
                if !sym.is_null() { Some(std::mem::transmute(sym)) } else { None }
            };
            if let Some(init) = fn_init_subsystem {
                init(0x00002000); // SDL_INIT_GAMECONTROLLER
            }

            let sym_num = libc::dlsym(handle, CString::new("SDL_NumJoysticks").unwrap().as_ptr());
            let sym_is = libc::dlsym(handle, CString::new("SDL_IsGameController").unwrap().as_ptr());
            let sym_open = libc::dlsym(handle, CString::new("SDL_GameControllerOpen").unwrap().as_ptr());
            let sym_close = libc::dlsym(handle, CString::new("SDL_GameControllerClose").unwrap().as_ptr());
            let sym_btn = libc::dlsym(handle, CString::new("SDL_GameControllerGetButton").unwrap().as_ptr());
            let sym_axis = libc::dlsym(handle, CString::new("SDL_GameControllerGetAxis").unwrap().as_ptr());
            let sym_rumble = libc::dlsym(handle, CString::new("SDL_GameControllerRumble").unwrap().as_ptr());
            let sym_att = libc::dlsym(handle, CString::new("SDL_GameControllerGetAttached").unwrap().as_ptr());
            let sym_upd = libc::dlsym(handle, CString::new("SDL_GameControllerUpdate").unwrap().as_ptr());

            if !sym_num.is_null() && !sym_is.is_null() && !sym_open.is_null() && !sym_close.is_null()
                && !sym_btn.is_null() && !sym_axis.is_null() && !sym_att.is_null() && !sym_upd.is_null()
            {
                let api = SdlControllerApi {
                    fn_num_joysticks: std::mem::transmute(sym_num),
                    fn_is_game_controller: std::mem::transmute(sym_is),
                    fn_open: std::mem::transmute(sym_open),
                    fn_close: std::mem::transmute(sym_close),
                    fn_get_button: std::mem::transmute(sym_btn),
                    fn_get_axis: std::mem::transmute(sym_axis),
                    fn_rumble: if !sym_rumble.is_null() { Some(std::mem::transmute(sym_rumble)) } else { None },
                    fn_get_attached: std::mem::transmute(sym_att),
                    fn_update: std::mem::transmute(sym_upd),
                };
                *guard = Some(Some(api));
            } else {
                *guard = Some(None);
            }
        }
    }

    if let Some(Some(ref api)) = *guard {
        // Safe to cast to static lifetime because guard is permanently stored in static Mutex
        Some(unsafe { &*(api as *const SdlControllerApi) })
    } else {
        None
    }
}

fn get_or_open_controller(user_index: usize, api: &SdlControllerApi) -> Option<*mut c_void> {
    if user_index >= 4 {
        return None;
    }
    let mut ctrl_guard = CONTROLLERS.lock().unwrap();
    if let Some(ptr_val) = ctrl_guard[user_index] {
        let ptr = ptr_val as *mut c_void;
        if unsafe { (api.fn_get_attached)(ptr) } != 0 {
            return Some(ptr);
        } else {
            unsafe { (api.fn_close)(ptr) };
            ctrl_guard[user_index] = None;
        }
    }

    // Attempt to discover available controller
    unsafe { (api.fn_update)() };
    let num = unsafe { (api.fn_num_joysticks)() };
    let mut controller_count = 0;
    for i in 0..num {
        if unsafe { (api.fn_is_game_controller)(i) } != 0 {
            if controller_count == user_index {
                let ptr = unsafe { (api.fn_open)(i) };
                if !ptr.is_null() {
                    ctrl_guard[user_index] = Some(ptr as usize);
                    return Some(ptr);
                }
            }
            controller_count += 1;
        }
    }
    None
}

#[no_mangle]
pub unsafe extern "win64" fn XInputGetState(dw_user_index: u32, p_state: *mut XINPUT_STATE) -> u32 {
    if dw_user_index >= 4 || p_state.is_null() {
        return ERROR_DEVICE_NOT_CONNECTED;
    }
    if !XINPUT_ENABLED.load(Ordering::Relaxed) {
        return ERROR_DEVICE_NOT_CONNECTED;
    }

    let api = match get_sdl_controller_api() {
        Some(a) => a,
        None => return ERROR_DEVICE_NOT_CONNECTED,
    };

    let controller = match get_or_open_controller(dw_user_index as usize, api) {
        Some(c) => c,
        None => return ERROR_DEVICE_NOT_CONNECTED,
    };

    unsafe { (api.fn_update)() };

    let mut buttons: u16 = 0;
    // SDL GameController buttons:
    // 0: A, 1: B, 2: X, 3: Y, 4: Back, 5: Guide, 6: Start, 7: LeftStick, 8: RightStick, 9: LeftShoulder, 10: RightShoulder, 11: Up, 12: Down, 13: Left, 14: Right
    if (api.fn_get_button)(controller, 0) != 0 { buttons |= XINPUT_GAMEPAD_A; }
    if (api.fn_get_button)(controller, 1) != 0 { buttons |= XINPUT_GAMEPAD_B; }
    if (api.fn_get_button)(controller, 2) != 0 { buttons |= XINPUT_GAMEPAD_X; }
    if (api.fn_get_button)(controller, 3) != 0 { buttons |= XINPUT_GAMEPAD_Y; }
    if (api.fn_get_button)(controller, 4) != 0 { buttons |= XINPUT_GAMEPAD_BACK; }
    if (api.fn_get_button)(controller, 6) != 0 { buttons |= XINPUT_GAMEPAD_START; }
    if (api.fn_get_button)(controller, 7) != 0 { buttons |= XINPUT_GAMEPAD_LEFT_THUMB; }
    if (api.fn_get_button)(controller, 8) != 0 { buttons |= XINPUT_GAMEPAD_RIGHT_THUMB; }
    if (api.fn_get_button)(controller, 9) != 0 { buttons |= XINPUT_GAMEPAD_LEFT_SHOULDER; }
    if (api.fn_get_button)(controller, 10) != 0 { buttons |= XINPUT_GAMEPAD_RIGHT_SHOULDER; }
    if (api.fn_get_button)(controller, 11) != 0 { buttons |= XINPUT_GAMEPAD_DPAD_UP; }
    if (api.fn_get_button)(controller, 12) != 0 { buttons |= XINPUT_GAMEPAD_DPAD_DOWN; }
    if (api.fn_get_button)(controller, 13) != 0 { buttons |= XINPUT_GAMEPAD_DPAD_LEFT; }
    if (api.fn_get_button)(controller, 14) != 0 { buttons |= XINPUT_GAMEPAD_DPAD_RIGHT; }

    // Axes:
    // 0: LeftX, 1: LeftY (SDL Y down positive, XInput Y up positive), 2: RightX, 3: RightY, 4: TriggerLeft (0..32767), 5: TriggerRight (0..32767)
    let lx = (api.fn_get_axis)(controller, 0);
    let ly_sdl = (api.fn_get_axis)(controller, 1);
    let ly = if ly_sdl == -32768 { 32767 } else { -ly_sdl };
    let rx = (api.fn_get_axis)(controller, 2);
    let ry_sdl = (api.fn_get_axis)(controller, 3);
    let ry = if ry_sdl == -32768 { 32767 } else { -ry_sdl };

    let lt_raw = (api.fn_get_axis)(controller, 4).max(0) as u32;
    let rt_raw = (api.fn_get_axis)(controller, 5).max(0) as u32;
    let lt = (lt_raw * 255 / 32767) as u8;
    let rt = (rt_raw * 255 / 32767) as u8;

    let pkt = PACKET_NUMBERS[dw_user_index as usize].fetch_add(1, Ordering::Relaxed);

    (*p_state).dwPacketNumber = pkt;
    (*p_state).Gamepad = XINPUT_GAMEPAD {
        wButtons: buttons,
        bLeftTrigger: lt,
        bRightTrigger: rt,
        sThumbLX: lx,
        sThumbLY: ly,
        sThumbRX: rx,
        sThumbRY: ry,
    };

    ERROR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn XInputSetState(dw_user_index: u32, p_vibration: *const XINPUT_VIBRATION) -> u32 {
    if dw_user_index >= 4 || p_vibration.is_null() {
        return ERROR_DEVICE_NOT_CONNECTED;
    }
    if !XINPUT_ENABLED.load(Ordering::Relaxed) {
        return ERROR_DEVICE_NOT_CONNECTED;
    }

    let api = match get_sdl_controller_api() {
        Some(a) => a,
        None => return ERROR_DEVICE_NOT_CONNECTED,
    };

    let controller = match get_or_open_controller(dw_user_index as usize, api) {
        Some(c) => c,
        None => return ERROR_DEVICE_NOT_CONNECTED,
    };

    if let Some(rumble) = api.fn_rumble {
        let left = (*p_vibration).wLeftMotorSpeed;
        let right = (*p_vibration).wRightMotorSpeed;
        unsafe { rumble(controller, left, right, 2000) };
    }

    ERROR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn XInputGetCapabilities(
    dw_user_index: u32,
    _dw_flags: u32,
    p_capabilities: *mut XINPUT_CAPABILITIES,
) -> u32 {
    if dw_user_index >= 4 || p_capabilities.is_null() {
        return ERROR_DEVICE_NOT_CONNECTED;
    }

    let api = match get_sdl_controller_api() {
        Some(a) => a,
        None => return ERROR_DEVICE_NOT_CONNECTED,
    };

    if get_or_open_controller(dw_user_index as usize, api).is_none() {
        return ERROR_DEVICE_NOT_CONNECTED;
    }

    (*p_capabilities).Type = XINPUT_DEVTYPE_GAMEPAD;
    (*p_capabilities).SubType = XINPUT_DEVSUBTYPE_GAMEPAD;
    (*p_capabilities).Flags = 0;
    (*p_capabilities).Gamepad = XINPUT_GAMEPAD {
        wButtons: 0xFFFF,
        bLeftTrigger: 0xFF,
        bRightTrigger: 0xFF,
        sThumbLX: -1,
        sThumbLY: -1,
        sThumbRX: -1,
        sThumbRY: -1,
    };
    (*p_capabilities).Vibration = XINPUT_VIBRATION {
        wLeftMotorSpeed: 0xFFFF,
        wRightMotorSpeed: 0xFFFF,
    };

    ERROR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn XInputEnable(enable: i32) {
    XINPUT_ENABLED.store(enable != 0, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "win64" fn XInputGetBatteryInformation(
    dw_user_index: u32,
    _dev_type: u8,
    p_battery_information: *mut XINPUT_BATTERY_INFORMATION,
) -> u32 {
    if dw_user_index >= 4 || p_battery_information.is_null() {
        return ERROR_DEVICE_NOT_CONNECTED;
    }

    let api = match get_sdl_controller_api() {
        Some(a) => a,
        None => return ERROR_DEVICE_NOT_CONNECTED,
    };

    if get_or_open_controller(dw_user_index as usize, api).is_none() {
        return ERROR_DEVICE_NOT_CONNECTED;
    }

    (*p_battery_information).BatteryType = 1; // BATTERY_TYPE_WIRED
    (*p_battery_information).BatteryLevel = 3; // BATTERY_LEVEL_FULL
    ERROR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn XInputGetKeystroke(
    dw_user_index: u32,
    _dw_reserved: u32,
    _p_keystroke: *mut XINPUT_KEYSTROKE,
) -> u32 {
    if dw_user_index >= 4 {
        return ERROR_DEVICE_NOT_CONNECTED;
    }
    ERROR_EMPTY
}

#[no_mangle]
pub unsafe extern "win64" fn XInputGetAudioDeviceIds(
    dw_user_index: u32,
    _p_render_device_id: *mut u16,
    p_render_count: *mut u32,
    _p_capture_device_id: *mut u16,
    p_capture_count: *mut u32,
) -> u32 {
    if dw_user_index >= 4 {
        return ERROR_DEVICE_NOT_CONNECTED;
    }
    if !p_render_count.is_null() {
        *p_render_count = 0;
    }
    if !p_capture_count.is_null() {
        *p_capture_count = 0;
    }
    ERROR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn XInputGetDSoundAudioDeviceGuids(
    dw_user_index: u32,
    p_dsound_render_guid: *mut [u8; 16],
    p_dsound_capture_guid: *mut [u8; 16],
) -> u32 {
    if dw_user_index >= 4 {
        return ERROR_DEVICE_NOT_CONNECTED;
    }
    if !p_dsound_render_guid.is_null() {
        *p_dsound_render_guid = [0u8; 16];
    }
    if !p_dsound_capture_guid.is_null() {
        *p_dsound_capture_guid = [0u8; 16];
    }
    ERROR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn XInputGetStateEx(dw_user_index: u32, p_state_ex: *mut XINPUT_STATE_EX) -> u32 {
    if dw_user_index >= 4 || p_state_ex.is_null() {
        return ERROR_DEVICE_NOT_CONNECTED;
    }
    let mut state = XINPUT_STATE::default();
    let res = XInputGetState(dw_user_index, &mut state);
    if res == ERROR_SUCCESS {
        (*p_state_ex).dwPacketNumber = state.dwPacketNumber;
        (*p_state_ex).Gamepad = state.Gamepad;
        (*p_state_ex).wButtonsEx = (state.Gamepad.wButtons as u32) & 0x0400; // Guide button
    }
    res
}

#[no_mangle]
pub unsafe extern "win64" fn XInputWaitForGuideButton(
    _dw_user_index: u32,
    _dw_flags: u32,
    _p_unknown: *mut c_void,
) -> u32 {
    ERROR_DEVICE_NOT_CONNECTED
}

#[no_mangle]
pub unsafe extern "win64" fn XInputCancelGuideButtonWait(_dw_user_index: u32) -> u32 {
    ERROR_DEVICE_NOT_CONNECTED
}

#[no_mangle]
pub unsafe extern "win64" fn XInputPowerOffController(_dw_user_index: u32) -> u32 {
    ERROR_DEVICE_NOT_CONNECTED
}
