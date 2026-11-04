# Themes (theme packages)

A **theme** is what the app manages. A theme is also the "snapshot" you go back
to after a mistake: apply it again and the look returns. There is no separate
"system snapshot" entity. Saving, applying, the themes shipped in the repository
and (planned) `.zip` export/import all use one format: the **theme package**.

A theme stores **everything that affects how the desktop looks — and nothing
else**: KDE colours and fonts, KWin effects and decoration, window rules,
panels and widgets (`plasma-org.kde.plasma.desktop-appletsrc`), GTK 3/4,
Kvantum, terminal colours/fonts/opacity, launchers (wofi), btop, fastfetch,
editor themes (Zed, VS Code, Cursor), Zen (`user.js` and chrome CSS), Obsidian,
plus user-defined components and the wallpaper. Applying someone's theme (or
switching your own) never resets screen scaling, virtual desktops, tiling
layouts, key bindings, editor behaviour or an SSH host.

### What is stored per file (`rust_core/src/appearance.rs`)

| File | Stored and applied | Left alone |
|---|---|---|
| `kdeglobals` | `[Colors:*]`, `[ColorEffects:*]`, `[WM]`, `[Icons]`, fonts, `General` ColorScheme/Accent*, `KDE` LookAndFeelPackage/widgetStyle/contrast/AnimationDurationFactor | `[KScreen]` scaling, `TerminalApplication`, `[KFileDialog Settings]`, `[PreviewSettings]`, other `[KDE]` behaviour |
| `kwinrc` | `[Effect-*]`, `[org.kde.kdecoration2]`, effect switches in `[Plugins]` | `[Desktops]`, `[Tiling]`, `[Windows]`, `[Wayland]`, KWin *scripts* in `[Plugins]` (krohnkite, snapping…) |
| `kwinrulesrc` | the theme's rules (by id) | the user's other rules |
| `plasmarc`, `plasmashellrc` | `[Theme]`; `[PlasmaViews…]` (panel opacity, floating, thickness) | `[Wallpapers]` list, `[Updates]` |
| GTK `settings.ini` | theme, icons, cursor, font, dark preference, title-bar buttons, animations, font rendering | cursor blinking, sounds, `gtk-modules`, … |
| `btop.conf` | `color_theme`, `theme_background`, `truecolor`, `rounded_corners`, `graph_symbol*` | ~70 behaviour keys |
| VS Code / Cursor `settings.json` | `workbench.colorTheme`/`iconTheme`/`productIconTheme`/`colorCustomizations`, `editor.tokenColorCustomizations`, editor/terminal fonts, line highlight, APC / Animations / Custom UI Style keys | everything else (SSH hosts, AI, wrapping, …) |
| Zed `settings.json` | `theme`, `icon_theme`, `ui_font_*`, `buffer_font_*`, `theme_overrides` | agent/models, panels, keymap, sessions, … |
| kitty | `kitty/theme-profile.conf` (fonts, colours, cursor, opacity/blur, padding, borders, tab bar) | `kitty.conf` — only gains `include theme-profile.conf` (last line) |
| Alacritty | `alacritty/theme-profile.toml` (`[colors.*]`, `[font]`, window opacity/blur/padding/decorations, cursor style) | `alacritty.toml` — only gains `general.import` |
| everything else (Klassy, Breeze, GTK CSS, wofi CSS, Zen, Obsidian appearance, Trolltech palette, Kvantum, colour schemes, L&F, fastfetch, panels file) | whole file | — |

Mixed files are **merged** on apply: the theme's allowed keys are written,
allowed keys the theme lacks are removed (e.g. a fixed accent the theme does
not have), every other key keeps the user's value. JSON-with-comments files are
patched by position (comments, order and other keys stay). kitty/Alacritty use
the ricers' approach: the look lives in its own file the user's config
includes; the first save, palette apply or theme apply moves the look out of
`kitty.conf`/`alacritty.toml` into those files (the original goes to
`backups/pre-split-<app>-…` or the pre-apply backup).

Not stored, because they are not appearance: `QtProject.conf` (file-dialog
state), `obsidian.json` (vault list) and Obsidian `Preferences` (spellcheck,
zoom). Desktop icon positions inside `appletsrc` are saved with the file but
never restored: apply keeps the current ones (`positions`, `changedPositions`,
`lastResolution`, `[ScreenMapping]`), so switching themes does not move icons.
Themes saved before 2026-11-04 evening may still contain those files; apply
ignores them.

> 2026-11-04: a short-lived values-only format (`theme@1`: only the 71 registry
> fields) lost terminal colours, GTK4, wofi, Zen, KWin effect toggles and more.
> Do not reintroduce it. Every change to theme storage must be verified by
> applying each theme and diffing the live files against its snapshot.

```
<profiles>/<name>/                 # ~/.config/theme-profile/profiles/<name>
├── theme.json                     # describes the theme (schema theme-profile/theme@2)
├── manifest.json, manifest.txt    # tracked paths, components, created
├── metadata/kde/                  # KWin effects state at save time
├── wallpaper/<file> + source      # optional
├── .config/…, .local/share/…,     # the files themselves, at their
└── .zen/…, obsidian/…             #   $HOME-relative paths
```

Local assets the theme uses live inside the snapshot at their `$HOME`-relative
path: `.local/share/color-schemes/<Name>.colors`, the active Kvantum family dir
`.config/Kvantum/<Family>/`, `.local/share/plasma/look-and-feel/<id>/`, a user
Plasma style `.local/share/plasma/desktoptheme/<id>/`, btop and Zed theme files.

## `theme.json`

| Key | Meaning |
|---|---|
| `schema` | `theme-profile/theme@2` |
| `name`, `description`, `created`, `generator` | metadata (`description` here is authoritative) |
| `values` | field id → value, read from the snapshot through the shared registry `gui/qt-app/src/core/theme_fields.json`: what the GUI's profile view shows and what `check` verifies. Informational: apply restores the files. |
| `palette` | base16 palette + accent (see `docs/PALETTE.md`) when the colour scheme is generated from it, else `null` |
| `wallpaper` | `wallpaper/<file>`, or `null` → applying keeps the current wallpaper |
| `assets[]` | `{kind, name, path, origin: generated\|user, redistributable}` — local assets, `path` inside the theme |
| `sources` | field id → `{store, id, url}`: where to get what is not bundled (`vscode-marketplace`, `open-vsx`, `zed`, or a URL) |

System resources (Breeze, `/usr/share/Kvantum/*`, icon themes, Zed and VS Code
built-ins, …) are referenced by name, not copied. Editor themes from the VS Code
Marketplace / Open VSX / Zed extensions are referenced through `sources`.

## CLI

```bash
theme-profile save NAME [--description=...] [--no-wallpaper]
theme-profile apply NAME [--keep-wallpaper] [--install-missing] [--json]
theme-profile check NAME [--json]          # what the theme references but this machine lacks
theme-profile migrate NAME|--all           # add theme.json to a snapshot profile (files untouched)
theme-profile export NAME [--out=FILE.zip | --dir=DIR] [--no-wallpaper]
theme-profile import FILE.zip|DIR [--name=NEW] [--force] [--apply] [--json]
```

- **save** snapshots the look of every tracked file (filtered as above; old
  version of the theme → `backups/pre-save-<name>-…`), copies in local assets
  that are not tracked files, and writes `theme.json` (`appearance_only: true`).
  `--no-wallpaper` drops `wallpaper/`.
- **apply**:
  1. `--install-missing`: installs missing VS Code / Cursor / Zed themes from
     their official stores;
  2. puts back local assets that live outside the tracked files (only if absent);
  3. the restore: pre-apply backup of every tracked file (whole, as they were)
     in `backups/pre-apply-<name>-…`, plasmashell stopped, whole files copied
     and mixed files merged (above), the colour scheme / accent / widget style
     / KWin effects / kitty re-applied, plasmashell restarted (this is what
     repaints panels and running apps);
  4. wallpaper: the theme's wallpaper comes with its restored desktop layout;
     a theme without a wallpaper, or `--keep-wallpaper`, re-applies the
     wallpaper that was on screen before;
  5. report: `missing` (field, name, where to get it: KDE Store search link,
     store URL or Zed extension id); `--json` prints `{applied, skipped,
     installed, missing, errors, backup}` (used by the GUI and MCP).
- **check** lists what the theme references but this machine lacks; exit code 0.
- **migrate** adds `theme.json` to snapshot profiles and reduces stored files
  to the look (kitty.conf/alacritty.toml → their theme files); the theme is
  copied to `backups/pre-normalize-<name>-…` first. Runs once per theme
  (`appearance_only`). A values-only `theme@1` package cannot be migrated:
  apply its look and save it again.

`theme-profile field set` (interactive edits from the GUI's Theme page) applies
single values: command first (`plasma-apply-colorscheme`,
`plasma-apply-lookandfeel -a`, `kvantummanager`, …), then the key is read back
and written directly if the tool did not store it
(`plasma-apply-colorscheme --accent-color` reports success without writing
`AccentColor`).

## GUI

- **Save Profile** dialog: name, description, *Include the current wallpaper*.
- **Apply** (confirmation dialog): *Keep the current wallpaper*.
- After applying, a **Missing Theme Parts** dialog lists what the theme needs
  but this machine lacks, with links; **Install from stores** re-applies with
  `--install-missing` when an editor theme has a store id.
- The Theme page's profile scope reads the theme values (read-only).

## MCP

`create_profile` (`includeWallpaper`), `apply_profile` (`keepWallpaper`,
`installMissing`, returns `missing`/`installed`), `validate_profile` (returns
`missing`). See `docs/MCP.md`.

## Export and import

An export (`.ktmtheme.zip`, or an unpacked directory with `--dir`) is a
portable copy of a theme:

| Part | In the export |
|---|---|
| `export.json` | `format: theme-profile/export@1`, theme name, date, generator, `home_token: "$HOME"`, `files`: path → SHA-256 |
| `theme.json`, `manifest.json`, `manifest.txt` | `manifest.txt` without its `user=` line |
| tracked appearance files | every file the theme restores (`.config/…`, `.local/share/…`, `.zen/…`, …) |
| library folders | from `color-schemes`, `Kvantum`, `look-and-feel`, `desktoptheme`, `btop/themes`, `zed/themes` only the entries the theme uses (its `assets`) — not every installed theme |
| `wallpaper/` | unless `--no-wallpaper` (then `theme.json` has `wallpaper: null`) |
| left out | `metadata/` (diagnostics), files the theme no longer tracks, symlinks |

Portability: in every text file the home directory becomes the literal `$HOME`
(only as a whole path prefix: `/home/cbr18` is not `/home/cbr`), and import
expands it to the receiver's home — KDE does not expand `$HOME` in appletsrc
and similar files. A literal `$HOME` already in a file (a comment in
`btop.conf`) is expanded too, which means the same thing. Desktop icon
positions are removed from appletsrc.

Import (`.zip` or directory) refuses:
- paths that are absolute, contain `..` (zip-slip) or are symlinks;
- files missing from `export.json` or whose SHA-256 differs, and incomplete exports;
- more than 20 000 files or 1 GiB;
- an existing name (use `--name=NEW`, or `--force`: the old theme goes to
  `backups/pre-import-<name>-…`) and names differing only by case.

It writes into a temporary `profiles/.import-*` directory and renames it into
place, so a refused import leaves nothing behind. The result lists what the
theme references but this machine lacks (`missing`, with where to get it);
`--apply` applies the theme right away. GUI: card menu → *Export…*,
*Import theme…* next to *Save current…* (name conflict → new name or replace),
then the *Missing Theme Parts* dialog. MCP: `export_theme`, `import_theme`.

## Licensing

The code is MIT; each bundled asset keeps its own license
(`redistributable` flag; a LICENSE/source note beside it in published themes).
Anything that may not be redistributed is shipped as a reference only and shows
up in the missing-parts report.
