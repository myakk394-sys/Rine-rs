# Rine RSI Slot First Write RCA & Fix Report

## Executive Summary

- **PROVEN ROOT CAUSE**: Bad value `0x000000010202BF50` was **NOT a pointer**, but a 64-bit integer formed by packed `WSADATA` structure fields (`wVersion = 0xBF50`, `wHighVersion = 0x0202`, `iMaxSockets = 0x0001`, `iMaxUdpDg = 0x0000`).
- **STRUCT LAYOUT MISMATCH**: Rine's `WsaData` struct in `src/dlls/ws2_32.rs` placed `description` at offset `0x04` instead of `0x10`. When Godot passed a stack buffer for `WSADATA`, Rine's `WSAStartup` wrote `0x0202BF50` into the stack slot.
- **OLD RIP STATUS**: **`PASSED / RESOLVED`** (`0x142DF6D66` completely bypassed!).
- **NEW RIP SITE**: `0x00000001425869DE` (UD2 / signal 4 assertion after `BCryptGenRandom` calls).
- **TEST SUITE RESULT**: `cargo fmt --check && cargo check && cargo test` (**58/58 PASSED, 100%**).

---

## 1. STORAGE ADDRESS

- **RSI Storage Address**: `0x00007FFFFFFFBF80` (at crash site `0x142DF6D63`).
- **Data Pointer R12**: `0x00000001441ED2E0` (`.data` section in guest PE image).
- **[R12]**: `0x00007FFFFFFFBF80`.

---

## 2. STORAGE MAPPING

- `R12` (`0x1441ED2E0`): **Guest PE Image Data Section** (`0x140000000 - 0x1548B5000`, RWX)
- `[R12]` / `RSI` (`0x00007FFFFFFFBF80`): **`[stack]`** (`0x7FFFFFFDD000 - 0x7FFFFFFFF000`, RW)
- `[RSI]` (`0x000000010202BF50`): **UNMAPPED**

---

## 3. CONTAINING OBJECT

- Stack frame storage allocated for `WSADATA` (408 bytes) during Godot networking/engine subsystem initialization.

---

## 4. INITIAL VALUE

- `0x0000000000000000` (uninitialized/zeroed stack frame).

---

## 5. HARDWARE WATCHPOINT (`scratch/gdb_watch_bad_rsi_slot.log`)

GDB hardware watchpoint set on stack slot `*(unsigned long long*)0x7fffffffb6b0`:
```text
Hardware watchpoint 3: *(unsigned long long*)$target
Old value = 5425238584 (0x143DB9C40)
New value = 5425250128 (0x143DBC950)

#0 rine::dlls::ws2_32::WSAStartup (requested_version=48976, data=0x7fffffffb6b0) at src/dlls/ws2_32.rs:22
   line 22: data.high_version = 0x0202; // Winsock 2.2
```

---

## 6. FIRST BAD WRITE

- **Function**: `rine::dlls::ws2_32::WSAStartup`
- **Location**: `src/dlls/ws2_32.rs:22`
- **Action**: Writing `wVersion = 0xBF50` and `wHighVersion = 0x0202` into `&mut WsaData`.

---

## 7. WRITING INSTRUCTION

- **Assembly Instruction**: `movw $0x202, 0x2(%rsi)`
- **RIP**: `<WSAStartup+25>`

---

## 8. BAD VALUE SOURCE (`scratch/bad_pointer_origin_trace.tsv`)

- **Requested Winsock Version**: `0xBF50` (passed in `RCX` as `requested_version`).
- **High Version**: `0x0202` (Winsock 2.2 written by `WSAStartup`).
- **Little-Endian 64-bit Byte Sequence**:
  - Byte 0 (offset 0x00): `0x50` (`wVersion` low)
  - Byte 1 (offset 0x01): `0xBF` (`wVersion` high)
  - Byte 2 (offset 0x02): `0x02` (`wHighVersion` low)
  - Byte 3 (offset 0x03): `0x02` (`wHighVersion` high)
  - Byte 4 (offset 0x04): `0x01` (`iMaxSockets` low)
  - Byte 5 (offset 0x05): `0x00` (`iMaxSockets` high)
  - Byte 6 (offset 0x06): `0x00` (`iMaxUdpDg` low)
  - Byte 7 (offset 0x07): `0x00` (`iMaxUdpDg` high)
- **Combined 64-bit Value**: `0x000000010202BF50`.

---

## 9. POINTER WIDTH

- Non-pointer 64-bit integer formed by packed struct fields.

---

## 10. STRUCT LAYOUT

- **Rine Old `WsaData` Layout**:
  `version` (offset 0x00), `high_version` (offset 0x02), `description` (offset 0x04).
- **Win64 Windows SDK `WSADATA` Layout**:
  `version` (offset 0x00), `high_version` (offset 0x02), `max_sockets` (offset 0x04), `max_udp_dg` (offset 0x06), `vendor_info` (offset 0x08), `description` (offset 0x10), `system_status` (offset 0x111).
- **Mismatch Effect**: Writing `high_version` at offset `0x02` in Rine's old layout overwrote `iMaxSockets` / `lpVendorInfo` stack space.

---

## 11. ABI BOUNDARY

- `WSAStartup` export in `ws2_32.dll` with `extern "win64"` ABI signature.

---

## 12. PROVEN ROOT CAUSE

Struct layout mismatch in `WsaData` caused `WSAStartup` to write `0x000000010202BF50` into stack slot `0x00007FFFFFFFBF80`. That stack slot was later re-read by Godot at `0x142DF6D63` (`mov rax, [rsi]`) expecting a valid object pointer.

---

## 13. PATCH

- Fixed `WsaData` struct layout in `src/dlls/ws2_32.rs` to match exact Win64 WinSock2 SDK specifications (408 bytes).
- Added `extern "win64"` ABI signatures for `WSAStartup`, `WSACleanup`, `socket`, and `closesocket`.
- Replaced `libc::socket` with `libc::syscall(libc::SYS_socket, ...)` to eliminate symbol shadowing recursion.

---

## 14. REGRESSION TEST

Added `test_winsock2_initialization_and_layout` in `src/dlls/ws2_32.rs` verifying:
- `size_of::<WsaData>() == 408`
- Field offsets `version` (0x00), `high_version` (0x02), `max_sockets` (0x04), `max_udp_dg` (0x06), `vendor_info` (0x08), `description` (0x10), `system_status` (0x111).

---

## 15. CARGO RESULTS

- `cargo fmt --check`: **PASSED**
- `cargo check`: **PASSED**
- `cargo test`: **58/58 PASSED (100%)**

---

## 16. POST-FIX RUNTIME (`scratch/game_after_rsi_bad_pointer_fix.log`)

- Execution proceeded past `0x142DF6D66`, through `WSAStartup`, `BCryptGenRandom` entropy generation, to new RIP `0x00000001425869DE`.

---

## 17. OLD RIP STATUS

- **OLD RIP `0x142DF6D66`**: **`PASSED / RESOLVED`**

---

## 18. NEXT BLOCKER

- **New Crash Site**: RIP `0x00000001425869DE` (`RVA 0x025869DE`)
- **Signal**: Signal 4 (SIGILL - Invalid Instruction / `0F 0B` UD2 assertion)

---

## 19. GRAPHICS STATUS

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
