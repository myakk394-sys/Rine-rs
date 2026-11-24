# Rine Module Handle Lifecycle, RtlGetVersion, and Post-Version Progress Report

## 1. Executive Summary

This milestone resolves the exact root cause of the previous blocker around `FreeLibrary(0x140000000)` and accomplishes a major architectural transition in Rine:
1. **Module Identity & Scoping Fix**: `LoadLibraryW(L"ntdll.dll")` previously returned `0x140000000` (the main EXE base), causing `GetProcAddress(0x140000000, "RtlGetVersion")` to fail to find `ntdll` symbols. We implemented a unified, thread-safe `ModuleRegistry` in `src/dlls/kernel32.rs` that returns stable module handles (`0x7FFD0000` for `ntdll.dll`), tracks reference counts, and preserves core resident module semantics (`[ALPHA CORE MODULE LIFETIME SEMANTICS]`).
2. **`RtlGetVersion` Implementation**: Defined `OsVersionInfoExW` layout and implemented `ntdll.dll!RtlGetVersion`, returning `STATUS_SUCCESS (0)` and Windows 10/11 version (`10.0.19045`).
3. **`FreeLibrary` Implementation**: Implemented `FreeLibrary(HMODULE)` with reference counting, returning `1 (TRUE)` on valid modules, protecting the main executable from unmapping, and returning `0` with `ERROR_INVALID_HANDLE (6)` on invalid handles.
4. **Post-Version Execution & Windowing/Vulkan Initialization**: Godot successfully completed version checks, initialized display server window class structures (`LoadIconA`, `RegisterClassExW`, `RegisterRawInputDevices`), initialized Vulkan instance/device (`vkCreateInstance`), and progressed into window hook registration (`USER32.dll!SetWindowsHookExA`).

---

## 2. Verified GDB Execution Trace

```text
=== BEFORE LoadLibraryW at 0x14005abb8 ===
RCX (name): 0x1435de67c: u"ntdll.dll"
[KERNEL32] LoadLibraryW('ntdll.dll')

=== AFTER LoadLibraryW at 0x14005abba ===
RAX (returned HMODULE) = 0x000000007ffd0000

=== BEFORE GetProcAddress at 0x14005abcc ===
RCX (hModule): 0x000000007ffd0000
RDX (symbol):  RtlGetVersion
[RINE GETPROCADDRESS BIND] hmodule=0x000000007FFD0000 module='ntdll.dll' symbol='RtlGetVersion' returned_addr=0x0000555555F3C590 provider=NativeRine

=== AFTER GetProcAddress at 0x14005abd2 ===
RAX (fn ptr) = 0x0000555555f3c590

=== BEFORE calling RtlGetVersion at 0x14005abda ===
RCX (struct ptr): 0x00007fffffff9f60
struct dwOSVersionInfoSize: 276
[NTDLL] RtlGetVersion -> Win 10.0.19045 (size=276)

=== AFTER RtlGetVersion at 0x14005abdc ===
EAX (status) = 0 (0x0)
dwMajorVersion = 10
dwMinorVersion = 0
dwBuildNumber  = 19045

=== BEFORE FreeLibrary at 0x14005abdf ===
RCX (hModule): 0x000000007ffd0000
[KERNEL32] FreeLibrary(0x000000007FFD0000)

=== AFTER FreeLibrary at 0x14005abe5 ===
EAX (result) = 1
```

---

## 3. Downstream Progress & Milestones

Following the completion of the `RtlGetVersion` lifecycle, the target executed:
1. `LoadLibraryW('uxtheme.dll')` -> probed dark theme ordinals `#132`, `#95`, `#96`, `#98`.
2. `LoadLibraryW('wintab32.dll')` -> absent module returned 0, handled cleanly.
3. `LoadLibraryW('user32.dll')` -> resolved `GetPointerType`, `GetPointerPenInfo`, `LogicalToPhysicalPointForPerMonitorDPI`, `PhysicalToLogicalPointForPerMonitorDPI`.
4. `LoadLibraryW('Shcore.dll')` -> resolved `SetProcessDpiAwareness(1)` -> returned `S_OK`.
5. Window Class Registration:
   - `USER32.dll!LoadIconA(hInst = 0, icon = 0x7F05 (IDI_APPLICATION))` -> returned dummy icon handle `0x10010`.
   - `USER32.dll!RegisterClassExW` -> registered `WndProc` at `0x1400628A0`.
   - `USER32.dll!RegisterRawInputDevices(num_devices = 1, cb_size = 16)` -> returned `TRUE (1)`.
6. Vulkan Initialization:
   - `LoadLibraryA('vulkan-1.dll')` -> loaded native Vulkan bridge.
   - `vkGetInstanceProcAddr`, `vkCreateInstance` -> **SUCCESS** with 3 extensions:
     - `VK_KHR_get_physical_device_properties2`
     - `VK_KHR_surface`
     - `VK_KHR_xlib_surface`
   - Instance created: `0x6067d2170b30`.

---

## 4. Current Exact Blocker

- **API**: `USER32.dll!SetWindowsHookExA`
- **Call Instruction VA**: `0x14005BE2E`
- **Return VA**: `0x14005BE34`
- **Caller RVA**: `0x5BE34`
- **IAT Slot VA**: `0x1443A4208`
- **Stub ID**: 170
- **Arguments**:
  - `RCX = 0x7` (`idHook` = `WH_CALLWNDPROC` / `WH_MSGFILTER` / `WH_JOURNALRECORD` / 7)
  - `RDX = 0x140055BC0` (`lpfn` hook procedure)
  - `R8 = 0x0` (`hmod`)
  - `R9 = 0x98A1` (`dwThreadId`)

---

## 5. Test Suite Status

- **115 unit tests passing** (0 failures).
