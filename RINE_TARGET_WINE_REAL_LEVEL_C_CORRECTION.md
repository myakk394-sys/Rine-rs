# Rine Target-Game Wine Adapter Level C / D Correction Report

## Corrected Status Summary

- **WINE LEVEL B**: **`PROVEN`** (Real Wine PE export executed by Rine loader).
- **SYNTHETIC WINE LEVEL C**: **`PROVEN`** (Verified `hid.dll!HidD_GetHidGuid` populated 16-byte output GUID structure `{4D1E55B2-F16F-11CF-88CB-001111000030}` and returned cleanly in proof binary).
- **TARGET WINE LEVEL C**: **`NOT YET PROVEN`** (All target imports executed so far were handled by Native Rine Priority 1; `wine_calls` count = 0 in main game process).
- **TARGET WINE LEVEL D**: **`NOT YET PROVEN`** (Native Rine `AttachConsole` passed the initial console blocker, but Wine Adapter has not yet passed a real target game blocker).

---

## 1. ATTACHCONSOLE CALL PATH & DIAGNOSTICS FIX

Diagnosed fields for `KERNEL32.dll!AttachConsole`:

- **`call_instruction_va`**: `0x000000014002B60E` (`FF 15 E4 7E 37 04` -> `call qword ptr [0x1443A34E8]`)
- **`return_va`**: `0x000000014002B614` (`48 83 C4 28` -> `add rsp, 0x28`)
- **`iat_slot_va`**: `0x00000001443A34E8`
- **`iat_slot_value`**: `0x000055555589DC40` (Actual entry point VA of Native Rine `AttachConsole` in host memory space)
- **`resolved_function_va`**: `0x000055555589DC40` (Native Rine implementation address)

---

## 2. IAT SLOT ACTUAL VALUE

- **Correction**: The diagnostic output in `src/dlls/registry.rs` has been corrected so that `iat_slot_value` displays the actual target function address stored in the IAT slot (`0x000055555589DC40`), rather than confusing it with the return address (`0x000000014002B614`).

---

## 3. UNIVERSAL THUNK ALIGNMENT

Detailed step-by-step stack alignment math for `universal_stub_thunk` (`src/dlls/registry.rs`):

| Checkpoint | Stack Modification | RSP Alignment (`RSP % 16`) | Notes |
|---|---|---|---|
| **Thunk Entry** | `call [iat_slot]` pushed 8-byte ret address | **`8`** | Standard Win64 entry condition |
| **After Eight Pushes** | `push rbx..r15` (64 bytes) | **`8`** | `(8 - 64) % 16 = -56 % 16 = 8` |
| **After XMM Allocation** | `sub rsp, 0xA0` (160 bytes) | **`8`** | `(8 - 160) % 16 = -152 % 16 = 8` |
| **Immediately Before Host CALL** | `sub rsp, 0x28` (40 bytes) | **`0`** | `(8 - 40) % 16 = -32 % 16 = 0` |
| **Host Callee Entry (`universal_stub_function`)** | `call {inner_stub}` pushed 8-byte ret address | **`8`** | Callee entry point has `RSP % 16 = 8` |

- **Callee Stack Space**: Inside `universal_stub_function`, rustc allocates local stack space `sub rsp, N` where `N % 16 == 0` (e.g. `0x1680`), making `(rsp + 0x1650) % 16 == 0`. Aligned SIMD instruction `movaps` executes cleanly with **zero** SIGSEGV.

---

## 4. ATTACHCONSOLE SEMANTIC STATUS

Current `KERNEL32.dll!AttachConsole` native implementation status:

- **`ABI VERIFIED`**: Win64 calling convention, nonvolatile register preservation verified.
- **`ALPHA COMPAT SEMANTICS`**: Unconditional `TRUE` (1) return allows Godot engine initialization to proceed.
- **`FULL WIN32 SEMANTICS INCOMPLETE`**: Does not perform full Win32 console handle attachment to POSIX ttys.

---

## 5. FIRST ACTUAL TARGET WINE API

- **Current Progress**: Executing `That's Not My Neighbor.exe` with `--wine-provider`.
- **Target Call Count**:
  - `native_rine_calls`: 564
  - `wine_calls`: 0
- **Status**: No real target import has invoked Wine Adapter yet (`wine_calls = 0`). Therefore, **TARGET Wine Level C remains NOT YET PROVEN**.

---

## 6. VERSION.DLL DEPENDENCY FAILURE

Full dependency chain for `version.dll!GetFileVersionInfoW`:

```
That's Not My Neighbor.exe
  └─► version.dll!GetFileVersionInfoW (Export RVA: 0x00001304)
        └─► jmp qword ptr [0x0000B2B0] (IAT Slot RVA: 0x0000B2B0)
              └─► kernelbase.dll!GetFileVersionInfoW
                    └─► Dynamic Fallback Stub (Class D / Missing Native Rine Implementation)
```

- **Wine Export RVA**: `0x00001304` (`0x7FFFF78DB304`)
- **Internal Jump Offset**: `[0x0000B2B0]`
- **Target Forwarder/Import**: `kernelbase.dll!GetFileVersionInfoW`
- **Dependency Classification**: **Class D** (Unsupported / Missing Native Rine implementation).

---

## 7. TARGET WINE CALL GDB PROOF PROTOCOL

To establish TARGET Wine Level C in future runs, the following machine-stepping protocol must be executed:

1. `break *<Wine Export VA>`
2. `x/i $rip`
3. `x/gx <IAT Slot VA>`
4. `info registers rsp`
5. `stepi` -> Verify `RIP` inside mapped Wine `.text` section.
6. `finish` / `stepi` -> Verify `RIP` at actual Godot instruction immediately after `CALL`.

---

## 8. PROVIDER STATS (`scratch/game_real_wine_provider_target.log`)

```
[RINE PROVIDER STATS]
native_rine_calls=564
native_rine_symbols=379

wine_calls=0
wine_symbols=0

unresolved_calls=326
unresolved_symbols=245
```

---

## 9. CURRENT GODOT RIP & BLOCKER ANALYSIS

- **Last Observed RIP**: `0x000000014002B731` (`RVA: 0x0002B731`)
- **Disassembly Context**:

```assembly
0x14002B713: call   0x142FDE220          # Calls DXGI / DWrite setup helper
0x14002B718: test   eax, eax
0x14002B71A: js     0x14002B818
0x14002B720: mov    rcx, [rsi + 0x258]   # Loads COM Interface Pointer
0x14002B727: xor    r8d, r8d
0x14002B72A: lea    rdx, [rsi + 0x268]
0x14002B731: mov    rax, [rcx]           # FAULT SITE: RCX is NULL (0x0)
0x14002B734: call   qword ptr [rax + 0x18] # VTable Slot 3 (QueryInterface / GetDesc)
```

- **Analysis**: `0x14002B731` is a **crash RIP** resulting from a NULL COM interface pointer dereference when invoking Slot 3 of the COM Virtual Table.

---

## 10. GRAPHICS STATUS

| Feature / Milestone | Status | Details |
|---|---|---|
| **DXVK Mapped** | **YES** | `d3d11.dll` mapped @ `0x0000000359050000` |
| **CreateDXGIFactory1 Resolved** | **YES** | Resolved via Native Rine `dxgi.dll` |
| **CreateDXGIFactory1 Called** | **YES** | Invoked during static subsystem initialization |
| **D3D11CreateDevice Resolved** | **YES** | Resolved via DXVK `d3d11.dll` |
| **D3D11CreateDevice Called** | **NO** | Render pipeline setup incomplete |
| **vkCreateInstance Called** | **NO** | Vulkan instance not initialized |
| **vkCreateDevice Called** | **NO** | Vulkan physical device not created |
| **Swapchain Created** | **NO** | Swapchain object not allocated |
| **vkQueuePresentKHR Called** | **NO** | No present calls issued |
| **Visible Frame Confirmed** | **NO** | Frame buffer presentation not reached |

---

## 11. TARGET WINE LEVEL C & D STATUS

- **TARGET WINE LEVEL C**: **`NOT YET PROVEN`**
- **TARGET WINE LEVEL D**: **`NOT YET PROVEN`**
- **RINE TARGET PROGRESS MILESTONE**: Passed initial console blocker `AttachConsole` via Native Rine.

---

## 12. NEXT EXACT BLOCKER

COM VTable Interface Query setup for DXGI Factory / Adapter at `0x000000014002B731`.
