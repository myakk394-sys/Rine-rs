# RINE GETCURRENTDIRECTORYW & PATH MODEL REPORT

**Target Binary**: `That's not my neighbor.exe` (Version 1.0.3.3 Windows)  
**Image Base**: `0x0000000140000000`  
**Execution Environment**: Rine Win64 Compatibility Runtime  
**Test Status**: 75 passed, 0 failed, 0 warnings

---

## 1. CURRENT VERIFIED STATUS

Target game (*That's Not My Neighbor.exe*) executes cleanly under Rine across startup stages:
- **DirectWrite & Winsock2**: Fully passed.
- **Worker Thread Model**: Independent 4KB TEB, dedicated GS base, thread-local TLS and LastError, Win64 stack alignment (`RSP % 16 == 8`).
- **Resource Parsing / Indirect Calls**: `0x1429AD3B2` (`ResourceLoader::_recognize`) and `0x14260BF50` (`ResourceFormatLoaderBinary::recognize`) are verified and functional.
- **CRT Contract Audits**:
  - `_errno`: Function pointer bound to IAT; storage is strictly thread-local.
  - `Stat64`: Exact 56-byte Win64 MSVCRT ABI layout, 8-byte aligned, field offsets verified.
- **Drive Discovery (`GetLogicalDrives`)**: Proven passing at `0x14001BFFC` -> returns `0x00000004` (Drive `C:`).
- **Directory Query (`GetCurrentDirectoryW`)**: **RESOLVED AND PROVEN** at `0x140AA4560`.
- **Current Blocker**: `KERNEL32.dll!SetCurrentDirectoryW` (Call VA: `0x140AA4596`, Return VA: `0x140AA459C`, IAT slot: `0x1443A38A0`).

---

## 2. TARGET GETCURRENTDIRECTORYW CALL

Machine snapshot captured during GDB runtime execution at call site `0x140AA4560`:

```text
=== [PROVED TARGET CALL] REACHED 0x140AA4560 (GetCurrentDirectoryW) ===
RIP: 0x0000000140aa4560 (guest PE .text, RVA: 0x00AA4560)
RCX (nBufferLength): 0x800 (2048 WCHARs)
RDX (lpBuffer): 0x00007fffffff96e0 (main thread stack buffer)
R15: 0x0000555555f456e0 (loaded from IAT slot 0x1443a35c0)
IAT slot [0x1443A35C0] points to: 0x0000555555f456e0 (kernel32!GetCurrentDirectoryW)
```

---

## 3. CALLER DISASSEMBLY

Disassembly of enclosing Godot function `0x140AA44C0` (`DirAccessWindows::change_dir` / directory navigation):

```assembly
0x140aa4551: mov    %rsi,%rdx               ; RDX = lpBuffer (0x7FFFFFFF96E0)
0x140aa4554: mov    $0x800,%ecx             ; RCX = nBufferLength = 2048 WCHARs
0x140aa4559: mov    0x38ff060(%rip),%r15    # IAT: kernel32!GetCurrentDirectoryW
0x140aa4560: call   *%r15                   ; Invoke GetCurrentDirectoryW
0x140aa4563: mov    $0xffffffff,%r8d        ; Return point (RVA: 0x00AA4563)
0x140aa4569: mov    %rsi,%rdx               ; Pass returned UTF-16 buffer
0x140aa456c: mov    %rdi,%rcx               ; Godot String destination
0x140aa456f: movq   $0x0,0x38(%rsp)
0x140aa4578: call   0x142a16fe0             ; String::parse_utf16
0x140aa457d: mov    %r14,%rdx
0x140aa4580: mov    %rbx,%rcx
0x140aa4583: call   0x142a17580             ; Path comparison / prefix check
0x140aa4588: mov    %rbx,%rcx
0x140aa458b: call   0x1429c4d70             ; Path normalization
0x140aa4590: mov    0x38ff309(%rip),%rbp    # IAT: kernel32!SetCurrentDirectoryW (0x1443a38a0)
0x140aa4597: mov    %rax,%rcx               ; Pass target path WCHAR pointer
0x140aa459a: call   *%rbp                   ; Invoke SetCurrentDirectoryW
```

---

## 4. WIN32 API CONTRACT

- **Signature**: `DWORD WINAPI GetCurrentDirectoryW(DWORD nBufferLength, LPWSTR lpBuffer)`
- **Success Semantics**:
  - Writes the current directory path in UTF-16 into `lpBuffer`.
  - Appends a terminating null character (`0x0000`).
  - Returns the number of characters written **excluding** the terminating null.
- **Insufficient Buffer / Sizing Semantics**:
  - When `nBufferLength <= len` or `lpBuffer` is `NULL`: returns the required buffer size **including** the terminating null character (`len + 1`).
  - Does not write past `nBufferLength` characters.

---

## 5. RINE DRIVE REGISTRY

- Centralized in `src/dlls/kernel32.rs` via `LogicalDriveRegistry`.
- Primary Logical Drive: `C:` (Bit 2, mask `0x00000004`).
- Configured drives: `["C:"]`.

---

## 6. RINE CURRENT DIRECTORY MODEL

- Centralized process-level working directory state managed in [`src/dlls/kernel32.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/kernel32.rs):
  - `PROCESS_CWD: std::sync::RwLock<Option<String>>`
  - `get_process_current_directory() -> String`
  - `set_process_current_directory(dir: &str)`
- Documented in [`scratch/rine_current_directory_model.txt`](file:///home/fizzy/Рабочий%20стол/Rine/scratch/rine_current_directory_model.txt).

---

## 7. HOST CWD, EXECUTABLE DIRECTORY, GUEST CWD

- **Host CWD**: `/home/fizzy/Рабочий стол/Rine`
- **Target Exe Directory**: `/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows`
- **Rine Guest CWD**: `\home\fizzy\Рабочий стол\Rine` (normalized with Windows backslashes `\`)

---

## 8. PATH TRANSLATION & GETMODULEFILENAMEW CONSISTENCY

- `GetModuleFileNameW` returns `\home\fizzy\Загрузки\That's not my neighbor (1.0.3.3) Windows\That's not my neighbor.exe`.
- `GetCurrentDirectoryW` returns `\home\fizzy\Рабочий стол\Rine`.
- Both functions use consistent Windows backslash path formatting.

---

## 9. UTF-16 CONTRACT & IMPLEMENTATION

Implemented in [`src/dlls/kernel32.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/kernel32.rs):

```rust
#[no_mangle]
pub unsafe extern "win64" fn GetCurrentDirectoryW(
    n_buffer_length: u32,
    lp_buffer: *mut u16,
) -> u32 {
    let win_cwd = get_process_current_directory();
    let utf16: Vec<u16> = win_cwd.encode_utf16().collect();
    let len = utf16.len();

    if n_buffer_length <= (len as u32) || lp_buffer.is_null() {
        return (len + 1) as u32;
    }

    std::ptr::copy_nonoverlapping(utf16.as_ptr(), lp_buffer, len);
    *lp_buffer.add(len) = 0;

    len as u32
}
```

---

## 10. RESOLVER BINDING

- **DLL**: `KERNEL32.dll`
- **Symbol**: `GetCurrentDirectoryW`
- **Resolver**: Native symbol mapping in [`src/pe/imports.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/pe/imports.rs):
  ```rust
  "GetCurrentDirectoryW" => Some(dlls::kernel32::GetCurrentDirectoryW as *const () as usize),
  ```

---

## 11. TARGET OUTPUT BUFFER & TARGET RETURN PROOF

Captured in `scratch/gdb_getcurrentdirectoryw_success.log`:

```text
=== [PROVED TARGET CALL] REACHED 0x140AA4560 (GetCurrentDirectoryW) ===
RIP: 0x0000000140aa4560
RCX (nBufferLength): 0x800 (2048)
RDX (lpBuffer): 0x00007fffffff96e0
IAT slot [0x1443A35C0] points to: 0x0000555555f456e0
[RINE FILESYSTEM] GetCurrentDirectoryW -> '\home\fizzy\Рабочий стол\Rine' (len=29)

=== [PROVED TARGET RETURN] RETURNED TO 0x140AA4563 ===
RIP: 0x0000000140aa4563
EAX (return WCHAR count): 29 (0x0000001d)
RSI (buffer ptr): 0x00007fffffff96e0
UTF-16 buffer hex dump:
0x7fffffff96e0:	92	104	111	109	101	92	102	105
0x7fffffff96f0:	122	122	121	92	1056	1072	1073	1086
0x7fffffff9700:	1095	1080	1081	32	1089	1090	1086	1083
0x7fffffff9710:	92	82	105	110	101	0
```

---

## 12. GODOT PATH CONSUMER

- Godot converted the UTF-16 buffer at `0x140AA4578` into a String object.
- It evaluated the working path and proceeded to `0x140AA4596` to invoke `SetCurrentDirectoryW`.

---

## 13. REGRESSION TESTS

1. `test_get_current_directory_w_contract_suite`: Asserts sizing when buffer is sufficient (`len`), exact capacity (`len`), one WCHAR too small (`len + 1`), and zero capacity (`len + 1`).
2. `test_get_current_directory_w_unicode`: Asserts correct UTF-16 code units and round-trip decoding on non-ASCII paths (`C:\Тест\Проект`).
3. `test_get_current_directory_w_buffer_bounds`: Asserts no memory write past buffer boundaries using canary guards.
4. `test_iat_symbol_resolution`: Asserts `KERNEL32.dll!GetCurrentDirectoryW` resolves to native function.

---

## 14. CARGO RESULTS

```text
cargo fmt --check && cargo check && cargo test
test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

## 15. OLD 0x140AA455D STATUS

- **Status**: **RESOLVED AND PROVEN**.
- `GetCurrentDirectoryW` resolves natively, executes, populates UTF-16 string, returns 29 WCHARs in `EAX`, and returns to `0x140AA4563`.

---

## 16. NEW EXACT BLOCKER

Captured during live target runtime execution in `scratch/game_after_getcurrentdirectoryw.log`:

```text
========================================================
[RINE UNRESOLVED CALL]
stub_id=91
dll=KERNEL32.dll
symbol=SetCurrentDirectoryW
ordinal=N/A
importer=/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe
call_instruction_va=0x140AA4596
return_va=0x140AA459C
iat_slot_va=0x1443A38A0
iat_slot_value=0x7BDD0AE26B40
resolved_function_va=0x7BDD0AE26B40
caller_module=/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe
caller_rva=0xAA459C
rsp=0x7FFD1E6FCB30
rcx=0x5F9ED793D350 (lpPathName pointer to UTF-16 string)
rdx=0x2
r8=0x0
r9=0x6E
========================================================
[RINE UNRESOLVED API FATAL]
dll=KERNEL32.dll
symbol=SetCurrentDirectoryW
call_instruction_va=0x140AA4596
return_va=0x140AA459C
iat_slot_va=0x1443A38A0
========================================================
```

- **Thread**: Main thread (ID: `17659`).
- **Call Instruction VA**: `0x0000000140AA4596` (RVA: `0x00AA4596`).
- **Return VA**: `0x0000000140AA459C` (RVA: `0x00AA459C`).
- **Target Symbol**: `KERNEL32.dll!SetCurrentDirectoryW` (IAT Slot: `0x1443A38A0`).
- **Arguments**:
  - `RCX` (`lpPathName`): `0x5F9ED793D350` (UTF-16 directory path).

---

## 17. GRAPHICS STATUS

- `CreateDXGIFactory1`: NOT CALLED yet.
- `D3D11CreateDevice`: NOT CALLED yet.
- `D3D11CreateDeviceAndSwapChain`: NOT CALLED yet.
- `vkCreateInstance`: NOT CALLED yet.
- `visible frame`: NO.

---

## 18. NEXT ACTION

1. Implement `KERNEL32.dll!SetCurrentDirectoryW` in `src/dlls/kernel32.rs` (`pub unsafe extern "win64" fn SetCurrentDirectoryW(lp_path_name: *const u16) -> i32`).
2. Read UTF-16 `lp_path_name`, update process-level `PROCESS_CWD`, and sync with host POSIX `libc::chdir` if directory exists.
3. Return `1` (TRUE) on success, or `0` (FALSE) with `SetLastError(ERROR_PATH_NOT_FOUND)` on failure.
4. Bind `KERNEL32.dll!SetCurrentDirectoryW` in `src/pe/imports.rs`.
5. Add unit and regression tests for `SetCurrentDirectoryW`.
6. Re-run target game to advance to the next initialization milestone.

---

## 19. REQUIREMENTS SELF-CHECK

- [x] Baseline cargo checks & tests pass (75 tests passing).
- [x] Machine trace of `GetCurrentDirectoryW` captured at `0x140AA4560`.
- [x] Caller of `GetCurrentDirectoryW` disassembled and analyzed.
- [x] Win32 API contract implemented (UTF-16, length excluding null, sizing returning `len + 1`).
- [x] Sizing and boundary unit tests added (canary protection verified).
- [x] Rine directory model documented in `scratch/rine_current_directory_model.txt`.
- [x] Central process-level working directory state implemented.
- [x] Unicode non-ASCII test passed.
- [x] Target game runtime proof: `0x140AA4560` returned 29 WCHARs to `0x140AA4563`.
- [x] Godot path consumer traced.
- [x] Next exact blocker identified (`KERNEL32.dll!SetCurrentDirectoryW` @ `0x140AA4596`).
- [x] Graphics status tracked.

---

## 20. MILESTONE TABLE

| MILESTONE | STATUS | EVIDENCE / DETAILS |
| :--- | :---: | :--- |
| **`GetLogicalDrives` remains passed** | **YES** | Target game executes past `0x14001BFFC` with `mask = 0x00000004`. |
| **`GetCurrentDirectoryW` caller traced** | **PROVEN** | Disassembled `0x140AA44C0` (`DirAccessWindows::change_dir`). |
| **Rine cwd model identified** | **PROVEN** | Central `PROCESS_CWD` in `src/dlls/kernel32.rs`. |
| **`C:` mapping identified** | **PROVEN** | Declared in `LogicalDriveRegistry`. |
| **UTF-16 contract verified** | **PROVEN** | Tested for standard, exact, undersized, and non-ASCII paths. |
| **Target output buffer valid** | **PROVEN** | GDB dump shows 29 UTF-16 WCHARs with null terminator. |
| **Target return to 0x140AA4563** | **YES** | Target returned with `EAX = 29`. |
| **Godot consumed cwd** | **YES** | Processed through `String::parse_utf16` and advanced to `SetCurrentDirectoryW`. |
| **Path-model inconsistency found** | **NO** | Backslash format is uniformly supported by Godot String parser. |
| **Next blocker** | **PROVEN** | `KERNEL32.dll!SetCurrentDirectoryW` (Call VA: `0x140AA4596`, Return VA: `0x140AA459C`). |
| **`D3D11CreateDevice` called** | **NO** | Engine is in filesystem directory navigation stage. |
| **`vkCreateInstance` called** | **NO** | Not yet reached. |
| **Visible frame** | **NO** | Pre-window stage. |
