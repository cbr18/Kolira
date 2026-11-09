# ТЗ: Theme Profile Manager — Qt/QML Application

---

## 1. Цель

Написать нативное KDE/Qt-приложение для управления профилями тем (glass/paper/custom), заменив `theme-menu` (wofi) и расширив функционал:
- Единый интерфейс: профили + обои + расширенные настройки
- Гранулярный выбор компонентов профиля (галочки)
- Безопасность: отсутствие ошибок при неустановленных приложениях
- Применение темы к **самоому приложению** (dogfooding)

---

## 2. Архитектура

```
┌─────────────────────────────────────────────────────────────┐
│  KoliraManager (Qt/QML Application)                   │
├─────────────────────────────────────────────────────────────┤
│  UI Layer (QML/Kirigami)                                    │
│  ├── MainPage: профили + обои (основа)                      │
│  ├── SettingsPage: детальные настройки с чекбоксами         │
│  └── Components: ProfileCard, WallpaperGrid, ToggleGroup   │
├─────────────────────────────────────────────────────────────┤
│  Logic Layer (Python/PySide6 или C++)                       │
│  ├── ProfileManager: save/apply/list/diff (wrap bash)       │
│  ├── WallpaperManager: picker/next/prev (wrap bash)         │
│  ├── TrackedAppsRegistry: детекшн установленных приложений  │
│  └── ConfigWatcher: live reload настроек приложения         │
├─────────────────────────────────────────────────────────────┤
│  Backend: существующие bash-скрипты (kolira,         │
│           wallpaper-picker/next/prev) — НЕ МЕНЯЕМ           │
└─────────────────────────────────────────────────────────────┘
```

**Выбор стека**: **Python 3.11+ + PySide6 (Qt6) + QML/Kirigami**
- Быстрая разработка, типизация, доступ к системным утилитам
- Легко обёртывает существующие bash-скрипты через `QProcess`
- Kirigami даёт нативный KDE вид без лишних зависимостей

---

## 3. Отслеживаемые приложения (из `TRACKED_PATHS`)

### Уже отслеживаются (текущий `kolira`)

| Категория | Приложение | Конфиги | Статус детекта |
|-----------|------------|---------|----------------|
| **KDE/Plasma** | KWin | `kwinrc`, `kwinrulesrc` | ✅ Всегда |
| | Plasma Shell | `plasmarc`, `plasmashellrc`, `plasma-org.kde.plasma.desktop-appletsrc` | ✅ Всегда |
| | Global | `kdeglobals` (цвета, акцент, L&F) | ✅ Всегда |
| **GTK** | GTK3 | `settings.ini`, `gtk.css`, `colors.css` | ✅ Файловый |
| | GTK4 | `settings.ini`, `gtk.css`, `colors.css` | ✅ Файловый |
| **Qt** | Qt5/6 | `QtProject.conf`, `Trolltech.conf` | ✅ Файловый |
| **Window Deco** | Klassy | `klassyrc`, `windecopresetsrc` | ⚠️ Если установлен |
| | Kvantum | `kvantum.kvconfig`, `Sandstone/` | ⚠️ Если установлен |
| **Терминалы** | Alacritty | `alacritty.toml`, `themes/noctalia.toml` | ⚠️ Если установлен |
| | Kitty | `kitty.conf` | ⚠️ Если установлен + live reload |
| **Launchers/Menu** | Wofi | `style.css`, `wallpaper.css` | ⚠️ Если установлен |
| **Мониторинг** | btop | `btop.conf`, `themes/noctalia.theme`, `paper.theme` | ⚠️ Если установлен |
| **Sysinfo** | Fastfetch | `config.jsonc`, `logo`, `cat-source.jpg` | ⚠️ Если установлен |
| **Редакторы** | Zed | `settings.json`, `themes/` | ⚠️ Если установлен |
| | VS Code | `settings.json` | ⚠️ Если установлен |
| | Cursor | `settings.json` | ⚠️ Если установлен |
| **Браузер** | Zen Browser | `user.js`, `userChrome.css`, `userContent.css`, `zen-themes.css` | ⚠️ Если установлен |
| **Notes** | Obsidian | `obsidian.json`, `Preferences`, `vault/.obsidian/appearance.json` | ⚠️ Если установлен |
| **System** | Color Schemes | `KoliraPaper.colors` | ✅ Профиль |
| | Look & Feel | `glass/`, `paper/` в `~/.local/share/plasma/look-and-feel/` | ✅ Профиль |

**Итого: 19 приложений/сущностей**, из которых **6 базовых (KDE/GTK/Qt)** всегда присутствуют, остальные — опционально.

---

## 4. UX / Экраны

### 4.1 Главная страница (MainPage) — "Профили + Обои"

```
┌────────────────────────────────────────────────────────────┐
│ Theme Profile Manager                    [⚙ Настройки]     │
├────────────────────────────────────────────────────────────┤
│  Активный профиль:  ● glass    ○ paper    ○ custom        │
│                                                          │
│  ┌──────────────────────────────────────────────────────┐ │
│  │ ПРОФИЛИ                                              │ │
│  │ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────┐ │ │
│  │ │ ● glass │ │ ○ paper │ │ + Нов.  │ │ Импорт...   │ │ │
│  │ │  Glass  │ │  Light  │ │ профиль │ │             │ │ │
│  │ │ blur+   │ │ solid   │ │         │ │             │ │ │
│  │ └─────────┘ └─────────┘ └─────────┘ └─────────────┘ │ │
│  └──────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌──────────────────────────────────────────────────────┐ │
│  │ ОБОИ (отдельно от профиля)                           │ │
│  │ [Превью текущих]  [◀ Назад] [Выбрать...] [Вперёд ▶]  │ │
│  │                                                       │ │
│  │ ⚠ Управление обоями также доступно через:            │ │
│  │   • Правый клик на рабочем столе → Настроить обои    │ │
│  │   • Системные настройки → Внешний вид → Обои         │ │
│  │   • wallpaper-picker / wallpaper-next (терминал)     │ │
│  └──────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────┘
```

**Ключевые решения:**
- Профили — радио-кнопки (только один активен)
- Обои — отдельный блок, НЕ привязан к профилю (как сейчас)
- Подсказка про альтернативные способы смены обоев
- Кнопка "Настройки" → открывает SettingsPage

---

### 4.2 Страница настроек (SettingsPage) — "Гранулярный контроль"

```
┌────────────────────────────────────────────────────────────┐
│ Настройки профиля: glass                    [Готово]       │
├────────────────────────────────────────────────────────────┤
│  ☑ Применять ВСЁ (режим по умолчанию)                      │
│  ────────────────────────────────────────────────────────  │
│  🔧 KDE / Plasma                    [▼ Развернуть]        │
│     ☑ KWin (эффекты, правила)          ✅ Доступно        │
│     ☑ Plasma Shell (панель, виджеты)   ✅ Доступно        │
│     ☑ Цвета / Акцент / L&F             ✅ Доступно        │
│                                                          │
│  🎨 GTK / Qt                        [▼ Развернуть]        │
│     ☑ GTK3 тема                        ✅ Доступно        │
│     ☑ GTK4 тема                        ✅ Доступно        │
│     ☑ Qt5/Qt6 стиль                      ✅ Доступно        │
│                                                          │
│  🪟 Оконные декорации               [▼ Развернуть]        │
│     ☑ Klassy                           ⚠️ Не установлен   │
│     ☑ Kvantum                          ⚠️ Не установлен   │
│                                                          │
│  💻 Терминалы                       [▼ Развернуть]        │
│     ☑ Kitty (live reload)              ⚠️ Не установлен   │
│     ☑ Alacritty                        ⚠️ Не установлен   │
│                                                          │
│  📱 Приложения                      [▼ Развернуть]        │
│     ☑ Wofi (стиль меню)                ⚠️ Не установлен   │
│     ☑ btop (тема)                      ⚠️ Не установлен   │
│     ☑ Fastfetch                        ⚠️ Не установлен   │
│     ☑ Zed                              ⚠️ Не установлен   │
│     ☑ VS Code / Cursor                 ⚠️ Не установлен   │
│     ☑ Zen Browser                      ⚠️ Не установлен   │
│     ☑ Obsidian                         ⚠️ Не установлен   │
│                                                          │
│  💾 Системные профили               [▼ Развернуть]        │
│     ☑ Color Schemes                    ✅ Доступно        │
│     ☑ Look & Feel (glass/paper)        ✅ Доступно        │
│                                                          │
│  ────────────────────────────────────────────────────────  │
│  [Сохранить как пресет]  [Сбросить на умолчания]          │
└────────────────────────────────────────────────────────────┘
```

**Правила:**
- **По умолчанию — всё включено** (галочка "Применять ВСЁ")
- Если приложение **не установлено** — чекбокс disabled, иконка ⚠️, текст "Не установлен"
- Никаких ошибок, кричащих в консоль — тихий skip
- Можно развернуть группу → выбрать индивидуально
- Пресеты настроек сохраняются в `~/.config/kolira/app-preferences.json`

---

### 4.3 Применение темы к самому приложению (Dogfooding)

При запуске приложение:
1. Читает активный профиль (`~/.config/kolira/active-profile`)
2. Применяет **свою** цветовую схему через `Kirigami.Theme` / `QtQuick.Controls.Material`
3. Для `glass` — включает прозрачность окна (`Qt::WA_TranslucentBackground` + KWin blur)
4. Для `paper` — solide фон, светлая схема

```qml
// ThemeController.qml (singleton)
Kirigami.Theme {
    id: appTheme
    // При старте: читает active-profile, применяет цвета
}

// MainWindow.qml
ApplicationWindow {
    // glass профиль:
    flags: Qt.Window | Qt.FramelessWindowHint
    color: "transparent"
    background: Rectangle {
        anchors.fill: parent
        color: Kirigami.Theme.backgroundColor
        layer.enabled: true
        layer.effect: FastBlur { radius: 16 } // KWin blur
    }
}
```

---

## 5. Реестр отслеживаемых приложений (TrackedAppsRegistry)

```python
# tracked_apps.py
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
import shutil

class AppStatus(Enum):
    AVAILABLE = "available"      # Установлен, конфиги найдены
    NOT_INSTALLED = "not_installed"  # Не установлен
    PARTIAL = "partial"          # Установлен, но часть конфигов нет

@dataclass
class TrackedApp:
    id: str
    name: str
    category: str
    config_paths: list[str]      # Относительно $HOME
    detect_command: str | None = None  # Доп. проверка (which, flatpak list)
    supports_live_reload: bool = False

TRACKED_APPS = [
    # === Базовые (всегда) ===
    TrackedApp("kwin", "KWin", "KDE/Plasma",
        [".config/kwinrc", ".config/kwinrulesrc"]),
    TrackedApp("plasma_shell", "Plasma Shell", "KDE/Plasma",
        [".config/plasmarc", ".config/plasmashellrc",
         ".config/plasma-org.kde.plasma.desktop-appletsrc"]),
    TrackedApp("kdeglobals", "KDE Globals", "KDE/Plasma",
        [".config/kdeglobals"]),
    
    # === GTK ===
    TrackedApp("gtk3", "GTK3", "GTK/Qt",
        [".config/gtk-3.0/settings.ini", ".config/gtk-3.0/gtk.css", ".config/gtk-3.0/colors.css"]),
    TrackedApp("gtk4", "GTK4", "GTK/Qt",
        [".config/gtk-4.0/settings.ini", ".config/gtk-4.0/gtk.css", ".config/gtk-4.0/colors.css"]),
    
    # === Qt ===
    TrackedApp("qt5ct", "Qt5/Qt6", "GTK/Qt",
        [".config/QtProject.conf", ".config/Trolltech.conf"]),
    
    # === Window Deco ===
    TrackedApp("klassy", "Klassy", "Window Deco",
        [".config/klassy/klassyrc", ".config/klassy/windecopresetsrc"],
        detect_command="which klassy"),
    TrackedApp("kvantum", "Kvantum", "Window Deco",
        [".config/Kvantum/kvantum.kvconfig", ".config/Kvantum/Sandstone"],
        detect_command="which kvantummanager"),
    
    # === Terminals ===
    TrackedApp("kitty", "Kitty", "Terminals",
        [".config/kitty/kitty.conf"],
        detect_command="which kitty",
        supports_live_reload=True),
    TrackedApp("alacritty", "Alacritty", "Terminals",
        [".config/alacritty/alacritty.toml", ".config/alacritty/themes/noctalia.toml"],
        detect_command="which alacritty"),
    
    # === Launchers ===
    TrackedApp("wofi", "Wofi", "Applications",
        [".config/wofi/style.css", ".config/wofi/wallpaper.css"],
        detect_command="which wofi"),
    
    # === Monitors ===
    TrackedApp("btop", "btop", "Applications",
        [".config/btop/btop.conf", ".config/btop/themes/noctalia.theme", ".config/btop/themes/paper.theme"],
        detect_command="which btop"),
    
    # === Sysinfo ===
    TrackedApp("fastfetch", "Fastfetch", "Applications",
        [".config/fastfetch/config.jsonc", ".config/fastfetch/logo", ".config/fastfetch/cat-source.jpg"],
        detect_command="which fastfetch"),
    
    # === Editors ===
    TrackedApp("zed", "Zed", "Editors",
        [".config/zed/settings.json", ".config/zed/themes"],
        detect_command="which zed"),
    TrackedApp("vscode", "VS Code", "Editors",
        [".config/Code/User/settings.json"],
        detect_command="which code"),
    TrackedApp("cursor", "Cursor", "Editors",
        [".config/Cursor/User/settings.json"],
        detect_command="which cursor"),
    
    # === Browser ===
    TrackedApp("zen", "Zen Browser", "Applications",
        [".zen/gp0y8wrv.Default (release)/user.js",
         ".zen/gp0y8wrv.Default (release)/chrome/userChrome.css",
         ".zen/gp0y8wrv.Default (release)/chrome/userContent.css",
         ".zen/gp0y8wrv.Default (release)/chrome/zen-themes.css"],
        detect_command="flatpak list | grep app.zen_browser.zen"),
    
    # === Notes ===
    TrackedApp("obsidian", "Obsidian", "Applications",
        [".config/obsidian/obsidian.json", ".config/obsidian/Preferences",
         "obsidian/vault/.obsidian/appearance.json"],
        detect_command="which obsidian"),
    
    # === System Profiles ===
    TrackedApp("color_schemes", "Color Schemes", "System",
        [".local/share/color-schemes/KoliraPaper.colors"]),
    TrackedApp("look_and_feel", "Look & Feel", "System",
        [".local/share/plasma/look-and-feel/glass",
         ".local/share/plasma/look-and-feel/paper"]),
]

def detect_all_apps() -> dict[str, AppStatus]:
    """Возвращает статус каждого приложения."""
    results = {}
    for app in TRACKED_APPS:
        # 1. Проверка команды (если есть)
        if app.detect_command:
            if shutil.which(app.detect_command.split()[0]) is None:
                results[app.id] = AppStatus.NOT_INSTALLED
                continue
        
        # 2. Проверка наличия хотя бы одного конфига
        found = any((Path.home() / p).exists() for p in app.config_paths)
        results[app.id] = AppStatus.AVAILABLE if found else AppStatus.NOT_INSTALLED
    
    return results
```

---

## 6. Интеграция с существующим `kolira`

**Не переписываем логику** — оборачиваем через `QProcess`:

```python
# profile_manager.py
from PySide6.QtCore import QProcess, QObject, Signal

class ProfileManager(QObject):
    applyFinished = Signal(bool, str)  # success, message
    saveFinished = Signal(bool, str)
    
    def __init__(self):
        super().__init__()
        self._proc = QProcess(self)
        self._proc.setProgram("kolira")
    
    def apply(self, profile: str, components: list[str] | None = None):
        """components = None → всё (по умолчанию).
           Если задан список — нужно расширить kolira поддерживать --components."""
        args = ["apply", profile]
        if components:
            args.extend(["--components", ",".join(components)])
        self._proc.start(args)
    
    def save(self, name: str):
        self._proc.start(["save", name])
    
    def list_profiles(self) -> list[str]:
        proc = QProcess()
        proc.start("kolira", ["list"])
        proc.waitForFinished()
        return proc.readAllStandardOutput().data().decode().strip().split("\n")
```

**Важно**: Для гранулярного применения (`--components`) нужно **расширить `kolira` bash-скрипт** — добавить аргумент, который фильтрует `TRACKED_PATHS`. Это единственное изменение в bash.

---

## 7. Настройки приложения (User Preferences)

Файл: `~/.config/kolira/app-preferences.json`

```json
{
  "default_components": "all",  // "all" | "custom"
  "custom_components": [
    "kwin", "plasma_shell", "kdeglobals",
    "gtk3", "gtk4", "qt5ct",
    "kitty", "zed"
  ],
  "wallpaper_linked_to_profile": false,
  "auto_apply_on_startup": false,
  "show_not_installed_warning": true,
  "theme": "system"  // "system" | "glass" | "paper" | "dark" | "light"
}
```

---

## 8. План реализации (по этапам)

| Этап | Задача | Оценка |
|------|--------|--------|
| 1 | Проект PySide6 + QML, базовое окно, Kirigami тема | 2-3ч |
| 2 | `TrackedAppsRegistry` — детект приложений, модель для QML | 2ч |
| 3 | MainPage: профили (radio) + обои (панель с подсказкой) | 3ч |
| 4 | SettingsPage: раскрывающиеся группы, чекбоксы, пресеты | 4ч |
| 5 | Интеграция с `kolira` / `wallpaper-*` через QProcess | 2ч |
| 6 | Dogfooding: применение темы к самому приложению (glass blur) | 2ч |
| 7 | Polish: иконки, анимации, тултипы, переводы | 2ч |
| **Итого** | | **~17-18ч** |

---

## 9. Файловая структура проекта

```
~/kolira/
├── files/bin/                    # Существующие bash-скрипты (НЕ ТРОГАТЬ)
├── qt-app/                       # НОВОЕ Qt-приложение
│   ├── pyproject.toml
│   ├── src/
│   │   ├── __main__.py           # Entry point
│   │   ├── profile_manager.py    # Wrapper kolira
│   │   ├── wallpaper_manager.py  # Wrapper wallpaper-*
│   │   ├── tracked_apps.py       # Реестр + детект
│   │   └── settings.py           # App preferences
│   ├── qml/
│   │   ├── Main.qml              # ApplicationWindow
│   │   ├── MainPage.qml          # Профили + обои
│   │   ├── SettingsPage.qml      # Гранулярные настройки
│   │   ├── components/
│   │   │   ├── ProfileCard.qml
│   │   │   ├── WallpaperPreview.qml
│   │   │   ├── AppToggleGroup.qml
│   │   │   └── ComponentCheckbox.qml
│   │   └── ThemeController.qml   # Singleton для dogfooding
│   └── resources/
│       ├── icons/                # SVG иконки категорий
│       └── qmldir
├── scripts/install-kolira.sh  # Обновить: добавить qt-app деплой
└── docs/TZ-QT-APP.md             # Этот файл
```

---

## 10. Деплой

Добавить в `install-kolira.sh`:

```bash
# Сборка/установка Qt-приложения
install_qt_app() {
    local src="$KIT_DIR/qt-app"
    local dst="$HOME/.local/bin/kolira-manager"
    
    # Создаём wrapper скрипт
    cat > "$dst" <<'EOF'
#!/usr/bin/env bash
cd "$HOME/.local/share/kolira-manager"
exec python3 -m src "$@"
EOF
    chmod +x "$dst"
    
    # Копируем исходники
    rsync -a "$src/" "$HOME/.local/share/kolira-manager/"
    
    # .desktop файл для меню приложений
    mkdir -p "$HOME/.local/share/applications"
    cat > "$HOME/.local/share/applications/kolira-manager.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Theme Profile Manager
Comment=Manage KDE theme profiles and wallpapers
Exec=kolira-manager
Icon=preferences-desktop-theme
Categories=Settings;DesktopSettings;Qt;KDE;
StartupNotify=true
EOF
}
```

---

## 11. Риски и митигация

| Риск | Митигация |
|------|-----------|
| `kolira` не поддерживает `--components` | Добавить в bash за 30 мин (фильтр `TRACKED_PATHS` по префиксам) |
| KWin blur не работает на Wayland без настройки | Проверить `KWIN_FORCE_BLUR=1`, документация в README |
| PySide6 не в системе | `uv pip install pyside6` или `pacman -S pyside6` |
| Zen Browser профиль — хардкод пути | Автодетект через `find ~/.zen -name "user.js" -path "*/Default*"` |
| Большой размер бинарника (PySide) | Использовать `pyside6-deploy` или `nuitka` для сборки, либо оставить как скрипт |

---

## 12. Приоритеты (MoSCoW)

| Must Have | Should Have | Could Have | Won't (v1) |
|-----------|-------------|------------|------------|
| MainPage: профили + обои | SettingsPage: пресеты настроек | Экспорт/импорт профилей | Редактор конфигов в-app |
| Детект установленных приложений | Live reload Kitty в настройках | Синхро RGB (rgb-control) | Плагины для новых приложений |
| Dogfooding: тема приложения | Уведомления об успехе/ошибке | Авто-апдейт из GitHub | Web-интерфейс |
| Без ошибок при отсутствии приложений | Тултипы с путями конфигов | | |

---

## 13. Вопросы для уточнения

1. **Язык UI**: только русский или нужен i18n (gettext)?
2. **System tray**: нужен ли常驻 иконка в трее с быстрым переключением?
3. **Global hotkey**: `Meta+T` → открыть/скрыть окно? — **решение**: не
   реализуем; KGlobalAccel не создаёт компонент без desktop-entry с
   `X-KDE-GlobalAccel-CommandShortcut=true`, а toggle требует single-instance.
   Быстрое переключение профилей доступно из трея.
4. **Автосохранение**: сохранять текущее состояние как "last-known" при ручных изменениях в системе?
5. **Профиль "auto"**: переключение glass↔paper по времени суток / системной теме?

---

*Создано на основе анализа `~/kolira/files/bin/kolira` (TRACKED_PATHS) и текущей архитектуры.*