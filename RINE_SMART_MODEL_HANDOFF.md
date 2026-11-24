# RINE Technical Handoff & Architecture Audit (Smart Model Handoff)

============================================================
1. СОСТОЯНИЕ РЕПОЗИТОРИЯ
============================================================

### Git Output Verification:
- **`git status --short`**:
  ```text
   M src/dlls/advapi32.rs
   M src/dlls/d3d11.rs
   M src/dlls/dxgi.rs
   M src/dlls/mod.rs
   M src/dlls/registry.rs
   M src/pe/imports.rs
   M src/sys/signal.rs
   M src/sys/trampoline.rs
  ?? RINE_BEFORE_MOCK_VTABLE_RCA.diff
  ?? RINE_DXVK_RUNTIME_AFTER_FIX.log
  ?? RINE_SMART_MODEL_CURRENT.diff
  ?? RINE_SMART_MODEL_FILE_INDEX.txt
  ?? RINE_SMART_MODEL_LOG_EXCERPT.txt
  ?? RINE_VULKAN_AUDIT.md
  ?? gui/
  ?? scratch/
  ?? src/dlls/object_registry.rs
  ?? target/
  ?? wine/
  ```
- **`git rev-parse HEAD`**: `42dd98f53f2525f534d69b6c41ee289087c19f40`
- **`git diff --stat`**:
  ```text
   src/dlls/advapi32.rs  |  90 +++++++++++++++++++++
   src/dlls/d3d11.rs     | 106 ++++++++++++++++---------
   src/dlls/dxgi.rs      | 102 ++++++++++++++++++++++--
   src/dlls/mod.rs       |   1 +
   src/dlls/registry.rs  |   2 +-
   src/pe/imports.rs     |  20 +++++
   src/sys/signal.rs     | 214 ++++++++++++++++++++------------------------------
   src/sys/trampoline.rs |  89 ++++++++++++++++++++-
   8 files changed, 448 insertions(+), 176 deletions(-)
  ```
- **`git diff --check`**: Clean (no trailing whitespace errors).
- **Staged Changes (`git diff --cached --stat`)**: None (0 staged files).
- **Repository State Note**: Working tree contains uncommitted changes in `d3d11.rs`, `dxgi.rs`, `object_registry.rs`, `advapi32.rs`, `registry.rs`, `imports.rs`, `signal.rs`, `trampoline.rs` implementing safety patches and exact API signatures.

---

============================================================
2. EXECUTIVE SUMMARY — (НЕ БОЛЕЕ 35 СТРОК)
============================================================

- **Rine Engine**: PE64/Windows native runtime on Linux written in Rust.
- **Target EXE**: `That's not my neighbor.exe` (Godot Engine v4.x 64-bit PE64 executable).
- **Guest / Host Architecture**: Windows x86_64 Guest / Linux x86_64 Host.
- **Subsystem Execution Status**:
  - `[PROVEN]` PE64 mapping at ImageBase `0x140000000`, section loading, relocation processing.
  - `[PROVEN]` PEB/TEB initialization, `gs:[0x30]`/`gs:[0x60]` segment base setup.
  - `[PROVEN]` PE Entry Point `0x1400013D0` reached.
  - `[PROVEN]` MSVC CRT static initializers (`_initterm`) executed cleanly.
  - `[PROVEN]` `TlsAlloc` (TLS Index 10, 11) and `Ole32!CoInitializeEx` reached.
- **Current Runtime Crash**:
  - `[PROVEN]` `RIP = 0x0000000142585F16` (`main_exe` RVA `0x02585F16`).
  - `[PROVEN]` `mov rax, [rsi]` faults because `mov rsi, [rcx + 0x48]` loads uninitialized recycled heap bytes `0x303078300A646574` (`"ted\n0x00"`).
- **Graphics Pipeline Status**: `[NOT REACHED]` D3D11, DXGI, Vulkan instance/device, and Present milestones are not yet reached.
- **External DXVK Subsystem**: `[IMPLEMENTED]` Automatically mapped from Bottles path `/home/fizzy/.var/app/com.usebottles.bottles/data/bottles/dxvk/dxvk-2.7.1/x64/`.
- **Mock COM Patch Note**: `[IMPLEMENTED]` `[NOT RUNTIME VERIFIED]` The mock COM patch is an isolated safety patch and **not** the root cause of the current crash.
- **Primary Unconfirmed Question**: Is `RDI` corrupted during a guest->host SysV call boundary prior to `mov rcx, rdi` at `0x140043972`?

---

============================================================
3. ТЕКУЩИЙ ПОДТВЕРЖДЁННЫЙ CRASH
============================================================

- **Crash Location**:
  - `RIP`: `0x0000000142585F16`
  - `Module`: `main_exe` (Base: `0x0000000140000000`)
  - `RVA`: `0x02585F16`
  - `Instruction`: `mov rax, [rsi]` (`48 8B 06`)
  - `RSI`: `0x303078300A646574` (Little-endian ASCII `"ted\n0x00"`)
- **Caller Location**:
  - `[RSP + 0x38]`: `0x000000014004397A` (`main_exe` RVA `0x0004397A`) in `Main::setup`.
- **Disassembly around Crash (`0x142585F00`)**:
  ```assembly
  142585f07: mov rsi, QWORD PTR [rcx+0x48]   ; Read reference pointer from [rcx+0x48]
  142585f0b: mov rbx, rcx                    ; Save Ref<T> container pointer
  142585f0e: mov rdi, rdx                    ; Save incoming object pointer
  142585f11: test rsi, rsi                   ; Check if reference pointer is NULL
  142585f14: je 0x142585f27                  ; Skip unref if NULL
  142585f16: mov rax, QWORD PTR [rsi]        ; <--- FAULT SITE (reads unmapped 0x303078300A646574)
  142585f19: mov rcx, rsi
  142585f1c: call QWORD PTR [rax+0x10]       ; Call Release / destructor
  142585f27: mov QWORD PTR [rbx+0x48], rdi   ; Store new reference
  ```

### Proven Crash Facts:
- `[PROVEN]` Crash occurs strictly on `mov rax, [rsi]` at `0x142585f16`.
- `[PROVEN]` `call QWORD PTR [rax+0x10]` was **not** executed in the current run.
- `[PROVEN]` `RAX` in the crash dump (`0x143CB3360`) is **not** a proven vtable of current `RSI`, because the memory read `mov rax, [rsi]` failed before completing.

---

============================================================
4. НЕ СМЕШИВАТЬ RBP/RDX И RDI/RCX
============================================================

### Disassembly in `Main::setup` (`0x14004393E` - `0x140043975`):
```assembly
14004393e: mov ecx, 0x18                ; Request 24-byte allocation (0x18)
140043943: call 0x142e0a2b0            ; Memory::alloc_static(24) -> RAX
14004395c: mov rbp, rax                ; rbp = new 24-byte OS_Win32 instance
14004395f: call 0x14002c990            ; Object base constructor
140043967: mov rcx, rbp                ; rcx = rbp
14004396a: call 0x14264fef0            ; OS_Win32 constructor (assigns VTable 0x143cb3360)
14004396f: mov rdx, rbp                ; rdx = rbp (new OS_Win32 instance passed as arg 2)
140043972: mov rcx, rdi                ; rcx = rdi (target container pointer passed as arg 1)
140043975: call 0x142585f00            ; Ref<T>::operator=(OS_Win32 *p_os)
```

### Strict Separation:
- **`RBP / RDX`**: Represents the newly instantiated `OS_Win32` object. Allocated via `Memory::alloc_static(0x18)` (24 bytes). Passed as **2nd argument** (`RDX`) into `Ref<T>::operator=`.
- **`RDI / RCX`**: Represents an independent target/container pointer (`Ref<OS_Win32>*`). Passed as **1st argument** (`RCX` / `this`) into `Ref<T>::operator=`. Field `[RCX + 0x48]` is read inside `Ref<T>::operator=`. Its origin is currently `[UNKNOWN]`.

### Register State Table Before `0x142585F00`:

| Register | Runtime Value | Origin | Expected Semantic Role | Mapping | Allocation Size | Confidence |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`RCX`** | `0x5B01A49E2D70` | `mov rcx, rdi` (`0x140043972`) | `Ref<OS_Win32>*` | `[heap]` | `[UNKNOWN]` | `[HYPOTHESIS]` |
| **`RDX`** | `0x5B01A67466F0` | `mov rdx, rbp` (`0x14004396F`) | `OS_Win32*` | `[heap]` | 24 bytes (`0x18`) | `[PROVEN]` |
| **`RDI`** | `0x5B01A49E2D70` | `Main::setup` prologue | `Ref<OS_Win32>*` | `[heap]` | `[UNKNOWN]` | `[HYPOTHESIS]` |
| **`RBP`** | `0x5B01A67466F0` | `mov rbp, rax` (`0x14004395C`) | `OS_Win32*` | `[heap]` | 24 bytes (`0x18`) | `[PROVEN]` |

---

============================================================
5. DATAFLOW RDI
============================================================

### Function Call Sequence in `Main::setup` before `0x140043972`:
1. `0x14004375D`: `call QWORD PTR [rip+0x4360005]` (# `0x1443a3768` IAT) -> `GetStdHandle` / `GetConsoleMode`.
2. `0x1400437D4`: `call 0x140037470` -> Internal Godot OS initialization.
3. `0x140043922`: `call 0x142e0a2b0` -> `Memory::alloc_static(8)` (Logger allocation).
4. `0x140043939`: `call 0x142e0cf00` -> `OS::set_logger`.
5. `0x140043943`: `call 0x142e0a2b0` -> `Memory::alloc_static(24)` (`OS_Win32` allocation).
6. `0x14004395F`: `call 0x14002c990` -> `Object` base constructor.
7. `0x14004396A`: `call 0x14264fef0` -> `OS_Win32` constructor.

### RDI Dataflow Audit Table:

| Caller RVA | Call Target | DLL!Symbol | Resolved Address | Implementation File | ABI | RDI Before | RDI After | Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `0x0004375D` | `[IAT+0x768]` | `kernel32!GetStdHandle` | `0x009F8120` | `src/dlls/kernel32.rs` | `win64` | `[UNKNOWN]` | `[UNKNOWN]` | Code inspection |
| `0x00043903` | `[IAT+0x890]` | `kernel32!GetConsoleMode` | `0x009F8140` | `src/dlls/kernel32.rs` | `win64` | `[UNKNOWN]` | `[UNKNOWN]` | Code inspection |
| `0x00043922` | `0x142e0a2b0` | `msvcrt!malloc` | `0x009F6410` | `src/dlls/crt.rs` | `win64` | `[UNKNOWN]` | `[UNKNOWN]` | Code inspection |
| `0x00043943` | `0x142e0a2b0` | `msvcrt!malloc` | `0x009F6410` | `src/dlls/crt.rs` | `win64` | `[UNKNOWN]` | `[UNKNOWN]` | Code inspection |

### Primary Working Hypothesis:
`[HYPOTHESIS]`
Godot stores the target container pointer in `RDI`, then executes a guest->host call. If any exported function or dynamic stub uses standard Linux SysV ABI or missing `extern "win64"`, `RDI` (which is a scratch register in SysV ABI) gets clobbered with a host heap address containing log buffer text. Subsequently, `mov rcx, rdi` transfers the corrupted `RDI` value into `RCX`.

---

============================================================
6. WIN64 TRAMPOLINE
============================================================

### Trampoline Code ([`src/sys/trampoline.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/sys/trampoline.rs)):
```assembly
mov [{target_slot}], rax
mov rbp, rsp
and rsp, -16
push r10          ; Push exit_stub_ptr as return address
sub rsp, 0x20     ; Reserve 32-byte Win64 shadow space
jmp rax
```

### Guest Entry Stack Layout:
- `[RSP + 0x00]`: Shadow Slot #1 (RCX home)
- `[RSP + 0x08]`: Shadow Slot #2 (RDX home)
- `[RSP + 0x10]`: Shadow Slot #3 (R8 home)
- `[RSP + 0x18]`: Shadow Slot #4 (R9 home)
- `[RSP + 0x20]`: Return address (`rine_exit_thread_stub`)
- `[RSP + 0x28]`: Align padding / Host frame
- **`RSP mod 16`**: Inside PE Entry Point after `sub rsp, 0x28` prologue, `RSP % 16 == 0`.

### Execution Integration Test Status:
`[NOT TESTED]` There is currently no executable integration test where a guest PE `extern "win64"` function executes `ret`, returns into `rine_exit_thread_stub`, and restores host `RSP` and non-volatile registers (`RBX`, `RBP`, `RSI`, `RDI`, `R12`-`r15`).

---

============================================================
7. ABI-АУДИТ GUEST-CALLABLE ФУНКЦИЙ
============================================================

| Symbol | DLL | File | Rust Function | ABI | Return Type | Guest-Callable? | Non-Volatile Risk | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `msvcrt_malloc` | `msvcrt.dll` | `src/dlls/crt.rs` | `msvcrt_malloc` | `extern "win64"` | `*mut c_void` | Yes | Low | `[IMPLEMENTED]` |
| `msvcrt_strlen` | `msvcrt.dll` | `src/dlls/crt.rs` | `msvcrt_strlen` | `extern "win64"` | `usize` | Yes | Low | `[IMPLEMENTED]` |
| `_initterm` | `msvcrt.dll` | `src/dlls/crt.rs` | `_initterm` | `extern "win64"` | `i32` | Yes | Low | `[IMPLEMENTED]` |
| `_initterm_e` | `msvcrt.dll` | `src/dlls/crt.rs` | `_initterm_e` | `extern "win64"` | `i32` | Yes | Low | `[IMPLEMENTED]` |
| `GetStdHandle` | `kernel32.dll` | `src/dlls/kernel32.rs` | `GetStdHandle` | `extern "win64"` | `u64` | Yes | Low | `[IMPLEMENTED]` |
| `log_unresolved_stub_call` | `Dynamic` | `src/dlls/registry.rs` | `log_unresolved_stub_call` | `extern "C"` | `usize` | Yes | `[CRITICAL ABI RISK]` | `[HIGH RISK]` |
| `BCryptGenRandom` | `bcrypt.dll` | `src/dlls/advapi32.rs` | `BCryptGenRandom` | `extern "win64"` | `i32` | Yes | Low | `[IMPLEMENTED]` |
| `AdjustTokenPrivileges` | `advapi32.dll` | `src/dlls/advapi32.rs` | `AdjustTokenPrivileges` | `extern "win64"` | `u32` | Yes | Low | `[IMPLEMENTED]` |

---

============================================================
8. MOCK COM PATCH СИЛЬНОЙ МОДЕЛИ
============================================================

### Safety Patch Summary ([`src/dlls/object_registry.rs`](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/object_registry.rs)):
- Generic `S_OK` thunks replaced with explicit interface checks.
- Unsupported interface methods return `E_NOTIMPL` (`0x80004001`).
- `QueryInterface` nullifies out-pointer (`*ppv = NULL`) on failure and returns `E_NOINTERFACE` (`0x80004002`).

### Strict Status:
- `[IMPLEMENTED]`
- `[NOT COMPILED IN CURRENT ENVIRONMENT]`
- `[NOT RUNTIME VERIFIED]`
- `[NOT PROVEN ROOT CAUSE OF CURRENT CRASH]`

### Known Risks:
1. Single generic `E_NOTIMPL` stub does not match all COM method parameter signatures.
2. `QueryInterface` handling only `IID_IUnknown` is incomplete for complex COM hierarchies.
3. COM VTable slots have mixed return types (`HRESULT`, `ULONG`, `void`, `BOOL`).

---

============================================================
9. ВНЕШНИЙ DXVK
============================================================

### Loader Architecture:
- Path search: `src/dlls/d3d11.rs` (lines 15–30) and `src/dlls/dxgi.rs` (lines 20–40) look for `/home/fizzy/.var/app/com.usebottles.bottles/data/bottles/dxvk/dxvk-2.7.1/x64/`.
- Fallback: Optional fallback to mock internal D3D11 device if DXVK `.dll` files are missing.
- Priority: External DXVK has **higher priority** over internal Rine mocks.

### Runtime Evidence (`scratch/game_after_field48_proven_fix.log`):
- `[DXVK LOADER] dxgi.dll loaded @ 0x00000002EDF00000`
- `[DXVK LOADER] d3d11.dll loaded @ 0x0000000359050000`

### Graphics Milestones Table:

| Graphics Milestone | Status |
| :--- | :--- |
| DXVK `dxgi.dll` mapped | `[RESOLVED]` |
| DXVK `d3d11.dll` mapped | `[RESOLVED]` |
| `D3D11CreateDevice` resolved | `[RESOLVED]` |
| `D3D11CreateDeviceAndSwapChain` resolved | `[RESOLVED]` |
| `CreateDXGIFactory` resolved | `[RESOLVED]` |
| `D3D11CreateDevice` called | `[NOT REACHED]` |
| `vkGetInstanceProcAddr` called | `[NOT REACHED]` |
| `vkCreateInstance` called | `[NOT REACHED]` |
| `vkCreateDevice` called | `[NOT REACHED]` |
| Vulkan Surface created | `[NOT REACHED]` |
| `vkQueuePresentKHR` called | `[NOT REACHED]` |
| Visible frame confirmed | `[NOT REACHED]` |

---

============================================================
10. DATA DIRECTORY 12 / IAT FALLBACK
============================================================

- **Logic**: Unmapped/null IAT slots in DataDirectory[12] range are bound to dynamic stubs (`RVA_..._Val_0x0`).
- **File & Function**: `src/pe/imports.rs` (`resolve_iat`).
- **Risk**: A zeroed IAT slot might be sentinel padding rather than an imported function pointer.
- **Status**: `[IMPLEMENTED]`, `[HIGH RISK]`, `[NOT PROVEN AS CURRENT CRASH SOURCE]`.

---

============================================================
11. ПРЕДЫДУЩИЕ УТВЕРЖДЕНИЯ
============================================================

| Claim | Status | Evidence | Comment |
| :--- | :--- | :--- | :--- |
| Crash was on `call [rax+0x10]` | `[CONTRADICTED]` | Log `RIP = 0x142585F16` | Crash occurs on `mov rax, [rsi]`. |
| Crash was on `mov rax, [rsi]` | `[PROVEN]` | Opcode `48 8B 06` | Fault occurs during memory read. |
| RAX was vtable of current object | `[REJECTED]` | Log `RAX = 0x143CB3360` | Read failed before completing. |
| Reason was NULL COM slot | `[REJECTED]` | Log disassembly | Failure occurs inside `Ref<T>::operator=`. |
| RCX was OS_Win32 allocation 0x18 | `[HYPOTHESIS]` | `Main::setup` disassembly | `RCX` is `rdi` (`Ref<T>*`), `RDX` is `rbp` (24 bytes). |
| RDX was OS_Win32 allocation 0x18 | `[PROVEN]` | Disassembly `0x140043943` | `Memory::alloc_static(24)` -> `rbp` -> `RDX`. |
| RDI/RCX was separate target | `[PROVEN]` | Disassembly `0x140043972` | `mov rcx, rdi` sets 1st argument. |
| Object was stack local | `[CONTRADICTED]` | `/proc/<pid>/maps` | Address `0x5555573c4d70` is inside `[heap]`. |
| Object was heap allocated | `[PROVEN]` | `/proc/<pid>/maps` | Address `0x5555573c4d70` is `rw-p [heap]`. |
| `eprintln` wrote log text into object | `[PROVEN]` | ASCII dump | Memory contained `"ted\n0x00"`. |
| `malloc` should be `calloc` | `[REJECTED]` | C standard | Malloc must not zero memory. |
| Stack wiping fixed crash | `[REJECTED]` | Log trace | Crash still occurs at `0x142585F16`. |
| Universal return 0 is valid | `[REJECTED]` | API contracts | Returns fake success without populating out params. |
| Blanket `S_OK` is valid | `[REJECTED]` | COM spec | Returns fake success for unhandled methods. |
| Old crash is truly passed | `[REJECTED]` | Log trace | Execution crashes at `0x142585F16`. |
| Vulkan was reached | `[NOT REACHED]` | Log trace | Vulkan proc resolver not invoked. |

---

============================================================
12. FIRST ACTION
============================================================

**FIRST ACTION**:
Test the actual Win64 entry/return trampoline layout with a real executable integration test, then trace the preservation of `RDI` from its last valid assignment down to `mov rcx, rdi` at `0x140043972` in `Main::setup`.

**WHY**:
The current crash reads an invalid target pointer via `RDI / RCX`. `RBP / RDX` belongs to a separate 24-byte object. A violation of the Win64 non-volatile register contract or stack layout explains how `RDI` became corrupted with a host heap log buffer address.

**FILES**:
- `src/sys/trampoline.rs`
- `src/dlls/registry.rs`
- `src/pe/imports.rs`
- `src/dlls/crt.rs`
- `scratch/game_after_field48_proven_fix.log`

**BREAKPOINTS**:
- `0x140043740` (`Main::setup` entry)
- `0x14004375D` (`GetStdHandle` / `GetConsoleMode` call)
- `0x140043922` (`Memory::alloc_static(8)`)
- `0x140043943` (`Memory::alloc_static(24)`)
- `0x14004396F` (`mov rdx, rbp`)
- `0x140043972` (`mov rcx, rdi`)
- `0x140043975` (`call 0x142585f00`)
- `0x142585F16` (Fault site)

**EXPECTED EVIDENCE**:
- `RDI` value before and after each `call` in `Main::setup`.
- Identification of the first `call` that clobbers `RDI`.
- Exact resolved function target and ABI of the clobbering call.
- Executable proof of guest `ret` returning into sentinel and restoring host state.

**DO NOT**:
- Do NOT modify COM code.
- Do NOT zero `RDI` / `RSI` as a workaround.
- Do NOT wipe stack memory.
- Do NOT replace `malloc` with `calloc`.
- Do NOT return universal `S_OK` or `0`.
- Do NOT silently recover from signals.
- Do NOT declare the old crash fixed without a new runtime run.

**SUCCESS CRITERION**:
- Trampoline layout bug is proven or ruled out.
- `RDI` corruption cause is proven.
- First clobbering call is identified.
- Old RIP `0x142585F16` is passed after architectural fix.
- Next exact blocker is reached.

---

============================================================
13. LOG EXCERPT SUMMARY
============================================================
Curated log excerpt saved to [RINE_SMART_MODEL_LOG_EXCERPT.txt](file:///home/fizzy/Рабочий%20стол/Rine/RINE_SMART_MODEL_LOG_EXCERPT.txt).

---

============================================================
14. FILE INDEX SUMMARY
============================================================
File index (17 files) saved to [RINE_SMART_MODEL_FILE_INDEX.txt](file:///home/fizzy/Рабочий%20стол/Rine/RINE_SMART_MODEL_FILE_INDEX.txt).
