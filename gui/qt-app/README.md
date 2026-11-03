# KDE Theme Manager — Qt/QML Application (v0.5.0)

Native KDE Plasma application for managing theme profiles and wallpapers.

## Features

- **Native KDE UI** — Built with Kirigami/QML, follows KDE HIG
- **Profile Management** — Apply/save/diff profiles (glass, paper, custom)
  with optional descriptions shown on the profile cards
- **Select, then Apply** — clicking a profile only *selects* it; an explicit
  **Apply** button commits it (switching profiles is slow, so nothing happens
  until you ask for it)
- **Preview & Diff** — see exactly which components a profile carries and how it
  differs from the active one *before* applying
- **Granular Control** — Choose which components to apply per profile
  (*Settings* page in the global drawer)
- **Theme bundles** — Theme page assembles KDE/Plasma assets, desktop/window
  settings, Kvantum, GTK and app themes; installed themes are picked from
  lists, edits are staged, and profiles store a v2 `theme-bundle.json` beside
  the legacy snapshot
- **Application theme adapters** — Zed, VS Code, Cursor, Obsidian and btop can
  expose current/installed themes (VS Code/Cursor map a theme `id` such as a
  Doki UUID back to its label); Discord/Telegram provide safe partial
  integrations (Telegram shows no picker — its active theme lives in `tdata`); Kitty and Alacritty are config-only integrations with an “Open
  config” action via `xdg-open`
- **MCP server** — `mcp/theme_profile_mcp.py` lets AI agents enumerate themes,
  create derived color/Kvantum themes, build/validate profiles and explicitly
  apply them (see `../docs/MCP.md`)
- **Applications page** — manage what is tracked: disable built-in apps with a
  checkbox (they are never deleted), add/edit/remove your own components
  (files or folders to sync, optional reload command run after apply)
- **Wallpaper Management** — Picker with previews, next/prev navigation
- **System Tray** — quick profile switching (active profile ticked), wallpaper
  next/prev/pick and desktop notifications, without opening the window
- **App Detection** — Automatically detects installed/configured apps
- **Non-blocking** — apply/save/delete *and* wallpaper operations (including the
  interactive picker) run off the GUI thread with a busy state
- **Single instance** — a second launch hands over to the running app (raises
  its window) and exits, so there is never more than one tray icon. If the
  local socket cannot be created at all (e.g. `$TMPDIR` so long that the path
  exceeds the 108-byte AF_UNIX limit), the app prints the reason and starts
  without single-instance protection instead of claiming it is "already running"
- **Dogfooding** — App themes itself from the *active* profile: `glass` →
  translucent dark, `paper` → light; any other profile/scheme (e.g.
  `everforest`) uses **System** mode, which reads the live `kdeglobals`
  `[Colors:*]` palette. `ThemeController.colors` publishes that palette to
  `ThemeColors.qml`, so cards, borders and dim text follow dark schemes too
- **Single core** — a Python adapter (`core/cli_core.py`) shells out to the Rust
  `theme-profile` / `wallpaper-*` binaries; every selector (`--core core|auto`,
  `$KTM_CORE`, Settings → Core) names that one core

## System tray

The tray icon offers, without opening the window:

- **Profiles ▸** — every profile, the active one ticked and sorted first; a
  click applies it immediately (quick switching, no confirmation dialog), with
  the result reported as a desktop notification. Entries are disabled while an
  operation runs, and clicking the active profile is a no-op.
- **Wallpaper ▸** — Next / Previous / Pick (the current file name is shown
  below them).
- **Show** — raise the window (double-clicking the icon toggles it).
- **Quit** — really quits. Needed because *Settings → Minimize to system tray*
  makes the window's close button hide the app instead; hiding is only done
  when the tray icon actually exists, so the window can never become
  unreachable.

Notifications go through `QSystemTrayIcon.showMessage`, not a hand-built
`org.freedesktop.Notifications.Notify` call: PySide6 marshals Python ints as
D-Bus `i` while `Notify`'s `replaces_id` is `u`, so a hand-built call is
rejected with *"No such method 'Notify'"* and dropped silently.

The icon is installed as `ktm-theme-manager`: KDE's icon loader collapses
`<existing-icon>-<suffix>` to the existing icon, so `kde-theme-manager`
resolved to the KDE logo (and `plasma-*` to the Plasma logo) instead of ours.

## Theming (dogfooding)

The application styles itself from the active profile:

| Active profile | App look |
|----------------|----------|
| `glass` | Translucent dark "glass panel", dark controls, light text, KWin blur requested |
| `paper` | Light panel, light controls, dark text |
| `dark` / `light` | Plain dark / light palette |
| `system` | Follows the system colour scheme |

Implementation notes:

- The whole **application palette** is switched (`ThemeController._apply_palette`)
  so Qt Quick Controls (buttons, chips, check boxes…) match the custom QML
  colours. Without this the window looked "half dark".
- `Kirigami.Page` paints `Platform.Theme.backgroundColor` behind its content;
  `Main.qml` overrides the page `background` so glass mode shows the translucent
  glass panel instead of an opaque light rectangle.
- **All colours live in one place**: `qml/Components/ThemeColors.qml`, a QML
  singleton of `glass`-parameterised accessors (`ThemeColors.cardBg(glassMode)`,
  `ThemeColors.fgColor(glassMode)`, …). It is the only file allowed to read
  `Kirigami.Theme` directly — those colour roles can be *invalid* with PySide6's
  bundled Qt, which is why it wraps every lookup in `safeColor(role, fallback)`.
  Previously every component copy-pasted that helper and the palettes drifted
  apart.
- **All font sizes come from** `qml/Components/TypeScale.qml` (also a singleton:
  `body`, `subtitle`, `title`, `caption`, derived from the platform font).
  Never bind `font.pointSize` to `Platform.Units.*Spacing` — those are spacing
  tokens (4 pt / 6 pt here) and rendered body text near-illegible.
- Window transparency is owned by QML (`color:` binding) — the Python side must
  **not** call `QWindow.setColor()`, as that overrides the QML binding.
- The global drawer is `modal: true` (the default). A non-modal `GlobalDrawer` is
  a *permanent* sidebar on desktop, which eats ~220 px and overlaps the header.

## Select, then Apply

Switching a profile runs `theme-profile apply`, which touches dozens of config
files and takes seconds. Two consequences shape the UI:

- Clicking a profile card (or pressing Space while it has focus) only sets
  `selectedProfile`. Nothing is written until the **Apply** button is pressed,
  and the button is disabled while the selection already matches the active
  profile. Secondary per-profile actions (Duplicate, Diff, Edit description,
  Delete) live in the card's **⋯** menu.
- Apply/save/delete run on a `QThreadPool` worker (`ProfileController.busy`,
  `operationStarted` / `operationFinished`), so the window keeps repainting and
  the busy indicator animates while the work happens. The result is shown in an
  inline message that auto-clears after a few seconds.
- The check-box `checked` state is model-driven. A Qt Quick control writes
  to `checked` on click, which destroys a plain `checked:` binding, so the
  component re-arms it with
  `Binding { restoreMode: Binding.RestoreBindingOrValue }`.

## Screenshots

| Main Page | Settings Page |
|-----------|---------------|
| Profiles + Wallpaper | Granular component toggles |

## Requirements

- **KDE Plasma 6** (Wayland or X11)
- **Python 3.11+**
- **PySide6 6.7+**
- **Rust CLI tools** — `theme-profile`, `theme-menu`, `wallpaper-picker/next/prev`
  (built from `src/core/rust_core`; the bash scripts were removed in 0.5.0)

## Installation

Normal use — one command installs the app, its launcher and the desktop entry
(with a backup of whatever it replaces):

```bash
# From the theme-profile-kit root: full install (CLI + GUI)
scripts/install-theme-profile-kit.sh

# GUI only (CLI must already be installed)
scripts/install-theme-profile-kit.sh --only-gui

# Options
scripts/install-theme-profile-kit.sh --help       # paths and details
scripts/install-theme-profile-kit.sh --no-venv    # never build a venv
scripts/install-theme-profile-kit.sh --with-venv  # always build one

# Remove only the GUI again (profiles and data are kept)
scripts/uninstall-theme-profile-kit.sh --only-gui
```

| Installed | Where |
|---|---|
| application copy | `~/.local/share/kde-theme-manager/` |
| launcher (`kde-theme-manager`) | `~/.local/bin/` |
| desktop entry | `~/.local/share/applications/kde-theme-manager.desktop` |
| icon | `~/.local/share/icons/hicolor/scalable/apps/ktm-theme-manager.svg` |
| backups of replaced files | `~/.config/theme-profile/backups/deploy-kde-theme-manager-*/` |

`~/.local/share` and `~/.local/bin` follow `XDG_DATA_HOME` / `XDG_BIN_HOME`.
A venv is created inside the app copy **only** when the system Python cannot
`import PySide6`; with the distro `pyside6` the copy is ~650 KB and runs on it
directly. The launcher respects `KDE_THEME_MANAGER_DIR` if you keep the app
somewhere else.

Where the data lives (never touched by install/update):

```
$XDG_CONFIG_HOME/theme-profile/   (default ~/.config/theme-profile/)
├── profiles/<name>/     the profiles themselves (theme snapshots)
├── backups/             pre-apply + deploy backups
├── manifests/           what a profile covers
├── active-profile       currently applied profile
└── app-preferences.json GUI settings (created when you first save settings)
```

The data root is XDG-aware: an existing `$XDG_CONFIG_HOME/theme-profile` wins,
then an existing legacy `~/.config/theme-profile`, then the XDG path. Profile
snapshots themselves always keep the historical `.config/...` layout, so
profiles created before XDG support keep working unchanged.

The theme itself is not stored by the app — applying a profile writes the usual
KDE configuration (`~/.config/kdeglobals`, `kwinrc`, `gtk-3.0/settings.ini`, …).
Wallpapers live in `~/Изображения/Wallpaper` (or `wallpaper_directory` from the
settings) with the index cache in `~/.cache/wallpaper_index`.

### Running from the source tree (development)

```bash
cd gui/qt-app
uv pip install -e ".[dev]"  # or pip install -e ".[dev]"
./theme-profile-manager.sh  # sets QML import paths, prefers ./.venv, then python -m src
```

## Development

```bash
# Run from source
cd gui/qt-app
python -m src

# Run with uv
uv run python -m src

# Lint (ruff/mypy come from the [dev] extra)
ruff check src/
mypy src/

# QML syntax/type lint (PySide6 6.12 ships its own qmllint)
.venv/bin/pyside6-qmllint -I qml -I /usr/lib/qt6/qml qml/Main.qml qml/pages/SettingsPage.qml qml/Components/*.qml

# Tests (Python: 236, from gui/qt-app)
.venv/bin/python -m pytest tests/ -q

# CLI + installer tests (118, from the repo root)
bash tests/test-scripts.sh

# Static shell analysis (repo root)
find . -name "*.sh" -not -path "*/.venv/*" -print0 | xargs -0 shellcheck -x

# Visual smoke test: render the window to a PNG and exit (dev helper)
KTM_SCREENSHOT=/tmp/win.png python -m src
```

Known, harmless `qmllint` output:

- `Unqualified access [unqualified]` for `profileController`, `themeController`,
  `settingsController`, `trackedAppsController` — they are context properties, so
  qmllint cannot resolve them. Expected.
- `Type ThemeColors/TypeScale not declared as singleton in qmldir but using pragma
  Singleton [import]` — only emitted when the singleton file itself is passed on
  the command line; the `qmldir` does declare both and the runtime resolves them.

## Architecture

```
src/
├── core/
│   ├── protocol.py            # Abstract Base Classes (CONTRACT)
│   ├── cli_core.py            # CLI adapter (shells to theme-profile / wallpaper-*)
│   ├── tracked_apps.py        # App registry with detection
│   ├── theme_core.py          # Curated field readers/writers + staging/undo
│   ├── app_theme_adapters.py  # Per-app theme detection/list/apply
│   └── theme_bundle.py        # High-level component registry + v2 profile bundles
├── controllers/               # PySide6 QObject controllers
│   ├── ProfileController
│   ├── WallpaperController
│   ├── TrackedAppsController
│   ├── SettingsController
│   ├── ThemeInspectorController  # Legacy field inspector (Advanced/Customize)
│   ├── ThemeBundleController     # New theme bundle UI model
│   └── MainController            # Aggregate
├── theme_controller.py        # Dogfooding: app theming
└── __main__.py                # Entry point

qml/
├── Main.qml              # Main page (active profile, profiles, wallpaper) + dialogs
├── pages/
│   ├── ThemePage.qml             # Theme bundle overview and scope switch
│   ├── ThemeComponentsPage.qml   # KDE/Plasma, Desktop, Qt/Kvantum, GTK choices
│   ├── ThemeApplicationsPage.qml # Per-app adapter cards and native fallbacks
│   ├── SectionPage.qml           # Legacy field editor (Advanced/Customize)
│   ├── ApplicationsPage.qml      # Tracked apps: disable built-ins, CRUD for custom
│   └── SettingsPage.qml          # Settings with component toggles (drawer → Settings)
└── Components/
    ├── ThemeColors.qml              # Singleton: the only colour source
    ├── TypeScale.qml                # Singleton: typography scale
    ├── ProfileCardDelegate.qml      # Profile card: select + 3 actions + ⋯ menu
    ├── ComponentCategoryGroup.qml   # Collapsible component category
    └── ComponentCheckbox.qml        # Single component toggle
```

> Note: `ScrollablePage` must contain a **single layout** child — its children are
> only anchored left/right, never laid out, so bare cards collapse to zero height.
> `Kirigami.Card` needs an explicit `contentItem:` for the same reason.

> Note: a `@Slot()` in `src/controllers/__init__.py` must declare **every**
> argument type. `@Slot(str, result=str) def diff_profiles(a, b)` makes Qt drop
> the second argument silently, and the call raises `missing 1 required positional
> argument` at runtime. `tests/test_gui.py::test_slots_declare_every_python_argument`
> guards this for every controller.

## Wallpapers

**What is currently shown** is read from Plasma itself
(`~/.config/plasma-org.kde.plasma.desktop-appletsrc`, group
`[Containments][N][Wallpaper][org.kde.image][General]`, key `Image`), with the
cached index `~/.cache/wallpaper_index` only as a fallback. The cache is a
*position* in the file list, so it silently points at another wallpaper as soon
as files are added or removed — or when the list order changes.

**The list order is a contract** shared by this app and
the Rust CLI (`src/bin/wallpaper-{picker,next,prev}`):

| rule | Rust CLI | GUI core (Python) |
|---|---|---|
| follow symlinks | `fs::is_file()` (follows links) | `Path.is_file()` |
| extensions `jpg jpeg png webp bmp`, case-insensitive | `to_lowercase()` | `.suffix.lower()` |
| byte/code-point order | `Vec<String>::sort()` | `sorted(key=str)` |

A locale-aware `sort` (`ru_RU.UTF-8` folds case and ignores punctuation) ordered
the files differently from `sorted()`, so the GUI resolved the index to a
different image than the one the scripts had applied. `next`/`prev` now also
step from the wallpaper Plasma is showing instead of the cached index, which
makes them self-healing. Guarded by
`tests/test_gui.py::TestWallpaperOrdering` (runs the real script and compares the
walked order) and `tests/test-scripts.sh` sections 8–9.

The app passes `WALLPAPER_DIR` to the scripts so that a custom
`wallpaper_directory` in the settings file cannot diverge from the scripts'
default.

`wallpaper-picker` passes `--define dmenu-print_line_num=true` to wofi and uses
the printed value as a **direct 0-based index** into the displayed list: wofi
numbers the lines from 0 in input order (`modes/dmenu.c`:
`uint16_t line_num = 0; … snprintf("%u", line_num++)`). The previous `-1`
correction applied the image above the clicked one and made the first image a
no-op.

## Core Protocol

The `core/protocol.py` defines **Abstract Base Classes** for:
- `ProfileCore` — profile operations
- `WallpaperCore` — wallpaper operations
- `TrackedAppsCore` — app detection
- `SettingsCore` — app preferences

A single implementation exists — the core (`core/cli_core.py`): profiles and
wallpapers go through the Rust CLI binaries (`theme-profile`, `wallpaper-*`),
tracked apps through the Python registry (`core/tracked_apps.py`), settings
through JSON in `~/.config/theme-profile/app-preferences.json`.

(The former second implementation — a `libkde_theme_core.so` shared library
behind a C ABI — was removed; one core keeps behaviour and tests uniform.)

### Choosing a core

There is nothing to switch — every selector names the single core:

```bash
kde-theme-manager --core core      # core | auto (same core)
kde-theme-manager --list-cores     # what the core delegates to
KTM_CORE=core kde-theme-manager    # same as --core, for one run
kde-theme-manager                 # Settings → Core (saved, applied on restart)
```

Priority: `--core` → `$KTM_CORE` → saved setting → `core`.

### Building the Rust CLI

```bash
cd src/core/rust_core
cargo build --release   # → target/release/theme-profile, wallpaper-*, theme-menu
```

The kit installer copies those binaries into `~/.local/bin/`.

## Tracked Applications (20)

| Category | Apps |
|----------|------|
| KDE/Plasma | KWin, Plasma Shell, KDE Globals |
| GTK/Qt | GTK3, GTK4, Qt5/Qt6 |
| Window Deco | Klassy, Kvantum |
| Terminals | Kitty (live reload), Alacritty |
| Launchers | Wofi |
| Monitors | btop |
| Sysinfo | Fastfetch |
| Editors | Zed, VS Code, Cursor |
| Browser | Zen Browser |
| Notes | Obsidian |
| System | Color Schemes, Look & Feel |

## Configuration

Settings stored in `~/.config/theme-profile/app-preferences.json` (or
`$XDG_CONFIG_HOME/theme-profile/`, see below):

```json
{
  "default_components_mode": "all",
  "custom_components": [],
  "wallpaper_linked_to_profile": false,
  "wallpaper_directory": "~/Изображения/Wallpaper",
  "theme": "system",
  "show_not_installed_warning": true,
  "auto_apply_on_startup": false,
  "minimize_to_tray": false,
  "global_hotkey": "",
  "confirm_before_apply": true,
  "create_backup_before_apply": true,
  "notify_on_success": true,
  "core_backend": "core"
}
```

Keys no longer backed by a Settings control are kept for forward/backward
compatibility and are ignored by the GUI:

- `global_hotkey` — the configurable hotkey was removed; a bare KGlobalAccel
  registration never produced a working shortcut (see below).
- `wallpaper_linked_to_profile`, `show_not_installed_warning`,
  `create_backup_before_apply` — the corresponding dead toggles were dropped
  from the Settings page; backups are always taken before an apply.


## Building

### Desktop Entry

Installed by `theme-profile-kit/scripts/install-theme-profile-kit.sh` (GUI
installation is included by default; use `--only-gui` for the app alone):

- `~/.local/bin/kde-theme-manager` — launcher (execs the app's own
  `theme-profile-manager.sh`, which sets the QML import paths)
- `~/.local/share/kde-theme-manager/` — the app copy
- `~/.local/share/applications/kde-theme-manager.desktop`
- `~/.local/share/icons/hicolor/scalable/apps/ktm-theme-manager.svg`

Both source files live in the repository, so edit them there:
`resources/kde-theme-manager.desktop` and
`resources/icons/ktm-theme-manager.svg` (icon = dark/light disc with the Breeze
accent ring). After editing, re-run the installer to redeploy.

### Standalone Binary (future)

```bash
# With Nuitka (Python)
uv pip install nuitka
python -m nuitka --standalone --enable-plugin=PySide6 src/__main__.py
```

(GUI stays Python per the architecture decision — a Rust GUI rewrite is not
planned; the single core remains the Rust CLI binaries.)

## Roadmap

- [x] Profile preview before apply (Preview / Diff dialogs)
- [x] System tray: quick profile switching (active profile ticked, wallpaper
      next/prev/pick, notifications, Quit)
- [x] Single CLI-backed core behind `--core` / `$KTM_CORE` / Settings (the
      former `libkde_theme_core.so` second core was removed)
- [ ] Auto-sync with system theme changes
- [ ] Plasma applet version

> A configurable global hotkey (Meta+T) was dropped: KGlobalAccel only creates a
> component object for apps whose desktop entry carries
> `X-KDE-GlobalAccel-CommandShortcut=true`, so a bare D-Bus `doRegister` never
> yielded a usable shortcut.

## License

MIT — see LICENSE in parent directory.