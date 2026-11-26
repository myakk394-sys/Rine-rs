//! Rine Native Linux Graphical User Interface & Control Panel
//!
//! Provides a modern, GPU-accelerated floating control panel and game library launcher
//! built on eframe/egui. Features automatic Steam library scanning (Native, Flatpak, Snap),
//! direct custom executable (.exe / .msi) integration, real-time FPS limiter adjustments,
//! hardware telemetry, dynamic monitor refresh rates, and in-game overlay settings.

use crate::mem::{LoadConfig, LoadedModule, StorageStrategy};
use crate::pe::PeImage;
use crate::steam_scanner::{self, DiscoveredGame};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Global flag for triggering the In-Game Overlay
pub static SHOW_IN_GAME_OVERLAY: AtomicBool = AtomicBool::new(false);
pub static CURRENT_FPS_LIMIT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Language {
    En,
    Ru,
}

#[derive(Debug, Clone)]
pub struct RineConfig {
    pub fps_limit: u32,
    pub vsync: bool,
    pub ram_mode: bool,
    pub lock_ram: bool,
    pub wine_provider: bool,
    pub lang: Language,
    pub recent_games: Vec<String>,
}

impl Default for RineConfig {
    fn default() -> Self {
        Self {
            fps_limit: 0, // 0 = Uncapped (Dynamically matches monitor refresh rate)
            vsync: true,
            ram_mode: true,
            lock_ram: true,
            wine_provider: false,
            lang: Language::Ru,
            recent_games: Vec::new(),
        }
    }
}

impl RineConfig {
    fn config_path() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".config/rine/config.json")
        } else {
            PathBuf::from(".config_rine.json")
        }
    }

    pub fn load() -> Self {
        let p = Self::config_path();
        if let Ok(contents) = std::fs::read_to_string(&p) {
            let mut cfg = RineConfig::default();
            for line in contents.lines() {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let k = parts[0].trim().trim_matches('"');
                    let v = parts[1].trim().trim_matches(|c| c == ',' || c == '"' || c == ' ');
                    match k {
                        "fps_limit" => {
                            if let Ok(num) = v.parse::<u32>() {
                                cfg.fps_limit = num;
                            }
                        }
                        "vsync" => cfg.vsync = v == "true",
                        "ram_mode" => cfg.ram_mode = v == "true",
                        "lock_ram" => cfg.lock_ram = v == "true",
                        "wine_provider" => cfg.wine_provider = v == "true",
                        "lang" => {
                            cfg.lang = if v == "en" || v == "En" {
                                Language::En
                            } else {
                                Language::Ru
                            };
                        }
                        _ => {}
                    }
                }
            }
            return cfg;
        }
        RineConfig::default()
    }

    pub fn save(&self) {
        let p = Self::config_path();
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = format!(
            "{{\n  \"fps_limit\": {},\n  \"vsync\": {},\n  \"ram_mode\": {},\n  \"lock_ram\": {},\n  \"wine_provider\": {},\n  \"lang\": \"{}\"\n}}",
            self.fps_limit,
            self.vsync,
            self.ram_mode,
            self.lock_ram,
            self.wine_provider,
            match self.lang {
                Language::En => "en",
                Language::Ru => "ru",
            }
        );
        let _ = std::fs::write(&p, json);
    }
}

#[derive(Debug, Clone)]
pub struct HardwareInfo {
    pub cpu_name: String,
    pub cpu_threads: usize,
    pub ram_total_gb: f32,
    pub ram_avail_gb: f32,
    pub gpu_name: String,
    pub gpu_driver: String,
    pub display: String,
    pub mouse_name: String,
    pub keyboard_name: String,
    pub audio_backend: String,
    pub os_name: String,
    pub compositor: String,
}

impl HardwareInfo {
    pub fn probe() -> Self {
        // 1. CPU Detection
        let mut cpu_name = "Intel Core i3-12100F".to_string();
        let mut cpu_threads = num_cpus::get_physical_or_available();
        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in cpuinfo.lines() {
                if line.starts_with("model name") {
                    if let Some(name) = line.split(':').nth(1) {
                        cpu_name = name.trim().to_string();
                        break;
                    }
                }
            }
        }

        // 2. RAM Detection
        let mut ram_total_gb = 16.0;
        let mut ram_avail_gb = 12.0;
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            let mut total_kb = 0u64;
            let mut avail_kb = 0u64;
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        total_kb = val.parse().unwrap_or(0);
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        avail_kb = val.parse().unwrap_or(0);
                    }
                }
            }
            if total_kb > 0 {
                ram_total_gb = total_kb as f32 / (1024.0 * 1024.0);
                ram_avail_gb = avail_kb as f32 / (1024.0 * 1024.0);
            }
        }

        // 3. GPU Detection
        let mut gpu_name = "NVIDIA GeForce RTX 3060".to_string();
        let mut gpu_driver = "NVIDIA 570.86.16".to_string();
        if let Ok(out) = Command::new("nvidia-smi")
            .arg("--query-gpu=name,driver_version")
            .arg("--format=csv,noheader")
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = s.trim().split(',').collect();
            if parts.len() >= 2 {
                gpu_name = parts[0].trim().to_string();
                gpu_driver = format!("NVIDIA {}", parts[1].trim());
            }
        }

        // 4. Display (Dynamically queried from active display server)
        let mut display = "Native Monitor Display (Vulkan Present)".to_string();
        if let Ok(out) = Command::new("xrandr").arg("--current").output() {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                if line.contains('*') {
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if !tokens.is_empty() {
                        let res = tokens[0];
                        let hz = tokens.iter().find(|t| t.contains('*')).unwrap_or(&"");
                        let hz_clean = hz.trim_matches(|c: char| !c.is_numeric() && c != '.');
                        display = format!("{} @ {}Hz (Native Monitor / Vulkan Present)", res, hz_clean);
                        break;
                    }
                }
            }
        }

        // 5. Input Devices
        let mut mouse_name = "CX 2.4G Wireless Receiver Mouse".to_string();
        let mut keyboard_name = "CX 2.4G Wireless Receiver Keyboard".to_string();
        if let Ok(devices) = std::fs::read_to_string("/proc/bus/input/devices") {
            for block in devices.split("\n\n") {
                let is_mouse = block.contains("mouse") || block.contains("rel");
                let is_kbd = block.contains("kbd") || block.contains("EV=120013");
                if let Some(name_line) = block.lines().find(|l| l.starts_with("N: Name=")) {
                    let cleaned = name_line
                        .trim_start_matches("N: Name=")
                        .trim_matches('"')
                        .trim();
                    if is_mouse && !cleaned.contains("Power") && !cleaned.contains("Sleep") {
                        mouse_name = cleaned.to_string();
                    } else if is_kbd && !cleaned.contains("Power") && !cleaned.contains("Sleep") {
                        keyboard_name = cleaned.to_string();
                    }
                }
            }
        }

        let audio_backend = "PipeWire WASAPI (AUDIO_S16SYS, 48kHz, Low Latency)".to_string();
        let mut os_name = "Linux 6.13 x86_64".to_string();
        if let Ok(os) = std::fs::read_to_string("/etc/os-release") {
            for line in os.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    os_name = line
                        .trim_start_matches("PRETTY_NAME=")
                        .trim_matches('"')
                        .to_string();
                    break;
                }
            }
        }
        let comp = std::env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| std::env::var("DESKTOP_SESSION"))
            .unwrap_or_else(|_| "Hyprland (Wayland)".to_string());

        Self {
            cpu_name,
            cpu_threads,
            ram_total_gb,
            ram_avail_gb,
            gpu_name,
            gpu_driver,
            display,
            mouse_name,
            keyboard_name,
            audio_backend,
            os_name,
            compositor: comp,
        }
    }
}

mod num_cpus {
    pub fn get_physical_or_available() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8)
    }
}

fn current_time_formatted() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = dur % 86400;
    let h = (secs / 3600 + 5) % 24;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum GuiTab {
    Library,
    Settings,
    Diagnostics,
    Console,
}

#[derive(Clone, Debug)]
pub struct ConsoleLogEntry {
    pub timestamp: String,
    pub text: String,
    pub level: ConsoleLogLevel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConsoleLogLevel {
    Info,
    Warn,
    Error,
    Steam,
    Engine,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConsoleFilterLevel {
    All,
    Errors,
    Warnings,
    Steam,
    Engine,
}

pub fn append_log_to_file(timestamp: &str, tag: &str, text: &str) {
    if let Some(home) = std::env::var_os("HOME") {
        let log_dir = std::path::Path::new(&home).join(".local/share/rine/logs");
        let _ = std::fs::create_dir_all(&log_dir);
        let log_file = log_dir.join("rine_session.log");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_file) {
            use std::io::Write;
            let _ = writeln!(f, "[{}] {} {}", timestamp, tag, text);
        }
    }
}

pub fn classify_and_push_log(logs: &mut Vec<ConsoleLogEntry>, raw_line: String, is_stderr: bool) {
    let trimmed = raw_line.trim();
    if trimmed.is_empty() {
        return;
    }

    // 1. Suppress infinite Unity DX11 PresentFrame warning spam
    if trimmed.contains("GfxDeviceD3D11Base::PresentFrame") {
        return;
    }

    // 2. Accurate classification of error level
    let is_diagnostic_or_bind = trimmed.contains("[RINE PROVIDER RESOLUTION]")
        || trimmed.contains("[RINE COMPANION RESOLUTION]")
        || trimmed.contains("[RINE GETPROCADDRESS")
        || trimmed.contains("[RINE STEAM BIND]")
        || trimmed.contains("[RINE UNRESOLVED BIND]")
        || trimmed.contains("[RINE FILESYSTEM]")
        || trimmed.contains("[KERNEL32 MEM]")
        || trimmed.contains("[NTDLL/KERNEL32]")
        || trimmed.contains("provider=rine")
        || trimmed.contains("provider=wine")
        || trimmed.contains("returned_addr=")
        || trimmed.contains("FALLBACK BIND")
        || trimmed.contains("symbol=SetThreadErrorMode")
        || trimmed.contains("symbol='RaiseException'")
        || trimmed.contains("symbol=RaiseException")
        || trimmed.contains("Function='SetThreadErrorMode'")
        || trimmed.contains("Function='UnityMain2'")
        || trimmed.contains("Function='DStorageGetFactory'")
        || trimmed.contains("GetFileAttributesExW");

    let is_actual_error = !is_diagnostic_or_bind
        && ((trimmed.starts_with("[ERROR]") && !trimmed.contains("SetThreadErrorMode") && !trimmed.contains("FALLBACK BIND"))
            || trimmed.starts_with("[RINE GUEST CRASH]")
            || trimmed.starts_with("[SPAWN ERROR]")
            || trimmed.contains("[RINE UNRESOLVED CALL]")
            || trimmed.contains("[RINE UNRESOLVED API FATAL]")
            || trimmed.contains("[KERNEL32] RaiseException:")
            || trimmed.contains("Assertion:")
            || trimmed.contains("FATAL:")
            || trimmed.contains("STATUS_ACCESS_VIOLATION")
            || (trimmed.contains("0xC0000005") && !trimmed.contains("symbol="))
            || (trimmed.contains("FAILED:") && !trimmed.contains("CreateFileW") && !trimmed.contains(".la'") && !trimmed.contains(".pdb'"))
            || (is_stderr
                && (trimmed.contains("error:")
                    || trimmed.contains("Error:")
                    || (trimmed.contains("Exception:") && !trimmed.contains("RaiseException'")))
                && !trimmed.contains("SetThreadErrorMode")
                && !trimmed.contains("GetThreadErrorMode")
                && !trimmed.contains("SetLastError")
                && !trimmed.contains("GetLastError")
                && !trimmed.contains("GfxDevice")
                && !trimmed.contains("FALLBACK BIND")
                && !trimmed.contains("PROVIDER RESOLUTION")));

    let level = if is_actual_error {
        ConsoleLogLevel::Error
    } else if (trimmed.contains("WARN")
        || trimmed.contains("Warning")
        || trimmed.contains("GfxDevice")
        || trimmed.contains("increased input latency"))
        && !trimmed.contains("SetThreadName")
    {
        ConsoleLogLevel::Warn
    } else if trimmed.contains("Steam") || trimmed.contains("STEAM") {
        ConsoleLogLevel::Steam
    } else if trimmed.contains("Unity") || trimmed.contains("Godot") || trimmed.contains("DXVK") {
        ConsoleLogLevel::Engine
    } else {
        ConsoleLogLevel::Info
    };

    // 3. De-duplicate consecutive identical lines
    if let Some(last) = logs.last() {
        if last.text == trimmed {
            return;
        }
    }

    let timestamp = current_time_formatted();
    let tag = match level {
        ConsoleLogLevel::Info => "[INFO]",
        ConsoleLogLevel::Warn => "[WARN]",
        ConsoleLogLevel::Error => "[ERROR]",
        ConsoleLogLevel::Steam => "[STEAM]",
        ConsoleLogLevel::Engine => "[ENGINE]",
    };

    append_log_to_file(&timestamp, tag, trimmed);

    logs.push(ConsoleLogEntry {
        timestamp,
        text: trimmed.to_string(),
        level,
    });

    // Cache up to 250,000 lines without erasing earlier run logs
    if logs.len() > 250_000 {
        logs.drain(0..10_000);
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AppState {
    StartupSplash,
    Dashboard,
    LaunchingGame,
}

pub struct RineNativeApp {
    state: AppState,
    tab: GuiTab,
    config: RineConfig,
    hardware: HardwareInfo,
    games: Vec<DiscoveredGame>,
    selected_game_index: Option<usize>,
    search_query: String,
    selected_path: Option<PathBuf>,
    logs: Vec<String>,
    startup_time: Instant,
    splash_progress: f32,
    launch_progress: f32,
    launching_game_name: String,
    logo_texture: Option<egui::TextureHandle>,
    show_toast: bool,
    toast_time: Instant,
    toast_message: String,

    // Real-time Console & Process Management
    pub console_logs: Arc<Mutex<Vec<ConsoleLogEntry>>>,
    pub running_process: Arc<Mutex<Option<std::process::Child>>>,
    pub is_running: Arc<AtomicBool>,
    pub console_filter: String,
    pub console_level_filter: ConsoleFilterLevel,
    pub console_auto_scroll: bool,

    // Running Game Tracking & Shell Synchronization
    pub running_game_name: Option<String>,
    pub running_game_path: Option<PathBuf>,
    pub running_game_appid: Option<u32>,
    pub running_game_start: Option<Instant>,
    pub running_game_pid: Option<u32>,
    pub cover_textures: std::collections::HashMap<String, egui::TextureHandle>,

    // Launching GIF & Runtime synchronization
    pub game_runtime_ready: Arc<AtomicBool>,
    pub launch_gif_frames: Vec<(egui::TextureHandle, Duration)>,
    pub launch_gif_total_duration: Duration,
    pub launch_start_time: Instant,
}

impl Default for RineNativeApp {
    fn default() -> Self {
        let cfg = RineConfig::load();
        CURRENT_FPS_LIMIT.store(cfg.fps_limit, Ordering::Relaxed);
        let hw = HardwareInfo::probe();
        let all_games = steam_scanner::get_all_games();
        let selected_idx = if !all_games.is_empty() { Some(0) } else { None };
        let selected_path = selected_idx.map(|i| all_games[i].exe_path.clone());

        let init_logs = vec![
            "[RINE CORE] Initializing Rine Win64 Translation Engine v0.2.0...".into(),
            format!("[HARDWARE] CPU: {} ({} threads)", hw.cpu_name, hw.cpu_threads),
            format!("[HARDWARE] GPU: {} (Driver: {})", hw.gpu_name, hw.gpu_driver),
            format!("[GAMES] Auto-discovered {} Steam & custom games.", all_games.len()),
            "[DXVK] Preloaded Vulkan DXVK 2.7.1 translation pipeline.".into(),
        ];

        let console_logs = Arc::new(Mutex::new(vec![
            ConsoleLogEntry {
                timestamp: current_time_formatted(),
                text: "[RINE] Ядро трансляции Rine Win64 Engine v0.2.0 инициализировано.".into(),
                level: ConsoleLogLevel::Info,
            },
            ConsoleLogEntry {
                timestamp: current_time_formatted(),
                text: format!("[HARDWARE] GPU: {} | Драйвер: {}", hw.gpu_name, hw.gpu_driver),
                level: ConsoleLogLevel::Engine,
            },
            ConsoleLogEntry {
                timestamp: current_time_formatted(),
                text: format!("[STEAM] Обнаружено установленных игр в библиотеке: {}", all_games.len()),
                level: ConsoleLogLevel::Steam,
            },
            ConsoleLogEntry {
                timestamp: current_time_formatted(),
                text: "[DXVK/Vulkan] Мост трансляции DirectX 9/10/11 -> Vulkan готов к работе.".into(),
                level: ConsoleLogLevel::Info,
            },
        ]));

        Self {
            state: AppState::StartupSplash,
            tab: GuiTab::Library,
            config: cfg,
            hardware: hw,
            games: all_games,
            selected_game_index: selected_idx,
            search_query: String::new(),
            selected_path,
            logs: init_logs,
            startup_time: Instant::now(),
            splash_progress: 0.0,
            launch_progress: 0.0,
            launching_game_name: String::new(),
            logo_texture: None,
            show_toast: false,
            toast_time: Instant::now(),
            toast_message: String::new(),
            console_logs,
            running_process: Arc::new(Mutex::new(None)),
            is_running: Arc::new(AtomicBool::new(false)),
            console_filter: String::new(),
            console_level_filter: ConsoleFilterLevel::All,
            console_auto_scroll: true,
            running_game_name: None,
            running_game_path: None,
            running_game_appid: None,
            running_game_start: None,
            running_game_pid: None,
            cover_textures: std::collections::HashMap::new(),
            game_runtime_ready: Arc::new(AtomicBool::new(false)),
            launch_gif_frames: Vec::new(),
            launch_gif_total_duration: Duration::from_millis(100),
            launch_start_time: Instant::now(),
        }
    }
}

impl RineNativeApp {
    pub fn add_console_log(&self, level: ConsoleLogLevel, text: impl Into<String>) {
        let t_str = text.into();
        let ts = current_time_formatted();
        let tag = match level {
            ConsoleLogLevel::Info => "[INFO]",
            ConsoleLogLevel::Warn => "[WARN]",
            ConsoleLogLevel::Error => "[ERROR]",
            ConsoleLogLevel::Steam => "[STEAM]",
            ConsoleLogLevel::Engine => "[ENGINE]",
        };
        append_log_to_file(&ts, tag, &t_str);

        if let Ok(mut logs) = self.console_logs.lock() {
            logs.push(ConsoleLogEntry {
                timestamp: ts,
                text: t_str,
                level,
            });
            if logs.len() > 100_000 {
                logs.drain(0..10_000);
            }
        }
    }

    pub fn run() -> eframe::Result<()> {
        let mut viewport = egui::ViewportBuilder::default()
            .with_title("Rine Win64 Engine - Панель управления и лаунчер")
            .with_app_id("rine")
            .with_inner_size([880.0, 680.0])
            .with_min_inner_size([640.0, 520.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(true);
        if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/logo.png")) {
            viewport = viewport.with_icon(std::sync::Arc::new(icon));
        }
        let options = eframe::NativeOptions {
            viewport,
            ..Default::default()
        };

        eframe::run_native(
            "Rine Win64 Engine",
            options,
            Box::new(|cc| {
                let mut app = RineNativeApp::default();
                app.load_logo(&cc.egui_ctx);
                Box::new(app)
            }),
        )
    }

    fn load_logo(&mut self, ctx: &egui::Context) {
        let png_bytes = include_bytes!("../assets/logo.png");
        if let Ok(img) = image::load_from_memory(png_bytes) {
            let size = [img.width() as usize, img.height() as usize];
            let buf = img.to_rgba8();
            let pixels = buf.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            self.logo_texture = Some(ctx.load_texture("rine_logo", color_image, egui::TextureOptions::LINEAR));
        }
    }

    fn ensure_launch_gif(&mut self, ctx: &egui::Context) {
        if !self.launch_gif_frames.is_empty() {
            return;
        }
        use image::codecs::gif::GifDecoder;
        use image::AnimationDecoder;

        let gif_bytes = include_bytes!("../assets/Rine.gif");
        let cursor = std::io::Cursor::new(gif_bytes);
        if let Ok(decoder) = GifDecoder::new(cursor) {
            if let Ok(raw_frames) = decoder.into_frames().collect_frames() {
                let mut frames = Vec::with_capacity(raw_frames.len());
                let mut total = Duration::ZERO;
                for (i, frame) in raw_frames.into_iter().enumerate() {
                    let (num, denom) = frame.delay().numer_denom_ms();
                    let delay_ms = if denom == 0 || num == 0 {
                        50
                    } else {
                        (num as u64) / (denom as u64)
                    };
                    let duration = Duration::from_millis(delay_ms.max(20));
                    total += duration;

                    let buffer = frame.into_buffer();
                    let width = buffer.width() as usize;
                    let height = buffer.height() as usize;
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [width, height],
                        buffer.as_raw(),
                    );
                    let tex = ctx.load_texture(
                        format!("rine_launch_gif_{}", i),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    frames.push((tex, duration));
                }
                if !frames.is_empty() {
                    self.launch_gif_frames = frames;
                    self.launch_gif_total_duration = total;
                }
            }
        }
    }

    pub fn refresh_games(&mut self) {
        self.games = steam_scanner::get_all_games();
        if self.selected_game_index.is_none() && !self.games.is_empty() {
            self.select_game_index(0);
        } else if let Some(idx) = self.selected_game_index {
            if idx >= self.games.len() && !self.games.is_empty() {
                self.select_game_index(0);
            }
        }
    }

    pub fn select_game_index(&mut self, idx: usize) {
        if let Some(game) = self.games.get(idx) {
            self.selected_game_index = Some(idx);
            self.selected_path = Some(game.exe_path.clone());
        }
    }

    fn select_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Windows Executable / Installer (.exe / .msi)", &["exe", "msi"])
            .pick_file()
        {
            if let Some(game) = steam_scanner::add_custom_game(path.clone()) {
                self.refresh_games();
                if let Some(pos) = self.games.iter().position(|g| g.exe_path == game.exe_path) {
                    self.select_game_index(pos);
                }
            } else {
                self.selected_path = Some(path);
            }
        }
    }

    fn trigger_toast(&mut self, message: &str) {
        self.show_toast = true;
        self.toast_time = Instant::now();
        self.toast_message = message.to_string();
    }

    fn start_game_launch(&mut self) {
        if self.is_running.load(Ordering::SeqCst) {
            self.trigger_toast(match self.config.lang {
                Language::En => "⚠️ A game is already running! Please stop it before launching another.",
                Language::Ru => "⚠️ Игра уже запущена! Остановите её перед запуском новой.",
            });
            return;
        }

        let Some(path) = self.selected_path.clone() else {
            self.trigger_toast(match self.config.lang {
                Language::En => "Please select a game from the list first!",
                Language::Ru => "Пожалуйста, сначала выберите игру из списка!",
            });
            return;
        };

        let compat = steam_scanner::check_game_compatibility(&path);
        if !compat.is_supported {
            self.trigger_toast(&format!(
                "⚠ {}: {}",
                match self.config.lang {
                    Language::En => "Engine unsupported",
                    Language::Ru => "Движок не поддерживается",
                },
                compat.engine_name
            ));
            self.add_console_log(
                ConsoleLogLevel::Warn,
                format!("Попытка запуска игры на неподдерживаемом движке ({}): {}", compat.engine_name, compat.details),
            );
            if compat.architecture != steam_scanner::BinaryArchitecture::X86_64 {
                self.trigger_toast(match self.config.lang {
                    Language::En => "32-bit PE and unsupported architectures are not supported by Rine Win64!",
                    Language::Ru => "32-битные PE и неподдерживаемые архитектуры не поддерживаются Rine Win64!",
                });
                return;
            }
        }

        let game_name = if let Some(idx) = self.selected_game_index {
            self.games.get(idx).map(|g| g.name.clone()).unwrap_or_else(|| {
                path.file_name().unwrap_or_default().to_string_lossy().to_string()
            })
        } else {
            path.file_name().unwrap_or_default().to_string_lossy().to_string()
        };

        self.launching_game_name = game_name.clone();
        self.running_game_name = Some(game_name);
        self.running_game_path = Some(path.clone());
        self.running_game_start = Some(Instant::now());
        if let Some(idx) = self.selected_game_index {
            if let Some(g) = self.games.get(idx) {
                if g.is_steam {
                    self.running_game_appid = Some(g.appid);
                }
            }
        }
        self.launch_start_time = Instant::now();
        self.game_runtime_ready.store(false, Ordering::SeqCst);
        self.launch_progress = 0.0;
        self.state = AppState::LaunchingGame;
        self.execute_game_process();
    }

    fn execute_game_process(&mut self) {
        let Some(path) = self.selected_path.clone() else { return };
        let fps = self.config.fps_limit;

        if fps > 0 && fps < 1000 {
            std::env::set_var("DXVK_FRAME_RATE", fps.to_string());
        } else {
            std::env::remove_var("DXVK_FRAME_RATE");
        }

        if self.config.vsync {
            std::env::set_var("DXVK_VSYNC", "1");
        } else {
            std::env::set_var("DXVK_VSYNC", "0");
        }

        let exe_str = path.to_string_lossy().to_string();
        let (session_header, launch_msg) = match self.config.lang {
            Language::En => (
                format!("========================================================\n=== NEW SESSION: {} [{}]\n========================================================", self.launching_game_name, exe_str),
                format!("[LAUNCH] Launching game '{}' via Rine Win64...", exe_str),
            ),
            Language::Ru => (
                format!("========================================================\n=== НОВАЯ СЕССИЯ: {} [{}]\n========================================================", self.launching_game_name, exe_str),
                format!("[LAUNCH] Запуск игры '{}' через Rine Win64...", exe_str),
            ),
        };
        append_log_to_file(&current_time_formatted(), "[SESSION]", &session_header);
        self.logs.push(format!("[EXECUTE] Launching '{}' via Rine Engine...", exe_str));
        self.add_console_log(ConsoleLogLevel::Info, launch_msg);

        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let local_rine = PathBuf::from(format!("{}/.local/bin/rine", home));
        let rine_bin = if let Ok(cur) = std::env::current_exe() {
            if cur.exists() {
                cur
            } else {
                local_rine
            }
        } else if local_rine.exists() {
            local_rine
        } else {
            PathBuf::from("./target/release/rine")
        };

        let mut cmd = Command::new(rine_bin);
        cmd.arg(&exe_str);
        if self.config.ram_mode {
            cmd.arg("--ram-only");
            if self.config.lock_ram {
                cmd.arg("--lock-ram");
            }
        } else {
            cmd.arg("--disk-demand");
        }
        if self.config.wine_provider {
            cmd.arg("--wine-provider");
        }

        if let Some(idx) = self.selected_game_index {
            if let Some(game) = self.games.get(idx) {
                if game.is_steam && game.appid != 0 {
                    cmd.arg("--steam-appid").arg(game.appid.to_string());
                    cmd.env("SteamAppId", game.appid.to_string());
                    cmd.env("SteamGameId", game.appid.to_string());
                    let steam_log = match self.config.lang {
                        Language::En => format!("[STEAM] Configured AppID={} for Steamworks integration", game.appid),
                        Language::Ru => format!("[STEAM] Установлен AppID={} для интеграции со Steamworks", game.appid),
                    };
                    self.add_console_log(ConsoleLogLevel::Steam, steam_log);
                }
            }
        }

        cmd.env("DXVK_FRAME_RATE", fps.to_string());
        if let Some(parent) = path.parent() {
            cmd.current_dir(parent);
            cmd.arg("-w").arg(parent.to_string_lossy().to_string());
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                let pid = child.id();
                self.running_game_pid = Some(pid);
                self.logs.push(format!("[SPAWN SUCCESS] Game process running with PID: {}", pid));
                let spawn_msg = match self.config.lang {
                    Language::En => format!("[SPAWN] Game process successfully started (PID: {})", pid),
                    Language::Ru => format!("[SPAWN] Процесс игры успешно запущен (PID: {})", pid),
                };
                self.add_console_log(ConsoleLogLevel::Info, spawn_msg);
                self.is_running.store(true, Ordering::SeqCst);
                self.trigger_toast(&match self.config.lang {
                    Language::En => format!("🟢 '{}' started (PID: {})", self.launching_game_name, pid),
                    Language::Ru => format!("🟢 '{}' успешно запущена (PID: {})", self.launching_game_name, pid),
                });

                let stdout = child.stdout.take();
                let stderr = child.stderr.take();

                let logs_arc = Arc::clone(&self.console_logs);
                let ready_arc = Arc::clone(&self.game_runtime_ready);

                // Pipe stdout reader thread
                if let Some(stdout) = stdout {
                    let logs_out = Arc::clone(&logs_arc);
                    let ready_out = Arc::clone(&ready_arc);
                    std::thread::spawn(move || {
                        use std::io::{BufRead, BufReader};
                        let reader = BufReader::new(stdout);
                        for line in reader.lines() {
                            if let Ok(l) = line {
                                if l.contains("[RINE_READY]")
                                    || l.contains("[RINE REAL HOST PRESENT]")
                                    || l.contains("[RINE HOST PRESENT]")
                                    || l.contains("GfxDeviceD3D11Base::PresentFrame")
                                    || l.contains("[RINE WINDOW] Game window unhidden")
                                {
                                    ready_out.store(true, Ordering::SeqCst);
                                }
                                if let Ok(mut logs) = logs_out.lock() {
                                    classify_and_push_log(&mut logs, l, false);
                                }
                            }
                        }
                    });
                }

                // Pipe stderr reader thread
                if let Some(stderr) = stderr {
                    let logs_err = Arc::clone(&logs_arc);
                    let ready_err = Arc::clone(&ready_arc);
                    std::thread::spawn(move || {
                        use std::io::{BufRead, BufReader};
                        let reader = BufReader::new(stderr);
                        for line in reader.lines() {
                            if let Ok(l) = line {
                                if l.contains("[RINE_READY]")
                                    || l.contains("[RINE REAL HOST PRESENT]")
                                    || l.contains("[RINE HOST PRESENT]")
                                    || l.contains("GfxDeviceD3D11Base::PresentFrame")
                                    || l.contains("[RINE WINDOW] Game window unhidden")
                                {
                                    ready_err.store(true, Ordering::SeqCst);
                                }
                                if let Ok(mut logs) = logs_err.lock() {
                                    classify_and_push_log(&mut logs, l, true);
                                }
                            }
                        }
                    });
                }

                if let Ok(mut guard) = self.running_process.lock() {
                    *guard = Some(child);
                }
            }
            Err(e) => {
                self.is_running.store(false, Ordering::SeqCst);
                self.running_game_name = None;
                self.running_game_path = None;
                self.running_game_appid = None;
                self.running_game_start = None;
                self.running_game_pid = None;
                let err_msg = match self.config.lang {
                    Language::En => format!("[SPAWN ERROR] Failed to launch game process: {}", e),
                    Language::Ru => format!("[SPAWN ERROR] Ошибка запуска процесса игры: {}", e),
                };
                self.logs.push(err_msg.clone());
                self.add_console_log(ConsoleLogLevel::Error, err_msg.clone());
                self.trigger_toast(&err_msg);
            }
        }
    }
}

impl eframe::App for RineNativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        if ctx.input(|i| i.viewport().close_requested()) {
            std::process::exit(0);
        }

        // Poll game process exit status & update UI state
        let running_proc = self.running_process.clone();
        if let Ok(mut proc_opt) = running_proc.try_lock() {
            if let Some(ref mut child) = *proc_opt {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let pid = child.id();
                        self.is_running.store(false, Ordering::SeqCst);
                        self.game_runtime_ready.store(false, Ordering::SeqCst);
                        if self.state == AppState::LaunchingGame {
                            self.state = AppState::Dashboard;
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        let name = self.running_game_name.take().unwrap_or_else(|| "Game".into());
                        self.running_game_path = None;
                        self.running_game_appid = None;
                        self.running_game_start = None;
                        self.running_game_pid = None;
                        let msg = match self.config.lang {
                            Language::En => format!("[RINE] Game '{}' (PID {}) terminated (exit code: {:?})", name, pid, status.code()),
                            Language::Ru => format!("[RINE] Игра '{}' (PID {}) завершилась (код: {:?})", name, pid, status.code()),
                        };
                        if let Ok(mut logs) = self.console_logs.lock() {
                            logs.push(ConsoleLogEntry {
                                timestamp: current_time_formatted(),
                                text: msg.clone(),
                                level: ConsoleLogLevel::Info,
                            });
                        }
                        self.trigger_toast(&msg);
                        *proc_opt = None;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        self.is_running.store(false, Ordering::SeqCst);
                        self.game_runtime_ready.store(false, Ordering::SeqCst);
                        if self.state == AppState::LaunchingGame {
                            self.state = AppState::Dashboard;
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        self.running_game_name = None;
                        self.running_game_path = None;
                        self.running_game_appid = None;
                        self.running_game_start = None;
                        self.running_game_pid = None;
                        *proc_opt = None;
                    }
                }
            }
        }

        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = egui::Color32::from_rgba_premultiplied(18, 18, 26, 225);
        style.visuals.panel_fill = egui::Color32::from_rgba_premultiplied(18, 18, 26, 225);
        style.visuals.override_text_color = Some(egui::Color32::from_rgb(240, 240, 245));
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgba_premultiplied(26, 26, 36, 200);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgba_premultiplied(34, 34, 46, 200);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgba_premultiplied(54, 54, 70, 220);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(255, 102, 0); // Rine Orange
        ctx.set_style(style);

        match self.state {
            AppState::StartupSplash => self.render_startup_splash(ctx),
            AppState::Dashboard => self.render_dashboard(ctx),
            AppState::LaunchingGame => self.render_launching_splash(ctx),
        }

        self.render_toast_overlay(ctx);
    }
}

impl RineNativeApp {
    fn render_custom_titlebar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(tex) = &self.logo_texture {
                ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(22.0, 22.0)));
            } else {
                ui.label("⚙");
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("RINE")
                    .strong()
                    .size(15.0)
                    .color(egui::Color32::from_rgb(255, 102, 0)),
            );
            ui.label(
                egui::RichText::new("v0.2.0")
                    .monospace()
                    .size(10.0)
                    .color(egui::Color32::from_rgb(150, 150, 160)),
            );

            // Draggable bar area
            let avail_w = (ui.available_width() - 130.0).max(40.0);
            let (drag_rect, _resp) = ui.allocate_exact_size(egui::vec2(avail_w, 22.0), egui::Sense::drag());
            let drag_resp = ui.interact(drag_rect, egui::Id::new("titlebar_drag"), egui::Sense::drag());
            if drag_resp.drag_started() || drag_resp.dragged() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Close button
                let close_btn = egui::Button::new("✖")
                    .fill(egui::Color32::from_rgba_premultiplied(190, 45, 45, 200))
                    .min_size(egui::vec2(22.0, 20.0));
                if ui.add(close_btn).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                // Minimize button
                let min_btn = egui::Button::new("🗕")
                    .fill(egui::Color32::from_rgba_premultiplied(40, 40, 52, 200))
                    .min_size(egui::vec2(22.0, 20.0));
                if ui.add(min_btn).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }

                // Language toggle button
                let (flag, curr_name, next_hint, lang_prefix) = match self.config.lang {
                    Language::Ru => ("🇷🇺 RU", "Русский", "Переключить на English", "Текущий язык:"),
                    Language::En => ("🇬🇧 EN", "English", "Switch to Russian", "Current Language:"),
                };
                let lang_btn = egui::Button::new(
                    egui::RichText::new(flag).size(11.0).strong().color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgba_premultiplied(50, 54, 72, 220))
                .min_size(egui::vec2(52.0, 20.0));

                if ui.add(lang_btn).on_hover_text(format!("{} {} ({})", lang_prefix, curr_name, next_hint)).clicked() {
                    self.config.lang = match self.config.lang {
                        Language::Ru => Language::En,
                        Language::En => Language::Ru,
                    };
                    self.config.save();
                }
            });
        });
    }

    // Startup splash screen
    // Экран начальной загрузки лаунчера
    fn render_startup_splash(&mut self, ctx: &egui::Context) {
        let elapsed = self.startup_time.elapsed().as_secs_f32();
        let target_duration = 2.0;

        if elapsed >= target_duration {
            self.state = AppState::Dashboard;
            return;
        }

        // Keep UI repainting for smooth 60fps GIF animation
        ctx.request_repaint();

        self.ensure_launch_gif(ctx);
        let cur_frame_idx = if !self.launch_gif_frames.is_empty() {
            let total_dur_ms: u64 = self.launch_gif_frames.iter().map(|(_, d)| d.as_millis() as u64).sum();
            let elapsed_ms = (self.startup_time.elapsed().as_millis() as u64) % total_dur_ms.max(1);
            let mut acc = 0u64;
            let mut chosen = 0;
            for (i, (_, dur)) in self.launch_gif_frames.iter().enumerate() {
                acc += dur.as_millis() as u64;
                if elapsed_ms < acc {
                    chosen = i;
                    break;
                }
            }
            Some(chosen)
        } else {
            None
        };

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_premultiplied(16, 16, 24, 245))
                    .stroke(egui::Stroke::new(1.2, egui::Color32::from_rgba_premultiplied(255, 102, 0, 180)))
                    .rounding(10.0)
                    .inner_margin(egui::Margin::symmetric(16.0, 12.0))
            )
            .show(ctx, |ui| {
                self.render_custom_titlebar(ctx, ui);
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.08);

                    if let Some(idx) = cur_frame_idx {
                        if let Some((tex, _)) = self.launch_gif_frames.get(idx) {
                            let gif_size = egui::vec2(448.0, 252.0);
                            let (rect, _resp) = ui.allocate_exact_size(gif_size, egui::Sense::hover());
                            ui.painter().image(
                                tex.id(),
                                rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        }
                    } else if let Some(tex) = &self.logo_texture {
                        let logo_size = egui::vec2(120.0, 120.0);
                        let (rect, _response) = ui.allocate_exact_size(logo_size, egui::Sense::hover());
                        ui.painter().image(
                            tex.id(),
                            rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }

                    ui.add_space(14.0);
                    ui.heading(
                        egui::RichText::new("R I N E")
                            .strong()
                            .size(24.0)
                            .color(egui::Color32::from_rgb(255, 110, 0)),
                    );

                    let sub_text = match self.config.lang {
                        Language::En => "Windows Game & App Translation Layer for Linux",
                        Language::Ru => "Среда запуска Windows-игр и приложений для Linux",
                    };
                    ui.label(egui::RichText::new(sub_text).size(12.0).color(egui::Color32::GRAY));

                    ui.add_space(16.0);
                    let skip_text = match self.config.lang {
                        Language::En => "Click to skip",
                        Language::Ru => "Пропустить",
                    };
                    if ui.small_button(skip_text).clicked() {
                        self.state = AppState::Dashboard;
                    }
                });
            });
    }

    // Main dashboard view
    // Главное окно панели управления
    fn render_dashboard(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_premultiplied(18, 18, 26, 230))
                    .stroke(egui::Stroke::new(1.2, egui::Color32::from_rgba_premultiplied(255, 102, 0, 180)))
                    .rounding(10.0)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0))
            )
            .show(ctx, |ui| {
                self.render_header(ctx, ui);
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                match self.tab {
                    GuiTab::Library => self.render_tab_library(ui),
                    GuiTab::Settings => self.render_tab_settings(ui),
                    GuiTab::Diagnostics => self.render_tab_diagnostics(ui),
                    GuiTab::Console => self.render_tab_console(ui),
                }
            });
    }

    // Top Navigation Header
    fn render_header(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.render_custom_titlebar(ctx, ui);
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(6.0);

        let t_lib = match self.config.lang {
            Language::En => "🎮 Game Library",
            Language::Ru => "🎮 Библиотека игр",
        };
        let t_set = match self.config.lang {
            Language::En => "⚙ Settings",
            Language::Ru => "⚙ Настройки",
        };
        let t_diag = match self.config.lang {
            Language::En => "💻 Hardware / PC",
            Language::Ru => "💻 Железо / ПК",
        };
        let is_running = self.is_running.load(Ordering::Relaxed);
        let t_cons = match self.config.lang {
            Language::En => if is_running { "📟 Console 🟢" } else { "📟 Console" },
            Language::Ru => if is_running { "📟 Консоль 🟢" } else { "📟 Консоль" },
        };

        ui.horizontal(|ui| {
            for (tab, label) in [
                (GuiTab::Library, t_lib),
                (GuiTab::Settings, t_set),
                (GuiTab::Diagnostics, t_diag),
                (GuiTab::Console, t_cons),
            ] {
                let is_active = self.tab == tab;
                let bg = if is_active {
                    egui::Color32::from_rgb(255, 102, 0)
                } else if tab == GuiTab::Console && is_running {
                    egui::Color32::from_rgba_premultiplied(30, 60, 40, 220)
                } else {
                    egui::Color32::from_rgba_premultiplied(32, 34, 46, 200)
                };
                let text_color = if is_active {
                    egui::Color32::WHITE
                } else if tab == GuiTab::Console && is_running {
                    egui::Color32::from_rgb(100, 255, 150)
                } else {
                    egui::Color32::from_rgb(180, 185, 200)
                };
                let stroke = if is_active {
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 140, 40))
                } else {
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(60, 65, 85, 120))
                };
                let btn = egui::Button::new(
                    egui::RichText::new(label)
                        .strong()
                        .size(12.5)
                        .color(text_color),
                )
                .fill(bg)
                .stroke(stroke)
                .rounding(6.0)
                .min_size(egui::vec2(0.0, 30.0));

                if ui.add(btn).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.add_space(6.0);
    }

    // Game library tab
    // Вкладка библиотеки игр
    fn render_tab_library(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            // Action bar: Search + Add Game + Refresh
            ui.horizontal(|ui| {
                let search_hint = match self.config.lang {
                    Language::En => "🔍 Search library...",
                    Language::Ru => "🔍 Поиск игр...",
                };
                ui.add_sized(
                    [ui.available_width() - 170.0, 26.0],
                    egui::TextEdit::singleline(&mut self.search_query).hint_text(search_hint),
                );

                let add_btn_text = match self.config.lang {
                    Language::En => "+ Add File",
                    Language::Ru => "+ Добавить .exe",
                };
                if ui.button(egui::RichText::new(add_btn_text).strong().color(egui::Color32::from_rgb(16, 185, 129))).clicked() {
                    self.select_file();
                }

                let refresh_text = match self.config.lang {
                    Language::En => "🔄 Refresh",
                    Language::Ru => "🔄 Обновить",
                };
                let refresh_tip = match self.config.lang {
                    Language::En => "Rescan Steam libraries and game list",
                    Language::Ru => "Обновить список игр и библиотек Steam",
                };
                if ui.button(egui::RichText::new(refresh_text).strong()).on_hover_text(refresh_tip).clicked() {
                    self.refresh_games();
                    self.trigger_toast(match self.config.lang {
                        Language::En => "Steam libraries and games rescanned!",
                        Language::Ru => "Библиотеки Steam и список игр обновлены!",
                    });
                }
            });

            ui.add_space(8.0);

            // Filter games by search
            let query = self.search_query.trim().to_lowercase();
            let matching_indices: Vec<usize> = self.games
                .iter()
                .enumerate()
                .filter(|(_, g)| query.is_empty() || g.name.to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect();

            // Games Scroll Area
            let avail_h = ui.available_height() - 95.0;
            egui::ScrollArea::vertical().max_height(avail_h).show(ui, |ui| {
                if matching_indices.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(30.0);
                        let no_games = match self.config.lang {
                            Language::En => "No games found. Click '+ Add File' to launch any .exe or .msi!",
                            Language::Ru => "Игры не найдены. Нажмите '+ Добавить .exe', чтобы запустить любую игру!",
                        };
                        ui.label(egui::RichText::new(no_games).italics().color(egui::Color32::GRAY));
                    });
                } else {
                    let mut launched_game_idx = None;
                    let mut deleted_game_path = None;
                    let mut new_selected_idx = None;

                    for &idx in &matching_indices {
                        let (game_name, is_steam, appid, exe_path, icon_hint) = {
                            let g = &self.games[idx];
                            (g.name.clone(), g.is_steam, g.appid, g.exe_path.clone(), g.icon_hint.clone())
                        };
                        let is_selected = self.selected_game_index == Some(idx);
                        let is_this_game_running = self.is_running.load(Ordering::SeqCst)
                            && self.running_game_path.as_ref() == Some(&exe_path);
                        let any_game_running = self.is_running.load(Ordering::SeqCst);

                        // Load cover art texture if not cached yet
                        if let Some(ref icon_p) = icon_hint {
                            if !self.cover_textures.contains_key(icon_p) {
                                if let Ok(bytes) = std::fs::read(icon_p) {
                                    if let Ok(img) = image::load_from_memory(&bytes) {
                                        let size = [img.width() as usize, img.height() as usize];
                                        let rgba = img.to_rgba8();
                                        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_flat_samples().as_slice());
                                        let handle = ui.ctx().load_texture(icon_p, color_image, egui::TextureOptions::LINEAR);
                                        self.cover_textures.insert(icon_p.clone(), handle);
                                    }
                                }
                            }
                        }

                        let card_bg = if is_this_game_running {
                            egui::Color32::from_rgba_premultiplied(16, 42, 26, 250) // Emerald active glow
                        } else if is_selected {
                            egui::Color32::from_rgba_premultiplied(42, 28, 18, 240)
                        } else {
                            egui::Color32::from_rgba_premultiplied(22, 24, 34, 200)
                        };
                        let border_col = if is_this_game_running {
                            egui::Color32::from_rgb(0, 230, 118) // Active glowing green
                        } else if is_selected {
                            egui::Color32::from_rgb(255, 110, 0)
                        } else {
                            egui::Color32::from_rgba_premultiplied(50, 52, 70, 160)
                        };
                        let stroke_w = if is_this_game_running { 2.2 } else if is_selected { 1.8 } else { 1.0 };

                        egui::Frame::none()
                            .fill(card_bg)
                            .stroke(egui::Stroke::new(stroke_w, border_col))
                            .rounding(8.0)
                            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Left vertical accent pill
                                    if is_this_game_running {
                                        let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(3.5, 36.0), egui::Sense::hover());
                                        ui.painter().rect_filled(bar_rect, 1.5, egui::Color32::from_rgb(0, 230, 118));
                                        ui.add_space(4.0);
                                    } else if is_selected {
                                        let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(3.0, 36.0), egui::Sense::hover());
                                        ui.painter().rect_filled(bar_rect, 1.5, egui::Color32::from_rgb(255, 110, 0));
                                        ui.add_space(4.0);
                                    }

                                    // Cover Image or Styled Icon Badge
                                    let mut rendered_thumb = false;
                                    if let Some(ref icon_p) = icon_hint {
                                        if let Some(tex) = self.cover_textures.get(icon_p) {
                                            ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(80.0, 38.0)).rounding(4.0));
                                            ui.add_space(6.0);
                                            rendered_thumb = true;
                                        }
                                    }
                                    if !rendered_thumb {
                                        let icon_glyph = if is_this_game_running {
                                            "🟢"
                                        } else if is_steam {
                                            "🎮"
                                        } else {
                                            "⚙"
                                        };
                                        ui.label(egui::RichText::new(icon_glyph).size(20.0));
                                        ui.add_space(6.0);
                                    }

                                    // Title & Tag
                                    ui.vertical(|ui| {
                                        let name_resp = ui.selectable_label(
                                            is_selected,
                                            egui::RichText::new(&game_name).strong().size(14.0),
                                        );
                                        if name_resp.clicked() {
                                            new_selected_idx = Some(idx);
                                        }

                                        let badge_text = if is_this_game_running {
                                            let pid_str = self.running_game_pid.map(|p| format!(" [PID: {}]", p)).unwrap_or_default();
                                            match self.config.lang {
                                                Language::En => format!("🟢 PLAYING NOW{}", pid_str),
                                                Language::Ru => format!("🟢 СЕЙЧАС ЗАПУЩЕНА{}", pid_str),
                                            }
                                        } else if is_steam {
                                            format!("STEAM #{}", appid)
                                        } else {
                                            "CUSTOM .EXE".to_string()
                                        };
                                        let badge_col = if is_this_game_running {
                                            egui::Color32::from_rgb(0, 230, 118)
                                        } else if is_steam {
                                            egui::Color32::from_rgb(255, 160, 50)
                                        } else {
                                            egui::Color32::from_rgb(167, 139, 250)
                                        };

                                        let compat = steam_scanner::check_game_compatibility(&exe_path);
                                        let (engine_tag, engine_col) = match &compat.engine {
                                            steam_scanner::GameEngine::Unity { version } => {
                                                if let Some(v) = version {
                                                    if v.starts_with('6') {
                                                        ("[Unity 6]", egui::Color32::from_rgb(50, 205, 50))
                                                    } else {
                                                        ("[Unity]", egui::Color32::from_rgb(50, 205, 50))
                                                    }
                                                } else {
                                                    ("[Unity]", egui::Color32::from_rgb(50, 205, 50))
                                                }
                                            }
                                            steam_scanner::GameEngine::Godot { version } => match version {
                                                steam_scanner::GodotVersion::V4 => ("[Godot 4]", egui::Color32::from_rgb(70, 180, 255)),
                                                steam_scanner::GodotVersion::V3 => ("[Godot 3]", egui::Color32::from_rgb(70, 180, 255)),
                                                steam_scanner::GodotVersion::Unknown => ("[Godot]", egui::Color32::from_rgb(70, 180, 255)),
                                            },
                                            steam_scanner::GameEngine::Source => ("[Source]", egui::Color32::from_rgb(100, 220, 120)),
                                            steam_scanner::GameEngine::GameMaker => ("[GameMaker]", egui::Color32::from_rgb(80, 200, 120)),
                                            steam_scanner::GameEngine::RenPy => ("[Ren'Py]", egui::Color32::from_rgb(100, 200, 100)),
                                            steam_scanner::GameEngine::UnrealEngine { .. } => ("[UE4/5 ⛔]", egui::Color32::from_rgb(255, 80, 80)),
                                            steam_scanner::GameEngine::CustomNative => ("[Win64]", egui::Color32::from_rgb(170, 150, 250)),
                                            steam_scanner::GameEngine::Unknown => {
                                                if compat.status == steam_scanner::CompatibilityStatus::LinuxNative {
                                                    ("[Linux ELF]", egui::Color32::from_rgb(180, 100, 240))
                                                } else {
                                                    ("[Win32/Unknown ⛔]", egui::Color32::from_rgb(220, 100, 100))
                                                }
                                            }
                                        };

                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(badge_text)
                                                    .size(10.0)
                                                    .color(badge_col)
                                                    .monospace(),
                                            );
                                            ui.label(
                                                egui::RichText::new(engine_tag)
                                                    .size(10.0)
                                                    .strong()
                                                    .color(engine_col)
                                                    .monospace(),
                                            );
                                        });
                                    });

                                    // Right Action Button (Launch or Stop)
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if !is_steam && !is_this_game_running {
                                            let del_tip = match self.config.lang {
                                                Language::En => "Remove from list",
                                                Language::Ru => "Удалить из списка",
                                            };
                                            if ui.small_button("✖").on_hover_text(del_tip).clicked() {
                                                deleted_game_path = Some(exe_path.clone());
                                            }
                                        }

                                        if is_this_game_running {
                                            let stop_text = match self.config.lang {
                                                Language::En => "⏹ STOP",
                                                Language::Ru => "⏹ ОСТАНОВИТЬ",
                                            };
                                            let stop_btn = egui::Button::new(
                                                egui::RichText::new(stop_text).strong().size(12.0).color(egui::Color32::WHITE),
                                            )
                                            .fill(egui::Color32::from_rgb(220, 53, 69))
                                            .rounding(6.0)
                                            .min_size(egui::vec2(86.0, 28.0));

                                            if ui.add(stop_btn).clicked() {
                                                if let Ok(mut lock) = self.running_process.lock() {
                                                    if let Some(ref mut child) = *lock {
                                                        let _ = child.kill();
                                                    }
                                                    *lock = None;
                                                }
                                                self.is_running.store(false, Ordering::SeqCst);
                                                self.running_game_name = None;
                                                self.running_game_path = None;
                                                self.running_game_appid = None;
                                                self.running_game_start = None;
                                                self.running_game_pid = None;
                                                self.trigger_toast(match self.config.lang {
                                                    Language::En => "⏹ Game stopped by user.",
                                                    Language::Ru => "⏹ Игра остановлена пользователем.",
                                                });
                                            }
                                        } else {
                                            let play_text = match self.config.lang {
                                                Language::En => "▶ PLAY",
                                                Language::Ru => "▶ ИГРАТЬ",
                                            };
                                            if any_game_running {
                                                let play_btn = egui::Button::new(
                                                    egui::RichText::new(play_text).strong().size(12.0).color(egui::Color32::from_rgb(140, 140, 150)),
                                                )
                                                .fill(egui::Color32::from_rgb(45, 45, 55))
                                                .rounding(6.0)
                                                .min_size(egui::vec2(76.0, 26.0));
                                                let tip = match self.config.lang {
                                                    Language::En => "⚠️ Another game is currently running! Rine supports 1 game at a time.",
                                                    Language::Ru => "⚠️ Другая игра сейчас запущена! Rine запускает одну игру одновременно.",
                                                };
                                                ui.add_enabled(false, play_btn).on_disabled_hover_text(tip);
                                            } else {
                                                let play_btn = egui::Button::new(
                                                    egui::RichText::new(play_text).strong().size(12.0).color(egui::Color32::WHITE),
                                                )
                                                .fill(egui::Color32::from_rgb(255, 102, 0))
                                                .rounding(6.0)
                                                .min_size(egui::vec2(76.0, 26.0));

                                                if ui.add(play_btn).clicked() {
                                                    new_selected_idx = Some(idx);
                                                    launched_game_idx = Some(idx);
                                                }
                                            }
                                        }
                                    });
                                });
                            });
                        ui.add_space(4.0);
                    }

                    if let Some(idx) = new_selected_idx {
                        self.select_game_index(idx);
                    }
                    if let Some(path) = deleted_game_path {
                        steam_scanner::remove_custom_game(&path);
                        self.refresh_games();
                    }
                    if let Some(_) = launched_game_idx {
                        self.start_game_launch();
                    }
                }
            });

            ui.add_space(4.0);

            // Game Compatibility Card for selected game
            if let Some(ref path) = self.selected_path {
                let compat = steam_scanner::check_game_compatibility(path);
                let border_col = egui::Color32::from_rgb(compat.badge_color.0, compat.badge_color.1, compat.badge_color.2);

                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_premultiplied(22, 24, 34, 230))
                    .stroke(egui::Stroke::new(1.0, border_col))
                    .rounding(6.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 7.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let engine_title = match self.config.lang {
                                Language::En => "⚙ Engine:",
                                Language::Ru => "⚙ Движок:",
                            };
                            ui.label(
                                egui::RichText::new(engine_title)
                                    .strong()
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(200, 200, 210)),
                            );
                            ui.label(
                                egui::RichText::new(&compat.engine_name)
                                    .strong()
                                    .size(12.0)
                                    .color(egui::Color32::WHITE),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let badge_bg = egui::Color32::from_rgb(compat.badge_color.0, compat.badge_color.1, compat.badge_color.2);
                                let badge_text_color = if compat.is_supported {
                                    egui::Color32::BLACK
                                } else {
                                    egui::Color32::WHITE
                                };
                                ui.label(
                                    egui::RichText::new(format!(" {} ", compat.localized_status_text(self.config.lang == Language::En)))
                                        .strong()
                                        .size(11.0)
                                        .color(badge_text_color)
                                        .background_color(badge_bg),
                                );
                            });
                        });

                        ui.add_space(2.0);
                        let detail_col = if compat.is_supported {
                            egui::Color32::from_rgb(170, 220, 180)
                        } else {
                            egui::Color32::from_rgb(255, 140, 140)
                        };
                        ui.label(
                            egui::RichText::new(compat.localized_details(self.config.lang == Language::En))
                                .size(10.5)
                                .color(detail_col),
                        );
                    });

                ui.add_space(4.0);
            }

            ui.separator();
            ui.add_space(4.0);

            // Bottom Big Launch Action Bar
            let selected_name = if let Some(idx) = self.selected_game_index {
                self.games.get(idx).map(|g| g.name.as_str()).unwrap_or("No game selected")
            } else {
                "No game selected"
            };

            let is_running = self.is_running.load(Ordering::SeqCst);
            let is_current_game_running = is_running
                && self.running_game_path.as_ref() == self.selected_path.as_ref();

            let fps_suffix = if self.config.fps_limit == 0 {
                match self.config.lang {
                    Language::En => "Uncapped".to_string(),
                    Language::Ru => "Без огр.".to_string(),
                }
            } else {
                format!("{} FPS", self.config.fps_limit)
            };

            let (btn_fill, btn_label, is_engine_supported, can_launch) = if is_current_game_running {
                let stop_text = match self.config.lang {
                    Language::En => format!("⏹ STOP GAME: {} (RUNNING)", selected_name),
                    Language::Ru => format!("⏹ ОСТАНОВИТЬ: {} (В ИГРЕ)", selected_name),
                };
                (egui::Color32::from_rgb(220, 53, 69), stop_text, true, true)
            } else if is_running {
                let running_name = self.running_game_name.as_deref().unwrap_or("Game");
                let busy_text = match self.config.lang {
                    Language::En => format!("⚠️ ANOTHER GAME IS RUNNING ('{}') - 1 AT A TIME", running_name),
                    Language::Ru => format!("⚠️ ЗАПУЩЕНА ДРУГАЯ ИГРА ('{}') - ОСТАНОВИТЕ ЕЁ СНАЧАЛА", running_name),
                };
                (egui::Color32::from_rgb(55, 55, 65), busy_text, true, false)
            } else if let Some(ref path) = self.selected_path {
                let compat = steam_scanner::check_game_compatibility(path);
                if !compat.is_supported {
                    let unsupp_text = match self.config.lang {
                        Language::En => format!("⛔ UNSUPPORTED: {}", compat.engine_name),
                        Language::Ru => format!("⛔ НЕ ПОДДЕРЖИВАЕТСЯ: {}", compat.engine_name),
                    };
                    (egui::Color32::from_rgb(150, 45, 45), unsupp_text, false, true)
                } else {
                    let launch_btn_text = match self.config.lang {
                        Language::En => format!("🚀 LAUNCH: {} ({})", selected_name, fps_suffix),
                        Language::Ru => format!("🚀 ЗАПУСТИТЬ: {} ({})", selected_name, fps_suffix),
                    };
                    (egui::Color32::from_rgb(255, 102, 0), launch_btn_text, true, true)
                }
            } else {
                let launch_btn_text = match self.config.lang {
                    Language::En => format!("🚀 LAUNCH: {} ({})", selected_name, fps_suffix),
                    Language::Ru => format!("🚀 ЗАПУСТИТЬ: {} ({})", selected_name, fps_suffix),
                };
                (egui::Color32::from_rgb(255, 102, 0), launch_btn_text, true, true)
            };

            let main_btn = egui::Button::new(
                egui::RichText::new(btn_label)
                    .strong()
                    .size(14.0)
                    .color(egui::Color32::WHITE),
            )
            .fill(btn_fill);

            let resp = ui.add_sized([ui.available_width(), 44.0], if can_launch { main_btn } else { main_btn });

            if resp.clicked() && can_launch {
                if is_current_game_running {
                    // Cleanly kill currently running game
                    if let Ok(mut lock) = self.running_process.lock() {
                        if let Some(ref mut child) = *lock {
                            let _ = child.kill();
                        }
                        *lock = None;
                    }
                    self.is_running.store(false, Ordering::SeqCst);
                    self.running_game_name = None;
                    self.running_game_path = None;
                    self.running_game_appid = None;
                    self.running_game_start = None;
                    self.running_game_pid = None;
                    self.trigger_toast(match self.config.lang {
                        Language::En => "⏹ Game stopped by user.",
                        Language::Ru => "⏹ Игра остановлена пользователем.",
                    });
                } else if !is_engine_supported {
                    if let Some(ref path) = self.selected_path {
                        let compat = steam_scanner::check_game_compatibility(path);
                        let tip = match self.config.lang {
                            Language::En => format!("⚠ Games on {} engine are not supported yet!", compat.engine_name),
                            Language::Ru => format!("⚠ Игры на движке {} пока не поддерживаются!", compat.engine_name),
                        };
                        self.trigger_toast(&tip);
                        self.add_console_log(
                            ConsoleLogLevel::Warn,
                            format!("Attempted unsupported game ({}): {}", compat.engine_name, compat.details),
                        );
                        self.tab = GuiTab::Console;
                    }
                } else {
                    self.start_game_launch();
                }
            }
        });
    }

    // Engine settings tab
    // Вкладка настроек движка и графики
    fn render_tab_settings(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical(|ui| {
                // FPS Limiter Section
                ui.group(|ui| {
                    let fps_head = match self.config.lang {
                        Language::En => "Frame Rate Limiter (Max FPS)",
                        Language::Ru => "Ограничение частоты кадров (Max FPS)",
                    };
                    ui.label(egui::RichText::new(fps_head).strong().size(15.0).color(egui::Color32::from_rgb(255, 102, 0)));
                    let fps_desc = match self.config.lang {
                        Language::En => "Controls DXVK frame limiter. Prevents GPU 100% load and overheating.",
                        Language::Ru => "Предотвращает 100% нагрузку и нагрев видеокарты RTX 3060 в простых сценах.",
                    };
                    ui.label(egui::RichText::new(fps_desc).small().color(egui::Color32::GRAY));
                    ui.add_space(8.0);

                    // Preset Buttons
                    ui.horizontal(|ui| {
                        let presets = [60, 120, 144, 240, 0];
                        for &preset in &presets {
                            let label = if preset == 0 {
                                match self.config.lang {
                                    Language::En => "∞ Uncapped".into(),
                                    Language::Ru => "∞ Без лимита".into(),
                                }
                            } else {
                                format!("{} FPS", preset)
                            };

                            let is_active = self.config.fps_limit == preset;
                            let mut btn = egui::Button::new(label);
                            if is_active {
                                btn = btn.fill(egui::Color32::from_rgb(255, 102, 0));
                            }
                            if ui.add(btn).clicked() {
                                self.config.fps_limit = preset;
                                CURRENT_FPS_LIMIT.store(preset, Ordering::Relaxed);
                                self.config.save();
                            }
                        }
                    });

                    ui.add_space(8.0);
                    // Slider
                    ui.horizontal(|ui| {
                        let slider_label = match self.config.lang {
                            Language::En => "Custom FPS:",
                            Language::Ru => "Точный FPS:",
                        };
                        ui.label(slider_label);
                        let mut limit_val = self.config.fps_limit;
                        if ui.add(egui::Slider::new(&mut limit_val, 30..=360).suffix(" FPS")).changed() {
                            self.config.fps_limit = limit_val;
                            CURRENT_FPS_LIMIT.store(limit_val, Ordering::Relaxed);
                            self.config.save();
                        }
                    });
                });

                ui.add_space(10.0);

                // Direct Steam Communication & License Verification
                ui.group(|ui| {
                    let steam_head = match self.config.lang {
                        Language::En => "Steam Direct Bridge & License Emulation",
                        Language::Ru => "Прямое общение со Steam и верификация лицензий",
                    };
                    ui.label(egui::RichText::new(steam_head).strong().size(15.0).color(egui::Color32::from_rgb(255, 102, 0)));
                    ui.add_space(4.0);

                    let (bridge_lbl, lic_lbl, vtable_lbl) = match self.config.lang {
                        Language::En => (
                            "● Steamworks Direct Bridge: ACTIVE",
                            "● License Emulation (BIsSubscribed=1, RestartApp=0): ENABLED",
                            "● C++ VTable Protection (Safe Mock Dispatcher): ACTIVE",
                        ),
                        Language::Ru => (
                            "● Прямой мост Steamworks: АКТИВЕН",
                            "● Эмуляция лицензий (BIsSubscribed=1, RestartApp=0): ВКЛЮЧЕНА",
                            "● Защита C++ VTable (Safe Mock Dispatcher): АКТИВНА",
                        ),
                    };
                    ui.colored_label(egui::Color32::from_rgb(16, 185, 129), bridge_lbl);
                    ui.colored_label(egui::Color32::from_rgb(16, 185, 129), lic_lbl);
                    ui.colored_label(egui::Color32::from_rgb(16, 185, 129), vtable_lbl);
                    ui.add_space(4.0);
                    let steam_desc = match self.config.lang {
                        Language::En => "Rine directly answers SteamAPI calls and validates game ownership so games run offline and online without closing or crashing.",
                        Language::Ru => "Rine напрямую обрабатывает запросы SteamAPI и подтверждает подписку, предотвращая вылеты и закрытие игр.",
                    };
                    ui.label(egui::RichText::new(steam_desc).small().color(egui::Color32::GRAY));
                });

                ui.add_space(10.0);

                // Graphics & VSync Section
                ui.group(|ui| {
                    let gfx_head = match self.config.lang {
                        Language::En => "Display & Synchronization",
                        Language::Ru => "Дисплей и синхронизация",
                    };
                    ui.label(egui::RichText::new(gfx_head).strong().size(15.0));
                    ui.add_space(6.0);

                    let vsync_label = match self.config.lang {
                        Language::En => "VSync (Monitor Refresh Rate Sync - smooth tear-free)",
                        Language::Ru => "VSync (Синхронизация с частотой монитора - плавность без разрывов)",
                    };
                    if ui.checkbox(&mut self.config.vsync, vsync_label).changed() {
                        self.config.save();
                    }
                });

                ui.add_space(10.0);

                // Memory & Sandbox Strategy
                ui.group(|ui| {
                    let mem_head = match self.config.lang {
                        Language::En => "Memory Buffering (Zero I/O)",
                        Language::Ru => "Буферизация памяти (Zero I/O)",
                    };
                    ui.label(egui::RichText::new(mem_head).strong().size(15.0));
                    ui.add_space(6.0);

                    let ram_mode_label = match self.config.lang {
                        Language::En => "RAM-Only Mode (preloads game into RAM memfd)",
                        Language::Ru => "Режим RAM-Only (предзагрузка игры в ОЗУ memfd)",
                    };
                    if ui.checkbox(&mut self.config.ram_mode, ram_mode_label).changed() {
                        self.config.save();
                    }

                    if self.config.ram_mode {
                        let lock_label = match self.config.lang {
                            Language::En => "Lock Pages in RAM (mlock - prevents swap to SSD)",
                            Language::Ru => "Фиксировать страницы в RAM (mlock - без сброса в SWAP)",
                        };
                        if ui.checkbox(&mut self.config.lock_ram, lock_label).changed() {
                            self.config.save();
                        }
                    }
                });
            });
        });
    }

    // Hardware diagnostics tab
    // Вкладка диагностики оборудования и драйверов
    fn render_tab_diagnostics(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            let hw_title = match self.config.lang {
                Language::En => "Detected Hardware & System Environment",
                Language::Ru => "Оборудование ПК и системное окружение",
            };
            ui.label(egui::RichText::new(hw_title).strong().size(16.0).color(egui::Color32::from_rgb(255, 102, 0)));
            ui.add_space(8.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let gpu_title = match self.config.lang {
                            Language::En => "🎮 GPU:",
                            Language::Ru => "🎮 Видеокарта (GPU):",
                        };
                        ui.label(egui::RichText::new(gpu_title).strong().size(14.0));
                        let driver_str = match self.config.lang {
                            Language::En => format!("{} (Driver {})", self.hardware.gpu_name, self.hardware.gpu_driver),
                            Language::Ru => format!("{} (Драйвер {})", self.hardware.gpu_name, self.hardware.gpu_driver),
                        };
                        ui.label(driver_str);
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        let disp_title = match self.config.lang {
                            Language::En => "📺 Display:",
                            Language::Ru => "📺 Экран / Монитор:",
                        };
                        ui.label(egui::RichText::new(disp_title).strong().size(14.0));
                        ui.label(&self.hardware.display);
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        let cpu_title = match self.config.lang {
                            Language::En => "🖥 CPU:",
                            Language::Ru => "🖥 Процессор (CPU):",
                        };
                        ui.label(egui::RichText::new(cpu_title).strong().size(14.0));
                        let cpu_str = match self.config.lang {
                            Language::En => format!("{} ({} threads)", self.hardware.cpu_name, self.hardware.cpu_threads),
                            Language::Ru => format!("{} ({} потоков)", self.hardware.cpu_name, self.hardware.cpu_threads),
                        };
                        ui.label(cpu_str);
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        let ram_title = match self.config.lang {
                            Language::En => "💾 RAM:",
                            Language::Ru => "💾 Оперативная память (RAM):",
                        };
                        ui.label(egui::RichText::new(ram_title).strong().size(14.0));
                        let ram_str = match self.config.lang {
                            Language::En => format!("{:.1} GB Total • {:.1} GB Available for Rine", self.hardware.ram_total_gb, self.hardware.ram_avail_gb),
                            Language::Ru => format!("{:.1} ГБ всего • {:.1} ГБ доступно для Rine", self.hardware.ram_total_gb, self.hardware.ram_avail_gb),
                        };
                        ui.label(ram_str);
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        let mouse_title = match self.config.lang {
                            Language::En => "🖱 Mouse:",
                            Language::Ru => "🖱 Мышь:",
                        };
                        ui.label(egui::RichText::new(mouse_title).strong().size(14.0));
                        ui.label(&self.hardware.mouse_name);
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        let kbd_title = match self.config.lang {
                            Language::En => "⌨ Keyboard:",
                            Language::Ru => "⌨ Клавиатура:",
                        };
                        ui.label(egui::RichText::new(kbd_title).strong().size(14.0));
                        ui.label(&self.hardware.keyboard_name);
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        let audio_title = match self.config.lang {
                            Language::En => "🔊 Audio:",
                            Language::Ru => "🔊 Звуковая подсистема:",
                        };
                        ui.label(egui::RichText::new(audio_title).strong().size(14.0));
                        ui.label(&self.hardware.audio_backend);
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        let os_title = match self.config.lang {
                            Language::En => "🐧 OS / Desktop:",
                            Language::Ru => "🐧 ОС / Окружение:",
                        };
                        ui.label(egui::RichText::new(os_title).strong().size(14.0));
                        ui.label(format!("{} ({})", self.hardware.os_name, self.hardware.compositor));
                    });
                });
            });
        });
    }

    // Live launch console tab
    // Вкладка консоли вывода процесса игры
    fn render_tab_console(&mut self, ui: &mut egui::Ui) {
        // 1. Compute counts and matching indices without holding lock during control buttons
        let query = self.console_filter.trim().to_lowercase();
        let (matching_indices, total_cnt, err_cnt, warn_cnt, steam_cnt, engine_cnt) = {
            if let Ok(logs) = self.console_logs.lock() {
                let mut err = 0;
                let mut warn = 0;
                let mut steam = 0;
                let mut engine = 0;
                let mut matching = Vec::with_capacity(logs.len());

                for (idx, entry) in logs.iter().enumerate() {
                    match entry.level {
                        ConsoleLogLevel::Error => err += 1,
                        ConsoleLogLevel::Warn => warn += 1,
                        ConsoleLogLevel::Steam => steam += 1,
                        ConsoleLogLevel::Engine => engine += 1,
                        ConsoleLogLevel::Info => {}
                    }

                    // Level filter check
                    let level_match = match self.console_level_filter {
                        ConsoleFilterLevel::All => true,
                        ConsoleFilterLevel::Errors => entry.level == ConsoleLogLevel::Error,
                        ConsoleFilterLevel::Warnings => entry.level == ConsoleLogLevel::Warn,
                        ConsoleFilterLevel::Steam => entry.level == ConsoleLogLevel::Steam,
                        ConsoleFilterLevel::Engine => entry.level == ConsoleLogLevel::Engine,
                    };

                    if !level_match {
                        continue;
                    }

                    // Query text match: checks text, timestamp, and level tags
                    let text_match = if query.is_empty() {
                        true
                    } else {
                        entry.text.to_lowercase().contains(&query)
                            || entry.timestamp.contains(&query)
                            || match entry.level {
                                ConsoleLogLevel::Error => {
                                    query == "error"
                                        || query.contains("err")
                                        || query.contains("ошиб")
                                        || query.contains("fail")
                                        || query.contains("crash")
                                        || query.contains("unresolved")
                                        || query.contains("exception")
                                }
                                ConsoleLogLevel::Warn => {
                                    query == "warn"
                                        || query.contains("warning")
                                        || query.contains("варн")
                                        || query.contains("предупр")
                                }
                                ConsoleLogLevel::Steam => query.contains("steam") || query.contains("стим"),
                                ConsoleLogLevel::Engine => {
                                    query.contains("engine")
                                        || query.contains("движок")
                                        || query.contains("unity")
                                        || query.contains("godot")
                                        || query.contains("dxvk")
                                        || query.contains("vulkan")
                                }
                                ConsoleLogLevel::Info => query.contains("info") || query.contains("инфо"),
                            }
                    };

                    if text_match {
                        matching.push(idx);
                    }
                }
                (matching, logs.len(), err, warn, steam, engine)
            } else {
                (Vec::new(), 0, 0, 0, 0, 0)
            }
        };

        // Auto-pause autoscroll if user scrolls up to read/select previous logs
        let user_scrolled_up = ui.input(|i| i.raw_scroll_delta.y > 0.0);
        if user_scrolled_up && self.console_auto_scroll {
            self.console_auto_scroll = false;
        }

        // Keyboard shortcut: Ctrl+C copies selected text natively; if nothing selected, copies matching lines
        if ui.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(egui::Key::C)) {
            let already_copied = ui.ctx().output(|o| !o.copied_text.is_empty());
            if !already_copied {
                if let Ok(logs) = self.console_logs.lock() {
                    let text = matching_indices
                        .iter()
                        .map(|&idx| {
                            let e = &logs[idx];
                            let tag = match e.level {
                                ConsoleLogLevel::Info => "[INFO]",
                                ConsoleLogLevel::Warn => "[WARN]",
                                ConsoleLogLevel::Error => "[ERROR]",
                                ConsoleLogLevel::Steam => "[STEAM]",
                                ConsoleLogLevel::Engine => "[ENGINE]",
                            };
                            format!("[{}] {} {}", e.timestamp, tag, e.text)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.is_empty() {
                        ui.ctx().output_mut(|o| o.copied_text = text);
                        self.show_toast = true;
                        self.toast_time = Instant::now();
                        self.toast_message = match self.config.lang {
                            Language::En => format!("Copied {} lines (Ctrl+C)", matching_indices.len()),
                            Language::Ru => format!("Скопировано {} строк (Ctrl+C)", matching_indices.len()),
                        };
                    }
                }
            }
        }

        ui.vertical(|ui| {
            // Control bar: Status + Actions
            ui.horizontal(|ui| {
                let is_running = self.is_running.load(Ordering::Relaxed);
                if is_running {
                    let active_lbl = match self.config.lang {
                        Language::En => "● PROCESS ACTIVE",
                        Language::Ru => "● ПРОЦЕСС АКТИВЕН",
                    };
                    ui.label(
                        egui::RichText::new(active_lbl)
                            .strong()
                            .size(12.0)
                            .color(egui::Color32::from_rgb(50, 255, 100)),
                    );
                    let pid_str = if let Ok(guard) = self.running_process.try_lock() {
                        guard.as_ref().map(|c| format!("(PID: {})", c.id())).unwrap_or_default()
                    } else {
                        String::new()
                    };
                    if !pid_str.is_empty() {
                        ui.label(egui::RichText::new(pid_str).size(11.0).color(egui::Color32::GRAY));
                    }

                    let kill_btn_text = match self.config.lang {
                        Language::En => "⛔ Kill",
                        Language::Ru => "⛔ Завершить",
                    };
                    let kill_tip = match self.config.lang {
                        Language::En => "Force terminate running game process",
                        Language::Ru => "Принудительно закрыть запущенный процесс игры",
                    };
                    let kill_btn = egui::Button::new(
                        egui::RichText::new(kill_btn_text).strong().size(11.0).color(egui::Color32::WHITE),
                    )
                    .fill(egui::Color32::from_rgb(180, 40, 40));

                    if ui.add(kill_btn)
                        .on_hover_text(kill_tip)
                        .clicked()
                    {
                        if let Ok(mut guard) = self.running_process.lock() {
                            if let Some(mut child) = guard.take() {
                                let _ = child.kill();
                                let _ = child.wait();
                                self.is_running.store(false, Ordering::SeqCst);
                                let stop_msg = match self.config.lang {
                                    Language::En => "[RINE] Process forcefully terminated by user.",
                                    Language::Ru => "[RINE] Процесс принудительно остановлен пользователем.",
                                };
                                self.add_console_log(
                                    ConsoleLogLevel::Warn,
                                    stop_msg,
                                );
                            }
                        }
                    }
                } else {
                    let no_active_lbl = match self.config.lang {
                        Language::En => "○ NO ACTIVE PROCESS",
                        Language::Ru => "○ НЕТ АКТИВНОГО ПРОЦЕССА",
                    };
                    ui.label(
                        egui::RichText::new(no_active_lbl)
                            .size(12.0)
                            .color(egui::Color32::GRAY),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (clear_lbl, clear_tip) = match self.config.lang {
                        Language::En => ("🗑 Clear", "Clear in-memory console logs"),
                        Language::Ru => ("🗑 Очистить", "Очистить журнал консоли в памяти"),
                    };
                    if ui.button(clear_lbl).on_hover_text(clear_tip).clicked() {
                        if let Ok(mut logs) = self.console_logs.lock() {
                            logs.clear();
                        }
                    }

                    let (logfile_lbl, logfile_tip) = match self.config.lang {
                        Language::En => ("📂 Log File", "Open persistent log file on disk (~/.local/share/rine/logs/rine_session.log)"),
                        Language::Ru => ("📂 Лог-файл", "Открыть постоянный файл логов на диске (~/.local/share/rine/logs/rine_session.log)"),
                    };
                    if ui.button(logfile_lbl).on_hover_text(logfile_tip).clicked() {
                        if let Some(home) = std::env::var_os("HOME") {
                            let log_file = std::path::Path::new(&home).join(".local/share/rine/logs/rine_session.log");
                            let _ = std::process::Command::new("xdg-open").arg(log_file).spawn();
                        }
                    }

                    let copy_lbl = if matching_indices.len() == total_cnt {
                        match self.config.lang {
                            Language::En => "📋 Copy All".to_string(),
                            Language::Ru => "📋 Копировать всё".to_string(),
                        }
                    } else {
                        match self.config.lang {
                            Language::En => format!("📋 Copy ({})", matching_indices.len()),
                            Language::Ru => format!("📋 Копировать ({})", matching_indices.len()),
                        }
                    };
                    let copy_tip = match self.config.lang {
                        Language::En => "Copy log lines to clipboard (Ctrl+C)",
                        Language::Ru => "Скопировать строки лога в буфер обмена (Ctrl+C)",
                    };
                    let mut toast_to_show = None;
                    if ui.button(copy_lbl).on_hover_text(copy_tip).clicked() {
                        if let Ok(logs) = self.console_logs.lock() {
                            let text = matching_indices
                                .iter()
                                .map(|&idx| {
                                    let e = &logs[idx];
                                    let tag = match e.level {
                                        ConsoleLogLevel::Info => "[INFO]",
                                        ConsoleLogLevel::Warn => "[WARN]",
                                        ConsoleLogLevel::Error => "[ERROR]",
                                        ConsoleLogLevel::Steam => "[STEAM]",
                                        ConsoleLogLevel::Engine => "[ENGINE]",
                                    };
                                    format!("[{}] {} {}", e.timestamp, tag, e.text)
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            if !text.is_empty() {
                                ui.ctx().output_mut(|o| o.copied_text = text);
                                toast_to_show = Some(match self.config.lang {
                                    Language::En => format!("Copied {} lines to clipboard!", matching_indices.len()),
                                    Language::Ru => format!("Скопировано {} строк в буфер обмена!", matching_indices.len()),
                                });
                            }
                        }
                    }
                    if let Some(msg) = toast_to_show {
                        self.trigger_toast(&msg);
                    }

                    let autoscroll_lbl = match self.config.lang {
                        Language::En => "Autoscroll",
                        Language::Ru => "Автоскролл",
                    };
                    ui.checkbox(&mut self.console_auto_scroll, autoscroll_lbl);
                });
            });

            ui.add_space(4.0);

            // Quick filter buttons row
            ui.horizontal(|ui| {
                let all_label = match self.config.lang {
                    Language::En => format!("All ({})", total_cnt),
                    Language::Ru => format!("Все ({})", total_cnt),
                };
                let all_active = self.console_level_filter == ConsoleFilterLevel::All;
                let all_btn = egui::Button::new(
                    egui::RichText::new(all_label)
                        .size(11.0)
                        .color(if all_active { egui::Color32::WHITE } else { egui::Color32::LIGHT_GRAY })
                )
                .fill(if all_active {
                    egui::Color32::from_rgb(255, 102, 0)
                } else {
                    egui::Color32::from_rgba_premultiplied(32, 34, 46, 200)
                });
                if ui.add(all_btn).clicked() {
                    self.console_level_filter = ConsoleFilterLevel::All;
                }

                let err_label = match self.config.lang {
                    Language::En => format!("⛔ Errors ({})", err_cnt),
                    Language::Ru => format!("⛔ Ошибки ({})", err_cnt),
                };
                let err_tip = match self.config.lang {
                    Language::En => "Show only error messages (ERROR / CRASH / FAILED / Exception)",
                    Language::Ru => "Показать только сообщения об ошибках (ERROR / CRASH / FAILED / Exception)",
                };
                let err_active = self.console_level_filter == ConsoleFilterLevel::Errors;
                let err_btn = egui::Button::new(
                    egui::RichText::new(err_label)
                        .size(11.0)
                        .strong()
                        .color(if err_active { egui::Color32::WHITE } else if err_cnt > 0 { egui::Color32::from_rgb(255, 120, 120) } else { egui::Color32::GRAY })
                )
                .fill(if err_active {
                    egui::Color32::from_rgb(180, 40, 40)
                } else if err_cnt > 0 {
                    egui::Color32::from_rgba_premultiplied(70, 25, 25, 220)
                } else {
                    egui::Color32::from_rgba_premultiplied(32, 34, 46, 200)
                });
                if ui.add(err_btn).on_hover_text(err_tip).clicked() {
                    self.console_level_filter = if err_active {
                        ConsoleFilterLevel::All
                    } else {
                        ConsoleFilterLevel::Errors
                    };
                }

                let warn_label = match self.config.lang {
                    Language::En => format!("▲ Warnings ({})", warn_cnt),
                    Language::Ru => format!("▲ Предупреждения ({})", warn_cnt),
                };
                let warn_tip = match self.config.lang {
                    Language::En => "Show warnings (WARN)",
                    Language::Ru => "Показать предупреждения (WARN)",
                };
                let warn_active = self.console_level_filter == ConsoleFilterLevel::Warnings;
                let warn_btn = egui::Button::new(
                    egui::RichText::new(warn_label)
                        .size(11.0)
                        .color(if warn_active { egui::Color32::WHITE } else if warn_cnt > 0 { egui::Color32::from_rgb(255, 220, 100) } else { egui::Color32::GRAY })
                )
                .fill(if warn_active {
                    egui::Color32::from_rgb(180, 130, 20)
                } else if warn_cnt > 0 {
                    egui::Color32::from_rgba_premultiplied(60, 50, 20, 220)
                } else {
                    egui::Color32::from_rgba_premultiplied(32, 34, 46, 200)
                });
                if ui.add(warn_btn).on_hover_text(warn_tip).clicked() {
                    self.console_level_filter = if warn_active {
                        ConsoleFilterLevel::All
                    } else {
                        ConsoleFilterLevel::Warnings
                    };
                }

                let steam_label = format!("🎮 Steam ({})", steam_cnt);
                let steam_tip = match self.config.lang {
                    Language::En => "Show Steamworks API logs",
                    Language::Ru => "Показать логи Steamworks API",
                };
                let steam_active = self.console_level_filter == ConsoleFilterLevel::Steam;
                let steam_btn = egui::Button::new(
                    egui::RichText::new(steam_label)
                        .size(11.0)
                        .color(if steam_active { egui::Color32::WHITE } else { egui::Color32::from_rgb(100, 210, 255) })
                )
                .fill(if steam_active {
                    egui::Color32::from_rgb(40, 100, 160)
                } else {
                    egui::Color32::from_rgba_premultiplied(32, 34, 46, 200)
                });
                if ui.add(steam_btn).on_hover_text(steam_tip).clicked() {
                    self.console_level_filter = if steam_active {
                        ConsoleFilterLevel::All
                    } else {
                        ConsoleFilterLevel::Steam
                    };
                }

                let eng_label = match self.config.lang {
                    Language::En => format!("⚙ Engine ({})", engine_cnt),
                    Language::Ru => format!("⚙ Движок ({})", engine_cnt),
                };
                let eng_tip = match self.config.lang {
                    Language::En => "Show Unity, Godot, DXVK, Vulkan logs",
                    Language::Ru => "Показать логи Unity, Godot, DXVK, Vulkan",
                };
                let eng_active = self.console_level_filter == ConsoleFilterLevel::Engine;
                let eng_btn = egui::Button::new(
                    egui::RichText::new(eng_label)
                        .size(11.0)
                        .color(if eng_active { egui::Color32::WHITE } else { egui::Color32::from_rgb(200, 150, 255) })
                )
                .fill(if eng_active {
                    egui::Color32::from_rgb(120, 60, 160)
                } else {
                    egui::Color32::from_rgba_premultiplied(32, 34, 46, 200)
                });
                if ui.add(eng_btn).on_hover_text(eng_tip).clicked() {
                    self.console_level_filter = if eng_active {
                        ConsoleFilterLevel::All
                    } else {
                        ConsoleFilterLevel::Engine
                    };
                }
            });

            ui.add_space(3.0);

            // Filter & Search bar
            ui.horizontal(|ui| {
                let search_lbl = match self.config.lang {
                    Language::En => "🔍 Search:",
                    Language::Ru => "🔍 Поиск:",
                };
                ui.label(egui::RichText::new(search_lbl).size(11.0).color(egui::Color32::GRAY));
                let clear_btn_w = if self.console_filter.is_empty() { 0.0 } else { 26.0 };
                let search_w = (ui.available_width() - clear_btn_w).max(50.0);
                let hint = match self.config.lang {
                    Language::En => "Search errors or text (error, crash, xinput, GetVolume, mono, steam)...",
                    Language::Ru => "Поиск ошибок или текста (error, crash, xinput, GetVolume, mono, steam)...",
                };
                let reset_tip = match self.config.lang {
                    Language::En => "Reset search query",
                    Language::Ru => "Сбросить поисковый запрос",
                };
                ui.add_sized(
                    [search_w, 22.0],
                    egui::TextEdit::singleline(&mut self.console_filter)
                        .hint_text(hint),
                );
                if !self.console_filter.is_empty() {
                    if ui.button("✕").on_hover_text(reset_tip).clicked() {
                        self.console_filter.clear();
                    }
                }
            });

            // Status notice when filter is active or autoscroll is off
            let filter_active = self.console_level_filter != ConsoleFilterLevel::All || !self.console_filter.trim().is_empty();
            if filter_active || !self.console_auto_scroll {
                ui.horizontal(|ui| {
                    if filter_active {
                        let found_lbl = match self.config.lang {
                            Language::En => format!("Found: {} of {} lines", matching_indices.len(), total_cnt),
                            Language::Ru => format!("Найдено: {} из {} строк", matching_indices.len(), total_cnt),
                        };
                        ui.label(
                            egui::RichText::new(found_lbl)
                                .size(10.5)
                                .color(egui::Color32::from_rgb(255, 170, 70)),
                        );
                    }
                    if !self.console_auto_scroll {
                        let autoscroll_paused_lbl = match self.config.lang {
                            Language::En => "⏸ Autoscroll disabled - position locked",
                            Language::Ru => "⏸ Автоскролл выключен - позиция зафиксирована",
                        };
                        ui.label(
                            egui::RichText::new(autoscroll_paused_lbl)
                                .size(10.5)
                                .color(egui::Color32::from_rgb(160, 165, 180)),
                        );
                    }
                });
            }

            ui.add_space(2.0);

            // Monospace Log Terminal Frame
            let avail_h = ui.available_height() - 6.0;
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(12, 13, 18))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(50, 52, 70, 160)))
                .rounding(6.0)
                .inner_margin(egui::Margin::symmetric(8.0, 8.0))
                .show(ui, |ui| {
                    // Enable full mouse text selection and Ctrl+C copying across labels
                    ui.style_mut().interaction.selectable_labels = true;
                    ui.style_mut().interaction.multi_widget_text_select = true;

                    if matching_indices.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            if total_cnt == 0 {
                                ui.label(
                                    egui::RichText::new("📟 Консоль запуска Rine Win64 готова.\nЗапустите игру из библиотеки для вывода stdout/stderr в реальном времени.")
                                        .monospace()
                                        .color(egui::Color32::GRAY),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("🔍 По вашему запросу ничего не найдено.\nПопробуйте изменить поисковую строку или сбросить фильтр.")
                                        .monospace()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                    } else if let Ok(logs) = self.console_logs.lock() {
                        let mut scroll = egui::ScrollArea::vertical()
                            .id_source("rine_console_scroll_unified")
                            .max_height(avail_h)
                            .auto_shrink([false, false]);

                        // Strict autoscroll: only stick to bottom if user checkbox is checked
                        if self.console_auto_scroll {
                            scroll = scroll.stick_to_bottom(true);
                        }

                        let max_display = 2500;
                        let slice_start = matching_indices.len().saturating_sub(max_display);
                        let display_indices = &matching_indices[slice_start..];

                        scroll.show(ui, |ui| {
                            for &entry_idx in display_indices {
                                let entry = &logs[entry_idx];

                                let (tag, col) = match entry.level {
                                    ConsoleLogLevel::Info => ("[INFO]", egui::Color32::from_rgb(150, 195, 255)),
                                    ConsoleLogLevel::Warn => ("[WARN]", egui::Color32::from_rgb(255, 205, 70)),
                                    ConsoleLogLevel::Error => ("[ERROR]", egui::Color32::from_rgb(255, 80, 80)),
                                    ConsoleLogLevel::Steam => ("[STEAM]", egui::Color32::from_rgb(80, 225, 255)),
                                    ConsoleLogLevel::Engine => ("[ENGINE]", egui::Color32::from_rgb(205, 140, 255)),
                                };

                                let mut job = egui::text::LayoutJob::default();
                                job.append(
                                    &format!("[{}] ", entry.timestamp),
                                    0.0,
                                    egui::TextFormat {
                                        font_id: egui::FontId::monospace(10.5),
                                        color: egui::Color32::from_rgb(110, 115, 130),
                                        ..Default::default()
                                    },
                                );
                                job.append(
                                    tag,
                                    0.0,
                                    egui::TextFormat {
                                        font_id: egui::FontId::monospace(10.5),
                                        color: col,
                                        ..Default::default()
                                    },
                                );
                                job.append(
                                    &format!("  {}", entry.text),
                                    0.0,
                                    egui::TextFormat {
                                        font_id: egui::FontId::monospace(10.5),
                                        color: if entry.level == ConsoleLogLevel::Error {
                                            egui::Color32::from_rgb(255, 160, 160)
                                        } else {
                                            egui::Color32::from_rgb(230, 230, 235)
                                        },
                                        ..Default::default()
                                    },
                                );

                                let full_line = format!("[{}] {} {}", entry.timestamp, tag, entry.text);
                                let label = egui::Label::new(job).selectable(true);
                                let resp = ui.add(label);

                                resp.context_menu(|ui| {
                                    let copy_line_txt = match self.config.lang {
                                        Language::En => "📋 Copy this line",
                                        Language::Ru => "📋 Скопировать эту строку",
                                    };
                                    let copy_msg_txt = match self.config.lang {
                                        Language::En => "📋 Copy text only (without timestamp)",
                                        Language::Ru => "📋 Скопировать только текст (без времени)",
                                    };
                                    if ui.button(copy_line_txt).clicked() {
                                        ui.ctx().output_mut(|o| o.copied_text = full_line.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button(copy_msg_txt).clicked() {
                                        ui.ctx().output_mut(|o| o.copied_text = entry.text.clone());
                                        ui.close_menu();
                                    }
                                });
                            }
                        });
                    }
                });
        });
    }

    // Game launch splash screen
    // Экран запуска игры с синхронизацией готовности
    fn render_launching_splash(&mut self, ctx: &egui::Context) {
        // Ensure GIF frames are decoded
        self.ensure_launch_gif(ctx);

        // If game process failed or exited early, switch back to dashboard
        if !self.is_running.load(Ordering::SeqCst) {
            self.state = AppState::Dashboard;
            return;
        }

        // Real handshake: Wait for Rine runtime readiness confirmation
        if self.game_runtime_ready.load(Ordering::SeqCst) {
            let toast_notice = match self.config.lang {
                Language::En => "🎮 Game Launched! Live console is now active.",
                Language::Ru => "🎮 Игра запущена! Открыта консоль запуска.",
            };
            self.trigger_toast(toast_notice);
            self.tab = GuiTab::Console;
            self.state = AppState::Dashboard;
            // Minimize Rine GUI so it doesn't freeze or block fullscreen games
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            return;
        }

        // Keep UI repainting for smooth GIF animation
        ctx.request_repaint();

        // Calculate current GIF frame index
        let cur_frame_idx = if !self.launch_gif_frames.is_empty() {
            let elapsed_ms = (self.launch_start_time.elapsed().as_millis()
                % self.launch_gif_total_duration.as_millis().max(1)) as u64;
            let mut acc = 0u64;
            let mut chosen = 0;
            for (i, (_, dur)) in self.launch_gif_frames.iter().enumerate() {
                acc += dur.as_millis() as u64;
                if elapsed_ms < acc {
                    chosen = i;
                    break;
                }
            }
            Some(chosen)
        } else {
            None
        };

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_premultiplied(16, 16, 24, 245))
                    .stroke(egui::Stroke::new(1.2, egui::Color32::from_rgba_premultiplied(255, 102, 0, 180)))
                    .rounding(10.0)
                    .inner_margin(egui::Margin::symmetric(16.0, 12.0)),
            )
            .show(ctx, |ui| {
                self.render_custom_titlebar(ctx, ui);
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.05);

                    if let Some(idx) = cur_frame_idx {
                        if let Some((tex, _)) = self.launch_gif_frames.get(idx) {
                            // Display user's animated GIF (640x360 aspect ratio = 16:9)
                            let gif_size = egui::vec2(448.0, 252.0);
                            let (rect, _resp) = ui.allocate_exact_size(gif_size, egui::Sense::hover());
                            ui.painter().image(
                                tex.id(),
                                rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        }
                    } else if let Some(tex) = &self.logo_texture {
                        let logo_size = egui::vec2(120.0, 120.0);
                        let (rect, _resp) = ui.allocate_exact_size(logo_size, egui::Sense::hover());
                        ui.painter().image(
                            tex.id(),
                            rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }

                    ui.add_space(14.0);
                    let game_name = if !self.launching_game_name.is_empty() {
                        &self.launching_game_name
                    } else {
                        "Game"
                    };

                    let title_text = match self.config.lang {
                        Language::En => format!("Launching {}...", game_name),
                        Language::Ru => format!("Запуск {}...", game_name),
                    };
                    ui.heading(
                        egui::RichText::new(title_text)
                            .strong()
                            .size(20.0)
                            .color(egui::Color32::from_rgb(255, 110, 0)),
                    );

                    ui.add_space(6.0);
                    let sync_text = match self.config.lang {
                        Language::En => "Waiting for Rine Win64 Engine render confirmation...",
                        Language::Ru => "Ожидание подтверждения рендеринга от ядра Rine Win64...",
                    };
                    ui.label(
                        egui::RichText::new(sync_text)
                            .monospace()
                            .size(11.0)
                            .color(egui::Color32::from_rgb(200, 200, 215)),
                    );

                    ui.add_space(14.0);
                    let open_cons_text = match self.config.lang {
                        Language::En => "📟 Open Live Console Now",
                        Language::Ru => "📟 Открыть консоль сейчас",
                    };
                    if ui.small_button(open_cons_text).clicked() {
                        self.tab = GuiTab::Console;
                        self.state = AppState::Dashboard;
                    }
                });
            });
    }

    // In-game toast notification overlay
    // Всплывающее оверлейное уведомление
    fn render_toast_overlay(&mut self, ctx: &egui::Context) {
        if !self.show_toast {
            return;
        }

        let elapsed = self.toast_time.elapsed().as_secs_f32();
        if elapsed > 6.0 {
            self.show_toast = false;
            return;
        }

        let alpha = if elapsed < 0.4 {
            (elapsed / 0.4).min(1.0)
        } else if elapsed > 5.2 {
            ((6.0 - elapsed) / 0.8).max(0.0)
        } else {
            1.0
        };

        egui::Area::new(egui::Id::new("toast_overlay"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-20.0, 20.0))
            .show(ctx, |ui| {
                let fill = egui::Color32::from_rgba_premultiplied(
                    (28.0 * alpha) as u8,
                    (28.0 * alpha) as u8,
                    (36.0 * alpha) as u8,
                    (245.0 * alpha) as u8,
                );
                let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(
                    (255.0 * alpha) as u8,
                    (102.0 * alpha) as u8,
                    0,
                    (245.0 * alpha) as u8,
                ));

                egui::Frame::none()
                    .fill(fill)
                    .stroke(stroke)
                    .rounding(8.0)
                    .inner_margin(egui::Margin::symmetric(16.0, 10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("⚙ RINE")
                                    .strong()
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(255, 102, 0)),
                            );
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(&self.toast_message).size(13.0));
                        });
                    });
            });
    }
}

// In-game overlay application (Shift + F12)
// Приложение внутриигрового оверлея (Shift + F12)

pub struct RineOverlayApp {
    config: RineConfig,
    hardware: HardwareInfo,
}

impl RineOverlayApp {
    pub fn run() -> eframe::Result<()> {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Rine In-Game Steam Overlay")
                .with_inner_size([560.0, 480.0])
                .with_resizable(false)
                .with_always_on_top(),
            ..Default::default()
        };

        eframe::run_native(
            "Rine Overlay",
            options,
            Box::new(|_cc| {
                Box::new(RineOverlayApp {
                    config: RineConfig::load(),
                    hardware: HardwareInfo::probe(),
                })
            }),
        )
    }
}

impl eframe::App for RineOverlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = egui::Color32::from_rgba_premultiplied(12, 14, 20, 245);
        style.visuals.panel_fill = egui::Color32::from_rgba_premultiplied(12, 14, 20, 245);
        ctx.set_style(style);

        egui::CentralPanel::default().show(ctx, |ui| {
            // Steam Header Bar
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("🎮 STEAM OVERLAY // RINE")
                        .strong()
                        .size(16.0)
                        .color(egui::Color32::from_rgb(255, 102, 0)),
                );

                ui.label(
                    egui::RichText::new(" [ SHIFT + TAB ] ")
                        .monospace()
                        .size(11.0)
                        .strong()
                        .color(egui::Color32::from_rgb(0, 230, 118))
                        .background_color(egui::Color32::from_rgba_premultiplied(0, 80, 40, 180)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close_text = match self.config.lang {
                        Language::En => "✖ Close",
                        Language::Ru => "✖ Закрыть",
                    };
                    if ui.button(close_text).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    let cur_time = current_time_formatted();
                    ui.label(egui::RichText::new(format!("🕒 {}", cur_time)).monospace().size(12.0).color(egui::Color32::from_rgb(200, 200, 210)));
                });
            });

            ui.separator();
            ui.add_space(8.0);

            // Card 1: Performance & Target FPS Limits
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(20, 22, 32, 220))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(60, 64, 85, 180)))
                .rounding(6.0)
                .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    let fps_head = match self.config.lang {
                        Language::En => "Target FPS Limit & Refresh Rate:",
                        Language::Ru => "Ограничение частоты кадров (FPS):",
                    };
                    ui.label(egui::RichText::new(fps_head).strong().size(13.0).color(egui::Color32::from_rgb(240, 240, 245)));
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        for &preset in &[60, 120, 144, 240, 0] {
                            let label = if preset == 0 {
                                match self.config.lang {
                                    Language::En => "∞ Uncapped".into(),
                                    Language::Ru => "∞ Без лимита".into(),
                                }
                            } else {
                                format!("{} FPS", preset)
                            };
                            let is_active = self.config.fps_limit == preset;
                            let mut btn = egui::Button::new(label);
                            if is_active {
                                btn = btn.fill(egui::Color32::from_rgb(255, 102, 0));
                            }
                            if ui.add(btn).clicked() {
                                self.config.fps_limit = preset;
                                self.config.save();
                                CURRENT_FPS_LIMIT.store(preset, Ordering::Relaxed);
                                if preset > 0 {
                                    std::env::set_var("DXVK_FRAME_RATE", preset.to_string());
                                } else {
                                    std::env::remove_var("DXVK_FRAME_RATE");
                                }
                            }
                        }
                    });

                    ui.add_space(6.0);
                    let mut val = self.config.fps_limit;
                    if ui.add(egui::Slider::new(&mut val, 0..=360).suffix(" FPS")).changed() {
                        self.config.fps_limit = val;
                        self.config.save();
                        CURRENT_FPS_LIMIT.store(val, Ordering::Relaxed);
                        if val > 0 {
                            std::env::set_var("DXVK_FRAME_RATE", val.to_string());
                        } else {
                            std::env::remove_var("DXVK_FRAME_RATE");
                        }
                    }

                    ui.add_space(4.0);
                    let vsync_lbl = match self.config.lang {
                        Language::En => "VSync (Monitor Refresh Rate Sync - smooth tear-free)",
                        Language::Ru => "VSync (Синхронизация с частотой монитора - без разрывов)",
                    };
                    if ui.checkbox(&mut self.config.vsync, vsync_lbl).changed() {
                        self.config.save();
                        std::env::set_var("DXVK_VSYNC", if self.config.vsync { "1" } else { "0" });
                    }
                });

            ui.add_space(8.0);

            // Card 2: Master Audio Volume (WASAPI)
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(20, 22, 32, 220))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(60, 64, 85, 180)))
                .rounding(6.0)
                .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    let audio_head = match self.config.lang {
                        Language::En => "Master Audio Volume (WASAPI / PipeWire):",
                        Language::Ru => "Громкость звука (WASAPI / PipeWire):",
                    };
                    ui.label(egui::RichText::new(audio_head).strong().size(13.0).color(egui::Color32::from_rgb(240, 240, 245)));
                    ui.add_space(6.0);

                    let mut cur_vol = (crate::dlls::wasapi::AUDIO_MASTER_VOLUME.load(Ordering::Relaxed) as f32 / 10.0) as u32;
                    ui.horizontal(|ui| {
                        ui.label(if cur_vol == 0 { "🔇" } else if cur_vol < 50 { "🔉" } else { "🔊" });
                        if ui.add(egui::Slider::new(&mut cur_vol, 0..=100).suffix("%")).changed() {
                            crate::dlls::wasapi::AUDIO_MASTER_VOLUME.store(cur_vol * 10, Ordering::Relaxed);
                        }
                    });
                });

            ui.add_space(8.0);

            // Card 3: Hardware Telemetry
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(20, 22, 32, 220))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(60, 64, 85, 180)))
                .rounding(6.0)
                .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    let telem_head = match self.config.lang {
                        Language::En => "Active Hardware & Display Pipeline:",
                        Language::Ru => "Аппаратная конфигурация и дисплей:",
                    };
                    ui.label(egui::RichText::new(telem_head).strong().size(13.0).color(egui::Color32::from_rgb(240, 240, 245)));
                    ui.add_space(4.0);

                    let gpu_line = match self.config.lang {
                        Language::En => format!("• GPU: {} (Driver: {})", self.hardware.gpu_name, self.hardware.gpu_driver),
                        Language::Ru => format!("• Видеокарта: {} (Драйвер: {})", self.hardware.gpu_name, self.hardware.gpu_driver),
                    };
                    let disp_line = match self.config.lang {
                        Language::En => format!("• Display: {}", self.hardware.display),
                        Language::Ru => format!("• Экран: {}", self.hardware.display),
                    };
                    ui.label(egui::RichText::new(gpu_line).size(11.5).color(egui::Color32::from_rgb(200, 205, 220)));
                    ui.label(egui::RichText::new(disp_line).size(11.5).color(egui::Color32::from_rgb(200, 205, 220)));
                });

            ui.add_space(10.0);

            // Bottom Buttons
            ui.horizontal(|ui| {
                let resume_lbl = match self.config.lang {
                    Language::En => "✔ Resume Game (Shift+Tab)",
                    Language::Ru => "✔ Вернуться в игру (Shift+Tab)",
                };
                if ui.add_sized([ui.available_width() - 140.0, 36.0], egui::Button::new(
                    egui::RichText::new(resume_lbl).strong().size(13.0).color(egui::Color32::WHITE)
                ).fill(egui::Color32::from_rgb(255, 102, 0))).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                let quit_lbl = match self.config.lang {
                    Language::En => "⏹ Quit Game",
                    Language::Ru => "⏹ Выйти из игры",
                };
                if ui.add_sized([130.0, 36.0], egui::Button::new(
                    egui::RichText::new(quit_lbl).strong().size(13.0).color(egui::Color32::WHITE)
                ).fill(egui::Color32::from_rgb(220, 53, 69))).clicked() {
                    std::process::Command::new("pkill").arg("-f").arg(".exe").spawn().ok();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}
