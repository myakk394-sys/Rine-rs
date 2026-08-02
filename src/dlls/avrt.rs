#![allow(non_snake_case)]

pub unsafe extern "system" fn AvSetMmThreadCharacteristicsW(
    _task_name: u64,
    task_index: *mut u32,
) -> u64 {
    if !task_index.is_null() {
        *task_index = 1;
    }
    0x000A0001
}

pub unsafe extern "system" fn AvSetMmThreadPriority(_handle: u64, _priority: i32) -> i32 {
    1
}
