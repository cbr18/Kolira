"""
Kolira — Qt/QML Application Entry Point.

v0.6.0 — Native KDE UI with Kirigami, Clean Architecture, Rust core + CLI.
"""

import enum
import os
import sys
from pathlib import Path
from typing import Optional

# Добавляем src в путь для импортов
sys.path.insert(0, str(Path(__file__).parent))

from PySide6.QtWidgets import QApplication
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtCore import QDir, QUrl, QCoreApplication, QTimer, QThreadPool, Qt
from PySide6.QtGui import QPalette, QColor
from PySide6.QtNetwork import QLocalServer, QLocalSocket

# Importing QtQuick ensures the QML root window is typed as QQuickWindow
# (needed for grabWindow() and other QQuickWindow-only API).
import PySide6.QtQuick  # noqa: F401

# Импорт контроллеров
from app_icon import app_icon
from controllers import register_controllers
from theme_controller import register_theme_controller
from system_tray import register_system_tray


def _show_help() -> int:
    """Print help to stdout (avoids QWidget/QApplication dependency)."""
    print(
        """Kolira v0.6.0
A Qt/QML-based theme profile manager for KDE Plasma.

Usage: kolira-gui [options]

Options:
  --core NAME    Core implementation: core | auto
                 (core is the default; auto picks core)
  --list-cores   Show the available cores and exit
  --help, -h     Show this help message
  --version      Show version and exit

Without --core the value of $KTM_CORE is used, then the saved setting
(Settings page → Core). An explicitly requested but missing Rust core is an
error; an unavailable saved setting falls back to the CLI core with a warning.

Requires KDE Plasma 6 + Kirigami (org.kde.kirigami, org.kde.desktop QML modules).
For PySide6 bundled Qt, ensure system QML import paths are available.
"""
    )
    return 0


def _extract_core_arg() -> Optional[str]:
    """Pull ``--core``/``--core=`` out of sys.argv (Qt must not see it)."""
    args = sys.argv[1:]
    requested: Optional[str] = None
    rest = [sys.argv[0]]

    index = 0
    while index < len(args):
        arg = args[index]
        if arg.startswith("--core="):
            requested = arg.split("=", 1)[1]
        elif arg == "--core":
            if index + 1 >= len(args):
                raise SystemExit("--core requires a value: core | auto")
            index += 1
            requested = args[index]
        else:
            rest.append(arg)
        index += 1

    sys.argv[:] = rest
    return requested


def _list_cores() -> int:
    """Print the cores and whether they can be used."""
    print("core     always available (delegates to the Rust CLI kolira / wallpaper-*; local settings)")
    return 0


SINGLE_INSTANCE_SOCKET = "kolira"


class SingleInstance(enum.Enum):
    """Outcome of the single-instance handshake."""

    ACQUIRED = "acquired"        # we own the socket — run normally
    HANDED_OVER = "handed_over"  # a live instance was asked to show itself — exit
    UNAVAILABLE = "unavailable"  # the socket cannot be created — run without the lock


def _ask_running_instance_to_show(name: str) -> bool:
    """Ask a live instance to show its window; False when nobody answers."""
    sock = QLocalSocket()
    sock.connectToServer(name)
    if not sock.waitForConnected(300):
        sock.disconnectFromServer()
        return False
    sock.write(b"show")
    sock.flush()
    sock.waitForBytesWritten(300)
    sock.disconnectFromServer()
    return True


def _acquire_single_instance(name: str, server: QLocalServer) -> SingleInstance:
    """Become the single instance, hand over to the running one, or report
    that the lock is unavailable.

    A live instance can be connected to; only then do we consider the socket
    stale and clean it up. Blindly removing the socket when `listen` fails
    would break the genuinely running first instance (two tray icons). The
    `removeServer` before `listen` handles the crashed-run case.

    `listen` can also fail for reasons unrelated to another copy — e.g. a
    socket path over the 108-byte AF_UNIX limit when $TMPDIR is long. That
    used to be reported as "already running"; now it is UNAVAILABLE and the
    caller starts without the lock and says why.
    """
    if _ask_running_instance_to_show(name):
        return SingleInstance.HANDED_OVER

    QLocalServer.removeServer(name)  # stale socket from a previously-crashed run
    if server.listen(name):
        return SingleInstance.ACQUIRED

    # Lost a race (someone grabbed the name between the probe and the listen)
    # only if that someone answers now; otherwise the socket itself is broken.
    if _ask_running_instance_to_show(name):
        return SingleInstance.HANDED_OVER
    return SingleInstance.UNAVAILABLE


def _single_instance_error(name: str, server: QLocalServer) -> str:
    """Human-readable reason why the single-instance socket is unavailable."""
    path = Path(QDir.tempPath()) / name
    reason = server.errorString() or "unknown error"
    hint = ""
    if len(str(path).encode()) >= 108:
        hint = " (socket path exceeds the 108-byte AF_UNIX limit; shorten $TMPDIR)"
    return f"single-instance socket unavailable at {path}: {reason}{hint}"


def _activate_on_secondary(single_instance: Optional[QLocalServer], main_window: object) -> None:
    """Show/raise the window when a second launch asks the running app for it."""
    if single_instance is None:
        return

    def _on_connection() -> None:
        while single_instance.hasPendingConnections():
            conn = single_instance.nextPendingConnection()
            if conn is not None:
                conn.close()
        try:
            main_window.show()
            main_window.raise_()
            main_window.activateWindow()
        except Exception:  # noqa: BLE001 - cosmetic; the tray must keep working
            pass

    single_instance.newConnection.connect(_on_connection)


def _select_core(requested: Optional[str]) -> None:
    """Choose the core backend before any controller is created.

    Priority: --core flag → $KTM_CORE → saved setting → core.
    """
    from core.protocol import CoreFactory

    explicit = requested is not None or bool(os.environ.get(CoreFactory.ENV_VAR, "").strip())
    if requested is None:
        requested = os.environ.get(CoreFactory.ENV_VAR, "").strip() or None

    if requested is None:
        try:
            from core.cli_core import CliSettingsCore

            requested = CliSettingsCore().load().core_backend or "core"
        except Exception:
            requested = "core"

    # A core asked for by flag/env must exist; a stale saved setting must not
    # stop the application from starting.
    try:
        CoreFactory.set_backend(requested, strict=explicit)
    except Exception as exc:  # noqa: BLE001 - report the reason and bail out
        print(f"Error: {exc}", file=sys.stderr)
        raise SystemExit(3) from exc

    print(
        f"Core: {CoreFactory.backend()} ({CoreFactory.backend_label()})",
        file=sys.stderr,
    )


def _find_system_qml_paths() -> list[str]:
    """Find system QML import directories (Qt6, Qt5) for KDE modules."""
    candidates = [
        "/usr/lib/qt6/qml",
        "/usr/lib64/qt6/qml",
        "/usr/lib/qt5/qml",
        "/usr/lib64/qt5/qml",
        "/usr/share/qt6/qml",
        "/usr/share/qt5/qml",
    ]
    return [p for p in candidates if Path(p).is_dir()]


def _setup_palette(app: QApplication) -> None:
    """Ensure valid palette colors for Kirigami.Theme.

    PySide6 bundles its own Qt; the Kirigami QML singleton reads colors
    from the QApplication palette. Without this, Kirigami.Theme may return
    invalid QColor (QVariant(Invalid)), producing QQuickColorValueType warnings.
    """
    palette = app.palette()
    palette.setColor(QPalette.ColorRole.Window, QColor(239, 240, 243))
    palette.setColor(QPalette.ColorRole.WindowText, QColor(31, 31, 31))
    palette.setColor(QPalette.ColorRole.Base, QColor(255, 255, 255))
    palette.setColor(QPalette.ColorRole.AlternateBase, QColor(245, 245, 245))
    palette.setColor(QPalette.ColorRole.ToolTipBase, QColor(255, 255, 255))
    palette.setColor(QPalette.ColorRole.Text, QColor(31, 31, 31))
    palette.setColor(QPalette.ColorRole.Button, QColor(239, 240, 243))
    palette.setColor(QPalette.ColorRole.ButtonText, QColor(31, 31, 31))
    palette.setColor(QPalette.ColorRole.Highlight, QColor(41, 128, 185))
    palette.setColor(QPalette.ColorRole.HighlightedText, QColor(255, 255, 255))
    palette.setColor(QPalette.ColorRole.Link, QColor(41, 128, 185))
    app.setPalette(palette)


def _maybe_setup_screenshot(app: QApplication, main_window: object) -> None:
    """Dev helper: render the window to a PNG then quit.

    Enabled by setting KTM_SCREENSHOT=/path/to.png — useful for visual
    smoke tests, since the compositor may not allow screen capture.
    """
    shot = os.environ.get("KTM_SCREENSHOT")
    if not shot:
        return

    def _grab() -> None:
        try:
            img = main_window.grabWindow()  # type: ignore[attr-defined]
            ok = img.save(shot)
            print(
                f"screenshot saved: {shot} ok={ok} size={img.width()}x{img.height()}",
                file=sys.stderr,
            )
        except Exception as exc:  # pragma: no cover - dev helper
            print(f"screenshot failed: {exc!r}", file=sys.stderr)
        app.quit()

    QTimer.singleShot(2500, _grab)


def _close_core_session() -> None:
    # Do not remove RPC state while an apply worker still owns it. A busy
    # session remains in XDG cache for recovery; durable backups are separate.
    if QThreadPool.globalInstance().waitForDone(5000):
        from core.protocol import CoreFactory
        CoreFactory.close_session()


def main() -> int:
    # Handle --help / -h early (before QML engine setup)
    if "--help" in sys.argv or "-h" in sys.argv:
        return _show_help()

    # Handle --version
    if "--version" in sys.argv:
        print("kolira 0.6.0")
        return 0

    if "--list-cores" in sys.argv:
        return _list_cores()

    # Core backend must be chosen before any controller builds its core.
    _select_core(_extract_core_arg())

    # Настройка приложения
    QCoreApplication.setApplicationName("kolira")
    QCoreApplication.setApplicationVersion("0.6.0")
    QCoreApplication.setOrganizationName("cbr")
    QCoreApplication.setOrganizationDomain("github.com/cbr18/Kolira")

    # Включаем High DPI scaling
    QApplication.setHighDpiScaleFactorRoundingPolicy(
        Qt.HighDpiScaleFactorRoundingPolicy.PassThrough
    )

    app = QApplication(sys.argv)
    app.aboutToQuit.connect(_close_core_session)
    app.setWindowIcon(app_icon())

    # Переводы по системной локали (без ручного выбора языка). QTranslator
    # ставится до загрузки QML — движок сразу видит переводы qsTr.
    from PySide6.QtCore import QLibraryInfo, QLocale, QTranslator

    i18n_dir = Path(__file__).resolve().parent.parent / "i18n"
    translator = QTranslator(app)
    if translator.load(QLocale.system(), "kolira", "_", str(i18n_dir)):
        app.installTranslator(translator)
    # Стандартные строки Qt (кнопки диалогов и т.п.) — из системных переводов.
    qt_base = QTranslator(app)
    if qt_base.load(QLocale.system(), "qtbase", "_",
                    QLibraryInfo.location(QLibraryInfo.TranslationsPath)):
        app.installTranslator(qt_base)

    # Single instance: two running copies meant two tray icons. A second
    # launch tells the first to show its window and exits.
    single_instance: Optional[QLocalServer] = QLocalServer(app)
    outcome = _acquire_single_instance(SINGLE_INSTANCE_SOCKET, single_instance)
    if outcome is SingleInstance.HANDED_OVER:
        print("kolira is already running.", file=sys.stderr)
        return 0
    if outcome is SingleInstance.UNAVAILABLE:
        print(
            "warning: "
            + _single_instance_error(SINGLE_INSTANCE_SOCKET, single_instance)
            + " — starting without single-instance protection.",
            file=sys.stderr,
        )
        single_instance = None

    # Ensure valid palette for Kirigami (PySide6 bundled Qt may lack KDE palette)
    _setup_palette(app)

    # QML Engine
    engine = QQmlApplicationEngine()

    # Добавляем системные пути для QML импортов (Kirigami, org.kde.desktop)
    # PySide6 bundles its own Qt — system KDE QML modules need explicit paths
    for sys_qml_path in _find_system_qml_paths():
        engine.addImportPath(sys_qml_path)

    # Регистрация контроллеров
    main_controller = register_controllers(engine)

    # Регистрация ThemeController (dogfooding)
    theme_controller = register_theme_controller(engine)

    # Синхронизировать тему с активным профилем
    def sync_theme():
        active = main_controller.profile.activeProfile
        if active:
            theme_controller.sync_with_active_profile(active)

    main_controller.profile.activeProfileChanged.connect(sync_theme)

    # Инициализация контроллеров ДО загрузки QML.
    # Python — единственный источник истины: initialize() идемпотентен
    # (MainController._initialized), и QML НЕ должен повторно вызывать
    # refresh()/load() из Component.onCompleted — это делается здесь.
    main_controller.initialize()

    qml_path = Path(__file__).parent.parent / "qml"
    engine.addImportPath(str(qml_path))

    # Загрузка главной QML
    main_qml = qml_path / "Main.qml"
    engine.load(QUrl.fromLocalFile(str(main_qml)))

    if not engine.rootObjects():
        print("Failed to load QML", file=sys.stderr)
        try:
            for error in engine.errors():
                print(f"  QML Error: {error}", file=sys.stderr)
        except AttributeError:
            pass
        return 1

    # Получить главное окно и зарегистрировать System Tray
    # (требуется окно; QSystemTrayIcon создаётся ровно один раз)
    main_window = engine.rootObjects()[0]
    register_system_tray(engine, main_window, main_controller)

    # Secondary launches ask this instance to pop up the window.
    _activate_on_secondary(single_instance, main_window)

    # Dev helper for visual smoke tests: open a Theme page immediately.
    if os.environ.get("KTM_OPEN_THEME") == "1":
        from PySide6.QtCore import QMetaObject

        page = os.environ.get("KTM_THEME_PAGE", "overview")
        function = {
            "overview": "openThemePage",
            "components": "openThemeComponentsPageDefault",
            "apps": "openThemeApplicationsPage",
            "palette": "openPalettePage",
            "dependencies": "openDependenciesPage",
        }.get(page, "openThemePage")
        QTimer.singleShot(
            0,
            lambda: QMetaObject.invokeMethod(main_window, function),
        )

    # Apply theme synchronously before showing (avoid light flash)
    # sync_theme was triggered during initialize(); force-apply now
    # to ensure glass/paper theme is active before the window appears
    theme_controller._apply_theme()

    main_window.show()

    _maybe_setup_screenshot(app, main_window)

    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
