"""
Controllers — слой между UI (QML) и Core.

Содержит бизнес-логику, валидацию, маппинг данных для QML.
Использует PySide6 Signals/Slots для реактивного UI.
"""

from typing import Callable, Optional
from dataclasses import dataclass
import shutil
import sys

from PySide6.QtCore import (
    QObject, Signal, Slot, Property, QTimer, QRunnable, QThreadPool
)

from core.protocol import (
    ProfileCore, WallpaperCore, TrackedAppsCore, SettingsCore,
    ProfileInfo, Component, AppStatus, Result, AppSettings, CustomComponent
)
from core import CoreFactory
from core.paths import validate_profile_name
from .theme_inspector import ThemeInspectorController


# ──────────────────────────────────────────────────────────────
# Асинхронные операции (shared worker для QThreadPool)
# ──────────────────────────────────────────────────────────────

class _OperationSignals(QObject):
    """Signals holder for a background operation.

    Must be created on the main (GUI) thread so that Qt uses a queued
    connection when the worker emits back to the owning controller.
    """

    finished = Signal(bool, str)  # success, message


class _OperationWorker(QRunnable):
    """Run a zero-argument callable returning ``(bool, str)`` off the GUI thread.

    The callable is executed on the global thread pool; the result (or the
    stringified exception) is emitted via ``signals.finished``. Widgets / QML
    objects must never be touched from here — only the signal is emitted.
    """

    def __init__(self, func: Callable[[], tuple[bool, str]], signals: _OperationSignals) -> None:
        super().__init__()
        self._func = func
        self._signals = signals

    def run(self) -> None:
        try:
            success, message = self._func()
        except Exception as exc:  # noqa: BLE001 - must surface to the UI
            success, message = False, str(exc)
        self._signals.finished.emit(bool(success), message)


class _StringSignals(QObject):
    """Signals holder for a background job that returns a single string."""

    done = Signal(str)


class _StringWorker(QRunnable):
    """Run ``func() -> str`` off the GUI thread and emit the result."""

    def __init__(self, func: Callable[[], str], signals: _StringSignals) -> None:
        super().__init__()
        self._func = func
        self._signals = signals

    def run(self) -> None:
        try:
            text = str(self._func())
        except Exception as exc:  # noqa: BLE001 - surface as text
            text = f"Error: {exc}"
        self._signals.done.emit(text)


def _run_async_str(owner: QObject, func: Callable[[], str], on_done: Callable[[str], None]) -> None:
    """Start ``func`` on the thread pool, deliver its string via ``on_done``."""
    signals = _StringSignals(owner)
    signals.done.connect(on_done)
    signals.done.connect(signals.deleteLater)
    QThreadPool.globalInstance().start(_StringWorker(func, signals))


def _run_async(
    owner: QObject,
    func: Callable[[], tuple[bool, str]],
    on_finished: Callable[[bool, str], None],
) -> None:
    """Start ``func`` on the global thread pool, call ``on_finished`` on the GUI thread.

    ``owner`` keeps the signal object alive and provides the correct thread
    affinity for the queued connection back to ``on_finished``.
    """
    signals = _OperationSignals(owner)
    signals.finished.connect(on_finished)
    # The holder is parented to `owner` so it outlives the worker; release it
    # once the queued result has been delivered.
    signals.finished.connect(signals.deleteLater)
    QThreadPool.globalInstance().start(_OperationWorker(func, signals))


# ──────────────────────────────────────────────────────────────
# Контроллер профилей
# ──────────────────────────────────────────────────────────────

class ProfileController(QObject):
    """Контроллер для управления профилями."""

    # Signals для QML
    profilesChanged = Signal()
    activeProfileChanged = Signal()
    operationStarted = Signal(str)      # operation name
    operationFinished = Signal(bool, str)  # success, message
    componentsForProfileChanged = Signal(list)  # list of component IDs
    diffReady = Signal(str, str, str)   # profile_a, profile_b, diff text
    busyChanged = Signal()

    def __init__(self, parent: Optional[QObject] = None):
        super().__init__(parent)
        self._core: ProfileCore = CoreFactory.profile_core()
        self._profiles: list[ProfileInfo] = []
        self._profiles_model: list[dict] = []   # QML-ready cache (built once per refresh)
        self._active_profile: str = ""
        self._components_for_active: list[str] = []
        self._busy: bool = False
        self._refresh_timer = QTimer(self)
        self._refresh_timer.setSingleShot(True)
        self._refresh_timer.timeout.connect(self._delayed_refresh)

    @Property(list, notify=profilesChanged)
    def profiles(self) -> list:
        """Список профилей для QML (list of dicts) — возвращает кэш, не пересобирает."""
        return self._profiles_model

    @Property(bool, notify=busyChanged)
    def busy(self) -> bool:
        """True пока выполняется фоновая операция (apply/save/delete)."""
        return self._busy

    @Property(str, notify=activeProfileChanged)
    def activeProfile(self) -> str:
        return self._active_profile

    @Property(list, notify=componentsForProfileChanged)
    def componentsForActiveProfile(self) -> list:
        return self._components_for_active

    def _set_busy(self, value: bool) -> None:
        if self._busy != value:
            self._busy = value
            self.busyChanged.emit()

    def _rebuild_profiles_model(self) -> None:
        """Пересобрать QML-модель один раз (вызывается из refresh())."""
        self._profiles_model = [
            {
                "name": p.name,
                "isActive": p.is_active,
                "componentCount": len(p.components),
                "components": list(p.components),
                "createdAt": p.created_at,
                "description": p.description,
            }
            for p in self._profiles
        ]

    @Slot()
    def refresh(self) -> None:
        """Обновить список профилей."""
        self._profiles = self._core.list_profiles()
        self._active_profile = self._core.get_active_profile()
        self._rebuild_profiles_model()
        self._update_active_components()
        self.profilesChanged.emit()
        self.activeProfileChanged.emit()

    def _update_active_components(self) -> None:
        if self._active_profile:
            self._components_for_active = self._core.get_profile_components(self._active_profile)
        else:
            self._components_for_active = []
        self.componentsForProfileChanged.emit(self._components_for_active)

    def _delayed_refresh(self) -> None:
        self.refresh()

    def _start_operation(self, name: str, func: Callable[[], tuple[bool, str]]) -> None:
        """Запустить фоновую операцию (не блокирует UI).

        Повторный запрос во время выполнения игнорируется.
        """
        if self._busy:
            self.operationFinished.emit(False, "Another operation is still running")
            return
        self._set_busy(True)
        self.operationStarted.emit(name)
        _run_async(self, func, self._on_operation_finished)

    @Slot(bool, str)
    def _on_operation_finished(self, success: bool, message: str) -> None:
        """Слот, вызываемый в главном потоке по завершении воркера."""
        self._set_busy(False)
        self.operationFinished.emit(success, message)
        if success:
            self._refresh_timer.start(500)  # небольшая задержка для применения

    def _settings_components(self) -> tuple[Optional[list[str]], str]:
        settings_core = CoreFactory.settings_core()
        settings = settings_core.load()
        disabled = set(settings.disabled_components)
        if settings.default_components_mode == "all":
            if not disabled:
                return None, ""
            known = [c.id for c in CoreFactory.tracked_apps_core().get_all_components()]
            components = [c for c in known if c not in disabled]
            if not components:
                return None, "All components are disabled (see Applications)"
            return components, ""
        components = [c for c in settings.custom_components if c not in disabled]
        if not components:
            return None, "Select at least one component in Settings"
        return components, ""

    @Slot(str)
    def applyProfile(self, profile: str) -> None:
        """Применить профиль (асинхронно)."""
        try:
            profile = validate_profile_name(profile)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return
        components, error = self._settings_components()
        if error:
            self.operationFinished.emit(False, error)
            return

        def _work() -> tuple[bool, str]:
            result = self._core.apply(profile, components)
            return result.success, result.message

        self._start_operation("apply", _work)

    @Slot(str, str)
    def saveProfile(self, name: str, description: str = "") -> None:
        """Сохранить текущее состояние как профиль (асинхронно).

        Описание (необязательное) пишется сразу после сохранения через
        ``describe --set`` — в один проход до рефреша модели.
        """
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return
        components, error = self._settings_components()
        if error:
            self.operationFinished.emit(False, error)
            return

        def _work() -> tuple[bool, str]:
            result = self._core.save(name, components)
            if not result.success:
                return result.success, result.message
            if description.strip():
                described = self._core.set_description(name, description.strip())
                if not described.success:
                    return False, described.message
            return True, f"Saved profile: {name}"

        self._start_operation("save", _work)

    @Slot(str, str)
    def setProfileDescription(self, profile: str, description: str) -> None:
        """Изменить описание существующего профиля (асинхронно)."""
        try:
            profile = validate_profile_name(profile)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return

        def _work() -> tuple[bool, str]:
            result = self._core.set_description(profile, description.strip())
            return result.success, result.message or "Description updated"

        self._start_operation("describe", _work)

    @Slot(str)
    def deleteProfile(self, name: str) -> None:
        """Удалить профиль (асинхронно)."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return

        def _work() -> tuple[bool, str]:
            result = self._core.delete(name)
            return result.success, result.message

        self._start_operation("delete", _work)

    @Slot(str, str)
    def duplicateProfile(self, source: str, target: str) -> None:
        """Дублировать профиль (асинхронно): копия снимка под новым именем."""
        try:
            source = validate_profile_name(source)
            target = validate_profile_name(target)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return

        def _work() -> tuple[bool, str]:
            result = self._core.clone(source, target)
            return result.success, result.message or f"Cloned profile: {source} -> {target}"

        self._start_operation("clone", _work)

    @Slot(str, str)
    def requestDiff(self, profile_a: str, profile_b: str) -> None:
        """Сравнить два профиля в фоне.

        The recursive diff of two config trees can take seconds; computing it
        on the GUI thread froze the window. The result is emitted on
        ``diffReady`` (with the requested pair) once the thread pool is done.
        """
        def _work() -> str:
            return self._core.diff(profile_a, profile_b)

        def _emit(text: str) -> None:
            self.diffReady.emit(profile_a, profile_b, text)

        _run_async_str(self, _work, _emit)

    @Slot(str, str, result=str)
    def diffProfiles(self, profile_a: str, profile_b: str) -> str:
        """Синхронное сравнение (Python-вызовы и тесты; QML использует requestDiff)."""
        return self._core.diff(profile_a, profile_b)

    @Slot(str, result=list)
    def getProfileComponents(self, profile: str) -> list:
        """Получить компоненты профиля для отображения в UI."""
        comps = self._core.get_profile_components(profile)
        return comps


# ──────────────────────────────────────────────────────────────
# Контроллер обоев
# ──────────────────────────────────────────────────────────────

class WallpaperController(QObject):
    """Контроллер для управления обоями."""

    currentWallpaperChanged = Signal()
    wallpapersChanged = Signal()
    operationStarted = Signal(str)
    operationFinished = Signal(bool, str)
    busyChanged = Signal()

    def __init__(self, parent: Optional[QObject] = None):
        super().__init__(parent)
        self._core: WallpaperCore = CoreFactory.wallpaper_core()
        self._wallpapers: list[str] = []
        self._current_wallpaper: str = ""
        self._busy: bool = False
        self._refresh_timer = QTimer(self)
        self._refresh_timer.setSingleShot(True)
        self._refresh_timer.timeout.connect(self._delayed_refresh)

    @Property(str, notify=currentWallpaperChanged)
    def currentWallpaper(self) -> str:
        return self._current_wallpaper

    @Property(list, notify=wallpapersChanged)
    def wallpapers(self) -> list:
        return self._wallpapers

    @Property(bool, notify=busyChanged)
    def busy(self) -> bool:
        """True while a background wallpaper operation runs.

        Opening the picker (wofi) can take as long as the user needs, so it
        must never run on the GUI thread — this is what the tray menu and the
        QML buttons key off.
        """
        return self._busy

    def _set_busy(self, value: bool) -> None:
        if self._busy != value:
            self._busy = value
            self.busyChanged.emit()

    @Slot()
    def refresh(self) -> None:
        self._wallpapers = self._core.list_wallpapers()
        self._current_wallpaper = self._core.current() or ""
        self.wallpapersChanged.emit()
        self.currentWallpaperChanged.emit()

    def _delayed_refresh(self) -> None:
        self.refresh()

    def _start_operation(self, name: str, func: Callable[[], tuple[bool, str]]) -> None:
        """Запустить фоновую операцию (не блокирует UI)."""
        if self._busy:
            self.operationFinished.emit(False, "Another operation is still running")
            return
        self._set_busy(True)
        self.operationStarted.emit(name)
        _run_async(self, func, self._on_operation_finished)

    @Slot(bool, str)
    def _on_operation_finished(self, success: bool, message: str) -> None:
        self._set_busy(False)
        self.operationFinished.emit(success, message)
        if success:
            self._refresh_timer.start(500)

    @Slot(result=str)
    def pickWallpaper(self) -> str:
        """Открыть пикер в фоне.

        Returns an empty string: the picker is interactive, so the result
        arrives asynchronously through ``currentWallpaperChanged``.
        """
        def _work() -> tuple[bool, str]:
            path = self._core.pick()
            return path is not None, path or "Cancelled"

        self._start_operation("pick", _work)
        return ""

    @Slot()
    def nextWallpaper(self) -> None:
        def _work() -> tuple[bool, str]:
            result = self._core.next()
            return result.success, result.message

        self._start_operation("next", _work)

    @Slot()
    def prevWallpaper(self) -> None:
        def _work() -> tuple[bool, str]:
            result = self._core.prev()
            return result.success, result.message

        self._start_operation("prev", _work)

    @Slot(str)
    def setWallpaper(self, path: str) -> None:
        def _work() -> tuple[bool, str]:
            result = self._core.set_wallpaper(path)
            return result.success, result.message

        self._start_operation("set", _work)


# ──────────────────────────────────────────────────────────────
# Контроллер отслеживаемых приложений
# ──────────────────────────────────────────────────────────────

class TrackedAppsController(QObject):
    """Контроллер для реестра приложений."""

    componentsChanged = Signal()
    categoriesChanged = Signal()
    componentsRevisionChanged = Signal()

    def __init__(self, parent: Optional[QObject] = None):
        super().__init__(parent)
        self._core: TrackedAppsCore = CoreFactory.tracked_apps_core()
        self._components: list[Component] = []
        self._components_model: list[dict] = []          # QML cache (built once per refresh)
        self._components_by_category: dict[str, list[dict]] = {}
        self._categories: list[str] = []
        self._components_revision: int = 0
        self._detecting: bool = False
        self._pending_refresh: bool = False
        self._detected: list[Component] = []

    @Property(list, notify=componentsChanged)
    def components(self) -> list:
        """Компоненты для QML (list of dicts) — возвращает кэш."""
        return self._components_model

    @Property(list, notify=categoriesChanged)
    def categories(self) -> list:
        return self._categories

    @Property(int, notify=componentsRevisionChanged)
    def componentsRevision(self) -> int:
        """Счётчик ревизий: меняется при установке новых результатов детекта."""
        return self._components_revision

    def _status_text(self, status: AppStatus) -> str:
        return {
            AppStatus.AVAILABLE: "Установлен",
            AppStatus.NOT_INSTALLED: "Не установлен",
            AppStatus.PARTIAL: "Частично",
        }[status]

    def _disabled_ids(self) -> set[str]:
        try:
            return set(CoreFactory.settings_core().load().disabled_components)
        except Exception:
            return set()

    def _component_to_dict(self, c: Component) -> dict:
        """Полное представление компонента (для self.components)."""
        return {
            "id": c.id,
            "name": c.name,
            "category": c.category,
            "status": c.status.value,
            "statusText": self._status_text(c.status),
            "supportsLiveReload": c.supports_live_reload,
            "description": c.description,
            "icon": c.icon,
            "docsUrl": c.docs_url,
            "configPaths": list(c.config_paths),
            "isCustom": c.is_custom,
            "reloadCommand": c.reload_command,
            "enabled": c.id not in self._disabled_ids(),
        }

    def _category_entry(self, c: Component) -> dict:
        """Компонент в разрезе категории (для getComponentsByCategory)."""
        return {
            "id": c.id,
            "name": c.name,
            "status": c.status.value,
            "statusText": self._status_text(c.status),
            "supportsLiveReload": c.supports_live_reload,
            "description": c.description,
            "icon": c.icon,
            "docsUrl": c.docs_url,
            "isCustom": c.is_custom,
            "reloadCommand": c.reload_command,
            "enabled": c.id not in self._disabled_ids(),
        }

    def _install_detection(self, components: list[Component]) -> None:
        """Установить результаты детекта (только главный поток) и разослать сигналы."""
        self._components = list(components or [])
        self._components_model = [self._component_to_dict(c) for c in self._components]

        categories: list[str] = []
        by_category: dict[str, list[dict]] = {}
        for c in self._components:
            entry = self._category_entry(c)
            by_category.setdefault(c.category, []).append(entry)
            if c.category not in categories:
                categories.append(c.category)
        self._categories = categories
        self._components_by_category = by_category

        self._components_revision += 1
        self.componentsChanged.emit()
        self.categoriesChanged.emit()
        self.componentsRevisionChanged.emit()

    @Slot()
    def refresh(self) -> None:
        """Запустить детект в фоне (не блокирует первый кадр)."""
        if self._detecting:
            # Уже идёт — схлопываем в единственный отложенный повтор
            self._pending_refresh = True
            return
        self._detecting = True

        def _work() -> tuple[bool, str]:
            self._detected = self._core.detect_all()
            return True, ""

        _run_async(self, _work, self._on_detection_finished)

    @Slot(bool, str)
    def _on_detection_finished(self, success: bool, message: str) -> None:
        """Воркер вернулся (главный поток): установить результат и перезапустить при необходимости."""
        self._detecting = False
        if success:
            self._install_detection(self._detected)
        if self._pending_refresh:
            self._pending_refresh = False
            self.refresh()

    @Slot(str, result=list)
    def getComponentsByCategory(self, category: str) -> list:
        """Компоненты категории из предвычисленного кэша."""
        return self._components_by_category.get(category, [])

    @Slot(str, bool)
    def setComponentTracked(self, comp_id: str, tracked: bool) -> None:
        """Галочка «отслеживать» (страница «Приложения»)."""
        settings_core = CoreFactory.settings_core()
        settings = settings_core.load()
        disabled = set(settings.disabled_components)
        if tracked:
            disabled.discard(comp_id)
        else:
            disabled.add(comp_id)
        if disabled == set(settings.disabled_components):
            return
        settings_core.save(AppSettings(
            **{**settings.to_dict(), "disabled_components": tuple(sorted(disabled))}
        ))
        self._install_detection(self._components)

    # ── Пользовательские компоненты: CRUD ─────────────────────

    _RELOAD_FORBIDDEN = set("|&;<>$`\"'\n\r")

    @classmethod
    def _validate_reload_command(cls, command: str) -> str:
        """Ранняя валидация (зеркало Rust validate_reload_command)."""
        if not command:
            return ""
        bad = next((ch for ch in command if ch in cls._RELOAD_FORBIDDEN), None)
        if bad is not None:
            return f"Reload command contains a forbidden shell character: {bad!r}"
        binary = command.split()[0]
        if shutil.which(binary) is None:
            return f"Reload command binary not found in PATH: {binary}"
        return ""

    @staticmethod
    def _custom_from_dict(fields: dict) -> tuple[Optional[CustomComponent], str]:
        import re

        name = str(fields.get("name", "")).strip()
        if not name:
            return None, "Name is required"
        raw_id = str(fields.get("id", "")).strip()
        if not raw_id:
            # Авто-id из имени: слаг [A-Za-z0-9._-]; сменить позже нельзя
            # (удалить и создать заново).
            raw_id = re.sub(r"[^A-Za-z0-9]+", "-", name).strip("-_.")[:64]
        try:
            comp_id = validate_profile_name(raw_id)
        except ValueError:
            return None, "Invalid id (allowed: letters, digits, . _ -)"
        raw_paths = fields.get("configPaths", []) or []
        paths = tuple(dict.fromkeys(str(p).strip() for p in raw_paths if str(p).strip()))
        if not paths:
            return None, "At least one config path is required"
        for p in paths:
            if p.startswith("/") or ".." in p:
                return None, f"Path must be relative without '..': {p}"
        command = str(fields.get("reloadCommand", "")).strip()
        error = TrackedAppsController._validate_reload_command(command)
        if error:
            return None, error
        return CustomComponent(
            id=comp_id,
            name=name,
            category=str(fields.get("category", "")).strip() or "Custom",
            config_paths=paths,
            reload_command=command,
            supports_live_reload=bool(command),
            description=str(fields.get("description", "")).strip(),
            icon=str(fields.get("icon", "")).strip() or "application-x-executable",
        ), ""

    def _scrub_settings_and_refresh(self, comp_id: str, *, drop_from_selection: bool) -> None:
        """Убрать id из списков настроек и перечитать реестр."""
        settings_core = CoreFactory.settings_core()
        settings = settings_core.load()
        data = settings.to_dict()
        changed = False
        disabled = [i for i in settings.disabled_components if i != comp_id]
        if len(disabled) != len(settings.disabled_components):
            data["disabled_components"] = disabled
            changed = True
        if drop_from_selection:
            selected = [i for i in settings.custom_components if i != comp_id]
            if len(selected) != len(settings.custom_components):
                data["custom_components"] = selected
                changed = True
        if changed:
            settings_core.save(AppSettings(**data))
        self.refresh()

    @Slot(dict, result=str)
    def addCustomComponent(self, fields: dict) -> str:
        """Создать пользовательский компонент; "" = успех, иначе текст ошибки."""
        comp, error = self._custom_from_dict(fields)
        if error:
            return error
        result = self._core.add_custom(comp)
        if not result.success:
            return result.message
        self.refresh()
        return ""

    @Slot(dict, result=str)
    def updateCustomComponent(self, fields: dict) -> str:
        comp, error = self._custom_from_dict(fields)
        if error:
            return error
        result = self._core.update_custom(comp)
        if not result.success:
            return result.message
        self.refresh()
        return ""

    @Slot(str, result=str)
    def removeCustomComponent(self, comp_id: str) -> str:
        try:
            comp_id = validate_profile_name(str(comp_id))
        except ValueError as exc:
            return str(exc)
        result = self._core.remove_custom(comp_id)
        if not result.success:
            return result.message
        self._scrub_settings_and_refresh(comp_id, drop_from_selection=True)
        return ""


# ──────────────────────────────────────────────────────────────
# Контроллер настроек
# ──────────────────────────────────────────────────────────────

class SettingsController(QObject):
    """Контроллер настроек приложения."""

    settingsChanged = Signal()
    availableComponentsChanged = Signal()

    def __init__(self, parent: Optional[QObject] = None):
        super().__init__(parent)
        self._core: SettingsCore = CoreFactory.settings_core()
        self._settings: AppSettings = self._core.get_defaults()
        self._available_components: list[str] = []
        # Optional reference to the tracked-apps controller (wired by
        # MainController) so this cache never triggers its own detection.
        self._tracked_apps: Optional[QObject] = None

    @Property(str, notify=settingsChanged)
    def defaultComponentsMode(self) -> str:
        return self._settings.default_components_mode

    @defaultComponentsMode.setter
    def defaultComponentsMode(self, value: str) -> None:
        if self._settings.default_components_mode != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "default_components_mode": value}
            )
            self.settingsChanged.emit()

    @Property(list, notify=settingsChanged)
    def customComponents(self) -> list:
        return list(self._settings.custom_components)

    @customComponents.setter
    def customComponents(self, value: list) -> None:
        if list(self._settings.custom_components) != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "custom_components": tuple(value)}
            )
            self.settingsChanged.emit()

    @Property(bool, notify=settingsChanged)
    def wallpaperLinkedToProfile(self) -> bool:
        return self._settings.wallpaper_linked_to_profile

    @wallpaperLinkedToProfile.setter
    def wallpaperLinkedToProfile(self, value: bool) -> None:
        if self._settings.wallpaper_linked_to_profile != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "wallpaper_linked_to_profile": value}
            )
            self.settingsChanged.emit()

    @Property(str, notify=settingsChanged)
    def wallpaperDirectory(self) -> str:
        return self._settings.wallpaper_directory

    @wallpaperDirectory.setter
    def wallpaperDirectory(self, value: str) -> None:
        if self._settings.wallpaper_directory != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "wallpaper_directory": value}
            )
            self.settingsChanged.emit()

    @Property(str, notify=settingsChanged)
    def coreBackend(self) -> str:
        """Ядро, которое будет использовано при следующем запуске."""
        return self._settings.core_backend

    @coreBackend.setter
    def coreBackend(self, value: str) -> None:
        if self._settings.core_backend != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "core_backend": value}
            )
            self.settingsChanged.emit()

    @Property(str, notify=settingsChanged)
    def activeCore(self) -> str:
        """Ядро, работающее прямо сейчас (переключение — рестартом)."""
        return CoreFactory.backend_label()

    @Property(str, notify=settingsChanged)
    def theme(self) -> str:
        return self._settings.theme

    @theme.setter
    def theme(self, value: str) -> None:
        if self._settings.theme != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "theme": value}
            )
            self.settingsChanged.emit()
            self._apply_theme()

    @Property(bool, notify=settingsChanged)
    def showNotInstalledWarning(self) -> bool:
        return self._settings.show_not_installed_warning

    @showNotInstalledWarning.setter
    def showNotInstalledWarning(self, value: bool) -> None:
        if self._settings.show_not_installed_warning != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "show_not_installed_warning": value}
            )
            self.settingsChanged.emit()

    @Property(bool, notify=settingsChanged)
    def autoApplyOnStartup(self) -> bool:
        return self._settings.auto_apply_on_startup

    @autoApplyOnStartup.setter
    def autoApplyOnStartup(self, value: bool) -> None:
        if self._settings.auto_apply_on_startup != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "auto_apply_on_startup": value}
            )
            self.settingsChanged.emit()

    @Property(bool, notify=settingsChanged)
    def minimizeToTray(self) -> bool:
        return self._settings.minimize_to_tray

    @minimizeToTray.setter
    def minimizeToTray(self, value: bool) -> None:
        if self._settings.minimize_to_tray != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "minimize_to_tray": value}
            )
            self.settingsChanged.emit()

    @Property(bool, notify=settingsChanged)
    def confirmBeforeApply(self) -> bool:
        return self._settings.confirm_before_apply

    @confirmBeforeApply.setter
    def confirmBeforeApply(self, value: bool) -> None:
        if self._settings.confirm_before_apply != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "confirm_before_apply": value}
            )
            self.settingsChanged.emit()

    @Property(bool, notify=settingsChanged)
    def createBackupBeforeApply(self) -> bool:
        return self._settings.create_backup_before_apply

    @createBackupBeforeApply.setter
    def createBackupBeforeApply(self, value: bool) -> None:
        if self._settings.create_backup_before_apply != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "create_backup_before_apply": value}
            )
            self.settingsChanged.emit()

    @Property(bool, notify=settingsChanged)
    def notifyOnSuccess(self) -> bool:
        return self._settings.notify_on_success

    @notifyOnSuccess.setter
    def notifyOnSuccess(self, value: bool) -> None:
        if self._settings.notify_on_success != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "notify_on_success": value}
            )
            self.settingsChanged.emit()

    @Property(list, notify=availableComponentsChanged)
    def availableComponents(self) -> list:
        """Список доступных компонентов для кастомного выбора (из кэша)."""
        return self._available_components

    @Slot()
    def refreshAvailableComponents(self) -> None:
        """Пересобрать кэш доступных компонентов (не вызывается на каждый read).

        Reads the already-detected registry from the tracked-apps controller
        when one is wired in: calling ``get_all_components()`` on a cold core
        cache would run a full (blocking) detection on the GUI thread.
        """
        if self._tracked_apps is not None:
            available = [
                c["id"] for c in self._tracked_apps.components
                if c["status"] == AppStatus.AVAILABLE.value
            ]
        else:
            apps_core = CoreFactory.tracked_apps_core()
            available = [c.id for c in apps_core.get_all_components() if c.status == AppStatus.AVAILABLE]

        if available == self._available_components:
            return
        self._available_components = available
        self.availableComponentsChanged.emit()

    def set_tracked_apps(self, controller: QObject) -> None:
        """Wire the tracked-apps controller so this cache never triggers a detection."""
        self._tracked_apps = controller

    def _apply_theme(self) -> None:
        """Применить тему к приложению (dogfooding)."""
        try:
            from theme_controller import ThemeController
            controller = ThemeController.instance()
            controller.currentTheme = self._settings.theme
        except Exception as e:
            print(f"ThemeController integration failed: {e}", file=sys.stderr)

    @Slot()
    def load(self) -> None:
        self._settings = self._core.load()
        self.refreshAvailableComponents()
        self.settingsChanged.emit()

    @Slot()
    def save(self) -> None:
        result = self._core.save(self._settings)
        if result.success:
            self.refreshAvailableComponents()

    @Slot()
    def resetToDefaults(self) -> None:
        self._settings = self._core.get_defaults()
        self.refreshAvailableComponents()
        self.settingsChanged.emit()


# ──────────────────────────────────────────────────────────────
# Главный контроллер (агрегатор)
# ──────────────────────────────────────────────────────────────

class MainController(QObject):
    """Главный контроллер — точка входа для QML."""

    def __init__(self, parent: Optional[QObject] = None):
        super().__init__(parent)
        self._profile = ProfileController(self)
        self._wallpaper = WallpaperController(self)
        self._tracked_apps = TrackedAppsController(self)
        self._settings = SettingsController(self)
        self._theme = ThemeInspectorController(self)
        self._initialized = False

        # The settings controller's "available components" cache must read the
        # already-detected registry instead of shelling out on every read.
        self._settings.set_tracked_apps(self._tracked_apps)
        self._tracked_apps.componentsChanged.connect(
            self._settings.refreshAvailableComponents
        )

    @Property(QObject, constant=True)
    def profile(self):
        return self._profile

    @Property(QObject, constant=True)
    def wallpaper(self):
        return self._wallpaper

    @Property(QObject, constant=True)
    def tracked_apps(self):
        return self._tracked_apps

    @Property(QObject, constant=True)
    def settings(self):
        return self._settings

    @Property(QObject, constant=True)
    def theme(self):
        return self._theme

    @Slot()
    def initialize(self) -> None:
        """Инициализация всех контроллеров (идемпотентно — второй вызов no-op)."""
        if self._initialized:
            return
        self._initialized = True
        self._profile.refresh()
        self._wallpaper.refresh()
        self._tracked_apps.refresh()
        self._settings.load()
        self._theme.refresh()

        # Opt-in: re-apply the last active profile after the UI is up.  The
        # setting existed for a long time but was never read.
        if self._settings.autoApplyOnStartup and self._profile.activeProfile:
            profile = self._profile.activeProfile
            QTimer.singleShot(0, lambda: self._profile.applyProfile(profile))


# ──────────────────────────────────────────────────────────────
# Регистрация типов для QML
# ──────────────────────────────────────────────────────────────

def register_controllers(engine) -> None:
    """Зарегистрировать контроллеры в QML engine."""
    main_ctrl = MainController()
    
    # Register main controller
    engine.rootContext().setContextProperty("mainController", main_ctrl)
    
    # Register sub-controllers as separate context properties for direct access
    engine.rootContext().setContextProperty("profileController", main_ctrl._profile)
    engine.rootContext().setContextProperty("wallpaperController", main_ctrl._wallpaper)
    engine.rootContext().setContextProperty("trackedAppsController", main_ctrl._tracked_apps)
    engine.rootContext().setContextProperty("settingsController", main_ctrl._settings)
    engine.rootContext().setContextProperty("themeInspector", main_ctrl._theme)
    
    return main_ctrl