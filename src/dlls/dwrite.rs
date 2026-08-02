#![allow(non_snake_case)]

pub unsafe extern "system" fn DWriteCreateFactory(
    _factory_type: u32,
    _iid: u64,
    factory_out: *mut u64,
) -> i32 {
    if !factory_out.is_null() {
        *factory_out = 0x00090001;
    }
    println!(
        "[DWRITE FONT ENGINE] DWriteCreateFactory: Created DirectWrite Font Factory = 0x00090001!"
    );
    0 // S_OK
}
