use std::path::Path;

/// Strategy for buffering and mapping PE binary memory
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageStrategy {
    /// On-demand page loading directly backed by disk (`mmap` with file descriptor).
    /// Low memory footprint, lazy loading by kernel page faults.
    DiskDemand,

    /// Hybrid mode: critical PE binaries stay in RAM (unlocked, swappable if needed),
    /// while large game assets, companion data, and DXVK stay on disk via on-demand mmap.
    Hybrid,

    /// Pure RAM-buffered execution.
    /// Pre-caches binary into an anonymous RAM buffer / `memfd_create`.
    /// Zero disk I/O latency during execution.
    RamOnly {
        /// If true, pins pages in physical RAM using `mlock` to prevent swapping.
        lock_in_memory: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct LoadConfig {
    pub strategy: StorageStrategy,
    pub enable_prefetch: bool,
}

impl Default for LoadConfig {
    fn default() -> Self {
        Self {
            strategy: StorageStrategy::RamOnly {
                lock_in_memory: false,
            },
            enable_prefetch: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryAssessment {
    pub available_ram_bytes: u64,
    pub game_size_bytes: u64,
    pub insufficient_ram: bool,
    pub recommended_strategy: StorageStrategy,
}

/// Reads `MemAvailable` from `/proc/meminfo` in bytes.
pub fn get_available_ram_bytes() -> u64 {
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                let kb: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                if kb > 0 {
                    return kb * 1024;
                }
            }
        }
    }
    0
}

fn dir_size_bounded(dir: &Path, depth: usize) -> u64 {
    if depth > 6 {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total: u64 = 0;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_file() {
            total = total.saturating_add(meta.len());
        } else if meta.is_dir() {
            total = total.saturating_add(dir_size_bounded(&entry.path(), depth + 1));
        }
    }
    total
}

/// Estimates total size on disk of the game directory containing `exe_path`.
pub fn estimate_game_dir_bytes(exe_path: &Path) -> u64 {
    if let Some(parent) = exe_path.parent() {
        let total = dir_size_bounded(parent, 0);
        if total > 0 {
            return total;
        }
    }
    std::fs::metadata(exe_path).map(|m| m.len()).unwrap_or(0)
}

/// Evaluates whether host available RAM can comfortably fit the game in `RamOnly` mode.
/// If not, recommends `StorageStrategy::Hybrid` (critical PE in RAM, assets on disk).
pub fn assess_memory_for_game(exe_path: &Path, requested: StorageStrategy) -> MemoryAssessment {
    let available = get_available_ram_bytes();
    let game_size = estimate_game_dir_bytes(exe_path);
    let insufficient = if available > 0 && game_size > 0 {
        // Keep at least 3 GB of available RAM as headroom for OS + GPU driver + DXVK
        const SYSTEM_HEADROOM_BYTES: u64 = 3 * 1024 * 1024 * 1024;
        available <= SYSTEM_HEADROOM_BYTES
            || game_size > available.saturating_sub(SYSTEM_HEADROOM_BYTES)
    } else {
        false
    };
    let recommended = if insufficient && matches!(requested, StorageStrategy::RamOnly { .. }) {
        StorageStrategy::Hybrid
    } else {
        requested
    };
    MemoryAssessment {
        available_ram_bytes: available,
        game_size_bytes: game_size,
        insufficient_ram: insufficient,
        recommended_strategy: recommended,
    }
}

/// Starts a background monitor in Hybrid mode that trims clean file-backed caches
/// whenever available system RAM drops below `min_free_bytes` (e.g. 3 GB).
pub fn start_hybrid_ram_headroom_guard(min_free_bytes: u64) {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("rine-ram-guard".to_string())
        .spawn(move || {
            let mut last_trim = std::time::Instant::now() - std::time::Duration::from_secs(10);
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let avail = get_available_ram_bytes();
                if avail > 0
                    && avail < min_free_bytes
                    && last_trim.elapsed() >= std::time::Duration::from_secs(4)
                {
                    crate::dlls::kernel32::trim_hybrid_file_caches();
                    unsafe {
                        libc::malloc_trim(0);
                    }
                    last_trim = std::time::Instant::now();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_ram_nonzero() {
        assert!(get_available_ram_bytes() > 0);
    }
}
