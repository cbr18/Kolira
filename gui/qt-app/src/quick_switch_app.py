"""Entry point of the Kolira Quick Switch overlay.

A separate process on purpose: the main GUI owns a single-instance socket,
the tray and the Inspector session, and the overlay must appear without
starting any of that.

Usage:
    kolira-quick-switch              # the overlay
    kolira-quick-switch --preview    # same UI, apply is disabled
"""

from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtCore import QTimer, QUrl
from PySide6.QtGui import QCursor, QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine, QQmlComponent

from qml_runtime import add_system_import_paths
from quick_switch_controller import QuickSwitchController

QML_DIR = Path(__file__).resolve().parent.parent / "qml"
HOST_SURFACE = QML_DIR / "quick-switch" / "HostSurface.qml"

# Absolute safety net: the overlay never outlives the transaction it shows.
HARD_STOP_MS = 60_000
WATCHDOG_MS = 45_000


def _primary_screen_index(app: QGuiApplication) -> int:
    """The monitor under the cursor, falling back to the primary screen."""
    screens = app.screens()
    hovered = app.screenAt(QCursor.pos())
    if hovered is not None and hovered in screens:
        return screens.index(hovered)
    primary = app.primaryScreen()
    if primary is not None and primary in screens:
        return screens.index(primary)
    return 0


def main(argv: list[str] | None = None) -> int:
    arguments = list(sys.argv if argv is None else argv)
    preview_only = "--preview" in arguments[1:]
    arguments = [
        argument
        for argument in arguments
        if argument not in ("--preview", "--quick-switch")
    ]

    app = QGuiApplication(arguments)
    app.setApplicationName("kolira-quick-switch")
    app.setOrganizationName("cbr")

    engine = QQmlApplicationEngine()
    add_system_import_paths(engine)

    # Parent the controller to the engine: it must outlive the QML bindings,
    # otherwise interpreter shutdown destroys it first and every binding
    # re-evaluation logs "Cannot read property ... of null".
    controller = QuickSwitchController(engine, preview_only=preview_only)
    engine.rootContext().setContextProperty("quickSwitch", controller)

    component = QQmlComponent(engine, QUrl.fromLocalFile(str(HOST_SURFACE)))
    if component.isError():
        for error in component.errors():
            print(f"Quick Switch: {error.toString()}", file=sys.stderr)
        return 1

    screens = app.screens()
    primary_index = _primary_screen_index(app)
    surfaces = []
    for index, screen in enumerate(screens):
        surface = component.createWithInitialProperties(
            {
                "screenIndex": index,
                "primary": index == primary_index,
                "interactive": index == primary_index,
            }
        )
        if surface is None:
            print(
                f"Quick Switch: cannot create a surface on {screen.name()}",
                file=sys.stderr,
            )
            return 1
        surfaces.append(surface)

    controller.initialize()
    controller.closed.connect(app.quit)

    # If the user is stuck, the overlay says so and Esc closes it.
    QTimer.singleShot(WATCHDOG_MS, controller.watchdog)
    QTimer.singleShot(HARD_STOP_MS, app.quit)

    return app.exec()


if __name__ == "__main__":
    sys.exit(main())