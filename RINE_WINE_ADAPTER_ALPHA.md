# Rine Provider Resolver & Wine Adapter Architecture (Alpha Phase)

## Executive Summary

In Rine Alpha, the WinAPI provider resolution system follows a strict non-invasive hierarchy:
1. **Priority 1 (Top Priority)**: **Native Rine Subsystem Implementation** (`IatSymbolResolver`).
2. **Priority 2 (Fallback)**: **Wine Adapter Provider** (Explicitly allowlisted PE DLLs loaded via Rine PE Loader).
3. **Priority 3 (Diagnostic Fail-fast)**: **Unresolved Universal Stub** (`[RINE UNRESOLVED API FATAL]`).

This design ensures Rine remains a custom, native Windows x86_64 runtime engine for Linux while leveraging host Wine PE DLLs on-demand to unblock missing low-level Windows APIs.

---

## 1. Provider Resolution Hierarchy & Policy

```
                              [API Call Target]
                                      │
                                      ▼
                        ┌───────────────────────────┐
                        │ Priority 1: Native Rine   │
                        │    (IatSymbolResolver)    │
                        └─────────────┬─────────────┘
                                      │ Found?
                              ┌───────┴───────┐
                              │               │
                           Yes│             No│
                              ▼               ▼
                        ┌───────────┐   ┌───────────────────────────┐
                        │ Native    │   │ Priority 2: Wine Adapter  │
                        │ Execution │   │ (Allowlist & Dynamic PE)  │
                        └───────────┘   └─────────────┬─────────────┘
                                                      │ Found & Allowed?
                                              ┌───────┴───────┐
                                              │               │
                                           Yes│             No│
                                              ▼               ▼
                                        ┌───────────┐   ┌───────────────────────────┐
                                        │ Wine DLL  │   │ Priority 3: Fail-Fast     │
                                        │ Execution │   │ ([RINE UNRESOLVED FATAL]) │
                                        └───────────┘   └───────────────────────────┘
```

- **Top Priority for Native Rine**: If an API is implemented in Rine (e.g. `kernel32`, `user32`, `d3d11`, `dxgi`), Rine's implementation is ALWAYS chosen.
- **Wine Adapter Opt-in**: Enabled via `--wine-provider` flag or `WineConfig { enabled: true, ... }`. It is disabled by default to maintain native runtime purity.
- **Allowlist Scope Control**: Wine PE DLLs are loaded only if explicitly added to `allowed_modules` (e.g. `version.dll`, `avrt.dll`).
- **Diagnostic Fail-fast**: Any API not resolved by Native Rine or Wine Adapter immediately hits `universal_stub_thunk`, logging full diagnostic context (symbol, module, registers, call site bytes, stack args) before exiting with code 127.

---

## 2. Dynamic PE Mapping & Win64 ABI Boundary

Wine PE DLLs (e.g., `avrt.dll`, `version.dll`) are loaded using Rine's internal PE parser & virtual memory loader (`load_and_map_wine_pe_dll`):
1. **PE Memory Mapping**: Sections mapped into virtual memory (`libc::mmap`).
2. **Base Relocations**: `.reloc` section processed if preferred base is unavailable.
3. **Import Binding**: Wine PE DLL dependencies categorized (Classes A–D):
   - **Class A**: Bound to Native Rine APIs (`kernel32`, `ucrtbase`).
   - **Class B**: Bound to allowlisted Wine PE DLLs.
   - **Class C**: Wine internal / Unix bridge dependencies (`__wine_unix_call`, `wineserver`) -> REJECTED to prevent native runtime pollution.
   - **Class D**: Unresolved dependencies bound to diagnostic stubs.
4. **TEB/PEB Context**: `ProcessEnvironment` sets `%gs` base via `SYS_arch_prctl` for guest thread.
5. **DllMain Initialization**: `DllMain(base, DLL_PROCESS_ATTACH, 0)` invoked safely.
6. **Win64 Safe Execution Thunk**: `call_win64_export_safely` enforces strict Win64 ABI register preservation:
   - **GPR Nonvolatiles Preserved**: `RBX`, `RBP`, `RSI`, `RDI`, `R12`, `R13`, `R14`, `R15`.
   - **SIMD Nonvolatiles Preserved**: `XMM6` – `XMM15` (`movdqu` save/restore).
   - **Stack Alignment**: 16-byte aligned before `call rax` with 32-byte Win64 shadow space.

---

## 3. Configuration & CLI Integration

Wine Adapter configuration is managed globally via `WineConfig` and CLI options:

```bash
# Run game with Wine Adapter enabled (autodetect host Wine PE path)
target/debug/rine --wine-provider "That's Not My Neighbor.exe"

# Specify explicit Wine PE DLL directory
target/debug/rine --wine-provider --wine-dll-path "/usr/lib/x86_64-linux-gnu/wine/x86_64-windows" "That's Not My Neighbor.exe"

# Force-disable Wine Adapter (Native Rine strict mode)
target/debug/rine --no-wine-provider "That's Not My Neighbor.exe"
```

---

## 4. Candidate Scan & License Compliance Summary

- **Scanned Candidate DLLs**: Recorded in `scratch/wine_candidate_scan.tsv`.
- **Top Safe Candidates**: `avrt.dll` (Audio & Thread Characteristics API) and `version.dll` (FileVersionInfo API).
- **Dependency Classification**: Recorded in `scratch/wine_dependency_graph.txt`.
- **License Notes**: Documented in `RINE_WINE_PROVIDER_LICENSE_NOTES.md`. External Wine binaries are covered under LGPL v2.1+. Rine remains clean MIT/Apache-2.0 open-source architecture by interacting with external Wine PE DLLs solely over standard Win64 binary interfaces.

---

## 5. Verification & Test Suite Results

All 10 unit and integration tests in `src/providers/` pass cleanly:

| Test Name | Result | Purpose |
|---|---|---|
| `provider_prefers_native_rine` | **PASSED** | Verifies Priority 1 native Rine selection over Wine |
| `provider_uses_wine_when_native_missing` | **PASSED** | Verifies Priority 2 Wine fallback for missing exports |
| `provider_fails_when_both_missing` | **PASSED** | Verifies Priority 3 fail-fast when API is unmapped |
| `wine_provider_disabled_by_default` | **PASSED** | Confirms Wine provider is disabled without `--wine-provider` |
| `wine_provider_allowlist_enforced` | **PASSED** | Confirms allowlist blocks non-whitelisted Wine DLLs |
| `wine_adapter_poc_load_and_call_export` | **PASSED** | **Proof of Concept**: Loads `avrt.dll`, runs `DllMain`, executes `AvSetMmThreadCharacteristicsA` and `AvRevertMmThreadCharacteristics` |
| `wine_provider_abi_boundary_preserves_gprs` | **PASSED** | Verifies assembly thunk Win64 ABI register preservation |
| `provider_stats_counts_native_calls` | **PASSED** | Validates native call atomic counter tracking |
| `provider_stats_counts_wine_calls` | **PASSED** | Validates Wine call atomic counter tracking |
| `provider_stats_counts_unresolved` | **PASSED** | Validates unresolved call atomic counter tracking |

```
Full Test Suite Execution: 56 passed; 0 failed; 0 ignored (100% PASS)
```
