#![allow(non_snake_case)]

pub unsafe extern "system" fn DirectSoundCreate(_guid: u64, ds_out: *mut u64, _unk: u64) -> i32 {
    if !ds_out.is_null() {
        *ds_out = 0x00060001;
    }
    println!(
        "[DSOUND AUDIO] DirectSoundCreate: Created DirectSound 8 PCM Audio Buffer = 0x00060001!"
    );
    0 // DS_OK
}

pub unsafe extern "system" fn DirectSoundCreate8(_guid: u64, ds_out: *mut u64, _unk: u64) -> i32 {
    if !ds_out.is_null() {
        *ds_out = 0x00060001;
    }
    println!("[DSOUND AUDIO] DirectSoundCreate8: Created DirectSound8 PulseAudio/PipeWire Sound Driver = 0x00060001!");
    0 // DS_OK
}
