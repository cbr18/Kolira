# План: конструктор тем — палитра, генераторы, магазины

Task: [2026_11_04_09_00_theme-constructor-palette.md](../tasks/2026_11_04_09_00_theme-constructor-palette.md)

Каждая фаза — отдельная ветка (`fix/p0-socket-installer`, `feat/wallpaper-*`,
`feat/p1-palette`, `feat/p2-new-theme`, `feat/p3-preview`), один или несколько
коммитов с документацией, полный прогон проверок перед мержем в `main`.

## Последовательность реализации

### P0 — мелкие ошибки (ветка `fix/p0-socket-installer`)

1. `gui/qt-app/src/__main__.py::_acquire_single_instance`: различать три исхода —
   «живая копия ответила» (передать `show`, выйти), «сокет занят, но мёртв»
   (`removeServer` + повтор), «`listen` не удался» (вывести
   `server.errorString()`, путь и подсказку про лимит 108 байт; запуститься
   без single-instance). Вернуть enum вместо bool.
2. `scripts/install-kolira.sh`: после установки GUI искать процесс
   `kolira` / `python -m src` с cwd в `~/.local/share/kolira`;
   предупреждать «запущена старая версия, перезапустите»; флаг `--restart` —
   мягко завершить (SIGTERM) и запустить заново. Тест в `tests/test-scripts.sh`.
3. Документация: `README.md` (флаг `--restart`), `gui/qt-app/README.md`
   (single-instance).

### Обои — отдельная задача

Выполнить [wallpaper-integration-audit](../tasks/2026_11_04_08_30_wallpaper-integration-audit.md)
до P1: «из обоев» и L&F опираются на единый поиск текущих обоев и каталог
обоев без хардкода локали.

### P1 — палитра и генераторы (ветка `feat/p1-palette`)

1. **Модель** `rust_core/src/palette.rs`:
   - `Palette { name, variant: dark|light, base: [Rgb; 16], accent: Rgb, source }`;
   - парсинг base16 YAML/JSON (tinted-theming: `palette.base00…base0F`, а также
     старый формат `base00: "…"`), сериализация в `palette.json`;
   - контраст WCAG (`contrast(a, b)`), предупреждения < 4.5:1 для текста.
2. **Генераторы** (`palette/generate/*.rs`, чистые функции `Palette → String`):
   - KDE `.colors`: все группы `Colors:{Window,View,Button,Selection,Tooltip,
     Complementary,Header,Header][Inactive}`, `[WM]`, `ColorEffects` из шаблона,
     `AccentColor`; маппинг ролей base16 → KDE зафиксировать таблицей в коде и
     `docs/PALETTE.md`; светлые схемы — тот же маппинг по `variant`;
   - kitty: блок цветов между маркерами `# BEGIN kolira palette` /
     `# END …` в `kitty.conf` (идемпотентная замена, остальной файл не трогаем);
   - Alacritty: `[colors.*]` в `alacritty.toml` через toml_edit (без
     переформатирования).
3. **Kvantum-шаблон**: `files/kvantum-template/` (SVG + kvconfig с метками
   `@base00@…@accent@`), генератор подставляет цвета →
   `$XDG_DATA_HOME/Kvantum/kolira-<name>/`. Выбор в UI: «Из палитры» или
   любая установленная тема Kvantum, или стиль Breeze.
4. **CLI**: `kolira palette show|import FILE|URL|gallery NAME|from-wallpaper PATH|from-colors JSON|apply [--targets=kde,kitty,alacritty,kvantum]`.
   `from-colors` принимает цвета, снятые GUI через ImageColors, и раскладывает
   их по слотам; `from-wallpaper` — свой k-means в OKLab (без `magick`).
5. **Галерея base16**: индекс tinted-theming скачивается по запросу в
   `$XDG_CACHE_HOME/kolira/base16/`; во встроенном наборе ~10 популярных
   схем (Everforest, Gruvbox, Nord, Catppuccin, Tokyo Night, Rose Pine, Dracula,
   Solarized ×2, One). Настройка `network_enabled` (по умолчанию вкл.).
6. **Магазины тем редакторов** — HTTP-клиенты и распаковка в Rust
   (`kolira apps store-search APP QUERY`, `store-install APP ID`),
   Python-адаптеры только вызывают CLI:
   - Zed: `GET api.zed.dev/extensions?filter=Q&provides=themes`, установка —
     скачать архив, извлечь `themes/*.json` в `~/.config/zed/themes/`;
   - Cursor / VS Code: поиск в Open VSX (Cursor, OSS-сборки) / VS Marketplace
     (Microsoft-сборка), установка `<bin> --install-extension ID`;
   - после установки — обновить список тем и выбрать тему.
7. **Детект приложений по бинарнику**: в адаптерах список имён (`zed`,
   `zeditor`; `code`, `code-oss`, `codium`; flatpak id); `launch_command`
   берётся из найденного; статус «нет бинарника, есть конфиг» = `not_installed`
   с подсказкой. Rust `tracked_apps.rs` — тот же список.
8. **btop**: при выборе палитры подсказывать установленную тему с похожим
   именем (`everforest` → `everforest-dark-medium`), без генерации.
9. **GUI**: страница «Палитра» в Theme: 16 слотов + акцент (ColorDialog),
   кнопки «Галерея», «Импорт», «Из обоев» (Kirigami.ImageColors → `from-colors`),
   «Применить палитру» с выбором целей; в «Темы приложений» — кнопка
   «Найти в магазине…» с диалогом поиска.
10. **MCP**: `palette_show`, `palette_import`, `palette_set`, `palette_apply`,
    `app_store_search`, `app_store_install`.
11. Профиль: `palette.json` в снапшоте; трекинг
    `$XDG_DATA_HOME/Kvantum/kolira-*`.

### P2 — новая тема (ветка `feat/p2-new-theme`)

1. **«Начать с…»**: диалог «Новая тема» — основа: любой профиль или встроенный
   «Нейтральный» (без размытия, translucency 100, kitty opacity 1.0, Breeze,
   без L&F). Применение основы — существующий `apply_profile` (для шаблона —
   встроенный псевдопрофиль `files/templates/neutral/`).
2. **Свой L&F**: при сохранении профиля генерировать
   `$XDG_DATA_HOME/plasma/look-and-feel/kolira-<name>/`
   (`metadata.json`, `contents/defaults` с ColorScheme, widgetStyle, иконками,
   курсорами, Plasma Style, оформлением окон; превью из палитры). Без layout.
   `plasma_global` пишется в bundle только если пользователь выбрал L&F явно
   или это сгенерированный пакет. Трекинг каталога `look-and-feel/kolira-*`.
3. **Темы приложений в bundle**: хранить только `explicit`-выбор — сессионный
   набор `_app_changes`, установки из магазина и сгенерированные; плюс
   галочки «входит в тему» на странице приложений. Старые bundle читаются как есть.

### P3 — предпросмотр (ветка `feat/p3-preview`)

1. QML-компонент `PalettePreview.qml`: мини-окно (заголовок `[WM]`, список с
   выделением, кнопки, поле ввода), терминал (16 цветов ANSI), обои фоном.
2. Индикаторы контраста (текст/фон, выделение) из Rust `contrast()`.
3. Показ в редакторе палитры и в диалоге галереи до применения.

## Проверки

- `cd gui/qt-app/src/core/rust_core && cargo build --release && cargo test && cargo clippy --all-targets -- -D warnings`
- `cd gui/qt-app && QT_QPA_PLATFORM=offscreen uv run --project . --extra dev pytest tests -q`
- `bash tests/test-scripts.sh`, `shellcheck scripts/*.sh tests/test-scripts.sh`
- `find gui/qt-app/qml -name '*.qml' -print0 | xargs -0 qmllint`
- `bash scripts/update-translations.sh` — без незавершённых переводов.
- Новые тесты: парсинг base16 (оба формата), каждый генератор (golden-файлы
  для Everforest dark/light), идемпотентность блока kitty, Kvantum-шаблон без
  оставшихся `@…@`, store-поиск на записанных ответах API (без сети),
  детект `zeditor`, single-instance при длинном `TMPDIR`, bundle без
  «чужих» приложений, L&F `defaults`.
- Ручная приёмка: пересоздать Everforest новым путём (см. критерии задачи).

## Риски

- **Сеть**: API магазинов меняются → изолировать клиенты, тесты на фикстурах,
  понятные ошибки; всё отключаемо настройкой.
- **Установка расширений** — внешнее действие: подтверждение в UI, бэкап
  `settings.json`, только CLI самого редактора.
- **L&F**: применение пакета может перезаписать пользовательские правки KDE —
  только `defaults`, без layout; тест на отсутствие `layouts/`.
- **kitty/alacritty**: правка пользовательских конфигов — только блок между
  маркерами, атомарная запись, бэкап.
- **ImageColors vs Rust**: разные алгоритмы дадут разные палитры из одних обоев —
  документировать; источник фиксируется в `palette.source`.
- Rust-зависимости (`serde_yaml`, `toml_edit`, HTTP-клиент `ureq`) — минимальные,
  без async.

## Заметки по откату

Каждая фаза — отдельная ветка/коммит, откат `git revert`. Генерируемые ресурсы
живут в отдельных именах (`kolira-*`, блоки с маркерами) и не
перезаписывают пользовательские темы; установщик продолжает делать бэкапы.

## Документация (в той же фазе)

`README.md`, `docs/CURRENT_APP.md`, `docs/MCP.md`, `gui/qt-app/README.md`,
новый `docs/PALETTE.md` (модель, маппинг base16 → KDE/kitty/Kvantum, источники),
`docs/ROADMAP.md` (отметить выполненное).
