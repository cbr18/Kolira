#!/usr/bin/env bash
set -euo pipefail

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m==> %s\033[0m\n' "$*" >&2; }

KIT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_DIR="$KIT_DIR/gui/qt-app/src/core/rust_core"
RELEASE_DIR="$RUST_DIR/target/release"
APP_SRC="$KIT_DIR/gui/qt-app"

BINDIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATADIR="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIGDIR="${XDG_CONFIG_HOME:-$HOME/.config}"
APPDIR="$DATADIR/kde-theme-manager"
LEGACY_APPDIR="$DATADIR/theme-profile-manager"
APPSDIR="$DATADIR/applications"
ICONDIR="$DATADIR/icons/hicolor/scalable/apps"
MARKER=".installed-by-theme-profile-kit"
BACKUP_ROOT="$CONFIGDIR/theme-profile/backups"
CLI_BACKUP_DIR="$BACKUP_ROOT/deploy-theme-profile-kit-$(date +%Y%m%d-%H%M%S)-$$"
GUI_BACKUP_DIR="$BACKUP_ROOT/deploy-kde-theme-manager-$(date +%Y%m%d-%H%M%S)"
[ -e "$CLI_BACKUP_DIR" ] && CLI_BACKUP_DIR="$CLI_BACKUP_DIR-$$"
[ -e "$GUI_BACKUP_DIR" ] && GUI_BACKUP_DIR="$GUI_BACKUP_DIR-$$"

MODE="all" # all | cli | gui
VENV_MODE="auto" # auto | yes | no
RESTART="no"     # yes: restart a running KDE Theme Manager after install
THEMES="yes"     # no: do not add the bundled themes (themes/)

usage() {
  cat <<'EOF'
Usage: install-theme-profile-kit.sh [OPTIONS]

Install theme-profile-kit: the Rust CLI (theme-profile, theme-menu,
wallpaper-picker, wallpaper-next, wallpaper-prev) and, by default, the KDE
Theme Manager GUI with its launcher and desktop entry.

Options:
  --only-cli    Install only the Rust CLI, skip the Qt/QML application
  --only-gui    Install only the KDE Theme Manager GUI (the CLI must already
                be installed for the app to work)
  --with-venv   Always build a venv inside the installed app
  --no-venv     Never build a venv — use the system PySide6
  --restart     Restart a running KDE Theme Manager so it picks up the new
                version (default: only warn that the old copy is running)
  --no-themes   Do not add the bundled themes from themes/ (by default each
                one is imported unless a theme with that name already exists;
                existing themes are never changed, nothing is applied)
  -h, --help    Show this help message and exit.

Binaries installed to ${XDG_BIN_HOME:-~/.local/bin}:
  theme-profile, theme-menu, wallpaper-picker, wallpaper-next, wallpaper-prev
(built with `cargo build --release` in gui/qt-app/src/core/rust_core)

GUI installed to:
  ${XDG_DATA_HOME:-~/.local/share}/kde-theme-manager/
  ~/.local/bin/kde-theme-manager
  ${XDG_DATA_HOME:-~/.local/share}/applications/kde-theme-manager.desktop
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --only-cli)
      MODE="cli"
      ;;
    --only-gui)
      MODE="gui"
      ;;
    --no-gui)
      # Backwards-compatible alias for --only-cli (kept for old scripts).
      MODE="cli"
      ;;
    --with-venv)
      VENV_MODE="yes"
      ;;
    --no-venv)
      VENV_MODE="no"
      ;;
    --restart)
      RESTART="yes"
      ;;
    --no-themes)
      THEMES="no"
      ;;
    *)
      say "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

CLI_SCRIPTS=(
  "theme-profile"
  "theme-menu"
  "wallpaper-picker"
  "wallpaper-next"
  "wallpaper-prev"
)

# PIDs of KDE Theme Manager processes running from the installed copy: the
# launcher cd's into $APPDIR and execs `python -m src`.
running_gui_pids() {
  local proc pid cwd cmdline app_real
  app_real="$(readlink -f "$APPDIR" 2>/dev/null || printf '%s' "$APPDIR")"
  for proc in /proc/[0-9]*; do
    pid="${proc#/proc/}"
    [ "$pid" = "$$" ] && continue
    cwd="$(readlink "$proc/cwd" 2>/dev/null)" || continue
    [ "$cwd" = "$app_real" ] || continue
    cmdline="$(tr '\0' ' ' <"$proc/cmdline" 2>/dev/null)" || continue
    case "$cmdline" in
      *" -m src"*) printf '%s\n' "$pid" ;;
    esac
  done
}

# A running copy keeps executing the old code until it is restarted.
handle_running_gui() {
  local pids
  pids="$(running_gui_pids | tr '\n' ' ')"
  pids="${pids% }"
  [ -n "$pids" ] || return 0

  if [ "$RESTART" != "yes" ]; then
    warn "KDE Theme Manager запущен (PID $pids) и работает на старой версии."
    warn "Перезапустите приложение или запустите установщик с --restart."
    return 0
  fi

  say "Перезапуск KDE Theme Manager (PID $pids)"
  # shellcheck disable=SC2086 # word splitting of the PID list is intended
  kill -TERM $pids 2>/dev/null || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    [ -z "$(running_gui_pids)" ] && break
    sleep 0.5
  done
  if [ -n "$(running_gui_pids)" ]; then
    warn "Старая копия не завершилась — перезапустите её вручную."
    return 0
  fi
  if [ -z "${KTM_INSTALL_NO_LAUNCH:-}" ] && command -v setsid >/dev/null 2>&1; then
    setsid -f "$BINDIR/kde-theme-manager" >/dev/null 2>&1 </dev/null || true
    say "Запущена новая версия."
  else
    say "Старая копия завершена; запустите kde-theme-manager."
  fi
}

install_cli() {
  say "Installing theme-profile-kit CLI (Rust)"
  mkdir -p "$BINDIR" "$CLI_BACKUP_DIR/bin"

  if ! command -v cargo >/dev/null 2>&1; then
    say "Error: cargo not found — cannot build the Rust CLI (theme-profile etc.)." >&2
    exit 1
  fi

  say "Building the Rust CLI (cargo build --release)"
  ( cd "$RUST_DIR" && cargo build --release --bins )
  for script in "${CLI_SCRIPTS[@]}"; do
    if [ ! -x "$RELEASE_DIR/$script" ]; then
      say "Missing built binary: $RELEASE_DIR/$script" >&2
      exit 1
    fi
  done

  say "Backing up existing binaries..."
  for script in "${CLI_SCRIPTS[@]}"; do
    if [ -e "$BINDIR/$script" ]; then
      cp -a "$BINDIR/$script" "$CLI_BACKUP_DIR/bin/$script"
    fi
  done

  for script in "${CLI_SCRIPTS[@]}"; do
    say "Installing $script"
    install -m 0755 "$RELEASE_DIR/$script" "$BINDIR/$script"
  done

  say "Installed theme-profile-kit CLI to $BINDIR"
  say "Backup: $CLI_BACKUP_DIR"
}

# The ready themes shipped in themes/ (exports made with `export --public`).
# Only names the user does not have yet; never applied, never overwritten.
# What applying themes may do by itself (store downloads, system packages,
# restarting programs) is the user's choice: ask it once, on a terminal, if
# not answered yet (the GUI asks on its first start otherwise).
ask_automation() {
  local cli="$BINDIR/theme-profile"
  [ -x "$cli" ] || return 0
  [ -t 0 ] && [ -t 1 ] || return 0
  "$cli" automation --json 2>/dev/null | grep -q '"configured":false' || return 0
  say "Автоматизация тем"
  "$cli" automation ask || warn "Не сохранено — приложение спросит при первом запуске."
}

install_bundled_themes() {
  [ "$THEMES" = "yes" ] || return 0
  local cli="$BINDIR/theme-profile" dir name added=0
  [ -x "$cli" ] || return 0
  [ -d "$KIT_DIR/themes" ] || return 0
  for dir in "$KIT_DIR/themes"/*/; do
    [ -f "$dir/export.json" ] || continue
    name="$(basename "$dir")"
    if "$cli" list 2>/dev/null | grep -qixF -- "$name"; then
      continue
    fi
    if "$cli" import "$dir" >/dev/null 2>&1; then
      say "Added theme: $name"
      added=$((added + 1))
    else
      warn "Could not add theme $name (run: theme-profile import $dir)"
    fi
  done
  [ "$added" -gt 0 ] && say "Ready themes added: $added (apply one in KDE Theme Manager or with: theme-profile apply NAME)"
  return 0
}

install_gui() {
  for required in "$APP_SRC/src/__main__.py" "$APP_SRC/theme-profile-manager.sh" \
    "$APP_SRC/resources/kde-theme-manager.desktop" \
    "$APP_SRC/resources/icons/ktm-theme-manager.svg"; do
    if [ ! -f "$required" ]; then
      warn "Missing source: $required"
      exit 1
    fi
  done

  backup() { # path label [prune-dev]
    [ -e "$1" ] || return 0
    mkdir -p "$(dirname "$GUI_BACKUP_DIR/$2")"
    rm -rf "${GUI_BACKUP_DIR:?}/$2"
    cp -a "$1" "$GUI_BACKUP_DIR/$2"

    if [ "${3:-}" = "prune-dev" ]; then
      local pruned=""
      for dev in ".venv" "src/core/rust_core/target"; do
        if [ -e "$GUI_BACKUP_DIR/$2/$dev" ]; then
          rm -rf "${GUI_BACKUP_DIR:?}/$2/$dev"
          pruned="$pruned $dev"
        fi
      done
      if [ -n "$pruned" ]; then
        say "Backup: $1 → $GUI_BACKUP_DIR/$2 (pruned:$pruned)"
        return 0
      fi
    fi

    say "Backup: $1 → $GUI_BACKUP_DIR/$2"
  }

  say "Installing KDE Theme Manager"
  mkdir -p "$BINDIR" "$APPSDIR" "$ICONDIR"

  backup "$APPDIR" "app" prune-dev
  backup "$LEGACY_APPDIR" "legacy-theme-profile-manager" prune-dev
  backup "$BINDIR/kde-theme-manager" "bin/kde-theme-manager"
  backup "$APPSDIR/kde-theme-manager.desktop" "applications/kde-theme-manager.desktop"

  say "Copying application → $APPDIR"
  mkdir -p "$APPDIR"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete \
      --exclude '.venv/' --exclude '__pycache__/' --exclude '*.egg-info/' \
      --exclude 'target/' --exclude 'tests/' \
      "$APP_SRC/" "$APPDIR/"
  else
    find "$APPDIR" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
    tar -C "$APP_SRC" \
      --exclude='.venv' --exclude='__pycache__' --exclude='*.egg-info' \
      --exclude='target' --exclude='tests' -cf - . |
      tar -C "$APPDIR" -xf -
  fi
  printf '%s\n' "installed by $KIT_DIR/scripts/install-theme-profile-kit.sh" > "$APPDIR/$MARKER"

  py_has_pyside() {
    command -v python3 >/dev/null 2>&1 &&
      python3 -c 'import PySide6' >/dev/null 2>&1
  }

  if [ "$VENV_MODE" = "no" ]; then
    say "Venv skipped (--no-venv)"
    if ! py_has_pyside; then
      warn "Warning: the system Python cannot import PySide6 either — run with --with-venv."
    fi
  elif [ "$VENV_MODE" = "auto" ] && py_has_pyside; then
    say "System PySide6 found ($(python3 -c 'import PySide6; print(PySide6.__version__)' 2>/dev/null)) — no venv needed"
  else
    say "Creating virtual environment in $APPDIR/.venv"
    if command -v uv >/dev/null 2>&1; then
      uv venv "$APPDIR/.venv"
      uv pip install --python "$APPDIR/.venv/bin/python" "PySide6>=6.7"
    else
      python3 -m venv "$APPDIR/.venv"
      "$APPDIR/.venv/bin/python" -m pip install --upgrade pip
      "$APPDIR/.venv/bin/python" -m pip install "PySide6>=6.7"
    fi
  fi

  say "Writing launcher → $BINDIR/kde-theme-manager"
  cat > "$BINDIR/kde-theme-manager" <<'LAUNCHER'
#!/usr/bin/env bash
# kde-theme-manager — launcher written by theme-profile-kit.
set -euo pipefail

APP_DIR="${KDE_THEME_MANAGER_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/kde-theme-manager}"
APP_SH="$APP_DIR/theme-profile-manager.sh"

if [ ! -x "$APP_SH" ]; then
  printf 'kde-theme-manager: not installed (%s not found)\n' "$APP_SH" >&2
  printf 'Install it with: %s/scripts/install-theme-profile-kit.sh\n' \
    "${THEME_PROFILE_KIT:-@KIT_DIR@}" >&2
  exit 127
fi

exec "$APP_SH" "$@"
LAUNCHER
  KIT_DIR_SED="${KIT_DIR//\\/\\\\}"
  KIT_DIR_SED="${KIT_DIR_SED//&/\\&}"
  KIT_DIR_SED="${KIT_DIR_SED//|/\\|}"
  sed -i "s|@KIT_DIR@|$KIT_DIR_SED|g" "$BINDIR/kde-theme-manager"
  chmod 755 "$BINDIR/kde-theme-manager" "$APPDIR/theme-profile-manager.sh"

  say "Installing desktop entry and icon"
  install -m 644 "$APP_SRC/resources/kde-theme-manager.desktop" \
    "$APPSDIR/kde-theme-manager.desktop"
  install -m 644 "$APP_SRC/resources/icons/ktm-theme-manager.svg" \
    "$ICONDIR/ktm-theme-manager.svg"
  rm -f "$ICONDIR/kde-theme-manager.svg"

  if [ -e "$LEGACY_APPDIR" ]; then
    say "Removing obsolete copy $LEGACY_APPDIR (backed up above)"
    rm -rf "$LEGACY_APPDIR"
  fi

  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPSDIR" 2>/dev/null || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$DATADIR/icons/hicolor" >/dev/null 2>&1 || true
  fi
  for ksycoca in kbuildsycoca6 kbuildsycoca5; do
    if command -v "$ksycoca" >/dev/null 2>&1; then
      "$ksycoca" >/dev/null 2>&1 || true
    fi
  done

  say "Done."
  say "Запуск: kde-theme-manager  (или ярлык «KDE Theme Manager» в меню)"
  say "Данные профилей остались в $CONFIGDIR/theme-profile/"
  if [ -d "$GUI_BACKUP_DIR" ]; then
    say "Бэкап заменённых GUI-файлов: $GUI_BACKUP_DIR"
  fi
  handle_running_gui
}

case "$MODE" in
  cli)
    install_cli
    install_bundled_themes
    ask_automation
    say "Skipped the GUI (--only-cli)."
    ;;
  gui)
    install_gui
    say "Skipped the CLI (--only-gui)."
    ;;
  *)
    install_cli
    install_bundled_themes
    ask_automation
    install_gui
    ;;
esac
