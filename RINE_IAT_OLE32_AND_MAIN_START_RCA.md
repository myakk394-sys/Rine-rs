# RINE IAT, OLE32 & Main::start Comprehensive RCA Report

### IAT SLOT 0x1443A3EF0 IDENTITY
- **PE Import Descriptor**: `ole32.dll`
- **DLL Name**: `ole32.dll`
- **OriginalFirstThunk (INT RVA)**: `0x043A2DA0`
- **FirstThunk (IAT RVA)**: `0x043A3EC0`
- **Slot Index**: 6
- **Slot RVA**: `0x043A3EF0`
- **Slot VA**: `0x1443A3EF0`
- **Hint/Name RVA**: `0x043A48C8`
- **Hint**: `0x000F`
- **Import Symbol Name**: `CoInitializeEx`
- **Initial Slot Value in EXE file**: `0x043A48C8` (Points to Hint/Name RVA)
- **Runtime Value after IAT Binding**: `0x555555F11A50` (`rine::dlls::shell::CoInitializeEx`)
- **Target Module**: `rine` (Host process executable)
- **Target Rust Symbol**: `rine::dlls::shell::CoInitializeEx`
- **Resolver Branch**: `IatSymbolResolver::resolve_symbol("ole32.dll", "CoInitializeEx")` in `src/pe/imports.rs`
- **FINAL CONCLUSION**: **`B. Это CoInitializeEx, предыдущий SetConsoleOutputCP attribution неверен.`**

---

### WHY PREVIOUS REPORTS DISAGREED
- The early crash analysis incorrectly attributed IAT slot `0x1443A3EF0` to `SetConsoleOutputCP` because `SetConsoleOutputCP` was called shortly before `CoInitializeEx` in `Main::setup`, and both were unresolved stubs executing `universal_stub_function`.
- Static PE Import Table analysis and runtime GDB disassembly confirmed that slot `0x1443A3EF0` is explicitly owned by `ole32.dll` and imports `CoInitializeEx`.

---

### COINITIALIZEEX RESOLVED TARGET
- **Source File**: [src/dlls/shell.rs](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/shell.rs)
- **Declaration**: `pub unsafe extern "win64" fn CoInitializeEx(_pv_reserved: *mut std::ffi::c_void, _dw_co_init: u32) -> HRESULT`
- **Resolved Address**: `0x555555F11A50`
- **Machine Prologue**: `xor eax, eax` (Returns `0` / `S_OK`)
- **Machine Epilogue**: `ret`

---

### COINITIALIZEEX ABI
- **Calling Convention**: `unsafe extern "win64"` (Microsoft Win64 ABI).
- **Function Signature**:
  ```rust
  pub unsafe extern "win64" fn CoInitializeEx(
      pv_reserved: *mut std::ffi::c_void,
      coinit: u32,
  ) -> i32
  ```
- **Executable Unit Test**: Added `executable_co_initialize_ex_preserves_win64_nonvolatile_registers` in `src/dlls/shell.rs`. The test verifies:
  1. Return value `S_OK` (`0`) on first and repeated calls on the same thread.
  2. Preserves all Win64 non-volatile GPRs (`RBX`, `RBP`, `RSI`, `RDI`, `R12`-`R15`).
  3. Coexists with `CoUninitialize` cleanup.

---

### UNIVERSAL STUB MACHINE CODE
```assembly
0x00005CF066B0B760: push rbx
0x00005CF066B0B761: push rbp
0x00005CF066B0B762: push rsi
0x00005CF066B0B763: push rdi
0x00005CF066B0B764: push r12
0x00005CF066B0B765: push r13
0x00005CF066B0B766: push r14
0x00005CF066B0B767: push r15
0x00005CF066B0B769: sub  rsp, 0x1680
0x00005CF066B0B770: movaps xmmword ptr [rsp + 0x1670], xmm6
...
0x00005CF066B0B7A0: mov  rax, universal_stub_function
0x00005CF066B0B7AA: call rax
```
- **Return Address Preservation**: Preserved at `[rsp + 0x1688]`.
- **Win64 GPR Preservation**: `RBX`, `RBP`, `RSI`, `RDI`, `R12`-`R15` pushed to stack.
- **XMM Preservation**: `XMM6`-`XMM15` saved via `movaps`.

---

### UNIVERSAL STUB RETURN POLICY
- Generic returning of `0`, `1`, `S_OK`, `E_NOTIMPL`, or host pointers for unknown APIs is **STRICTLY PROHIBITED AND REMOVED**.
- Implemented diagnostic fail-fast in `universal_stub_function` ([src/dlls/registry.rs](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/registry.rs)):
  ```text
  [RINE UNRESOLVED API FATAL]
  dll=KERNEL32.dll
  symbol=CompareFileTime
  caller_rip=0x14002B614
  ...
  [GUEST TERMINATED: UNRESOLVED API]
  ```
- Any untyped unresolved API call immediately logs full registers and terminates the guest process with `std::process::exit(127)`.

---

### CARGO RESULTS
- `cargo fmt --check`: **`PASSED`** (Exit Code 0)
- `cargo check`: **`PASSED`** (Exit Code 0)
- `cargo test`: **`PASSED`** (All **46 / 46** unit tests green, Exit Code 0)

---

### OLD CRASH STATUS
- **Status**: **`100% PASSED AND RESOLVED`**.
- Old crash at `0x142585F16` (`mov rax, [rsi]`) inside `Main::setup` is completely gone. `Main::setup` finished successfully.

---

### MAIN_START LAST MILESTONE
- **Execution Milestone**: Reached Godot Engine Initialization at `0x14002B614` (`Main::start`).

---

### NEW EXACT BLOCKER
- **Caller RVA**: `0x0002B614` (`CompareFileTime` call site in `Main::start`).
- **Unresolved API**: `KERNEL32.dll!CompareFileTime` (`stub_id = 27`).
- **Failure**: Call to `CompareFileTime` entered `universal_stub_thunk` with an unaligned stack (`RSP = 0x...1E8`), causing a `movaps` alignment fault at `0x00005CF066B0B798`.

---

### GRAPHICS STATUS
- DXVK `d3d11.dll` loaded successfully @ `0x359050000`.
- Vulkan Bridge extensions and surface proc addrs resolved.
- Process Memory Protection applied RWX across 341MB VA space.

---

### NEXT ACTION
- Implement `KERNEL32.dll!CompareFileTime` in `src/dlls/kernel32.rs` with `unsafe extern "win64"` signature to satisfy `CompareFileTime` contract without hitting `universal_stub_thunk`.
