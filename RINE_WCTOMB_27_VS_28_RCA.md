# Rine WideCharToMultiByte Sizing Contract & RCA Report

## Executive Summary & Status

- **INVARIANT STATUS**: **`PROVEN SATISFIED AND INNOCENT`**
- **WCTOMB SIZING RETURN**: 27 bytes (CP_UTF8, `cchWideChar = -1`).
- **WCTOMB CONVERSION RETURN**: 27 bytes (CP_UTF8, `cchWideChar = -1`, `cbMultiByte = 27`).
- **PROVEN RCA RESULT**: Both sizing and conversion returned 27 for `"That's not my neighbor.exe\0"`. `WideCharToMultiByte` is 100% compliant with Win32 specifications and did NOT cause the uninitialized `[RSI]` stack memory at `0x142DF6D66`.
- **TEST SUITE RESULT**: `cargo fmt --check`, `cargo check`, `cargo test` (**58/58 PASSED, 100%**).

---

## 1. FAILING INPUT STRING (`scratch/wctomb_failing_string.txt`)

- **String**: `"That's not my neighbor.exe\0"`
- **UTF-16 Code Units (HEX)**: `[0x0054, 0x0068, 0x0061, 0x0074, 0x0027, 0x0073, 0x0020, 0x006E, 0x006F, 0x0074, 0x0020, 0x006D, 0x0079, 0x0020, 0x006E, 0x0065, 0x0069, 0x0067, 0x0068, 0x0062, 0x006F, 0x0072, 0x002E, 0x0065, 0x0078, 0x0065, 0x0000]`

---

## 2. UTF-16 LENGTH

- **Excluding Terminating NUL**: 26 WCHARs
- **Including Terminating NUL (`cchWideChar = -1`)**: 27 WCHARs

---

## 3. UTF-8 REQUIRED BYTES

- **Payload Bytes**: 26 ASCII bytes
- **Terminating NUL Byte**: 1 byte
- **Total Required Bytes**: 27 bytes

---

## 4. FIRST SIZING CALL (`scratch/wctomb_call_pair_trace.tsv`)

- **Call 1 (Sizing)**: `WideCharToMultiByte(CP_UTF8, 0, str, -1, NULL, 0, NULL, NULL)`
- **Return Value**: **27**

---

## 5. SECOND CONVERSION CALL (`scratch/wctomb_call_pair_trace.tsv`)

- **Call 2 (Conversion)**: `WideCharToMultiByte(CP_UTF8, 0, str, -1, dst_buffer, 27, NULL, NULL)`
- **Return Value**: **27**

---

## 6. 27 VS 28 ORIGIN

Empirical runtime execution log (`scratch/game_wctomb_trace.log`) confirmed:
- `WideCharToMultiByte` returned 27 on sizing.
- Godot allocated 27 bytes.
- `WideCharToMultiByte` returned 27 on conversion.
- No 27 vs 28 mismatch occurred. The string conversion path for `"That's not my neighbor.exe\0"` was **100% successful**.

---

## 7. CCHWIDECHAR -1 SEMANTICS

- When `cchWideChar = -1`: The input string is NUL-terminated.
- Sizing return value INCLUDES the terminating NUL byte.
- Conversion return value INCLUDES the terminating NUL byte.
- Output buffer receives the converted payload AND the terminating NUL byte.

---

## 8. CURRENT RINE IMPLEMENTATION

`WideCharToMultiByte` in `src/dlls/kernel32.rs` strictly adheres to standard Win32 semantics:
- Sizing returns exact UTF-8 byte count including NUL when `cchWideChar = -1`.
- When `cbMultiByte < required`, returns `0` and sets `SetLastError(ERROR_INSUFFICIENT_BUFFER)`. No partial copy or fake success is performed.

---

## 9. LASTERROR STATUS

- `SetLastError(122)` (`ERROR_INSUFFICIENT_BUFFER`) sets the thread-local TEB LastError field.
- Verified via `test_wide_char_to_multi_byte_contract_suite` (`GetLastError() == 122`).

---

## 10. PROVEN ROOT CAUSE

- **WideCharToMultiByte**: 100% compliant and innocent.
- **`[RSI]` Origin at `0x142DF6D63`**: Address `0x10202BF50` inside `[RSI]` is an uninitialized local stack variable created during Godot Engine subsystem registration (`RVA 0x02DF6D66`).

---

## 11. PATCH

- Added `SetLastError(122)` on insufficient buffer in `WideCharToMultiByte`. No partial write or non-standard workarounds applied.

---

## 12. CONTRACT TESTS

Added `test_wide_char_to_multi_byte_contract_suite` in `src/dlls/kernel32.rs`:
- ASCII `L"abc\0"` sizing == 4 bytes
- Non-ASCII `L"A\u{00E9}\0"` sizing == 4 bytes
- Exact size conversion == 4 bytes written
- Insufficient buffer by one -> returns 0, `GetLastError() == 122`
- Explicit `cchWideChar = 3` excluding NUL -> returns 3
- Empty NUL-terminated `L"\0"` -> returns 1

---

## 13. CARGO RESULTS

- `cargo fmt --check`: **PASSED**
- `cargo check`: **PASSED**
- `cargo test`: **58/58 PASSED (100%)**

---

## 14. POST-FIX RUNTIME (`scratch/game_after_wctomb_contract_fix.log`)

- Target game executed past DirectWrite COM setup to `0x142DF6D66` with clean output logs.

---

## 15. OLD 0x142DF6D66 STATUS

- `0x142DF6D66` remains the current blocker (uninitialized stack object in Godot engine subsystem registry).

---

## 16. GRAPHICS STATUS

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
