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

Только скрипты: `install-theme-profile-kit.sh --no-gui`.
Отдельно GUI: `scripts/install-kde-theme-manager.sh` (venv создаётся лишь
если в системе нет PySide6) и `scripts/uninstall-kde-theme-manager.sh`.

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
- `.zen/<профиль>/user.js` — каталог профиля авто-детектится (первый с `user.js`)
- `.zen/<профиль>/chrome/userChrome.css`
- `.zen/<профиль>/chrome/userContent.css`
- `.zen/<профиль>/chrome/zen-themes.css`

### Notes (опционально)
- `.config/obsidian/obsidian.json`, `Preferences`
- `obsidian/vault/.obsidian/appearance.json`

### System Profiles
- `.local/share/color-schemes/ThemeProfilePaper.colors`
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

## GUI: Theme Inspector

Qt/QML-приложение (`gui/qt-app/`) уже ставится установщиком рядом с Rust CLI. Помимо профилей и обоев в нём есть страница **Theme**:

- 69 live-полей в секциях Colors, Typography, Windows & Effects, Desktop & Assets, Applications + Advanced;
- переключатель scope `Live | <profile>` и diff по полям; скопы профилей —
  **read-only** снимки (Undo/Revert для них отключены);
- валидация (схемы, L&F, иконки, Kvantum, шрифты, конфликт blur) с кнопкой **Fix**;
- редактирование с мгновенным применением, Undo/Revert и снимком в
  `theme-profile/backups/inspector-<ts>/`; при Revert файлы, созданные сессией,
  удаляются, а бэкап изменённого остаётся на диске;
- экспорт снимка в JSON, ссылки в `kcmshell6`, открытие папки конфигов;
- read-only просмотр конфигов отслеживаемых приложений: дерево
  «приложение → файл → параметр» для scope `live`/`profile:<name>`
  (INI/JSON/JSONC/TOML/user.js/line-based);
- правка INI/JSON/TOML/user.js/line-based по ключу из того же дерева
  (live-only): диалог с превью «было → станет», тип значения сохраняется
  (число/bool/список), TOML — секции `[section]`/`[a.b]` (inline-таблицы
  без заголовка отклоняются), line-based — в родном `key value` формате
  (kitty.conf), записи в общем undo/revert сессии; для экзотики (CSS,
  бинарники) — кнопка «Show in folder» вместо правки по ключу.

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
