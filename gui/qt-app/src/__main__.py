"""
KDE Theme Profile Manager — Qt/QML Application Entry Point.

v0.5.0 — Native KDE UI with Kirigami, Clean Architecture, Rust core + CLI.
"""

import os
import sys
from pathlib import Path
from typing import Optional

# Добавляем src в путь для импортов
sys.path.insert(0, str(Path(__file__).parent))

from PySide6.QtGui import QIcon
from PySide6.QtWidgets import QApplication
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtCore import QUrl, QCoreApplication, QTimer, Qt
from PySide6.QtGui import QPalette, QColor
from PySide6.QtNetwork import QLocalServer, QLocalSocket

# Importing QtQuick ensures the QML root window is typed as QQuickWindow
# (needed for grabWindow() and other QQuickWindow-only API).
import PySide6.QtQuick  # noqa: F401

# Импорт контроллеров
from app_icon import app_icon
from controllers import MainController, register_controllers
from theme_controller import register_theme_controller
from system_tray import register_system_tray

# Регистрация мета-типов для QML (not needed in PySide6, Python types auto-registered)
from core.protocol import Result, ProfileInfo, Component, AppStatus, AppSettings


def _show_help() -> int:
    """Print help to stdout (avoids QWidget/QApplication dependency)."""
    print(
        """KDE Theme Profile Manager v0.5.0
A Qt/QML-based theme profile manager for KDE Plasma.

Usage: kde-theme-manager [options]

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
    print("core     always available (delegates to the Rust CLI theme-profile / wallpaper-*; local settings)")
    return 0


SINGLE_INSTANCE_SOCKET = "kde-theme-manager"


def _raise_existing_instance(name: str) -> None:
    """Ask the already-running instance to show its window."""
    sock = QLocalSocket()
    sock.connectToServer(name)
    if sock.waitForConnected(300):
        sock.write(b"show")
        sock.flush()
        sock.waitForBytesWritten(300)
    sock.disconnectFromServer()


def _acquire_single_instance(name: str, server: QLocalServer) -> bool:
    """Become the single instance (True) or hand over to the running one (False).

    A live instance can be connected to; only then do we consider the socket
    stale and clean it up. Blindly removing the socket when `listen` fails
    would break the genuinely running first instance (two tray icons). The
    `removeServer` before the second `listen` handles the crashed-run case.
    """
    probe = QLocalSocket()
    probe.connectToServer(name)
    if probe.waitForConnected(300):
        # Another instance is alive — tell it to show the window and exit.
        probe.write(b"show")
        probe.flush()
        probe.waitForBytesWritten(300)
        probe.disconnectFromServer()
        return False
    probe.disconnectFromServer()

    QLocalServer.removeServer(name)  # stale socket from a previously-crashed run
    if server.listen(name):
        return True

    # Lost a race — someone grabbed the name between the probe and the listen.
    _raise_existing_instance(name)
    return False


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


def main() -> int:
    # Handle --help / -h early (before QML engine setup)
    if "--help" in sys.argv or "-h" in sys.argv:
        return _show_help()

    # Handle --version
    if "--version" in sys.argv:
        print("KDE Theme Manager 0.5.0")
        return 0

    if "--list-cores" in sys.argv:
        return _list_cores()

    # Core backend must be chosen before any controller builds its core.
    _select_core(_extract_core_arg())

    # Настройка приложения
    QCoreApplication.setApplicationName("KDE Theme Manager")
    QCoreApplication.setApplicationVersion("0.5.0")
    QCoreApplication.setOrganizationName("cbr")
    QCoreApplication.setOrganizationDomain("github.com/cbr18/kde-theme-manager")

    # Включаем High DPI scaling
    QApplication.setHighDpiScaleFactorRoundingPolicy(
        Qt.HighDpiScaleFactorRoundingPolicy.PassThrough
    )

    app = QApplication(sys.argv)
    app.setWindowIcon(app_icon())

    # Переводы по системной локали (без ручного выбора языка). QTranslator
    # ставится до загрузки QML — движок сразу видит переводы qsTr.
    from PySide6.QtCore import QLibraryInfo, QLocale, QTranslator

    i18n_dir = Path(__file__).resolve().parent.parent / "i18n"
    translator = QTranslator(app)
    if translator.load(QLocale.system(), "kde-theme-manager", "_", str(i18n_dir)):
        app.installTranslator(translator)
    # Стандартные строки Qt (кнопки диалогов и т.п.) — из системных переводов.
    qt_base = QTranslator(app)
    if qt_base.load(QLocale.system(), "qtbase", "_",
                    QLibraryInfo.location(QLibraryInfo.TranslationsPath)):
        app.installTranslator(qt_base)

    # Single instance: two running copies meant two tray icons. A second
    # launch tells the first to show its window and exits.
    single_instance = QLocalServer(app)
    if not _acquire_single_instance(SINGLE_INSTANCE_SOCKET, single_instance):
        print("KDE Theme Manager is already running.", file=sys.stderr)
        return 0

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
