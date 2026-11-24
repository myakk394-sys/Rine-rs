# Rine Guest Worker Thread & Next Blocker Report

## CURRENT STATUS

- **Secondary Guest Windows Thread**: **`FULLY PROVEN & EXECUTING`**
  - Godot Engine invokes `_beginthreadex` at VA `0x142586963` (RVA `0x02586963`).
  - Rine returns valid non-zero thread HANDLE `0x2003` in `RAX`.
  - Godot receives thread ID `512` (`0x200`) in `*thrdaddr`.
  - Old `ud2` panic trap at `0x1425869DE` is **100% PASSED / BYPASSED**.
  - Worker guest start routine `0x14255EB40` is reached and executes in a dedicated OS thread.
  - Worker receives independent Win64 stack (`RSP % 16 = 8` on guest entry), distinct TEB (`0x00007FFFD9D4E000`), matched `%gs` base register, thread-local TLS, and thread-local `LastError`.
- **Target Game Execution**: Proceeded past thread creation into Godot engine core initialization (`GetModuleFileNameW` -> string conversion -> subsystem loader dispatch).
- **New Real Blocker Identified**: Unhandled NULL virtual function dispatch at `0x1429ad3b2` (RVA `0x029AD3B2`), where Godot executes `call *(%rax)` on a resource loader / subsystem handler table containing `0x0`.
- **Test Suite**: `cargo fmt --check && cargo check && cargo test` (**67/67 PASSED, 100%**).

---

## _BEGINTHREAD VS _BEGINTHREADEX

- **Signature & ABI Separation**:
  - `_beginthread`:
    - Signature: `uintptr_t _beginthread(void (*start_address)(void *), unsigned stack_size, void *arglist);`
    - Calling convention: Win64 (`RCX = start_address`, `RDX = stack_size`, `R8 = arglist`).
    - Failure return value: `-1L` (`usize::MAX`).
    - Implemented as `msvcrt_beginthread` in `src/dlls/crt.rs`.
  - `_beginthreadex`:
    - Signature: `uintptr_t _beginthreadex(void *security, unsigned stack_size, unsigned (*start_address)(void *), void *arglist, unsigned initflag, unsigned *thrdaddr);`
    - Calling convention: Win64 (`RCX = security`, `RDX = stack_size`, `R8 = start_address`, `R9 = arglist`, `[RSP+0x20] = initflag`, `[RSP+0x28] = thrdaddr`).
    - Failure return value: `0` (`NULL`).
    - Implemented as `msvcrt_beginthreadex` in `src/dlls/crt.rs`.
- **Import Resolution**: `src/pe/imports.rs` binds `"_beginthread"` / `"_o__beginthread"` to `msvcrt_beginthread` and `"_beginthreadex"` / `"_o__beginthreadex"` to `msvcrt_beginthreadex` as distinct function pointers.

---

## TARGET _BEGINTHREADEX CALL

Evidence recorded in `scratch/gdb_target_beginthreadex_call.log`:

```text
=== GDB SNAPSHOT AT _beginthreadex CALL ===
RIP: 0x0000000142586963 (Caller VA) | RVA: 0x02586963
RSP: 0x00007fffffffaf60 (RSP % 16 = 0)
RCX (security):      0x0000000000000000 (NULL)
RDX (stack_size):    0x0000000000000000 (0 -> default 2MB stack)
R8  (start_address): 0x000000014255eb40
R9  (arglist):       0x0000555559955b80
Stack Arg 5 (initflag): 0x00000000
Stack Arg 6 (thrdaddr): 0x00007fffffffaf9c
*thrdaddr BEFORE call: 0x00000000
IAT slot [0x1443a3a48]: 0x0000555555f05900 (Resolved target: msvcrt_beginthreadex)
Disassembly:
=> 0x142586963: call *0x1e1d0df(%rip) # 0x1443a3a48
   0x142586969: test %rax,%rax
   0x14258696c: je   0x1425869d1
   0x14258696e: cmpq $0x0,0x8(%rbx)
   0x142586973: mov  0x3c(%rsp),%edx
```

---

## THREAD HANDLE

- **Thread Handle Value**: `0x0000000000002003` (Non-zero Win32-compatible handle format).
- **Return Register**: `RAX = 0x0000000000002003`.
- **Godot Verification**: At `0x142586969`, Godot executes `test %rax, %rax` -> non-zero -> jumps over `0x1425869d1` (assert trap) and stores `RAX` to `0x8(%rbx)`.

---

## THREAD ID

- **Allocated Thread ID**: `512` (`0x00000200`).
- **Host Thread LWP**: `26616`.

---

## THRDADDR

- **Address Passed by Godot**: `0x00007fffffffaf9c` (located at `0x3c(%rsp)` relative to Godot's frame).
- **`*thrdaddr` Before Call**: `0x00000000`.
- **`*thrdaddr` After Call**: `0x00000200` (`512`).
- **Verified Difference**: `*thrdaddr` strictly receives the 32-bit Thread ID (`512`), **NOT** the 64-bit Thread Handle (`0x2003`).

---

## GUEST START ROUTINE

- **Guest Start Address**: `0x000000014255EB40` (RVA `0x0255EB40`).
- **Breakpoint Proof**: Breakpoint at `0x14255EB40` successfully hit!
- **Runtime Snapshot**:
  ```text
  [RINE GUEST THREAD START] guest_start_va=0x000000014255EB40 guest_start_rva=0x00000000 thread_handle=0x0000000000002003 thread_id=512 host_thread_id=26616 argument=0x0000555559955B80 gs_base=0x00007FFFD9D4E000 teb=0x00007FFFD9D4E000 match=yes

  === GDB SNAPSHOT AT GUEST START ROUTINE ENTRY ===
  RIP: 0x000000014255eb40
  RSP: 0x00007fffcca81868 (RSP % 16 = 8)
  RCX (arglist): 0x0000555559955b80
  Return address at [RSP]: 0x0000555555f1a7ef
  Disassembly:
  => 0x14255eb40: push   %rbx
     0x14255eb41: sub    $0x20,%rsp
     0x14255eb45: mov    %rcx,%rbx
     0x14255eb48: lea    0x18(%rcx),%rdx
     0x14255eb4c: mov    0x20(%rcx),%rcx
  ```

---

## WIN64 WORKER STACK

- **Stack Layout on Entry**:
  - `RSP before call`: `0x00007fffcca81870` (`RSP % 16 == 0`).
  - `RSP at guest entry`: `0x00007fffcca81868` (`RSP % 16 == 8`, strictly compliant with standard Win64 callee entry ABI).
  - 32-byte shadow space allocated by host caller.
  - Return address at `[RSP]` points to `0x555555f1a7ef` (safe Rine thread return stub).

---

## WORKER TEB

- **Main Thread TEB**: `0x00007FFFF7F9F000`
- **Worker Thread TEB**: `0x00007FFFD9D4E000`
- **Non-Overlap / Distinction**: **`PROVEN`** (`main_teb != worker_teb`).
- **Worker TEB Structure**:
  - `tib.self_ptr` (offset `0x30`): `0x00007FFFD9D4E000`
  - `tib.stack_base` (offset `0x08`): top of worker stack range
  - `tib.stack_limit` (offset `0x10`): bottom of worker stack range
  - `client_id.unique_process` (offset `0x40`): `pid`
  - `client_id.unique_thread` (offset `0x48`): `512`
  - `thread_local_storage_pointer` (offset `0x58`): `0x00007FFFD9D4E200` (512-byte TLS array)
  - `peb` (offset `0x60`): `0x00007FFFF7FA0000`
  - `last_error_value` (offset `0x68`): `0`

---

## WORKER GS BASE

- **Worker `%gs` Base**: `0x00007FFFD9D4E000` (set via `arch_prctl(ARCH_SET_GS)`).
- **Match Confirmation**: `gs_base == worker_teb` (**`match=yes`**).
- **Guest Execution**: Worker thread executes instructions and invokes WinAPI functions (`ReleaseSRWLockExclusive`) safely.

---

## WORKER TLS

- **Isolation Mechanism**: Dual-layer per-thread TLS:
  1. `Teb64.thread_local_storage_pointer` (`0x58`) points to 64 per-thread `usize` slots.
  2. `thread_local!` map in Rust runtime for extended TLS indexes.
- **Validation Test**: `test_tls_thread_isolation` passes (values in slot `X` in main thread remain unmodified when worker reads/writes slot `X`).

---

## WORKER LASTERROR

- **Isolation Mechanism**: `SetLastError` / `GetLastError` synchronize with `teb.last_error_value` (at `gs:[0x68]`) and Rust thread-local `THREAD_LAST_ERROR`.
- **Validation Test**: `test_last_error_thread_isolation` passes (main sets 111, worker checks 0 and sets 222, main retains 111).

---

## CRT THREAD STATE

- **CRT Concurrency Status**: `[ALPHA CRT THREAD SEMANTICS]`
- **Thread State Bookkeeping**: Per-thread errno and TLS storage verified non-clobbering between main thread and secondary threads.

---

## THREAD RETURN PATH

- **Return Sentinel**: Host wrapper catches guest start routine return value in `RAX`.
- **Stored Exit Code**: `ThreadEntry.exit_code` updated from `259` (`STILL_ACTIVE`) to callee return value.
- **Termination Flag**: `ThreadEntry.is_terminated` set to `true`.
- **Evidence Log**: `scratch/gdb_guest_thread_return.log`.

---

## THREAD CLEANUP

- **Handle Management**: `CloseHandle(h_thread)` removes entry from `THREAD_HANDLE_TABLE`.
- **Joining Primitives**: `WaitForSingleObject` and `GetExitCodeThread` synchronize via `Arc<(Mutex<bool>, Condvar)>`.

---

## REGRESSION TESTS

All 9 dedicated regression tests added and passing:

1. `dlls::crt::tests::test_beginthread_win64_arguments` - PASSED
2. `dlls::crt::tests::test_beginthreadex_win64_arguments` - PASSED
3. `dlls::kernel32::tests::test_guest_worker_start_executes` - PASSED
4. `dlls::kernel32::tests::test_worker_stack_alignment` - PASSED
5. `dlls::kernel32::tests::test_worker_has_distinct_teb` - PASSED
6. `dlls::kernel32::tests::test_worker_gs_points_to_teb` - PASSED
7. `dlls::kernel32::tests::test_last_error_thread_isolation` - PASSED
8. `dlls::kernel32::tests::test_tls_thread_isolation` - PASSED
9. `dlls::kernel32::tests::test_guest_worker_clean_return` - PASSED

---

## CARGO RESULTS

- `cargo fmt --check`: **`PASSED`** (Code cleanly formatted).
- `cargo check`: **`PASSED`** (Zero errors).
- `cargo test`: **`67/67 PASSED (100%)`**.

---

## OLD UD2 STATUS

- **Instruction at `0x1425869DE`**: **`100% RESOLVED / PASSED`**.
- Target execution proceeds past the entire Godot thread creation routine without trapping.

---

## POST-THREAD TARGET RUN

Full log captured in `scratch/game_after_guest_thread_validation.log`.

Execution trace after `_beginthreadex`:
1. Godot worker thread spawned and executes `0x14255EB40`.
2. Main thread continues past `0x142586979` (stores handle `0x2003` to `0x8(%rbx)`).
3. Main thread calls `GetModuleFileNameW` at `0x14001a1d8`.
4. Main thread executes path normalization routines `0x142a16fe0` and `0x142a22de0`.
5. Main thread enters subsystem dispatcher `0x142a2a5d4` -> calls `0x1429ad350`.

---

## NEW EXACT BLOCKER

- **Faulting Address / RIP**: `0x0000000000000000` (SIGSEGV / null pointer dereference).
- **Trigger Call Site**: `0x1429ad3b2` (`call *(%rax)` in `0x1429ad350`).
- **Caller VA**: `0x142a2a613` (RVA `0x02A2A613`).
- **Caller Function**: Subsystem / Resource Loader initializer at `0x142a2a5d4` (RVA `0x02A2A5D4`).
- **Root Cause**: At `0x1429ad3b8`, Godot reads table pointer `mov 0x38(%rbp), %rax` (array of engine subsystem callbacks). At index `0` (`0x1429ad39e`), `%rcx` points to an object whose vtable entry `0` (`(%rcx)`) is `0x0`. The indirect call `call *(%rax)` executes `call 0x0`, resulting in SIGSEGV at `0x0`.

---

## BLOCKER THREAD

- **Role**: `thread_role=main`
- **Thread ID**: Host PID/TID `26813`
- **Worker Thread Status**: Worker thread (ID `512`) was alive and executing concurrent subsystem synchronization (`ReleaseSRWLockExclusive`).

---

## GRAPHICS STATUS

| Feature / Milestone | Status | Details |
|---|---|---|
| **DXVK Mapped** | **YES** | `d3d11.dll` mapped @ `0x0000000359050000` |
| **CreateDXGIFactory1 Resolved** | **YES** | Native Rine `dxgi.dll` |
| **CreateDXGIFactory1 Called** | **YES** | Invoked during early subsystem init |
| **D3D11CreateDevice Resolved** | **YES** | Resolved via DXVK `d3d11.dll` @ `0x000000035907CCC0` |
| **D3D11CreateDevice Called** | **NO** | Render backend setup blocked before device creation call |
| **D3D11CreateDeviceAndSwapChain Called** | **NO** | Not reached |
| **vkCreateInstance Called** | **NO** | Not reached |
| **vkCreateDevice Called** | **NO** | Not reached |
| **Swapchain Created** | **NO** | Not reached |
| **vkQueuePresentKHR Called** | **NO** | Not reached |
| **Visible Frame Confirmed** | **NO** | Not reached |

---

## NEXT ACTION

Investigate the object table at `0x38(%rbp)` in Godot's subsystem initialization routine `0x1429ad350`, analyze which subsystem / resource loader had a NULL vtable pointer, and provide the appropriate valid interface implementation.
