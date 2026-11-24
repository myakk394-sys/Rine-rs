# Rine UD2 0x1425869DE RCA & Fix Report

## Executive Summary

- **PROVEN ROOT CAUSE**: The `ud2` instruction at `0x1425869DE` was caused by a missing Win64 C Runtime export for `_beginthreadex` in `MSVCRT.DLL` / `UCRTBASE.DLL`.
  - At `0x142586963`, Godot Engine called `call qword ptr [0x1443a3a48]` (`msvcrt.dll!_beginthreadex`).
  - Because `_beginthreadex` was unmapped in `src/pe/imports.rs`, Rine fell back to `msvcrt_generic_stub`, which returned `0` (`NULL`).
  - At `0x142586969`, Godot tested `RAX == 0` and executed `je 0x1425869d1` -> `call 0x143544190` -> **`ud2`** at `0x1425869DE`.
- **BCryptGenRandom INNOCENCE**: All 4 `BCryptGenRandom` calls were proven innocent (valid stack buffers, `STATUS_SUCCESS` returned). Upgraded `BCryptGenRandom` to use `/dev/urandom` OS CSPRNG.
- **OLD UD2 STATUS**: **`PASSED / RESOLVED`** (`0x1425869DE` completely bypassed!).
- **TEST SUITE RESULTS**: `cargo fmt --check && cargo check && cargo test` (**58/58 PASSED, 100%**).

---

## 1. WINSOCK RCA CORRECTION

- **Requested Version Verification**: GDB breakpoint at `WSAStartup` entry proved:
  - `RCX` (`requested_version`) = `0x0202` (514 = Winsock 2.2).
  - `RDX` (`lpWsaData`) = `0x00007FFFFFFFBF80` (Valid stack pointer).
- **Typo Fix**: Byte 3 in packed struct layout is `0x02` (NOT `0x03`).
- **Winsock Module Freeze**: `src/dlls/ws2_32.rs` **FROZEN** (no further modifications made).

---

## 2. UD2 SNAPSHOT (`scratch/gdb_ud2_1425869de_snapshot.log`)

```text
RIP: 0x00000001425869DE | RSP: 0x00007FFFFFFFBF40
RAX: 0x0000000000000001 | RBX: 0x0000555559960F10
RCX: 0x000055500CC073AE | RDX: 0x000000000000000E
RSI: 0x000055555995E8C0 | RDI: 0x00007FFFFFFFBF88
RBP: 0x000000014263FA00 | R8 : 0x0000000000000030
R9 : 0xDE92C6635921EDC0 | R10: 0x0000000000000031
R11: 0x00007FFFF7C12AC0 | R12: 0x000055555995EED0
R13: 0x000055555995EDB0 | R14: 0x00000001437BBE10
R15: 0x000055555995EED0 | EFLAGS: 0x10202 [ IF RF ]
```

---

## 3. FUNCTION DISASSEMBLY (`scratch/function_1425869de_disassembly.txt`)

```assembly
   0x1425868e0: push   %r12
   0x1425868e2: push   %rbp
   0x1425868e3: push   %rdi
   0x1425868e4: push   %rsi
   0x1425868e5: push   %rbx
   0x1425868e6: sub    $0x40,%rsp
   ...
   0x142586963: call   qword ptr [0x1443a3a48]   ; IAT: msvcrt.dll!_beginthreadex
   0x142586969: test   %rax,%rax
   0x14258696c: je     0x1425869d1               ; Trap branch on NULL handle!
   0x14258696e: cmpq   $0x0,0x8(%rbx)
   0x142586973: mov    0x3c(%rsp),%edx
   0x142586977: jne    0x1425869e0
   0x142586979: mov    %rax,0x8(%rbx)            ; Store valid thread handle
   ...
   0x1425869d1: mov    $0x28,%edx
   0x1425869d6: mov    %rsi,%rcx
   0x1425869d9: call   0x143544190               ; Panic helper
   0x1425869de: ud2                              ; Assert trap!
```

---

## 4. UD2 PREDECESSOR

- **Control-Flow Predecessor Instruction**: `0x14258696C`: `je 0x1425869d1`
- **Trigger**: `RAX == 0` (NULL thread handle returned by `_beginthreadex`).

---

## 5. FAILED CONDITION

- **Expected Invariant**: `RAX != 0` (Valid Win64 thread handle returned by `_beginthreadex`).
- **Actual Runtime State**: `RAX == 0` (`msvcrt_generic_stub` fallback return value).

---

## 6. BCRYPT CALL TRACE (`scratch/bcrypt_genrandom_4calls.tsv`)

| Call Index | Guest Caller VA | Guest Caller RVA | hAlgorithm | pbBuffer | cbBuffer | dwFlags | Returned NTSTATUS |
|---|---|---|---|---|---|---|---|
| 1 | `0x1425D3460` | `0x025D3460` | `0x0` (NULL) | `0x7FFFFFFFAA20` | 128 | `0x2` (`BCRYPT_USE_SYSTEM_PREFERRED_RNG`) | `0x00000000` (`STATUS_SUCCESS`) |
| 2 | `0x1425D3460` | `0x025D3460` | `0x0` (NULL) | `0x7FFFFFFFAA20` | 128 | `0x2` (`BCRYPT_USE_SYSTEM_PREFERRED_RNG`) | `0x00000000` (`STATUS_SUCCESS`) |
| 3 | `0x1425D3460` | `0x025D3460` | `0x0` (NULL) | `0x7FFFFFFFAA20` | 128 | `0x2` (`BCRYPT_USE_SYSTEM_PREFERRED_RNG`) | `0x00000000` (`STATUS_SUCCESS`) |
| 4 | `0x1425D3460` | `0x025D3460` | `0x0` (NULL) | `0x7FFFFFFFAA20` | 128 | `0x2` (`BCRYPT_USE_SYSTEM_PREFERRED_RNG`) | `0x00000000` (`STATUS_SUCCESS`) |

---

## 7. BCRYPT ABI & BUFFER CONTRACT

- **Signature**: `extern "win64" fn BCryptGenRandom(hAlgorithm: usize, pbBuffer: *mut u8, cbBuffer: u32, dwFlags: u32) -> i32`
- **Validation Added**: Validates `hAlgorithm == 0` when `dwFlags & 0x2` is passed, returning `STATUS_INVALID_HANDLE` (`0xC0000008`) on violation.
- **RNG Implementation**: Uses Linux `/dev/urandom` OS CSPRNG for true cryptographic entropy.

---

## 8. PROVEN ROOT CAUSE

Unmapped `_beginthreadex` symbol in PE IAT resolver caused Godot's thread creation routine to receive a NULL handle (`0`), failing the `test rax, rax` check at `0x142586969` and jumping to `ud2` at `0x1425869DE`.

---

## 9. PATCH

- Implemented `msvcrt_beginthreadex` in `src/dlls/crt.rs` wrapping `CreateThread`.
- Mapped `_beginthreadex` and `_beginthread` in `src/pe/imports.rs`.
- Upgraded `BCryptGenRandom` in `src/dlls/advapi32.rs` to fill random buffers using `/dev/urandom`.

---

## 10. CARGO RESULTS

- `cargo fmt --check`: **PASSED**
- `cargo check`: **PASSED**
- `cargo test`: **58/58 PASSED (100%)**

---

## 11. OLD UD2 STATUS

- **OLD RIP `0x1425869DE`**: **`PASSED / RESOLVED`**

---

## 12. NEXT BLOCKER

- Execution proceeded past `0x1425869DE`, created background worker thread `id=512`, and reached `GetModuleFileNameW`.

---

## 13. GRAPHICS STATUS

| Feature / Milestone | Status | Details |
|---|---|---|
| **DXVK Mapped** | **YES** | `d3d11.dll` mapped @ `0x0000000359050000` |
| **CreateDXGIFactory1 Resolved** | **YES** | Resolved via Native Rine `dxgi.dll` |
| **CreateDXGIFactory1 Called** | **YES** | Invoked during static subsystem setup |
| **D3D11CreateDevice Resolved** | **YES** | Resolved via DXVK `d3d11.dll` |
| **D3D11CreateDevice Called** | **NO** | Render pipeline setup incomplete |
| **vkCreateInstance Called** | **NO** | Vulkan instance not initialized |
| **vkCreateDevice Called** | **NO** | Vulkan physical device not created |
| **Swapchain Created** | **NO** | Swapchain object not allocated |
| **vkQueuePresentKHR Called** | **NO** | No present calls issued |
| **Visible Frame Confirmed** | **NO** | Frame buffer presentation not reached |
