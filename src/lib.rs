//! Rine (Rine Is Not an Emulator)
//! Modern, zero-legacy Win64 API translation layer for Linux written in Rust.

pub mod architecture;
pub mod dlls;
pub mod graphics;
pub mod gui;
pub mod mem;
pub mod pe;
pub mod providers;
pub mod rineserver;
pub mod steam_scanner;
pub mod sys;
pub mod windowing;

pub use architecture::*;
pub use dlls::*;
pub use graphics::get_graphics_engine;
pub use mem::{LoadConfig, LoadedModule, StorageStrategy};
pub use pe::PeImage;
pub use providers::*;
pub use rineserver::RineServer;
pub use steam_scanner::*;
pub use sys::{Result, RineError};
pub use windowing::get_windowing_engine;
