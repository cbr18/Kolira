# kolira — Qt/QML Application

Application coverage: [app contracts](../../docs/APPLICATIONS.md) — colours, selection, safe return, Obsidian vault setup and runtime limits.

Native KDE Plasma application for managing theme profiles and wallpapers.

Fastfetch and the Plasma application launcher (including its button icon/image)
remain the user's own settings. Themes, including legacy imports, do not change
them or require Simple Application Launcher.

## Features

- **Native KDE UI** — Built with Kirigami/QML, follows KDE HIG
- **Profile Management** — Apply/save/diff profiles (glass, paper, custom)
  with optional descriptions shown on the profile cards. Profile refresh runs in
  the background and coalesces repeated requests; failed reads show an error and
  preserve the last successful list and active profile
- **My Themes** — search saved themes and open their details. **Apply** opens a
  review before a theme changes the desktop.
- **Preview & Diff** — see exactly which components a profile carries and how it
  differs from the active one *before* applying
- **Granular Control** — Choose which components to apply per profile
  (*Settings* page; legacy snapshot profiles only — theme packages always
  apply exactly what they define)
- **Theme bundles** — Theme page assembles KDE/Plasma assets, desktop/window
  settings, Kvantum, GTK and app themes; installed themes are picked from
  lists, edits are staged; saving writes a theme (full snapshot of the
  appearance files + `theme.json`, `docs/THEMES.md`) through the Rust CLI
- **Themes** — save dialog with *Include the current wallpaper*, apply dialog
  with *Keep the current wallpaper*, and a *Missing Theme Parts* dialog (links +
  *Install from stores*) fed by `kolira apply --json`
- **Dependencies** — after apply/import a *This Theme Needs* dialog lists what
  is missing with its source (KDE Store / package command / editor store) and
  installs the ticked items (`kolira deps --install`), then re-applies
- **Export / import** — card menu → *Export…* (`.ktmtheme.zip`, optional
  wallpaper), *Import theme…* (name conflict → new name or replace); the
  missing-parts dialog follows an import too (`kolira import --json`)
- **Palette** in the theme editor stages the KDE color scheme, custom accent
  and wallpaper accent for Live and saved themes. A saved theme also supports
  Base16 presets, individual color edits and a draft preview.
- **Base16 palette** in the Live editor supports gallery import and wallpaper
  (`Kirigami.ImageColors`) or k-means, colour editing, apply to KDE /
  terminals / Kvantum (`controllers/palette.py` → Rust CLI)
- **Applications** stages Kitty and Alacritty opacity and blur. A saved
  theme can also set VS Code, Cursor and btop themes, btop rounded corners,
  Zed mode and light/dark themes, Zen accent and light/dark mode, Obsidian
  color mode and theme, and the apps that use Base16 on Apply.
- **Application detection** — by executable/Flatpak, not by a leftover config;
  **Find in store…** for Zed / VS Code / Cursor (search + install via the Rust
  CLI, off the GUI thread: `ThemeBundleController.storeSearch/storeInstall`)
- **Application theme adapters** — Zed, VS Code, Cursor, Obsidian and btop can
  expose current/installed themes and stage selections in the Live draft
  (VS Code/Cursor map a theme `id` such as a
  Doki UUID back to its label); Discord provides a safe partial
  integration; Kitty and Alacritty are config-only integrations with an “Open
  config” action via `xdg-open`
- **MCP server** — `mcp/kolira_profile_mcp.py` lets AI agents enumerate themes,
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
- **System appearance** — the window follows the Qt platform palette and the
  live KDE colours. It picks up desktop style changes while it is open.
- **Single core** — a Python adapter (`core/cli_core.py`) shells out to the Rust
  `kolira` / `wallpaper-*` binaries; every selector (`--core core|auto`,
  `$KTM_CORE`, Settings → Core) names that one core

## System tray

The tray icon offers:

- **Profiles ▸** — every profile, the active one ticked and sorted first; a
  click opens the window with a pre-apply review; confirmation applies it, with
  the result reported as a desktop notification. Entries are disabled while an
  operation runs. A profile can be reviewed and applied again.
- **Wallpaper ▸** — Next / Previous / Pick (the current file name is shown
  below them).
- **Show** — raise the window (a click on the icon does the same).
- **Quit** — exits after checking for a pending draft. Needed because *Settings → Minimize to system tray*
  makes the window's close button hide the app instead; hiding is only done
  when the tray icon actually exists, so the window can never become
  unreachable.

Notifications go through `QSystemTrayIcon.showMessage`, not a hand-built
`org.freedesktop.Notifications.Notify` call: PySide6 marshals Python ints as
D-Bus `i` while `Notify`'s `replaces_id` is `u`, so a hand-built call is
rejected with *"No such method 'Notify'"* and dropped silently.

The icon is installed as `kolira-icon`: KDE's icon loader collapses
`<existing-icon>-<suffix>` to the existing icon, so `kolira`
resolved to the KDE logo (and `plasma-*` to the Plasma logo) instead of ours.

## System appearance

The Qt platform supplies the application font and fallback palette.
`ThemeController` reads the live KDE palette when available and updates the
window's colour roles. It watches `kdeglobals` and the active Kvantum theme so
changes from KDE System Settings, the CLI, or the GUI reach the open window.
`QT_STYLE_OVERRIDE` keeps the widget style fixed.

## Browse, review, and apply

- Choose a card to see saved components and an approximate appearance preview.
  Mouse and keyboard activation open the same details page. The header reads
  Live through Rust and shows the last successful profile apply if it is in
  history.
- **Apply** opens the same review from a card, details, history, or the tray.
  Expand the technical diff to see individual files. If a dependency needs
  attention, open the filtered dependency page and review again after checking
  it. The confirmation always applies the reviewed token through Rust.
- **Save as theme** captures Live under a new name. With pending changes you
  can choose the current appearance or include its draft. An optional
  Look & Feel resource is written into the package. The current desktop,
  active marker and draft stay intact.
- **Wallpapers** has its own page for the current image, picker, and navigation.
  Preview decoding and wallpaper operations run off the UI thread.

## Screenshots

| Main Page | Settings Page |
|-----------|---------------|
| Profiles + Wallpaper | Granular component toggles |

## Requirements

- **KDE Plasma 6** (Wayland or X11)
- **Python 3.11+**
- **PySide6 6.7+**
- **Rust CLI tools** — `kolira`, `theme-menu`, `wallpaper-picker/next/prev`
  (built from `src/core/rust_core`; the bash scripts were removed in 0.5.0)

## Installation

Normal use — one command installs the app, its launcher and the desktop entry
(with a backup of whatever it replaces):

```bash
# From the kolira root: full install (CLI + GUI)
scripts/install-kolira.sh

# GUI only (CLI must already be installed)
scripts/install-kolira.sh --only-gui

# Options
scripts/install-kolira.sh --help       # paths and details
scripts/install-kolira.sh --no-venv    # never build a venv
scripts/install-kolira.sh --with-venv  # always build one

# Remove only the GUI again (profiles and data are kept)
scripts/uninstall-kolira.sh --only-gui
```

| Installed | Where |
|---|---|
| application copy | `~/.local/share/kolira/` |
| launcher (`kolira-gui`) | `~/.local/bin/` |
| desktop entry | `~/.local/share/applications/kolira.desktop` |
| icon | `~/.local/share/icons/hicolor/scalable/apps/kolira-icon.svg` |
| backups of replaced files | `~/.config/kolira/backups/deploy-kolira-*/` |

`~/.local/share` and `~/.local/bin` follow `XDG_DATA_HOME` / `XDG_BIN_HOME`.
A venv is created inside the app copy **only** when the system Python cannot
`import PySide6`; with the distro `pyside6` the copy is ~650 KB and runs on it
directly. The launcher respects `KDE_THEME_MANAGER_DIR` if you keep the app
somewhere else.

Where the data lives (never touched by install/update):

```
$XDG_CONFIG_HOME/kolira/   (default ~/.config/kolira/)
├── profiles/<name>/     the profiles themselves (theme snapshots)
├── backups/             pre-apply + deploy backups
├── manifests/           what a profile covers
├── active-profile       currently applied profile
└── app-preferences.json GUI settings (created when you first save settings)
```

The data root is XDG-aware: an existing `$XDG_CONFIG_HOME/kolira` wins,
then an existing legacy `~/.config/kolira`, then the XDG path. Profile
snapshots themselves always keep the historical `.config/...` layout, so
profiles created before XDG support keep working unchanged.

The theme itself is not stored by the app — applying a profile writes the usual
KDE configuration (`~/.config/kdeglobals`, `kwinrc`, `gtk-3.0/settings.ini`, …).
Wallpapers live in the directory reported by `kolira wallpaper dir`
(`$WALLPAPER_DIR` → `wallpaper_directory` setting → XDG Pictures/Wallpaper →
`~/Pictures/Wallpaper`) with the index cache in `~/.cache/wallpaper_index`.
Profiles carry a copy of their wallpaper in `<profile>/wallpaper/`.

### Running from the source tree (development)

```bash
cd gui/qt-app
uv pip install -e ".[dev]"  # or pip install -e ".[dev]"
./kolira-manager.sh  # sets QML import paths, prefers ./.venv, then python -m src
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

# Tests (Python, from gui/qt-app; pytest prints the current count)
.venv/bin/python -m pytest tests/ -q

# CLI + installer tests (from the repo root; count is printed by the harness)
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
│   ├── cli_core.py            # CLI adapter (shells to kolira / wallpaper-*)
│   ├── tracked_apps.py        # App registry with detection
│   ├── theme_core.py          # Qt view adapter for Rust Inspector sessions
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

### Quick Switch (separate process)

`kolira-quick-switch` → `kolira-manager.sh --quick-switch` → `python -m src
--quick-switch` → `src/quick_switch_app.py`. It builds its own
`QQmlApplicationEngine`, loads `qml/quick-switch/HostSurface.qml` once per screen
and never touches the main window, the tray, the Inspector or `MainController`.

The hotkey launches the desktop entry again, so the process is a **toggle**:
a second launch connects to `QLocalServer("kolira-quick-switch")`, the running
overlay closes and the new process exits. During `preparing`/`applying` the
request is ignored — a hotkey must not hide a running transaction.

The overlay also refuses to open while another process is applying a theme.
Before it builds any window it calls the cheap `operation.busy` probe; if the
cross-process appearance lock is held it shows a passive notification and exits
without opening (preview mode, which never applies, is still allowed). This
avoids the confusing "could not apply" failure that used to happen when the hotkey
was pressed mid-transaction. The probe fails open — an unreachable core never
blocks the overlay.

```
src/
├── quick_switch_app.py         # QGuiApplication, one surface per monitor, watchdog, toggle
├── quick_switch_controller.py  # phases, pre-flight, apply, real stages
└── qml_runtime.py              # system QML import paths (shared with __main__.py)

qml/quick-switch/
├── HostSurface.qml        # layer-shell window (org.kde.layershell as LayerShell)
├── QuickSwitchContent.qml # dim + carousel + transitions + panels
├── ThemeCard.qml          # one card for every shape, masked content
├── CardPreview.qml        # face: wallpaper, mock window, name and palette
├── ApplyScreen.qml        # shader screen + real stage scale
├── CardGeometry.js        # card outline as an SVG path (mask, border, hit area)
└── shaders/apply.frag     # GLSL source; apply.qsb is the compiled artifact
```

- **Cards.** `ThemeCard` is only geometry (mask, border, glow, hit area);
  `CardPreview` is the face: the wallpaper, a mock window about a third of
  the card high centred a third from the top, and the name with the palette.
  Colours, fonts, radius and opacity come from `quick_switch.cards`
  (`previewColors`, `previewStyle`), the same values the gallery card uses.
- **Masking.** `OpacityMask` (Qt5Compat) clips the card content. A visible
  `MultiEffect` source paints itself unmasked and the preview spills outside
  the outline; `content` must be `layer.enabled` + invisible to be a texture
  provider, and the mask item is never drawn.
- **Antialiasing.** The overlay surface is created with 4× MSAA
  (`QSurfaceFormat.setSamples(4)` in `quick_switch_app.py`) and the card's
  `content`/glow layers use `layer.samples: 4`, so the non-rectangular outlines
  (rounded corners, parallelogram diagonals) are smooth instead of stair-stepped.
- **Thumbnails.** The first open renders small JPEGs into
  `$XDG_CACHE_HOME/kolira/quick-switch/thumbs` (Rust calls `magick`, the same
  external tool the wallpaper picker uses); later opens read those. The card
  also decodes with `sourceSize` and fades the image in, so a wallpaper never
  pops into an empty rectangle.
- **Parallelogram.** The shape is a sheared rectangle: its width is constant
  but it slides sideways, so `CardPreview` keeps the mock window inside
  `[skew·w, (1-skew)·w]` and the card leaves air inside its box.

- **Layer-shell.** The attached type is `LayerShell.Window`, so the import must be
  aliased (`import org.kde.layershell 1.0 as LayerShell`). Without the alias the
  engine reports “Non-existent attached object”. The shell is what puts the
  overlay above panels and normal windows; `keyboardInteractivity` is
  `Exclusive` only on the screen with the carousel.
- **Shaders.** Qt 6 needs precompiled `.qsb`: `qml/quick-switch/shaders/apply.frag`
  is the source, `apply.qsb` is generated with
  `/usr/lib/qt6/bin/qsb --qt6 --glsl 100es,120,150,330,440,450,460,300es -s -o apply.qsb apply.frag`.
  The shader reads `gl_FragCoord`, not `qt_Texcoord0`: the vertex stage writes
  `qt_TexCoord0` and the mismatch breaks linking. Uniforms live in one std140
  block in the order the QML properties are declared.
- **Stages.** `profile.apply` has no progress channel. The overlay polls
  `operation.list` while the transaction runs and maps the real record state
  (`preparing`, `applying`, `succeeded`, …) to the scale; the moving bar inside a
  stage is an animation, not a measurement.
- **No token in Quick Switch.** The overlay applies without a `preview_token`,
  exactly like the CLI (`kolira apply NAME --without-dependencies`). The
  pre-flight (`quick_switch.prepare`) is kept only for the attention check
  (missing resources / dependencies); it never gates the apply. This removes
  the whole class of `PREVIEW_STALE` failures caused by a token that went stale
  while the user was browsing — the overlay applies the state at write time.
- **Hotkey.** `~/.local/share/applications/kolira-quick-switch.desktop` carries
  `X-KDE-GlobalAccel-CommandShortcut=true`; the key itself lives in
  `[services][kolira-quick-switch.desktop]` in `~/.config/kglobalshortcutsrc` and
  is written by `quick_switch.set_hotkey` through `kwriteconfig6` with a backup.
  `Meta+K` is taken from `voxtype-toggle` once (moved to `Meta+Shift+K`).

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

| rule | Shared Rust implementation | GUI adapter |
|---|---|---|
| follow symlinks | `fs::is_file()` (follows links) | delegates to Rust |
| extensions `jpg jpeg png webp bmp`, case-insensitive | `to_lowercase()` | delegates to Rust |
| byte/code-point order | `Vec<String>::sort()` | delegates to Rust |

A locale-aware `sort` (`ru_RU.UTF-8` folds case and ignores punctuation) ordered
the files differently from `sorted()`, so the GUI resolved the index to a
different image than the one the scripts had applied. `next`/`prev` now also
step from the wallpaper Plasma is showing instead of the cached index, which
makes them self-healing. Guarded by
`tests/test_gui.py::TestWallpaperOrdering` (runs the real script and compares the
walked order) and `tests/test-scripts.sh` sections 8–9.

Directory and "current wallpaper" come from the Rust CLI (`kolira
wallpaper dir|current`) — the GUI no longer parses the appletsrc itself. The app
still passes `WALLPAPER_DIR` to the scripts so an unsaved explicit setting
cannot diverge from what they enumerate. Tests point the GUI at the freshly
built CLI via `KTM_THEME_PROFILE_BIN` (`tests/conftest.py`).

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

A single Rust implementation owns profiles, wallpapers, Inspector parsing,
staging/Undo/Revert, app theme adapters, resource cloning, preferences and the
registry. Python view adapters call it through `core/cli_core.py`; preference
data lives in `$XDG_CONFIG_HOME/kolira/app-preferences.json`.
The local JSON interface is `kolira core METHOD` ([API v1](../../docs/CORE_API.md)).
Update CLI and GUI together. There is no FFI or Python mutation fallback.

Audit fixes preserve normal save/apply/Undo flows: failed Inspector changes
remain staged for retry, open component pages follow controller updates,
dependency results scroll within a bounded panel, and obsolete palette searches
cannot replace newer results. Completed async helpers release their callbacks
after the runnable releases its Qt wrapper, including when their page closes.
The wallpaper folder URL is decoded in Python; unavailable previews display an
error instead of a permanent loading indicator.

The **Dependencies** page (`qml/pages/DependenciesPage.qml`) uses
`controllers/dependencies.py` as a Qt adapter. QProcess reads Rust registry queries
and JSONL operation events asynchronously through `core/cli_core.py`; provider detection, ownership, mutation
and result checks stay in Rust (`dependency_registry.rs`, `dependency_jobs.rs`).
The table has checkboxes, separate name/category columns and bulk delete with an
affected-theme confirmation. It shares the operation executor with the missing
dependencies dialog. Failed/cancelled downloads do not implicitly apply the theme;
the explicit skip path uses `--without-dependencies`. Remembered decisions update
the existing store/package automation keys before applying. See
[the CLI and ownership contract](../../docs/DEPENDENCIES.md).

### Choosing a core

There is nothing to switch — every selector names the single core:

```bash
kolira-gui --core core      # core | auto (same core)
kolira-gui --list-cores     # what the core delegates to
KTM_CORE=core kolira-gui    # same as --core, for one run
kolira                 # Settings → Core (saved, applied on restart)
```

Priority: `--core` → `$KTM_CORE` → saved setting → `core`.

### Building the Rust CLI

```bash
cd src/core/rust_core
cargo build --release   # → target/release/kolira, wallpaper-*, theme-menu
```

The kit installer copies those binaries into `~/.local/bin/`.

## Tracked Applications

| Category | Apps |
|----------|------|
| KDE/Plasma | KWin, Plasma Shell, KDE Globals |
| GTK/Qt | GTK3, GTK4, Qt5/Qt6 |
| Window Deco | Klassy, Kvantum |
| Terminals | Kitty (live reload), Alacritty, Konsole / Yakuake (manual refresh for open sessions) |
| Launchers | Wofi |
| Monitors | btop |
| Editors | Zed, VS Code, Cursor, Neovim (standalone colourscheme) |
| Browser | Zen / Firefox (colours and day/night only), Pywalfox (explicit opt-in) |
| Notes | Obsidian (selected vaults) |
| Chat | Discord / Vesktop / Vencord |
| System | Color Schemes, Look & Feel |

## Configuration

Settings stored in `~/.config/kolira/app-preferences.json` (or
`$XDG_CONFIG_HOME/kolira/`, see below):

```json
{
  "default_components_mode": "all",
  "custom_components": [],
  "wallpaper_linked_to_profile": false,
  "wallpaper_directory": "",
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

Installed by `kolira/scripts/install-kolira.sh` (GUI
installation is included by default; use `--only-gui` for the app alone):

- `~/.local/bin/kolira-gui` — launcher (execs the app's own
  `kolira-manager.sh`, which sets the QML import paths)
- `~/.local/share/kolira/` — the app copy
- `~/.local/share/applications/kolira.desktop`
- `~/.local/share/icons/hicolor/scalable/apps/kolira-icon.svg`

Both source files live in the repository, so edit them there:
`resources/kolira.desktop` and
`resources/icons/kolira-icon.svg` (icon = dark/light disc with the Breeze
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
      former `libkolira_core.so` second core was removed)
- [ ] Auto-sync with system theme changes
- [ ] Plasma applet version

> A configurable global hotkey (Meta+T) was dropped: KGlobalAccel only creates a
> component object for apps whose desktop entry carries
> `X-KDE-GlobalAccel-CommandShortcut=true`, so a bare D-Bus `doRegister` never
> yielded a usable shortcut.

## License

MIT — see LICENSE in parent directory.

## Apply protection

Profile application shares a process lock with CLI, Inspector writes, app themes,
palettes and wallpapers. A failed profile application restores its captured files;
errors include an operation id, backup and recovery diagnostics. The profile page
keeps interrupted operations in Theme History without startup/refresh error popups;
CLI `kolira operations recover ID`
provides explicit recovery with conflict detection. Inspector keeps staged failures
for retry. See [coverage and limits](../../docs/APPLY_PROTECTION.md).

Paper buttons use `Components/AppButton.qml`, a thin native Button wrapper.
For KDE StyleItem backgrounds, focus painting follows `visualFocus` in Paper:
mouse focus and `highlighted` do not draw Sandstone's inset focus rectangle.
Hover/pressed painting, text and icons stay native; keyboard focus remains
visible. Leaving Paper restores the original native focus binding.

## Pre-apply review

Manual Apply from the main window, saved Theme page and tray opens an asynchronous
file review, including resets, missing dependencies and follow-up actions. Users
can keep wallpaper or save the current appearance first. Changed state invalidates
the confirmation token and opens a new review. Dependency installation is separate
and returns to review. `confirm_before_apply` remains a compatibility setting and
no longer bypasses review. [Contract, CLI and limits](../../docs/APPLY_PREVIEW.md).
The Russian catalog generator fills both Python and QML catalogs before lrelease.

### История и расписание

Меню содержит страницы истории последних 10 смен и правил расписания.
`HistoryScheduleController` выполняет JSON-запросы через асинхронный `QProcess`;
расчёт времени, состояние, копии и восстановление находятся в Rust. Новые точки
подключения: `historySchedule`, `pages/AuditPage.qml`, `pages/SchedulePage.qml`.
Эти точки нужно сохранить при объединении параллельной ветки исправлений UI.
Подробности: [расписание](../../docs/SCHEDULING.md),
[история и восстановление](../../docs/APPLY_PROTECTION.md).

## Layout, feedback, localization and profile cards

Pages use Kirigami grid-unit padding once; cards/dialogs use largeSpacing,
sections largeSpacing, adjacent controls small/mediumSpacing. `AppDialog`
retains native header/footer/lazy sizing and insets content. `ActionFlow` wraps
buttons at the 640×480 minimum and with a larger font. `InsetScrollView` reserves
transient scrollbar channels. Do not combine page padding with a second outer
anchors.margins or use ineffective Layout.margins on a ScrollablePage root child.

The dependency table keeps its vertical bar at the visible viewport edge;
headers and rows share widths and horizontal movement. Pre-apply metadata,
virtualized files and options use one vertical list; only expanded code diff
has its own bounded text scroll. Long names elide with full names available;
prose and paths wrap. Main theme selectors keep a separate row from actions.

All ways to open a component group preserve live bindings. Errors are visible
on the active screen or inside its store/editor modal, and persist until closed
or corrected. Palette colours accept Tab and Enter/Space with a visible focus.
Rust `profile.summaries` loads card fingerprints in one background request;
Python caches/coalesces results and discards stale generations. QML performs no
synchronous field requests or directory walks. Damaged profiles keep basic cards.

Russian includes known runtime statuses, scheduling/history, proper plurals,
locale-aware dates/numbers/sizes, component names and palette source tags.
Translate templates before substitution; user values and unknown external
errors remain verbatim. Translation additions are in tools/ui_i18n_ru.py;
update TS/QM using gui/qt-app/tools/update_translations.py as usual.

Source-only visual/interaction harness (isolated HOME/XDG, no theme application
or network):

```bash
.venv/bin/python ../../tests/e2e/ui-audit.py /tmp/theme.png --page ThemePage --width 640 --height 480 --large-font
.venv/bin/python ../../tests/e2e/ui-audit.py /tmp/diff.png --dialog applyDialog --theme dark --active-bars
.venv/bin/python ../../tests/e2e/ui-audit.py /tmp/check.txt --check
```

It also supports --locale, --long-names, QT_SCALE_FACTOR, and --platform
xcb/wayland for a temporary real window. Renders save PNG/geometry JSON and fail
on runtime QML errors. `--check` exercises all 16 palette colours plus the
accent, Space/Enter, Tab/Shift+Tab, validation feedback, scroll gutters and opening/
closing the import/export file selectors. Real windows request activation again
after closing native colour dialogs; this also makes the offscreen focus check
repeatable. The task records actual checks and remaining live limits.

The Docker friend harness emulates only `plasma-plasmashell.service` when no user
systemd is available. Its service starts in a separate session with `setsid`,
matching systemd's ownership rather than the core helper's process group. Service
status excludes zombie processes. The core's timeout and child cleanup stay active.

CLI/GUI command names and legacy compatibility: [renaming contract](../../docs/RENAMING.md).

### App follow-ups and palette saving

The Apply preview includes “Skip generated app themes and editor installs”.
Its value is bound to the reviewed Rust plan and confirmation token; snapshot
app settings still restore. Both stages share the appearance lock and recovery
copy, and run in a GUI worker. Saved app resources take priority over generated
palette themes. General System Settings and individual KCM buttons are both
available. Palette saving requires applying the current palette to KDE first;
Rust verifies live colors again before saving. See `../../docs/THEMES.md`.

### Returning to a theme with omitted configs

The shared Rust journal records before/after data for mixed configs and generated
app resources. When a subsequent theme omits a config, the restored baseline is
kept. Later user values remain; generated files disappear only if still owned by
the previous Apply and no surviving personal host references them. The preview
shows this same restoration plan. `--keep-wallpaper` preserves the image file too.


The Rust tracked application registry contains 56 entries, including Pure, fzf,
lazygit, Zellij, OpenCode V2, Claude Code, Codex, Rofi, Fuzzel and Walker. Paths and activation limits
are documented in [APPLICATIONS.md](../../docs/APPLICATIONS.md).

Third-batch native colour contracts and activation limits are documented
in [APPLICATIONS.md](../../docs/APPLICATIONS.md). LibreOffice is excluded.

Starship, tmux, Foot and WezTerm use the same Rust palette/apply/return core.
Native tmux changes do not restart sessions; Foot/WezTerm may need a restart.
[Contracts, version limits and native verification](../../docs/APPLICATIONS.md#starship-tmux-foot-wezterm).

Rofi uses native inline colour overrides without changing the selected layout.
Fuzzel uses `[colors]` RGBA and preserves personal opacity, fonts and launch commands.
Walker 2.x patches an existing selected theme stylesheet only. Embedded defaults,
SCSS, external themes and incompatible versions are skipped; Walker remains P4
until a native client restart/return cycle is verified. All three use the shared
Rust Plan and journal; portable snapshots exclude personal Rofi/Walker hosts.
[Launcher contracts and native tests](../../docs/APPLICATIONS.md#лаунчеры).
