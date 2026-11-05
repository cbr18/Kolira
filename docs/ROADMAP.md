# KDE Theme Manager — Полный план разработки (Roadmap)

> **Версионирование:** актуальная линия — `0.5.0`. Экспериментальные теги
> `v1.0.0`–`v1.1.2` оставлены в истории, но публично откатаны: они не являются
> стабильной линией. Новые релизы продолжаются с `v0.5.0`.

---

## v0.5.0 — Qt/QML (текущий этап)

### ✅ Готово
- [x] Clean Architecture: `core/protocol.py` (ABC contracts)
- [x] RustCore + Rust CLI: нативные мутации/status, CLI-адаптер (`cli_core.py`)
- [x] TrackedAppsCore: 20 приложений с авто-детектом
- [x] Controllers: Profile, Wallpaper, TrackedApps, Settings, Main
- [x] ThemeController: dogfooding (glass/paper/dark/light/system)
- [x] QML UI: MainPage (профили + обои), SettingsPage (гранулярные чекбоксы)
- [x] Theme Inspector: страница Theme, 69 полей, diff live↔profile, валидация, live-правки с Undo/Revert, экспорт JSON, ссылки в KCM
- [x] Components: ProfileCardDelegate, ProfileRadioButton, ComponentCategoryGroup, ComponentCheckbox
- [x] Install/Update/Uninstall скрипты для Qt app
- [x] pyproject.toml, README, .gitignore
- [x] Git tags: v0.1.0 (bash), v0.2.0-dev, v0.3.0, v0.5.0 (Qt)
- [x] Конструктор тем: `theme-profile new` (нейтральная основа или копия), свой Look & Feel, предпросмотр палитры (2026-11-05)
- [x] Расположения вне тем: тема несёт только вид панелей/виджетов/правил окон, раскладка остаётся пользовательской (`layout.rs`, 2026-11-05)

### 🔄 В процессе / Нужно доделать

#### 1. Одно ядро вместо двух — ✅ сделано и закоммичено (`42675f5`)
Два ядра (Rust-библиотека + CLI-адаптер) сведены к одному — CLI-адаптер
(`core/cli_core.py`, бэкенд `core`). Удалено: `core/rust_core.py`, FFI-слой
(`libkde_theme_core.so`, cdylib), Rust-трейты без потребителей, `rustCoreBuilt`
в контроллерах/QML, persist-ветка в `__main__`, флаги `--with-rust/--no-rust`
в установщике. Тесты переписаны под одно ядро (скипов нет).
По `docs/PLAN-THEME-MODEL.md` этапы A/B/C тоже выполнены и закоммичены:
GUI описаний и страница «Приложения», `reload_command`, редактор
параметров (C1–C3).

#### 2. Исправить QML баги и UX
- [x] ProfileRadioButton: группировка (модель-authoritative `selected` + Binding re-arm вместо ExclusiveGroup)
- [x] ComponentCheckbox: связка с SettingsController.customComponents
- [x] SettingsPage: ComponentCategoryGroup — передача selectionChanged
- [x] Wallpaper preview: асинхронная загрузка (`asynchronous: true`), BusyIndicator-placeholder
- [ ] DiffDialog: подсветка синтаксиса diff (моноширинный шрифт уже есть)

#### 3. Dogfooding — полноценная темизация приложения
- [x] Glass: прозрачность окна + KWin blur (translucency из QML + blur через D-Bus в `theme_controller.py`)
- [x] Paper: светлая тема Material
- [x] Dark/Light: переключение QtQuick.Controls.Material
- [x] System: следование за системной темой (KDE color scheme, live-подписка на colorSchemeChanged)
- [x] Сохранение выбранной темы в настройках (`AppSettings.theme`, восстановление при старте)

#### 4. System Tray
- [x] System tray icon (Kirigami.ApplicationWindow + QSystemTrayIcon)
- [x] Minimize to tray вместо закрытия
- [x] ~~Global hotkey: Meta+T → toggle window~~ — **убрано**: KGlobalAccel создаёт
  компонент только для `.desktop` с `X-KDE-GlobalAccel-CommandShortcut=true`, а
  голый `doRegister` не давал рабочего шортката. См. `gui/qt-app/README.md`.

#### 5. Авто-детект Zen Browser профиля — ✅ сделано в 0.5.0
`zen_profile_dir()` в Rust (`tracked_apps.rs`) и Python (`tracked_apps.py`)
сканирует `~/.zen` и берёт первый профиль с `user.js`.

#### 6. Тесты и CI
- [x] GitHub Actions: rust (hard gate) + python-gui + security (non-strict)
- [x] pytest для core логики (`tests/test_gui.py`: 200 passed, skipped нет —
      свёртка двух ядер в одно завершена; покрыты manifest.json и CRUD
      пользовательских компонентов)
- [ ] pytest-qt для контроллеров

---

## v0.3.0 — Polish & Features (история)

### UX Improvements
- [ ] **Profile Preview**: показывать diff перед apply (модальное окно)
- [ ] **Wallpaper Grid**: полноценный пикер в приложении (не wofi)
- [ ] **Profile Import/Export**: JSON/YAML бэкапы профилей
- [ ] **Auto-sync**: следить за изменениями в системе, предлагать сохранить
- [ ] **Notifications**: KNotification при успехе/ошибке apply

### Wallpaper Integration
- [ ] Опция "Link wallpaper to profile" — сохранять выбранные обои в профиль
- [ ] При apply профиля — автоматически ставить сохранённые обои (если включено)
- [ ] Поддержка нескольких мониторов (кнопка "Configure per screen")

### Settings
- [ ] Редактор путей к конфигам (advanced mode)
- [ ] Кастомные категории компонентов
- [ ] Профиль "Auto" — переключение glass↔paper по времени/системной теме

---

## v0.5.0 — Rust Core polish & Single Binary

> **Сделано (0.5.0).** The bash scripts are gone: the CLI is a single Rust
> binary set (`theme-profile`, `theme-menu`, `wallpaper-picker/next/prev`) built
> from `src/core/rust_core/src/bin/`; mutations (`apply`/`save`/`diff`) run
> natively in `profile.rs` (no delegation); `status` is a data-driven native
> module (milliseconds instead of ~300 ms; one optional D-Bus call for
> loaded effects) with strict JSONC/INI readers, a real
> `kwin_glass` (better-blur-dx) and Zen auto-detection; the GUI diff runs
> off-thread. Файлы Rust-крейта:
> `src/core/rust_core/src/{lib,types,paths,profile,wallpaper,tracked_apps,
> settings,config_parser,status}.rs` + `src/bin/`; Python-адаптер —
> `src/core/cli_core.py`.
>
> **Сделано (текущее дерево).** Два ядра сведены к одному: Rust-библиотека
> (`libkde_theme_core.so`), FFI-слой и Python-биндинг `rust_core.py` удалены
> полностью, остался единый бэкенд `core` — CLI-адаптер. Переключение:
> `--core core|auto`, `$KTM_CORE`, *Settings → Core* (все значения именуют
> одно ядро).

### Архитектура
```
src/
├── core/
│   ├── protocol.py       # ABC (остаётся)
│   ├── cli_core.py       # CLI adapter (theme-profile / wallpaper-*)
│   └── rust_core/        # Новый Rust crate
│       ├── Cargo.toml
│       ├── src/
│       │   ├── lib.rs       # FFI interface (cdylib)
│       │   ├── profile.rs   # Profile operations
│       │   ├── wallpaper.rs # Wallpaper operations
│       │   ├── tracked.rs   # App detection
│       │   ├── config.rs    # Config parsing (kwinrc, kdeglobals, etc.)
│       │   └── settings.rs  # Settings
│       └── build.rs
├── controllers/          # Python (unchanged)
│   └── ...
```

### Rust Implementation Plan

#### Phase 1: Core Library (cdylib + C FFI)
- [ ] `cargo new --lib kde_theme_core`
- [ ] FFI функции:
  ```rust
  // profile
  extern "C" fn list_profiles() -> *mut ProfileList
  extern "C" fn apply_profile(name: *const c_char, components: *const ComponentArray) -> ResultCode
  extern "C" fn save_profile(name: *const c_char, components: *const ComponentArray) -> ResultCode
  extern "C" fn diff_profiles(a: *const c_char, b: *const c_char) -> *mut c_char
  
  // wallpaper
  extern "C" fn pick_wallpaper() -> *mut c_char
  extern "C" fn next_wallpaper() -> ResultCode
  extern "C" fn prev_wallpaper() -> ResultCode
  extern "C" fn list_wallpapers() -> *mut WallpaperList
  extern "C" fn set_wallpaper(path: *const c_char) -> ResultCode
  
  // tracked apps
  extern "C" fn detect_apps() -> *mut ComponentList
  extern "C" fn refresh_apps() -> *mut ComponentList
  
  // settings
  extern "C" fn load_settings() -> Settings
  extern "C" fn save_settings(settings: Settings) -> ResultCode
  ```
- [ ] Memory management: `free_profile_list`, `free_component_list`, etc.
- [ ] Build: `cargo build --release` → ~~`libkde_theme_core.so`~~ (отменено:
  FFI-ядро удалено вместе со вторым core; сборка даёт только CLI-бинарники)

#### Phase 2: Python Bindings (cffi / pyo3)
- [ ] Вариант A: `cffi` — загрузка `.so` через `ctypes`/`cffi`, сохраняем Python controllers
- [ ] Вариант B: `pyo3` — нативный Python модуль, быстрее, но сложнее сборка
- [x] Обновить `CoreFactory` для выбора `RustCore` vs `CliCore`

#### Phase 3: Standalone Binary — ❌ закрыто решением по архитектуре
GUI остаётся 100% Python (PySide6 + QML), см. `AGENTS.md`
(«Architecture decision»). Одиночный бинарь и удаление Python не планируются.

#### Phase 4: Config Parsing (Native)
- [ ] Парсинг INI (kwinrc, kdeglobals, gtk settings.ini) — `configparser` crate
- [ ] Парсинг TOML (alacritty) — `toml` crate
- [ ] Парсинг JSON/JSONC (vscode, zed, fastfetch) — `serde_json`
- [ ] Парсинг CSS (gtk.css, wofi) — свой парсер или `cssparser`
- [ ] Парсинг user.js (Zen) — regex-based

---

## v1.0.0 — Plasma Applet (Native Integration)

### C++/Qt6 Plasma Plugin
```
plasma-applet/
├── CMakeLists.txt
├── metadata.json
├── src/
│   ├── ThemeManagerApplet.cpp/.h    # Plasma::Applet
│   ├── ProfileModel.cpp/.h          # QAbstractListModel
│   ├── WallpaperModel.cpp/.h
│   └── SettingsDialog.cpp/.h        # KConfigDialog
├── qml/
│   ├── main.qml                     # Compact representation
│   ├── FullRepresentation.qml       # Expanded panel
│   └── Settings.qml
└── contents/
    └── ui/
        └── main.qml
```

### Features
- [ ] Встроен в панель (system tray или отдельный апплет)
- [ ] Compact view: текущий профиль + клик → меню профилей
- [ ] Hover/click → переключение профилей без открытия окна
- [ ] Right-click → настройки, wallpaper next/prev
- [ ] KConfigXT для настроек (интеграция с System Settings)
- [ ] DBus API для внешних скриптов

### Distribution
- [ ] AUR package: `kde-theme-manager-applet`
- [ ] Flatpak: `org.kde.ThemeManager`
- [ ] KDE Store: store.kde.org

---

## v2.0.0 — Ecosystem & Advanced

### RGB Integration
- [ ] Синхронизация с `rgb-control-kit` (Hyperion, OpenRazer, AULA)
- [ ] Профиль темы → RGB режим (glass=slow_cycle, paper=accent)
- [ ] UI для управления RGB в приложении

### Cloud Sync
- [ ] Экспорт профилей в GitHub Gist / Git repo
- [ ] Синхронизация между машинами
- [ ] Community profiles marketplace

### Advanced Theming
- [ ] Генерация тем из wallpaper (matugen / pywal интеграция)
- [ ] Редактор цветовых схем в-app
- [ ] Поддержка GTK4 libadwaita themes

---

## Зависимости по версиям

| Версия | Runtime | Build | Packaging |
|--------|---------|-------|-----------|
| v0.2.x | Python 3.11+, PySide6, uv | pip/uv | GitHub Releases |
| v0.3.0 | + KNotification | | |
| v0.5.0 | Python 3.11+, PySide6, Rust 1.75+ (default) | uv, cargo | GitHub Releases |
| v1.0.0+ | Python (PySide6 GUI) + Rust core/CLI (без C++-переписывания) | uv, cargo | GitHub Releases |

---

## Команда / Ресурсы

| Задача | Навыки | Оценка |
|--------|--------|--------|
| `--components` в bash | Bash, KDE config | 1ч |
| QML полировка | QML, Kirigami | 4-8ч |
| Dogfooding (glass blur) | Qt Wayland, KWin | 2-4ч |
| System Tray | QSystemTrayIcon | 4-6ч |
| Rust Core FFI | ~~Rust, C FFI, cffi/pyo3~~ — отменено: ядро одно (CLI-адаптер) | — |
| Config parsers (Rust) | Rust, serde, toml, ini | 10-20ч |
| Plasma Applet | C++, Qt6, KDE Frameworks | 30-60ч |

---

## Приоритеты (MoSCoW для 0.3.x, история)

| Must Have | Should Have | Could Have | Won't (v0.3) |
|-----------|-------------|------------|--------------|
| `--components` в bash | Profile Preview | Wallpaper Grid | Rust Core |
| QML багфиксы | Import/Export | Auto-sync | Plasma Applet |
| Dogfooding (basic) | Notifications | Multi-monitor | Cloud Sync |
| System Tray | Zen profile autodetect | | |

> Глобальный хоткей (Meta+T) снят с плана: без desktop-entry с
> `X-KDE-GlobalAccel-CommandShortcut=true` KGlobalAccel не создаёт компонент, а
> для toggle-семантики нужен ещё и single-instance слой.
>
> Профиль `default` (baseline текущей темы) отложен. Если делать — только
> вручную, кнопкой «Сохранить как default», без автосоздания при запуске. Имя
> `system` не используем, чтобы не путать с `AppSettings.theme = "system"`
> (внешний вид приложения, а не профиль).

---

## Команды для разработки

```bash
# Разработка Qt app
cd ~/theme-profile-kit/qt-app
uv pip install -e ".[dev]"
python -m src

# Линтинг
ruff check src/
mypy src/

# Тесты
pytest

# Сборка Rust CLI
cd ~/projects/kde_mods/theme-profile-kit/gui/qt-app/src/core/rust_core
cargo build --release

# Запуск (ядро одно — CLI-адаптер)
python -m src --core core     # или: KTM_CORE=core python -m src

# Установка в систему
cd ~/theme-profile-kit
./scripts/install-theme-profile-kit.sh
```

---

## Риски и митигация

| Риск | Вероятность | Влияние | Митигация |
|------|-------------|---------|-----------|
| KWin blur не работает на Wayland | Высокая | Среднее | Fallback на прозрачность без blur, документация |
| PySide6 версия в дистре старая | Средняя | Высокое | uv venv + uv pip install PySide6@latest |
| Rust FFI сложности | Средняя | Высокое | Начать с cffi (проще), pyo3 позже |
| Plasma 6 API изменения | Низкая | Среднее | Тестировать на Plasma 6.1+, пинить версии |
| Zen Browser путь меняется | Высокая | Низкое | Автодетект через find, настройка в UI |

---

*Последнее обновление: v0.5.0*
*Репозиторий: https://github.com/cbr18/kde-theme-manager*