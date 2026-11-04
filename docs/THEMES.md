# Themes (theme packages)

A **theme** is what the app manages. A theme is also the "snapshot" you go back
to after a mistake: apply it again and the look returns. There is no separate
"system snapshot" entity. Saving, applying, the themes shipped in the repository
and (planned) `.zip` export/import all use one format: the **theme package**.

```
<profiles>/<name>/                 # ~/.config/theme-profile/profiles/<name>
├── theme.json                     # manifest (schema theme-profile/theme@1)
└── assets/
    ├── color-scheme/<Name>.colors # local color scheme the theme uses
    ├── kvantum/<Family>/          # whole Kvantum family dir (variants live inside)
    ├── look-and-feel/<id>/        # local Global Theme
    ├── desktoptheme/<id>/         # local Plasma Style
    ├── btop/<name>.theme
    ├── wallpaper/<file>           # optional
    └── files/<home-relative path> # whole files of user-defined components
```

## `theme.json`

| Key | Meaning |
|---|---|
| `schema` | `theme-profile/theme@1` |
| `name`, `description`, `created`, `generator` | metadata |
| `values` | field id → value, for the fields of the shared registry `gui/qt-app/src/core/theme_fields.json` (colors, fonts, decorations, blur, Kvantum, GTK, terminal opacity, editor themes, Zen, Obsidian, …) |
| `palette` | base16 palette + accent (see `docs/PALETTE.md`), or `null` |
| `wallpaper` | `assets/wallpaper/<file>` or `null` |
| `assets[]` | `{kind, name, path, origin: generated\|user, redistributable}` |
| `sources` | field id → `{store, id, url}`: where to get what is not bundled (`vscode-marketplace`, `open-vsx`, `zed`, or a URL) |
| `files[]` | user-defined component files kept whole |

A theme contains **appearance only**. Panel layout and widgets, window rules,
shortcuts, kitty fonts/keys, Zed settings and other personal configs are never
stored in it.

Installed system resources (Breeze, `/usr/share/Kvantum/*`, icon themes, Zed
built-ins, VS Code built-ins, …) are referenced by name, not copied. Editor
themes from the VS Code Marketplace / Open VSX / Zed extensions are referenced
through `sources` — they are not ours to redistribute.

## CLI

```bash
theme-profile save NAME [--description=...] [--no-wallpaper]
theme-profile apply NAME [--keep-wallpaper] [--install-missing] [--json]
theme-profile check NAME [--json]          # what applying would miss + where to get it
theme-profile migrate NAME|--all           # legacy snapshot profile → package
theme-profile field get [ID...] [--profile=NAME] [--json]
theme-profile field set ID=VALUE ...
```

- **save** reads every registry field from the live session (the same Rust
  field engine the GUI uses), bundles local assets, the wallpaper unless
  `--no-wallpaper`, and replaces an existing theme of that name after moving it
  to `backups/pre-save-<name>-…`.
- **apply** changes **only what the theme defines**:
  - absent component = untouched: a theme without a wallpaper leaves the
    wallpaper as it is — no flag needed; `--keep-wallpaper` keeps it even when
    the theme has one;
  - bundled assets are installed without overwriting existing ones (generated
    assets may be refreshed);
  - a field whose asset is missing is skipped and reported (`missing=field=name
    (source)`: a KDE Store search link, a store URL or the Zed extension id);
    `--install-missing` installs VS Code / Cursor / Zed themes from their
    official stores first;
  - application configs are edited but never created: an app that is not set
    up on this machine is reported as `skipped (app not set up here)`;
  - every touched file is copied to `backups/pre-apply-<name>-…` first;
  - commands run before key writes where Plasma needs them
    (`plasma-apply-colorscheme`, `plasma-apply-lookandfeel`,
    `plasma-apply-desktoptheme`, `kvantummanager`), then the palette broadcast;
    plasmashell is not restarted;
  - `--json` prints the report `{applied, skipped, installed, missing, errors,
    backup}` instead of text (used by the GUI and MCP).
- **check** lists what `apply` would skip as missing; exit code 0 either way.
- **migrate** converts a legacy snapshot profile (full config copies) into a
  package, taking assets from the snapshot and falling back to the live ones;
  the old snapshot is moved to `backups/pre-migrate-<name>-…`.

Legacy snapshot profiles still apply the old way (file restore + component
filter from *Settings → Profile Apply Mode*) until they are migrated.

## GUI

- **Save Profile** dialog: name, description, *Include the current wallpaper*.
- **Apply** (confirmation dialog): *Keep the current wallpaper*.
- After applying, a **Missing Theme Parts** dialog lists what the theme needs
  but this machine lacks, with links; **Install from stores** re-applies with
  `--install-missing` when an editor theme has a store id.
- The Theme page's profile scope reads the package values (read-only).

## MCP

`create_profile` (`includeWallpaper`), `apply_profile` (`keepWallpaper`,
`installMissing`, returns `missing`/`installed`), `validate_profile` (returns
`missing`). See `docs/MCP.md`.

## Licensing

The code is MIT. Each bundled asset keeps its own license (`redistributable`
flag; a LICENSE/source note beside it in published themes). Anything that may
not be redistributed is shipped as a reference only and shows up in the
missing-parts report.
