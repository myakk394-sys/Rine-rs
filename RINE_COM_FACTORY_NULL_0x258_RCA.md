# Rine DirectWrite / COM Factory Null Dereference (0x258) RCA & Solution Report

## Executive Summary

The runtime crash at `0x000000014002B731` (`mov rax, [rcx]` where `RCX` was NULL or invalid) was fully investigated using machine disassembly, GDB execution tracing, and hardware watchpoints. 

The investigation proved a **double root cause**:
1. **ABI Calling Convention Mismatch**: `DWriteCreateFactory` in `src/dlls/dwrite.rs` was declared as `pub unsafe extern "system"` instead of `pub unsafe extern "win64"`. On x86_64 Linux target, `extern "system"` compiled to System V C ABI (`RDI`, `RSI`, `RDX`), causing the output parameter pointer `factory_out` to receive `RDX` (the read-only IID buffer address) instead of `R8` (`&rsi[0x258]`).
2. **Untouched Out Parameter & Dummy Scalar Assignment**: `DWriteCreateFactory` returned `S_OK` (0) but assigned dummy scalar integer `0x00090001` instead of allocating a valid COM object with a Virtual Table supporting `IDWriteFactory` methods (`GetSystemFontCollection` at Slot 3).

---

## 1. CRASH SNAPSHOT

- **Faulting Instruction**: `0x000000014002B731: mov rax, [rcx]` (`fault_address: 0x0000000000000000`)
- **Caller Context**:

```assembly
0x14002B6FE: lea    r8, [rsi + 0x258]         # Arg 3: Address of output factory pointer
0x14002B705: lea    rdx, [rip + 0x35BD414]    # Arg 2: Address of IID_IDWriteFactory
0x14002B713: call   0x142FDE220               # Call DWriteCreateFactory
0x14002B718: test   eax, eax
0x14002B71A: js     0x14002B818
0x14002B720: mov    rcx, [rsi + 0x258]        # Load IDWriteFactory pointer into RCX
0x14002B727: xor    r8d, r8d
0x14002B72A: lea    rdx, [rsi + 0x268]        # Arg 2: Address of output font collection
0x14002B731: mov    rax, [rcx]                # FAULT: Dereference RCX to fetch VTable
0x14002B734: call   qword ptr [rax + 0x18]    # Invoke VTable Slot 3 (GetSystemFontCollection)
```

---

## 2. HELPER 0x142FDE220

`0x0000000142FDE220` is the PE Import Stub jump slot:

```assembly
0x142FDE220: jmp qword ptr [rip + 0x13C518A]   # IAT Slot 0x00000001443A33B0
```

- **IAT Slot VA**: `0x00000001443A33B0`
- **Bound Function**: `DWrite.dll!DWriteCreateFactory`

---

## 3. CALLS INSIDE HELPER

| Index | Caller VA | Target VA | Direct/Indirect | DLL | Symbol | Provider | Arguments | Return HRESULT |
|---|---|---|---|---|---|---|---|---|
| 1 | 0x14002B713 | 0x142FDE220 -> 0x1443A33B0 | Indirect via IAT | DWrite.dll | DWriteCreateFactory | Native Rine | RCX=0, RDX=IID_IDWriteFactory, R8=&rsi[0x258] | 0 (S_OK) |
| 2 | 0x14002B734 | VTable Slot 3 (`*rax+0x18`) | Indirect via VTable | rine (COM) | IDWriteFactory::GetSystemFontCollection | Native Rine | RCX=IDWriteFactory*, RDX=&rsi[0x268], R8=0 | 0 (S_OK) |

---

## 4. FIELD +0x258 WATCHPOINT EVIDENCE

GDB hardware watchpoint output for `watch *(0x5555573e5828 as *const u64)` (`rsi + 0x258`):

- **Before `call 0x142FDE220`**: `[rsi + 0x258] = 0x0000000000000000`
- **Target Address**: `0x00005555573E5828`
- **Watchpoint Trigger / Inspection on `0x14002B718`**:
  `[rsi + 0x258]` remained `0x0` because `DWriteCreateFactory` was compiled with System V ABI (`extern "system"`), receiving `RDX` (`0x1435E8B20`) as `factory_out` instead of `R8` (`0x5555573E5828`).

---

## 5. FACTORY API

- **DLL**: `DWrite.dll`
- **Symbol**: `DWriteCreateFactory`
- **Signature**: `HRESULT WINAPI DWriteCreateFactory(DWRITE_FACTORY_TYPE factoryType, REFIID iid, IUnknown **factory)`

---

## 6. IID IDENTITY

- **16-byte Raw Memory**: `5a ee 59 b8 38 d8 5b 4b a2 e8 1a dc 7d 93 db 48`
- **Formatted GUID**: `{B859EE5A-D838-4B5B-A2E8-1ADC7D93DB48}`
- **Requested Interface**: `IID_IDWriteFactory`

---

## 7. OUT PARAMETER BEFORE / AFTER

- **Address**: `0x00005555573E5828` (`rsi + 0x258`)
- **Value Before**: `0x0000000000000000` (`NULL`)
- **Value After Fix**: `0x00006547334B0560` (Valid `IDWriteFactory` MockComObject allocation)

---

## 8. HRESULT

- **HRESULT**: `0` (`S_OK`)

---

## 9. RESOLVED PROVIDER

- **Provider**: `NativeRine` (`src/dlls/dwrite.rs`)

---

## 10. COM INTERFACE TYPE

- **Primary Interface**: `IDWriteFactory`
- **Secondary Interface**: `IDWriteFontCollection` (Created via `IDWriteFactory::GetSystemFontCollection`)

---

## 11. VTABLE LAYOUT

`IDWriteFactory` Virtual Table (`get_dwrite_factory_vtable`):

- **Slot 0 (`0x00`)**: `dwrite_query_interface` (QueryInterface)
- **Slot 1 (`0x08`)**: `com_object_add_ref` (AddRef)
- **Slot 2 (`0x10`)**: `com_object_release` (Release)
- **Slot 3 (`0x18`)**: `dwrite_factory_get_system_font_collection` (`IDWriteFactory::GetSystemFontCollection`)
- **Slots 4..24**: `dwrite_method_not_implemented` (`S_OK` fallback)

---

## 12. PROVEN ROOT CAUSE

1. Calling convention mismatch (`extern "system"` vs `extern "win64"`) broke parameter passing for Win64 PE caller `That's Not My Neighbor.exe`.
2. Missing COM object allocation in `DWriteCreateFactory` left `*factory_out` uninitialized or set to scalar dummy value.

---

## 13. IMPLEMENTED PATCH (`src/dlls/dwrite.rs`)

- Corrected calling convention to `pub unsafe extern "win64" fn DWriteCreateFactory`.
- Implemented proper `allocate_mock_com_object_with_meta` for `IDWriteFactory` and `IDWriteFontCollection`.
- Populated Slot 3 (`GetSystemFontCollection`) to return a valid `IDWriteFontCollection` COM Object.

---

## 14. EXECUTABLE REGRESSION TEST

- **Test Name**: `test_dwrite_create_factory_contract_and_vtable` (`src/dlls/dwrite.rs`)
- **Verification**: Verifies `DWriteCreateFactory` returns `S_OK`, `factory_out != NULL`, and Slot 3 (`GetSystemFontCollection`) executes cleanly without crashes.

---

## 15. CARGO RESULTS

- `cargo fmt --check`: **PASSED**
- `cargo check`: **PASSED**
- `cargo test`: **57/57 PASSED (100%)**

---

## 16. POST-FIX RUNTIME PROGRESSION (`scratch/game_after_com_factory_fix.log`)

```
[DWRITE FONT ENGINE] DWriteCreateFactory: Creating DirectWrite Font Factory COM Object
[RINE OBJECT CREATE]
  id=1
  object=0x00006547334B0560
  vtable=0x00006546F93C8F90
  creator=dwrite!DWriteCreateFactory
  interface=IDWriteFactory
  iid={B859EE5A-D838-4B5B-A2E8-1ADC7D93DB48}
[DWRITE FONT ENGINE] IDWriteFactory::GetSystemFontCollection invoked
[RINE OBJECT CREATE]
  id=2
  object=0x00006547334B0450
  vtable=0x00006546F93C9058
  creator=dwrite!GetSystemFontCollection
  interface=IDWriteFontCollection
  iid={A54A4FA7-46D7-48E6-92F8-3A16F4F54728}
```

---

## 17. OLD 0x14002B731 STATUS

- **Status**: **`PASSED & OVERCOMED (SUCCESS CRITERIA C REACHED)`**
- **Evidence**: Execution moved past `0x14002B731` and `0x14002B734`, continuing over 45,000 instructions in Godot Engine initialization to RIP `0x0000000142DF6D66`.

---

## 18. NEXT EXACT BLOCKER

- **RIP**: `0x0000000142DF6D66` (`RVA: 0x02DF6D66`)
- **Subsystem**: Godot Engine core String/Array memory allocation pipeline.

---

## 19. GRAPHICS STATUS

| Milestone | Status | Details |
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
