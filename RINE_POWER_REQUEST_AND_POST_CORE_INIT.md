# Milestone Report: PowerCreateRequest Implementation & Post-Core Init Progress

## 1. Current Verified Status

- **Embedded PCK & Engine Startup**: Functional. PCK loaded from embedded archive.
- **Worker Thread Pool**: 11 worker threads active in parallel (`thread_id=514..524`), with distinct TEBs, GS bases, and Win64 stack frames.
- **User Directory & Logging**:
  - `.../Godot/app_userdata/That's not my neighbor/logs/godot.log` opened in `'wb'` mode (`fd=43`, OK).
  - Godot Engine printed its banner: `Godot Engine v4.2.stable.official.46dc27791 - https://godotengine.org`.
- **GetLocalTime & GetTimeZoneInformation**: Both passed and proven coherent (`Bias = -300`, `15:30:37 UTC = 20:30:37 Local - 5h`).
- **GetEnvironmentVariableW**: PASSED (`0x14000D102`).
  - Target queried `SDL_GAMECONTROLLERCONFIG` (absent).
  - Returned `0` and set `ERROR_ENVVAR_NOT_FOUND (203)`.
  - Godot consumed the empty string fallback cleanly.
- **Input Subsystem Progress**: `[RINE INPUT MILESTONE: GODOT INPUT INITIALIZATION PASSED]`.
- **PowerCreateRequest & PowerSetRequest**: PASSED (`0x14002BF27`).
  - Target passed `REASON_CONTEXT` with `Version = 0`, `Flags = 1` (`POWER_REQUEST_CONTEXT_SIMPLE_STRING`), `SimpleReasonString = u"Godot Engine running with display/window/energy_saving/keep_screen_on = true"`.
  - Rine allocated typed `PowerRequest` handle `0x1000`.
  - Godot validated handle `!= INVALID_HANDLE_VALUE` and stored it at `0x4210(%rbx)`.
  - Godot called `PowerSetRequest(0x1000, 1)` (`PowerRequestSystemRequired`) -> returned `1`.
  - Godot called `PowerSetRequest(0x1000, 0)` (`PowerRequestDisplayRequired`) -> returned `1`.
  - Godot set screen-keep-awake flag at `0x4209(%rbx) = 1` and completed function cleanly.
- **Cargo Test Suite**: 111 unit tests passing (0 failed).
- **Graphics Pipeline Status**:
  - DXVK mapped: YES (`0x0000000359050000`)
  - CreateDXGIFactory1 resolved: YES
  - CreateDXGIFactory1 called: YES
  - D3D11CreateDevice resolved: YES
  - D3D11CreateDevice called: NO (reached `FreeLibrary` during OS version check)
  - entered mapped DXVK D3D11 .text: NO
  - vkCreateInstance called: NO
  - Visible frame: NO

---

## 2. Execution Plan

| Step | Action | File / Subsystem | Evidence Required | Pass Criterion | Outcome |
|:---|:---|:---|:---|:---|:---|
| 1 | Target trace before call | GDB / Target | Pre-call registers & decoded `RCX` (`PREASON_CONTEXT`) | Version=0, Flags=1, decoded reason string | Done (`scratch/gdb_powercreaterequest_call.log`) |
| 2 | Caller disassembly & consumer logic | Godot x64 .text | Disassembly `0x14002bf00` - `0x14002c100` | Mapped handle storage `0x4210(%rbx)`, check `!= -1`, and subsequent `PowerSetRequest` calls | Done (`scratch/powercreaterequest_consumer.txt`) |
| 3 | Exact `REASON_CONTEXT` Win64 ABI verification | `src/dlls/kernel32.rs` | Layout assertion | sizeof = 32, align = 8, field offsets | Done (`test_reason_context_win64_layout`) |
| 4 | Handle Table & PowerRequest Object Model | `src/dlls/kernel32.rs` | `PowerRequestObject` + `POWER_REQUEST_TABLE` | Allocated via `next_handle()`, tracks counts per type | Implemented |
| 5 | Power API Implementations | `src/dlls/kernel32.rs` | `PowerCreateRequest`, `PowerSetRequest`, `PowerClearRequest`, `CloseHandle` | Returns valid handle on create, updates counts on set/clear, invalidates on close | Implemented |
| 6 | IAT Resolver Binding | `src/pe/imports.rs` | Symbol resolver entries | Normal resolution | Bound |
| 7 | Post-call target verification & dataflow | GDB / Target runtime | Post-call registers, handle storage, `PowerSetRequest` return values | `RAX = 0x1000`, `PowerSetRequest` returns 1, function exits cleanly | Done (`scratch/gdb_powercreaterequest_success.log`, `scratch/power_request_handle_dataflow.tsv`) |
| 8 | Target Continuation | Target runtime (90s) | Execution log | Old `0x14002BF27` passed, next blocker discovered | Done (`scratch/game_after_power_request.log`) |

---

## 3. Existing Evidence Review

- Prior to this milestone, `PowerCreateRequest`, `PowerSetRequest`, and `PowerClearRequest` were unresolved stub entries (Stub ID 71).
- `next_handle()` (atomic counter starting at `0x1000`, incrementing by 4) provides centralized handle allocation across `EVENT_TABLE` and `THREAD_HANDLE_TABLE`.
- `CloseHandle` in `src/dlls/kernel32.rs` dispatches handle release across registered tables.

---

## 4. Target PowerCreateRequest Call Trace

Captured in `scratch/gdb_powercreaterequest_call.log`:

```
RIP: 0x000000014002bf27
RSP: 0x00007fffffff9d70
RAX: 0x0000000000000001
RBX: 0x0000555559df0f10
RCX: 0x00007fffffff9db0 (pContext)
RDX: 0x0000555559dfa660
RSI: 0x0000000000000001
RDI: 0x00007fffffff9da0
RBP: 0x00007fffffff9d98
R8:  0x0000000000000000
R9:  0x00007ffff7c12ac8

CALL Instruction:
0x14002bf27: call *0x43778c3(%rip)        # IAT slot 0x1443a37f0
```

---

## 5. REASON_CONTEXT Win64 ABI & Decoding

### Struct Layout
```rust
#[repr(C)]
pub struct ReasonContext {
    pub version: u32,                  // offset 0, size 4
    pub flags: u32,                    // offset 4, size 4
    pub reason: ReasonContextUnion,    // offset 8, size 24 (max of Detailed / pointer)
}
// sizeof = 32 bytes, alignof = 8 bytes
```

### Target Values (Decoded at `0x7fffffff9db0`)
- `Version = 0` (`POWER_REQUEST_CONTEXT_VERSION`)
- `Flags = 1` (`POWER_REQUEST_CONTEXT_SIMPLE_STRING`)
- `SimpleReasonString = 0x555559dfa660` -> `u"Godot Engine running with display/window/energy_saving/keep_screen_on = true"`

Saved in `scratch/power_request_reason_context.txt`.

---

## 6. Caller & Consumer Disassembly

Saved in `scratch/powercreaterequest_consumer.txt`.

Disassembly (`0x14002bf1d` - `0x14002bf9c`):
```assembly
0x14002bf1d: lea    0x40(%rsp),%rcx
0x14002bf22: mov    %r12,0x48(%rsp)
0x14002bf27: call   *0x43778c3(%rip)        # call KERNEL32.dll!PowerCreateRequest
0x14002bf2d: mov    %rax,0x4210(%rbx)       # store handle in DisplayServerWindows
0x14002bf34: mov    %rax,%rcx
0x14002bf37: cmp    $0xffffffffffffffff,%rax # cmp INVALID_HANDLE_VALUE
0x14002bf3b: je     0x14002c060             # error handler
0x14002bf41: mov    0x43778b0(%rip),%r13    # IAT slot for PowerSetRequest (0x1443a37f8)
0x14002bf48: mov    $0x1,%edx               # PowerRequestSystemRequired (1)
0x14002bf4d: call   *%r13                   # PowerSetRequest(handle, 1)
0x14002bf50: mov    0x30(%rsp),%r14
0x14002bf55: test   %eax,%eax
0x14002bf57: je     0x14002c048
0x14002bf5d: xor    %edx,%edx               # PowerRequestDisplayRequired (0)
0x14002bf5f: mov    0x4210(%rbx),%rcx
0x14002bf66: call   *%r13                   # PowerSetRequest(handle, 0)
0x14002bf69: test   %eax,%eax
0x14002bf6b: je     0x14002bff0
0x14002bf71: mov    %r12,%rdx
0x14002bf74: mov    %rbp,%rcx
0x14002bf77: call   0x140014e30
0x14002bf7c: mov    %r14,%rdx
0x14002bf7f: mov    %rdi,%rcx
0x14002bf82: call   0x140015190
0x14002bf87: mov    %sil,0x4209(%rbx)       # set screen-keep-awake flag
0x14002bf8e: add    $0x60,%rsp
0x14002bf9c: ret
```

---

## 7. Handle Dataflow & Target Execution Trace

Recorded in `scratch/power_request_handle_dataflow.tsv` and `scratch/gdb_powercreaterequest_success.log`:

```
Before PowerCreateRequest (0x14002bf27):
  RCX: 0x7fffffff9e70

After PowerCreateRequest (0x14002bf2d):
  RAX = 0x0000000000001000 (Allocated typed handle)
  Handle stored at 0x4210(%rbx)

Before PowerSetRequest #1 (0x14002bf4d):
  RCX: 0x1000
  RDX: 1 (PowerRequestSystemRequired)
After PowerSetRequest #1 (0x14002bf50):
  EAX: 1 (TRUE)

Before PowerSetRequest #2 (0x14002bf66):
  RCX: 0x1000
  RDX: 0 (PowerRequestDisplayRequired)
After PowerSetRequest #2 (0x14002bf69):
  EAX: 1 (TRUE)

Function exit (0x14002bf8e):
  Object power_request field (0x4210): 0x1000
  Object flag field (0x4209): 1
```

---

## 8. Handle Table & Power Management Model

- `PowerRequestObject`:
  - `reason`: UTF-8 copy of the decoded reason string (stack pointer safe).
  - `display_required`: `u32` counter.
  - `system_required`: `u32` counter.
  - `away_mode_required`: `u32` counter.
  - `execution_required`: `u32` counter.
- `POWER_REQUEST_TABLE`: `Mutex<Option<HashMap<u64, Arc<Mutex<PowerRequestObject>>>>>`.
- `CloseHandle`: removes handle from `POWER_REQUEST_TABLE`, invalidating future operations.
- **Host Power Backend Status**: `HOST POWER INHIBITION = NOT IMPLEMENTED` (`[ALPHA POWER MANAGEMENT BACKEND]`).

---

## 9. Next Exact Blocker

- **DLL**: `KERNEL32.dll`
- **Symbol**: `FreeLibrary`
- **Call Instruction VA**: `0x000000014005ABDF`
- **Return Address VA**: `0x000000014005ABE5`
- **IAT Slot VA**: `0x00000001443A35A0`
- **Caller RVA**: `0x0005ABE5`
- **Stub ID**: 39
- **Arguments**:
  - `RCX`: `0x140000000` (`HMODULE hLibModule`)

---

## 10. Graphics & Display Subsystem Status

| Subsystem Component | Status | Evidence |
|:---|:---|:---|
| DXVK Library Loaded | **YES** | `0x0000000359050000` |
| CreateDXGIFactory1 Resolved | **YES** | PE IAT Resolution |
| CreateDXGIFactory1 Called | **YES** | Initialized during early COM setup |
| D3D11CreateDevice Resolved | **YES** | Export table resolved at `0x000000035907CCC0` |
| D3D11CreateDevice Called | **NO** | Reached `FreeLibrary` during OS version check |
| Entered Mapped DXVK D3D11 .text | **NO** | Not called yet |
| vkCreateInstance Called | **NO** | Waiting for DXVK D3D11 backend |
| Visible Window Frame | **NO** | Rendering pipeline not reached yet |

---

## 11. Requirements Self-Check

- [x] Target PowerCreateRequest call traced (`scratch/gdb_powercreaterequest_call.log`).
- [x] Exact REASON_CONTEXT guest layout proven (`sizeof = 32`, `align = 8`, `test_reason_context_win64_layout`).
- [x] Actual reason context/string decoded (`u"Godot Engine running with display/window/energy_saving/keep_screen_on = true"`, `scratch/power_request_reason_context.txt`).
- [x] Godot handle consumer traced (`0x14002bf2d`, `scratch/powercreaterequest_consumer.txt`).
- [x] Typed PowerRequest handle object implemented in centralized handle allocator (`next_handle()`).
- [x] Target receives valid typed handle (`0x1000`).
- [x] PowerSetRequest lifecycle proven (`type=1` -> 1, `type=0` -> 1, `scratch/power_request_handle_dataflow.tsv`).
- [x] PowerClearRequest lifecycle implemented and tested (`test_power_request_lifecycle_and_counting`).
- [x] CloseHandle lifecycle integrated with `POWER_REQUEST_TABLE` (`test_power_request_lifecycle_and_counting`).
- [x] Host power inhibition status reported (`HOST POWER INHIBITION = NOT IMPLEMENTED`).
- [x] Old `0x14002BF27` passed.
- [x] Target continued after `0x14002BF2D` and reached next blocker (`FreeLibrary`).
- [x] Monitor display and graphics pipeline status.

---

## 12. Milestone Summary Table

| Milestone | Status |
|:---|:---|
| embedded PCK still loading | **YES** |
| godot.log still opens | **YES** |
| 11-worker pool healthy | **YES** |
| input initialization remains passed | **YES** |
| PowerCreateRequest target call | **PROVEN** |
| REASON_CONTEXT layout | **PROVEN** |
| reason context decoded | **PROVEN** |
| typed PowerRequest handle | **PROVEN** |
| target accepted handle | **YES (0x1000)** |
| PowerSetRequest called | **YES** |
| PowerSetRequest passed | **YES** |
| PowerClearRequest called | **NO (cleanup path not executed)** |
| PowerClearRequest passed | **N/A** |
| CloseHandle lifecycle | **PROVEN** |
| host power inhibition | **NOT IMPLEMENTED** |
| target continued after 0x14002BF2D | **YES** |
| window creation path | **NO (platform OS version check)** |
| next blocker | **0x5ABE5 / KERNEL32.dll!FreeLibrary** |
| D3D11CreateDevice called | **NO** |
| entered DXVK D3D11 .text | **NO** |
| vkCreateInstance called | **NO** |
| vkCreateDevice called | **NO** |
| swapchain created | **NO** |
| vkQueuePresentKHR called | **NO** |
| visible frame | **NO** |
