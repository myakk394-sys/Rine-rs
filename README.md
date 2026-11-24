<p align="center">
  <img src="https://raw.githubusercontent.com/myakk394-sys/Rine-rs/main/assets/Rine_logo.gif" alt="Rine Logo" width="320"/>
</p>

<h1 align="center">Rine-rs</h1>

<p align="center">
  <b>Rine Is Not an Emulator. Win64 compatibility runtime built from scratch in Rust.</b>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL--3.0-blue.svg" alt="GPL-3.0 License"/>
  <img src="https://img.shields.io/badge/Status-Early_Experimental_Build-orange.svg" alt="Project Status"/>
  <img src="https://img.shields.io/badge/Architecture-x86__64_In--Process-informational.svg" alt="Architecture"/>
</p>

<p align="center">
  <a href="#english">English</a> • <a href="#русский">Русский</a>
</p>

---

<a name="english"></a>
## English

> **Notice:** Rine is currently in an **early experimental stage**. It is an active proof-of-concept compatibility layer, tested against a limited selection of target games.

### Overview

**Rine** (*Rine is Not an Emulator*) is an in-process Win64 execution environment and Windows API translation layer written from scratch in Rust for Linux x86_64 systems under the **GNU General Public License v3.0 (GPL-3.0)**.

Unlike traditional Wine/Proton setups that rely on a mandatory central `wineserver` daemon and a web of auxiliary background processes, Rine executes guest Windows PE binaries directly inside the host process's memory space by default. Win32 API calls are mapped directly onto native Linux kernel primitives (`futex`, `mmap`, POSIX I/O) and hardware thread registers via `arch_prctl`, eliminating IPC overhead and scheduling latency for single-process games. (An optional lightweight `rineserver` IPC module is also included in the codebase for multi-process descriptor sharing when needed.)

### API Coverage: Implemented vs. Registered Stubs

To maintain full transparency about project scope:
- **Actively Implemented Native Subsystems (~2,000+ APIs):** Hand-crafted Rust implementations covering core `KERNEL32`, `NTDLL`, `USER32`, `GDI32`, `ADVAPI32`, `WS2_32`, `OLE32`, `WASAPI` (PipeWire/PulseAudio), PE32+ loading, and Vulkan/DXVK bridging.
- **Registered IAT Stubs (~650+ DLL modules):** Auto-generated export tables and safe dynamic fallback stubs (`[RINE UNRESOLVED BIND]`) that prevent immediate crashes on unimplemented secondary imports and log missing symbols for targeted development.

### Key Architectural Concepts

- **In-Process Subsystem Execution:** Core Win32 libraries execute as native Rust functions within the guest's address space without inter-process communication.
- **PE32+ Loader & Relocation Engine:** Full support for PE sections (`.text`, `.rdata`, `.data`, `.pdata`, `.rsrc`, `.reloc`), base relocations, and memory allocation strategies (`mmap` on-demand or preloaded into memory via `memfd_create`).
- **Hardware Thread Context Virtualization:** Direct `TEB64` and `PEB64` allocation with `GS Base` binding via Linux `arch_prctl`.
- **Direct Vulkan Bridge & Standalone DXVK:** Native `vkCreateWin32SurfaceKHR` translation to X11 (also compatible with Wayland sessions via XWayland/SDL2) paired with bundled standalone DXVK (`assets/dxvk/` and `~/.local/share/rine/dxvk/`), feeding Direct3D 9/10/11 commands straight to the host GPU driver without requiring Proton or Wine to be installed.
- **Audio & Input Pipelines:** Low-latency WASAPI streams redirected to PipeWire/PulseAudio; Win32 message loop dispatching (`WM_KEYDOWN`, `WM_KEYUP`, keybinding persistence across sessions).
- **Offline Steamworks Compatibility Layer:** Local stub implementation of `steam_api64.dll` (Flat API, callback queues, and mock interface vtables) enabling standalone execution of owned or developer-tested builds without requiring a running Steam background client.
- **Filesystem & Landlock Sandbox:** Virtual Win32 drive mapping (`RineVfs`) isolating `C:\Users\...` writes to local prefixes (`~/.local/share/rine/prefixes/`) combined with Linux Landlock filesystem sandboxing (`src/sys/sandbox.rs`).

### Performance Benchmark: Cuphead (60-Second Profile)

Single-session comparative measurements taken during a 60-second gameplay run of *Cuphead* on Linux (NVIDIA GeForce RTX 3060) comparing **Proton Experimental** against **Rine Win64 Native**:

| Parameter / Metric | Proton Experimental | Rine Win64 Native | Notes |
| :--- | :--- | :--- | :--- |
| **RAM Usage (Average)** | 2,723 MB (~2.7 GB) | **783 MB (~0.8 GB)** | **Saves 1.94 GB RAM (3.5x less memory)** |
| **Peak RAM Consumption** | 3,310 MB (3.3 GB) | **818 MB (0.8 GB)** | **2.5 GB lower peak usage (4x lighter)** |
| **Background Processes** | 22 – 24 processes | **1 single process** | Monolithic in-process runtime (no background daemons) |
| **Peak CPU Spike** | 280.5% | **142.0%** | **~2x lower peak CPU spike** |
| **Average CPU Load** | **91.4%** | 113.6% | Higher average CPU due to single-process polling/throughput without wineserver stalls |
| **GPU Power Draw** | 57.1 W | **48.3 W** | **Saves ~9W GPU power** |
| **Average GPU Utilization**| 48.4% | **36.6%** | Lower GPU overhead |

### Compatibility & Engine Testing Status

Rine is currently verified on a small set of targets representing distinct engine generations:

| Title | Engine | Current Status | Notes |
| :--- | :--- | :--- | :--- |
| **That's Not My Neighbor** | Godot Engine | **Fully Working** | Stable graphics, audio, and user input. |
| **Cuphead** | Unity (Mono runtime) | **Fully Working** | Smooth gameplay, stable 60 FPS rendering, WASAPI audio, input loop, key rebindings persist to disk. |
| **How to Fish** | Unity 6 (`6000.4.4f1`) | **Partial / Work in Progress** | Renders loading graphics via DXVK/Vulkan bridge; stalls at preloader stage (details below). |

#### Known Technical Challenges: Unity 6 Support

Modern Unity 6 builds introduce architectural hurdles compared to legacy Unity 5 / 2017:
1. **Native C# Job System & Threading:** Unity 6 moves asset streaming, deserialization, and shader compilation into an aggressive worker pool (`UnityWorker` threads) that relies heavily on fine-grained Win32 synchronization primitives. Synchronization misses or lost wakeups cause the `blackScreenCanvasGroup.alpha` fade-out to remain stuck at 1.0 (preloader screen) in `LoadingManager`.
2. **DXVK Frame Pacing Telemetry:** Unity 6 queries D3D11 frame presentation timestamps (`WaitForLastPresentationAndGetTimestamp`). Timing divergences over DXVK can cause the render loop to throttle itself thinking the previous frame was dropped.
3. **Multithreaded Vulkan Pipeline Creation:** Concurrent compilation of SPIR-V shaders and calls to `vkCreatePipelineLayout` / `vkCreateDescriptorSetLayout` from worker threads demand strict structure alignment and mutex locking in host driver bridges (`libnvidia-gpucomp.so`).
4. **Modern Windows API Requirements:** Aggressive polling of Per-Monitor DPI v2 (`GetDpiForWindow`, `SetProcessDpiAwarenessContext`) and microsecond-level timer queues (`CreateTimerQueueTimer`).

### Building & Running

#### Prerequisites
- Rust 1.80+ (supporting `extern "win64"`)
- Linux x86_64 with a Vulkan 1.3+ capable driver
- X11 / SDL2 development headers (`libX11`, `libsdl2-dev`)

#### Build
```bash
cargo check
cargo build --release --bin rine
```

#### Run
```bash
cargo run --release --bin rine -- "/path/to/game.exe"
```

### License

This project is licensed under the **GNU General Public License v3.0 (GPL-3.0)**. See the `LICENSE` file for details.

---

<a name="русский"></a>
## Русский

> **Важно:** Rine находится на **ранней экспериментальной стадии разработки**. Это активный proof-of-concept слой совместимости, протестированный на ограниченном пуле целевых игр.

### О проекте

**Rine** (*Rine is Not an Emulator*) — независимая среда выполнения Win64 и слой трансляции Windows API для систем Linux x86_64, разрабатываемый с нуля на языке Rust под лицензией **GNU General Public License v3.0 (GPL-3.0)**.

В отличие от классической связки Wine/Proton с обязательным фоновым демоном `wineserver` и десятками служебных процессов, Rine по умолчанию исполняет бинарные файлы Windows PE прямо внутри адресного пространства единого нативного процесса. Системные вызовы Win32 маппятся напрямую на примитивы ядра Linux (`futex`, `mmap`, POSIX I/O) и аппаратные регистры процессора через `arch_prctl`, исключая оверхед на IPC для однопроцессных игр. (В кодовой базе также предусмотрен опциональный лёгкий модуль `rineserver` для межпроцессного обмена дескрипторами при необходимости.)

### Покрытие API: реальная реализация и заглушки

Для честного и прозрачного разделения объёма кода:
- **Реально реализованные подсистемы (~2 000+ функций):** Нативные реализации на Rust для `KERNEL32`, `NTDLL`, `USER32`, `GDI32`, `ADVAPI32`, `WS2_32`, `OLE32`, звука `WASAPI` (PipeWire/PulseAudio), загрузчика PE32+ и моста Vulkan/DXVK.
- **Зарегистрированные IAT-заглушки (~650+ DLL-модулей):** Сгенерированные таблицы экспортов и безопасные динамические стабы (`[RINE UNRESOLVED BIND]`), которые предотвращают падение при обращении к второстепенным неиспользуемым импортам и логируют недостающие символы для отладки.

### Архитектурные особенности

- **Внутрипроцессная архитектура API:** Основные системные библиотеки работают как нативные Rust-функции в том же адресном пространстве без контекстных переключений между демонами.
- **Загрузчик PE32+ и релокации:** Парсинг секций PE (`.text`, `.rdata`, `.data`, `.pdata`, `.rsrc`, `.reloc`), релокации базовых адресов и гибкая загрузка (`mmap` по требованию или предзагрузка в RAM через `memfd_create`).
- **Виртуализация контекста потоков (TEB / PEB):** Инициализация структур `TEB64` и `PEB64` с привязкой сегментного регистра `GS Base` через Linux `arch_prctl`.
- **Прямой Vulkan Bridge и встроенный DXVK:** Трансляция `vkCreateWin32SurfaceKHR` на X11 (с поддержкой Wayland-сессий через XWayland/SDL2) в паре с автономным DXVK (`assets/dxvk/` и `~/.local/share/rine/dxvk/`). Работает **без установленного Proton или Wine**.
- **Звуковая подсистема и ввод:** Вывод звука через WASAPI в PipeWire и PulseAudio. Диспетчеризация сообщений ввода (`WM_KEYDOWN`, `WM_KEYUP`), сохранение настроек управления между сессиями.
- **Автономный слой совместимости Steamworks:** Локальная реализация заглушек `steam_api64.dll` (Flat API, очереди коллбеков и mock vtable интерфейсов), позволяющая запускать собственные или поддерживаемые сборки без запущенного фонового клиента Steam.
- **Виртуальная ФС и изоляция Landlock:** Виртуальный маппинг дисков (`RineVfs`) перенаправляет запись `C:\Users\...` в локальный префикс (`~/.local/share/rine/prefixes/`), а модуль `src/sys/sandbox.rs` ограничивает доступ к хост-системе через Linux Landlock.

### Результаты сравнительного бенчмарка: Cuphead (60-секундный замер)

Единичный сравнительный замер 60-секундной сессии в игре *Cuphead* на Linux (NVIDIA GeForce RTX 3060) между **Proton Experimental** и **Rine Win64 Native**:

| Параметр / Метрика | Proton Experimental | Rine Win64 Native | Примечания |
| :--- | :--- | :--- | :--- |
| **Потребление RAM (Среднее)** | 2 723 MB (~2.7 ГБ) | **783 MB (~0.8 ГБ)** | **Экономия 1.94 ГБ RAM (в 3.5 раза меньше)** |
| **Пиковое потребление RAM** | 3 310 MB (3.3 ГБ) | **818 MB (0.8 ГБ)** | **На 2.5 ГБ легче в пике (в 4 раза меньше)** |
| **Количество процессов** | 22 – 24 процесса | **1 процесс** | Монолитная In-Process архитектура без фоновых демонов |
| **Пиковый скачок CPU** | 280.5% | **142.0%** | **Пиковая нагрузка на процессор в ~2 раза ниже** |
| **Средняя нагрузка CPU** | **91.4%** | 113.6% | Более высокая средняя загрузка CPU за счёт однопроцессного цикла рендера/опроса без простоев wineserver |
| **Энергопотребление GPU (Ватты)** | 57.1 W | **48.3 W** | **Экономия ~9 Вт питания видеокарты** |
| **Средняя утилизация GPU** | 48.4% | **36.6%** | Меньший оверхед на GPU |

### Текущий статус совместимости с играми

На текущем этапе проект проверен на трёх играх, представляющих разные поколения движков:

| Игра | Движок | Статус | Примечания |
| :--- | :--- | :--- | :--- |
| **That's Not My Neighbor** | Godot Engine | **Работает идеально** | Стабильный рендеринг, звук, логика и обработка ввода. |
| **Cuphead** | Unity (Mono runtime) | **Работает отлично** | Стабильный 60 FPS геймплей, чистый звук WASAPI, полный цикл ввода, сохранение биндов клавиш в конфиг. |
| **How to Fish** | Unity 6 (`6000.4.4f1`) | **Частично (в разработке)** | Отрисовывается загрузочный экран через DXVK/Vulkan, зависает на этапе прелоадера (детали ниже). |

#### Технические сложности и текущие проблемы с Unity 6

Архитектура Unity 6 принципиально отличается от классического Unity 5:
1. **Native C# Job System и потоки:** В Unity 6 загрузка сцен, десериализация и компиляция вынесены в пул рабочих потоков (`UnityWorker`), интенсивно использующих примитивы синхронизации Win32. Пропуск пробуждения (lost wakeup) в планировщике приводит к тому, что `LoadingManager` не переключает `blackScreenCanvasGroup.alpha` из 1.0 в 0.0 (экран остаётся серым со спиннером-крючком).
2. **Рассинхронизация темпа кадров (DXVK Frame Pacing):** Движок запрашивает у DXGI точные метрики времени кадра (`WaitForLastPresentationAndGetTimestamp`). При трансляции через DXVK задержки презентации интерпретируются движком иначе, что может приводить к самоторможению конвейера рендеринга.
3. **Многопоточный Vulkan:** Параллельная компиляция шейдеров SPIR-V и создание дескрипторов (`vkCreatePipelineLayout` / `vkCreateDescriptorSetLayout`) требуют абсолютной корректности синхронизации и выравнивания структур внутри драйвера (`libnvidia-gpucomp.so`).
4. **Новые вызовы Windows API:** Активное использование Per-Monitor DPI v2 (`GetDpiForWindow`, `SetProcessDpiAwarenessContext`) и таймеров высокого разрешения (`CreateTimerQueueTimer`).

### Сборка и запуск

#### Системные требования
- Rust 1.80+ (с поддержкой `extern "win64"`)
- Linux x86_64, драйвер с поддержкой Vulkan 1.3+
- Библиотеки разработки X11 / SDL2 (`libX11`, `libsdl2-dev`)

#### Сборка
```bash
cargo check
cargo build --release --bin rine
```

#### Запуск
```bash
cargo run --release --bin rine -- "/путь/к/игре.exe"
```

### Лицензия

Проект распространяется под лицензией **GNU General Public License v3.0 (GPL-3.0)**. Подробности см. в файле `LICENSE`.
