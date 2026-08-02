pub type NtStatus = i32;

pub const STATUS_SUCCESS: NtStatus = 0x00000000;
pub const STATUS_TIMEOUT: NtStatus = 0x00000102;
pub const STATUS_UNSUCCESSFUL: NtStatus = -0x3FFFFFFF; // 0xC0000001
pub const STATUS_NOT_IMPLEMENTED: NtStatus = -0x3FFFFFFE; // 0xC0000002
pub const STATUS_ACCESS_VIOLATION: NtStatus = -0x3FFFFFFB; // 0xC0000005
pub const STATUS_INVALID_HANDLE: NtStatus = -0x3FFFFFF8; // 0xC0000008
pub const STATUS_INVALID_PARAMETER: NtStatus = -0x3FFFFFF3; // 0xC000000D
pub const STATUS_NO_MEMORY: NtStatus = -0x3FFFFFE9; // 0xC0000017
pub const STATUS_OBJECT_NAME_NOT_FOUND: NtStatus = -0x3FFFFFCC; // 0xC0000034
pub const STATUS_NOT_SUPPORTED: NtStatus = -0x3FFFFF45; // 0xC00000BB

#[inline]
pub fn is_success(status: NtStatus) -> bool {
    status >= 0
}

/// Converts Linux POSIX `errno` to Windows `NTSTATUS`.
pub fn errno_to_ntstatus(errno: libc::c_int) -> NtStatus {
    match errno {
        0 => STATUS_SUCCESS,
        libc::EPERM | libc::EACCES => STATUS_ACCESS_VIOLATION,
        libc::ENOENT => STATUS_OBJECT_NAME_NOT_FOUND,
        libc::ENOMEM => STATUS_NO_MEMORY,
        libc::EINVAL => STATUS_INVALID_PARAMETER,
        libc::EBADF => STATUS_INVALID_HANDLE,
        libc::ENOSYS | libc::EOPNOTSUPP => STATUS_NOT_SUPPORTED,
        _ => STATUS_UNSUCCESSFUL,
    }
}
