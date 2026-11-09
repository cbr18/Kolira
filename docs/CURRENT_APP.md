# Текущее приложение: theme-profile-kit (0.6.0)

> **Статус**: 0.6.0 — Rust CLI + Qt/QML приложение KDE Theme Manager.
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

Создаёт бэкап предыдущих версий в `~/.config/kde-theme-profile/backups/`.
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
`$XDG_CONFIG_HOME/kde-theme-profile/` (по умолчанию `~/.config/kde-theme-profile/`), и
установкой не затрагиваются. Существующий legacy-каталог
`~/.config/kde-theme-profile/` определяется и сохраняется; снимки профилей всегда
используют историческую раскладку `.config/...`, поэтому профили, созданные до
поддержки XDG, продолжают работать.

---

## Использование

```bash
theme-profile list               # список профилей
theme-profile apply glass        # применить тему (--keep-wallpaper — не трогать обои)
theme-profile save my-profile    # сохранить текущее оформление как тему-пакет
theme-profile check glass        # чего не хватит при применении и где взять
theme-profile migrate --all      # добавить theme.json старым профилям-снимкам
theme-profile export glass --out=glass.ktmtheme.zip   # переносимая копия темы
theme-profile import glass.ktmtheme.zip               # [--name=NEW] [--force] [--apply]
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

Профили хранятся в `$XDG_CONFIG_HOME/kde-theme-profile/themes/` (по умолчанию
`~/.config/kde-theme-profile/themes/`).
Активный профиль — маркер `theme-profile/active-profile` в том же корне.

**Профиль = тема** (формат и поведение — `docs/THEMES.md`): полный снимок всех
файлов, влияющих на внешний вид (TRACKED_PATHS ниже: KDE, KWin вместе с
правилами окон, панели, GTK, Kvantum, терминалы, wofi, редакторы, Zen,
Obsidian…), плюс `theme.json` (описание, значения для GUI, ресурсы, источники
тем редакторов, обои). Применение — восстановление снимка (описано ниже) плюс
доустановка ресурсов, отчёт о недостающем со ссылками и правило обоев: тема без
обоев или `--keep-wallpaper` оставляет текущие обои. Формат «только значения»
(`theme@1`, 2026-11-04) терял цвета терминала, GTK4, wofi, Zen и эффекты KWin —
от него отказались.

При apply Rust-ядро после восстановления файлов заново применяет цветовую схему через `plasma-apply-colorscheme`. Если в профиле есть явный `AccentColor` и `accentColorFromWallpaper=false`, он передаётся через `--accent-color`; если `accentColorFromWallpaper=true`, схема применяется без `--accent-color`, а акцент пересчитывает сама Plasma из восстановленных обоев при перезапуске plasmashell (проверено 2026-11-04; ImageMagick для акцента больше не используется). Так как `kdeglobals` восстанавливается копированием файла, `apply` дополнительно: (1) переписывает `accentColorFromWallpaper` через `kwriteconfig6 --notify` (сначала противоположным значением, затем настоящим) — иначе kded-сервис `plasma_accentcolor_service` не узнаёт о смене флага и отбрасывает акцент из обоев; (2) шлёт `org.kde.KGlobalSettings.notifyChange(0,0)` (PaletteChanged) — `plasma-apply-colorscheme` на уже записанную схему ничего не делает («уже используется»), и без сигнала открытые приложения оставались с палитрой прошлого профиля (оливковый paper после glass). На монохромных обоях (например, чёрно-белые обои glass) Plasma не может извлечь цвет — акцент становится схемным/серым, это поведение самой Plasma. Отдельно переприменяется Qt widget style: для `widgetStyle=kvantum` вызывается `kvantummanager --set <theme>`, чтобы уже запущенные Qt-приложения получили тему Kvantum, а не только обновлённый файл. Обои профиля: при `save` копия текущих обоев кладётся в `<профиль>/wallpaper/` (+ `source` с исходным путём), при `apply` используется оригинал, а если его нет — копия возвращается на место (или в каталог обоев) и применяется; это работает одинаково для CLI, `theme-menu`, GUI и MCP. Каталог обоев: `$WALLPAPER_DIR` → настройка `wallpaper_directory` → `$(xdg-user-dir PICTURES)/Wallpaper` → `~/Pictures/Wallpaper` (`theme-profile wallpaper dir`). Профиль `glass` имеет `accentColorFromWallpaper=true` (акцент из обоев), профиль `paper` — `accentColorFromWallpaper=false` с явным `AccentColor=155,150,77`. Поэтому акцент переключается между профилями: `glass` берёт акцент из обоев, `paper` — оливковый из явного `AccentColor`.

---

## Что отслеживается (TRACKED_PATHS)

### KDE/Plasma (базовые, всегда)
- `.config/kwinrc` — KWin эффекты, правила
- `.config/kwinrulesrc` — правила окон (только свойства оформления)
- `.config/kdeglobals` — цвета, акцент, Look & Feel, цветовая схема
- `.config/plasmarc` — настройки Plasma
- `.config/plasma-org.kde.plasma.desktop-appletsrc` — только вид: обои рабочих столов, Panel Colorizer (выключенный — только сам выключатель); раскладка панелей и виджетов остаётся пользовательской (`rust_core/src/layout.rs`)
- `.config/plasmashellrc` — только `floating`/`panelOpacity` панелей (размеры и автоскрытие — пользовательские)

### GTK
- `.config/gtk-3.0/settings.ini`, `gtk.css`, `colors.css`
- `.config/gtk-4.0/settings.ini`, `gtk.css`, `colors.css`

### Qt
- `.config/Trolltech.conf` (палитра Qt, которую KDE выводит из цветовой схемы)

`QtProject.conf` не сохраняется: это состояние диалога файлов (последняя
папка, вид), а не оформление — восстановление откатывало диалог.

### Window Decorations (опционально)
- `.config/breezerc` — стиль Breeze, хранится только если стиль приложений или оформление окон — Breeze (компонент `breeze`; прозрачность меню `[Style] MenuOpacity`, поле инспектора «Breeze menu opacity»)
- `.config/klassy/klassyrc`, `windecopresetsrc` — хранятся только если стиль или оформление — Klassy
- `.config/Kvantum/kvantum.kvconfig` + папка семейства **активной** темы Kvantum

### Terminals (опционально)
- `.config/alacritty/theme-profile.toml` (оформление; `alacritty.toml` только импортирует его)
- `.config/kitty/theme-profile.conf` (оформление; `kitty.conf` только подключает его через `include`) — с live reload через `kitty @`

Журнал применения (`<theme root>/apply-journal/`, `rust_core/src/undo.rs`): следующее
применение откатывает то, что оставила предыдущая тема; см. `docs/THEMES.md`.

Смешанные файлы (`kdeglobals`, `kwinrc`, `plasmarc`, `plasmashellrc`, GTK
`settings.ini`, `btop.conf`, `settings.json` редакторов) сохраняются и
применяются **только ключами оформления** со слиянием в живой файл — масштаб
экрана, рабочие столы, тайлинг, клавиши, SSH-хосты и т.п. не трогаются; таблица
— `docs/THEMES.md`, код — `rust_core/src/appearance.rs`.

- Konsole — `[Appearance]` профиля по умолчанию + его `.colorscheme` (`rust_core/src/konsole.rs`)

### Launchers/Menu (опционально)
- `.config/wofi/style.css`, `wallpaper.css`

### Monitors (опционально)
- `.config/btop/btop.conf` + файл **активной** темы из `themes/`

### Sysinfo (опционально)

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
- `obsidian/vault/.obsidian/appearance.json`

`obsidian.json` (список хранилищ) и `Preferences` (проверка орфографии,
масштаб) не сохраняются — это не оформление.

**Значки рабочего стола.** `plasma-org.kde.plasma.desktop-appletsrc` (панели,
виджеты, рабочий стол) восстанавливается целиком, кроме положения значков:
ключи `positions`, `changedPositions`, `lastResolution` и группа
`[ScreenMapping]` берутся из текущего файла (прочитанного после остановки
plasmashell), так что смена темы не двигает значки.

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

- **Страница «Палитра»** в GUI (Тема → Палитра): галерея, «Из обоев» (`Kirigami.ImageColors`), «Из обоев (подробно)» (k-means), правка цветов, применение по целям; настройка «Сетевые функции»;
- **Палитра** (`rust_core/src/palette.rs`, `theme-profile palette …`): base16 + акцент → сгенерированная цветовая схема KDE `ThemeProfile<Имя>.colors`, блок цветов в `kitty.conf` (между маркерами, live-reload по сокетам `/tmp/kitty-theme-profile-<pid>`) и `[colors.*]` в `alacritty.toml`; по запросу — тема Kvantum, перекрашенная из установленной базовой (`KvArcDark`/`KvArc`) в `~/.config/Kvantum/ThemeProfile<Имя>/`; профили сохраняют и восстанавливают активную пользовательскую тему Kvantum целиком; живая палитра `$XDG_CONFIG_HOME/kde-theme-profile/palette.json` сохраняется в профилях (компонент `palette`); подробности — `docs/PALETTE.md`;
- **ThemeBundleCore** (`src/core/theme_bundle.py`) собирает компоненты из установленных тем: KDE/Plasma (Global theme, Plasma Style, colors, decorations, icons, cursors, fonts), Desktop & Windows (обои, углы, blur, translucency, animations), Qt/Kvantum и GTK;
- выбор компонента идёт из списков установленных ресурсов; core-поля сначала стейджатся и применяются одним батчом через `AppThemeCore`;
- установленность приложения определяется по исполняемому файлу или Flatpak (`binaries`/`flatpak_ids` адаптера: Zed — `zeditor`/`zed`/`dev.zed.Zed`, VS Code — `code`/`code-oss`/`codium`/`vscodium`, …); только конфиг без программы — статус «не установлено — найден только старый конфиг»; компоненты профиля (`tracked_apps`) принимают альтернативы `a|b` в `detect_command`;
- магазины тем редакторов (`rust_core/src/stores.rs`, `theme-profile apps store-search|store-install`, GUI «Найти в магазине…»): Zed — индекс `api.zed.dev` (`provides=themes`), установка = `themes/*.json` из архива расширения в `~/.config/zed/themes/<id>--<файл>.json`; VS Code/Cursor — поиск Open VSX, установка CLI редактора `--install-extension <id>` (расширения VSCodium/OSS — `~/.vscode-oss/extensions`); сеть — через общий `net.rs` (отключаемо);
- адаптеры приложений (`src/core/app_theme_adapters.py`) имеют уровень интеграции: Zed, VS Code, Cursor, Obsidian и btop умеют показывать/применять темы (Zed перечисляет имена из `themes[]` каждого файла-семейства в `~/.config/zed/themes` и в установленных расширениях); Discord читает текущий light/dark fallback и открывает нативные настройки; Telegram перечисляет `.tdesktop-theme`, но не пишет `tdata` — поэтому в UI для него нет выпадающего списка, только подпись «Тема выбирается в самом приложении»; VS Code/Cursor хранят в `workbench.colorTheme` поле `id` темы из `package.json` расширения (у Doki это UUID), а в UI показывается её `label`; Kitty и Alacritty — `integration=config`, без списка тем и авто-применения, с кнопкой **Open config**, которая открывает конфиг в системном редакторе через `xdg-open`;
- адаптеры перед записью копируют старый конфиг в `theme-profile/backups/app-theme-<timestamp>/`;
- сохранение профиля пишет тему-пакет через Rust CLI (`theme-profile save`, без отдельного `theme-bundle.json`); область профиля на странице Theme читает значения из `theme.json`; старые профили с `theme-bundle.json`/снимком открываются как legacy; диалог сохранения — флажок «Include the current wallpaper», диалог применения — «Keep the current wallpaper», после применения — диалог «Missing Theme Parts» со ссылками и кнопкой «Install from stores» (`apply --install-missing`);
- есть создание пользовательских производных тем (`create_custom_theme`): копия базовой color scheme в `$XDG_DATA_HOME/color-schemes` с перенесёнными живыми группами `[Colors:*]`/`[WM]` из `kdeglobals` (правки цветов из Theme Inspector не теряются) и Kvantum-темы в `$XDG_DATA_HOME/Kvantum`; системные базовые темы не изменяются;
- старый field-based Theme Inspector остаётся для Advanced/Customize (`SectionPage.qml`, `ThemeFieldRow.qml`), но не является главным экраном;
- для AI-агентов добавлен stdio MCP-сервер `mcp/theme_profile_mcp.py` (см. `docs/MCP.md`): перечисление тем, сборка/валидация/явное применение профилей, создание производных тем; удаление профилей не экспонируется;
- интерфейс на системном языке: QML через `qsTr`, Python через `tr()`; статические подписи групп/компонентов `ThemeBundleCore` помечены `QT_TRANSLATE_NOOP("ThemeBundle", …)` и переводятся при выдаче в QML; пайплайн `scripts/update-translations.sh` по-прежнему собирает `kde-theme-manager_ru.qm`.
- тексты из данных тоже переводятся: подписи и описания полей инспектора
  (`theme_fields.json`, контекст `ThemeFields`), описания приложений
  (`TrackedApps`), тексты ядра — пункты автоматизации и действия после
  применения (`Core`, в QML `qsTranslate("Core", …)`), строки отчётов ядра
  (`CoreMessages`: `core/core_messages.py` узнаёт известные строки — «installed …
  into …», «… restarted» — и собирает их из переводимых шаблонов; CLI остаётся
  английским для скриптов). `gui/qt-app/tools/gen_i18n_strings.py` пишет
  `src/core/i18n_strings.py` (QT_TRANSLATE_NOOP для pylupdate6) и вписывает
  переводы из `tools/i18n_ru.py` в `py_ru.ts`; `update-translations.sh` вызывает
  его до и после pylupdate6. Тесты проверяют, что файл строк покрывает поля,
  приложения и тексты ядра, а каталоги переведены полностью.

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

Инспектор использует единое Rust-ядро: чтение, staging, Apply, Undo/Revert,
настройки, темы приложений и клонирование ресурсов исполняются в Rust.
`CoreFactory.theme_core()` возвращает Python-адаптер моделей Qt.
Контракт: [CORE_API.md](CORE_API.md).
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
$XDG_CONFIG_HOME/kde-theme-profile/        # по умолчанию ~/.config/kde-theme-profile/
├── active-profile          # имя активного профиля
├── profiles/
│   ├── glass/              # тема (docs/THEMES.md)
│   │   ├── theme.json
│   │   ├── manifest.json
│   │   ├── wallpaper/
│   │   ├── .config/kwinrc, .config/kdeglobals, …
│   │   └── .local/share/…  # свои цветовые схемы, Look & Feel
│   ├── Paper/
│   └── custom/             # пользовательские темы
├── backups/                # авто-бэкапы
│   ├── pre-apply-glass-20260115-120000/    # все отслеживаемые файлы до apply
│   └── pre-save-glass-…/                   # прежняя версия перезаписанной темы
├── manifests/              # манифесты сохранения (опционально)
├── installed-assets.json   # единый реестр зависимостей, схема 2: источники/ресурсы/владение
├── dependency-operations/  # состояние операций установки, результаты и токены отмены
├── dependency-locks/       # блокировки реестра и ресурсов
└── app-preferences.json    # настройки GUI + выбор автоматизации ("automation", ядро)
```

---

Страница **«Зависимости»** показывает требования приложения и тем, позволяет
выбрать ресурсы галочками и установить/удалить/переустановить их. Название и
категория — отдельные столбцы. Rust ведёт реестр и операции; Python получает
JSONL-события асинхронно через QProcess. Старые записи источников мигрируются
с резервной копией, без присвоения права удаления. Повреждённый реестр требует
восстановления и не заменяется пустым. См. [контракт](DEPENDENCIES.md).

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
- Второе FFI-ядро удалено: backend один —
  CLI-адаптер (`core/cli_core.py`) поверх Rust-бинарников `theme-profile` /
  `wallpaper-*`, выбор через `--core core|auto`, `$KTM_CORE` или *Settings →
  Core*. Второе ядро (Rust-библиотека `libkde_theme_core.so`) убрано — паритет
  двух backend больше не нужен. Перенос оставшейся логики выполнен;
  [контракт API и сессий](CORE_API.md),
  [результат переноса](../plans/2026_11_08_single-core-completion.md).

Fastfetch (конфиг и логотип) и меню запуска Plasma (включая значок кнопки)
не входят в темы и сохраняются без изменений, в том числе при применении старых тем.
Simple Application Launcher не требуется и не устанавливается.

## Защита Apply

Профильные Apply сериализуются через Rust. Подготовка проверяется до записи;
сбой сохраняет бэкап и запускает проверяемое восстановление. Прерванные операции
показываются в GUI и блокируют новые изменения до `operations recover`.
Страница истории 10 смен тем остаётся отдельной задачей.
[Подробности и ограничения](APPLY_PROTECTION.md).

Кнопки Paper используют общий `AppButton`: при наведении остаётся штатное
затемнение без внутренней светлой рамки. Рамка фокуса сохраняется для клавиатуры;
при переключении с Paper возвращается обычное поведение системного стиля.

## Просмотр перед Apply

GUI/трей используют асинхронный Rust-план изменений с проверяемым токеном,
сохранением текущего оформления и отдельной установкой зависимостей.
CLI включает `preview`; [контракт и пределы](APPLY_PREVIEW.md).

### Расписание, история и инструкции агентам

- Rust `schedule.rs`: локальные правила в `schedule.json`, исполнитель
  `theme-profile schedule daemon`, XDG autostart в Plasma; [контракт](SCHEDULING.md).
- История операций: 10 завершённых плюс все pending, связь с проверяемыми копиями
  и восстановление завершённой смены с новой страховочной транзакцией.
- Rust `agent.rs`: offline-подготовка пакета и доставка workspace-инструкций в
  `themes/`. `agent-instructions.json` отслеживает версии содержимого;
  пользовательские изменения сохраняются, обновление пишется в `.kit-new`.
- Установленный MCP: `$XDG_DATA_HOME/kde-theme-manager/theme_profile_mcp.py`,
  запускается Python окружения GUI с абсолютным путём, независимо от cwd.

Live-проверка новых сценариев и обнаружение инструкций реальными агентскими
клиентами имеют отдельный статус в карточках задач. Наличие файла не доказывает,
что произвольный клиент его автоматически прочитал.

## Уточнения UI после аудита 2026-11-08

Минимум 640×480 сохранён: действия переносятся, выбор темы приложения имеет
отдельную строку, длинный текст ограничен областью содержимого. Диалоги имеют
общие внутренние поля; полосы не перекрывают строки. Вертикальная полоса
зависимостей доступна в любом положении горизонтальной прокрутки. Карточки
получают сводки одним фоновым Rust-запросом, без обходов профилей из QML.
Цвета палитры управляются клавиатурой; фокус карточек виден. Ошибки показываются
на активном экране/внутри редактора. Русский включает известные статусы ядра,
историю и расписание, локальные форматы и названия компонентов. Пути, имена и
неизвестная внешняя диагностика сохраняются. Проверки и ограничения перечислены
в [задаче](../tasks/2026_11_08_ui-consistency-and-localization.md).
