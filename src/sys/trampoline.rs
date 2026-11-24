use crate::sys::{Result, RineError};

pub const ARCH_SET_GS: libc::c_int = 0x1001;
pub const ARCH_SET_FS: libc::c_int = 0x1002;

/// Sets up the x86_64 %gs segment base register to point to Win64 TEB64.
pub fn set_thread_teb_gs_base(teb_address: u64) -> Result<()> {
    let res = unsafe { libc::syscall(libc::SYS_arch_prctl, ARCH_SET_GS, teb_address) };
    if res != 0 {
        return Err(RineError::MemoryMappingFailed(format!(
            "arch_prctl(ARCH_SET_GS) failed with errno {}",
            unsafe { *libc::__errno_location() }
        )));
    }
    crate::dlls::kernel32::CACHED_TEB_PTR.with(|c| {
        c.set(teb_address as *mut crate::pe::teb::Teb64);
    });
    Ok(())
}

#[no_mangle]
pub unsafe extern "win64" fn rine_exit_thread_stub(exit_code: u64) -> ! {
    eprintln!(
        "[GUEST EXITED] PE entry point returned with RAX=0x{:016X}; host window pump remains active.",
        exit_code
    );

    crate::dlls::user32::run_main_window_event_loop();

    std::process::exit((exit_code & 0xFFFFFFFF) as i32);
}

pub fn get_current_thread_stack_bounds() -> (u64, u64) {
    unsafe {
        let mut stackaddr: *mut libc::c_void = std::ptr::null_mut();
        let mut stacksize: usize = 0;
        let mut attr: libc::pthread_attr_t = std::mem::zeroed();
        if libc::pthread_getattr_np(libc::pthread_self(), &mut attr) == 0 {
            libc::pthread_attr_getstack(&attr, &mut stackaddr, &mut stacksize);
            let mut guardsize: usize = 0;
            libc::pthread_attr_getguardsize(&attr, &mut guardsize);
            libc::pthread_attr_destroy(&mut attr);
            let limit = (stackaddr as u64) + (guardsize as u64);
            let base = (stackaddr as u64) + (stacksize as u64);
            println!(
                "[STACK BOUNDS] stackaddr={:p} stacksize=0x{:X} guardsize=0x{:X} => limit=0x{:X} base=0x{:X}",
                stackaddr, stacksize, guardsize, limit, base
            );
            return (limit, base);
        }
        let rsp: u64;
        std::arch::asm!("mov {}, rsp", out(reg) rsp);
        (rsp.saturating_sub(0x10000), rsp + 0x1000)
    }
}

#[no_mangle]
pub unsafe extern "C" fn jump_to_win64_entry_point(
    entry_point: u64,
    peb_address: u64,
    teb_address: u64,
) -> ! {
    use std::io::Write;

    println!("[RINE EXEC] >>> INSIDE jump_to_win64_entry_point <<<");
    println!(
        "[RINE EXEC] Entry Point: 0x{:016X}, PEB: 0x{:016X}, TEB: 0x{:016X}",
        entry_point, peb_address, teb_address
    );
    let _ = std::io::stdout().flush();

    let code_ptr = entry_point as *const u8;
    if !code_ptr.is_null() {
        let bytes = std::slice::from_raw_parts(code_ptr, 64);
        println!("[RINE EXEC] Entry Point Opcodes (64 bytes): {:02X?}", bytes);
        let _ = std::io::stdout().flush();
    }

    println!("[RINE EXEC] Setting GS Base to TEB: 0x{:016X}", teb_address);
    let _ = std::io::stdout().flush();

    let res = set_thread_teb_gs_base(teb_address);
    println!("[RINE EXEC] GS Base status: {:?}", res);
    let _ = std::io::stdout().flush();

    // Initialize main thread TLS slots for all loaded PE modules
    let p_teb = teb_address as *mut crate::pe::teb::Teb64;
    if !p_teb.is_null() {
        let tls_array_addr = (*p_teb).thread_local_storage_pointer;
        crate::mem::loader::init_thread_tls(tls_array_addr);
        println!(
            "[RINE EXEC] Initialized main thread TLS slots @ 0x{:016X}",
            tls_array_addr
        );
    }

    let peb_ptr = peb_address as *const u64;
    let image_base = if !peb_ptr.is_null() {
        unsafe { *peb_ptr.add(2) } // Offset 0x10 in PEB is ImageBaseAddress
    } else {
        0x0000000140000000
    };

    // Populate TEB StackBase (gs:[0x08]) and StackLimit (gs:[0x10]) with real thread stack range
    let (real_stack_limit, real_stack_base) = get_current_thread_stack_bounds();
    let current_rsp: u64;
    std::arch::asm!("mov {}, rsp", out(reg) current_rsp);

    let stack_base_ptr = (teb_address + 0x08) as *mut u64;
    let stack_limit_ptr = (teb_address + 0x10) as *mut u64;
    *stack_base_ptr = real_stack_base;
    *stack_limit_ptr = real_stack_limit;

    let exit_stub_ptr = rine_exit_thread_stub as *const () as u64;
    let safe_buf = crate::dlls::registry::get_dummy_stub_buffer_ptr() as u64;

    pub static mut G_TRAMPOLINE_PRE_JUMP_TARGET: u64 = 0;

    let entry_rva = entry_point.saturating_sub(image_base);
    println!(
        "[RINE EXEC LOG Target] Final Jump Target VA=0x{:016X} (ImageBase=0x{:016X} + AddressOfEntryPoint RVA=0x{:08X})",
        entry_point, image_base, entry_rva
    );
    eprintln!(
        "[RINE TRAMPOLINE PRE-JUMP CONFIRMATION] Register RAX pre-bound to Entry Point VA = 0x{:016X}, RSP = 0x{:016X}, TEB = 0x{:016X}, ReturnStub = 0x{:016X}",
        entry_point, current_rsp, teb_address, exit_stub_ptr
    );
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    let target_slot_ptr = &raw mut G_TRAMPOLINE_PRE_JUMP_TARGET;

    #[cfg(target_arch = "x86_64")]
    std::arch::asm!(
        "mov [{target_slot}], rax",
        "mov rbp, rsp",
        "and rsp, -16",
        "push r10",
        "sub rsp, 0x20",
        "mov rsi, r11",
        "mov rdi, r11",
        "xor rcx, rcx",
        "xor rdx, rdx",
        "xor r8, r8",
        "xor r9, r9",
        "xor rbx, rbx",
        "xor r12, r12",
        "xor r13, r13",
        "xor r14, r14",
        "xor r15, r15",
        "jmp rax",
        target_slot = in(reg) target_slot_ptr,
        in("rax") entry_point,
        in("r10") exit_stub_ptr,
        in("r11") safe_buf,
        options(noreturn)
    );

    #[cfg(not(target_arch = "x86_64"))]
    panic!("Win64 execution requires x86_64 architecture!");
}
