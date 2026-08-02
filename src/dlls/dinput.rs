#![allow(non_snake_case)]

pub unsafe extern "system" fn DirectInput8Create(
    _hinst: u64,
    _version: u32,
    _riid: u64,
    out_di: *mut u64,
    _unk: u64,
) -> i32 {
    if !out_di.is_null() {
        *out_di = 0x00070001;
    }
    println!("[DINPUT8 ENGINE] DirectInput8Create: Created DirectInput8 Keyboard/Mouse/Gamepad Interface = 0x00070001!");
    0 // DI_OK
}
