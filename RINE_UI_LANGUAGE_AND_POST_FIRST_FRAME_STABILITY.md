# RINE UI LANGUAGE AND POST-FIRST-FRAME STABILITY REPORT

## 1. CURRENT VERIFIED STATUS

- **Source State**: Clean, formatted (`cargo fmt`).
- **Compilation**: `cargo build --bin rine` -> **PASS** (dev profile, exit code 0).
- **Unit Tests**: `cargo test` -> **124 passed; 0 failed; 0 ignored** (100% pass rate).
- **User32 Subsystem**:
  * `SetWindowsHookExA` -> `HHOOK = 0x3000` (WH_MOUSE, tid 9560).
  * `CreateWindowExW` -> `HWND = 0xE00002` (Title "Engine", 1920x1080).
  * Native X11 Window: `0xE00002` on Display `0x59da17af8e70`, VisualID `0x45`.
- **Vulkan Subsystem**:
  * `vkCreateWin32SurfaceKHR` bridged to `vkCreateXlibSurfaceKHR` -> `VkSurfaceKHR = 0x59DA18B029A0`.
  * Physical Device: `NVIDIA GeForce RTX 3060` (Vulkan API 1.4.329, Forward+).
  * Logical Device: `VkDevice = 0x59da18b60cf0`.
  * Virtual Swapchain: `VkSwapchainKHR = 0x4000` (3 GPU images allocated).
  * Present Dispatch: `guest_vkQueuePresentKHR` executed and confirmed.

---

## 2. FIRST PRESENT MILESTONE & VISIBLE FRAME EVIDENCE

- **Milestone Reached**: `[RINE FRAME PRESENTED] Frame #1 successfully presented on desktop window (1920x1080)!`
- **Execution Log Line**: Line 7800 in target runtime log.
- **Evidence Classification**:
  * `PRESENTED FRAME`: **PROVEN** (`guest_vkQueuePresentKHR` executed with valid Swapchain `0x4000`, `image_index=0`, image displayed via native X11 flush).
  * `VISIBLE GAME SCENE`: **IN PROGRESS** (Godot has entered scene tree / ICU locale initialization).

---

## 3. EXECUTION PLAN

| Step | Subsystem | Action | Verification / Log | Status |
| :--- | :--- | :--- | :--- | :--- |
| 1 | Locale Model | Design central `GuestLocale` snapshotting host environment | `src/dlls/kernel32.rs` | **DONE** |
| 2 | NLS APIs | Implement `GetUserDefaultUILanguage` & related NLS functions | `GetUserDefaultUILanguage() -> u16` | **DONE** |
| 3 | PE Imports | Map NLS symbols in IAT symbol resolver table | `src/pe/imports.rs` | **DONE** |
| 4 | Disassembly | Capture exact caller/consumer instructions around 0x14001A449 | `scratch/getuserdefaultuilanguage_consumer.txt` | **PROVEN** |
| 5 | Target Proof | Trace `GetUserDefaultUILanguage` return value and registers | `scratch/gdb_getuserdefaultuilanguage_output.log` | **PROVEN (RAX=0x419)** |
| 6 | Unit Tests | Add 4 new deterministic tests for locale mapping and resolution | `cargo test` -> 124 tests pass | **PASS** |
| 7 | Stability Run | Execute target run to observe post-locale scene progression | `scratch/game_after_ui_language.log` | **EXECUTED** |

---

## 4. EXISTING EVIDENCE REVIEW

- Previous blocker: Target halted at `KERNEL32.dll!GetUserDefaultUILanguage` (CALL VA `0x14001A449`, IAT `0x1443A3718`).
- Consumer analysis revealed that Godot queries the user's UI language to configure the translation server and locale strings during initial window frame setup.

---

## 5. TARGET GETUSERDEFAULTUILANGUAGE CALL & CONSUMER DISASSEMBLY

### Disassembly Around 0x14001A449:
```text
14001a449: ff 15 c9 92 38 04     callq  *0x43892c9(%rip)        # KERNEL32.dll!GetUserDefaultUILanguage
14001a44f: 48 8d 15 57 ff 5b 03  leaq   0x35bff57(%rip),%rdx    # Default fallback string
14001a456: 48 c7 44 24 20 00 00  movq   $0x0,0x20(%rsp)
14001a45d: 00 00
14001a45f: 89 c7                 movl   %eax,%edi               # EAX = returned LANGID
14001a461: 89 fe                 movl   %edi,%esi
14001a463: 66 c1 ef 0a           shrl   $0xa,%di                # DI = SUBLANGID (lang_id >> 10)
14001a467: 81 e6 ff 03 00 00     andl   $0x3ff,%esi             # ESI = PRIMARYLANGID (lang_id & 0x3ff)
14001a46d: 0f b7 ff              movzwl %di,%edi
14001a470: eb 13                 jmp    0x14001a485
14001a485: 39 73 08              cmpl   %esi,0x8(%rbx)          # Match primary language in Godot table
14001a488: 75 ee                 jne    0x14001a478
14001a48a: 8b 43 0c              movl   0xc(%rbx),%eax          # Match sublanguage
14001a48d: 85 c0                 testl  %eax,%eax
14001a48f: 74 4f                 je     0x14001a4e0
14001a491: 39 c7                 cmpl   %eax,%edi
```

---

## 6. GUEST LOCALE MODEL & HOST LOCALE INHERITANCE

- **Architecture**: `GuestLocale` encapsulates `lang_id`, `lcid`, and `locale_name`.
- **Policy**: `[ALPHA HOST LOCALE INHERITANCE]`
  * Reads `LC_ALL` / `LC_MESSAGES` / `LANG` at startup.
  * Maps `ru_*` -> `0x0419` (`ru-RU`, Primary `0x19`, Sub `0x01`).
  * Maps `de_*` -> `0x0407` (`de-DE`).
  * Maps `fr_*` -> `0x040C` (`fr-FR`).
  * Maps `es_*` -> `0x0C0A` (`es-ES`).
  * Maps `zh_*` -> `0x0804` (`zh-CN`).
  * Maps `ja_*` -> `0x0411` (`ja-JP`).
  * Fallback: `0x0409` (`en-US`, Primary `0x09`, Sub `0x01`).

---

## 7. TARGET RETURN PROOF & GODOT SELECTED LOCALE

From GDB runtime trace at `0x14001A44F`:
- `RAX`: `0x0000000000000419` (1049 decimal)
- `ESI (Primary)`: `0x19` (`LANG_RUSSIAN`)
- `EDI (Sublanguage)`: `0x01` (`SUBLANG_RUSSIAN_RUSSIA`)
- **Godot Match**: Matched table entry at `0x1435e7ee0` -> Locale string mapped to `"ru-RU"` / `"ru"`.
- **Engine Progression**: Godot immediately initialized TextServer ICU, looked for optional `icudt73l.dat`, and safely initialized built-in locale translation data.

---

## 8. POST-LOCALE THREADING & SUSPENSION ADVANCEMENT

- **Enhanced Thread Management**:
  * Implemented real `CREATE_SUSPENDED` (flag `0x4`) and `ResumeThread` synchronization in `src/dlls/kernel32.rs`.
  * Threads created in suspended state cleanly wait on `suspended_sync` condvar until resumed by caller.
  * `ResumeThread` properly wakes up target threads.
  * Resolved GPU semaphore synchronization in `acquire_next_image` by dispatching GPU signal completion on active queues.

---

## 9. WASAPI DEFERRED STATUS

- **Status**: **DEFERRED / NON-BLOCKING**
- **Call**: `CoCreateInstance(CLSID_MMDeviceEnumerator)` on background thread 526 continues non-blocking polling without impacting main thread rendering or scene loading.

---

## 10. TEST & REGRESSION RESULTS

- `cargo fmt --check`: Clean (0 diffs).
- `cargo test`: **124 passed; 0 failed; 0 ignored**
  * `test_get_user_default_ui_language_resolver` -> **PASS**
  * `test_get_user_default_ui_language_win64_call` -> **PASS**
  * `test_guest_ui_language_mapping` -> **PASS**
  * `test_guest_ui_language_fallback` -> **PASS**

---

## 11. CURRENT EXACT RUNTIME STATE & NEXT ACTION

- **Current State**:
  * Frame #1 presented on native X11 desktop window.
  * `GetUserDefaultUILanguage` executed and fully passed.
  * Godot text/locale server initialized.
  * Threading subsystem handles suspended threads and worker dispatches cleanly.
- **Next Action**:
  * Observe continuous game loop iterations and monitor scene tree instantiation toward the game menu / gameplay nodes.
