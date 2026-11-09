#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }

# --help / -h
if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  cat <<'EOF'
Usage: update-theme-profile-kit.sh [OPTIONS]

Update theme-profile-kit from git, then reinstall the scripts and the GUI.

Options:
  --restart     Restart a running KDE Theme Manager on the new version.
  -h, --help    Show this help message and exit.
Other options go to install-theme-profile-kit.sh as they are.
EOF
  exit 0
fi

say "Обновление theme-profile-kit из git"
if [ -d "$PROJECT_DIR/.git" ]; then
  cd "$PROJECT_DIR"

  # Warn about uncommitted local changes
  if ! git diff --quiet --exit-code; then
    say "Warning: есть незакоммиченные локальные изменения."
  fi

  # Fetch latest and pull (fast-forward only to avoid merge commits)
  say "Fetching latest changes..."
  if ! git fetch; then
    say "Error: git fetch failed (network issue?). Aborting." >&2
    exit 1
  fi

  say "Pulling updates (--ff-only)..."
  if ! git pull --ff-only; then
    say "Error: git pull failed (merge conflict or fast-forward impossible). Aborting." >&2
    say "Resolve the conflict manually, then re-run this script." >&2
    exit 1
  fi

  say "Запуск установки"
  exec "$SCRIPT_DIR/install-theme-profile-kit.sh" "$@"
else
  say "Не git-репозиторий. Просто переустановка."
  exec "$SCRIPT_DIR/install-theme-profile-kit.sh" "$@"
fi