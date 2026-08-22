//! Universal Wine DLL Export Registry generated from Wine .spec files
//! Maps all 550+ Windows DLLs and exported functions into Rine Rust execution stubs.

#![allow(non_snake_case)]
use std::collections::HashMap;
use std::sync::OnceLock;

pub type UniversalStubFn = unsafe extern "win64" fn() -> u64;

pub fn get_dummy_stub_buffer_ptr() -> usize {
    static SAFE_BUF: OnceLock<usize> = OnceLock::new();
    *SAFE_BUF.get_or_init(|| {
        let region = crate::mem::mmap::MmapRegion::allocate_anonymous(
            None,
            0x10000,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
        )
        .unwrap();
        let ptr = region.as_ptr() as usize;
        unsafe {
            // Fill memory buffer with x86_64 ret instructions (0xC3) so any execution returns cleanly
            std::ptr::write_bytes(ptr as *mut u8, 0xC3, 0x10000);
        }
        std::mem::forget(region);
        ptr
    })
}

pub fn get_dummy_data_buffer_ptr() -> usize {
    static DATA_BUF: OnceLock<usize> = OnceLock::new();
    *DATA_BUF.get_or_init(|| {
        let region = crate::mem::mmap::MmapRegion::allocate_anonymous(
            None,
            0x10000,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
        )
        .unwrap();
        let ptr = region.as_ptr() as usize;
        unsafe {
            // Fill memory buffer completely with 0x00 zero bytes so any pointer/flag checks evaluate to NULL (0)
            std::ptr::write_bytes(ptr as *mut u8, 0x00, 0x10000);
        }
        std::mem::forget(region);
        ptr
    })
}

#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "win64" fn universal_stub_thunk() {
    std::arch::naked_asm!(
        "push rbx",
        "push rbp",
        "push rsi",
        "push rdi",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "sub rsp, 0xA0",
        "movdqu [rsp + 0x00], xmm6",
        "movdqu [rsp + 0x10], xmm7",
        "movdqu [rsp + 0x20], xmm8",
        "movdqu [rsp + 0x30], xmm9",
        "movdqu [rsp + 0x40], xmm10",
        "movdqu [rsp + 0x50], xmm11",
        "movdqu [rsp + 0x60], xmm12",
        "movdqu [rsp + 0x70], xmm13",
        "movdqu [rsp + 0x80], xmm14",
        "movdqu [rsp + 0x90], xmm15",
        "sub rsp, 0x28",
        "call {inner_stub}",
        "add rsp, 0x28",
        "movdqu xmm6, [rsp + 0x00]",
        "movdqu xmm7, [rsp + 0x10]",
        "movdqu xmm8, [rsp + 0x20]",
        "movdqu xmm9, [rsp + 0x30]",
        "movdqu xmm10, [rsp + 0x40]",
        "movdqu xmm11, [rsp + 0x50]",
        "movdqu xmm12, [rsp + 0x60]",
        "movdqu xmm13, [rsp + 0x70]",
        "movdqu xmm14, [rsp + 0x80]",
        "movdqu xmm15, [rsp + 0x90]",
        "add rsp, 0xA0",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rdi",
        "pop rsi",
        "pop rbp",
        "pop rbx",
        "ret",
        inner_stub = sym universal_stub_function,
    );
}

#[no_mangle]
pub unsafe extern "win64" fn universal_stub_function(
    rcx: usize,
    rdx: usize,
    r8: usize,
    r9: usize,
) -> usize {
    let r10_val: u64;
    let ret_addr: u64;
    let rsp_val: u64;
    std::arch::asm!(
        "mov {}, r10",
        "mov {}, r11",
        "mov {}, rsp",
        out(reg) r10_val,
        out(reg) ret_addr,
        out(reg) rsp_val
    );

    static CALL_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let count = CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let (stub_id_str, dll_str, symbol_str, ordinal_str, importer_str, iat_va_str) =
        if (r10_val & 0xFFFFFFFF00000000) == 0x52494E4500000000 {
            let id = (r10_val & 0xFFFFFFFF) as u32;
            if let Some(meta) = crate::pe::imports::get_stub_metadata(id) {
                (
                    meta.stub_id.to_string(),
                    meta.dll.clone(),
                    meta.symbol.clone(),
                    if meta.symbol.starts_with('#') {
                        meta.symbol.clone()
                    } else {
                        "N/A".to_string()
                    },
                    meta.importer.clone(),
                    format!("0x{:X}", meta.iat_va),
                )
            } else {
                (
                    format!("UnknownId_{}", id),
                    "UnknownDLL".into(),
                    "UnknownSymbol".into(),
                    "N/A".into(),
                    "UnknownImporter".into(),
                    "0x0".into(),
                )
            }
        } else {
            let sym_name = crate::pe::imports::lookup_function_by_ret_addr(ret_addr)
                .unwrap_or_else(|| "UnknownStub (Untagged R10)".to_string());
            (
                "0 (Untagged)".into(),
                "UnknownDLL".into(),
                sym_name,
                "N/A".into(),
                "UnknownImporter".into(),
                "0x0".into(),
            )
        };

    let (caller_module_str, caller_rva_str) = if let Some((mod_name, base, _)) =
        crate::pe::imports::find_module_by_addr(ret_addr as usize)
    {
        (
            mod_name,
            format!("0x{:X}", ret_addr.saturating_sub(base as u64)),
        )
    } else {
        (
            "Native/Non-PE".to_string(),
            format!("0x{:X} (absolute)", ret_addr),
        )
    };

    let call_inst_va = ret_addr.saturating_sub(6);
    let iat_slot_va_val = if iat_va_str.starts_with("0x") {
        u64::from_str_radix(&iat_va_str[2..], 16).unwrap_or(0)
    } else {
        0
    };
    let iat_slot_value =
        if iat_slot_va_val != 0 && crate::mem::mmap::is_readable(iat_slot_va_val, 8) {
            unsafe { *(iat_slot_va_val as *const u64) }
        } else {
            0
        };

    if count < 50 {
        eprintln!(
            "========================================================\n\
             [RINE UNRESOLVED CALL]\n\
             stub_id={}\n\
             dll={}\n\
             symbol={}\n\
             ordinal={}\n\
             importer={}\n\
             call_instruction_va=0x{:X}\n\
             return_va=0x{:X}\n\
             iat_slot_va=0x{:X}\n\
             iat_slot_value=0x{:X}\n\
             resolved_function_va=0x{:X}\n\
             caller_module={}\n\
             caller_rva={}\n\
             rsp=0x{:X}\n\
             rcx=0x{:X}\n\
             rdx=0x{:X}\n\
             r8=0x{:X}\n\
             r9=0x{:X}",
            stub_id_str,
            dll_str,
            symbol_str,
            ordinal_str,
            importer_str,
            call_inst_va,
            ret_addr,
            iat_slot_va_val,
            iat_slot_value,
            iat_slot_value,
            caller_module_str,
            caller_rva_str,
            rsp_val,
            rcx,
            rdx,
            r8,
            r9
        );

        if crate::mem::mmap::is_readable(rsp_val + 8, 64) {
            let arg5 = *((rsp_val + 40) as *const u64);
            let arg6 = *((rsp_val + 48) as *const u64);
            let arg7 = *((rsp_val + 56) as *const u64);
            let arg8 = *((rsp_val + 64) as *const u64);
            eprintln!(
                "  Shadow/Stack Args: [rsp+0x28]=0x{:016X}, [rsp+0x30]=0x{:016X}, [rsp+0x38]=0x{:016X}, [rsp+0x40]=0x{:016X}",
                arg5, arg6, arg7, arg8
            );
        } else {
            eprintln!(
                "  Shadow/Stack Args: <Stack unreadable at 0x{:X}>",
                rsp_val + 8
            );
        }

        if crate::mem::mmap::is_readable(ret_addr.saturating_sub(12), 12) {
            let mut bytes = [0u8; 12];
            std::ptr::copy_nonoverlapping((ret_addr - 12) as *const u8, bytes.as_mut_ptr(), 12);
            eprintln!("  Call Site Bytes (-12..0 before ret_addr): {:02X?}", bytes);

            if bytes[6] == 0xFF && bytes[7] == 0x15 {
                let disp32 = i32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
                let iat_target = (ret_addr as i64 + disp32 as i64) as u64;
                eprintln!(
                    "  Call Type: call [IAT slot] (FF 15 disp32={}) -> Target Slot VA: 0x{:016X}",
                    disp32, iat_target
                );
            } else if bytes[7] == 0xE8 {
                let rel32 = i32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
                let call_target = (ret_addr as i64 + rel32 as i64) as u64;
                eprintln!(
                    "  Call Type: call direct rel32 (E8 rel32={}) -> Target VA: 0x{:016X}",
                    rel32, call_target
                );
            } else if bytes[10] == 0xFF && (bytes[11] & 0xF8 == 0xD0) {
                let reg_num = bytes[11] & 0x07;
                let reg_names = ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"];
                eprintln!(
                    "  Call Type: call register (FF {:02X} = call {})",
                    bytes[11], reg_names[reg_num as usize]
                );
            } else if bytes[9] == 0x41 && bytes[10] == 0xFF && (bytes[11] & 0xF8 == 0xD0) {
                let reg_num = (bytes[11] & 0x07) + 8;
                eprintln!(
                    "  Call Type: call register (41 FF {:02X} = call r{})",
                    bytes[11], reg_num
                );
            } else {
                eprintln!("  Call Type: indirect/complex (see bytes)");
            }
        } else {
            eprintln!(
                "  Call Site Bytes: <Unreadable memory before ret_addr 0x{:X}>",
                ret_addr
            );
        }

        eprintln!("--- /proc/self/maps entries for relevant pointers ---");
        if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
            let addrs_to_check = [
                ("ret_addr", ret_addr as usize),
                ("rcx", rcx),
                ("rdx", rdx),
                ("r8", r8),
                ("r9", r9),
            ];
            for (name, val) in addrs_to_check {
                if val > 0x1000 {
                    for line in maps.lines() {
                        if let Some(dash) = line.find('-') {
                            if let (Ok(start), Some(space)) =
                                (usize::from_str_radix(&line[..dash], 16), line.find(' '))
                            {
                                if let Ok(end) = usize::from_str_radix(&line[dash + 1..space], 16) {
                                    if val >= start && val < end {
                                        eprintln!("    {} (0x{:X}): {}", name, val, line);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        eprintln!("========================================================");
    } else if count == 50 {
        eprintln!("[RINE UNRESOLVED CALL] Suppressing further logs (exceeded 50 calls)...");
    }

    eprintln!(
        "========================================================\n\
         [RINE UNRESOLVED API FATAL]\n\
         dll={}\n\
         symbol={}\n\
         call_instruction_va=0x{:X}\n\
         return_va=0x{:X}\n\
         iat_slot_va=0x{:X}\n\
         iat_slot_value=0x{:X}\n\
         resolved_function_va=0x{:X}\n\
         caller_rva={}\n\
         stub_id={}\n\
         registers: RCX=0x{:X}, RDX=0x{:X}, R8=0x{:X}, R9=0x{:X}, RSP=0x{:X}\n\
         ========================================================\n\
         [GUEST TERMINATED: UNRESOLVED API]",
        dll_str,
        symbol_str,
        call_inst_va,
        ret_addr,
        iat_slot_va_val,
        iat_slot_value,
        iat_slot_value,
        caller_rva_str,
        stub_id_str,
        rcx,
        rdx,
        r8,
        r9,
        rsp_val
    );

    crate::providers::record_unresolved_call(&dll_str, &symbol_str);
    crate::providers::print_provider_stats();

    std::process::exit(127);
}

pub struct UniversalDllRegistry {
    pub known_dlls: HashMap<String, usize>,
}

impl UniversalDllRegistry {
    pub fn new() -> Self {
        let mut known_dlls = HashMap::new();
        known_dlls.insert("acledit.dll".into(), 7);
        known_dlls.insert("aclui.dll".into(), 2);
        known_dlls.insert("activeds.dll".into(), 24);
        known_dlls.insert("actxprxy.dll".into(), 1);
        known_dlls.insert("adsldpc.dll".into(), 168);
        known_dlls.insert("advapi32.dll".into(), 876);
        known_dlls.insert("advpack.dll".into(), 81);
        known_dlls.insert("amsi.dll".into(), 9);
        known_dlls.insert("apphelp.dll".into(), 168);
        known_dlls.insert("appwiz.cpl".into(), 1);
        known_dlls.insert("appxdeploymentclient.dll".into(), 77);
        known_dlls.insert("atl.dll".into(), 48);
        known_dlls.insert("atl100.dll".into(), 52);
        known_dlls.insert("atl110.dll".into(), 52);
        known_dlls.insert("atl80.dll".into(), 53);
        known_dlls.insert("atl90.dll".into(), 52);
        known_dlls.insert("atlthunk.dll".into(), 4);
        known_dlls.insert("atmlib.dll".into(), 76);
        known_dlls.insert("authz.dll".into(), 42);
        known_dlls.insert("avicap32.dll".into(), 4);
        known_dlls.insert("avifil32.dll".into(), 67);
        known_dlls.insert("avrt.dll".into(), 14);
        known_dlls.insert("bcp47langs.dll".into(), 69);
        known_dlls.insert("bcrypt.dll".into(), 62);
        known_dlls.insert("bcryptprimitives.dll".into(), 1);
        known_dlls.insert("bluetoothapis.dll".into(), 96);
        known_dlls.insert("bthprops.cpl".into(), 63);
        known_dlls.insert("cabinet.dll".into(), 19);
        known_dlls.insert("capi2032.dll".into(), 11);
        known_dlls.insert("cards.dll".into(), 5);
        known_dlls.insert("cfgmgr32.dll".into(), 276);
        known_dlls.insert("chakra.dll".into(), 156);
        known_dlls.insert("cldapi.dll".into(), 51);
        known_dlls.insert("clusapi.dll".into(), 115);
        known_dlls.insert("cng.sys".into(), 63);
        known_dlls.insert("combase.dll".into(), 344);
        known_dlls.insert("comctl32.dll".into(), 180);
        known_dlls.insert("comctl32_v6.dll".into(), 190);
        known_dlls.insert("comdlg32.dll".into(), 25);
        known_dlls.insert("coml2.dll".into(), 22);
        known_dlls.insert("compstui.dll".into(), 4);
        known_dlls.insert("comsvcs.dll".into(), 16);
        known_dlls.insert("connect.dll".into(), 13);
        known_dlls.insert("coremessaging.dll".into(), 28);
        known_dlls.insert("credui.dll".into(), 17);
        known_dlls.insert("crtdll.dll".into(), 423);
        known_dlls.insert("crypt32.dll".into(), 246);
        known_dlls.insert("cryptbase.dll".into(), 11);
        known_dlls.insert("cryptdlg.dll".into(), 19);
        known_dlls.insert("cryptdll.dll".into(), 14);
        known_dlls.insert("cryptext.dll".into(), 26);
        known_dlls.insert("cryptnet.dll".into(), 16);
        known_dlls.insert("cryptsp.dll".into(), 65);
        known_dlls.insert("cryptui.dll".into(), 46);
        known_dlls.insert("cryptxml.dll".into(), 19);
        known_dlls.insert("ctapi32.dll".into(), 3);
        known_dlls.insert("ctl3d32.dll".into(), 25);
        known_dlls.insert("d2d1.dll".into(), 12);
        known_dlls.insert("d3d10.dll".into(), 30);
        known_dlls.insert("d3d10_1.dll".into(), 30);
        known_dlls.insert("d3d10core.dll".into(), 2);
        known_dlls.insert("d3d11.dll".into(), 45);
        known_dlls.insert("d3d12.dll".into(), 11);
        known_dlls.insert("d3d12core.dll".into(), 2);
        known_dlls.insert("d3d8.dll".into(), 5);
        known_dlls.insert("d3d8thk.dll".into(), 56);
        known_dlls.insert("d3d9.dll".into(), 15);
        known_dlls.insert("d3dcompiler_33.dll".into(), 10);
        known_dlls.insert("d3dcompiler_34.dll".into(), 10);
        known_dlls.insert("d3dcompiler_35.dll".into(), 10);
        known_dlls.insert("d3dcompiler_36.dll".into(), 10);
        known_dlls.insert("d3dcompiler_37.dll".into(), 10);
        known_dlls.insert("d3dcompiler_38.dll".into(), 11);
        known_dlls.insert("d3dcompiler_39.dll".into(), 11);
        known_dlls.insert("d3dcompiler_40.dll".into(), 12);
        known_dlls.insert("d3dcompiler_41.dll".into(), 13);
        known_dlls.insert("d3dcompiler_42.dll".into(), 13);
        known_dlls.insert("d3dcompiler_43.dll".into(), 17);
        known_dlls.insert("d3dcompiler_46.dll".into(), 25);
        known_dlls.insert("d3dcompiler_47.dll".into(), 29);
        known_dlls.insert("d3dim.dll".into(), 13);
        known_dlls.insert("d3dim700.dll".into(), 18);
        known_dlls.insert("d3drm.dll".into(), 21);
        known_dlls.insert("d3dx10_33.dll".into(), 177);
        known_dlls.insert("d3dx10_34.dll".into(), 177);
        known_dlls.insert("d3dx10_35.dll".into(), 180);
        known_dlls.insert("d3dx10_36.dll".into(), 180);
        known_dlls.insert("d3dx10_37.dll".into(), 181);
        known_dlls.insert("d3dx10_38.dll".into(), 180);
        known_dlls.insert("d3dx10_39.dll".into(), 180);
        known_dlls.insert("d3dx10_40.dll".into(), 176);
        known_dlls.insert("d3dx10_41.dll".into(), 176);
        known_dlls.insert("d3dx10_42.dll".into(), 176);
        known_dlls.insert("d3dx10_43.dll".into(), 176);
        known_dlls.insert("d3dx11_42.dll".into(), 44);
        known_dlls.insert("d3dx11_43.dll".into(), 44);
        known_dlls.insert("d3dx9_24.dll".into(), 320);
        known_dlls.insert("d3dx9_25.dll".into(), 323);
        known_dlls.insert("d3dx9_26.dll".into(), 327);
        known_dlls.insert("d3dx9_27.dll".into(), 327);
        known_dlls.insert("d3dx9_28.dll".into(), 332);
        known_dlls.insert("d3dx9_29.dll".into(), 332);
        known_dlls.insert("d3dx9_30.dll".into(), 332);
        known_dlls.insert("d3dx9_31.dll".into(), 329);
        known_dlls.insert("d3dx9_32.dll".into(), 334);
        known_dlls.insert("d3dx9_33.dll".into(), 334);
        known_dlls.insert("d3dx9_34.dll".into(), 334);
        known_dlls.insert("d3dx9_35.dll".into(), 334);
        known_dlls.insert("d3dx9_36.dll".into(), 336);
        known_dlls.insert("d3dx9_37.dll".into(), 336);
        known_dlls.insert("d3dx9_38.dll".into(), 336);
        known_dlls.insert("d3dx9_39.dll".into(), 336);
        known_dlls.insert("d3dx9_40.dll".into(), 336);
        known_dlls.insert("d3dx9_41.dll".into(), 336);
        known_dlls.insert("d3dx9_42.dll".into(), 329);
        known_dlls.insert("d3dx9_43.dll".into(), 329);
        known_dlls.insert("d3dxof.dll".into(), 1);
        known_dlls.insert("davclnt.dll".into(), 20);
        known_dlls.insert("dbgeng.dll".into(), 5);
        known_dlls.insert("dbghelp.dll".into(), 195);
        known_dlls.insert("dciman32.dll".into(), 20);
        known_dlls.insert("dcomp.dll".into(), 13);
        known_dlls.insert("ddraw.dll".into(), 24);
        known_dlls.insert("desk.cpl".into(), 1);
        known_dlls.insert("dhcpcsvc.dll".into(), 45);
        known_dlls.insert("dhcpcsvc6.dll".into(), 22);
        known_dlls.insert("diasymreader.dll".into(), 3);
        known_dlls.insert("difxapi.dll".into(), 12);
        known_dlls.insert("dinput.dll".into(), 3);
        known_dlls.insert("dinput8.dll".into(), 1);
        known_dlls.insert("directmanipulation.dll".into(), 1);
        known_dlls.insert("dmusic32.dll".into(), 2);
        known_dlls.insert("dnsapi.dll".into(), 133);
        known_dlls.insert("dplay.dll".into(), 2);
        known_dlls.insert("dplayx.dll".into(), 6);
        known_dlls.insert("dpnaddr.dll".into(), 1);
        known_dlls.insert("dpnet.dll".into(), 1);
        known_dlls.insert("dpnhpast.dll".into(), 1);
        known_dlls.insert("dpnhupnp.dll".into(), 1);
        known_dlls.insert("dpnlobby.dll".into(), 1);
        known_dlls.insert("dpvoice.dll".into(), 1);
        known_dlls.insert("dpwsockx.dll".into(), 3);
        known_dlls.insert("drmclien.dll".into(), 18);
        known_dlls.insert("dsound.dll".into(), 10);
        known_dlls.insert("dsquery.dll".into(), 3);
        known_dlls.insert("dsrole.dll".into(), 2);
        known_dlls.insert("dssenh.dll".into(), 25);
        known_dlls.insert("dsuiext.dll".into(), 4);
        known_dlls.insert("dwmapi.dll".into(), 64);
        known_dlls.insert("dwrite.dll".into(), 1);
        known_dlls.insert("dx8vb.dll".into(), 135);
        known_dlls.insert("dxcore.dll".into(), 1);
        known_dlls.insert("dxgi.dll".into(), 6);
        known_dlls.insert("dxva2.dll".into(), 38);
        known_dlls.insert("esent.dll".into(), 336);
        known_dlls.insert("evr.dll".into(), 24);
        known_dlls.insert("explorerframe.dll".into(), 3);
        known_dlls.insert("faultrep.dll".into(), 14);
        known_dlls.insert("feclient.dll".into(), 50);
        known_dlls.insert("fltlib.dll".into(), 29);
        known_dlls.insert("fltmgr.sys".into(), 163);
        known_dlls.insert("fntcache.dll".into(), 1);
        known_dlls.insert("fontsub.dll".into(), 2);
        known_dlls.insert("fusion.dll".into(), 17);
        known_dlls.insert("fwpuclnt.dll".into(), 146);
        known_dlls.insert("gameinput.dll".into(), 1);
        known_dlls.insert("gameux.dll".into(), 1);
        known_dlls.insert("gamingtcui.dll".into(), 26);
        known_dlls.insert("gdi32.dll".into(), 641);
        known_dlls.insert("gdiplus.dll".into(), 630);
        known_dlls.insert("glu32.dll".into(), 53);
        known_dlls.insert("gphoto2.ds.dll".into(), 1);
        known_dlls.insert("gpkcsp.dll".into(), 23);
        known_dlls.insert("hal.dll".into(), 75);
        known_dlls.insert("hhctrl.ocx".into(), 3);
        known_dlls.insert("hid.dll".into(), 44);
        known_dlls.insert("hidclass.sys".into(), 1);
        known_dlls.insert("hidparse.sys".into(), 30);
        known_dlls.insert("hlink.dll".into(), 28);
        known_dlls.insert("hrtfapo.dll".into(), 5);
        known_dlls.insert("httpapi.dll".into(), 37);
        known_dlls.insert("iccvid.dll".into(), 1);
        known_dlls.insert("icmp.dll".into(), 8);
        known_dlls.insert("icmui.dll".into(), 2);
        known_dlls.insert("icu.dll".into(), 1052);
        known_dlls.insert("icuin.dll".into(), 410);
        known_dlls.insert("icuuc.dll".into(), 541);
        known_dlls.insert("ieframe.dll".into(), 5);
        known_dlls.insert("ieproxy.dll".into(), 1);
        known_dlls.insert("iertutil.dll".into(), 6);
        known_dlls.insert("imaadp32.acm.dll".into(), 1);
        known_dlls.insert("imagehlp.dll".into(), 107);
        known_dlls.insert("imm32.dll".into(), 136);
        known_dlls.insert("inetcomm.dll".into(), 102);
        known_dlls.insert("inetcpl.cpl".into(), 24);
        known_dlls.insert("inetmib1.dll".into(), 4);
        known_dlls.insert("initpki.dll".into(), 1);
        known_dlls.insert("inseng.dll".into(), 7);
        known_dlls.insert("iphlpapi.dll".into(), 298);
        known_dlls.insert("iprop.dll".into(), 8);
        known_dlls.insert("ir50_32.dll".into(), 1);
        known_dlls.insert("irprops.cpl".into(), 52);
        known_dlls.insert("iyuv_32.dll".into(), 1);
        known_dlls.insert("joy.cpl".into(), 2);
        known_dlls.insert("jsproxy.dll".into(), 6);
        known_dlls.insert("kerberos.dll".into(), 10);
        known_dlls.insert("kernel32.dll".into(), 1639);
        known_dlls.insert("kernelbase.dll".into(), 1813);
        known_dlls.insert("ksecdd.sys".into(), 104);
        known_dlls.insert("ksproxy.ax.dll".into(), 6);
        known_dlls.insert("ksuser.dll".into(), 4);
        known_dlls.insert("ktmw32.dll".into(), 44);
        known_dlls.insert("l3codeca.acm.dll".into(), 1);
        known_dlls.insert("loadperf.dll".into(), 14);
        known_dlls.insert("localspl.dll".into(), 63);
        known_dlls.insert("localui.dll".into(), 1);
        known_dlls.insert("lz32.dll".into(), 12);
        known_dlls.insert("magnification.dll".into(), 21);
        known_dlls.insert("mapi32.dll".into(), 166);
        known_dlls.insert("mapistub.dll".into(), 166);
        known_dlls.insert("mciavi32.dll".into(), 1);
        known_dlls.insert("mcicda.dll".into(), 1);
        known_dlls.insert("mciqtz32.dll".into(), 1);
        known_dlls.insert("mciseq.dll".into(), 1);
        known_dlls.insert("mciwave.dll".into(), 1);
        known_dlls.insert("mf.dll".into(), 82);
        known_dlls.insert("mf3216.dll".into(), 2);
        known_dlls.insert("mfplat.dll".into(), 184);
        known_dlls.insert("mfplay.dll".into(), 2);
        known_dlls.insert("mfreadwrite.dll".into(), 5);
        known_dlls.insert("mfsrcsnk.dll".into(), 1);
        known_dlls.insert("mgmtapi.dll".into(), 9);
        known_dlls.insert("midimap.dll".into(), 3);
        known_dlls.insert("mlang.dll".into(), 10);
        known_dlls.insert("mmcndmgr.dll".into(), 1);
        known_dlls.insert("mmdevapi.dll".into(), 5);
        known_dlls.insert("mp3dmod.dll".into(), 1);
        known_dlls.insert("mpr.dll".into(), 88);
        known_dlls.insert("mprapi.dll".into(), 132);
        known_dlls.insert("msacm32.dll".into(), 44);
        known_dlls.insert("msacm32.drv".into(), 3);
        known_dlls.insert("msadp32.acm.dll".into(), 1);
        known_dlls.insert("msasn1.dll".into(), 266);
        known_dlls.insert("mscat32.dll".into(), 34);
        known_dlls.insert("mscms.dll".into(), 103);
        known_dlls.insert("mscoree.dll".into(), 114);
        known_dlls.insert("mscorwks.dll".into(), 106);
        known_dlls.insert("msctf.dll".into(), 30);
        known_dlls.insert("msctfmonitor.dll".into(), 3);
        known_dlls.insert("msctfp.dll".into(), 1);
        known_dlls.insert("msdelta.dll".into(), 15);
        known_dlls.insert("msdmo.dll".into(), 15);
        known_dlls.insert("msdrm.dll".into(), 84);
        known_dlls.insert("msftedit.dll".into(), 8);
        known_dlls.insert("msg711.acm.dll".into(), 1);
        known_dlls.insert("msgsm32.acm.dll".into(), 1);
        known_dlls.insert("mshtml.dll".into(), 9);
        known_dlls.insert("msi.dll".into(), 290);
        known_dlls.insert("msimg32.dll".into(), 4);
        known_dlls.insert("msimtf.dll".into(), 2);
        known_dlls.insert("msisip.dll".into(), 6);
        known_dlls.insert("msls31.dll".into(), 79);
        known_dlls.insert("msmpeg2vdec.dll".into(), 1);
        known_dlls.insert("mspatcha.dll".into(), 16);
        known_dlls.insert("msports.dll".into(), 11);
        known_dlls.insert("msrle32.dll".into(), 1);
        known_dlls.insert("msscript.ocx".into(), 1);
        known_dlls.insert("mssign32.dll".into(), 28);
        known_dlls.insert("mssip32.dll".into(), 7);
        known_dlls.insert("mstask.dll".into(), 24);
        known_dlls.insert("msv1_0.dll".into(), 18);
        known_dlls.insert("msvcirt.dll".into(), 2);
        known_dlls.insert("msvcm80.dll".into(), 3);
        known_dlls.insert("msvcm90.dll".into(), 3);
        known_dlls.insert("msvcp100.dll".into(), 57);
        known_dlls.insert("msvcp110.dll".into(), 147);
        known_dlls.insert("msvcp120.dll".into(), 150);
        known_dlls.insert("msvcp120_app.dll".into(), 150);
        known_dlls.insert("msvcp140.dll".into(), 120);
        known_dlls.insert("msvcp140_1.dll".into(), 7);
        known_dlls.insert("msvcp60.dll".into(), 41);
        known_dlls.insert("msvcp70.dll".into(), 46);
        known_dlls.insert("msvcp71.dll".into(), 48);
        known_dlls.insert("msvcp80.dll".into(), 61);
        known_dlls.insert("msvcp90.dll".into(), 56);
        known_dlls.insert("msvcp_win.dll".into(), 120);
        known_dlls.insert("msvcr100.dll".into(), 1048);
        known_dlls.insert("msvcr110.dll".into(), 1049);
        known_dlls.insert("msvcr120.dll".into(), 1304);
        known_dlls.insert("msvcr120_app.dll".into(), 1028);
        known_dlls.insert("msvcr70.dll".into(), 558);
        known_dlls.insert("msvcr71.dll".into(), 560);
        known_dlls.insert("msvcr80.dll".into(), 1051);
        known_dlls.insert("msvcr90.dll".into(), 1049);
        known_dlls.insert("msvcrt.dll".into(), 913);
        known_dlls.insert("msvcrt20.dll".into(), 537);
        known_dlls.insert("msvcrt40.dll".into(), 516);
        known_dlls.insert("msvcrtd.dll".into(), 543);
        known_dlls.insert("msvfw32.dll".into(), 47);
        known_dlls.insert("msvidc32.dll".into(), 1);
        known_dlls.insert("mswsock.dll".into(), 32);
        known_dlls.insert("mtxdm.dll".into(), 1);
        known_dlls.insert("ncrypt.dll".into(), 138);
        known_dlls.insert("nddeapi.dll".into(), 28);
        known_dlls.insert("ndfapi.dll".into(), 24);
        known_dlls.insert("ndis.sys".into(), 276);
        known_dlls.insert("netapi32.dll".into(), 300);
        known_dlls.insert("netcfgx.dll".into(), 12);
        known_dlls.insert("netio.sys".into(), 389);
        known_dlls.insert("netutils.dll".into(), 22);
        known_dlls.insert("newdev.dll".into(), 18);
        known_dlls.insert("ninput.dll".into(), 24);
        known_dlls.insert("normaliz.dll".into(), 5);
        known_dlls.insert("npmshtml.dll".into(), 3);
        known_dlls.insert("npptools.dll".into(), 60);
        known_dlls.insert("nsi.dll".into(), 26);
        known_dlls.insert("ntdll.dll".into(), 1540);
        known_dlls.insert("ntdsapi.dll".into(), 96);
        known_dlls.insert("ntoskrnl.exe.dll".into(), 1614);
        known_dlls.insert("ntprint.dll".into(), 35);
        known_dlls.insert("odbc32.dll".into(), 174);
        known_dlls.insert("odbcbcp.dll".into(), 28);
        known_dlls.insert("odbccp32.dll".into(), 57);
        known_dlls.insert("odbccu32.dll".into(), 37);
        known_dlls.insert("ole32.dll".into(), 298);
        known_dlls.insert("oleacc.dll".into(), 16);
        known_dlls.insert("oleaut32.dll".into(), 414);
        known_dlls.insert("olecli32.dll".into(), 56);
        known_dlls.insert("oledlg.dll".into(), 21);
        known_dlls.insert("olepro32.dll".into(), 7);
        known_dlls.insert("olesvr32.dll".into(), 12);
        known_dlls.insert("olethk32.dll".into(), 14);
        known_dlls.insert("opencl.dll".into(), 98);
        known_dlls.insert("opengl32.dll".into(), 361);
        known_dlls.insert("pdh.dll".into(), 163);
        known_dlls.insert("pidgen.dll".into(), 7);
        known_dlls.insert("powrprof.dll".into(), 33);
        known_dlls.insert("printui.dll".into(), 21);
        known_dlls.insert("prntvpt.dll".into(), 22);
        known_dlls.insert("propsys.dll".into(), 183);
        known_dlls.insert("psapi.dll".into(), 27);
        known_dlls.insert("pstorec.dll".into(), 3);
        known_dlls.insert("pwrshplugin.dll".into(), 12);
        known_dlls.insert("qmgr.dll".into(), 1);
        known_dlls.insert("qmgrprxy.dll".into(), 1);
        known_dlls.insert("quartz.dll".into(), 5);
        known_dlls.insert("query.dll".into(), 42);
        known_dlls.insert("qwave.dll".into(), 14);
        known_dlls.insert("rasapi32.dll".into(), 202);
        known_dlls.insert("rasdlg.dll".into(), 36);
        known_dlls.insert("regapi.dll".into(), 69);
        known_dlls.insert("resutils.dll".into(), 71);
        known_dlls.insert("riched20.dll".into(), 4);
        known_dlls.insert("rometadata.dll".into(), 1);
        known_dlls.insert("rpcrt4.dll".into(), 524);
        known_dlls.insert("rsabase.dll".into(), 25);
        known_dlls.insert("rsaenh.dll".into(), 25);
        known_dlls.insert("rstrtmgr.dll".into(), 12);
        known_dlls.insert("rtutils.dll".into(), 56);
        known_dlls.insert("rtworkq.dll".into(), 37);
        known_dlls.insert("samlib.dll".into(), 65);
        known_dlls.insert("sane.ds.dll".into(), 1);
        known_dlls.insert("sas.dll".into(), 1);
        known_dlls.insert("scarddlg.dll".into(), 5);
        known_dlls.insert("scardsvr.dll".into(), 3);
        known_dlls.insert("sccbase.dll".into(), 24);
        known_dlls.insert("schannel.dll".into(), 37);
        known_dlls.insert("schedsvc.dll".into(), 1);
        known_dlls.insert("scrrun.dll".into(), 2);
        known_dlls.insert("scsiport.sys".into(), 47);
        known_dlls.insert("sechost.dll".into(), 208);
        known_dlls.insert("secur32.dll".into(), 80);
        known_dlls.insert("security.dll".into(), 36);
        known_dlls.insert("sensapi.dll".into(), 3);
        known_dlls.insert("serialui.dll".into(), 7);
        known_dlls.insert("setupapi.dll".into(), 617);
        known_dlls.insert("sfc.dll".into(), 7);
        known_dlls.insert("sfc_os.dll".into(), 18);
        known_dlls.insert("shcore.dll".into(), 194);
        known_dlls.insert("shdocvw.dll".into(), 117);
        known_dlls.insert("shell32.dll".into(), 467);
        known_dlls.insert("shfolder.dll".into(), 2);
        known_dlls.insert("shlwapi.dll".into(), 843);
        known_dlls.insert("slbcsp.dll".into(), 25);
        known_dlls.insert("slc.dll".into(), 41);
        known_dlls.insert("snmpapi.dll".into(), 44);
        known_dlls.insert("softpub.dll".into(), 22);
        known_dlls.insert("spoolss.dll".into(), 148);
        known_dlls.insert("sppc.dll".into(), 67);
        known_dlls.insert("srclient.dll".into(), 13);
        known_dlls.insert("srvcli.dll".into(), 61);
        known_dlls.insert("srvsvc.dll".into(), 1);
        known_dlls.insert("sspicli.dll".into(), 104);
        known_dlls.insert("sti.dll".into(), 3);
        known_dlls.insert("strmdll.dll".into(), 5);
        known_dlls.insert("svrapi.dll".into(), 20);
        known_dlls.insert("sxs.dll".into(), 4);
        known_dlls.insert("t2embed.dll".into(), 25);
        known_dlls.insert("tapi32.dll".into(), 183);
        known_dlls.insert("tbs.dll".into(), 21);
        known_dlls.insert("tdh.dll".into(), 34);
        known_dlls.insert("tdi.sys".into(), 54);
        known_dlls.insert("tiptsf.dll".into(), 7);
        known_dlls.insert("traffic.dll".into(), 21);
        known_dlls.insert("twain_32.dll".into(), 1);
        known_dlls.insert("twaindsm.dll".into(), 1);
        known_dlls.insert("twinapi.appcore.dll".into(), 124);
        known_dlls.insert("ucrtbase.dll".into(), 2302);
        known_dlls.insert("uiautomationcore.dll".into(), 100);
        known_dlls.insert("unicows.dll".into(), 504);
        known_dlls.insert("updspapi.dll".into(), 78);
        known_dlls.insert("url.dll".into(), 21);
        known_dlls.insert("urlmon.dll".into(), 89);
        known_dlls.insert("usbd.sys".into(), 35);
        known_dlls.insert("user32.dll".into(), 1142);
        known_dlls.insert("userenv.dll".into(), 22);
        known_dlls.insert("usp10.dll".into(), 44);
        known_dlls.insert("utildll.dll".into(), 36);
        known_dlls.insert("uxtheme.dll".into(), 122);
        known_dlls.insert("vcomp.dll".into(), 111);
        known_dlls.insert("vcomp100.dll".into(), 111);
        known_dlls.insert("vcomp110.dll".into(), 111);
        known_dlls.insert("vcomp120.dll".into(), 111);
        known_dlls.insert("vcomp140.dll".into(), 111);
        known_dlls.insert("vcomp90.dll".into(), 111);
        known_dlls.insert("vcruntime140.dll".into(), 28);
        known_dlls.insert("vdmdbg.dll".into(), 19);
        known_dlls.insert("version.dll".into(), 16);
        known_dlls.insert("virtdisk.dll".into(), 21);
        known_dlls.insert("vssapi.dll".into(), 13);
        known_dlls.insert("vulkan-1.dll".into(), 262);
        known_dlls.insert("w32skrnl.dll".into(), 97);
        known_dlls.insert("wdscore.dll".into(), 93);
        known_dlls.insert("webservices.dll".into(), 193);
        known_dlls.insert("websocket.dll".into(), 13);
        known_dlls.insert("wer.dll".into(), 77);
        known_dlls.insert("wevtapi.dll".into(), 46);
        known_dlls.insert("wevtsvc.dll".into(), 1);
        known_dlls.insert("wiaservc.dll".into(), 55);
        known_dlls.insert("wimgapi.dll".into(), 40);
        known_dlls.insert("win32u.dll".into(), 1540);
        known_dlls.insert("winbio.dll".into(), 74);
        known_dlls.insert("winbrand.dll".into(), 15);
        known_dlls.insert("windows.devices.radios.dll".into(), 6);
        known_dlls.insert("windows.media.mediacontrol.dll".into(), 8);
        known_dlls.insert("windows.networking.dll".into(), 1);
        known_dlls.insert("windows.networking.connectivity.dll".into(), 5);
        known_dlls.insert("windows.networking.hostname.dll".into(), 6);
        known_dlls.insert("windows.perception.stub.dll".into(), 1);
        known_dlls.insert("windows.storage.dll".into(), 257);
        known_dlls.insert("windows.storage.applicationdata.dll".into(), 1);
        known_dlls.insert("windows.ui.dll".into(), 2);
        known_dlls.insert("windows.ui.xaml.dll".into(), 12);
        known_dlls.insert("windowscodecs.dll".into(), 114);
        known_dlls.insert("windowscodecsext.dll".into(), 2);
        known_dlls.insert("wined3d.dll".into(), 375);
        known_dlls.insert("winedmo.dll".into(), 8);
        known_dlls.insert("winegstreamer.dll".into(), 2);
        known_dlls.insert("winemapi.dll".into(), 11);
        known_dlls.insert("wineps.drv".into(), 8);
        known_dlls.insert("winevulkan.dll".into(), 265);
        known_dlls.insert("wing32.dll".into(), 10);
        known_dlls.insert("winhttp.dll".into(), 45);
        known_dlls.insert("wininet.dll".into(), 247);
        known_dlls.insert("winmm.dll".into(), 186);
        known_dlls.insert("winnls32.dll".into(), 7);
        known_dlls.insert("winprint.dll".into(), 6);
        known_dlls.insert("winscard.dll".into(), 60);
        known_dlls.insert("winspool.drv".into(), 188);
        known_dlls.insert("winsqlite3.dll".into(), 284);
        known_dlls.insert("winsta.dll".into(), 95);
        known_dlls.insert("wintab32.dll".into(), 44);
        known_dlls.insert("wintrust.dll".into(), 154);
        known_dlls.insert("wintypes.dll".into(), 8);
        known_dlls.insert("winusb.dll".into(), 22);
        known_dlls.insert("wkscli.dll".into(), 23);
        known_dlls.insert("wlanapi.dll".into(), 39);
        known_dlls.insert("wlanui.dll".into(), 5);
        known_dlls.insert("wldap32.dll".into(), 243);
        known_dlls.insert("wldp.dll".into(), 29);
        known_dlls.insert("wmasf.dll".into(), 35);
        known_dlls.insert("wmi.dll".into(), 45);
        known_dlls.insert("wmilib.sys".into(), 3);
        known_dlls.insert("wminet_utils.dll".into(), 61);
        known_dlls.insert("wmvcore.dll".into(), 19);
        known_dlls.insert("wnaspi32.dll".into(), 7);
        known_dlls.insert("wofutil.dll".into(), 11);
        known_dlls.insert("wow32.dll".into(), 17);
        known_dlls.insert("wow64.dll".into(), 28);
        known_dlls.insert("wow64cpu.dll".into(), 10);
        known_dlls.insert("wpc.dll".into(), 1);
        known_dlls.insert("wpcap.dll".into(), 111);
        known_dlls.insert("ws2_32.dll".into(), 135);
        known_dlls.insert("wsdapi.dll".into(), 45);
        known_dlls.insert("wsnmp32.dll".into(), 48);
        known_dlls.insert("wsock32.dll".into(), 74);
        known_dlls.insert("wtsapi32.dll".into(), 52);
        known_dlls.insert("wuaueng.dll".into(), 11);
        known_dlls.insert("x3daudio1_3.dll".into(), 2);
        known_dlls.insert("x3daudio1_4.dll".into(), 2);
        known_dlls.insert("x3daudio1_5.dll".into(), 2);
        known_dlls.insert("x3daudio1_6.dll".into(), 2);
        known_dlls.insert("x3daudio1_7.dll".into(), 2);
        known_dlls.insert("xapofx1_1.dll".into(), 1);
        known_dlls.insert("xapofx1_2.dll".into(), 1);
        known_dlls.insert("xapofx1_3.dll".into(), 1);
        known_dlls.insert("xapofx1_4.dll".into(), 1);
        known_dlls.insert("xapofx1_5.dll".into(), 1);
        known_dlls.insert("xaudio2_8.dll".into(), 7);
        known_dlls.insert("xaudio2_9.dll".into(), 7);
        known_dlls.insert("xinput1_1.dll".into(), 5);
        known_dlls.insert("xinput1_2.dll".into(), 5);
        known_dlls.insert("xinput1_3.dll".into(), 8);
        known_dlls.insert("xinput1_4.dll".into(), 9);
        known_dlls.insert("xinput9_1_0.dll".into(), 4);
        known_dlls.insert("xinputuap.dll".into(), 7);
        known_dlls.insert("xmllite.dll".into(), 6);
        known_dlls.insert("xolehlp.dll".into(), 6);
        known_dlls.insert("xpsprint.dll".into(), 2);
        known_dlls.insert("xpssvcs.dll".into(), 9);
        known_dlls.insert("xtajit64.dll".into(), 16);
        Self { known_dlls }
    }
}

static REGISTRY: OnceLock<UniversalDllRegistry> = OnceLock::new();

pub fn get_dll_registry() -> &'static UniversalDllRegistry {
    REGISTRY.get_or_init(UniversalDllRegistry::new)
}

pub fn is_registered_dll(name: &str) -> bool {
    let reg = get_dll_registry();
    reg.known_dlls.contains_key(&name.to_lowercase())
}
