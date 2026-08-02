/// Strategy for buffering and mapping PE binary memory
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageStrategy {
    /// On-demand page loading directly backed by disk (`mmap` with file descriptor).
    /// Low memory footprint, lazy loading by kernel page faults.
    DiskDemand,

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
