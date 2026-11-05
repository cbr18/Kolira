# AGENTS.md — theme-profile-kit

Instructions for AI agents and contributors working in this repository.

## What this repository is

`theme-profile-kit` is the **source of truth** for:

| Area | Source |
|---|---|
| Rust CLI | `gui/qt-app/src/core/rust_core/src/bin/` (`theme-profile`, `theme-menu`, `wallpaper-*`) |
| Qt/QML GUI ("KDE Theme Manager") | `gui/qt-app/` |
| Install / update / uninstall | `scripts/*.sh` |
| Tests | `tests/test-scripts.sh`, `gui/qt-app/tests/` |

Deployed copies live in `~/.local/bin/`, `~/.local/share/kde-theme-manager/`
and `~/.local/share/applications/`. **Never edit deployed copies directly** —
edit the kit and re-run the installer.

The end-user data directory (`profiles/`, `backups/`, `manifests/`,
`active-profile`, `app-preferences.json`) is separate from the code and is never
touched by install/update.

## Architecture decision (fixed — no other variants)

**The stack is «Rust core + Python GUI». Everything else is off the table.**

1. **GUI is 100 % Python (PySide6 + QML).** The application will not be
   rewritten in Rust, C++/Qt, nor any other GUI framework. There is no "Rust
   GUI" plan.
2. **Core and CLI are 100 % Rust; the GUI has a single core.** `gui/qt-app/src/core/rust_core/`
   provides the CLI binaries (`theme-profile`, `theme-menu`,
   `wallpaper-picker/next/prev` in `src/bin/`); the GUI talks to them through
   one Python adapter (`core/cli_core.py`, backend name `core`). All profile
   mutations, `status`, wallpaper logic, settings and the app registry live in
   Rust. The former second core — the `libkde_theme_core.so` shared library
   behind `ctypes` — was removed; do not reintroduce a second core.
3. **Bash is an exception, not an option.** A bash script may exist only where
   Rust genuinely cannot express the task or would cost roughly 20× the code
   with no measurable benefit — today that is limited to:
   - deployment/update/uninstall (`scripts/*.sh`),
   - the PySide6 launcher wrapper (`gui/qt-app/theme-profile-manager.sh`),
   - the test harness (`tests/test-scripts.sh`).
   No application logic lives in bash. External tools (kreadconfig6,
   kwriteconfig6, magick, plasma-apply-wallpaperimage, kitty, wofi) are
   binaries invoked from Rust/Python, not bash scripts.

When in doubt: new logic goes to the Rust core; GUI-only glue (Qt signal/slot,
QML mapping) goes to Python. If you think bash is needed, it is a decision to
be reviewed, not an implementation detail.

## Documentation rule (mandatory)

**Every code change must be reflected in the documentation in the same task.**
A change is not done until the docs agree with the code. At minimum:

1. Update the doc(s) that describe the changed behaviour. See the map below.
2. If a user-visible behaviour, path, default, CLI flag, setting, or contract
   changed, update the relevant README/docs text and any code example.
3. If a documented test count, file path, or architecture diagram becomes stale,
   fix it in the same task.
4. Keep code comments and docstrings in sync when they explain *why* something
   is done — stale rationale is worse than none.

### Where documentation lives

| Document | Covers |
|---|---|
| `README.md` | Install/usage, wallpapers, profiles, Theme Inspector, update/uninstall, rollback |
| `docs/CURRENT_APP.md` | Current app, tracked paths, directory layout, known limits |
| `docs/THEME_INSPECTOR.md` | Theme Inspector UX + architecture |
| `docs/ROLLBACK.md` | Backup layout and recovery |
| `docs/ROADMAP.md` | Direction and completed work |
| `docs/TZ-QT-APP.md`, `docs/TZ-THEME-INSPECTOR.md` | Original specifications |
| `docs/TZ-THEME-REBUILD.md` | Target product/design spec for the theme-bundle rebuild |
| `docs/MCP.md` | Local MCP server usage, tool contract and safety notes |
| `docs/PALETTE.md` | base16 palette model, `theme-profile palette` CLI, base16 → KDE role mapping |
| `docs/THEMES.md` | Theme format (appearance-only snapshot + `theme.json`), per-file appearance policies, dependencies, save/apply/check/migrate, export/import |
| `gui/qt-app/README.md` | GUI details: cores, tray, theming, wallpapers, architecture, dev commands |

## Contracts that must not drift

- **Profile-name validation**: `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, implemented
  identically in `gui/qt-app/src/core/paths.py` (`validate_profile_name`), the
  Rust CLI + core (`src/paths.rs`). Names are single
  path components; reject anything else (traversal, separators, leading dot).
- **XDG-aware paths**: live files resolve through `XDG_CONFIG_HOME` /
  `XDG_DATA_HOME` / `XDG_CACHE_HOME`. The data root prefers an existing
  `$XDG_CONFIG_HOME/theme-profile`, then an existing legacy
  `~/.config/theme-profile`, then the XDG path. Profile *snapshots* keep the
  historical `.config/...` layout — do not change that without a migration.
- **Wallpaper list ordering**: shared by the Rust CLI and the Python GUI core —
  follow symlinks, extensions `jpg jpeg png webp bmp` (case-insensitive),
  byte/code-point order (`Vec<String>::sort()` / `sorted()`). See the table in
  `gui/qt-app/README.md`.
- **Profile scopes are read-only snapshots** in the Theme Inspector; only
  *Live* is editable. Mutating profile operations delegate to the Rust CLI
  `theme-profile`.
- **`KTM_THEME_NO_APPLY=1`** disables external commands and D-Bus hooks (used by
  tests and dry runs).
- **JSONC files** (Zed / VS Code / Cursor / Obsidian / fastfetch) are patched by
  value position and never reserialised.
- **Single core**: `gui/qt-app/src/core/cli_core.py` shells to the Rust CLI for
  profile/wallpaper operations; the backend name is `core` (`auto` is the same
  thing). Select with `--core`, `$KTM_CORE` or *Settings → Core*.

## Verification (run before committing)

```bash
# Build the Rust CLI first
cd gui/qt-app/src/core/rust_core && cargo build --release && cargo test && cargo clippy --all-targets -- -D warnings

# Python tests (offscreen Qt; 254 tests)
cd gui/qt-app && QT_QPA_PLATFORM=offscreen uv run --project . --extra dev pytest tests -q

# Bash + installer tests (~198 assertions) — from the repo root
bash tests/test-scripts.sh

# Shell static analysis (installer scripts)
shellcheck scripts/*.sh tests/test-scripts.sh

# QML lint
find gui/qt-app/qml -name '*.qml' -print0 | xargs -0 qmllint

# Translations (QML + Python → merged kde-theme-manager_ru.qm)
bash scripts/update-translations.sh
```

`ruff` / `mypy` configs exist but are **not** enforced by CI; do not churn the
codebase to satisfy them unless asked.

## Editing rules

1. Edit `gui/qt-app/src/core/rust_core/**` for the CLI and core; deploy with
   `scripts/install-theme-profile-kit.sh` (builds with `cargo build --release`).
2. Edit `gui/qt-app/**` for the GUI; deploy with
   `scripts/install-theme-profile-kit.sh` (use `--only-gui` for the app alone).
3. Do **not** edit `~/.config/kwinrc`, `plasmashellrc`, `kdeglobals` without a
   backup; do **not** edit Zen `prefs.js` (use the profile `user.js`).
4. Prefer `uv` for Python and `cargo` for Rust on this machine.
5. Keep writes to config files atomic and permission-preserving
   (`gui/qt-app/src/core/fileutil.py`).

## License

MIT — see `LICENSE`. Remote: `github.com/cbr18/kde-theme-manager`.
