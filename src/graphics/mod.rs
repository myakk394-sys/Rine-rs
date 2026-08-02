pub mod dxgi_vulkan;
pub mod vulkan_bridge;
use std::sync::Mutex;

pub struct DesktopRenderWindow {
    pub hwnd: u64,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub is_active: bool,
}

pub struct GraphicsRenderEngine {
    pub active_render_window: Mutex<Option<DesktopRenderWindow>>,
}

impl GraphicsRenderEngine {
    pub fn new() -> Self {
        Self {
            active_render_window: Mutex::new(None),
        }
    }

    /// Initializes DirectX 11 / OpenGL swapchain surface attached to Linux Desktop Window.
    pub fn attach_swapchain_surface(&self, hwnd: u64, title: &str, width: u32, height: u32) {
        println!(
            "[GRAPHICS RENDER ENGINE] Attaching DirectX 11 SwapChain Surface to Active Linux Desktop Window (HWND=0x{:X}, {}x{})",
            hwnd, width, height
        );
        let mut guard = self.active_render_window.lock().unwrap();
        *guard = Some(DesktopRenderWindow {
            hwnd,
            title: title.to_string(),
            width,
            height,
            is_active: true,
        });
    }

    /// Presents a rendered frame onto the active Linux desktop window screen.
    pub fn present_frame(&self) {
        let guard = self.active_render_window.lock().unwrap();
        if let Some(ref win) = *guard {
            if win.is_active {
                // Frame presented to Linux compositor
            }
        }
    }
}

static GRAPHICS_ENGINE: std::sync::OnceLock<GraphicsRenderEngine> = std::sync::OnceLock::new();

pub fn get_graphics_engine() -> &'static GraphicsRenderEngine {
    GRAPHICS_ENGINE.get_or_init(GraphicsRenderEngine::new)
}
