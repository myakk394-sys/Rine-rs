# RINE Fast Trampoline & Runtime Validation Report

### SMART PATCH REVIEW
- **Stack Wiping**: Deleted temporary 128KB stack zeroing (`std::ptr::write_bytes`) from `jump_to_win64_entry_point`.
- **Guest Register Zeroing**: Removed artificial register zeroing (`xor rbx, rbx`, `xor rsi, rsi`, `xor rdi, rdi`, `xor r12..r15`).
- **Trampoline Sequence**:
  ```assembly
  mov [{target_slot}], rax
  mov rbp, rsp
  and rsp, -16
  sub rsp, 0x20
  push r10          ; Push exit stub return sentinel
  jmp rax           ; Jump to PE entry point
  ```
- **Executable Test**: Preserved real assembly execution test `executable_guest_ret_reaches_sentinel_with_win64_layout`.

---

### COMPILE ERRORS FOUND
1. `Rust 2024 Naked Attribute`: `#[naked]` requires `#[unsafe(naked)]` syntax in Rust 2024 edition.
2. `Type Mismatch in Test Indexing`: `index * 8` (`usize`) addition to `rsp` (`u64`) required `(index * 8) as u64` cast.
3. `Win64 Volatile Register Clobber`: In the original test, `r11` was overwritten inside `ret_guest` epilogue before `ret_sentinel` executed `mov rax, qword ptr [r11]`. Fixed by using static observation pointers.

---

### FIXES TO SMART PATCH
- Applied `#[unsafe(naked)]` and `(index * 8) as u64` to `src/sys/trampoline.rs`.
- Fixed test stack frame preservation by saving/restoring host registers (`push rbx, rbp, r12..r15`) and passing `observation` pointer via Win64 ABI register `RCX`.
- Preserved 100% of the architectural implementation of the trampoline assembly and executable test without replacing it with address arithmetic mocks.

---

### EXECUTABLE TEST RESULT
- **Command**: `cargo test executable_guest_ret_reaches_sentinel_with_win64_layout -- --nocapture`
- **Status**: `PASSED` (Exit Code 0).
- **Verified Assertions**:
  - `observation.sentinel_hit == 1` (guest `ret` successfully returned to `ret_sentinel`).
  - `rsp_after == rsp_before` (host stack pointer restored).
  - `observation.guest_rsp % 16 == 8` (Win64 callee entry stack alignment verified).
  - `observation.shadow_words == [0, 0, 0, 0]`.
  - Non-volatile registers (`RBX`, `RBP`, `RSI`, `RDI`, `R12`-`R15`) preserved 100%.

---

### MACHINE-CODE LAYOUT
Disassembly of `jump_to_win64_entry_point` in `target/debug/rine` via GDB (`RINE_FAST_TRAMPOLINE_DISASM.txt`):
```assembly
0x00000000009d4ab1 <+2753>: 48 89 e5          mov    rbp,rsp
0x00000000009d4ab4 <+2756>: 48 83 e4 f0       and    rsp,0xfffffffffffffff0
0x00000000009d4ab8 <+2760>: 48 83 ec 20       sub    rsp,0x20
0x00000000009d4abc <+2764>: 41 52             push   r10
0x00000000009d4abe <+2766>: ff e0             jmp    rax
```
- **Sentinel Location**: `[RSP + 0x00]` contains `r10` (`rine_exit_thread_stub`).
- **Shadow Space**: `[RSP + 0x08 .. RSP + 0x27]` (32-byte home area for RCX, RDX, R8, R9).
- **Alignment**: Inside PE Entry Point after `sub rsp, 0x28` prologue, `RSP % 16 == 0`.
- **Return Behavior**: An ordinary `ret` instruction in the guest PE pops `[RSP + 0x00]`, receiving `exit_stub_ptr`.

---

### CARGO RESULTS
- **`cargo fmt --check`**: `PASSED` (Exit Code 0).
- **`cargo check`**: `PASSED` (Exit Code 0).
- **`cargo test`**: `PASSED` (35 / 35 unit tests green, Exit Code 0).

---

### RUNTIME COMMAND
```bash
mkdir -p scratch
timeout 90s cargo run --bin rine -- \
  "/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/That's not my neighbor.exe" \
  2>&1 | tee scratch/game_after_trampoline_rdi_fix.log
```

---

### RUNTIME BUILD MARKER
- Marker: `[RINE BUILD MARKER] trampoline_layout_v3`
- Confirmation line in log:
  ```text
  [RINE BUILD MARKER] trampoline_layout_v3
  [RINE TRAMPOLINE PRE-JUMP CONFIRMATION 0x48_TRACE_v2] Register RAX pre-bound to Entry Point VA = 0x00000001400013D0, RSP = 0x00007FFF8F8F5970, TEB = 0x0000752504826000, ReturnStub = 0x000061C524AF5E60
  ```

---

### OLD CRASH STATUS
- **Classification**: **`A. Старый crash повторился: RIP = 0x142585F16`**.
- **Instruction**: `mov rax, [rsi]` (`48 8B 06`).
- **Fault Address**: `0x0000000000000000` (Accessing address `0x303078300A646574`).

---

### RDI/RCX/RBP/RDX SNAPSHOT
GDB hardware breakpoint trace log (`scratch/gdb_rdi_main_setup_trace.log`):

#### At `0x14004396F` (`mov rdx, rbp` inside `Main::setup`):
- `RIP`: `0x000000014004396F`
- `RDI`: `0x00005555573C4D70` (Already corrupted host heap address containing log text `"[KERNEL32] TlsAlloc -> TLS Index 11\n..."`)
- `RBP`: `0x0000555557E8B280` (Valid 24-byte `OS_Win32` allocation)
- `Memory at RBP`: `0x0000000143CB3360` (Valid `OS_Win32` VTable)
- `RDX`: `0x00005555573BD5D8`

#### At `0x140043972` (`mov rcx, rdi`):
- `RDI`: `0x00005555573C4D70` (Corrupted)
- `RDX`: `0x0000555557E8B280` (`OS_Win32` object)

#### At `0x142585F16` (Fault site inside `Ref<T>::operator=`):
- `RCX`: `0x00005555573C4D70` (Passed corrupted `RDI`)
- `RDX`: `0x0000555557E8B280` (New `OS_Win32` instance)
- `[RCX + 0x48]`: `0x303078300A646574` (`"ted\n0x00"`)
- `RSI`: `0x303078300A646574`
- `Fault`: `mov rax, [rsi]` crashes because `RSI` is an invalid pointer.

---

### NEW CRASH OR MILESTONE
- **Status**: The crash at `0x142585F16` remains identical.
- **Empirical Evidence**: `RDI` was already holding `0x00005555573C4D70` prior to `0x14004396F`.
- **Implication**: `RDI` was assigned or clobbered earlier in `Main::setup` (between `0x140043740` and `0x14004396F`).

---

### FILES CREATED
1. `RINE_SMART_TRAMPOLINE_PATCH.diff` (9,169 bytes)
2. `RINE_FAST_FMT.log` (0 bytes)
3. `RINE_FAST_CHECK.log` (1,061 bytes)
4. `RINE_FAST_TEST.log` (2,752 bytes)
5. `RINE_FAST_TRAMPOLINE_TEST.log` (1,061 bytes)
6. `RINE_FAST_TRAMPOLINE_DISASM.txt` (111,842 bytes)
7. `scratch/trace_rdi_main_setup.gdb` (2,185 bytes)
8. `scratch/gdb_rdi_main_setup_trace.log` (111,737 bytes)
9. `scratch/game_after_trampoline_rdi_fix.log` (137,284 bytes)
10. `scratch/game_after_trampoline_rdi_fix_excerpt.log` (15,482 bytes)
11. `RINE_FAST_TRAMPOLINE_VALIDATED.diff` (9,531 bytes)
12. `RINE_FAST_TRAMPOLINE_VALIDATION.md` (6,412 bytes)

---

### REMAINING UNKNOWN
- Exactly which function call inside `Main::setup` prior to `0x14004396F` first modifies or clobbers `RDI`.
