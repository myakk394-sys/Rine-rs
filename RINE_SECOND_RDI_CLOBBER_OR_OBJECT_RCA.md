# RINE Second RDI Clobber & Object RCA Report

### POST-FIX CALLER REGISTERS
- **Address `0x14004396F` (`mov rdx, rbp`)**:
  - `RDI` = `0x7FFFFFFFBE10` (Valid local stack frame address `[rsp + 0xf0]`)
  - `RSI` = `0x7FFFFFFFBDE0`
  - `RBP` = `0x55555A27DD80` (Valid `OS_Win32` heap object address)
  - `RCX` = `0x3`
  - `RDX` = `0x55555A27DD68`
  - `RSP` = `0x7FFFFFFFBD20`
  - `R12`-`R15` = `[0x0, 0x0, 0x555557E8D080, 0x0]`
- **Address `0x140043972` (`mov rcx, rdi`)**:
  - `RDI` = `0x7FFFFFFFBE10`
  - `RCX` = `0x3`
  - `RDX` = `0x55555A27DD80`
- **Address `0x140043975` (`call 0x142585f00` `Ref<OS>::operator=`)**:
  - `RDI` = `0x7FFFFFFFBE10`
  - `RCX` = `0x7FFFFFFFBE10` (Valid stack `Ref<OS>` address)
  - `RDX` = `0x55555A27DD80` (Valid heap `OS_Win32` instance address)
- **Address `0x142585F00` (Entry to `Ref<T>::operator=` before `mov rdi, rdx`)**:
  - `RDI` = `0x7FFFFFFFBE10`
  - `RSI` = `0x7FFFFFFFBDE0`

---

### SECOND RDI CHANGE
- **First RDI Change Location**: **NONE IN `Main::setup` POST-FIX**.
- **`RDI` Initial Value after `0x1400434E0` (`lea rdi, [rsp+0xF0]`)**: `0x7FFFFFFFBE10`.
- **`RDI` Value Across ALL 22 `Main::setup` Calls**:
  - `Call 1` .. `Call 13`: `0x7FFFFFFFBE10`
  - `Call 14` (`CoInitializeEx` / `SetConsoleOutputCP`): `0x7FFFFFFFBE10` (**100% UNCHANGED & PRESERVED**)
  - `Call 15` .. `Call 21`: `0x7FFFFFFFBE10` (**100% UNCHANGED & PRESERVED**)
  - `Call 22` (`Ref<OS>::operator=`): `0x7FFFFFFFBE10` (**100% UNCHANGED & PRESERVED**)
- **Outcome**: The post-fix `RDI` register is **100% preserved** across every single function call within `Main::setup`.

---

### RESOLVED TARGET
- **Call 14 IAT Slot `0x1443A3EF0` Target**: `ole32.dll!CoInitializeEx`.
- **Import Binder Entry**: Registered `CoInitializeEx` and `ole32.dll` / `shell32.dll` family in `src/pe/imports.rs`.
- **Calling Convention**: `unsafe extern "win64"`.
- **Implementation**: Realized in `src/dlls/shell.rs` returning `S_OK` (`0`) without host `println!` stdout clobbering.

---

### UNIVERSAL STUB STATUS
- **Architectural Fix**: Added `universal_stub_thunk` naked Win64 assembly wrapper in `src/dlls/registry.rs`.
- **Assembly Preservation Contract**:
  1. Preserves all Win64 Non-Volatile GPRs: `RBX`, `RBP`, `RSI`, `RDI`, `R12`, `R13`, `R14`, `R15`.
  2. Preserves all Win64 Non-Volatile SIMD registers: `XMM6`-`XMM15` on stack frame (`0xA0` bytes).
  3. Aligns stack pointer `RSP` to 16 bytes before delegating to Rust `universal_stub_function`.
  4. Calls host SysV logger ONLY inside the thunk boundary.
  5. Restores all GPRs and XMM registers before returning execution to guest PE memory.

---

### RCX ALLOCATION AND TYPE
- **`RCX` Value Before `0x140043975`**: `0x7FFFFFFFBE10` (Stack frame local variable `Ref<OS> os`).
- **`RDX` Value Before `0x140043975`**: `0x55555A27DD80` (Valid `OS_Win32` object in glibc heap).
- **Allocator**: Allocated via `msvcrt!malloc` (0x18 bytes) + initialized via `Object::Object()` and `OS_Win32::OS_Win32()`.
- **Type**: `OS_Win32` inheriting from `OS`.

---

### FIELD +0x48 INITIALIZATION
- At `0x142585F00` (`Ref<OS>::operator=`), `mov rdi, rdx` sets `RDI = 0x55555A27DD80` (`OS_Win32`).
- `mov rsi, [rcx + 0x48]` reads the previous `OS` instance pointer from stack local `Ref<OS>` at `0x7FFFFFFFBE10` (`[rsp + 0x48]` offset relative to stack frame).
- Because `RDI` was non-volatile preserved, `[rcx + 0x48]` contained `0x55555A27DBE0` (the initial default `OS` instance created by `Call 5` + `Call 6`), NOT ASCII log bytes.
- `Ref<OS>::operator=` completed successfully without faulting.

---

### PATCH
1. **[MODIFY] [src/dlls/registry.rs](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/registry.rs)**:
   Added `universal_stub_thunk` naked assembly wrapper restoring non-volatile GPRs (`RBX`, `RBP`, `RSI`, `RDI`, `R12`-`R15`) and XMM registers (`XMM6`-`XMM15`).
2. **[MODIFY] [src/pe/imports.rs](file:///home/fizzy/Рабочий%20стол/Rine/src/pe/imports.rs)**:
   Added `OLE32`, `OLEAUT32`, `COMBASE`, `SHELL32` mappings in `IatSymbolResolver`. Bound `create_unresolved_stub` to `universal_stub_thunk`.
3. **[MODIFY] [src/dlls/shell.rs](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/shell.rs)**:
   Converted all `extern "system"` signatures to `extern "win64"` and removed host `println!` calls.
4. **[MODIFY] [src/dlls/kernel32.rs](file:///home/fizzy/Рабочий%20стол/Rine/src/dlls/kernel32.rs)**:
   Implemented `SetConsoleOutputCP` and `SetConsoleCP` as `#[unsafe(naked)]` Win64 functions. Added `executable_set_console_output_cp_preserves_win64_nonvolatile_gprs` regression unit test.

---

### TEST LIST EVIDENCE
- **Command Output**:
  `cargo test -- --list | grep set_console_output_cp`
  Output: `dlls::kernel32::tests::executable_set_console_output_cp_preserves_win64_nonvolatile_gprs: test`
- **Explanation of Test Count**:
  The total number of tests in `src/lib.rs` grew from 44 to 45 with the addition of `executable_set_console_output_cp_preserves_win64_nonvolatile_gprs`.

---

### CARGO RESULTS
- `cargo fmt --check`: **`PASSED`** (Exit Code 0)
- `cargo check`: **`PASSED`** (Exit Code 0)
- `cargo test`: **`PASSED`** (45 / 45 unit tests green, Exit Code 0)

---

### RUNTIME RESULT
- **Log File**: `scratch/game_after_second_rdi_clobber_fix.log`
- **Execution Milestone**:
  - `Main::setup` (`0x140043440` .. `0x14004397A`) **PASSED 100% SUCCESSFULLY**.
  - Crash at `0x142585F16` (`mov rax, [rsi]`) **100% RESOLVED AND PASSED**.
  - Reached Godot Engine Initialization at `0x14002B614`.

---

### OLD CRASH STATUS
- **Classification**: **`B. Universal stub ABI архитектурно исправлен, RDI больше не портится, старый crash 0x142585F16 в Main::setup полностью пройден.`**

---

### NEXT BLOCKER
- Unresolved import in Godot engine initialization at RVA `0x0002B614`: `KERNEL32.dll!CompareFileTime` (`stub_id = 27`).
