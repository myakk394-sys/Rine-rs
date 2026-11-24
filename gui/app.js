document.addEventListener('DOMContentLoaded', () => {
    const cardRam = document.getElementById('cardRam');
    const cardSsd = document.getElementById('cardSsd');
    const radioInputs = document.getElementsByName('storageStrategy');
    
    const fileInput = document.getElementById('fileInput');
    const dropzone = document.getElementById('dropzone');
    const dropzoneContent = document.getElementById('dropzoneContent');
    const fileSelectedInfo = document.getElementById('fileSelectedInfo');
    const fileName = document.getElementById('fileName');
    const fileSize = document.getElementById('fileSize');
    const btnClearFile = document.getElementById('btnClearFile');
    
    const btnStart = document.getElementById('btnStart');
    const chkLockRam = document.getElementById('chkLockRam');
    const chkPrefetch = document.getElementById('chkPrefetch');

    const consoleBody = document.getElementById('consoleBody');
    const btnClearConsole = document.getElementById('btnClearConsole');

    const specEntryPoint = document.getElementById('specEntryPoint');
    const specImageBase = document.getElementById('specImageBase');
    const specPeb = document.getElementById('specPeb');
    const specTeb = document.getElementById('specTeb');

    let currentFile = null;

    // 1. Radio Cards Selection ("Кружочки")
    cardRam.addEventListener('click', () => {
        cardRam.classList.add('active');
        cardSsd.classList.remove('active');
        cardRam.querySelector('input').checked = true;
        logConsole('sys', '[CONFIG] Выбран режим: RAM-buffering (Zero I/O, tmpfs/memfd_create).');
    });

    cardSsd.addEventListener('click', () => {
        cardSsd.classList.add('active');
        cardRam.classList.remove('active');
        cardSsd.querySelector('input').checked = true;
        logConsole('sys', '[CONFIG] Выбран режим: SSD Demand (Demand Paging via mmap).');
    });

    // 2. File Selection & Drag-and-Drop
    dropzone.addEventListener('dragover', (e) => {
        e.preventDefault();
        dropzone.style.borderColor = 'var(--accent-cyan)';
    });

    dropzone.addEventListener('dragleave', () => {
        dropzone.style.borderColor = 'var(--card-border-hover)';
    });

    dropzone.addEventListener('drop', (e) => {
        e.preventDefault();
        dropzone.style.borderColor = 'var(--card-border-hover)';
        if (e.dataTransfer.files.length > 0) {
            handleFileSelect(e.dataTransfer.files[0]);
        }
    });

    fileInput.addEventListener('change', (e) => {
        if (e.target.files.length > 0) {
            handleFileSelect(e.target.files[0]);
        }
    });

    btnClearFile.addEventListener('click', (e) => {
        e.stopPropagation();
        currentFile = null;
        fileInput.value = '';
        fileSelectedInfo.classList.add('hidden');
        dropzoneContent.classList.remove('hidden');
        logConsole('info', '[SYS] Файл сброшен.');
    });

    function handleFileSelect(file) {
        if (!file.name.toLowerCase().endsWith('.exe') && !file.name.toLowerCase().endsWith('.dll')) {
            alert('Пожалуйста, выберите 64-битный исполняемый файл Windows (.exe или .dll)');
            return;
        }

        currentFile = file;
        fileName.textContent = file.name;
        const sizeMb = (file.size / (1024 * 1024)).toFixed(1);
        fileSize.textContent = `${sizeMb} MB • PE32+ (Win64)`;

        dropzoneContent.classList.add('hidden');
        fileSelectedInfo.classList.remove('hidden');

        // Simulate parsing real PE headers
        const mockEntryPoint = '0x' + (Math.floor(Math.random() * 0x80000) + 0x10000).toString(16).toUpperCase();
        const mockPeb = '0x' + (BigInt('0x7FFF80000000') + BigInt(Math.floor(Math.random() * 0x100000))).toString(16).toUpperCase();
        const mockTeb = '0x' + (BigInt(mockPeb) + BigInt(0x1000)).toString(16).toUpperCase();

        specEntryPoint.textContent = mockEntryPoint;
        specPeb.textContent = mockPeb;
        specTeb.textContent = mockTeb;

        logConsole('success', `[PE PARSER] Проанализирован исполняемый файл: ${file.name}`);
        logConsole('success', `[PE PARSER] Сигнатура: PE\\0\\0 | Архитектура: AMD64 (x86_64) | Точка входа RVA: ${mockEntryPoint}`);
        logConsole('success', `[PE PARSER] Найдено секций: 4 (.text, .rdata, .data, .reloc)`);
    }

    // 3. Console Logger
    function logConsole(type, text) {
        const line = document.createElement('div');
        line.className = `log-line ${type}`;
        const time = new Date().toLocaleTimeString();
        line.textContent = `[${time}] ${text}`;
        consoleBody.appendChild(line);
        consoleBody.scrollTop = consoleBody.scrollHeight;
    }

    btnClearConsole.addEventListener('click', () => {
        consoleBody.innerHTML = '';
    });

    // 4. Action Start Button ("Кнопка Старт")
    btnStart.addEventListener('click', () => {
        const isRam = cardRam.classList.contains('active');
        const lockRam = chkLockRam.checked;
        const targetFile = currentFile ? currentFile.name : 'RineApp64.exe';

        logConsole('sys', '==================================================');
        logConsole('sys', `[RINE KERNEL] Запуск Rine Translation Layer v0.1.0...`);
        logConsole('sys', `[RINE KERNEL] Целевой файл: ${targetFile}`);
        logConsole('sys', `[RINE KERNEL] Стратегия памяти: ${isRam ? 'RamOnly (memfd_create)' : 'DiskDemand (mmap)'}`);
        if (isRam && lockRam) {
            logConsole('sys', `[RINE KERNEL] mlock: ВКЛЮЧЕН (Страницы закреплены в физической ОЗУ)`);
        }

        logConsole('info', `[MEM LOADER] Выделение памяти под образ (Image Base: 0x0000000140000000)...`);
        setTimeout(() => {
            logConsole('info', `[MEM LOADER] Размещение секций: .text (PROT_READ|PROT_EXEC), .data (PROT_READ|PROT_WRITE)`);
            logConsole('info', `[PEB/TEB] Создание блока PEB64 по адресу: ${specPeb.textContent}`);
            logConsole('info', `[PEB/TEB] Создание блока TEB64 по адресу: ${specTeb.textContent}`);
            logConsole('success', `[RINE ENGINE] Ядро трансляции успешно инициализировано! Точка входа готова.`);
        }, 300);
    });
});
