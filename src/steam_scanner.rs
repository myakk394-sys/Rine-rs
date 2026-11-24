//! Steam & Application Auto-Discovery Engine for Rine v0.2.0
//!
//! Provides comprehensive discovery of Steam games across multiple library folders
//! (Native, Flatpak, Snap, external storage), parses VDF library configurations
//! and ACF manifests with StateFlags support, inspects PE32/PE32+/ELF binary headers,
//! and accurately detects game engines (Godot 3/4 with embedded PCK, Unity, Unreal,
//! GameMaker, Source, Ren'Py) with unified compatibility metadata.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

// Binary format and architecture definitions
// Определения форматов бинарных файлов и архитектур

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BinaryFormat {
    Pe32,      // Windows 32-bit PE
    Pe32Plus,  // Windows 64-bit PE (PE32+)
    Elf64,     // Linux 64-bit ELF
    Elf32,     // Linux 32-bit ELF
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BinaryArchitecture {
    X86_64,
    X86_32,
    Arm64,
    Unknown(u16),
}

impl BinaryArchitecture {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::X86_64 => "64-bit (x86_64)",
            Self::X86_32 => "32-bit (x86)",
            Self::Arm64 => "64-bit (ARM64)",
            Self::Unknown(_) => "Unknown Architecture",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BinaryInspection {
    pub format: BinaryFormat,
    pub arch: BinaryArchitecture,
    pub is_win64: bool,
    pub details: String,
}

/// Inspects executable file headers directly (PE32, PE32+, ELF32, ELF64).
/// Never guesses architecture by filename or host OS.
pub fn inspect_binary(path: &Path) -> BinaryInspection {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            return BinaryInspection {
                format: BinaryFormat::Unknown,
                arch: BinaryArchitecture::Unknown(0),
                is_win64: false,
                details: format!("Не удалось открыть файл: {}", e),
            };
        }
    };

    let mut header = [0u8; 64];
    let bytes_read = match file.read(&mut header) {
        Ok(n) => n,
        Err(e) => {
            return BinaryInspection {
                format: BinaryFormat::Unknown,
                arch: BinaryArchitecture::Unknown(0),
                is_win64: false,
                details: format!("Ошибка чтения заголовка: {}", e),
            };
        }
    };

    if bytes_read < 4 {
        return BinaryInspection {
            format: BinaryFormat::Unknown,
            arch: BinaryArchitecture::Unknown(0),
            is_win64: false,
            details: "Файл поврежден (размер меньше 4 байт)".into(),
        };
    }

    // 1. Check for Linux ELF: [0x7F, 'E', 'L', 'F']
    if header[0] == 0x7F && header[1] == b'E' && header[2] == b'L' && header[3] == b'F' {
        let is_64 = header.get(4).copied().unwrap_or(2) == 2;
        let format = if is_64 {
            BinaryFormat::Elf64
        } else {
            BinaryFormat::Elf32
        };

        // Read ELF e_machine at offset 0x12 (little-endian)
        let arch = if bytes_read >= 20 {
            let e_machine = u16::from_le_bytes([header[0x12], header[0x13]]);
            match e_machine {
                0x3E => BinaryArchitecture::X86_64, // AMD x86-64
                0x03 => BinaryArchitecture::X86_32, // Intel 80386
                0xB7 => BinaryArchitecture::Arm64,  // ARM AArch64
                other => BinaryArchitecture::Unknown(other),
            }
        } else {
            BinaryArchitecture::Unknown(0)
        };

        return BinaryInspection {
            format,
            arch,
            is_win64: false,
            details: format!(
                "Нативный исполняемый файл Linux ({}, {})",
                if is_64 { "ELF64" } else { "ELF32" },
                arch.as_str()
            ),
        };
    }

    // 2. Check for Windows PE: DOS Header 'MZ'
    if header[0] == b'M' && header[1] == b'Z' && bytes_read >= 64 {
        let e_lfanew =
            u32::from_le_bytes([header[60], header[61], header[62], header[63]]) as u64;

        if file.seek(SeekFrom::Start(e_lfanew)).is_ok() {
            let mut pe_header = [0u8; 26];
            if file.read_exact(&mut pe_header).is_ok() && &pe_header[0..4] == b"PE\0\0" {
                let machine = u16::from_le_bytes([pe_header[4], pe_header[5]]);
                // Optional header magic is at offset 24
                let opt_magic = u16::from_le_bytes([pe_header[24], pe_header[25]]);

                let (format, arch, is_win64) = match (machine, opt_magic) {
                    (0x8664, 0x020B) => {
                        (BinaryFormat::Pe32Plus, BinaryArchitecture::X86_64, true)
                    }
                    (0x014C, 0x010B) => {
                        (BinaryFormat::Pe32, BinaryArchitecture::X86_32, false)
                    }
                    (0xAA64, 0x020B) => {
                        (BinaryFormat::Pe32Plus, BinaryArchitecture::Arm64, false)
                    }
                    (0x8664, _) => {
                        (BinaryFormat::Pe32Plus, BinaryArchitecture::X86_64, true)
                    }
                    (0x014C, _) => {
                        (BinaryFormat::Pe32, BinaryArchitecture::X86_32, false)
                    }
                    (other, magic) => (
                        if magic == 0x020B {
                            BinaryFormat::Pe32Plus
                        } else {
                            BinaryFormat::Pe32
                        },
                        BinaryArchitecture::Unknown(other),
                        false,
                    ),
                };

                return BinaryInspection {
                    format,
                    arch,
                    is_win64,
                    details: format!(
                        "Исполняемый файл Windows ({}, {})",
                        if is_win64 { "PE32+" } else { "PE32" },
                        arch.as_str()
                    ),
                };
            }
        }
    }

    BinaryInspection {
        format: BinaryFormat::Unknown,
        arch: BinaryArchitecture::Unknown(0),
        is_win64: false,
        details: "Неизвестный или поврежденный формат исполняемого файла".into(),
    }
}

// Game engine definitions and identification
// Определения игровых движков и методы их распознавания

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GodotVersion {
    V3,
    V4,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GameEngine {
    Unity { version: Option<String> },
    Godot { version: GodotVersion },
    UnrealEngine { version: Option<String> },
    GameMaker,
    Source,
    RenPy,
    CustomNative,
    Unknown,
}

impl GameEngine {
    pub fn display_name(&self) -> String {
        match self {
            Self::Unity { version } => {
                if let Some(v) = version {
                    format!("Unity ({})", v)
                } else {
                    "Unity Engine".into()
                }
            }
            Self::Godot { version } => match version {
                GodotVersion::V4 => "Godot Engine 4.x".into(),
                GodotVersion::V3 => "Godot Engine 3.x".into(),
                GodotVersion::Unknown => "Godot Engine".into(),
            },
            Self::UnrealEngine { version } => {
                if let Some(v) = version {
                    format!("Unreal Engine ({})", v)
                } else {
                    "Unreal Engine 4/5".into()
                }
            }
            Self::GameMaker => "GameMaker Studio".into(),
            Self::Source => "Source Engine".into(),
            Self::RenPy => "Ren'Py Visual Novel Engine".into(),
            Self::CustomNative => "Win64 Application".into(),
            Self::Unknown => "Unknown Engine".into(),
        }
    }
}

// Game compatibility and metadata status
// Статусы совместимости игр и метаданные

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompatibilityStatus {
    Supported,
    UnsupportedArchitecture,
    UnsupportedEngine,
    AntiCheatProtected,
    LinuxNative,
    MissingExecutable,
    InstallationIncomplete,
    DetectionError,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameCompatibility {
    pub engine: GameEngine,
    pub engine_name: String,
    pub binary_format: BinaryFormat,
    pub architecture: BinaryArchitecture,
    pub status: CompatibilityStatus,
    pub is_supported: bool,
    pub status_text: String,
    pub badge_color: (u8, u8, u8),
    pub details: String,
}

impl GameCompatibility {
    pub fn localized_status_text(&self, is_en: bool) -> String {
        if !is_en {
            return self.status_text.clone();
        }
        match self.status {
            CompatibilityStatus::Supported => {
                match &self.engine {
                    GameEngine::Unity { .. } => "Fully Supported (Unity)".into(),
                    GameEngine::Godot { .. } => "Fully Supported (Godot)".into(),
                    GameEngine::GameMaker => "Supported (GameMaker)".into(),
                    GameEngine::Source => "Supported (Source)".into(),
                    GameEngine::RenPy => "Supported (Ren'Py)".into(),
                    GameEngine::CustomNative => "Supported (Win64 Native)".into(),
                    _ => "Supported".into(),
                }
            }
            CompatibilityStatus::UnsupportedArchitecture => {
                format!("Unsupported ({})", self.architecture.as_str())
            }
            CompatibilityStatus::UnsupportedEngine => {
                format!("Unsupported ({})", self.engine_name)
            }
            CompatibilityStatus::AntiCheatProtected => {
                "Unsupported (Kernel Anti-Cheat)".into()
            }
            CompatibilityStatus::LinuxNative => {
                format!("Native Linux ({})", self.architecture.as_str())
            }
            CompatibilityStatus::MissingExecutable => {
                "File Not Found".into()
            }
            CompatibilityStatus::InstallationIncomplete => {
                "Incomplete Installation".into()
            }
            CompatibilityStatus::DetectionError => {
                "Detection Error".into()
            }
        }
    }

    pub fn localized_details(&self, is_en: bool) -> String {
        if !is_en {
            return self.details.clone();
        }
        match self.status {
            CompatibilityStatus::Supported => {
                match &self.engine {
                    GameEngine::Unity { .. } => "Unity engine fully supported: Vulkan/DXVK graphics bridge, Mono/IL2CPP runtime translation, windowing & input synchronization.".into(),
                    GameEngine::Godot { .. } => "Godot engine fully supported: Vulkan renderer, virtual filesystem translation (including embedded PCK archives).".into(),
                    GameEngine::GameMaker => "GameMaker Studio supported via DirectX 11 -> Vulkan translation pipeline.".into(),
                    GameEngine::Source => "Source engine supported via DirectX 9 -> Vulkan translation pipeline.".into(),
                    GameEngine::RenPy => "Ren'Py visual novel engine supported via Python/SDL runtime bridge.".into(),
                    GameEngine::CustomNative => "Win64 standalone PE application with Direct3D / Vulkan graphics pipeline.".into(),
                    _ => "Game engine is supported by Rine Win64 Translation Engine.".into(),
                }
            }
            CompatibilityStatus::UnsupportedArchitecture => {
                format!("This game was compiled for {}. Rine v0.2.0 translates 64-bit (x86_64) Windows PE32+ binaries.", self.architecture.as_str())
            }
            CompatibilityStatus::UnsupportedEngine => {
                format!("{} engine games require kernel-level hooks or specialized features currently in active development.", self.engine_name)
            }
            CompatibilityStatus::AntiCheatProtected => {
                "Kernel-level driver anti-cheat detected (EasyAntiCheat / BattlEye). Games with kernel anti-cheat cannot run via user-space PE translation.".into()
            }
            CompatibilityStatus::LinuxNative => {
                "This binary is built for native Linux (ELF). Rine is for Windows PE32+ (x86_64). Run this executable directly in Linux.".into()
            }
            CompatibilityStatus::MissingExecutable => {
                "The game executable does not exist at the specified path.".into()
            }
            CompatibilityStatus::InstallationIncomplete => {
                "Game files or engine runtime data directory is missing or corrupt.".into()
            }
            CompatibilityStatus::DetectionError => {
                "Could not parse PE executable headers.".into()
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredGame {
    pub name: String,
    pub appid: u32,
    pub install_dir: PathBuf,
    pub exe_path: PathBuf,
    pub is_steam: bool,
    pub icon_hint: Option<String>,
}

/// Resolves cover art / banner / poster for a game from Steam librarycache or game directory
pub fn resolve_game_cover_art(appid: u32, game_dir: &Path) -> Option<String> {
    if appid != 0 {
        let roots = get_steam_candidate_roots();
        for root in roots {
            let cache_dir = root.join("appcache").join("librarycache").join(appid.to_string());
            if cache_dir.exists() {
                let priorities = [
                    "header.jpg",
                    "library_header.jpg",
                    "library_capsule.jpg",
                    "library_600x900.jpg",
                    "library_hero.jpg",
                    "logo.png",
                ];
                for pri in priorities {
                    let direct = cache_dir.join(pri);
                    if direct.exists() {
                        return Some(direct.to_string_lossy().to_string());
                    }
                }
                if let Ok(entries) = fs::read_dir(&cache_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            for pri in priorities {
                                let sub = path.join(pri);
                                if sub.exists() {
                                    return Some(sub.to_string_lossy().to_string());
                                }
                            }
                        } else if let Some(ext) = path.extension() {
                            if (ext == "jpg" || ext == "png")
                                && path.metadata().map(|m| m.len() > 1000).unwrap_or(false)
                            {
                                return Some(path.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
            let icon_file = root.join("steam").join("games").join(format!("{}.ico", appid));
            if icon_file.exists() {
                return Some(icon_file.to_string_lossy().to_string());
            }
        }
    }

    for file in &["header.jpg", "cover.jpg", "cover.png", "icon.png"] {
        let p = game_dir.join(file);
        if p.exists() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}



// Engine detection logic (including embedded Godot PCK packages)
// Логика определения движков (включая встроенные пакеты Godot PCK)

/// Detects if an executable contains an embedded Godot PCK (`GDPC` magic)
/// or standalone Godot signatures.
pub fn detect_embedded_godot_pck(exe_path: &Path) -> Option<GodotVersion> {
    let mut file = fs::File::open(exe_path).ok()?;
    let file_len = file.metadata().ok()?.len();
    if file_len < 64 {
        return None;
    }

    // Godot PCKs embed 'GDPC' (0x47, 0x44, 0x50, 0x43).
    // Often located either in the last 64KB (appended package) or in the first few MBs.
    // 1. Check last 64KB first
    let scan_tail_len = (64 * 1024).min(file_len);
    let _ = file.seek(SeekFrom::End(-(scan_tail_len as i64)));
    let mut tail_buf = vec![0u8; scan_tail_len as usize];
    if file.read_exact(&mut tail_buf).is_ok() {
        if let Some(pos) = tail_buf.windows(4).position(|w| w == b"GDPC") {
            // Check PCK format version
            let version = if pos + 16 <= tail_buf.len() {
                let major = u32::from_le_bytes([
                    tail_buf[pos + 8],
                    tail_buf[pos + 9],
                    tail_buf[pos + 10],
                    tail_buf[pos + 11],
                ]);
                if major == 4 {
                    GodotVersion::V4
                } else if major == 3 {
                    GodotVersion::V3
                } else {
                    GodotVersion::Unknown
                }
            } else {
                GodotVersion::Unknown
            };
            return Some(version);
        }
    }

    // 2. Scan first 4MB for Godot Engine strings
    let scan_head_len = (4 * 1024 * 1024).min(file_len);
    let _ = file.seek(SeekFrom::Start(0));
    let mut head_buf = vec![0u8; scan_head_len as usize];
    if file.read_exact(&mut head_buf).is_ok() {
        if head_buf.windows(4).any(|w| w == b"GDPC") {
            if head_buf.windows(15).any(|w| w == b"Godot Engine v4") {
                return Some(GodotVersion::V4);
            }
            if head_buf.windows(15).any(|w| w == b"Godot Engine v3") {
                return Some(GodotVersion::V3);
            }
            return Some(GodotVersion::Unknown);
        }
    }

    None
}

/// Detects the engine and compatibility status of a game executable.
pub fn check_game_compatibility(exe_path: &Path) -> GameCompatibility {
    if !exe_path.exists() {
        return GameCompatibility {
            engine: GameEngine::Unknown,
            engine_name: "Неизвестно".into(),
            binary_format: BinaryFormat::Unknown,
            architecture: BinaryArchitecture::Unknown(0),
            status: CompatibilityStatus::MissingExecutable,
            is_supported: false,
            status_text: "Файл не найден".into(),
            badge_color: (220, 50, 50),
            details: "Исполняемый файл не существует по указанному пути.".into(),
        };
    }

    let inspection = inspect_binary(exe_path);
    let parent = exe_path.parent().unwrap_or(Path::new(""));
    let stem = exe_path.file_stem().unwrap_or_default().to_string_lossy();

    // Check for Anti-cheat drivers
    let has_anticheat = parent.join("EasyAntiCheat").exists()
        || parent.join("EasyAntiCheat_Setup.exe").exists()
        || parent.join("start_protected_game.exe").exists()
        || parent.join("BattlEye").exists()
        || parent.join("BEService.exe").exists();

    if has_anticheat {
        return GameCompatibility {
            engine: GameEngine::Unknown,
            engine_name: "Anti-Cheat Protected".into(),
            binary_format: inspection.format,
            architecture: inspection.arch,
            status: CompatibilityStatus::AntiCheatProtected,
            is_supported: false,
            status_text: "Не поддерживается (Античит)".into(),
            badge_color: (220, 50, 50),
            details: "Обнаружен драйверный античит уровня ядра (EasyAntiCheat / BattlEye). Онлайн-игры с античитом не могут запускаться через пользовательскую трансляцию.".into(),
        };
    }

    // Check for Linux Native ELF executable
    if inspection.format == BinaryFormat::Elf64 || inspection.format == BinaryFormat::Elf32 {
        // Detect if this Linux binary is a Godot or Unity game
        let engine = if let Some(v) = detect_embedded_godot_pck(exe_path) {
            GameEngine::Godot { version: v }
        } else if parent.join(format!("{}.pck", stem)).exists() {
            GameEngine::Godot {
                version: GodotVersion::Unknown,
            }
        } else {
            GameEngine::Unknown
        };

        return GameCompatibility {
            engine,
            engine_name: "Linux Native (ELF)".into(),
            binary_format: inspection.format,
            architecture: inspection.arch,
            status: CompatibilityStatus::LinuxNative,
            is_supported: false,
            status_text: format!("Нативный Linux ({})", inspection.arch.as_str()),
            badge_color: (180, 100, 240),
            details: "Исполняемый файл собран для Linux (ELF). Rine предназначен для Windows PE32+ (x86_64). Запустите этот файл напрямую в Linux.".into(),
        };
    }

    // Check for 32-bit Windows PE (x86) or ARM64
    if !inspection.is_win64 {
        let arch_str = inspection.arch.as_str();
        return GameCompatibility {
            engine: GameEngine::Unknown,
            engine_name: arch_str.into(),
            binary_format: inspection.format,
            architecture: inspection.arch,
            status: CompatibilityStatus::UnsupportedArchitecture,
            is_supported: false,
            status_text: format!("Не поддерживается ({})", arch_str),
            badge_color: (220, 60, 60),
            details: format!(
                "Игра собрана под архитектуру {}. Rine v0.2.0 работает с 64-битными (x86_64) приложениями Windows.",
                arch_str
            ),
        };
    }

    // Check for Unreal Engine 4 / 5
    let has_ue_dir = parent.join("Engine").exists()
        || parent
            .parent()
            .map(|p| p.join("Engine").exists())
            .unwrap_or(false)
        || exe_path.to_string_lossy().contains("Binaries/Win64")
        || exe_path.to_string_lossy().contains("Binaries\\Win64");

    let has_ue_dlls = parent.join("UE4PrereqSetup_x64.exe").exists()
        || parent.join("UnrealCEFSubProcess.exe").exists()
        || parent.join("EpicGames").exists();

    if has_ue_dir || has_ue_dlls {
        return GameCompatibility {
            engine: GameEngine::UnrealEngine { version: None },
            engine_name: "Unreal Engine 4/5".into(),
            binary_format: inspection.format,
            architecture: inspection.arch,
            status: CompatibilityStatus::UnsupportedEngine,
            is_supported: false,
            status_text: "Не поддерживается (Unreal Engine)".into(),
            badge_color: (220, 50, 50),
            details: "Движок Unreal Engine 4/5 требует специфичные вызовы ядра Windows и расширенную трансляцию DX12/Nanite. Поддержка Unreal Engine находится в активной разработке.".into(),
        };
    }

    // Check for Unity
    let has_unity_player = parent.join("UnityPlayer.dll").exists()
        || parent.join("unityplayer.dll").exists()
        || parent
            .parent()
            .map(|p| p.join("UnityPlayer.dll").exists())
            .unwrap_or(false);
    let has_unity_data = parent.join(format!("{}_Data", stem)).exists()
        || parent.join("Data").exists()
        || parent.join("MonoBleedingEdge").exists()
        || parent.join("GameAssembly.dll").exists();

    if has_unity_player || has_unity_data {
        // Read Unity version from globalgamemanagers if available
        let mut unity_version = None;
        let data_dir = parent.join(format!("{}_Data", stem));
        let ggm_path = data_dir.join("globalgamemanagers");
        if let Ok(mut f) = fs::File::open(&ggm_path) {
            let mut buf = [0u8; 128];
            if f.read_exact(&mut buf).is_ok() {
                if let Some(pos) = buf.windows(4).position(|w| w.starts_with(b"20") || w.starts_with(b"60")) {
                    let end = buf[pos..]
                        .iter()
                        .position(|&b| b == 0 || b < 32)
                        .unwrap_or(buf.len() - pos);
                    if let Ok(ver_str) = std::str::from_utf8(&buf[pos..pos + end]) {
                        unity_version = Some(ver_str.to_string());
                    }
                }
            }
        }

        let engine = GameEngine::Unity {
            version: unity_version,
        };
        let engine_name = engine.display_name();

        return GameCompatibility {
            engine,
            engine_name,
            binary_format: inspection.format,
            architecture: inspection.arch,
            status: CompatibilityStatus::Supported,
            is_supported: true,
            status_text: "Полная поддержка (Unity)".into(),
            badge_color: (50, 205, 50),
            details: "Движок Unity полностью поддерживается: прямой мост Vulkan/DXVK, трансляция Mono/IL2CPP, синхронизация окон и ввода.".into(),
        };
    }

    // Check for Godot (loose .pck OR embedded GDPC)
    let has_godot_pck_file = parent.join(format!("{}.pck", stem)).exists()
        || parent
            .read_dir()
            .map(|entries| {
                entries
                    .flatten()
                    .any(|e| e.path().extension().map(|ext| ext == "pck").unwrap_or(false))
            })
            .unwrap_or(false);

    let embedded_godot = detect_embedded_godot_pck(exe_path);

    if has_godot_pck_file || embedded_godot.is_some() {
        let version = embedded_godot.unwrap_or(GodotVersion::Unknown);
        let engine = GameEngine::Godot { version };
        let engine_name = engine.display_name();

        return GameCompatibility {
            engine,
            engine_name,
            binary_format: inspection.format,
            architecture: inspection.arch,
            status: CompatibilityStatus::Supported,
            is_supported: true,
            status_text: "Полная поддержка (Godot)".into(),
            badge_color: (50, 205, 50),
            details: "Движок Godot полностью поддерживается через нативный рендерер Vulkan и трансляцию виртуальной файловой системы (включая встроенные PCK архивы).".into(),
        };
    }

    // Check for GameMaker
    let has_gamemaker = parent.join("data.win").exists()
        || parent
            .read_dir()
            .map(|entries| {
                entries.flatten().any(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with("audiogroup")
                })
            })
            .unwrap_or(false);

    if has_gamemaker {
        return GameCompatibility {
            engine: GameEngine::GameMaker,
            engine_name: "GameMaker Studio".into(),
            binary_format: inspection.format,
            architecture: inspection.arch,
            status: CompatibilityStatus::Supported,
            is_supported: true,
            status_text: "Поддерживается (GameMaker)".into(),
            badge_color: (80, 200, 120),
            details: "Движок GameMaker Studio поддерживается через трансляцию DirectX 11 в Vulkan.".into(),
        };
    }

    // Check for Source Engine
    let has_source = parent.join("tier0.dll").exists()
        || parent.join("vstdlib.dll").exists()
        || parent.join("bin/tier0.dll").exists();
    if has_source {
        return GameCompatibility {
            engine: GameEngine::Source,
            engine_name: "Source Engine".into(),
            binary_format: inspection.format,
            architecture: inspection.arch,
            status: CompatibilityStatus::Supported,
            is_supported: true,
            status_text: "Поддерживается (Source)".into(),
            badge_color: (100, 200, 100),
            details: "Движок Source Engine поддерживается через трансляцию DirectX 9/11 в Vulkan.".into(),
        };
    }

    // Check for Ren'Py
    let has_renpy = parent.join("renpy").is_dir()
        || parent
            .read_dir()
            .map(|entries| {
                entries
                    .flatten()
                    .any(|e| e.path().extension().map(|ext| ext == "rpa").unwrap_or(false))
            })
            .unwrap_or(false);

    if has_renpy {
        return GameCompatibility {
            engine: GameEngine::RenPy,
            engine_name: "Ren'Py".into(),
            binary_format: inspection.format,
            architecture: inspection.arch,
            status: CompatibilityStatus::Supported,
            is_supported: true,
            status_text: "Поддерживается (Ren'Py)".into(),
            badge_color: (100, 200, 100),
            details: "Движок визуальных новелл Ren'Py поддерживается.".into(),
        };
    }

    // Standard 64-bit Windows PE
    GameCompatibility {
        engine: GameEngine::CustomNative,
        engine_name: "Win64 Application".into(),
        binary_format: inspection.format,
        architecture: inspection.arch,
        status: CompatibilityStatus::Supported,
        is_supported: true,
        status_text: "Совместимо (Базовый Win64)".into(),
        badge_color: (70, 170, 255),
        details: "Стандартный 64-битный исполняемый файл Windows. KERNEL32, USER32, NTDLL, GDI32 и DXVK поддерживаются.".into(),
    }
}

// Steam library discovery (multi-library, Flatpak, Snap, external drives)
// Поиск библиотек Steam (несколько каталогов, Flatpak, Snap, внешние накопители)

const IGNORED_APP_IDS: &[u32] = &[
    228980,  // Steamworks Common Redistributables
    1070560, // Steam Linux Runtime
    1391110, // Steam Linux Runtime - Soldier
    1493710, // Proton Experimental
    1887720, // Proton 7.0
    2180100, // Proton 8.0
    2805730, // Proton 9.0
    250820,  // SteamVR
];

fn get_home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home/fizzy"))
}

/// Candidate Steam installation root directories across distributions.
fn get_steam_candidate_roots() -> Vec<PathBuf> {
    let home = get_home_dir();
    vec![
        // Native Arch / Debian / Fedora / Ubuntu
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
        // Flatpak Steam
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/.steam/steam"),
        // Snap Steam
        home.join("snap/steam/common/.local/share/Steam"),
    ]
}

/// Discovers all Steam library folders across all candidate roots and libraryfolders.vdf.
pub fn discover_steam_library_folders() -> Vec<PathBuf> {
    let mut libraries = Vec::new();

    for root in get_steam_candidate_roots() {
        if !root.exists() {
            continue;
        }

        let root_steamapps = root.join("steamapps");
        if root_steamapps.exists() && !libraries.contains(&root) {
            libraries.push(root.clone());
        }

        // Check libraryfolders.vdf
        let vdf_path = root_steamapps.join("libraryfolders.vdf");
        if let Ok(content) = fs::read_to_string(&vdf_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("\"path\"") {
                    let parts: Vec<&str> = trimmed.split('"').collect();
                    if parts.len() >= 4 {
                        let path_str = parts[3];
                        let path = PathBuf::from(path_str);
                        if path.exists() && !libraries.contains(&path) {
                            libraries.push(path);
                        }
                    }
                }
            }
        }
    }

    libraries
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcfManifest {
    pub appid: u32,
    pub name: String,
    pub installdir: String,
    pub state_flags: u32,
    pub is_fully_installed: bool,
}

/// Parses an appmanifest_<id>.acf file with StateFlags validation.
pub fn parse_acf_manifest(content: &str) -> Option<AcfManifest> {
    let mut appid = None;
    let mut name = None;
    let mut installdir = None;
    let mut state_flags = 4u32; // Default to fully installed (flag 4)

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"appid\"") {
            let parts: Vec<&str> = trimmed.split('"').collect();
            if parts.len() >= 4 {
                appid = parts[3].parse::<u32>().ok();
            }
        } else if trimmed.starts_with("\"name\"") {
            let parts: Vec<&str> = trimmed.split('"').collect();
            if parts.len() >= 4 {
                name = Some(parts[3].to_string());
            }
        } else if trimmed.starts_with("\"installdir\"") {
            let parts: Vec<&str> = trimmed.split('"').collect();
            if parts.len() >= 4 {
                installdir = Some(parts[3].to_string());
            }
        } else if trimmed.starts_with("\"StateFlags\"") {
            let parts: Vec<&str> = trimmed.split('"').collect();
            if parts.len() >= 4 {
                if let Ok(flags) = parts[3].parse::<u32>() {
                    state_flags = flags;
                }
            }
        }
    }

    if let (Some(aid), Some(n), Some(dir)) = (appid, name, installdir) {
        // Flag 4: StateFullyInstalled. Flags with 1024 or 2 are updating/downloading.
        let is_fully_installed = (state_flags & 4) != 0;
        Some(AcfManifest {
            appid: aid,
            name: n,
            installdir: dir,
            state_flags,
            is_fully_installed,
        })
    } else {
        None
    }
}

pub fn parse_acf_file(content: &str) -> Option<(u32, String, String)> {
    parse_acf_manifest(content).map(|m| (m.appid, m.name, m.installdir))
}

/// Locates the primary game executable (PE .exe preferred; .x86_64 fallback)
pub fn find_primary_executable(dir: &Path, game_name: &str) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }

    let mut candidates = Vec::new();
    collect_exes(dir, 0, 3, &mut candidates);

    if candidates.is_empty() {
        return None;
    }

    let norm_game_name = game_name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();

    candidates.sort_by(|a, b| {
        let a_score = score_exe_candidate(a, &norm_game_name);
        let b_score = score_exe_candidate(b, &norm_game_name);
        b_score.cmp(&a_score)
    });

    candidates.into_iter().next()
}

fn collect_exes(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ext_str.eq_ignore_ascii_case("exe") || ext_str.eq_ignore_ascii_case("x86_64") {
                    let file_lower = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase();
                    // Filter out crash handlers, redistributables and uninstallers
                    if !file_lower.contains("crashhandler")
                        && !file_lower.contains("unitycrash")
                        && !file_lower.starts_with("unins")
                        && !file_lower.contains("dxsetup")
                        && !file_lower.contains("vcredist")
                        && !file_lower.contains("vc_redist")
                    {
                        out.push(path);
                    }
                }
            }
        } else if path.is_dir() {
            collect_exes(&path, depth + 1, max_depth, out);
        }
    }
}

fn score_exe_candidate(path: &Path, norm_game_name: &str) -> u64 {
    let mut score = 0u64;
    let file_stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let norm_stem = file_stem
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();

    if norm_stem == norm_game_name {
        score += 100_000_000;
    } else if norm_stem.contains(norm_game_name) || norm_game_name.contains(&norm_stem) {
        score += 50_000_000;
    }

    // Prefer Windows .exe over Linux binaries for Rine
    if let Some(ext) = path.extension() {
        if ext.eq_ignore_ascii_case("exe") {
            score += 20_000_000;
        }
    }

    // Prefer non-unpacked if original exists
    if file_stem.contains("unpacked") {
        score = score.saturating_sub(10_000);
    }

    // Add file size in MB as tie breaker (main game binaries are larger)
    if let Ok(meta) = fs::metadata(path) {
        let size_mb = meta.len() / (1024 * 1024);
        score += size_mb.min(10_000);
    }

    score
}

/// Automatically scans for all installed Steam games across all libraries.
pub fn scan_steam_games() -> Vec<DiscoveredGame> {
    let mut games = Vec::new();
    let libraries = discover_steam_library_folders();

    for lib in libraries {
        let steamapps = lib.join("steamapps");
        let Ok(entries) = fs::read_dir(&steamapps) else { continue };

        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.starts_with("appmanifest_") && filename.ends_with(".acf") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Some(manifest) = parse_acf_manifest(&content) {
                        let appid = manifest.appid;
                        let name = manifest.name;
                        let installdir = manifest.installdir;

                        if IGNORED_APP_IDS.contains(&appid) {
                            continue;
                        }
                        if name.contains("Steam Linux Runtime")
                            || name.contains("Proton")
                            || name.contains("Steamworks Common")
                        {
                            continue;
                        }



                        let game_dir = steamapps.join("common").join(&installdir);
                        if let Some(exe) = find_primary_executable(&game_dir, &name) {
                            // Check for duplicates
                            if !games.iter().any(|g: &DiscoveredGame| g.appid == appid) {
                                let icon_hint = resolve_game_cover_art(appid, &game_dir);
                                games.push(DiscoveredGame {
                                    name,
                                    appid,
                                    install_dir: game_dir,
                                    exe_path: exe,
                                    is_steam: true,
                                    icon_hint,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    games
}

// Custom user games management (persistence in custom_games.json)
// Управление добавленными пользователем играми (сохранение в custom_games.json)

fn get_config_file_path() -> PathBuf {
    let conf_dir = get_home_dir().join(".config").join("rine");
    let _ = fs::create_dir_all(&conf_dir);
    conf_dir.join("custom_games.json")
}

pub fn load_custom_games() -> Vec<DiscoveredGame> {
    let file = get_config_file_path();
    if let Ok(content) = fs::read_to_string(file) {
        if let Ok(games) = serde_json::from_str::<Vec<DiscoveredGame>>(&content) {
            return games.into_iter().filter(|g| g.exe_path.exists()).collect();
        }
    }
    Vec::new()
}

pub fn save_custom_games(games: &[DiscoveredGame]) {
    let file = get_config_file_path();
    if let Ok(data) = serde_json::to_string_pretty(games) {
        let _ = fs::write(file, data);
    }
}

pub fn add_custom_game(path: PathBuf) -> Option<DiscoveredGame> {
    if !path.exists() {
        return None;
    }

    let raw_name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let name = raw_name.replace('_', " ").replace('.', " ");
    let install_dir = path.parent().unwrap_or(&path).to_path_buf();

    let icon_hint = resolve_game_cover_art(0, &install_dir);
    let new_game = DiscoveredGame {
        name,
        appid: 0,
        install_dir,
        exe_path: path.clone(),
        is_steam: false,
        icon_hint,
    };

    let mut custom = load_custom_games();
    custom.retain(|g| g.exe_path != path);
    custom.push(new_game.clone());
    save_custom_games(&custom);

    Some(new_game)
}

pub fn remove_custom_game(exe_path: &Path) {
    let mut custom = load_custom_games();
    custom.retain(|g| g.exe_path != exe_path);
    save_custom_games(&custom);
}

/// Retrieves the complete game library (Discovered Steam games + Custom games).
pub fn get_all_games() -> Vec<DiscoveredGame> {
    let mut list = scan_steam_games();
    let custom = load_custom_games();

    for c in custom {
        if !list.iter().any(|g| g.exe_path == c.exe_path) {
            list.push(c);
        }
    }

    list
}

/// Dynamic rescan trigger for Steam libraries and custom games.
pub fn refresh_steam_games() -> Vec<DiscoveredGame> {
    get_all_games()
}

// Unit tests
// Модульные тесты

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steam_game_scanner() {
        let games = scan_steam_games();
        println!("Discovered {} games from Steam:", games.len());
        for g in &games {
            println!("  -> [{}] {} | exe: {:?}", g.appid, g.name, g.exe_path);
        }
        assert!(!games.is_empty(), "Should discover at least one installed Steam game");
    }

    #[test]
    fn test_game_engine_compatibility_checker() {
        let games = get_all_games();
        for g in &games {
            let compat = check_game_compatibility(&g.exe_path);
            println!(
                "Game: {} | Engine: {} | Format: {:?} | Arch: {:?} | Supported: {} | Status: {}",
                g.name,
                compat.engine_name,
                compat.binary_format,
                compat.architecture,
                compat.is_supported,
                compat.status_text
            );
            if g.name.to_lowercase().contains("how to fish") {
                assert!(matches!(compat.engine, GameEngine::Unity { .. }));
                assert!(compat.is_supported);
                assert_eq!(compat.binary_format, BinaryFormat::Pe32Plus);
                assert_eq!(compat.architecture, BinaryArchitecture::X86_64);
            }
        }
    }

    #[test]
    fn test_embedded_godot_detection() {
        let neighbor_path = PathBuf::from(
            "/home/fizzy/Downloads/home/fizzy/Рабочий стол/home/fizzy/Рабочий стол/перенос/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe",
        );
        if neighbor_path.exists() {
            let compat = check_game_compatibility(&neighbor_path);
            println!("That's not my neighbor: {:?}", compat);
            assert!(matches!(compat.engine, GameEngine::Godot { .. }));
            assert!(compat.is_supported);
            assert_eq!(compat.binary_format, BinaryFormat::Pe32Plus);
        }
    }

    #[test]
    fn test_elf_inspection_not_misidentified_as_32bit() {
        let buckshot_elf = PathBuf::from(
            "/home/fizzy/.local/share/Steam/steamapps/common/Buckshot Roulette/Buckshot Roulette_linux/Buckshot Roulette.x86_64",
        );
        if buckshot_elf.exists() {
            let inspection = inspect_binary(&buckshot_elf);
            assert_eq!(inspection.format, BinaryFormat::Elf64);
            assert_eq!(inspection.arch, BinaryArchitecture::X86_64);
            let compat = check_game_compatibility(&buckshot_elf);
            assert_eq!(compat.status, CompatibilityStatus::LinuxNative);
            assert!(!compat.status_text.contains("32-bit"));
        }
    }

    #[test]
    fn test_pe32_32bit_inspection() {
        let spacewar_exe = PathBuf::from(
            "/home/fizzy/.local/share/Steam/steamapps/common/Spacewar/SteamworksExample.exe",
        );
        if spacewar_exe.exists() {
            let inspection = inspect_binary(&spacewar_exe);
            assert_eq!(inspection.format, BinaryFormat::Pe32);
            assert_eq!(inspection.arch, BinaryArchitecture::X86_32);
            let compat = check_game_compatibility(&spacewar_exe);
            assert_eq!(compat.status, CompatibilityStatus::UnsupportedArchitecture);
            assert!(compat.status_text.contains("32-bit"));
        }
    }

    #[test]
    fn test_acf_manifest_parser() {
        let sample = r#"
"AppState"
{
    "appid"     "2835570"
    "Universe"  "1"
    "name"      "Buckshot Roulette"
    "StateFlags" "4"
    "installdir" "Buckshot Roulette"
}
"#;
        let manifest = parse_acf_manifest(sample).expect("Should parse valid ACF");
        assert_eq!(manifest.appid, 2835570);
        assert_eq!(manifest.name, "Buckshot Roulette");
        assert_eq!(manifest.installdir, "Buckshot Roulette");
        assert_eq!(manifest.state_flags, 4);
        assert!(manifest.is_fully_installed);
    }
}
