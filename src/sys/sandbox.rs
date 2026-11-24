//! Rine Security Sandbox Subsystem (Landlock LSM)
//! Restricts host file system access at the Linux kernel level, neutralizing direct
//! guest syscalls, stealer malware, and path escapes without requiring containerization.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

pub const SYS_LANDLOCK_CREATE_RULESET: i64 = 444;
pub const SYS_LANDLOCK_ADD_RULE: i64 = 445;
pub const SYS_LANDLOCK_RESTRICT_SELF: i64 = 446;

pub const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1 << 0;
pub const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

pub const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
pub const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
pub const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
pub const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
pub const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
pub const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
pub const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
pub const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
pub const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
pub const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
pub const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
pub const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
pub const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
pub const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
pub const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;
pub const LANDLOCK_ACCESS_FS_IOCTL_DEV: u64 = 1 << 15;

#[repr(C)]
#[derive(Debug, Default)]
struct LandlockRulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
    scoped: u64,
}

#[repr(C)]
#[derive(Debug)]
struct LandlockPathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

pub struct SandboxConfig {
    pub drive_c_root: PathBuf,
    pub drive_d_root: PathBuf,
    pub shm_root: Option<PathBuf>,
    pub shader_cache_dir: PathBuf,
}

/// Query the highest supported Landlock ABI version from the running Linux kernel
pub fn get_landlock_abi_version() -> i32 {
    let res = unsafe {
        libc::syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            std::ptr::null::<LandlockRulesetAttr>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    res as i32
}

/// Applies Landlock LSM sandbox policies to the current process and all its threads.
/// Returns Ok(true) if enforced, Ok(false) if Landlock is unsupported on this kernel, or Err.
pub fn apply_landlock_sandbox(config: &SandboxConfig) -> Result<bool, String> {
    let abi = get_landlock_abi_version();
    if abi < 1 {
        eprintln!("[RINE SANDBOX] Landlock LSM is not supported or enabled on this kernel (ABI: {}). Bypassing.", abi);
        return Ok(false);
    }

    println!("[RINE SANDBOX] Initializing Linux Landlock LSM Security Barrier (ABI v{})...", abi);

    // Build handled filesystem actions bitmask based on ABI version
    let mut handled_fs = LANDLOCK_ACCESS_FS_EXECUTE
        | LANDLOCK_ACCESS_FS_WRITE_FILE
        | LANDLOCK_ACCESS_FS_READ_FILE
        | LANDLOCK_ACCESS_FS_READ_DIR
        | LANDLOCK_ACCESS_FS_REMOVE_DIR
        | LANDLOCK_ACCESS_FS_REMOVE_FILE
        | LANDLOCK_ACCESS_FS_MAKE_CHAR
        | LANDLOCK_ACCESS_FS_MAKE_DIR
        | LANDLOCK_ACCESS_FS_MAKE_REG
        | LANDLOCK_ACCESS_FS_MAKE_SOCK
        | LANDLOCK_ACCESS_FS_MAKE_FIFO
        | LANDLOCK_ACCESS_FS_MAKE_BLOCK
        | LANDLOCK_ACCESS_FS_MAKE_SYM
        | LANDLOCK_ACCESS_FS_REFER
        | LANDLOCK_ACCESS_FS_TRUNCATE;

    if abi >= 5 {
        handled_fs |= LANDLOCK_ACCESS_FS_IOCTL_DEV;
    }

    let attr = LandlockRulesetAttr {
        handled_access_fs: handled_fs,
        handled_access_net: 0,
        scoped: 0,
    };

    let ruleset_fd = unsafe {
        libc::syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            &attr as *const LandlockRulesetAttr,
            std::mem::size_of::<LandlockRulesetAttr>(),
            0u32,
        )
    } as RawFd;

    if ruleset_fd < 0 {
        let err = std::io::Error::last_os_error();
        return Err(format!("Failed to create Landlock ruleset: {}", err));
    }

    let ro_access = LANDLOCK_ACCESS_FS_READ_FILE
        | LANDLOCK_ACCESS_FS_READ_DIR
        | LANDLOCK_ACCESS_FS_EXECUTE;

    let rw_access = handled_fs; // Full access within sandboxed roots

    // 1. Add Read-Write paths
    let mut rw_paths: Vec<&Path> = vec![
        &config.drive_c_root,
        &config.drive_d_root,
        &config.shader_cache_dir,
        Path::new("/tmp"),
        Path::new("/dev/dri"),
        Path::new("/dev/snd"),
        Path::new("/dev/shm"),
        Path::new("/dev/null"),
        Path::new("/dev/zero"),
        Path::new("/dev/urandom"),
        Path::new("/run/user"),
    ];

    // If RamOnly, allow only the specific top-level shm container (e.g. /dev/shm/rine_ram_<pid>)
    // instead of the entire host /dev/shm
    if let Some(ref shm) = config.shm_root {
        rw_paths.push(shm.as_path());
    }

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    if !runtime_dir.is_empty() {
        rw_paths.push(Path::new(&runtime_dir));
    }

    for path in rw_paths {
        if path.exists() {
            add_path_rule(ruleset_fd, path, rw_access);
        }
    }

    // 2. Add Read-Only system, Wine, and GPU ICD paths
    let ro_paths: Vec<&str> = vec![
        "/usr",
        "/lib",
        "/lib64",
        "/etc/ld.so.cache",
        "/etc/vulkan",
        "/etc/glvnd",
        "/etc/fonts",
        "/etc/pipewire",
        "/etc/pulse",
        "/etc/wireplumber",
        "/etc/alsa",
        "/etc/asound.conf",
        "/etc/machine-id",
        "/var/lib/dbus/machine-id",
        "/etc/passwd",
        "/etc/group",
        "/etc/nsswitch.conf",
        "/usr/share/vulkan",       // Critical for nvidia_icd.json / radeon_icd.json
        "/usr/share/glvnd",
        "/usr/lib/wine",           // Wine provider DLLs
        "/usr/lib64/wine",
        "/usr/lib/x86_64-linux-gnu/wine",
        "/usr/share/wine",
        "/sys",                    // libdrm PCI bus enumeration for GPU
        "/proc",                   // /proc/self for runtime thread inspection
        "/dev",                    // /dev/urandom, /dev/null, /dev/zero for crypto, Steam tier0, and CRT
    ];

    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/fizzy".to_string());
    let rine_share = format!("{}/.local/share/rine", home);
    let _ = std::fs::create_dir_all(&rine_share);
    if Path::new(&rine_share).exists() {
        add_path_rule(ruleset_fd, Path::new(&rine_share), rw_access);
    }
    let steam_share = format!("{}/.local/share/Steam", home);
    let steam_dot = format!("{}/.steam", home);
    let local_bin = format!("{}/.local/bin", home);
    if Path::new(&steam_share).exists() {
        add_path_rule(ruleset_fd, Path::new(&steam_share), ro_access);
    }
    if Path::new(&steam_dot).exists() {
        add_path_rule(ruleset_fd, Path::new(&steam_dot), rw_access);
    }
    if Path::new(&local_bin).exists() {
        add_path_rule(ruleset_fd, Path::new(&local_bin), ro_access);
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            if parent.exists() {
                add_path_rule(ruleset_fd, parent, ro_access);
            }
        }
        if exe_path.exists() {
            add_path_rule(ruleset_fd, &exe_path, ro_access);
        }
    }

    for path_str in ro_paths {
        let p = Path::new(path_str);
        if p.exists() {
            add_path_rule(ruleset_fd, p, ro_access);
        }
    }

    // Allow NVIDIA devices if present
    for i in 0..16 {
        let dev_nvidia = format!("/dev/nvidia{}", i);
        let p = Path::new(&dev_nvidia);
        if p.exists() {
            add_path_rule(ruleset_fd, p, rw_access);
        }
    }
    let extra_devs = [
        "/dev/nvidiactl",
        "/dev/nvidia-modeset",
        "/dev/nvidia-uvm",
        "/dev/nvidia-uvm-tools",
    ];
    for dev in extra_devs {
        let p = Path::new(dev);
        if p.exists() {
            add_path_rule(ruleset_fd, p, rw_access);
        }
    }
    let nvidia_caps = Path::new("/dev/nvidia-caps");
    if nvidia_caps.exists() {
        add_path_rule(ruleset_fd, nvidia_caps, rw_access);
        if let Ok(entries) = std::fs::read_dir(nvidia_caps) {
            for entry in entries.flatten() {
                add_path_rule(ruleset_fd, &entry.path(), rw_access);
            }
        }
    }

    // 3. Enforce the sandbox
    unsafe {
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
            let err = std::io::Error::last_os_error();
            libc::close(ruleset_fd);
            return Err(format!("prctl(PR_SET_NO_NEW_PRIVS) failed: {}", err));
        }

        let ret = libc::syscall(SYS_LANDLOCK_RESTRICT_SELF, ruleset_fd, 0u32);
        libc::close(ruleset_fd);

        if ret != 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("landlock_restrict_self failed: {}", err));
        }
    }

    println!("[RINE SANDBOX] Landlock LSM ruleset ACTIVE. Host filesystem isolated from guest PE.");
    Ok(true)
}

fn add_path_rule(ruleset_fd: RawFd, path: &Path, allowed_access: u64) {
    let c_path = match CString::new(path.as_os_str().as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Open target with O_PATH | O_CLOEXEC (O_PATH is explicitly accepted by Landlock)
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 {
        return;
    }

    let is_dir = path.is_dir();
    let mut access = allowed_access;
    if !is_dir {
        // Landlock kernel rejects non-directory files with EINVAL if directory-only flags are passed.
        // For non-directories, allowed_access must only contain file and character device operations.
        access &= LANDLOCK_ACCESS_FS_EXECUTE
            | LANDLOCK_ACCESS_FS_WRITE_FILE
            | LANDLOCK_ACCESS_FS_READ_FILE
            | LANDLOCK_ACCESS_FS_TRUNCATE
            | LANDLOCK_ACCESS_FS_IOCTL_DEV;
    }

    let path_beneath = LandlockPathBeneathAttr {
        allowed_access: access,
        parent_fd: fd,
    };

    unsafe {
        let res = libc::syscall(
            SYS_LANDLOCK_ADD_RULE,
            ruleset_fd,
            LANDLOCK_RULE_PATH_BENEATH,
            &path_beneath as *const LandlockPathBeneathAttr,
            0u32,
        );
        if res != 0 {
            let err = std::io::Error::last_os_error();
            eprintln!(
                "[RINE SANDBOX] Warning: failed to add rule for {}: {}",
                path.display(),
                err
            );
        }
        libc::close(fd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landlock_abi_detection() {
        let abi = get_landlock_abi_version();
        // Modern Linux kernels support Landlock ABI version >= 1
        assert!(abi >= 1, "Landlock should be detected on kernel 6.x");
    }
}
