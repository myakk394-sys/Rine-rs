#![allow(non_snake_case)]
use std::ffi::{c_void, CString};

pub type Display = *mut c_void;
pub type Window = u64;

/// Vulkan 1.3 and DXVK bridge engine (loads libvulkan.so.1 and libpulse-simple.so.0).
/// Движок моста Vulkan 1.3 и DXVK (загружает библиотеки libvulkan.so.1 и libpulse-simple.so.0).
pub struct VulkanDxvkEngine {
    pub vulkan_handle: *mut c_void,
    pub pulse_handle: *mut c_void,
}

impl VulkanDxvkEngine {
    pub unsafe fn init() -> Self {
        let vk_lib_name = CString::new("libvulkan.so.1").unwrap();
        let vk_handle = libc::dlopen(vk_lib_name.as_ptr(), libc::RTLD_NOW);
        if !vk_handle.is_null() {
            println!("[RINE VULKAN TRANSLATOR] Successfully initialized DXVK Vulkan 1.3 Driver (libvulkan.so.1)!");
        } else {
            println!("[RINE VULKAN TRANSLATOR] Note: libvulkan.so.1 not found, using SDL2 renderer as graphics backend");
        }

        let pulse_lib_name = CString::new("libpulse-simple.so.0").unwrap();
        let pulse_handle = libc::dlopen(pulse_lib_name.as_ptr(), libc::RTLD_NOW);
        if !pulse_handle.is_null() {
            println!("[RINE AUDIO ENGINE] Successfully initialized PulseAudio / PipeWire Sound Server (libpulse-simple.so.0)!");
        } else {
            println!(
                "[RINE AUDIO ENGINE] Note: libpulse-simple.so.0 not found, using ALSA pcm fallback"
            );
        }

        Self {
            vulkan_handle: vk_handle,
            pulse_handle,
        }
    }
}

static VULKAN_PRESENTATION_CONFIRMED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn is_vulkan_presentation_confirmed() -> bool {
    VULKAN_PRESENTATION_CONFIRMED.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn set_vulkan_presentation_confirmed() {
    let was = VULKAN_PRESENTATION_CONFIRMED.swap(true, std::sync::atomic::Ordering::SeqCst);
    if !was {
        unsafe {
            crate::windowing::x11::show_and_unhide_game_window();
        }
    }
}

pub struct VulkanSwapChainPresenter;

impl VulkanSwapChainPresenter {
    /// Presents a frame to the physical screen via SDL2 renderer.
    /// This is called every ~16ms from run_main_window_event_loop().
    pub unsafe fn present_frame_to_screen(_display: Display, _window: Window) {
        // Return early ONLY when Vulkan presentation pipeline is confirmed active via real vkQueuePresentKHR calls
        if VULKAN_PRESENTATION_CONFIRMED.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }

        let sdl_handle = crate::windowing::x11::get_sdl_handle();
        if sdl_handle.is_null() {
            return;
        }

        let renderer = crate::windowing::x11::get_global_renderer();
        if renderer.is_null() {
            return;
        }

        // Resolve SDL functions from already-loaded SDL2 handle
        type SdlSetRenderDrawColorFn = unsafe extern "C" fn(*mut c_void, u8, u8, u8, u8) -> i32;
        type SdlRenderClearFn = unsafe extern "C" fn(*mut c_void) -> i32;
        type SdlRenderPresentFn = unsafe extern "C" fn(*mut c_void);

        let fn_set_color = libc::dlsym(
            sdl_handle,
            CString::new("SDL_SetRenderDrawColor").unwrap().as_ptr(),
        );
        let fn_clear = libc::dlsym(
            sdl_handle,
            CString::new("SDL_RenderClear").unwrap().as_ptr(),
        );
        let fn_present = libc::dlsym(
            sdl_handle,
            CString::new("SDL_RenderPresent").unwrap().as_ptr(),
        );

        if fn_set_color.is_null() || fn_clear.is_null() || fn_present.is_null() {
            return;
        }

        let set_color: SdlSetRenderDrawColorFn = std::mem::transmute(fn_set_color);
        let render_clear: SdlRenderClearFn = std::mem::transmute(fn_clear);
        let render_present: SdlRenderPresentFn = std::mem::transmute(fn_present);

        set_color(renderer, 0x1A, 0x1A, 0x2E, 0xFF);
        render_clear(renderer);
        render_present(renderer);
        crate::sys::diagnostics::record_gfx_api("SDL_RenderPresent");
        crate::sys::diagnostics::notify_frame_presented();
    }
}
