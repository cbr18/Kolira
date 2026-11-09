"""
System Tray Controller.

Provides:
- System tray icon with context menu (if available)
- Quick profile switching straight from the tray
- Quick wallpaper navigation
- Desktop notifications for background operations
- Minimize to tray instead of close

A configurable global hotkey was removed: KGlobalAccel only creates a component
object for applications that install a desktop entry carrying
``X-KDE-GlobalAccel-CommandShortcut=true``, so a bare D-Bus ``doRegister`` never
produced a working shortcut and the setting was dead.
"""

from __future__ import annotations

import os
from PySide6.QtCore import QObject, Signal, Slot, Property, QCoreApplication
from PySide6.QtGui import QAction, QActionGroup, QWindow

from app_icon import app_icon

# Make system tray optional - import lazily
QSystemTrayIcon = None
QMenu = None


def _import_widgets():
    """Lazy import of QtWidgets to avoid segfault on Wayland."""
    global QSystemTrayIcon, QMenu
    if QSystemTrayIcon is None:
        try:
            from PySide6.QtWidgets import QSystemTrayIcon as _QSystemTrayIcon, QMenu as _QMenu
            QSystemTrayIcon = _QSystemTrayIcon
            QMenu = _QMenu
        except Exception:
            QSystemTrayIcon = None
            QMenu = None


class SystemTrayController(QObject):
    """Управляет системным треем."""

    trayIconVisibleChanged = Signal(bool)

    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            cls._instance._initialized = False
        return cls._instance

    def __init__(self):
        if self._initialized:
            return
        super().__init__()
        self._initialized = True

        self._tray_icon = None
        self._tray_menu = None
        self._profiles_menu = None
        self._wallpaper_menu = None
        self._main_window = None
        self._profile_controller = None
        self._wallpaper_controller = None
        self._pending_profile = ""
        self._minimize_to_tray = False
        self._notify_on_success = True

    @classmethod
    def instance(cls) -> "SystemTrayController":
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    def set_main_window(self, window) -> None:
        """Установить главное окно для управления видимостью."""
        self._main_window = window

    def set_profile_controller(self, controller) -> None:
        """Подключить ProfileController: список профилей в трее + быстрый выбор."""
        if controller is None:
            return
        self._profile_controller = controller
        controller.profilesChanged.connect(self._rebuild_profiles_menu)
        controller.activeProfileChanged.connect(self._rebuild_profiles_menu)
        controller.busyChanged.connect(self._rebuild_profiles_menu)
        controller.operationFinished.connect(self._on_profile_operation_finished)
        self._rebuild_profiles_menu()

    def set_wallpaper_controller(self, controller) -> None:
        """Подключить WallpaperController: next/prev прямо из трея."""
        if controller is None:
            return
        self._wallpaper_controller = controller
        controller.currentWallpaperChanged.connect(self._rebuild_wallpaper_menu)
        controller.wallpapersChanged.connect(self._rebuild_wallpaper_menu)
        controller.busyChanged.connect(self._rebuild_wallpaper_menu)
        self._rebuild_wallpaper_menu()

    @Property(bool, notify=trayIconVisibleChanged)
    def trayIconVisible(self) -> bool:
        return self._tray_icon is not None and self._tray_icon.isVisible()

    @Property(bool)
    def minimizeToTray(self) -> bool:
        return self._minimize_to_tray

    @minimizeToTray.setter
    def minimizeToTray(self, value: bool) -> None:
        self._minimize_to_tray = value

    def initialize(self) -> None:
        """Инициализировать трей."""
        try:
            self._create_tray_icon()
        except Exception as e:
            print(f"System tray not available: {e}")
            self._tray_icon = None

    def _create_tray_icon(self) -> None:
        """Создать иконку в системном трее."""
        _import_widgets()
        if QSystemTrayIcon is None:
            raise RuntimeError("QtWidgets not available")

        if self._tray_icon is not None:
            return

        # Check if system tray is available
        try:
            if not QSystemTrayIcon.isSystemTrayAvailable():
                raise RuntimeError("System tray not available")
        except Exception:
            raise RuntimeError("System tray not available")

        # System tray requires QApplication (QWidget), not just QGuiApplication
        from PySide6.QtWidgets import QApplication
        if not QApplication.instance():
            raise RuntimeError("QApplication required for system tray")

        try:
            self._tray_icon = QSystemTrayIcon()
        except Exception as e:
            raise RuntimeError(f"Failed to create system tray icon: {e}")

        self._tray_icon.setIcon(app_icon())
        self._tray_icon.setToolTip("kolira")

        # Контекстное меню
        _import_widgets()
        self._tray_menu = QMenu()

        show_action = QAction(
            QCoreApplication.translate("SystemTray", "Show"), self._tray_menu
        )
        show_action.triggered.connect(self._show_window)
        self._tray_menu.addAction(show_action)

        # Quick profile switching (rebuilt whenever the profile list changes).
        self._tray_menu.addSeparator()
        self._profiles_menu = QMenu(
            QCoreApplication.translate("SystemTray", "Profiles"), self._tray_menu
        )
        self._tray_menu.addMenu(self._profiles_menu)

        # Wallpaper navigation.
        self._wallpaper_menu = QMenu(
            QCoreApplication.translate("SystemTray", "Wallpaper"), self._tray_menu
        )
        self._tray_menu.addMenu(self._wallpaper_menu)

        self._tray_menu.addSeparator()
        quit_action = QAction(
            QCoreApplication.translate("SystemTray", "Quit"), self._tray_menu
        )
        quit_action.triggered.connect(self._quit_app)
        self._tray_menu.addAction(quit_action)

        self._rebuild_profiles_menu()
        self._rebuild_wallpaper_menu()

        self._tray_icon.setContextMenu(self._tray_menu)

        # Клик — показать/скрыть (см. _on_tray_activated)
        self._tray_icon.activated.connect(self._on_tray_activated)

        self._tray_icon.show()
        self.trayIconVisibleChanged.emit(True)

    # ──────────────────────────────────────────────────────────────
    # Tray menu contents
    # ──────────────────────────────────────────────────────────────

    @staticmethod
    def _profile_entries(profiles, active: str) -> list[dict]:
        """QML-модель профилей → пункты меню (активный первым, затем по алфавиту).

        Pure function so it can be tested without a QApplication.
        """
        entries: list[dict] = []
        for profile in profiles or []:
            if isinstance(profile, dict):
                name = str(profile.get("name", ""))
                count = profile.get("componentCount")
            else:
                name = str(profile)
                count = None
            if not name:
                continue
            label = f"{name} ({count})" if isinstance(count, int) and count > 0 else name
            entries.append({"name": name, "label": label, "active": name == active})
        entries.sort(key=lambda e: (not e["active"], e["name"].casefold()))
        return entries

    def _rebuild_profiles_menu(self) -> None:
        """Пересобрать подменю профилей (активный отмечен галочкой)."""
        if self._profiles_menu is None:
            return

        self._profiles_menu.clear()
        controller = self._profile_controller

        if controller is None:
            empty = self._profiles_menu.addAction(
                QCoreApplication.translate("SystemTray", "Unavailable")
            )
            empty.setEnabled(False)
            return

        entries = self._profile_entries(controller.profiles, controller.activeProfile)
        if not entries:
            empty = self._profiles_menu.addAction(
                QCoreApplication.translate("SystemTray", "No profiles")
            )
            empty.setEnabled(False)
            return

        busy = bool(controller.busy)
        group = QActionGroup(self._profiles_menu)
        group.setExclusive(True)
        for entry in entries:
            action = self._profiles_menu.addAction(entry["label"])
            action.setCheckable(True)
            action.setChecked(entry["active"])
            action.setEnabled(not busy)
            action.triggered.connect(
                lambda _checked=False, name=entry["name"]: self._apply_profile(name)
            )
            group.addAction(action)

        if self._tray_icon is not None:
            active = controller.activeProfile or "—"
            self._tray_icon.setToolTip(f"kolira — {active}")

    def _rebuild_wallpaper_menu(self) -> None:
        """Пересобрать подменю обоев."""
        if self._wallpaper_menu is None:
            return

        self._wallpaper_menu.clear()
        controller = self._wallpaper_controller
        if controller is None:
            empty = self._wallpaper_menu.addAction(
                QCoreApplication.translate("SystemTray", "Unavailable")
            )
            empty.setEnabled(False)
            return

        next_action = QAction(
            QCoreApplication.translate("SystemTray", "Next wallpaper"), self._wallpaper_menu
        )
        next_action.triggered.connect(controller.nextWallpaper)
        prev_action = QAction(
            QCoreApplication.translate("SystemTray", "Previous wallpaper"),
            self._wallpaper_menu,
        )
        prev_action.triggered.connect(controller.prevWallpaper)
        pick_action = QAction(
            QCoreApplication.translate("SystemTray", "Pick wallpaper…"), self._wallpaper_menu
        )
        pick_action.triggered.connect(controller.pickWallpaper)

        for action in (next_action, prev_action, pick_action):
            action.setEnabled(not bool(controller.busy))
            self._wallpaper_menu.addAction(action)

        current = controller.currentWallpaper
        if current:
            self._wallpaper_menu.addSeparator()
            label = os.path.basename(current) or current
            current_action = self._wallpaper_menu.addAction(label)
            current_action.setEnabled(False)

    # ──────────────────────────────────────────────────────────────
    # Actions
    # ──────────────────────────────────────────────────────────────

    def _apply_profile(self, name: str) -> None:
        """Open the shared pre-apply review from the tray."""
        controller = self._profile_controller
        if controller is None or not name:
            return
        if name == controller.activeProfile or controller.busy:
            self._rebuild_profiles_menu()
            return
        self._pending_profile = name
        controller.previewApply(name, False)

    @Slot(bool, str)
    def _on_profile_operation_finished(self, success: bool, message: str) -> None:
        """Показать результат применения профиля, запущенного из трея."""
        name = self._pending_profile
        self._pending_profile = ""
        self._rebuild_profiles_menu()
        if not name:
            # An operation started from the GUI — it reports for itself.
            return
        if success:
            if self._notify_on_success:
                self._notify(
                    QCoreApplication.translate("SystemTray", "Profile applied"),
                    message or name,
                )
        else:
            self._notify(
                QCoreApplication.translate("SystemTray", "Could not apply profile"),
                message or name,
                critical=True,
            )

    def _notify(self, title: str, body: str, critical: bool = False) -> None:
        """Показать уведомление рабочего стола.

        Uses ``QSystemTrayIcon.showMessage`` rather than a hand-built
        ``org.freedesktop.Notifications.Notify`` call: PySide6 marshals Python
        ints as D-Bus ``i``, but ``Notify`` requires ``u`` for ``replaces_id``,
        so a hand-built call is rejected with "No such method 'Notify'" and the
        notification is dropped silently. Qt's own implementation marshals it
        correctly (verified on Plasma 6 with dbus-monitor).

        Without a tray icon there is nothing to route the notification through
        (pure-QML mode); the message is then skipped.
        """
        if self._tray_icon is None:
            return
        try:
            _import_widgets()
            icon = (
                QSystemTrayIcon.MessageIcon.Critical
                if critical
                else app_icon()
            )
            self._tray_icon.showMessage(title, body, icon, 5000)
        except Exception:
            pass

    def _quit_app(self) -> None:
        """Полностью выйти из приложения (из трея)."""
        QCoreApplication.quit()

    def _on_tray_activated(self, reason: QSystemTrayIcon.ActivationReason) -> None:
        """Обработка кликов по иконке трея.

        Plasma's system tray (StatusNotifierItem) sends *Activate* on a single
        left click — Qt reports it as ``Trigger``; a double click hardly ever
        arrives. Reacting only to ``DoubleClick`` made the icon look dead.
        Any click does what the menu's *Show* does: show and raise the window.
        """
        if reason in (QSystemTrayIcon.Trigger, QSystemTrayIcon.DoubleClick, QSystemTrayIcon.MiddleClick):
            self._show_window()

    def _show_window(self) -> None:
        """Показать главное окно (в том числе свёрнутое)."""
        if self._main_window:
            if self._main_window.visibility() == QWindow.Visibility.Minimized:
                self._main_window.showNormal()
            else:
                self._main_window.show()
            self._main_window.raise_()
            self._main_window.requestActivate()


    def set_notify_on_success(self, value: bool) -> None:
        self._notify_on_success = bool(value)


def register_system_tray(engine, main_window, main_controller=None) -> None:
    """Зарегистрировать SystemTrayController в QML."""
    from PySide6.QtWidgets import QApplication
    if not QApplication.instance():
        # QGuiApplication (QtQuick/QML only) - skip system tray to avoid QWidget crash
        print("System tray skipped: QApplication not available (pure QML mode)")
        return None

    controller = SystemTrayController.instance()
    controller.set_main_window(main_window)

    if main_controller is not None:
        # Quick switching from the tray: needs the profile list, the wallpaper
        # navigation and the "minimize to tray" preference.
        controller.set_profile_controller(main_controller.profile)
        controller.set_wallpaper_controller(main_controller.wallpaper)

        settings = main_controller.settings
        controller.minimizeToTray = settings.minimizeToTray
        controller.set_notify_on_success(settings.notifyOnSuccess)

        def _sync_settings() -> None:
            controller.minimizeToTray = settings.minimizeToTray
            controller.set_notify_on_success(settings.notifyOnSuccess)

        settings.settingsChanged.connect(_sync_settings)

    controller.initialize()
    engine.rootContext().setContextProperty("trayController", controller)
    return controller
