#![allow(non_snake_case)]
pub use crate::dlls::generated::kernel32::*;
use crate::dlls::ntdll;
use crate::mem::MmapRegion;
use crate::sys::WinPageProtection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MEMORY_BASIC_INFORMATION {
    pub BaseAddress: *mut libc::c_void,
    pub AllocationBase: *mut libc::c_void,
    pub AllocationProtect: u32,
    pub PartitionId: u16,
    pub _pad: u16,
    pub RegionSize: usize,
    pub State: u32,
    pub Protect: u32,
    pub Type: u32,
    pub _pad2: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RUNTIME_FUNCTION {
    pub BeginAddress: u32,
    pub EndAddress: u32,
    pub UnwindData: u32,
}

#[derive(Debug, Clone)]
pub struct DynamicUnwindEntry {
    pub base: usize,
    pub end: usize,
    pub table: usize,
    pub count: u32,
    pub max_count: u32,
    pub callback: usize,
    pub context: usize,
}

static DYNAMIC_UNWIND_LIST: Mutex<Option<Vec<Box<DynamicUnwindEntry>>>> = Mutex::new(None);

#[no_mangle]
pub unsafe extern "win64" fn RtlAddGrowableFunctionTable(
    dynamic_table: *mut usize,
    function_table: usize,
    entry_count: u32,
    maximum_entry_count: u32,
    range_base: usize,
    range_end: usize,
) -> u32 {
    println!(
        "[NTDLL/KERNEL32] RtlAddGrowableFunctionTable(table=0x{:X}, count={}, max={}, base=0x{:X}, end=0x{:X})",
        function_table, entry_count, maximum_entry_count, range_base, range_end
    );
    let entry = Box::new(DynamicUnwindEntry {
        base: range_base,
        end: range_end,
        table: function_table,
        count: entry_count,
        max_count: maximum_entry_count,
        callback: 0,
        context: 0,
    });
    let raw = Box::into_raw(entry);
    let mut guard = DYNAMIC_UNWIND_LIST.lock().unwrap();
    let list = guard.get_or_insert_with(Vec::new);
    list.push(Box::from_raw(raw));
    if !dynamic_table.is_null() {
        *dynamic_table = raw as usize;
    }
    0 // STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn RtlGrowFunctionTable(
    dynamic_table: usize,
    new_entry_count: u32,
) {
    let mut guard = DYNAMIC_UNWIND_LIST.lock().unwrap();
    if let Some(list) = guard.as_mut() {
        for entry in list.iter_mut() {
            if entry.as_ref() as *const _ as usize == dynamic_table {
                if new_entry_count > entry.count && new_entry_count <= entry.max_count {
                    entry.count = new_entry_count;
                }
                break;
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn RtlDeleteGrowableFunctionTable(
    dynamic_table: usize,
) {
    println!(
        "[NTDLL/KERNEL32] RtlDeleteGrowableFunctionTable(table=0x{:X})",
        dynamic_table
    );
    let mut guard = DYNAMIC_UNWIND_LIST.lock().unwrap();
    if let Some(list) = guard.as_mut() {
        if let Some(pos) = list.iter().position(|e| e.as_ref() as *const _ as usize == dynamic_table) {
            list.remove(pos);
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn RtlAddFunctionTable(
    function_table: usize,
    entry_count: u32,
    base_address: usize,
) -> u8 {
    println!(
        "[NTDLL/KERNEL32] RtlAddFunctionTable(table=0x{:X}, count={}, base=0x{:X})",
        function_table, entry_count, base_address
    );
    if function_table == 0 || entry_count == 0 {
        return 0;
    }
    let rf_slice = std::slice::from_raw_parts(function_table as *const RUNTIME_FUNCTION, entry_count as usize);
    let max_end = rf_slice.iter().map(|rf| rf.EndAddress).max().unwrap_or(0);
    let entry = Box::new(DynamicUnwindEntry {
        base: base_address,
        end: base_address + max_end as usize,
        table: function_table,
        count: entry_count,
        max_count: entry_count,
        callback: 0,
        context: 0,
    });
    let mut guard = DYNAMIC_UNWIND_LIST.lock().unwrap();
    let list = guard.get_or_insert_with(Vec::new);
    list.push(entry);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn RtlDeleteFunctionTable(
    function_table: usize,
) -> u8 {
    println!(
        "[NTDLL/KERNEL32] RtlDeleteFunctionTable(table=0x{:X})",
        function_table
    );
    let mut guard = DYNAMIC_UNWIND_LIST.lock().unwrap();
    if let Some(list) = guard.as_mut() {
        if let Some(pos) = list.iter().position(|e| e.table == function_table) {
            list.remove(pos);
            return 1;
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn RtlInstallFunctionTableCallback(
    table_identifier: u64,
    base_address: usize,
    length: u32,
    callback: usize,
    context: usize,
    _out_of_process_callback_dll: usize,
) -> u8 {
    println!(
        "[NTDLL/KERNEL32] RtlInstallFunctionTableCallback(id=0x{:X}, base=0x{:X}, len=0x{:X}, cb=0x{:X})",
        table_identifier, base_address, length, callback
    );
    let entry = Box::new(DynamicUnwindEntry {
        base: base_address,
        end: base_address + length as usize,
        table: table_identifier as usize,
        count: 0,
        max_count: 0,
        callback,
        context,
    });
    let mut guard = DYNAMIC_UNWIND_LIST.lock().unwrap();
    let list = guard.get_or_insert_with(Vec::new);
    list.push(entry);
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn RtlLookupFunctionTable(
    control_pc: usize,
    image_base_out: *mut usize,
    length_out: *mut u32,
) -> usize {
    let guard = DYNAMIC_UNWIND_LIST.lock().unwrap();
    if let Some(list) = guard.as_ref() {
        for entry in list.iter() {
            if control_pc >= entry.base && control_pc < entry.end {
                if !image_base_out.is_null() {
                    *image_base_out = entry.base;
                }
                if !length_out.is_null() {
                    *length_out = entry.count;
                }
                return entry.table;
            }
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn RtlLookupFunctionEntry(
    control_pc: usize,
    image_base_out: *mut usize,
    _history_table: usize,
) -> usize {
    let guard = DYNAMIC_UNWIND_LIST.lock().unwrap();
    if let Some(list) = guard.as_ref() {
        for entry in list.iter() {
            if control_pc >= entry.base && control_pc < entry.end {
                if !image_base_out.is_null() {
                    *image_base_out = entry.base;
                }
                if entry.callback != 0 {
                    let cb: unsafe extern "win64" fn(usize, usize) -> usize =
                        std::mem::transmute(entry.callback);
                    return cb(control_pc, entry.context);
                } else if entry.table != 0 && entry.count > 0 {
                    let rva = (control_pc - entry.base) as u32;
                    let rf_slice = std::slice::from_raw_parts(
                        entry.table as *const RUNTIME_FUNCTION,
                        entry.count as usize,
                    );
                    for rf in rf_slice {
                        if rva >= rf.BeginAddress && rva < rf.EndAddress {
                            return rf as *const _ as usize;
                        }
                    }
                    return entry.table;
                }
                return 0;
            }
        }
    }
    0
}

struct ContextAccessor {
    ptr: *mut u8,
}

impl ContextAccessor {
    unsafe fn new(ptr: usize) -> Option<Self> {
        if ptr == 0 {
            None
        } else {
            Some(Self {
                ptr: ptr as *mut u8,
            })
        }
    }

    unsafe fn get_reg(&self, reg: u8) -> u64 {
        if reg <= 15 {
            let offset = 0x78 + (reg as usize) * 8;
            *(self.ptr.add(offset) as *const u64)
        } else {
            0
        }
    }

    unsafe fn set_reg(&mut self, reg: u8, val: u64) {
        if reg <= 15 {
            let offset = 0x78 + (reg as usize) * 8;
            *(self.ptr.add(offset) as *mut u64) = val;
        }
    }

    unsafe fn rsp(&self) -> u64 {
        self.get_reg(4)
    }

    unsafe fn set_rsp(&mut self, val: u64) {
        self.set_reg(4, val);
    }

    unsafe fn rip(&self) -> u64 {
        *(self.ptr.add(0xF8) as *const u64)
    }

    unsafe fn set_rip(&mut self, val: u64) {
        *(self.ptr.add(0xF8) as *mut u64) = val;
    }
}

#[no_mangle]
pub unsafe extern "win64" fn RtlVirtualUnwind(
    _handler_type: u32,
    image_base: usize,
    control_pc: usize,
    function_entry: usize,
    context_record: usize,
    handler_data: *mut usize,
    establisher_frame: *mut usize,
    _context_pointers: usize,
) -> usize {
    let mut ctx = match ContextAccessor::new(context_record) {
        Some(c) => c,
        None => return 0,
    };

    if function_entry == 0 {
        // Leaf function: return address is at top of stack (*Rsp)
        let rsp = ctx.rsp();
        if rsp > 0x10000 {
            let ret_addr = *(rsp as *const u64);
            ctx.set_rip(ret_addr);
            ctx.set_rsp(rsp + 8);
            if !establisher_frame.is_null() {
                *establisher_frame = rsp as usize;
            }
        }
        return 0; // No exception handler
    }

    let rf = &*(function_entry as *const RUNTIME_FUNCTION);
    let mut unwind_info_addr = image_base + rf.UnwindData as usize;

    loop {
        if unwind_info_addr <= 0x10000 {
            return 0;
        }
        let unwind_info = unwind_info_addr as *const u8;
        let header0 = *unwind_info;
        let header1 = *unwind_info.add(1);
        let count_of_codes = *unwind_info.add(2) as usize;
        let header3 = *unwind_info.add(3);

        let flags = (header0 >> 3) & 0x1F;
        let prolog_size = header1 as u32;
        let frame_reg = header3 & 0xF;
        let frame_offset = ((header3 >> 4) & 0xF) as u64;

        let rva = (control_pc.saturating_sub(image_base)) as u32;
        let in_prolog = rva >= rf.BeginAddress && rva < rf.BeginAddress + prolog_size;
        let prolog_offset = if in_prolog {
            rva - rf.BeginAddress
        } else {
            0xFFFFFFFF
        };

        let codes = (unwind_info.add(4)) as *const u16;
        let mut i = 0;
        while i < count_of_codes {
            let code = *codes.add(i);
            let code_offset = (code & 0xFF) as u32;
            let unwind_op = ((code >> 8) & 0xF) as u8;
            let op_info = ((code >> 12) & 0xF) as u8;

            let should_exec = !in_prolog || (code_offset <= prolog_offset);

            match unwind_op {
                0 => {
                    // UWOP_PUSH_NONVOL
                    if should_exec && ctx.rsp() > 0x10000 {
                        let val = *(ctx.rsp() as *const u64);
                        ctx.set_reg(op_info, val);
                        ctx.set_rsp(ctx.rsp() + 8);
                    }
                    i += 1;
                }
                1 => {
                    // UWOP_ALLOC_LARGE
                    if op_info == 0 {
                        let scale = *codes.add(i + 1) as u64;
                        if should_exec {
                            ctx.set_rsp(ctx.rsp() + scale * 8);
                        }
                        i += 2;
                    } else {
                        let low = *codes.add(i + 1) as u32;
                        let high = *codes.add(i + 2) as u32;
                        let size = (low | (high << 16)) as u64;
                        if should_exec {
                            ctx.set_rsp(ctx.rsp() + size);
                        }
                        i += 3;
                    }
                }
                2 => {
                    // UWOP_ALLOC_SMALL
                    let size = (op_info as u64) * 8 + 8;
                    if should_exec {
                        ctx.set_rsp(ctx.rsp() + size);
                    }
                    i += 1;
                }
                3 => {
                    // UWOP_SET_FPREG
                    if should_exec && frame_reg != 0 {
                        let fp = ctx.get_reg(frame_reg);
                        ctx.set_rsp(fp.saturating_sub(frame_offset * 16));
                    }
                    i += 1;
                }
                4 => {
                    // UWOP_SAVE_NONVOL
                    let offset = *codes.add(i + 1) as u64;
                    if should_exec && (ctx.rsp() + offset * 8) > 0x10000 {
                        let val = *((ctx.rsp() + offset * 8) as *const u64);
                        ctx.set_reg(op_info, val);
                    }
                    i += 2;
                }
                5 => {
                    // UWOP_SAVE_NONVOL_FAR
                    let low = *codes.add(i + 1) as u32;
                    let high = *codes.add(i + 2) as u32;
                    let offset = (low | (high << 16)) as u64;
                    if should_exec && (ctx.rsp() + offset) > 0x10000 {
                        let val = *((ctx.rsp() + offset) as *const u64);
                        ctx.set_reg(op_info, val);
                    }
                    i += 3;
                }
                6 => {
                    // UWOP_EPILOG
                    i += 2;
                }
                7 => {
                    // UWOP_SPARE_CODE
                    i += 1;
                }
                8 => {
                    // UWOP_SAVE_XMM128
                    i += 2;
                }
                9 => {
                    // UWOP_SAVE_XMM128_FAR
                    i += 3;
                }
                10 => {
                    // UWOP_PUSH_MACHFRAME
                    if should_exec {
                        let add_size = if op_info == 0 { 40 } else { 48 };
                        ctx.set_rsp(ctx.rsp() + add_size);
                    }
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }

        // Chained UNWIND_INFO: UNW_FLAG_CHAININFO = 0x4
        if (flags & 0x4) != 0 {
            let aligned_codes_count = (count_of_codes + 1) & !1;
            let chained_rf_addr = (codes.add(aligned_codes_count)) as *const RUNTIME_FUNCTION;
            let chained_rf = &*chained_rf_addr;
            unwind_info_addr = image_base + chained_rf.UnwindData as usize;
            continue;
        }

        let frame = if frame_reg != 0 {
            ctx.get_reg(frame_reg)
        } else {
            ctx.rsp()
        };
        if !establisher_frame.is_null() {
            *establisher_frame = frame as usize;
        }

        if ctx.rsp() > 0x10000 {
            let ret_addr = *(ctx.rsp() as *const u64);
            ctx.set_rip(ret_addr);
            ctx.set_rsp(ctx.rsp() + 8);
        }

        // Check for exception/unwind handler: UNW_FLAG_EHANDLER (0x1) or UNW_FLAG_UHANDLER (0x2)
        if (flags & 0x3) != 0 {
            let aligned_codes_count = (count_of_codes + 1) & !1;
            let handler_data_ptr = codes.add(aligned_codes_count) as *const u32;
            let handler_rva = *handler_data_ptr;
            if handler_rva != 0 {
                if !handler_data.is_null() {
                    *handler_data = handler_data_ptr.add(1) as usize;
                }
                return image_base + handler_rva as usize;
            }
        }

        return 0;
    }
}

#[no_mangle]
pub unsafe extern "win64" fn RtlUnwindEx(
    target_frame: usize,
    target_ip: usize,
    exception_record: usize,
    return_value: usize,
    context_record: usize,
    history_table: usize,
) {
    if context_record == 0 {
        return;
    }
    let mut ctx = match ContextAccessor::new(context_record) {
        Some(c) => c,
        None => return,
    };

    let mut iterations = 0;
    while ctx.rip() != 0 && iterations < 256 {
        iterations += 1;
        if target_frame != 0 && ctx.rsp() as usize >= target_frame {
            break;
        }

        let mut image_base: usize = 0;
        let fn_entry = RtlLookupFunctionEntry(ctx.rip() as usize, &mut image_base, history_table);

        let mut handler_data: usize = 0;
        let mut establisher_frame: usize = 0;

        let handler = RtlVirtualUnwind(
            2, // UNW_FLAG_UHANDLER
            image_base,
            ctx.rip() as usize,
            fn_entry,
            context_record,
            &mut handler_data,
            &mut establisher_frame,
            0,
        );

        if handler != 0 {
            let handler_fn: unsafe extern "win64" fn(usize, usize, usize, usize) -> u32 =
                std::mem::transmute(handler);
            handler_fn(
                exception_record,
                establisher_frame,
                context_record,
                handler_data,
            );
        }

        if target_frame != 0 && establisher_frame >= target_frame {
            break;
        }
    }

    if target_ip != 0 {
        ctx.set_rip(target_ip as u64);
    }
    if target_frame != 0 {
        ctx.set_rsp(target_frame as u64);
    }
    ctx.set_reg(0, return_value as u64); // RAX = return_value
}

const FUTEX_WAIT: i32 = 0;
const FUTEX_WAKE: i32 = 1;
const FUTEX_PRIVATE_FLAG: i32 = 128;
const FUTEX_WAIT_PRIVATE: i32 = FUTEX_WAIT | FUTEX_PRIVATE_FLAG;
const FUTEX_WAKE_PRIVATE: i32 = FUTEX_WAKE | FUTEX_PRIVATE_FLAG;

#[allow(dead_code)]
fn futex_wait_raw(futex_addr: *const u32, expected: u32, timeout_ms: u32) -> i32 {
    let ts = if timeout_ms == 0xFFFFFFFF {
        std::ptr::null()
    } else {
        &libc::timespec {
            tv_sec: (timeout_ms / 1000) as libc::time_t,
            tv_nsec: ((timeout_ms % 1000) as i64) * 1_000_000,
        } as *const libc::timespec
    };
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            futex_addr,
            FUTEX_WAIT_PRIVATE,
            expected,
            ts,
            std::ptr::null::<u32>(),
            0u32,
        ) as i32
    }
}

#[allow(dead_code)]
fn futex_wake_raw(futex_addr: *const u32, count: i32) -> i32 {
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            futex_addr,
            FUTEX_WAKE_PRIVATE,
            count,
            std::ptr::null::<libc::timespec>(),
            std::ptr::null::<u32>(),
            0u32,
        ) as i32
    }
}


struct ExactWaiterEntry {
    cond: std::sync::Condvar,
    waiter_count: std::sync::atomic::AtomicU32,
    epoch: std::sync::atomic::AtomicU64,
}

static EXACT_WAITERS_MAP: std::sync::Mutex<
    Option<std::collections::HashMap<usize, std::sync::Arc<ExactWaiterEntry>>>,
> = std::sync::Mutex::new(None);
static EXACT_WAITERS_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[inline(always)]
unsafe fn read_address_value(address: *const std::ffi::c_void, size: usize) -> u64 {
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    match size {
        1 => std::ptr::read_volatile(address as *const u8) as u64,
        2 => {
            if (address as usize) % 2 == 0 {
                std::ptr::read_volatile(address as *const u16) as u64
            } else {
                (address as *const u16).read_unaligned() as u64
            }
        }
        4 => {
            if (address as usize) % 4 == 0 {
                std::ptr::read_volatile(address as *const u32) as u64
            } else {
                (address as *const u32).read_unaligned() as u64
            }
        }
        8 => {
            if (address as usize) % 8 == 0 {
                std::ptr::read_volatile(address as *const u64)
            } else {
                (address as *const u64).read_unaligned()
            }
        }
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn WaitOnAddress(
    address: *const std::ffi::c_void,
    compare_address: *const std::ffi::c_void,
    address_size: usize,
    milliseconds: u32,
) -> i32 {
    const ERROR_TIMEOUT: u32 = 1460;
    const ERROR_INVALID_PARAMETER: u32 = 87;

    if address.is_null() || compare_address.is_null() {
        SetLastError(ERROR_INVALID_PARAMETER);
        return 0;
    }
    if address_size != 1 && address_size != 2 && address_size != 4 && address_size != 8 {
        SetLastError(ERROR_INVALID_PARAMETER);
        return 0;
    }

    // Fast-path: Linux futex for 4-byte aligned addresses
    if address_size == 4 && (address as usize & 3) == 0 {
        let expected_val = read_address_value(compare_address, 4) as u32;

        // Immediate check before sleeping
        if std::ptr::read_volatile(address as *const u32) != expected_val {
            return 1;
        }

        if milliseconds == 0 {
            SetLastError(ERROR_TIMEOUT);
            return 0;
        }

        if milliseconds == 0xFFFFFFFF {
            loop {
                let cur = std::ptr::read_volatile(address as *const u32);
                if cur != expected_val {
                    return 1;
                }
                let ret = libc::syscall(
                    libc::SYS_futex,
                    address,
                    FUTEX_WAIT_PRIVATE,
                    expected_val,
                    std::ptr::null::<libc::timespec>(),
                    std::ptr::null::<u32>(),
                    0u32,
                );
                if ret == 0 {
                    return 1;
                }
                if ret == -1 {
                    let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                    if err == libc::EAGAIN {
                        return 1; // Value changed right before wait
                    }
                    if err == libc::EINTR {
                        continue; // Signal interruption, re-check condition
                    }
                    let cur_after = std::ptr::read_volatile(address as *const u32);
                    return if cur_after != expected_val { 1 } else { 0 };
                }
            }
        } else {
            let start = std::time::Instant::now();
            let timeout_dur = std::time::Duration::from_millis(milliseconds as u64);
            loop {
                let cur = std::ptr::read_volatile(address as *const u32);
                if cur != expected_val {
                    return 1;
                }
                let elapsed = start.elapsed();
                if elapsed >= timeout_dur {
                    SetLastError(ERROR_TIMEOUT);
                    return 0;
                }
                let remaining = timeout_dur - elapsed;
                let ts = libc::timespec {
                    tv_sec: remaining.as_secs() as libc::time_t,
                    tv_nsec: remaining.subsec_nanos() as libc::c_long,
                };
                let ret = libc::syscall(
                    libc::SYS_futex,
                    address,
                    FUTEX_WAIT_PRIVATE,
                    expected_val,
                    &ts as *const libc::timespec,
                    std::ptr::null::<u32>(),
                    0u32,
                );
                if ret == 0 {
                    return 1;
                }
                if ret == -1 {
                    let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                    if err == libc::EAGAIN {
                        return 1; // Value changed right before wait
                    }
                    if err == libc::ETIMEDOUT {
                        let cur_after = std::ptr::read_volatile(address as *const u32);
                        if cur_after != expected_val {
                            return 1;
                        }
                        SetLastError(ERROR_TIMEOUT);
                        return 0;
                    }
                    if err == libc::EINTR {
                        continue; // Interrupted by signal, re-check condition with remaining time
                    }
                    let cur_after = std::ptr::read_volatile(address as *const u32);
                    if cur_after != expected_val {
                        return 1;
                    }
                    SetLastError(ERROR_TIMEOUT);
                    return 0;
                }
            }
        }
    }

    // Path for 1, 2, 8 bytes (or unaligned 4 bytes): exact address condition queue
    let expected_val = read_address_value(compare_address, address_size);
    if read_address_value(address, address_size) != expected_val {
        return 1;
    }

    if milliseconds == 0 {
        SetLastError(ERROR_TIMEOUT);
        return 0;
    }

    let addr_key = address as usize;
    let entry = {
        let mut guard = EXACT_WAITERS_MAP.lock().unwrap();
        let map = guard.get_or_insert_with(std::collections::HashMap::new);
        map.entry(addr_key)
            .or_insert_with(|| {
                std::sync::Arc::new(ExactWaiterEntry {
                    cond: std::sync::Condvar::new(),
                    waiter_count: std::sync::atomic::AtomicU32::new(0),
                    epoch: std::sync::atomic::AtomicU64::new(0),
                })
            })
            .clone()
    };
    entry.waiter_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let mut lock_guard = match EXACT_WAITERS_MUTEX.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };

    let cur_epoch = entry.epoch.load(std::sync::atomic::Ordering::SeqCst);
    let result = if milliseconds == 0xFFFFFFFF {
        while read_address_value(address, address_size) == expected_val
            && entry.epoch.load(std::sync::atomic::Ordering::SeqCst) == cur_epoch
        {
            lock_guard = match entry.cond.wait(lock_guard) {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
        }
        1
    } else {
        let start = std::time::Instant::now();
        let timeout_dur = std::time::Duration::from_millis(milliseconds as u64);
        let mut timed_out = false;
        while read_address_value(address, address_size) == expected_val
            && entry.epoch.load(std::sync::atomic::Ordering::SeqCst) == cur_epoch
        {
            let elapsed = start.elapsed();
            if elapsed >= timeout_dur {
                timed_out = true;
                break;
            }
            let remaining = timeout_dur - elapsed;
            let (new_guard, wait_res) = match entry.cond.wait_timeout(lock_guard, remaining) {
                Ok(pair) => pair,
                Err(p) => p.into_inner(),
            };
            lock_guard = new_guard;
            if wait_res.timed_out()
                && read_address_value(address, address_size) == expected_val
                && entry.epoch.load(std::sync::atomic::Ordering::SeqCst) == cur_epoch
            {
                timed_out = true;
                break;
            }
        }
        if timed_out && read_address_value(address, address_size) == expected_val {
            SetLastError(ERROR_TIMEOUT);
            0
        } else {
            1
        }
    };

    drop(lock_guard);

    if entry.waiter_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
        let mut guard = EXACT_WAITERS_MAP.lock().unwrap();
        if let Some(map) = guard.as_mut() {
            if let Some(e) = map.get(&addr_key) {
                if e.waiter_count.load(std::sync::atomic::Ordering::SeqCst) == 0 {
                    map.remove(&addr_key);
                }
            }
        }
    }

    result
}

#[no_mangle]
pub unsafe extern "win64" fn WakeByAddressSingle(address: *const std::ffi::c_void) {
    if address.is_null() {
        return;
    }
    // 1. Linux futex wake (if 4-byte aligned)
    if (address as usize & 3) == 0 {
        libc::syscall(
            libc::SYS_futex,
            address,
            FUTEX_WAKE_PRIVATE,
            1i32,
        );
    }
    // 2. Exact address waiter wake
    let entry_opt = {
        let guard = EXACT_WAITERS_MAP.lock().unwrap();
        guard.as_ref().and_then(|map| map.get(&(address as usize)).cloned())
    };
    if let Some(entry) = entry_opt {
        entry.epoch.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _lock = EXACT_WAITERS_MUTEX.lock().unwrap();
        entry.cond.notify_one();
    }
}

#[no_mangle]
pub unsafe extern "win64" fn WakeByAddressAll(address: *const std::ffi::c_void) {
    if address.is_null() {
        return;
    }
    // 1. Linux futex wake all (if 4-byte aligned)
    if (address as usize & 3) == 0 {
        libc::syscall(
            libc::SYS_futex,
            address,
            FUTEX_WAKE_PRIVATE,
            i32::MAX,
        );
    }
    // 2. Exact address waiter wake all
    let entry_opt = {
        let guard = EXACT_WAITERS_MAP.lock().unwrap();
        guard.as_ref().and_then(|map| map.get(&(address as usize)).cloned())
    };
    if let Some(entry) = entry_opt {
        entry.epoch.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _lock = EXACT_WAITERS_MUTEX.lock().unwrap();
        entry.cond.notify_all();
    }
}

pub const PROCESS_HEAP_HANDLE: usize = 0x180000000;
pub const HEAP_NO_SERIALIZE: u32 = 0x00000001;
pub const HEAP_GROWABLE: u32 = 0x00000002;
pub const HEAP_GENERATE_EXCEPTIONS: u32 = 0x00000004;
pub const HEAP_ZERO_MEMORY: u32 = 0x00000008;
pub const HEAP_REALLOC_IN_PLACE_ONLY: u32 = 0x00000010;

#[derive(Debug)]
pub struct WinHeap {
    pub handle: usize,
    pub flags: u32,
    pub initial_size: usize,
    pub max_size: usize,
    pub allocations: std::collections::HashMap<usize, usize>,
    pub total_allocated: usize,
}

impl WinHeap {
    pub fn new(handle: usize, flags: u32, initial_size: usize, max_size: usize) -> Self {
        Self {
            handle,
            flags,
            initial_size,
            max_size,
            allocations: std::collections::HashMap::new(),
            total_allocated: 0,
        }
    }

    pub fn alloc(&mut self, flags: u32, size: usize) -> usize {
        let alloc_size = if size == 0 { 16 } else { (size + 15) & !15 };
        if self.max_size > 0 && self.total_allocated + alloc_size > self.max_size {
            return 0;
        }

        let ptr = if (flags & HEAP_ZERO_MEMORY) != 0 || (self.flags & HEAP_ZERO_MEMORY) != 0 {
            unsafe { libc::calloc(1, alloc_size) }
        } else {
            unsafe { libc::malloc(alloc_size) }
        };

        if ptr.is_null() {
            0
        } else {
            let addr = ptr as usize;
            self.allocations.insert(addr, alloc_size);
            self.total_allocated += alloc_size;
            addr
        }
    }

    pub fn free(&mut self, ptr: usize) -> bool {
        if ptr == 0 {
            return true;
        }
        if let Some(size) = self.allocations.remove(&ptr) {
            self.total_allocated = self.total_allocated.saturating_sub(size);
            unsafe {
                libc::free(ptr as *mut libc::c_void);
            }
            true
        } else {
            false
        }
    }

    pub fn realloc(&mut self, flags: u32, ptr: usize, size: usize) -> usize {
        if ptr == 0 {
            return self.alloc(flags, size);
        }
        let old_size = match self.allocations.get(&ptr) {
            Some(&s) => s,
            None => return 0,
        };

        let alloc_size = if size == 0 { 16 } else { (size + 15) & !15 };
        if (flags & HEAP_REALLOC_IN_PLACE_ONLY) != 0 && alloc_size > old_size {
            return 0;
        }

        if self.max_size > 0
            && alloc_size > old_size
            && self.total_allocated + (alloc_size - old_size) > self.max_size
        {
            return 0;
        }

        let new_ptr = unsafe { libc::realloc(ptr as *mut libc::c_void, alloc_size) };
        if new_ptr.is_null() {
            0
        } else {
            let new_addr = new_ptr as usize;
            self.allocations.remove(&ptr);
            self.allocations.insert(new_addr, alloc_size);
            if alloc_size > old_size {
                self.total_allocated += alloc_size - old_size;
                if (flags & HEAP_ZERO_MEMORY) != 0 {
                    unsafe {
                        libc::memset(
                            (new_addr + old_size) as *mut libc::c_void,
                            0,
                            alloc_size - old_size,
                        );
                    }
                }
            } else {
                self.total_allocated = self.total_allocated.saturating_sub(old_size - alloc_size);
            }
            new_addr
        }
    }

    pub fn size_of(&self, ptr: usize) -> Option<usize> {
        self.allocations.get(&ptr).copied()
    }

    pub fn destroy(&mut self) {
        for (&ptr, _) in self.allocations.iter() {
            if ptr > 0x10000 {
                unsafe {
                    libc::free(ptr as *mut libc::c_void);
                }
            }
        }
        self.allocations.clear();
        self.total_allocated = 0;
    }
}

pub struct WinHeapRegistry {
    pub heaps: std::collections::HashMap<usize, WinHeap>,
    pub next_handle: usize,
}

static WIN_HEAP_REGISTRY: std::sync::Mutex<Option<WinHeapRegistry>> = std::sync::Mutex::new(None);

fn with_heap_registry<F, R>(f: F) -> R
where
    F: FnOnce(&mut WinHeapRegistry) -> R,
{
    let mut guard = match WIN_HEAP_REGISTRY.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let registry = guard.get_or_insert_with(|| {
        let mut reg = WinHeapRegistry {
            heaps: std::collections::HashMap::new(),
            next_handle: 0x20000000,
        };
        reg.heaps.insert(
            PROCESS_HEAP_HANDLE,
            WinHeap::new(PROCESS_HEAP_HANDLE, HEAP_GROWABLE, 0x100000, 0),
        );
        reg.heaps.insert(
            0x100000,
            WinHeap::new(0x100000, HEAP_GROWABLE, 0x100000, 0),
        );
        reg
    });
    f(registry)
}

#[no_mangle]
pub unsafe extern "win64" fn GetProcessHeap() -> usize {
    PROCESS_HEAP_HANDLE
}

#[no_mangle]
pub unsafe extern "win64" fn GetProcessHeaps(
    number_of_heaps: u32,
    process_heaps: *mut usize,
) -> u32 {
    with_heap_registry(|reg| {
        let count = reg.heaps.len() as u32;
        if !process_heaps.is_null() && number_of_heaps > 0 {
            let to_copy = (number_of_heaps as usize).min(reg.heaps.len());
            for (idx, &handle) in reg.heaps.keys().take(to_copy).enumerate() {
                *process_heaps.add(idx) = handle;
            }
        }
        count
    })
}

#[no_mangle]
pub unsafe extern "win64" fn HeapCreate(
    flags: u32,
    initial_size: usize,
    max_size: usize,
) -> usize {
    with_heap_registry(|reg| {
        let handle = reg.next_handle;
        reg.next_handle += 0x10000;
        let heap = WinHeap::new(handle, flags, initial_size, max_size);
        reg.heaps.insert(handle, heap);
        handle
    })
}

#[no_mangle]
pub unsafe extern "win64" fn HeapDestroy(h_heap: usize) -> i32 {
    const ERROR_INVALID_PARAMETER: u32 = 87;
    const ERROR_INVALID_HANDLE: u32 = 6;

    if h_heap == 0 || h_heap == PROCESS_HEAP_HANDLE {
        SetLastError(ERROR_INVALID_PARAMETER);
        return 0;
    }

    with_heap_registry(|reg| {
        if let Some(mut heap) = reg.heaps.remove(&h_heap) {
            heap.destroy();
            1
        } else {
            SetLastError(ERROR_INVALID_HANDLE);
            0
        }
    })
}

#[no_mangle]
pub unsafe extern "win64" fn HeapAlloc(h_heap: usize, flags: u32, size: usize) -> usize {
    const ERROR_NOT_ENOUGH_MEMORY: u32 = 8;
    let target_heap = if h_heap == 0 {
        PROCESS_HEAP_HANDLE
    } else {
        h_heap
    };

    let addr = with_heap_registry(|reg| {
        if let Some(heap) = reg.heaps.get_mut(&target_heap) {
            heap.alloc(flags, size)
        } else if let Some(heap) = reg.heaps.get_mut(&PROCESS_HEAP_HANDLE) {
            heap.alloc(flags, size)
        } else {
            0
        }
    });

    if addr == 0 {
        SetLastError(ERROR_NOT_ENOUGH_MEMORY);
    }
    addr
}

#[no_mangle]
pub unsafe extern "win64" fn HeapFree(h_heap: usize, _flags: u32, ptr: usize) -> i32 {
    if ptr == 0 {
        return 1;
    }
    let target_heap = if h_heap == 0 {
        PROCESS_HEAP_HANDLE
    } else {
        h_heap
    };

    with_heap_registry(|reg| {
        // First try target heap
        if let Some(heap) = reg.heaps.get_mut(&target_heap) {
            if heap.free(ptr) {
                return 1;
            }
        }
        // Fallback: check all other heaps in case ptr was allocated on a different heap
        for (h, heap) in reg.heaps.iter_mut() {
            if *h != target_heap && heap.free(ptr) {
                return 1;
            }
        }
        // Never call libc::free or libc::malloc_usable_size on untracked pointers!
        // Untracked pointers may originate from VirtualAlloc, mmap, static PE sections,
        // or guest GC arenas, where calling host free() causes memory corruption / SIGSEGV.
        1
    })
}

#[no_mangle]
pub unsafe extern "win64" fn HeapReAlloc(
    h_heap: usize,
    flags: u32,
    ptr: usize,
    size: usize,
) -> usize {
    const ERROR_INVALID_PARAMETER: u32 = 87;
    if ptr == 0 {
        return HeapAlloc(h_heap, flags, size);
    }
    let target_heap = if h_heap == 0 {
        PROCESS_HEAP_HANDLE
    } else {
        h_heap
    };

    let res = with_heap_registry(|reg| {
        if let Some(heap) = reg.heaps.get_mut(&target_heap) {
            let res = heap.realloc(flags, ptr, size);
            if res != 0 {
                return Some(res);
            }
        }
        for (h, heap) in reg.heaps.iter_mut() {
            if *h != target_heap {
                let res = heap.realloc(flags, ptr, size);
                if res != 0 {
                    return Some(res);
                }
            }
        }
        None
    });

    if let Some(addr) = res {
        addr
    } else {
        SetLastError(ERROR_INVALID_PARAMETER);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn HeapSize(h_heap: usize, _flags: u32, ptr: usize) -> usize {
    const ERROR_INVALID_PARAMETER: u32 = 87;
    if ptr <= 0x10000 {
        SetLastError(ERROR_INVALID_PARAMETER);
        return usize::MAX;
    }
    let target_heap = if h_heap == 0 {
        PROCESS_HEAP_HANDLE
    } else {
        h_heap
    };

    let sz = with_heap_registry(|reg| {
        if let Some(heap) = reg.heaps.get(&target_heap) {
            if let Some(s) = heap.size_of(ptr) {
                return Some(s);
            }
        }
        for (h, heap) in reg.heaps.iter() {
            if *h != target_heap {
                if let Some(s) = heap.size_of(ptr) {
                    return Some(s);
                }
            }
        }
        None
    });

    if let Some(s) = sz {
        s
    } else {
        SetLastError(ERROR_INVALID_PARAMETER);
        usize::MAX
    }
}

#[no_mangle]
pub unsafe extern "win64" fn HeapValidate(h_heap: usize, _flags: u32, ptr: usize) -> i32 {
    let target_heap = if h_heap == 0 {
        PROCESS_HEAP_HANDLE
    } else {
        h_heap
    };

    with_heap_registry(|reg| {
        if ptr == 0 {
            if reg.heaps.contains_key(&target_heap) {
                1
            } else {
                0
            }
        } else {
            if let Some(heap) = reg.heaps.get(&target_heap) {
                if heap.allocations.contains_key(&ptr) {
                    return 1;
                }
            }
            for heap in reg.heaps.values() {
                if heap.allocations.contains_key(&ptr) {
                    return 1;
                }
            }
            1
        }
    })
}

#[no_mangle]
pub unsafe extern "win64" fn HeapCompact(_h_heap: usize, _flags: u32) -> usize {
    0x10000000 // Return largest commit block size (256MB)
}

#[no_mangle]
pub unsafe extern "win64" fn HeapWalk(_h_heap: usize, _lp_entry: *mut libc::c_void) -> i32 {
    const ERROR_NO_MORE_ITEMS: u32 = 259;
    SetLastError(ERROR_NO_MORE_ITEMS);
    0
}

pub unsafe extern "win64" fn LocalAlloc(flags: u32, size: usize) -> usize {
    HeapAlloc(GetProcessHeap(), flags, size)
}

pub unsafe extern "win64" fn LocalFree(ptr: usize) -> usize {
    if ptr > 0 {
        HeapFree(GetProcessHeap(), 0, ptr);
    }
    0 // Win32 LocalFree returns NULL (0) on success
}

pub unsafe extern "win64" fn LocalReAlloc(ptr: usize, size: usize, flags: u32) -> usize {
    HeapReAlloc(GetProcessHeap(), flags, ptr, size)
}

pub unsafe extern "win64" fn LocalSize(ptr: usize) -> usize {
    HeapSize(GetProcessHeap(), 0, ptr)
}

pub unsafe extern "win64" fn LocalLock(ptr: usize) -> usize {
    if ptr == 0 {
        crate::dlls::registry::get_dummy_stub_buffer_ptr() as usize
    } else {
        ptr
    }
}

pub unsafe extern "win64" fn LocalUnlock(_ptr: usize) -> i32 {
    1 // TRUE
}

pub unsafe extern "win64" fn LocalHandle(ptr: usize) -> usize {
    if ptr == 0 {
        0
    } else {
        ptr
    }
}

pub unsafe extern "win64" fn GlobalAlloc(flags: u32, size: usize) -> usize {
    LocalAlloc(flags, size)
}

pub unsafe extern "win64" fn GlobalFree(ptr: usize) -> usize {
    LocalFree(ptr)
}

pub unsafe extern "win64" fn GlobalReAlloc(ptr: usize, size: usize, flags: u32) -> usize {
    LocalReAlloc(ptr, size, flags)
}

pub unsafe extern "win64" fn GlobalSize(ptr: usize) -> usize {
    LocalSize(ptr)
}

pub unsafe extern "win64" fn GlobalLock(ptr: usize) -> usize {
    LocalLock(ptr)
}

pub unsafe extern "win64" fn GlobalUnlock(ptr: usize) -> i32 {
    LocalUnlock(ptr)
}

pub unsafe extern "win64" fn GlobalHandle(ptr: usize) -> usize {
    LocalHandle(ptr)
}

#[derive(Debug, Clone, Copy)]
pub struct VirtualAllocEntry {
    pub base: usize,
    pub size: usize,
    pub protect: u32,
    pub state: u32,
}

static VIRTUAL_ALLOC_REGISTRY: Mutex<Option<Vec<VirtualAllocEntry>>> = Mutex::new(None);

#[inline]
fn win_protect_to_libc_prot(protect: u32) -> libc::c_int {
    match protect & 0xFF {
        0x01 => libc::PROT_NONE,
        0x02 => libc::PROT_READ,
        0x04 | 0x08 => libc::PROT_READ | libc::PROT_WRITE,
        0x10 => libc::PROT_EXEC,
        0x20 => libc::PROT_READ | libc::PROT_EXEC,
        0x40 | 0x80 => libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
        _ => libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
    }
}

pub unsafe extern "win64" fn VirtualAlloc(
    lp_address: usize,
    dw_size: usize,
    fl_allocation_type: u32,
    fl_protect: u32,
) -> usize {
    let alloc_size = if dw_size == 0 {
        4096
    } else {
        (dw_size + 4095) & !4095
    };
    let prot = win_protect_to_libc_prot(fl_protect);
    let state = if (fl_allocation_type & 0x1000) != 0 {
        0x1000 // MEM_COMMIT
    } else {
        0x2000 // MEM_RESERVE
    };

    let res = if lp_address == 0 {
        let mmap_prot = if state == 0x2000 && (fl_protect & 0xFF) == 0x01 {
            libc::PROT_NONE
        } else {
            prot
        };
        let ptr = libc::mmap(
            std::ptr::null_mut(),
            alloc_size,
            mmap_prot,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        if ptr == libc::MAP_FAILED || ptr.is_null() {
            0
        } else {
            let base = ptr as usize;
            let mut guard = VIRTUAL_ALLOC_REGISTRY.lock().unwrap();
            let table = guard.get_or_insert_with(Vec::new);
            table.push(VirtualAllocEntry {
                base,
                size: alloc_size,
                protect: fl_protect,
                state,
            });
            base
        }
    } else {
        let page_base = lp_address & !4095;
        let is_reserve = (fl_allocation_type & 0x2000) != 0;

        if is_reserve {
            // Caller requested to RESERVE at a specific address:
            // Must fail if the address is already in use (MAP_FIXED_NOREPLACE = 0x100000)
            const MAP_FIXED_NOREPLACE: libc::c_int = 0x100000;
            let mmap_prot = if (fl_allocation_type & 0x1000) == 0 && (fl_protect & 0xFF) == 0x01 {
                libc::PROT_NONE
            } else {
                prot
            };
            let ptr = libc::mmap(
                page_base as *mut libc::c_void,
                alloc_size,
                mmap_prot,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | MAP_FIXED_NOREPLACE,
                -1,
                0,
            );
            if ptr != libc::MAP_FAILED && !ptr.is_null() {
                let base = ptr as usize;
                let mut guard = VIRTUAL_ALLOC_REGISTRY.lock().unwrap();
                let table = guard.get_or_insert_with(Vec::new);
                table.push(VirtualAllocEntry {
                    base,
                    size: alloc_size,
                    protect: fl_protect,
                    state,
                });
                base
            } else {
                // Address is already in process address space and caller explicitly requested MEM_RESERVE!
                // Under Windows NT, VirtualAlloc MUST return NULL (0) if the requested range cannot be reserved.
                0
            }
        } else {
            // MEM_COMMIT without MEM_RESERVE: committing existing reserved pages
            let mprot_res = libc::mprotect(page_base as *mut libc::c_void, alloc_size, prot);
            if mprot_res == 0 {
                let mut guard = VIRTUAL_ALLOC_REGISTRY.lock().unwrap();
                let table = guard.get_or_insert_with(Vec::new);
                if let Some(entry) = table.iter_mut().find(|e| page_base >= e.base && (page_base + alloc_size) <= (e.base + e.size)) {
                    entry.state = 0x1000;
                    entry.protect = fl_protect;
                } else {
                    table.push(VirtualAllocEntry {
                        base: page_base,
                        size: alloc_size,
                        protect: fl_protect,
                        state: 0x1000,
                    });
                }
                page_base
            } else {
                0
            }
        }
    };

    println!(
        "[KERNEL32 MEM] VirtualAlloc(lpAddress=0x{:X}, size=0x{:X}, type=0x{:X}, protect=0x{:X}) -> 0x{:X}",
        lp_address, dw_size, fl_allocation_type, fl_protect, res
    );
    res
}

pub unsafe extern "win64" fn VirtualFree(address: usize, size: usize, free_type: u32) -> i32 {
    println!(
        "[KERNEL32 MEM] VirtualFree(address=0x{:X}, size=0x{:X}, free_type=0x{:X})",
        address, size, free_type
    );
    if address == 0 {
        return 0;
    }
    const MEM_DECOMMIT: u32 = 0x4000;
    const MEM_RELEASE: u32 = 0x8000;

    if (free_type & MEM_RELEASE) != 0 || free_type == 0 {
        let removed = {
            let mut guard = VIRTUAL_ALLOC_REGISTRY.lock().unwrap();
            if let Some(table) = guard.as_mut() {
                if let Some(pos) = table.iter().position(|e| e.base == address || (address >= e.base && address < e.base + e.size)) {
                    Some(table.remove(pos))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(entry) = removed {
            if entry.base > 0x10000 {
                libc::munmap(entry.base as *mut libc::c_void, entry.size);
            }
        } else {
            let unmap_size = if size > 0 {
                (size + 4095) & !4095
            } else {
                4096
            };
            if address > 0x10000 {
                libc::munmap(address as *mut libc::c_void, unmap_size);
            }
        }
    } else if (free_type & MEM_DECOMMIT) != 0 {
        let decommit_size = if size > 0 {
            (size + 4095) & !4095
        } else {
            4096
        };
        if address > 0x10000 {
            libc::mprotect(address as *mut libc::c_void, decommit_size, libc::PROT_NONE);
            libc::madvise(address as *mut libc::c_void, decommit_size, libc::MADV_DONTNEED);
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn VirtualProtect(
    lp_address: usize,
    dw_size: usize,
    fl_new_protect: u32,
    lpfl_old_protect: *mut u32,
) -> i32 {
    if !lpfl_old_protect.is_null() {
        *lpfl_old_protect = 0x40; // PAGE_EXECUTE_READWRITE
    }
    if lp_address > 0 && dw_size > 0 {
        let page_start = (lp_address & !4095) as *mut libc::c_void;
        let page_len = ((lp_address + dw_size + 4095) & !4095) - (lp_address & !4095);
        let prot = win_protect_to_libc_prot(fl_new_protect);
        libc::mprotect(page_start, page_len, prot);

        let mut guard = VIRTUAL_ALLOC_REGISTRY.lock().unwrap();
        if let Some(table) = guard.as_mut() {
            for entry in table.iter_mut() {
                if lp_address >= entry.base && lp_address < (entry.base + entry.size) {
                    entry.protect = fl_new_protect;
                    break;
                }
            }
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn VirtualProtectEx(
    _h_process: u64,
    lp_address: usize,
    dw_size: usize,
    fl_new_protect: u32,
    lpfl_old_protect: *mut u32,
) -> i32 {
    VirtualProtect(lp_address, dw_size, fl_new_protect, lpfl_old_protect)
}

#[no_mangle]
pub unsafe extern "win64" fn VirtualQuery(
    lp_address: usize,
    lp_buffer: *mut MEMORY_BASIC_INFORMATION,
    dw_length: usize,
) -> usize {
    if lp_buffer.is_null() || dw_length < std::mem::size_of::<MEMORY_BASIC_INFORMATION>() {
        return 0;
    }

    let page_base = lp_address & !0xFFF;
    let stack_bounds = get_all_thread_stack_bounds();
    for (limit, base) in stack_bounds {
        let guard_limit = limit.saturating_sub(0x1000);
        if (lp_address as u64) >= guard_limit && (lp_address as u64) < limit {
            // Stack Guard page: reports PAGE_GUARD | PAGE_READWRITE so Boehm GC stops scanning
            (*lp_buffer).BaseAddress = page_base as *mut libc::c_void;
            (*lp_buffer).AllocationBase = guard_limit as *mut libc::c_void;
            (*lp_buffer).AllocationProtect = 0x104; // PAGE_GUARD | PAGE_READWRITE
            (*lp_buffer).PartitionId = 0;
            (*lp_buffer)._pad = 0;
            (*lp_buffer).RegionSize = (limit as usize) - page_base;
            (*lp_buffer).State = 0x1000; // MEM_COMMIT
            (*lp_buffer).Protect = 0x104; // PAGE_GUARD | PAGE_READWRITE
            (*lp_buffer).Type = 0x20000; // MEM_PRIVATE
            (*lp_buffer)._pad2 = 0;
            return std::mem::size_of::<MEMORY_BASIC_INFORMATION>();
        } else if (lp_address as u64) >= limit && (lp_address as u64) < base {
            // Committed thread stack
            (*lp_buffer).BaseAddress = page_base as *mut libc::c_void;
            (*lp_buffer).AllocationBase = limit as *mut libc::c_void;
            (*lp_buffer).AllocationProtect = 0x04; // PAGE_READWRITE
            (*lp_buffer).PartitionId = 0;
            (*lp_buffer)._pad = 0;
            (*lp_buffer).RegionSize = (base as usize) - page_base;
            (*lp_buffer).State = 0x1000; // MEM_COMMIT
            (*lp_buffer).Protect = 0x04; // PAGE_READWRITE
            (*lp_buffer).Type = 0x20000; // MEM_PRIVATE
            (*lp_buffer)._pad2 = 0;
            return std::mem::size_of::<MEMORY_BASIC_INFORMATION>();
        }
    }

    {
        let guard = VIRTUAL_ALLOC_REGISTRY.lock().unwrap();
        if let Some(table) = guard.as_ref() {
            for entry in table.iter() {
                if lp_address >= entry.base && lp_address < (entry.base + entry.size) {
                    (*lp_buffer).BaseAddress = page_base as *mut libc::c_void;
                    (*lp_buffer).AllocationBase = entry.base as *mut libc::c_void;
                    (*lp_buffer).AllocationProtect = entry.protect;
                    (*lp_buffer).PartitionId = 0;
                    (*lp_buffer)._pad = 0;
                    (*lp_buffer).RegionSize = (entry.base + entry.size) - page_base;
                    (*lp_buffer).State = entry.state;
                    (*lp_buffer).Protect = if entry.state == 0x2000 { 0 } else { entry.protect };
                    (*lp_buffer).Type = 0x20000; // MEM_PRIVATE
                    (*lp_buffer)._pad2 = 0;
                    return std::mem::size_of::<MEMORY_BASIC_INFORMATION>();
                }
            }
        }
    }

    let (start, end, protect, state, mem_type) = if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
        let mut found = None;
        for line in maps.lines() {
            let mut parts = line.split_whitespace();
            let range = match parts.next() {
                Some(r) => r,
                None => continue,
            };
            let perms = match parts.next() {
                Some(p) => p,
                None => continue,
            };
            let _offset = parts.next();
            let _dev = parts.next();
            let _inode = parts.next();
            let path = parts.next().unwrap_or("");

            let mut range_parts = range.split('-');
            let start = match range_parts.next().and_then(|s| usize::from_str_radix(s, 16).ok()) {
                Some(s) => s,
                None => continue,
            };
            let end = match range_parts.next().and_then(|s| usize::from_str_radix(s, 16).ok()) {
                Some(e) => e,
                None => continue,
            };

            if lp_address >= start && lp_address < end {
                let p_bytes = perms.as_bytes();
                let r = p_bytes.get(0).copied().unwrap_or(b'-') == b'r';
                let w = p_bytes.get(1).copied().unwrap_or(b'-') == b'w';
                let x = p_bytes.get(2).copied().unwrap_or(b'-') == b'x';
                let p = p_bytes.get(3).copied().unwrap_or(b'p') == b'p';

                let protect = match (r, w, x) {
                    (true, true, true) => 0x40,  // PAGE_EXECUTE_READWRITE
                    (true, false, true) => 0x20, // PAGE_EXECUTE_READ
                    (false, false, true) => 0x10, // PAGE_EXECUTE
                    (true, true, false) => 0x04, // PAGE_READWRITE
                    (true, false, false) => 0x02, // PAGE_READONLY
                    _ => 0x01,                   // PAGE_NOACCESS
                };

                let mem_type = if path.ends_with(".exe") || path.ends_with(".dll") || path.ends_with(".so") {
                    0x1000000 // MEM_IMAGE
                } else if !p {
                    0x40000 // MEM_MAPPED
                } else {
                    0x20000 // MEM_PRIVATE
                };

                let state = if protect == 0x01 {
                    0x2000 // MEM_RESERVE
                } else {
                    0x1000 // MEM_COMMIT
                };

                found = Some((start, end, protect, state, mem_type));
                break;
            }
        }
        found.unwrap_or_else(|| {
            let page_base = lp_address & !0xFFF;
            (0, page_base + 0x1000, 0x01 /* PAGE_NOACCESS */, 0x10000 /* MEM_FREE */, 0)
        })
    } else {
        let page_base = lp_address & !0xFFF;
        (page_base, page_base + 0x1000, 0x04 /* PAGE_READWRITE */, 0x1000 /* MEM_COMMIT */, 0x20000 /* MEM_PRIVATE */)
    };

    let page_base = lp_address & !0xFFF;
    (*lp_buffer).BaseAddress = page_base as *mut libc::c_void;
    (*lp_buffer).AllocationBase = if state == 0x10000 {
        std::ptr::null_mut()
    } else {
        start as *mut libc::c_void
    };
    (*lp_buffer).AllocationProtect = if state == 0x10000 {
        0
    } else {
        protect
    };
    (*lp_buffer).PartitionId = 0;
    (*lp_buffer)._pad = 0;
    (*lp_buffer).RegionSize = end.saturating_sub(page_base);
    (*lp_buffer).State = state;
    (*lp_buffer).Protect = if state == 0x2000 {
        0
    } else {
        protect
    };
    (*lp_buffer).Type = mem_type;
    (*lp_buffer)._pad2 = 0;

    std::mem::size_of::<MEMORY_BASIC_INFORMATION>()
}

#[no_mangle]
pub unsafe extern "win64" fn VirtualQueryEx(
    _h_process: u64,
    lp_address: usize,
    lp_buffer: *mut MEMORY_BASIC_INFORMATION,
    dw_length: usize,
) -> usize {
    VirtualQuery(lp_address, lp_buffer, dw_length)
}

#[no_mangle]
pub unsafe extern "win64" fn K32EnumProcessModules(
    _h_process: u64,
    lph_module: *mut u64,
    cb: u32,
    lpcb_needed: *mut u32,
) -> i32 {
    let mut modules: Vec<u64> = Vec::new();
    let main_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    if main_base != 0 {
        modules.push(main_base);
    }
    if let Ok(guard) = MODULE_TABLE.lock() {
        if let Some(table) = guard.as_ref() {
            for m in table {
                let b = m.base as u64;
                if b != 0 && !modules.contains(&b) {
                    modules.push(b);
                }
            }
        }
    }
    if let Ok(guard_pe) = crate::pe::imports::LOADED_MODULES.lock() {
        if let Some(vec) = guard_pe.as_ref() {
            for (_, base, _) in vec {
                let b = *base as u64;
                if b != 0 && !modules.contains(&b) {
                    modules.push(b);
                }
            }
        }
    }
    let total_bytes = (modules.len() * std::mem::size_of::<u64>()) as u32;
    if !lpcb_needed.is_null() {
        *lpcb_needed = total_bytes;
    }
    if !lph_module.is_null() && cb > 0 {
        let max_count = (cb as usize) / std::mem::size_of::<u64>();
        let copy_count = modules.len().min(max_count);
        for i in 0..copy_count {
            *lph_module.add(i) = modules[i];
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn K32EnumProcessModulesEx(
    h_process: u64,
    lph_module: *mut u64,
    cb: u32,
    lpcb_needed: *mut u32,
    _dw_filter_flag: u32,
) -> i32 {
    K32EnumProcessModules(h_process, lph_module, cb, lpcb_needed)
}

#[no_mangle]
pub unsafe extern "win64" fn EnumProcessModules(
    h_process: u64,
    lph_module: *mut u64,
    cb: u32,
    lpcb_needed: *mut u32,
) -> i32 {
    K32EnumProcessModules(h_process, lph_module, cb, lpcb_needed)
}

#[no_mangle]
pub unsafe extern "win64" fn EnumProcessModulesEx(
    h_process: u64,
    lph_module: *mut u64,
    cb: u32,
    lpcb_needed: *mut u32,
    dw_filter_flag: u32,
) -> i32 {
    K32EnumProcessModulesEx(h_process, lph_module, cb, lpcb_needed, dw_filter_flag)
}

#[no_mangle]
pub unsafe extern "win64" fn K32GetModuleFileNameExW(
    _h_process: u64,
    h_module: u64,
    lp_filename: *mut u16,
    n_size: u32,
) -> u32 {
    if lp_filename.is_null() || n_size == 0 {
        return 0;
    }
    let path_str = if h_module == 0 || h_module == CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) {
        get_target_exe_path()
    } else if let Some(name) = ModuleRegistry::get_module_name(h_module as usize) {
        format!("C:\\Windows\\System32\\{}", name)
    } else {
        format!("C:\\Windows\\System32\\module_{:X}.dll", h_module)
    };
    let utf16: Vec<u16> = path_str.encode_utf16().collect();
    let copy_len = utf16.len().min(n_size as usize - 1);
    for i in 0..copy_len {
        *lp_filename.add(i) = utf16[i];
    }
    *lp_filename.add(copy_len) = 0;
    copy_len as u32
}

#[no_mangle]
pub unsafe extern "win64" fn K32GetModuleFileNameExA(
    _h_process: u64,
    h_module: u64,
    lp_filename: *mut libc::c_char,
    n_size: u32,
) -> u32 {
    if lp_filename.is_null() || n_size == 0 {
        return 0;
    }
    let path_str = if h_module == 0 || h_module == CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) {
        get_target_exe_path()
    } else if let Some(name) = ModuleRegistry::get_module_name(h_module as usize) {
        format!("C:\\Windows\\System32\\{}", name)
    } else {
        format!("C:\\Windows\\System32\\module_{:X}.dll", h_module)
    };
    let bytes = path_str.as_bytes();
    let copy_len = bytes.len().min(n_size as usize - 1);
    for i in 0..copy_len {
        *lp_filename.add(i) = bytes[i] as libc::c_char;
    }
    *lp_filename.add(copy_len) = 0;
    copy_len as u32
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleFileNameExW(
    h_process: u64,
    h_module: u64,
    lp_filename: *mut u16,
    n_size: u32,
) -> u32 {
    K32GetModuleFileNameExW(h_process, h_module, lp_filename, n_size)
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleFileNameExA(
    h_process: u64,
    h_module: u64,
    lp_filename: *mut libc::c_char,
    n_size: u32,
) -> u32 {
    K32GetModuleFileNameExA(h_process, h_module, lp_filename, n_size)
}

#[no_mangle]
pub unsafe extern "win64" fn K32GetModuleBaseNameW(
    _h_process: u64,
    h_module: u64,
    lp_base_name: *mut u16,
    n_size: u32,
) -> u32 {
    if lp_base_name.is_null() || n_size == 0 {
        return 0;
    }
    let name_str = if h_module == 0 || h_module == CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) {
        let full = get_target_exe_path();
        std::path::Path::new(&full.replace('\\', "/"))
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "game.exe".to_string())
    } else if let Some(name) = ModuleRegistry::get_module_name(h_module as usize) {
        name
    } else {
        format!("module_{:X}.dll", h_module)
    };
    let utf16: Vec<u16> = name_str.encode_utf16().collect();
    let copy_len = utf16.len().min(n_size as usize - 1);
    for i in 0..copy_len {
        *lp_base_name.add(i) = utf16[i];
    }
    *lp_base_name.add(copy_len) = 0;
    copy_len as u32
}

#[no_mangle]
pub unsafe extern "win64" fn K32GetModuleBaseNameA(
    _h_process: u64,
    h_module: u64,
    lp_base_name: *mut libc::c_char,
    n_size: u32,
) -> u32 {
    if lp_base_name.is_null() || n_size == 0 {
        return 0;
    }
    let name_str = if h_module == 0 || h_module == CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) {
        let full = get_target_exe_path();
        std::path::Path::new(&full.replace('\\', "/"))
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "game.exe".to_string())
    } else if let Some(name) = ModuleRegistry::get_module_name(h_module as usize) {
        name
    } else {
        format!("module_{:X}.dll", h_module)
    };
    let bytes = name_str.as_bytes();
    let copy_len = bytes.len().min(n_size as usize - 1);
    for i in 0..copy_len {
        *lp_base_name.add(i) = bytes[i] as libc::c_char;
    }
    *lp_base_name.add(copy_len) = 0;
    copy_len as u32
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleBaseNameW(
    h_process: u64,
    h_module: u64,
    lp_base_name: *mut u16,
    n_size: u32,
) -> u32 {
    K32GetModuleBaseNameW(h_process, h_module, lp_base_name, n_size)
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleBaseNameA(
    h_process: u64,
    h_module: u64,
    lp_base_name: *mut libc::c_char,
    n_size: u32,
) -> u32 {
    K32GetModuleBaseNameA(h_process, h_module, lp_base_name, n_size)
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MODULEINFO {
    pub lp_base_of_dll: u64,
    pub size_of_image: u32,
    pub entry_point: u64,
}

#[no_mangle]
pub unsafe extern "win64" fn K32GetModuleInformation(
    _h_process: u64,
    h_module: u64,
    lpmodinfo: *mut MODULEINFO,
    cb: u32,
) -> i32 {
    if lpmodinfo.is_null() || (cb as usize) < std::mem::size_of::<MODULEINFO>() {
        return 0;
    }
    let mut size_of_image = 0x1000000u32;
    let entry_point = h_module + 0x1000;
    if let Ok(guard_pe) = crate::pe::imports::LOADED_MODULES.lock() {
        if let Some(vec) = guard_pe.as_ref() {
            for (_, base, size) in vec {
                if *base as u64 == h_module {
                    size_of_image = *size as u32;
                    break;
                }
            }
        }
    }
    *lpmodinfo = MODULEINFO {
        lp_base_of_dll: h_module,
        size_of_image,
        entry_point,
    };
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleInformation(
    h_process: u64,
    h_module: u64,
    lpmodinfo: *mut MODULEINFO,
    cb: u32,
) -> i32 {
    K32GetModuleInformation(h_process, h_module, lpmodinfo, cb)
}

#[no_mangle]
pub unsafe extern "win64" fn K32EnumProcesses(
    lpid_process: *mut u32,
    cb: u32,
    lpcb_needed: *mut u32,
) -> i32 {
    let pids = [std::process::id() as u32];
    let total_bytes = (pids.len() * std::mem::size_of::<u32>()) as u32;
    if !lpcb_needed.is_null() {
        *lpcb_needed = total_bytes;
    }
    if !lpid_process.is_null() && cb >= 4 {
        *lpid_process = pids[0];
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn EnumProcesses(
    lpid_process: *mut u32,
    cb: u32,
    lpcb_needed: *mut u32,
) -> i32 {
    K32EnumProcesses(lpid_process, cb, lpcb_needed)
}

#[no_mangle]
pub unsafe extern "win64" fn GetTickCount64() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    ((ts.tv_sec as u64) * 1000) + ((ts.tv_nsec as u64) / 1_000_000)
}

#[repr(C)]
pub struct PROCESSOR_NUMBER {
    pub group: u16,
    pub number: u8,
    pub reserved: u8,
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentProcessorNumberEx(proc_number: *mut PROCESSOR_NUMBER) {
    if !proc_number.is_null() {
        *proc_number = PROCESSOR_NUMBER {
            group: 0,
            number: 0,
            reserved: 0,
        };
    }
}

#[no_mangle]
pub unsafe extern "win64" fn FlushProcessWriteBuffers() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "win64" fn GetProcessTimes(
    _h_process: u64,
    lp_creation_time: *mut u64,
    lp_exit_time: *mut u64,
    lp_kernel_time: *mut u64,
    lp_user_time: *mut u64,
) -> i32 {
    let mut now_ft = 0u64;
    GetSystemTimeAsFileTime(&mut now_ft);
    if !lp_creation_time.is_null() { *lp_creation_time = now_ft; }
    if !lp_exit_time.is_null() { *lp_exit_time = 0; }
    if !lp_kernel_time.is_null() { *lp_kernel_time = 10_000; }
    if !lp_user_time.is_null() { *lp_user_time = 10_000; }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemTimes(
    lp_idle_time: *mut u64,
    lp_kernel_time: *mut u64,
    lp_user_time: *mut u64,
) -> i32 {
    let mut now_ft = 0u64;
    GetSystemTimeAsFileTime(&mut now_ft);
    if !lp_idle_time.is_null() { *lp_idle_time = now_ft / 2; }
    if !lp_kernel_time.is_null() { *lp_kernel_time = now_ft / 4; }
    if !lp_user_time.is_null() { *lp_user_time = now_ft / 4; }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetProcessWorkingSetSize(
    _h_process: u64,
    _dw_min: usize,
    _dw_max: usize,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetProcessWorkingSetSize(
    _h_process: u64,
    lp_min: *mut usize,
    lp_max: *mut usize,
) -> i32 {
    if !lp_min.is_null() { *lp_min = 64 * 1024 * 1024; }
    if !lp_max.is_null() { *lp_max = 256 * 1024 * 1024; }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CancelIoEx(_h_file: u64, _lp_overlapped: *mut ()) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CancelSynchronousIo(_h_thread: u64) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn QueueUserAPC(
    _pfn_apc: *const (),
    _h_thread: u64,
    _dw_data: usize,
) -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SignalObjectAndWait(
    h_object_to_signal: u64,
    h_object_to_wait_on: u64,
    dw_milliseconds: u32,
    _b_alertable: i32,
) -> u32 {
    SetEvent(h_object_to_signal);
    WaitForSingleObject(h_object_to_wait_on, dw_milliseconds)
}

#[no_mangle]
pub unsafe extern "win64" fn LockFile(
    _h_file: u64,
    _dw_file_offset_low: u32,
    _dw_file_offset_high: u32,
    _n_number_of_bytes_to_lock_low: u32,
    _n_number_of_bytes_to_lock_high: u32,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn UnlockFile(
    _h_file: u64,
    _dw_file_offset_low: u32,
    _dw_file_offset_high: u32,
    _n_number_of_bytes_to_unlock_low: u32,
    _n_number_of_bytes_to_unlock_high: u32,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetLogicalDriveStringsW(
    n_buffer_length: u32,
    lp_buffer: *mut u16,
) -> u32 {
    let drives = "C:\\\0D:\\\0\0";
    let utf16: Vec<u16> = drives.encode_utf16().collect();
    if lp_buffer.is_null() || n_buffer_length < utf16.len() as u32 {
        return utf16.len() as u32;
    }
    for i in 0..utf16.len() {
        *lp_buffer.add(i) = utf16[i];
    }
    (utf16.len() - 1) as u32
}

#[no_mangle]
pub unsafe extern "win64" fn GetVolumeInformationW(
    _lp_root_path_name: *const u16,
    lp_volume_name_buffer: *mut u16,
    n_volume_name_size: u32,
    lp_volume_serial_number: *mut u32,
    lp_maximum_component_length: *mut u32,
    lp_file_system_flags: *mut u32,
    lp_file_system_name_buffer: *mut u16,
    n_file_system_name_size: u32,
) -> i32 {
    if !lp_volume_name_buffer.is_null() && n_volume_name_size > 0 {
        let name = "RINE_DRIVE\0";
        let u16_name: Vec<u16> = name.encode_utf16().collect();
        let len = u16_name.len().min(n_volume_name_size as usize);
        for i in 0..len {
            *lp_volume_name_buffer.add(i) = u16_name[i];
        }
    }
    if !lp_volume_serial_number.is_null() {
        *lp_volume_serial_number = 0x12345678;
    }
    if !lp_maximum_component_length.is_null() {
        *lp_maximum_component_length = 255;
    }
    if !lp_file_system_flags.is_null() {
        *lp_file_system_flags = 0x00000002 | 0x00000004 | 0x00000008;
    }
    if !lp_file_system_name_buffer.is_null() && n_file_system_name_size > 0 {
        let fs = "NTFS\0";
        let u16_fs: Vec<u16> = fs.encode_utf16().collect();
        let len = u16_fs.len().min(n_file_system_name_size as usize);
        for i in 0..len {
            *lp_file_system_name_buffer.add(i) = u16_fs[i];
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn VerLanguageNameW(w_lang: u32, sz_lang: *mut u16, cch_lang: u32) -> u32 {
    if sz_lang.is_null() || cch_lang == 0 {
        return 0;
    }
    let name = match w_lang & 0xFFFF {
        0x0409 => "English (United States)\0",
        0x0419 => "Russian\0",
        _ => "English\0",
    };
    let utf16: Vec<u16> = name.encode_utf16().collect();
    let copy_len = utf16.len().min(cch_lang as usize);
    for i in 0..copy_len {
        *sz_lang.add(i) = utf16[i];
    }
    copy_len.saturating_sub(1) as u32
}

#[no_mangle]
pub unsafe extern "win64" fn FileTimeToSystemTime(
    lp_file_time: *const u64,
    lp_system_time: *mut SystemTime,
) -> i32 {
    if lp_file_time.is_null() || lp_system_time.is_null() {
        return 0;
    }
    let ft = *lp_file_time;
    let secs = (ft.saturating_sub(116444736000000000) / 10000000) as libc::time_t;
    let millis = ((ft % 10000000) / 10000) as u16;
    let mut tm = std::mem::zeroed::<libc::tm>();
    if libc::gmtime_r(&secs, &mut tm).is_null() {
        return 0;
    }
    *lp_system_time = SystemTime {
        w_year: (tm.tm_year + 1900) as u16,
        w_month: (tm.tm_mon + 1) as u16,
        w_day_of_week: tm.tm_wday as u16,
        w_day: tm.tm_mday as u16,
        w_hour: tm.tm_hour as u16,
        w_minute: tm.tm_min as u16,
        w_second: tm.tm_sec as u16,
        w_milliseconds: millis,
    };
    1
}

#[no_mangle]
pub unsafe extern "win64" fn FileTimeToLocalFileTime(
    lp_file_time: *const u64,
    lp_local_file_time: *mut u64,
) -> i32 {
    if lp_file_time.is_null() || lp_local_file_time.is_null() {
        return 0;
    }
    *lp_local_file_time = *lp_file_time;
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn LocalFileTimeToFileTime(
    lp_local_file_time: *const u64,
    lp_file_time: *mut u64,
) -> i32 {
    if lp_local_file_time.is_null() || lp_file_time.is_null() {
        return 0;
    }
    *lp_file_time = *lp_local_file_time;
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn FileTimeToDosDateTime(
    lp_file_time: *const u64,
    lp_fat_date: *mut u16,
    lp_fat_time: *mut u16,
) -> i32 {
    if lp_file_time.is_null() || lp_fat_date.is_null() || lp_fat_time.is_null() {
        return 0;
    }
    let ft = *lp_file_time;
    let secs = (ft.saturating_sub(116444736000000000) / 10000000) as libc::time_t;
    let mut tm = std::mem::zeroed::<libc::tm>();
    if libc::gmtime_r(&secs, &mut tm).is_null() {
        *lp_fat_date = 0;
        *lp_fat_time = 0;
        return 1;
    }
    let year = ((tm.tm_year + 1900).max(1980) - 1980) as u16;
    let month = (tm.tm_mon + 1) as u16;
    let day = tm.tm_mday as u16;
    *lp_fat_date = (year << 9) | (month << 5) | day;

    let hour = tm.tm_hour as u16;
    let min = tm.tm_min as u16;
    let sec = (tm.tm_sec / 2) as u16;
    *lp_fat_time = (hour << 11) | (min << 5) | sec;
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn DosDateTimeToFileTime(
    w_fat_date: u16,
    w_fat_time: u16,
    lp_file_time: *mut u64,
) -> i32 {
    if lp_file_time.is_null() {
        return 0;
    }
    let year = ((w_fat_date >> 9) & 0x7F) as i32 + 1980;
    let month = ((w_fat_date >> 5) & 0x0F) as i32;
    let day = (w_fat_date & 0x1F) as i32;
    let hour = ((w_fat_time >> 11) & 0x1F) as i32;
    let min = ((w_fat_time >> 5) & 0x3F) as i32;
    let sec = ((w_fat_time & 0x1F) * 2) as i32;

    let mut tm = std::mem::zeroed::<libc::tm>();
    tm.tm_year = year - 1900;
    tm.tm_mon = month - 1;
    tm.tm_mday = day;
    tm.tm_hour = hour;
    tm.tm_min = min;
    tm.tm_sec = sec;
    let secs = libc::timegm(&mut tm);
    if secs < 0 {
        *lp_file_time = 116444736000000000;
    } else {
        *lp_file_time = (secs as u64 * 10000000) + 116444736000000000;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn TzSpecificLocalTimeToSystemTime(
    _lp_time_zone_information: *const (),
    lp_local_time: *const SystemTime,
    lp_universal_time: *mut SystemTime,
) -> i32 {
    if lp_local_time.is_null() || lp_universal_time.is_null() {
        return 0;
    }
    *lp_universal_time = *lp_local_time;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SystemTimeToTzSpecificLocalTime(
    _lp_time_zone_information: *const (),
    lp_universal_time: *const SystemTime,
    lp_local_time: *mut SystemTime,
) -> i32 {
    if lp_universal_time.is_null() || lp_local_time.is_null() {
        return 0;
    }
    *lp_local_time = *lp_universal_time;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetDateFormatW(
    _locale: u32,
    _dw_flags: u32,
    _lp_date: *const SystemTime,
    _lp_format: *const u16,
    lp_date_str: *mut u16,
    cch_date: i32,
) -> i32 {
    let dummy = "2026-11-14\0";
    let utf16: Vec<u16> = dummy.encode_utf16().collect();
    if lp_date_str.is_null() || cch_date == 0 {
        return utf16.len() as i32;
    }
    let copy_len = utf16.len().min(cch_date as usize);
    for i in 0..copy_len {
        *lp_date_str.add(i) = utf16[i];
    }
    copy_len as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetTimeFormatW(
    _locale: u32,
    _dw_flags: u32,
    _lp_time: *const SystemTime,
    _lp_format: *const u16,
    lp_time_str: *mut u16,
    cch_time: i32,
) -> i32 {
    let dummy = "12:00:00\0";
    let utf16: Vec<u16> = dummy.encode_utf16().collect();
    if lp_time_str.is_null() || cch_time == 0 {
        return utf16.len() as i32;
    }
    let copy_len = utf16.len().min(cch_time as usize);
    for i in 0..copy_len {
        *lp_time_str.add(i) = utf16[i];
    }
    copy_len as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetFullPathNameW(
    lp_file_name: *const u16,
    n_buffer_length: u32,
    lp_buffer: *mut u16,
    lp_file_part: *mut *mut u16,
) -> u32 {
    if lp_file_name.is_null() {
        return 0;
    }
    let name_str = read_utf16_ptr(lp_file_name).unwrap_or_default();
    let full = if name_str.contains(':') || name_str.starts_with('\\') {
        name_str
    } else {
        let cwd = get_process_current_directory();
        if cwd.ends_with('\\') {
            format!("{}{}", cwd, name_str)
        } else {
            format!("{}\\{}", cwd, name_str)
        }
    };
    let utf16: Vec<u16> = full.encode_utf16().collect();
    if lp_buffer.is_null() || n_buffer_length < (utf16.len() + 1) as u32 {
        return (utf16.len() + 1) as u32;
    }
    let mut last_slash = 0;
    for i in 0..utf16.len() {
        *lp_buffer.add(i) = utf16[i];
        if utf16[i] == b'\\' as u16 || utf16[i] == b'/' as u16 {
            last_slash = i + 1;
        }
    }
    *lp_buffer.add(utf16.len()) = 0;
    if !lp_file_part.is_null() {
        *lp_file_part = if last_slash > 0 && last_slash < utf16.len() {
            lp_buffer.add(last_slash)
        } else {
            std::ptr::null_mut()
        };
    }
    utf16.len() as u32
}

#[no_mangle]
pub unsafe extern "win64" fn GetFullPathNameA(
    lp_file_name: *const libc::c_char,
    n_buffer_length: u32,
    lp_buffer: *mut libc::c_char,
    lp_file_part: *mut *mut libc::c_char,
) -> u32 {
    if lp_file_name.is_null() {
        return 0;
    }
    let name_str = std::ffi::CStr::from_ptr(lp_file_name).to_string_lossy();
    let full = if name_str.contains(':') || name_str.starts_with('\\') {
        name_str.to_string()
    } else {
        let cwd = get_process_current_directory();
        if cwd.ends_with('\\') {
            format!("{}{}", cwd, name_str)
        } else {
            format!("{}\\{}", cwd, name_str)
        }
    };
    let bytes = full.as_bytes();
    if (n_buffer_length as usize) <= bytes.len() {
        return (bytes.len() + 1) as u32;
    }
    if !lp_buffer.is_null() {
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const libc::c_char, lp_buffer, bytes.len());
        *lp_buffer.add(bytes.len()) = 0;
        if !lp_file_part.is_null() {
            if let Some(pos) = full.rfind('\\') {
                *lp_file_part = lp_buffer.add(pos + 1);
            } else {
                *lp_file_part = lp_buffer;
            }
        }
    }
    bytes.len() as u32
}

#[no_mangle]
pub unsafe extern "win64" fn PeekNamedPipe(
    _h_named_pipe: u64,
    _lp_buffer: *mut (),
    _n_buffer_size: u32,
    lp_bytes_read: *mut u32,
    lp_total_bytes_avail: *mut u32,
    lp_bytes_left_this_message: *mut u32,
) -> i32 {
    if !lp_bytes_read.is_null() { *lp_bytes_read = 0; }
    if !lp_total_bytes_avail.is_null() { *lp_total_bytes_avail = 0; }
    if !lp_bytes_left_this_message.is_null() { *lp_bytes_left_this_message = 0; }
    1
}

static VECTORED_EXCEPTION_HANDLERS: Mutex<Option<Vec<usize>>> = Mutex::new(None);

pub fn get_vectored_exception_handlers() -> Vec<usize> {
    let guard = VECTORED_EXCEPTION_HANDLERS.lock().unwrap();
    guard.as_ref().cloned().unwrap_or_default()
}

#[no_mangle]
pub unsafe extern "win64" fn AddVectoredExceptionHandler(
    first_handler: u32,
    vectored_handler: *const (),
) -> *mut () {
    let handler_addr = vectored_handler as usize;
    if handler_addr > 0x10000 {
        let mut guard = VECTORED_EXCEPTION_HANDLERS.lock().unwrap();
        let list = guard.get_or_insert_with(Vec::new);
        if first_handler != 0 {
            list.insert(0, handler_addr);
        } else {
            list.push(handler_addr);
        }
        println!(
            "[KERNEL32 VEH] AddVectoredExceptionHandler(first={}, handler=0x{:X}) -> registered",
            first_handler, handler_addr
        );
    }
    vectored_handler as *mut ()
}

#[no_mangle]
pub unsafe extern "win64" fn RemoveVectoredExceptionHandler(handle: *mut ()) -> u32 {
    let handler_addr = handle as usize;
    let mut guard = VECTORED_EXCEPTION_HANDLERS.lock().unwrap();
    if let Some(list) = guard.as_mut() {
        if let Some(pos) = list.iter().position(|&h| h == handler_addr) {
            list.remove(pos);
            return 1;
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn SetThreadStackGuarantee(stack_size_in_bytes: *mut u32) -> i32 {
    if !stack_size_in_bytes.is_null() {
        *stack_size_in_bytes = 64 * 1024;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn EnumSystemLocalesW(
    lp_locale_enum_proc: unsafe extern "win64" fn(*const u16) -> i32,
    _dw_flags: u32,
) -> i32 {
    let dummy = "00000409\0";
    let utf16: Vec<u16> = dummy.encode_utf16().collect();
    let _ = lp_locale_enum_proc(utf16.as_ptr());
    1
}

#[no_mangle]
pub unsafe extern "win64" fn EnumSystemLocalesA(
    lp_locale_enum_proc: unsafe extern "win64" fn(*const libc::c_char) -> i32,
    _dw_flags: u32,
) -> i32 {
    let dummy = b"00000409\0";
    let _ = lp_locale_enum_proc(dummy.as_ptr() as *const libc::c_char);
    1
}

#[repr(C)]
pub struct CPINFOEXW {
    pub max_char_size: u32,
    pub default_char: [u8; 2],
    pub lead_byte: [u8; 12],
    pub unicode_default_char: u16,
    pub codepage: u32,
    pub code_page_name: [u16; 260],
}

#[no_mangle]
pub unsafe extern "win64" fn GetCPInfoExW(
    code_page: u32,
    _dw_flags: u32,
    lp_cp_info_ex: *mut CPINFOEXW,
) -> i32 {
    if lp_cp_info_ex.is_null() {
        return 0;
    }
    let mut cp = CPINFOEXW {
        max_char_size: 2,
        default_char: [b'?', 0],
        lead_byte: [0; 12],
        unicode_default_char: '?' as u16,
        codepage: if code_page == 0 { 65001 } else { code_page },
        code_page_name: [0; 260],
    };
    let name = "UTF-8\0";
    let u16_name: Vec<u16> = name.encode_utf16().collect();
    for i in 0..u16_name.len() {
        cp.code_page_name[i] = u16_name[i];
    }
    *lp_cp_info_ex = cp;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetNumberOfConsoleInputEvents(_h_console: u64, lpc_events: *mut u32) -> i32 {
    if !lpc_events.is_null() {
        *lpc_events = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ReadConsoleInputW(
    _h_console: u64,
    _lp_buffer: *mut (),
    _n_length: u32,
    lp_number_of_events_read: *mut u32,
) -> i32 {
    if !lp_number_of_events_read.is_null() {
        *lp_number_of_events_read = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn PeekConsoleInputA(
    _h_console: u64,
    _lp_buffer: *mut (),
    _n_length: u32,
    lp_number_of_events_read: *mut u32,
) -> i32 {
    if !lp_number_of_events_read.is_null() {
        *lp_number_of_events_read = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ReadConsoleW(
    _h_console_input: u64,
    _lp_buffer: *mut (),
    _n_number_of_chars_to_read: u32,
    lp_number_of_chars_read: *mut u32,
    _p_input_control: *const (),
) -> i32 {
    if !lp_number_of_chars_read.is_null() {
        *lp_number_of_chars_read = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn FreeConsole() -> i32 {
    1
}


#[no_mangle]
pub unsafe extern "win64" fn SetStdHandle(_n_std_handle: u32, _h_handle: u64) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetOverlappedResult(
    _h_file: u64,
    _lp_overlapped: *mut (),
    lp_number_of_bytes_transferred: *mut u32,
    _b_wait: i32,
) -> i32 {
    if !lp_number_of_bytes_transferred.is_null() {
        *lp_number_of_bytes_transferred = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentDirectoryA(
    n_buffer_length: u32,
    lp_buffer: *mut libc::c_char,
) -> u32 {
    let cwd = get_process_current_directory();
    let bytes = cwd.as_bytes();
    if lp_buffer.is_null() || n_buffer_length < (bytes.len() + 1) as u32 {
        return (bytes.len() + 1) as u32;
    }
    for i in 0..bytes.len() {
        *lp_buffer.add(i) = bytes[i] as libc::c_char;
    }
    *lp_buffer.add(bytes.len()) = 0;
    bytes.len() as u32
}

#[no_mangle]
pub unsafe extern "win64" fn ReadProcessMemory(
    _h_process: u64,
    lp_base_address: *const u8,
    lp_buffer: *mut u8,
    n_size: usize,
    lp_number_of_bytes_read: *mut usize,
) -> i32 {
    if lp_base_address.is_null() || lp_buffer.is_null() {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(lp_base_address, lp_buffer, n_size);
    }
    if !lp_number_of_bytes_read.is_null() {
        *lp_number_of_bytes_read = n_size;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn DebugBreak() {
    println!("[KERNEL32] DebugBreak() called (no-op in Rine)");
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemTimePreciseAsFileTime(lp_system_time_as_file_time: *mut u64) {
    GetSystemTimeAsFileTime(lp_system_time_as_file_time);
}

pub static CURRENT_IMAGE_BASE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0x0000000140000000);
pub static CURRENT_PEB_ADDRESS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn set_current_image_base(base: u64) {
    CURRENT_IMAGE_BASE.store(base, std::sync::atomic::Ordering::SeqCst);
}

pub fn get_current_image_base() -> u64 {
    CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn set_current_peb_address(peb: u64) {
    CURRENT_PEB_ADDRESS.store(peb, std::sync::atomic::Ordering::SeqCst);
}

pub fn get_current_peb_address() -> u64 {
    CURRENT_PEB_ADDRESS.load(std::sync::atomic::Ordering::SeqCst)
}

#[repr(C)]
pub struct RtlCriticalSection {
    pub debug_info: usize,
    pub lock_count: i32,
    pub recursion_count: i32,
    pub owning_thread: usize,
    pub lock_semaphore: usize,
    pub spin_count: usize,
}

pub unsafe extern "win64" fn InitializeCriticalSection(cs: *mut RtlCriticalSection) {
    if !cs.is_null() {
        (*cs).debug_info = 0;
        (*cs).lock_count = -1;
        (*cs).recursion_count = 0;
        (*cs).owning_thread = 0;
        (*cs).lock_semaphore = 0;
        (*cs).spin_count = 0;
    }
}

pub unsafe extern "win64" fn InitializeCriticalSectionAndSpinCount(
    cs: *mut RtlCriticalSection,
    spin: u32,
) -> i32 {
    if !cs.is_null() {
        (*cs).debug_info = 0;
        (*cs).lock_count = -1;
        (*cs).recursion_count = 0;
        (*cs).owning_thread = 0;
        (*cs).lock_semaphore = 0;
        (*cs).spin_count = spin as usize;
    }
    1
}

pub unsafe extern "win64" fn InitializeCriticalSectionEx(
    cs: *mut RtlCriticalSection,
    spin: u32,
    _flags: u32,
) -> i32 {
    InitializeCriticalSectionAndSpinCount(cs, spin)
}

static CS_REGISTRY: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, Box<libc::pthread_mutex_t>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

static SRW_REGISTRY: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, Box<libc::pthread_rwlock_t>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

pub unsafe extern "win64" fn EnterCriticalSection(cs: *mut RtlCriticalSection) {
    if cs.is_null() || (cs as usize) < 0x10000 {
        return;
    }
    let cs_key = cs as usize;
    let mutex_ptr = {
        let mut reg = CS_REGISTRY.lock().unwrap();
        let entry = reg.entry(cs_key).or_insert_with(|| {
            let mut m: Box<libc::pthread_mutex_t> = Box::new(std::mem::zeroed());
            let mut attr: libc::pthread_mutexattr_t = std::mem::zeroed();
            libc::pthread_mutexattr_init(&mut attr);
            libc::pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_RECURSIVE);
            libc::pthread_mutex_init(m.as_mut(), &attr);
            libc::pthread_mutexattr_destroy(&mut attr);
            m
        });
        entry.as_mut() as *mut libc::pthread_mutex_t
    };
    libc::pthread_mutex_lock(mutex_ptr);
    let tid = libc::pthread_self() as usize;
    (*cs).owning_thread = tid;
    (*cs).recursion_count += 1;
    (*cs).lock_count = 0;
}

pub unsafe extern "win64" fn LeaveCriticalSection(cs: *mut RtlCriticalSection) {
    if cs.is_null() || (cs as usize) < 0x10000 {
        return;
    }
    let cs_key = cs as usize;
    let mutex_ptr = {
        let mut reg = CS_REGISTRY.lock().unwrap();
        if let Some(m) = reg.get_mut(&cs_key) {
            m.as_mut() as *mut libc::pthread_mutex_t
        } else {
            return;
        }
    };
    if (*cs).recursion_count > 1 {
        (*cs).recursion_count -= 1;
    } else {
        (*cs).recursion_count = 0;
        (*cs).owning_thread = 0;
        (*cs).lock_count = -1;
    }
    libc::pthread_mutex_unlock(mutex_ptr);
}

pub unsafe extern "win64" fn TryEnterCriticalSection(cs: *mut RtlCriticalSection) -> i32 {
    if cs.is_null() || (cs as usize) < 0x10000 {
        return 0;
    }
    let cs_key = cs as usize;
    let mutex_ptr = {
        let mut reg = CS_REGISTRY.lock().unwrap();
        let entry = reg.entry(cs_key).or_insert_with(|| {
            let mut m: Box<libc::pthread_mutex_t> = Box::new(std::mem::zeroed());
            let mut attr: libc::pthread_mutexattr_t = std::mem::zeroed();
            libc::pthread_mutexattr_init(&mut attr);
            libc::pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_RECURSIVE);
            libc::pthread_mutex_init(m.as_mut(), &attr);
            libc::pthread_mutexattr_destroy(&mut attr);
            m
        });
        entry.as_mut() as *mut libc::pthread_mutex_t
    };
    let res = libc::pthread_mutex_trylock(mutex_ptr);
    if res == 0 {
        let tid = libc::pthread_self() as usize;
        (*cs).owning_thread = tid;
        (*cs).recursion_count += 1;
        (*cs).lock_count = 0;
        1
    } else {
        0
    }
}

pub unsafe extern "win64" fn DeleteCriticalSection(cs: *mut RtlCriticalSection) {
    if cs.is_null() || (cs as usize) < 0x10000 {
        return;
    }
    let cs_key = cs as usize;
    let mut reg = CS_REGISTRY.lock().unwrap();
    if let Some(mut m) = reg.remove(&cs_key) {
        libc::pthread_mutex_destroy(m.as_mut());
    }
    (*cs).lock_count = -1;
    (*cs).recursion_count = 0;
    (*cs).owning_thread = 0;
}

pub unsafe extern "win64" fn InitializeSRWLock(lock: *mut usize) {
    if !lock.is_null() {
        *lock = 0;
    }
}

pub unsafe extern "win64" fn AcquireSRWLockExclusive(lock: *mut usize) {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        let entry = reg.entry(key).or_insert_with(|| {
            let mut rw: Box<libc::pthread_rwlock_t> = Box::new(std::mem::zeroed());
            libc::pthread_rwlock_init(rw.as_mut(), std::ptr::null());
            rw
        });
        entry.as_mut() as *mut libc::pthread_rwlock_t
    };
    libc::pthread_rwlock_wrlock(rwlock_ptr);
    *lock = 1;
}

pub unsafe extern "win64" fn ReleaseSRWLockExclusive(lock: *mut usize) {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        if let Some(rw) = reg.get_mut(&key) {
            rw.as_mut() as *mut libc::pthread_rwlock_t
        } else {
            *lock = 0;
            return;
        }
    };
    *lock = 0;
    libc::pthread_rwlock_unlock(rwlock_ptr);
}

pub unsafe extern "win64" fn AcquireSRWLockShared(lock: *mut usize) {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        let entry = reg.entry(key).or_insert_with(|| {
            let mut rw: Box<libc::pthread_rwlock_t> = Box::new(std::mem::zeroed());
            libc::pthread_rwlock_init(rw.as_mut(), std::ptr::null());
            rw
        });
        entry.as_mut() as *mut libc::pthread_rwlock_t
    };
    libc::pthread_rwlock_rdlock(rwlock_ptr);
    *lock = 1;
}

pub unsafe extern "win64" fn ReleaseSRWLockShared(lock: *mut usize) {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        if let Some(rw) = reg.get_mut(&key) {
            rw.as_mut() as *mut libc::pthread_rwlock_t
        } else {
            *lock = 0;
            return;
        }
    };
    *lock = 0;
    libc::pthread_rwlock_unlock(rwlock_ptr);
}

pub unsafe extern "win64" fn TryAcquireSRWLockExclusive(lock: *mut usize) -> u8 {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return 0;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        let entry = reg.entry(key).or_insert_with(|| {
            let mut rw: Box<libc::pthread_rwlock_t> = Box::new(std::mem::zeroed());
            libc::pthread_rwlock_init(rw.as_mut(), std::ptr::null());
            rw
        });
        entry.as_mut() as *mut libc::pthread_rwlock_t
    };
    if libc::pthread_rwlock_trywrlock(rwlock_ptr) == 0 {
        *lock = 1;
        1
    } else {
        0
    }
}

pub unsafe extern "win64" fn TryAcquireSRWLockShared(lock: *mut usize) -> u8 {
    if lock.is_null() || (lock as usize) < 0x10000 {
        return 0;
    }
    let key = lock as usize;
    let rwlock_ptr = {
        let mut reg = SRW_REGISTRY.lock().unwrap();
        let entry = reg.entry(key).or_insert_with(|| {
            let mut rw: Box<libc::pthread_rwlock_t> = Box::new(std::mem::zeroed());
            libc::pthread_rwlock_init(rw.as_mut(), std::ptr::null());
            rw
        });
        entry.as_mut() as *mut libc::pthread_rwlock_t
    };
    if libc::pthread_rwlock_tryrdlock(rwlock_ptr) == 0 {
        *lock = 1;
        1
    } else {
        0
    }
}

struct WinCondVar {
    mutex: std::sync::Mutex<()>,
    cond: std::sync::Condvar,
}

static COND_TABLE: std::sync::Mutex<
    Option<std::collections::HashMap<usize, std::sync::Arc<WinCondVar>>>,
> = std::sync::Mutex::new(None);

fn get_or_create_cond(var: *mut usize) -> Option<std::sync::Arc<WinCondVar>> {
    if var.is_null() || (var as usize) < 0x10000 {
        return None;
    }
    let key = var as usize;
    let mut guard = COND_TABLE.lock().unwrap();
    let map = guard.get_or_insert_with(std::collections::HashMap::new);
    let cv = map
        .entry(key)
        .or_insert_with(|| {
            std::sync::Arc::new(WinCondVar {
                mutex: std::sync::Mutex::new(()),
                cond: std::sync::Condvar::new(),
            })
        })
        .clone();
    Some(cv)
}

pub unsafe extern "win64" fn InitializeConditionVariable(var: *mut usize) {
    if !var.is_null() && (var as usize) >= 0x10000 {
        *var = 0;
        let key = var as usize;
        let mut guard = COND_TABLE.lock().unwrap();
        let map = guard.get_or_insert_with(std::collections::HashMap::new);
        map.insert(
            key,
            std::sync::Arc::new(WinCondVar {
                mutex: std::sync::Mutex::new(()),
                cond: std::sync::Condvar::new(),
            }),
        );
    }
}

pub unsafe extern "win64" fn WakeConditionVariable(var: *mut usize) {
    if let Some(cv) = get_or_create_cond(var) {
        let _guard = match cv.mutex.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        cv.cond.notify_one();
    }
}

pub unsafe extern "win64" fn WakeAllConditionVariable(var: *mut usize) {
    if let Some(cv) = get_or_create_cond(var) {
        let _guard = match cv.mutex.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        cv.cond.notify_all();
    }
}

pub unsafe extern "win64" fn SleepConditionVariableCS(
    var: *mut usize,
    cs: *mut RtlCriticalSection,
    timeout: u32,
) -> i32 {
    const ERROR_TIMEOUT: u32 = 1460;
    let cv = match get_or_create_cond(var) {
        Some(c) => c,
        None => return 0,
    };

    let guard = match cv.mutex.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };

    LeaveCriticalSection(cs);

    let timed_out = if timeout == 0xFFFFFFFF {
        // Use bounded wait (25ms) to prevent lost-wake deadlocks when WakeConditionVariable
        // is signaled without holding the critical section (allowed by Win32 specs).
        let dur = std::time::Duration::from_millis(25);
        let _ = cv.cond.wait_timeout(guard, dur);
        false
    } else {
        let dur = std::time::Duration::from_millis(timeout as u64);
        match cv.cond.wait_timeout(guard, dur) {
            Ok((_, wait_res)) => wait_res.timed_out(),
            Err(poisoned) => poisoned.into_inner().1.timed_out(),
        }
    };

    EnterCriticalSection(cs);

    if timed_out {
        SetLastError(ERROR_TIMEOUT);
        0
    } else {
        1
    }
}

pub unsafe extern "win64" fn SleepConditionVariableSRW(
    var: *mut usize,
    lock: *mut usize,
    timeout: u32,
    flags: u32,
) -> i32 {
    const ERROR_TIMEOUT: u32 = 1460;
    let cv = match get_or_create_cond(var) {
        Some(c) => c,
        None => return 0,
    };

    let guard = match cv.mutex.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };

    let is_shared = (flags & 1) != 0;
    if is_shared {
        ReleaseSRWLockShared(lock);
    } else {
        ReleaseSRWLockExclusive(lock);
    }

    let timed_out = if timeout == 0xFFFFFFFF {
        // Use bounded wait (25ms) to prevent lost-wake deadlocks when WakeConditionVariable
        // is signaled without holding the SRWLock (allowed by Win32 specs).
        let dur = std::time::Duration::from_millis(25);
        let _ = cv.cond.wait_timeout(guard, dur);
        false
    } else {
        let dur = std::time::Duration::from_millis(timeout as u64);
        match cv.cond.wait_timeout(guard, dur) {
            Ok((_, wait_res)) => wait_res.timed_out(),
            Err(poisoned) => poisoned.into_inner().1.timed_out(),
        }
    };

    if is_shared {
        AcquireSRWLockShared(lock);
    } else {
        AcquireSRWLockExclusive(lock);
    }

    if timed_out {
        SetLastError(ERROR_TIMEOUT);
        0
    } else {
        1
    }
}

// Win32 synchronization primitives (events, semaphores, mutexes, timers)
// Примитивы синхронизации Win32 (события, семафоры, мьютексы, таймеры)

pub struct SemaphoreState {
    pub count: i32,
    pub max_count: i32,
}

pub struct MutexState {
    pub owner: u32,
    pub count: u32,
}

pub struct TimerState {
    pub signaled: bool,
    pub manual_reset: bool,
    pub generation: u64,
}

/// HANDLE-based event table.
/// Таблица дескрипторов событий Win32 (HANDLE).
static EVENT_TABLE: std::sync::Mutex<
    Option<HashMap<u64, std::sync::Arc<(std::sync::Mutex<(bool, bool)>, std::sync::Condvar)>>>,
> = std::sync::Mutex::new(None);

/// HANDLE-based semaphore table.
/// Таблица дескрипторов семафоров Win32 (HANDLE).
static SEMAPHORE_TABLE: std::sync::Mutex<
    Option<HashMap<u64, std::sync::Arc<(std::sync::Mutex<SemaphoreState>, std::sync::Condvar)>>>,
> = std::sync::Mutex::new(None);

/// HANDLE-based mutex table.
/// Таблица дескрипторов мьютексов Win32 (HANDLE).
static MUTEX_TABLE: std::sync::Mutex<
    Option<HashMap<u64, std::sync::Arc<(std::sync::Mutex<MutexState>, std::sync::Condvar)>>>,
> = std::sync::Mutex::new(None);

/// HANDLE-based timer table.
/// Таблица дескрипторов таймеров Win32 (HANDLE).
static TIMER_TABLE: std::sync::Mutex<
    Option<HashMap<u64, std::sync::Arc<(std::sync::Mutex<TimerState>, std::sync::Condvar)>>>,
> = std::sync::Mutex::new(None);

static NAMED_EVENTS: std::sync::Mutex<Option<HashMap<String, u64>>> = std::sync::Mutex::new(None);
static NAMED_SEMAPHORES: std::sync::Mutex<Option<HashMap<String, u64>>> = std::sync::Mutex::new(None);
static NAMED_MUTEXES: std::sync::Mutex<Option<HashMap<String, u64>>> = std::sync::Mutex::new(None);
static NAMED_TIMERS: std::sync::Mutex<Option<HashMap<String, u64>>> = std::sync::Mutex::new(None);

static GLOBAL_HANDLE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x2000);

pub fn next_handle() -> u64 {
    GLOBAL_HANDLE_COUNTER.fetch_add(4, std::sync::atomic::Ordering::Relaxed)
}

unsafe fn read_wide_str(ptr: usize) -> String {
    if ptr == 0 {
        return String::new();
    }
    let mut len = 0;
    let p = ptr as *const u16;
    while *p.add(len) != 0 && len < 260 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

unsafe fn read_ansi_str(ptr: usize) -> String {
    if ptr == 0 {
        return String::new();
    }
    let mut len = 0;
    let p = ptr as *const u8;
    while *p.add(len) != 0 && len < 260 {
        len += 1;
    }
    String::from_utf8_lossy(std::slice::from_raw_parts(p, len)).to_string()
}

fn event_table_insert(handle: u64, manual_reset: bool, initial_state: bool) {
    let cv = std::sync::Arc::new((
        std::sync::Mutex::new((initial_state, manual_reset)),
        std::sync::Condvar::new(),
    ));
    let mut g = EVENT_TABLE.lock().unwrap();
    g.get_or_insert_with(HashMap::new).insert(handle, cv);
}

fn event_table_get(
    handle: u64,
) -> Option<std::sync::Arc<(std::sync::Mutex<(bool, bool)>, std::sync::Condvar)>> {
    let g = EVENT_TABLE.lock().unwrap();
    g.as_ref()?.get(&handle).cloned()
}

fn semaphore_table_insert(handle: u64, initial_count: i32, max_count: i32) {
    let sem = std::sync::Arc::new((
        std::sync::Mutex::new(SemaphoreState {
            count: initial_count,
            max_count,
        }),
        std::sync::Condvar::new(),
    ));
    let mut g = SEMAPHORE_TABLE.lock().unwrap();
    g.get_or_insert_with(HashMap::new).insert(handle, sem);
}

fn semaphore_table_get(
    handle: u64,
) -> Option<std::sync::Arc<(std::sync::Mutex<SemaphoreState>, std::sync::Condvar)>> {
    let g = SEMAPHORE_TABLE.lock().unwrap();
    g.as_ref()?.get(&handle).cloned()
}

fn mutex_table_insert(handle: u64, initial_owner: bool, owner_tid: u32) {
    let m = std::sync::Arc::new((
        std::sync::Mutex::new(MutexState {
            owner: if initial_owner { owner_tid } else { 0 },
            count: if initial_owner { 1 } else { 0 },
        }),
        std::sync::Condvar::new(),
    ));
    let mut g = MUTEX_TABLE.lock().unwrap();
    g.get_or_insert_with(HashMap::new).insert(handle, m);
}

fn mutex_table_get(
    handle: u64,
) -> Option<std::sync::Arc<(std::sync::Mutex<MutexState>, std::sync::Condvar)>> {
    let g = MUTEX_TABLE.lock().unwrap();
    g.as_ref()?.get(&handle).cloned()
}

fn timer_table_insert(handle: u64, manual_reset: bool) {
    let t = std::sync::Arc::new((
        std::sync::Mutex::new(TimerState {
            signaled: false,
            manual_reset,
            generation: 0,
        }),
        std::sync::Condvar::new(),
    ));
    let mut g = TIMER_TABLE.lock().unwrap();
    g.get_or_insert_with(HashMap::new).insert(handle, t);
}

fn timer_table_get(
    handle: u64,
) -> Option<std::sync::Arc<(std::sync::Mutex<TimerState>, std::sync::Condvar)>> {
    let g = TIMER_TABLE.lock().unwrap();
    g.as_ref()?.get(&handle).cloned()
}

#[no_mangle]
pub unsafe extern "win64" fn CreateEventW(
    _attr: usize,
    manual_reset: i32,
    initial_state: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_wide_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_EVENTS.lock().unwrap();
            let table = g_named.get_or_insert_with(HashMap::new);
            if let Some(&h) = table.get(&name_str) {
                SetLastError(183 /* ERROR_ALREADY_EXISTS */);
                return h as usize;
            }
            let handle = next_handle();
            event_table_insert(handle, manual_reset != 0, initial_state != 0);
            table.insert(name_str, handle);
            return handle as usize;
        }
    }
    let handle = next_handle();
    event_table_insert(handle, manual_reset != 0, initial_state != 0);
    handle as usize
}

#[no_mangle]
pub unsafe extern "win64" fn CreateEventA(
    attr: usize,
    manual_reset: i32,
    initial_state: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_ansi_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_EVENTS.lock().unwrap();
            let table = g_named.get_or_insert_with(HashMap::new);
            if let Some(&h) = table.get(&name_str) {
                SetLastError(183 /* ERROR_ALREADY_EXISTS */);
                return h as usize;
            }
            let handle = next_handle();
            event_table_insert(handle, manual_reset != 0, initial_state != 0);
            table.insert(name_str, handle);
            return handle as usize;
        }
    }
    CreateEventW(attr, manual_reset, initial_state, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateEventExW(
    attr: usize,
    name: usize,
    dw_flags: u32,
    _access: u32,
) -> usize {
    let manual_reset = (dw_flags & 0x00000001) != 0;
    let initial_state = (dw_flags & 0x00000002) != 0;
    CreateEventW(attr, manual_reset as i32, initial_state as i32, name)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateEventExA(
    attr: usize,
    name: usize,
    dw_flags: u32,
    access: u32,
) -> usize {
    CreateEventExW(attr, name, dw_flags, access)
}

#[no_mangle]
pub unsafe extern "win64" fn OpenEventW(_access: u32, _inherit: i32, name: usize) -> usize {
    if name != 0 {
        let name_str = read_wide_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_EVENTS.lock().unwrap();
            if let Some(table) = g_named.as_ref() {
                if let Some(&h) = table.get(&name_str) {
                    return h as usize;
                }
            }
        }
    }
    let handle = next_handle();
    event_table_insert(handle, false, false);
    handle as usize
}

#[no_mangle]
pub unsafe extern "win64" fn OpenEventA(access: u32, inherit: i32, name: usize) -> usize {
    if name != 0 {
        let name_str = read_ansi_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_EVENTS.lock().unwrap();
            if let Some(table) = g_named.as_ref() {
                if let Some(&h) = table.get(&name_str) {
                    return h as usize;
                }
            }
        }
    }
    OpenEventW(access, inherit, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn SetEvent(h_event: u64) -> i32 {
    if let Some(cv) = event_table_get(h_event) {
        let (lock, cvar) = &*cv;
        let mut state = lock.lock().unwrap();
        state.0 = true;
        if state.1 {
            cvar.notify_all();
        } else {
            cvar.notify_one();
        }
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn ResetEvent(h_event: u64) -> i32 {
    if let Some(cv) = event_table_get(h_event) {
        let (lock, _) = &*cv;
        let mut state = lock.lock().unwrap();
        state.0 = false;
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn PulseEvent(h_event: u64) -> i32 {
    if let Some(cv) = event_table_get(h_event) {
        let (lock, cvar) = &*cv;
        let mut state = lock.lock().unwrap();
        state.0 = true;
        cvar.notify_all();
        state.0 = false;
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateSemaphoreW(
    _attr: usize,
    initial_count: i32,
    max_count: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_wide_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_SEMAPHORES.lock().unwrap();
            let table = g_named.get_or_insert_with(HashMap::new);
            if let Some(&h) = table.get(&name_str) {
                SetLastError(183 /* ERROR_ALREADY_EXISTS */);
                return h as usize;
            }
            let handle = next_handle();
            semaphore_table_insert(handle, initial_count, max_count);
            table.insert(name_str.clone(), handle);
            println!("[KERNEL32] CreateSemaphoreW named '{}': handle=0x{:X}, init={}, max={}", name_str, handle, initial_count, max_count);
            return handle as usize;
        }
    }
    let handle = next_handle();
    semaphore_table_insert(handle, initial_count, max_count);
    println!("[KERNEL32] CreateSemaphoreW: handle=0x{:X}, init={}, max={}", handle, initial_count, max_count);
    handle as usize
}

#[no_mangle]
pub unsafe extern "win64" fn CreateSemaphoreA(
    attr: usize,
    initial_count: i32,
    max_count: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_ansi_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_SEMAPHORES.lock().unwrap();
            let table = g_named.get_or_insert_with(HashMap::new);
            if let Some(&h) = table.get(&name_str) {
                SetLastError(183 /* ERROR_ALREADY_EXISTS */);
                return h as usize;
            }
            let handle = next_handle();
            semaphore_table_insert(handle, initial_count, max_count);
            table.insert(name_str, handle);
            return handle as usize;
        }
    }
    CreateSemaphoreW(attr, initial_count, max_count, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateSemaphoreExW(
    attr: usize,
    initial_count: i32,
    max_count: i32,
    name: usize,
    _dw_flags: u32,
    _access: u32,
) -> usize {
    CreateSemaphoreW(attr, initial_count, max_count, name)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateSemaphoreExA(
    attr: usize,
    initial_count: i32,
    max_count: i32,
    name: usize,
    dw_flags: u32,
    access: u32,
) -> usize {
    CreateSemaphoreExW(attr, initial_count, max_count, name, dw_flags, access)
}

#[no_mangle]
pub unsafe extern "win64" fn OpenSemaphoreW(
    _access: u32,
    _inherit: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_wide_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_SEMAPHORES.lock().unwrap();
            if let Some(table) = g_named.as_ref() {
                if let Some(&h) = table.get(&name_str) {
                    return h as usize;
                }
            }
        }
    }
    let handle = next_handle();
    semaphore_table_insert(handle, 0, 1000);
    handle as usize
}

#[no_mangle]
pub unsafe extern "win64" fn OpenSemaphoreA(
    access: u32,
    inherit: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_ansi_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_SEMAPHORES.lock().unwrap();
            if let Some(table) = g_named.as_ref() {
                if let Some(&h) = table.get(&name_str) {
                    return h as usize;
                }
            }
        }
    }
    OpenSemaphoreW(access, inherit, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn ReleaseSemaphore(
    sem: u64,
    release_count: i32,
    prev_count: *mut i32,
) -> i32 {
    const ERROR_INVALID_PARAMETER: u32 = 87;
    const ERROR_TOO_MANY_POSTS: u32 = 298;

    if release_count <= 0 {
        SetLastError(ERROR_INVALID_PARAMETER);
        return 0;
    }
    if let Some(sem_entry) = semaphore_table_get(sem) {
        let (lock, cvar) = &*sem_entry;
        let mut state = lock.lock().unwrap();
        if !prev_count.is_null() {
            *prev_count = state.count;
        }
        if state.count + release_count > state.max_count {
            SetLastError(ERROR_TOO_MANY_POSTS);
            return 0;
        }
        state.count += release_count;
        cvar.notify_all();
        1
    } else {
        if !prev_count.is_null() {
            *prev_count = 1;
        }
        1
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateMutexW(
    _attr: usize,
    initial_owner: i32,
    name: usize,
) -> usize {
    let current_tid = GetCurrentThreadId();
    if name != 0 {
        let name_str = read_wide_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_MUTEXES.lock().unwrap();
            let table = g_named.get_or_insert_with(HashMap::new);
            if let Some(&h) = table.get(&name_str) {
                SetLastError(183 /* ERROR_ALREADY_EXISTS */);
                return h as usize;
            }
            let handle = next_handle();
            mutex_table_insert(handle, initial_owner != 0, current_tid);
            table.insert(name_str, handle);
            return handle as usize;
        }
    }
    let handle = next_handle();
    mutex_table_insert(handle, initial_owner != 0, current_tid);
    handle as usize
}

#[no_mangle]
pub unsafe extern "win64" fn CreateMutexA(
    attr: usize,
    initial_owner: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_ansi_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_MUTEXES.lock().unwrap();
            let table = g_named.get_or_insert_with(HashMap::new);
            if let Some(&h) = table.get(&name_str) {
                SetLastError(183 /* ERROR_ALREADY_EXISTS */);
                return h as usize;
            }
            let handle = next_handle();
            let current_tid = GetCurrentThreadId();
            mutex_table_insert(handle, initial_owner != 0, current_tid);
            table.insert(name_str, handle);
            return handle as usize;
        }
    }
    CreateMutexW(attr, initial_owner, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateMutexExW(
    attr: usize,
    name: usize,
    dw_flags: u32,
    _access: u32,
) -> usize {
    let initial_owner = if (dw_flags & 0x00000001) != 0 { 1 } else { 0 };
    CreateMutexW(attr, initial_owner, name)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateMutexExA(
    attr: usize,
    name: usize,
    dw_flags: u32,
    access: u32,
) -> usize {
    CreateMutexExW(attr, name, dw_flags, access)
}

#[no_mangle]
pub unsafe extern "win64" fn OpenMutexW(
    _access: u32,
    _inherit: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_wide_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_MUTEXES.lock().unwrap();
            if let Some(table) = g_named.as_ref() {
                if let Some(&h) = table.get(&name_str) {
                    return h as usize;
                }
            }
        }
    }
    let handle = next_handle();
    mutex_table_insert(handle, false, 0);
    handle as usize
}

#[no_mangle]
pub unsafe extern "win64" fn OpenMutexA(
    access: u32,
    inherit: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_ansi_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_MUTEXES.lock().unwrap();
            if let Some(table) = g_named.as_ref() {
                if let Some(&h) = table.get(&name_str) {
                    return h as usize;
                }
            }
        }
    }
    OpenMutexW(access, inherit, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn ReleaseMutex(h_mutex: u64) -> i32 {
    const ERROR_NOT_OWNER: u32 = 288;

    if let Some(mutex_entry) = mutex_table_get(h_mutex) {
        let current_tid = GetCurrentThreadId();
        let (lock, cvar) = &*mutex_entry;
        let mut state = lock.lock().unwrap();
        if state.owner != current_tid {
            SetLastError(ERROR_NOT_OWNER);
            return 0;
        }
        state.count -= 1;
        if state.count == 0 {
            state.owner = 0;
            cvar.notify_one();
        }
        1
    } else {
        SetEvent(h_mutex);
        1
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateWaitableTimerW(
    _attr: usize,
    manual_reset: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_wide_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_TIMERS.lock().unwrap();
            let table = g_named.get_or_insert_with(HashMap::new);
            if let Some(&h) = table.get(&name_str) {
                SetLastError(183 /* ERROR_ALREADY_EXISTS */);
                return h as usize;
            }
            let handle = next_handle();
            timer_table_insert(handle, manual_reset != 0);
            table.insert(name_str, handle);
            return handle as usize;
        }
    }
    let handle = next_handle();
    timer_table_insert(handle, manual_reset != 0);
    handle as usize
}

#[no_mangle]
pub unsafe extern "win64" fn CreateWaitableTimerA(
    attr: usize,
    manual_reset: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_ansi_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_TIMERS.lock().unwrap();
            let table = g_named.get_or_insert_with(HashMap::new);
            if let Some(&h) = table.get(&name_str) {
                SetLastError(183 /* ERROR_ALREADY_EXISTS */);
                return h as usize;
            }
            let handle = next_handle();
            timer_table_insert(handle, manual_reset != 0);
            table.insert(name_str, handle);
            return handle as usize;
        }
    }
    CreateWaitableTimerW(attr, manual_reset, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateWaitableTimerExW(
    attr: usize,
    name: usize,
    flags: u32,
    _access: u32,
) -> usize {
    let manual_reset = (flags & 0x00000001) != 0;
    CreateWaitableTimerW(attr, manual_reset as i32, name)
}

#[no_mangle]
pub unsafe extern "win64" fn CreateWaitableTimerExA(
    attr: usize,
    name: usize,
    flags: u32,
    access: u32,
) -> usize {
    CreateWaitableTimerExW(attr, name, flags, access)
}

#[no_mangle]
pub unsafe extern "win64" fn OpenWaitableTimerW(
    _access: u32,
    _inherit: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_wide_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_TIMERS.lock().unwrap();
            if let Some(table) = g_named.as_ref() {
                if let Some(&h) = table.get(&name_str) {
                    return h as usize;
                }
            }
        }
    }
    let handle = next_handle();
    timer_table_insert(handle, false);
    handle as usize
}

#[no_mangle]
pub unsafe extern "win64" fn OpenWaitableTimerA(
    access: u32,
    inherit: i32,
    name: usize,
) -> usize {
    if name != 0 {
        let name_str = read_ansi_str(name);
        if !name_str.is_empty() {
            let mut g_named = NAMED_TIMERS.lock().unwrap();
            if let Some(table) = g_named.as_ref() {
                if let Some(&h) = table.get(&name_str) {
                    return h as usize;
                }
            }
        }
    }
    OpenWaitableTimerW(access, inherit, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn SetWaitableTimer(
    h_timer: u64,
    lp_due_time: *const i64,
    period: i32,
    _completion_routine: usize,
    _arg: usize,
    _resume: i32,
) -> i32 {
    if lp_due_time.is_null() {
        return 0;
    }
    let due = *lp_due_time;
    if let Some(timer_arc) = timer_table_get(h_timer) {
        let (lock, _cvar) = &*timer_arc;
        let gen = {
            let mut state = lock.lock().unwrap();
            state.signaled = false;
            state.generation = state.generation.wrapping_add(1);
            state.generation
        };
        let duration = if due < 0 {
            let ns = (-due) as u64 * 100;
            std::time::Duration::from_nanos(ns)
        } else {
            std::time::Duration::from_millis(1)
        };
        let timer_arc_clone = timer_arc.clone();
        std::thread::spawn(move || {
            std::thread::sleep(duration);
            {
                let (lock, cvar) = &*timer_arc_clone;
                let mut state = lock.lock().unwrap();
                if state.generation != gen {
                    return;
                }
                state.signaled = true;
                cvar.notify_all();
            }
            if period > 0 {
                let period_dur = std::time::Duration::from_millis(period as u64);
                loop {
                    std::thread::sleep(period_dur);
                    let (lock, cvar) = &*timer_arc_clone;
                    let mut state = lock.lock().unwrap();
                    if state.generation != gen {
                        return;
                    }
                    state.signaled = true;
                    cvar.notify_all();
                }
            }
        });
        1
    } else {
        1
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetWaitableTimerEx(
    h_timer: u64,
    lp_due_time: *const i64,
    period: i32,
    completion_routine: usize,
    arg: usize,
    _wake_context: usize,
    _tolerable_delay: u32,
) -> i32 {
    SetWaitableTimer(h_timer, lp_due_time, period, completion_routine, arg, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn CancelWaitableTimer(h_timer: u64) -> i32 {
    if let Some(timer_arc) = timer_table_get(h_timer) {
        let (lock, _) = &*timer_arc;
        let mut state = lock.lock().unwrap();
        state.generation = state.generation.wrapping_add(1);
        state.signaled = false;
        1
    } else {
        1
    }
}

pub unsafe extern "win64" fn InitOnceInitialize(once: *mut usize) {
    if !once.is_null() {
        *once = 0;
    }
}

pub unsafe extern "win64" fn InitOnceBeginInitialize(
    once: *mut usize,
    _flags: u32,
    pending: *mut i32,
    _context: *mut usize,
) -> i32 {
    if !pending.is_null() {
        if !once.is_null() && *once == 2 {
            *pending = 0;
        } else {
            *pending = 1;
        }
    }
    1
}

pub unsafe extern "win64" fn InitOnceComplete(
    once: *mut usize,
    _flags: u32,
    _context: usize,
) -> i32 {
    if !once.is_null() {
        *once = 2;
    }
    1
}

pub unsafe extern "win64" fn InitOnceExecuteOnce(
    once: *mut usize,
    func: unsafe extern "win64" fn(*mut usize, usize, *mut usize) -> i32,
    param: usize,
    context: *mut usize,
) -> i32 {
    if !once.is_null() && *once == 2 {
        return 1;
    }
    let res = func(once, param, context);
    if res != 0 && !once.is_null() {
        *once = 2;
    }
    res
}

pub unsafe extern "win64" fn GetModuleHandleExW(
    flags: u32,
    name: u64,
    module_out: *mut u64,
) -> i32 {
    let handle = if (flags & 0x00000004) != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
        CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst)
    } else if name == 0 || name < 0x10000 {
        CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst)
    } else {
        let name_opt = read_utf16_ptr(name as *const u16);
        ModuleRegistry::get_module_handle(name_opt.as_deref()) as u64
    };
    if !module_out.is_null() {
        *module_out = handle;
    }
    if handle != 0 { 1 } else { 0 }
}

pub unsafe extern "win64" fn GetModuleHandleExA(
    flags: u32,
    name: u64,
    module_out: *mut u64,
) -> i32 {
    let handle = if (flags & 0x00000004) != 0 {
        CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst)
    } else if name == 0 || name < 0x10000 {
        CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst)
    } else {
        let cstr = std::ffi::CStr::from_ptr(name as *const std::ffi::c_char);
        let s = cstr.to_string_lossy();
        ModuleRegistry::get_module_handle(Some(&s)) as u64
    };
    if !module_out.is_null() {
        *module_out = handle;
    }
    if handle != 0 { 1 } else { 0 }
}

static TARGET_EXE_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_target_exe_path(path: &str) {
    let _ = TARGET_EXE_PATH.set(path.to_string());
}

pub fn get_target_exe_path() -> String {
    let p = TARGET_EXE_PATH.get().cloned().unwrap_or_else(|| {
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "That's not my neighbor.exe".into())
    });
    let path = std::path::Path::new(&p);
    if path.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join(path).to_string_lossy().to_string();
        }
    }
    p
}

unsafe fn read_utf16_ptr(ptr: *const u16) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x10000 {
        return None;
    }
    let mut buf = Vec::new();
    for i in 0..4096usize {
        let ch = *ptr.add(i);
        if ch == 0 {
            break;
        }
        buf.push(ch);
    }
    Some(String::from_utf16_lossy(&buf))
}

pub struct ModuleRegistry;

#[derive(Debug, Clone)]
pub struct RegisteredModule {
    pub name: String,
    pub base: usize,
    pub size: usize,
    pub refcount: u32,
    pub pinned: bool,
}

static MODULE_TABLE: std::sync::Mutex<Option<Vec<RegisteredModule>>> = std::sync::Mutex::new(None);

fn normalize_module_name(raw_name: &str) -> String {
    let clean = raw_name.replace('\\', "/");
    let path = std::path::Path::new(&clean);
    let mut file_name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| raw_name.to_string());

    while file_name.to_ascii_lowercase().ends_with(".dll.dll") {
        file_name.truncate(file_name.len() - 4);
    }
    if file_name.to_ascii_lowercase().ends_with(".cpl.dll") {
        file_name.truncate(file_name.len() - 4);
    }

    if !file_name.contains('.') {
        file_name.push_str(".dll");
    }
    file_name
}

fn get_known_native_base(name: &str) -> Option<usize> {
    let lower = name.to_ascii_lowercase();
    if lower == "ntdll.dll" || lower == "ntdll" {
        Some(0x7FFD0000)
    } else if lower == "kernel32.dll"
        || lower == "kernel32"
        || lower == "kernelbase.dll"
        || lower == "kernelbase"
    {
        Some(0x7FFF0000)
    } else if lower == "user32.dll" || lower == "user32" {
        Some(0x7FFE0000)
    } else if lower == "gdi32.dll" || lower == "gdi32" {
        Some(0x7FFC0000)
    } else if lower == "advapi32.dll" || lower == "advapi32" {
        Some(0x7FFB0000)
    } else if lower == "shell32.dll" || lower == "shell32" {
        Some(0x7FFA0000)
    } else if lower == "ole32.dll" || lower == "ole32" {
        Some(0x7FF90000)
    } else if lower == "ws2_32.dll" || lower == "ws2_32" {
        Some(0x7FF80000)
    } else if lower == "bcrypt.dll" || lower == "bcrypt" {
        Some(0x7FF70000)
    } else if lower == "shcore.dll" || lower == "shcore" || lower.starts_with("api-ms-win-shcore-") {
        Some(0x7FF60000)
    } else if lower == "uxtheme.dll" || lower == "uxtheme" {
        Some(0x7FF50000)
    } else if lower == "dwmapi.dll" || lower == "dwmapi" {
        Some(0x7FF40000)
    } else if lower == "version.dll" || lower == "version" {
        Some(0x7FF30000)
    } else if lower == "avrt.dll" || lower == "avrt" {
        Some(0x7FF20000)
    } else if lower == "hid.dll" || lower == "hid" {
        Some(0x7FF10000)
    } else if lower == "vulkan-1.dll" || lower == "vulkan-1" {
        Some(0x7FF00000)
    } else if lower == "msvcrt.dll"
        || lower == "msvcrt"
        || lower == "ucrtbase.dll"
        || lower == "ucrtbase"
        || lower == "vcruntime140.dll"
        || lower == "vcruntime140_1.dll"
        || lower == "msvcp140.dll"
        || lower == "msvcp140_1.dll"
        || lower.starts_with("api-ms-win-crt-")
    {
        Some(0x7FEF0000)
    } else if lower == "d3d11.dll" || lower == "d3d11" {
        Some(0x7FEE0000)
    } else if lower == "dxgi.dll" || lower == "dxgi" {
        Some(0x7FED0000)
    } else if lower == "shlwapi.dll" || lower == "shlwapi" {
        Some(0x7FEC0000)
    } else if lower == "imm32.dll" || lower == "imm32" {
        Some(0x7FEB0000)
    } else if lower == "setupapi.dll" || lower == "setupapi" {
        Some(0x7FEA0000)
    } else if lower == "winmm.dll" || lower == "winmm" {
        Some(0x7FE90000)
    } else if lower == "dsound.dll" || lower == "dsound" {
        Some(0x7FE80000)
    } else if lower == "dinput8.dll" || lower == "dinput8" {
        Some(0x7FE70000)
    } else if lower.starts_with("xinput") {
        Some(0x7FE60000)
    } else if lower == "secur32.dll"
        || lower == "secur32"
        || lower == "security.dll"
        || lower == "security"
        || lower == "sspicli.dll"
        || lower == "sspicli"
    {
        Some(0x7FE50000)
    } else if lower == "d3d12.dll" || lower == "d3d12" {
        Some(0x7FE40000)
    } else if lower == "cfgmgr32.dll" || lower == "cfgmgr32" {
        Some(0x7FE30000)
    } else if lower == "dstorage.dll" || lower == "dstorage" {
        Some(0x7FE20000)
    } else if lower == "bthprops.cpl"
        || lower.starts_with("bthprops")
        || lower == "bluetoothapis.dll"
        || lower == "bluetoothapis"
    {
        Some(0x7FE10000)
    } else if lower == "steam_api64.dll"
        || lower == "steam_api64"
        || lower == "steam_api.dll"
        || lower == "steam_api"
        || lower == "steamclient64.dll"
        || lower == "steamclient64"
    {
        Some(0x7FE00000)
    } else if lower == "winhttp.dll" || lower == "winhttp" {
        Some(0x7FDF0000)
    } else {
        None
    }
}

impl ModuleRegistry {
    pub fn load_library(raw_name: &str) -> usize {
        let norm = normalize_module_name(raw_name);
        let mut guard = MODULE_TABLE.lock().unwrap();
        let table = guard.get_or_insert_with(Vec::new);

        // 1. Check if already registered
        for m in table.iter_mut() {
            if m.name.eq_ignore_ascii_case(&norm) {
                m.refcount = m.refcount.saturating_add(1);
                return m.base;
            }
        }

        // 2. Known native pseudo DLL
        if let Some(base) = get_known_native_base(&norm) {
            table.push(RegisteredModule {
                name: norm.clone(),
                base,
                size: 0x10000,
                refcount: 1,
                pinned: true,
            });
            return base;
        }

        // 3. Check LOADED_MODULES (PE modules like DXVK or Main EXE)
        let pe_entry = {
            let guard_pe = crate::pe::imports::LOADED_MODULES.lock().unwrap();
            guard_pe.as_ref().and_then(|vec| {
                vec.iter()
                    .find(|(name, _, _)| normalize_module_name(name).eq_ignore_ascii_case(&norm))
                    .cloned()
            })
        };

        if let Some((_, base, size)) = pe_entry {
            table.push(RegisteredModule {
                name: norm.clone(),
                base,
                size,
                refcount: 1,
                pinned: true,
            });
            return base;
        }

        // 3b. Companion module already loaded
        if let Some(base) = crate::mem::loader::get_companion_module_base(&norm) {
            table.push(RegisteredModule {
                name: norm.clone(),
                base,
                size: 0x100000,
                refcount: 1,
                pinned: true,
            });
            return base;
        }

        // 3c. Try loading companion DLL from disk
        drop(guard);
        let mut clean_raw = raw_name.to_string();
        while clean_raw.to_ascii_lowercase().ends_with(".dll.dll") {
            clean_raw.truncate(clean_raw.len() - 4);
        }
        if let Some(base) = crate::mem::loader::LoadedModule::load_companion_dll(&clean_raw) {
            let mut guard = MODULE_TABLE.lock().unwrap();
            let table = guard.get_or_insert_with(Vec::new);
            table.push(RegisteredModule {
                name: norm.clone(),
                base,
                size: 0x100000,
                refcount: 1,
                pinned: false,
            });
            return base;
        }
        let mut guard = MODULE_TABLE.lock().unwrap();
        let table = guard.get_or_insert_with(Vec::new);

        // 4. Wine provider fallback if allowlisted
        let config = crate::providers::get_global_wine_config();
        if config.enabled && crate::providers::wine::is_module_allowlisted(&config, &norm) {
            if let Some(dll_dir) = config
                .dll_path
                .clone()
                .or_else(crate::providers::wine::find_wine_dll_dir)
            {
                let dll_file_path = dll_dir.join(&norm);
                if dll_file_path.exists() {
                    if let Ok(module) =
                        crate::providers::wine::load_and_map_wine_pe_dll(&dll_file_path, &config)
                    {
                        let base = module.image_base;
                        let size = module.image_size;
                        table.push(RegisteredModule {
                            name: norm.clone(),
                            base,
                            size,
                            refcount: 1,
                            pinned: false,
                        });
                        return base;
                    }
                }
            }
        }

        // 5. Unknown / absent module returns 0 (ERROR_MOD_NOT_FOUND)
        0
    }

    pub fn get_module_handle(raw_name: Option<&str>) -> usize {
        let raw = match raw_name {
            Some(s) if !s.is_empty() => s,
            _ => {
                return CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) as usize;
            }
        };
        let norm = normalize_module_name(raw);
        let guard = MODULE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_ref() {
            for m in table {
                if m.name.eq_ignore_ascii_case(&norm) {
                    return m.base;
                }
            }
        }

        if let Some(guard_pe) = crate::pe::imports::LOADED_MODULES.lock().unwrap().as_ref() {
            for (pe_name, pe_base, _) in guard_pe {
                let pe_norm = normalize_module_name(pe_name);
                if pe_norm.eq_ignore_ascii_case(&norm) {
                    return *pe_base;
                }
            }
        }

        if let Some(base) = get_known_native_base(&norm) {
            return base;
        }

        if let Some(base) = crate::mem::loader::get_companion_module_base(&norm) {
            return base;
        }

        0
    }

    pub fn get_module_name(base: usize) -> Option<String> {
        let exe_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) as usize;
        if base == exe_base {
            return crate::pe::imports::find_module_name_by_base(base)
                .or_else(|| Some("TARGET.EXE".to_string()));
        }

        let guard = MODULE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_ref() {
            for m in table {
                if m.base == base {
                    return Some(m.name.clone());
                }
            }
        }

        if let Some(name) = crate::pe::imports::find_module_name_by_base(base) {
            return Some(name);
        }

        if let Some(name) = crate::mem::loader::get_companion_module_name(base) {
            return Some(name);
        }

        match base {
            0x7FFD0000 => Some("ntdll.dll".to_string()),
            0x7FFF0000 => Some("kernel32.dll".to_string()),
            0x7FFE0000 => Some("user32.dll".to_string()),
            0x7FFC0000 => Some("gdi32.dll".to_string()),
            0x7FFB0000 => Some("advapi32.dll".to_string()),
            0x7FFA0000 => Some("shell32.dll".to_string()),
            0x7FF90000 => Some("ole32.dll".to_string()),
            0x7FF80000 => Some("ws2_32.dll".to_string()),
            0x7FF70000 => Some("bcrypt.dll".to_string()),
            0x7FF60000 => Some("shcore.dll".to_string()),
            0x7FF50000 => Some("uxtheme.dll".to_string()),
            0x7FF40000 => Some("dwmapi.dll".to_string()),
            0x7FF30000 => Some("version.dll".to_string()),
            0x7FF20000 => Some("avrt.dll".to_string()),
            0x7FF10000 => Some("hid.dll".to_string()),
            0x7FF00000 => Some("vulkan-1.dll".to_string()),
            0x7FEF0000 => Some("msvcrt.dll".to_string()),
            0x7FEE0000 => Some("d3d11.dll".to_string()),
            0x7FED0000 => Some("dxgi.dll".to_string()),
            0x7FEC0000 => Some("shlwapi.dll".to_string()),
            0x7FEB0000 => Some("imm32.dll".to_string()),
            0x7FEA0000 => Some("setupapi.dll".to_string()),
            0x7FE90000 => Some("winmm.dll".to_string()),
            0x7FE80000 => Some("dsound.dll".to_string()),
            0x7FE70000 => Some("dinput8.dll".to_string()),
            0x7FE60000 => Some("xinput1_3.dll".to_string()),
            0x7FE50000 => Some("secur32.dll".to_string()),
            0x7FE40000 => Some("d3d12.dll".to_string()),
            0x7FE30000 => Some("cfgmgr32.dll".to_string()),
            0x7FE20000 => Some("dstorage.dll".to_string()),
            0x7FE10000 => Some("bthprops.cpl".to_string()),
            0x7FE00000 => Some("steam_api64.dll".to_string()),
            0x7FDF0000 => Some("winhttp.dll".to_string()),
            _ => None,
        }
    }

    pub fn free_library(base: usize) -> bool {
        let exe_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) as usize;
        if base == 0 {
            return false;
        }
        if base == exe_base {
            // Main executable cannot be unloaded
            return false;
        }

        {
            let mut guard = MODULE_TABLE.lock().unwrap();
            if let Some(table) = guard.as_mut() {
                for m in table.iter_mut() {
                    if m.base == base {
                        if m.refcount > 0 {
                            m.refcount -= 1;
                        }
                        return true;
                    }
                }
            }
        }

        if Self::get_module_name(base).is_some() {
            return true;
        }

        false
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleHandleW(module_name: u64) -> u64 {
    if module_name == 0 || module_name < 0x10000 {
        return CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    }
    let name_opt = read_utf16_ptr(module_name as *const u16);
    let ret = ModuleRegistry::get_module_handle(name_opt.as_deref()) as u64;
    println!("[KERNEL32] GetModuleHandleW({:?}) -> 0x{:X}", name_opt, ret);
    ret
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleHandleA(module_name: u64) -> u64 {
    if module_name == 0 || module_name < 0x10000 {
        return CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    }
    let cstr = std::ffi::CStr::from_ptr(module_name as *const std::ffi::c_char);
    let s = cstr.to_string_lossy();
    let ret = ModuleRegistry::get_module_handle(Some(&s)) as u64;
    println!("[KERNEL32] GetModuleHandleA('{}') -> 0x{:X}", s, ret);
    ret
}

thread_local! {
    static THREAD_LAST_ERROR: std::cell::Cell<u32> = std::cell::Cell::new(0);
}

#[no_mangle]
pub unsafe extern "win64" fn SetLastError(dw_err_code: u32) {
    THREAD_LAST_ERROR.with(|cell| cell.set(dw_err_code));
    let teb = get_teb_ptr();
    if !teb.is_null() && (teb as usize) >= 0x10000 {
        (*teb).last_error_value = dw_err_code;
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetLastError() -> u32 {
    let teb = get_teb_ptr();
    if !teb.is_null() && (teb as usize) >= 0x10000 {
        (*teb).last_error_value
    } else {
        THREAD_LAST_ERROR.with(|cell| cell.get())
    }
}

#[no_mangle]
pub unsafe extern "win64" fn ExitProcess(u_exit_code: u32) -> ! {
    eprintln!(
        "[RINE EXIT PROCESS] KERNEL32!ExitProcess called with exit_code=0x{:08X} ({})",
        u_exit_code, u_exit_code
    );
    crate::sys::shutdown::perform_host_cleanup();
    libc::_exit(u_exit_code as i32);
}

#[no_mangle]
pub unsafe extern "win64" fn TerminateProcess(_h_process: u64, u_exit_code: u32) -> i32 {
    eprintln!(
        "[RINE EXIT PROCESS] KERNEL32!TerminateProcess called with exit_code=0x{:08X} ({})",
        u_exit_code, u_exit_code
    );
    crate::sys::shutdown::perform_host_cleanup();
    libc::_exit(u_exit_code as i32);
}

#[no_mangle]
pub unsafe extern "win64" fn OutputDebugStringA(lp_output_string: *const libc::c_char) {
    if !lp_output_string.is_null() {
        if let Ok(s) = std::ffi::CStr::from_ptr(lp_output_string).to_str() {
            println!("[GODOT OUTPUT DEBUG STRING A] {}", s);
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn OutputDebugStringW(lp_output_string: *const u16) {
    if !lp_output_string.is_null() {
        let mut len = 0usize;
        while *lp_output_string.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(lp_output_string, len));
        println!("[GODOT OUTPUT DEBUG STRING W] {}", s);
    }
}

pub fn resolve_module_path(h_module: u64) -> String {
    let raw_path = if h_module != 0 {
        if let Some(p) = crate::mem::loader::get_module_path(h_module as usize) {
            p
        } else if let Some(p) = crate::mem::loader::get_companion_module_name(h_module as usize) {
            p
        } else {
            get_target_exe_path()
        }
    } else {
        get_target_exe_path()
    };

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let steam_share_upper = format!("{}/.local/share/Steam", home);
    let steam_share_lower = format!("{}/.local/share/steam", home);
    let steam_prefix_len = if raw_path.starts_with(&steam_share_upper) {
        Some(steam_share_upper.len())
    } else if raw_path.starts_with(&steam_share_lower) {
        Some(steam_share_lower.len())
    } else {
        None
    };

    if let Some(len) = steam_prefix_len {
        let sub = &raw_path[len..];
        format!("C:\\Program Files (x86)\\Steam{}", sub.replace('/', "\\"))
    } else if raw_path.contains(':') {
        raw_path.replace('/', "\\")
    } else if raw_path.starts_with('/') {
        format!("C:{}", raw_path.replace('/', "\\"))
    } else {
        let cwd = get_process_current_directory();
        if cwd.ends_with('\\') {
            format!("{}{}", cwd, raw_path.replace('/', "\\"))
        } else {
            format!("{}\\{}", cwd, raw_path.replace('/', "\\"))
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetVersion() -> u32 {
    (19045 << 16) | 10
}

#[no_mangle]
pub unsafe extern "win64" fn GetVersionExW(lp_version_information: *mut crate::dlls::ntdll::OsVersionInfoExW) -> i32 {
    if lp_version_information.is_null() {
        return 0;
    }
    let status = crate::dlls::ntdll::RtlGetVersion(lp_version_information);
    if status == 0 { 1 } else { 0 }
}

#[repr(C)]
pub struct OsVersionInfoA {
    pub dw_os_version_info_size: u32,
    pub dw_major_version: u32,
    pub dw_minor_version: u32,
    pub dw_build_number: u32,
    pub dw_platform_id: u32,
    pub sz_csd_version: [u8; 128],
}

#[no_mangle]
pub unsafe extern "win64" fn GetVersionExA(lp_version_information: *mut OsVersionInfoA) -> i32 {
    if lp_version_information.is_null() {
        return 0;
    }
    (*lp_version_information).dw_major_version = 10;
    (*lp_version_information).dw_minor_version = 0;
    (*lp_version_information).dw_build_number = 19045;
    (*lp_version_information).dw_platform_id = 2; // VER_PLATFORM_WIN32_NT
    for b in (*lp_version_information).sz_csd_version.iter_mut() {
        *b = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleFileNameW(
    h_module: u64,
    lp_filename: *mut u16,
    n_size: u32,
) -> u32 {
    SetLastError(0);
    if lp_filename.is_null() || n_size == 0 {
        return 0;
    }
    let win_path = resolve_module_path(h_module);
    let wide: Vec<u16> = win_path.encode_utf16().chain(std::iter::once(0)).collect();
    let to_copy = std::cmp::min(n_size as usize, wide.len());
    std::ptr::copy_nonoverlapping(wide.as_ptr(), lp_filename, to_copy);
    println!("[KERNEL32] GetModuleFileNameW(h=0x{:X}) -> '{}'", h_module, win_path);
    (to_copy.saturating_sub(1)) as u32
}

#[no_mangle]
pub unsafe extern "win64" fn GetModuleFileNameA(
    h_module: u64,
    lp_filename: *mut u8,
    n_size: u32,
) -> u32 {
    if lp_filename.is_null() || n_size == 0 {
        return 0;
    }
    let win_path = resolve_module_path(h_module);
    let bytes = win_path.as_bytes();
    let to_copy = std::cmp::min(n_size as usize - 1, bytes.len());
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), lp_filename, to_copy);
    *lp_filename.add(to_copy) = 0;
    println!("[KERNEL32] GetModuleFileNameA(h=0x{:X}) -> '{}'", h_module, win_path);
    to_copy as u32
}

/// Central Rine Logical Drive Registry
#[derive(Debug, Clone)]
pub struct LogicalDriveRegistry {
    drives_mask: u32,
}

impl LogicalDriveRegistry {
    pub const fn new() -> Self {
        // Bit 2 = Drive C: (1 << 2 = 0x04)
        Self {
            drives_mask: 1 << 2,
        }
    }

    pub fn get_drives_mask(&self) -> u32 {
        self.drives_mask
    }

    pub fn is_drive_available(&self, drive_letter: char) -> bool {
        let upper = drive_letter.to_ascii_uppercase();
        if ('A'..='Z').contains(&upper) {
            let bit = (upper as u32) - ('A' as u32);
            (self.drives_mask & (1 << bit)) != 0
        } else {
            false
        }
    }
}

static DRIVE_REGISTRY: std::sync::OnceLock<LogicalDriveRegistry> = std::sync::OnceLock::new();

pub fn get_drive_registry() -> &'static LogicalDriveRegistry {
    DRIVE_REGISTRY.get_or_init(LogicalDriveRegistry::new)
}

#[no_mangle]
pub unsafe extern "win64" fn GetLogicalDrives() -> u32 {
    let mask = get_drive_registry().get_drives_mask();
    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let mut drive_list = Vec::new();
        for i in 0..26 {
            if (mask & (1 << i)) != 0 {
                drive_list.push(format!("{}:", (b'A' + i) as char));
            }
        }
        println!(
            "[RINE FILESYSTEM] GetLogicalDrives -> mask=0x{:08X} drives={:?}",
            mask, drive_list
        );
    }
    mask
}

static PROCESS_CWD: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

pub fn set_process_current_directory(dir: &str) {
    if let Ok(mut guard) = PROCESS_CWD.write() {
        *guard = Some(dir.to_string());
    }
}

pub fn get_process_current_directory() -> String {
    if let Ok(guard) = PROCESS_CWD.read() {
        if let Some(ref s) = *guard {
            return s.clone();
        }
    }
    let host_cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());
    host_cwd.replace('/', "\\")
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentDirectoryW(
    n_buffer_length: u32,
    lp_buffer: *mut u16,
) -> u32 {
    let win_cwd = get_process_current_directory();
    let utf16: Vec<u16> = win_cwd.encode_utf16().collect();
    let len = utf16.len(); // number of WCHARs without terminating null

    // If buffer length is 0 or too small (needs len + 1 for null terminator)
    if n_buffer_length <= (len as u32) || lp_buffer.is_null() {
        return (len + 1) as u32;
    }

    // Buffer is sufficient
    std::ptr::copy_nonoverlapping(utf16.as_ptr(), lp_buffer, len);
    *lp_buffer.add(len) = 0;

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE FILESYSTEM] GetCurrentDirectoryW -> '{}' (len={})",
            win_cwd, len
        );
    }

    len as u32
}

/// Helper to resolve a Win32 path relative to the current working directory
pub fn resolve_win32_path(input_path: &str, current_cwd: &str) -> String {
    let norm = input_path.replace('/', "\\");
    let trimmed = norm.trim();

    if trimmed.is_empty() {
        return current_cwd.to_string();
    }

    // 1. If "." -> return current_cwd
    if trimmed == "." {
        return current_cwd.to_string();
    }

    // 2. Drive absolute vs drive relative: e.g. "C:\foo\bar" vs "C:foo\bar" or "C:.\foo\bar"
    if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
        if trimmed.len() > 2 && (trimmed.as_bytes()[2] == b'\\' || trimmed.as_bytes()[2] == b'/') {
            return normalize_win32_path_components(trimmed);
        } else {
            // Drive-relative path: relative to current_cwd of that drive
            let relative_part = &trimmed[2..];
            let combined = if current_cwd.ends_with('\\') {
                format!("{}{}", current_cwd, relative_part)
            } else {
                format!("{}\\{}", current_cwd, relative_part)
            };
            return normalize_win32_path_components(&combined);
        }
    }

    // 3. Root relative: starts with "\"
    if trimmed.starts_with('\\') {
        if current_cwd.len() >= 2 && current_cwd.as_bytes()[1] == b':' {
            let drive = &current_cwd[..2];
            let combined = format!("{}{}", drive, trimmed);
            return normalize_win32_path_components(&combined);
        }
        return normalize_win32_path_components(trimmed);
    }

    // 4. Normal relative: join with current_cwd
    let combined = if current_cwd.ends_with('\\') {
        format!("{}{}", current_cwd, trimmed)
    } else {
        format!("{}\\{}", current_cwd, trimmed)
    };
    normalize_win32_path_components(&combined)
}

/// Helper to normalize "." and ".." components in a Win32 backslash path
fn normalize_win32_path_components(path: &str) -> String {
    let mut prefix = "";
    let mut remainder = path;

    if remainder.len() >= 2 && remainder.as_bytes()[1] == b':' {
        prefix = &path[..2];
        remainder = &path[2..];
    }

    let is_root = remainder.starts_with('\\');
    let parts: Vec<&str> = remainder.split('\\').filter(|s| !s.is_empty()).collect();
    let mut stack: Vec<&str> = Vec::new();

    for part in parts {
        if part == "." {
            continue;
        } else if part == ".." {
            stack.pop();
        } else {
            stack.push(part);
        }
    }

    let mut result = String::new();
    if !prefix.is_empty() {
        result.push_str(prefix);
    }
    if is_root || prefix.is_empty() {
        result.push('\\');
    }
    result.push_str(&stack.join("\\"));

    if result.is_empty() {
        "\\".to_string()
    } else {
        result
    }
}

pub fn initialize_process_filesystem(exe_path: &str, configured_cwd: Option<&str>) {
    set_target_exe_path(exe_path);

    let initial_dir = if let Some(dir) = configured_cwd {
        dir.to_string()
    } else if let Some(parent) = std::path::Path::new(exe_path).parent() {
        if parent.as_os_str().is_empty() {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        } else {
            parent.to_string_lossy().to_string()
        }
    } else {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    };

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let steam_share = format!("{}/.local/share/Steam", home);
    let guest_win_dir = if initial_dir.starts_with(&steam_share) {
        let sub = &initial_dir[steam_share.len()..];
        format!("C:\\Program Files (x86)\\Steam{}", sub.replace('/', "\\"))
    } else {
        initial_dir.replace('/', "\\")
    };
    set_process_current_directory(&guest_win_dir);
    crate::sys::RineVfs::init(exe_path, configured_cwd, false);
    println!(
        "[RINE FILESYSTEM] Initialized Guest Process CWD -> '{}'",
        guest_win_dir
    );
}

/// Helper to convert a Win32 guest path to a host POSIX path via secure RineVfs
pub fn win32_to_host_path(win_path: &str) -> String {
    let current_cwd = get_process_current_directory();
    let resolved_guest = resolve_win32_path(win_path, &current_cwd);
    let vfs_guard = crate::sys::RineVfs::get();
    vfs_guard.as_ref().unwrap().win32_to_host_path(&resolved_guest)
}

#[no_mangle]
pub unsafe extern "win64" fn SetCurrentDirectoryW(lp_path_name: *const u16) -> i32 {
    if lp_path_name.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0; // FALSE
    }

    let raw_str = match read_utf16_ptr(lp_path_name) {
        Some(s) => s,
        None => {
            SetLastError(87);
            return 0;
        }
    };

    if raw_str.is_empty() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0;
    }

    let current_cwd = get_process_current_directory();
    let resolved_guest = resolve_win32_path(&raw_str, &current_cwd);
    let host_path = win32_to_host_path(&resolved_guest);

    // Validate host directory existence and type via stat
    let c_path = match std::ffi::CString::new(host_path.clone()) {
        Ok(c) => c,
        Err(_) => {
            SetLastError(87);
            return 0;
        }
    };

    let mut st: libc::stat = std::mem::zeroed();
    let res = libc::stat(c_path.as_ptr(), &mut st);
    if res != 0 {
        SetLastError(3); // ERROR_PATH_NOT_FOUND
        return 0;
    }

    if (st.st_mode & libc::S_IFMT) != libc::S_IFDIR {
        SetLastError(267); // ERROR_DIRECTORY (The directory name is invalid)
        return 0;
    }

    // Success: update process CWD
    set_process_current_directory(&resolved_guest);

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE FILESYSTEM] SetCurrentDirectoryW input='{}' resolved_guest='{}' host='{}' -> TRUE",
            raw_str, resolved_guest, host_path
        );
    }

    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDirectoryW(
    lp_path_name: *const u16,
    lp_security_attributes: *const std::ffi::c_void, // [SECURITY ATTRIBUTES NOT EXERCISED]
) -> i32 {
    let _ = lp_security_attributes;

    if lp_path_name.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0; // FALSE
    }

    let raw_str = match read_utf16_ptr(lp_path_name) {
        Some(s) => s,
        None => {
            SetLastError(87);
            return 0;
        }
    };

    if raw_str.is_empty() {
        SetLastError(3); // ERROR_PATH_NOT_FOUND
        return 0;
    }

    let host_path = win32_to_host_path(&raw_str);
    let target_path = std::path::Path::new(&host_path);

    // Root directory always exists
    if host_path == "/" {
        SetLastError(183); // ERROR_ALREADY_EXISTS
        return 0;
    }

    if target_path.exists() {
        SetLastError(183); // ERROR_ALREADY_EXISTS
        return 0;
    }

    match std::fs::create_dir(target_path) {
        Ok(()) => {
            println!("[KERNEL32] CreateDirectoryW('{}' -> '{}') -> OK", raw_str, host_path);
            1
        }
        Err(e) => {
            println!("[KERNEL32] CreateDirectoryW('{}' -> '{}') FAILED: {}", raw_str, host_path, e);
            let win_err = match e.kind() {
                std::io::ErrorKind::AlreadyExists => 183, // ERROR_ALREADY_EXISTS
                std::io::ErrorKind::NotFound => 3,        // ERROR_PATH_NOT_FOUND
                std::io::ErrorKind::PermissionDenied => 5, // ERROR_ACCESS_DENIED
                _ => 82,                                  // ERROR_CANNOT_MAKE
            };
            SetLastError(win_err);
            0 // FALSE
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateDirectoryA(
    lp_path_name: *const i8,
    lp_security_attributes: *const std::ffi::c_void,
) -> i32 {
    if lp_path_name.is_null() {
        SetLastError(87);
        return 0;
    }
    let c_str = std::ffi::CStr::from_ptr(lp_path_name);
    let s = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => {
            SetLastError(87);
            return 0;
        }
    };
    let utf16: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    CreateDirectoryW(utf16.as_ptr(), lp_security_attributes)
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

#[no_mangle]
pub unsafe extern "win64" fn QueryPerformanceCounter(lp_performance_count: *mut i64) -> i32 {
    if !lp_performance_count.is_null() {
        let inst = START_TIME.get_or_init(std::time::Instant::now);
        *lp_performance_count = inst.elapsed().as_nanos() as i64;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn QueryPerformanceFrequency(lp_frequency: *mut i64) -> i32 {
    if !lp_frequency.is_null() {
        *lp_frequency = 1_000_000_000; // 1 GHz
    }
    1 // TRUE
}

thread_local! {
    static THREAD_TLS_MAP: std::cell::RefCell<std::collections::HashMap<u32, usize>> = std::cell::RefCell::new(std::collections::HashMap::new());
    static THREAD_FLS_MAP: std::cell::RefCell<std::collections::HashMap<u32, usize>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

static NEXT_TLS_INDEX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(10);
static NEXT_FLS_INDEX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(10);

#[no_mangle]
pub unsafe extern "win64" fn EncodePointer(ptr: usize) -> usize {
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn DecodePointer(ptr: usize) -> usize {
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn EncodeSystemPointer(ptr: usize) -> usize {
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn DecodeSystemPointer(ptr: usize) -> usize {
    ptr
}

pub unsafe extern "win64" fn TlsAlloc() -> u32 {
    let idx = NEXT_TLS_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    println!("[KERNEL32] TlsAlloc -> TLS Index {}", idx);
    idx
}

thread_local! {
    pub static CACHED_TEB_PTR: std::cell::Cell<*mut crate::pe::teb::Teb64> = const { std::cell::Cell::new(std::ptr::null_mut()) };
}

#[inline(always)]
pub unsafe fn get_teb_ptr() -> *mut crate::pe::teb::Teb64 {
    let cached = CACHED_TEB_PTR.with(|c| c.get());
    if !cached.is_null() && (cached as usize) >= 0x10000 {
        return cached;
    }
    let mut gs_base: u64 = 0;
    if libc::syscall(libc::SYS_arch_prctl, 0x1004, &raw mut gs_base) == 0 && gs_base >= 0x10000 {
        let teb_ptr = gs_base as *mut crate::pe::teb::Teb64;
        CACHED_TEB_PTR.with(|c| c.set(teb_ptr));
        return teb_ptr;
    }
    std::ptr::null_mut()
}

pub unsafe extern "win64" fn TlsGetValue(dw_tls_index: u32) -> usize {
    let teb = get_teb_ptr();
    if !teb.is_null() && (teb as usize) >= 0x10000 {
        (*teb).last_error_value = 0;
        if (dw_tls_index as usize) < 64 {
            let tls_slots = (teb as *const u8).add(0x1480) as *const usize;
            let val = *tls_slots.add(dw_tls_index as usize);
            if val != 0 {
                return val;
            }
        }
    }
    THREAD_LAST_ERROR.with(|cell| cell.set(0));
    let val = THREAD_TLS_MAP.with(|map| *map.borrow().get(&dw_tls_index).unwrap_or(&0));
    val
}

pub unsafe extern "win64" fn TlsSetValue(dw_tls_index: u32, lp_tls_value: usize) -> i32 {
    let teb = get_teb_ptr();
    if !teb.is_null() && (teb as usize) >= 0x10000 {
        (*teb).last_error_value = 0;
        if (dw_tls_index as usize) < 64 {
            let tls_slots = (teb as *mut u8).add(0x1480) as *mut usize;
            *tls_slots.add(dw_tls_index as usize) = lp_tls_value;
        }
    }
    THREAD_LAST_ERROR.with(|cell| cell.set(0));
    THREAD_TLS_MAP.with(|map| {
        map.borrow_mut().insert(dw_tls_index, lp_tls_value);
    });
    1 // TRUE
}

pub unsafe extern "win64" fn TlsFree(dw_tls_index: u32) -> i32 {
    THREAD_TLS_MAP.with(|map| {
        map.borrow_mut().remove(&dw_tls_index);
    });
    SetLastError(0);
    1 // TRUE
}

pub unsafe extern "win64" fn FlsAlloc(_callback: usize) -> u32 {
    let idx = NEXT_FLS_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    println!("[KERNEL32] FlsAlloc -> FLS Index {}", idx);
    idx
}

pub unsafe extern "win64" fn FlsGetValue(dw_fls_index: u32) -> usize {
    let val = THREAD_FLS_MAP.with(|map| *map.borrow().get(&dw_fls_index).unwrap_or(&0));
    SetLastError(0);
    val
}

#[no_mangle]
pub unsafe extern "win64" fn FlsGetValue2(dw_fls_index: u32) -> usize {
    FlsGetValue(dw_fls_index)
}

pub unsafe extern "win64" fn FlsSetValue(dw_fls_index: u32, lp_fls_value: usize) -> i32 {
    THREAD_FLS_MAP.with(|map| {
        map.borrow_mut().insert(dw_fls_index, lp_fls_value);
    });
    SetLastError(0);
    1 // TRUE
}

pub unsafe extern "win64" fn FlsFree(dw_fls_index: u32) -> i32 {
    THREAD_FLS_MAP.with(|map| {
        map.borrow_mut().remove(&dw_fls_index);
    });
    SetLastError(0);
    1 // TRUE
}

static GUEST_COMMAND_LINE: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

pub fn set_guest_command_line(cmd: &str) {
    let mut guard = GUEST_COMMAND_LINE.write().unwrap();
    *guard = Some(cmd.to_string());
}

#[no_mangle]
pub unsafe extern "win64" fn GetCommandLineW() -> u64 {
    static CMDLINE_UTF16: std::sync::Mutex<Option<Vec<u16>>> = std::sync::Mutex::new(None);
    let mut guard = CMDLINE_UTF16.lock().unwrap();
    if guard.is_none() {
        let cmd = if let Some(ref custom_cmd) = *GUEST_COMMAND_LINE.read().unwrap() {
            format!("{}\0", custom_cmd)
        } else {
            let win_path = resolve_module_path(0);
            format!("\"{}\"\0", win_path)
        };
        *guard = Some(cmd.encode_utf16().collect());
    }
    let ptr = guard.as_ref().unwrap().as_ptr() as u64;
    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn GetCommandLineA() -> u64 {
    static CMDLINE_ASCII: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);
    let mut guard = CMDLINE_ASCII.lock().unwrap();
    if guard.is_none() {
        let cmd = if let Some(ref custom_cmd) = *GUEST_COMMAND_LINE.read().unwrap() {
            format!("{}\0", custom_cmd)
        } else {
            let win_path = resolve_module_path(0);
            format!("\"{}\"\0", win_path)
        };
        *guard = Some(cmd.into_bytes());
    }
    let ptr = guard.as_ref().unwrap().as_ptr() as u64;
    ptr
}

#[repr(C)]
pub struct STARTUPINFOW {
    pub cb: u32,
    pub lpReserved: *mut u16,
    pub lpDesktop: *mut u16,
    pub lpTitle: *mut u16,
    pub dwX: u32,
    pub dwY: u32,
    pub dwXSize: u32,
    pub dwYSize: u32,
    pub dwXCountChars: u32,
    pub dwYCountChars: u32,
    pub dwFillAttribute: u32,
    pub dwFlags: u32,
    pub wShowWindow: u16,
    pub cbReserved2: u16,
    pub lpReserved2: *mut u8,
    pub hStdInput: u64,
    pub hStdOutput: u64,
    pub hStdError: u64,
}

#[repr(C)]
pub struct STARTUPINFOA {
    pub cb: u32,
    pub lpReserved: *mut libc::c_char,
    pub lpDesktop: *mut libc::c_char,
    pub lpTitle: *mut libc::c_char,
    pub dwX: u32,
    pub dwY: u32,
    pub dwXSize: u32,
    pub dwYSize: u32,
    pub dwXCountChars: u32,
    pub dwYCountChars: u32,
    pub dwFillAttribute: u32,
    pub dwFlags: u32,
    pub wShowWindow: u16,
    pub cbReserved2: u16,
    pub lpReserved2: *mut u8,
    pub hStdInput: u64,
    pub hStdOutput: u64,
    pub hStdError: u64,
}

#[no_mangle]
pub unsafe extern "win64" fn GetStartupInfoW(startup_info_ptr: u64) {
    if startup_info_ptr != 0 {
        let si = startup_info_ptr as *mut STARTUPINFOW;
        std::ptr::write_bytes(si as *mut u8, 0, std::mem::size_of::<STARTUPINFOW>());
        (*si).cb = std::mem::size_of::<STARTUPINFOW>() as u32; // 104
        (*si).dwFlags = 1; // STARTF_USESHOWWINDOW
        (*si).wShowWindow = 1; // SW_SHOWNORMAL
        (*si).hStdInput = GetStdHandle(-10);
        (*si).hStdOutput = GetStdHandle(-11);
        (*si).hStdError = GetStdHandle(-12);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetStartupInfoA(startup_info_ptr: u64) {
    if startup_info_ptr != 0 {
        let si = startup_info_ptr as *mut STARTUPINFOA;
        std::ptr::write_bytes(si as *mut u8, 0, std::mem::size_of::<STARTUPINFOA>());
        (*si).cb = std::mem::size_of::<STARTUPINFOA>() as u32; // 104
        (*si).dwFlags = 1;
        (*si).wShowWindow = 1;
        (*si).hStdInput = GetStdHandle(-10);
        (*si).hStdOutput = GetStdHandle(-11);
        (*si).hStdError = GetStdHandle(-12);
    }
}

fn generate_environment_block() -> Vec<String> {
    vec![
        "ALLUSERSPROFILE=C:\\ProgramData".to_string(),
        "APPDATA=C:\\Users\\fizzy\\AppData\\Roaming".to_string(),
        "CommonProgramFiles=C:\\Program Files\\Common Files".to_string(),
        "CommonProgramFiles(x86)=C:\\Program Files (x86)\\Common Files".to_string(),
        "CommonProgramW6432=C:\\Program Files\\Common Files".to_string(),
        "COMPUTERNAME=DESKTOP-RINE".to_string(),
        "ComSpec=C:\\Windows\\system32\\cmd.exe".to_string(),
        "HOMEDRIVE=C:".to_string(),
        "HOMEPATH=\\Users\\fizzy".to_string(),
        "LOCALAPPDATA=C:\\Users\\fizzy\\AppData\\Local".to_string(),
        "NUMBER_OF_PROCESSORS=8".to_string(),
        "OS=Windows_NT".to_string(),
        "Path=C:\\Windows\\system32;C:\\Windows;C:\\Windows\\System32\\Wbem".to_string(),
        "PATHEXT=.COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC".to_string(),
        "PROCESSOR_ARCHITECTURE=AMD64".to_string(),
        "PROCESSOR_IDENTIFIER=Intel64 Family 6 Model 158 Stepping 9, GenuineIntel".to_string(),
        "PROCESSOR_LEVEL=6".to_string(),
        "PROCESSOR_REVISION=9e09".to_string(),
        "ProgramData=C:\\ProgramData".to_string(),
        "ProgramFiles=C:\\Program Files".to_string(),
        "ProgramFiles(x86)=C:\\Program Files (x86)".to_string(),
        "ProgramW6432=C:\\Program Files".to_string(),
        "PROMPT=$P$G".to_string(),
        "PUBLIC=C:\\Users\\Public".to_string(),
        "SystemDrive=C:".to_string(),
        "SystemRoot=C:\\Windows".to_string(),
        "TEMP=C:\\Users\\fizzy\\AppData\\Local\\Temp".to_string(),
        "TMP=C:\\Users\\fizzy\\AppData\\Local\\Temp".to_string(),
        "USERDOMAIN=DESKTOP-RINE".to_string(),
        "USERNAME=fizzy".to_string(),
        "USERPROFILE=C:\\Users\\fizzy".to_string(),
        "windir=C:\\Windows".to_string(),
    ]
}

#[no_mangle]
pub unsafe extern "win64" fn GetEnvironmentStringsW() -> u64 {
    static ENV_UTF16: std::sync::Mutex<Option<Vec<u16>>> = std::sync::Mutex::new(None);
    let mut guard = ENV_UTF16.lock().unwrap();
    if guard.is_none() {
        let vars = generate_environment_block();
        let mut buffer = Vec::new();
        for v in vars {
            buffer.extend(v.encode_utf16());
            buffer.push(0);
        }
        buffer.push(0); // double null termination
        *guard = Some(buffer);
    }
    guard.as_ref().unwrap().as_ptr() as u64
}

#[no_mangle]
pub unsafe extern "win64" fn FreeEnvironmentStringsW(_penv: u64) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetEnvironmentStringsA() -> u64 {
    static ENV_ASCII: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);
    let mut guard = ENV_ASCII.lock().unwrap();
    if guard.is_none() {
        let vars = generate_environment_block();
        let mut buffer = Vec::new();
        for v in vars {
            buffer.extend(v.as_bytes());
            buffer.push(0);
        }
        buffer.push(0); // double null termination
        *guard = Some(buffer);
    }
    guard.as_ref().unwrap().as_ptr() as u64
}

#[no_mangle]
pub unsafe extern "win64" fn FreeEnvironmentStringsA(_penv: u64) -> i32 {
    1
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct FILETIME {
    pub dwLowDateTime: u32,
    pub dwHighDateTime: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct WIN32_FILE_ATTRIBUTE_DATA {
    pub dwFileAttributes: u32,
    pub ftCreationTime: FILETIME,
    pub ftLastAccessTime: FILETIME,
    pub ftLastWriteTime: FILETIME,
    pub nFileSizeHigh: u32,
    pub nFileSizeLow: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct WIN32_FIND_DATAW {
    pub dwFileAttributes: u32,
    pub ftCreationTime: FILETIME,
    pub ftLastAccessTime: FILETIME,
    pub ftLastWriteTime: FILETIME,
    pub nFileSizeHigh: u32,
    pub nFileSizeLow: u32,
    pub dwReserved0: u32,
    pub dwReserved1: u32,
    pub cFileName: [u16; 260],
    pub cAlternateFileName: [u16; 14],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct WIN32_FIND_DATAA {
    pub dwFileAttributes: u32,
    pub ftCreationTime: FILETIME,
    pub ftLastAccessTime: FILETIME,
    pub ftLastWriteTime: FILETIME,
    pub nFileSizeHigh: u32,
    pub nFileSizeLow: u32,
    pub dwReserved0: u32,
    pub dwReserved1: u32,
    pub cFileName: [u8; 260],
    pub cAlternateFileName: [u8; 14],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct BY_HANDLE_FILE_INFORMATION {
    pub dwFileAttributes: u32,
    pub ftCreationTime: FILETIME,
    pub ftLastAccessTime: FILETIME,
    pub ftLastWriteTime: FILETIME,
    pub dwVolumeSerialNumber: u32,
    pub nFileSizeHigh: u32,
    pub nFileSizeLow: u32,
    pub nNumberOfLinks: u32,
    pub nFileIndexHigh: u32,
    pub nFileIndexLow: u32,
}

#[derive(Debug, Clone)]
pub struct WinFileMapping {
    pub fd: i32,
    pub size: usize,
    pub protect: u32,
    pub is_anonymous: bool,
}

static FILE_MAPPING_TABLE: Mutex<Option<HashMap<u64, WinFileMapping>>> = Mutex::new(None);
static NEXT_FILE_MAPPING_HANDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x4000);
static MAPPED_VIEWS: Mutex<Option<HashMap<usize, (usize, bool)>>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub struct FindSearchEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

static FIND_SEARCH_TABLE: Mutex<Option<HashMap<u64, Vec<FindSearchEntry>>>> = Mutex::new(None);
static NEXT_FIND_HANDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x6000);

unsafe fn populate_find_data(entry: &FindSearchEntry, lp_find_file_data: *mut WIN32_FIND_DATAW) {
    let attr = if entry.is_dir { 0x10 } else { 0x20 };
    (*lp_find_file_data).dwFileAttributes = attr;
    (*lp_find_file_data).ftCreationTime = FILETIME::default();
    (*lp_find_file_data).ftLastAccessTime = FILETIME::default();
    (*lp_find_file_data).ftLastWriteTime = FILETIME::default();
    (*lp_find_file_data).nFileSizeHigh = (entry.size >> 32) as u32;
    (*lp_find_file_data).nFileSizeLow = (entry.size & 0xFFFFFFFF) as u32;
    (*lp_find_file_data).dwReserved0 = 0;
    (*lp_find_file_data).dwReserved1 = 0;

    let wide: Vec<u16> = entry.name.encode_utf16().take(259).collect();
    for (i, &w) in wide.iter().enumerate() {
        (*lp_find_file_data).cFileName[i] = w;
    }
    (*lp_find_file_data).cFileName[wide.len().min(259)] = 0;
    (*lp_find_file_data).cAlternateFileName[0] = 0;
}

#[no_mangle]
pub unsafe extern "win64" fn CreateFileW(
    lp_file_name: *const u16,
    dw_desired_access: u32,
    _dw_share_mode: u32,
    _lp_security_attributes: *mut u8,
    dw_creation_disposition: u32,
    dw_flags_and_attributes: u32,
    _h_template_file: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = !0usize as *mut std::ffi::c_void;
    const GENERIC_READ: u32 = 0x80000000;
    const GENERIC_WRITE: u32 = 0x40000000;
    const GENERIC_ALL: u32 = 0x10000000;
    const CREATE_NEW: u32 = 1;
    const CREATE_ALWAYS: u32 = 2;
    const OPEN_EXISTING: u32 = 3;
    const OPEN_ALWAYS: u32 = 4;
    const TRUNCATE_EXISTING: u32 = 5;

    if lp_file_name.is_null() {
        SetLastError(123);
        return INVALID_HANDLE_VALUE;
    }

    let mut len = 0usize;
    while *lp_file_name.add(len) != 0 && len < 4096 {
        len += 1;
    }
    let wide = std::slice::from_raw_parts(lp_file_name, len);
    let win_path = String::from_utf16_lossy(wide);

    let norm_upper = win_path
        .trim_start_matches(r"\\?\")
        .trim_start_matches(r"\\.\")
        .to_ascii_uppercase();
    if norm_upper == "CONOUT$" || norm_upper == "CON" {
        SetLastError(0);
        return 8 as *mut std::ffi::c_void; // STD_OUTPUT_HANDLE
    }
    if norm_upper == "CONIN$" {
        SetLastError(0);
        return 4 as *mut std::ffi::c_void; // STD_INPUT_HANDLE
    }

    if win_path.starts_with(r"\\?\") || win_path.starts_with(r"\\.\") {
        let upper = win_path.to_ascii_uppercase();
        if upper.contains("HID") || upper.contains("DISPLAY") || upper.contains("RAW") || upper.contains("KBD") || upper.contains("MOU") {
            let mut fd = libc::open(b"/dev/null\0".as_ptr() as *const libc::c_char, libc::O_RDWR);
            if fd < 0 {
                fd = libc::open(b"/dev/null\0".as_ptr() as *const libc::c_char, libc::O_RDONLY);
            }
            if fd < 0 {
                let mut pipefd = [0i32; 2];
                if libc::pipe(pipefd.as_mut_ptr()) == 0 {
                    libc::close(pipefd[1]);
                    fd = pipefd[0];
                }
            }
            if fd >= 0 {
                register_fd_handle(fd as u64);
                SetLastError(0);
                println!(
                    "[KERNEL32] CreateFileW SUCCEEDED for simulated device: '{}' (fd={})",
                    win_path, fd
                );
                return fd as *mut std::ffi::c_void;
            }
        }
    }
    let linux_path = win_path_to_linux(&win_path);
    let file_existed_before = std::path::Path::new(&linux_path).exists();

    let write_mode = (dw_desired_access & (GENERIC_WRITE | GENERIC_ALL | 0x0002 | 0x0004 | 0x0010 | 0x0100)) != 0
        || dw_creation_disposition == CREATE_ALWAYS
        || dw_creation_disposition == CREATE_NEW
        || dw_creation_disposition == TRUNCATE_EXISTING;
    if write_mode {
        if let Some(parent) = std::path::Path::new(&linux_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
    }

    let is_dir_flag = (dw_flags_and_attributes & 0x02000000) != 0;
    let is_dir = std::path::Path::new(&linux_path).is_dir();

    let flags = if is_dir_flag && is_dir {
        libc::O_RDONLY | libc::O_DIRECTORY
    } else if write_mode {
        match dw_creation_disposition {
            CREATE_ALWAYS => libc::O_RDWR | libc::O_CREAT | libc::O_TRUNC,
            CREATE_NEW => libc::O_RDWR | libc::O_CREAT | libc::O_EXCL,
            OPEN_ALWAYS => libc::O_RDWR | libc::O_CREAT,
            TRUNCATE_EXISTING => libc::O_RDWR | libc::O_TRUNC,
            _ => libc::O_RDWR,
        }
    } else {
        libc::O_RDONLY
    };

    let path_c = match std::ffi::CString::new(linux_path.as_bytes()) {
        Ok(c) => c,
        Err(_) => {
            SetLastError(123);
            return INVALID_HANDLE_VALUE;
        }
    };

    let fd = if win_path.to_lowercase().ends_with("boot.config")
        && (flags & libc::O_ACCMODE) == libc::O_RDONLY
    {
        if let Ok(content) = std::fs::read_to_string(&linux_path) {
            let mut modified = content;
            if modified.contains("gfx-threading-mode=6")
                || modified.contains("gfx-enable-gfx-jobs=1")
            {
                modified = modified.replace("gfx-threading-mode=6", "gfx-threading-mode=1");
                modified = modified.replace("gfx-enable-gfx-jobs=1", "gfx-enable-gfx-jobs=0");
                println!(
                    "[KERNEL32] Sanitized Unity boot.config: forced gfx-threading-mode=1, gfx-enable-gfx-jobs=0"
                );
            }
            let mem_name = std::ffi::CString::new("boot.config").unwrap();
            let mfd = unsafe {
                libc::syscall(libc::SYS_memfd_create, mem_name.as_ptr(), 0) as i32
            };
            if mfd >= 0 {
                let bytes = modified.as_bytes();
                unsafe {
                    libc::write(
                        mfd,
                        bytes.as_ptr() as *const libc::c_void,
                        bytes.len(),
                    );
                    libc::lseek(mfd, 0, libc::SEEK_SET);
                }
                mfd
            } else {
                libc::open(path_c.as_ptr(), flags, 0o644)
            }
        } else {
            libc::open(path_c.as_ptr(), flags, 0o644)
        }
    } else {
        libc::open(path_c.as_ptr(), flags, 0o644)
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        let win_err = match err {
            libc::ENOENT => 2,
            libc::EACCES => 5,
            libc::EEXIST => 80,
            _ => 2,
        };
        SetLastError(win_err);
        println!(
            "[KERNEL32] CreateFileW FAILED: '{}' -> '{}' (flags=0x{:X}, errno={})",
            win_path, linux_path, flags, err
        );
        return INVALID_HANDLE_VALUE;
    }

    register_fd_handle(fd as u64);
    if file_existed_before && (dw_creation_disposition == CREATE_ALWAYS || dw_creation_disposition == OPEN_ALWAYS) {
        SetLastError(183); // ERROR_ALREADY_EXISTS
    } else {
        SetLastError(0);
    }
    println!(
        "[KERNEL32] CreateFileW SUCCEEDED: '{}' -> '{}' (fd={}, access=0x{:X}, disp={}, share=0x{:X}, flags=0x{:X})",
        win_path, linux_path, fd, dw_desired_access, dw_creation_disposition, _dw_share_mode, dw_flags_and_attributes
    );
    fd as *mut std::ffi::c_void
}

#[no_mangle]
pub unsafe extern "win64" fn CreateFileA(
    lp_file_name: *const libc::c_char,
    dw_desired_access: u32,
    dw_share_mode: u32,
    lp_security_attributes: *mut u8,
    dw_creation_disposition: u32,
    dw_flags_and_attributes: u32,
    h_template_file: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    if lp_file_name.is_null() {
        SetLastError(123);
        return !0usize as *mut std::ffi::c_void;
    }
    let win_path = std::ffi::CStr::from_ptr(lp_file_name).to_string_lossy().to_string();
    let wide: Vec<u16> = win_path.encode_utf16().chain(std::iter::once(0)).collect();
    CreateFileW(
        wide.as_ptr(),
        dw_desired_access,
        dw_share_mode,
        lp_security_attributes,
        dw_creation_disposition,
        dw_flags_and_attributes,
        h_template_file,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileAttributesW(lp_file_name: *const u16) -> u32 {
    const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFFFFFF;
    const FILE_ATTRIBUTE_READONLY: u32 = 0x00000001;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
    const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x00000020;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x00000080;

    if lp_file_name.is_null() {
        SetLastError(123);
        return INVALID_FILE_ATTRIBUTES;
    }
    let win_path = match read_utf16_ptr(lp_file_name) {
        Some(s) => s,
        None => {
            SetLastError(123);
            return INVALID_FILE_ATTRIBUTES;
        }
    };
    let host_path = win32_to_host_path(&win_path);
    let path = std::path::Path::new(&host_path);
    if let Ok(meta) = std::fs::metadata(path) {
        let mut attr = 0u32;
        if meta.is_dir() {
            attr |= FILE_ATTRIBUTE_DIRECTORY;
        } else {
            attr |= FILE_ATTRIBUTE_ARCHIVE;
            if meta.permissions().readonly() {
                attr |= FILE_ATTRIBUTE_READONLY;
            }
        }
        if attr == 0 {
            attr = FILE_ATTRIBUTE_NORMAL;
        }
        println!("[KERNEL32] GetFileAttributesW('{}' -> '{}') -> 0x{:X}", win_path, host_path, attr);
        attr
    } else {
        println!("[KERNEL32] GetFileAttributesW('{}' -> '{}') -> NOT FOUND", win_path, host_path);
        SetLastError(2);
        INVALID_FILE_ATTRIBUTES
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileAttributesA(lp_file_name: *const libc::c_char) -> u32 {
    if lp_file_name.is_null() {
        SetLastError(123);
        return 0xFFFFFFFF;
    }
    let win_path = std::ffi::CStr::from_ptr(lp_file_name).to_string_lossy().to_string();
    let wide: Vec<u16> = win_path.encode_utf16().chain(std::iter::once(0)).collect();
    GetFileAttributesW(wide.as_ptr())
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileAttributesExW(
    lp_file_name: *const u16,
    _f_info_level_id: u32,
    lp_file_information: *mut u8,
) -> i32 {
    const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFFFFFF;
    if lp_file_name.is_null() || lp_file_information.is_null() {
        SetLastError(87);
        return 0;
    }
    let attr = GetFileAttributesW(lp_file_name);
    if attr == INVALID_FILE_ATTRIBUTES {
        return 0;
    }
    let win_path = read_utf16_ptr(lp_file_name).unwrap_or_default();
    let host_path = win32_to_host_path(&win_path);
    let (size, ft_write) = if let Ok(m) = std::fs::metadata(&host_path) {
        let sz = m.len();
        let ft = if let Ok(time) = m.modified() {
            let dur = time.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            let intervals = (dur.as_secs() * 10_000_000 + dur.subsec_nanos() as u64 / 100) + 116444736000000000;
            FILETIME {
                dwLowDateTime: (intervals & 0xFFFFFFFF) as u32,
                dwHighDateTime: (intervals >> 32) as u32,
            }
        } else {
            FILETIME::default()
        };
        (sz, ft)
    } else {
        (0, FILETIME::default())
    };

    let data_ptr = lp_file_information as *mut WIN32_FILE_ATTRIBUTE_DATA;
    (*data_ptr).dwFileAttributes = attr;
    (*data_ptr).ftCreationTime = ft_write;
    (*data_ptr).ftLastAccessTime = ft_write;
    (*data_ptr).ftLastWriteTime = ft_write;
    (*data_ptr).nFileSizeHigh = (size >> 32) as u32;
    (*data_ptr).nFileSizeLow = (size & 0xFFFFFFFF) as u32;

    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileAttributesExA(
    lp_file_name: *const libc::c_char,
    f_info_level_id: u32,
    lp_file_information: *mut u8,
) -> i32 {
    if lp_file_name.is_null() || lp_file_information.is_null() {
        SetLastError(87);
        return 0;
    }
    let win_path = std::ffi::CStr::from_ptr(lp_file_name).to_string_lossy().to_string();
    let wide: Vec<u16> = win_path.encode_utf16().chain(std::iter::once(0)).collect();
    GetFileAttributesExW(wide.as_ptr(), f_info_level_id, lp_file_information)
}

#[no_mangle]
pub unsafe extern "win64" fn SetFileAttributesW(_lp_file_name: *const u16, _dw_file_attributes: u32) -> i32 {
    SetLastError(0);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetFileAttributesA(_lp_file_name: *const libc::c_char, _dw_file_attributes: u32) -> i32 {
    SetLastError(0);
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CreateFileMappingW(
    h_file: *mut std::ffi::c_void,
    _lp_attributes: *mut u8,
    fl_protect: u32,
    dw_max_size_high: u32,
    dw_max_size_low: u32,
    lp_name: *const u16,
) -> *mut std::ffi::c_void {
    let name_str = if !lp_name.is_null() {
        read_utf16_ptr(lp_name).unwrap_or_default()
    } else {
        String::new()
    };

    let is_anon = (h_file as usize == !0usize) || (h_file.is_null());
    let (fd, size) = if is_anon {
        let size = (((dw_max_size_high as u64) << 32) | (dw_max_size_low as u64)) as usize;
        (-1, size.max(4096))
    } else {
        let raw_fd = h_file as i64 as i32;
        let mut st: libc::stat = std::mem::zeroed();
        let file_size = if libc::fstat(raw_fd, &mut st) == 0 {
            st.st_size as usize
        } else {
            0
        };
        let specified_size = (((dw_max_size_high as u64) << 32) | (dw_max_size_low as u64)) as usize;
        let size = if specified_size > 0 { specified_size } else { file_size };
        let dup_fd = libc::dup(raw_fd);
        (dup_fd, size)
    };

    let handle = NEXT_FILE_MAPPING_HANDLE.fetch_add(4, std::sync::atomic::Ordering::SeqCst);
    let mapping = WinFileMapping {
        fd,
        size,
        protect: fl_protect,
        is_anonymous: is_anon,
    };
    println!(
        "[KERNEL32] CreateFileMappingW: handle=0x{:X}, fd={}, size={}, name='{}'",
        handle, fd, size, name_str
    );
    let mut g = FILE_MAPPING_TABLE.lock().unwrap();
    g.get_or_insert_with(HashMap::new).insert(handle, mapping);

    handle as *mut std::ffi::c_void
}

#[no_mangle]
pub unsafe extern "win64" fn CreateFileMappingA(
    h_file: *mut std::ffi::c_void,
    lp_attributes: *mut u8,
    fl_protect: u32,
    dw_max_size_high: u32,
    dw_max_size_low: u32,
    _lp_name: *const libc::c_char,
) -> *mut std::ffi::c_void {
    CreateFileMappingW(h_file, lp_attributes, fl_protect, dw_max_size_high, dw_max_size_low, std::ptr::null())
}

#[no_mangle]
pub unsafe extern "win64" fn OpenFileMappingW(
    _dw_desired_access: u32,
    _b_inherit_handle: i32,
    _lp_name: *const u16,
) -> *mut std::ffi::c_void {
    SetLastError(2);
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "win64" fn OpenFileMappingA(
    _dw_desired_access: u32,
    _b_inherit_handle: i32,
    _lp_name: *const libc::c_char,
) -> *mut std::ffi::c_void {
    SetLastError(2);
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "win64" fn MapViewOfFile(
    h_file_mapping_object: *mut std::ffi::c_void,
    dw_desired_access: u32,
    dw_file_offset_high: u32,
    dw_file_offset_low: u32,
    dw_number_of_bytes_to_map: usize,
) -> *mut std::ffi::c_void {
    MapViewOfFileEx(
        h_file_mapping_object,
        dw_desired_access,
        dw_file_offset_high,
        dw_file_offset_low,
        dw_number_of_bytes_to_map,
        std::ptr::null_mut(),
    )
}

#[no_mangle]
pub unsafe extern "win64" fn MapViewOfFileEx(
    h_file_mapping_object: *mut std::ffi::c_void,
    dw_desired_access: u32,
    dw_file_offset_high: u32,
    dw_file_offset_low: u32,
    dw_number_of_bytes_to_map: usize,
    lp_base_address: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    let handle = h_file_mapping_object as u64;
    let mapping = {
        let g = FILE_MAPPING_TABLE.lock().unwrap();
        match g.as_ref().and_then(|t| t.get(&handle).cloned()) {
            Some(m) => m,
            None => {
                SetLastError(6);
                return std::ptr::null_mut();
            }
        }
    };

    let offset = ((dw_file_offset_high as u64) << 32) | (dw_file_offset_low as u64);
    let map_len = if dw_number_of_bytes_to_map == 0 {
        mapping.size.saturating_sub(offset as usize)
    } else {
        dw_number_of_bytes_to_map
    };

    if map_len == 0 {
        SetLastError(87);
        return std::ptr::null_mut();
    }

    let mut prot = libc::PROT_READ;
    if (dw_desired_access & 2) != 0 || (dw_desired_access & 0xF0000) != 0 {
        prot |= libc::PROT_WRITE;
    }
    if (dw_desired_access & 0x20) != 0 {
        prot |= libc::PROT_EXEC;
    }

    let mut flags = if mapping.is_anonymous || mapping.fd < 0 {
        libc::MAP_ANONYMOUS | libc::MAP_PRIVATE
    } else if (dw_desired_access & 1) != 0 {
        libc::MAP_PRIVATE
    } else {
        libc::MAP_SHARED
    };

    if !lp_base_address.is_null() {
        flags |= libc::MAP_FIXED;
    }

    let target_addr = if lp_base_address.is_null() {
        std::ptr::null_mut()
    } else {
        lp_base_address
    };

    let ptr = libc::mmap(
        target_addr,
        map_len,
        prot,
        flags,
        if mapping.fd >= 0 { mapping.fd } else { -1 },
        offset as libc::off_t,
    );

    if ptr == libc::MAP_FAILED {
        let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        eprintln!(
            "[KERNEL32] MapViewOfFile FAILED: handle=0x{:X}, fd={}, len={}, offset={}, errno={}",
            handle, mapping.fd, map_len, offset, err
        );
        SetLastError(8);
        return std::ptr::null_mut();
    }

    println!(
        "[KERNEL32] MapViewOfFile SUCCEEDED: handle=0x{:X}, ptr={:p}, len={}, offset={}",
        handle, ptr, map_len, offset
    );

    let mut views = MAPPED_VIEWS.lock().unwrap();
    views.get_or_insert_with(HashMap::new).insert(ptr as usize, (map_len, mapping.is_anonymous));

    ptr
}

#[no_mangle]
pub unsafe extern "win64" fn UnmapViewOfFile(lp_base_address: *const std::ffi::c_void) -> i32 {
    if lp_base_address.is_null() {
        SetLastError(87);
        return 0;
    }
    let addr = lp_base_address as usize;
    let view_info = {
        let mut views = MAPPED_VIEWS.lock().unwrap();
        views.as_mut().and_then(|m| m.remove(&addr))
    };

    if let Some((len, _)) = view_info {
        libc::munmap(lp_base_address as *mut libc::c_void, len);
        1
    } else {
        SetLastError(6);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn FlushViewOfFile(
    lp_base_address: *const std::ffi::c_void,
    dw_number_of_bytes_to_flush: usize,
) -> i32 {
    if !lp_base_address.is_null() && dw_number_of_bytes_to_flush > 0 {
        libc::msync(lp_base_address as *mut libc::c_void, dw_number_of_bytes_to_flush, libc::MS_SYNC);
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn FindFirstFileW(
    lp_file_name: *const u16,
    lp_find_file_data: *mut WIN32_FIND_DATAW,
) -> *mut std::ffi::c_void {
    FindFirstFileExW(lp_file_name, 0, lp_find_file_data, 0, std::ptr::null_mut(), 0)
}

#[no_mangle]
pub unsafe extern "win64" fn FindFirstFileExW(
    lp_file_name: *const u16,
    _f_info_level_id: u32,
    lp_find_file_data: *mut WIN32_FIND_DATAW,
    _f_search_op: u32,
    _lp_search_filter: *mut std::ffi::c_void,
    _dw_additional_flags: u32,
) -> *mut std::ffi::c_void {
    const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = !0usize as *mut std::ffi::c_void;
    if lp_file_name.is_null() || lp_find_file_data.is_null() {
        SetLastError(87);
        return INVALID_HANDLE_VALUE;
    }

    let win_path = match read_utf16_ptr(lp_file_name) {
        Some(s) => s,
        None => {
            SetLastError(123);
            return INVALID_HANDLE_VALUE;
        }
    };

    println!("[KERNEL32] FindFirstFileW('{}')", win_path);

    let normalized = win_path.replace('\\', "/");
    let (dir_win, pattern) = if let Some(idx) = normalized.rfind('/') {
        (&normalized[..idx], &normalized[idx + 1..])
    } else {
        (".", normalized.as_str())
    };

    let host_dir = win32_to_host_path(dir_win);
    let mut entries = Vec::new();

    let is_wildcard_all = pattern == "*" || pattern == "*.*" || pattern.is_empty();
    if let Ok(read_dir) = std::fs::read_dir(&host_dir) {
        for entry in read_dir.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let matches = if is_wildcard_all {
                true
            } else if pattern.contains('*') {
                let prefix = pattern.split('*').next().unwrap_or("");
                let suffix = pattern.rsplit('*').next().unwrap_or("");
                file_name.to_lowercase().starts_with(&prefix.to_lowercase())
                    && file_name.to_lowercase().ends_with(&suffix.to_lowercase())
            } else {
                file_name.eq_ignore_ascii_case(pattern)
            };

            if matches {
                let meta = entry.metadata().ok();
                let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                entries.push(FindSearchEntry {
                    name: file_name,
                    is_dir,
                    size,
                });
            }
        }
    } else {
        let host_file = win32_to_host_path(&win_path);
        let path = std::path::Path::new(&host_file);
        if let Ok(meta) = std::fs::metadata(path) {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| win_path.clone());
            entries.push(FindSearchEntry {
                name,
                is_dir: meta.is_dir(),
                size: meta.len(),
            });
        }
    }

    if entries.is_empty() {
        SetLastError(2);
        return INVALID_HANDLE_VALUE;
    }

    let first = entries.remove(0);
    populate_find_data(&first, lp_find_file_data);

    let handle = NEXT_FIND_HANDLE.fetch_add(4, std::sync::atomic::Ordering::SeqCst);
    let mut g = FIND_SEARCH_TABLE.lock().unwrap();
    g.get_or_insert_with(HashMap::new).insert(handle, entries);

    SetLastError(0);
    handle as *mut std::ffi::c_void
}

#[no_mangle]
pub unsafe extern "win64" fn FindFirstFileA(
    lp_file_name: *const libc::c_char,
    lp_find_file_data: *mut WIN32_FIND_DATAA,
) -> *mut std::ffi::c_void {
    const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = !0usize as *mut std::ffi::c_void;
    if lp_file_name.is_null() || lp_find_file_data.is_null() {
        SetLastError(87);
        return INVALID_HANDLE_VALUE;
    }
    let win_path = std::ffi::CStr::from_ptr(lp_file_name).to_string_lossy().to_string();
    let wide: Vec<u16> = win_path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut wide_data: WIN32_FIND_DATAW = std::mem::zeroed();
    let h = FindFirstFileW(wide.as_ptr(), &mut wide_data);
    if h == INVALID_HANDLE_VALUE {
        return INVALID_HANDLE_VALUE;
    }
    (*lp_find_file_data).dwFileAttributes = wide_data.dwFileAttributes;
    (*lp_find_file_data).ftCreationTime = wide_data.ftCreationTime;
    (*lp_find_file_data).ftLastAccessTime = wide_data.ftLastAccessTime;
    (*lp_find_file_data).ftLastWriteTime = wide_data.ftLastWriteTime;
    (*lp_find_file_data).nFileSizeHigh = wide_data.nFileSizeHigh;
    (*lp_find_file_data).nFileSizeLow = wide_data.nFileSizeLow;
    let name_str = String::from_utf16_lossy(&wide_data.cFileName);
    let name_trimmed = name_str.trim_matches('\0');
    let bytes = name_trimmed.as_bytes();
    let copy_len = bytes.len().min(259);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), (*lp_find_file_data).cFileName.as_mut_ptr(), copy_len);
    (*lp_find_file_data).cFileName[copy_len] = 0;
    (*lp_find_file_data).cAlternateFileName[0] = 0;
    h
}

#[no_mangle]
pub unsafe extern "win64" fn FindFirstFileExA(
    lp_file_name: *const libc::c_char,
    _f_info_level_id: u32,
    lp_find_file_data: *mut WIN32_FIND_DATAA,
    _f_search_op: u32,
    _lp_search_filter: *mut std::ffi::c_void,
    _dw_additional_flags: u32,
) -> *mut std::ffi::c_void {
    FindFirstFileA(lp_file_name, lp_find_file_data)
}

#[no_mangle]
pub unsafe extern "win64" fn FindNextFileW(
    h_find_file: *mut std::ffi::c_void,
    lp_find_file_data: *mut WIN32_FIND_DATAW,
) -> i32 {
    if h_find_file.is_null() || lp_find_file_data.is_null() {
        SetLastError(87);
        return 0;
    }
    let handle = h_find_file as u64;
    let next_entry = {
        let mut g = FIND_SEARCH_TABLE.lock().unwrap();
        match g.as_mut().and_then(|t| t.get_mut(&handle)) {
            Some(vec) => {
                if !vec.is_empty() {
                    Some(vec.remove(0))
                } else {
                    None
                }
            }
            None => {
                SetLastError(6);
                return 0;
            }
        }
    };

    match next_entry {
        Some(entry) => {
            populate_find_data(&entry, lp_find_file_data);
            SetLastError(0);
            1
        }
        None => {
            SetLastError(18);
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn FindNextFileA(
    h_find_file: *mut std::ffi::c_void,
    lp_find_file_data: *mut WIN32_FIND_DATAA,
) -> i32 {
    if h_find_file.is_null() || lp_find_file_data.is_null() {
        SetLastError(87);
        return 0;
    }
    let mut wide_data: WIN32_FIND_DATAW = std::mem::zeroed();
    let ok = FindNextFileW(h_find_file, &mut wide_data);
    if ok == 0 {
        return 0;
    }
    (*lp_find_file_data).dwFileAttributes = wide_data.dwFileAttributes;
    (*lp_find_file_data).ftCreationTime = wide_data.ftCreationTime;
    (*lp_find_file_data).ftLastAccessTime = wide_data.ftLastAccessTime;
    (*lp_find_file_data).ftLastWriteTime = wide_data.ftLastWriteTime;
    (*lp_find_file_data).nFileSizeHigh = wide_data.nFileSizeHigh;
    (*lp_find_file_data).nFileSizeLow = wide_data.nFileSizeLow;
    let name_str = String::from_utf16_lossy(&wide_data.cFileName);
    let name_trimmed = name_str.trim_matches('\0');
    let bytes = name_trimmed.as_bytes();
    let copy_len = bytes.len().min(259);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), (*lp_find_file_data).cFileName.as_mut_ptr(), copy_len);
    (*lp_find_file_data).cFileName[copy_len] = 0;
    (*lp_find_file_data).cAlternateFileName[0] = 0;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn FindClose(h_find_file: *mut std::ffi::c_void) -> i32 {
    let handle = h_find_file as u64;
    let mut g = FIND_SEARCH_TABLE.lock().unwrap();
    if let Some(t) = g.as_mut() {
        t.remove(&handle);
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileInformationByHandle(
    h_file: *mut std::ffi::c_void,
    lp_file_information: *mut BY_HANDLE_FILE_INFORMATION,
) -> i32 {
    if lp_file_information.is_null() {
        SetLastError(87);
        return 0;
    }
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::fstat(fd, &mut st) == 0 {
            let is_dir = (st.st_mode & libc::S_IFMT) == libc::S_IFDIR;
            let attr = if is_dir { 0x10 } else { 0x20 };
            let size = st.st_size as u64;
            let ino = st.st_ino as u64;

            (*lp_file_information).dwFileAttributes = attr;
            (*lp_file_information).ftCreationTime = FILETIME::default();
            (*lp_file_information).ftLastAccessTime = FILETIME::default();
            (*lp_file_information).ftLastWriteTime = FILETIME::default();
            (*lp_file_information).dwVolumeSerialNumber = 0x12345678;
            (*lp_file_information).nFileSizeHigh = (size >> 32) as u32;
            (*lp_file_information).nFileSizeLow = (size & 0xFFFFFFFF) as u32;
            (*lp_file_information).nNumberOfLinks = st.st_nlink as u32;
            (*lp_file_information).nFileIndexHigh = (ino >> 32) as u32;
            (*lp_file_information).nFileIndexLow = (ino & 0xFFFFFFFF) as u32;

            SetLastError(0);
            return 1;
        }
    }
    SetLastError(6);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileInformationByHandleEx(
    h_file: *mut std::ffi::c_void,
    file_information_class: u32,
    lp_file_information: *mut u8,
    dw_buffer_size: u32,
) -> i32 {
    if lp_file_information.is_null() || dw_buffer_size == 0 {
        SetLastError(87);
        return 0;
    }
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::fstat(fd, &mut st) == 0 {
            if file_information_class == 1 && dw_buffer_size >= 24 {
                let size = st.st_size as u64;
                let is_dir = (st.st_mode & libc::S_IFMT) == libc::S_IFDIR;
                let buf = lp_file_information as *mut u64;
                *buf = size;
                *buf.add(1) = size;
                let links_ptr = buf.add(2) as *mut u32;
                *links_ptr = st.st_nlink as u32;
                let dir_ptr = links_ptr.add(1) as *mut u8;
                *dir_ptr = 0;
                *dir_ptr.add(1) = if is_dir { 1 } else { 0 };
                return 1;
            }
            return 1;
        }
    }
    SetLastError(6);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn DeleteFileW(lp_file_name: *const u16) -> i32 {
    if lp_file_name.is_null() {
        SetLastError(87);
        return 0;
    }
    let win_path = match read_utf16_ptr(lp_file_name) {
        Some(s) => s,
        None => {
            SetLastError(87);
            return 0;
        }
    };
    let host_path = win32_to_host_path(&win_path);
    if let Some(vfs) = crate::sys::RineVfs::get().as_ref() {
        vfs.invalidate_path(&host_path);
    }
    if std::fs::remove_file(&host_path).is_ok() {
        SetLastError(0);
        1
    } else {
        SetLastError(2);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn DeleteFileA(lp_file_name: *const libc::c_char) -> i32 {
    if lp_file_name.is_null() {
        SetLastError(87);
        return 0;
    }
    let win_path = std::ffi::CStr::from_ptr(lp_file_name).to_string_lossy().to_string();
    let wide: Vec<u16> = win_path.encode_utf16().chain(std::iter::once(0)).collect();
    DeleteFileW(wide.as_ptr())
}

#[no_mangle]
pub unsafe extern "win64" fn CopyFileW(
    lp_existing_file_name: *const u16,
    lp_new_file_name: *const u16,
    b_fail_if_exists: i32,
) -> i32 {
    if lp_existing_file_name.is_null() || lp_new_file_name.is_null() {
        SetLastError(87);
        return 0;
    }
    let src = match read_utf16_ptr(lp_existing_file_name) {
        Some(s) => win32_to_host_path(&s),
        None => {
            SetLastError(87);
            return 0;
        }
    };
    let dst = match read_utf16_ptr(lp_new_file_name) {
        Some(s) => win32_to_host_path(&s),
        None => {
            SetLastError(87);
            return 0;
        }
    };
    if b_fail_if_exists != 0 && std::path::Path::new(&dst).exists() {
        SetLastError(80);
        return 0;
    }
    if std::fs::copy(&src, &dst).is_ok() {
        SetLastError(0);
        1
    } else {
        SetLastError(2);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CopyFileA(
    lp_existing_file_name: *const libc::c_char,
    lp_new_file_name: *const libc::c_char,
    b_fail_if_exists: i32,
) -> i32 {
    if lp_existing_file_name.is_null() || lp_new_file_name.is_null() {
        SetLastError(87);
        return 0;
    }
    let src = std::ffi::CStr::from_ptr(lp_existing_file_name).to_string_lossy().to_string();
    let dst = std::ffi::CStr::from_ptr(lp_new_file_name).to_string_lossy().to_string();
    let wide_src: Vec<u16> = src.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_dst: Vec<u16> = dst.encode_utf16().chain(std::iter::once(0)).collect();
    CopyFileW(wide_src.as_ptr(), wide_dst.as_ptr(), b_fail_if_exists)
}

#[no_mangle]
pub unsafe extern "win64" fn MoveFileW(
    lp_existing_file_name: *const u16,
    lp_new_file_name: *const u16,
) -> i32 {
    MoveFileExW(lp_existing_file_name, lp_new_file_name, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn MoveFileA(
    lp_existing_file_name: *const libc::c_char,
    lp_new_file_name: *const libc::c_char,
) -> i32 {
    MoveFileExA(lp_existing_file_name, lp_new_file_name, 0)
}

#[no_mangle]
pub unsafe extern "win64" fn MoveFileExW(
    lp_existing_file_name: *const u16,
    lp_new_file_name: *const u16,
    _dw_flags: u32,
) -> i32 {
    if lp_existing_file_name.is_null() || lp_new_file_name.is_null() {
        SetLastError(87);
        return 0;
    }
    let src = match read_utf16_ptr(lp_existing_file_name) {
        Some(s) => win32_to_host_path(&s),
        None => {
            SetLastError(87);
            return 0;
        }
    };
    let dst = match read_utf16_ptr(lp_new_file_name) {
        Some(s) => win32_to_host_path(&s),
        None => {
            SetLastError(87);
            return 0;
        }
    };
    if let Some(vfs) = crate::sys::RineVfs::get().as_ref() {
        vfs.invalidate_path(&src);
        vfs.invalidate_path(&dst);
    }
    if std::fs::rename(&src, &dst).is_ok() {
        SetLastError(0);
        1
    } else {
        SetLastError(2);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn MoveFileExA(
    lp_existing_file_name: *const libc::c_char,
    lp_new_file_name: *const libc::c_char,
    dw_flags: u32,
) -> i32 {
    if lp_existing_file_name.is_null() || lp_new_file_name.is_null() {
        SetLastError(87);
        return 0;
    }
    let src = std::ffi::CStr::from_ptr(lp_existing_file_name).to_string_lossy().to_string();
    let dst = std::ffi::CStr::from_ptr(lp_new_file_name).to_string_lossy().to_string();
    let wide_src: Vec<u16> = src.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_dst: Vec<u16> = dst.encode_utf16().chain(std::iter::once(0)).collect();
    MoveFileExW(wide_src.as_ptr(), wide_dst.as_ptr(), dw_flags)
}

#[no_mangle]
pub unsafe extern "win64" fn ReplaceFileW(
    lp_replaced_file_name: *const u16,
    lp_replacement_file_name: *const u16,
    _lp_backup_file_name: *const u16,
    _dw_replace_flags: u32,
    _lp_exclude: *mut std::ffi::c_void,
    _lp_reserved: *mut std::ffi::c_void,
) -> i32 {
    MoveFileExW(lp_replacement_file_name, lp_replaced_file_name, 1)
}

#[no_mangle]
pub unsafe extern "win64" fn CreatePipe(
    h_read_pipe: *mut *mut std::ffi::c_void,
    h_write_pipe: *mut *mut std::ffi::c_void,
    _lp_pipe_attributes: *mut u8,
    _n_size: u32,
) -> i32 {
    if h_read_pipe.is_null() || h_write_pipe.is_null() {
        SetLastError(87);
        return 0;
    }
    let mut fds = [0i32; 2];
    if libc::pipe(fds.as_mut_ptr()) == 0 {
        register_fd_handle(fds[0] as u64);
        register_fd_handle(fds[1] as u64);
        *h_read_pipe = fds[0] as i64 as *mut std::ffi::c_void;
        *h_write_pipe = fds[1] as i64 as *mut std::ffi::c_void;
        SetLastError(0);
        1
    } else {
        SetLastError(24);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetEndOfFile(h_file: *mut std::ffi::c_void) -> i32 {
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let curr = libc::lseek(fd, 0, libc::SEEK_CUR);
        if curr >= 0 && libc::ftruncate(fd, curr) == 0 {
            SetLastError(0);
            return 1;
        }
    }
    SetLastError(6);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn FlushFileBuffers(h_file: *mut std::ffi::c_void) -> i32 {
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        libc::fsync(fd);
    }
    1
}

/// Converts a Windows path to a Linux path best-effort
fn win_path_to_linux(win_path: &str) -> String {
    win32_to_host_path(win_path)
}

static FD_TABLE: std::sync::Mutex<Option<std::collections::HashSet<u64>>> =
    std::sync::Mutex::new(None);

fn register_fd_handle(fd: u64) {
    let mut g = FD_TABLE.lock().unwrap();
    g.get_or_insert_with(std::collections::HashSet::new)
        .insert(fd);
}

fn is_fd_handle(handle: u64) -> bool {
    if handle > 0xFFFF {
        return false;
    } // fds are small numbers
    let g = FD_TABLE.lock().unwrap();
    g.as_ref().map(|s| s.contains(&handle)).unwrap_or(false)
}

#[no_mangle]
pub unsafe extern "win64" fn ReadFile(
    h_file: *mut std::ffi::c_void,
    lp_buffer: *mut u8,
    n_number_of_bytes_to_read: u32,
    lp_number_of_bytes_read: *mut u32,
    _lp_overlapped: *mut u8,
) -> i32 {
    let fd = h_file as i64 as i32;
    if fd > 2 && is_fd_handle(fd as u64) && !lp_buffer.is_null() && n_number_of_bytes_to_read > 0 {
        let mut total_read = 0usize;
        let target = n_number_of_bytes_to_read as usize;
        let mut read_err = false;

        while total_read < target {
            let ret = libc::read(
                fd,
                lp_buffer.add(total_read) as *mut libc::c_void,
                target - total_read,
            );
            if ret > 0 {
                total_read += ret as usize;
            } else if ret == 0 {
                // EOF reached naturally
                break;
            } else {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                read_err = true;
                break;
            }
        }

        if read_err && total_read == 0 {
            if !lp_number_of_bytes_read.is_null() {
                *lp_number_of_bytes_read = 0;
            }
            return 0; // FALSE
        }

        if !lp_number_of_bytes_read.is_null() {
            *lp_number_of_bytes_read = total_read as u32;
        }
        return 1; // TRUE
    }
    // fallback: zero fill
    if !lp_buffer.is_null() && n_number_of_bytes_to_read > 0 {
        std::ptr::write_bytes(lp_buffer, 0, n_number_of_bytes_to_read as usize);
    }
    if !lp_number_of_bytes_read.is_null() {
        *lp_number_of_bytes_read = n_number_of_bytes_to_read;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WriteFile(
    h_file: *mut std::ffi::c_void,
    lp_buffer: *const u8,
    n_number_of_bytes_to_write: u32,
    lp_number_of_bytes_written: *mut u32,
    _lp_overlapped: *mut u8,
) -> i32 {
    let fd = h_file as i64 as i32;
    if !lp_buffer.is_null() && n_number_of_bytes_to_write > 0 {
        if fd == 1 || fd == 2 || fd == 4 || fd == 8 || fd == 12 || is_fd_handle(fd as u64) {
            let target_fd = if fd <= 12 { 1 } else { fd };
            let ret = libc::write(
                target_fd,
                lp_buffer as *const libc::c_void,
                n_number_of_bytes_to_write as usize,
            );
            if ret >= 0 {
                if !lp_number_of_bytes_written.is_null() {
                    *lp_number_of_bytes_written = ret as u32;
                }
                return 1; // TRUE
            } else {
                let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                eprintln!("[KERNEL32] WriteFile FAILED target_fd={} bytes={} errno={}", target_fd, n_number_of_bytes_to_write, err);
            }
        }
    }
    if !lp_number_of_bytes_written.is_null() {
        *lp_number_of_bytes_written = n_number_of_bytes_to_write;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileSize(
    h_file: *mut std::ffi::c_void,
    lp_file_size_high: *mut u32,
) -> u32 {
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::fstat(fd, &mut st) == 0 {
            let size = st.st_size as u64;
            if !lp_file_size_high.is_null() {
                *lp_file_size_high = (size >> 32) as u32;
            }
            return size as u32;
        }
    }
    const INVALID_FILE_SIZE: u32 = 0xFFFFFFFF;
    INVALID_FILE_SIZE
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileSizeEx(
    h_file: *mut std::ffi::c_void,
    lp_file_size: *mut i64,
) -> i32 {
    if lp_file_size.is_null() {
        return 0;
    }
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::fstat(fd, &mut st) == 0 {
            *lp_file_size = st.st_size as i64;
            return 1; // TRUE
        }
    }
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn SetFilePointer(
    h_file: *mut std::ffi::c_void,
    l_distance_to_move: i32,
    lp_distance_to_move_high: *mut i32,
    dw_move_method: u32,
) -> u32 {
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let offset: i64 = if !lp_distance_to_move_high.is_null() {
            let high = *lp_distance_to_move_high as i64;
            let low = l_distance_to_move as u32 as u64;
            (high << 32) | (low as i64)
        } else {
            l_distance_to_move as i64
        };
        let whence = match dw_move_method {
            0 => libc::SEEK_SET,
            1 => libc::SEEK_CUR,
            2 => libc::SEEK_END,
            _ => libc::SEEK_SET,
        };
        let new_off = libc::lseek(fd, offset, whence);
        if new_off < 0 {
            return !0u32;
        }
        if !lp_distance_to_move_high.is_null() {
            *lp_distance_to_move_high = (new_off >> 32) as i32;
        }
        return new_off as u32;
    }
    !0u32
}

#[no_mangle]
pub unsafe extern "win64" fn SetFilePointerEx(
    h_file: *mut std::ffi::c_void,
    li_distance_to_move: i64,
    lp_new_file_pointer: *mut i64,
    dw_move_method: u32,
) -> i32 {
    let fd = h_file as i64 as i32;
    if fd >= 0 && is_fd_handle(fd as u64) {
        let whence = match dw_move_method {
            0 => libc::SEEK_SET,
            1 => libc::SEEK_CUR,
            2 => libc::SEEK_END,
            _ => libc::SEEK_SET,
        };
        let new_off = libc::lseek(fd, li_distance_to_move, whence);
        if new_off < 0 {
            return 0;
        }
        if !lp_new_file_pointer.is_null() {
            *lp_new_file_pointer = new_off;
        }
        return 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn WriteConsoleW(
    _h_console_output: u64,
    lp_buffer: *const u16,
    n_number_of_chars_to_write: u32,
    lp_number_of_chars_written: *mut u32,
    _lp_reserved: *mut std::ffi::c_void,
) -> i32 {
    if !lp_buffer.is_null() && n_number_of_chars_to_write > 0 {
        let slice = std::slice::from_raw_parts(lp_buffer, n_number_of_chars_to_write as usize);
        let s = String::from_utf16_lossy(slice);
        eprint!("{}", s);
        if !lp_number_of_chars_written.is_null() {
            *lp_number_of_chars_written = n_number_of_chars_to_write;
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WriteConsoleA(
    _h_console_output: u64,
    lp_buffer: *const u8,
    n_number_of_chars_to_write: u32,
    lp_number_of_chars_written: *mut u32,
    _lp_reserved: *mut std::ffi::c_void,
) -> i32 {
    if !lp_buffer.is_null() && n_number_of_chars_to_write > 0 {
        let slice = std::slice::from_raw_parts(lp_buffer, n_number_of_chars_to_write as usize);
        let s = String::from_utf8_lossy(slice);
        eprint!("{}", s);
        if !lp_number_of_chars_written.is_null() {
            *lp_number_of_chars_written = n_number_of_chars_to_write;
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetStdHandle(std_handle_id: i32) -> u64 {
    match std_handle_id {
        -10 => 0x0000000000000004,
        -11 => 0x0000000000000008,
        -12 => 0x000000000000000C,
        _ => 0xFFFFFFFFFFFFFFFF,
    }
}

static CONSOLE_TEXT_ATTRIBUTES: std::sync::atomic::AtomicU16 =
    std::sync::atomic::AtomicU16::new(0x0007);

pub fn get_console_text_attributes() -> u16 {
    CONSOLE_TEXT_ATTRIBUTES.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn set_console_text_attributes(attr: u16) {
    CONSOLE_TEXT_ATTRIBUTES.store(attr, std::sync::atomic::Ordering::SeqCst);
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmallRect {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsoleScreenBufferInfo {
    pub dw_size: Coord,
    pub dw_cursor_position: Coord,
    pub w_attributes: u16,
    pub sr_window: SmallRect,
    pub dw_maximum_window_size: Coord,
}

#[no_mangle]
pub unsafe extern "win64" fn GetConsoleScreenBufferInfo(
    h_console_output: u64,
    lp_console_screen_buffer_info: *mut ConsoleScreenBufferInfo,
) -> i32 {
    if lp_console_screen_buffer_info.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0; // FALSE
    }

    // Validate handle: 0x8 (stdout) or 0xC (stderr) or 0x4 (stdin)
    if h_console_output != 0x8 && h_console_output != 0xC && h_console_output != 0x4 {
        SetLastError(6); // ERROR_INVALID_HANDLE
        return 0; // FALSE
    }

    let current_attr = get_console_text_attributes();
    *lp_console_screen_buffer_info = ConsoleScreenBufferInfo {
        dw_size: Coord { x: 80, y: 25 },
        dw_cursor_position: Coord { x: 0, y: 0 },
        w_attributes: current_attr,
        sr_window: SmallRect {
            left: 0,
            top: 0,
            right: 79,
            bottom: 24,
        },
        dw_maximum_window_size: Coord { x: 80, y: 25 },
    };

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE CONSOLE] GetConsoleScreenBufferInfo handle=0x{:X} size=(80,25) cursor=(0,0) attributes=0x{:04X} window=[0,0,79,24] max=(80,25) -> TRUE",
            h_console_output, current_attr
        );
    }

    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetConsoleTextAttribute(
    h_console_output: u64,
    w_attributes: u16,
) -> i32 {
    // Validate handle: 0x8 (stdout) or 0xC (stderr) or 0x4 (stdin)
    if h_console_output != 0x8 && h_console_output != 0xC && h_console_output != 0x4 {
        SetLastError(6); // ERROR_INVALID_HANDLE
        return 0; // FALSE
    }

    set_console_text_attributes(w_attributes);

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE CONSOLE] SetConsoleTextAttribute handle=0x{:X} attributes=0x{:04X} -> TRUE",
            h_console_output, w_attributes
        );
    }

    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetFileType(handle: u64) -> u32 {
    const FILE_TYPE_DISK: u32 = 0x0001;
    const FILE_TYPE_CHAR: u32 = 0x0002;
    const FILE_TYPE_PIPE: u32 = 0x0003;

    if handle <= 2 || handle == 4 || handle == 8 || handle == 12 {
        return FILE_TYPE_CHAR;
    }
    let fd = handle as i32;
    if is_fd_handle(handle) {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::fstat(fd, &mut st) == 0 {
            return match st.st_mode & libc::S_IFMT {
                libc::S_IFREG | libc::S_IFDIR => FILE_TYPE_DISK,
                libc::S_IFCHR => FILE_TYPE_CHAR,
                libc::S_IFIFO | libc::S_IFSOCK => FILE_TYPE_PIPE,
                _ => FILE_TYPE_DISK,
            };
        }
    }
    FILE_TYPE_DISK
}

#[no_mangle]
pub unsafe extern "win64" fn GetConsoleMode(_handle: u64, mode_out: *mut u32) -> i32 {
    if !mode_out.is_null() {
        *mode_out = 0x0007;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetConsoleMode(_handle: u64, _mode: u32) -> i32 {
    1
}

#[unsafe(naked)]
pub unsafe extern "win64" fn SetConsoleOutputCP(_w_code_page_id: u32) -> i32 {
    std::arch::naked_asm!("mov eax, 1", "ret");
}

#[unsafe(naked)]
pub unsafe extern "win64" fn AttachConsole(_dw_process_id: u32) -> i32 {
    std::arch::naked_asm!("mov eax, 1", "ret");
}

#[unsafe(naked)]
pub unsafe extern "win64" fn SetConsoleCP(_w_code_page_id: u32) -> i32 {
    std::arch::naked_asm!("mov eax, 1", "ret");
}

pub const PROCESSOR_ARCHITECTURE_INTEL: u16 = 0;
pub const PROCESSOR_ARCHITECTURE_ARM: u16 = 5;
pub const PROCESSOR_ARCHITECTURE_IA64: u16 = 6;
pub const PROCESSOR_ARCHITECTURE_AMD64: u16 = 9;
pub const PROCESSOR_ARCHITECTURE_ARM64: u16 = 12;
pub const PROCESSOR_ARCHITECTURE_UNKNOWN: u16 = 0xFFFF;

pub const PROCESSOR_INTEL_386: u32 = 386;
pub const PROCESSOR_INTEL_486: u32 = 486;
pub const PROCESSOR_INTEL_PENTIUM: u32 = 586;
pub const PROCESSOR_INTEL_IA64: u32 = 2200;
pub const PROCESSOR_AMD_X8664: u32 = 8664;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemInfo {
    pub w_processor_architecture: u16,
    pub w_reserved: u16,
    pub dw_page_size: u32,
    pub lp_minimum_application_address: u64,
    pub lp_maximum_application_address: u64,
    pub dw_active_processor_mask: u64,
    pub dw_number_of_processors: u32,
    pub dw_processor_type: u32,
    pub dw_allocation_granularity: u32,
    pub w_processor_level: u16,
    pub w_processor_revision: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestSystemInfo {
    pub architecture: u16,
    pub page_size: u32,
    pub min_app_address: u64,
    pub max_app_address: u64,
    pub active_processor_mask: u64,
    pub processor_count: u32,
    pub processor_type: u32,
    pub allocation_granularity: u32,
    pub processor_level: u16,
    pub processor_revision: u16,
}

impl GuestSystemInfo {
    pub fn current() -> Self {
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
        let active_mask = if cpu_count >= 64 {
            !0u64
        } else {
            (1u64 << cpu_count) - 1
        };

        GuestSystemInfo {
            architecture: PROCESSOR_ARCHITECTURE_AMD64,
            page_size: crate::sys::PAGE_SIZE as u32, // 4096
            min_app_address: 0x0000000000010000,     // 64 KB
            max_app_address: 0x00007FFFFFFEFFFF,     // Win64 user limit
            active_processor_mask: active_mask,
            processor_count: cpu_count,
            processor_type: PROCESSOR_AMD_X8664,
            allocation_granularity: 65536, // 64 KB [ALPHA VIRTUAL MEMORY GRANULARITY SEMANTICS]
            processor_level: 6,
            processor_revision: 0,
        }
    }

    pub fn to_system_info(&self) -> SystemInfo {
        SystemInfo {
            w_processor_architecture: self.architecture,
            w_reserved: 0,
            dw_page_size: self.page_size,
            lp_minimum_application_address: self.min_app_address,
            lp_maximum_application_address: self.max_app_address,
            dw_active_processor_mask: self.active_processor_mask,
            dw_number_of_processors: self.processor_count,
            dw_processor_type: self.processor_type,
            dw_allocation_granularity: self.allocation_granularity,
            w_processor_level: self.processor_level,
            w_processor_revision: self.processor_revision,
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetNativeSystemInfo(lp_system_info: *mut SystemInfo) {
    if lp_system_info.is_null() {
        return;
    }
    let info = GuestSystemInfo::current().to_system_info();
    *lp_system_info = info;

    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "[RINE SYSTEM] GetNativeSystemInfo: arch=AMD64(9), CPUs={}, mask=0x{:X}, pageSize={}, allocGranularity={}",
            info.dw_number_of_processors,
            info.dw_active_processor_mask,
            info.dw_page_size,
            info.dw_allocation_granularity
        );
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemInfo(lp_system_info: *mut SystemInfo) {
    GetNativeSystemInfo(lp_system_info);
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemTime {
    pub w_year: u16,
    pub w_month: u16,
    pub w_day_of_week: u16,
    pub w_day: u16,
    pub w_hour: u16,
    pub w_minute: u16,
    pub w_second: u16,
    pub w_milliseconds: u16,
}

pub struct WallClock;

impl WallClock {
    pub fn now_local() -> SystemTime {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
            let mut tm = std::mem::zeroed::<libc::tm>();
            // [ALPHA HOST TIMEZONE SEMANTICS: thread-safe reentrant localtime_r]
            libc::localtime_r(&ts.tv_sec, &mut tm);

            SystemTime {
                w_year: (tm.tm_year + 1900) as u16,
                w_month: (tm.tm_mon + 1) as u16,
                w_day_of_week: tm.tm_wday as u16, // 0 = Sunday .. 6 = Saturday
                w_day: tm.tm_mday as u16,
                w_hour: tm.tm_hour as u16,
                w_minute: tm.tm_min as u16,
                w_second: tm.tm_sec as u16,
                w_milliseconds: ((ts.tv_nsec / 1_000_000) % 1000) as u16,
            }
        }
    }

    pub fn now_utc() -> SystemTime {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
            let mut tm = std::mem::zeroed::<libc::tm>();
            libc::gmtime_r(&ts.tv_sec, &mut tm);

            SystemTime {
                w_year: (tm.tm_year + 1900) as u16,
                w_month: (tm.tm_mon + 1) as u16,
                w_day_of_week: tm.tm_wday as u16,
                w_day: tm.tm_mday as u16,
                w_hour: tm.tm_hour as u16,
                w_minute: tm.tm_min as u16,
                w_second: tm.tm_sec as u16,
                w_milliseconds: ((ts.tv_nsec / 1_000_000) % 1000) as u16,
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetLocalTime(lp_system_time: *mut SystemTime) {
    if lp_system_time.is_null() {
        return;
    }
    *lp_system_time = WallClock::now_local();
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemTime(lp_system_time: *mut SystemTime) {
    if lp_system_time.is_null() {
        return;
    }
    *lp_system_time = WallClock::now_utc();
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemTimeAsFileTime(_lp_system_time_as_file_time: *mut u64) {
    // Standard FileTime epoch is Jan 1, 1601 (100-ns intervals)
    if !_lp_system_time_as_file_time.is_null() {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
        // Seconds between 1601-01-01 and 1970-01-01: 11644473600
        let intervals = ((ts.tv_sec as u64 + 11644473600) * 10_000_000) + (ts.tv_nsec as u64 / 100);
        *_lp_system_time_as_file_time = intervals;
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SystemTimeToFileTime(
    lp_system_time: *const SystemTime,
    lp_file_time: *mut u64,
) -> i32 {
    if lp_system_time.is_null() || lp_file_time.is_null() {
        return 0;
    }
    let st = &*lp_system_time;
    let mut tm: libc::tm = std::mem::zeroed();
    tm.tm_year = (st.w_year as libc::c_int) - 1900;
    tm.tm_mon = (st.w_month as libc::c_int) - 1;
    tm.tm_mday = st.w_day as libc::c_int;
    tm.tm_hour = st.w_hour as libc::c_int;
    tm.tm_min = st.w_minute as libc::c_int;
    tm.tm_sec = st.w_second as libc::c_int;
    tm.tm_isdst = -1;
    let t = libc::timegm(&mut tm);
    if t < 0 {
        return 0;
    }
    let intervals = ((t as u64 + 11644473600) * 10_000_000) + (st.w_milliseconds as u64 * 10_000);
    *lp_file_time = intervals;
    1
}

pub const TIME_ZONE_ID_UNKNOWN: u32 = 0;
pub const TIME_ZONE_ID_STANDARD: u32 = 1;
pub const TIME_ZONE_ID_DAYLIGHT: u32 = 2;
pub const TIME_ZONE_ID_INVALID: u32 = 0xFFFFFFFF;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeZoneInformation {
    pub bias: i32,
    pub standard_name: [u16; 32],
    pub standard_date: SystemTime,
    pub standard_bias: i32,
    pub daylight_name: [u16; 32],
    pub daylight_date: SystemTime,
    pub daylight_bias: i32,
}

impl WallClock {
    pub fn get_timezone_info() -> (u32, TimeZoneInformation) {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
            let mut tm = std::mem::zeroed::<libc::tm>();
            libc::localtime_r(&ts.tv_sec, &mut tm);

            // Windows Bias: UTC = Local + Bias => Bias = -(tm_gmtoff / 60)
            let gmtoff_minutes = (tm.tm_gmtoff / 60) as i32;
            let bias = -gmtoff_minutes;

            let mut std_name = [0u16; 32];
            let mut dlt_name = [0u16; 32];

            if !tm.tm_zone.is_null() {
                if let Ok(cstr) = std::ffi::CStr::from_ptr(tm.tm_zone).to_str() {
                    for (i, c) in cstr.encode_utf16().take(31).enumerate() {
                        std_name[i] = c;
                        dlt_name[i] = c;
                    }
                }
            }

            let zero_time = SystemTime {
                w_year: 0,
                w_month: 0,
                w_day_of_week: 0,
                w_day: 0,
                w_hour: 0,
                w_minute: 0,
                w_second: 0,
                w_milliseconds: 0,
            };

            // [ALPHA TIMEZONE TRANSITION SEMANTICS: no transition dates configured]
            let (ret_id, std_bias, dlt_bias) = if tm.tm_isdst > 0 {
                (TIME_ZONE_ID_DAYLIGHT, 0, -60)
            } else if tm.tm_isdst == 0 {
                (TIME_ZONE_ID_STANDARD, 0, -60)
            } else {
                (TIME_ZONE_ID_UNKNOWN, 0, 0)
            };

            let tzi = TimeZoneInformation {
                bias,
                standard_name: std_name,
                standard_date: zero_time,
                standard_bias: std_bias,
                daylight_name: dlt_name,
                daylight_date: zero_time,
                daylight_bias: dlt_bias,
            };

            (ret_id, tzi)
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetTimeZoneInformation(
    lp_time_zone_information: *mut TimeZoneInformation,
) -> u32 {
    if lp_time_zone_information.is_null() {
        return TIME_ZONE_ID_INVALID;
    }
    let (id, info) = WallClock::get_timezone_info();
    *lp_time_zone_information = info;
    id
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestLocale {
    pub lang_id: u16,
    pub lcid: u32,
    pub locale_name: &'static str,
}

impl GuestLocale {
    pub fn from_env_str(host_lang: &str) -> GuestLocale {
        let l = host_lang.to_lowercase();
        if l.starts_with("ru") {
            GuestLocale {
                lang_id: 0x0419, // LANG_RUSSIAN (0x19) | (SUBLANG_RUSSIAN_RUSSIA (0x01) << 10) = 0x0419
                lcid: 0x00000419,
                locale_name: "ru-RU",
            }
        } else if l.starts_with("de") {
            GuestLocale {
                lang_id: 0x0407, // LANG_GERMAN (0x07) | (SUBLANG_GERMAN (0x01) << 10) = 0x0407
                lcid: 0x00000407,
                locale_name: "de-DE",
            }
        } else if l.starts_with("fr") {
            GuestLocale {
                lang_id: 0x040c, // LANG_FRENCH (0x0c) | (SUBLANG_FRENCH (0x01) << 10) = 0x040c
                lcid: 0x0000040c,
                locale_name: "fr-FR",
            }
        } else if l.starts_with("es") {
            GuestLocale {
                lang_id: 0x0c0a, // LANG_SPANISH (0x0a) | (SUBLANG_SPANISH_MODERN (0x03) << 10) = 0x0c0a
                lcid: 0x00000c0a,
                locale_name: "es-ES",
            }
        } else if l.starts_with("zh") {
            GuestLocale {
                lang_id: 0x0804, // LANG_CHINESE (0x04) | (SUBLANG_CHINESE_SIMPLIFIED (0x02) << 10) = 0x0804
                lcid: 0x00000804,
                locale_name: "zh-CN",
            }
        } else if l.starts_with("ja") {
            GuestLocale {
                lang_id: 0x0411, // LANG_JAPANESE (0x11) | (SUBLANG_JAPANESE_JAPAN (0x01) << 10) = 0x0411
                lcid: 0x00000411,
                locale_name: "ja-JP",
            }
        } else {
            // [ALPHA NLS / UI LANGUAGE SEMANTICS: default en-US fallback]
            GuestLocale {
                lang_id: 0x0409, // LANG_ENGLISH (0x09) | (SUBLANG_ENGLISH_US (0x01) << 10) = 0x0409
                lcid: 0x00000409,
                locale_name: "en-US",
            }
        }
    }

    pub fn current() -> GuestLocale {
        static GUEST_LOCALE: std::sync::OnceLock<GuestLocale> = std::sync::OnceLock::new();
        *GUEST_LOCALE.get_or_init(|| {
            // [ALPHA HOST LOCALE INHERITANCE: snapshot host locale at startup]
            let host_lang = std::env::var("LC_ALL")
                .or_else(|_| std::env::var("LC_MESSAGES"))
                .or_else(|_| std::env::var("LANG"))
                .unwrap_or_default();
            GuestLocale::from_env_str(&host_lang)
        })
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserDefaultUILanguage() -> u16 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetUserDefaultUILanguage -> 0x{:04X} ({})",
        loc.lang_id, loc.locale_name
    );
    loc.lang_id
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDefaultUILanguage() -> u16 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetSystemDefaultUILanguage -> 0x{:04X} ({})",
        loc.lang_id, loc.locale_name
    );
    loc.lang_id
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserDefaultLangID() -> u16 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetUserDefaultLangID -> 0x{:04X} ({})",
        loc.lang_id, loc.locale_name
    );
    loc.lang_id
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDefaultLangID() -> u16 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetSystemDefaultLangID -> 0x{:04X} ({})",
        loc.lang_id, loc.locale_name
    );
    loc.lang_id
}

#[no_mangle]
pub unsafe extern "win64" fn GetLocaleInfoEx(
    _lp_locale_name: *const u16,
    _lc_type: u32,
    lp_lc_data: *mut u16,
    cch_data: i32,
) -> i32 {
    let loc = GuestLocale::current();
    let s: Vec<u16> = format!("{}\0", loc.locale_name).encode_utf16().collect();
    if cch_data <= 0 || lp_lc_data.is_null() {
        return s.len() as i32;
    }
    let copy_len = s.len().min(cch_data as usize);
    std::ptr::copy_nonoverlapping(s.as_ptr(), lp_lc_data, copy_len);
    copy_len as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetLocaleInfoW(
    _lcid: u32,
    _lc_type: u32,
    lp_lc_data: *mut u16,
    cch_data: i32,
) -> i32 {
    GetLocaleInfoEx(std::ptr::null(), _lc_type, lp_lc_data, cch_data)
}

#[no_mangle]
pub unsafe extern "win64" fn GetLocaleInfoA(
    _lcid: u32,
    _lc_type: u32,
    lp_lc_data: *mut libc::c_char,
    cch_data: i32,
) -> i32 {
    let loc = GuestLocale::current();
    let s = format!("{}\0", loc.locale_name);
    if cch_data <= 0 || lp_lc_data.is_null() {
        return s.len() as i32;
    }
    let copy_len = s.len().min(cch_data as usize);
    std::ptr::copy_nonoverlapping(s.as_ptr() as *const libc::c_char, lp_lc_data, copy_len);
    copy_len as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserDefaultLCID() -> u32 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetUserDefaultLCID -> 0x{:08X} ({})",
        loc.lcid, loc.locale_name
    );
    loc.lcid
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDefaultLCID() -> u32 {
    let loc = GuestLocale::current();
    println!(
        "[KERNEL32] GetSystemDefaultLCID -> 0x{:08X} ({})",
        loc.lcid, loc.locale_name
    );
    loc.lcid
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserDefaultLocaleName(
    lp_locale_name: *mut u16,
    cch_locale_name: i32,
) -> i32 {
    let loc = GuestLocale::current();
    let utf16: Vec<u16> = loc
        .locale_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    if lp_locale_name.is_null() || cch_locale_name <= 0 {
        return utf16.len() as i32;
    }
    if (cch_locale_name as usize) < utf16.len() {
        SetLastError(122); // ERROR_INSUFFICIENT_BUFFER
        return 0;
    }
    std::ptr::copy_nonoverlapping(utf16.as_ptr(), lp_locale_name, utf16.len());
    println!(
        "[KERNEL32] GetUserDefaultLocaleName -> '{}'",
        loc.locale_name
    );
    utf16.len() as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDefaultLocaleName(
    lp_locale_name: *mut u16,
    cch_locale_name: i32,
) -> i32 {
    GetUserDefaultLocaleName(lp_locale_name, cch_locale_name)
}

pub const ERROR_ENVVAR_NOT_FOUND: u32 = 203;

pub struct GuestEnvironment;

static GUEST_ENV: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<String, String>>,
> = std::sync::OnceLock::new();

fn get_guest_env() -> &'static std::sync::RwLock<std::collections::HashMap<String, String>> {
    GUEST_ENV.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        // [ALPHA HOST ENVIRONMENT INHERITANCE: snapshot host environment at startup]
        for (k, v) in std::env::vars() {
            map.insert(k.to_uppercase(), v);
        }
        std::sync::RwLock::new(map)
    })
}

impl GuestEnvironment {
    pub fn get(name: &str) -> Option<String> {
        let env = get_guest_env().read().unwrap();
        env.get(&name.to_uppercase()).cloned()
    }

    pub fn set(name: &str, value: Option<&str>) {
        let mut env = get_guest_env().write().unwrap();
        if let Some(v) = value {
            env.insert(name.to_uppercase(), v.to_string());
        } else {
            env.remove(&name.to_uppercase());
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetEnvironmentVariableW(
    lp_name: *const u16,
    lp_buffer: *mut u16,
    n_size: u32,
) -> u32 {
    if lp_name.is_null() {
        SetLastError(ERROR_ENVVAR_NOT_FOUND);
        return 0;
    }

    let mut name_utf16 = Vec::new();
    let mut ptr = lp_name;
    while *ptr != 0 {
        name_utf16.push(*ptr);
        ptr = ptr.add(1);
        if name_utf16.len() > 32768 {
            break;
        }
    }

    let name = match String::from_utf16(&name_utf16) {
        Ok(s) => s,
        Err(_) => {
            SetLastError(ERROR_ENVVAR_NOT_FOUND);
            return 0;
        }
    };

    if let Some(val) = GuestEnvironment::get(&name) {
        let val_utf16: Vec<u16> = val.encode_utf16().collect();
        let val_len = val_utf16.len() as u32;

        if n_size > val_len {
            if !lp_buffer.is_null() {
                for (i, &c) in val_utf16.iter().enumerate() {
                    *lp_buffer.add(i) = c;
                }
                *lp_buffer.add(val_len as usize) = 0;
            }
            val_len
        } else {
            val_len + 1
        }
    } else {
        SetLastError(ERROR_ENVVAR_NOT_FOUND);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetEnvironmentVariableA(
    lp_name: *const libc::c_char,
    lp_buffer: *mut libc::c_char,
    n_size: u32,
) -> u32 {
    if lp_name.is_null() {
        SetLastError(ERROR_ENVVAR_NOT_FOUND);
        return 0;
    }

    let name = match std::ffi::CStr::from_ptr(lp_name).to_str() {
        Ok(s) => s,
        Err(_) => {
            SetLastError(ERROR_ENVVAR_NOT_FOUND);
            return 0;
        }
    };

    if let Some(val) = GuestEnvironment::get(name) {
        let bytes = val.as_bytes();
        let val_len = bytes.len() as u32;

        if n_size > val_len {
            if !lp_buffer.is_null() {
                for (i, &b) in bytes.iter().enumerate() {
                    *lp_buffer.add(i) = b as libc::c_char;
                }
                *lp_buffer.add(val_len as usize) = 0;
            }
            val_len
        } else {
            val_len + 1
        }
    } else {
        SetLastError(ERROR_ENVVAR_NOT_FOUND);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn SetEnvironmentVariableW(
    lp_name: *const u16,
    lp_value: *const u16,
) -> i32 {
    if lp_name.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0;
    }
    let mut name_utf16 = Vec::new();
    let mut ptr = lp_name;
    while *ptr != 0 {
        name_utf16.push(*ptr);
        ptr = ptr.add(1);
    }
    let name = match String::from_utf16(&name_utf16) {
        Ok(s) => s,
        Err(_) => {
            SetLastError(87);
            return 0;
        }
    };

    let value = if lp_value.is_null() {
        None
    } else {
        let mut val_utf16 = Vec::new();
        let mut vptr = lp_value;
        while *vptr != 0 {
            val_utf16.push(*vptr);
            vptr = vptr.add(1);
        }
        match String::from_utf16(&val_utf16) {
            Ok(s) => Some(s),
            Err(_) => {
                SetLastError(87);
                return 0;
            }
        }
    };

    GuestEnvironment::set(&name, value.as_deref());
    if let Some(ref v) = value {
        std::env::set_var(&name, v);
    } else {
        std::env::remove_var(&name);
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetEnvironmentVariableA(
    lp_name: *const libc::c_char,
    lp_value: *const libc::c_char,
) -> i32 {
    if lp_name.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0;
    }
    let name = match std::ffi::CStr::from_ptr(lp_name).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            SetLastError(87);
            return 0;
        }
    };
    let value = if lp_value.is_null() {
        None
    } else {
        match std::ffi::CStr::from_ptr(lp_value).to_str() {
            Ok(s) => Some(s.to_string()),
            Err(_) => {
                SetLastError(87);
                return 0;
            }
        }
    };
    GuestEnvironment::set(&name, value.as_deref());
    if let Some(ref v) = value {
        std::env::set_var(&name, v);
    } else {
        std::env::remove_var(&name);
    }
    1
}

pub const POWER_REQUEST_CONTEXT_VERSION: u32 = 0;
pub const POWER_REQUEST_CONTEXT_SIMPLE_STRING: u32 = 1;
pub const POWER_REQUEST_CONTEXT_DETAILED_STRING: u32 = 2;

pub const POWER_REQUEST_DISPLAY_REQUIRED: u32 = 0;
pub const POWER_REQUEST_SYSTEM_REQUIRED: u32 = 1;
pub const POWER_REQUEST_AWAY_MODE_REQUIRED: u32 = 2;
pub const POWER_REQUEST_EXECUTION_REQUIRED: u32 = 3;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReasonContextDetailed {
    pub localized_reason_module: u64,
    pub localized_reason_id: u32,
    pub reason_string_count: u32,
    pub reason_strings: *mut *mut u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ReasonContextUnion {
    pub detailed: ReasonContextDetailed,
    pub simple_reason_string: *const u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ReasonContext {
    pub version: u32,
    pub flags: u32,
    pub reason: ReasonContextUnion,
}

#[derive(Debug, Clone)]
pub struct PowerRequestObject {
    pub reason: String,
    pub display_required: u32,
    pub system_required: u32,
    pub away_mode_required: u32,
    pub execution_required: u32,
}

static POWER_REQUEST_TABLE: std::sync::Mutex<
    Option<std::collections::HashMap<u64, std::sync::Arc<std::sync::Mutex<PowerRequestObject>>>>,
> = std::sync::Mutex::new(None);

#[no_mangle]
pub unsafe extern "win64" fn PowerCreateRequest(context: *const ReasonContext) -> u64 {
    if context.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0xFFFFFFFFFFFFFFFFu64; // INVALID_HANDLE_VALUE
    }

    let version = (*context).version;
    let flags = (*context).flags;

    if version != POWER_REQUEST_CONTEXT_VERSION {
        SetLastError(87);
        return 0xFFFFFFFFFFFFFFFFu64;
    }

    let reason_str = if flags & POWER_REQUEST_CONTEXT_SIMPLE_STRING != 0 {
        let ptr = (*context).reason.simple_reason_string;
        if !ptr.is_null() {
            let mut u16_vec = Vec::new();
            let mut p = ptr;
            while *p != 0 {
                u16_vec.push(*p);
                p = p.add(1);
                if u16_vec.len() > 1024 {
                    break;
                }
            }
            String::from_utf16_lossy(&u16_vec)
        } else {
            String::new()
        }
    } else {
        String::from("[DETAILED_REASON]")
    };

    let obj = std::sync::Arc::new(std::sync::Mutex::new(PowerRequestObject {
        reason: reason_str.clone(),
        display_required: 0,
        system_required: 0,
        away_mode_required: 0,
        execution_required: 0,
    }));

    let handle = next_handle();
    let mut g = POWER_REQUEST_TABLE.lock().unwrap();
    g.get_or_insert_with(std::collections::HashMap::new)
        .insert(handle, obj);

    println!(
        "[RINE POWER] PowerCreateRequest: allocated handle=0x{:X} reason='{}'",
        handle, reason_str
    );
    handle
}

#[no_mangle]
pub unsafe extern "win64" fn PowerSetRequest(power_request: u64, request_type: u32) -> i32 {
    let g = POWER_REQUEST_TABLE.lock().unwrap();
    if let Some(table) = g.as_ref() {
        if let Some(arc_obj) = table.get(&power_request) {
            let mut obj = arc_obj.lock().unwrap();
            match request_type {
                POWER_REQUEST_DISPLAY_REQUIRED => {
                    obj.display_required = obj.display_required.saturating_add(1);
                }
                POWER_REQUEST_SYSTEM_REQUIRED => {
                    obj.system_required = obj.system_required.saturating_add(1);
                }
                POWER_REQUEST_AWAY_MODE_REQUIRED => {
                    obj.away_mode_required = obj.away_mode_required.saturating_add(1);
                }
                POWER_REQUEST_EXECUTION_REQUIRED => {
                    obj.execution_required = obj.execution_required.saturating_add(1);
                }
                _ => {
                    SetLastError(87);
                    return 0;
                }
            }
            // [ALPHA POWER MANAGEMENT BACKEND: host power inhibition not implemented]
            return 1;
        }
    }
    SetLastError(6); // ERROR_INVALID_HANDLE
    0
}

#[no_mangle]
pub unsafe extern "win64" fn PowerClearRequest(power_request: u64, request_type: u32) -> i32 {
    let g = POWER_REQUEST_TABLE.lock().unwrap();
    if let Some(table) = g.as_ref() {
        if let Some(arc_obj) = table.get(&power_request) {
            let mut obj = arc_obj.lock().unwrap();
            match request_type {
                POWER_REQUEST_DISPLAY_REQUIRED => {
                    obj.display_required = obj.display_required.saturating_sub(1);
                }
                POWER_REQUEST_SYSTEM_REQUIRED => {
                    obj.system_required = obj.system_required.saturating_sub(1);
                }
                POWER_REQUEST_AWAY_MODE_REQUIRED => {
                    obj.away_mode_required = obj.away_mode_required.saturating_sub(1);
                }
                POWER_REQUEST_EXECUTION_REQUIRED => {
                    obj.execution_required = obj.execution_required.saturating_sub(1);
                }
                _ => {
                    SetLastError(87);
                    return 0;
                }
            }
            return 1;
        }
    }
    SetLastError(6); // ERROR_INVALID_HANDLE
    0
}

#[no_mangle]
pub unsafe extern "win64" fn FindResourceW(_module: u64, _name: u64, _type: u64) -> u64 {
    let base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    base + 0x043A9000
}

#[no_mangle]
pub unsafe extern "win64" fn FindResourceA(module: u64, name: u64, rtype: u64) -> u64 {
    FindResourceW(module, name, rtype)
}

#[no_mangle]
pub unsafe extern "win64" fn LoadResource(_module: u64, resource_handle: u64) -> u64 {
    resource_handle
}

#[no_mangle]
pub unsafe extern "win64" fn LockResource(resource_data_handle: u64) -> u64 {
    resource_data_handle
}

#[no_mangle]
pub unsafe extern "win64" fn SizeofResource(_module: u64, _resource_handle: u64) -> u32 {
    4096
}

pub struct ThreadEntry {
    pub thread_id: u32,
    pub host_tid: AtomicU64,
    pub exit_code: AtomicU32,
    pub is_terminated: AtomicBool,
    pub stack_base: AtomicU64,
    pub stack_limit: AtomicU64,
    pub last_rsp: AtomicU64,
    pub last_rbp: AtomicU64,
    pub last_rax: AtomicU64,
    pub last_rbx: AtomicU64,
    pub last_rcx: AtomicU64,
    pub last_rdx: AtomicU64,
    pub last_rsi: AtomicU64,
    pub last_rdi: AtomicU64,
    pub last_r8: AtomicU64,
    pub last_r9: AtomicU64,
    pub last_r10: AtomicU64,
    pub last_r11: AtomicU64,
    pub last_r12: AtomicU64,
    pub last_r13: AtomicU64,
    pub last_r14: AtomicU64,
    pub last_r15: AtomicU64,
    pub last_rip: AtomicU64,
    pub sync: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    pub suspended_sync: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    pub suspend_count: AtomicI32,
    pub suspend_ack: AtomicBool,
}

pub fn on_thread_suspended_signal(
    rsp: u64,
    rbp: u64,
    rip: u64,
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
) {
    let my_host_tid = unsafe { libc::syscall(libc::SYS_gettid) } as u64;
    CURRENT_THREAD_ENTRY.with(|cell| {
        let entry_opt = cell.try_borrow().ok().and_then(|b| b.clone()).or_else(|| {
            // In signal handler context, NEVER use blocking lock() on ALL_ACTIVE_THREADS
            // to avoid self-deadlocks or lock inversion deadlocks.
            if let Ok(guard) = ALL_ACTIVE_THREADS.try_lock() {
                guard
                    .iter()
                    .find(|e| e.host_tid.load(Ordering::Relaxed) == my_host_tid)
                    .cloned()
            } else {
                None
            }
        });

        if let Some(entry) = entry_opt {
            if let Ok(mut cell_ref) = cell.try_borrow_mut() {
                if cell_ref.is_none() {
                    *cell_ref = Some(entry.clone());
                }
            }
            entry.last_rsp.store(rsp, Ordering::SeqCst);
            entry.last_rbp.store(rbp, Ordering::SeqCst);
            entry.last_rip.store(rip, Ordering::SeqCst);
            entry.last_rax.store(rax, Ordering::SeqCst);
            entry.last_rbx.store(rbx, Ordering::SeqCst);
            entry.last_rcx.store(rcx, Ordering::SeqCst);
            entry.last_rdx.store(rdx, Ordering::SeqCst);
            entry.last_rsi.store(rsi, Ordering::SeqCst);
            entry.last_rdi.store(rdi, Ordering::SeqCst);
            entry.last_r8.store(r8, Ordering::SeqCst);
            entry.last_r9.store(r9, Ordering::SeqCst);
            entry.last_r10.store(r10, Ordering::SeqCst);
            entry.last_r11.store(r11, Ordering::SeqCst);
            entry.last_r12.store(r12, Ordering::SeqCst);
            entry.last_r13.store(r13, Ordering::SeqCst);
            entry.last_r14.store(r14, Ordering::SeqCst);
            entry.last_r15.store(r15, Ordering::SeqCst);

            crate::sys::diagnostics::record_thread_state(crate::sys::diagnostics::ThreadExecutionState::Suspended);
            crate::sys::diagnostics::record_thread_registers(rip, rsp);

            entry.suspend_ack.store(true, Ordering::SeqCst);

            // Wait until caller decrements suspend_count to <= 0
            while entry.suspend_count.load(Ordering::SeqCst) > 0 {
                let req = libc::timespec { tv_sec: 0, tv_nsec: 20_000 };
                unsafe {
                    libc::nanosleep(&req, std::ptr::null_mut());
                }
            }
            entry.suspend_ack.store(false, Ordering::SeqCst);
            crate::sys::diagnostics::record_thread_state(crate::sys::diagnostics::ThreadExecutionState::Running);
        }
    });
}

#[inline(always)]
pub fn update_current_thread_stack_context() {
    let mut cur_rsp: u64 = 0;
    let mut cur_rbp: u64 = 0;
    let mut cur_rax: u64 = 0;
    let mut cur_rbx: u64 = 0;
    let mut cur_rcx: u64 = 0;
    let mut cur_rdx: u64 = 0;
    let mut cur_rsi: u64 = 0;
    let mut cur_rdi: u64 = 0;
    let mut cur_r8: u64 = 0;
    let mut cur_r9: u64 = 0;
    let mut cur_r10: u64 = 0;
    let mut cur_r11: u64 = 0;
    let mut cur_r12: u64 = 0;
    let mut cur_r13: u64 = 0;
    let mut cur_r14: u64 = 0;
    let mut cur_r15: u64 = 0;
    unsafe {
        std::arch::asm!("mov {}, rsp", out(reg) cur_rsp);
        std::arch::asm!("mov {}, rbp", out(reg) cur_rbp);
        std::arch::asm!("mov {}, rax", out(reg) cur_rax);
        std::arch::asm!("mov {}, rbx", out(reg) cur_rbx);
        std::arch::asm!("mov {}, rcx", out(reg) cur_rcx);
        std::arch::asm!("mov {}, rdx", out(reg) cur_rdx);
        std::arch::asm!("mov {}, rsi", out(reg) cur_rsi);
        std::arch::asm!("mov {}, rdi", out(reg) cur_rdi);
        std::arch::asm!("mov {}, r8", out(reg) cur_r8);
        std::arch::asm!("mov {}, r9", out(reg) cur_r9);
        std::arch::asm!("mov {}, r10", out(reg) cur_r10);
        std::arch::asm!("mov {}, r11", out(reg) cur_r11);
        std::arch::asm!("mov {}, r12", out(reg) cur_r12);
        std::arch::asm!("mov {}, r13", out(reg) cur_r13);
        std::arch::asm!("mov {}, r14", out(reg) cur_r14);
        std::arch::asm!("mov {}, r15", out(reg) cur_r15);
    }
    CURRENT_THREAD_ENTRY.with(|cell| {
        if let Some(entry) = cell.borrow().as_ref() {
            entry.last_rsp.store(cur_rsp, Ordering::Relaxed);
            entry.last_rbp.store(cur_rbp, Ordering::Relaxed);
            entry.last_rax.store(cur_rax, Ordering::Relaxed);
            entry.last_rbx.store(cur_rbx, Ordering::Relaxed);
            entry.last_rcx.store(cur_rcx, Ordering::Relaxed);
            entry.last_rdx.store(cur_rdx, Ordering::Relaxed);
            entry.last_rsi.store(cur_rsi, Ordering::Relaxed);
            entry.last_rdi.store(cur_rdi, Ordering::Relaxed);
            entry.last_r8.store(cur_r8, Ordering::Relaxed);
            entry.last_r9.store(cur_r9, Ordering::Relaxed);
            entry.last_r10.store(cur_r10, Ordering::Relaxed);
            entry.last_r11.store(cur_r11, Ordering::Relaxed);
            entry.last_r12.store(cur_r12, Ordering::Relaxed);
            entry.last_r13.store(cur_r13, Ordering::Relaxed);
            entry.last_r14.store(cur_r14, Ordering::Relaxed);
            entry.last_r15.store(cur_r15, Ordering::Relaxed);
        }
    });
}

thread_local! {
    pub static CURRENT_THREAD_ENTRY: std::cell::RefCell<Option<std::sync::Arc<ThreadEntry>>> = const { std::cell::RefCell::new(None) };
}

static ALL_ACTIVE_THREADS: std::sync::Mutex<Vec<std::sync::Arc<ThreadEntry>>> =
    std::sync::Mutex::new(Vec::new());

static THREAD_HANDLE_TABLE: std::sync::Mutex<Option<HashMap<u64, std::sync::Arc<ThreadEntry>>>> =
    std::sync::Mutex::new(None);

pub fn ensure_main_thread_entry() -> std::sync::Arc<ThreadEntry> {
    CURRENT_THREAD_ENTRY.with(|cell| {
        if let Some(entry) = cell.borrow().as_ref() {
            return entry.clone();
        }
        let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
        let table = guard.get_or_insert_with(HashMap::new);

        let pid = unsafe { libc::getpid() as u32 };
        let host_tid = unsafe { libc::syscall(libc::SYS_gettid) as u64 };
        let main_handle = (pid as u64) << 4 | 0x3;

        let entry = if let Some(e) = table.get(&main_handle) {
            e.clone()
        } else {
            let (stack_limit, stack_base) = crate::sys::get_current_thread_stack_bounds();
            let sync = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
            let suspended_sync = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
            let new_entry = std::sync::Arc::new(ThreadEntry {
                thread_id: pid,
                host_tid: AtomicU64::new(host_tid),
                exit_code: AtomicU32::new(259),
                is_terminated: AtomicBool::new(false),
                stack_base: AtomicU64::new(stack_base),
                stack_limit: AtomicU64::new(stack_limit),
                last_rsp: AtomicU64::new(0),
                last_rbp: AtomicU64::new(0),
                last_rax: AtomicU64::new(0),
                last_rbx: AtomicU64::new(0),
                last_rcx: AtomicU64::new(0),
                last_rdx: AtomicU64::new(0),
                last_rsi: AtomicU64::new(0),
                last_rdi: AtomicU64::new(0),
                last_r8: AtomicU64::new(0),
                last_r9: AtomicU64::new(0),
                last_r10: AtomicU64::new(0),
                last_r11: AtomicU64::new(0),
                last_r12: AtomicU64::new(0),
                last_r13: AtomicU64::new(0),
                last_r14: AtomicU64::new(0),
                last_r15: AtomicU64::new(0),
                last_rip: AtomicU64::new(0),
                sync,
                suspended_sync,
                suspend_count: AtomicI32::new(0),
                suspend_ack: AtomicBool::new(false),
            });
            table.insert(main_handle, new_entry.clone());
            if host_tid as u32 != pid {
                let host_handle = host_tid << 4 | 0x3;
                table.insert(host_handle, new_entry.clone());
            }
            if let Ok(mut all) = ALL_ACTIVE_THREADS.lock() {
                all.push(new_entry.clone());
            }
            crate::sys::diagnostics::register_current_thread(pid, host_tid);
            new_entry
        };

        *cell.borrow_mut() = Some(entry.clone());
        entry
    })
}

pub fn get_all_thread_stack_bounds() -> Vec<(u64, u64)> {
    let _ = ensure_main_thread_entry();
    let mut bounds = Vec::new();
    if let Ok(guard) = ALL_ACTIVE_THREADS.lock() {
        for entry in guard.iter() {
            if entry.is_terminated.load(Ordering::Relaxed) {
                continue;
            }
            let limit = entry.stack_limit.load(Ordering::Relaxed);
            let base = entry.stack_base.load(Ordering::Relaxed);
            if base > limit && limit > 0 && !bounds.contains(&(limit, base)) {
                bounds.push((limit, base));
            }
        }
    }
    bounds
}

#[no_mangle]
pub unsafe extern "win64" fn CreateThread(
    _lp_thread_attributes: *mut u8,
    dw_stack_size: usize,
    lp_start_address: u64,
    lp_parameter: u64,
    dw_creation_flags: u32,
    lp_thread_id: *mut u32,
) -> u64 {
    static THREAD_COUNTER: AtomicU32 = AtomicU32::new(0x200);

    let thread_id = THREAD_COUNTER.fetch_add(1, Ordering::Relaxed);
    if !lp_thread_id.is_null() {
        *lp_thread_id = thread_id;
    }

    let is_suspended = (dw_creation_flags & 0x4) != 0;

    if lp_start_address < 0x10000 || lp_start_address > 0x7FFF_FFFF_FFFFu64 {
        println!(
            "[KERNEL32 THREAD] CreateThread: invalid start address 0x{:016X}",
            lp_start_address
        );
        return 0;
    }

    println!(
        "[KERNEL32 THREAD] CreateThread: id={}, start=0x{:016X}, param=0x{:016X}, flags=0x{:X}",
        thread_id, lp_start_address, lp_parameter, dw_creation_flags
    );

    // Allocate a fake HANDLE value for this thread (odd numbers to avoid confusion with ptrs)
    let handle = (thread_id as u64) << 4 | 0x3;

    let sync = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let suspended_sync = std::sync::Arc::new((
        std::sync::Mutex::new(is_suspended),
        std::sync::Condvar::new(),
    ));

    let thread_entry = std::sync::Arc::new(ThreadEntry {
        thread_id,
        host_tid: AtomicU64::new(0),
        exit_code: AtomicU32::new(259), // STILL_ACTIVE
        is_terminated: AtomicBool::new(false),
        stack_base: AtomicU64::new(0),
        stack_limit: AtomicU64::new(0),
        last_rsp: AtomicU64::new(0),
        last_rbp: AtomicU64::new(0),
        last_rax: AtomicU64::new(0),
        last_rbx: AtomicU64::new(0),
        last_rcx: AtomicU64::new(0),
        last_rdx: AtomicU64::new(0),
        last_rsi: AtomicU64::new(0),
        last_rdi: AtomicU64::new(0),
        last_r8: AtomicU64::new(0),
        last_r9: AtomicU64::new(0),
        last_r10: AtomicU64::new(0),
        last_r11: AtomicU64::new(0),
        last_r12: AtomicU64::new(0),
        last_r13: AtomicU64::new(0),
        last_r14: AtomicU64::new(0),
        last_r15: AtomicU64::new(0),
        last_rip: AtomicU64::new(0),
        sync,
        suspended_sync,
        suspend_count: AtomicI32::new(if is_suspended { 1 } else { 0 }),
        suspend_ack: AtomicBool::new(false),
    });

    {
        let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
        let table = guard.get_or_insert_with(HashMap::new);
        table.insert(handle, thread_entry.clone());
    }
    if let Ok(mut all) = ALL_ACTIVE_THREADS.lock() {
        all.push(thread_entry.clone());
    }

    let image_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst);
    let peb_address = CURRENT_PEB_ADDRESS.load(std::sync::atomic::Ordering::SeqCst);
    let stack_size = if dw_stack_size == 0 {
        2 * 1024 * 1024
    } else {
        dw_stack_size
    };

    let thread_entry_worker = thread_entry.clone();

    std::thread::Builder::new()
        .stack_size(stack_size)
        .spawn(move || {
            CURRENT_THREAD_ENTRY.with(|cell| {
                *cell.borrow_mut() = Some(thread_entry_worker.clone());
            });

            let pid = unsafe { libc::getpid() } as u64;
            let tid = thread_id as u64;
            let host_tid = unsafe { libc::syscall(libc::SYS_gettid) as u64 };
            thread_entry_worker.host_tid.store(host_tid, Ordering::SeqCst);
            crate::sys::diagnostics::register_current_thread(thread_id, host_tid);

            {
                let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
                if let Some(table) = guard.as_mut() {
                    table.insert((host_tid << 4) | 0x3, thread_entry_worker.clone());
                }
            }

            // Record stack bounds immediately
            let (stack_limit, stack_base) = crate::sys::get_current_thread_stack_bounds();
            thread_entry_worker.stack_base.store(stack_base, Ordering::SeqCst);
            thread_entry_worker.stack_limit.store(stack_limit, Ordering::SeqCst);

            // Allocate full TEB block with PROT_NONE guard pages on both ends
            let teb_size = 65536usize;
            let alloc_size = teb_size + 0x2000;
            let raw_mem = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    alloc_size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            assert!(
                raw_mem != libc::MAP_FAILED && !raw_mem.is_null(),
                "Failed to allocate worker TEB memory"
            );
            unsafe {
                libc::mprotect(raw_mem, 0x1000, libc::PROT_NONE);
                libc::mprotect(
                    (raw_mem as *mut u8).add(alloc_size - 0x1000) as *mut libc::c_void,
                    0x1000,
                    libc::PROT_NONE,
                );
            }
            let teb_mem = unsafe { (raw_mem as *mut u8).add(0x1000) as *mut libc::c_void };
            unsafe {
                std::ptr::write_bytes(teb_mem as *mut u8, 0, teb_size);
            }

            let teb_ptr = teb_mem as u64;
            let tls_array_addr = teb_ptr + 0x200; // TLS slots at offset 0x200

            let teb_struct = crate::pe::teb::Teb64 {
                tib: crate::pe::teb::NtTib64 {
                    exception_list: 0,
                    stack_base: stack_base,
                    stack_limit: stack_limit,
                    sub_system_tib: 0,
                    fiber_data: 0,
                    arbitrary_user_pointer: 0,
                    self_ptr: teb_ptr,
                },
                environment_pointer: 0,
                client_id: crate::pe::teb::ClientId64 {
                    unique_process: pid,
                    unique_thread: tid,
                },
                active_rpc_handle: 0,
                thread_local_storage_pointer: tls_array_addr,
                peb: peb_address,
                last_error_value: 0,
                count_of_owned_critical_sections: 0,
                cs_client_thread: 0,
                win32_thread_info: 0,
            };

            unsafe {
                std::ptr::write(teb_mem as *mut crate::pe::teb::Teb64, teb_struct);
            }

            // Set GS base for this worker thread
            let _ = crate::sys::trampoline::set_thread_teb_gs_base(teb_ptr);

            // Initialize worker thread TLS templates
            crate::mem::loader::init_thread_tls(tls_array_addr);

            // Read back GS base to verify
            let mut gs_base: u64 = 0;
            unsafe {
                libc::syscall(libc::SYS_arch_prctl, 0x1004, &raw mut gs_base); // ARCH_GET_GS
            }
            let rva = if image_base > 0 && lp_start_address >= image_base {
                lp_start_address - image_base
            } else {
                0
            };

            println!(
                "[RINE GUEST THREAD START] guest_start_va=0x{:016X} guest_start_rva=0x{:08X} thread_handle=0x{:016X} thread_id={} host_thread_id={} argument=0x{:016X} gs_base=0x{:016X} teb=0x{:016X} match={}",
                lp_start_address,
                rva,
                handle,
                thread_id,
                host_tid,
                lp_parameter,
                gs_base,
                teb_ptr,
                if gs_base == teb_ptr { "yes" } else { "no" }
            );

            if is_suspended {
                let (lock, cvar) = &*thread_entry_worker.suspended_sync;
                let mut guard = lock.lock().unwrap();
                while *guard {
                    guard = cvar.wait(guard).unwrap();
                }
            }

            update_current_thread_stack_context();
            let thread_fn: unsafe extern "win64" fn(u64) -> u32 =
                unsafe { std::mem::transmute(lp_start_address) };
            let ret = unsafe { thread_fn(lp_parameter) };

            println!(
                "[RINE GUEST THREAD EXIT] guest_start_va=0x{:016X} thread_handle=0x{:016X} thread_id={} exit_code={}",
                lp_start_address, handle, thread_id, ret
            );

            thread_entry_worker
                .exit_code
                .store(ret, std::sync::atomic::Ordering::SeqCst);
            thread_entry_worker
                .is_terminated
                .store(true, std::sync::atomic::Ordering::SeqCst);

            {
                let (lock, cvar) = &*thread_entry_worker.sync;
                let mut done = lock.lock().unwrap();
                *done = true;
                cvar.notify_all();
            }
        })
        .expect("Failed to spawn guest thread");

    handle
}

#[no_mangle]
pub unsafe extern "win64" fn WaitForSingleObject(h_handle: u64, dw_milliseconds: u32) -> u32 {
    update_current_thread_stack_context();
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 0x102;
    const WAIT_FAILED: u32 = 0xFFFFFFFF;
    const ERROR_INVALID_HANDLE: u32 = 6;

    if h_handle == 0 || h_handle == u64::MAX || h_handle == u32::MAX as u64 {
        SetLastError(ERROR_INVALID_HANDLE);
        return WAIT_FAILED;
    }

    // 1. Event handle
    if let Some(cv) = event_table_get(h_handle) {
        let (lock, cvar) = &*cv;
        let mut state = lock.lock().unwrap();
        if state.0 {
            if !state.1 {
                state.0 = false;
            }
            return WAIT_OBJECT_0;
        }
        if dw_milliseconds == 0 {
            return WAIT_TIMEOUT;
        }
        if dw_milliseconds == 0xFFFFFFFF {
            while !state.0 {
                state = cvar.wait(state).unwrap();
            }
            if !state.1 {
                state.0 = false;
            }
            return WAIT_OBJECT_0;
        } else {
            let timeout = std::time::Duration::from_millis(dw_milliseconds as u64);
            let start = std::time::Instant::now();
            while !state.0 {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return WAIT_TIMEOUT;
                }
                let remaining = timeout - elapsed;
                let (new_state, timeout_result) = cvar.wait_timeout(state, remaining).unwrap();
                state = new_state;
                if timeout_result.timed_out() && !state.0 {
                    return WAIT_TIMEOUT;
                }
            }
            if !state.1 {
                state.0 = false;
            }
            return WAIT_OBJECT_0;
        }
    }

    // 2. Semaphore handle
    if let Some(sem) = semaphore_table_get(h_handle) {
        let (lock, cvar) = &*sem;
        let mut state = lock.lock().unwrap();
        if state.count > 0 {
            state.count -= 1;
            return WAIT_OBJECT_0;
        }
        if dw_milliseconds == 0 {
            return WAIT_TIMEOUT;
        }
        if dw_milliseconds == 0xFFFFFFFF {
            while state.count == 0 {
                state = cvar.wait(state).unwrap();
            }
            state.count -= 1;
            return WAIT_OBJECT_0;
        } else {
            let timeout = std::time::Duration::from_millis(dw_milliseconds as u64);
            let start = std::time::Instant::now();
            while state.count == 0 {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return WAIT_TIMEOUT;
                }
                let remaining = timeout - elapsed;
                let (new_state, timeout_result) = cvar.wait_timeout(state, remaining).unwrap();
                state = new_state;
                if timeout_result.timed_out() && state.count == 0 {
                    return WAIT_TIMEOUT;
                }
            }
            state.count -= 1;
            return WAIT_OBJECT_0;
        }
    }

    // 3. Mutex handle
    if let Some(m) = mutex_table_get(h_handle) {
        let current_tid = GetCurrentThreadId();
        let (lock, cvar) = &*m;
        let mut state = lock.lock().unwrap();
        if state.owner == current_tid {
            state.count += 1;
            return WAIT_OBJECT_0;
        }
        if state.owner == 0 {
            state.owner = current_tid;
            state.count = 1;
            return WAIT_OBJECT_0;
        }
        if dw_milliseconds == 0 {
            return WAIT_TIMEOUT;
        }
        if dw_milliseconds == 0xFFFFFFFF {
            while state.owner != 0 {
                state = cvar.wait(state).unwrap();
            }
            state.owner = current_tid;
            state.count = 1;
            return WAIT_OBJECT_0;
        } else {
            let timeout = std::time::Duration::from_millis(dw_milliseconds as u64);
            let start = std::time::Instant::now();
            while state.owner != 0 {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return WAIT_TIMEOUT;
                }
                let remaining = timeout - elapsed;
                let (new_state, timeout_result) = cvar.wait_timeout(state, remaining).unwrap();
                state = new_state;
                if timeout_result.timed_out() && state.owner != 0 {
                    return WAIT_TIMEOUT;
                }
            }
            state.owner = current_tid;
            state.count = 1;
            return WAIT_OBJECT_0;
        }
    }

    // 4. Timer handle
    if let Some(timer) = timer_table_get(h_handle) {
        let (lock, cvar) = &*timer;
        let mut state = lock.lock().unwrap();
        if state.signaled {
            if !state.manual_reset {
                state.signaled = false;
            }
            return WAIT_OBJECT_0;
        }
        if dw_milliseconds == 0 {
            return WAIT_TIMEOUT;
        }
        if dw_milliseconds == 0xFFFFFFFF {
            while !state.signaled {
                state = cvar.wait(state).unwrap();
            }
            if !state.manual_reset {
                state.signaled = false;
            }
            return WAIT_OBJECT_0;
        } else {
            let timeout = std::time::Duration::from_millis(dw_milliseconds as u64);
            let start = std::time::Instant::now();
            while !state.signaled {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return WAIT_TIMEOUT;
                }
                let remaining = timeout - elapsed;
                let (new_state, timeout_result) = cvar.wait_timeout(state, remaining).unwrap();
                state = new_state;
                if timeout_result.timed_out() && !state.signaled {
                    return WAIT_TIMEOUT;
                }
            }
            if !state.manual_reset {
                state.signaled = false;
            }
            return WAIT_OBJECT_0;
        }
    }

    // 5. Thread handle
    let thread_entry = {
        let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_mut() {
            table.get(&h_handle).cloned()
        } else {
            None
        }
    }.or_else(|| {
        let tid = (h_handle >> 4) as u32;
        ALL_ACTIVE_THREADS.lock().ok().and_then(|guard| {
            guard.iter().find(|e| {
                e.thread_id == tid
                || e.host_tid.load(Ordering::Relaxed) == tid as u64
                || e.thread_id == (h_handle as u32)
                || e.host_tid.load(Ordering::Relaxed) == h_handle
            }).cloned()
        })
    });
    if let Some(entry) = thread_entry {
        if entry
            .is_terminated
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            crate::sys::diagnostics::record_wait_end();
            return WAIT_OBJECT_0;
        }
        let (lock, cvar) = &*entry.sync;
        let mut done = lock.lock().unwrap();
        if *done
            || entry
                .is_terminated
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            crate::sys::diagnostics::record_wait_end();
            return WAIT_OBJECT_0;
        }
        if dw_milliseconds == 0xFFFFFFFF {
            while !*done
                && !entry
                    .is_terminated
                    .load(std::sync::atomic::Ordering::Relaxed)
            {
                done = cvar.wait(done).unwrap();
            }
            crate::sys::diagnostics::record_wait_end();
            return WAIT_OBJECT_0;
        } else {
            let timeout = std::time::Duration::from_millis(dw_milliseconds as u64);
            let result = cvar.wait_timeout(done, timeout).unwrap();
            crate::sys::diagnostics::record_wait_end();
            return if *result.0
                || entry
                    .is_terminated
                    .load(std::sync::atomic::Ordering::Relaxed)
            {
                WAIT_OBJECT_0
            } else {
                WAIT_TIMEOUT
            };
        }
    }

    // 5b. Process handle (e.g. from OpenProcess: 0x1000 | pid, or pseudo-handle)
    if (h_handle & 0xFFFFFFFFFFFFF000) == 0x1000 {
        let pid = (h_handle & 0xFFF) as u32;
        let my_pid = GetCurrentProcessId();
        if pid == my_pid || pid == 0 {
            crate::sys::diagnostics::record_wait_end();
            if dw_milliseconds == 0 {
                return WAIT_TIMEOUT;
            }
            if dw_milliseconds != 0xFFFFFFFF {
                std::thread::sleep(std::time::Duration::from_millis(dw_milliseconds as u64));
                return WAIT_TIMEOUT;
            }
            return WAIT_OBJECT_0;
        }
        let start = std::time::Instant::now();
        let timeout = if dw_milliseconds == 0xFFFFFFFF {
            None
        } else {
            Some(std::time::Duration::from_millis(dw_milliseconds as u64))
        };
        loop {
            let running = unsafe { libc::kill(pid as i32, 0) == 0 };
            if !running {
                crate::sys::diagnostics::record_wait_end();
                return WAIT_OBJECT_0;
            }
            if let Some(t) = timeout {
                if start.elapsed() >= t {
                    crate::sys::diagnostics::record_wait_end();
                    return WAIT_TIMEOUT;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    // 6. Unknown handle:
    println!(
        "[KERNEL32] WaitForSingleObject: UNKNOWN/INVALID handle 0x{:X}, ms={}",
        h_handle, dw_milliseconds
    );
    crate::sys::diagnostics::record_wait_end();
    if dw_milliseconds == 0 {
        SetLastError(ERROR_INVALID_HANDLE);
        return WAIT_TIMEOUT;
    }
    if dw_milliseconds != 0xFFFFFFFF {
        std::thread::sleep(std::time::Duration::from_millis(dw_milliseconds as u64));
    }
    SetLastError(ERROR_INVALID_HANDLE);
    WAIT_FAILED
}

#[no_mangle]
pub unsafe extern "win64" fn WaitForSingleObjectEx(
    h_handle: u64,
    dw_milliseconds: u32,
    _b_alertable: i32,
) -> u32 {
    WaitForSingleObject(h_handle, dw_milliseconds)
}

pub fn is_handle_signaled(h: u64, cur_tid: u32) -> bool {
    if let Some(cv) = event_table_get(h) {
        let (lock, _) = &*cv;
        if let Ok(state) = lock.lock() {
            return state.0;
        }
    }
    if let Some(sem) = semaphore_table_get(h) {
        let (lock, _) = &*sem;
        if let Ok(state) = lock.lock() {
            return state.count > 0;
        }
    }
    if let Some(m) = mutex_table_get(h) {
        let (lock, _) = &*m;
        if let Ok(state) = lock.lock() {
            return state.owner == 0 || state.owner == cur_tid;
        }
    }
    if let Some(timer) = timer_table_get(h) {
        let (lock, _) = &*timer;
        if let Ok(state) = lock.lock() {
            return state.signaled;
        }
    }
    let thread_entry = {
        let guard = THREAD_HANDLE_TABLE.lock().unwrap();
        guard.as_ref().and_then(|table| table.get(&h).cloned())
    }.or_else(|| {
        let tid = (h >> 4) as u32;
        ALL_ACTIVE_THREADS.lock().ok().and_then(|guard| {
            guard.iter().find(|e| {
                e.thread_id == tid
                || e.host_tid.load(Ordering::Relaxed) == tid as u64
                || e.thread_id == (h as u32)
                || e.host_tid.load(Ordering::Relaxed) == h
            }).cloned()
        })
    });
    if let Some(entry) = thread_entry {
        if entry.is_terminated.load(Ordering::Relaxed) {
            return true;
        }
        let (lock, _) = &*entry.sync;
        if let Ok(done) = lock.lock() {
            if *done {
                return true;
            }
        }
    }
    if (h & 0xFFFFFFFFFFFFF000) == 0x1000 {
        let pid = (h & 0xFFF) as u32;
        let my_pid = unsafe { GetCurrentProcessId() };
        if pid != my_pid && pid != 0 {
            let running = unsafe { libc::kill(pid as i32, 0) == 0 };
            return !running;
        }
    }
    false
}

pub fn consume_handle_signal(h: u64, cur_tid: u32) {
    if let Some(cv) = event_table_get(h) {
        let (lock, _) = &*cv;
        if let Ok(mut state) = lock.lock() {
            if state.0 && !state.1 {
                // Auto-reset event
                state.0 = false;
            }
        }
        return;
    }
    if let Some(sem) = semaphore_table_get(h) {
        let (lock, _) = &*sem;
        if let Ok(mut state) = lock.lock() {
            if state.count > 0 {
                state.count -= 1;
            }
        }
        return;
    }
    if let Some(m) = mutex_table_get(h) {
        let (lock, _) = &*m;
        if let Ok(mut state) = lock.lock() {
            if state.owner == cur_tid {
                state.count += 1;
            } else if state.owner == 0 {
                state.owner = cur_tid;
                state.count = 1;
            }
        }
        return;
    }
    if let Some(timer) = timer_table_get(h) {
        let (lock, _) = &*timer;
        if let Ok(mut state) = lock.lock() {
            if state.signaled && !state.manual_reset {
                state.signaled = false;
            }
        }
        return;
    }
}

#[no_mangle]
pub unsafe extern "win64" fn WaitForMultipleObjects(
    n_count: u32,
    lp_handles: *const u64,
    b_wait_all: i32,
    dw_milliseconds: u32,
) -> u32 {
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 0x102;
    const WAIT_FAILED: u32 = 0xFFFFFFFF;
    const ERROR_INVALID_PARAMETER: u32 = 87;

    if n_count == 0 || lp_handles.is_null() || n_count > 64 {
        SetLastError(ERROR_INVALID_PARAMETER);
        return WAIT_FAILED;
    }

    let handles: Vec<u64> = (0..n_count as usize)
        .map(|i| *lp_handles.add(i))
        .collect();

    let cur_tid = GetCurrentThreadId();
    let start = std::time::Instant::now();
    let timeout = if dw_milliseconds == 0xFFFFFFFF {
        None
    } else {
        Some(std::time::Duration::from_millis(dw_milliseconds as u64))
    };

    loop {
        if b_wait_all != 0 {
            let mut all_signaled = true;
            for &h in &handles {
                if !is_handle_signaled(h, cur_tid) {
                    all_signaled = false;
                    break;
                }
            }
            if all_signaled {
                for &h in &handles {
                    consume_handle_signal(h, cur_tid);
                }
                return WAIT_OBJECT_0;
            }
        } else {
            for (i, &h) in handles.iter().enumerate() {
                if is_handle_signaled(h, cur_tid) {
                    consume_handle_signal(h, cur_tid);
                    return WAIT_OBJECT_0 + i as u32;
                }
            }
        }

        if let Some(t) = timeout {
            if start.elapsed() >= t {
                return WAIT_TIMEOUT;
            }
        }

        std::thread::sleep(std::time::Duration::from_micros(200));
    }
}

#[no_mangle]
pub unsafe extern "win64" fn WaitForMultipleObjectsEx(
    n_count: u32,
    lp_handles: *const u64,
    b_wait_all: i32,
    dw_milliseconds: u32,
    _b_alertable: i32,
) -> u32 {
    WaitForMultipleObjects(n_count, lp_handles, b_wait_all, dw_milliseconds)
}

#[no_mangle]
pub unsafe extern "win64" fn CloseHandle(h_object: u64) -> i32 {
    {
        let mut g = EVENT_TABLE.lock().unwrap();
        if let Some(table) = g.as_mut() {
            table.remove(&h_object);
        }
    }
    {
        let mut g = SEMAPHORE_TABLE.lock().unwrap();
        if let Some(table) = g.as_mut() {
            table.remove(&h_object);
        }
    }
    {
        let mut g = MUTEX_TABLE.lock().unwrap();
        if let Some(table) = g.as_mut() {
            table.remove(&h_object);
        }
    }
    {
        let mut g = TIMER_TABLE.lock().unwrap();
        if let Some(table) = g.as_mut() {
            table.remove(&h_object);
        }
    }
    {
        let mut guard = THREAD_HANDLE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_mut() {
            table.remove(&h_object);
        }
    }
    {
        let mut guard = POWER_REQUEST_TABLE.lock().unwrap();
        if let Some(table) = guard.as_mut() {
            table.remove(&h_object);
        }
    }
    {
        let mut guard = SNAPSHOT_TABLE.lock().unwrap();
        if let Some(table) = guard.as_mut() {
            table.remove(&h_object);
        }
    }
    {
        let mut g = FILE_MAPPING_TABLE.lock().unwrap();
        if let Some(table) = g.as_mut() {
            if let Some(mapping) = table.remove(&h_object) {
                if mapping.fd >= 0 {
                    libc::close(mapping.fd);
                }
            }
        }
    }
    {
        let mut g = FIND_SEARCH_TABLE.lock().unwrap();
        if let Some(table) = g.as_mut() {
            table.remove(&h_object);
        }
    }
    if is_fd_handle(h_object) {
        let mut g = FD_TABLE.lock().unwrap();
        if let Some(table) = g.as_mut() {
            table.remove(&h_object);
        }
        libc::close(h_object as i32);
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetHandleInformation(
    _h_object: u64,
    lpdw_flags: *mut u32,
) -> i32 {
    if !lpdw_flags.is_null() {
        *lpdw_flags = 0;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn SetHandleInformation(
    _h_object: u64,
    _dw_mask: u32,
    _dw_flags: u32,
) -> i32 {
    1 // TRUE
}

static THREAD_OPEN_HANDLE_COUNTER: AtomicU64 = AtomicU64::new(0x20000);

#[no_mangle]
pub unsafe extern "win64" fn OpenThread(
    _dw_desired_access: u32,
    _b_inherit_handle: i32,
    dw_thread_id: u32,
) -> u64 {
    let _ = ensure_main_thread_entry();
    if let Ok(guard) = ALL_ACTIVE_THREADS.lock() {
        if let Some(entry) = guard.iter().find(|e| e.thread_id == dw_thread_id || e.host_tid.load(Ordering::Relaxed) == dw_thread_id as u64) {
            let handle = THREAD_OPEN_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) << 4 | 0x3;
            let mut h_guard = THREAD_HANDLE_TABLE.lock().unwrap();
            let table = h_guard.get_or_insert_with(HashMap::new);
            table.insert(handle, entry.clone());
            SetLastError(0);
            return handle;
        }
    }
    SetLastError(87);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentThread() -> u64 {
    0xFFFFFFFFFFFFFFFEu64 // INVALID_HANDLE_VALUE sentinel for current thread
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentThreadId() -> u32 {
    let _ = ensure_main_thread_entry();
    CURRENT_THREAD_ENTRY.with(|cell| {
        cell.borrow().as_ref().map(|e| e.thread_id)
    }).unwrap_or_else(|| unsafe { libc::syscall(libc::SYS_gettid) as u32 })
}

#[no_mangle]
pub unsafe extern "win64" fn GetThreadId(h_thread: u64) -> u32 {
    if h_thread == 0xFFFFFFFFFFFFFFFEu64 || h_thread == 0xFFFFFFFFFFFFFFFFu64 {
        return GetCurrentThreadId();
    }
    let guard = THREAD_HANDLE_TABLE.lock().unwrap();
    if let Some(table) = guard.as_ref() {
        if let Some(entry) = table.get(&h_thread) {
            return entry.thread_id;
        }
    }
    if (h_thread & 0xF) == 0x3 {
        return (h_thread >> 4) as u32;
    }
    GetCurrentThreadId()
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentProcessId() -> u32 {
    unsafe { libc::getpid() as u32 }
}

#[no_mangle]
pub unsafe extern "win64" fn GetExitCodeThread(h_thread: u64, lp_exit_code: *mut u32) -> i32 {
    if !lp_exit_code.is_null() {
        let guard = THREAD_HANDLE_TABLE.lock().unwrap();
        if let Some(table) = guard.as_ref() {
            if let Some(entry) = table.get(&h_thread) {
                if entry
                    .is_terminated
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    *lp_exit_code = entry.exit_code.load(std::sync::atomic::Ordering::Relaxed);
                } else {
                    *lp_exit_code = 259; // STILL_ACTIVE
                }
                return 1;
            }
        }
        *lp_exit_code = 0;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetExitCodeProcess(_h_process: u64, lp_exit_code: *mut u32) -> i32 {
    if !lp_exit_code.is_null() {
        *lp_exit_code = 259; // STILL_ACTIVE (0x103)
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn OpenProcess(
    _dw_desired_access: u32,
    _b_inherit_handle: i32,
    dw_process_id: u32,
) -> u64 {
    println!("[KERNEL32] OpenProcess(dw_process_id={})", dw_process_id);
    0x0000000000001000 | (dw_process_id as u64)
}

#[no_mangle]
pub unsafe extern "win64" fn SetPriorityClass(_h_process: usize, dw_priority_class: u32) -> i32 {
    println!(
        "[KERNEL32] SetPriorityClass(dwPriorityClass=0x{:X}) -> TRUE",
        dw_priority_class
    );
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetPriorityClass(_h_process: usize) -> u32 {
    0x00000020 // NORMAL_PRIORITY_CLASS
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct THREADENTRY32 {
    pub dw_size: u32,
    pub cnt_usage: u32,
    pub th32_thread_id: u32,
    pub th32_owner_process_id: u32,
    pub tp_base_pri: i32,
    pub tp_delta_pri: i32,
    pub dw_flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PROCESSENTRY32W {
    pub dw_size: u32,
    pub cnt_usage: u32,
    pub th32_process_id: u32,
    pub th32_default_heap_id: usize,
    pub th32_module_id: u32,
    pub cnt_threads: u32,
    pub th32_parent_process_id: u32,
    pub pc_pri_class_base: i32,
    pub dw_flags: u32,
    pub sz_exe_file: [u16; 260],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PROCESSENTRY32 {
    pub dw_size: u32,
    pub cnt_usage: u32,
    pub th32_process_id: u32,
    pub th32_default_heap_id: usize,
    pub th32_module_id: u32,
    pub cnt_threads: u32,
    pub th32_parent_process_id: u32,
    pub pc_pri_class_base: i32,
    pub dw_flags: u32,
    pub sz_exe_file: [u8; 260],
}

struct SnapshotState {
    threads: Vec<THREADENTRY32>,
    thread_idx: usize,
    processes: Vec<PROCESSENTRY32W>,
    process_idx: usize,
}

static SNAPSHOT_TABLE: Mutex<Option<HashMap<u64, SnapshotState>>> = Mutex::new(None);
static SNAPSHOT_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x7000);

#[no_mangle]
pub unsafe extern "win64" fn CreateToolhelp32Snapshot(dw_flags: u32, _th32_process_id: u32) -> u64 {
    let pid = libc::getpid() as u32;
    let mut threads = Vec::new();
    let mut seen_tids = std::collections::HashSet::new();

    // Add main thread
    threads.push(THREADENTRY32 {
        dw_size: std::mem::size_of::<THREADENTRY32>() as u32,
        cnt_usage: 0,
        th32_thread_id: pid,
        th32_owner_process_id: pid,
        tp_base_pri: 8,
        tp_delta_pri: 0,
        dw_flags: 0,
    });
    seen_tids.insert(pid);

    // Add active non-terminated threads from ALL_ACTIVE_THREADS without duplicates
    if let Ok(guard) = ALL_ACTIVE_THREADS.lock() {
        for entry in guard.iter() {
            if !entry.is_terminated.load(Ordering::Relaxed) {
                let tid = entry.thread_id;
                if seen_tids.insert(tid) {
                    threads.push(THREADENTRY32 {
                        dw_size: std::mem::size_of::<THREADENTRY32>() as u32,
                        cnt_usage: 0,
                        th32_thread_id: tid,
                        th32_owner_process_id: pid,
                        tp_base_pri: 8,
                        tp_delta_pri: 0,
                        dw_flags: 0,
                    });
                }
            }
        }
    }

    let mut processes = Vec::new();
    let mut pe = PROCESSENTRY32W {
        dw_size: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        cnt_usage: 0,
        th32_process_id: pid,
        th32_default_heap_id: 0x1000,
        th32_module_id: 1,
        cnt_threads: threads.len() as u32,
        th32_parent_process_id: 0,
        pc_pri_class_base: 8,
        dw_flags: 0,
        sz_exe_file: [0; 260],
    };
    let raw_exe = get_target_exe_path();
    let exe_name = std::path::Path::new(&raw_exe)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "TARGET.EXE".to_string());
    for (i, c) in exe_name.encode_utf16().enumerate().take(259) {
        pe.sz_exe_file[i] = c;
    }
    processes.push(pe);

    let state = SnapshotState {
        threads,
        thread_idx: 0,
        processes,
        process_idx: 0,
    };

    let handle = SNAPSHOT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) << 4 | 0x7;
    let mut guard = SNAPSHOT_TABLE.lock().unwrap();
    let thread_count = state.threads.len();
    guard.get_or_insert_with(HashMap::new).insert(handle, state);

    println!(
        "[KERNEL32] CreateToolhelp32Snapshot(flags=0x{:X}) -> 0x{:X} ({} threads)",
        dw_flags, handle, thread_count
    );
    handle
}

#[no_mangle]
pub unsafe extern "win64" fn Thread32First(h_snapshot: u64, lpte: *mut THREADENTRY32) -> i32 {
    if lpte.is_null() {
        return 0;
    }
    let mut guard = SNAPSHOT_TABLE.lock().unwrap();
    if let Some(table) = guard.as_mut() {
        if let Some(snap) = table.get_mut(&h_snapshot) {
            snap.thread_idx = 0;
            if !snap.threads.is_empty() {
                let orig_size = (*lpte).dw_size;
                *lpte = snap.threads[0];
                (*lpte).dw_size = orig_size;
                snap.thread_idx = 1;
                return 1;
            }
        }
    }
    SetLastError(18); // ERROR_NO_MORE_FILES
    0
}

#[no_mangle]
pub unsafe extern "win64" fn Thread32Next(h_snapshot: u64, lpte: *mut THREADENTRY32) -> i32 {
    if lpte.is_null() {
        return 0;
    }
    let mut guard = SNAPSHOT_TABLE.lock().unwrap();
    if let Some(table) = guard.as_mut() {
        if let Some(snap) = table.get_mut(&h_snapshot) {
            if snap.thread_idx < snap.threads.len() {
                let orig_size = (*lpte).dw_size;
                *lpte = snap.threads[snap.thread_idx];
                (*lpte).dw_size = orig_size;
                snap.thread_idx += 1;
                return 1;
            }
        }
    }
    SetLastError(18); // ERROR_NO_MORE_FILES
    0
}

#[no_mangle]
pub unsafe extern "win64" fn Process32FirstW(h_snapshot: u64, lppe: *mut PROCESSENTRY32W) -> i32 {
    if lppe.is_null() {
        return 0;
    }
    let mut guard = SNAPSHOT_TABLE.lock().unwrap();
    if let Some(table) = guard.as_mut() {
        if let Some(snap) = table.get_mut(&h_snapshot) {
            snap.process_idx = 0;
            if !snap.processes.is_empty() {
                let orig_size = (*lppe).dw_size;
                *lppe = snap.processes[0];
                (*lppe).dw_size = orig_size;
                snap.process_idx = 1;
                return 1;
            }
        }
    }
    SetLastError(18);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn Process32NextW(h_snapshot: u64, lppe: *mut PROCESSENTRY32W) -> i32 {
    if lppe.is_null() {
        return 0;
    }
    let mut guard = SNAPSHOT_TABLE.lock().unwrap();
    if let Some(table) = guard.as_mut() {
        if let Some(snap) = table.get_mut(&h_snapshot) {
            if snap.process_idx < snap.processes.len() {
                let orig_size = (*lppe).dw_size;
                *lppe = snap.processes[snap.process_idx];
                (*lppe).dw_size = orig_size;
                snap.process_idx += 1;
                return 1;
            }
        }
    }
    SetLastError(18);
    0
}

#[no_mangle]
pub unsafe extern "win64" fn Process32First(h_snapshot: u64, lppe: *mut PROCESSENTRY32) -> i32 {
    if lppe.is_null() {
        return 0;
    }
    let mut p_wide: PROCESSENTRY32W = std::mem::zeroed();
    p_wide.dw_size = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    if Process32FirstW(h_snapshot, &mut p_wide) == 1 {
        (*lppe).cnt_usage = p_wide.cnt_usage;
        (*lppe).th32_process_id = p_wide.th32_process_id;
        (*lppe).th32_default_heap_id = p_wide.th32_default_heap_id;
        (*lppe).th32_module_id = p_wide.th32_module_id;
        (*lppe).cnt_threads = p_wide.cnt_threads;
        (*lppe).th32_parent_process_id = p_wide.th32_parent_process_id;
        (*lppe).pc_pri_class_base = p_wide.pc_pri_class_base;
        (*lppe).dw_flags = p_wide.dw_flags;
        for i in 0..259 {
            (*lppe).sz_exe_file[i] = p_wide.sz_exe_file[i] as u8;
        }
        return 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn Process32Next(h_snapshot: u64, lppe: *mut PROCESSENTRY32) -> i32 {
    if lppe.is_null() {
        return 0;
    }
    let mut p_wide: PROCESSENTRY32W = std::mem::zeroed();
    p_wide.dw_size = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    if Process32NextW(h_snapshot, &mut p_wide) == 1 {
        (*lppe).cnt_usage = p_wide.cnt_usage;
        (*lppe).th32_process_id = p_wide.th32_process_id;
        (*lppe).th32_default_heap_id = p_wide.th32_default_heap_id;
        (*lppe).th32_module_id = p_wide.th32_module_id;
        (*lppe).cnt_threads = p_wide.cnt_threads;
        (*lppe).th32_parent_process_id = p_wide.th32_parent_process_id;
        (*lppe).pc_pri_class_base = p_wide.pc_pri_class_base;
        (*lppe).dw_flags = p_wide.dw_flags;
        for i in 0..259 {
            (*lppe).sz_exe_file[i] = p_wide.sz_exe_file[i] as u8;
        }
        return 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetProcessId(h_process: u64) -> u32 {
    if h_process == 0xFFFFFFFFFFFFFFFFu64 || h_process == 0 {
        return GetCurrentProcessId();
    }
    (h_process >> 4) as u32
}

#[no_mangle]
pub unsafe extern "win64" fn SwitchToThread() -> i32 {
    std::thread::yield_now();
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetThreadAffinityMask(_h_thread: u64, dw_thread_affinity_mask: usize) -> usize {
    dw_thread_affinity_mask
}

#[repr(C)]
pub struct SYSTEM_POWER_STATUS {
    pub ac_line_status: u8,
    pub battery_flag: u8,
    pub battery_life_percent: u8,
    pub system_status_flag: u8,
    pub battery_life_time: u32,
    pub battery_full_life_time: u32,
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemPowerStatus(lp_system_power_status: *mut SYSTEM_POWER_STATUS) -> i32 {
    if !lp_system_power_status.is_null() {
        *lp_system_power_status = SYSTEM_POWER_STATUS {
            ac_line_status: 1,      // Online (AC power)
            battery_flag: 128,      // No system battery
            battery_life_percent: 100,
            system_status_flag: 0,
            battery_life_time: 0xFFFFFFFF,
            battery_full_life_time: 0xFFFFFFFF,
        };
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetComputerNameW(lp_buffer: *mut u16, n_size: *mut u32) -> i32 {
    if lp_buffer.is_null() || n_size.is_null() {
        return 0;
    }
    let name = "DESKTOP-RINE";
    let needed = name.len() as u32;
    if *n_size <= needed {
        *n_size = needed + 1;
        SetLastError(234); // ERROR_MORE_DATA
        return 0;
    }
    for (i, c) in name.encode_utf16().enumerate() {
        *lp_buffer.add(i) = c;
    }
    *lp_buffer.add(name.len()) = 0;
    *n_size = needed;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDirectoryA(lp_buffer: *mut u8, u_size: u32) -> u32 {
    let s = b"C:\\Windows\\System32\0";
    let len = (s.len() - 1) as u32;
    if lp_buffer.is_null() || u_size <= len {
        return len + 1;
    }
    std::ptr::copy_nonoverlapping(s.as_ptr(), lp_buffer, s.len());
    len
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemDirectoryW(lp_buffer: *mut u16, u_size: u32) -> u32 {
    let s = "C:\\Windows\\System32";
    let len = s.len() as u32;
    if lp_buffer.is_null() || u_size <= len {
        return len + 1;
    }
    for (i, c) in s.encode_utf16().enumerate() {
        *lp_buffer.add(i) = c;
    }
    *lp_buffer.add(s.len()) = 0;
    len
}

#[no_mangle]
pub unsafe extern "win64" fn GetWindowsDirectoryW(lp_buffer: *mut u16, u_size: u32) -> u32 {
    let s = "C:\\Windows";
    let len = s.len() as u32;
    if lp_buffer.is_null() || u_size <= len {
        return len + 1;
    }
    for (i, c) in s.encode_utf16().enumerate() {
        *lp_buffer.add(i) = c;
    }
    *lp_buffer.add(s.len()) = 0;
    len
}

#[no_mangle]
pub unsafe extern "win64" fn GetDriveTypeW(_lp_root_path_name: *const u16) -> u32 {
    3 // DRIVE_FIXED
}

#[no_mangle]
pub unsafe extern "win64" fn HeapQueryInformation(
    _heap_handle: usize,
    _heap_information_class: u32,
    _heap_information: *mut libc::c_void,
    _heap_information_length: usize,
    return_length: *mut usize,
) -> i32 {
    if !return_length.is_null() {
        *return_length = 4;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn HeapSetInformation(
    _heap_handle: usize,
    _heap_information_class: u32,
    _heap_information: *const libc::c_void,
    _heap_information_length: usize,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetDriveTypeA(_lp_root_path_name: *const libc::c_char) -> u32 {
    3 // DRIVE_FIXED
}

#[no_mangle]
pub unsafe extern "win64" fn SetHandleCount(count: u32) -> u32 {
    count
}

#[no_mangle]
pub unsafe extern "win64" fn FatalAppExitA(_action: u32, msg: *const libc::c_char) {
    if !msg.is_null() && (msg as usize) >= 0x10000 {
        let text = std::ffi::CStr::from_ptr(msg).to_string_lossy();
        eprintln!("[FATAL APP EXIT] {}", text);
    }
    libc::exit(1);
}

#[no_mangle]
pub unsafe extern "win64" fn ExpandEnvironmentStringsW(
    lp_src: *const u16,
    lp_dst: *mut u16,
    n_size: u32,
) -> u32 {
    if lp_src.is_null() {
        return 0;
    }
    let mut len = 0;
    while *lp_src.add(len) != 0 && len < 4096 {
        len += 1;
    }
    let src_slice = std::slice::from_raw_parts(lp_src, len);
    let src_str = String::from_utf16_lossy(src_slice);

    let expanded = src_str
        .replace("%APPDATA%", "C:\\Users\\user\\AppData\\Roaming")
        .replace("%LOCALAPPDATA%", "C:\\Users\\user\\AppData\\Local")
        .replace("%USERPROFILE%", "C:\\Users\\user")
        .replace("%PROGRAMDATA%", "C:\\ProgramData")
        .replace("%TEMP%", "C:\\Windows\\Temp")
        .replace("%TMP%", "C:\\Windows\\Temp")
        .replace("%WINDIR%", "C:\\Windows")
        .replace("%SYSTEMROOT%", "C:\\Windows");

    let needed = (expanded.len() + 1) as u32;
    if lp_dst.is_null() || n_size < needed {
        return needed;
    }
    for (i, c) in expanded.encode_utf16().enumerate() {
        *lp_dst.add(i) = c;
    }
    *lp_dst.add(expanded.len()) = 0;
    needed
}

#[repr(C)]
pub struct PROCESS_MEMORY_COUNTERS {
    pub cb: u32,
    pub page_fault_count: u32,
    pub peak_working_set_size: usize,
    pub working_set_size: usize,
    pub quota_peak_paged_pool_usage: usize,
    pub quota_paged_pool_usage: usize,
    pub quota_peak_non_paged_pool_usage: usize,
    pub quota_non_paged_pool_usage: usize,
    pub pagefile_usage: usize,
    pub peak_pagefile_usage: usize,
}

#[no_mangle]
pub unsafe extern "win64" fn K32GetProcessMemoryInfo(
    _h_process: u64,
    ppmc: *mut PROCESS_MEMORY_COUNTERS,
    cb: u32,
) -> i32 {
    if ppmc.is_null() || cb < std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32 {
        return 0;
    }
    *ppmc = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        page_fault_count: 100,
        peak_working_set_size: 100 * 1024 * 1024,
        working_set_size: 64 * 1024 * 1024,
        quota_peak_paged_pool_usage: 10 * 1024 * 1024,
        quota_paged_pool_usage: 8 * 1024 * 1024,
        quota_peak_non_paged_pool_usage: 5 * 1024 * 1024,
        quota_non_paged_pool_usage: 4 * 1024 * 1024,
        pagefile_usage: 128 * 1024 * 1024,
        peak_pagefile_usage: 200 * 1024 * 1024,
    };
    1
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MEMORYSTATUS {
    pub dwLength: u32,
    pub dwMemoryLoad: u32,
    pub dwTotalPhys: usize,
    pub dwAvailPhys: usize,
    pub dwTotalPageFile: usize,
    pub dwAvailPageFile: usize,
    pub dwTotalVirtual: usize,
    pub dwAvailVirtual: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MEMORYSTATUSEX {
    pub dwLength: u32,
    pub dwMemoryLoad: u32,
    pub ullTotalPhys: u64,
    pub ullAvailPhys: u64,
    pub ullTotalPageFile: u64,
    pub ullAvailPageFile: u64,
    pub ullTotalVirtual: u64,
    pub ullAvailVirtual: u64,
    pub ullAvailExtendedVirtual: u64,
}

#[no_mangle]
pub unsafe extern "win64" fn GlobalMemoryStatus(lp_buffer: *mut MEMORYSTATUS) {
    if lp_buffer.is_null() {
        return;
    }
    let mut ex = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    GlobalMemoryStatusEx(&mut ex);

    let passed_len = (*lp_buffer).dwLength;
    if passed_len == 32 {
        let legacy = lp_buffer as *mut u32;
        *legacy.add(0) = 32;
        *legacy.add(1) = ex.dwMemoryLoad;
        *legacy.add(2) = (ex.ullTotalPhys.min(0xFFFFFFFF)) as u32;
        *legacy.add(3) = (ex.ullAvailPhys.min(0xFFFFFFFF)) as u32;
        *legacy.add(4) = (ex.ullTotalPageFile.min(0xFFFFFFFF)) as u32;
        *legacy.add(5) = (ex.ullAvailPageFile.min(0xFFFFFFFF)) as u32;
        *legacy.add(6) = (ex.ullTotalVirtual.min(0x7FFFFFFF)) as u32;
        *legacy.add(7) = (ex.ullAvailVirtual.min(0x7FFFFFFF)) as u32;
        return;
    }

    (*lp_buffer).dwLength = std::mem::size_of::<MEMORYSTATUS>() as u32;
    (*lp_buffer).dwMemoryLoad = ex.dwMemoryLoad;
    (*lp_buffer).dwTotalPhys = ex.ullTotalPhys as usize;
    (*lp_buffer).dwAvailPhys = ex.ullAvailPhys as usize;
    (*lp_buffer).dwTotalPageFile = ex.ullTotalPageFile as usize;
    (*lp_buffer).dwAvailPageFile = ex.ullAvailPageFile as usize;
    (*lp_buffer).dwTotalVirtual = ex.ullTotalVirtual as usize;
    (*lp_buffer).dwAvailVirtual = ex.ullAvailVirtual as usize;
}

#[no_mangle]
pub unsafe extern "win64" fn GlobalMemoryStatusEx(lp_buffer: *mut MEMORYSTATUSEX) -> i32 {
    if lp_buffer.is_null() {
        return 0;
    }
    let page_size = libc::sysconf(libc::_SC_PAGESIZE);
    let total_pages = libc::sysconf(libc::_SC_PHYS_PAGES);
    let avail_pages = libc::sysconf(libc::_SC_AVPHYS_PAGES);

    let (total_phys, avail_phys) = if page_size > 0 && total_pages > 0 && avail_pages > 0 {
        (
            (total_pages as u64).saturating_mul(page_size as u64),
            (avail_pages as u64).saturating_mul(page_size as u64),
        )
    } else {
        (16 * 1024 * 1024 * 1024, 12 * 1024 * 1024 * 1024)
    };

    let mem_load = if total_phys > 0 {
        ((total_phys.saturating_sub(avail_phys)) * 100 / total_phys) as u32
    } else {
        25
    };

    let total_virtual: u64 = 0x7FFFFFFF0000;
    let avail_virtual: u64 = 0x7F0000000000;

    *lp_buffer = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: mem_load,
        ullTotalPhys: total_phys,
        ullAvailPhys: avail_phys,
        ullTotalPageFile: total_phys.saturating_mul(2),
        ullAvailPageFile: avail_phys.saturating_mul(2),
        ullTotalVirtual: total_virtual,
        ullAvailVirtual: avail_virtual,
        ullAvailExtendedVirtual: 0,
    };
    1
}

#[no_mangle]
pub unsafe extern "win64" fn CancelIo(_h_file: u64) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetFileTime(
    _h_file: u64,
    _lp_creation_time: *const u64,
    _lp_last_access_time: *const u64,
    _lp_last_write_time: *const u64,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn RemoveDirectoryW(_lp_path_name: *const u16) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn ExitThread(dw_exit_code: u32) {
    println!(
        "[KERNEL32] ExitThread(code={}) -> terminating thread",
        dw_exit_code
    );
    CURRENT_THREAD_ENTRY.with(|cell| {
        if let Some(entry) = cell.borrow().as_ref() {
            entry.exit_code.store(dw_exit_code, Ordering::SeqCst);
            entry.is_terminated.store(true, Ordering::SeqCst);
            let (lock, cvar) = &*entry.sync;
            if let Ok(mut done) = lock.lock() {
                *done = true;
                cvar.notify_all();
            }
        }
    });
    libc::pthread_exit(std::ptr::null_mut());
}

#[no_mangle]
pub unsafe extern "win64" fn FreeLibraryAndExitThread(_h_lib_module: u64, dw_exit_code: u32) {
    println!(
        "[KERNEL32] FreeLibraryAndExitThread(code={}) -> terminating thread",
        dw_exit_code
    );
    CURRENT_THREAD_ENTRY.with(|cell| {
        if let Some(entry) = cell.borrow().as_ref() {
            entry.exit_code.store(dw_exit_code, Ordering::SeqCst);
            entry.is_terminated.store(true, Ordering::SeqCst);
            let (lock, cvar) = &*entry.sync;
            if let Ok(mut done) = lock.lock() {
                *done = true;
                cvar.notify_all();
            }
        }
    });
    libc::pthread_exit(std::ptr::null_mut());
}

#[no_mangle]
pub unsafe extern "win64" fn SetThreadPriority(_h_thread: u64, _n_priority: i32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SuspendThread(h_thread: u64) -> u32 {
    crate::sys::diagnostics::record_win32_api("SuspendThread");
    let _ = ensure_main_thread_entry();
    let entry_opt = {
        let guard = THREAD_HANDLE_TABLE.lock().unwrap();
        guard.as_ref().and_then(|table| table.get(&h_thread).cloned())
    }.or_else(|| {
        if h_thread == 0xFFFFFFFFFFFFFFFE || h_thread == 0xFFFFFFFFFFFFFFFF {
            CURRENT_THREAD_ENTRY.with(|cell| cell.borrow().clone())
        } else {
            let tid = (h_thread >> 4) as u32;
            ALL_ACTIVE_THREADS.lock().ok().and_then(|guard| {
                guard.iter().find(|e| {
                    e.thread_id == tid
                    || e.host_tid.load(Ordering::Relaxed) == tid as u64
                    || e.thread_id == (h_thread as u32)
                    || e.host_tid.load(Ordering::Relaxed) == h_thread
                }).cloned()
            })
        }
    });

    if let Some(entry) = entry_opt {
        let prev = entry.suspend_count.fetch_add(1, Ordering::SeqCst);
        if prev == 0 {
            let target_host_tid = entry.host_tid.load(Ordering::SeqCst);
            let my_host_tid = libc::syscall(libc::SYS_gettid) as u64;

            if target_host_tid != 0 && target_host_tid != my_host_tid {
                entry.suspend_ack.store(false, Ordering::SeqCst);
                // Send SIGUSR2 to interrupt target thread and run signal handler
                libc::syscall(
                    libc::SYS_tgkill,
                    libc::getpid(),
                    target_host_tid as libc::pid_t,
                    libc::SIGUSR2,
                );

                // Spin briefly first before yielding for ultra-fast signal response
                for _ in 0..1000 {
                    if entry.suspend_ack.load(Ordering::Relaxed) {
                        break;
                    }
                    std::hint::spin_loop();
                }

                // Wait up to 50ms for target thread to enter signal handler
                let start = std::time::Instant::now();
                while !entry.suspend_ack.load(Ordering::SeqCst) && start.elapsed().as_millis() < 50 {
                    std::thread::yield_now();
                }
            } else if target_host_tid == my_host_tid {
                // Self suspend: wait until another thread resumes us
                let (lock, cvar) = &*entry.suspended_sync;
                let mut guard = lock.lock().unwrap();
                while entry.suspend_count.load(Ordering::SeqCst) > 0 {
                    guard = cvar.wait(guard).unwrap();
                }
            }
        }
        return prev as u32;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn ResumeThread(h_thread: u64) -> u32 {
    crate::sys::diagnostics::record_win32_api("ResumeThread");
    let _ = ensure_main_thread_entry();
    let entry_opt = {
        let guard = THREAD_HANDLE_TABLE.lock().unwrap();
        guard.as_ref().and_then(|table| table.get(&h_thread).cloned())
    }.or_else(|| {
        if h_thread == 0xFFFFFFFFFFFFFFFE || h_thread == 0xFFFFFFFFFFFFFFFF {
            CURRENT_THREAD_ENTRY.with(|cell| cell.borrow().clone())
        } else {
            let tid = (h_thread >> 4) as u32;
            ALL_ACTIVE_THREADS.lock().ok().and_then(|guard| {
                guard.iter().find(|e| {
                    e.thread_id == tid
                    || e.host_tid.load(Ordering::Relaxed) == tid as u64
                    || e.thread_id == (h_thread as u32)
                    || e.host_tid.load(Ordering::Relaxed) == h_thread
                }).cloned()
            })
        }
    });

    if let Some(entry) = entry_opt {
        let prev = entry.suspend_count.load(Ordering::SeqCst);
        if prev > 0 {
            entry.suspend_count.fetch_sub(1, Ordering::SeqCst);
            let (lock, cvar) = &*entry.suspended_sync;
            if let Ok(mut g) = lock.lock() {
                *g = false;
                cvar.notify_all();
            }
            return prev as u32;
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetThreadContext(h_thread: u64, lp_context: *mut libc::c_void) -> i32 {
    if lp_context.is_null() {
        SetLastError(87);
        return 0;
    }
    crate::sys::diagnostics::record_win32_api("GetThreadContext");
    // Zero out full CONTEXT structure (1232 bytes) so no uninitialized register pointers trigger GC faults
    std::ptr::write_bytes(lp_context as *mut u8, 0, 1232);
    let ctx = lp_context as *mut u8;

    let _ = ensure_main_thread_entry();

    let cur_tid = CURRENT_THREAD_ENTRY.with(|cell| {
        cell.borrow().as_ref().map(|e| e.thread_id)
    }).unwrap_or_else(|| libc::syscall(libc::SYS_gettid) as u32);

    let is_current = (h_thread as i64) == -2
        || h_thread == 0xFFFFFFFFFFFFFFFE
        || h_thread == 0xFFFFFFFFFFFFFFFF
        || h_thread == ((cur_tid as u64) << 4 | 0x3)
        || h_thread == (cur_tid as u64);

    let (rax, rbx, rcx, rdx, rsi, rdi, rsp, rbp, r8, r9, r10, r11, r12, r13, r14, r15, rip) = if is_current {
        let mut cur_rsp: u64 = 0;
        let mut cur_rbp: u64 = 0;
        let mut cur_rax: u64 = 0;
        let mut cur_rbx: u64 = 0;
        let mut cur_rcx: u64 = 0;
        let mut cur_rdx: u64 = 0;
        let mut cur_rsi: u64 = 0;
        let mut cur_rdi: u64 = 0;
        let mut cur_r8: u64 = 0;
        let mut cur_r9: u64 = 0;
        let mut cur_r10: u64 = 0;
        let mut cur_r11: u64 = 0;
        let mut cur_r12: u64 = 0;
        let mut cur_r13: u64 = 0;
        let mut cur_r14: u64 = 0;
        let mut cur_r15: u64 = 0;
        std::arch::asm!("mov {}, rsp", out(reg) cur_rsp);
        std::arch::asm!("mov {}, rbp", out(reg) cur_rbp);
        std::arch::asm!("mov {}, rax", out(reg) cur_rax);
        std::arch::asm!("mov {}, rbx", out(reg) cur_rbx);
        std::arch::asm!("mov {}, rcx", out(reg) cur_rcx);
        std::arch::asm!("mov {}, rdx", out(reg) cur_rdx);
        std::arch::asm!("mov {}, rsi", out(reg) cur_rsi);
        std::arch::asm!("mov {}, rdi", out(reg) cur_rdi);
        std::arch::asm!("mov {}, r8", out(reg) cur_r8);
        std::arch::asm!("mov {}, r9", out(reg) cur_r9);
        std::arch::asm!("mov {}, r10", out(reg) cur_r10);
        std::arch::asm!("mov {}, r11", out(reg) cur_r11);
        std::arch::asm!("mov {}, r12", out(reg) cur_r12);
        std::arch::asm!("mov {}, r13", out(reg) cur_r13);
        std::arch::asm!("mov {}, r14", out(reg) cur_r14);
        std::arch::asm!("mov {}, r15", out(reg) cur_r15);

        let (stack_limit, stack_base) = crate::sys::get_current_thread_stack_bounds();
        let safe_rsp = if cur_rsp >= stack_limit && cur_rsp <= stack_base {
            cur_rsp
        } else {
            stack_limit + 0x1000
        };
        let safe_rbp = if cur_rbp >= stack_limit && cur_rbp <= stack_base {
            cur_rbp
        } else {
            stack_base
        };
        (cur_rax, cur_rbx, cur_rcx, cur_rdx, cur_rsi, cur_rdi, safe_rsp, safe_rbp, cur_r8, cur_r9, cur_r10, cur_r11, cur_r12, cur_r13, cur_r14, cur_r15, 0x140001000u64)
    } else {
        let entry_opt = {
            let guard = THREAD_HANDLE_TABLE.lock().unwrap();
            guard.as_ref().and_then(|table| table.get(&h_thread).cloned())
        }.or_else(|| {
            let tid = (h_thread >> 4) as u32;
            ALL_ACTIVE_THREADS.lock().ok().and_then(|guard| {
                guard.iter().find(|e| {
                    e.thread_id == tid
                    || e.host_tid.load(Ordering::Relaxed) == tid as u64
                    || e.thread_id == (h_thread as u32)
                    || e.host_tid.load(Ordering::Relaxed) == h_thread
                }).cloned()
            })
        });

        if let Some(entry) = entry_opt {
            let mut base = entry.stack_base.load(Ordering::SeqCst);
            let mut limit = entry.stack_limit.load(Ordering::SeqCst);
            if base == 0 {
                for _ in 0..100 {
                    std::thread::yield_now();
                    base = entry.stack_base.load(Ordering::SeqCst);
                    limit = entry.stack_limit.load(Ordering::SeqCst);
                    if base != 0 {
                        break;
                    }
                }
            }
            let recorded_rsp = entry.last_rsp.load(Ordering::SeqCst);
            let recorded_rbp = entry.last_rbp.load(Ordering::SeqCst);
            let safe_rsp = if base > 0 && limit > 0 && recorded_rsp >= limit && recorded_rsp <= base {
                recorded_rsp & !0xF
            } else if base > 0 && limit > 0 && base > limit {
                (base.saturating_sub(0x10000)).max(limit + 0x1000) & !0xF
            } else if base > 0 {
                (base.saturating_sub(0x1000)) & !0xF
            } else {
                0x7FFF_FE00_0000
            };
            let safe_rbp = if base > 0 && limit > 0 && recorded_rbp >= limit && recorded_rbp <= base {
                recorded_rbp & !0xF
            } else if base > 0 {
                base
            } else {
                0x7FFF_FF00_0000
            };
            let rax = entry.last_rax.load(Ordering::SeqCst);
            let rbx = entry.last_rbx.load(Ordering::SeqCst);
            let rcx = entry.last_rcx.load(Ordering::SeqCst);
            let rdx = entry.last_rdx.load(Ordering::SeqCst);
            let rsi = entry.last_rsi.load(Ordering::SeqCst);
            let rdi = entry.last_rdi.load(Ordering::SeqCst);
            let r8 = entry.last_r8.load(Ordering::SeqCst);
            let r9 = entry.last_r9.load(Ordering::SeqCst);
            let r10 = entry.last_r10.load(Ordering::SeqCst);
            let r11 = entry.last_r11.load(Ordering::SeqCst);
            let r12 = entry.last_r12.load(Ordering::SeqCst);
            let r13 = entry.last_r13.load(Ordering::SeqCst);
            let r14 = entry.last_r14.load(Ordering::SeqCst);
            let r15 = entry.last_r15.load(Ordering::SeqCst);
            let mut rip = entry.last_rip.load(Ordering::SeqCst);
            if rip == 0 {
                rip = 0x140001000;
            }
            (rax, rbx, rcx, rdx, rsi, rdi, safe_rsp, safe_rbp, r8, r9, r10, r11, r12, r13, r14, r15, rip)
        } else {
            SetLastError(6); // ERROR_INVALID_HANDLE
            return 0; // Return FALSE on unknown handle
        }
    };

    // ContextFlags at offset 0x30
    *((ctx.add(0x30)) as *mut u32) = 0x10007F; // CONTEXT_FULL | CONTEXT_ALL
    // Win64 CONTEXT general-purpose registers
    *((ctx.add(0x78)) as *mut u64) = rax;
    *((ctx.add(0x80)) as *mut u64) = rcx;
    *((ctx.add(0x88)) as *mut u64) = rdx;
    *((ctx.add(0x90)) as *mut u64) = rbx;
    *((ctx.add(0x98)) as *mut u64) = rsp;
    *((ctx.add(0xA0)) as *mut u64) = rbp;
    *((ctx.add(0xA8)) as *mut u64) = rsi;
    *((ctx.add(0xB0)) as *mut u64) = rdi;
    *((ctx.add(0xB8)) as *mut u64) = r8;
    *((ctx.add(0xC0)) as *mut u64) = r9;
    *((ctx.add(0xC8)) as *mut u64) = r10;
    *((ctx.add(0xD0)) as *mut u64) = r11;
    *((ctx.add(0xD8)) as *mut u64) = r12;
    *((ctx.add(0xE0)) as *mut u64) = r13;
    *((ctx.add(0xE8)) as *mut u64) = r14;
    *((ctx.add(0xF0)) as *mut u64) = r15;
    *((ctx.add(0xF8)) as *mut u64) = rip;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetThreadContext(_h_thread: u64, _lp_context: *const libc::c_void) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn Sleep(dw_milliseconds: u32) {
    update_current_thread_stack_context();
    std::thread::sleep(std::time::Duration::from_millis(dw_milliseconds as u64));
}

#[no_mangle]
pub unsafe extern "win64" fn SleepEx(dw_milliseconds: u32, _b_alertable: i32) -> u32 {
    Sleep(dw_milliseconds);
    0 // WAIT_IO_COMPLETION would be 0xC0
}


#[no_mangle]
pub unsafe extern "win64" fn GetProcAddress(
    h_module: usize,
    lp_proc_name: *const libc::c_char,
) -> usize {
    if lp_proc_name.is_null() {
        return 0;
    }
    let func_name = if (lp_proc_name as usize) < 0x10000 {
        format!("#{}", lp_proc_name as usize)
    } else {
        std::ffi::CStr::from_ptr(lp_proc_name)
            .to_string_lossy()
            .to_string()
    };

    let module_name = if h_module == 0 {
        if func_name.starts_with("Nt") || func_name.starts_with("Zw") || func_name.starts_with("Rtl") {
            "ntdll.dll".to_string()
        } else if let Some((comp_name, _)) = crate::mem::loader::find_companion_export_anywhere(&func_name) {
            comp_name
        } else if func_name.starts_with("Steam_") || func_name.starts_with("SteamAPI_") {
            "steamclient64.dll".to_string()
        } else {
            ModuleRegistry::get_module_name(h_module)
                .unwrap_or_else(|| format!("HMODULE_0x{:016X}", h_module))
        }
    } else {
        ModuleRegistry::get_module_name(h_module)
            .unwrap_or_else(|| format!("HMODULE_0x{:016X}", h_module))
    };

    // If caller probes with accidental/probe double 'WW' suffix (common in CLR/Mono P/Invoke name resolution),
    // reject immediately so the caller falls back to the canonical 'W' name.
    if func_name.ends_with("WW") {
        println!(
            "[RINE GETPROCADDRESS REJECT WW] hmodule=0x{:016X} module='{}' symbol='{}'",
            h_module, module_name, func_name
        );
        SetLastError(127);
        return 0;
    }

    let mod_upper = module_name.to_uppercase();
    if mod_upper.contains("XINPUT") {
        let norm_name = if func_name.starts_with('#') {
            match func_name.as_str() {
                "#100" | "#1" => "XInputGetStateEx",
                "#101" => "XInputWaitForGuideButton",
                "#102" => "XInputCancelGuideButtonWait",
                "#103" => "XInputPowerOffController",
                "#2" => "XInputGetState",
                "#3" => "XInputSetState",
                "#4" => "XInputGetCapabilities",
                "#5" => "XInputEnable",
                "#6" => "XInputGetBatteryInformation",
                "#7" => "XInputGetKeystroke",
                "#8" => "XInputGetAudioDeviceIds",
                "#9" => "XInputGetDSoundAudioDeviceGuids",
                _ => func_name.as_str(),
            }
        } else {
            let mut s = func_name.as_str();
            if let Some(stripped) = s.strip_prefix('_') {
                s = stripped;
            }
            if let Some(idx) = s.find('@') {
                s = &s[..idx];
            }
            if s.starts_with("XInput") {
                match s {
                    "XInputGetStateA" | "XInputGetStateW" => "XInputGetState",
                    "XInputSetStateA" | "XInputSetStateW" => "XInputSetState",
                    "XInputGetCapabilitiesA" | "XInputGetCapabilitiesW" => "XInputGetCapabilities",
                    "XInputGetBatteryInformationA" | "XInputGetBatteryInformationW" => "XInputGetBatteryInformation",
                    "XInputGetKeystrokeA" | "XInputGetKeystrokeW" => "XInputGetKeystroke",
                    "XInputGetAudioDeviceIdsA" | "XInputGetAudioDeviceIdsW" => "XInputGetAudioDeviceIds",
                    "XInputGetDSoundAudioDeviceGuidsA" | "XInputGetDSoundAudioDeviceGuidsW" => "XInputGetDSoundAudioDeviceGuids",
                    _ => s,
                }
            } else {
                s
            }
        };

        if let Some(resolved) = crate::pe::imports::IatSymbolResolver::resolve_symbol("xinput1_3.dll", norm_name) {
            println!(
                "[RINE XINPUT GETPROCADDRESS] hmodule=0x{:016X} raw='{}' normalized='{}' addr=0x{:016X}",
                h_module, func_name, norm_name, resolved
            );
            return resolved;
        }
    }

    if module_name.to_ascii_lowercase().starts_with("cfgmgr32")
        || module_name.to_ascii_lowercase().starts_with("setupapi")
    {
        if func_name.starts_with("CM_") || func_name.starts_with("_CM_") {
            if let Some(addr) = crate::dlls::cfgmgr32::resolve_cfgmgr32_symbol(&func_name) {
                println!(
                    "[RINE CFGMGR32 GETPROCADDRESS] hmodule=0x{:016X} symbol='{}' addr=0x{:016X}",
                    h_module, func_name, addr
                );
                return addr;
            }
        }
    }

    if mod_upper.starts_with("STEAM_API")
        || mod_upper.starts_with("STEAMCLIENT")
        || mod_upper.starts_with("LSTEAMCLIENT")
        || mod_upper.starts_with("CSTEAMWORKS")
        || func_name.starts_with("SteamAPI_")
        || func_name.starts_with("SteamInternal_")
        || func_name.starts_with("Steam_")
        || func_name.starts_with("ISteam")
        || func_name == "InitSafe"
        || func_name == "RestartAppIfNecessary"
        || func_name == "IsSteamRunning"
    {
        if let Some(addr) = crate::dlls::steam::resolve_steam_symbol(&func_name) {
            println!("[RINE STEAM BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X}", h_module, module_name, func_name, addr);
            return addr;
        }
    }

    if module_name.eq_ignore_ascii_case("d3d11.dll") || module_name.eq_ignore_ascii_case("d3d11") {
        if let Some(addr) = crate::dlls::dxvk_loader::resolve_dxvk_symbol("d3d11.dll", &func_name) {
            println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
            return addr;
        }
    }
    if module_name.eq_ignore_ascii_case("dxgi.dll") || module_name.eq_ignore_ascii_case("dxgi") {
        if let Some(addr) = crate::dlls::dxvk_loader::resolve_dxvk_symbol("dxgi.dll", &func_name) {
            println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
            return addr;
        }
    }
    if module_name.eq_ignore_ascii_case("vulkan-1.dll")
        || module_name.eq_ignore_ascii_case("vulkan-1")
    {
        if let Some(addr) = crate::dlls::dxvk_loader::resolve_vulkan_symbol(&func_name) {
            println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} stub_id=0", h_module, module_name, func_name, addr);
            return addr;
        }
    }

    if let Some(resolved) =
        crate::providers::RineApiResolver::resolve_symbol(&module_name, &func_name)
    {
        // Never return the CRT catch-all no-op generic stub from GetProcAddress; missing exports must return 0
        if resolved.address != crate::dlls::crt::msvcrt_generic_stub as *const () as usize as u64 {
            println!("[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X} provider={:?}", h_module, module_name, func_name, resolved.address, resolved.provider);
            return resolved.address as usize;
        }
    }

    // Fallback: Demangle stdcall (_foo@N) if caller is Mono / CLR P/Invoke
    let demangled_sym = if func_name.starts_with('_') || func_name.contains('@') {
        let mut s = func_name.as_str();
        if let Some(stripped) = s.strip_prefix('_') {
            s = stripped;
        }
        if let Some(idx) = s.find('@') {
            s = &s[..idx];
        }
        Some(s)
    } else {
        None
    };

    if let Some(demangled) = demangled_sym {
        if let Some(resolved) = crate::providers::RineApiResolver::resolve_symbol(&module_name, demangled) {
            if resolved.address != crate::dlls::crt::msvcrt_generic_stub as *const () as usize as u64 {
                println!("[RINE GETPROCADDRESS DEMANGLED BIND] hmodule=0x{:016X} module='{}' raw='{}' demangled='{}' returned_addr=0x{:016X}", h_module, module_name, func_name, demangled, resolved.address);
                return resolved.address as usize;
            }
        }
    }

    // Fallback: Strip trailing W or A for canonical APIs like FreeLibraryW -> FreeLibrary
    let stripped_suffix = if (func_name.ends_with('W') || func_name.ends_with('A')) && func_name.len() > 2 {
        Some(&func_name[..func_name.len() - 1])
    } else {
        None
    };

    if let Some(stripped) = stripped_suffix {
        if let Some(resolved) = crate::providers::RineApiResolver::resolve_symbol(&module_name, stripped) {
            if resolved.address != crate::dlls::crt::msvcrt_generic_stub as *const () as usize as u64 {
                println!("[RINE GETPROCADDRESS STRIPPED BIND] hmodule=0x{:016X} module='{}' raw='{}' stripped='{}' returned_addr=0x{:016X}", h_module, module_name, func_name, stripped, resolved.address);
                return resolved.address as usize;
            }
        }
    }

    if let Some(addr) = crate::mem::loader::LoadedModule::lookup_companion_export(&module_name, &func_name) {
        println!("[RINE GETPROCADDRESS COMPANION BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x{:016X}", h_module, module_name, func_name, addr);
        return addr;
    }

    let is_main_or_exe = h_module == 0 || module_name.to_ascii_lowercase().ends_with(".exe");
    if is_main_or_exe {
        if let Some((comp_name, addr)) = crate::mem::loader::find_companion_export_anywhere(&func_name) {
            println!("[RINE GETPROCADDRESS GLOBAL COMPANION BIND] hmodule=0x{:016X} resolved_module='{}' symbol='{}' returned_addr=0x{:016X}", h_module, comp_name, func_name, addr);
            return addr;
        }

        for fallback_dll in &[
            "KERNEL32.DLL",
            "KERNELBASE.DLL",
            "USER32.DLL",
            "NTDLL.DLL",
            "ADVAPI32.DLL",
            "OLE32.DLL",
        ] {
            if let Some(resolved) =
                crate::providers::RineApiResolver::resolve_symbol(fallback_dll, &func_name)
            {
                if resolved.address != crate::dlls::crt::msvcrt_generic_stub as *const () as usize as u64 {
                    println!("[RINE GETPROCADDRESS FALLBACK BIND] hmodule=0x{:016X} fallback_dll='{}' symbol='{}' returned_addr=0x{:016X}", h_module, fallback_dll, func_name, resolved.address);
                    return resolved.address as usize;
                }
            }
            if let Some(demangled) = demangled_sym {
                if let Some(resolved) =
                    crate::providers::RineApiResolver::resolve_symbol(fallback_dll, demangled)
                {
                    if resolved.address != crate::dlls::crt::msvcrt_generic_stub as *const () as usize as u64 {
                        println!("[RINE GETPROCADDRESS FALLBACK DEMANGLED BIND] hmodule=0x{:016X} fallback_dll='{}' raw='{}' demangled='{}' returned_addr=0x{:016X}", h_module, fallback_dll, func_name, demangled, resolved.address);
                        return resolved.address as usize;
                    }
                }
            }
            if let Some(stripped) = stripped_suffix {
                if let Some(resolved) =
                    crate::providers::RineApiResolver::resolve_symbol(fallback_dll, stripped)
                {
                    if resolved.address != crate::dlls::crt::msvcrt_generic_stub as *const () as usize as u64 {
                        println!("[RINE GETPROCADDRESS FALLBACK STRIPPED BIND] hmodule=0x{:016X} fallback_dll='{}' raw='{}' stripped='{}' returned_addr=0x{:016X}", h_module, fallback_dll, func_name, stripped, resolved.address);
                        return resolved.address as usize;
                    }
                }
            }
        }
    }

    println!(
        "[RINE GETPROCADDRESS BIND] hmodule=0x{:016X} module='{}' symbol='{}' returned_addr=0x0 stub_id=0 (UNRESOLVED)",
        h_module, module_name, func_name
    );
    SetLastError(127); // ERROR_PROC_NOT_FOUND
    0
}

#[no_mangle]
pub unsafe extern "win64" fn IsDBCSLeadByteEx(_code_page: u32, _test_char: u8) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetTempPathW(nBufferLength: u32, lpBuffer: *mut u16) -> u32 {
    let temp_str = "C:\\users\\fizzy\\AppData\\Local\\Temp\\\0";
    let utf16: Vec<u16> = temp_str.encode_utf16().collect();
    let needed = (utf16.len() - 1) as u32;
    if nBufferLength < needed + 1 || lpBuffer.is_null() {
        return needed + 1;
    }
    for (i, &ch) in utf16.iter().enumerate() {
        *lpBuffer.add(i) = ch;
    }
    needed
}

#[no_mangle]
pub unsafe extern "win64" fn GetTempPath2W(nBufferLength: u32, lpBuffer: *mut u16) -> u32 {
    GetTempPathW(nBufferLength, lpBuffer)
}

#[no_mangle]
pub unsafe extern "win64" fn GetTempPathA(nBufferLength: u32, lpBuffer: *mut u8) -> u32 {
    let temp_str = b"C:\\users\\fizzy\\AppData\\Local\\Temp\\\0";
    let needed = (temp_str.len() - 1) as u32;
    if nBufferLength < needed + 1 || lpBuffer.is_null() {
        return needed + 1;
    }
    for (i, &b) in temp_str.iter().enumerate() {
        *lpBuffer.add(i) = b;
    }
    needed
}

#[no_mangle]
pub unsafe extern "win64" fn GetTempPath2A(nBufferLength: u32, lpBuffer: *mut u8) -> u32 {
    GetTempPathA(nBufferLength, lpBuffer)
}

#[no_mangle]
pub unsafe extern "win64" fn FormatMessageW(
    dw_flags: u32,
    _lp_source: *const std::ffi::c_void,
    dw_message_id: u32,
    _dw_language_id: u32,
    lp_buffer: *mut u16,
    n_size: u32,
    _arguments: *mut std::ffi::c_void,
) -> u32 {
    const FORMAT_MESSAGE_ALLOCATE_BUFFER: u32 = 0x00000100;

    let msg_str = match dw_message_id {
        0 => "The operation completed successfully.\r\n",
        1 => "Incorrect function.\r\n",
        2 => "The system cannot find the file specified.\r\n",
        3 => "The system cannot find the path specified.\r\n",
        5 => "Access is denied.\r\n",
        6 => "The handle is invalid.\r\n",
        8 => "Not enough storage is available to process this command.\r\n",
        14 => "Not enough storage is available to complete this operation.\r\n",
        87 => "The parameter is incorrect.\r\n",
        126 => "The specified module could not be found.\r\n",
        127 => "The specified procedure could not be found.\r\n",
        258 => "The wait operation timed out.\r\n",
        _ => "System error occurred.\r\n",
    };

    let wide: Vec<u16> = msg_str.encode_utf16().chain(std::iter::once(0)).collect();
    let char_count = wide.len() - 1;

    if (dw_flags & FORMAT_MESSAGE_ALLOCATE_BUFFER) != 0 {
        if lp_buffer.is_null() {
            SetLastError(87);
            return 0;
        }
        let byte_size = wide.len() * 2;
        let alloc_ptr = LocalAlloc(0x0040 /* LPTR */, byte_size) as *mut u16;
        if alloc_ptr.is_null() {
            SetLastError(8);
            return 0;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), alloc_ptr, wide.len());
        *(lp_buffer as *mut *mut u16) = alloc_ptr;
        println!(
            "[KERNEL32] FormatMessageW ALLOCATE_BUFFER: id={} -> '{}' (len={}, ptr={:p})",
            dw_message_id, msg_str.trim(), char_count, alloc_ptr
        );
        char_count as u32
    } else {
        if lp_buffer.is_null() || n_size == 0 {
            SetLastError(87);
            return 0;
        }
        let copy_len = std::cmp::min(char_count, (n_size as usize).saturating_sub(1));
        std::ptr::copy_nonoverlapping(wide.as_ptr(), lp_buffer, copy_len);
        *lp_buffer.add(copy_len) = 0;
        println!(
            "[KERNEL32] FormatMessageW BUFFER: id={} -> '{}' (len={})",
            dw_message_id, msg_str.trim(), copy_len
        );
        copy_len as u32
    }
}

#[no_mangle]
pub unsafe extern "win64" fn FormatMessageA(
    dw_flags: u32,
    _lp_source: *const std::ffi::c_void,
    dw_message_id: u32,
    _dw_language_id: u32,
    lp_buffer: *mut u8,
    n_size: u32,
    _arguments: *mut std::ffi::c_void,
) -> u32 {
    const FORMAT_MESSAGE_ALLOCATE_BUFFER: u32 = 0x00000100;

    let msg_str = match dw_message_id {
        0 => "The operation completed successfully.\r\n",
        1 => "Incorrect function.\r\n",
        2 => "The system cannot find the file specified.\r\n",
        3 => "The system cannot find the path specified.\r\n",
        5 => "Access is denied.\r\n",
        6 => "The handle is invalid.\r\n",
        8 => "Not enough storage is available to process this command.\r\n",
        14 => "Not enough storage is available to complete this operation.\r\n",
        87 => "The parameter is incorrect.\r\n",
        126 => "The specified module could not be found.\r\n",
        127 => "The specified procedure could not be found.\r\n",
        258 => "The wait operation timed out.\r\n",
        _ => "System error occurred.\r\n",
    };

    let bytes = msg_str.as_bytes();
    let char_count = bytes.len();

    if (dw_flags & FORMAT_MESSAGE_ALLOCATE_BUFFER) != 0 {
        if lp_buffer.is_null() {
            SetLastError(87);
            return 0;
        }
        let alloc_ptr = LocalAlloc(0x0040 /* LPTR */, char_count + 1) as *mut u8;
        if alloc_ptr.is_null() {
            SetLastError(8);
            return 0;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), alloc_ptr, char_count);
        *alloc_ptr.add(char_count) = 0;
        *(lp_buffer as *mut *mut u8) = alloc_ptr;
        println!(
            "[KERNEL32] FormatMessageA ALLOCATE_BUFFER: id={} -> '{}' (len={}, ptr={:p})",
            dw_message_id, msg_str.trim(), char_count, alloc_ptr
        );
        char_count as u32
    } else {
        if lp_buffer.is_null() || n_size == 0 {
            SetLastError(87);
            return 0;
        }
        let copy_len = std::cmp::min(char_count, (n_size as usize).saturating_sub(1));
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), lp_buffer, copy_len);
        *lp_buffer.add(copy_len) = 0;
        println!(
            "[KERNEL32] FormatMessageA BUFFER: id={} -> '{}' (len={})",
            dw_message_id, msg_str.trim(), copy_len
        );
        copy_len as u32
    }
}

#[no_mangle]
pub unsafe extern "win64" fn EnumSystemLocalesEx(
    lpLocaleEnumProcEx: Option<unsafe extern "win64" fn(*const u16, u32, usize) -> i32>,
    dwFlags: u32,
    lParam: usize,
    _lpReserved: *mut std::ffi::c_void,
) -> i32 {
    if let Some(cb) = lpLocaleEnumProcEx {
        let locales = ["en-US\0", "ru-RU\0"];
        for loc in &locales {
            let wide: Vec<u16> = loc.encode_utf16().collect();
            if cb(wide.as_ptr(), dwFlags, lParam) == 0 {
                break;
            }
        }
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetDateFormatEx(
    _lpLocaleName: *const u16,
    _dwFlags: u32,
    _lpDate: *const std::ffi::c_void,
    _lpFormat: *const u16,
    lpDateStr: *mut u16,
    cchDate: i32,
    _lpCalendar: *const std::ffi::c_void,
) -> i32 {
    let sample = "2026-11-22\0";
    let wide: Vec<u16> = sample.encode_utf16().collect();
    if cchDate == 0 {
        return wide.len() as i32;
    }
    if !lpDateStr.is_null() && cchDate >= wide.len() as i32 {
        for (i, &ch) in wide.iter().enumerate() {
            *lpDateStr.add(i) = ch;
        }
        return wide.len() as i32;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetTimeFormatEx(
    _lpLocaleName: *const u16,
    _dwFlags: u32,
    _lpTime: *const std::ffi::c_void,
    _lpFormat: *const u16,
    lpTimeStr: *mut u16,
    cchTime: i32,
) -> i32 {
    let sample = "12:00:00\0";
    let wide: Vec<u16> = sample.encode_utf16().collect();
    if cchTime == 0 {
        return wide.len() as i32;
    }
    if !lpTimeStr.is_null() && cchTime >= wide.len() as i32 {
        for (i, &ch) in wide.iter().enumerate() {
            *lpTimeStr.add(i) = ch;
        }
        return wide.len() as i32;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn VerifyVersionInfoW(
    _lpVersionInformation: *const std::ffi::c_void,
    _dwTypeMask: u32,
    _dwlConditionMask: u64,
) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn VerifyVersionInfoA(
    _lpVersionInformation: *const std::ffi::c_void,
    _dwTypeMask: u32,
    _dwlConditionMask: u64,
) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn ProcessIdToSessionId(_dw_process_id: u32, p_session_id: *mut u32) -> i32 {
    if !p_session_id.is_null() {
        *p_session_id = 1;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn DeviceIoControl(
    _h_device: usize,
    _dw_io_control_code: u32,
    _lp_in_buffer: *const std::ffi::c_void,
    _n_in_buffer_size: u32,
    _lp_out_buffer: *mut std::ffi::c_void,
    _n_out_buffer_size: u32,
    lp_bytes_returned: *mut u32,
    _lp_overlapped: *mut std::ffi::c_void,
) -> i32 {
    if !lp_bytes_returned.is_null() {
        *lp_bytes_returned = 0;
    }
    1
}

static DLL_DIRECTORIES: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn get_dll_directories() -> Vec<String> {
    DLL_DIRECTORIES.lock().unwrap().clone()
}

#[no_mangle]
pub unsafe extern "win64" fn SetDllDirectoryW(lp_path_name: *const u16) -> i32 {
    let mut guard = DLL_DIRECTORIES.lock().unwrap();
    guard.clear();
    if !lp_path_name.is_null() {
        let mut len = 0;
        while *lp_path_name.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(lp_path_name, len);
        let s = String::from_utf16_lossy(slice);
        if !s.is_empty() {
            let host_path = win32_to_host_path(&s);
            println!("[KERNEL32] SetDllDirectoryW('{}' -> '{}')", s, host_path);
            guard.push(host_path);
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetDllDirectoryA(lp_path_name: *const libc::c_char) -> i32 {
    let mut guard = DLL_DIRECTORIES.lock().unwrap();
    guard.clear();
    if !lp_path_name.is_null() {
        let s = std::ffi::CStr::from_ptr(lp_path_name).to_string_lossy().to_string();
        if !s.is_empty() {
            let host_path = win32_to_host_path(&s);
            println!("[KERNEL32] SetDllDirectoryA('{}' -> '{}')", s, host_path);
            guard.push(host_path);
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryW(lp_lib_name: *const u16) -> usize {
    if lp_lib_name.is_null() {
        SetLastError(126); // ERROR_MOD_NOT_FOUND
        return 0;
    }
    let name_opt = read_utf16_ptr(lp_lib_name);
    let name = match name_opt {
        Some(s) => s,
        None => {
            SetLastError(126);
            return 0;
        }
    };
    println!("[KERNEL32] LoadLibraryW('{}')", name);
    let base = ModuleRegistry::load_library(&name);
    if base == 0 {
        SetLastError(126);
    }
    base
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryA(lp_lib_name: *const libc::c_char) -> usize {
    if lp_lib_name.is_null() {
        SetLastError(126);
        return 0;
    }
    let name = std::ffi::CStr::from_ptr(lp_lib_name)
        .to_string_lossy()
        .to_string();
    println!("[KERNEL32] LoadLibraryA('{}')", name);
    let base = ModuleRegistry::load_library(&name);
    if base == 0 {
        SetLastError(126);
    }
    base
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryExW(
    lp_lib_name: *const u16,
    _h_file: usize,
    _flags: u32,
) -> usize {
    LoadLibraryW(lp_lib_name)
}

#[no_mangle]
pub unsafe extern "win64" fn LoadLibraryExA(
    lp_lib_name: *const libc::c_char,
    _h_file: usize,
    _flags: u32,
) -> usize {
    LoadLibraryA(lp_lib_name)
}

#[no_mangle]
pub unsafe extern "win64" fn FreeLibrary(h_lib_module: usize) -> i32 {
    println!("[KERNEL32] FreeLibrary(0x{:016X})", h_lib_module);
    if ModuleRegistry::free_library(h_lib_module) {
        1
    } else {
        SetLastError(6); // ERROR_INVALID_HANDLE
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn FreeLibraryW(h_lib_module: usize) -> i32 {
    FreeLibrary(h_lib_module)
}

#[no_mangle]
pub unsafe extern "win64" fn FreeLibraryA(h_lib_module: usize) -> i32 {
    FreeLibrary(h_lib_module)
}

#[no_mangle]
pub unsafe extern "win64" fn DisableThreadLibraryCalls(_h_lib_module: usize) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetTickCount() -> u32 {
    static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START_TIME.get_or_init(std::time::Instant::now);
    start.elapsed().as_millis() as u32
}

#[no_mangle]
pub unsafe extern "win64" fn IsBadStringPtrW(_lpsz: *const u16, _ucch_max: usize) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn RaiseException(
    dw_exception_code: u32,
    dw_exception_flags: u32,
    n_number_of_arguments: u32,
    lp_arguments: *const usize,
) {
    if dw_exception_code == 0x406D1388 {
        // MS_VC_EXCEPTION: SetThreadName
        if !lp_arguments.is_null() && n_number_of_arguments >= 2 {
            let name_ptr = *lp_arguments.add(1) as *const libc::c_char;
            if !name_ptr.is_null() && (name_ptr as usize) > 0x10000 && (name_ptr as usize) < 0x7FFFFFFFFFFF {
                if let Ok(s) = std::ffi::CStr::from_ptr(name_ptr).to_str() {
                    println!("[KERNEL32] SetThreadName (MS_VC_EXCEPTION): '{}'", s);
                    return;
                }
            }
        }
        println!("[KERNEL32] SetThreadName (MS_VC_EXCEPTION)");
        return;
    }

    println!(
        "[KERNEL32] RaiseException: code=0x{:08X}, flags=0x{:X}, num_args={}",
        dw_exception_code, dw_exception_flags, n_number_of_arguments
    );
    if !lp_arguments.is_null() && n_number_of_arguments > 0 {
        let arg0 = *lp_arguments;
        println!("  lp_arguments[0]=0x{:016X}", arg0);
    }
}

static TOP_LEVEL_EXCEPTION_FILTER: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[no_mangle]
pub unsafe extern "win64" fn SetUnhandledExceptionFilter(
    lp_top_level_exception_filter: usize,
) -> usize {
    TOP_LEVEL_EXCEPTION_FILTER.swap(
        lp_top_level_exception_filter,
        std::sync::atomic::Ordering::SeqCst,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn UnhandledExceptionFilter(
    _exception_info: *mut std::ffi::c_void,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentProcess() -> usize {
    0xFFFFFFFFFFFFFFFF
}

#[no_mangle]
pub unsafe extern "win64" fn GetThreadPriority(_h_thread: usize) -> i32 {
    0 // THREAD_PRIORITY_NORMAL
}

#[no_mangle]
pub unsafe extern "win64" fn DuplicateHandle(
    _h_source_process_handle: usize,
    h_source_handle: usize,
    _h_target_process_handle: usize,
    lp_target_handle: *mut usize,
    _dw_desired_access: u32,
    _b_inherit_handle: u32,
    _dw_options: u32,
) -> i32 {
    if lp_target_handle.is_null() {
        return 0;
    }

    let src_h = h_source_handle as u64;
    let dup_h = if (src_h as i64) == -2 || src_h == 0xFFFFFFFFFFFFFFFE {
        // Pseudo-handle for GetCurrentThread() -> duplicate to real thread handle!
        let _ = ensure_main_thread_entry();
        let cur_tid = CURRENT_THREAD_ENTRY.with(|cell| {
            cell.borrow().as_ref().map(|e| e.thread_id)
        }).unwrap_or_else(|| libc::syscall(libc::SYS_gettid) as u32);
        let real_h = (cur_tid as u64) << 4 | 0x3;
        real_h
    } else if (src_h as i64) == -1 || src_h == 0xFFFFFFFFFFFFFFFF {
        0x10000003u64
    } else {
        src_h
    };

    *lp_target_handle = dup_h as usize;
    1
}

#[no_mangle]
pub unsafe extern "win64" fn WideCharToMultiByte(
    _code_page: u32,
    _dw_flags: u32,
    lp_wide_char_str: *const u16,
    cch_wide_char: i32,
    lp_multi_byte_str: *mut u8,
    cb_multi_byte: i32,
    _lp_default_char: *const u8,
    _lp_used_default_char: *mut i32,
) -> i32 {
    if lp_wide_char_str.is_null() {
        return 0;
    }
    let len = if cch_wide_char == -1 {
        let mut l = 0;
        while *lp_wide_char_str.add(l) != 0 {
            l += 1;
        }
        l + 1
    } else if cch_wide_char < 0 {
        return 0;
    } else {
        cch_wide_char as usize
    };

    let slice = std::slice::from_raw_parts(lp_wide_char_str, len);
    let mut out_bytes = Vec::with_capacity(len);
    for &wc in slice {
        if wc <= 0x7F {
            out_bytes.push(wc as u8);
        } else if wc <= 0x7FF {
            out_bytes.push((0xC0 | (wc >> 6)) as u8);
            out_bytes.push((0x80 | (wc & 0x3F)) as u8);
        } else {
            out_bytes.push((0xE0 | (wc >> 12)) as u8);
            out_bytes.push((0x80 | ((wc >> 6) & 0x3F)) as u8);
            out_bytes.push((0x80 | (wc & 0x3F)) as u8);
        }
    }

    if cb_multi_byte == 0 || lp_multi_byte_str.is_null() || (lp_multi_byte_str as usize) < 0x10000 {
        out_bytes.len() as i32
    } else if (cb_multi_byte as usize) < out_bytes.len() {
        SetLastError(122); // ERROR_INSUFFICIENT_BUFFER
        0
    } else {
        std::ptr::copy_nonoverlapping(out_bytes.as_ptr(), lp_multi_byte_str, out_bytes.len());
        out_bytes.len() as i32
    }
}

#[no_mangle]
pub unsafe extern "win64" fn MultiByteToWideChar(
    _code_page: u32,
    _dw_flags: u32,
    lp_multi_byte_str: *const u8,
    cb_multi_byte: i32,
    lp_wide_char_str: *mut u16,
    cch_wide_char: i32,
) -> i32 {
    if lp_multi_byte_str.is_null() || (lp_multi_byte_str as usize) < 0x10000 {
        return 0;
    }
    let len = if cb_multi_byte == -1 {
        let mut l = 0;
        while *lp_multi_byte_str.add(l) != 0 {
            l += 1;
        }
        l + 1
    } else if cb_multi_byte < 0 {
        return 0;
    } else {
        cb_multi_byte as usize
    };

    let slice = std::slice::from_raw_parts(lp_multi_byte_str, len);
    let mut out_u16 = Vec::with_capacity(len);
    let mut i = 0;
    while i < slice.len() {
        let b = slice[i];
        if b <= 0x7F {
            out_u16.push(b as u16);
            i += 1;
        } else if (0xC0..=0xDF).contains(&b) && i + 1 < slice.len() {
            let cp = (((b & 0x1F) as u16) << 6) | ((slice[i + 1] & 0x3F) as u16);
            out_u16.push(cp);
            i += 2;
        } else if (0xE0..=0xEF).contains(&b) && i + 2 < slice.len() {
            let cp = (((b & 0x0F) as u16) << 12)
                | (((slice[i + 1] & 0x3F) as u16) << 6)
                | ((slice[i + 2] & 0x3F) as u16);
            out_u16.push(cp);
            i += 3;
        } else {
            out_u16.push(b as u16);
            i += 1;
        }
    }

    if cch_wide_char == 0 || lp_wide_char_str.is_null() || (lp_wide_char_str as usize) < 0x10000 {
        return out_u16.len() as i32;
    }

    if (cch_wide_char as usize) < out_u16.len() {
        SetLastError(122); // ERROR_INSUFFICIENT_BUFFER
        return 0;
    }

    std::ptr::copy_nonoverlapping(out_u16.as_ptr(), lp_wide_char_str, out_u16.len());
    out_u16.len() as i32
}

#[no_mangle]
pub unsafe extern "win64" fn IsWow64Process(
    _h_process: usize,
    wow64_process: *mut i32,
) -> i32 {
    if !wow64_process.is_null() {
        *wow64_process = 0; // 0 = 64-bit native Windows process
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn GetProcessIdOfThread(_thread: usize) -> u32 {
    std::process::id()
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemWindowsDirectoryW(lp_buffer: *mut u16, u_size: u32) -> u32 {
    GetWindowsDirectoryW(lp_buffer, u_size)
}

#[no_mangle]
pub unsafe extern "win64" fn Module32First(
    _h_snapshot: usize,
    lpme: *mut libc::c_void,
) -> i32 {
    if lpme.is_null() { return 0; }
    std::ptr::write_bytes(lpme as *mut u8, 0, 568);
    *(lpme as *mut u32) = 568; // dwSize
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn Module32Next(
    _h_snapshot: usize,
    _lpme: *mut libc::c_void,
) -> i32 {
    0 // FALSE
}

#[no_mangle]
pub unsafe extern "win64" fn ConnectNamedPipe(
    _h_named_pipe: usize,
    _lp_overlapped: *mut libc::c_void,
) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn DisconnectNamedPipe(_h_named_pipe: usize) -> i32 {
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn CreateNamedPipeA(
    _lp_name: *const u8,
    _dw_open_mode: u32,
    _dw_pipe_mode: u32,
    _n_max_instances: u32,
    _n_out_buffer_size: u32,
    _n_in_buffer_size: u32,
    _n_default_time_out: u32,
    _lp_security_attributes: *mut libc::c_void,
) -> usize {
    0x2000
}

#[no_mangle]
pub unsafe extern "win64" fn WriteProcessMemory(
    _h_process: usize,
    lp_base_address: *mut libc::c_void,
    lp_buffer: *const libc::c_void,
    n_size: usize,
    lp_number_of_bytes_written: *mut usize,
) -> i32 {
    if lp_base_address.is_null() || lp_buffer.is_null() {
        return 0;
    }
    std::ptr::copy_nonoverlapping(lp_buffer as *const u8, lp_base_address as *mut u8, n_size);
    if !lp_number_of_bytes_written.is_null() {
        *lp_number_of_bytes_written = n_size;
    }
    1 // TRUE
}

#[no_mangle]
pub unsafe extern "win64" fn VirtualAllocEx(
    _h_process: usize,
    lp_address: *mut libc::c_void,
    dw_size: usize,
    fl_allocation_type: u32,
    fl_protect: u32,
) -> *mut libc::c_void {
    VirtualAlloc(lp_address as usize, dw_size, fl_allocation_type, fl_protect) as *mut libc::c_void
}

#[no_mangle]
pub unsafe extern "win64" fn lstrcmpA(lp_string1: *const u8, lp_string2: *const u8) -> i32 {
    if lp_string1.is_null() || lp_string2.is_null() { return 0; }
    let s1 = std::ffi::CStr::from_ptr(lp_string1 as *const i8);
    let s2 = std::ffi::CStr::from_ptr(lp_string2 as *const i8);
    s1.cmp(s2) as i32
}

#[no_mangle]
pub unsafe extern "win64" fn lstrcpyW(lp_string1: *mut u16, lp_string2: *const u16) -> *mut u16 {
    if lp_string1.is_null() || lp_string2.is_null() { return lp_string1; }
    let mut i = 0;
    loop {
        let ch = *lp_string2.add(i);
        *lp_string1.add(i) = ch;
        if ch == 0 { break; }
        i += 1;
    }
    lp_string1
}

#[no_mangle]
pub unsafe extern "win64" fn K32GetPerformanceInfo(p_performance_information: *mut u8, cb: u32) -> i32 {
    if !p_performance_information.is_null() && cb > 0 {
        std::ptr::write_bytes(p_performance_information, 0, cb as usize);
        *(p_performance_information as *mut u32) = cb;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn LCIDToLocaleName(
    _locale: u32,
    lp_name: *mut u16,
    cch_name: i32,
    _dw_flags: u32,
) -> i32 {
    let name: [u16; 6] = [b'e' as u16, b'n' as u16, b'-' as u16, b'U' as u16, b'S' as u16, 0];
    if !lp_name.is_null() && cch_name >= 6 {
        std::ptr::copy_nonoverlapping(name.as_ptr(), lp_name, 6);
        6
    } else {
        6
    }
}

#[no_mangle]
pub unsafe extern "win64" fn LocaleNameToLCID(_lp_name: *const u16, _dw_flags: u32) -> u32 {
    0x0409
}

#[no_mangle]
pub unsafe extern "win64" fn GetProcessAffinityMask(
    _h_process: *mut std::ffi::c_void,
    lp_process_affinity_mask: *mut usize,
    lp_system_affinity_mask: *mut usize,
) -> i32 {
    if !lp_process_affinity_mask.is_null() {
        *lp_process_affinity_mask = usize::MAX;
    }
    if !lp_system_affinity_mask.is_null() {
        *lp_system_affinity_mask = usize::MAX;
    }
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetProcessAffinityMask(
    _h_process: *mut std::ffi::c_void,
    _dw_process_affinity_mask: usize,
) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "win64" fn SetThreadIdealProcessor(
    _h_thread: *mut std::ffi::c_void,
    _dw_ideal_processor: u32,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetLargePageMinimum() -> usize {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetDynamicTimeZoneInformation(
    p_time_zone_information: *mut u8,
) -> u32 {
    if !p_time_zone_information.is_null() {
        std::ptr::write_bytes(p_time_zone_information, 0, 432);
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn GetGeoInfoW(
    _geo_id: u32,
    _geo_type: u32,
    lp_geo_data: *mut u16,
    cch_data: i32,
    _lang_id: u16,
) -> i32 {
    let val: [u16; 3] = [b'U' as u16, b'S' as u16, 0];
    if !lp_geo_data.is_null() && cch_data >= 3 {
        std::ptr::copy_nonoverlapping(val.as_ptr(), lp_geo_data, 3);
        3
    } else {
        3
    }
}

#[no_mangle]
pub unsafe extern "win64" fn GetUserGeoID(_geo_class: u32) -> u32 {
    0xF4
}

#[no_mangle]
pub unsafe extern "win64" fn TerminateThread(
    _h_thread: *mut std::ffi::c_void,
    _dw_exit_code: u32,
) -> i32 {
    1
}

// ---------------------------------------------------------------------------
// Win64 SLIST_HEADER (Lock-Free Singly Linked List) Implementation
// ---------------------------------------------------------------------------

static SLIST_LOCKS: std::sync::LazyLock<[std::sync::Mutex<()>; 64]> =
    std::sync::LazyLock::new(|| std::array::from_fn(|_| std::sync::Mutex::new(())));

#[inline]
fn slist_lock(head: usize) -> std::sync::MutexGuard<'static, ()> {
    let idx = (head >> 4) % 64;
    SLIST_LOCKS[idx].lock().unwrap_or_else(|e| e.into_inner())
}

/// Initializes a 16-byte Win64 `SLIST_HEADER`.
/// Layout used: `[0]` low 16 bits = depth, `[1]` = pointer to first `SLIST_ENTRY`.
pub unsafe extern "win64" fn InitializeSListHead(list_head: *mut u64) {
    if list_head.is_null() {
        return;
    }
    let _g = slist_lock(list_head as usize);
    std::ptr::write_volatile(list_head, 0);
    std::ptr::write_volatile(list_head.add(1), 0);
}

/// Returns the number of entries in the singly linked list.
pub unsafe extern "win64" fn QueryDepthSList(list_head: *const u64) -> u16 {
    if list_head.is_null() {
        return 0;
    }
    let _g = slist_lock(list_head as usize);
    (std::ptr::read_volatile(list_head) & 0xFFFF) as u16
}

/// Inserts an item at the front of a singly linked list and returns the previous first item.
pub unsafe extern "win64" fn InterlockedPushEntrySList(
    list_head: *mut u64,
    list_entry: *mut usize,
) -> *mut usize {
    if list_head.is_null() || list_entry.is_null() {
        return std::ptr::null_mut();
    }
    let _g = slist_lock(list_head as usize);
    let old_first = std::ptr::read_volatile(list_head.add(1)) as *mut usize;
    std::ptr::write_volatile(list_entry, old_first as usize);
    std::ptr::write_volatile(list_head.add(1), list_entry as u64);
    let low = std::ptr::read_volatile(list_head);
    let depth = (low & 0xFFFF).wrapping_add(1) & 0xFFFF;
    std::ptr::write_volatile(list_head, (low & !0xFFFF) | depth);
    old_first
}

/// Removes an item from the front of a singly linked list and returns it.
pub unsafe extern "win64" fn InterlockedPopEntrySList(list_head: *mut u64) -> *mut usize {
    if list_head.is_null() {
        return std::ptr::null_mut();
    }
    let _g = slist_lock(list_head as usize);
    let first = std::ptr::read_volatile(list_head.add(1)) as *mut usize;
    if first.is_null() {
        return std::ptr::null_mut();
    }
    let next = std::ptr::read_volatile(first);
    std::ptr::write_volatile(list_head.add(1), next as u64);
    let low = std::ptr::read_volatile(list_head);
    let depth = low & 0xFFFF;
    if depth > 0 {
        std::ptr::write_volatile(list_head, (low & !0xFFFF) | (depth - 1));
    }
    first
}

/// Removes all items from a singly linked list and returns a pointer to the previous first item.
pub unsafe extern "win64" fn InterlockedFlushSList(list_head: *mut u64) -> *mut usize {
    if list_head.is_null() {
        return std::ptr::null_mut();
    }
    let _g = slist_lock(list_head as usize);
    let first = std::ptr::read_volatile(list_head.add(1)) as *mut usize;
    std::ptr::write_volatile(list_head.add(1), 0);
    let low = std::ptr::read_volatile(list_head);
    std::ptr::write_volatile(list_head, low & !0xFFFF);
    first
}

// ---------------------------------------------------------------------------
// GetLogicalProcessorInformationEx
// ---------------------------------------------------------------------------

/// Retrieves information about the relationships of logical processors and related hardware.
pub unsafe extern "win64" fn GetLogicalProcessorInformationEx(
    relationship_type: u32,
    buffer: *mut u8,
    returned_length: *mut u32,
) -> i32 {
    if returned_length.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0;
    }

    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 64);
    let all_mask: u64 = if num_cpus == 64 { u64::MAX } else { (1u64 << num_cpus) - 1 };

    let mut out = Vec::<u8>::new();

    let write_group_affinity = |buf: &mut Vec<u8>, mask: u64| {
        buf.extend_from_slice(&mask.to_le_bytes()); // Mask: KAFFINITY (u64)
        buf.extend_from_slice(&0u16.to_le_bytes()); // Group: u16
        buf.extend_from_slice(&[0u8; 6]); // Reserved: [u16; 3]
    };

    // RelationProcessorCore = 0
    if relationship_type == 0 || relationship_type == 0xFFFF {
        for cpu in 0..num_cpus {
            out.extend_from_slice(&0u32.to_le_bytes()); // Relationship = 0
            out.extend_from_slice(&48u32.to_le_bytes()); // Size = 48
            out.push(0); // Flags
            out.push(0); // EfficiencyClass
            out.extend_from_slice(&[0u8; 20]); // Reserved
            out.extend_from_slice(&1u16.to_le_bytes()); // GroupCount = 1
            write_group_affinity(&mut out, 1u64 << cpu);
        }
    }

    // RelationNumaNode = 1
    if relationship_type == 1 || relationship_type == 0xFFFF {
        out.extend_from_slice(&1u32.to_le_bytes()); // Relationship = 1
        out.extend_from_slice(&48u32.to_le_bytes()); // Size = 48
        out.extend_from_slice(&0u32.to_le_bytes()); // NodeNumber = 0
        out.extend_from_slice(&[0u8; 18]); // Reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // GroupCount = 1
        write_group_affinity(&mut out, all_mask);
    }

    // RelationCache = 2
    if relationship_type == 2 || relationship_type == 0xFFFF {
        // Report L1/L2/L3 cache entries
        for (level, size_bytes, cache_type) in [(1u8, 32 * 1024u32, 1u32), (2u8, 256 * 1024u32, 0u32), (3u8, 8 * 1024 * 1024u32, 0u32)] {
            out.extend_from_slice(&2u32.to_le_bytes()); // Relationship = 2
            out.extend_from_slice(&56u32.to_le_bytes()); // Size = 56
            out.push(level); // Level
            out.push(8); // Associativity
            out.extend_from_slice(&64u16.to_le_bytes()); // LineSize = 64
            out.extend_from_slice(&size_bytes.to_le_bytes()); // CacheSize
            out.extend_from_slice(&cache_type.to_le_bytes()); // Type
            out.extend_from_slice(&[0u8; 18]); // Reserved
            out.extend_from_slice(&1u16.to_le_bytes()); // GroupCount = 1
            write_group_affinity(&mut out, all_mask);
        }
    }

    // RelationProcessorPackage = 3
    if relationship_type == 3 || relationship_type == 0xFFFF {
        out.extend_from_slice(&3u32.to_le_bytes()); // Relationship = 3
        out.extend_from_slice(&48u32.to_le_bytes()); // Size = 48
        out.push(0); // Flags
        out.push(0); // EfficiencyClass
        out.extend_from_slice(&[0u8; 20]); // Reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // GroupCount = 1
        write_group_affinity(&mut out, all_mask);
    }

    // RelationGroup = 4
    if relationship_type == 4 || relationship_type == 0xFFFF {
        out.extend_from_slice(&4u32.to_le_bytes()); // Relationship = 4
        out.extend_from_slice(&80u32.to_le_bytes()); // Size = 80
        out.extend_from_slice(&1u16.to_le_bytes()); // MaximumGroupCount = 1
        out.extend_from_slice(&1u16.to_le_bytes()); // ActiveGroupCount = 1
        out.extend_from_slice(&[0u8; 20]); // Reserved
        // PROCESSOR_GROUP_INFO (48 bytes)
        out.push(num_cpus as u8); // MaximumProcessorCount
        out.push(num_cpus as u8); // ActiveProcessorCount
        out.extend_from_slice(&[0u8; 38]); // Reserved
        out.extend_from_slice(&all_mask.to_le_bytes()); // ActiveProcessorMask
    }

    let required = out.len() as u32;
    let provided = *returned_length;
    *returned_length = required;

    if buffer.is_null() || provided < required {
        SetLastError(122); // ERROR_INSUFFICIENT_BUFFER
        return 0;
    }

    std::ptr::copy_nonoverlapping(out.as_ptr(), buffer, out.len());
    SetLastError(0);
    1
}

// ---------------------------------------------------------------------------
// Processor Features, Code Pages, String Comparison, Disk Space
// ---------------------------------------------------------------------------

/// Reports whether a CPU feature is supported on the current x86_64 host.
pub unsafe extern "win64" fn IsProcessorFeaturePresent(processor_feature: u32) -> i32 {
    match processor_feature {
        2  // PF_COMPARE_EXCHANGE_DOUBLE
        | 6  // PF_XMMI_INSTRUCTIONS_AVAILABLE (SSE)
        | 8  // PF_RDTSC_INSTRUCTION_AVAILABLE
        | 10 // PF_XMMI64_INSTRUCTIONS_AVAILABLE (SSE2)
        | 12 // PF_NX_ENABLED
        | 13 // PF_SSE3_INSTRUCTIONS_AVAILABLE
        | 14 // PF_COMPARE_EXCHANGE128 (cmpxchg16b)
        | 17 // PF_XSAVE_ENABLED
        | 23 // PF_FASTFAIL_AVAILABLE
        | 36 // PF_SSE4_1_INSTRUCTIONS_AVAILABLE
        | 37 // PF_SSE4_2_INSTRUCTIONS_AVAILABLE
        | 39 // PF_AVX_INSTRUCTIONS_AVAILABLE
        | 40 // PF_AVX2_INSTRUCTIONS_AVAILABLE
        => 1,
        _ => 0,
    }
}

pub unsafe extern "win64" fn GetACP() -> u32 {
    65001 // CP_UTF8
}

pub unsafe extern "win64" fn GetOEMCP() -> u32 {
    437
}

pub unsafe extern "win64" fn IsValidCodePage(code_page: u32) -> i32 {
    if code_page != 0 { 1 } else { 0 }
}

pub unsafe extern "win64" fn GetDiskFreeSpaceExW(
    _lp_directory_name: *const u16,
    lp_free_bytes_available_to_caller: *mut u64,
    lp_total_number_of_bytes: *mut u64,
    lp_total_number_of_free_bytes: *mut u64,
) -> i32 {
    let mut stat: libc::statvfs = std::mem::zeroed();
    let (avail, total, free) = if libc::statvfs(b"/\0".as_ptr() as *const i8, &mut stat) == 0 {
        let frag = stat.f_frsize as u64;
        (
            stat.f_bavail * frag,
            stat.f_blocks * frag,
            stat.f_bfree * frag,
        )
    } else {
        (100 * 1024 * 1024 * 1024, 500 * 1024 * 1024 * 1024, 100 * 1024 * 1024 * 1024)
    };
    if !lp_free_bytes_available_to_caller.is_null() {
        *lp_free_bytes_available_to_caller = avail;
    }
    if !lp_total_number_of_bytes.is_null() {
        *lp_total_number_of_bytes = total;
    }
    if !lp_total_number_of_free_bytes.is_null() {
        *lp_total_number_of_free_bytes = free;
    }
    1
}

unsafe fn read_utf16_slice(ptr: *const u16, cch: i32) -> Vec<u16> {
    if ptr.is_null() {
        return Vec::new();
    }
    if cch < 0 {
        let mut len = 0usize;
        while *ptr.add(len) != 0 && len < 65536 {
            len += 1;
        }
        std::slice::from_raw_parts(ptr, len).to_vec()
    } else {
        std::slice::from_raw_parts(ptr, cch as usize).to_vec()
    }
}

pub unsafe extern "win64" fn CompareStringW(
    _locale: u32,
    dw_cmp_flags: u32,
    lp_string1: *const u16,
    cch_count1: i32,
    lp_string2: *const u16,
    cch_count2: i32,
) -> i32 {
    if lp_string1.is_null() || lp_string2.is_null() {
        SetLastError(87);
        return 0;
    }
    let s1 = String::from_utf16_lossy(&read_utf16_slice(lp_string1, cch_count1));
    let s2 = String::from_utf16_lossy(&read_utf16_slice(lp_string2, cch_count2));
    let ord = if (dw_cmp_flags & 1) != 0 {
        s1.to_lowercase().cmp(&s2.to_lowercase())
    } else {
        s1.cmp(&s2)
    };
    match ord {
        std::cmp::Ordering::Less => 1,    // CSTR_LESS_THAN
        std::cmp::Ordering::Equal => 2,   // CSTR_EQUAL
        std::cmp::Ordering::Greater => 3, // CSTR_GREATER_THAN
    }
}

pub unsafe extern "win64" fn CompareStringEx(
    _lp_locale_name: *const u16,
    dw_cmp_flags: u32,
    lp_string1: *const u16,
    cch_count1: i32,
    lp_string2: *const u16,
    cch_count2: i32,
    _lp_version_information: usize,
    _lp_reserved: usize,
    _l_param: isize,
) -> i32 {
    CompareStringW(0, dw_cmp_flags, lp_string1, cch_count1, lp_string2, cch_count2)
}

pub unsafe extern "win64" fn LCMapStringW(
    _locale: u32,
    dw_map_flags: u32,
    lp_src_str: *const u16,
    cch_src: i32,
    lp_dest_str: *mut u16,
    cch_dest: i32,
) -> i32 {
    if lp_src_str.is_null() {
        SetLastError(87);
        return 0;
    }
    let src_u16 = read_utf16_slice(lp_src_str, cch_src);
    let src_str = String::from_utf16_lossy(&src_u16);
    let mapped = if (dw_map_flags & 0x00000100) != 0 {
        src_str.to_lowercase()
    } else if (dw_map_flags & 0x00000200) != 0 {
        src_str.to_uppercase()
    } else {
        src_str
    };
    let mut out_u16: Vec<u16> = mapped.encode_utf16().collect();
    if cch_src < 0 {
        out_u16.push(0);
    }
    let req = out_u16.len() as i32;
    if cch_dest == 0 || lp_dest_str.is_null() {
        return req;
    }
    if cch_dest < req {
        SetLastError(122);
        return 0;
    }
    std::ptr::copy_nonoverlapping(out_u16.as_ptr(), lp_dest_str, out_u16.len());
    req
}

pub unsafe extern "win64" fn LCMapStringEx(
    _lp_locale_name: *const u16,
    dw_map_flags: u32,
    lp_src_str: *const u16,
    cch_src: i32,
    lp_dest_str: *mut u16,
    cch_dest: i32,
    _lp_version_information: usize,
    _lp_reserved: usize,
    _sort_handle: isize,
) -> i32 {
    LCMapStringW(0, dw_map_flags, lp_src_str, cch_src, lp_dest_str, cch_dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_heap_allocation() {
        let heap = unsafe { GetProcessHeap() };
        let ptr = unsafe { HeapAlloc(heap, HEAP_ZERO_MEMORY, 256) };
        assert!(ptr > 0);

        unsafe {
            let slice = std::slice::from_raw_parts(ptr as *const u8, 256);
            assert!(slice.iter().all(|&b| b == 0));
        }

        assert_eq!(unsafe { HeapFree(heap, 0, ptr) }, 1);
    }

    #[test]
    fn executable_set_console_output_cp_preserves_win64_nonvolatile_gprs() {
        let fn_ptr = SetConsoleOutputCP as unsafe extern "win64" fn(u32) -> i32;

        let res: i32;
        let mut saved_regs = [0u64; 7];
        let ptr_val = saved_regs.as_mut_ptr();

        unsafe {
            std::arch::asm!(
                "mov rax, {fn_ptr}",
                "mov r10, {ptr}",

                "push rbx",
                "push rbp",
                "push rsi",
                "push rdi",
                "push r12",
                "push r13",
                "push r14",
                "push r15",

                "mov rbp, 0x5555666677778888",
                "mov rsi, 0x9999AAAABBBBCCCC",
                "mov rdi, 0xDDDDEEEEFFFF0000",
                "mov r12, 0x123456789ABCDEF0",
                "mov r13, 0x0FEDCBA987654321",
                "mov r14, 0xA1B2C3D4E5F60718",
                "mov r15, 0x9F8E7D6C5B4A3928",

                "mov ecx, 65001",
                "sub rsp, 0x20",
                "call rax",
                "add rsp, 0x20",

                "mov {res:e}, eax",

                "mov [r10 + 0x00], rbp",
                "mov [r10 + 0x08], rsi",
                "mov [r10 + 0x10], rdi",
                "mov [r10 + 0x18], r12",
                "mov [r10 + 0x20], r13",
                "mov [r10 + 0x28], r14",
                "mov [r10 + 0x30], r15",

                "pop r15",
                "pop r14",
                "pop r13",
                "pop r12",
                "pop rdi",
                "pop rsi",
                "pop rbp",
                "pop rbx",
                fn_ptr = in(reg) fn_ptr,
                ptr = in(reg) ptr_val,
                res = out(reg) res,
                out("rax") _,
                out("r10") _,
            );
        }

        assert_eq!(res, 1, "SetConsoleOutputCP must return Win32 BOOL TRUE (1)");
        assert_eq!(saved_regs[0], 0x5555666677778888, "RBP clobbered!");
        assert_eq!(saved_regs[1], 0x9999AAAABBBBCCCC, "RSI clobbered!");
        assert_eq!(saved_regs[2], 0xDDDDEEEEFFFF0000, "RDI clobbered!");
        assert_eq!(saved_regs[3], 0x123456789ABCDEF0, "R12 clobbered!");
        assert_eq!(saved_regs[4], 0x0FEDCBA987654321, "R13 clobbered!");
        assert_eq!(saved_regs[5], 0xA1B2C3D4E5F60718, "R14 clobbered!");
        assert_eq!(saved_regs[6], 0x9F8E7D6C5B4A3928, "R15 clobbered!");
    }

    #[test]
    fn test_wide_char_to_multi_byte_contract_suite() {
        unsafe {
            // A. ASCII + cchWideChar = -1
            let wstr_ascii: Vec<u16> = "abc\0".encode_utf16().collect();
            let req_ascii = WideCharToMultiByte(
                65001,
                0,
                wstr_ascii.as_ptr(),
                -1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(req_ascii, 4, "ASCII 'abc\\0' sizing must equal 4 bytes");

            // B. Non-ASCII UTF-8 (é is U+00E9 -> 2 UTF-8 bytes)
            let wstr_non_ascii: Vec<u16> = "A\u{00E9}\0".encode_utf16().collect();
            let req_non_ascii = WideCharToMultiByte(
                65001,
                0,
                wstr_non_ascii.as_ptr(),
                -1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(
                req_non_ascii, 4,
                "Non-ASCII 'A\\u{{00E9}}\\0' (1+2+1=4 bytes) sizing"
            );
            assert_ne!(
                wstr_non_ascii.len() as i32,
                req_non_ascii,
                "UTF-16 code units (3) != UTF-8 byte count (4)"
            );

            // C. Exact size conversion (cb_multi_byte == required)
            let mut dst_exact = vec![0u8; req_ascii as usize];
            let conv_exact = WideCharToMultiByte(
                65001,
                0,
                wstr_ascii.as_ptr(),
                -1,
                dst_exact.as_mut_ptr(),
                req_ascii,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(conv_exact, req_ascii);
            assert_eq!(&dst_exact[..], b"abc\0");

            // D. Insufficient buffer by one
            SetLastError(0);
            let mut dst_short = vec![0u8; (req_ascii - 1) as usize];
            let conv_short = WideCharToMultiByte(
                65001,
                0,
                wstr_ascii.as_ptr(),
                -1,
                dst_short.as_mut_ptr(),
                req_ascii - 1,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(conv_short, 0, "Insufficient buffer must return 0");
            assert_eq!(
                GetLastError(),
                122,
                "LastError must set ERROR_INSUFFICIENT_BUFFER (122)"
            );

            // E. Explicit cchWideChar excluding NUL
            let mut dst_no_nul = vec![0u8; 3];
            let conv_no_nul = WideCharToMultiByte(
                65001,
                0,
                wstr_ascii.as_ptr(),
                3,
                dst_no_nul.as_mut_ptr(),
                3,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(conv_no_nul, 3, "Explicit cch=3 excluding NUL must return 3");
            assert_eq!(&dst_no_nul[..], b"abc");

            // F. Empty NUL-terminated string L"\0" with cch = -1
            let wstr_empty: Vec<u16> = "\0".encode_utf16().collect();
            let req_empty = WideCharToMultiByte(
                65001,
                0,
                wstr_empty.as_ptr(),
                -1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(
                req_empty, 1,
                "Empty NUL-terminated string L'\\0' sizing must equal 1"
            );
        }
    }

    static WORKER_EXECUTED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    unsafe extern "win64" fn test_worker_func(_param: u64) -> u32 {
        WORKER_EXECUTED.store(true, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_guest_worker_start_executes() {
        WORKER_EXECUTED.store(false, std::sync::atomic::Ordering::SeqCst);
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_worker_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        assert!(WORKER_EXECUTED.load(std::sync::atomic::Ordering::SeqCst));
    }

    static STACK_ALIGN_RSP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    unsafe extern "win64" fn test_stack_align_func(_param: u64) -> u32 {
        let rsp: u64;
        std::arch::asm!("mov {}, rsp", out(reg) rsp);
        STACK_ALIGN_RSP.store(rsp, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_worker_stack_alignment() {
        STACK_ALIGN_RSP.store(0, std::sync::atomic::Ordering::SeqCst);
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_stack_align_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        let rsp = STACK_ALIGN_RSP.load(std::sync::atomic::Ordering::SeqCst);
        assert_ne!(rsp, 0);
        assert_eq!(rsp % 8, 0, "Stack pointer must be 8-byte aligned");
    }

    static WORKER_TEB_1: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    unsafe extern "win64" fn test_teb_func_1(_param: u64) -> u32 {
        let teb = get_teb_ptr() as u64;
        WORKER_TEB_1.store(teb, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_worker_has_distinct_teb() {
        let main_teb = unsafe { get_teb_ptr() as u64 };
        WORKER_TEB_1.store(0, std::sync::atomic::Ordering::SeqCst);
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_teb_func_1 as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        let worker_teb = WORKER_TEB_1.load(std::sync::atomic::Ordering::SeqCst);
        assert_ne!(worker_teb, 0, "Worker TEB must not be NULL");
        assert_ne!(
            worker_teb, main_teb,
            "Worker TEB must be distinct from main thread TEB"
        );
    }

    static WORKER_TEB_2: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static WORKER_GS_2: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    unsafe extern "win64" fn test_teb_func_2(_param: u64) -> u32 {
        let teb = get_teb_ptr() as u64;
        WORKER_TEB_2.store(teb, std::sync::atomic::Ordering::SeqCst);
        let mut gs_base: u64 = 0;
        libc::syscall(libc::SYS_arch_prctl, 0x1004, &raw mut gs_base);
        WORKER_GS_2.store(gs_base, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_worker_gs_points_to_teb() {
        WORKER_TEB_2.store(0, std::sync::atomic::Ordering::SeqCst);
        WORKER_GS_2.store(0, std::sync::atomic::Ordering::SeqCst);
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_teb_func_2 as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        let worker_teb = WORKER_TEB_2.load(std::sync::atomic::Ordering::SeqCst);
        let worker_gs = WORKER_GS_2.load(std::sync::atomic::Ordering::SeqCst);
        assert_ne!(worker_teb, 0);
        assert_eq!(
            worker_gs, worker_teb,
            "GS base must match worker TEB pointer"
        );
    }

    static WORKER_LAST_ERR0: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    static WORKER_LAST_ERR1: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    unsafe extern "win64" fn test_last_error_func(_param: u64) -> u32 {
        let err0 = GetLastError();
        WORKER_LAST_ERR0.store(err0, std::sync::atomic::Ordering::SeqCst);
        SetLastError(222);
        let err1 = GetLastError();
        WORKER_LAST_ERR1.store(err1, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_last_error_thread_isolation() {
        unsafe {
            SetLastError(111);
        }
        assert_eq!(unsafe { GetLastError() }, 111);

        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_last_error_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }

        assert_eq!(
            WORKER_LAST_ERR0.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(
            WORKER_LAST_ERR1.load(std::sync::atomic::Ordering::SeqCst),
            222
        );
        assert_eq!(
            unsafe { GetLastError() },
            111,
            "Main thread LastError must remain 111 after worker execution"
        );
    }

    static WORKER_TLS_VAL0: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static WORKER_TLS_VAL1: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static TEST_TLS_SLOT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    unsafe extern "win64" fn test_tls_func(_param: u64) -> u32 {
        let slot = TEST_TLS_SLOT.load(std::sync::atomic::Ordering::SeqCst);
        let v0 = TlsGetValue(slot);
        WORKER_TLS_VAL0.store(v0, std::sync::atomic::Ordering::SeqCst);
        TlsSetValue(slot, 0xBBBB);
        let v1 = TlsGetValue(slot);
        WORKER_TLS_VAL1.store(v1, std::sync::atomic::Ordering::SeqCst);
        0
    }

    #[test]
    fn test_tls_thread_isolation() {
        let slot = unsafe { TlsAlloc() };
        TEST_TLS_SLOT.store(slot, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            TlsSetValue(slot, 0xAAAA);
        }
        assert_eq!(unsafe { TlsGetValue(slot) }, 0xAAAA);

        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_tls_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }

        assert_eq!(WORKER_TLS_VAL0.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(
            WORKER_TLS_VAL1.load(std::sync::atomic::Ordering::SeqCst),
            0xBBBB
        );
        assert_eq!(
            unsafe { TlsGetValue(slot) },
            0xAAAA,
            "Main thread TLS value must remain 0xAAAA"
        );
        unsafe {
            TlsFree(slot);
        }
    }

    unsafe extern "win64" fn test_clean_return_func(_param: u64) -> u32 {
        0xDEADBEEF
    }

    #[test]
    fn test_guest_worker_clean_return() {
        let handle = unsafe {
            CreateThread(
                std::ptr::null_mut(),
                65536,
                test_clean_return_func as *const () as u64,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, 0);
        unsafe {
            WaitForSingleObject(handle, 5000);
        }
        let mut exit_code: u32 = 0;
        let res = unsafe { GetExitCodeThread(handle, &raw mut exit_code) };
        assert_eq!(res, 1);
        assert_eq!(exit_code, 0xDEADBEEF);
        unsafe {
            CloseHandle(handle);
        }
    }

    #[test]
    fn test_get_logical_drives_mask() {
        let mask = unsafe { GetLogicalDrives() };
        assert_ne!(mask, 0, "Logical drive mask must be non-zero");
        assert_eq!(
            mask & (1 << 2),
            1 << 2,
            "Bit 2 (Drive C:) must be set in logical drive mask"
        );
        assert_eq!(
            mask,
            1 << 2,
            "Alpha virtual drive policy must only set Bit 2 (Drive C:)"
        );
        let registry = get_drive_registry();
        assert!(registry.is_drive_available('C'));
        assert!(registry.is_drive_available('c'));
        assert!(!registry.is_drive_available('A'));
        assert!(!registry.is_drive_available('Z'));
    }

    #[test]
    fn test_get_logical_drives_win64_abi() {
        let func_ptr: unsafe extern "win64" fn() -> u32 = GetLogicalDrives;
        let mask = unsafe { func_ptr() };
        assert_eq!(mask, 0x00000004);
    }

    static CWD_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_get_current_directory_w_contract_suite() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("C:\\foo");
        let expected_len = 6u32; // "C:\foo" has 6 WCHARs

        // Test A: Sufficient buffer
        let mut buf = [0u16; 64];
        let ret = unsafe { GetCurrentDirectoryW(64, buf.as_mut_ptr()) };
        assert_eq!(ret, expected_len, "Must return WCHAR count excluding null");
        assert_eq!(
            String::from_utf16_lossy(&buf[..expected_len as usize]),
            "C:\\foo"
        );
        assert_eq!(buf[expected_len as usize], 0, "Must be null-terminated");

        // Test B: Exact capacity (len + 1 = 7)
        let mut exact_buf = [0u16; 7];
        let ret_exact = unsafe { GetCurrentDirectoryW(7, exact_buf.as_mut_ptr()) };
        assert_eq!(ret_exact, expected_len);
        assert_eq!(
            String::from_utf16_lossy(&exact_buf[..expected_len as usize]),
            "C:\\foo"
        );
        assert_eq!(exact_buf[expected_len as usize], 0);

        // Test C: One WCHAR too small (nBufferLength = len = 6)
        let mut small_buf = [0xAAAAu16; 6];
        let ret_small = unsafe { GetCurrentDirectoryW(6, small_buf.as_mut_ptr()) };
        assert_eq!(
            ret_small,
            expected_len + 1,
            "Must return required capacity (len + 1) when too small"
        );
        assert_eq!(
            small_buf, [0xAAAAu16; 6],
            "Small buffer must not be overwritten"
        );

        // Test D: nBufferLength = 0 / NULL buffer
        let ret_zero = unsafe { GetCurrentDirectoryW(0, std::ptr::null_mut()) };
        assert_eq!(ret_zero, expected_len + 1);
    }

    #[test]
    fn test_get_current_directory_w_unicode() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        let unicode_path = "C:\\Тест\\Проект";
        set_process_current_directory(unicode_path);
        let expected_utf16: Vec<u16> = unicode_path.encode_utf16().collect();
        let expected_len = expected_utf16.len() as u32;

        let mut buf = [0u16; 128];
        let ret = unsafe { GetCurrentDirectoryW(128, buf.as_mut_ptr()) };
        assert_eq!(ret, expected_len);
        assert_eq!(&buf[..expected_len as usize], expected_utf16.as_slice());
        assert_eq!(buf[expected_len as usize], 0);
        assert_eq!(
            String::from_utf16_lossy(&buf[..expected_len as usize]),
            unicode_path
        );
    }

    #[test]
    fn test_get_current_directory_w_buffer_bounds() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("C:\\Games\\Neighbor");
        let utf16: Vec<u16> = "C:\\Games\\Neighbor".encode_utf16().collect();
        let len = utf16.len();

        #[repr(C)]
        struct Guarded {
            canary_before: [u16; 8],
            buffer: [u16; 32],
            canary_after: [u16; 8],
        }

        let mut g = Guarded {
            canary_before: [0xCAFE; 8],
            buffer: [0; 32],
            canary_after: [0xBEEF; 8],
        };

        let ret = unsafe { GetCurrentDirectoryW(32, g.buffer.as_mut_ptr()) };
        assert_eq!(ret, len as u32);
        assert_eq!(g.canary_before, [0xCAFE; 8]);
        assert_eq!(g.canary_after, [0xBEEF; 8]);
        assert_eq!(g.buffer[len], 0);
    }

    #[test]
    fn test_set_current_directory_w_dot() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("\\tmp");
        let dot_utf16: Vec<u16> = ".\0".encode_utf16().collect();
        let res = unsafe { SetCurrentDirectoryW(dot_utf16.as_ptr()) };
        assert_eq!(res, 1, "SetCurrentDirectoryW('.') must succeed");
        assert_eq!(get_process_current_directory(), "\\tmp");
    }

    #[test]
    fn test_set_current_directory_w_nonexistent_does_not_change_state() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("\\tmp");
        let bad_utf16: Vec<u16> = "C:\\nonexistent_dir_987654321\0".encode_utf16().collect();
        let res = unsafe { SetCurrentDirectoryW(bad_utf16.as_ptr()) };
        assert_eq!(
            res, 0,
            "SetCurrentDirectoryW must fail on non-existent path"
        );
        assert_eq!(
            get_process_current_directory(),
            "\\tmp",
            "CWD must not change on failure"
        );
        let err = unsafe { GetLastError() };
        assert_ne!(err, 0, "LastError must be set on failure");
    }

    #[test]
    fn test_set_then_get_current_directory_roundtrip() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        let target_utf16: Vec<u16> = "C:\\tmp\0".encode_utf16().collect();
        let res = unsafe { SetCurrentDirectoryW(target_utf16.as_ptr()) };
        assert_eq!(res, 1);

        let mut buf = [0u16; 64];
        let len = unsafe { GetCurrentDirectoryW(64, buf.as_mut_ptr()) };
        assert!(len > 0);
        let read_back = String::from_utf16_lossy(&buf[..len as usize]);
        assert_eq!(read_back, "C:\\tmp");
    }

    #[test]
    fn test_process_cwd_shared_between_threads() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();
        set_process_current_directory("\\tmp");
        let handle = std::thread::spawn(|| {
            assert_eq!(get_process_current_directory(), "\\tmp");
            set_process_current_directory("\\proc");
        });
        handle.join().unwrap();
        assert_eq!(get_process_current_directory(), "\\proc");
    }

    #[test]
    fn test_console_screen_buffer_info_layout_and_bounds() {
        assert_eq!(
            std::mem::size_of::<ConsoleScreenBufferInfo>(),
            22,
            "CONSOLE_SCREEN_BUFFER_INFO must be 22 bytes in Win32/Win64 ABI"
        );
        assert_eq!(
            std::mem::align_of::<ConsoleScreenBufferInfo>(),
            2,
            "CONSOLE_SCREEN_BUFFER_INFO alignment must be 2"
        );

        let dummy: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let base = &dummy as *const _ as usize;
        let off_dw_size = (&dummy.dw_size as *const _ as usize) - base;
        let off_cursor = (&dummy.dw_cursor_position as *const _ as usize) - base;
        let off_attr = (&dummy.w_attributes as *const _ as usize) - base;
        let off_window = (&dummy.sr_window as *const _ as usize) - base;
        let off_max = (&dummy.dw_maximum_window_size as *const _ as usize) - base;

        assert_eq!(off_dw_size, 0, "dwSize offset must be 0");
        assert_eq!(off_cursor, 4, "dwCursorPosition offset must be 4");
        assert_eq!(off_attr, 8, "wAttributes offset must be 8");
        assert_eq!(off_window, 10, "srWindow offset must be 10");
        assert_eq!(off_max, 18, "dwMaximumWindowSize offset must be 18");

        #[repr(C)]
        struct GuardedInfo {
            canary_before: [u32; 4],
            info: ConsoleScreenBufferInfo,
            canary_after: [u32; 4],
        }

        let mut guarded = GuardedInfo {
            canary_before: [0xDEADBEEF; 4],
            info: unsafe { std::mem::zeroed() },
            canary_after: [0xCAFEBABE; 4],
        };

        let res = unsafe { GetConsoleScreenBufferInfo(0x8, &mut guarded.info) };
        assert_eq!(res, 1);
        assert_eq!(guarded.canary_before, [0xDEADBEEF; 4]);
        assert_eq!(guarded.canary_after, [0xCAFEBABE; 4]);
    }

    #[test]
    fn test_get_console_screen_buffer_info_valid_console_handle() {
        let mut info: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let res = unsafe { GetConsoleScreenBufferInfo(0x8, &mut info) };
        assert_eq!(res, 1);
        assert_eq!(info.dw_size, Coord { x: 80, y: 25 });
        assert_eq!(info.dw_cursor_position, Coord { x: 0, y: 0 });
        assert_eq!(info.w_attributes, 0x0007);
        assert_eq!(
            info.sr_window,
            SmallRect {
                left: 0,
                top: 0,
                right: 79,
                bottom: 24,
            }
        );
        assert_eq!(info.dw_maximum_window_size, Coord { x: 80, y: 25 });
    }

    #[test]
    fn test_get_console_screen_buffer_info_invalid_handle() {
        let mut info: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let res = unsafe { GetConsoleScreenBufferInfo(0x9999, &mut info) };
        assert_eq!(res, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 6, "Must set ERROR_INVALID_HANDLE (6)");
    }

    #[test]
    fn test_get_console_screen_buffer_info_null_output() {
        let res = unsafe { GetConsoleScreenBufferInfo(0x8, std::ptr::null_mut()) };
        assert_eq!(res, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 87, "Must set ERROR_INVALID_PARAMETER (87)");
    }

    #[test]
    fn executable_get_console_screen_buffer_info_preserves_win64_nonvolatile_gprs() {
        let func_ptr: unsafe extern "win64" fn(u64, *mut ConsoleScreenBufferInfo) -> i32 =
            GetConsoleScreenBufferInfo;
        let mut info: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let res = unsafe { func_ptr(0x8, &mut info) };
        assert_eq!(res, 1);
    }

    #[test]
    fn test_set_console_text_attribute_state_roundtrip() {
        set_console_text_attributes(0x0007);
        let mut info: ConsoleScreenBufferInfo = unsafe { std::mem::zeroed() };
        let res1 = unsafe { GetConsoleScreenBufferInfo(0x8, &mut info) };
        assert_eq!(res1, 1);
        assert_eq!(info.w_attributes, 0x0007);

        let res2 = unsafe { SetConsoleTextAttribute(0x8, 0x000C) };
        assert_eq!(res2, 1);

        let res3 = unsafe { GetConsoleScreenBufferInfo(0x8, &mut info) };
        assert_eq!(res3, 1);
        assert_eq!(info.w_attributes, 0x000C);

        let res4 = unsafe { SetConsoleTextAttribute(0x8, 0x0007) };
        assert_eq!(res4, 1);

        let res5 = unsafe { GetConsoleScreenBufferInfo(0x8, &mut info) };
        assert_eq!(res5, 1);
        assert_eq!(info.w_attributes, 0x0007);
    }

    #[test]
    fn test_set_console_text_attribute_invalid_handle() {
        let res = unsafe { SetConsoleTextAttribute(0x9999, 0x000C) };
        assert_eq!(res, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 6, "Must set ERROR_INVALID_HANDLE (6)");
    }

    #[test]
    fn test_set_console_text_attribute_word_width() {
        let res = unsafe { SetConsoleTextAttribute(0x8, 0x002F) };
        assert_eq!(res, 1);
        assert_eq!(get_console_text_attributes(), 0x002F);

        // Reset to default
        set_console_text_attributes(0x0007);
    }

    #[test]
    fn executable_set_console_text_attribute_preserves_win64_nonvolatile_gprs() {
        let func_ptr: unsafe extern "win64" fn(u64, u16) -> i32 = SetConsoleTextAttribute;
        let res = unsafe { func_ptr(0x8, 0x0007) };
        assert_eq!(res, 1);
    }

    #[test]
    fn test_initial_guest_cwd_policy() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();

        // 1. Explicit configured CWD
        initialize_process_filesystem("/opt/games/test/game.exe", Some("/custom/cwd"));
        assert_eq!(get_process_current_directory(), "\\custom\\cwd");

        // 2. Default to parent directory of executable
        initialize_process_filesystem("/opt/games/test/game.exe", None);
        assert_eq!(get_process_current_directory(), "\\opt\\games\\test");

        // 3. Project data relative path resolution
        let rel_pck = win32_to_host_path("game.pck");
        assert_eq!(rel_pck, "/opt/games/test/game.pck");

        // 4. Absolute path preserves target inside virtual drive_c
        let abs_path = win32_to_host_path("C:\\opt\\games\\test\\game.exe");
        assert!(abs_path.contains("drive_c"));
        assert!(abs_path.ends_with("/opt/games/test/game.exe"));
    }

    #[test]
    fn test_system_info_win64_layout() {
        assert_eq!(
            std::mem::size_of::<SystemInfo>(),
            48,
            "Win64 SYSTEM_INFO size must be exactly 48 bytes"
        );
        assert_eq!(
            std::mem::align_of::<SystemInfo>(),
            8,
            "Win64 SYSTEM_INFO alignment must be 8 bytes"
        );

        let dummy = SystemInfo {
            w_processor_architecture: 0,
            w_reserved: 0,
            dw_page_size: 0,
            lp_minimum_application_address: 0,
            lp_maximum_application_address: 0,
            dw_active_processor_mask: 0,
            dw_number_of_processors: 0,
            dw_processor_type: 0,
            dw_allocation_granularity: 0,
            w_processor_level: 0,
            w_processor_revision: 0,
        };
        let base = &raw const dummy as usize;
        assert_eq!(
            (&raw const dummy.w_processor_architecture as usize) - base,
            0
        );
        assert_eq!((&raw const dummy.w_reserved as usize) - base, 2);
        assert_eq!((&raw const dummy.dw_page_size as usize) - base, 4);
        assert_eq!(
            (&raw const dummy.lp_minimum_application_address as usize) - base,
            8
        );
        assert_eq!(
            (&raw const dummy.lp_maximum_application_address as usize) - base,
            16
        );
        assert_eq!(
            (&raw const dummy.dw_active_processor_mask as usize) - base,
            24
        );
        assert_eq!(
            (&raw const dummy.dw_number_of_processors as usize) - base,
            32
        );
        assert_eq!((&raw const dummy.dw_processor_type as usize) - base, 36);
        assert_eq!(
            (&raw const dummy.dw_allocation_granularity as usize) - base,
            40
        );
        assert_eq!((&raw const dummy.w_processor_level as usize) - base, 44);
        assert_eq!((&raw const dummy.w_processor_revision as usize) - base, 46);
    }

    #[test]
    fn test_get_native_system_info_buffer_bounds() {
        #[repr(C)]
        struct GuardedSystemInfo {
            guard_before: [u8; 16],
            info: SystemInfo,
            guard_after: [u8; 16],
        }

        let mut guarded = GuardedSystemInfo {
            guard_before: [0xAA; 16],
            info: unsafe { std::mem::zeroed() },
            guard_after: [0xBB; 16],
        };

        unsafe {
            GetNativeSystemInfo(&mut guarded.info);
        }

        assert_eq!(
            guarded.guard_before, [0xAA; 16],
            "guard_before was overwritten!"
        );
        assert_eq!(
            guarded.guard_after, [0xBB; 16],
            "guard_after was overwritten!"
        );
        assert_eq!(
            guarded.info.w_processor_architecture,
            PROCESSOR_ARCHITECTURE_AMD64
        );
        assert_eq!(guarded.info.dw_page_size, 4096);
        assert_eq!(guarded.info.dw_allocation_granularity, 65536);
        assert!(guarded.info.dw_number_of_processors >= 1);
        assert_ne!(guarded.info.dw_active_processor_mask, 0);
    }

    #[test]
    fn test_get_native_system_info_fields() {
        let mut info: SystemInfo = unsafe { std::mem::zeroed() };
        unsafe {
            GetNativeSystemInfo(&mut info);
        }

        assert_eq!(info.w_processor_architecture, PROCESSOR_ARCHITECTURE_AMD64);
        assert_eq!(info.w_reserved, 0);
        assert_eq!(info.dw_page_size, 4096);
        assert_eq!(info.lp_minimum_application_address, 0x10000);
        assert_eq!(info.lp_maximum_application_address, 0x00007FFFFFFEFFFF);
        assert_eq!(info.dw_processor_type, PROCESSOR_AMD_X8664);
        assert_eq!(info.dw_allocation_granularity, 65536);
        assert_eq!(info.w_processor_level, 6);
        assert_eq!(info.w_processor_revision, 0);
        assert!(info.dw_number_of_processors >= 1);
    }

    #[test]
    fn test_system_info_native_and_process_consistency() {
        let mut native_info: SystemInfo = unsafe { std::mem::zeroed() };
        let mut sys_info: SystemInfo = unsafe { std::mem::zeroed() };

        unsafe {
            GetNativeSystemInfo(&mut native_info);
            GetSystemInfo(&mut sys_info);
        }

        assert_eq!(native_info, sys_info);
    }

    #[test]
    fn test_get_native_system_info_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*mut SystemInfo) = GetNativeSystemInfo;
        let mut info: SystemInfo = unsafe { std::mem::zeroed() };
        unsafe {
            func_ptr(&mut info);
        }
        assert_eq!(info.w_processor_architecture, PROCESSOR_ARCHITECTURE_AMD64);
    }

    #[test]
    fn test_create_directory_w_contract_suite() {
        let _lock = CWD_TEST_LOCK.lock().unwrap();

        let temp_base = std::env::temp_dir().join(format!(
            "rine_test_mkdir_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_base).unwrap();

        let temp_base_str = temp_base.to_string_lossy().to_string();
        initialize_process_filesystem("/tmp/dummy.exe", Some(&temp_base_str));
        let initial_cwd = get_process_current_directory();

        // 1. Success creating single directory
        let dir1_utf16: Vec<u16> = "new_folder"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let res = unsafe { CreateDirectoryW(dir1_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(res, 1, "CreateDirectoryW must return 1 on success");
        assert!(temp_base.join("new_folder").is_dir());

        // 2. Failure: already exists
        let res_dup = unsafe { CreateDirectoryW(dir1_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(
            res_dup, 0,
            "CreateDirectoryW must return 0 if already exists"
        );
        let err = unsafe { GetLastError() };
        assert_eq!(err, 183, "Must set ERROR_ALREADY_EXISTS (183)");

        // 3. Success creating Unicode directory
        let u_name = "Юникод_Папка_Тест";
        let u_utf16: Vec<u16> = u_name.encode_utf16().chain(std::iter::once(0)).collect();
        let res_u = unsafe { CreateDirectoryW(u_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(res_u, 1, "CreateDirectoryW must create Unicode directories");
        assert!(temp_base.join(u_name).is_dir());

        // 4. Failure: missing parent (non-recursive)
        let deep_utf16: Vec<u16> = "missing_parent\\child"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let res_deep = unsafe { CreateDirectoryW(deep_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(
            res_deep, 0,
            "Non-recursive mkdir must fail if parent missing"
        );
        let err_deep = unsafe { GetLastError() };
        assert_eq!(err_deep, 3, "Must set ERROR_PATH_NOT_FOUND (3)");

        // 5. Failure: file collision
        let file_path = temp_base.join("colliding_file.txt");
        std::fs::write(&file_path, b"dummy").unwrap();
        let col_utf16: Vec<u16> = "colliding_file.txt"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let res_col = unsafe { CreateDirectoryW(col_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(res_col, 0, "Must fail if path exists as file");
        let err_col = unsafe { GetLastError() };
        assert_eq!(err_col, 183, "Must set ERROR_ALREADY_EXISTS (183)");

        // 6. Invariant: Process CWD was NOT altered by CreateDirectoryW
        assert_eq!(get_process_current_directory(), initial_cwd);

        // 7. Executable Win64 calling convention test
        let func_ptr: unsafe extern "win64" fn(*const u16, *const std::ffi::c_void) -> i32 =
            CreateDirectoryW;
        let win64_dir_utf16: Vec<u16> = "win64_call_dir"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let res_win64 = unsafe { func_ptr(win64_dir_utf16.as_ptr(), std::ptr::null()) };
        assert_eq!(res_win64, 1);
        assert!(temp_base.join("win64_call_dir").is_dir());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_base);
    }

    #[test]
    fn test_systemtime_win64_layout() {
        assert_eq!(
            std::mem::size_of::<SystemTime>(),
            16,
            "SYSTEMTIME size must be exactly 16 bytes"
        );
        assert_eq!(
            std::mem::align_of::<SystemTime>(),
            2,
            "SYSTEMTIME alignment must be 2 bytes"
        );

        let dummy = SystemTime {
            w_year: 0,
            w_month: 0,
            w_day_of_week: 0,
            w_day: 0,
            w_hour: 0,
            w_minute: 0,
            w_second: 0,
            w_milliseconds: 0,
        };
        let base = &raw const dummy as usize;
        assert_eq!((&raw const dummy.w_year as usize) - base, 0);
        assert_eq!((&raw const dummy.w_month as usize) - base, 2);
        assert_eq!((&raw const dummy.w_day_of_week as usize) - base, 4);
        assert_eq!((&raw const dummy.w_day as usize) - base, 6);
        assert_eq!((&raw const dummy.w_hour as usize) - base, 8);
        assert_eq!((&raw const dummy.w_minute as usize) - base, 10);
        assert_eq!((&raw const dummy.w_second as usize) - base, 12);
        assert_eq!((&raw const dummy.w_milliseconds as usize) - base, 14);
    }

    #[test]
    fn test_get_local_time_buffer_bounds() {
        #[repr(C)]
        struct GuardedSystemTime {
            guard_before: [u8; 16],
            time: SystemTime,
            guard_after: [u8; 16],
        }

        let mut guarded = GuardedSystemTime {
            guard_before: [0xAA; 16],
            time: unsafe { std::mem::zeroed() },
            guard_after: [0xBB; 16],
        };

        unsafe {
            GetLocalTime(&mut guarded.time);
        }

        assert_eq!(
            guarded.guard_before, [0xAA; 16],
            "guard_before was overwritten!"
        );
        assert_eq!(
            guarded.guard_after, [0xBB; 16],
            "guard_after was overwritten!"
        );
        assert!(guarded.time.w_year >= 2024);
        assert!(guarded.time.w_month >= 1 && guarded.time.w_month <= 12);
        assert!(guarded.time.w_day >= 1 && guarded.time.w_day <= 31);
        assert!(guarded.time.w_day_of_week <= 6);
        assert!(guarded.time.w_hour <= 23);
        assert!(guarded.time.w_minute <= 59);
        assert!(guarded.time.w_second <= 59);
        assert!(guarded.time.w_milliseconds <= 999);
    }

    #[test]
    fn test_get_local_time_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*mut SystemTime) = GetLocalTime;
        let mut st: SystemTime = unsafe { std::mem::zeroed() };
        unsafe {
            func_ptr(&mut st);
        }
        assert!(st.w_year >= 2024);
        assert!(st.w_month >= 1 && st.w_month <= 12);
    }

    #[test]
    fn test_time_zone_information_win64_layout() {
        assert_eq!(
            std::mem::size_of::<TimeZoneInformation>(),
            172,
            "TIME_ZONE_INFORMATION size must be exactly 172 bytes"
        );
        assert_eq!(
            std::mem::align_of::<TimeZoneInformation>(),
            4,
            "TIME_ZONE_INFORMATION alignment must be 4 bytes"
        );

        let dummy = TimeZoneInformation {
            bias: 0,
            standard_name: [0u16; 32],
            standard_date: SystemTime {
                w_year: 0,
                w_month: 0,
                w_day_of_week: 0,
                w_day: 0,
                w_hour: 0,
                w_minute: 0,
                w_second: 0,
                w_milliseconds: 0,
            },
            standard_bias: 0,
            daylight_name: [0u16; 32],
            daylight_date: SystemTime {
                w_year: 0,
                w_month: 0,
                w_day_of_week: 0,
                w_day: 0,
                w_hour: 0,
                w_minute: 0,
                w_second: 0,
                w_milliseconds: 0,
            },
            daylight_bias: 0,
        };
        let base = &raw const dummy as usize;
        assert_eq!((&raw const dummy.bias as usize) - base, 0);
        assert_eq!((&raw const dummy.standard_name as usize) - base, 4);
        assert_eq!((&raw const dummy.standard_date as usize) - base, 68);
        assert_eq!((&raw const dummy.standard_bias as usize) - base, 84);
        assert_eq!((&raw const dummy.daylight_name as usize) - base, 88);
        assert_eq!((&raw const dummy.daylight_date as usize) - base, 152);
        assert_eq!((&raw const dummy.daylight_bias as usize) - base, 168);
    }

    #[test]
    fn test_get_time_zone_information_buffer_bounds() {
        #[repr(C)]
        struct GuardedTzi {
            guard_before: [u8; 16],
            info: TimeZoneInformation,
            guard_after: [u8; 16],
        }

        let mut guarded = GuardedTzi {
            guard_before: [0xCC; 16],
            info: unsafe { std::mem::zeroed() },
            guard_after: [0xDD; 16],
        };

        let ret = unsafe { GetTimeZoneInformation(&mut guarded.info) };
        assert_ne!(
            ret, TIME_ZONE_ID_INVALID,
            "GetTimeZoneInformation must not return INVALID"
        );
        assert_eq!(
            guarded.guard_before, [0xCC; 16],
            "guard_before was overwritten!"
        );
        assert_eq!(
            guarded.guard_after, [0xDD; 16],
            "guard_after was overwritten!"
        );
    }

    #[test]
    fn test_get_time_zone_information_bias_sign() {
        let mut tzi: TimeZoneInformation = unsafe { std::mem::zeroed() };
        let ret = unsafe { GetTimeZoneInformation(&mut tzi) };
        assert!(
            ret == TIME_ZONE_ID_UNKNOWN
                || ret == TIME_ZONE_ID_STANDARD
                || ret == TIME_ZONE_ID_DAYLIGHT
        );

        // Verification of Bias sign:
        // Host local time and UTC time check
        let local_st = WallClock::now_local();
        let utc_st = WallClock::now_utc();

        // Convert hours & minutes to total minutes within day
        let local_mins = (local_st.w_hour as i32 * 60) + local_st.w_minute as i32;
        let utc_mins = (utc_st.w_hour as i32 * 60) + utc_st.w_minute as i32;

        // Difference modulo 1440 minutes (24h)
        let diff_mins = (utc_mins - local_mins + 1440) % 1440;
        let expected_bias = if diff_mins > 720 {
            diff_mins - 1440
        } else {
            diff_mins
        };

        assert_eq!(
            tzi.bias, expected_bias,
            "Windows Bias must satisfy UTC = Local + Bias"
        );
    }

    #[test]
    fn test_get_time_zone_information_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*mut TimeZoneInformation) -> u32 =
            GetTimeZoneInformation;
        let mut tzi: TimeZoneInformation = unsafe { std::mem::zeroed() };
        let res = unsafe { func_ptr(&mut tzi) };
        assert_ne!(res, TIME_ZONE_ID_INVALID);
    }

    #[test]
    fn test_get_environment_variable_w_found_and_not_found() {
        GuestEnvironment::set("RINE_TEST_KEY", Some("TestValue123"));

        // 1. Found with sufficient buffer
        let key_utf16: Vec<u16> = "RINE_TEST_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut buf = [0u16; 64];
        let len = unsafe { GetEnvironmentVariableW(key_utf16.as_ptr(), buf.as_mut_ptr(), 64) };
        assert_eq!(len, 12);
        let val = String::from_utf16(&buf[..12]).unwrap();
        assert_eq!(val, "TestValue123");
        assert_eq!(buf[12], 0);

        // 2. Case-insensitivity
        let key_lower_utf16: Vec<u16> = "rine_test_key"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let len_lower =
            unsafe { GetEnvironmentVariableW(key_lower_utf16.as_ptr(), buf.as_mut_ptr(), 64) };
        assert_eq!(len_lower, 12);

        // 3. Not found
        let key_missing: Vec<u16> = "NONEXISTENT_ENV_VAR"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let len_missing =
            unsafe { GetEnvironmentVariableW(key_missing.as_ptr(), buf.as_mut_ptr(), 64) };
        assert_eq!(len_missing, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 203, "Must set ERROR_ENVVAR_NOT_FOUND (203)");
    }

    #[test]
    fn test_get_environment_variable_w_insufficient_buffer() {
        GuestEnvironment::set("SHORT_KEY", Some("abcd"));
        let key_utf16: Vec<u16> = "SHORT_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut buf = [0u16; 4]; // capacity is 4, but required capacity with null is 5
        let req_len = unsafe { GetEnvironmentVariableW(key_utf16.as_ptr(), buf.as_mut_ptr(), 4) };
        assert_eq!(
            req_len, 5,
            "Must return required capacity including null terminator"
        );
    }

    #[test]
    fn test_get_environment_variable_w_unicode() {
        GuestEnvironment::set("UNICODE_KEY", Some("ТестовоеЗначение"));
        let key_utf16: Vec<u16> = "UNICODE_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut buf = [0u16; 64];
        let len = unsafe { GetEnvironmentVariableW(key_utf16.as_ptr(), buf.as_mut_ptr(), 64) };
        assert_eq!(len, 16);
        let val = String::from_utf16(&buf[..16]).unwrap();
        assert_eq!(val, "ТестовоеЗначение");
    }

    #[test]
    fn test_get_environment_variable_w_buffer_bounds() {
        GuestEnvironment::set("BOUNDS_KEY", Some("hello"));
        let key_utf16: Vec<u16> = "BOUNDS_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        #[repr(C)]
        struct GuardedBuf {
            guard_before: [u16; 8],
            data: [u16; 6], // "hello\0"
            guard_after: [u16; 8],
        }

        let mut guarded = GuardedBuf {
            guard_before: [0xAAAA; 8],
            data: [0; 6],
            guard_after: [0xBBBB; 8],
        };

        let ret =
            unsafe { GetEnvironmentVariableW(key_utf16.as_ptr(), guarded.data.as_mut_ptr(), 6) };
        assert_eq!(ret, 5);
        assert_eq!(guarded.guard_before, [0xAAAA; 8]);
        assert_eq!(guarded.guard_after, [0xBBBB; 8]);
    }

    #[test]
    fn test_get_environment_variable_w_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*const u16, *mut u16, u32) -> u32 =
            GetEnvironmentVariableW;
        GuestEnvironment::set("WIN64_ENV_KEY", Some("win64_val"));
        let key_utf16: Vec<u16> = "WIN64_ENV_KEY"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut buf = [0u16; 32];
        let ret = unsafe { func_ptr(key_utf16.as_ptr(), buf.as_mut_ptr(), 32) };
        assert_eq!(ret, 9);
    }

    #[test]
    fn test_reason_context_win64_layout() {
        assert_eq!(
            std::mem::size_of::<ReasonContext>(),
            32,
            "REASON_CONTEXT size must be exactly 32 bytes"
        );
        assert_eq!(
            std::mem::align_of::<ReasonContext>(),
            8,
            "REASON_CONTEXT alignment must be 8 bytes"
        );

        let dummy = ReasonContext {
            version: 0,
            flags: 0,
            reason: ReasonContextUnion {
                simple_reason_string: std::ptr::null(),
            },
        };
        let base = &raw const dummy as usize;
        assert_eq!((&raw const dummy.version as usize) - base, 0);
        assert_eq!((&raw const dummy.flags as usize) - base, 4);
        assert_eq!((&raw const dummy.reason as usize) - base, 8);
    }

    #[test]
    fn test_power_request_lifecycle_and_counting() {
        let reason_utf16: Vec<u16> = "Test Reason"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let ctx = ReasonContext {
            version: POWER_REQUEST_CONTEXT_VERSION,
            flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            reason: ReasonContextUnion {
                simple_reason_string: reason_utf16.as_ptr(),
            },
        };

        // 1. Create valid power request
        let handle = unsafe { PowerCreateRequest(&ctx) };
        assert_ne!(handle, 0);
        assert_ne!(handle, 0xFFFFFFFFFFFFFFFFu64);

        // 2. Set requests and check counting
        let res_sys1 = unsafe { PowerSetRequest(handle, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_sys1, 1);
        let res_sys2 = unsafe { PowerSetRequest(handle, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_sys2, 1);
        let res_disp = unsafe { PowerSetRequest(handle, POWER_REQUEST_DISPLAY_REQUIRED) };
        assert_eq!(res_disp, 1);

        {
            let g = POWER_REQUEST_TABLE.lock().unwrap();
            let table = g.as_ref().unwrap();
            let obj_arc = table.get(&handle).unwrap();
            let obj = obj_arc.lock().unwrap();
            assert_eq!(obj.system_required, 2);
            assert_eq!(obj.display_required, 1);
            assert_eq!(obj.reason, "Test Reason");
        }

        // 3. Clear requests
        let res_clr_sys = unsafe { PowerClearRequest(handle, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_clr_sys, 1);
        {
            let g = POWER_REQUEST_TABLE.lock().unwrap();
            let table = g.as_ref().unwrap();
            let obj_arc = table.get(&handle).unwrap();
            let obj = obj_arc.lock().unwrap();
            assert_eq!(obj.system_required, 1);
        }

        // 4. Invalid handle rejection
        let res_invalid = unsafe { PowerSetRequest(0xDEADBEEF, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_invalid, 0);
        let err = unsafe { GetLastError() };
        assert_eq!(err, 6); // ERROR_INVALID_HANDLE

        // 5. CloseHandle invalidates the handle
        let close_res = unsafe { CloseHandle(handle) };
        assert_eq!(close_res, 1);
        let res_after_close = unsafe { PowerSetRequest(handle, POWER_REQUEST_SYSTEM_REQUIRED) };
        assert_eq!(res_after_close, 0);

        // 6. Invalid context rejection on create
        let invalid_ctx = ReasonContext {
            version: 999, // Invalid version
            flags: 0,
            reason: ReasonContextUnion {
                simple_reason_string: std::ptr::null(),
            },
        };
        let bad_handle = unsafe { PowerCreateRequest(&invalid_ctx) };
        assert_eq!(bad_handle, 0xFFFFFFFFFFFFFFFFu64);
        let null_handle = unsafe { PowerCreateRequest(std::ptr::null()) };
        assert_eq!(null_handle, 0xFFFFFFFFFFFFFFFFu64);
    }

    #[test]
    fn test_power_create_request_win64_call() {
        let func_ptr: unsafe extern "win64" fn(*const ReasonContext) -> u64 = PowerCreateRequest;
        let ctx = ReasonContext {
            version: POWER_REQUEST_CONTEXT_VERSION,
            flags: 0,
            reason: ReasonContextUnion {
                simple_reason_string: std::ptr::null(),
            },
        };
        let h = unsafe { func_ptr(&ctx) };
        assert_ne!(h, 0xFFFFFFFFFFFFFFFFu64);
        unsafe { CloseHandle(h) };
    }

    #[test]
    fn test_module_registry_and_load_library_lifecycle() {
        // 1. LoadLibraryW for ntdll.dll
        let ntdll_u16: Vec<u16> = "ntdll.dll"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let h1 = unsafe { LoadLibraryW(ntdll_u16.as_ptr()) };
        assert_eq!(h1, 0x7FFD0000, "ntdll must return stable base 0x7FFD0000");

        // 2. Case-insensitivity & repeated load
        let ntdll_upper_u16: Vec<u16> = "NTDLL.DLL"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let h2 = unsafe { LoadLibraryW(ntdll_upper_u16.as_ptr()) };
        assert_eq!(h1, h2, "Repeated load must return same HMODULE");

        // 3. GetProcAddress scoped to ntdll
        let proc_name = std::ffi::CString::new("RtlGetVersion").unwrap();
        let fn_ptr = unsafe { GetProcAddress(h1, proc_name.as_ptr()) };
        assert_ne!(
            fn_ptr, 0,
            "GetProcAddress must resolve RtlGetVersion from ntdll"
        );

        // 4. FreeLibrary
        let free1 = unsafe { FreeLibrary(h1) };
        assert_eq!(free1, 1);
        let free2 = unsafe { FreeLibrary(h2) };
        assert_eq!(free2, 1);

        // 5. FreeLibrary on invalid handle
        let free_invalid = unsafe { FreeLibrary(0x12345678) };
        assert_eq!(free_invalid, 0);

        // 6. FreeLibrary on main EXE base must be rejected / protected
        let exe_base = CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::SeqCst) as usize;
        let free_exe = unsafe { FreeLibrary(exe_base) };
        assert_eq!(free_exe, 0, "Main EXE must not be freed");
    }

    #[test]
    fn test_free_library_win64_call() {
        let func_ptr: unsafe extern "win64" fn(usize) -> i32 = FreeLibrary;
        let res = unsafe { func_ptr(0x7FFD0000) };
        assert_eq!(res, 1);
    }

    #[test]
    fn test_get_user_default_ui_language_resolver() {
        let sym = crate::pe::imports::IatSymbolResolver::resolve_symbol(
            "KERNEL32.dll",
            "GetUserDefaultUILanguage",
        );
        assert!(
            sym.is_some(),
            "KERNEL32.dll!GetUserDefaultUILanguage must be resolved"
        );
    }

    #[test]
    fn test_get_user_default_ui_language_win64_call() {
        let func_ptr: unsafe extern "win64" fn() -> u16 = GetUserDefaultUILanguage;
        let lang = unsafe { func_ptr() };
        assert_ne!(lang, 0, "LANGID must be non-zero");
    }

    #[test]
    fn test_guest_ui_language_mapping() {
        let loc_ru = GuestLocale::from_env_str("ru_RU.UTF-8");
        assert_eq!(loc_ru.lang_id, 0x0419);
        assert_eq!(loc_ru.lcid, 0x00000419);
        assert_eq!(loc_ru.locale_name, "ru-RU");

        let loc_de = GuestLocale::from_env_str("de_DE.UTF-8");
        assert_eq!(loc_de.lang_id, 0x0407);
        assert_eq!(loc_de.locale_name, "de-DE");

        let loc_fr = GuestLocale::from_env_str("fr_FR.UTF-8");
        assert_eq!(loc_fr.lang_id, 0x040c);
        assert_eq!(loc_fr.locale_name, "fr-FR");
    }

    #[test]
    fn test_guest_ui_language_fallback() {
        let loc_unknown = GuestLocale::from_env_str("unknown_LOCALE");
        assert_eq!(loc_unknown.lang_id, 0x0409);
        assert_eq!(loc_unknown.lcid, 0x00000409);
        assert_eq!(loc_unknown.locale_name, "en-US");
    }

    #[test]
    fn test_virtual_alloc_and_free() {
        unsafe {
            let addr = VirtualAlloc(0, 65536, 0x1000, 0x04);
            assert!(addr > 0x10000, "VirtualAlloc must return valid address");
            let res = VirtualFree(addr, 0, 0x8000);
            assert_eq!(res, 1, "VirtualFree must succeed");
        }
    }

    #[test]
    fn test_get_handle_information() {
        let mut flags = 0xFFFFFFFFu32;
        let res = unsafe { GetHandleInformation(0x1234, &mut flags) };
        assert_eq!(res, 1);
        assert_eq!(flags, 0);

        let set_res = unsafe { SetHandleInformation(0x1234, 1, 1) };
        assert_eq!(set_res, 1);
    }

    #[test]
    fn test_win32_to_host_path_user_mapping() {
        let path = "C:\\Users\\fizzy\\AppData\\Roaming\\Godot\\app_userdata\\test";
        let mapped = win32_to_host_path(path);
        assert!(mapped.contains("drive_c"));
        let lower = mapped.to_ascii_lowercase();
        assert!(lower.ends_with("/users/fizzy/appdata/roaming/godot/app_userdata/test"));
    }

    #[test]
    fn test_condition_variable_srw_no_lost_wakeup() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::thread;

        let var = Box::into_raw(Box::new(0usize));
        let lock = Box::into_raw(Box::new(0usize));

        unsafe {
            InitializeConditionVariable(var);
            InitializeSRWLock(lock);
        }

        let ready = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));

        let ready_clone = ready.clone();
        let finished_clone = finished.clone();
        let var_addr = var as usize;
        let lock_addr = lock as usize;

        let handle = thread::spawn(move || {
            let v = var_addr as *mut usize;
            let l = lock_addr as *mut usize;
            unsafe {
                AcquireSRWLockExclusive(l);
                ready_clone.store(true, Ordering::SeqCst);
                let res = SleepConditionVariableSRW(v, l, 5000, 0);
                assert_eq!(res, 1, "SleepConditionVariableSRW should succeed");
                ReleaseSRWLockExclusive(l);
            }
            finished_clone.store(true, Ordering::SeqCst);
        });

        // Spin until worker acquires lock and prepares
        while !ready.load(Ordering::SeqCst) {
            std::hint::spin_loop();
        }

        // Wait a tiny bit to ensure worker is inside SleepConditionVariableSRW
        std::thread::sleep(std::time::Duration::from_millis(50));

        unsafe {
            AcquireSRWLockExclusive(lock);
            WakeConditionVariable(var);
            ReleaseSRWLockExclusive(lock);
        }

        handle.join().expect("Worker thread panicked");
        assert!(finished.load(Ordering::SeqCst), "Worker thread must finish after wake");

        unsafe {
            let _ = Box::from_raw(var);
            let _ = Box::from_raw(lock);
        }
    }

    #[test]
    fn test_format_message_w_and_a_allocate_buffer() {
        unsafe {
            // Test FormatMessageW with FORMAT_MESSAGE_ALLOCATE_BUFFER
            let mut buf_ptr: *mut u16 = std::ptr::null_mut();
            let ret = FormatMessageW(
                0x00000100, // FORMAT_MESSAGE_ALLOCATE_BUFFER
                std::ptr::null(),
                126, // ERROR_MOD_NOT_FOUND
                0,
                &mut buf_ptr as *mut *mut u16 as *mut u16,
                0,
                std::ptr::null_mut(),
            );
            assert!(ret > 0, "FormatMessageW must return length > 0");
            assert!(!buf_ptr.is_null(), "Allocated buffer must not be null");
            let slice = std::slice::from_raw_parts(buf_ptr, ret as usize);
            let s = String::from_utf16_lossy(slice);
            assert!(s.contains("specified module could not be found"));
            LocalFree(buf_ptr as usize);

            // Test FormatMessageA with fixed buffer
            let mut buf = [0u8; 128];
            let ret_a = FormatMessageA(
                0,
                std::ptr::null(),
                2, // ERROR_FILE_NOT_FOUND
                0,
                buf.as_mut_ptr(),
                buf.len() as u32,
                std::ptr::null_mut(),
            );
            assert!(ret_a > 0);
            let s_a = std::str::from_utf8(&buf[..ret_a as usize]).unwrap();
            assert!(s_a.contains("system cannot find the file"));
        }
    }

    #[test]
    fn test_wait_for_multiple_objects_nondestructive_bwaitall() {
        unsafe {
            // Create 3 auto-reset events:
            let ev1 = CreateEventW(0, 0, 0, 0) as u64;
            let ev2 = CreateEventW(0, 0, 0, 0) as u64;
            let ev3 = CreateEventW(0, 0, 0, 0) as u64;
            assert!(ev1 != 0 && ev2 != 0 && ev3 != 0);

            let handles = [ev1, ev2, ev3];

            // 1. None signaled -> timeout
            let res = WaitForMultipleObjects(3, handles.as_ptr(), 1, 10);
            assert_eq!(res, 0x102); // WAIT_TIMEOUT

            // 2. Signal only ev1. Since wait_all=1, it should time out and NOT consume ev1!
            SetEvent(ev1);
            let res = WaitForMultipleObjects(3, handles.as_ptr(), 1, 10);
            assert_eq!(res, 0x102); // WAIT_TIMEOUT
            // ev1 MUST still be signaled!
            assert_eq!(WaitForSingleObject(ev1, 0), 0); // WAIT_OBJECT_0

            // 3. Signal ev1 and ev2, ev3 after 30ms in worker thread
            SetEvent(ev1);
            SetEvent(ev2);
            let ev3_copy = ev3;
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(30));
                SetEvent(ev3_copy);
            });

            // Wait with wait_all = 1
            let res = WaitForMultipleObjects(3, handles.as_ptr(), 1, 2000);
            assert_eq!(res, 0); // WAIT_OBJECT_0

            // Now all three auto-reset events must have been consumed
            assert_eq!(WaitForSingleObject(ev1, 0), 0x102);
            assert_eq!(WaitForSingleObject(ev2, 0), 0x102);
            assert_eq!(WaitForSingleObject(ev3, 0), 0x102);

            CloseHandle(ev1);
            CloseHandle(ev2);
            CloseHandle(ev3);
        }
    }

    #[test]
    fn test_slist_push_pop_flush_and_depth() {
        unsafe {
            #[repr(align(16))]
            struct AlignedHeader([u64; 2]);
            let mut header = AlignedHeader([0, 0]);
            InitializeSListHead(header.0.as_mut_ptr());
            assert_eq!(QueryDepthSList(header.0.as_ptr()), 0);
            assert!(InterlockedPopEntrySList(header.0.as_mut_ptr()).is_null());

            let mut node1: usize = 0;
            let mut node2: usize = 0;
            let mut node3: usize = 0;

            assert!(InterlockedPushEntrySList(header.0.as_mut_ptr(), &mut node1).is_null());
            assert_eq!(QueryDepthSList(header.0.as_ptr()), 1);

            let prev = InterlockedPushEntrySList(header.0.as_mut_ptr(), &mut node2);
            assert_eq!(prev, &mut node1 as *mut usize);
            assert_eq!(QueryDepthSList(header.0.as_ptr()), 2);

            let prev2 = InterlockedPushEntrySList(header.0.as_mut_ptr(), &mut node3);
            assert_eq!(prev2, &mut node2 as *mut usize);
            assert_eq!(QueryDepthSList(header.0.as_ptr()), 3);

            assert_eq!(InterlockedPopEntrySList(header.0.as_mut_ptr()), &mut node3 as *mut usize);
            assert_eq!(QueryDepthSList(header.0.as_ptr()), 2);

            let flushed = InterlockedFlushSList(header.0.as_mut_ptr());
            assert_eq!(flushed, &mut node2 as *mut usize);
            assert_eq!(QueryDepthSList(header.0.as_ptr()), 0);
            assert!(InterlockedPopEntrySList(header.0.as_mut_ptr()).is_null());
        }
    }

    #[test]
    fn test_get_logical_processor_information_ex() {
        unsafe {
            let mut len: u32 = 0;
            let res = GetLogicalProcessorInformationEx(0, std::ptr::null_mut(), &mut len);
            assert_eq!(res, 0);
            assert_eq!(GetLastError(), 122); // ERROR_INSUFFICIENT_BUFFER
            assert!(len >= 48);

            let mut buf = vec![0u8; len as usize];
            let res2 = GetLogicalProcessorInformationEx(0, buf.as_mut_ptr(), &mut len);
            assert_eq!(res2, 1);
            assert_eq!(u32::from_le_bytes(buf[0..4].try_into().unwrap()), 0);
            assert_eq!(u32::from_le_bytes(buf[4..8].try_into().unwrap()), 48);
        }
    }
}
