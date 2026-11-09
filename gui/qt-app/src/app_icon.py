"""Application icon resolution.

The installer ships an icon named :data:`ICON_NAME` (see
``resources/icons/kolira-icon.svg`` and the ``Icon=`` key in
``resources/kolira.desktop``). When the app runs from the source
tree that icon is not installed yet, so fall back to the Breeze theme icon.

The name deliberately does **not** start with ``kde-`` or ``plasma-``: KDE's
icon loader collapses ``<existing-icon>-<anything>`` to the existing icon, so
``kde-theme-manager`` silently resolved to the KDE logo (and
``plasma-theme-manager`` to the Plasma logo) instead of our own icon.
"""

from __future__ import annotations

from PySide6.QtGui import QIcon

ICON_NAME = "kolira-icon"
FALLBACK_ICON_NAME = "preferences-desktop-theme"


def app_icon() -> QIcon:
    """Window/tray/notification icon: ours if installed, Breeze icon otherwise."""
    return QIcon.fromTheme(ICON_NAME, QIcon.fromTheme(FALLBACK_ICON_NAME))
