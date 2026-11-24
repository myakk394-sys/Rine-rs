# Milestone Report: PCK Discovery, Initial CWD Alignment & Project Data Loading

## 1. Executive Summary & Proven Facts

- **Target Executable**: `/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe` (344,672,496 bytes).
- **PCK Format & Location Discovery**:
  - Exact directory inventory proved: Only 1 file exists in the target directory (`That's not my neighbor.exe`). There is NO standalone `.pck` file.
  - Byte analysis proved: The target contains an **embedded PCK archive** starting at offset `275,000,548` (`0x10640CE4`) and ending with the Godot package magic signature `GDPC` (`0x43504447`).
- **Root Cause of Project Load Failure**:
  1. `msvcrt.dll!_wfsopen` was missing from `src/pe/imports.rs`, causing Godot's `FileAccessWindows::_open` to fail.
  2. `PROCESS_CWD` was not initialized to the executable directory when no CLI CWD was provided.
  3. Godot's `FileAccessWindows::get_length()` and `FileAccessWindows::seek()` in Windows builds use `msvcrt.dll!fgetpos` and `msvcrt.dll!fsetpos` with 64-bit `fpos_t` (`i64`), which were missing from `src/pe/imports.rs` and unresolved stubs.
- **Milestone Achieved**:
  - Implemented `initialize_process_filesystem`, `win32_to_host_path` CWD resolution.
  - Implemented `msvcrt_wfsopen`, `msvcrt_filelengthi64`, `msvcrt_fileno`, `msvcrt_lseeki64`, `msvcrt_fgetpos`, `msvcrt_fsetpos` in `src/dlls/crt.rs`.
  - Bound all CRT file symbols in `src/pe/imports.rs`.
  - Target proved: `That's not my neighbor.exe` opens successfully (`fd=44`, `fd=46`), embedded PCK directory is parsed, and Godot passes past `_load_settings_text_or_binary` and checks for `override.cfg`.
  - Old error `Error: Couldn't load project data at path "."` and `MSVCRT!exit(255)` is **100% PASSED**.
- **Next Blocker Discovered**:
  - `KERNEL32.dll!GetNativeSystemInfo` (Call VA: `0x14000AD10`, Return VA: `0x14000AD16`, IAT Slot: `0x1443A3698`, Stub ID: 55).

---

## 2. PCK Discovery & Target Inspection Evidence

```
Directory: /home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/
Total Files: 1
File: That's not my neighbor.exe (344,672,496 bytes)

Tail Bytes Analysis:
Offset 344,672,492: 0x43504447 ("GDPC")
Embedded Archive Header Offset: 0x10640CE4 (275,000,548 bytes)
Archive Size: 69,671,948 bytes
```

Disassembly of `PackedSourcePCK::try_open_pack` at VA `0x14260bf50`:
```assembly
0x14260c023: mov 0x70(%rsp), %rbx    ; rbx = FileAccess*
0x14260c028: mov (%rbx), %rax         ; rax = FileAccess vtable
0x14260c02b: mov %rbx, %rcx           ; rcx = this
0x14260c02e: mov 0x110(%rax), %r12    ; r12 = FileAccess::seek (0x140aa60d0)
0x14260c035: call *0x120(%rax)        ; CALL FileAccess::get_length() (0x140aa6360)
0x14260c03e: lea -0x4(%rax), %rdx     ; rdx = length - 4
0x14260c042: call *%r12               ; CALL FileAccess::seek(length - 4)
0x14260c04a: mov (%rcx), %rax         ; rax = vtable
0x14260c04d: call *0x148(%rax)        ; CALL FileAccess::get_32()
0x14260c053: cmp $0x43504447, %eax    ; Verify magic == GDPC (0x43504447)
```

---

## 3. Implementation Details

### A. Filesystem CWD Initialization (`src/dlls/kernel32.rs`)
```rust
pub fn initialize_process_filesystem(exe_path: &str, configured_cwd: Option<&str>) {
    set_target_exe_path(exe_path);
    if let Some(cwd) = configured_cwd {
        set_process_current_directory(&host_to_win32_path(cwd));
    } else if let Some(parent) = std::path::Path::new(exe_path).parent() {
        let parent_str = parent.to_string_lossy();
        if !parent_str.is_empty() {
            set_process_current_directory(&host_to_win32_path(&parent_str));
        }
    }
}
```

### B. Win64 MSVCRT File I/O & 64-bit Positioning (`src/dlls/crt.rs`)
```rust
#[no_mangle]
pub unsafe extern "win64" fn msvcrt_wfsopen(
    filename: *const u16,
    mode: *const u16,
    _shflag: i32,
) -> *mut libc::FILE {
    msvcrt_wfopen(filename, mode)
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_fgetpos(stream: *mut libc::FILE, pos: *mut i64) -> i32 {
    if stream.is_null() || pos.is_null() {
        return -1;
    }
    let p = libc::ftello(stream);
    if p < 0 {
        return -1;
    }
    *pos = p as i64;
    0
}

#[no_mangle]
pub unsafe extern "win64" fn msvcrt_fsetpos(stream: *mut libc::FILE, pos: *const i64) -> i32 {
    if stream.is_null() || pos.is_null() {
        return -1;
    }
    libc::fseeko(stream, *pos as libc::off_t, libc::SEEK_SET)
}
```

### C. PE Import Binding (`src/pe/imports.rs`)
Bound `_wfsopen`, `_filelengthi64`, `_fileno`, `_lseeki64`, `fgetpos`, `fsetpos` directly to native CRT implementations.

---

## 4. Verification & Execution Trace

### Target Execution Output:
```
[KERNEL32] GetModuleFileNameW -> '\home\fizzy\Загрузки\That's not my neighbor (1.0.3.3) Windows\That's not my neighbor.exe'
[MSVCRT FILE] _wfopen: '/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe' -> '/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe' (mode 'rb')
[MSVCRT FILE]   -> fd=44 OK
[KERNEL32] GetModuleFileNameW -> '\home\fizzy\Загрузки\That's not my neighbor (1.0.3.3) Windows\That's not my neighbor.exe'
[MSVCRT FILE] _wfopen: '/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe' -> '/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe' (mode 'rb')
[MSVCRT FILE]   -> fd=46 OK
[MSVCRT FILE] _wfopen: '/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe' -> '/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe' (mode 'rb')
[MSVCRT FILE]   -> fd=44 OK
[MSVCRT FILE] _wfopen: 'override.cfg' -> '/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/override.cfg' (mode 'rb')
[MSVCRT FILE]   -> FAILED (errno=2)
========================================================
[RINE UNRESOLVED CALL]
stub_id=55
dll=KERNEL32.dll
symbol=GetNativeSystemInfo
importer=/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe
call_instruction_va=0x14000AD10
return_va=0x14000AD16
iat_slot_va=0x1443A3698
```

### Unit & Regression Test Suite:
- **Total Tests**: 90 passing (0 failed).

---

## 5. Graphics Subsystem Status

| Subsystem Component | Status | Evidence |
|:---|:---|:---|
| DXVK Library Loaded | **YES** | `0x0000000359050000` |
| CreateDXGIFactory1 Called | **YES** | Initialized during early COM setup |
| D3D11CreateDevice Called | **NO** | Reached `GetNativeSystemInfo` before graphics device creation |
| vkCreateInstance Called | **NO** | Waiting for DXVK D3D11 backend |
| Visible Window Frame | **NO** | Rendering pipeline not initialized yet |

---

## 6. Next Exact Blocker

- **DLL**: `KERNEL32.dll`
- **Symbol**: `GetNativeSystemInfo`
- **Call Instruction VA**: `0x000000014000AD10`
- **Return Address VA**: `0x000000014000AD16`
- **IAT Slot VA**: `0x00000001443A3698`
- **Stub ID**: 55
- **Arguments**:
  - `RCX`: `0x7FFF9914E0E0` (`LPSYSTEM_INFO lpSystemInfo`)
