# Kolira

> **Create, inspect and apply themes for KDE Plasma.**
> Kolira сохраняет оформление в переносимые темы, помогает собирать их из
> установленных компонентов и палитр, а затем применяет с резервной копией.

---

Импорт и применение проверяют назначения ресурсов темы: посторонние файлы HOME
не могут быть объявлены ресурсами оформления. При ошибке Inspector Apply
неудачные правки остаются подготовленными для повторения; успешные доступны
через Undo. Подробнее: [формат тем](docs/THEMES.md),
[Inspector](docs/THEME_INSPECTOR.md).

## Table of Contents / Оглавление

- [Overview](#overview--обзор)
- [Contents](#contents--состав)
- [Dependencies](#dependencies--зависимости)
- [Installation](#installation--установка)
- [Usage](#usage--использование)
- [Profiles](#profiles--профили)
- [Theme Inspector](#theme-inspector--инспектор-темы)
- [Update](#update--обновление)
- [Uninstall](#uninstall--удаление)
- [Rollback & Recovery](#rollback--восстановление)
- [Troubleshooting](#troubleshooting--устранение-неполадок)
- [Editing Rule](#editing-rule--правило-редактирования)
- [Hardening](#hardening--безопасность)
- [License](#license--лицензия)

---

## Overview / Обзор

Kolira управляет темами оформления KDE Plasma. Можно сохранить текущий вид,
создать тему из палитры или установленных компонентов, посмотреть изменения
перед применением, а затем вернуть сохранённое состояние. Темы могут включать
настройки Plasma и KWin, GTK/Qt, обои, терминалы и поддерживаемые приложения.
Приложение также ведёт историю смен и поддерживает восстановление и расписание.

## Contents / Состав

### Rust CLI (core)

| Script | Purpose |
|--------|---------|
| `kolira` | Rust CLI: save/apply/list/diff/delete/status профилей |
| `theme-menu` | wofi‑menu for applying, saving, restarting Plasma, status |
| `wallpaper-picker` | Choose wallpaper via wofi with thumbnail preview |
| `wallpaper-next` / `wallpaper-prev` | Cycle wallpapers |

### GUI Application (optional, installed by default)

| Component | Purpose |
|-----------|---------|
| `gui/qt-app/` | **kolira** — native Qt/QML + Kirigami desktop app with profile cards, wallpaper picker, settings and system tray; one core: a Python adapter over the Rust CLI binaries |

Run from the source tree during development:

```bash
cd gui/qt-app
uv venv && uv pip install -e ".[dev]"
uv run kolira-gui  # or: python -m src
```

For normal use install it once — see below.

Requires KDE Plasma 6 + Kirigami. See `gui/qt-app/README.md` for details.

## Dependencies / Зависимости

В приложении есть страница **«Зависимости»**: отдельные столбцы названия и
категории, галочки выбора, установка, удаление выбранных и переустановка.
Реестр показывает фактическое состояние и использующие ресурс темы, защищает
внешние/изменённые файлы. Перед удалением общей зависимости показывает затронутые
темы. «Больше не спрашивать» сохраняет выбор отдельно для магазинов и пакетов;
изменить его можно в настройках автоматизации. Прогресс, отмена и ошибки установки
доступны без блокировки интерфейса. [Подробности и CLI](docs/DEPENDENCIES.md).

Зависимости зависят от используемых возможностей. Для основных операций нужны
утилиты Plasma; меню и выбор обоев используют дополнительные программы:

| Package | Why |
|---|---|
| `wofi` | Menus (`theme-menu`, `wallpaper-picker`) |
| `plasma-workspace` | `kreadconfig6` / `kwriteconfig6` / `kbuildsycoca6` |
| `plasma-workspace` | `plasma-apply-wallpaperimage` |
| `imagemagick` (or `magick`) | Thumbnail generation (`wallpaper-picker`) |

Optional:
- `kitty` — live reload of terminal theme
- `dolphin` — "Open profile folder" button in `theme-menu`
- `rg` (ripgrep) — used in `status` for VS Code / Cursor theme detection (falls back gracefully)

## Installation / Установка

```bash
~/kolira/scripts/install-kolira.sh
```

Installs the command line scripts **and** the GUI (app copy, launcher, desktop
entry). Creates timestamped backups of every replaced file in
`~/.config/kolira/backups/`.

| Installed | Where |
|---|---|
| `kolira`, `theme-menu`, `wallpaper-picker`, `wallpaper-next`, `wallpaper-prev` | `~/.local/bin/` |
| kolira (app copy) | `~/.local/share/kolira/` |
| `kolira-gui` launcher | `~/.local/bin/` |
| desktop entry | `~/.local/share/applications/kolira.desktop` |
| app icon | `~/.local/share/icons/hicolor/scalable/apps/kolira-icon.svg` |

Install locations follow `XDG_BIN_HOME` / `XDG_DATA_HOME` when set.

The installer also delivers `README.md` and `AGENTS.md` into the user themes
directory (`~/.config/kolira/themes/`, XDG-aware). They describe offline authoring,
validation and explicit apply through CLI/MCP. Edited documents are preserved;
updates are written alongside as `.kit-new`.

**Your current look is saved first.** On the first install, when you have no
themes yet, the look you have now becomes the theme `my-original-look`, so you
can always come back: `kolira apply my-original-look` or the app's theme
list. An update never overwrites it; `--no-original-theme` skips it. Saving also
moves kitty's and Alacritty's look into their own include file (the look does not
change; the originals go to `backups/`).

**Automation choices.** Run in a terminal, the installer asks once what
applying a theme may do by itself — download from stores, install system
packages in a terminal (you type the password), restart Vesktop / Discord /
Zen / Firefox — each *ask*, *always* or *never* (`kolira automation
ask`). Otherwise the app asks on its first start; change it later in
*Settings → Automation* or with `kolira automation set KEY=POLICY`.

**Data is separate from code** — profiles, backups and settings live in
`$XDG_CONFIG_HOME/kolira/` (default `~/.config/kolira/`;
`themes/`, `backups/`, `manifests/`, `active-profile`,
`app-preferences.json`). Installation preserves user content. Rust migrates
legacy `kde-theme-profile/` and `theme-profile/` data without overwriting
conflicts; profile snapshots retain their `.config/...` layout.
See [rename compatibility](docs/RENAMING.md).
The applied theme itself is the usual KDE configuration (`kdeglobals`,
`kwinrc`, `gtk-3.0/settings.ini`, …).

Options:
```bash
install-kolira.sh --help       # show help
install-kolira.sh --only-cli   # Rust CLI only, no GUI
install-kolira.sh --only-gui   # GUI only (CLI must already be installed)
install-kolira.sh --no-venv    # use system PySide6, never build a venv
install-kolira.sh --with-venv  # always build a venv in the installed app
install-kolira.sh --restart    # restart a running kolira afterwards
```

A running kolira keeps executing the old code after an update. The
installer detects a copy running from the installed app directory and warns;
with `--restart` it stops it (SIGTERM — unapplied staged edits are lost) and
starts the new version.

A virtual environment is created inside the installed app only when the system
Python cannot `import PySide6` (e.g. `pyside6` is not installed from the
distro); with a system PySide6 the copy stays ~650 KB.

### Apply protection

Изменения через GUI и CLI выполняются по очереди. Если подготовка не удалась,
оформление не меняется. После сбоя или прерывания запись операции и резервная
копия остаются для проверки. Когда процесс завершился, старая запись сама по
себе не мешает новым изменениям. Посмотреть её или запустить восстановление:

```bash
kolira operations list
kolira operations show apply-ID
kolira operations recover apply-ID
```

Восстановление может обнаружить конфликт с более поздними личными изменениями.
Подробности и ограничения описаны в [правилах восстановления](docs/APPLY_PROTECTION.md).

### GUI core / Ядро GUI

The app has a **single core** — a Python adapter (`core/cli_core.py`) that
delegates to the Rust CLI binaries (`kolira`, `wallpaper-*`). There is
nothing to switch; `--core`/`KTM_CORE`/Settings → Core accept only `core` (or
`auto`, which is the same):

```bash
kolira-gui --core core      # core | auto — one and the same core
kolira-gui --list-cores     # show the core and what it delegates to
KTM_CORE=core kolira-gui    # per-run override
```

Inspector, application theme adapters, resource cloning, settings, registry and
wallpaper operations execute in Rust. Python handles Qt models and presentation.
See the [local JSON API and session contract](docs/CORE_API.md); update the CLI
and GUI together.

(The former second core — the `libkolira_core.so` shared library — was
removed; the app always starts on the single CLI-backed core.)

## Usage / Использование

```bash
kolira list               # list available profiles
kolira apply glass        # apply a theme (creates a pre-apply backup)
kolira apply glass --keep-wallpaper   # …but leave the wallpaper alone
kolira save my-profile    # save the current look as a theme
kolira save my-profile --no-wallpaper # a theme that never changes the wallpaper
kolira check glass        # what applying would miss + where to get it (+ lint warnings)
kolira lint glass         # icons shadowing Breeze, unreadable scheme/terminal colours
kolira migrate --all      # add theme.json to old snapshot profiles
kolira prune --all        # drop leftovers that are not part of a theme (backup first)
kolira export glass --out=glass.ktmtheme.zip  # portable copy ($HOME, no desktop icons)
kolira import glass.ktmtheme.zip [--name=NEW] [--force] [--apply]
kolira clone glass glass2 # duplicate a theme under a new name
kolira save glass --description="Dark glass"   # with a card description
kolira describe glass --set "New note"         # edit the description
kolira diff glass paper   # show config differences
kolira status             # diagnostic: dump 30+ config values
theme-menu                       # interactive wofi menu
wallpaper-picker                 # pick wallpaper with thumbnails
wallpaper-next                   # next wallpaper
wallpaper-prev                   # previous wallpaper
```

All commands support `--help` / `-h` for usage info.

Profile names may only contain letters, numbers, dots, underscores, and hyphens (`[A-Za-z0-9._-]`).

### Wallpapers / Обои

`wallpaper-next` / `wallpaper-prev` step relative to the wallpaper Plasma is
**actually showing** (read from
`~/.config/plasma-org.kde.plasma.desktop-appletsrc`), so they stay correct even
after you add or remove files. `~/.cache/wallpaper_index` is only a fallback for
when that config cannot be read.

The list itself is a shared contract between all three scripts and the Qt app:
same extensions (`jpg`, `jpeg`, `png`, `webp`, `bmp`), symlinks followed, and
**byte order** (`Vec<String>::sort()` in the shared Rust implementation;
GUI delegates to the same list).
The cached index is a *position* in that list, so a locale-aware sort
(`ru_RU.UTF-8`, which folds case and ignores punctuation) made the GUI resolve
the index to a different image than the one applied. All three scripts must keep
enumerating identically — `tests/test-scripts.sh` (section 9) fails if they drift.

**Wallpaper directory** (one Rust implementation, `kolira wallpaper dir`):
`$WALLPAPER_DIR` → `wallpaper_directory` in `app-preferences.json` → `$(xdg-user-dir PICTURES)/Wallpaper` → `~/Pictures/Wallpaper`. On a Russian locale XDG Pictures is `~/Изображения`, so the
default stays `~/Изображения/Wallpaper`; nothing is hard-coded to one language.
`kolira wallpaper current` prints the wallpaper Plasma shows on the
primary screen (`lastScreen=0`, `org.kde.image` only — slideshow/POTD report
nothing). One wallpaper is applied to all screens (`plasma-apply-wallpaperimage`).

**Wallpaper in themes.** `kolira save` copies the current wallpaper into
the theme (`wallpaper/<file>` + `source`) unless `--no-wallpaper` is given.
`kolira apply` — CLI, `theme-menu`, GUI, MCP alike — sets the theme's
wallpaper (putting the file back if it is gone; existing files are never
overwritten). A theme without a wallpaper leaves the current wallpaper on
screen, and `--keep-wallpaper` keeps it even when the theme has one. See
`docs/THEMES.md`.

Environment overrides (also used by the GUI and the tests): `WALLPAPER_DIR`,
`WALLPAPER_STATE`, `WALLPAPER_THUMB_DIR`, `PLASMA_APPLETSRC`,
`WALLPAPER_DRY_RUN=1` (print the chosen file instead of applying it).

`wallpaper-picker` feeds wofi with `--define dmenu-print_line_num=true`, which
prints the **0-based** index of the selected line (`wofi`'s `modes/dmenu.c` does
`uint16_t line_num = 0; … snprintf("%u", line_num++)` over the entries in input
order). Subtracting 1 there applied the image *above* the clicked one, so the
number is now used as a direct index into the displayed list — section 10 of the
test suite clicks lines 0, 3 and the last one against a stubbed `wofi`.

## Palette / Палитра

A generated theme starts from a **base16 palette + KDE accent** (any
[tinted-theming](https://github.com/tinted-theming/schemes) scheme imports as-is):

```bash
kolira palette gallery gruv      # 15 bundled schemes; --online for ~360 from tinted-theming
kolira palette import gallery:gruvbox-dark-medium   # or: palette import everforest.yaml
kolira palette from-wallpaper   # or build it from the current wallpaper
kolira palette set accent=#a7c080
kolira palette apply            # KDE scheme + kitty/Alacritty colours (backups first)
kolira palette apply --targets=kvantum   # recolour an installed Kvantum theme (KvArcDark/KvArc)
```

In the GUI: **Theme → Palette** (gallery, from wallpaper via Plasma's own
`Kirigami.ImageColors`, colour editing, apply targets). The live palette is
saved with profiles (component `palette`). Mapping, CLI and
limits: [`docs/PALETTE.md`](docs/PALETTE.md).

## Profiles / Профили

A profile **is a theme**: a full snapshot of every file that shapes the look
(KDE, KWin incl. the look of window rules, the look of panels — never their
layout, GTK, Kvantum, terminals with their
colours, launchers, editors, Zen, Obsidian, wallpaper) described by
`theme.json` (description, values, local assets, editor-theme sources).
Applying restores it after a pre-apply backup and reports what this machine
lacks, with where to get it. Format, CLI and GUI behaviour:
**`docs/THEMES.md`**.

Ready themes ship in `themes/` and are added by the installer (licences, sources
and dependencies: `themes/README.md`):

- **`glass`** — dark translucent glass, blur everywhere, monochrome Saturn wallpaper
- **`Paper`** — light, flat and warm, watercolour wallpaper
- **`everforest`** — Everforest Dark Medium, misty pine forest
- **`dracula`** — Dracula palette on glass, official Dracula wallpaper
- **`minecraft-overworld-dark`** — stone-grey and grass-green pixel theme, Kvantum/Klassy,
  dark Breeze panels, Glass-style blur and original voxel night wallpaper; requires Monocraft (see `themes/README.md`)

Start a new theme with `kolira new NAME --from=neutral-light`
(or `neutral-dark`, or an existing theme to copy) — GUI: **New theme…** — then
change the look and save over it. `kolira look-and-feel NAME` (or the
Save dialog's checkbox) adds a Look & Feel package with the look and no panel
layout.

Profiles live in `~/.config/kolira/themes/`. Old snapshot profiles keep
working until converted with `kolira migrate --all`.

## Theme bundles / Комплекты тем

The GUI's **Theme** page (drawer → *Theme*, or **Inspect** on a profile card) is
a high-level theme builder rather than a raw config editor. A bundle combines:

- KDE/Plasma: Global theme, Plasma Style, colors, decorations, icons, cursors,
  fonts;
- Desktop & Windows: wallpaper, corners, blur, translucency and animation;
- Qt/Kvantum and GTK;
- per-application theme adapters.

Installed themes are selected from lists wherever possible. Core KDE values are
staged and applied as one batch; **Undo/Revert** remain available after Apply.
Application adapters apply their own config updates immediately and copy the
previous file to `~/.config/kolira/backups/app-theme-<timestamp>/`.
Saving from the Theme page writes a theme package (`docs/THEMES.md`); old
profiles with a `theme-bundle.json` beside a snapshot are still read as legacy.

Supported application adapters currently include Zed, VS Code, Cursor,
Obsidian and btop. Discord reads its current light/dark background preference
and opens native settings; Telegram can list `.tdesktop-theme` files but cannot
read or apply its active theme, so the UI shows no theme picker for it. VS Code /
Cursor themes that declare an `id` in their extension manifest (e.g. Doki uses
UUIDs) are stored by that id in `workbench.colorTheme` and shown by their label. Kitty and
Alacritty use a config-only integration: the UI
does not pretend they have familiar theme entities, but exposes their config
file and opens it in the system's default editor via `xdg-open`.

An application counts as installed only when its executable (or Flatpak) is
found — `zeditor`/`zed`, `code`/`code-oss`/`codium`, `cursor`, … — a leftover
config alone shows "not installed — only an old config was found". **Zed, VS
Code and Cursor** have *Find in store…*: search the editor's theme store (Zed:
`api.zed.dev`; VS Code/Cursor: Open VSX) and install a theme (Zed themes are
copied to `~/.config/zed/themes/`, VS Code/Cursor use their own
`--install-extension`). CLI: `kolira apps store-search|store-install`. Profile
snapshots still include those config files when the apps are enabled in the
tracked component list. Unsupported capabilities are shown as fallbacks with a
button to open the app's native theme settings.

The repository also includes a local MCP server for AI agents:

```bash
uv run --project gui/qt-app python mcp/kolira_profile_mcp.py
```

It can enumerate installed themes, create user-owned derived color/Kvantum
themes, assemble profiles, validate theme references and explicitly apply a
profile. See [`docs/MCP.md`](docs/MCP.md).

Tests use a temporary `$HOME`; `KTM_THEME_NO_APPLY=1` suppresses external
commands (D-Bus, `plasma-apply-*`, `kvantummanager`) for dry runs.

## Update / Обновление

```bash
~/kolira/scripts/update-kolira.sh
```

Fetches latest from git (`--ff-only`), then reinstalls the scripts **and** the GUI
(re-runs the installer, so the launcher, desktop entry and icon are refreshed
too). Options go to the installer: `--restart` restarts a running kolira
on the new version. Warns if you have uncommitted local changes.

The ready themes follow the kit: new ones are added, and ones the kit changed are
replaced (the old one goes to `backups/`) unless you changed them; then the
installer says so once per version and gives the command. Themes installed before
this feature have no record of their original state, so they are kept the same way.
Nothing is applied.

## Uninstall / Удаление

```bash
~/kolira/scripts/uninstall-kolira.sh            # interactive (confirms)
~/kolira/scripts/uninstall-kolira.sh --force    # skip confirmation
~/kolira/scripts/uninstall-kolira.sh --only-cli # remove CLI only, keep GUI/data
~/kolira/scripts/uninstall-kolira.sh --only-gui # remove GUI only, keep CLI/data
```

Removes the scripts from `~/.local/bin/`, the app from
`~/.local/share/kolira/`, its launcher, desktop entry and icon, and
wipes the data directory (`$XDG_CONFIG_HOME/kolira/`, falling back to
`~/.config/kolira/`). The confirmation prompt is shown **before** any
file is removed, so answering "no" leaves the installation untouched. Does
**not** touch your KDE config files, wofi styles, or application settings —
those must be reverted manually.
See [Rollback & Recovery](#rollback--восстановление).

## Rollback / Восстановление

See [`docs/ROLLBACK.md`](docs/ROLLBACK.md) for full instructions. Quick summary:

- **Undo profile apply**: pre-apply backups are at `~/.config/kolira/backups/pre-apply-<name>-<ts>/`
- **Restore old scripts**: `cp -a ~/.config/kolira/backups/deploy-*/bin/. ~/.local/bin/`
- **Restore the previous app**: `cp -a ~/.config/kolira/backups/deploy-kolira-*/app/. ~/.local/share/kolira/`
- **Plasma won't start**: `systemctl --user start plasma-plasmashell.service`
- **Full remove**: `uninstall-kolira.sh --force`

## Troubleshooting / Устранение неполадок

| Problem | Fix |
|---|---|
| `command not found: kreadconfig6` | Install `plasma-workspace`: `sudo pacman -S plasma-workspace` |
| `command not found: wofi` | Install: `sudo pacman -S wofi` |
| wofi menu appears empty | Ensure `~/.config/wofi/style.css` exists |
| `kolira list` shows nothing | `mkdir -p ~/.config/kolira/themes && kolira save my-profile` |
| Wallpaper picker shows no images | Check `kolira wallpaper dir` (XDG Pictures/Wallpaper by default). Override with `export WALLPAPER_DIR=...` or the GUI setting |
| Plasma won't start after `apply` | `systemctl --user start plasma-plasmashell.service` |

## Editing Rule / Правило редактирования

1. Edit `~/kolira/gui/qt-app/src/core/rust_core/**` first.
2. Re-run `scripts/install-kolira.sh` (it builds the Rust CLI with cargo).
3. Do **not** edit `~/.local/bin/` directly — copy changes back into the kit immediately.

## Hardening / Безопасность

- **Profile names** are validated (`[A-Za-z0-9][A-Za-z0-9._-]{0,63}`) in Rust
  and Python before touching the filesystem; `..`, separators and shell
  metacharacters are rejected, and `diff` cannot traverse outside `themes/`.
  `save`/`clone` also reject a name that differs from an existing profile only
  by case (`paper` next to `Paper`); the GUI save dialogs warn as you type.
- **Custom component lists** are validated against the known component set; an
  explicitly empty selection is rejected instead of silently applying everything.
- **Config writes are atomic and permission-preserving**: an existing file keeps
  its mode, a new file is created `0600`, and `app-preferences.json` is private.
- **A revert** of a Theme Inspector session also removes files that the session
  created, while keeping the on-disk backup of what it changed.
- **Installers** escape a checkout path before `sed` substitution and take
  backups with unique names (no silent overwrite on same-second runs).

## License / Лицензия

MIT — see [`LICENSE`](LICENSE).

Fastfetch (конфиг и логотип) и меню запуска Plasma (включая значок кнопки)
не входят в темы и сохраняются без изменений, в том числе при применении старых тем.
Simple Application Launcher не требуется и не устанавливается.

### Просмотр перед применением

В GUI и трее Apply сначала открывает список изменений с diff, сбросами,
зависимостями и перезапусками. Здесь можно сохранить текущее оформление и
оставить обои. При изменении исходного состояния требуется новое подтверждение.
CLI: `kolira preview Paper --json`, затем
`kolira apply Paper --preview-token=TOKEN`.
[Подробности и границы предварительного расчёта](docs/APPLY_PREVIEW.md).

### Расписание и история смен

В меню приложения доступны расписание тем и история последних 10 смен с
подробностями и переходом к резервной копии. Восстановление завершённой смены
сначала создаёт страховочную копию текущего состояния. Успешная смена не
показывает всплывающий отчёт; ошибки сохраняются в истории.

Расписание работает в Plasma и при закрытом окне приложения. Выберите тему,
время, дни недели, сохраните правило и включите исполнитель. Пропущенные во
время сна события не догоняются; недостающие зависимости устанавливаются
вручную. [Правила времени и CLI](docs/SCHEDULING.md).

Для агентов добавлен MCP `prepare_theme` / Rust API `agent.prepare`: подготовка
нового пакета без смены живого оформления. Установщик доставляет инструкции в
пользовательский каталог `themes/`, сохраняя изменённые пользователем файлы.
[Подключение MCP и рецепт](docs/MCP.md).

### Читаемость и язык GUI

Поддерживаются окно 640×480 и увеличенный системный шрифт: действия переносятся,
полосы не перекрывают текст, диалоги имеют внутренние поля. Цвета палитры доступны
с клавиатуры, ошибки видны на текущем экране, сводки профилей загружаются в фоне.
Русский выбирается по системной локали и охватывает известные статусы ядра,
историю/расписание и локальные форматы. Имена, пути и неизвестные внешние сообщения
сохраняются без перевода.

Переименование и совместимость старых данных/тем: [docs/RENAMING.md](docs/RENAMING.md).
