# Themes (theme packages)

A **theme** is what the app manages. A theme is also the "snapshot" you go back
to after a mistake: apply it again and the look returns. There is no separate
"system snapshot" entity. Saving, applying, the themes shipped in the repository
and (planned) `.zip` export/import all use one format: the **theme package**.

A theme stores **everything that affects how the desktop looks**, as whole
files: KDE colours and fonts, KWin effects and window rules, panels and widgets
(`plasma-org.kde.plasma.desktop-appletsrc`), GTK 3/4 (settings and CSS),
Kvantum, terminal configs with their colours, launchers (wofi), btop,
fastfetch, editor settings (Zed, VS Code, Cursor), Zen (`user.js` and chrome
CSS), Obsidian, plus user-defined components and the wallpaper.

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
```

- **save** snapshots every tracked file (old version of the theme →
  `backups/pre-save-<name>-…`), copies in local assets that are not tracked
  files, and writes `theme.json`. `--no-wallpaper` drops `wallpaper/`.
- **apply**:
  1. `--install-missing`: installs missing VS Code / Cursor / Zed themes from
     their official stores;
  2. puts back local assets that live outside the tracked files (only if absent);
  3. the snapshot restore: pre-apply backup of every tracked file
     (`backups/pre-apply-<name>-…`), plasmashell stopped, files restored, the
     colour scheme / accent / widget style / KWin effects / kitty re-applied,
     plasmashell restarted (this is what repaints panels and running apps);
  4. wallpaper: the theme's wallpaper comes with its restored desktop layout;
     a theme without a wallpaper, or `--keep-wallpaper`, re-applies the
     wallpaper that was on screen before;
  5. report: `missing` (field, name, where to get it: KDE Store search link,
     store URL or Zed extension id); `--json` prints `{applied, skipped,
     installed, missing, errors, backup}` (used by the GUI and MCP).
- **check** lists what the theme references but this machine lacks; exit code 0.
- **migrate** only adds `theme.json` (plus local assets the snapshot lacked) —
  the snapshot files are not changed. A values-only `theme@1` package cannot be
  migrated: apply its look and save it again.

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

## Sharing and licensing

A theme holds personal files (panels with their widgets, window rules, editor
settings, `$HOME` paths). Export (planned) must sanitise them before a theme
leaves the machine. The code is MIT; each bundled asset keeps its own license
(`redistributable` flag; a LICENSE/source note beside it in published themes).
Anything that may not be redistributed is shipped as a reference only and shows
up in the missing-parts report.
