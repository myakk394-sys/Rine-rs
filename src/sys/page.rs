use bitflags::bitflags;

pub const PAGE_SIZE: usize = 4096;

#[inline]
pub fn align_up(addr: usize, alignment: usize) -> usize {
    if alignment == 0 {
        return addr;
    }
    (addr + alignment - 1) & !(alignment - 1)
}

#[inline]
pub fn align_down(addr: usize, alignment: usize) -> usize {
    if alignment == 0 {
        return addr;
    }
    addr & !(alignment - 1)
}

bitflags! {
    /// Windows Page Protection constants
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WinPageProtection: u32 {
        const NOACCESS          = 0x01;
        const READONLY          = 0x02;
        const READWRITE         = 0x04;
        const WRITECOPY         = 0x08;
        const EXECUTE           = 0x10;
        const EXECUTE_READ      = 0x20;
        const EXECUTE_READWRITE = 0x40;
        const EXECUTE_WRITECOPY = 0x80;
        const GUARD             = 0x100;
        const NOCACHE           = 0x200;
        const WRITECOMBINE      = 0x400;
    }
}

impl WinPageProtection {
    /// Converts Windows memory protection flags to POSIX `mmap`/`mprotect` `PROT_*` flags.
    pub fn to_posix_prot(self) -> libc::c_int {
        let mut prot = libc::PROT_NONE;

        if self.contains(WinPageProtection::READONLY) {
            prot |= libc::PROT_READ;
        }
        if self.contains(WinPageProtection::READWRITE)
            || self.contains(WinPageProtection::WRITECOPY)
        {
            prot |= libc::PROT_READ | libc::PROT_WRITE;
        }
        if self.contains(WinPageProtection::EXECUTE) {
            prot |= libc::PROT_EXEC;
        }
        if self.contains(WinPageProtection::EXECUTE_READ) {
            prot |= libc::PROT_READ | libc::PROT_EXEC;
        }
        if self.contains(WinPageProtection::EXECUTE_READWRITE)
            || self.contains(WinPageProtection::EXECUTE_WRITECOPY)
        {
            prot |= libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC;
        }

        prot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignments() {
        assert_eq!(align_up(0, 4096), 0);
        assert_eq!(align_up(1, 4096), 4096);
        assert_eq!(align_up(4096, 4096), 4096);
        assert_eq!(align_up(4097, 4096), 8192);

        assert_eq!(align_down(4095, 4096), 0);
        assert_eq!(align_down(4096, 4096), 4096);
        assert_eq!(align_down(4097, 4096), 4096);
    }

    #[test]
    fn test_protection_conversion() {
        assert_eq!(WinPageProtection::READONLY.to_posix_prot(), libc::PROT_READ);
        assert_eq!(
            WinPageProtection::READWRITE.to_posix_prot(),
            libc::PROT_READ | libc::PROT_WRITE
        );
        assert_eq!(
            WinPageProtection::EXECUTE_READ.to_posix_prot(),
            libc::PROT_READ | libc::PROT_EXEC
        );
        assert_eq!(
            WinPageProtection::EXECUTE_READWRITE.to_posix_prot(),
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC
        );
    }
}
