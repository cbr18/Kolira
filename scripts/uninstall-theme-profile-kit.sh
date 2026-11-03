#!/usr/bin/env bash
set -euo pipefail

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }

BINDIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATADIR="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIGDIR="${XDG_CONFIG_HOME:-$HOME/.config}"
APPDIR="$DATADIR/kde-theme-manager"
LEGACY_APPDIR="$DATADIR/theme-profile-manager"
APPSDIR="$DATADIR/applications"
ICON="$DATADIR/icons/hicolor/scalable/apps/ktm-theme-manager.svg"
LEGACY_ICON="$DATADIR/icons/hicolor/scalable/apps/kde-theme-manager.svg"
MARKER=".installed-by-theme-profile-kit"
THEMEDIR="$CONFIGDIR/theme-profile"
if [ ! -d "$THEMEDIR" ] && [ -n "${XDG_CONFIG_HOME:-}" ] && [ -d "${HOME}/.config/theme-profile" ]; then
  THEMEDIR="${HOME}/.config/theme-profile"
fi

MODE="all" # all | cli | gui
FORCE=0

usage() {
  cat <<'EOF'
Usage: uninstall-theme-profile-kit.sh [OPTIONS]

Uninstall theme-profile-kit from the home directory.

By default this removes the Rust CLI, the KDE Theme Manager GUI and the
theme-profile data directory (profiles, backups and settings).

Options:
  --only-cli    Remove only the Rust CLI binaries; keep the GUI and all data
  --only-gui    Remove only the KDE Theme Manager GUI; keep the CLI and data
  -f, --force   Skip confirmation prompts
  -h, --help    Show this help message and exit
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
    -f | --force)
      FORCE=1
      ;;
    *)
      printf 'Unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

# The full uninstall removes user data, so ask up front. Component-only modes do
# not touch the data directory and therefore do not need this confirmation.
if [ "$MODE" = "all" ] && [ "$FORCE" -eq 0 ] && [ -e "$THEMEDIR" ]; then
  read -r -p "This will remove all profiles, backups, and metadata in $THEMEDIR. Continue? [y/N] " resp
  case "$resp" in
    [yY]*) ;;
    *) say "Aborted."; exit 0 ;;
  esac
fi

remove_gui() {
  say "Removing GUI launcher"
  rm -f "$BINDIR/kde-theme-manager"

  say "Removing desktop entry and icon"
  rm -f "$APPSDIR/kde-theme-manager.desktop"
  rm -f "$ICON"
  rm -f "$LEGACY_ICON"

  remove_appdir() { # dir label
    [ -e "$1" ] || return 0
    if [ ! -f "$1/$MARKER" ] && [ ! -f "$1/src/__main__.py" ]; then
      say "Skipping $1 (does not look like an app install)"
      return 0
    fi
    say "Removing $2 ($1)"
    rm -rf "$1"
  }

  remove_appdir "$APPDIR" "application"
  remove_appdir "$LEGACY_APPDIR" "obsolete application copy"

  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPSDIR" 2>/dev/null || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$DATADIR/icons/hicolor" >/dev/null 2>&1 || true
  fi
}

remove_cli() {
  say "Removing CLI binaries"
  rm -f "$BINDIR/theme-profile"
  rm -f "$BINDIR/theme-menu"
  rm -f "$BINDIR/wallpaper-picker"
  rm -f "$BINDIR/wallpaper-next"
  rm -f "$BINDIR/wallpaper-prev"
}

case "$MODE" in
  cli)
    remove_cli
    say "GUI and $THEMEDIR were left untouched (--only-cli)."
    ;;
  gui)
    remove_gui
    say "CLI binaries and $THEMEDIR were left untouched (--only-gui)."
    ;;
  *)
    remove_gui
    remove_cli
    if [ -e "$THEMEDIR" ]; then
      say "Removing: $THEMEDIR"
      rm -rf "$THEMEDIR"
    fi
    say "Готово. theme-profile-kit полностью удалён."
    say "Конфиги wofi ($CONFIGDIR/wofi/*.css) и настройки приложений не тронуты."
    ;;
esac
