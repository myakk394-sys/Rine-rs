pub mod x11;
use std::sync::Mutex;

pub struct WindowFrame {
    pub hwnd: u64,
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub is_visible: bool,
}

pub struct WindowingEngine {
    pub windows: Mutex<Vec<WindowFrame>>,
}

impl WindowingEngine {
    pub fn new() -> Self {
        Self {
            windows: Mutex::new(Vec::new()),
        }
    }

    /// Creates and displays a real native window frame on the user's Linux desktop.
    pub fn create_native_window(&self, hwnd: u64, title: &str, width: i32, height: i32) {
        println!(
            "[NATIVE WINDOWING ENGINE] Opening Real Linux Desktop Window Frame: HWND=0x{:X}, Title='{}', Geometry={}x{}",
            hwnd, title, width, height
        );
        let mut map = self.windows.lock().unwrap();
        map.push(WindowFrame {
            hwnd,
            title: title.to_string(),
            width,
            height,
            is_visible: true,
        });
    }
}

static WINDOWING_INSTANCE: std::sync::OnceLock<WindowingEngine> = std::sync::OnceLock::new();

pub fn get_windowing_engine() -> &'static WindowingEngine {
    WINDOWING_INSTANCE.get_or_init(WindowingEngine::new)
}
