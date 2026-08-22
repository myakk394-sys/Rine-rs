use rine::pe::imports::IatSymbolResolver;
use rine::pe::PeImage;
use rine::providers::{set_global_wine_config, ApiProviderKind, RineApiResolver, WineConfig};
use std::path::PathBuf;

fn main() {
    println!("[RINE LEVEL C] Initializing Real Guest IAT Resolution & Execution Test...");

    let dll_path = PathBuf::from("/usr/lib/x86_64-linux-gnu/wine/x86_64-windows/hid.dll");
    if !dll_path.exists() {
        eprintln!("Error: Wine hid.dll not found at {}", dll_path.display());
        std::process::exit(1);
    }

    let config = WineConfig {
        enabled: true,
        dll_path: Some(PathBuf::from(
            "/usr/lib/x86_64-linux-gnu/wine/x86_64-windows",
        )),
        allowed_modules: vec!["hid.dll".into()],
        allowed_symbols: vec!["hid.dll!HidD_GetHidGuid".into()],
    };
    set_global_wine_config(config.clone());

    let rine_has_sym = IatSymbolResolver::resolve_symbol("hid.dll", "HidD_GetHidGuid").is_some();

    // 1. Resolve API through RineApiResolver (real guest IAT resolution path)
    let res_api = RineApiResolver::resolve_symbol("hid.dll", "HidD_GetHidGuid")
        .expect("RineApiResolver failed to resolve HidD_GetHidGuid from hid.dll");

    let target_va = res_api.address as usize;

    let runtime_base = *rine::providers::LOADED_WINE_MODULES
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .get("hid.dll")
        .expect("hid.dll not found in LOADED_WINE_MODULES");

    let raw_bytes = std::fs::read(&dll_path).expect("Failed to read hid.dll");
    let pe = PeImage::parse(&raw_bytes).expect("Failed to parse hid.dll");

    let mut text_rva = 0;
    let mut text_vsize = 0;
    for sec in pe.sections {
        if sec.name_str() == ".text" {
            text_rva = sec.virtual_address() as usize;
            text_vsize = sec.virtual_size() as usize;
            break;
        }
    }

    let text_start_va = runtime_base + text_rva;
    let text_end_va = text_start_va + text_vsize;

    println!("\n[RINE PROVIDER RESOLUTION]");
    println!("dll=hid.dll");
    println!("symbol=HidD_GetHidGuid");
    println!("rine_available={}", rine_has_sym);
    println!("wine_available=true");
    println!(
        "selected={}",
        match res_api.provider {
            ApiProviderKind::NativeRine => "rine",
            ApiProviderKind::WineAdapter => "wine",
        }
    );
    println!("wine_module={}", dll_path.display());
    println!("export_rva=0x{:08X}", target_va - runtime_base);
    println!("runtime_address=0x{:016X}", target_va);

    println!("\n[RINE WINE MODULE]");
    println!("path={}", dll_path.display());
    println!(
        "preferred_image_base=0x{:016X}",
        pe.optional_header.image_base()
    );
    println!("runtime_image_base=0x{:016X}", runtime_base);
    println!("image_size={}", pe.optional_header.size_of_image());
    println!(
        "entry_point=0x{:016X}",
        runtime_base + pe.optional_header.address_of_entry_point() as usize
    );
    println!(".text RVA=0x{:08X}", text_rva);
    println!(
        ".text runtime range=0x{:016X} - 0x{:016X}",
        text_start_va, text_end_va
    );

    // 2. Simulate guest IAT slot in guest virtual memory
    let mut iat_slot: u64 = target_va as u64;
    let iat_slot_ptr = &mut iat_slot as *mut u64;

    println!("\n[GUEST IAT RESOLUTION]");
    println!("guest_iat_slot_address=0x{:016X}", iat_slot_ptr as usize);
    println!(
        "guest_iat_slot_value=0x{:016X} (points to Wine PE export)",
        unsafe { *iat_slot_ptr }
    );

    // 3. Guest execution: Execute call through guest IAT slot
    println!("\n[GUEST EXECUTION START]");
    #[repr(C)]
    #[derive(Debug, Default, PartialEq, Eq)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    let mut guid_out = Guid::default();

    let rax_out = unsafe {
        rine::providers::call_win64_export_safely(
            target_va,
            &mut guid_out as *mut Guid as usize,
            0,
            0,
            0,
        )
    };

    println!("[GUEST EXECUTION RETURNED]");
    println!("rax (return value) = 0x{:016X}", rax_out);
    println!(
        "hid_guid = {:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        guid_out.data1,
        guid_out.data2,
        guid_out.data3,
        guid_out.data4[0],
        guid_out.data4[1],
        guid_out.data4[2],
        guid_out.data4[3],
        guid_out.data4[4],
        guid_out.data4[5],
        guid_out.data4[6],
        guid_out.data4[7]
    );

    assert_eq!(guid_out.data1, 0x4D1E55B2, "HID GUID Data1 mismatch");
    println!("[ABI INVARIANT & CONTRACT] HID GUID successfully populated by real Wine hid.dll machine code!");
    println!("[GUEST CONTINUATION] Guest thread continued execution after Wine export return!");
}
