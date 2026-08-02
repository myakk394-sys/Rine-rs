#![allow(non_snake_case)]
use crate::pe::peb::UnicodeString64;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UnicodeString32 {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: u32, // 32-bit pointer
}

impl UnicodeString32 {
    pub fn to_unicode64(&self) -> UnicodeString64 {
        UnicodeString64 {
            length: self.length,
            maximum_length: self.maximum_length,
            buffer: self.buffer as u64,
        }
    }
}

/// WoW64 Retranslator: Converts 32-bit memory pointers to 64-bit address space pointers.
#[inline]
pub fn wow64_thunk_ptr32_to_64(ptr32: u32) -> u64 {
    ptr32 as u64
}

/// WoW64 Retranslator: Converts 64-bit pointers to 32-bit address space.
#[inline]
pub fn wow64_thunk_ptr64_to_32(ptr64: u64) -> u32 {
    (ptr64 & 0xFFFFFFFF) as u32
}

pub struct Wow64Retranslator;

impl Wow64Retranslator {
    /// Translates a 32-bit NT system call request into 64-bit Rine kernel execution.
    pub fn retranslate_syscall_32_to_64(syscall_id: u32, arg1: u32, arg2: u32) -> u64 {
        println!(
            "[WoW64 RETRANSLATOR] Thunking 32-bit Syscall ID 0x{:04X} (Arg1: 0x{:08X}, Arg2: 0x{:08X}) -> 64-bit POSIX Core",
            syscall_id, arg1, arg2
        );
        let arg1_64 = wow64_thunk_ptr32_to_64(arg1);
        let _arg2_64 = wow64_thunk_ptr32_to_64(arg2);

        arg1_64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wow64_thunking() {
        let ptr32: u32 = 0x12345678;
        let ptr64 = wow64_thunk_ptr32_to_64(ptr32);
        assert_eq!(ptr64, 0x0000000012345678);
        assert_eq!(wow64_thunk_ptr64_to_32(ptr64), ptr32);
    }
}
