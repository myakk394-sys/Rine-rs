use rine::pe::imports::IatSymbolResolver;
use rine::pe::PeImage;
use rine::providers::{
    call_win64_export_safely, load_and_map_wine_pe_dll, set_global_wine_config, ApiProviderKind,
    RineApiResolver, WineConfig,
};
use std::path::PathBuf;

fn main() {
    let dll_path = PathBuf::from("/usr/lib/x86_64-linux-gnu/wine/x86_64-windows/avrt.dll");
    if !dll_path.exists() {
        eprintln!("Error: Wine avrt.dll not found at {}", dll_path.display());
        std::process::exit(1);
    }

    let raw_bytes = std::fs::read(&dll_path).expect("Failed to read avrt.dll");
    let pe = PeImage::parse(&raw_bytes).expect("Failed to parse PE image avrt.dll");

    let pref_base = pe.optional_header.image_base() as usize;
    let img_size = pe.optional_header.size_of_image() as usize;
    let entry_rva = pe.optional_header.address_of_entry_point() as usize;

    let mut text_rva = 0;
    let mut text_vsize = 0;
    for sec in pe.sections {
        if sec.name_str() == ".text" {
            text_rva = sec.virtual_address() as usize;
            text_vsize = sec.virtual_size() as usize;
            break;
        }
    }

    let config = WineConfig {
        enabled: true,
        dll_path: Some(PathBuf::from(
            "/usr/lib/x86_64-linux-gnu/wine/x86_64-windows",
        )),
        allowed_modules: vec!["avrt.dll".into()],
        allowed_symbols: vec!["avrt.dll!AvSetMmThreadCharacteristicsA".into()],
    };
    set_global_wine_config(config.clone());

    let rine_has_a =
        IatSymbolResolver::resolve_symbol("avrt.dll", "AvSetMmThreadCharacteristicsA").is_some();

    let loaded = load_and_map_wine_pe_dll(&dll_path, &config).expect("Failed to map avrt.dll");
    let runtime_base = loaded.image_base;
    let text_start_va = runtime_base + text_rva;
    let text_end_va = text_start_va + text_vsize;

    println!("[RINE WINE MODULE]");
    println!("path={}", dll_path.display());
    println!("preferred_image_base=0x{:016X}", pref_base);
    println!("runtime_image_base=0x{:016X}", runtime_base);
    println!("image_size={}", img_size);
    println!("entry_point=0x{:016X}", runtime_base + entry_rva);
    println!(".text RVA=0x{:08X}", text_rva);
    println!(
        ".text runtime range=0x{:016X} - 0x{:016X}",
        text_start_va, text_end_va
    );

    let export_a_addr = *loaded
        .exports
        .get("AvSetMmThreadCharacteristicsA")
        .expect("AvSetMmThreadCharacteristicsA not found");

    let export_revert_addr = *loaded
        .exports
        .get("AvRevertMmThreadCharacteristics")
        .expect("AvRevertMmThreadCharacteristics not found");

    let export_a_rva = export_a_addr - runtime_base;
    let export_revert_rva = export_revert_addr - runtime_base;

    let res_api = RineApiResolver::resolve_symbol("avrt.dll", "AvSetMmThreadCharacteristicsA")
        .expect("RineApiResolver failed to resolve AvSetMmThreadCharacteristicsA");

    println!("\n[RINE PROVIDER RESOLUTION]");
    println!("dll=avrt.dll");
    println!("symbol=AvSetMmThreadCharacteristicsA");
    println!("rine_available={}", rine_has_a);
    println!("wine_available=true");
    println!(
        "selected={}",
        match res_api.provider {
            ApiProviderKind::NativeRine => "rine",
            ApiProviderKind::WineAdapter => "wine",
        }
    );
    println!("runtime_address=0x{:016X}", res_api.address);

    println!("\n[RINE WINE DEPENDENCY GRAPH]");
    let imports = pe.imports();
    for (dep_dll, funcs) in &imports {
        for (fn_name, iat_thunk_rva) in funcs {
            let iat_va = runtime_base + *iat_thunk_rva as usize;
            let target_ptr = unsafe { std::ptr::read_unaligned(iat_va as *const u64) as usize };
            let prov = if IatSymbolResolver::resolve_symbol(dep_dll, fn_name).is_some() {
                "NativeRine"
            } else if target_ptr != 0 {
                "WineAdapter/Stub"
            } else {
                "Unresolved"
            };
            println!(
                "{}!{} | provider={} | resolved_va=0x{:016X}",
                dep_dll, fn_name, prov, target_ptr
            );
        }
    }

    let code_a = unsafe { std::slice::from_raw_parts(export_a_addr as *const u8, 32) };
    let code_revert = unsafe { std::slice::from_raw_parts(export_revert_addr as *const u8, 32) };

    println!("\n[EXPORT DETAIL: AvSetMmThreadCharacteristicsA]");
    println!("export RVA=0x{:08X}", export_a_rva);
    println!("runtime VA=0x{:016X}", export_a_addr);
    println!("bytes={}", hex_str(code_a));

    println!("\n[EXPORT DETAIL: AvRevertMmThreadCharacteristics]");
    println!("export RVA=0x{:08X}", export_revert_rva);
    println!("runtime VA=0x{:016X}", export_revert_addr);
    println!("bytes={}", hex_str(code_revert));

    // Pause for GDB attach or execution if requested
    if std::env::var("RINE_GDB_WAIT").is_ok() {
        println!("\n[GDB READY] Press ENTER after attaching GDB and setting breakpoints...");
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
    }

    println!("\n[RINE WINE EXECUTION START]");
    let mut task_idx: u32 = 0;
    let task_name = "Audio\0";
    let handle = unsafe {
        call_win64_export_safely(
            export_a_addr,
            task_name.as_ptr() as usize,
            &mut task_idx as *mut u32 as usize,
            0,
            0,
        )
    };

    println!("[RINE WINE EXECUTION RESULT]");
    println!("handle=0x{:016X}", handle);
    println!("task_idx={}", task_idx);

    let revert_status = unsafe { call_win64_export_safely(export_revert_addr, handle, 0, 0, 0) };

    println!("[RINE WINE REVERT RESULT]");
    println!("status={}", revert_status);
}

fn hex_str(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}
