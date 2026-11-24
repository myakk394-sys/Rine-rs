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

Unlike traditional Wine/Proton setups that rely on a central `wineserver` daemon and a web of auxiliary background processes, Rine executes guest Windows PE binaries directly inside the host process's memory space. Win32 API calls are mapped directly onto native Linux kernel primitives (`futex`, `mmap`, POSIX I/O) and hardware thread registers via `arch_prctl`, eliminating the high IPC overhead and scheduling latency typical of client-server runtimes.

### Key Architectural Concepts

- **In-Process Subsystem Execution:** Standard Win32 libraries (`KERNEL32`, `NTDLL`, `USER32`, `GDI32`, `ADVAPI32`, `WS2_32`) execute as native Rust functions within the guest's address space without inter-process communication.
- **PE32+ Loader & Relocation Engine:** Full support for PE sections (`.text`, `.rdata`, `.data`, `.pdata`, `.rsrc`, `.reloc`), base relocations, and memory allocation strategies (`mmap` on-demand or preloaded into memory via `memfd_create`).
- **Hardware Thread Context Virtualization:** Direct `TEB64` and `PEB64` allocation with `GS Base` binding via Linux `arch_prctl`.
- **Direct Vulkan Bridge & DXVK:** Native `vkCreateWin32SurfaceKHR` translation to X11/Wayland paired with DXVK, feeding Direct3D 9/10/11 commands straight to the host GPU driver.
- **Audio & Input Pipelines:** Low-latency WASAPI streams redirected to PipeWire/PulseAudio; accurate Win32 message loop dispatching (`WM_KEYDOWN`, `WM_KEYUP`, keybinding persistence across sessions).
- **Steamworks API Emulation:** Offline local emulation of `steam_api64.dll` (Flat API, callback queues, ticket simulation) enabling DRM checks and initialization handshakes to pass seamlessly.

### Performance Benchmark: Cuphead (60-Second Profile)

Comparative measurements taken during a 60-second gameplay run of *Cuphead* on Linux (NVIDIA GeForce RTX 3060) comparing **Proton Experimental** against **Rine Win64 Native**:

| Parameter / Metric | Proton Experimental | Rine Win64 Native | Advantage & Impact |
| :--- | :--- | :--- | :--- |
| **RAM Usage (Average)** | 2,723 MB (~2.7 GB) | **783 MB (~0.8 GB)** | **Saves 1.94 GB RAM (3.5x less memory)** |
| **Peak RAM Consumption** | 3,310 MB (3.3 GB) | **818 MB (0.8 GB)** | **2.5 GB lower peak usage (4x lighter)** |
| **Background Processes** | 22 – 24 processes | **1 single process** | Pure monolithic in-process runtime (no wineserver, services, etc.) |
| **Peak CPU Spike** | 280.5% | **142.0%** | **Peak CPU load cut in half** |
| **Average CPU Load** | 91.4% | **113.6%** | Smooth frame throughput without throttling |
| **GPU Power Draw** | 57.1 W | **48.3 W** | **Saves ~9W GPU power** |
| **Average GPU Utilization**| 48.4% | **36.6%** | Higher hardware efficiency |

### Compatibility & Engine Testing Status

Rine is currently verified on a small set of targets representing distinct engine generations:

| Title | Engine | Current Status | Notes |
| :--- | :--- | :--- | :--- |
| **That's Not My Neighbor** | Godot Engine | **Fully Working** | Stable graphics, audio, and user input. |
| **Cuphead** | Unity (Mono runtime) | **Fully Working** | Smooth gameplay, stable 60 FPS rendering, WASAPI audio, input loop, key rebindings persist to disk. |
| **How to Fish** | Unity 6 (`6000.4.4f1`) | **Partial / Work in Progress** | Renders loading graphics via DXVK/Vulkan bridge; stalls at preloader stage (details below). |

#### Known Technical Challenges: Unity 6 Support

Modern Unity 6 builds introduce architectural hurdles compared to legacy Unity 5 / 2017:
1. **Native C# Job System & Threading:** Unity 6 moves asset streaming, deserialization, and shader compilation into an aggressive worker pool (`UnityWorker` threads) that relies heavily on fine-grained Win32 synchronization primitives. Synchronization misses or lost wakeups cause the `blackScreenCanvasGroup.alpha` fade-out to remain stuck at 1.0 (preloader hook screen) in `LoadingManager`.
2. **DXVK Frame Pacing Telemetry:** Unity 6 queries D3D11 frame presentation timestamps (`WaitForLastPresentationAndGetTimestamp`). Timing divergences over DXVK can cause the render loop to throttle itself thinking the previous frame was dropped.
3. **Multithreaded Vulkan Pipeline Creation:** Concurrent compilation of SPIR-V shaders and calls to `vkCreatePipelineLayout` / `vkCreateDescriptorSetLayout` from worker threads demand strict structure alignment and mutex locking in host driver bridges (`libnvidia-gpucomp.so`).
4. **Modern Windows API Requirements:** Aggressive polling of Per-Monitor DPI v2 (`GetDpiForWindow`, `SetProcessDpiAwarenessContext`) and microsecond-level timer queues (`CreateTimerQueueTimer`).

### Building & Running

#### Prerequisites
- Rust 1.80+ (supporting `extern "win64"`)
- Linux x86_64 with a Vulkan 1.3+ capable driver
- X11 development headers (`libX11`, SDL2)

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

В отличие от классической связки Wine/Proton с фоновым демоном `wineserver` и десятками служебных процессов, Rine исполняет бинарные файлы Windows PE прямо внутри адресного пространства единого нативного процесса. Системные вызовы Win32 маппятся напрямую на примитивы ядра Linux (`futex`, `mmap`, POSIX I/O) и аппаратные регистры процессора через `arch_prctl`, полностью исключая оверхед на IPC и контекстные переключения.

### Архитектурные особенности

- **Внутрипроцессная архитектура API:** Реализации системных библиотек (`KERNEL32`, `NTDLL`, `USER32`, `GDI32`, `ADVAPI32`, `WS2_32`) работают как нативные Rust-функции в том же адресном пространстве, убирая контекстные переключения между демонами.
- **Загрузчик PE32+ и релокации:** Корректный парсинг всех секций PE (`.text`, `.rdata`, `.data`, `.pdata`, `.rsrc`, `.reloc`), исправление базовых адресов через таблицу релокаций и поддержка гибких стратегий загрузки (`mmap` или предзагрузка в RAM через `memfd_create`).
- **Виртуализация контекста потоков (TEB / PEB):** Полноценная инициализация структур `TEB64` и `PEB64` с аппаратной привязкой сегментного регистра `GS Base` через Linux `arch_prctl`.
- **Прямой Vulkan Bridge и интеграция с DXVK:** Собственный мост трансляции оконных поверхностей Win32 Vulkan (`vkCreateWin32SurfaceKHR` в X11 / Wayland), обеспечивающий прямой проброс вызовов Direct3D 9/10/11 в видеодрайвер.
- **Звуковая подсистема и ввод:** Низколатентный вывод звука через WASAPI с перенаправлением в PipeWire и PulseAudio. Точная диспетчеризация сообщений ввода (`WM_KEYDOWN`, `WM_KEYUP`), сохранение настроек управления между сессиями.
- **Поддержка Steamworks и DRM:** Встроенный локальный слой эмуляции API Steam (`steam_api64.dll`, Flat API, диспетчеризация очередей коллбеков и генерация mock vtable интерфейсов), необходимый для прохождения DRM-проверок и стартовых рукопожатий.

### Результаты сравнительного бенчмарка: Cuphead (60-секундный замер)

Сравнение показателей 60-секундной сессии в игре *Cuphead* на Linux (NVIDIA GeForce RTX 3060) между **Proton Experimental** и **Rine Win64 Native**:

| Параметр / Метрика | Proton Experimental | Rine Win64 Native | Разница и преимущество |
| :--- | :--- | :--- | :--- |
| **Потребление RAM (Среднее)** | 2 723 MB (~2.7 ГБ) | **783 MB (~0.8 ГБ)** | **Rine экономит 1.94 ГБ RAM (в 3.5 раза меньше!)** |
| **Пиковое потребление RAM** | 3 310 MB (3.3 ГБ) | **818 MB (0.8 ГБ)** | **Rine легче на 2.5 ГБ в пике (в 4 раза меньше!)** |
| **Количество процессов** | 22 – 24 процесса | **Ровно 1 процесс** | Монолитная In-Process архитектура (без wineserver и фоновых демонов) |
| **Пиковый скачок CPU** | 280.5% | **142.0%** | **Пиковая нагрузка на процессор в 2 раза ниже** |
| **Средняя нагрузка CPU** | 91.4% | **113.6%** | Равномерный рендер без троттлинга конвейера |
| **Энергопотребление GPU (Ватты)** | 57.1 W | **48.3 W** | **Rine экономит ~9 Ватт питания видеокарты** |
| **Средняя утилизация GPU** | 48.4% | **36.6%** | Видеокарта работает эффективнее и холоднее |

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
- Библиотеки X11 (`libX11`, SDL2)

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
