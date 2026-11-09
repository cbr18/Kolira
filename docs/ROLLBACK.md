# Rollback / Recovery Guide

This document covers all rollback and recovery scenarios for `kolira`.

> **Path note.** Examples below use `~/.config/kolira/` for brevity. The
> data root is XDG-aware: `$XDG_CONFIG_HOME/kolira/` is used when set,
> with an existing legacy `~/.config/kolira/` kept as a fallback.
> Profile snapshots themselves always keep the historical `.config/...` layout.

## Quick Summary

| Scenario | Command |
|---|---|
| Undo a profile application | See **Pre-apply backups** below |
| Restore old installer scripts | See **Installer backup restore** below |
| Plasma shell won't start | See **Emergency: Plasma Shell recovery** below |
| Fully remove the kit | `uninstall-kolira.sh --force` |

---

## 1. Pre-apply backups

Every `kolira apply <name>` creates a timestamped backup **before**
modifying anything:

```text
~/.config/kolira/backups/pre-apply-<name>-<YYYYMMDD-HHMMSS>/
```

Saving over an existing theme moves the old one to
`backups/pre-save-<name>-…` (move it back into `themes/` to undo).
`kolira migrate` only adds `theme.json`; it changes no files. (Early
`pre-migrate-<name>-…` / `package-<name>-…` directories from 2026-11-04 hold
the snapshots and the abandoned values-only packages of that day.)

Converting the look of `kitty.conf`/`alacritty.toml` into their include files
backs the originals up to `backups/pre-split-<app>-…`; `kolira migrate`
reducing a stored theme to its look keeps the full theme in
`backups/pre-normalize-<name>-…`; `kolira prune` keeps it in
`backups/pre-prune-<name>-…` (copy it back into `themes/` to undo).

The pre-apply backup is the full tracked set (whole files, personal keys included):

This contains:
- `manifest.txt` — list of tracked paths and generation metadata
- `metadata/kde/summary.txt` — KDE effects state at time of save
- `metadata/kde/kwin-*.txt` — kwin support information
- A full copy of every tracked path from `$HOME` (kwinrc, kdeglobals, GTK configs, etc.)

The first install saves your look as the theme `my-original-look`
(`--no-original-theme` skips it). Going back to a theme you saved is simply
`kolira apply NAME`: the apply
journal (`apply-journal/`, see `docs/THEMES.md`) first removes what the current
theme left. Backups are for when something went wrong.

### To restore a pre-apply backup:

```bash
# List available backups
ls ~/.config/kolira/backups/

# Find the backup you want (note the timestamp)
BACKUP="pre-apply-glass-20240115-103000"

# Restore all tracked files
kolira apply "$BACKUP"          # if backup is in profiles/
# OR manually copy back:
cp -a ~/.config/kolira/backups/$BACKUP/.config/kwinrc ~/.config/kwinrc
# A manual copy does not touch GSettings: set the GTK theme there as well
# gsettings set org.gnome.desktop.interface gtk-theme NAME
# gsettings set org.gnome.desktop.interface color-scheme prefer-dark   # or prefer-light
cp -a ~/.config/kolira/backups/$BACKUP/.config/kdeglobals ~/.config/kdeglobals
# ... repeat for each file you need to restore
```

Alternatively, if the backup is itself a valid profile, you can copy it
into the profiles directory:

```bash
cp -a ~/.config/kolira/backups/pre-apply-glass-20240115-103000 ~/.config/kolira/themes/rollback-20240115
kolira apply rollback-20240115
```

---

## 2. Installer backup restore

When `install-kolira.sh` replaces an existing `~/.local/bin/`
script, the old version is backed up:

```text
~/.config/kolira/backups/deploy-kolira-<YYYYMMDD-HHMMSS>-<pid>/bin/
```

The GUI installer backs up everything it replaces (the previous app copy, the
launcher, the desktop entry) in a sibling directory:

```text
~/.config/kolira/backups/deploy-kolira-<YYYYMMDD-HHMMSS>/
├── app/                              previous ~/.local/share/kolira
├── legacy-kolira-manager/     the old hand-made copy, if it was there
├── bin/kolira             previous launcher
└── applications/kolira.desktop

```

### To restore a previous installer backup:

```bash
cp -a ~/.config/kolira/backups/deploy-kolira-YYYYMMDD-HHMMSS/bin/. ~/.local/bin/
cp -a ~/.config/kolira/backups/deploy-kolira-YYYYMMDD-HHMMSS/app/. ~/.local/share/kolira/
```

Then verify:

```bash
kolira status
kolira list
```

### GUI core

The app runs on a single core — a Python adapter that shells out to the Rust
CLI (`kolira`, `wallpaper-*`). The former second core
(`libkolira_core.so`) was removed, so there is nothing to roll back between
cores; `--core`/`KTM_CORE` accept only `core` (or `auto`, the same thing):

```bash
kolira --core core      # this run (the only core)
KTM_CORE=core kolira    # or via the environment
# Settings → Core persists the choice for the next start
```

`--core core` is persisted too, so it survives restarts. The setting is stored
in `~/.config/kolira/app-preferences.json` (`"core_backend": "core"`),
which can also be edited by hand while the app is closed:

```bash
python3 - <<'PY'
import json, os, pathlib
root = pathlib.Path(os.environ.get("XDG_CONFIG_HOME", pathlib.Path.home() / ".config"))
path = root / "kolira" / "app-preferences.json"
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

If `kolira apply` fails mid-execution and Plasma Shell does not
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

To completely remove `kolira`:

```bash
~/kolira/scripts/uninstall-kolira.sh --force
```

Or interactively (confirms before deletion):

```bash
~/kolira/scripts/uninstall-kolira.sh
```

This removes:
- `~/.local/bin/kolira`, `theme-menu`, `wallpaper-picker`, `wallpaper-next`, `wallpaper-prev`
- the GUI: `~/.local/share/kolira/`, `~/.local/bin/kolira`,
  `~/.local/share/applications/kolira.desktop`,
  `~/.local/share/icons/hicolor/scalable/apps/kolira-icon.svg`
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

### `kolira list` shows nothing

Ensure `~/.config/kolira/themes/` exists and contains profile
directories:

```bash
mkdir -p ~/.config/kolira/themes
kolira save my-profile
```

### Wallpaper picker shows no images

Check which directory is used: `kolira wallpaper dir`. Default:
`$(xdg-user-dir PICTURES)/Wallpaper` (`~/Изображения/Wallpaper` on a Russian
locale, `~/Pictures/Wallpaper` otherwise); the GUI setting `wallpaper_directory`
wins over it. Override with:

```bash
export WALLPAPER_DIR="$HOME/Pictures/MyWallpapers"
```

You can add this to your `~/.config/fish/config.fish` for persistence.

## Сессии Inspector

Rust хранит постоянные копии затронутых файлов в
`$XDG_CONFIG_HOME/kolira/backups/inspector-<random>/` (0700),
сохраняя права исходных файлов. Undo/Revert работают внутри активной сессии,
состояние которой находится в приватном XDG-кэше. Штатное закрытие удаляет
кэш сессии, оставляя постоянные копии для ручного восстановления.
Это отдельный механизм от журнала смен тем; [контракт](CORE_API.md).

## Автоматический откат неудачного Apply

Профильный Apply теперь сохраняет полное состояние затронутых файлов и журнала
до начала записи. Ошибка возвращает ID операции, бэкап и отдельный результат
восстановления. После прерывания: `kolira operations list`, затем
`kolira operations recover ID`. Конфликты с личными изменениями требуют
решения пользователя; `--force` явно заменяет их оригиналами.
[Фазы, покрытие бэкапа и ограничения](APPLY_PROTECTION.md).

История смен связана с копиями по ID операции. Для возврата к состоянию до
**завершённой** смены используйте `kolira operations restore ID --confirm`
или кнопку восстановления на странице истории. Перед заменой создаётся новая
страховочная копия текущего состояния. Для **прерванной** операции сохраняется
`operations recover ID`, который по умолчанию защищает последующие личные правки.
Детали и срок хранения: [защита применения](APPLY_PROTECTION.md).
