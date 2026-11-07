# Rollback / Recovery Guide

This document covers all rollback and recovery scenarios for `theme-profile-kit`.

> **Path note.** Examples below use `~/.config/theme-profile/` for brevity. The
> data root is XDG-aware: `$XDG_CONFIG_HOME/theme-profile/` is used when set,
> with an existing legacy `~/.config/theme-profile/` kept as a fallback.
> Profile snapshots themselves always keep the historical `.config/...` layout.

## Quick Summary

| Scenario | Command |
|---|---|
| Undo a profile application | See **Pre-apply backups** below |
| Restore old installer scripts | See **Installer backup restore** below |
| Plasma shell won't start | See **Emergency: Plasma Shell recovery** below |
| Fully remove the kit | `uninstall-theme-profile-kit.sh --force` |

---

## 1. Pre-apply backups

Every `theme-profile apply <name>` creates a timestamped backup **before**
modifying anything:

```text
~/.config/theme-profile/backups/pre-apply-<name>-<YYYYMMDD-HHMMSS>/
```

Saving over an existing theme moves the old one to
`backups/pre-save-<name>-…` (move it back into `profiles/` to undo).
`theme-profile migrate` only adds `theme.json`; it changes no files. (Early
`pre-migrate-<name>-…` / `package-<name>-…` directories from 2026-11-04 hold
the snapshots and the abandoned values-only packages of that day.)

Converting the look of `kitty.conf`/`alacritty.toml` into their include files
backs the originals up to `backups/pre-split-<app>-…`; `theme-profile migrate`
reducing a stored theme to its look keeps the full theme in
`backups/pre-normalize-<name>-…`; `theme-profile prune` keeps it in
`backups/pre-prune-<name>-…` (copy it back into `profiles/` to undo).

The pre-apply backup is the full tracked set (whole files, personal keys included):

This contains:
- `manifest.txt` — list of tracked paths and generation metadata
- `metadata/kde/summary.txt` — KDE effects state at time of save
- `metadata/kde/kwin-*.txt` — kwin support information
- A full copy of every tracked path from `$HOME` (kwinrc, kdeglobals, GTK configs, etc.)

The first install saves your look as the theme `my-original-look`
(`--no-original-theme` skips it). Going back to a theme you saved is simply
`theme-profile apply NAME`: the apply
journal (`apply-journal/`, see `docs/THEMES.md`) first removes what the current
theme left. Backups are for when something went wrong.

### To restore a pre-apply backup:

```bash
# List available backups
ls ~/.config/theme-profile/backups/

# Find the backup you want (note the timestamp)
BACKUP="pre-apply-glass-20240115-103000"

# Restore all tracked files
theme-profile apply "$BACKUP"          # if backup is in profiles/
# OR manually copy back:
cp -a ~/.config/theme-profile/backups/$BACKUP/.config/kwinrc ~/.config/kwinrc
# A manual copy does not touch GSettings: set the GTK theme there as well
# gsettings set org.gnome.desktop.interface gtk-theme NAME
# gsettings set org.gnome.desktop.interface color-scheme prefer-dark   # or prefer-light
cp -a ~/.config/theme-profile/backups/$BACKUP/.config/kdeglobals ~/.config/kdeglobals
# ... repeat for each file you need to restore
```

Alternatively, if the backup is itself a valid profile, you can copy it
into the profiles directory:

```bash
cp -a ~/.config/theme-profile/backups/pre-apply-glass-20240115-103000 ~/.config/theme-profile/profiles/rollback-20240115
theme-profile apply rollback-20240115
```

---

## 2. Installer backup restore

When `install-theme-profile-kit.sh` replaces an existing `~/.local/bin/`
script, the old version is backed up:

```text
~/.config/theme-profile/backups/deploy-theme-profile-kit-<YYYYMMDD-HHMMSS>-<pid>/bin/
```

The GUI installer backs up everything it replaces (the previous app copy, the
launcher, the desktop entry) in a sibling directory:

```text
~/.config/theme-profile/backups/deploy-kde-theme-manager-<YYYYMMDD-HHMMSS>/
├── app/                              previous ~/.local/share/kde-theme-manager
├── legacy-theme-profile-manager/     the old hand-made copy, if it was there
├── bin/kde-theme-manager             previous launcher
└── applications/kde-theme-manager.desktop

```

### To restore a previous installer backup:

```bash
cp -a ~/.config/theme-profile/backups/deploy-theme-profile-kit-YYYYMMDD-HHMMSS/bin/. ~/.local/bin/
cp -a ~/.config/theme-profile/backups/deploy-kde-theme-manager-YYYYMMDD-HHMMSS/app/. ~/.local/share/kde-theme-manager/
```

Then verify:

```bash
theme-profile status
theme-profile list
```

### GUI core

The app runs on a single core — a Python adapter that shells out to the Rust
CLI (`theme-profile`, `wallpaper-*`). The former second core
(`libkde_theme_core.so`) was removed, so there is nothing to roll back between
cores; `--core`/`KTM_CORE` accept only `core` (or `auto`, the same thing):

```bash
kde-theme-manager --core core      # this run (the only core)
KTM_CORE=core kde-theme-manager    # or via the environment
# Settings → Core persists the choice for the next start
```

`--core core` is persisted too, so it survives restarts. The setting is stored
in `~/.config/theme-profile/app-preferences.json` (`"core_backend": "core"`),
which can also be edited by hand while the app is closed:

```bash
python3 - <<'PY'
import json, os, pathlib
root = pathlib.Path(os.environ.get("XDG_CONFIG_HOME", pathlib.Path.home() / ".config"))
path = root / "theme-profile" / "app-preferences.json"
data = json.loads(path.read_text())
data["core_backend"] = "core"
path.write_text(json.dumps(data, indent=2) + "\n")
PY
```

All profile mutations (apply/save/diff) run natively in the Rust CLI binary —
the Python core adapter only invokes it and parses the output, so there are no
per-core behavioural differences to worry about.

---

## 3. Emergency: Plasma Shell recovery

If `theme-profile apply` fails mid-execution and Plasma Shell does not
restart automatically, recover manually:

```bash
systemctl --user start plasma-plasmashell.service
systemctl --user reset-failed plasma-plasmashell.service
kquitapp6 plasmashell && kstart6 plasmashell &  # fallback if the above doesn't work
```

The `apply` command now uses `trap 'start_plasma_shell' ERR` to
automatically attempt recovery on any error after stopping Plasma. If
the session is already dead, the recovery trap may not fire — use the
commands above.

---

## 4. Full removal (complete rollback)

To completely remove `theme-profile-kit`:

```bash
~/theme-profile-kit/scripts/uninstall-theme-profile-kit.sh --force
```

Or interactively (confirms before deletion):

```bash
~/theme-profile-kit/scripts/uninstall-theme-profile-kit.sh
```

This removes:
- `~/.local/bin/theme-profile`, `theme-menu`, `wallpaper-picker`, `wallpaper-next`, `wallpaper-prev`
- the GUI: `~/.local/share/kde-theme-manager/`, `~/.local/bin/kde-theme-manager`,
  `~/.local/share/applications/kde-theme-manager.desktop`,
  `~/.local/share/icons/hicolor/scalable/apps/ktm-theme-manager.svg`
  (component-only removal: `--only-cli` or `--only-gui`)

**But does NOT touch** — you must revert these manually if needed:
- `~/.config/wofi/*.css` — wofi styles
- `~/.config/kwinrc`, `~/.config/kdeglobals`, `~/.config/plasmashellrc` — KDE configs
- `~/.config/kitty/kitty.conf`, `~/.config/alacritty/...` — terminal configs
- Zen, VS Code, Cursor, Obsidian, btop, fastfetch configs

---

## 5. Troubleshooting

### Plasma won't start after `apply`

```bash
systemctl --user start plasma-plasmashell.service
```

### `kreadconfig6` / `kwriteconfig6` not found

These come from `plasma-workspace`. Install it:

```bash
sudo pacman -S plasma-workspace   # CachyOS / Arch
```

### wofi menu appears empty

Ensure `~/.config/wofi/style.css` exists. If not:

```bash
mkdir -p ~/.config/wofi
echo '* { background-color: #1e1e1e; }' > ~/.config/wofi/style.css
```

### `theme-profile list` shows nothing

Ensure `~/.config/theme-profile/profiles/` exists and contains profile
directories:

```bash
mkdir -p ~/.config/theme-profile/profiles
theme-profile save my-profile
```

### Wallpaper picker shows no images

Check which directory is used: `theme-profile wallpaper dir`. Default:
`$(xdg-user-dir PICTURES)/Wallpaper` (`~/Изображения/Wallpaper` on a Russian
locale, `~/Pictures/Wallpaper` otherwise); the GUI setting `wallpaper_directory`
wins over it. Override with:

```bash
export WALLPAPER_DIR="$HOME/Pictures/MyWallpapers"
```

You can add this to your `~/.config/fish/config.fish` for persistence.
