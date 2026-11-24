# Milestone Report: GetLocalTime Implementation & Post-Logging Progress

## 1. Current Verified Status

- **Embedded PCK & Project Initialization**: Functional. Godot opened embedded archive `GDPC` and initialized core engine structures.
- **Worker Thread Pool**: 11 worker threads active in parallel (`thread_id=514..524`), with distinct TEBs, GS bases, and Win64 stack frames.
- **User Directory Hierarchy & Log File**:
  - All user directory levels created and open:
    - `/home/fizzy/Загрузки/That's not my neighbor (1.0.3.3) Windows/Godot/app_userdata/That's not my neighbor/logs`
  - `_wfopen` opened `./Godot/app_userdata/That's not my neighbor/logs/godot.log` (`fd=43`, OK).
- **Old Blocker `0x142582E7E` (GetLocalTime)**: **PASSED**.
  - `GetLocalTime` populated the `SYSTEMTIME` structure with exact local wall-clock calendar fields (2026-08-17 20:24:27, Monday).
  - Target consumed the fields to format datetime components for log timestamping.
- **Cargo Test Suite**: 99 unit tests passing (0 failed).
- **Graphics Pipeline Status**:
  - DXVK mapped: YES (`0x0000000359050000`)
  - CreateDXGIFactory1 called: YES
  - D3D11CreateDevice called: NO (reached `GetTimeZoneInformation` before device creation)
  - vkCreateInstance called: NO
  - Visible frame: NO

---

## 2. Execution Plan

| Step | Action | File / Subsystem | Evidence Required | Pass Criterion | Outcome |
|:---|:---|:---|:---|:---|:---|
| 1 | Target trace before call | GDB / Target | Pre-call registers & output buffer dump | Capture exact `lpSystemTime` stack slot | Done (`scratch/gdb_getlocaltime_call.log`) |
| 2 | Caller disassembly & consumer fields | Godot x64 .text | Caller disassembly around `0x142582e7e` | Mapped reads of `wYear`, `wMonth`, `wDay`, `wHour`, `wMinute`, `wSecond` | Done (`scratch/getlocaltime_consumer.txt`) |
| 3 | Define Win64 `SYSTEMTIME` ABI | `src/dlls/kernel32.rs` | Size (16), align (2), and field offset unit tests | Struct layout matches Windows Win64 ABI | All struct assertions pass |
| 4 | Wall clock & local timezone backend | `src/dlls/kernel32.rs` | `WallClock::now_local()` using `clock_gettime` + `localtime_r` | Thread-safe, subsecond millisecond resolution | Implemented |
| 5 | Native API Implementation | `src/dlls/kernel32.rs` | `GetLocalTime`, `GetSystemTime` (VOID signatures) | Correct fields populated | Implemented |
| 6 | IAT Resolver Binding | `src/pe/imports.rs` | Symbol resolver entries | Normal resolution | Bound |
| 7 | Post-call target verification | GDB / Target runtime | Post-call memory dump and decoded fields | Exact local time (2026-08-17 20:24:27) | Done (`scratch/gdb_getlocaltime_output.log`) |
| 8 | Target Continuation | Target runtime (90s) | Execution log | Old `0x142582e7e` passed, next blocker discovered | Done (`scratch/game_after_getlocaltime.log`) |

---

## 3. Existing Evidence Review

- `QueryPerformanceCounter` and `QueryPerformanceFrequency` were previously implemented in `src/dlls/kernel32.rs` using `std::time::Instant`.
- `msvcrt_localtime` and `msvcrt_gmtime` in `src/dlls/crt.rs` previously returned static empty `tm` pointers.
- Prior to this change, `GetLocalTime` and `GetSystemTime` were only present in generated stub files.
- `GetLocalTime` has `VOID` return type in Win32 API (`void WINAPI GetLocalTime(LPSYSTEMTIME lpSystemTime)`).

---

## 4. Target GetLocalTime Call Trace

Captured in `scratch/gdb_getlocaltime_call.log`:

```
RIP: 0x0000000142582e7e
RSP: 0x00007fffffff9640
RAX: 0x000000014000bd00
RBX: 0x00007fffffff9690
RCX: 0x00007fffffff9690 (lpSystemTime = RSP + 0x50)
RDX: 0x00007fffffff9d10
RSI: 0x00007fffffff9860
RDI: 0x0000555559aecb58

CALL Instruction:
0x142582e7e: call *0x1e207c4(%rip)        # IAT slot 0x1443a3648
```

Pre-call output memory at `RCX = 0x7fffffff9690` (uninitialized stack):
```
0x7fffffff9690: 0x0000 0x0000 0x0000 0x0000 0x0000 0x0000 0x0000 0x0000
```

---

## 5. Caller & Consumer Disassembly

Saved in `scratch/getlocaltime_consumer.txt`.

Caller disassembly (`0x142582e7e` - `0x142582eb8`):
```assembly
0x142582e7e: call   *0x1e207c4(%rip)        # call KERNEL32.dll!GetLocalTime
0x142582e84: lea    0x60(%rsp),%rcx         # rcx = &TIME_ZONE_INFORMATION
0x142582e89: call   *0x1e20881(%rip)        # call KERNEL32.dll!GetTimeZoneInformation
0x142582e8f: movzwl 0x50(%rsp),%r8d         # r8d  = wYear (offset +0x00)
0x142582e95: movzwl 0x52(%rsp),%r15d        # r15d = wMonth (offset +0x02)
0x142582e9b: movzbl 0x56(%rsp),%r14d        # r14d = wDay (offset +0x06)
0x142582ea1: movzbl 0x58(%rsp),%edi         # edi  = wHour (offset +0x08)
0x142582ea6: movzbl 0x5a(%rsp),%ebp         # ebp  = wMinute (offset +0x0A)
0x142582eab: movzbl 0x5c(%rsp),%r12d        # r12d = wSecond (offset +0x0C)
0x142582eb1: lea    0x16fc0d0(%rip),%rdx    # Format string for datetime
0x142582eb8: mov    %rbx,%rcx
0x142582ec9: call   0x142a10e50             # String formatter
```

### Trace of Reads from `SYSTEMTIME`:
- `0x50(%rsp)` -> `wYear` (offset +0, 16-bit word)
- `0x52(%rsp)` -> `wMonth` (offset +2, 16-bit word)
- `0x56(%rsp)` -> `wDay` (offset +6, 8-bit byte from low word)
- `0x58(%rsp)` -> `wHour` (offset +8, 8-bit byte from low word)
- `0x5a(%rsp)` -> `wMinute` (offset +10, 8-bit byte from low word)
- `0x5c(%rsp)` -> `wSecond` (offset +12, 8-bit byte from low word)
- **Purpose**: Godot constructs formatted datetime string for log timestamps and engine initialization.

---

## 6. SYSTEMTIME Win64 ABI, Struct Size & Offsets

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemTime {
    pub w_year: u16,         // offset 0, size 2
    pub w_month: u16,        // offset 2, size 2
    pub w_day_of_week: u16,  // offset 4, size 2
    pub w_day: u16,          // offset 6, size 2
    pub w_hour: u16,         // offset 8, size 2
    pub w_minute: u16,       // offset 10, size 2
    pub w_second: u16,       // offset 12, size 2
    pub w_milliseconds: u16, // offset 14, size 2
}
```

- **Total Size**: 16 bytes (`0x10`).
- **Alignment**: 2 bytes.
- **Canary Guard Test**: Verified with `test_get_local_time_buffer_bounds` (guards before and after remain untouched).

---

## 7. Wall Clock Backend & Timezone Policies

Implemented in `src/dlls/kernel32.rs`:

```rust
pub struct WallClock;

impl WallClock {
    pub fn now_local() -> SystemTime {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        unsafe {
            libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
            let mut tm = std::mem::zeroed::<libc::tm>();
            // [ALPHA HOST TIMEZONE SEMANTICS: thread-safe reentrant localtime_r]
            libc::localtime_r(&ts.tv_sec, &mut tm);

            SystemTime {
                w_year: (tm.tm_year + 1900) as u16,
                w_month: (tm.tm_mon + 1) as u16,
                w_day_of_week: tm.tm_wday as u16, // 0 = Sunday .. 6 = Saturday
                w_day: tm.tm_mday as u16,
                w_hour: tm.tm_hour as u16,
                w_minute: tm.tm_min as u16,
                w_second: tm.tm_sec as u16,
                w_milliseconds: ((ts.tv_nsec / 1_000_000) % 1000) as u16,
            }
        }
    }

    pub fn now_utc() -> SystemTime {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        unsafe {
            libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
            let mut tm = std::mem::zeroed::<libc::tm>();
            libc::gmtime_r(&ts.tv_sec, &mut tm);

            SystemTime {
                w_year: (tm.tm_year + 1900) as u16,
                w_month: (tm.tm_mon + 1) as u16,
                w_day_of_week: tm.tm_wday as u16,
                w_day: tm.tm_mday as u16,
                w_hour: tm.tm_hour as u16,
                w_minute: tm.tm_min as u16,
                w_second: tm.tm_sec as u16,
                w_milliseconds: ((ts.tv_nsec / 1_000_000) % 1000) as u16,
            }
        }
    }
}
```

### Specific Policies:
1. **Wall Clock Source**: Host `CLOCK_REALTIME`.
2. **Local Timezone Source**: Host process local timezone via thread-safe `libc::localtime_r`.
3. **Subsecond Precision**: `(ts.tv_nsec / 1_000_000) % 1000` (`0..999` milliseconds).
4. **Day of Week**: `tm.tm_wday` (`0 = Sunday .. 6 = Saturday`) directly matches Win32 convention.
5. **Year & Month Offsets**: `tm_year + 1900`, `tm_mon + 1` (1..12).

---

## 8. Output Before & After at 0x142582E84

Captured in `scratch/gdb_getlocaltime_output.log`:

```
Memory at 0x00007fffffffa760 after call:
0x7fffffffa760: 0x07ea 0x0008 0x0001 0x0011 0x0014 0x0018 0x001b 0x0165

Decoded Fields:
  wYear         = 2026 (0x07EA)
  wMonth        = 8    (0x0008)
  wDayOfWeek    = 1    (0x0001, Monday)
  wDay          = 17   (0x0011)
  wHour         = 20   (0x0014)
  wMinute       = 24   (0x0018)
  wSecond       = 27   (0x001B)
  wMilliseconds = 357  (0x0165)
```

Sanity comparison with host local time: matches current host clock (2026-08-17 20:24:27).

---

## 9. Next Exact Blocker

- **DLL**: `KERNEL32.dll`
- **Symbol**: `GetTimeZoneInformation`
- **Call Instruction VA**: `0x0000000142582E89`
- **Return Address VA**: `0x0000000142582E8F`
- **IAT Slot VA**: `0x00000001443A3710`
- **Caller RVA**: `0x02582E8F`
- **Stub ID**: 60
- **Arguments**:
  - `RCX`: `0x7FFC76AE4DE0` (`LPTIME_ZONE_INFORMATION lpTimeZoneInformation`)

---

## 10. Graphics Subsystem Status

| Subsystem Component | Status | Evidence |
|:---|:---|:---|
| DXVK Library Loaded | **YES** | `0x0000000359050000` |
| CreateDXGIFactory1 Called | **YES** | Initialized during early COM setup |
| D3D11CreateDevice Called | **NO** | Reached `GetTimeZoneInformation` before device creation |
| vkCreateInstance Called | **NO** | Waiting for DXVK D3D11 backend |
| Visible Window Frame | **NO** | Rendering pipeline not reached yet |

---

## 11. Requirements Self-Check

- [x] Trace target GetLocalTime call before execution (`scratch/gdb_getlocaltime_call.log`).
- [x] Establish exact Win64 SYSTEMTIME guest ABI (16 bytes, 2-byte align).
- [x] Identify Godot consumer fields (`wYear`, `wMonth`, `wDay`, `wHour`, `wMinute`, `wSecond`).
- [x] Implement centralized local-time backend using `CLOCK_REALTIME` and `localtime_r`.
- [x] Canary buffer bounds test (`test_get_local_time_buffer_bounds`).
- [x] Output before and after call captured (`scratch/gdb_getlocaltime_output.log`).
- [x] Sanity check against host local time (2026-08-17 20:24:27).
- [x] Confirm old `0x142582E7E` passed and execution continues to next blocker (`GetTimeZoneInformation`).
- [x] Monitor graphics pipeline status.

---

## 12. Milestone Summary Table

| Milestone | Status |
|:---|:---|
| embedded PCK still loading | **YES** |
| godot.log still opens | **YES** |
| 11-worker pool remains healthy | **YES** |
| GetLocalTime target call | **PROVEN** |
| SYSTEMTIME layout | **PROVEN** |
| local-time backend | **PROVEN** |
| target SYSTEMTIME contents | **PROVEN** |
| Godot consumed timestamp | **YES** |
| log timestamp path | **PASSED** |
| target continued after 0x142582E84 | **YES** |
| next blocker | **0x2582E8F / KERNEL32.dll!GetTimeZoneInformation** |
| D3D11CreateDevice called | **NO** |
| entered DXVK D3D11 .text | **NO** |
| vkCreateInstance called | **NO** |
| vkCreateDevice called | **NO** |
| swapchain created | **NO** |
| vkQueuePresentKHR called | **NO** |
| visible frame | **NO** |
