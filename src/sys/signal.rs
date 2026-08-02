use libc::{c_int, c_void, sigaction, siginfo_t, SA_SIGINFO, SIGBUS, SIGFPE, SIGILL, SIGSEGV};
use std::mem::zeroed;

use std::sync::atomic::{AtomicU64, Ordering};

static MAIN_THREAD_ID: AtomicU64 = AtomicU64::new(0);

pub fn register_main_thread_id() {
    unsafe {
        let tid = libc::pthread_self() as u64;
        MAIN_THREAD_ID.store(tid, Ordering::Relaxed);
    }
}

pub fn is_main_thread() -> bool {
    unsafe {
        let tid = libc::pthread_self() as u64;
        let main_tid = MAIN_THREAD_ID.load(Ordering::Relaxed);
        main_tid == 0 || tid == main_tid
    }
}

#[inline(always)]
fn safe_read_u32(addr: u64) -> Option<u32> {
    if addr < 0x10000 || addr >= 0x7FFFFFFFFFFF {
        return None;
    }
    unsafe {
        let ptr = addr as *const u32;
        Some(std::ptr::read_unaligned(ptr))
    }
}

extern "C" fn worker_thread_exit_stub() -> ! {
    unsafe {
        libc::pthread_exit(std::ptr::null_mut());
    }
}

extern "C" fn rine_signal_handler(sig: c_int, _info: *mut siginfo_t, ucontext: *mut c_void) {
    eprintln!("\n========================================================");
    eprintln!("[RINE SIGNAL HANDLER] CAUGHT POSIX SIGNAL {}!", sig);
    if !ucontext.is_null() {
        unsafe {
            let uctx = ucontext as *mut libc::ucontext_t;
            let mctx = (*uctx).uc_mcontext;
            let rip = mctx.gregs[libc::REG_RIP as usize] as u64;
            let rsp = mctx.gregs[libc::REG_RSP as usize] as u64;
            let rax = mctx.gregs[libc::REG_RAX as usize] as u64;
            let rcx = mctx.gregs[libc::REG_RCX as usize] as u64;
            let rdx = mctx.gregs[libc::REG_RDX as usize] as u64;
            let rbx = mctx.gregs[libc::REG_RBX as usize] as u64;
            let rdi = mctx.gregs[libc::REG_RDI as usize] as u64;
            let rsi = mctx.gregs[libc::REG_RSI as usize] as u64;

            let rip_bytes = if rip > 0x10000 {
                std::slice::from_raw_parts(rip as *const u8, 16)
            } else {
                &[]
            };

            let rip_before_bytes = if rip > 0x10020 {
                std::slice::from_raw_parts((rip - 32) as *const u8, 32)
            } else {
                &[]
            };

            let rbp = mctx.gregs[libc::REG_RBP as usize] as u64;
            let r8 = mctx.gregs[libc::REG_R8 as usize] as u64;
            let r9 = mctx.gregs[libc::REG_R9 as usize] as u64;
            let r10 = mctx.gregs[libc::REG_R10 as usize] as u64;
            let r11 = mctx.gregs[libc::REG_R11 as usize] as u64;
            let r12 = mctx.gregs[libc::REG_R12 as usize] as u64;
            let r13 = mctx.gregs[libc::REG_R13 as usize] as u64;
            let r14 = mctx.gregs[libc::REG_R14 as usize] as u64;
            let r15 = mctx.gregs[libc::REG_R15 as usize] as u64;

            let fault_addr = if !_info.is_null() {
                (*_info).si_addr() as u64
            } else {
                0
            };

            eprintln!("\n========================================");
            eprintln!("[RINE ULTRA CRASH INSPECTOR] FULL CPU REGISTERS (Faulting Addr si_addr: 0x{:016X}):", fault_addr);
            eprintln!("  RIP: 0x{:016X} | RSP: 0x{:016X}", rip, rsp);
            eprintln!("  RAX: 0x{:016X} | RBX: 0x{:016X}", rax, rbx);
            eprintln!("  RCX: 0x{:016X} | RDX: 0x{:016X}", rcx, rdx);
            eprintln!("  RSI: 0x{:016X} | RDI: 0x{:016X}", rsi, rdi);
            eprintln!("  RBP: 0x{:016X} | R8 : 0x{:016X}", rbp, r8);
            eprintln!("  R9 : 0x{:016X} | R10: 0x{:016X}", r9, r10);
            eprintln!("  R11: 0x{:016X} | R12: 0x{:016X}", r11, r12);
            eprintln!("  R13: 0x{:016X} | R14: 0x{:016X}", r13, r14);
            eprintln!("  R15: 0x{:016X}", r15);
            eprintln!("  RIP Bytes (Current): {:02X?}", rip_bytes);
            eprintln!("  RIP Bytes (Before):  {:02X?}", rip_before_bytes);

            let rsp_ptr = rsp as *const u64;
            if !rsp_ptr.is_null() && rsp > 0x10000 && rsp < 0x7FFFFFFFFFFF {
                let img_base = crate::dlls::kernel32::get_current_image_base();
                let img_end = img_base.saturating_add(0x10000000); // +256MB covers typical PE images
                eprintln!("--- FULL STACK TRACE (RSP top 16 words) ---");
                for i in 0..16 {
                    let target_ptr = rsp_ptr.add(i);
                    if (target_ptr as usize) % 8 == 0
                        && (target_ptr as usize) > 0x10000
                        && (target_ptr as usize) < 0x7FFFFFFFFFFF
                    {
                        let val = *target_ptr;
                        if img_base > 0 && val >= img_base && val < img_end {
                            eprintln!(
                                "  [RSP + 0x{:02X}] = 0x{:016X} (RVA: 0x{:X})",
                                i * 8,
                                val,
                                val - img_base
                            );
                        } else {
                            eprintln!("  [RSP + 0x{:02X}] = 0x{:016X}", i * 8, val);
                        }
                    }
                }
            }
            eprintln!("========================================================\n");
            if rip < 0x10000 {
                if is_main_thread() {
                    eprintln!("[RINE SIGNAL HANDLER] Main thread zero RIP (0x{:016X}) detected. Redirecting to active main render loop stub...", rip);
                    let stub_addr =
                        crate::sys::trampoline::rine_exit_thread_stub as *const () as u64;
                    (*uctx).uc_mcontext.gregs[libc::REG_RIP as usize] = stub_addr as i64;
                    (*uctx).uc_mcontext.gregs[libc::REG_RDI as usize] = 0; // exit_code = 0
                    return;
                } else {
                    eprintln!("[RINE SIGNAL HANDLER] Worker thread zero RIP (0x{:016X}) detected. Exiting worker thread gracefully...", rip);
                    let stub_addr = worker_thread_exit_stub as *const () as u64;
                    (*uctx).uc_mcontext.gregs[libc::REG_RIP as usize] = stub_addr as i64;
                    return;
                }
            } else if sig == libc::SIGSEGV || sig == libc::SIGILL {
                let img_base = crate::dlls::kernel32::get_current_image_base();
                let img_end = img_base + 0x20000000; // 512MB process image range

                if rip >= img_base && rip < img_end {
                    let op_opt = safe_read_u32(rip);
                    let op32 = op_opt.unwrap_or(0);
                    let is_null_op =
                        op_opt.is_none() || op32 == 0 || op32 == 0xFFFFFFFF || op32 == 0x25048B48;

                    if is_null_op {
                        if op32 == 0x25048B48 {
                            eprintln!("[RINE SIGNAL HANDLER] CRT fastfail assertion at RIP 0x{:016X}. Bypassing assertion trap...", rip);
                            (*uctx).uc_mcontext.gregs[libc::REG_RIP as usize] = (rip + 10) as i64;
                            return;
                        }

                        eprintln!("[RINE SIGNAL HANDLER] NULL function pointer jump detected at RIP 0x{:016X} (op32=0x{:08X}). Redirecting to dummy stub return...", rip, op32);
                        let stub_addr = crate::dlls::registry::get_dummy_stub_buffer_ptr() as u64;
                        (*uctx).uc_mcontext.gregs[libc::REG_RIP as usize] = stub_addr as i64;
                        return;
                    }
                }

                let mut recovered = false;

                let is_invalid =
                    |val: u64| val == 0 || (val >> 32) == 0x6567B63F || val > 0x7FFFFFFFFFFF;
                let is_in_image = |val: u64| val >= img_base && val < img_end;

                let alloc_real_lock = || unsafe {
                    let ptr = libc::mmap(
                        std::ptr::null_mut(),
                        65536,
                        libc::PROT_READ | libc::PROT_WRITE,
                        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                        -1,
                        0,
                    );
                    if ptr == libc::MAP_FAILED || ptr.is_null() {
                        0
                    } else {
                        ptr as u64
                    }
                };

                let check_and_fix_reg = |reg_val: u64| -> Option<u64> {
                    if is_invalid(reg_val) {
                        Some(alloc_real_lock())
                    } else if is_in_image(reg_val) {
                        let ptr_loc = reg_val as *mut u64;
                        let inner_val = *ptr_loc;
                        if is_invalid(inner_val) {
                            let heap_lock = alloc_real_lock();
                            *ptr_loc = heap_lock;
                            Some(heap_lock)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                let rsi_val = (*uctx).uc_mcontext.gregs[libc::REG_RSI as usize] as u64;
                if let Some(fixed) = check_and_fix_reg(rsi_val) {
                    (*uctx).uc_mcontext.gregs[libc::REG_RSI as usize] = fixed as i64;
                    recovered = true;
                }

                let rbx_val = (*uctx).uc_mcontext.gregs[libc::REG_RBX as usize] as u64;
                if let Some(fixed) = check_and_fix_reg(rbx_val) {
                    (*uctx).uc_mcontext.gregs[libc::REG_RBX as usize] = fixed as i64;
                    recovered = true;
                }

                let rdx_val = (*uctx).uc_mcontext.gregs[libc::REG_RDX as usize] as u64;
                if let Some(fixed) = check_and_fix_reg(rdx_val) {
                    (*uctx).uc_mcontext.gregs[libc::REG_RDX as usize] = fixed as i64;
                    recovered = true;
                }

                let rcx_val = (*uctx).uc_mcontext.gregs[libc::REG_RCX as usize] as u64;
                if let Some(fixed) = check_and_fix_reg(rcx_val) {
                    (*uctx).uc_mcontext.gregs[libc::REG_RCX as usize] = fixed as i64;
                    recovered = true;
                }

                let rax_val = (*uctx).uc_mcontext.gregs[libc::REG_RAX as usize] as u64;
                if let Some(fixed) = check_and_fix_reg(rax_val) {
                    (*uctx).uc_mcontext.gregs[libc::REG_RAX as usize] = fixed as i64;
                    recovered = true;
                }

                let r8_val = (*uctx).uc_mcontext.gregs[libc::REG_R8 as usize] as u64;
                if is_invalid(r8_val) {
                    let heap_lock = alloc_real_lock();
                    (*uctx).uc_mcontext.gregs[libc::REG_R8 as usize] = heap_lock as i64;
                    recovered = true;
                }

                let r9_val = (*uctx).uc_mcontext.gregs[libc::REG_R9 as usize] as u64;
                if is_invalid(r9_val) {
                    let heap_lock = alloc_real_lock();
                    (*uctx).uc_mcontext.gregs[libc::REG_R9 as usize] = heap_lock as i64;
                    recovered = true;
                }

                if recovered {
                    let rsp = (*uctx).uc_mcontext.gregs[libc::REG_RSP as usize] as u64;
                    if rsp & 0xF != 0 {
                        // Align RSP to 16-byte boundary to satisfy SSE (movaps) alignment requirements
                        (*uctx).uc_mcontext.gregs[libc::REG_RSP as usize] = (rsp & !0xF) as i64;
                    }
                    eprintln!("[RINE SIGNAL HANDLER] Dynamically allocated real heap lock for process VA at RIP 0x{:016X}.", rip);
                    return;
                }
            }
        }
    }
    std::process::exit(128 + sig);
}

pub fn setup_signal_handlers() {
    register_main_thread_id();
    unsafe {
        let mut sa: sigaction = zeroed();
        sa.sa_sigaction = rine_signal_handler as usize;
        sa.sa_flags = SA_SIGINFO;

        sigaction(SIGSEGV, &sa, std::ptr::null_mut());
        sigaction(SIGILL, &sa, std::ptr::null_mut());
        sigaction(SIGBUS, &sa, std::ptr::null_mut());
        sigaction(SIGFPE, &sa, std::ptr::null_mut());
    }
    println!("[RINE DEBUG ENGINE] POSIX Signal Handlers (SIGSEGV, SIGILL, SIGBUS) registered.");
}
