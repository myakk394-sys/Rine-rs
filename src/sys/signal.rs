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

pub fn resolve_rip_module(rip: u64) -> (u64, u64, &'static str) {
    if rip >= 0x0000000140000000 && rip < 0x0000000145000000 {
        return (0x0000000140000000, rip - 0x0000000140000000, "main_exe");
    }
    if rip >= 0x0000000359050000 && rip < 0x0000000360000000 {
        return (0x0000000359050000, rip - 0x0000000359050000, "d3d11.dll");
    }
    if rip >= 0x00000002EDF00000 && rip < 0x00000002F0000000 {
        return (0x00000002EDF00000, rip - 0x00000002EDF00000, "dxgi.dll");
    }
    let main_base = crate::dlls::kernel32::get_current_image_base();
    if main_base > 0 && rip >= main_base && rip < main_base + 0x10000000 {
        return (main_base, rip - main_base, "main_exe");
    }
    (0, 0, "unknown_module")
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

extern "C" fn rine_signal_handler(sig: c_int, info: *mut siginfo_t, ucontext: *mut c_void) {
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

            let fault_addr = if !info.is_null() {
                (*info).si_addr() as u64
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
            let fault_addr = if !info.is_null() {
                (*info).si_addr() as u64
            } else {
                0
            };

            let (mod_base, rva, mod_name) = resolve_rip_module(rip);
            let role = if is_main_thread() { "main" } else { "worker" };
            let thread_id = libc::syscall(libc::SYS_gettid) as u64;

            eprintln!("========================================================");
            eprintln!("[RINE GUEST CRASH]");
            eprintln!("  signal={}", sig);
            eprintln!("  thread_role={}", role);
            eprintln!("  guest_thread_id={}", thread_id);
            eprintln!("  rip=0x{:016X}", rip);
            eprintln!("  rva=0x{:08X}", rva);
            eprintln!("  fault_address=0x{:016X}", fault_addr);
            eprintln!("  guest_module={} (base=0x{:016X})", mod_name, mod_base);
            eprintln!("  rax=0x{:016X}", rax);
            eprintln!("  rbx=0x{:016X}", rbx);
            eprintln!("  rcx=0x{:016X}", rcx);
            eprintln!("  rdx=0x{:016X}", rdx);
            eprintln!("  rsi=0x{:016X}", rsi);
            eprintln!("  rdi=0x{:016X}", rdi);
            eprintln!("  rsp=0x{:016X}", rsp);
            eprintln!("  rbp=0x{:016X}", rbp);
            eprintln!("  r8 =0x{:016X}", r8);
            eprintln!("  r9 =0x{:016X}", r9);
            eprintln!("  r10=0x{:016X}", r10);
            eprintln!("  r11=0x{:016X}", r11);
            eprintln!("  r12=0x{:016X}", r12);
            eprintln!("  r13=0x{:016X}", r13);
            eprintln!("  r14=0x{:016X}", r14);
            eprintln!("  r15=0x{:016X}", r15);

            if let Some(obj) = crate::dlls::object_registry::lookup_rine_object(rsi) {
                eprintln!(
                    "[CRASH OBJECT MATCH (RSI)] id={} creator={} vtable=0x{:016X} interface={}",
                    obj.object_id, obj.creator_symbol, obj.vtable_address, obj.interface_name
                );
            }
            if let Some(obj) = crate::dlls::object_registry::lookup_rine_object(rax) {
                eprintln!(
                    "[CRASH OBJECT MATCH (RAX)] id={} creator={} vtable=0x{:016X} interface={}",
                    obj.object_id, obj.creator_symbol, obj.vtable_address, obj.interface_name
                );
            }

            if rsp > 0x10000 && rsp < 0x7FFFFFFFFFFF {
                let rsp_ptr = rsp as *const u64;
                eprintln!("--- STACK TRACE (RSP top 16 words) ---");
                for i in 0..16 {
                    let target_ptr = rsp_ptr.add(i);
                    if (target_ptr as usize) % 8 == 0
                        && (target_ptr as usize) > 0x10000
                        && (target_ptr as usize) < 0x7FFFFFFFFFFF
                    {
                        let val = *target_ptr;
                        let (val_base, val_rva, _) = resolve_rip_module(val);
                        if val_base > 0 {
                            eprintln!(
                                "  [RSP + 0x{:02X}] = 0x{:016X} (RVA: 0x{:08X})",
                                i * 8,
                                val,
                                val_rva
                            );
                        } else {
                            eprintln!("  [RSP + 0x{:02X}] = 0x{:016X}", i * 8, val);
                        }
                    }
                }
            }
            eprintln!("========================================================\n");
            std::process::exit(128 + sig);
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
