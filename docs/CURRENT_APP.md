# Текущее приложение: theme-profile-kit (0.5.0)

> **Статус**: 0.5.0 — Rust CLI + Qt/QML приложение KDE Theme Manager.
> **GUI**: устанавливается по умолчанию (см. [gui/qt-app/README.md](../gui/qt-app/README.md)).

---

## Обзор

`theme-profile-kit` — Rust CLI и Qt/QML приложение для сохранения и применения профилей оформления KDE Plasma. Позволяет переключаться между темами **glass** (прозрачное стекло с размытием) и **paper** (светлая solid тема) одной командой.

---

## Состав

| Компонент | Назначение |
|-----------|------------|
| `theme-profile` | CLI: `save`, `apply`, `list`, `diff`, `status` профилей |
| `theme-menu` | Wofi-меню для управления профилями (apply/save/status/open folder) |
| `wallpaper-picker` | Выбор обоев через wofi с превью (ImageMagick) |
| `wallpaper-next` / `wallpaper-prev` | Листание обоев по циклу |
| `kde-theme-manager` | Qt/QML GUI: профили, обои, трей, Theme Inspector |

---

## Установка

```bash
~/theme-profile-kit/scripts/install-theme-profile-kit.sh
```

Создаёт бэкап предыдущих версий в `~/.config/theme-profile/backups/`.
Деплоит скрипты в `~/.local/bin/` (должно быть в PATH) и GUI:

| Что | Куда |
|-----|------|
| `theme-profile`, `theme-menu`, `wallpaper-*` | `~/.local/bin/` |
| KDE Theme Manager (копия приложения) | `~/.local/share/kde-theme-manager/` |
| обёртка `kde-theme-manager` | `~/.local/bin/` |
| ярлык | `~/.local/share/applications/kde-theme-manager.desktop` |
| иконка | `~/.local/share/icons/hicolor/scalable/apps/ktm-theme-manager.svg` |

Только CLI: `install-theme-profile-kit.sh --only-cli`. Только GUI:
`install-theme-profile-kit.sh --only-gui` (для работы приложения CLI уже должен
быть установлен). Venv создаётся лишь если в системе нет PySide6.

Данные (профили, бэкапы, настройки) живут отдельно, в
`$XDG_CONFIG_HOME/theme-profile/` (по умолчанию `~/.config/theme-profile/`), и
установкой не затрагиваются. Существующий legacy-каталог
`~/.config/theme-profile/` определяется и сохраняется; снимки профилей всегда
используют историческую раскладку `.config/...`, поэтому профили, созданные до
поддержки XDG, продолжают работать.

---

## Использование

```bash
theme-profile list               # список профилей
theme-profile apply glass        # применить профиль
theme-profile save my-profile    # сохранить текущее состояние
theme-profile status             # текущее состояние
theme-profile diff glass paper   # сравнить профили

theme-menu                       # wofi-меню
wallpaper-picker                 # выбор обоев
wallpaper-next                   # следующие обои
wallpaper-prev                   # предыдущие обои
```

---

## Профили

- **glass** — полупрозрачные окна с размытием, KWin glass enabled, blur, better-blur-dx для выбранных классов окон (zen, code, kitty, zed)
- **paper** — светлая/solid тема, без прозрачностей

Профили хранятся в `$XDG_CONFIG_HOME/theme-profile/profiles/` (по умолчанию
`~/.config/theme-profile/profiles/`).
Активный профиль — маркер `theme-profile/active-profile` в том же корне.

При apply Rust-ядро после восстановления файлов заново применяет цветовую схему через `plasma-apply-colorscheme`. Если в профиле есть явный `AccentColor` и `accentColorFromWallpaper=false`, он передаётся через `--accent-color`; если `accentColorFromWallpaper=true`, схема применяется без `--accent-color`, а акцент пересчитывает сама Plasma из восстановленных обоев при перезапуске plasmashell (проверено 2026-11-04; ImageMagick для акцента больше не используется). Отдельно переприменяется Qt widget style: для `widgetStyle=kvantum` вызывается `kvantummanager --set <theme>`, чтобы уже запущенные Qt-приложения получили тему Kvantum, а не только обновлённый файл. Обои профиля: при `save` копия текущих обоев кладётся в `<профиль>/wallpaper/` (+ `source` с исходным путём), при `apply` используется оригинал, а если его нет — копия возвращается на место (или в каталог обоев) и применяется; это работает одинаково для CLI, `theme-menu`, GUI и MCP. Каталог обоев: `$WALLPAPER_DIR` → настройка `wallpaper_directory` → `$(xdg-user-dir PICTURES)/Wallpaper` → `~/Pictures/Wallpaper` (`theme-profile wallpaper dir`). Профиль `glass` имеет `accentColorFromWallpaper=true` (акцент из обоев), профиль `paper` — `accentColorFromWallpaper=false` с явным `AccentColor=155,150,77`. Поэтому акцент переключается между профилями: `glass` берёт акцент из обоев, `paper` — оливковый из явного `AccentColor`.

---

## Что отслеживается (TRACKED_PATHS)

### KDE/Plasma (базовые, всегда)
- `.config/kwinrc` — KWin эффекты, правила
- `.config/kwinrulesrc` — правила окон
- `.config/kdeglobals` — цвета, акцент, Look & Feel, цветовая схема
- `.config/plasmarc` — настройки Plasma
- `.config/plasma-org.kde.plasma.desktop-appletsrc` — панель, виджеты, обои (весь конфиг)
- `.config/plasmashellrc` — поведение оболочки

### GTK
- `.config/gtk-3.0/settings.ini`, `gtk.css`, `colors.css`
- `.config/gtk-4.0/settings.ini`, `gtk.css`, `colors.css`

### Qt
- `.config/QtProject.conf`
- `.config/Trolltech.conf`

### Window Decorations (опционально)
- `.config/klassy/klassyrc`, `windecopresetsrc`
- `.config/Kvantum/kvantum.kvconfig`, `Sandstone/`

### Terminals (опционально)
- `.config/alacritty/alacritty.toml`, `themes/noctalia.toml`
- `.config/kitty/kitty.conf` — с live reload через `kitty @`

### Launchers/Menu (опционально)
- `.config/wofi/style.css`, `wallpaper.css`

### Monitors (опционально)
- `.config/btop/btop.conf`, `themes/noctalia.theme`, `paper.theme`

### Sysinfo (опционально)
- `.config/fastfetch/config.jsonc`, `logo`, `cat-source.jpg`

### Editors (опционально)
- `.config/zed/settings.json`, `themes/`
- `.config/Code/User/settings.json`
- `.config/Cursor/User/settings.json`

### Browser (опционально)
- `.zen/<профиль>/user.js` — каталог профиля авто-детектится (первый с `user.js`); включая `zen.view.window.scheme` (`0` dark, `1` light, `2` system)
- `.zen/<профиль>/chrome/userChrome.css`
- `.zen/<профиль>/chrome/userContent.css`
- `.zen/<профиль>/chrome/zen-themes.css`

### Notes (опционально)
- `.config/obsidian/obsidian.json`, `Preferences`
- `obsidian/vault/.obsidian/appearance.json`

### System Profiles
- `.local/share/color-schemes/` — весь каталог пользовательских схем (раньше только `ThemeProfilePaper.colors`), чтобы профиль со своей схемой (например, `Everforest.colors`) был самодостаточным; восстановление сливает файлы и не удаляет схемы, созданные позже
- `.local/share/plasma/look-and-feel/glass/`
- `.local/share/plasma/look-and-feel/paper/`

---

## Зависимости

| Пакет | Зачем |
|-------|-------|
| `wofi` | меню (`theme-menu`, `wallpaper-picker`) |
| `plasma-workspace` | `kreadconfig6`/`kwriteconfig6`/`kbuildsycoca6`/`plasma-apply-wallpaperimage` |
| `imagemagick` | генерация превью обоев (`wallpaper-picker`) |
| `kitty` | опционально, для live-reload темы терминала |
| `dolphin` | опционально, кнопка "Open profile folder" в `theme-menu` |

Всё базовое ставится с KDE Plasma из коробки.

---

## Правило редактирования

1. Править Rust CLI в `~/theme-profile-kit/gui/qt-app/src/core/rust_core/src/bin/`
2. Запустить `~/theme-profile-kit/scripts/install-theme-profile-kit.sh`
3. **Не редактировать** `~/.local/bin/` напрямую (или сразу скопировать изменения обратно в kit)

---

## GUI: Theme bundles

Qt/QML-приложение (`gui/qt-app/`) ставится установщиком рядом с Rust CLI. Страница **Theme** теперь является конструктором комплекта тем:

- **Палитра** (`rust_core/src/palette.rs`, `theme-profile palette …`): base16 + акцент → сгенерированная цветовая схема KDE `ThemeProfile<Имя>.colors`, блок цветов в `kitty.conf` (между маркерами, live-reload по сокетам `/tmp/kitty-theme-profile-<pid>`) и `[colors.*]` в `alacritty.toml`; по запросу — тема Kvantum, перекрашенная из установленной базовой (`KvArcDark`/`KvArc`) в `~/.config/Kvantum/ThemeProfile<Имя>/`; профили сохраняют и восстанавливают активную пользовательскую тему Kvantum целиком; живая палитра `$XDG_CONFIG_HOME/theme-profile/palette.json` сохраняется в профилях (компонент `palette`); подробности — `docs/PALETTE.md`;
- **ThemeBundleCore** (`src/core/theme_bundle.py`) собирает компоненты из установленных тем: KDE/Plasma (Global theme, Plasma Style, colors, decorations, icons, cursors, fonts), Desktop & Windows (обои, углы, blur, translucency, animations), Qt/Kvantum и GTK;
- выбор компонента идёт из списков установленных ресурсов; core-поля сначала стейджатся и применяются одним батчом через `AppThemeCore`;
- адаптеры приложений (`src/core/app_theme_adapters.py`) имеют уровень интеграции: Zed, VS Code, Cursor, Obsidian и btop умеют показывать/применять темы (Zed перечисляет имена из `themes[]` каждого файла-семейства в `~/.config/zed/themes` и в установленных расширениях); в `theme-bundle.json` попадают только приложения, чью тему можно применить (`apply_theme`), а `description` берётся из сохранения профиля; Discord читает текущий light/dark fallback и открывает нативные настройки; Telegram перечисляет `.tdesktop-theme`, но не пишет `tdata` — поэтому в UI для него нет выпадающего списка, только подпись «Тема выбирается в самом приложении»; VS Code/Cursor хранят в `workbench.colorTheme` поле `id` темы из `package.json` расширения (у Doki это UUID), а в UI показывается её `label`; Kitty и Alacritty — `integration=config`, без списка тем и авто-применения, с кнопкой **Open config**, которая открывает конфиг в системном редакторе через `xdg-open`;
- адаптеры перед записью копируют старый конфиг в `theme-profile/backups/app-theme-<timestamp>/`;
- профили v2 сохраняют `theme-bundle.json` с компонентными/приложенными ссылками поверх старого снимка; профили v1 открываются как legacy и по возможности синтезируют bundle из снимка;
- есть создание пользовательских производных тем (`create_custom_theme`): копия базовой color scheme в `$XDG_DATA_HOME/color-schemes` с перенесёнными живыми группами `[Colors:*]`/`[WM]` из `kdeglobals` (правки цветов из Theme Inspector не теряются) и Kvantum-темы в `$XDG_DATA_HOME/Kvantum`; системные базовые темы не изменяются;
- старый field-based Theme Inspector остаётся для Advanced/Customize (`SectionPage.qml`, `ThemeFieldRow.qml`), но не является главным экраном;
- для AI-агентов добавлен stdio MCP-сервер `mcp/theme_profile_mcp.py` (см. `docs/MCP.md`): перечисление тем, сборка/валидация/явное применение профилей, создание производных тем; удаление профилей не экспонируется;
- интерфейс на системном языке: QML через `qsTr`, Python через `tr()`; статические подписи групп/компонентов `ThemeBundleCore` помечены `QT_TRANSLATE_NOOP("ThemeBundle", …)` и переводятся при выдаче в QML; пайплайн `scripts/update-translations.sh` по-прежнему собирает `kde-theme-manager_ru.qm`.

Карточки профилей показывают палитру-свотчи, отпечаток темы, дату, число
компонентов (без дублирующих чипов) и описание (правится карандашом на
карточке, хранится в `manifest.json` профиля); клик по карточке только **выбирает**
профиль, применение — отдельной кнопкой **Apply** (с подтверждением, если так
настроено), а клик по профилю в трее применяет сразу. Если маркер
активного профиля указывает на удалённый каталог (сняли вне приложения),
при старте/рефреше он сбрасывается с предупреждением — иначе синхронизация
темы приложения уходила в system-detect и UI неожиданно светлел.
Дублирование — кнопкой
**Duplicate** на карточке (диалог с новым именем, `theme-profile clone SOURCE
TARGET` в CLI: копия снимка, `manifest.json` переименовывается, описание и
дата сохраняются, активный профиль не меняется). На главной странице кнопка
**Edit live theme…** сразу открывает инспектор в scope Live; в read-only
баннере инспектора — действие **Switch to Live**.

Инспектор — отдельный Python-модуль (`CoreFactory.theme_core()`), не часть
ядра `--core`, поэтому он работает независимо от значения `--core`.
Подробности: [THEME_INSPECTOR.md](THEME_INSPECTOR.md), ТЗ: [TZ-THEME-INSPECTOR.md](TZ-THEME-INSPECTOR.md).

---

## Пользовательские приложения (страница «Приложения»)

Реестр отслеживаемых приложений расширяется из GUI (drawer → *Applications*)
или через `theme-profile components <list|add|edit|remove>`. Встроенные
(20 шт.) отключаются галочкой, но не удаляются; свои добавляются,
редактируются и удаляются. Галочка = участие в save/apply: снятый id попадает
в `disabled_components` (app-preferences.json), файлы и снимки не трогаются.

Пример с реальным приложением — mpv:

```bash
theme-profile components add '{
  "id": "mpv",
  "name": "mpv",
  "category": "Media",
  "config_paths": [".config/mpv/mpv.conf", ".config/mpv/input.conf"]
}'
```

- `id` — уникален, не должен совпадать со встроенным; сменить нельзя
  (удалить и создать заново).
- `config_paths` — относительные пути файлов/папок от `$HOME`, без `..`;
  папки копируются рекурсивно.
- `reload_command` — опционально: одна команда после apply (один бинарник из
  PATH, без shell-операторов `| & ; < > $ \` " '` — валидация при
  регистрации и в GUI); ошибка команды не прерывает apply.
- Реестр живёт в `$THEME_ROOT/components.json` (schema v1); при совпадении id
  со встроенным компонентом встроенный выигрывает.

---

## Известные ограничения

1. **Обои не в профиле** — `theme-profile save/apply` не сохраняет/не применяет обои. Обои управляются отдельно через `wallpaper-*` скрипты.
2. **Meta+T не работает** — программируемый глобальный хоткей убран: KGlobalAccel создаёт
   объект-компонент только для приложений, у которых в `.desktop` есть
   `X-KDE-GlobalAccel-CommandShortcut=true`, поэтому голый D-Bus `doRegister`
   не давал рабочего шортката. Быстрое переключение доступно из трея.

Ранее числившиеся ограничения закрыты: гранулярность (`--components`), детект
приложений, превью/diff профилей, настраиваемая папка обоев, native Qt/QML UI,
Zen-автодетект профиля, выбор ядра через `--core`/`$KTM_CORE`/Settings — ядро
теперь одно (CLI-адаптер поверх Rust-бинарников).

---

## Структура директорий

```
$XDG_CONFIG_HOME/theme-profile/        # по умолчанию ~/.config/theme-profile/
├── active-profile          # имя активного профиля
├── profiles/
│   ├── glass/              # профиль glass
│   │   ├── .config/kwinrc
│   │   ├── .config/kdeglobals
│   │   └── ...
│   ├── paper/              # профиль paper
│   └── custom/             # пользовательские профили
├── backups/                # авто-бэкапы перед apply
│   └── pre-apply-glass-20260115-120000/
├── manifests/              # манифесты сохранения (опционально)
└── app-preferences.json    # настройки GUI
```

---

## Интеграция с RGB Control

Отдельная система: `~/rgb-control-kit/` → `rgb-control`, `rgb-menu`.
Синхронизирует Hyperion, OpenRazer, AULA F108 Pro, ASUS keyboard, OpenRGB.
Читает акцент из `~/.config/kdeglobals` (меняется при смене профиля через `theme-profile`).

---

## Связанные проекты

| Проект | Путь | Описание |
|--------|------|----------|
| `rgb-control-kit` | `~/rgb-control-kit/` | Управление RGB устройствами |
| `theme-profile-kit` | `~/theme-profile-kit/` | **Этот проект** (Rust CLI + Qt) |
| `taskmanager-newinstance` | `~/projects/kde_mods/taskmanager-newinstance/` | KWin таскбар мод (C++) |
| `mdock` | `~/projects/mdock/` | Markdown vault server (Go) |

---

## Планы (v2 — Qt/QML)

См. [TZ-QT-APP.md](TZ-QT-APP.md) — полное техническое задание на переписывание:
- Native KDE UI на Kirigami/QML — **сделано** (`gui/qt-app/`, см. `gui/qt-app/README.md`)
- Гранулярный выбор компонентов (галочки) — **сделано**
- Детект установленных приложений — **сделано**
- Dogfooding (приложение само темизируется) — **сделано**
- Clean Architecture для замены Core на Rust — **сделано**: ядро одно —
  CLI-адаптер (`core/cli_core.py`) поверх Rust-бинарников `theme-profile` /
  `wallpaper-*`, выбор через `--core core|auto`, `$KTM_CORE` или *Settings →
  Core*. Второе ядро (Rust-библиотека `libkde_theme_core.so`) убрано — паритет
  двух ядер больше не нужен, ядро покрыто `tests/test_gui.py`.
