"""QML runtime helpers shared by the main app and the Quick Switch overlay."""

from __future__ import annotations

from pathlib import Path

# PySide6 bundles its own Qt; the KDE QML modules (Kirigami, layer-shell)
# belong to the system Qt and must be added explicitly.
SYSTEM_QML_PATHS = (
    "/usr/lib/qt6/qml",
    "/usr/lib64/qt6/qml",
    "/usr/share/qt6/qml",
    "/usr/lib/qt5/qml",
    "/usr/lib64/qt5/qml",
    "/usr/share/qt5/qml",
)


def find_system_qml_paths() -> list[str]:
    """System QML import directories that exist on this machine."""
    return [path for path in SYSTEM_QML_PATHS if Path(path).is_dir()]


def add_system_import_paths(engine) -> None:
    """Register the system QML import paths on a QQmlEngine."""
    for path in find_system_qml_paths():
        engine.addImportPath(path)