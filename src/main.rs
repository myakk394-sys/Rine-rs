use rine::{LoadConfig, LoadedModule, PeImage, StorageStrategy};
use std::env;
use std::fs;
use std::path::Path;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("[RINE ENGINE] Initializing Rine Win64 PE Execution Engine v0.2.0...");
    rine::sys::setup_signal_handlers();

    if args.len() < 2 || args.contains(&"--gui".to_string()) || args.contains(&"--ui".to_string()) {
        println!("[Rine GUI] Launching Native Linux Desktop Application...");
        if let Err(e) = rine::gui::RineNativeApp::run() {
            eprintln!("[Rine GUI] Error starting desktop application: {}", e);
        }
        return;
    }

    let mut exe_path = "";
    let mut debug_mode = false;
    let mut lock_ram = false;
    let mut strategy = StorageStrategy::RamOnly {
        lock_in_memory: false,
    };

    for arg in &args[1..] {
        match arg.as_str() {
            "--debug" | "-d" => {
                debug_mode = true;
            }
            "--ram-only" => {
                strategy = StorageStrategy::RamOnly {
                    lock_in_memory: lock_ram,
                };
            }
            "--lock-ram" => {
                lock_ram = true;
                if let StorageStrategy::RamOnly {
                    ref mut lock_in_memory,
                } = strategy
                {
                    *lock_in_memory = true;
                }
            }
            "--disk-demand" => {
                strategy = StorageStrategy::DiskDemand;
            }
            s if !s.starts_with('-') && exe_path.is_empty() => {
                exe_path = s;
            }
            _ => {}
        }
    }

    if exe_path.is_empty() {
        exe_path = &args[1];
    }

    if debug_mode {
        println!("[RINE ENGINE] Verbose Debug Mode ENABLED (--debug)");
    }

    // Derive window title from EXE filename (works for any application)
    let exe_stem = Path::new(exe_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Rine Application");
    let window_title = format!("{} (Rine)", exe_stem);

    println!("[Rine] Loading PE binary: {}", exe_path);
    println!("[Rine] Storage strategy: {:?}", strategy);

    rine::dlls::kernel32::set_target_exe_path(exe_path);

    let bytes = match fs::read(Path::new(exe_path)) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[Rine] Error reading file '{}': {}", exe_path, e);
            exit(1);
        }
    };

    // 1. Inspect PE Image
    let subsystem = match PeImage::parse(&bytes) {
        Ok(pe) => {
            println!(
                "[PE] Target Machine: 0x{:04X} (Win64 / AMD64)",
                pe.file_header.machine()
            );
            println!("[PE] Subsystem: {}", pe.optional_header.subsystem());
            println!(
                "[PE] Entry Point RVA: 0x{:08X}",
                pe.optional_header.address_of_entry_point()
            );
            println!(
                "[PE] Image Base: 0x{:016X}",
                pe.optional_header.image_base()
            );
            println!(
                "[PE] Size of Image: {} bytes",
                pe.optional_header.size_of_image()
            );
            println!("[PE] Sections Count: {}", pe.sections.len());

            for sec in pe.sections {
                println!(
                    "  -> Section [{:8}] VAddr: 0x{:08X}, VSize: 0x{:08X}, RawSize: 0x{:08X}, Flags: 0x{:08X}",
                    sec.name_str(),
                    sec.virtual_address(),
                    sec.virtual_size(),
                    sec.size_of_raw_data(),
                    sec.characteristics()
                );
            }

            let imports = pe.imports();
            println!("[PE] Imported Libraries ({}):", imports.len());
            for (dll, _) in imports {
                println!("  -> DLL: {}", dll);
            }

            pe.optional_header.subsystem()
        }
        Err(e) => {
            eprintln!("[Rine] Failed to parse PE image: {}", e);
            exit(1);
        }
    };

    // 2. Load into Virtual Memory Space
    let config = LoadConfig {
        strategy,
        enable_prefetch: true,
    };

    match LoadedModule::load(exe_path, &bytes, config) {
        Ok(module) => {
            println!("[Rine] Module successfully mapped into memory!");
            println!("  -> Allocated Image Base: 0x{:016X}", module.image_base);
            println!("  -> Resolved Entry Point: 0x{:016X}", module.entry_point);
            println!("  -> Total Mapped Size:    {} bytes", module.size_of_image);
            println!(
                "  -> PEB64 Virtual Address: 0x{:016X}",
                module.env.peb_address
            );
            println!(
                "  -> TEB64 Virtual Address: 0x{:016X}",
                module.env.teb_address
            );

            std::panic::set_hook(Box::new(|info| {
                println!("[RINE PANIC DETECTED]: {:?}", info);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }));

            let ep = module.entry_point as u64;
            let peb = module.env.peb_address as u64;
            let teb = module.env.teb_address as u64;

            // Determine default window size based on subsystem
            // 2 = GUI (Windows app), 3 = CUI (console app)
            let (win_w, win_h) = if subsystem == 3 {
                (800u32, 600u32)
            } else {
                (1280u32, 720u32)
            };

            // Create SDL2 window with correct EXE title — works for any application
            let window_id = unsafe {
                rine::windowing::x11::create_physical_x11_window(&window_title, win_w, win_h)
            };
            if window_id > 0 {
                println!(
                    "[RINE WINDOW] Created desktop window: '{}' ({}x{})",
                    window_title, win_w, win_h
                );
            } else {
                println!("[RINE WINDOW] Note: no graphical display available (headless mode)");
            }

            // Initialize Vulkan & audio
            let _vk_engine = unsafe { rine::graphics::dxgi_vulkan::VulkanDxvkEngine::init() };

            use std::io::Write;
            let _ = std::io::stdout().flush();

            // CRT section initializers are executed by MSVC _initterm which skips null pointers.

            println!("[RINE MAIN] Mapping complete. Dispatching to Win64 entry point...");
            println!(
                "[RINE EXEC] EP=0x{:016X}, PEB=0x{:016X}, TEB=0x{:016X}",
                ep, peb, teb
            );

            // Load DXVK PE DLLs so game's D3D11/DXGI IAT calls are routed to real Vulkan
            if let Some(dxvk_dir) = rine::dlls::dxvk_loader::find_dxvk_path() {
                rine::dlls::dxvk_loader::preload_dxvk(dxvk_dir);
            } else {
                println!("[RINE MAIN] DXVK not found — D3D11 rendering will use stub fallbacks");
            }

            unsafe {
                rine::sys::jump_to_win64_entry_point(ep, peb, teb);
            }
        }
        Err(e) => {
            eprintln!("[Rine] Memory loader error: {}", e);
            exit(1);
        }
    }
}
