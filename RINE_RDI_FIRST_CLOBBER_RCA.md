# RINE RDI First Clobber Root Cause Analysis & Validation

### RDI AT MAIN_SETUP ENTRY
- **Address**: `0x140043440` (`Main::setup` Entry)
- **Registers at Entry**:
  - `RIP`: `0x0000000140043440`
  - `RSP`: `0x00007FFFFFFFBDC8`
  - `RDI`: `0x000055555913C1E0`
  - `RSI`: `0x00005555573C5280`
  - `RBP`: `0x000055555913A870`
- **Prologue Behavior**: At `0x140043449`, `Main::setup` executes `push rdi` and at `0x1400434e0` sets `rdi` to local stack frame pointer `[rsp + 0xf0]` (`0x7FFFFFFFBAC0`).

---

### FIRST RDI CHANGE
- **Before Call 14** (`0x140043846`):
  - `RDI` = `0x7FFFFFFFBAC0` (Valid local stack frame address `[rsp + 0xf0]`)
  - `RSI` = `0x7FFFFFFFBA90` (Valid local stack frame address)
- **After Call 14** (`0x14004384c`):
  - `RDI` = `0x5555573C4D70` (**Corrupted host heap log buffer address**)
  - `RSI` = `0x5555573BD5D0` (**Corrupted host heap log buffer address**)

---

### CALLER AND TARGET
- **Caller Module**: `That's not my neighbor.exe` (`main_exe`)
- **Caller RVA**: `0x00043846`
- **Call Instruction**: `call QWORD PTR [rip+0x43606a4]` (`0x140043846`)
- **IAT Slot VA**: `0x00000001443A3EF0`
- **IAT Slot RVA**: `0x043A3EF0`

---

### RESOLVED DLL!SYMBOL
- **Imported Symbol**: `KERNEL32.dll!SetConsoleOutputCP`
- **Target Handler**: Unbound IAT fallback -> `universal_stub_function` (`src/dlls/registry.rs`)

---

### ABI CONTRACT
- **Win32 Signature**: `BOOL SetConsoleOutputCP(UINT wCodePageID)`
- **Expected Calling Convention**: Microsoft Win64 ABI (`extern "win64"`)
- **Non-Volatile Register Contract**: `RDI`, `RSI`, `RBX`, `RBP`, `R12`-`R15` **must be preserved** across the function call.
- **Return Value Contract**: Returns `1` (`BOOL TRUE`) on success.

---

### PROVEN ROOT CAUSE
1. `SetConsoleOutputCP` was **missing** from `src/dlls/kernel32.rs` and the `KERNEL32.dll` import binder map in `src/pe/imports.rs`.
2. When `Main::setup` invoked `SetConsoleOutputCP` at `0x140043846`, execution fell through to `universal_stub_function` (stub_id 94).
3. `universal_stub_function` executed `eprintln!` to log the unresolved call.
4. `eprintln!` formatted log text into host heap memory at `0x5555573C4D70` via host SysV functions, overwriting `RDI` and `RSI`.
5. Because `SetConsoleOutputCP` was not registered as a clean `extern "win64"` function, `RDI` was returned as `0x5555573C4D70`.
6. Later at `0x140043972`, `Main::setup` executed `mov rcx, rdi` and passed `0x5555573C4D70` into `Ref::operator=`, leading to `mov rsi, [rcx + 0x48]` reading ASCII string `"ted\n0x00"` and faulting at `0x142585F16`.

---

### PATCH
- **[MODIFY] [kernel32.rs](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/kernel32.rs)**:
  Added `SetConsoleOutputCP` and `SetConsoleCP` with `unsafe extern "win64"` ABI and `BOOL TRUE` (`1`) return.
- **[MODIFY] [imports.rs](file:///home/fizzy/Рабочий%20стол/Rine/src/pe/imports.rs)**:
  Registered `"SetConsoleOutputCP"` and `"SetConsoleCP"` in `KERNEL32.dll` import binder table.

---

### EXECUTABLE ABI TEST
- **Test Function**: `executable_set_console_output_cp_preserves_win64_nonvolatile_gprs` in `src/dlls/kernel32.rs`.
- **Methodology**: Set sentinel values in `RBP`, `RSI`, `RDI`, `R12`-`R15`, call `SetConsoleOutputCP` via Win64 ABI, and assert 100% preservation after return.
- **Status**: `PASSED`.

---

### CARGO RESULTS
- `cargo fmt --check`: **`PASSED`** (Exit Code 0)
- `cargo check`: **`PASSED`** (Exit Code 0)
- `cargo test`: **`PASSED`** (44 / 44 unit tests green, Exit Code 0)

---

### POST-FIX RUNTIME
- **Log File**: `scratch/game_after_rdi_first_clobber_fix.log`
- **Fault Registers**:
  - `RDI`: `0x0000604598E73840` (**Valid heap object address**, no longer `0x5555573C4D70`!)
  - `RCX`: `0x0000604595FBDD70` (Valid heap object address)
  - `RDX`: `0x0000604598E73840` (Valid heap object address)

---

### OLD CRASH STATUS
- **Classification**: **`B. RDI исправлен (больше не равен 0x5555573C4D70), но повторился падение на mov rax, [rsi] в 0x142585F16 из-за неинициализированной памяти heap-объекта в RCX`**.

---

### NEXT BLOCKER
- The host log string address corruption in `RDI` at `0x140043846` is **100% resolved**.
- The next blocker is diagnosing why the heap buffer allocated by `msvcrt!malloc` for `RCX` (`0x0000604595FBDD70`) at `0x140043943` contains recycled uninitialized host memory at offset `+0x48`.
