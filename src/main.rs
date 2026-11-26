use rine::{LoadConfig, LoadedModule, PeImage, StorageStrategy};
use std::env;
use std::fs;
use std::path::Path;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("[RINE ENGINE] Initializing Rine Win64 PE Execution Engine v0.2.0...");
    if std::env::var_os("DXVK_LOG_LEVEL").is_none() {
        std::env::set_var("DXVK_LOG_LEVEL", "warn");
    }
    if std::env::var_os("DXVK_LOG_PATH").is_none() {
        std::env::set_var("DXVK_LOG_PATH", "/tmp");
    }
    rine::sys::setup_signal_handlers();
    rine::windowing::x11::ensure_x11_environment();

    if args.contains(&"--overlay".to_string()) {
        println!("[Rine Overlay] Launching In-Game Overlay Settings Window...");
        if let Err(e) = rine::gui::RineOverlayApp::run() {
            eprintln!("[Rine Overlay] Error starting overlay: {}", e);
        }
        return;
    }

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
    let mut wine_enabled = false;
    let mut wine_dll_path: Option<std::path::PathBuf> = None;
    let mut working_dir: Option<String> = None;
    let mut custom_appid: Option<String> = None;
    let mut steam_offline = false;
    let allowed_modules: Vec<String> = vec![
        "version.dll".into(),
        "avrt.dll".into(),
        "hid.dll".into(),
        "dwmapi.dll".into(),
    ];

    let mut i = 1;
    let mut guest_args: Vec<String> = Vec::new();
    let mut in_guest_args = false;
    while i < args.len() {
        let arg = &args[i];
        if in_guest_args {
            guest_args.push(arg.clone());
            i += 1;
            continue;
        }
        match arg.as_str() {
            "--" => {
                in_guest_args = true;
            }
            "--debug" | "-d" => {
                debug_mode = true;
            }
            "--wine-provider" => {
                wine_enabled = true;
            }
            "--no-wine-provider" => {
                wine_enabled = false;
            }
            "--wine-dll-path" => {
                if i + 1 < args.len() {
                    wine_dll_path = Some(std::path::PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--working-directory" | "-w" => {
                if i + 1 < args.len() {
                    working_dir = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--steam-appid" | "-a" => {
                if i + 1 < args.len() {
                    custom_appid = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--steam-offline" => {
                steam_offline = true;
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
            _ => {
                if !exe_path.is_empty() {
                    guest_args.push(arg.clone());
                }
            }
        }
        i += 1;
    }

    if exe_path.is_empty() && args.len() > 1 {
        for arg in &args[1..] {
            if !arg.starts_with('-') {
                exe_path = arg;
                break;
            }
        }
    }

    if debug_mode {
        println!("[RINE ENGINE] Verbose Debug Mode ENABLED (--debug)");
    }

    let allowed_symbols: Vec<String> = vec![
        "version.dll!GetFileVersionInfoW".into(),
        "version.dll!GetFileVersionInfoSizeW".into(),
    ];

    let wine_config = rine::providers::WineConfig {
        enabled: wine_enabled,
        dll_path: wine_dll_path,
        allowed_modules,
        allowed_symbols,
    };
    rine::providers::set_global_wine_config(wine_config);

    // Derive window title from EXE filename (works for any application)
    let exe_stem = Path::new(exe_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Rine Application");
    let window_title = format!("{} (Rine)", exe_stem);

    println!("[Rine] Loading PE binary: {}", exe_path);
    println!("[Rine] Storage strategy: {:?}", strategy);

    let is_ram = matches!(strategy, StorageStrategy::RamOnly { .. });
    rine::sys::RineVfs::init(exe_path, working_dir.as_deref(), is_ram);
    rine::dlls::kernel32::initialize_process_filesystem(exe_path, working_dir.as_deref());

    // Build guest command-line passing trailing CLI arguments to the PE game
    let win_path = rine::dlls::kernel32::resolve_module_path(0);
    let trailing_args = guest_args.join(" ");
    let is_unity_game = Path::new(exe_path)
        .parent()
        .map(|p| p.join("UnityPlayer.dll").exists())
        .unwrap_or(false);
    let mut unity_extra_args = String::new();
    if is_unity_game {
        if !trailing_args.contains("-screen-width") {
            unity_extra_args.push_str(" -screen-width 1920 -screen-height 1080");
        }
        if !trailing_args.contains("-screen-fullscreen") {
            unity_extra_args.push_str(" -screen-fullscreen 0");
        }
    }

    let full_guest_cmd = if trailing_args.is_empty() {
        if unity_extra_args.is_empty() {
            format!("\"{}\"", win_path)
        } else {
            format!("\"{}\"{}", win_path, unity_extra_args)
        }
    } else {
        format!("\"{}\" {}{}", win_path, trailing_args, unity_extra_args)
    };
    rine::dlls::kernel32::set_guest_command_line(&full_guest_cmd);
    println!("[RINE GUEST CMDLINE] Set guest command line: {}", full_guest_cmd);

    // Detect Unity engine generation and select the matching Steam license/API profile
    // before initializing the Steamworks bridge.
    rine::dlls::steam::configure_unity_license_profile(Path::new(exe_path));

    // Initialize native Steam bridge (loads real steam_api64.dll + steamclient.so if present,
    // or warns if original steam_api64.dll is missing)
    rine::dlls::steam_bridge::init_steam_bridge(exe_path, steam_offline);

    // Configure Steam environment if present
    let exe_parent = Path::new(exe_path).parent();
    let mut detected_appid = custom_appid;
    if detected_appid.is_none() {
        if let Some(p) = exe_parent {
            let appid_file = p.join("steam_appid.txt");
            if let Ok(id_str) = fs::read_to_string(&appid_file) {
                let trimmed = id_str.trim().to_string();
                if !trimmed.is_empty() {
                    detected_appid = Some(trimmed);
                }
            }
        }
    }
    // Search up parent folders for appmanifest_*.acf
    if detected_appid.is_none() {
        if let Some(mut cur) = exe_parent {
            for _ in 0..6 {
                if let Some(parent) = cur.parent() {
                    if parent.file_name().map(|n| n == "common").unwrap_or(false) {
                        if let Some(steamapps) = parent.parent() {
                            if let Ok(entries) = fs::read_dir(steamapps) {
                                for entry in entries.flatten() {
                                    let fname = entry.file_name().to_string_lossy().to_string();
                                    if fname.starts_with("appmanifest_") && fname.ends_with(".acf") {
                                        if let Ok(acf) = fs::read_to_string(entry.path()) {
                                            if let Some((aid, _name, dir)) = rine::steam_scanner::parse_acf_file(&acf) {
                                                if exe_path.contains(&dir) {
                                                    detected_appid = Some(aid.to_string());
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    cur = parent;
                } else {
                    break;
                }
            }
        }
    }
    if detected_appid.is_none() && exe_path.contains("How to Fish") {
        detected_appid = Some("4001890".to_string());
    }
    if let Some(appid) = detected_appid {
        println!("[RINE STEAM] Configured SteamAppId={}", appid);
        if let Ok(num) = appid.parse::<u32>() {
            rine::dlls::steam::set_active_app_id(num);
        }
        std::env::set_var("SteamAppId", &appid);
        std::env::set_var("SteamGameId", &appid);
        if let Some(p) = exe_parent {
            let appid_file = p.join("steam_appid.txt");
            if !appid_file.exists() {
                let _ = fs::write(appid_file, &appid);
            }
        }
    }

    // Start background runtime hang detector (10 second stall threshold)
    rine::sys::start_hang_detector(10);

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

    // Load DXVK PE DLLs early so imports in the game PE image can bind to real Vulkan
    // Загрузка DLL DXVK для прямой трансляции Direct3D в Vulkan
    if let Some(dxvk_dir) = rine::dlls::dxvk_loader::find_dxvk_path() {
        rine::dlls::dxvk_loader::preload_dxvk(dxvk_dir);
    } else {
        println!("[RINE MAIN] DXVK not found, D3D11 rendering will use stub fallbacks");
    }

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
            } else if is_unity_game {
                (1920u32, 1080u32)
            } else {
                (1280u32, 720u32)
            };

            // Create host window with target title for display
            // Создание окна хоста с заголовком целевого приложения
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

            let cfg = rine::gui::RineConfig::load();
            if std::env::var_os("DXVK_FRAME_RATE").is_none() && cfg.fps_limit > 0 && cfg.fps_limit < 1000 {
                std::env::set_var("DXVK_FRAME_RATE", cfg.fps_limit.to_string());
                println!("[RINE CONFIG] Applied saved FPS limit: {} FPS", cfg.fps_limit);
            }

            println!("[RINE MAIN] Mapping complete. Dispatching to Win64 entry point...");
            println!(
                "[RINE EXEC] EP=0x{:016X}, PEB=0x{:016X}, TEB=0x{:016X}",
                ep, peb, teb
            );

            // Enforce Landlock LSM Sandbox right before executing guest PE code
            {
                let vfs_guard = rine::sys::RineVfs::get();
                if let Some(ref vfs) = *vfs_guard {
                    let sandbox_cfg = rine::sys::SandboxConfig {
                        drive_c_root: vfs.drive_c_root.clone(),
                        drive_d_root: vfs.drive_d_root.clone(),
                        shm_root: vfs.shm_root.clone(),
                        shader_cache_dir: vfs.shader_cache_dir.clone(),
                    };
                    if let Err(e) = rine::sys::apply_landlock_sandbox(&sandbox_cfg) {
                        eprintln!("[RINE SANDBOX WARNING] Could not enforce Landlock: {}", e);
                    }
                }
            }

            start_resource_telemetry_thread();

            unsafe {
                rine::sys::jump_to_win64_entry_point(ep, peb, teb);
            }
            rine::sys::shutdown::perform_host_cleanup();
            exit(0);
        }
        Err(e) => {
            eprintln!("[Rine] Memory loader error: {}", e);
            exit(1);
        }
    }
}

fn start_resource_telemetry_thread() {
    std::thread::Builder::new()
        .name("rine_telemetry".into())
        .spawn(|| {
            use std::io::Write;
            let file_path = "scratch/postfix_resource_audio_telemetry.tsv";
            let mut file = match std::fs::File::create(file_path) {
                Ok(f) => f,
                Err(_) => return,
            };
            let header = "elapsed_sec\trine_rss_kb\trine_vm_kb\tsystem_mem_available_kb\tswap_used_kb\tthread_count\tfd_count\taudio_device_id\taudio_device_status\taudio_queue_bytes\taudio_queue_frames\tbuffer_size_frames\tpadding_frames\tavailable_frames\tGetBuffer_count_total\tReleaseBuffer_count_total\tReleaseBuffer_per_sec\taudio_bytes_submitted_total\tnonzero_samples_total\tpeak_amplitude_milli\n";
            let _ = file.write_all(header.as_bytes());
            let _ = file.flush();

            let start_time = std::time::Instant::now();
            let mut prev_release_count = 0u64;

            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let elapsed = start_time.elapsed().as_secs();

                // Read proc stats
                let mut rine_vm_kb = 0u64;
                let mut rine_rss_kb = 0u64;
                let mut thread_count = 0u64;
                let mut fd_count = 0usize;

                if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
                    let parts: Vec<&str> = statm.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let page_size_kb = 4u64;
                        rine_vm_kb = parts[0].parse::<u64>().unwrap_or(0) * page_size_kb;
                        rine_rss_kb = parts[1].parse::<u64>().unwrap_or(0) * page_size_kb;
                    }
                }

                if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
                    fd_count = entries.count();
                }

                if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                    for line in status.lines() {
                        if line.starts_with("Threads:") {
                            thread_count = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                        }
                    }
                }

                let mut mem_avail_kb = 0u64;
                let mut swap_used_kb = 0u64;
                let mut swap_total_kb = 0u64;
                let mut swap_free_kb = 0u64;

                if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
                    for line in meminfo.lines() {
                        if line.starts_with("MemAvailable:") {
                            mem_avail_kb = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                        } else if line.starts_with("SwapTotal:") {
                            swap_total_kb = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                        } else if line.starts_with("SwapFree:") {
                            swap_free_kb = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                        }
                    }
                    swap_used_kb = swap_total_kb.saturating_sub(swap_free_kb);
                }

                let (
                    dev_id,
                    dev_status,
                    queued_bytes,
                    queued_frames,
                    buf_frames,
                    padding_frames,
                    avail_frames,
                    get_count,
                    rel_count,
                    bytes_sub,
                    nonzero_samples,
                    peak_amp,
                ) = rine::dlls::wasapi::get_audio_telemetry_snapshot();

                let rel_per_sec = rel_count.saturating_sub(prev_release_count);
                prev_release_count = rel_count;

                let row = format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    elapsed,
                    rine_rss_kb,
                    rine_vm_kb,
                    mem_avail_kb,
                    swap_used_kb,
                    thread_count,
                    fd_count,
                    dev_id,
                    dev_status,
                    queued_bytes,
                    queued_frames,
                    buf_frames,
                    padding_frames,
                    avail_frames,
                    get_count,
                    rel_count,
                    rel_per_sec,
                    bytes_sub,
                    nonzero_samples,
                    peak_amp
                );
                let _ = file.write_all(row.as_bytes());
                let _ = file.flush();

                // Safety guard: if RSS exceeds 4 GB, terminate safely
                if rine_rss_kb > 4_000_000 {
                    eprintln!("[RINE SAFETY GUARD] RSS exceeded 4GB ({} kB). Stopping to prevent host OOM.", rine_rss_kb);
                    std::process::exit(101);
                }
            }
        })
        .ok();
}
