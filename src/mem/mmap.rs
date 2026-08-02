use crate::sys::{align_up, Result, RineError, PAGE_SIZE};
use std::ffi::CString;

pub struct MmapRegion {
    ptr: *mut libc::c_void,
    size: usize,
    locked: bool,
}

impl MmapRegion {
    /// Allocates an anonymous memory region of specified size and protection flags.
    pub fn allocate_anonymous(
        preferred_addr: Option<usize>,
        size: usize,
        prot: libc::c_int,
    ) -> Result<Self> {
        let aligned_size = align_up(size, PAGE_SIZE);
        let target_ptr = preferred_addr.unwrap_or(0) as *mut libc::c_void;

        let flags = if target_ptr.is_null() {
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS
        } else {
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE
        };

        let mut ptr = unsafe { libc::mmap(target_ptr, aligned_size, prot, flags, -1, 0) };

        if ptr == libc::MAP_FAILED && !target_ptr.is_null() {
            // Fallback without MAP_FIXED_NOREPLACE if preferred address is occupied
            ptr = unsafe {
                libc::mmap(
                    target_ptr,
                    aligned_size,
                    prot,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
        }

        if ptr == libc::MAP_FAILED {
            return Err(RineError::MemoryMappingFailed(format!(
                "mmap failed with error code {}",
                unsafe { *libc::__errno_location() }
            )));
        }

        Ok(Self {
            ptr,
            size: aligned_size,
            locked: false,
        })
    }

    /// Allocates memory via `memfd_create` (RAM-disk backing) and copies binary bytes.
    pub fn allocate_memfd(
        name: &str,
        preferred_addr: Option<usize>,
        bytes: &[u8],
        prot: libc::c_int,
    ) -> Result<Self> {
        let c_name = CString::new(name).unwrap_or_default();
        let fd = unsafe { libc::memfd_create(c_name.as_ptr(), libc::MFD_CLOEXEC) };

        if fd < 0 {
            return Err(RineError::MemoryMappingFailed(format!(
                "memfd_create failed with errno {}",
                unsafe { *libc::__errno_location() }
            )));
        }

        let write_res =
            unsafe { libc::write(fd, bytes.as_ptr() as *const libc::c_void, bytes.len()) };
        if write_res < 0 || write_res as usize != bytes.len() {
            unsafe { libc::close(fd) };
            return Err(RineError::MemoryMappingFailed(
                "failed to write binary buffer into RAM memfd".into(),
            ));
        }

        let aligned_size = align_up(bytes.len(), PAGE_SIZE);
        let target_ptr = preferred_addr.unwrap_or(0) as *mut libc::c_void;

        let flags = if target_ptr.is_null() {
            libc::MAP_PRIVATE
        } else {
            libc::MAP_PRIVATE | libc::MAP_FIXED
        };

        let mut ptr = unsafe { libc::mmap(target_ptr, aligned_size, prot, flags, fd, 0) };

        if ptr == libc::MAP_FAILED && !target_ptr.is_null() {
            ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    aligned_size,
                    prot,
                    libc::MAP_PRIVATE,
                    fd,
                    0,
                )
            };
        }

        unsafe { libc::close(fd) };

        if ptr == libc::MAP_FAILED {
            return Err(RineError::MemoryMappingFailed(format!(
                "mmap on memfd failed with errno {}",
                unsafe { *libc::__errno_location() }
            )));
        }

        Ok(Self {
            ptr,
            size: aligned_size,
            locked: false,
        })
    }

    /// Locks pages in RAM using `mlock`. Soft fallback if process hits RLIMIT_MEMLOCK.
    pub fn lock(&mut self) -> Result<()> {
        if !self.locked {
            let res = unsafe { libc::mlock(self.ptr, self.size) };
            if res != 0 {
                let errno = unsafe { *libc::__errno_location() };
                if errno == libc::ENOMEM {
                    // Soft fallback: Linux non-root process exceeded RLIMIT_MEMLOCK limit.
                    // Keep RAM buffer allocated in tmpfs/memfd, just skip page pin lock.
                    return Ok(());
                }
                return Err(RineError::MemoryMappingFailed(format!(
                    "mlock failed with errno {}",
                    errno
                )));
            }
            self.locked = true;
        }
        Ok(())
    }

    /// Changes memory protection flags using `mprotect`.
    pub fn protect(&mut self, offset: usize, len: usize, prot: libc::c_int) -> Result<()> {
        if offset + len > self.size {
            return Err(RineError::MemoryMappingFailed(
                "out of bounds mprotect".into(),
            ));
        }

        let target_ptr = unsafe { (self.ptr as *mut u8).add(offset) as *mut libc::c_void };
        let aligned_len = align_up(len, PAGE_SIZE);

        let res = unsafe { libc::mprotect(target_ptr, aligned_len, prot) };
        if res != 0 {
            return Err(RineError::MemoryMappingFailed(format!(
                "mprotect failed with errno {}",
                unsafe { *libc::__errno_location() }
            )));
        }

        Ok(())
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr as *const u8
    }

    #[inline]
    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr as *mut u8
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }
}

/// Safely tests whether a memory range is mapped and readable without triggering SIGSEGV.
pub fn is_readable(addr: u64, size: usize) -> bool {
    if addr < 0x1000 || addr.checked_add(size as u64).is_none() {
        return false;
    }
    let start_page = (addr as usize) & !0xFFF;
    let end_page = ((addr as usize + size.saturating_sub(1)) & !0xFFF) + 0x1000;
    let len = end_page.saturating_sub(start_page);
    if len == 0 || len > 0x100000000 {
        // limit check to 4GB max
        return false;
    }
    let mut vec = vec![0u8; (len >> 12) + 1];
    let res = unsafe { libc::mincore(start_page as *mut libc::c_void, len, vec.as_mut_ptr()) };
    res == 0
}

impl Drop for MmapRegion {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.ptr != libc::MAP_FAILED {
            if self.locked {
                unsafe { libc::munlock(self.ptr, self.size) };
            }
            unsafe { libc::munmap(self.ptr, self.size) };
        }
    }
}

unsafe impl Send for MmapRegion {}
unsafe impl Sync for MmapRegion {}
