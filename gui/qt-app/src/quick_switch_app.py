"""Entry point of the Kolira Quick Switch overlay.

A separate process on purpose: the main GUI owns a single-instance socket,
the tray and the Inspector session, and the overlay must appear without
starting any of that.

Usage:
    kolira-quick-switch              # the overlay
    kolira-quick-switch --preview    # same UI, apply is disabled
"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

from PySide6.QtCore import QTimer, QUrl
from PySide6.QtGui import QCursor, QGuiApplication, QSurfaceFormat
from PySide6.QtNetwork import QLocalServer, QLocalSocket
from PySide6.QtQml import QQmlApplicationEngine, QQmlComponent

from qml_runtime import add_system_import_paths
from quick_switch_controller import QuickSwitchController, appearance_is_busy

QML_DIR = Path(__file__).resolve().parent.parent / "qml"
HOST_SURFACE = QML_DIR / "quick-switch" / "HostSurface.qml"

# The hotkey launches the desktop entry again, so a second process must ask
# the running overlay to close instead of opening another window.
SINGLE_INSTANCE_SOCKET = "kolira-quick-switch"

# Absolute safety net: the overlay never outlives the transaction it shows.
HARD_STOP_MS = 60_000
WATCHDOG_MS = 45_000


def _ask_running_instance_to_close(name: str) -> bool:
    """Ask a live overlay to close (toggle); False when nobody answers."""
    sock = QLocalSocket()
    sock.connectToServer(name)
    if not sock.waitForConnected(400):
        # Nobody is listening: do not touch an unconnected socket.
        return False
    sock.write(b"toggle")
    sock.flush()
    sock.disconnectFromServer()
    return True


def _acquire_single_instance(name: str, server: QLocalServer) -> bool:
    """Own the socket (True) or hand over to the running overlay (False)."""
    if _ask_running_instance_to_close(name):
        return False
    QLocalServer.removeServer(name)  # stale socket from a crashed run
    if server.listen(name):
        return True
    # Lost a race only if someone answers now; otherwise the socket is broken
    # (e.g. a long $TMPDIR) and we simply run without the lock.
    return not _ask_running_instance_to_close(name)


def _notify_busy() -> None:
    """Tell the user why the overlay did not open.

    The hotkey launch is silent otherwise (its stderr goes to the journal), so
    a passive desktop notification is the only visible feedback. Fire-and-forget
    and best-effort: a missing notification daemon must not matter.
    """
    message = "Kolira: тема ещё применяется, переключатель откроется после завершения"
    notify = shutil.which("notify-send")
    if notify:
        try:
            subprocess.run(
                [notify, "--app-name=Kolira", "Kolira", message],
                capture_output=True,
                timeout=5,
                check=False,
            )
        except (OSError, subprocess.SubprocessError):
            pass
    print(f"kolira-quick-switch: {message}", file=sys.stderr, flush=True)


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

    # Multisample the overlay surface: the card outlines (rounded corners,
    # parallelogram edges) are drawn straight to the window, and without MSAA
    # their diagonals look stair-stepped. 4× is plenty for a fullscreen layer.
    surface_format = QSurfaceFormat.defaultFormat()
    if surface_format.samples() < 4:
        surface_format.setSamples(4)
        QSurfaceFormat.setDefaultFormat(surface_format)

    app = QGuiApplication(arguments)
    app.setApplicationName("kolira-quick-switch")
    app.setOrganizationName("cbr")

    # Toggle, not stack: a second launch closes the open overlay.
    single_instance = QLocalServer(app)
    if not _acquire_single_instance(SINGLE_INSTANCE_SOCKET, single_instance):
        print("kolira-quick-switch: closing the open overlay", file=sys.stderr)
        return 0
    app.aboutToQuit.connect(single_instance.close)

    # Never open on top of a running transaction: the pre-flight and the apply
    # both need the appearance lock, so the overlay would only fail. Preview
    # never applies, so it is allowed through.
    if not preview_only and appearance_is_busy():
        print(
            "kolira-quick-switch: an appearance operation is running; not opening",
            file=sys.stderr,
            flush=True,
        )
        _notify_busy()
        return 0

    engine = QQmlApplicationEngine()
    add_system_import_paths(engine)

    # Parent the controller to the engine: it must outlive the QML bindings,
    # otherwise interpreter shutdown destroys it first and every binding
    # re-evaluation logs "Cannot read property ... of null".
    controller = QuickSwitchController(engine, preview_only=preview_only)
    engine.rootContext().setContextProperty("quickSwitch", controller)

    def _on_second_launch() -> None:
        # Accept and forget: the socket closes when the process quits right
        # after this. Closing it here would race the second process' flush.
        while single_instance.hasPendingConnections():
            single_instance.nextPendingConnection()
        controller.requestToggle()

    single_instance.newConnection.connect(_on_second_launch)

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