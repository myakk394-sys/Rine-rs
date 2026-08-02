//! Vulkan Bridge Logging Policy
//! Provides thread-safe, rate-limited logging for Vulkan functions.

use std::sync::atomic::{AtomicUsize, Ordering};

pub struct VulkanLogger;

impl VulkanLogger {
    pub fn log_first_n(tag: &str, msg: &str, max_count: usize) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        if count < max_count {
            println!("[RINE VULKAN BRIDGE] [{}] {}", tag, msg);
        }
    }

    pub fn log_always(tag: &str, msg: &str) {
        println!("[RINE VULKAN BRIDGE] [{}] {}", tag, msg);
    }
}
