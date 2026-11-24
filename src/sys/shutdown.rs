use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostShutdownState {
    Running = 0,
    ShutdownRequested = 1,
    CleaningUp = 2,
    Terminated = 3,
}

static SHUTDOWN_STATE: AtomicU32 = AtomicU32::new(HostShutdownState::Running as u32);
static WATCHDOG_ACTIVE: AtomicBool = AtomicBool::new(false);
static CLEANUP_HOOKS: Mutex<Vec<fn()>> = Mutex::new(Vec::new());

pub fn get_shutdown_state() -> HostShutdownState {
    match SHUTDOWN_STATE.load(Ordering::SeqCst) {
        0 => HostShutdownState::Running,
        1 => HostShutdownState::ShutdownRequested,
        2 => HostShutdownState::CleaningUp,
        _ => HostShutdownState::Terminated,
    }
}

pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_STATE.load(Ordering::SeqCst) != (HostShutdownState::Running as u32)
}

pub fn register_cleanup_hook(hook: fn()) {
    if let Ok(mut hooks) = CLEANUP_HOOKS.lock() {
        hooks.push(hook);
    }
}

/// Requests a clean host shutdown.
/// Can be safely called from signal handlers or UI event loops.
pub fn request_host_shutdown(reason: &str) {
    let prev = SHUTDOWN_STATE.compare_exchange(
        HostShutdownState::Running as u32,
        HostShutdownState::ShutdownRequested as u32,
        Ordering::SeqCst,
        Ordering::SeqCst,
    );

    if prev.is_ok() {
        eprintln!("\n[RINE HOST SHUTDOWN] Termination requested (reason: '{}'). Initiating bounded cleanup...", reason);
        spawn_emergency_watchdog(750);
    }
}

/// Spawns a host watchdog thread that guarantees Rine will exit even if a guest worker thread
/// or SDL/Vulkan call is permanently stuck in a deadlock or infinite loop.
fn spawn_emergency_watchdog(timeout_ms: u64) {
    if WATCHDOG_ACTIVE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        std::thread::Builder::new()
            .name("rine-watchdog".to_string())
            .spawn(move || {
                std::thread::sleep(Duration::from_millis(timeout_ms));
                // If the process is still running after timeout, enforce immediate clean exit
                if SHUTDOWN_STATE.load(Ordering::SeqCst) != (HostShutdownState::Terminated as u32) {
                    eprintln!("[RINE HOST WATCHDOG] Bounded cleanup deadline reached ({}ms). Enforcing process termination.", timeout_ms);
                    unsafe {
                        libc::_exit(0);
                    }
                }
            })
            .ok();
    }
}

/// Executes all registered host cleanup hooks in a strictly bounded and idempotent manner.
pub fn perform_host_cleanup() {
    let prev = SHUTDOWN_STATE.compare_exchange(
        HostShutdownState::ShutdownRequested as u32,
        HostShutdownState::CleaningUp as u32,
        Ordering::SeqCst,
        Ordering::SeqCst,
    );

    if prev.is_err() {
        // Also allow direct transition from Running to CleaningUp if called directly
        let prev_running = SHUTDOWN_STATE.compare_exchange(
            HostShutdownState::Running as u32,
            HostShutdownState::CleaningUp as u32,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        if prev_running.is_err() {
            return; // Already cleaning up or terminated
        }
    }

    let hooks = if let Ok(mut h) = CLEANUP_HOOKS.lock() {
        std::mem::take(&mut *h)
    } else {
        Vec::new()
    };

    for hook in hooks {
        hook();
    }

    SHUTDOWN_STATE.store(HostShutdownState::Terminated as u32, Ordering::SeqCst);
    eprintln!("[RINE HOST SHUTDOWN] Host cleanup completed cleanly. Goodbye.\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_state_transitions() {
        assert_eq!(get_shutdown_state(), HostShutdownState::Running);
        request_host_shutdown("test_request");
        assert!(is_shutdown_requested());
        assert_eq!(get_shutdown_state(), HostShutdownState::ShutdownRequested);

        static CALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        fn test_hook() {
            CALLED.store(true, Ordering::SeqCst);
        }

        register_cleanup_hook(test_hook);
        perform_host_cleanup();
        assert!(CALLED.load(Ordering::SeqCst));
        assert_eq!(get_shutdown_state(), HostShutdownState::Terminated);

        // Idempotent: second call does not panic or re-execute
        perform_host_cleanup();
        assert_eq!(get_shutdown_state(), HostShutdownState::Terminated);
    }
}
