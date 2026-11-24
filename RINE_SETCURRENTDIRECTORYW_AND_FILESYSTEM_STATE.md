# RINE SETCURRENTDIRECTORYW & FILESYSTEM STATE REPORT

**Target Binary**: `That's not my neighbor.exe` (Version 1.0.3.3 Windows)  
**Image Base**: `0x0000000140000000`  
**Execution Environment**: Rine Win64 Compatibility Runtime  
**Test Suite**: 79 passed, 0 failed, 0 warnings

---

## 1. CURRENT VERIFIED STATUS

Target game (*That's Not My Neighbor.exe*) executes cleanly under Rine across startup stages:
- **DirectWrite & Winsock2**: Fully passed.
- **Worker Thread Model**: Independent 4KB TEB, dedicated GS base, thread-local TLS and LastError, Win64 stack alignment (`RSP % 16 == 8`).
- **Resource Parsing / Binary Recognition**: `0x1429AD3B2` and `0x14260BF50` verified and functional.
- **Drive Discovery (`GetLogicalDrives`)**: Proven passing at `0x14001BFFC` -> returns `0x00000004` (Drive `C:`).
- **Directory Query (`GetCurrentDirectoryW`)**: Proven passing at `0x140AA4560` -> returns 29 WCHARs to `0x140AA4563`.
- **Directory Modification (`SetCurrentDirectoryW`)**: **RESOLVED AND PROVEN** at `0x140AA459A` -> returns `1` (TRUE) to `0x140AA459C`.
- **Current Blocker**: `KERNEL32.dll!GetConsoleScreenBufferInfo` (Call VA: `0x140013A58`, Return VA: `0x140013A5E`, IAT slot: `0x1443A35B8`).

---

## 2. EXISTING EVIDENCE REVIEW

- **What is PROVEN**:
  - `GetLogicalDrives` exposes bit 2 (`0x00000004`), Godot identifies drive `C:` and stores it at `[RBX+0x128]`.
  - `GetCurrentDirectoryW` writes UTF-16 representation of the current working directory, returns length in WCHARs excluding null (`29`), and Godot converts it via `String::parse_utf16`.
  - Godot's `DirAccessWindows::change_dir` normalizes paths and calls `SetCurrentDirectoryW`.
  - Target call to `SetCurrentDirectoryW` passes `lpPathName = "."`.
- **What was HYPOTHESIS**:
  - The exact form of `lpPathName` passed by Godot during initialization (now proven to be `"."`).
  - Next unresolved API after filesystem directory access (now proven to be `KERNEL32.dll!GetConsoleScreenBufferInfo`).
- **Architecture & Duplication Review**:
  - `PROCESS_CWD` in [`src/dlls/kernel32.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/kernel32.rs) now acts as the single central source of truth for process-wide working directory state.
  - Path normalization handles `.`, `..`, root-relative `\path`, and drive-absolute `C:\path` uniformly.

---

## 3. TARGET SETCURRENTDIRECTORYW CALL

Machine snapshot captured during GDB runtime execution at call site `0x140AA459A`:

```text
=== [PROVED TARGET CALL] REACHED 0x140AA459A (SetCurrentDirectoryW) ===
RIP: 0x0000000140aa459a (guest PE .text, RVA: 0x00AA459A)
RCX (lpPathName ptr): 0x00005555599fc9f0
IAT slot [0x1443A38A0] points to: 0x0000555555f47b70 (kernel32!SetCurrentDirectoryW)
RBP: 0x0000555555f47b70
RSP: 0x00007fffffff9320
```

---

## 4. INPUT UTF-16 PATH

Dump of memory at `RCX` (`0x00005555599fc9f0`):

```text
0x5555599fc9f0: 0x002e ('.')  0x0000 (NUL)
```

- **Decoded String**: `"."` (ASCII 46, length 1, null-terminated).

---

## 5. PATH CLASSIFICATION

- **Path Form**: **Class C (Normal Relative / Current Directory Reference)**.
- Target `lpPathName` is `"."`.

---

## 6. CURRENT DRIVE & C: DRIVE MAPPING

- **Current Drive**: `C:` (Alpha drive registry policy).
- **Drive Mapping**: `C:\` maps directly to host root `/` in current Alpha policy.
- **Drive Root**:
  - Guest: `C:\`
  - Host: `/`

---

## 7. PROCESS CWD STATE

- Managed via `PROCESS_CWD: std::sync::RwLock<Option<String>>` in `src/dlls/kernel32.rs`.
- Read via `get_process_current_directory() -> String`.
- Written via `set_process_current_directory(dir: &str)`.
- Shared across all guest threads (main and workers).

---

## 8. PATH RESOLUTION & GUEST TO HOST TRANSLATION

Given:
- `input_path` = `"."`
- `current_cwd` = `\home\fizzy\Рабочий стол\Rine`
- `current_drive` = `C:`

Resolution steps:
1. `"."` maps to `current_cwd` (`\home\fizzy\Рабочий стол\Rine`).
2. Components normalized via `normalize_win32_path_components`.
3. Translated to host POSIX path: `/home/fizzy/Рабочий стол/Rine`.

---

## 9. DIRECTORY VALIDATION

- Validates existence and directory attributes using POSIX `libc::stat`:
  ```rust
  let mut st: libc::stat = std::mem::zeroed();
  let res = libc::stat(c_path.as_ptr(), &mut st);
  if res != 0 {
      SetLastError(3); // ERROR_PATH_NOT_FOUND
      return 0;
  }
  if (st.st_mode & libc::S_IFMT) != libc::S_IFDIR {
      SetLastError(267); // ERROR_DIRECTORY
      return 0;
  }
  ```
- If valid directory: updates `PROCESS_CWD` and returns `1` (TRUE).
- If non-existent or regular file: keeps previous `PROCESS_CWD`, sets `LastError`, and returns `0` (FALSE).

---

## 10. SETCURRENTDIRECTORYW CONTRACT & IMPLEMENTATION

Implemented in [`src/dlls/kernel32.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/kernel32.rs):

```rust
#[no_mangle]
pub unsafe extern "win64" fn SetCurrentDirectoryW(lp_path_name: *const u16) -> i32 {
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
        SetLastError(87);
        return 0;
    }

    let current_cwd = get_process_current_directory();
    let resolved_guest = resolve_win32_path(&raw_str, &current_cwd);
    let host_path = win32_to_host_path(&resolved_guest);

    let c_path = match std::ffi::CString::new(host_path.clone()) {
        Ok(c) => c,
        Err(_) => {
            SetLastError(87);
            return 0;
        }
    };

    let mut st: libc::stat = std::mem::zeroed();
    let res = libc::stat(c_path.as_ptr(), &mut st);
    if res != 0 {
        SetLastError(3); // ERROR_PATH_NOT_FOUND
        return 0;
    }

    if (st.st_mode & libc::S_IFMT) != libc::S_IFDIR {
        SetLastError(267); // ERROR_DIRECTORY
        return 0;
    }

    set_process_current_directory(&resolved_guest);
    1 // TRUE
}
```

---

## 11. LASTERROR & SET/GET ROUNDTRIP

- On success: `SetLastError` is untouched.
- On failure: appropriate Win32 error code (`ERROR_PATH_NOT_FOUND` = 3, `ERROR_DIRECTORY` = 267, `ERROR_INVALID_PARAMETER` = 87) is set.
- Round-trip tested: `SetCurrentDirectoryW(path) -> TRUE` followed by `GetCurrentDirectoryW()` returns the exact resolved normalized path.

---

## 12. GETCURRENTDIRECTORYW NULL CASE & GETMODULEFILENAMEW CONSISTENCY

- `GetCurrentDirectoryW(0, NULL)` returns `len + 1` (sizing request).
- Consistency:
  - `GetCurrentDirectoryW`: `\home\fizzy\Рабочий стол\Rine`
  - `GetModuleFileNameW`: `\home\fizzy\Загрузки\That's not my neighbor (1.0.3.3) Windows\That's not my neighbor.exe`
  - **Path Model Consistency**: **PROVEN** (uniform backslash format across Win32 APIs).

---

## 13. TARGET RETURN PROOF & POST-CALL CWD

Captured in `scratch/gdb_setcurrentdirectoryw_success.log`:

```text
=== [PROVED TARGET CALL] REACHED 0x140AA459A (SetCurrentDirectoryW) ===
RIP: 0x0000000140aa459a
RCX (lpPathName ptr): 0x00005555599fc9f0
IAT slot [0x1443A38A0] points to: 0x0000555555f47b70
[RINE FILESYSTEM] SetCurrentDirectoryW input='.' resolved_guest='\home\fizzy\Рабочий стол\Rine' host='/home/fizzy/Рабочий стол/Rine' -> TRUE

=== [PROVED TARGET RETURN] RETURNED TO 0x140AA459C ===
RIP: 0x0000000140aa459c
EAX (return BOOL): 1 (0x00000001)
RSP: 0x00007fffffff9320
```

- Target returned to `0x140AA459C` with `EAX = 1`.
- Godot completed `DirAccessWindows::change_dir` successfully.

---

## 14. REGRESSION TESTS

1. `test_set_current_directory_w_dot`: Asserts `SetCurrentDirectoryW(".")` returns TRUE and keeps valid CWD.
2. `test_set_current_directory_w_nonexistent_does_not_change_state`: Asserts failure on non-existent path, verifies `LastError != 0` and `PROCESS_CWD` rollback.
3. `test_set_then_get_current_directory_roundtrip`: Asserts set followed by get round-trip consistency.
4. `test_process_cwd_shared_between_threads`: Asserts cross-thread shared state of `PROCESS_CWD`.
5. `test_iat_symbol_resolution`: Asserts `KERNEL32.dll!SetCurrentDirectoryW` resolves to native function.

---

## 15. CARGO RESULTS

```text
cargo test --lib
test result: ok. 79 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

## 16. OLD 0x140AA4596 STATUS

- **Status**: **RESOLVED AND PROVEN**.
- `SetCurrentDirectoryW` resolves natively, executes, updates `PROCESS_CWD`, returns `1` (TRUE) in `EAX`, and returns cleanly to `0x140AA459C`.

---

## 17. NEW EXACT BLOCKER

Captured during live target runtime execution in `scratch/game_after_setcurrentdirectoryw.log`:

```text
========================================================
[RINE UNRESOLVED CALL]
stub_id=42
dll=KERNEL32.dll
symbol=GetConsoleScreenBufferInfo
ordinal=N/A
importer=/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe
call_instruction_va=0x140013A58
return_va=0x140013A5E
iat_slot_va=0x1443A35B8
iat_slot_value=0x7173A9AA2520
resolved_function_va=0x7173A9AA2520
caller_module=/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe
caller_rva=0x13A5E
rsp=0x7FFF0B0FE380
rcx=0x8 (hConsoleOutput)
rdx=0x7FFF0B0FFCD0 (lpConsoleScreenBufferInfo ptr)
r8=0x143D06C30
r9=0x30F
========================================================
[RINE UNRESOLVED API FATAL]
dll=KERNEL32.dll
symbol=GetConsoleScreenBufferInfo
call_instruction_va=0x140013A58
return_va=0x140013A5E
iat_slot_va=0x1443A35B8
========================================================
```

- **Thread**: Main thread (ID: `20428`).
- **Call Instruction VA**: `0x0000000140013A58` (RVA: `0x00013A58`).
- **Return VA**: `0x0000000140013A5E` (RVA: `0x00013A5E`).
- **Target Symbol**: `KERNEL32.dll!GetConsoleScreenBufferInfo` (IAT Slot: `0x1443A35B8`).
- **Arguments**:
  - `RCX` (`hConsoleOutput`): `0x8` (Console stdout pseudo-handle).
  - `RDX` (`lpConsoleScreenBufferInfo`): `0x7FFF0B0FFCD0` (pointer to 22-byte `CONSOLE_SCREEN_BUFFER_INFO` struct).

---

## 18. GRAPHICS STATUS

- `CreateDXGIFactory1`: NOT CALLED yet.
- `D3D11CreateDevice`: NOT CALLED yet.
- `D3D11CreateDeviceAndSwapChain`: NOT CALLED yet.
- `vkCreateInstance`: NOT CALLED yet.
- `visible frame`: NO.

---

## 19. NEXT ACTION

1. Disassemble caller around `0x140013A58` (`Godot Win32 Console Logger`).
2. Verify Win64 ABI struct layout for `CONSOLE_SCREEN_BUFFER_INFO` (22 bytes, 2-byte field alignment: `dwSize`, `dwCursorPosition`, `wAttributes`, `srWindow`, `dwMaximumWindowSize`).
3. Implement `KERNEL32.dll!GetConsoleScreenBufferInfo` in `src/dlls/kernel32.rs`.
4. Bind `KERNEL32.dll!GetConsoleScreenBufferInfo` in `src/pe/imports.rs`.
5. Add unit and regression tests for `GetConsoleScreenBufferInfo`.
6. Re-run target game to advance to the next initialization milestone.

---

## 20. REQUIREMENTS SELF-CHECK

- [x] Baseline cargo checks & tests pass (79 tests passing).
- [x] Existing evidence reviewed before changing code.
- [x] Target `SetCurrentDirectoryW` call argument dumped in GDB (`lpPathName = "."`).
- [x] Target path classified (Class C: Normal Relative).
- [x] Win32 contract implemented (resolves path, validates directory via `stat`, updates `PROCESS_CWD`, returns `BOOL`).
- [x] `PROCESS_CWD` shared across threads and protected.
- [x] Unicode non-ASCII test passed.
- [x] Target return proved (`0x140AA459A` returned `1` in `EAX` to `0x140AA459C`).
- [x] Post-call CWD confirmed.
- [x] Next exact blocker identified (`KERNEL32.dll!GetConsoleScreenBufferInfo` @ `0x140013A58`).
- [x] Graphics status tracked.

---

## 21. MILESTONE TABLE

| MILESTONE | STATUS | EVIDENCE / DETAILS |
| :--- | :---: | :--- |
| **`GetCurrentDirectoryW` remains passed** | **YES** | Target executes past `0x140AA4560` returning 29 WCHARs. |
| **Target `SetCurrentDirectoryW` traced** | **PROVEN** | Machine trace captured at `0x140AA459A`. |
| **Input path decoded** | **PROVEN** | `lpPathName = "."` (UTF-16 `0x002E 0x0000`). |
| **Path class** | **PROVEN** | Class C (Normal Relative / Current Directory Reference). |
| **Current drive** | **PROVEN** | Declared in `LogicalDriveRegistry` as `C:`. |
| **`C:` host mapping** | **PROVEN** | `C:\` maps to host root `/` in Alpha policy. |
| **Process cwd updated** | **PROVEN** | `PROCESS_CWD` updated to `\home\fizzy\Рабочий стол\Rine`. |
| **Set/Get roundtrip** | **PROVEN** | Unit test `test_set_then_get_current_directory_roundtrip` verified. |
| **Target return `0x140AA459C`** | **YES** | Target returned to `0x140AA459C` with `EAX = 1`. |
| **Godot continued** | **YES** | Advanced past `DirAccessWindows` to console initialization. |
| **Path model consistency** | **PROVEN** | Uniform backslash representation across all WinAPI callers. |
| **Next blocker** | **PROVEN** | `KERNEL32.dll!GetConsoleScreenBufferInfo` (`0x140013A58` -> `0x140013A5E`). |
| **`D3D11CreateDevice` called** | **NO** | Engine is in console/logger initialization stage. |
| **`vkCreateInstance` called** | **NO** | Not yet reached. |
| **Visible frame** | **NO** | Pre-window stage. |
