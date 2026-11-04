"""QML controller for the high-level theme bundle view."""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from PySide6.QtCore import Property, QObject, QUrl, Signal, Slot

from core import CoreFactory
from core.paths import active_profile_file, validate_profile_name
from core.theme_bundle import ThemeBundleCore

from .theme_inspector import _run


class ThemeBundleController(QObject):
    """Theme → component groups, application adapters and profile bundles."""

    groupsChanged = Signal()
    applicationsChanged = Signal()
    profilesChanged = Signal()
    scopeChanged = Signal()
    busyChanged = Signal()
    storeChanged = Signal()
    toast = Signal(str, bool)

    def __init__(self, parent: Optional[QObject] = None) -> None:
        super().__init__(parent)
        self._core = ThemeBundleCore(
            theme_core=CoreFactory.theme_core(),
            profile_core=CoreFactory.profile_core(),
            wallpaper_core=CoreFactory.wallpaper_core(),
        )
        self._scope = "live"
        self._groups: list[dict] = []
        self._applications: list[dict] = []
        self._profiles: list[dict] = []
        self._profile_bundle: dict = {}
        self._busy = False
        self._store_items: list = []
        self._store_app = ""
        self._store_busy = False
        self.refresh()

    # ── properties ───────────────────────────────────────────────────────

    @Property(list, notify=groupsChanged)
    def groups(self) -> list:
        return self._groups

    @Property(list, notify=applicationsChanged)
    def applications(self) -> list:
        return self._applications

    @Property(list, notify=profilesChanged)
    def profiles(self) -> list:
        return self._profiles

    @Property("QVariantMap", notify=scopeChanged)
    def profileBundle(self) -> dict:
        return self._profile_bundle

    @Property(str, notify=scopeChanged)
    def scope(self) -> str:
        return self._scope

    @Property(str, notify=profilesChanged)
    def activeProfile(self) -> str:
        try:
            return active_profile_file().read_text(encoding="utf-8").strip()
        except OSError:
            return ""

    @Property(bool, notify=scopeChanged)
    def readOnly(self) -> bool:
        return self._scope != "live"

    @Property(int, notify=groupsChanged)
    def sessionChanges(self) -> int:
        return self._core.session_changes()

    @Property(int, notify=groupsChanged)
    def undoDepth(self) -> int:
        return self._core.undo_depth()

    @Property(bool, notify=busyChanged)
    def busy(self) -> bool:
        return self._busy

    @Property(list, notify=storeChanged)
    def storeItems(self) -> list:
        """Last store search results: {id, name, description, version, downloads}."""
        return self._store_items

    @Property(str, notify=storeChanged)
    def storeApp(self) -> str:
        return self._store_app

    @Property(bool, notify=storeChanged)
    def storeBusy(self) -> bool:
        return self._store_busy

    def _set_store(self, *, items: Optional[list] = None, app: Optional[str] = None,
                   busy: Optional[bool] = None) -> None:
        if items is not None:
            self._store_items = items
        if app is not None:
            self._store_app = app
        if busy is not None:
            self._store_busy = busy
        self.storeChanged.emit()

    @Slot(str, str)
    def storeSearch(self, app_id: str, query: str) -> None:
        """Search the editor's theme store off the GUI thread."""
        if self._store_busy:
            return
        holder: dict = {}

        def work() -> tuple[bool, str]:
            result = self._core.store_search(app_id, query)
            holder["items"] = (result.data or {}).get("items", [])
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_store(items=holder.get("items", []) if success else [], busy=False)
            if not success:
                self.toast.emit(message, False)

        self._set_store(app=app_id, items=[], busy=True)
        _run(self, work, done)

    @Slot(str, str)
    def storeInstall(self, app_id: str, extension_id: str) -> None:
        """Install a store theme; the app's theme list refreshes afterwards."""
        if self._store_busy:
            return

        def work() -> tuple[bool, str]:
            result = self._core.store_install(app_id, extension_id)
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_store(busy=False)
            self.toast.emit(message, success)
            self.refresh()

        self._set_store(busy=True)
        _run(self, work, done)

    # ── refresh ──────────────────────────────────────────────────────────

    @Slot()
    def refresh(self) -> None:
        self._groups = self._core.groups(self._scope)
        self._applications = self._core.applications()
        self._profiles = self._core.profiles()
        self.groupsChanged.emit()
        self.applicationsChanged.emit()
        self.profilesChanged.emit()

    def _set_busy(self, value: bool) -> None:
        if self._busy != value:
            self._busy = value
            self.busyChanged.emit()

    # ── scope and profiles ───────────────────────────────────────────────

    @Slot()
    def openLive(self) -> None:
        self._scope = "live"
        self._profile_bundle = {}
        self.scopeChanged.emit()
        self.refresh()

    @Slot(str)
    def openProfile(self, name: str) -> None:
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.toast.emit(str(exc), False)
            return
        result = self._core.profile_bundle(name)
        if not result.success:
            self.toast.emit(result.message, False)
            return
        self._scope = f"profile:{name}"
        self._profile_bundle = result.data or {}
        self.scopeChanged.emit()
        self.refresh()

    @Slot(str)
    def saveProfile(self, name: str, description: str = "") -> None:
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.toast.emit(str(exc), False)
            return
        if self._busy:
            self.toast.emit(self.tr("Another operation is still running"), False)
            return

        def work() -> tuple[bool, str]:
            result = self._core.save_profile(name, description)
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            self.toast.emit(message, success)
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    @Slot(str)
    def applyProfile(self, name: str) -> None:
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.toast.emit(str(exc), False)
            return
        if self._busy:
            self.toast.emit(self.tr("Another operation is still running"), False)
            return

        def work() -> tuple[bool, str]:
            result = self._core.apply_profile(name)
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            self.toast.emit(message, success)
            self.openLive()
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    # ── edits ────────────────────────────────────────────────────────────

    @Slot(str, "QVariant")
    def setComponent(self, component_id: str, value: object) -> None:
        if self._busy:
            self.toast.emit(self.tr("Another operation is still running"), False)
            return
        result = self._core.set_component(component_id, value)
        self.toast.emit(result.message, result.success)
        self.refresh()

    @Slot()
    def applyChanges(self) -> None:
        if self._busy:
            self.toast.emit(self.tr("Another operation is still running"), False)
            return

        def work() -> tuple[bool, str]:
            result = self._core.apply()
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            self.toast.emit(message, success)
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    @Slot()
    def undo(self) -> None:
        result = self._core.undo()
        self.toast.emit(result.message, result.success)
        self.refresh()

    @Slot()
    def revert(self) -> None:
        if self._busy:
            self.toast.emit(self.tr("Another operation is still running"), False)
            return

        def work() -> tuple[bool, str]:
            result = self._core.revert()
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            self.toast.emit(message, success)
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    @Slot(str)
    def createCustomTheme(self, name: str) -> None:
        if self._busy:
            self.toast.emit(self.tr("Another operation is still running"), False)
            return

        def work() -> tuple[bool, str]:
            result = self._core.create_custom_theme(name)
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            self.toast.emit(message, success)
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    @Slot(str)
    def setWallpaper(self, path_or_url: str) -> None:
        if not path_or_url:
            return
        local = QUrl(path_or_url).toLocalFile() if path_or_url.startswith("file:") else path_or_url
        result = self._core.set_component("wallpaper", local)
        self.toast.emit(result.message, result.success)
        self.refresh()

    # ── external actions ─────────────────────────────────────────────────

    @Slot(str)
    def openSystemSettings(self, module: str) -> None:
        result = self._core.open_system_settings(module)
        if not result.success:
            self.toast.emit(result.message, False)

    @Slot(str)
    def launchApp(self, app_id: str) -> None:
        result = self._core.launch_app(app_id)
        self.toast.emit(result.message, result.success)

    @Slot(str)
    def openAppConfig(self, app_id: str) -> None:
        result = self._core.open_app_config(app_id)
        self.toast.emit(result.message, result.success)

    @Slot(str)
    def openAppThemeSettings(self, app_id: str) -> None:
        result = self._core.open_app_theme_settings(app_id)
        self.toast.emit(result.message, result.success)


__all__ = ["ThemeBundleController"]
