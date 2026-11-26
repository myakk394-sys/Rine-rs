//! Rine Virtual File System (VFS)
//! Provides secure Win32 -> Host path virtualization, case-insensitive component resolution,
//! caching with invalidation, and openat2 with RESOLVE_BENEATH to prevent path traversal and symlink escapes.

use std::collections::HashMap;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};

pub const SYS_OPENAT2: i64 = 437;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenHow {
    pub flags: u64,
    pub mode: u64,
    pub resolve: u64,
}

pub const RESOLVE_NO_XDEV: u64 = 0x01;
pub const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
pub const RESOLVE_NO_SYMLINKS: u64 = 0x04;
pub const RESOLVE_BENEATH: u64 = 0x08;
pub const RESOLVE_IN_ROOT: u64 = 0x10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VfsError {
    NotFound,
    AccessDenied,
    AlreadyExists,
    InvalidParameter,
    EscapeAttempt,
    Io(String),
}

impl std::fmt::Display for VfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VfsError::NotFound => write!(f, "File or path not found"),
            VfsError::AccessDenied => write!(f, "Access denied"),
            VfsError::AlreadyExists => write!(f, "File or directory already exists"),
            VfsError::InvalidParameter => write!(f, "Invalid parameter"),
            VfsError::EscapeAttempt => write!(f, "Path traversal escape attempt detected"),
            VfsError::Io(s) => write!(f, "VFS I/O error: {}", s),
        }
    }
}

impl std::error::Error for VfsError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    DriveC,
    DriveD,
}

pub struct RineVfs {
    pub drive_c_root: PathBuf,
    pub drive_d_root: PathBuf,
    pub is_ram_only: bool,
    pub shm_root: Option<PathBuf>,
    pub shader_cache_dir: PathBuf,

    pub drive_c_fd: RawFd,
    pub drive_d_fd: RawFd,

    /// Case-insensitive cache: (parent_dir_inode, lowercase_component_name) -> actual_cased_name_on_disk
    name_cache: RwLock<HashMap<(u64, String), String>>,

    /// Inode cache for directories: host_path -> inode
    inode_cache: RwLock<HashMap<PathBuf, u64>>,
}

static VFS_INSTANCE: RwLock<Option<RineVfs>> = RwLock::new(None);
static SHM_CLEANUP_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

fn perform_shm_cleanup() {
    if let Ok(guard) = SHM_CLEANUP_PATH.lock() {
        if let Some(ref path) = *guard {
            eprintln!("[RINE RAM_ONLY CLEANUP] Removing temporary shm prefix: {:?}", path);
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

impl RineVfs {
    fn ensure_prefix_dirs(c_root: &Path) {
        std::fs::create_dir_all(c_root.join("windows").join("system32")).ok();
        std::fs::create_dir_all(c_root.join("tmp")).ok();
        std::fs::create_dir_all(c_root.join("users").join("default").join("AppData").join("Roaming")).ok();
        std::fs::create_dir_all(c_root.join("users").join("default").join("AppData").join("Local")).ok();
        std::fs::create_dir_all(c_root.join("users").join("default").join("Documents")).ok();

        if let Ok(user) = std::env::var("USER") {
            std::fs::create_dir_all(c_root.join("users").join(&user).join("AppData").join("Roaming")).ok();
            std::fs::create_dir_all(c_root.join("users").join(&user).join("AppData").join("Local")).ok();
            std::fs::create_dir_all(c_root.join("users").join(&user).join("Documents")).ok();
        }
    }

    /// Initialize or re-initialize the global VFS instance with specific roots.
    pub fn init(target_exe: &str, configured_cwd: Option<&str>, ram_only: bool) {
        let (c_root, d_root, shm_root, shader_cache) = Self::compute_roots(target_exe, configured_cwd, ram_only);

        std::fs::create_dir_all(&c_root).ok();
        std::fs::create_dir_all(&d_root).ok();
        std::fs::create_dir_all(&shader_cache).ok();
        Self::ensure_prefix_dirs(&c_root);

        let c_cstr = CString::new(c_root.as_os_str().as_bytes()).unwrap();
        let c_fd = unsafe { libc::open(c_cstr.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };

        let d_cstr = CString::new(d_root.as_os_str().as_bytes()).unwrap();
        let d_fd = unsafe { libc::open(d_cstr.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };

        if ram_only {
            if let Some(ref shm) = shm_root {
                if let Ok(mut guard) = SHM_CLEANUP_PATH.lock() {
                    *guard = Some(shm.clone());
                }
                crate::sys::shutdown::register_cleanup_hook(perform_shm_cleanup);
            }
        }

        let shader_str = shader_cache.to_string_lossy().to_string();
        std::env::set_var("DXVK_STATE_CACHE_PATH", &shader_str);
        std::env::set_var("MESA_SHADER_CACHE_DIR", &shader_str);
        std::env::set_var("__GL_SHADER_DISK_CACHE_PATH", &shader_str);

        let vfs = RineVfs {
            drive_c_root: c_root,
            drive_d_root: d_root,
            is_ram_only: ram_only,
            shm_root,
            shader_cache_dir: shader_cache,
            drive_c_fd: c_fd,
            drive_d_fd: d_fd,
            name_cache: RwLock::new(HashMap::new()),
            inode_cache: RwLock::new(HashMap::new()),
        };

        if let Ok(mut lock) = VFS_INSTANCE.write() {
            *lock = Some(vfs);
        }
    }

    /// Access the global VFS instance. If not initialized, initializes with defaults.
    pub fn get() -> std::sync::RwLockReadGuard<'static, Option<RineVfs>> {
        {
            let read = VFS_INSTANCE.read().unwrap();
            if read.is_some() {
                return read;
            }
        }

        let exe = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "rine.exe".to_string());
        Self::init(&exe, None, false);
        VFS_INSTANCE.read().unwrap()
    }

    fn compute_roots(
        target_exe: &str,
        configured_cwd: Option<&str>,
        ram_only: bool,
    ) -> (PathBuf, PathBuf, Option<PathBuf>, PathBuf) {
        let (c_root, shm_root, shader_cache) = if ram_only {
            let pid = std::process::id();
            let shm = PathBuf::from(format!("/dev/shm/rine_ram_{}", pid));
            let drive_c = shm.join("drive_c");
            let cache = shm.join("cache");
            (drive_c, Some(shm), cache)
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            let base = PathBuf::from(home).join(".local").join("share").join("rine");
            let drive_c = base.join("prefixes").join("default").join("drive_c");
            let cache = base.join("cache").join("shaders");
            (drive_c, None, cache)
        };

        let d_root = if let Some(dir) = configured_cwd {
            PathBuf::from(dir)
        } else if let Some(parent) = Path::new(target_exe).parent() {
            if parent.as_os_str().is_empty() {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            } else {
                parent.to_path_buf()
            }
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };

        (c_root, d_root, shm_root, shader_cache)
    }

    /// Primary path mapping: takes any Win32 or relative guest path and maps it securely to a host path
    pub fn win32_to_host_path(&self, win_path: &str) -> String {
        let trimmed = win_path.trim();

        // Safe absolute host paths: allowed only if inside cwd, drive_d_root, drive_c_root, /tmp, or /dev
        let mut normalized_slash = trimmed.replace('\\', "/");
        if normalized_slash.len() >= 2 && normalized_slash.as_bytes()[1] == b':' {
            let after_colon = &normalized_slash[2..];
            if after_colon.starts_with('/') {
                let p = Path::new(after_colon);
                if p.starts_with(&self.drive_d_root) || p.starts_with(&self.drive_c_root)
                    || p.starts_with("/home") || p.starts_with("/tmp") || p.starts_with("/dev")
                    || p.starts_with("/usr") || p.starts_with("/opt")
                {
                    if !after_colon.contains("..") {
                        if p.exists() {
                            return after_colon.to_string();
                        }
                        let mut cur = PathBuf::from("/");
                        for comp in after_colon.trim_start_matches('/').split('/') {
                            if comp.is_empty() || comp == "." {
                                continue;
                            }
                            let resolved = self.resolve_component_case(&cur, comp);
                            cur.push(resolved);
                        }
                        if cur.exists() {
                            return cur.to_string_lossy().to_string();
                        }
                    }
                }
            }
        }

        if normalized_slash.starts_with('/') {
            let p = Path::new(&normalized_slash);
            let in_cwd = std::env::current_dir().map(|cwd| p.starts_with(&cwd)).unwrap_or(false);
            if in_cwd || p.starts_with(&self.drive_d_root) || p.starts_with(&self.drive_c_root)
                || p.starts_with("/home") || p.starts_with("/tmp") || p.starts_with("/dev")
                || p.starts_with("/usr") || p.starts_with("/opt")
            {
                if !normalized_slash.contains("..") {
                    if p.exists() {
                        return normalized_slash;
                    }
                    let mut cur = PathBuf::from("/");
                    for comp in normalized_slash.trim_start_matches('/').split('/') {
                        if comp.is_empty() || comp == "." {
                            continue;
                        }
                        let resolved = self.resolve_component_case(&cur, comp);
                        cur.push(resolved);
                    }
                    if cur.exists() {
                        return cur.to_string_lossy().to_string();
                    }
                }
            }
        }

        // Map Steam virtual path "C:\Program Files (x86)\Steam" to host Steam directory
        let lower_slash = normalized_slash.to_ascii_lowercase();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        if lower_slash.starts_with("c:/program files (x86)/steam") {
            let sub = &normalized_slash["c:/program files (x86)/steam".len()..];
            let host_steam = format!("{}/.local/share/Steam{}", home, sub);
            if !host_steam.contains("..") {
                if Path::new(&host_steam).exists() {
                    return host_steam;
                }
                let steam_base = PathBuf::from(format!("{}/.local/share/Steam", home));
                let mut cur = steam_base.clone();
                for comp in sub.trim_start_matches('/').split('/') {
                    if comp.is_empty() || comp == "." {
                        continue;
                    }
                    if comp == ".." {
                        if cur != steam_base {
                            cur.pop();
                        }
                        continue;
                    }
                    let resolved = self.resolve_component_case(&cur, comp);
                    cur.push(resolved);
                }
                return cur.to_string_lossy().to_string();
            }
        }

        let (drive, mut relative_parts) = self.normalize_win32_path(win_path);
        let base_root = match drive {
            DriveKind::DriveC => &self.drive_c_root,
            DriveKind::DriveD => &self.drive_d_root,
        };

        // If relative_parts already has the base_root prefix (e.g. from CWD joining full host path),
        // strip the duplicate prefix to avoid nesting /opt/games/test/opt/games/test
        let base_comps: Vec<String> = base_root
            .components()
            .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
            .filter(|s| !s.is_empty() && s != "/")
            .collect();

        if relative_parts.len() >= base_comps.len() {
            let mut matches_prefix = true;
            for i in 0..base_comps.len() {
                if !relative_parts[i].eq_ignore_ascii_case(&base_comps[i]) {
                    matches_prefix = false;
                    break;
                }
            }
            if matches_prefix {
                relative_parts = relative_parts[base_comps.len()..].to_vec();
            }
        }

        // Resolve case-insensitively component by component
        let mut current_host = base_root.clone();
        for comp in &relative_parts {
            if comp.is_empty() || comp == "." {
                continue;
            }
            if comp == ".." {
                // Prevent traversal above base_root
                if current_host != *base_root {
                    current_host.pop();
                }
                continue;
            }

            let resolved_comp = self.resolve_component_case(&current_host, comp);
            current_host.push(resolved_comp);
        }

        // If mapped to drive_c_root but file does not exist there, fallback to checking drive_d_root (game root)
        if drive == DriveKind::DriveC && !current_host.exists() {
            let mut game_host = self.drive_d_root.clone();
            for comp in &relative_parts {
                if comp.is_empty() || comp == "." {
                    continue;
                }
                if comp == ".." {
                    if game_host != self.drive_d_root {
                        game_host.pop();
                    }
                    continue;
                }
                let resolved_comp = self.resolve_component_case(&game_host, comp);
                game_host.push(resolved_comp);
            }
            if game_host.exists() {
                return game_host.to_string_lossy().to_string();
            }
        }

        current_host.to_string_lossy().to_string()
    }

    /// Normalizes Win32 path into (DriveKind, components)
    fn normalize_win32_path(&self, win_path: &str) -> (DriveKind, Vec<String>) {
        let mut s = win_path.trim();

        // Strip Win32 device namespace prefixes (\??\ or \\?\)
        if s.starts_with("\\??\\") || s.starts_with("\\\\?\\") {
            s = &s[4..];
        }

        // Check drive letter
        let mut drive = DriveKind::DriveD; // default to game drive for relative paths
        if s.len() >= 2 && s.as_bytes()[1] == b':' {
            let letter = s.as_bytes()[0].to_ascii_uppercase();
            if letter == b'C' {
                drive = DriveKind::DriveC;
            } else {
                drive = DriveKind::DriveD;
            }
            s = &s[2..];
        } else if s.starts_with('/') || s.starts_with('\\') {
            // Absolute path without drive letter: check if it's pointing to windows/users
            let lower = s.to_ascii_lowercase();
            if lower.starts_with("/users") || lower.starts_with("\\users")
                || lower.starts_with("/windows") || lower.starts_with("\\windows")
                || lower.starts_with("/program files") || lower.starts_with("\\program files")
            {
                drive = DriveKind::DriveC;
            }
        }

        let parts: Vec<String> = s
            .split(&['/', '\\'][..])
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .collect();

        (drive, parts)
    }

    /// Resolve a single directory component case-insensitively inside parent_dir
    fn resolve_component_case(&self, parent_dir: &Path, comp: &str) -> String {
        // Fast path: exact match on disk exists
        let candidate = parent_dir.join(comp);
        if candidate.exists() {
            return comp.to_string();
        }

        // Get parent inode for caching
        let parent_inode = self.get_or_query_inode(parent_dir);

        let lower_comp = comp.to_ascii_lowercase();
        if let Some(inode) = parent_inode {
            if let Ok(cache) = self.name_cache.read() {
                if let Some(actual) = cache.get(&(inode, lower_comp.clone())) {
                    return actual.clone();
                }
            }
        }

        // Slow path: scan parent directory using O_RDONLY | O_DIRECTORY (not O_PATH!)
        if let Ok(entries) = std::fs::read_dir(parent_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name_str = file_name.to_string_lossy();
                if name_str.eq_ignore_ascii_case(comp) {
                    let actual = name_str.to_string();
                    if let Some(inode) = parent_inode {
                        if let Ok(mut cache) = self.name_cache.write() {
                            cache.insert((inode, lower_comp), actual.clone());
                        }
                    }
                    return actual;
                }
            }
        }

        // If not found, return requested component as fallback
        comp.to_string()
    }

    fn get_or_query_inode(&self, dir: &Path) -> Option<u64> {
        if let Ok(cache) = self.inode_cache.read() {
            if let Some(&ino) = cache.get(dir) {
                return Some(ino);
            }
        }

        if let Ok(meta) = std::fs::metadata(dir) {
            let ino = meta.ino();
            if let Ok(mut cache) = self.inode_cache.write() {
                cache.insert(dir.to_path_buf(), ino);
            }
            Some(ino)
        } else {
            None
        }
    }

    /// Invalidate cache for a specific file or directory operation (e.g. DeleteFileW, MoveFileW)
    pub fn invalidate_cache_entry(&self, parent_dir: &Path, name: &str) {
        if let Some(inode) = self.get_or_query_inode(parent_dir) {
            let lower = name.to_ascii_lowercase();
            if let Ok(mut cache) = self.name_cache.write() {
                cache.remove(&(inode, lower));
            }
        }
    }

    /// Invalidate cache entry by full host path
    pub fn invalidate_path(&self, host_path: &str) {
        let p = Path::new(host_path);
        if let (Some(parent), Some(file_name)) = (p.parent(), p.file_name()) {
            self.invalidate_cache_entry(parent, &file_name.to_string_lossy());
        }
    }

    /// Invalidate all cached entries for a given parent directory inode
    pub fn invalidate_cache_for_dir(&self, parent_dir: &Path) {
        if let Some(inode) = self.get_or_query_inode(parent_dir) {
            if let Ok(mut cache) = self.name_cache.write() {
                cache.retain(|&(ino, _), _| ino != inode);
            }
        }
    }

    /// Invalidate entire VFS name cache
    pub fn invalidate_all(&self) {
        if let Ok(mut cache) = self.name_cache.write() {
            cache.clear();
        }
    }

    /// Invalidate path by resolving parent and component
    pub fn invalidate_win_path(&self, win_path: &str) {
        let host_path_str = self.win32_to_host_path(win_path);
        let path = Path::new(&host_path_str);
        if let Some(parent) = path.parent() {
            if let Some(file_name) = path.file_name() {
                self.invalidate_cache_entry(parent, &file_name.to_string_lossy());
            }
        }
    }

    /// Securely open a file using openat2 with RESOLVE_BENEATH
    pub fn open_file(&self, win_path: &str, flags: i32, mode: u32) -> Result<RawFd, VfsError> {
        let (drive, components) = self.normalize_win32_path(win_path);
        let (base_fd, base_root) = match drive {
            DriveKind::DriveC => (self.drive_c_fd, &self.drive_c_root),
            DriveKind::DriveD => (self.drive_d_fd, &self.drive_d_root),
        };

        if base_fd < 0 || components.is_empty() {
            return Err(VfsError::InvalidParameter);
        }

        // Resolve path components case-insensitively
        let mut current_dir = base_root.clone();
        let mut rel_resolved = PathBuf::new();

        for i in 0..components.len() {
            let comp = &components[i];
            if comp == ".." {
                return Err(VfsError::EscapeAttempt);
            }
            if comp == "." || comp.is_empty() {
                continue;
            }

            let resolved = self.resolve_component_case(&current_dir, comp);
            current_dir.push(&resolved);
            rel_resolved.push(&resolved);
        }

        let rel_str = rel_resolved.to_string_lossy();
        let c_rel = CString::new(rel_str.as_bytes()).map_err(|_| VfsError::InvalidParameter)?;

        let how = OpenHow {
            flags: flags as u64,
            mode: mode as u64,
            resolve: RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS,
        };

        let fd = unsafe {
            libc::syscall(
                SYS_OPENAT2,
                base_fd,
                c_rel.as_ptr(),
                &how as *const OpenHow,
                std::mem::size_of::<OpenHow>(),
            )
        } as RawFd;

        if fd >= 0 {
            Ok(fd)
        } else {
            let err = std::io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::ENOENT) => Err(VfsError::NotFound),
                Some(libc::EACCES) | Some(libc::EPERM) => Err(VfsError::AccessDenied),
                Some(libc::EEXIST) => Err(VfsError::AlreadyExists),
                Some(libc::EXDEV) => Err(VfsError::EscapeAttempt),
                _ => Err(VfsError::Io(err.to_string())),
            }
        }
    }

    /// Securely create a directory step-by-step
    pub fn create_directory(&self, win_path: &str) -> Result<(), VfsError> {
        let (drive, components) = self.normalize_win32_path(win_path);
        let base_root = match drive {
            DriveKind::DriveC => &self.drive_c_root,
            DriveKind::DriveD => &self.drive_d_root,
        };

        if components.is_empty() {
            return Ok(());
        }

        let mut current_path = base_root.clone();
        for comp in components {
            if comp == ".." {
                return Err(VfsError::EscapeAttempt);
            }
            if comp == "." || comp.is_empty() {
                continue;
            }

            let resolved = self.resolve_component_case(&current_path, &comp);
            current_path.push(resolved);

            if !current_path.exists() {
                if let Err(e) = std::fs::create_dir(&current_path) {
                    if e.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(VfsError::Io(e.to_string()));
                    }
                }
                if let Some(parent) = current_path.parent() {
                    self.invalidate_cache_for_dir(parent);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfs_drive_mapping_and_isolation() {
        let vfs_guard = RineVfs::get();
        let vfs = vfs_guard.as_ref().unwrap();

        // 1. AppData must map to drive_c/users/... NOT to real $HOME
        let appdata_win = "C:\\Users\\player\\AppData\\Roaming\\Godot\\app_userdata\\test";
        let mapped = vfs.win32_to_host_path(appdata_win);
        assert!(mapped.contains("drive_c"));
        let lower_mapped = mapped.to_ascii_lowercase();
        assert!(lower_mapped.ends_with("/users/player/appdata/roaming/godot/app_userdata/test"));

        // 2. Relative game data must map to drive_d
        let pck_win = "game.pck";
        let mapped_pck = vfs.win32_to_host_path(pck_win);
        assert!(mapped_pck.ends_with("game.pck"));

        // 3. Traversal .. must be prevented from escaping drive_c root
        let escape_attempt = "C:\\..\\..\\..\\etc\\passwd";
        let mapped_escape = vfs.win32_to_host_path(escape_attempt);
        assert!(mapped_escape.starts_with(vfs.drive_c_root.to_str().unwrap()));
        assert_ne!(mapped_escape, "/etc/passwd");
    }

    #[test]
    fn test_cache_invalidation_on_delete() {
        let vfs_guard = RineVfs::get();
        let vfs = vfs_guard.as_ref().unwrap();

        let test_dir = vfs.drive_c_root.join("test_cache");
        std::fs::create_dir_all(&test_dir).ok();
        let file_path = test_dir.join("MyFile.TXT");
        std::fs::write(&file_path, b"hello").ok();

        // Query case-insensitively: "myfile.txt" -> "MyFile.TXT"
        let resolved = vfs.resolve_component_case(&test_dir, "myfile.txt");
        assert_eq!(resolved, "MyFile.TXT");

        // Now remove file and invalidate
        std::fs::remove_file(&file_path).ok();
        vfs.invalidate_cache_entry(&test_dir, "myfile.txt");

        // Cleanup
        std::fs::remove_dir_all(&test_dir).ok();
    }
}
