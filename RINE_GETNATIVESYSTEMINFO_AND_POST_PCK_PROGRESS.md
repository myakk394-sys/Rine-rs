# Milestone Report: GetNativeSystemInfo Implementation & Post-PCK Progression

## 1. Current Verified Status

- **Embedded PCK Loading**: Verified functional. Godot successfully discovers, opens, and indexes the embedded `GDPC` archive inside `That's not my neighbor.exe` (344,672,496 bytes).
- **Old Blocker `0x14000AD10`**: Passed. `GetNativeSystemInfo` successfully executed, populated the `SYSTEM_INFO` structure, and Godot consumed `dwNumberOfProcessors`.
- **Worker Thread Pool**: Godot spawned 11 worker threads based on the returned logical CPU count (12 total CPUs), each with a distinct TEB, GS base, and Win64 stack alignment.
- **Cargo Test Suite**: 95 tests passing (0 failed).
- **Graphics Pipeline**:
  - DXVK mapped: YES (`0x0000000359050000`)
  - CreateDXGIFactory1 called: YES
  - D3D11CreateDevice called: NO (reached `CreateDirectoryW` before device creation)
  - vkCreateInstance called: NO
  - Visible frame: NO

---

## 2. Execution Plan

| Step | Action | File / Subsystem | Evidence Required | Pass Criterion | Hypothesis to Prove |
|:---|:---|:---|:---|:---|:---|
| 1 | Target trace before call | GDB / Runtime | Pre-call registers & memory dump | Capture exact stack slot and caller context | Done (`scratch/gdb_getnativesysteminfo_call.log`) |
| 2 | Caller & consumer disassembly | Godot x64 .text | Instruction disassembly around `0x14000ad10` | Exact field read offset and width identified | `0x40(%rsp)` -> `dwNumberOfProcessors` read at `+0x20` |
| 3 | Define Win64 `SYSTEM_INFO` ABI | `src/dlls/kernel32.rs` | Size (48), align (8), and field offset unit tests | Struct layout matches Windows Win64 ABI | All struct assertions pass |
| 4 | Central System Info Model | `src/dlls/kernel32.rs` | `GuestSystemInfo` struct & constructor | Coherent guest values across architecture, page size, CPU count | Coherent with `PAGE_SIZE` (4096) and CPU topology |
| 5 | Native API Implementation | `src/dlls/kernel32.rs` | `GetNativeSystemInfo`, `GetSystemInfo` | VOID return signature, memory correctly populated | Output matches expected fields |
| 6 | IAT Resolver Binding | `src/pe/imports.rs` | Symbol resolver entries | Normal resolution (not unresolved stub) | Bound in process memory |
| 7 | Post-call target verification | GDB / Target runtime | Post-call memory dump and consumer step | Memory populated, Godot executes `mov 0x40(%rsp), %eax` | Done (`scratch/gdb_getnativesysteminfo_output.log`) |
| 8 | Target Continuation | Target runtime (90s) | Execution log | Old `0x14000ad10` passed, next blocker discovered | Done (`scratch/game_after_getnativesysteminfo.log`) |

---

## 3. Existing Evidence Review

- Prior to this implementation, `GetNativeSystemInfo` and `GetSystemInfo` were only present in generated stubs (`src/dlls/generated/kernel32.rs`), which returned stub values without filling memory.
- `PAGE_SIZE` in Rine is centrally defined in `src/sys/page.rs` as `4096`.
- `VirtualAlloc` and memory mapping subsystems operate on 4096-byte pages.
- There was no centralized `GuestSystemInfo` model; processor topology was queried ad-hoc.
- `GetNativeSystemInfo` has return type `VOID` in Win32 API (`void WINAPI GetNativeSystemInfo(LPSYSTEM_INFO lpSystemInfo)`), so `RAX` is ignored by callers.

---

## 4. Target GetNativeSystemInfo Call

Captured at `0x14000AD10` in `scratch/gdb_getnativesysteminfo_call.log`:

```
RIP: 0x000000014000ad10
RSP: 0x00007fffffffa3c0
RAX: 0x0000000000000001
RBX: 0x00000001441ecb50
RCX: 0x00007fffffffa3e0 (lpSystemInfo = RSP + 0x20)
RDX: 0x0000555559893570
RSI: 0x00007fffffffa770
RDI: 0x00000000ffffffff
RBP: 0x00007fffffffa678
R8:  0x0000000000000002
R9:  0x000000000000002f

CALL Instruction:
0x14000ad10: call *0x4398982(%rip)        # IAT slot 0x1443a3698
```

Pre-call output memory at `RCX = 0x7fffffffa3e0` (uninitialized stack):
```
0x7fffffffa3e0: 0xd7486170 0x00007fff 0x00000000 0x00000000
0x7fffffffa3f0: 0xd7488570 0x00007fff 0x00000000 0x00000000
0x7fffffffa400: 0x00000000 0x00000000 0x00000000 0x00000000
```

---

## 5. Caller & Consumer Disassembly

Saved in `scratch/getnativesysteminfo_consumer.txt`.

Caller disassembly (`0x14000acf8` - `0x14000ad28`):
```assembly
0x14000acf8: lea    0x41e1e51(%rip),%rbx        # 0x1441ecb50 (&lock)
0x14000acff: mov    %rbx,%rcx
0x14000ad02: call   0x1435446d0                 # acquire_lock
0x14000ad07: test   %eax,%eax
0x14000ad09: je     0x14000ace6
0x14000ad0b: lea    0x20(%rsp),%rcx             # rcx = &SYSTEM_INFO (0x20 bytes into stack frame)
0x14000ad10: call   *0x4398982(%rip)            # call KERNEL32.dll!GetNativeSystemInfo
0x14000ad16: mov    0x40(%rsp),%eax             # Read DWORD at offset +0x20 (dwNumberOfProcessors)
0x14000ad1a: mov    %rbx,%rcx
0x14000ad1d: mov    %eax,0x41e1e29(%rip)        # Store CPU count into 0x1441ecb4c
0x14000ad23: call   0x143544810                 # release_lock
0x14000ad28: jmp    0x14000ace6
0x14000ace6: mov    0x41e1e60(%rip),%eax        # Return CPU count in EAX
0x14000acec: add    $0x50,%rsp
0x14000acf0: pop    %rbx
0x14000acf1: ret
```

### Trace of Reads from `SYSTEM_INFO`:
- **Read VA**: `0x14000ad16`
- **Offset from base**: `0x40(%rsp) - 0x20(%rsp) = +0x20` (+32 bytes)
- **Access width**: 32-bit (`mov %eax, ...`)
- **Decoded Field**: `dwNumberOfProcessors`
- **Consumer Usage**: Stores logical processor count into Godot's global CPU count and uses it to instantiate the worker thread pool.

---

## 6. SYSTEM_INFO Win64 ABI, Struct Size & Offsets

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemInfo {
    pub w_processor_architecture: u16,      // offset 0,  size 2
    pub w_reserved: u16,                    // offset 2,  size 2
    pub dw_page_size: u32,                  // offset 4,  size 4
    pub lp_minimum_application_address: u64,// offset 8,  size 8
    pub lp_maximum_application_address: u64,// offset 16, size 8
    pub dw_active_processor_mask: u64,      // offset 24, size 8
    pub dw_number_of_processors: u32,       // offset 32, size 4 (0x20)
    pub dw_processor_type: u32,             // offset 36, size 4 (0x24)
    pub dw_allocation_granularity: u32,     // offset 40, size 4 (0x28)
    pub w_processor_level: u16,             // offset 44, size 2 (0x2C)
    pub w_processor_revision: u16,          // offset 46, size 2 (0x2E)
}
```

- **Total Size**: 48 bytes (`0x30`).
- **Alignment**: 8 bytes.
- **Canary Guard Test**: Verified with `test_get_native_system_info_buffer_bounds` (guards before and after remain unchanged).

---

## 7. Central System Info Model & Field Policies

Defined in `src/dlls/kernel32.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestSystemInfo {
    pub architecture: u16,
    pub page_size: u32,
    pub min_app_address: u64,
    pub max_app_address: u64,
    pub active_processor_mask: u64,
    pub processor_count: u32,
    pub processor_type: u32,
    pub allocation_granularity: u32,
    pub processor_level: u16,
    pub processor_revision: u16,
}
```

### Specific Field Policies:
1. **Processor Architecture**: `PROCESSOR_ARCHITECTURE_AMD64 = 9` (native x86-64 guest).
2. **Page Size**: `4096` (`0x1000`) matching `crate::sys::PAGE_SIZE`.
3. **Allocation Granularity**: `65536` (`0x10000`, 64 KB). Labeled `[ALPHA VIRTUAL MEMORY GRANULARITY SEMANTICS]`.
4. **Processor Count**: Detected via `std::thread::available_parallelism()` (returns 12 on host).
5. **Active Processor Mask**: `(1 << processor_count) - 1` (capped at 64-bit mask `0xFFF` for 12 CPUs).
6. **Address Range**:
   - `lpMinimumApplicationAddress`: `0x0000000000010000` (64 KB).
   - `lpMaximumApplicationAddress`: `0x00007FFFFFFEFFFF` (standard user-space address ceiling in Win64).
7. **Processor Type / Level / Revision**:
   - `dwProcessorType`: `PROCESSOR_AMD_X8664 = 8664`.
   - `wProcessorLevel`: `6`.
   - `wProcessorRevision`: `0`.

---

## 8. Implementation & Resolver Binding

Implemented in `src/dlls/kernel32.rs`:
```rust
#[no_mangle]
pub unsafe extern "win64" fn GetNativeSystemInfo(lp_system_info: *mut SystemInfo) {
    if lp_system_info.is_null() {
        return;
    }
    let info = GuestSystemInfo::current().to_system_info();
    *lp_system_info = info;
}

#[no_mangle]
pub unsafe extern "win64" fn GetSystemInfo(lp_system_info: *mut SystemInfo) {
    GetNativeSystemInfo(lp_system_info);
}
```

Bound in `src/pe/imports.rs`:
```rust
"GetNativeSystemInfo" => {
    Some(dlls::kernel32::GetNativeSystemInfo as *const () as usize)
}
"GetSystemInfo" => {
    Some(dlls::kernel32::GetSystemInfo as *const () as usize)
}
```

---

## 9. Output Before & After at 0x14000AD16

Captured in `scratch/gdb_getnativesysteminfo_output.log`:

```
Memory at 0x7fffffffaef0 after call:
0x7fffffffaef0:  0x00000009  0x00001000  0x00010000  0x00000000
0x7fffffffaf00:  0xfffeffff  0x00007fff  0x00000fff  0x00000000
0x7fffffffaf10:  0x0000000c  0x000021d8  0x00010000  0x00000006

Decoded Fields:
  wProcessorArchitecture      = 9 (0x0009, PROCESSOR_ARCHITECTURE_AMD64)
  wReserved                   = 0 (0x0000)
  dwPageSize                  = 4096 (0x00001000)
  lpMinimumApplicationAddress = 0x0000000000010000
  lpMaximumApplicationAddress = 0x00007ffffffeffff
  dwActiveProcessorMask       = 0x0000000000000fff
  dwNumberOfProcessors        = 12 (0x0000000c)
  dwProcessorType             = 8664 (0x000021d8)
  dwAllocationGranularity     = 65536 (0x00010000)
  wProcessorLevel             = 6 (0x0006)
  wProcessorRevision          = 0 (0x0000)
```

---

## 10. Godot Consumer Proof & Target Continuation

1. **Consumer Execution**:
   - Instruction `0x14000ad16: mov 0x40(%rsp), %eax` executed.
   - Read `dwNumberOfProcessors = 12` into `EAX`.
   - Stored into global `0x1441ecb4c`.
   - Returned `12` from `OS_Windows::get_processor_count()`.
2. **Worker Thread Spawning**:
   - Godot spawned 11 background worker threads: `thread_id=514, 515, 516, 517, 518, 519, 520, 521, 522, 523, 524`.
   - Each worker initialized with independent TEB and Win64 GS base.
3. **Execution Continuation**:
   - Godot continued past `GetNativeSystemInfo` and initialized user data access.

---

## 11. Technical Debt & Semantic Limitations

- `_wfsopen`: `[ALPHA CRT FILE SHARING SEMANTICS]` (delegates to `_wfopen`).
- Initial CWD: `[ALPHA DEFAULT CWD POLICY: EXE DIRECTORY]`.
- Virtual Memory Granularity: `[ALPHA VIRTUAL MEMORY GRANULARITY SEMANTICS]`.

---

## 12. Next Exact Blocker

- **DLL**: `KERNEL32.dll`
- **Symbol**: `CreateDirectoryW`
- **Call Instruction VA**: `0x0000000140AA4975`
- **Return Address VA**: `0x0000000140AA497B`
- **IAT Slot VA**: `0x00000001443A3508`
- **Stub ID**: 29
- **Caller RVA**: `0xAA497B`
- **Arguments**:
  - `RCX`: `0x64FC8175C8F0` (`LPCWSTR lpPathName`)
  - `RDX`: `0x0` (`LPSECURITY_ATTRIBUTES lpSecurityAttributes`)

---

## 13. Graphics Subsystem Status

| Subsystem Component | Status | Evidence |
|:---|:---|:---|
| DXVK Library Loaded | **YES** | `0x0000000359050000` |
| CreateDXGIFactory1 Called | **YES** | Initialized during early COM setup |
| D3D11CreateDevice Called | **NO** | Reached `CreateDirectoryW` before device creation |
| vkCreateInstance Called | **NO** | Waiting for DXVK D3D11 backend |
| Visible Window Frame | **NO** | Rendering pipeline not reached yet |

---

## 14. Requirements Self-Check

- [x] Trace target GetNativeSystemInfo call before execution (`scratch/gdb_getnativesysteminfo_call.log`).
- [x] Establish exact x64 guest SYSTEM_INFO ABI/layout (48 bytes, 8-byte align).
- [x] Determine which SYSTEM_INFO fields Godot actually consumes (`dwNumberOfProcessors` at offset `+0x20`).
- [x] Establish a centralized Rine guest system-information model (`GuestSystemInfo`).
- [x] Implement GetNativeSystemInfo with coherent x86-64 guest values (VOID return type).
- [x] Struct bounds test with canary guards (`test_get_native_system_info_buffer_bounds`).
- [x] Prove target consumes the structure correctly (`scratch/gdb_getnativesysteminfo_output.log`).
- [x] Continue to the first new exact blocker (`KERNEL32.dll!CreateDirectoryW` at `0x140AA4975`).
- [x] Monitor graphics pipeline status (`D3D11CreateDevice` not called yet).

---

## 15. Milestone Summary Table

| Milestone | Status |
|:---|:---|
| embedded PCK loading still works | **YES** |
| old exit(255) remains passed | **YES** |
| SYSTEM_INFO layout | **PROVEN** |
| target call traced | **PROVEN** |
| Godot consumer | **PROVEN** |
| central system-info model | **PROVEN** |
| target structure contents | **PROVEN** |
| target continued after 0x140000AD16 | **YES** |
| next blocker | **0xAA497B / KERNEL32.dll!CreateDirectoryW** |
| D3D11CreateDevice called | **NO** |
| entered DXVK d3d11 .text | **NO** |
| vkCreateInstance called | **NO** |
| swapchain created | **NO** |
| vkQueuePresentKHR called | **NO** |
| visible frame | **NO** |
