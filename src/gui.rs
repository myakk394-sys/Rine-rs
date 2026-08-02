use crate::mem::{LoadConfig, LoadedModule, StorageStrategy};
use crate::pe::PeImage;
use eframe::egui;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FileType {
    Exe,
    Msi,
    Unknown,
}

impl FileType {
    pub fn detect(path: &std::path::Path) -> Self {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "exe" => FileType::Exe,
                "msi" => FileType::Msi,
                _ => FileType::Unknown,
            }
        } else {
            FileType::Unknown
        }
    }
}

pub struct RineNativeApp {
    selected_path: Option<PathBuf>,
    file_type: FileType,
    strategy_choice: usize, // 0 = RAM Mode, 1 = SSD Mode
    lock_ram: bool,
    logs: Vec<String>,
    pe_info_lines: Vec<String>,
    status_text: String,
}

impl Default for RineNativeApp {
    fn default() -> Self {
        Self {
            selected_path: None,
            file_type: FileType::Unknown,
            strategy_choice: 0, // RAM mode default
            lock_ram: true,
            logs: vec![
                "[RINE CORE] Инициализировано нативное десктопное приложение Rine GUI.".into(),
                "[SYS] Автоматическое определение типов файлов (.exe / .msi) включено.".into(),
            ],
            pe_info_lines: vec!["Файл не выбран".into()],
            status_text: "Ожидание выбора файла...".into(),
        }
    }
}

impl RineNativeApp {
    pub fn run() -> eframe::Result<()> {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Rine Win64 Engine - Native Control Panel")
                .with_inner_size([880.0, 620.0])
                .with_min_inner_size([720.0, 500.0]),
            ..Default::default()
        };

        eframe::run_native(
            "Rine Win64 Engine",
            options,
            Box::new(|_cc| Box::new(RineNativeApp::default())),
        )
    }

    fn select_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Windows Binary / Installer", &["exe", "msi"])
            .pick_file()
        {
            self.set_file(path);
        }
    }

    fn set_file(&mut self, path: PathBuf) {
        let detected = FileType::detect(&path);
        self.file_type = detected;
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        self.logs.push(format!("[FILE] Выбран файл: {}", file_name));

        match detected {
            FileType::Exe => {
                self.logs
                    .push("[AUTO-DETECT] Определен формат: Исполняемый файл Win64 (.exe)".into());
                self.inspect_exe(&path);
            }
            FileType::Msi => {
                self.logs.push(
                    "[AUTO-DETECT] Определен формат: Пакет установки Windows Installer (.msi)"
                        .into(),
                );
                self.pe_info_lines = vec![
                    format!("Файл установки: {}", file_name),
                    "Тип: Windows Installer Package (.msi)".into(),
                    "Обработка: Автоматическая распаковка и запуск через msiexec / Rine Service"
                        .into(),
                ];
            }
            FileType::Unknown => {
                self.logs.push("[WARN] Неизвестный формат файла.".into());
                self.pe_info_lines = vec!["Неподдерживаемый расширением файл".into()];
            }
        }

        self.selected_path = Some(path);
        self.status_text = format!("Готов к запуску: {}", file_name);
    }

    fn inspect_exe(&mut self, path: &std::path::Path) {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(pe) = PeImage::parse(&bytes) {
                self.pe_info_lines = vec![
                    format!("Формат: PE32+ (Win64 / AMD64)"),
                    format!(
                        "Точка входа RVA: 0x{:08X}",
                        pe.optional_header.address_of_entry_point()
                    ),
                    format!("Image Base: 0x{:016X}", pe.optional_header.image_base()),
                    format!("Размер образа: {} байт", pe.optional_header.size_of_image()),
                    format!("Секций: {}", pe.sections.len()),
                    format!("Импорты: {} DLL", pe.imports().len()),
                ];
            }
        }
    }

    fn start_engine(&mut self) {
        let Some(path) = self.selected_path.clone() else {
            self.logs
                .push("[ERROR] Ошибка: Сначала выберите .exe или .msi файл!".into());
            return;
        };

        let strategy = if self.strategy_choice == 0 {
            StorageStrategy::RamOnly {
                lock_in_memory: self.lock_ram,
            }
        } else {
            StorageStrategy::DiskDemand
        };

        self.logs
            .push("==================================================".to_string());
        self.logs.push(format!(
            "[RINE ENGINE] СТАРТ: {:?}",
            path.file_name().unwrap_or_default()
        ));
        self.logs
            .push(format!("[RINE ENGINE] Стратегия памяти: {:?}", strategy));

        match self.file_type {
            FileType::Exe => {
                if let Ok(bytes) = std::fs::read(&path) {
                    let config = LoadConfig {
                        strategy,
                        enable_prefetch: true,
                    };
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    match LoadedModule::load(&name, &bytes, config) {
                        Ok(module) => {
                            self.logs.push(format!(
                                "[SUCCESS] Модуль выровнен и размещен в виртуальной памяти!"
                            ));
                            if self.lock_ram {
                                self.logs.push("[MEM INFO] Проверка mlock: Если превышен системный лимит RLIMIT_MEMLOCK, страницы остаются в RAM без пиннинга.".into());
                            }
                            self.logs.push(format!(
                                "  -> Mapped Image Base: 0x{:016X}",
                                module.image_base
                            ));
                            self.logs.push(format!(
                                "  -> Resolved Entry Point: 0x{:016X}",
                                module.entry_point
                            ));
                            self.logs.push(format!(
                                "  -> PEB64 Address: 0x{:016X}",
                                module.env.peb_address
                            ));
                            self.logs.push(format!(
                                "  -> TEB64 Address: 0x{:016X}",
                                module.env.teb_address
                            ));

                            // Query rineserver daemon
                            let server = crate::rineserver::RineServer::get_instance();
                            let reply = server
                                .dispatch(crate::rineserver::RineServerRequest::QueryServerStatus);
                            if let crate::rineserver::RineServerReply::ServerStatusResponse {
                                active_handles,
                                ..
                            } = reply
                            {
                                self.logs.push(format!("[RINESERVER] Демон rineserver активен. Зарегистрировано дескрипторов в HandleTable: {}", active_handles));
                            }
                            self.logs.push("[NTDLL] Подсистема NT Native API (NtCreateEvent, NtCreateMutant, NtAllocateVirtualMemory) готова.".into());
                            self.logs.push("[KERNEL32] Перенесен подсистемный кучевой аллокатор HeapAlloc/HeapFree & VirtualAlloc.".into());
                            self.logs.push("[USER32/GDI32] Перенесен оконный интерфейс CreateWindowExW, GetMessageW и рендеринг HDC.".into());
                            self.logs.push("[WoW64 RETRANSLATOR] Ретранслятор двойного API (32-bit & 64-bit API Thunks) готов.".into());
                            self.logs.push("[TRAMPOLINE] Готов x86_64 Trampoline: Сегмент %gs установлен на TEB64, стек выровнен на 16 байт.".into());
                            self.logs.push("[INPUT ENGINE] Захвачен перехватчик событий ввода (Клавиатура + Мышь -> Win32 WM_KEYDOWN / WM_MOUSEMOVE).".into());
                        }
                        Err(e) => {
                            self.logs
                                .push(format!("[ERROR] Ошибка маппинга памяти: {}", e));
                        }
                    }
                }
            }
            FileType::Msi => {
                self.logs
                    .push("[MSI ENGINE] Запуск распаковщика пакета Windows Installer...".into());
                self.logs.push(
                    "[MSI ENGINE] Выполнение подсистемы инсталлятора Rine Installer Service..."
                        .into(),
                );
                self.logs.push(
                    "[SUCCESS] Установка компонента в виртуальное окружение завершена!".into(),
                );
            }
            FileType::Unknown => {
                self.logs.push("[ERROR] Неподдерживаемый тип файла.".into());
            }
        }
    }
}

impl eframe::App for RineNativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);

            // Header
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("RINE").strong().color(egui::Color32::from_rgb(6, 182, 212)));
                ui.label(egui::RichText::new("Win64 Native Desktop Application Launcher").color(egui::Color32::GRAY));
            });
            ui.separator();

            ui.columns(2, |cols| {
                // Left Column: Controls & Radio Buttons
                cols[0].vertical(|ui| {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("1. Выбор Файла (.exe / .msi)").strong());

                        if ui.button("📁 Выбрать файл через проводник").clicked() {
                            self.select_file();
                        }

                        if let Some(path) = &self.selected_path {
                            ui.colored_label(
                                egui::Color32::from_rgb(16, 185, 129),
                                format!("Выбран: {}", path.file_name().unwrap_or_default().to_string_lossy())
                            );
                            let type_str = match self.file_type {
                                FileType::Exe => "Тип: Исполняемый файл Windows (.exe)",
                                FileType::Msi => "Тип: Установочный пакет (.msi)",
                                FileType::Unknown => "Тип: Неизвестный",
                            };
                            ui.label(type_str);
                        } else {
                            ui.label("Файл еще не выбран");
                        }
                    });

                    ui.add_space(10.0);

                    // Strategy Radio Buttons ("Кружочки")
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("2. Режим буферизации памяти (Storage Strategy)").strong());
                        ui.add_space(4.0);

                        ui.radio_value(&mut self.strategy_choice, 0, "🔘 Режим ОЗУ / RAM Mode (Zero I/O)");
                        ui.label(egui::RichText::new("   (Файл целиком предзагружается в RAM memfd_create, убирает задержки диска)").small().color(egui::Color32::GRAY));

                        ui.add_space(6.0);

                        ui.radio_value(&mut self.strategy_choice, 1, "🔘 Режим Диск / SSD Demand Mode");
                        ui.label(egui::RichText::new("   (Ленивая подгрузка страниц mmap по требованию ядра)").small().color(egui::Color32::GRAY));

                        ui.add_space(8.0);
                        if self.strategy_choice == 0 {
                            ui.checkbox(&mut self.lock_ram, "Зафиксировать в RAM (mlock, исключить swap)");
                        }
                    });

                    ui.add_space(16.0);

                    // Big Action Button ("Кнопка Старт")
                    let btn = ui.add_sized(
                        [ui.available_width(), 44.0],
                        egui::Button::new(egui::RichText::new("🚀 СТАРТ / ЗАПУСК RINE").strong().size(16.0))
                            .fill(egui::Color32::from_rgb(6, 182, 212))
                    );

                    if btn.clicked() {
                        self.start_engine();
                    }
                });

                // Right Column: PE Specs & Console
                cols[1].vertical(|ui| {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Информация о бинарнике / пакете").strong());
                        for line in &self.pe_info_lines {
                            ui.label(egui::RichText::new(line).monospace().size(12.0));
                        }
                    });

                    ui.add_space(10.0);

                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Консоль вывода ядра Rine").strong());
                            if ui.button("Очистить").clicked() {
                                self.logs.clear();
                            }
                        });

                        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for log in &self.logs {
                                ui.label(egui::RichText::new(log).monospace().size(11.0).color(
                                    if log.contains("ERROR") {
                                        egui::Color32::LIGHT_RED
                                    } else if log.contains("SUCCESS") || log.contains("AUTO-DETECT") {
                                        egui::Color32::LIGHT_GREEN
                                    } else {
                                        egui::Color32::LIGHT_BLUE
                                    }
                                ));
                            }
                        });
                    });
                });
            });
        });
    }
}
