# RINE GETLOGICALDRIVES & NEXT BLOCKER REPORT

**Target Binary**: `That's not my neighbor.exe` (Version 1.0.3.3 Windows)  
**Image Base**: `0x0000000140000000`  
**Execution Environment**: Rine Win64 Compatibility Runtime (Native + Wine Provider)  
**Baseline Test Status**: 72 passed, 0 failed, 0 warnings

---

## 1. CURRENT VERIFIED STATUS

The target game executes under Rine through PE loading, TLS initialization, CRT startup, DirectWrite COM factory creation, Winsock2 initialization, and CRT thread creation (`_beginthreadex`).

- **Main Thread**: Running with main TEB `0x00007FFFF7F9F000` and GS base `0x00007FFFF7F9F000`.
- **Worker Thread**: Running guest routine `0x14255EB40` concurrently with independent 4KB TEB `0x00007FFFD9D4E000`, dedicated GS base, TLS, LastError, and Win64 stack alignment (`RSP % 16 == 8`).
- **Resource Loading**: Passed `0x1429AD3B2` (`ResourceLoader::_recognize`), `0x14260BF50` (`ResourceFormatLoaderBinary::recognize`), `0x142DA6550` (`FileAccess::open`), and `0x140AA73C0` (`FileAccessWindows::_open`).
- **CRT Fixes Verified**:
  - `Stat64` stack buffer overflow eliminated (exact 56-byte layout with canary guard protection).
  - `_errno` function pointer binding and per-thread storage verified.
- **Previous Blocker (`GetLogicalDrives` @ `0x14001BFFC`)**: **RESOLVED AND PROVEN**.
- **Current Blocker**: `kernel32.dll!GetCurrentDirectoryW` (Call VA: `0x140AA455D`, Return VA: `0x140AA4563`, IAT slot: `0x1443A35C0`).

---

## 2. _ERRNO FUNCTION BINDING

- **Import Resolution**: The IAT binder in [`src/pe/imports.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/pe/imports.rs) previously evaluated `unsafe { dlls::crt::_errno() }` at load time, storing the heap data pointer to the errno integer directly into the IAT.
- **Fix**: Updated to return the function entry point:
  ```rust
  "_errno" => Some(dlls::crt::_errno as *const () as usize),
  ```
- **Verification**: `IatSymbolResolver::resolve_symbol("msvcrt.dll", "_errno")` resolves directly to `crate::dlls::crt::_errno as *const () as usize`.

---

## 3. _ERRNO THREAD STORAGE

- **ABI Signature**: `pub unsafe extern "win64" fn _errno() -> *mut i32`
- **Thread-Local Storage**: Implemented via Rust `thread_local!` containing `std::cell::UnsafeCell<i32>` in [`src/dlls/crt.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/crt.rs).
- **Regression Test**: `test_errno_thread_isolation` validates that setting `*_errno() = 111` on the main thread is unaffected when a worker thread concurrently reads 0 and writes `*_errno() = 222`. After worker thread termination, `*_errno()` on the main thread remains `111`.

---

## 4. STAT64 ABI LAYOUT

Field-by-field layout audit matching Microsoft Visual C++ 64-bit CRT:

| Field Name | Type | Size (Bytes) | Field Offset (Bytes) |
| :--- | :--- | :---: | :---: |
| `st_dev` | `u32` | 4 | `+0` (`0x00`) |
| `st_ino` | `u16` | 2 | `+4` (`0x04`) |
| `st_mode` | `u16` | 2 | `+6` (`0x06`) |
| `st_nlink` | `i16` | 2 | `+8` (`0x08`) |
| `st_uid` | `i16` | 2 | `+10` (`0x0A`) |
| `st_gid` | `i16` | 2 | `+12` (`0x0C`) |
| `_pad0` | `u16` | 2 | `+14` (`0x0E`) |
| `st_rdev` | `u32` | 4 | `+16` (`0x10`) |
| `_pad` | `u32` | 4 | `+20` (`0x14`) |
| `st_size` | `i64` | 8 | `+24` (`0x18`) |
| `st_atime` | `i64` | 8 | `+32` (`0x20`) |
| `st_mtime` | `i64` | 8 | `+40` (`0x28`) |
| `st_ctime` | `i64` | 8 | `+48` (`0x30`) |
| **Total Size** | | **56** (`0x38`) | |
| **Alignment** | | **8** | |

- **Verification**: `test_stat64_size_and_alignment` asserts `sizeof(Stat64) == 56`, `alignof(Stat64) == 8`, and asserts the exact offset of every individual field.
- **Buffer Safety**: `test_wstat64_buffer_bounds_and_fields` confirms canary bytes before and after `Stat64` remain untouched during `_wstat64` execution.

---

## 5. GETLOGICALDRIVES TARGET CALL

Captured machine snapshot at `0x14001BFFC`:

```text
=== REACHED GetLogicalDrives CALL SITE (0x14001BFFC) ===
RIP: 0x000000014001bffc (guest PE .text)
RSP: 0x00007fffffff9e40 (main thread stack)
RAX: 0x0000000000000002
RBX: 0x00005555599f61e0 (DirAccessWindows object)
RCX: 0x00005555599f6328
RDX: 0x00000001441ed240
R8 : 0x0000000000000000
R9 : 0x00007ffff7c12ac8
Instruction: call *0x438765e(%rip) # 0x1443a3660
IAT slot [0x1443A3660] points to: 0x0000555555f45760 (kernel32!GetLogicalDrives in Rine)
```

---

## 6. GETLOGICALDRIVES CALLER DISASSEMBLY

Disassembly of enclosing function `0x14001BF50` (`DirAccessWindows::create` / drive initialization):

```assembly
0x14001bf50: push   %r14
0x14001bf52: push   %r13
0x14001bf54: push   %r12
0x14001bf56: push   %rbp
0x14001bf57: push   %rdi
0x14001bf58: push   %rsi
0x14001bf59: push   %rbx
0x14001bf5a: sub    $0x40,%rsp
...
0x14001bfdc: call   0x142e0a2b0             ; Memory::alloc_static
0x14001bfe9: mov    %rax,0x120(%rbx)        ; Initialize drive structure
0x14001bff7: call   0x142a10e50             ; String initialization
0x14001bffc: call   *0x438765e(%rip)        # IAT: kernel32!GetLogicalDrives
0x14001c002: xor    %ecx,%ecx               ; ECX = 0 (drive index A..Z)
0x14001c004: mov    $0x1,%r8d               ; R8D = 1
0x14001c010: mov    %r8d,%edx               ; EDX = 1
0x14001c013: shl    %cl,%edx                ; EDX = (1 << ECX)
0x14001c015: test   %eax,%edx               ; Test if drive bit is present
0x14001c017: je     0x14001c038             ; If not set, skip
0x14001c019: movslq 0x144(%rbx),%r9         ; R9 = current drive_count
0x14001c020: lea    0x41(%rcx),%r10d        ; R10 = 'A' + ECX (e.g. 'A'+2 = 'C')
0x14001c024: mov    %r9,%rdx
0x14001c027: mov    %r10b,0x128(%rbx,%r9,1) ; Store drive letter in drive array
0x14001c02f: add    $0x1,%edx
0x14001c032: mov    %edx,0x144(%rbx)        ; drive_count++
0x14001c038: add    $0x1,%ecx               ; ECX++
0x14001c03b: cmp    $0x1a,%ecx              ; Loop 26 iterations (A..Z)
0x14001c03e: jne    0x14001c010
0x14001c040: lea    0x38(%rsp),%rdi         ; Loop exit -> proceed to directory access
```

---

## 7. RINE DRIVE MODEL

- Documented in [`scratch/rine_drive_model.txt`](file:///home/fizzy/Рабочий%20стол/Rine/scratch/rine_drive_model.txt).
- **Central Drive Architecture**: `LogicalDriveRegistry` defined in [`src/dlls/kernel32.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/kernel32.rs).
- **Drive Mapping**:
  - `C:\` -> Primary virtual system drive (mapped to virtual root `/`).
  - Bitmask: `0x00000004` (`1 << 2` for `C:`).
  - No unmapped host mount points are exposed as random drives.

---

## 8. LOGICAL DRIVE MASK

- **WinAPI Contract**: `DWORD WINAPI GetLogicalDrives(void)`
- **Bit Representation**:
  - Bit 0: `A:`
  - Bit 1: `B:`
  - **Bit 2: `C:` (`0x00000004`)**
  - Bit 3: `D:`
- **Mask Returned**: `0x00000004` (Drive `C:` only).

---

## 9. NATIVE IMPLEMENTATION

Implemented in [`src/dlls/kernel32.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/kernel32.rs):

```rust
#[no_mangle]
pub unsafe extern "win64" fn GetLogicalDrives() -> u32 {
    let mask = get_drive_registry().get_drives_mask();
    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let mut drive_list = Vec::new();
        for i in 0..26 {
            if (mask & (1 << i)) != 0 {
                drive_list.push(format!("{}:", (b'A' + i) as char));
            }
        }
        println!(
            "[RINE FILESYSTEM] GetLogicalDrives -> mask=0x{:08X} drives={:?}",
            mask, drive_list
        );
    }
    mask
}
```

---

## 10. RESOLVER BINDING

- **DLL**: `KERNEL32.dll`
- **Symbol**: `GetLogicalDrives`
- **Resolver**: Normal native Rine symbol resolution in [`src/pe/imports.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/pe/imports.rs):
  ```rust
  "GetLogicalDrives" => Some(dlls::kernel32::GetLogicalDrives as *const () as usize),
  ```

---

## 11. TARGET RETURN PROOF

Machine proof captured during live GDB target execution:

```text
=== [PROVED TARGET CALL] REACHED 0x14001BFFC (GetLogicalDrives) ===
RIP: 0x000000014001bffc
IAT slot [0x1443A3660] points to: 0x0000555555f45760 (kernel32!GetLogicalDrives)
[RINE FILESYSTEM] GetLogicalDrives -> mask=0x00000004 drives=["C:"]

=== [PROVED TARGET RETURN] RETURNED TO 0x14001C002 ===
RIP: 0x000000014001c002
EAX (return mask): 0x00000004
RSP: 0x00007fffffffae80

=== LOOP BIT TEST: ECX=2, EDX=0x00000004, EAX=0x00000004 ===
=== STORED DRIVE LETTER: 'C' (0x43) at RBX+0x128+0 ===

=== COMPLETED DRIVE LOOP at 0x14001c040 ===
Total drives found [RBX+0x144]: 1
Drive array at [RBX+0x128]: C
```

---

## 12. FIRST DRIVE ENUMERATED

- **Drive Accepted**: Drive `C:` (Bit 2, `'C'`).
- **Stored Representation**: Letter `'C'` stored at `0x128(%rbx)`, drive count set to `1` at `0x144(%rbx)`.
- **Subsequent Operation**: Engine constructs root path for drive `C:` and invokes directory querying routines.

---

## 13. PATH TRANSLATION

- `GetModuleFileNameW` returns normalized Windows-slash path `\home\fizzy\Загрузки\That's not my neighbor (1.0.3.3) Windows\That's not my neighbor.exe`.
- Path translation in `_wstat64` handles both `\` and `/` separators seamlessly via POSIX `libc::stat`.

---

## 14. REGRESSION TESTS

1. `test_get_logical_drives_resolver`: Asserts `IatSymbolResolver` binds `KERNEL32.dll!GetLogicalDrives` to `crate::dlls::kernel32::GetLogicalDrives`.
2. `test_get_logical_drives_mask`: Asserts `GetLogicalDrives()` returns `0x00000004`, `mask & (1 << 2) != 0`, and no unexpected drive bits are set.
3. `test_get_logical_drives_win64_abi`: Asserts invocation through `extern "win64"` function pointer returns DWORD without register corruption.
4. `test_errno_thread_isolation`: Asserts per-thread `_errno` storage across concurrent threads.
5. `test_stat64_size_and_alignment`: Asserts exact 56-byte layout and all field offsets.

---

## 15. CARGO RESULTS

```text
cargo fmt --check && cargo check && cargo test
test result: ok. 72 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

## 16. OLD 0x14001BFFC STATUS

- **Status**: **PASSED & PROVEN**.
- `GetLogicalDrives` at `0x14001BFFC` resolves natively, executes, returns `0x00000004`, and returns to `0x14001C002`.

---

## 17. NEW EXACT BLOCKER

Captured during live target runtime execution in `scratch/game_after_getlogicaldrives.log`:

```text
========================================================
[RINE UNRESOLVED CALL]
stub_id=43
dll=KERNEL32.dll
symbol=GetCurrentDirectoryW
ordinal=N/A
importer=/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe
call_instruction_va=0x140AA455D
return_va=0x140AA4563
iat_slot_va=0x1443A35C0
iat_slot_value=0x7E445720C540
resolved_function_va=0x7E445720C540
caller_module=/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe
caller_rva=0xAA4563
rsp=0x7FFF42251480
rcx=0x800 (nBufferLength = 2048 characters)
rdx=0x7FFF42252DE0 (lpBuffer)
r8=0x7FFF42252D58
r9=0x0
========================================================
[RINE UNRESOLVED API FATAL]
dll=KERNEL32.dll
symbol=GetCurrentDirectoryW
call_instruction_va=0x140AA455D
return_va=0x140AA4563
========================================================
```

- **Thread Role**: Main thread (ID: `16151`).
- **RIP / Call Site**: `0x0000000140AA455D` (RVA: `0x00AA455D`).
- **Return Address**: `0x0000000140AA4563` (RVA: `0x00AA4563`).
- **Target Import**: `KERNEL32.dll!GetCurrentDirectoryW` (IAT Slot: `0x1443A35C0`).
- **Arguments**:
  - `RCX` (`nBufferLength`): `0x800` (2048 WCHARs).
  - `RDX` (`lpBuffer`): `0x7FFF42252DE0` (Stack buffer).

---

## 18. GRAPHICS STATUS

- `CreateDXGIFactory1`: NOT CALLED yet.
- `D3D11CreateDevice`: NOT CALLED yet.
- `D3D11CreateDeviceAndSwapChain`: NOT CALLED yet.
- `vkCreateInstance`: NOT CALLED yet.
- `visible frame`: NO.

---

## 19. NEXT ACTION

1. Implement `KERNEL32.dll!GetCurrentDirectoryW` in `src/dlls/kernel32.rs` according to Win64 ABI (`DWORD WINAPI GetCurrentDirectoryW(DWORD nBufferLength, LPWSTR lpBuffer)`).
2. Populate `lpBuffer` with the current working directory path formatted as Windows wide-character string, returning length in characters (excluding terminating null).
3. Bind in `src/pe/imports.rs`.
4. Add unit test for `GetCurrentDirectoryW`.
5. Re-run target game and observe the subsequent initialization milestone.

---

## 20. REQUIREMENTS SELF-CHECK

- [x] Baseline cargo checks & tests pass (72 tests passing).
- [x] `_errno` function pointer binding verified.
- [x] `_errno` per-thread storage implemented and verified with concurrent test.
- [x] `Stat64` ABI layout audited, verified to be 56 bytes with offset assertions.
- [x] Machine trace of `GetLogicalDrives` captured at `0x14001BFFC`.
- [x] Caller of `GetLogicalDrives` disassembled and analyzed.
- [x] Rine drive model documented in `scratch/rine_drive_model.txt`.
- [x] Central `LogicalDriveRegistry` implemented.
- [x] `GetLogicalDrives` returns mask `0x00000004` (`C:`).
- [x] `GetLogicalDrives` Win64 ABI and resolver tests added.
- [x] Actual target runtime proof: target returned to `0x14001C002` and processed drive `C:`.
- [x] Next exact blocker identified (`KERNEL32.dll!GetCurrentDirectoryW` @ `0x140AA455D`).
- [x] Graphics status tracked.

---

## 21. MILESTONE TABLE

| MILESTONE | STATUS | EVIDENCE / DETAILS |
| :--- | :---: | :--- |
| **`_errno` IAT=function** | **PROVEN** | `IatSymbolResolver` returns `dlls::crt::_errno` function pointer. |
| **`_errno` thread-local** | **PROVEN** | `test_errno_thread_isolation` passed (distinct storage per thread). |
| **`Stat64` exact layout** | **PROVEN** | `sizeof == 56`, `alignof == 8`, all 13 field offsets assert-verified. |
| **`GetLogicalDrives` caller traced** | **PROVEN** | Disassembled `0x14001BF50` loop (tests bits 0..25, stores 'C'). |
| **Rine drive model** | **PROVEN** | Central `LogicalDriveRegistry` configured for virtual `C:` drive (`0x04`). |
| **Mask contract** | **PROVEN** | `0x00000004` (Bit 2 set for `C:`). |
| **Target returned after call** | **PROVEN** | GDB break at `0x14001C002` with `EAX = 0x00000004`. |
| **Drive enumeration continued** | **YES** | Target populated drive list with `'C'` and exited loop to `0x14001C040`. |
| **Next blocker** | **PROVEN** | `KERNEL32.dll!GetCurrentDirectoryW` (Call VA: `0x140AA455D`, RVA: `0xAA4563`). |
| **`D3D11CreateDevice` called** | **NO** | Engine currently querying working directory / filesystem. |
| **`vkCreateInstance` called** | **NO** | Not yet reached. |
| **Visible frame** | **NO** | Pre-window / pre-swapchain stage. |
