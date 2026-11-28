use libc::{c_int, c_void, sigaction, siginfo_t, SA_SIGINFO, SIGBUS, SIGFPE, SIGILL, SIGSEGV, SIGTRAP, SIGUSR2};
use std::mem::zeroed;

use std::sync::atomic::{AtomicU64, Ordering};

static MAIN_THREAD_ID: AtomicU64 = AtomicU64::new(0);

pub unsafe fn describe_address(addr: u64) -> String {
    if addr < 0x1000 {
        return "NULL/small".to_string();
    }
    let mut dlinfo: libc::Dl_info = std::mem::zeroed();
    if libc::dladdr(addr as *const libc::c_void, &mut dlinfo) != 0 {
        let fname = if !dlinfo.dli_fname.is_null() {
            std::ffi::CStr::from_ptr(dlinfo.dli_fname).to_string_lossy()
        } else {
            "?".into()
        };
        let sname = if !dlinfo.dli_sname.is_null() {
            std::ffi::CStr::from_ptr(dlinfo.dli_sname).to_string_lossy()
        } else {
            "?".into()
        };
        return format!("ELF: {}!{} (base=0x{:016X})", fname, sname, dlinfo.dli_fbase as usize);
    }
    let (mod_base, rva, mod_name) = resolve_rip_module(addr);
    if mod_base != 0 {
        return format!("PE: {} (base=0x{:016X}, rva=0x{:X})", mod_name, mod_base, rva);
    }
    "UNKNOWN".to_string()
}

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

pub unsafe fn dispatch_vectored_exception_handlers(
    exception_code: u32,
    fault_addr: u64,
    uctx: *mut libc::ucontext_t,
) -> bool {
    let handlers = crate::dlls::kernel32::get_vectored_exception_handlers();
    if handlers.is_empty() {
        return false;
    }

    let mctx = &mut (*uctx).uc_mcontext;
    let rip = mctx.gregs[libc::REG_RIP as usize] as u64;

    let mut context_buffer = [0u8; 1232];
    let ctx = context_buffer.as_mut_ptr();

    // Fill CONTEXT from mcontext
    *((ctx.add(0x30)) as *mut u32) = 0x10007F; // CONTEXT_FULL | CONTEXT_ALL
    *((ctx.add(0x44)) as *mut u32) = mctx.gregs[libc::REG_EFL as usize] as u32;
    *((ctx.add(0x78)) as *mut u64) = mctx.gregs[libc::REG_RAX as usize] as u64;
    *((ctx.add(0x80)) as *mut u64) = mctx.gregs[libc::REG_RCX as usize] as u64;
    *((ctx.add(0x88)) as *mut u64) = mctx.gregs[libc::REG_RDX as usize] as u64;
    *((ctx.add(0x90)) as *mut u64) = mctx.gregs[libc::REG_RBX as usize] as u64;
    *((ctx.add(0x98)) as *mut u64) = mctx.gregs[libc::REG_RSP as usize] as u64;
    *((ctx.add(0xA0)) as *mut u64) = mctx.gregs[libc::REG_RBP as usize] as u64;
    *((ctx.add(0xA8)) as *mut u64) = mctx.gregs[libc::REG_RSI as usize] as u64;
    *((ctx.add(0xB0)) as *mut u64) = mctx.gregs[libc::REG_RDI as usize] as u64;
    *((ctx.add(0xB8)) as *mut u64) = mctx.gregs[libc::REG_R8 as usize] as u64;
    *((ctx.add(0xC0)) as *mut u64) = mctx.gregs[libc::REG_R9 as usize] as u64;
    *((ctx.add(0xC8)) as *mut u64) = mctx.gregs[libc::REG_R10 as usize] as u64;
    *((ctx.add(0xD0)) as *mut u64) = mctx.gregs[libc::REG_R11 as usize] as u64;
    *((ctx.add(0xD8)) as *mut u64) = mctx.gregs[libc::REG_R12 as usize] as u64;
    *((ctx.add(0xE0)) as *mut u64) = mctx.gregs[libc::REG_R13 as usize] as u64;
    *((ctx.add(0xE8)) as *mut u64) = mctx.gregs[libc::REG_R14 as usize] as u64;
    *((ctx.add(0xF0)) as *mut u64) = mctx.gregs[libc::REG_R15 as usize] as u64;
    *((ctx.add(0xF8)) as *mut u64) = rip;

    let is_exec_fault = fault_addr == rip;
    let mut rec = [0u64; 20]; // ExceptionRecord (at least 152 bytes)
    let rec_ptr = rec.as_mut_ptr();
    *((rec_ptr as *mut u32).add(0)) = exception_code; // ExceptionCode (0xC0000005)
    *((rec_ptr as *mut u32).add(1)) = 0; // ExceptionFlags
    *((rec_ptr.add(1)) as *mut u64) = 0; // ExceptionRecord
    *((rec_ptr.add(2)) as *mut u64) = rip; // ExceptionAddress
    *((rec_ptr as *mut u32).add(6)) = 2; // NumberParameters
    // ExceptionInformation[0]: 0 = read, 1 = write, 8 = execute
    *((rec_ptr.add(4)) as *mut u64) = if is_exec_fault { 8 } else { 0 };
    // ExceptionInformation[1]: faulting address
    *((rec_ptr.add(5)) as *mut u64) = fault_addr;

    let mut ep = [rec_ptr as u64, ctx as u64];
    let ep_ptr = ep.as_mut_ptr() as *mut libc::c_void;

    for handler_addr in handlers {
        let handler_fn: unsafe extern "win64" fn(*mut libc::c_void) -> i32 =
            std::mem::transmute(handler_addr);
        let res = handler_fn(ep_ptr);
        if res == -1 /* EXCEPTION_CONTINUE_EXECUTION */ {
            eprintln!(
                "[RINE VEH] Handler 0x{:X} returned EXCEPTION_CONTINUE_EXECUTION. Resuming guest.",
                handler_addr
            );
            // Copy updated registers back to mcontext
            mctx.gregs[libc::REG_RAX as usize] = *((ctx.add(0x78)) as *const u64) as i64;
            mctx.gregs[libc::REG_RCX as usize] = *((ctx.add(0x80)) as *const u64) as i64;
            mctx.gregs[libc::REG_RDX as usize] = *((ctx.add(0x88)) as *const u64) as i64;
            mctx.gregs[libc::REG_RBX as usize] = *((ctx.add(0x90)) as *const u64) as i64;
            mctx.gregs[libc::REG_RSP as usize] = *((ctx.add(0x98)) as *const u64) as i64;
            mctx.gregs[libc::REG_RBP as usize] = *((ctx.add(0xA0)) as *const u64) as i64;
            mctx.gregs[libc::REG_RSI as usize] = *((ctx.add(0xA8)) as *const u64) as i64;
            mctx.gregs[libc::REG_RDI as usize] = *((ctx.add(0xB0)) as *const u64) as i64;
            mctx.gregs[libc::REG_R8 as usize] = *((ctx.add(0xB8)) as *const u64) as i64;
            mctx.gregs[libc::REG_R9 as usize] = *((ctx.add(0xC0)) as *const u64) as i64;
            mctx.gregs[libc::REG_R10 as usize] = *((ctx.add(0xC8)) as *const u64) as i64;
            mctx.gregs[libc::REG_R11 as usize] = *((ctx.add(0xD0)) as *const u64) as i64;
            mctx.gregs[libc::REG_R12 as usize] = *((ctx.add(0xD8)) as *const u64) as i64;
            mctx.gregs[libc::REG_R13 as usize] = *((ctx.add(0xE0)) as *const u64) as i64;
            mctx.gregs[libc::REG_R14 as usize] = *((ctx.add(0xE8)) as *const u64) as i64;
            mctx.gregs[libc::REG_R15 as usize] = *((ctx.add(0xF0)) as *const u64) as i64;
            mctx.gregs[libc::REG_RIP as usize] = *((ctx.add(0xF8)) as *const u64) as i64;
            return true;
        }
    }

    false
}

pub fn resolve_rip_module(rip: u64) -> (u64, u64, String) {
    if let Ok(guard) = crate::pe::imports::LOADED_MODULES.lock() {
        if let Some(modules) = guard.as_ref() {
            for (name, base, size) in modules {
                let b = *base as u64;
                let s = *size as u64;
                if rip >= b && rip < b + s {
                    return (b, rip - b, name.clone());
                }
            }
        }
    }
    if rip >= 0x0000000180000000 && rip < 0x0000000195000000 {
        return (0x0000000180000000, rip - 0x0000000180000000, "UnityPlayer.dll".to_string());
    }
    if rip >= 0x0000000140000000 && rip < 0x0000000145000000 {
        return (0x0000000140000000, rip - 0x0000000140000000, "main_exe".to_string());
    }
    if rip >= 0x0000000359050000 && rip < 0x0000000360000000 {
        return (0x0000000359050000, rip - 0x0000000359050000, "d3d11.dll".to_string());
    }
    if rip >= 0x00000002EDF00000 && rip < 0x00000002F0000000 {
        return (0x00000002EDF00000, rip - 0x00000002EDF00000, "dxgi.dll".to_string());
    }
    let main_base = crate::dlls::kernel32::get_current_image_base();
    if main_base > 0 && rip >= main_base && rip < main_base + 0x10000000 {
        return (main_base, rip - main_base, "main_exe".to_string());
    }
    (0, 0, "unknown_module".to_string())
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

            let fault_addr = if !info.is_null() {
                (*info).si_addr() as u64
            } else {
                0
            };

            let rip_readable = rip > 0x10000 && rip < 0x7FFFFFFFFFFF && fault_addr != rip;
            let rip_bytes = if rip_readable {
                std::slice::from_raw_parts(rip as *const u8, 16)
            } else {
                &[]
            };

            let rip_before_bytes = if rip_readable && rip > 0x10020 {
                std::slice::from_raw_parts((rip - 32) as *const u8, 32)
            } else {
                &[]
            };

            if sig == libc::SIGTRAP {
                eprintln!(
                    "[RINE SIGNAL HANDLER] CAUGHT SIGTRAP (int3 breakpoint) at RIP 0x{:016X}! Resuming execution.",
                    rip
                );
                if !rip_bytes.is_empty() && rip_bytes[0] == 0xCC {
                    (*uctx).uc_mcontext.gregs[libc::REG_RIP as usize] += 1;
                }
                return;
            }

            let role = if is_main_thread() { "main" } else { "worker" };
            let thread_id = libc::syscall(libc::SYS_gettid) as u64;

            let rbp = mctx.gregs[libc::REG_RBP as usize] as u64;
            let r8 = mctx.gregs[libc::REG_R8 as usize] as u64;
            let r9 = mctx.gregs[libc::REG_R9 as usize] as u64;
            let r10 = mctx.gregs[libc::REG_R10 as usize] as u64;
            let r11 = mctx.gregs[libc::REG_R11 as usize] as u64;
            let r12 = mctx.gregs[libc::REG_R12 as usize] as u64;
            let r13 = mctx.gregs[libc::REG_R13 as usize] as u64;
            let r14 = mctx.gregs[libc::REG_R14 as usize] as u64;
            let r15 = mctx.gregs[libc::REG_R15 as usize] as u64;

            // Unity 6 Null-Queue Event Dispatch Guard
            // In UnityPlayer.dll (6000.4.4f1), function at RVA 0x13CC240 accesses [r15 + 0x58] where r15 == rcx.
            // When invoked with a null queue pointer (e.g. initial tick before queues are allocated), r15 == 0, faulting at 0x58.
            // Divert RIP directly to the function's clean epilogue at RVA 0x13CC6B0, cleanly restoring the stack and returning.
            if sig == libc::SIGSEGV {
                let (mut mod_base, mut rva, mod_name) = resolve_rip_module(rip);
                if mod_base == 0 && rip >= 0x0000000180000000 && rip < 0x0000000195000000 {
                    mod_base = 0x0000000180000000;
                    rva = rip - mod_base;
                }
                let is_unity = mod_name.to_lowercase().contains("unityplayer")
                    || (rip >= 0x0000000180000000 && rip < 0x0000000195000000);
                if is_unity
                    && (rva == 0x13CC2A5 || (rva >= 0x13CC240 && rva < 0x13CC6B0 && fault_addr == 0x58 && r15 == 0))
                {
                    let base = if mod_base != 0 { mod_base } else { 0x0000000180000000 };
                    let epilogue = base + 0x13CC6B0;
                    eprintln!(
                        "[RINE SIGNAL RECOVERY] Intercepted Unity NULL-queue input dispatch at RIP 0x{:X} (RVA 0x{:X}). Diverting to epilogue 0x{:X}.",
                        rip, rva, epilogue
                    );
                    (*uctx).uc_mcontext.gregs[libc::REG_RIP as usize] = epilogue as i64;
                    (*uctx).uc_mcontext.gregs[libc::REG_RAX as usize] = 0;
                    return;
                }
            }

            if sig == libc::SIGSEGV || sig == libc::SIGBUS {
                let code = 0xC0000005u32; // EXCEPTION_ACCESS_VIOLATION
                if dispatch_vectored_exception_handlers(code, fault_addr, uctx) {
                    return;
                }
            }

            eprintln!("\n========================================");
            eprintln!("[RINE ULTRA CRASH INSPECTOR] FULL CPU REGISTERS (Faulting Addr si_addr: 0x{:016X}):", fault_addr);
            eprintln!("  RIP: 0x{:016X} ({})", rip, describe_address(rip));
            eprintln!("  RSP: 0x{:016X} ({})", rsp, describe_address(rsp));
            eprintln!("  RAX: 0x{:016X} ({})", rax, describe_address(rax));
            eprintln!("  RBX: 0x{:016X} ({})", rbx, describe_address(rbx));
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
                        let desc = describe_address(val);
                        if img_base > 0 && val >= img_base && val < img_end {
                            eprintln!(
                                "  [RSP + 0x{:02X}] = 0x{:016X} (RVA: 0x{:X}) [{}]",
                                i * 8,
                                val,
                                val - img_base,
                                desc
                            );
                        } else {
                            eprintln!("  [RSP + 0x{:02X}] = 0x{:016X} [{}]", i * 8, val, desc);
                        }
                        if i == 0 && val > 0x10020 && val < 0x7FFFFFFFFFFF && safe_read_u32(val - 16).is_some() {
                            let caller_bytes = std::slice::from_raw_parts((val - 16) as *const u8, 32);
                            eprintln!("    Caller instruction window [-16..+16]: {:02X?}", caller_bytes);
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
            if role == "worker" {
                eprintln!(
                    "[RINE WORKER CRASH HANDLED] Exiting faulting worker thread {} instead of aborting main process",
                    thread_id
                );
                libc::syscall(libc::SYS_exit, 0);
            }
            let code = if crate::sys::shutdown::is_shutdown_requested() {
                0
            } else {
                128 + sig
            };
            libc::_exit(code);
        }
    }
    let code = if crate::sys::shutdown::is_shutdown_requested() {
        0
    } else {
        128 + sig
    };
    unsafe {
        libc::_exit(code);
    }
}

extern "C" fn rine_termination_signal_handler(sig: c_int) {
    let _ = sig;
    unsafe {
        libc::_exit(0);
    }
}

extern "C" fn rine_suspend_signal_handler(
    _sig: c_int,
    _info: *mut siginfo_t,
    ucontext: *mut c_void,
) {
    if !ucontext.is_null() {
        unsafe {
            let uctx = ucontext as *mut libc::ucontext_t;
            let mctx = &(*uctx).uc_mcontext;
            let rip = mctx.gregs[libc::REG_RIP as usize] as u64;
            let rsp = mctx.gregs[libc::REG_RSP as usize] as u64;
            let rax = mctx.gregs[libc::REG_RAX as usize] as u64;
            let rbx = mctx.gregs[libc::REG_RBX as usize] as u64;
            let rcx = mctx.gregs[libc::REG_RCX as usize] as u64;
            let rdx = mctx.gregs[libc::REG_RDX as usize] as u64;
            let rsi = mctx.gregs[libc::REG_RSI as usize] as u64;
            let rdi = mctx.gregs[libc::REG_RDI as usize] as u64;
            let rbp = mctx.gregs[libc::REG_RBP as usize] as u64;
            let r8 = mctx.gregs[libc::REG_R8 as usize] as u64;
            let r9 = mctx.gregs[libc::REG_R9 as usize] as u64;
            let r10 = mctx.gregs[libc::REG_R10 as usize] as u64;
            let r11 = mctx.gregs[libc::REG_R11 as usize] as u64;
            let r12 = mctx.gregs[libc::REG_R12 as usize] as u64;
            let r13 = mctx.gregs[libc::REG_R13 as usize] as u64;
            let r14 = mctx.gregs[libc::REG_R14 as usize] as u64;
            let r15 = mctx.gregs[libc::REG_R15 as usize] as u64;

            if let Some(regs) = crate::dlls::kernel32::on_thread_suspended_signal(
                rsp, rbp, rip, rax, rbx, rcx, rdx, rsi, rdi, r8, r9, r10, r11, r12, r13, r14, r15,
            ) {
                let g = &mut (*uctx).uc_mcontext.gregs;
                g[libc::REG_RSP as usize] = regs[0] as libc::greg_t;
                g[libc::REG_RBP as usize] = regs[1] as libc::greg_t;
                g[libc::REG_RIP as usize] = regs[2] as libc::greg_t;
                g[libc::REG_RAX as usize] = regs[3] as libc::greg_t;
                g[libc::REG_RBX as usize] = regs[4] as libc::greg_t;
                g[libc::REG_RCX as usize] = regs[5] as libc::greg_t;
                g[libc::REG_RDX as usize] = regs[6] as libc::greg_t;
                g[libc::REG_RSI as usize] = regs[7] as libc::greg_t;
                g[libc::REG_RDI as usize] = regs[8] as libc::greg_t;
                g[libc::REG_R8 as usize] = regs[9] as libc::greg_t;
                g[libc::REG_R9 as usize] = regs[10] as libc::greg_t;
                g[libc::REG_R10 as usize] = regs[11] as libc::greg_t;
                g[libc::REG_R11 as usize] = regs[12] as libc::greg_t;
                g[libc::REG_R12 as usize] = regs[13] as libc::greg_t;
                g[libc::REG_R13 as usize] = regs[14] as libc::greg_t;
                g[libc::REG_R14 as usize] = regs[15] as libc::greg_t;
                g[libc::REG_R15 as usize] = regs[16] as libc::greg_t;
            }
        }
    }
}

extern "C" fn rine_diag_signal_handler(
    _sig: libc::c_int,
    _info: *mut siginfo_t,
    _ucontext: *mut c_void,
) {
    eprintln!("{}", crate::sys::diagnostics::generate_diagnostics_report());
}

pub fn setup_signal_handlers() {
    register_main_thread_id();
    unsafe {
        let mut sa: sigaction = zeroed();
        sa.sa_sigaction = rine_signal_handler as *const () as usize;
        sa.sa_flags = SA_SIGINFO;

        sigaction(SIGSEGV, &sa, std::ptr::null_mut());
        sigaction(SIGILL, &sa, std::ptr::null_mut());
        sigaction(SIGBUS, &sa, std::ptr::null_mut());
        sigaction(SIGFPE, &sa, std::ptr::null_mut());
        sigaction(SIGTRAP, &sa, std::ptr::null_mut());

        let mut susp_sa: sigaction = zeroed();
        susp_sa.sa_sigaction = rine_suspend_signal_handler as *const () as usize;
        susp_sa.sa_flags = SA_SIGINFO | libc::SA_RESTART;
        sigaction(libc::SIGUSR2, &susp_sa, std::ptr::null_mut());

        let mut diag_sa: sigaction = zeroed();
        diag_sa.sa_sigaction = rine_diag_signal_handler as *const () as usize;
        diag_sa.sa_flags = SA_SIGINFO;
        sigaction(libc::SIGUSR1, &diag_sa, std::ptr::null_mut());

        let mut term_sa: sigaction = zeroed();
        term_sa.sa_sigaction = rine_termination_signal_handler as *const () as usize;
        term_sa.sa_flags = 0;
        sigaction(libc::SIGINT, &term_sa, std::ptr::null_mut());
        sigaction(libc::SIGTERM, &term_sa, std::ptr::null_mut());
    }
    println!("[RINE DEBUG ENGINE] POSIX Signal Handlers (SIGSEGV, SIGILL, SIGBUS, SIGFPE, SIGTRAP, SIGUSR1, SIGUSR2, SIGINT, SIGTERM) registered.");
}
