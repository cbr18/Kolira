# theme-profile-kit / Тема‑профиль‑кит

> **Save and switch KDE Plasma themes with one command.**
> Переключайтесь между профилями оформления **glass** (полупрозрачное стекло) и **paper** (светлая/solid тема), сохраняя и восстанавливая десятки настроек.

---

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

Позволяет переключаться между темами оформления **glass** (полупрозрачное стекло с размытием) и **paper** (светлая/solid тема) одной командой, сохраняя и восстанавливая десятки настроек: KWin эффекты, темы GTK/Qt, настройки терминалов (kitty, Alacritty), редакторов (Zed, VS Code, Cursor), браузера Zen, Obsidian, btop, fastfetch, wofi, цветовые схемы Plasma и больше.

## Contents / Состав

### Rust CLI (core)

| Script | Purpose |
|--------|---------|
| `theme-profile` | Rust CLI: save/apply/list/diff/delete/status профилей |
| `theme-menu` | wofi‑menu for applying, saving, restarting Plasma, status |
| `wallpaper-picker` | Choose wallpaper via wofi with thumbnail preview |
| `wallpaper-next` / `wallpaper-prev` | Cycle wallpapers |

### GUI Application (optional, installed by default)

| Component | Purpose |
|-----------|---------|
| `gui/qt-app/` | **KDE Theme Manager** — native Qt/QML + Kirigami desktop app with profile cards, wallpaper picker, settings and system tray; one core: a Python adapter over the Rust CLI binaries |

Run from the source tree during development:

```bash
cd gui/qt-app
uv venv && uv pip install -e ".[dev]"
uv run kde-theme-manager  # or: python -m src
```

For normal use install it once — see below.

Requires KDE Plasma 6 + Kirigami. See `gui/qt-app/README.md` for details.

## Dependencies / Зависимости

Everything ships with KDE Plasma 6 — nothing extra to install:

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
~/theme-profile-kit/scripts/install-theme-profile-kit.sh
```

Installs the command line scripts **and** the GUI (app copy, launcher, desktop
entry). Creates timestamped backups of every replaced file in
`~/.config/theme-profile/backups/`.

| Installed | Where |
|---|---|
| `theme-profile`, `theme-menu`, `wallpaper-picker`, `wallpaper-next`, `wallpaper-prev` | `~/.local/bin/` |
| KDE Theme Manager (app copy) | `~/.local/share/kde-theme-manager/` |
| `kde-theme-manager` launcher | `~/.local/bin/` |
| desktop entry | `~/.local/share/applications/kde-theme-manager.desktop` |
| app icon | `~/.local/share/icons/hicolor/scalable/apps/ktm-theme-manager.svg` |

Install locations follow `XDG_BIN_HOME` / `XDG_DATA_HOME` when set.

**Data is separate from code** — profiles, backups and settings live in
`$XDG_CONFIG_HOME/theme-profile/` (default `~/.config/theme-profile/`;
`profiles/`, `backups/`, `manifests/`, `active-profile`,
`app-preferences.json`) and are never touched by an install. An existing legacy
`~/.config/theme-profile/` is detected and kept, and profile snapshots always
use the historical `.config/...` layout, so pre-XDG profiles keep working.
The applied theme itself is the usual KDE configuration (`kdeglobals`,
`kwinrc`, `gtk-3.0/settings.ini`, …).

Options:
```bash
install-theme-profile-kit.sh --help       # show help
install-theme-profile-kit.sh --only-cli   # Rust CLI only, no GUI
install-theme-profile-kit.sh --only-gui   # GUI only (CLI must already be installed)
install-theme-profile-kit.sh --no-venv    # use system PySide6, never build a venv
install-theme-profile-kit.sh --with-venv  # always build a venv in the installed app
install-theme-profile-kit.sh --restart    # restart a running KDE Theme Manager afterwards
```

A running KDE Theme Manager keeps executing the old code after an update. The
installer detects a copy running from the installed app directory and warns;
with `--restart` it stops it (SIGTERM — unapplied staged edits are lost) and
starts the new version.

A virtual environment is created inside the installed app only when the system
Python cannot `import PySide6` (e.g. `pyside6` is not installed from the
distro); with a system PySide6 the copy stays ~650 KB.

### GUI core / Ядро GUI

The app has a **single core** — a Python adapter (`core/cli_core.py`) that
delegates to the Rust CLI binaries (`theme-profile`, `wallpaper-*`). There is
nothing to switch; `--core`/`KTM_CORE`/Settings → Core accept only `core` (or
`auto`, which is the same):

```bash
kde-theme-manager --core core      # core | auto — one and the same core
kde-theme-manager --list-cores     # show the core and what it delegates to
KTM_CORE=core kde-theme-manager    # per-run override
```

(The former second core — the `libkde_theme_core.so` shared library — was
removed; the app always starts on the single CLI-backed core.)

## Usage / Использование

```bash
theme-profile list               # list available profiles
theme-profile apply glass        # apply a profile (creates a pre-apply backup)
theme-profile save my-profile    # save current state as a new profile
theme-profile clone glass glass2 # duplicate a profile snapshot under a new name
theme-profile save glass --description="Dark glass"   # with a card description
theme-profile describe glass --set "New note"         # edit the description
theme-profile diff glass paper   # show config differences
theme-profile status             # diagnostic: dump 30+ config values
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
**byte order** (`LC_ALL=C sort`, i.e. `Path.is_file()` + `sorted()` in Python).
The cached index is a *position* in that list, so a locale-aware sort
(`ru_RU.UTF-8`, which folds case and ignores punctuation) made the GUI resolve
the index to a different image than the one applied. All three scripts must keep
enumerating identically — `tests/test-scripts.sh` (section 9) fails if they drift.

**Wallpaper directory** (one Rust implementation, `theme-profile wallpaper dir`):
`$WALLPAPER_DIR` → `wallpaper_directory` in `app-preferences.json` → `$(xdg-user-dir PICTURES)/Wallpaper` → `~/Pictures/Wallpaper`. On a Russian locale XDG Pictures is `~/Изображения`, so the
default stays `~/Изображения/Wallpaper`; nothing is hard-coded to one language.
`theme-profile wallpaper current` prints the wallpaper Plasma shows on the
primary screen (`lastScreen=0`, `org.kde.image` only — slideshow/POTD report
nothing). One wallpaper is applied to all screens (`plasma-apply-wallpaperimage`).

**Wallpaper in profiles.** `theme-profile save` copies the current wallpaper into
`<profile>/wallpaper/` (plus `source` — its original path) whenever the desktop
layout (`plasma_shell`) is saved. `theme-profile apply` — CLI, `theme-menu`, GUI,
MCP alike — uses the original if it still exists, otherwise puts the saved copy
back (old path, else the wallpaper directory; existing files are never
overwritten) and applies it. Profiles saved before this fall back to the
`wallpaper` path in `theme-bundle.json`. `validate_profile` reports a wallpaper
that is neither on disk nor saved.

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
theme-profile palette gallery gruv      # 15 bundled schemes; --online for ~360 from tinted-theming
theme-profile palette import gallery:gruvbox-dark-medium   # or: palette import everforest.yaml
theme-profile palette from-wallpaper   # or build it from the current wallpaper
theme-profile palette set accent=#a7c080
theme-profile palette apply            # KDE scheme + kitty/Alacritty colours (backups first)
theme-profile palette apply --targets=kvantum   # recolour an installed Kvantum theme (KvArcDark/KvArc)
```

In the GUI: **Theme → Palette** (gallery, from wallpaper via Plasma's own
`Kirigami.ImageColors`, colour editing, apply targets). The live palette is
saved with profiles (component `palette`). Mapping, CLI and
limits: [`docs/PALETTE.md`](docs/PALETTE.md).

## Profiles / Профили

- **`glass`** — полупрозрачные окна с размытием, KWin glass enabled
- **`paper`** — светлая/solid тема, без прозрачностей

Profiles live in `~/.config/theme-profile/profiles/`.

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
previous file to `~/.config/theme-profile/backups/app-theme-<timestamp>/`.
Profiles keep the existing snapshot layout and additionally receive
`theme-bundle.json` with the v2 component/application references, so old
profiles remain usable as legacy snapshots.

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
`--install-extension`). CLI: `theme-profile apps store-search|store-install`. Profile
snapshots still include those config files when the apps are enabled in the
tracked component list. Unsupported capabilities are shown as fallbacks with a
button to open the app's native theme settings.

The repository also includes a local MCP server for AI agents:

```bash
uv run --project gui/qt-app python mcp/theme_profile_mcp.py
```

It can enumerate installed themes, create user-owned derived color/Kvantum
themes, assemble profiles, validate theme references and explicitly apply a
profile. See [`docs/MCP.md`](docs/MCP.md).

Tests use a temporary `$HOME`; `KTM_THEME_NO_APPLY=1` suppresses external
commands (D-Bus, `plasma-apply-*`, `kvantummanager`) for dry runs.

## Update / Обновление

```bash
~/theme-profile-kit/scripts/update-theme-profile-kit.sh
```

Fetches latest from git (`--ff-only`), then reinstalls the scripts **and** the GUI
(re-runs the installer, so the launcher, desktop entry and icon are refreshed
too). Warns if you have uncommitted local changes.

## Uninstall / Удаление

```bash
~/theme-profile-kit/scripts/uninstall-theme-profile-kit.sh            # interactive (confirms)
~/theme-profile-kit/scripts/uninstall-theme-profile-kit.sh --force    # skip confirmation
~/theme-profile-kit/scripts/uninstall-theme-profile-kit.sh --only-cli # remove CLI only, keep GUI/data
~/theme-profile-kit/scripts/uninstall-theme-profile-kit.sh --only-gui # remove GUI only, keep CLI/data
```

Removes the scripts from `~/.local/bin/`, the app from
`~/.local/share/kde-theme-manager/`, its launcher, desktop entry and icon, and
wipes the data directory (`$XDG_CONFIG_HOME/theme-profile/`, falling back to
`~/.config/theme-profile/`). The confirmation prompt is shown **before** any
file is removed, so answering "no" leaves the installation untouched. Does
**not** touch your KDE config files, wofi styles, or application settings —
those must be reverted manually.
See [Rollback & Recovery](#rollback--восстановление).

## Rollback / Восстановление

See [`docs/ROLLBACK.md`](docs/ROLLBACK.md) for full instructions. Quick summary:

- **Undo profile apply**: pre-apply backups are at `~/.config/theme-profile/backups/pre-apply-<name>-<ts>/`
- **Restore old scripts**: `cp -a ~/.config/theme-profile/backups/deploy-*/bin/. ~/.local/bin/`
- **Restore the previous app**: `cp -a ~/.config/theme-profile/backups/deploy-kde-theme-manager-*/app/. ~/.local/share/kde-theme-manager/`
- **Plasma won't start**: `systemctl --user start plasma-plasmashell.service`
- **Full remove**: `uninstall-theme-profile-kit.sh --force`

## Troubleshooting / Устранение неполадок

| Problem | Fix |
|---|---|
| `command not found: kreadconfig6` | Install `plasma-workspace`: `sudo pacman -S plasma-workspace` |
| `command not found: wofi` | Install: `sudo pacman -S wofi` |
| wofi menu appears empty | Ensure `~/.config/wofi/style.css` exists |
| `theme-profile list` shows nothing | `mkdir -p ~/.config/theme-profile/profiles && theme-profile save my-profile` |
| Wallpaper picker shows no images | Check `theme-profile wallpaper dir` (XDG Pictures/Wallpaper by default). Override with `export WALLPAPER_DIR=...` or the GUI setting |
| Plasma won't start after `apply` | `systemctl --user start plasma-plasmashell.service` |

## Editing Rule / Правило редактирования

1. Edit `~/theme-profile-kit/gui/qt-app/src/core/rust_core/**` first.
2. Re-run `scripts/install-theme-profile-kit.sh` (it builds the Rust CLI with cargo).
3. Do **not** edit `~/.local/bin/` directly — copy changes back into the kit immediately.

## Hardening / Безопасность

- **Profile names** are validated (`[A-Za-z0-9][A-Za-z0-9._-]{0,63}`) in Rust
  and Python before touching the filesystem; `..`, separators and shell
  metacharacters are rejected, and `diff` cannot traverse outside `profiles/`.
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
