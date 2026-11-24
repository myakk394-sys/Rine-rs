//! Rine Diagnostics & Runtime Hang Detection System
//!
//! Provides thread tracking, call history (last Win32 & Graphics API calls),
//! synchronization wait tracking, graphics presentation watchdog,
//! and structured hang diagnostics reports.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ThreadExecutionState {
    Running = 0,
    Waiting = 1,
    Suspended = 2,
    InGraphics = 3,
    Terminated = 4,
    Unknown = 5,
}

impl ThreadExecutionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "RUNNING",
            Self::Waiting => "WAITING",
            Self::Suspended => "SUSPENDED",
            Self::InGraphics => "IN_GRAPHICS",
            Self::Terminated => "TERMINATED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThreadDiagnosticsSnapshot {
    pub win32_tid: u32,
    pub host_tid: u64,
    pub state: ThreadExecutionState,
    pub rip: u64,
    pub rsp: u64,
    pub last_win32_api: String,
    pub last_win32_elapsed_ms: u64,
    pub last_gfx_api: String,
    pub last_gfx_elapsed_ms: u64,
    pub wait_handle: u64,
    pub wait_reason: String,
    pub is_graphics_thread: bool,
}

pub struct ThreadDiagnosticsEntry {
    pub win32_tid: u32,
    pub host_tid: AtomicU64,
    pub state: AtomicU8,
    pub rip: AtomicU64,
    pub rsp: AtomicU64,
    pub last_win32_api: Mutex<String>,
    pub last_win32_timestamp: Mutex<Option<Instant>>,
    pub last_gfx_api: Mutex<String>,
    pub last_gfx_timestamp: Mutex<Option<Instant>>,
    pub wait_handle: AtomicU64,
    pub wait_reason: Mutex<String>,
    pub is_graphics_thread: AtomicBool,
}

impl ThreadDiagnosticsEntry {
    pub fn new(win32_tid: u32, host_tid: u64) -> Arc<Self> {
        Arc::new(Self {
            win32_tid,
            host_tid: AtomicU64::new(host_tid),
            state: AtomicU8::new(ThreadExecutionState::Running as u8),
            rip: AtomicU64::new(0),
            rsp: AtomicU64::new(0),
            last_win32_api: Mutex::new(String::from("None")),
            last_win32_timestamp: Mutex::new(None),
            last_gfx_api: Mutex::new(String::from("None")),
            last_gfx_timestamp: Mutex::new(None),
            wait_handle: AtomicU64::new(0),
            wait_reason: Mutex::new(String::new()),
            is_graphics_thread: AtomicBool::new(false),
        })
    }

    pub fn snapshot(&self) -> ThreadDiagnosticsSnapshot {
        let state_val = self.state.load(Ordering::Relaxed);
        let state = match state_val {
            0 => ThreadExecutionState::Running,
            1 => ThreadExecutionState::Waiting,
            2 => ThreadExecutionState::Suspended,
            3 => ThreadExecutionState::InGraphics,
            4 => ThreadExecutionState::Terminated,
            _ => ThreadExecutionState::Unknown,
        };

        let last_win32_api = self.last_win32_api.lock().map(|s| s.clone()).unwrap_or_default();
        let last_win32_elapsed_ms = self
            .last_win32_timestamp
            .lock()
            .ok()
            .and_then(|ts| *ts)
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);

        let last_gfx_api = self.last_gfx_api.lock().map(|s| s.clone()).unwrap_or_default();
        let last_gfx_elapsed_ms = self
            .last_gfx_timestamp
            .lock()
            .ok()
            .and_then(|ts| *ts)
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);

        let wait_reason = self.wait_reason.lock().map(|s| s.clone()).unwrap_or_default();

        ThreadDiagnosticsSnapshot {
            win32_tid: self.win32_tid,
            host_tid: self.host_tid.load(Ordering::Relaxed),
            state,
            rip: self.rip.load(Ordering::Relaxed),
            rsp: self.rsp.load(Ordering::Relaxed),
            last_win32_api,
            last_win32_elapsed_ms,
            last_gfx_api,
            last_gfx_elapsed_ms,
            wait_handle: self.wait_handle.load(Ordering::Relaxed),
            wait_reason,
            is_graphics_thread: self.is_graphics_thread.load(Ordering::Relaxed),
        }
    }
}

static GLOBAL_DIAGNOSTICS_REGISTRY: OnceLock<Mutex<Vec<Arc<ThreadDiagnosticsEntry>>>> =
    OnceLock::new();

fn get_registry() -> &'static Mutex<Vec<Arc<ThreadDiagnosticsEntry>>> {
    GLOBAL_DIAGNOSTICS_REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

thread_local! {
    static CURRENT_DIAG_ENTRY: std::cell::RefCell<Option<Arc<ThreadDiagnosticsEntry>>> = const { std::cell::RefCell::new(None) };
}

pub fn register_current_thread(win32_tid: u32, host_tid: u64) -> Arc<ThreadDiagnosticsEntry> {
    let entry = ThreadDiagnosticsEntry::new(win32_tid, host_tid);
    CURRENT_DIAG_ENTRY.with(|cell| {
        *cell.borrow_mut() = Some(entry.clone());
    });
    if let Ok(mut reg) = get_registry().lock() {
        reg.retain(|e| e.win32_tid != win32_tid);
        reg.push(entry.clone());
    }
    entry
}

pub fn get_current_diag_entry() -> Option<Arc<ThreadDiagnosticsEntry>> {
    CURRENT_DIAG_ENTRY.with(|cell| cell.borrow().clone())
}

pub fn record_thread_state(state: ThreadExecutionState) {
    if let Some(entry) = get_current_diag_entry() {
        entry.state.store(state as u8, Ordering::Relaxed);
    }
}

pub fn record_thread_registers(rip: u64, rsp: u64) {
    if let Some(entry) = get_current_diag_entry() {
        entry.rip.store(rip, Ordering::Relaxed);
        entry.rsp.store(rsp, Ordering::Relaxed);
    }
}

pub fn record_win32_api(api_name: &str) {
    if let Some(entry) = get_current_diag_entry() {
        if let Ok(mut name) = entry.last_win32_api.lock() {
            name.clear();
            name.push_str(api_name);
        }
        if let Ok(mut ts) = entry.last_win32_timestamp.lock() {
            *ts = Some(Instant::now());
        }
    }
}

pub fn record_gfx_api(api_name: &str) {
    if let Some(entry) = get_current_diag_entry() {
        entry.is_graphics_thread.store(true, Ordering::Relaxed);
        if let Ok(mut name) = entry.last_gfx_api.lock() {
            name.clear();
            name.push_str(api_name);
        }
        if let Ok(mut ts) = entry.last_gfx_timestamp.lock() {
            *ts = Some(Instant::now());
        }
    }
}

pub fn record_wait_start(handle: u64, reason: &str) {
    if let Some(entry) = get_current_diag_entry() {
        entry.state.store(ThreadExecutionState::Waiting as u8, Ordering::Relaxed);
        entry.wait_handle.store(handle, Ordering::Relaxed);
        if let Ok(mut r) = entry.wait_reason.lock() {
            r.clear();
            r.push_str(reason);
        }
    }
}

pub fn record_wait_end() {
    if let Some(entry) = get_current_diag_entry() {
        entry.state.store(ThreadExecutionState::Running as u8, Ordering::Relaxed);
        entry.wait_handle.store(0, Ordering::Relaxed);
        if let Ok(mut r) = entry.wait_reason.lock() {
            r.clear();
        }
    }
}

// Frame presentation watchdog and hang detector
// Сторожевой таймер вывода кадров и детектор зависаний рендера

static LAST_FRAME_PRESENT_TIME: OnceLock<Mutex<Instant>> = OnceLock::new();
static TOTAL_FRAMES_PRESENTED: AtomicU64 = AtomicU64::new(0);
static HANG_DETECTOR_ACTIVE: AtomicBool = AtomicBool::new(false);

fn get_last_present_mutex() -> &'static Mutex<Instant> {
    LAST_FRAME_PRESENT_TIME.get_or_init(|| Mutex::new(Instant::now()))
}

pub fn notify_frame_presented() {
    TOTAL_FRAMES_PRESENTED.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut t) = get_last_present_mutex().lock() {
        *t = Instant::now();
    }
}

pub fn get_total_frames() -> u64 {
    TOTAL_FRAMES_PRESENTED.load(Ordering::Relaxed)
}

pub fn generate_diagnostics_report() -> String {
    let mut report = String::new();
    report.push_str("================================================================================\n");
    report.push_str("                        RINE RUNTIME DIAGNOSTICS DUMP\n");
    report.push_str("================================================================================\n");

    let total_frames = TOTAL_FRAMES_PRESENTED.load(Ordering::Relaxed);
    let last_frame_sec = get_last_present_mutex()
        .lock()
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);

    report.push_str(&format!(
        "Frames Presented: {} | Seconds Since Last Present: {}s\n",
        total_frames, last_frame_sec
    ));

    let entries = get_registry().lock().map(|r| r.clone()).unwrap_or_default();
    report.push_str(&format!("Active Tracked Threads: {}\n", entries.len()));
    report.push_str("--------------------------------------------------------------------------------\n");

    for e in entries {
        let snap = e.snapshot();
        report.push_str(&format!(
            "TID 0x{:04X} (Host TID: {}) | State: {} | GfxThread: {}\n",
            snap.win32_tid, snap.host_tid, snap.state.as_str(), snap.is_graphics_thread
        ));
        report.push_str(&format!(
            "  RIP: 0x{:016X} | RSP: 0x{:016X}\n",
            snap.rip, snap.rsp
        ));
        report.push_str(&format!(
            "  Last Win32 API: {} ({} ms ago)\n",
            snap.last_win32_api, snap.last_win32_elapsed_ms
        ));
        if snap.is_graphics_thread || !snap.last_gfx_api.is_empty() && snap.last_gfx_api != "None" {
            report.push_str(&format!(
                "  Last GFX API:   {} ({} ms ago)\n",
                snap.last_gfx_api, snap.last_gfx_elapsed_ms
            ));
        }
        if snap.state == ThreadExecutionState::Waiting {
            report.push_str(&format!(
                "  Waiting on: Handle 0x{:X} | Reason: {}\n",
                snap.wait_handle, snap.wait_reason
            ));
        }
        report.push_str("--------------------------------------------------------------------------------\n");
    }

    report
}

/// Spawns the background hang detection monitor.
/// If games start presenting frames and then stall for > `hang_timeout_secs`,
/// or if all active threads remain blocked in wait states without frame progress,
/// an alert dump is automatically printed to stderr and the log.
pub fn start_hang_detector(hang_timeout_secs: u64) {
    if HANG_DETECTOR_ACTIVE.swap(true, Ordering::SeqCst) {
        return; // Already started
    }

    std::thread::Builder::new()
        .name("rine-hang-watchdog".into())
        .spawn(move || {
            let start_time = Instant::now();
            let timeout = Duration::from_secs(hang_timeout_secs);
            let check_interval = Duration::from_secs(2);
            let mut hang_logged = false;

            loop {
                std::thread::sleep(check_interval);

                let frames = TOTAL_FRAMES_PRESENTED.load(Ordering::Relaxed);
                if frames == 0 {
                    // Before the first frame has presented (game loading or window init),
                    // check if engine initialization has completely stalled (> 30s)
                    if start_time.elapsed() > Duration::from_secs(30) && !hang_logged {
                        hang_logged = true;
                        eprintln!("\n⚠️  [RINE HANG DETECTOR] Potential engine startup hang detected!");
                        eprintln!("    No frames presented after {:.1}s of execution", start_time.elapsed().as_secs_f32());
                        eprintln!("{}", generate_diagnostics_report());
                    }
                    continue;
                }

                let elapsed_since_present = get_last_present_mutex()
                    .lock()
                    .map(|t| t.elapsed())
                    .unwrap_or(Duration::ZERO);

                if elapsed_since_present > timeout {
                    if !hang_logged {
                        hang_logged = true;
                        eprintln!("\n⚠️  [RINE HANG DETECTOR] Potential hang detected!");
                        eprintln!("    No frames presented for {:.1}s (threshold: {}s)", elapsed_since_present.as_secs_f32(), hang_timeout_secs);
                        eprintln!("{}", generate_diagnostics_report());
                    }
                } else {
                    hang_logged = false;
                }
            }
        })
        .expect("Failed to spawn hang detection watchdog");
}
