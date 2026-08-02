use crate::sys::WinPageProtection;
use bitflags::bitflags;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageSectionHeader {
    pub name: [u8; 8],
    pub virtual_size: u32,
    pub virtual_address: u32,
    pub size_of_raw_data: u32,
    pub pointer_to_raw_data: u32,
    pub pointer_to_relocations: u32,
    pub pointer_to_linenumbers: u32,
    pub number_of_relocations: u16,
    pub number_of_linenumbers: u16,
    pub characteristics: u32,
}

impl ImageSectionHeader {
    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(8);
        std::str::from_utf8(&self.name[..end]).unwrap_or(".unknown")
    }

    #[inline]
    pub fn virtual_address(&self) -> u32 {
        self.virtual_address
    }
    #[inline]
    pub fn virtual_size(&self) -> u32 {
        self.virtual_size
    }
    #[inline]
    pub fn size_of_raw_data(&self) -> u32 {
        self.size_of_raw_data
    }
    #[inline]
    pub fn pointer_to_raw_data(&self) -> u32 {
        self.pointer_to_raw_data
    }
    #[inline]
    pub fn characteristics(&self) -> u32 {
        self.characteristics
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SectionCharacteristics: u32 {
        const CNT_CODE               = 0x00000020;
        const CNT_INITIALIZED_DATA   = 0x00000040;
        const CNT_UNINITIALIZED_DATA = 0x00000080;
        const LNK_INFO               = 0x00000200;
        const LNK_REMOVE             = 0x00000800;
        const LNK_COMDAT             = 0x00001000;
        const NO_DEFER_SPEC_EXC      = 0x00004000;
        const MEM_FARDATA            = 0x00008000;
        const MEM_PURGEABLE          = 0x00020000;
        const MEM_LOCKED             = 0x00040000;
        const MEM_PRELOAD            = 0x00080000;
        const MEM_DISCARDABLE        = 0x02000000;
        const MEM_NOT_CACHED         = 0x04000000;
        const MEM_NOT_PAGED          = 0x08000000;
        const MEM_SHARED             = 0x10000000;
        const MEM_EXECUTE            = 0x20000000;
        const MEM_READ               = 0x40000000;
        const MEM_WRITE              = 0x80000000;
    }
}

impl SectionCharacteristics {
    pub fn to_win_protection(self) -> WinPageProtection {
        let is_exec = self.contains(SectionCharacteristics::MEM_EXECUTE);
        let is_read = self.contains(SectionCharacteristics::MEM_READ);
        let is_write = self.contains(SectionCharacteristics::MEM_WRITE);

        match (is_exec, is_read, is_write) {
            (true, true, true) => WinPageProtection::EXECUTE_READWRITE,
            (true, true, false) => WinPageProtection::EXECUTE_READ,
            (true, false, false) => WinPageProtection::EXECUTE,
            (false, true, true) => WinPageProtection::READWRITE,
            (false, true, false) => WinPageProtection::READONLY,
            (false, false, false) => WinPageProtection::NOACCESS,
            _ => WinPageProtection::READWRITE,
        }
    }
}
