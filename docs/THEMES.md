# Themes (theme packages)

A **theme** is what the app manages. A theme is also the "snapshot" you go back
to after a mistake: apply it again and the look returns. There is no separate
"system snapshot" entity. Saving, applying, the themes shipped in the repository
and (planned) `.zip` export/import all use one format: the **theme package**.

A theme stores **everything that affects how the desktop looks — and nothing
else**: KDE colours and fonts, KWin effects and decoration, the look of window rules,
the look of panels and widgets (wallpaper, Panel Colorizer,
panel floating/opacity), GTK 3/4,
Kvantum, terminal colours/fonts/opacity, launchers (wofi), btop,
editor themes (Zed, VS Code, Cursor), Zen (`user.js` and chrome CSS), Obsidian,
plus user-defined components and the wallpaper. Applying someone's theme (or
switching your own) never resets screen scaling, virtual desktops, tiling
layouts, key bindings, editor behaviour or an SSH host — nor the panel and
widget **layout**: which panels and widgets exist, where they sit, their sizes
and settings stay the user's (since 2026-11-05, `rust_core/src/layout.rs`).

### What is stored per file (`rust_core/src/appearance.rs`)

**Sizes never travel** — screens differ: font *families* are part of the look,
font sizes and the cursor size are the user's. Where a size lives inside a
font string (KDE `family,pointSize,pixelSize,…`, GTK `family size`), apply
takes the family from the theme and keeps the live size.

| File | Stored and applied | Left alone |
|---|---|---|
| `kdeglobals` | `[Colors:*]`, `[ColorEffects:*]`, `[WM]`, `[Icons]`, font families (live sizes kept), `General` ColorScheme/Accent*, `KDE` LookAndFeelPackage/widgetStyle/contrast/AnimationDurationFactor | `[KScreen]` scaling, `TerminalApplication`, `[KFileDialog Settings]`, `[PreviewSettings]`, other `[KDE]` behaviour |
| `kwinrc` | `[Effect-*]`, `[Round-Corners]` (Rounded Corners effect), `[org.kde.kdecoration2]`, effect switches in `[Plugins]`. Better Blur DX in the bundled themes blurs everything except `WindowClasses=plasmashell\\nswaync`: forced blur covers the whole surface, so panels and notifications, whose surfaces have transparent margins, use the exact area they request instead of getting a halo. The settings of an effect the theme keeps off are left out (`<id>Enabled=false`, or no key for `translucency`/`blurplus`, which are off by default) | `[Desktops]`, `[Tiling]`, `[Windows]`, `[Wayland]`, KWin *scripts* in `[Plugins]` (krohnkite, snapping…); the live settings of an effect the theme keeps off |
| `kwinrulesrc` | rules with a look property, reduced to their match keys (`Description`, `wmclass*`, `title*`, `types`, …) and look properties (`opacityactive/inactive`, `noborder`, `decocolor`, `decorationpolicy` + their `…rule`); merged by id — a live rule with the same id keeps its behaviour keys. The rule list is written in the live file's format (KWin 6: `Order=`, older: `count`/`rules`) | the user's other rules; position, size, desktops, shortcuts and other behaviour properties |
| `plasmarc` | `[Theme]` | `[Wallpapers]` list |
| `plasmashellrc` | per panel `[PlasmaViews][Panel N]` `floating`, `panelOpacity`; a live panel takes the theme panel with the same id, else the one on the same screen edge, else the first | thickness, lengths, alignment, offsets, auto-hide (`panelVisibility`), `[Updates]` |
| `plasma-org.kde.plasma.desktop-appletsrc` | per containment only `plugin`/`formfactor`/`location`/`lastScreen`/`wallpaperplugin` (to match, never restored); each desktop's active wallpaper plugin settings → the live desktop on the same screen (else the first); look keys of widgets the user already has — Panel Colorizer `globalSettings`/`configurationOverrides`/`isEnabled`/`hideWidget` — the n-th live widget of a kind takes the theme's n-th (else first). A Panel Colorizer switched off in the theme keeps only `isEnabled=false`: applying turns the user's off, keeps their styling, and it is not a dependency | which panels and widgets exist, their order and place, every other widget setting (favourites, weather location, clock format…), activities, desktop icons; a theme never adds a widget — a styled one the user lacks is simply not styled |
| GTK `settings.ini` | theme, icons, cursor theme, font family (live size kept), dark preference, title-bar buttons, animations, font rendering; apply and `field set` also write `gtk-theme` and `color-scheme` to GSettings `org.gnome.desktop.interface` (Wayland GTK 3 apps, Firefox and Zen included, read the theme there, and a stale dark theme gives a light browser white system text) | cursor size, cursor blinking, sounds, `gtk-modules`, … |
| `kcminputrc` | `[Mouse] cursorTheme` | cursor size, mouse settings |
| `btop.conf` | `color_theme`, `theme_background`, `truecolor`, `rounded_corners`, `graph_symbol*` | ~70 behaviour keys |
| VS Code / Cursor `settings.json` | `workbench.colorTheme`/`iconTheme`/`productIconTheme`/`colorCustomizations`, `editor.tokenColorCustomizations`, editor/terminal font family, weight, ligatures, line highlight, APC / Animations / Custom UI Style keys | font sizes, line height, everything else (SSH hosts, AI, wrapping, …) |
| Zed `settings.json` | `theme`, `icon_theme`, `ui_font_family/features/weight`, `buffer_font_family/features/weight`, `theme_overrides` | font sizes, agent/models, panels, keymap, sessions, … |
| kitty | `kitty/theme-profile.conf` (font faces, colours, cursor, opacity/blur, padding, borders, tab bar) | `kitty.conf` — keeps `font_size`/`modify_font` and gains `include theme-profile.conf` (last line) |
| Discord (Vencord / Vesktop) | `themes/` (CSS themes) + `settings/settings.json` `enabledThemes`/`themeLinks`/`transparent` (translucent window); Vesktop ↔ Vencord themes map to whichever client the receiver has | every other client setting; stock Discord has no themes → `discord-client` dependency |
| Spotify (Spicetify) | `Themes/` + `config-xpui.ini` `[Setting]` `current_theme`, `color_scheme`, `inject_css`, `replace_colors`, `overwrite_assets`, `inject_theme_js`; apply runs `spicetify apply` | `spotify_path`, `prefs_path`, extensions, custom apps; Spicetify itself is a dependency (`spicetify-cli`) |
| Telegram | `TelegramDesktop/theme-profile.tdesktop-palette` (palette file generated from the theme palette); Telegram keeps the applied theme in its encrypted settings: the user picks this file once (*Settings → Chat Settings → Choose from file*), then Telegram watches it and re-applies it whenever a theme or palette rewrites it | everything in `tdata` (account, chats, settings, the chat background) |
| oh-my-posh | the prompt theme the shell's `oh-my-posh init … --config` line uses, stored as `oh-my-posh/theme-profile.omp.json` | `config.fish` / `.zshrc` / `.bashrc` — only the `--config` path of an existing init line is changed (never added); oh-my-posh is a dependency |
| fish | `fish_variables` lines `SETUVAR fish_color_*` / `fish_pager_color_*` | other universal variables (`fish_user_paths`, …) |
| Ghostty | `ghostty/theme-profile` (colours, palette, opacity/blur, font families, padding) | `config` — keeps the rest and gains `config-file = ?theme-profile` (last) |
| cava | `[color]` (running cava reloaded with SIGUSR1) | every other section |
| yazi | `theme.toml`, `flavors/` (whole) | `yazi.toml`, `keymap.toml` |
| bat | `config` lines `--theme…` / `--italic-text…`, `themes/` (then `bat cache --build`) | other `config` lines |
| Konsole | the default profile's `[Appearance]` (`ColorScheme`, font family — live size kept) + its `.colorscheme` (whole: colours, opacity, blur; travels as an asset) | the profile's command, scrolling, keys…; without a profile a `Theme Profile` (`Parent=FALLBACK/`) is created and made default |
| Firefox (default profile from `installs.ini`/`profiles.ini`) | `chrome/userChrome.css`, `chrome/userContent.css` (whole); `user.js` only look prefs (`browser.theme.*`, `toolkit.legacyUserProfileCustomizations.stylesheets`, `extensions.activeThemeID`, …) | privacy/behaviour prefs in `user.js` |
| Alacritty | `alacritty/theme-profile.toml` (`[colors.*]`, `[font]` faces, window opacity/blur/padding/decorations, cursor style) | `[font] size`/offsets; `alacritty.toml` gains `general.import` |
| everything else (Klassy, Breeze, GTK CSS, wofi CSS, Zen, Obsidian appearance, Trolltech palette, Kvantum, colour schemes, L&F) | whole file (a Look & Feel package without its `contents/layouts/` scripts). `breezerc` only when the widget style or window decoration is Breeze (absent keys mean Breeze), `klassy/` only when either is Klassy | when not stored: the user's own `breezerc` / `klassy/` |

Browser profile folders have a random name per install (`gp0y8wrv.Default
(release)`): a theme made elsewhere stores its files under the author's
folder, and apply maps them to the receiver's default profile (Zen, Firefox);
the same for Konsole's default profile.

**What this machine lacks stays as it is.** An icon theme, cursor, GTK theme, window
decoration or Plasma style that is not installed is not switched to: the current one
stays and `skipped` in the report says so. Applying it would leave blank icons or
a KWin without its decoration.

**Downloads come first.** The window's Apply lists what is missing before anything
changes: *Download and apply* or *Apply without them*. Each item has its own limit
and a failed one costs only itself; the terminal for system packages is waited for.
Every external tool has a limit (`KTM_COMMAND_TIMEOUT`, 60 s) and each step goes to
`<theme root>/apply.log` at once, so after a freeze its last line names the step.

**What a theme does not specify goes back to the program's default.** A theme
without Vesktop files turns our Vesktop theme off (`enabledThemes` gone, our CSS
removed), a theme without Konsole removes the profile we created, and so on: a
mixed file loses its look keys, a generated `theme-profile*` file is removed. Only
what we generate is removed, a program's other files are the user's. A running
program that reads its look at start gets the usual offer to restart. Telegram
keeps the applied palette in its own settings, so it stays until chosen there.

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
zoom). Desktop icons, panel/widget layouts and Look & Feel layout scripts are
not stored either (see the table). Themes saved before 2026-11-05 carry the
whole `appletsrc`, `plasmashellrc` geometry, behaviour window rules and
`contents/layouts/`, and before 2026-11-07 also settings their look does not use
(`breezerc`, `klassy/`, off effects, a switched-off Panel Colorizer's styling):
apply reads only the look from the mixed files (the same filter), and
`theme-profile prune NAME|--all` reduces them on disk (backup first).

**The look before the app** is kept: the first install, with no themes yet, saves
it as the theme `my-original-look` (`install-theme-profile-kit.sh
--no-original-theme` skips it; offline, so it carries no store sources).

**Switching back leaves nothing behind** (`rust_core/src/undo.rs`). A theme only
says what it contains, so each apply first undoes what the previous one left, from
a journal in `<theme root>/apply-journal/` (local state, never in a theme):

- files that did not exist before the theme are removed;
- whole files it replaced get their earlier content back;
- `kwinrc` effect settings and `kwinrulesrc` rules it wrote go back to what they
  were (a theme's rule does not stay in the user's rules);
- GSettings `gtk-theme` / `color-scheme` go back to their earlier value;
- browser prefs: a pref set in `user.js` stays in `prefs.js`, so the earlier value
  (or, for known look prefs, the default) is written to `user.js` for the next start.

Only what the theme left untouched is undone: a file or block edited since then
stays. Applying also skips the config folder of a program this machine lacks (no
`~/.config/Cursor`, no `~/obsidian`), writes Zen files to the receiver's default
profile, keeps the receiver's launcher unchanged, and a save records the GTK theme GSettings really reports when it differs
from `settings.ini` (`gsettings_gtk_theme` in `theme.json`). External tools cannot
hang an apply: each is stopped after 60 s (`KTM_COMMAND_TIMEOUT`), and a Kvantum
theme that is not installed is not handed to `kvantummanager`. Checked by
`tests/e2e/`: `friend-in-container.sh` runs the whole story in a clean container
with a real KWin and plasmashell (see `tests/e2e/Dockerfile`).

> 2026-11-04: a short-lived values-only format (`theme@1`: only the 71 registry
> fields) lost terminal colours, GTK4, wofi, Zen, KWin effect toggles and more.
> Do not reintroduce it. Every change to theme storage must be verified by
> applying each theme and diffing the live files against its snapshot.

```
<themes>/<name>/                 # ~/.config/kde-theme-profile/themes/<name>
├── theme.json                     # describes the theme (schema theme-profile/theme@2)
├── manifest.json, manifest.txt    # tracked paths, components, created
├── wallpaper/<file> + source      # optional
├── .config/…, .local/share/…,     # the files themselves, at their
└── .zen/…, obsidian/…             #   $HOME-relative paths
```

**Only the theme's own assets.** From folders that collect many themes
(colour schemes, Kvantum, Look & Feel, btop/Zed themes) a theme stores just
what it uses — the active scheme, Kvantum family, Global Theme, btop/Zed theme
file — and applying restores exactly
those (listed in `theme.json` `assets`), backed up like every other file. (Until
2026-11-05 the registry hard-coded one user's assets, so every theme carried
every other theme's Sandstone, glass/paper Look & Feel, noctalia/paper btop
themes and all Zed themes; `theme-profile prune NAME|--all` removes such
leftovers, plus files no longer tracked and old `metadata/` diagnostics, after
copying the theme to `backups/pre-prune-<name>-…`.) Themes carry no
`metadata/kde` diagnostics any more (pre-apply backups still do).

Local assets the theme uses live inside the snapshot at their `$HOME`-relative
path: `.local/share/color-schemes/<Name>.colors`, the active Kvantum family dir
`.config/Kvantum/<Family>/`, `.local/share/plasma/look-and-feel/<id>/`, a user
Plasma style `.local/share/plasma/desktoptheme/<id>/`, a user Aurorae window
decoration `.local/share/aurorae/themes/<name>/` (field `decoration_theme` =
`__aurorae__svg__<name>`), btop and Zed theme files. A store decoration the
author may not redistribute is withheld by `export --public` and becomes a
dependency; a derivative with a free licence (frutiger-aero-light's
`Se7enAeroLight`, GPL-2) travels inside the theme.

Asset destinations are validated by Rust at import/load and again before apply:
each kind has an appearance-only container (colour schemes, Kvantum, Look & Feel,
Plasma styles, Aurorae, btop/Zed themes, Konsole `.colorscheme`, or `wallpaper/`).
Unknown kinds, absolute/traversing paths and container ancestors are rejected;
an asset cannot request a write to an unrelated HOME file. Apply checks source
and destination asset links before dependency installation or backups. Links
inside an asset or below its destination container are rejected; relocating the
container itself, including through XDG, remains supported. Legacy fastfetch
assets are ignored as before. These checks do not make external dependencies
or a complete theme application transactional.

`bat cache --build` uses the common external-command timeout. Its failure or
timeout is included in `apply --json` `errors`; the completed file restoration
and its backup remain available.

## `theme.json`

| Key | Meaning |
|---|---|
| `schema` | `theme-profile/theme@2` |
| `name`, `description`, `created`, `generator` | metadata (`description` here is authoritative) |
| `values` | field id → value, read from the snapshot through the shared registry `gui/qt-app/src/core/theme_fields.json`: what the GUI's profile view shows and what `check` verifies. Informational: apply restores the files. |
| `palette` | base16 palette + accent (see `docs/PALETTE.md`) when the colour scheme is generated from it, else `null` |
| `wallpaper` | `wallpaper/<file>`, or `null` → applying keeps the current wallpaper |
| `assets[]` | `{kind, name, path, origin: generated\|user, redistributable}` — local assets, `path` inside the theme |
| `sources` | field id → `{store, id, url}`: editor-theme extensions (`vscode-marketplace`, `open-vsx`, `zed`) |
| `dependencies[]` | `{kind, name, source, id, title, url, exact}` — everything the theme needs that does not travel inside it (see *Dependencies*) |
| `appearance_only` | the stored files hold only the look |

System resources (Breeze, `/usr/share/Kvantum/*`, icon themes, Zed and VS Code
built-ins, …) are referenced by name, not copied. Editor themes from the VS Code
Marketplace / Open VSX / Zed extensions are referenced through `sources`.

## CLI

```bash
theme-profile save NAME [--description=...] [--no-wallpaper]
theme-profile new NAME --from=neutral-light|neutral-dark|THEME [--description=...]
theme-profile look-and-feel NAME           # the live look as a Global Theme (no layout)
theme-profile apply NAME [--keep-wallpaper] [--install-missing] [--json]
theme-profile check NAME [--json]          # what the theme references but this machine lacks
theme-profile migrate NAME|--all           # add theme.json to a snapshot profile (files untouched)
theme-profile prune NAME|--all             # drop what is not part of the theme, incl. layouts (backup first)
theme-profile export NAME [--out=FILE.zip | --dir=DIR] [--no-wallpaper] [--public]
theme-profile license NAME ASSET --license=SPDX [--source=URL] [--no-redistribute]
theme-profile import FILE.zip|DIR [--name=NEW] [--force] [--apply] [--json]
theme-profile bundled sync THEMES_DIR      # add new ready themes, update unchanged ones
theme-profile deps NAME [--refresh] [--install[=all|0,2]] [--json]
theme-profile actions NAME [--json]        # what applying leaves to do (restarts, Telegram)
theme-profile action ID                    # do one: vesktop, discord, zen, firefox
theme-profile automation [--json] | set KEY=ask|always|never … | ask   # the user's choices
```

- **save** snapshots the look of every tracked file (filtered as above; old
  version of the theme → `backups/pre-save-<name>-…`), copies in local assets
  that are not tracked files, and writes `theme.json` (`appearance_only: true`).
  `--no-wallpaper` drops `wallpaper/`.
  Licences: generated assets get theirs on every save (colour/Konsole schemes
  MIT; a Kvantum recolour follows its base — see `docs/PALETTE.md`); marks set
  with `theme-profile license` carry over across re-saves, including a hand
  mark on a generated asset (Paper's Breeze-derived scheme).
- **apply**:
  1. `--install-missing`: installs missing VS Code / Cursor / Zed themes from
     their official stores;
  2. puts back local assets that live outside the tracked files (only if absent);
  3. the restore: pre-apply backup of every tracked file (whole, as they were)
     in `backups/pre-apply-<name>-…`, plasmashell stopped, whole files copied
     and mixed files merged (above), the colour scheme / accent / widget style
     / KWin effects / kitty re-applied, plasmashell restarted (this is what
     repaints panels and running apps);
  4. wallpaper: the theme's wallpaper is written to the desktops and applied; a
     theme without a wallpaper, or `--keep-wallpaper`, leaves the desktop's
     wallpaper settings as they are, of any kind (slideshow, picture of the day).
     Such a theme stores no wallpaper path or plugin, nor does `export
     --no-wallpaper`;
  5. report: `missing` (field, name, where to get it: KDE Store search link,
     store URL or Zed extension id); `--json` prints `{applied, skipped,
     installed, missing, errors, backup}` (used by the GUI and MCP).
- **new** starts a theme (`rust_core/src/constructor.rs`). From
  `neutral-light`/`neutral-dark` it applies a neutral template to the live
  session — Breeze colour scheme, widget style, icons, cursor, Plasma style and
  window decoration, Breeze GTK, no blur, no translucency, opaque kitty/Alacritty,
  no transparent Zen — and saves it under NAME. From an existing THEME it copies
  that theme under NAME and applies it. Either way NAME becomes the active
  theme; the user then changes the live look and saves over it. Applying makes
  the usual pre-apply backup first.
- **look-and-feel** writes `~/.local/share/plasma/look-and-feel/theme-profile-<name>/`
  (`metadata.json` + `contents/defaults`: colour scheme, widget style, icons,
  cursor, Plasma style, window decoration — never a layout, so choosing it in
  System Settings does not touch panels) and selects it (only the
  `LookAndFeelPackage` key; the look is already live). The next save carries it
  as the theme's own generated asset (MIT).
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

- **Save Profile** dialog: name, description, *Include the current wallpaper*,
  *Create a Look & Feel package for this theme* (runs `look-and-feel` before
  saving, so the package is part of the theme).
- **New theme…** (profiles toolbar): name and base — *Neutral, light/dark*
  or *Copy of "X"* for each theme — then `theme-profile new`; a warning says
  that starting replaces the current look (a backup is made).
- **Apply** (confirmation dialog): *Keep the current wallpaper*.
- After applying or importing, a **This Theme Needs** dialog lists the missing
  dependencies with their source: store items have a checkbox (exact matches
  ticked, search results unticked until confirmed), system packages show the
  command; **Install selected** installs them and (optionally) applies the
  theme again.
- The Theme page's profile scope reads the theme values (read-only).

## MCP

`create_profile` (`includeWallpaper`), `apply_profile` (`keepWallpaper`,
`installMissing`, returns `missing`/`installed`), `validate_profile` (returns
`missing`), `list_dependencies`, `install_dependencies`. See `docs/MCP.md`.

## Dependencies

Detected when a theme is saved (`rust_core/src/deps.rs`) for every asset it
references but does not carry: icons, cursors, GTK, Kvantum, Plasma style,
Global Theme, colour scheme, window decoration (plugin or Aurorae),
third-party KWin effects the theme switches on (`kwin-effect`: Better Blur DX,
Rounded Corners… — the package owning the plugin, or the store item of a
scripted effect), panel
widgets the user installed, editor themes, the Firefox theme add-on
(`amo`: link to addons.mozilla.org — add-ons are installed from inside
Firefox), a Discord theming client (`discord-client`: Vesktop from the AUR or
Vencord), oh-my-posh for a prompt theme. The login screen (SDDM) is not touched, read or
listed: it is system-wide, outside what a theme can give back (older themes that
list it are read without it). KDE's own assets (Breeze, Oxygen,
`org.kde.*`) are never dependencies. Sources, in order:

1. **KDE Store, exact** — KDE's KNewStuff registries
   (`~/.local/share/knewstuff3/*.knsregistry`, written by *Get New…*) record the
   store id of everything installed that way; items fetched by `deps --install`
   are recorded the same way in `<theme-profile data>/installed-assets.json`
   (schema 2: `sources` keeps path → store id/title, `resources` keeps dependency
   requirements, ownership and factual state), so re-saving a theme keeps their exact source;
2. **package** — `pacman -Qqo` on any installed copy (repo or AUR; a user copy
   shadowing a packaged one — Klassy's generated icons — is looked through);
3. **editor stores** — the installed VS Code / Cursor extension or Zed
   extension — ours (`zed/themes/<extension>--<file>.json`) or one Zed
   installed itself (`zed/extensions/installed/<extension>/`) (an editor that is not installed on this machine — no binary on
   PATH, no Flatpak — gets nothing: its theme is neither listed by `deps`,
   nor downloaded, nor reported missing by `check`/`apply`); VS Code and Cursor share extensions, so when one editor is not
   installed but names the same theme as the other, the other's extension id
   is used (VS Code Marketplace / Open VSX);
4. **KDE Store, search** (`exact: false`) — the OCS API searched by name within
   the right category, ranked (exact title, acronym like YAMIS = *Yet Another
   Monochrome Icon Set*, KDE/Plasma builds over forks); shown for confirmation;
5. otherwise just a KDE Store search link.

`theme-profile deps NAME` lists those not installed here (`--json` for the
GUI/MCP; the apply and import reports carry them as `dependencies`).
`--install` fetches them from the official source: KDE Store items through the
OCS API (`api.kde-look.org`) — Plasma styles, Global Themes and widgets with
`kpackagetool6`, icons/cursors/GTK/Kvantum/Aurorae unpacked into the user's
folders (zip via enclosed names, tarballs via `tar`; symlinks escaping the
tree are dropped; an existing folder is never overwritten), colour schemes
copied; editor themes via their stores. **System packages**
(they need root) go to one terminal the user sees, and the app waits for it — konsole, kitty, alacritty
or ghostty, the first installed — running one command for all of them
(`yay`/`paru -S --needed …`, or `sudo pacman -S --needed …` for repository-only
packages without an AUR helper); the user types their
own password there; the Rust terminal worker invokes the package manager and
publishes its result without waiting for an extra Enter. The GUI never asks for
or stores the password. Package identifiers are validated before invocation.
Aurorae decorations are looked up in
both the "Aurorae" and "Window Decorations" store categories. An icon theme
made for GNOME (Seven Icons: `Inherits=Humanity,…,gnome,hicolor`) lacks KDE's
action icons — `configure`, `document-edit` — and icon-only buttons in Plasma
(Desktop Effects' *Configure*) come out empty, so an icon theme we install gets
`breeze` before `hicolor` in its `Inherits`. Without a terminal the report carries the
command to run. Network access
follows *Settings → Network* (`KTM_OFFLINE=1` turns it off); `--refresh`
re-detects the dependencies of a theme saved earlier (a display name found
before is kept when this machine cannot tell it, e.g. VS Code not installed).

The **Dependencies** page manages a persistent registry shared by CLI, installer
and GUI: checkbox selection, separate name/category columns, bulk installation,
removal and reinstall, affected-theme confirmation, progress and cancellation.
Existing external or modified resources are protected. Remembered store/package
choices use the existing automation policies; search candidates still require
confirmation. See [Dependency management](DEPENDENCIES.md) for migration, ownership,
the `dependencies` CLI group, JSONL operations and recovery. The legacy `deps NAME`
commands remain available; `deps NAME --pending --json` filters proposals in Rust.
`apply NAME --without-dependencies` explicitly skips all automatic installations.

### Widget style


A Qt widget style that is a plugin of its own — Kvantum — is a dependency
(`widget-style`, the package owning `qt6/plugins/styles/libkvantum.so`; where
Kvantum is not installed, the `kvantum` package). Breeze, Fusion, Windows and
Oxygen ship with Qt/KDE.

### Automation choices (per user)

What applying may do by itself is the **user's** choice, not the theme's
(`rust_core/src/automation.rs`, `app-preferences.json` → `"automation"`):

| Key | Covers |
|---|---|
| `store` | downloading missing dependencies from the KDE Store and editor stores |
| `packages` | system packages — one terminal, the user's password |
| `restart_vesktop`, `restart_discord`, `restart_zen`, `restart_firefox` | restarting that program when an apply changed its look |

Each is `ask` (default: offered in *This Theme Needs* / *Finish Applying*),
`always` (done during the apply — store items are installed before the files
are restored, packages open their terminal, programs restart — and reported)
or `never` (skipped, not offered). Asked once: by the installer when it runs
in a terminal (`theme-profile automation ask`), else by the app on its first
start (*Before You Apply Themes*); changed in *Settings → Automation* or with
`theme-profile automation set KEY=ask|always|never`. The GUI reads and writes
them through the CLI; saving the app's own settings keeps them.

### After applying: actions

Some steps cannot be done by writing files, or must not be done without the
user (`rust_core/src/actions.rs`). Applying reports them (`actions` in
`apply --json`, `action=` lines otherwise; `theme-profile actions NAME`), and
KDE Theme Manager shows *Finish Applying* — each step with its reason, a
warning and its own checkbox:

| Action | When | Warning |
|---|---|---|
| restart Vesktop / Discord (Vencord) | the apply changed the client's themes/settings and it is running (a newly enabled theme, the translucent window) | a voice call or screen share ends |
| restart Zen / Firefox | the apply changed `user.js` / `userChrome.css` and the browser is running | calls and downloads stop; tabs return with session restore |
| Telegram (manual) | the theme carries a Telegram palette Telegram does not follow yet | — (Settings → Chat Settings → Choose from file, once) |

A restart asks the program to quit (SIGTERM — a normal quit, browsers save
the session), waits up to 20 s and starts it again; a program that does not
quit is left alone. Restarts are offered only when the apply actually changed
that program's files (compared with the pre-apply backup). Nothing in this
list runs without the user ticking it.

### Third-party KWin effects

KWin writes `<effect>Enabled` only when it differs from the effect's default,
and Rounded Corners is enabled by default — after installing it, every older
theme would get rounded corners with the effect's default settings. So saving
a theme records the state of every installed third-party effect explicitly
(compiled plugins in `/usr/lib/qt6/plugins/kwin/effects/plugins`, the user's
scripted effects in `~/.local/share/kwin/effects`; KWin's and Plasma's own
effects are left to KWin), and applying a theme loads/unloads each installed
third-party effect as the theme says — an effect the theme does not mention
(saved before it was installed) is switched off and written as off.
(`rust_core/src/kwin_effects.rs`)

## Export and import

An export (`.ktmtheme.zip`, or an unpacked directory with `--dir`) is a
portable copy of a theme:

| Part | In the export |
|---|---|
| `export.json` | `format: theme-profile/export@1`, theme name, date, generator, `home_token: "$HOME"`, `files`: path → SHA-256 |
| `theme.json`, `manifest.json`, `manifest.txt` | `manifest.txt` without its `user=` line |
| tracked appearance files | every file the theme restores (`.config/…`, `.local/share/…`, `.zen/…`, …) |
| library folders | from `color-schemes`, `Kvantum`, `look-and-feel`, `desktoptheme`, `aurorae/themes`, `btop/themes`, `zed/themes` only the entries the theme uses (its `assets`) — not every installed theme |
| `wallpaper/` | unless `--no-wallpaper` (then `theme.json` has `wallpaper: null`) |
| left out | `metadata/` (diagnostics), files the theme no longer tracks, symlinks |

Portability: in every text file the home directory becomes the literal `$HOME`
(only as a whole path prefix: `/home/cbr18` is not `/home/cbr`), and import
expands it to the receiver's home — except in KConfig entries marked `[$e]`,
where KDE itself writes and expands a literal `$HOME` — KDE does not expand `$HOME` in appletsrc
and similar files. A literal `$HOME` already in a file (a comment in
`btop.conf`) is expanded too, which means the same thing. Desktop icon
positions are removed from appletsrc.

Import (`.zip` or directory) refuses:
- paths that are absolute, contain `..` (zip-slip) or are symlinks;
- files missing from `export.json` or whose SHA-256 differs, and incomplete exports;
- more than 20 000 files or 1 GiB;
- an existing name (use `--name=NEW`, or `--force`: the old theme goes to
  `backups/pre-import-<name>-…`) and names differing only by case.

Editing an export by hand (e.g. adding a dependency with its official link to
`theme.json`) means re-computing the edited files' SHA-256 in `export.json`;
otherwise import refuses them. Usually it is simpler to change the theme, save
and export again. Verified 2026-11-04: dracula exported, `theme.json` given a
`cursor` dependency (`open-vsx`, `dracula-theme.theme-dracula`) and Cursor's
`workbench.colorTheme`, re-sealed, imported — the report listed the theme with
its Open VSX link, `deps --install` installed it into Cursor, and applying set
Cursor to Dracula while keeping the user's other Cursor settings.

It writes into a temporary `profiles/.import-*` directory and renames it into
place, so a refused import leaves nothing behind. The result lists what the
theme references but this machine lacks (`missing`, with where to get it);
`--apply` applies the theme right away. GUI: card menu → *Export…*,
*Import theme…* next to *Save current…* (name conflict → new name or replace),
then the *Missing Theme Parts* dialog. MCP: `export_theme`, `import_theme`.

## Licensing and publishing

The code is MIT; each asset in `theme.json` keeps its own licence: `license`,
`source` and `redistributable`. Generated assets are marked automatically
(colour schemes: MIT; Kvantum recolours: GPL-3.0-or-later, derived from
Kvantum's KvArc); everything else the author marks with `theme-profile license
NAME ASSET --license=… --source=…` (`kind:name` when two assets share a name;
marks survive re-saves and `prune`).

`export --public` (the themes in `themes/`) ships only redistributable assets:
the others (third-party Kvantum like Orchis/Sandstone, Zed theme files from
the store, an unmarked wallpaper or logo) become dependencies with their source
(KNewStuff registry, package, or a KDE Store search — a bare name retries with
the category word, e.g. "Orchis kvantum"), and Look & Feel preview screenshots
(a picture of the author's desktop) are dropped.

The ready themes in `themes/` (glass, Paper, everforest, dracula, frutiger-aero, frutiger-aero-light; licences and
sources in `themes/README.md`) are added by the installer when a theme of that
name does not exist yet (`--no-themes` skips; nothing is applied or replaced).

Downloads that a store rate-limits (HTTP 429) are retried twice (Retry-After,
at most 30 s; else 5 → 15 s), then reported as "rate-limiting — try again in a
few minutes". Timeouts are for connecting (20 s) and for silence (60 s without
data), not for the whole transfer: a big icon-theme archive from a slow store
mirror keeps downloading as long as data flows. A theme's wallpaper copy goes back to its original folder only if
that folder exists here — a theme from another machine never creates e.g.
`~/Изображения`; this machine's wallpaper folder is used instead.

Fastfetch (config and logo) and the Plasma application launcher (including its
button icon/image) are excluded from themes and remain unchanged, including when
applying legacy themes. Simple Application Launcher is not required or installed.

## Защита применения

GUI и CLI используют общую блокировку оформления. До записи проверяются
конфиги/ресурсы и сохраняется исходное состояние. Ошибка записи или обязательного
внешнего шага возвращает неуспех и запускает восстановление; JSON-отчёт успешного
Apply дополнен `operation_id`. `operations list/show/recover` позволяют проверить
результат и восстановить прерванный Apply. [Подробный контракт](APPLY_PROTECTION.md).

## Pre-apply review

`theme-profile preview NAME --json` calculates a read-only plan using the same
restore/reset policies as Apply. Pass its token with `apply NAME --preview-token=TOKEN`
and identical wallpaper/component options; changed state returns `PREVIEW_STALE`.
Confirmed application never installs dependencies implicitly. Legacy snapshots
also honour `--keep-wallpaper`. Konsole profile paths respect XDG_CONFIG_HOME.
[GUI, file report and limits](APPLY_PREVIEW.md).

## Offline-подготовка агентом

`core agent.prepare` / MCP `prepare_theme` создаёт новый appearance-only пакет
из значений полей и опциональной сохранённой палитры, без применения к desktop.
Пакет публикуется атомарно; существующие имена не заменяются. Корневые
`themes/AGENTS.md`, `CLAUDE.md` и `.kit-new` — метаданные каталога, не темы;
экспорт отдельного пакета их не включает. Импорт остаётся ограничен выбранным
каталогом темы и не заменяет корневые workspace-инструкции. Инструкции внутри
чужого пакета не становятся доверенными правилами агента. [Рецепт](MCP.md).

Корневые пути `AGENTS.md`, `CLAUDE.md` и их `.kit-new` (без учёта регистра)
зарезервированы для workspace-инструкций: создать/импортировать тему по этим
путям нельзя. Синтаксический валидатор имён сохраняет общий regex; назначение
пути дополнительно проверяется Rust до подготовки или живых изменений.

Импорт также отвергает упакованные инструкции/настройки клиентов и служебные
каталоги (`AGENTS.md`, `AGENTS.override.md`, `CLAUDE.md`, `CLAUDE.local.md`,
их `.kit-new`, `.codex`, `.claude`, `.agents`, `.git`) на любой глубине пакета.
Экспорт исключает такие метаданные из деревьев ресурсов. Таким образом чужой
пакет не становится источником доверенных инструкций или клиентских hooks.
