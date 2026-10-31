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
install-theme-profile-kit.sh --help      # show help
install-theme-profile-kit.sh --no-gui    # scripts only, no app

# GUI alone (paths, --with-venv / --no-venv):
scripts/install-kde-theme-manager.sh --help
```

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

Environment overrides (also used by the GUI and the tests): `WALLPAPER_DIR`,
`WALLPAPER_STATE`, `WALLPAPER_THUMB_DIR`, `PLASMA_APPLETSRC`,
`WALLPAPER_DRY_RUN=1` (print the chosen file instead of applying it).

`wallpaper-picker` feeds wofi with `--define dmenu-print_line_num=true`, which
prints the **0-based** index of the selected line (`wofi`'s `modes/dmenu.c` does
`uint16_t line_num = 0; … snprintf("%u", line_num++)` over the entries in input
order). Subtracting 1 there applied the image *above* the clicked one, so the
number is now used as a direct index into the displayed list — section 10 of the
test suite clicks lines 0, 3 and the last one against a stubbed `wofi`.

## Profiles / Профили

- **`glass`** — полупрозрачные окна с размытием, KWin glass enabled
- **`paper`** — светлая/solid тема, без прозрачностей

Profiles live in `~/.config/theme-profile/profiles/`.

## Theme Inspector / Инспектор темы

The GUI's **Theme** page (drawer → *Theme*, or the magnifier-style **Inspect**
button on a profile card) answers three questions without opening a config file:

- **What is my theme made of?** — 69 fields in five sections: Colors,
  Typography, Windows & Effects, Desktop & Assets, Applications.
- **How does the active profile differ from live?** — switch the scope combo to
  any profile; changed fields get an accent dot and the profile value in the
  tooltip.
- **Is anything broken?** — *Validation* flags missing colour schemes,
  look-and-feel/icon/Kvantum themes, fonts, a broken `LookAndFeelPackage`, and
  the `blurEnabled` + `better_blur_dxEnabled` conflict. **Fix** picks the value
  stored in the active profile (or an installed fallback).

Live values are editable (color picker, sliders, combo boxes, font family/size,
switches). Every write runs off the GUI thread and applies immediately — no
draft/Apply step. **Undo** reverts the last field, **Revert all** restores every
file touched in this session. Before the first write, originals are copied to
`~/.config/theme-profile/backups/inspector-<timestamp>/`.

Heavy components (Plasma theme, look-and-feel, icons) ask for confirmation
because they can replace the wallpaper or panel layout. Profile scopes are
read-only; deep settings stay in System Settings via `kcmshell6` links. The
inspector is a separate Python module (not part of the core adapter).

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
~/theme-profile-kit/scripts/uninstall-theme-profile-kit.sh      # interactive (confirms)
~/theme-profile-kit/scripts/uninstall-theme-profile-kit.sh --force  # skip confirmation
~/theme-profile-kit/scripts/uninstall-theme-profile-kit.sh --no-gui # keep the app

# GUI alone (profiles and data are kept):
scripts/uninstall-kde-theme-manager.sh
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
| Wallpaper picker shows no images | Default dir is `~/Изображения/Wallpaper`. Override with `export WALLPAPER_DIR="$HOME/Pictures/Wallpaper"` |
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
