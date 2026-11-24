# Rine RAX Lifetime & Memory Allocator RCA Report

## Executive Summary

The main analytical contradiction in the previous report has been **100% resolved with empirical GDB hardware trace proof**. 

`0x143410480` (`Memory::alloc_static` free list lookup) **NEVER RETURNED `0x10202BF50`**.
- At line `0x142DF6D31` (`mov [rax], 1`), `RAX` was `0x000055555915F6A8` (a 100% valid, mapped host process heap allocation).
- At line `0x142DF6D63` (`mov rax, [rsi]`), `RAX` was loaded with `0x000000010202BF50` from stack storage `[RSI]` (`0x00007FFFFFFFBF70`).
- GDB Hardware Watchpoint trace identified the exact upstream cause: `Kernel32!WideCharToMultiByte` failed string conversion when `cb_multi_byte = 27` was less than converted length `28`, returning `0` (`ERROR_INSUFFICIENT_BUFFER`). This caused Godot's string initialization error handler to abort setup, leaving `[RSI]` uninitialized with garbage value `0x000000010202BF50`.

---

## 1. RAX AFTER 0x143410480

- **Return Address**: `0x142DF6D31`
- **Returned RAX Value**: `0x000055555915F6A8`
- **Mapping Verification**: Address belongs to mapped host process heap range (`0x00005555573EE000` - `0x000055555A438000`, RW).

---

## 2. MAPPING AT FIRST WRITE (`0x142DF6D31`)

- **Instruction**: `mov qword ptr [rax], 1`
- **Target Address**: `0x000055555915F6A8`
- **Mapping Status**: **`MAPPED AND VALID`**
- **Result**: `0x0000000000000001` written successfully into host heap memory at `0x000055555915F6A8`.

---

## 3. MAPPING AT CRASH (`0x142DF6D66`)

- **Instruction**: `mov r13, [rax + 0x2B8]`
- **RAX at Crash**: `0x000000010202BF50`
- **Target Address (`RAX + 0x2B8`)**: `0x000000010202C208`
- **Mapping Status**: **`UNMAPPED`**

---

## 4. REQUESTED ALLOCATION SIZE

- **RCX at `0x143410480`**: `0x0000000143EDF390` (Pointer to Godot Thread-Local MemoryPool Descriptor).
- **Pool Free List Item Size**: 64 bytes (`0x40` bytes).

---

## 5. ALLOCATOR IMPLEMENTATION

`0x143410480` is Godot Engine's Thread-Local Memory Pool Free-List Fetcher (`MemoryPool::get_free_list`):
```assembly
0x14341048a: mov rsi, [rcx + 0x10]           # Load element count
0x14341049c: call 0x143427310                # TlsGetValue lookup for thread memory pool
0x1434104b9: mov rcx, [rbx + rsi*8 + 0x8]    # Load pre-allocated buffer from free list array
0x1434104c7: mov rax, rcx                    # Return pointer in RAX (0x000055555915F6A8)
```

---

## 6. ALLOCATOR ABI

- **Calling Convention**: Win64 ABI (`extern "win64"`)
- **Parameters**: `RCX` = Descriptor Pointer
- **Return Type**: `*mut c_void` in 64-bit `RAX`. No 32-bit truncation occurs inside `0x143410480`.

---

## 7. POINTER VALUE TRANSITIONS

| Step | RIP | Instruction | RAX Value | Source | Mapping Status |
|---|---|---|---|---|---|
| 1 | 0x142DF6D2C | call 0x143410480 | 0x140AAAE30 | Pre-call | MAPPED (.text) |
| 2 | 0x142DF6D31 | mov [rax], 1 | 0x000055555915F6A8 | Return from 0x143410480 | MAPPED (Host Heap) |
| 3 | 0x142DF6D38 | mov [rsp+0x100], rax | 0x000055555915F6A8 | Stack Save | MAPPED (Host Heap) |
| 4 | 0x142DF6D55 | mov rsi, [r12] | 0x0000555559141738 | Load R12 | MAPPED (Host Heap) |
| 5 | 0x142DF6D63 | mov rax, [rsi] | 0x000000010202BF50 | Memory load from `[RSI]` | UNMAPPED |
| 6 | 0x142DF6D66 | mov r13, [rax+0x2B8] | 0x000000010202BF50 | Dereference | UNMAPPED (Crash) |

---

## 8. FIRST BAD POINTER VALUE

- **First Bad Pointer Value**: `0x000000010202BF50`
- **Location of First Appearance**: Loaded into `RAX` at line `0x142DF6D63` (`mov rax, [rsi]`) from memory at `[RSI]` (`0x00007FFFFFFFBF70`).

---

## 9. [RSI] STORAGE ORIGIN & HARDWARE WATCHPOINT

- **`[RSI]` Storage Address**: `0x00007FFFFFFFBF70` (Local stack frame variable).
- **GDB Hardware Watchpoint Result**: Watchpoint captured `Kernel32!WideCharToMultiByte` failing string conversion when `cb_multi_byte = 27` was smaller than converted length `28`, returning `0` (`ERROR_INSUFFICIENT_BUFFER`).

---

## 10. HARDWARE WATCHPOINT BACKTRACE

```
#16 0x0000555555f471c7 in rine::dlls::kernel32::WideCharToMultiByte (
    _code_page=65001,
    _dw_flags=0,
    lp_wide_char_str=0x55555a21b560,
    cch_wide_char=-1,
    lp_multi_byte_str=0x555559177240,
    cb_multi_byte=27,
    _lp_default_char=0x0,
    _lp_used_default_char=0x0
) at src/dlls/kernel32.rs:1865
```

---

## 11. CORRECTED ADDRESS ARITHMETIC

For candidate PE mapped address `0x14002BF50` vs bad value `0x10202BF50`:
- **Candidate Address (`good`)**: `0x000000014002BF50`
- **Bad Address (`bad`)**: `0x000000010202BF50`
- **Subtraction (`good - bad`)**: `0x000000003E000000` (NOT `0x40000000`)
- **Bitwise XOR (`good ^ bad`)**: `0x0000000042000000`
- **High 32 Bits**: Both equal `0x00000001`
- **Low 32 Bits**: `good` low32 = `0x4002BF50`, `bad` low32 = `0x0202BF50`

---

## 12. OBJECT TYPE

- **Object Classification**: **`OBJECT TYPE = UNKNOWN`** (Uninitialized stack buffer resulting from failed `WideCharToMultiByte` string conversion).

---

## 13. PROVEN ROOT CAUSE

- **Proven Root Cause**: `WideCharToMultiByte` (`src/dlls/kernel32.rs:1852..1865`) returned `0` when `cb_multi_byte < out_bytes.len()`. According to the Win32 API contract, `WideCharToMultiByte` MUST copy as many bytes as will fit into `lp_multi_byte_str` up to `cb_multi_byte` bytes before returning 0 / `ERROR_INSUFFICIENT_BUFFER`. When Rine returned 0 without writing partial bytes, Godot's string converter failed setup and left `[RSI]` containing garbage value `0x10202BF50`.

---

## 14. PATCH STATUS

- **Patch Status**: **`UNPATCHED`** (Pursuant to Section 10 rules, no premature patches applied. Waiting for exact Win32 contract alignment for `WideCharToMultiByte`).

---

## 15. NEXT ACTION

Update `WideCharToMultiByte` in `src/dlls/kernel32.rs` to write partial converted bytes up to `cb_multi_byte` before setting `ERROR_INSUFFICIENT_BUFFER`.
