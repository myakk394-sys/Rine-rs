# Milestone Report: CreateDirectoryW Implementation & Post-Project Progress

## 1. Current Verified Status

- **Embedded PCK & Project Initialization**: Functional. Godot opened embedded archive `GDPC` and initialized core engine structures.
- **Worker Thread Pool**: 11 worker threads running in parallel (`thread_id=514..524`), with distinct TEBs, GS bases, and Win64 stack frames.
- **Old Blocker `0x140AA4975` (CreateDirectoryW)**: Passed.
  - `CreateDirectoryW` successfully created the user directories:
    - `/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/Godot`
    - `/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/Godot/app_userdata`
    - `/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/Godot/app_userdata/That's not my neighbor`
    - `/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/Godot/app_userdata/That's not my neighbor/logs`
  - Handled pre-existing parent directories (`/home`, `/home/fizzy`, etc.) by returning `0` (FALSE) with `GetLastError = 183` (`ERROR_ALREADY_EXISTS`), which Godot correctly processed to proceed down the tree.
- **Engine Logging Subsystem**:
  - `_wfopen` opened `./Godot/app_userdata/That's not my neighbor/logs/godot.log` (`fd=43`, OK).
  - Godot Engine printed: `Godot Engine v4.2.stable.official.46dc27791 - https://godotengine.org`.
- **Test Suite**: 96 unit tests passing (0 failed).
- **Graphics Pipeline Status**:
  - DXVK mapped: YES (`0x0000000359050000`)
  - CreateDXGIFactory1 called: YES
  - D3D11CreateDevice called: NO (reached `GetLocalTime` before device creation)
  - vkCreateInstance called: NO
  - Visible frame: NO

---

## 2. Execution Plan

| Step | Action | File / Subsystem | Evidence Required | Pass Criterion | Outcome |
|:---|:---|:---|:---|:---|:---|
| 1 | Target trace before call | GDB / Target | Pre-call registers & UTF-16 dump | Capture exact `lpPathName` | Done (`scratch/gdb_createdirectoryw_call.log`) |
| 2 | Path classification | Central Path Model | Path type, CWD resolution | `\home` classified as root-relative | Done (`scratch/createdirectoryw_path_resolution.txt`) |
| 3 | Caller disassembly & purpose | Godot x64 .text | Caller disassembly around `0x140aa4975` | Godot `DirAccess::make_dir` and error branch mapped | Done (`scratch/createdirectoryw_consumer.txt`) |
| 4 | Implement `CreateDirectoryW` | `src/dlls/kernel32.rs` | Win64 ABI, `win32_to_host_path`, `SetLastError` | Non-recursive creation with distinct error codes | Implemented & tested |
| 5 | Bind IAT | `src/pe/imports.rs` | Symbol resolver entries | Normal resolution | Bound |
| 6 | Unit tests & CWD invariant | `src/dlls/kernel32.rs` | Contract suite covering success, collision, missing parent, CWD | 96 tests passing | All pass |
| 7 | Target return proof | GDB / Target | Post-call `EAX`, step into consumer | Returns to `0x140aa497b`, steps into consumer | Done (`scratch/gdb_createdirectoryw_success.log`) |
| 8 | Post-fix execution trace | Target runtime (90s) | Execution log | Sequence of directories created, next blocker discovered | Done (`scratch/game_after_createdirectoryw.log`) |

---

## 3. Existing Evidence Review

- `resolve_win32_path` and `win32_to_host_path` in `src/dlls/kernel32.rs` normalize relative, root-relative, and drive-qualified paths against `get_process_current_directory()`.
- Prior to this change, `CreateDirectoryW` was only defined in generated stub files, which returned `0` without creating directories or setting `LastError`.
- `LastError` is thread-local in Rine.

---

## 4. Target CreateDirectoryW Call Trace

Captured in `scratch/gdb_createdirectoryw_call.log`:

```
RIP: 0x0000000140aa4975
RSP: 0x00007fffffffa3d0
RAX: 0x0000000000000006
RBX: 0x0000555559ad24e0
RCX: 0x0000555559ad24e0 (lpPathName)
RDX: 0x0000000000000000 (lpSecurityAttributes = NULL)
RSI: 0x00007fffffffa428
RDI: 0x00007fffffffa420

CALL Instruction:
0x140aa4975: call *0x38feb8d(%rip)        # IAT slot 0x1443a3508
```

---

## 5. Input UTF-16 Path & Classification

- **Raw UTF-16 bytes**: `0x005C, 0x0068, 0x006F, 0x006D, 0x0065, 0x0000`
- **Decoded String**: `\home`
- **Path Classification**: Root-relative path (starts with `\`).
- **Normalized Guest Path**: `\home`
- **Translated Host Path**: `/home`
- **Pre-existing State**: `/home` already exists on the host.

---

## 6. Caller & Godot Purpose

Disassembly from `scratch/createdirectoryw_consumer.txt`:
```assembly
0x140aa4970: xor    %edx,%edx                   # rdx = NULL (lpSecurityAttributes)
0x140aa4972: mov    %rbx,%rcx                   # rcx = lpPathName
0x140aa4975: call   *0x38feb8d(%rip)            # call CreateDirectoryW
0x140aa497b: mov    %eax,%esi                   # esi = BOOL result
0x140aa497d: lock subl $0x1,-0x8(%rbx)          # string refcount decrement
0x140aa4982: je     0x140aa49c0
0x140aa4984: call   *0x38fecb6(%rip)            # call GetLastError
0x140aa498a: xor    %ebx,%ebx
0x140aa498c: test   %esi,%esi                   # if (result != 0) -> OK (return 0)
0x140aa498e: jne    0x140aa49a1
0x140aa4990: cmp    $0xb7,%eax                  # if (GetLastError() == 183 [ERROR_ALREADY_EXISTS])
0x140aa4995: je     0x140aa49d0                 # -> ebx = 0x20 (ERR_ALREADY_EXISTS)
0x140aa4997: cmp    $0x5,%eax                   # if (GetLastError() == 5 [ERROR_ACCESS_DENIED])
0x140aa499a: je     0x140aa49d0                 # -> ebx = 0x20 (ERR_ALREADY_EXISTS)
0x140aa499c: mov    $0x14,%ebx                  # else -> ebx = 0x14 (ERR_CANT_CREATE)
0x140aa49a1: mov    %r12,%rcx
0x140aa49a4: call   *0x38fedee(%rip)            # LeaveCriticalSection
0x140aa49aa: mov    %ebx,%eax                   # return Godot Error enum
```

**Purpose**: Godot's `DirAccess::make_dir_recursive` iterates through path segments from root to leaf, calling `make_dir` at each level. When a directory already exists, it expects `GetLastError() == 183` (`ERROR_ALREADY_EXISTS`) and proceeds down to the next level.

---

## 7. CreateDirectoryW Contract & Implementation

Implemented in `src/dlls/kernel32.rs`:
```rust
#[no_mangle]
pub unsafe extern "win64" fn CreateDirectoryW(
    lp_path_name: *const u16,
    lp_security_attributes: *const std::ffi::c_void, // [SECURITY ATTRIBUTES NOT EXERCISED]
) -> i32 {
    let _ = lp_security_attributes;

    if lp_path_name.is_null() {
        SetLastError(87); // ERROR_INVALID_PARAMETER
        return 0; // FALSE
    }

    let raw_str = match read_utf16_ptr(lp_path_name) {
        Some(s) => s,
        None => {
            SetLastError(87);
            return 0;
        }
    };

    if raw_str.is_empty() {
        SetLastError(3); // ERROR_PATH_NOT_FOUND
        return 0;
    }

    let host_path = win32_to_host_path(&raw_str);
    let target_path = std::path::Path::new(&host_path);

    if host_path == "/" || target_path.exists() {
        SetLastError(183); // ERROR_ALREADY_EXISTS
        return 0;
    }

    if let Some(parent) = target_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            SetLastError(3); // ERROR_PATH_NOT_FOUND
            return 0;
        }
    }

    match std::fs::create_dir(target_path) {
        Ok(()) => {
            // [ALPHA DIRECTORY SECURITY/PERMISSION SEMANTICS: default host umask]
            println!(
                "[RINE FILESYSTEM] CreateDirectoryW input='{}' host='{}' -> TRUE",
                raw_str, host_path
            );
            1 // TRUE
        }
        Err(e) => {
            let win_err = match e.kind() {
                std::io::ErrorKind::AlreadyExists => 183,
                std::io::ErrorKind::NotFound => 3,
                std::io::ErrorKind::PermissionDenied => 5,
                _ => 82,
            };
            SetLastError(win_err);
            0 // FALSE
        }
    }
}
```

---

## 8. Directory Creation Call Sequence

Saved in `scratch/createdirectoryw_calls.tsv`:

| Sequence | Path | Pre-exists | Result | LastError |
|:---|:---|:---|:---|:---|
| 1 | `\home` | YES | 0 (FALSE) | 183 (`ERROR_ALREADY_EXISTS`) |
| 2 | `\home\fizzy` | YES | 0 (FALSE) | 183 (`ERROR_ALREADY_EXISTS`) |
| 3 | `\home\fizzy\Загрузки` | YES | 0 (FALSE) | 183 (`ERROR_ALREADY_EXISTS`) |
| 4 | `\home\fizzy\Загрузки\That's not my neighbor (1.0.3.3) Windows` | YES | 0 (FALSE) | 183 (`ERROR_ALREADY_EXISTS`) |
| 5 | `...\Godot` | NO | 1 (TRUE) | 0 |
| 6 | `...\Godot\app_userdata` | NO | 1 (TRUE) | 0 |
| 7 | `...\Godot\app_userdata\That's not my neighbor` | NO | 1 (TRUE) | 0 |
| 8 | `...\Godot\app_userdata\That's not my neighbor\logs` | NO | 1 (TRUE) | 0 |

---

## 9. Process CWD Invariant

- Verified that `CreateDirectoryW` does not mutate `PROCESS_CWD`.
- Unit test `test_create_directory_w_contract_suite` asserts `cwd_before == cwd_after`.

---

## 10. Thread Pool & Engine Progress

- All 11 worker threads (`thread_id=514..524`) remained active with valid TEB/GS context.
- Log file `./Godot/app_userdata/That's not my neighbor/logs/godot.log` created and opened with `_wfopen(mode='wb')`.
- Banner `Godot Engine v4.2.stable.official.46dc27791 - https://godotengine.org` printed to stdout.

---

## 11. Next Exact Blocker

- **DLL**: `KERNEL32.dll`
- **Symbol**: `GetLocalTime`
- **Call Instruction VA**: `0x0000000142582E7E`
- **Return Address VA**: `0x0000000142582E84`
- **IAT Slot VA**: `0x00000001443A3648`
- **Caller RVA**: `0x02582E84`
- **Stub ID**: 51
- **Arguments**:
  - `RCX`: `0x7FFFFFFFA390` (`LPSYSTEMTIME lpSystemTime`)

---

## 12. Graphics Subsystem Status

| Subsystem Component | Status | Evidence |
|:---|:---|:---|
| DXVK Library Loaded | **YES** | `0x0000000359050000` |
| CreateDXGIFactory1 Called | **YES** | Initialized during early COM setup |
| D3D11CreateDevice Called | **NO** | Reached `GetLocalTime` before device creation |
| vkCreateInstance Called | **NO** | Waiting for DXVK D3D11 backend |
| Visible Window Frame | **NO** | Rendering pipeline not reached yet |

---

## 13. Requirements Self-Check

- [x] Decode exact target `lpPathName` (`\home`, `\home\fizzy\...`, `scratch/gdb_createdirectoryw_call.log`).
- [x] Determine what user/project directory Godot is trying to create (`app_userdata/That's not my neighbor/logs`).
- [x] Trace guest-path -> normalized guest-path -> host-path (`scratch/createdirectoryw_path_resolution.txt`).
- [x] Implement correct CreateDirectoryW semantics for exercised path (non-recursive, `ERROR_ALREADY_EXISTS`, `ERROR_PATH_NOT_FOUND`).
- [x] Prove target consumes BOOL result and continues (`scratch/gdb_createdirectoryw_success.log`).
- [x] Log full sequence of CreateDirectoryW calls (`scratch/createdirectoryw_calls.tsv`).
- [x] Verify Process CWD invariant.
- [x] Verify thread pool health (all 11 workers running).
- [x] Find the next exact blocker (`KERNEL32.dll!GetLocalTime` at `0x142582E7E`).
- [x] Monitor graphics milestones.

---

## 14. Milestone Summary Table

| Milestone | Status |
|:---|:---|
| embedded PCK still loading | **YES** |
| GetNativeSystemInfo remains passed | **YES** |
| 11-worker pool remains healthy | **YES** |
| CreateDirectoryW target path decoded | **PROVEN** |
| path translation | **PROVEN** |
| directory outcome | **PROVEN** |
| target return 0x140AA497B | **YES** |
| Godot continued | **YES** |
| next blocker | **0x2582E84 / KERNEL32.dll!GetLocalTime** |
| D3D11CreateDevice called | **NO** |
| entered DXVK D3D11 .text | **NO** |
| vkCreateInstance called | **NO** |
| swapchain created | **NO** |
| vkQueuePresentKHR called | **NO** |
| visible frame | **NO** |
