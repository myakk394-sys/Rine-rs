#![allow(non_snake_case)]

#[no_mangle]
pub unsafe extern "win64" fn AvSetMmThreadCharacteristicsW(
    _task_name: *const u16,
    task_index: *mut u32,
) -> usize {
    if !task_index.is_null() {
        *task_index = 1;
    }
    println!("[AVRT] AvSetMmThreadCharacteristicsW -> handle=0x000A0001");
    0x000A0001
}

#[no_mangle]
pub unsafe extern "win64" fn AvSetMmThreadCharacteristicsA(
    _task_name: *const u8,
    task_index: *mut u32,
) -> usize {
    if !task_index.is_null() {
        *task_index = 1;
    }
    println!("[AVRT] AvSetMmThreadCharacteristicsA -> handle=0x000A0001");
    0x000A0001
}

#[no_mangle]
pub unsafe extern "win64" fn AvRevertMmThreadCharacteristics(_handle: usize) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn AvSetMmThreadPriority(_handle: usize, _priority: i32) -> i32 {
    1
}
