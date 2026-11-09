#!/usr/bin/env bash
# Wrapper script for kolira (PySide6 Qt/QML app).
# Sets up the environment needed for the system QML modules (Kirigami) to be
# found by PySide6's bundled Qt6, then launches the app from its own directory
# so it works regardless of the caller's current working directory.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# System Qt6 QML modules (Kirigami, org.kde.desktop) — needed because
# PySide6 bundles its own Qt that doesn't see system QML paths.
export QML2_IMPORT_PATH="/usr/lib/qt6/qml:/usr/lib64/qt6/qml:${QML2_IMPORT_PATH:-}"
export QT_PLUGIN_PATH="/usr/lib/qt6/plugins:/usr/lib64/qt6/plugins:${QT_PLUGIN_PATH:-}"

# Ensure the KDE platform theme is used (icons, colors, etc.).
export QT_QPA_PLATFORMTHEME="${QT_QPA_PLATFORMTHEME:-KDE}"

# Prefer the project venv when present.
if [ -x "$SCRIPT_DIR/.venv/bin/python" ]; then
    exec "$SCRIPT_DIR/.venv/bin/python" -m src "$@"
fi

exec python3 -m src "$@"
