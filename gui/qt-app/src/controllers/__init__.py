"""
Controllers — слой между UI (QML) и Core.

Содержит бизнес-логику, валидацию, маппинг данных для QML.
Использует PySide6 Signals/Slots для реактивного UI.
"""

from typing import Callable, Optional
import shutil
import sys

from PySide6.QtCore import (
    QCoreApplication, QObject, Signal, Slot, Property, QTimer, QRunnable, QThreadPool, QUrl
)

from core.core_messages import localize
from core.protocol import (
    ProfileCore, WallpaperCore, TrackedAppsCore, SettingsCore,
    ProfileInfo, Component, AppStatus, AppSettings, CustomComponent
)
from core import CoreFactory
from core.paths import profile_dir, validate_profile_name
from .theme_bundle import ThemeBundleController
from .palette import PaletteController
from .dependencies import DependencyController
from .history_schedule import HistoryScheduleController
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


def _reload_app_look() -> None:
    """Show what the CLI just applied in this window too (late import: QML registers it)."""
    from theme_controller import reload_app_look

    reload_app_look()


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

def _needed(report: Optional[dict]) -> list:
    """What to offer after apply/import: the theme's missing dependencies
    (with sources), or — for themes saved before dependencies existed — the
    missing references converted to the same shape."""
    report = report or {}
    deps = list(report.get("dependencies") or [])
    if deps:
        return deps
    return [
        {"index": -1, "kind": m.get("field", ""), "name": m.get("name", ""), "source": "none",
         "id": "", "title": "", "url": m.get("source", ""), "exact": True, "command": None}
        for m in report.get("missing") or []
    ]


class ProfileController(QObject):
    """Контроллер для управления профилями."""

    # Signals для QML
    previewRequested = Signal()
    previewChanged = Signal()
    previewBusyChanged = Signal()
    profilesChanged = Signal()
    summariesChanged = Signal()
    activeProfileChanged = Signal()
    operationStarted = Signal(str)      # operation name
    operationFinished = Signal(bool, str)  # success, message
    componentsForProfileChanged = Signal(list)  # list of component IDs
    diffReady = Signal(str, str, str)   # profile_a, profile_b, diff text
    busyChanged = Signal()
    # A theme was applied but references assets this machine lacks:
    # profile name, [{field, name, source}].
    dependenciesMissing = Signal(str, list)
    # Before applying: profile, what it lacks, keep the wallpaper.
    dependenciesBeforeApply = Signal(str, list, bool)
    dependencyProgress = Signal(str)
    actionsOffered = Signal(str, list)  # profile, follow-up actions (restart apps, Telegram…)

    def __init__(self, parent: Optional[QObject] = None):
        super().__init__(parent)
        self._core: ProfileCore = CoreFactory.profile_core()
        self._preview_data: dict = {}
        self._preview_serial = 0
        self._preview_running = False
        self._preview_queued = None
        self._preview_saved = ""
        self._pending_missing: tuple[str, list] | None = None
        self._pending_actions: tuple[str, list] | None = None
        self._pending_before: tuple[str, list, bool] | None = None
        self._cancelled = False
        self._profiles: list[ProfileInfo] = []
        self._summaries = {}
        self._summary_serial = 0
        self._summary_running = False
        self._summary_pending = None
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

    @Property("QVariantMap", notify=summariesChanged)
    def summaries(self):
        return self._summaries

    def _refresh_summaries(self):
        """Coalesce requests and invalidate results from a previous refresh."""
        names = [p.name for p in self._profiles]
        self._summary_serial += 1
        self._summaries = {k: v for k, v in self._summaries.items() if k in names}
        self.summariesChanged.emit()
        request = (self._summary_serial, names)
        if self._summary_running: self._summary_pending = request
        else: self._load_summaries(request)

    def _load_summaries(self, request):
        from core.cli_core import core_request
        from .theme_inspector import _run
        serial, names = request
        if not names:
            self._summaries = {}
            self.summariesChanged.emit()
            return
        self._summary_running = True
        payload = {}
        def work():
            payload["data"] = core_request("profile.summaries", names=names)
            return True, ""
        def done(ok, message):
            self._summary_running = False
            if serial == self._summary_serial and ok:
                self._summaries = payload["data"]
                self.summariesChanged.emit()
            if self._summary_pending is not None:
                pending, self._summary_pending = self._summary_pending, None
                self._load_summaries(pending)
        _run(self, work, done)

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
        stale = ""
        if self._active_profile and all(
            p.name != self._active_profile for p in self._profiles
        ):
            # Маркер указывает на удалённый профиль (каталог сняли вне
            # приложения). Сбрасываем его — как delete_profile — иначе
            # синхронизация темы уходит в system-detect и UI
            # неожиданно переключается на системную (возможно светлую).
            stale = self._active_profile
            try:
                self._core.clear_stale_active(stale)
            except (OSError, AttributeError):
                pass
            self._active_profile = ""
        self._rebuild_profiles_model()
        self._update_active_components()
        self.profilesChanged.emit()
        self._refresh_summaries()
        self.activeProfileChanged.emit()
        try:
            pending = self._core.pending_operations()
        except (RuntimeError, ValueError, AttributeError):
            pending = []
        for record in pending:
            seen = getattr(self, "_reported_interrupted", set())
            if record["id"] not in seen:
                seen.add(record["id"])
                self._reported_interrupted = seen
                self.operationFinished.emit(False, self.tr("Interrupted apply %1 requires recovery. Backup: %2. Run: kolira operations recover %1")
                    .replace("%1", record["id"]).replace("%2", str(record.get("backup") or "")))
        if stale:
            self.operationFinished.emit(
                False,
                f"Active profile “{stale}” is missing; the marker was reset",
            )

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
            self.operationFinished.emit(False, self.tr("Another operation is still running"))
            return
        self._set_busy(True)
        self._operation = name
        self.operationStarted.emit(name)
        _run_async(self, func, self._on_operation_finished)

    @Slot(bool, str)
    def _on_operation_finished(self, success: bool, message: str) -> None:
        """Слот, вызываемый в главном потоке по завершении воркера."""
        self._set_busy(False)
        if self._pending_before is not None:
            profile, missing, keep = self._pending_before
            self._pending_before = None
            self.dependenciesBeforeApply.emit(profile, missing, keep)
            return
        if success and self._operation in ("apply", "new"):
            _reload_app_look()
        self.operationFinished.emit(success, localize(message))
        if self._pending_missing is not None:
            profile, missing = self._pending_missing
            self._pending_missing = None
            if missing:
                self.dependenciesMissing.emit(profile, missing)
        if self._pending_actions is not None:
            profile, actions = self._pending_actions
            self._pending_actions = None
            if actions:
                self.actionsOffered.emit(profile, actions)
        if success:
            self._refresh_timer.start(500)  # небольшая задержка для применения

    @staticmethod
    def _is_package(profile: str) -> bool:
        try:
            return (profile_dir(profile) / "theme.json").is_file()
        except ValueError:
            return False

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

    @Property("QVariantMap", notify=previewChanged)
    def applyPreview(self) -> dict:
        return self._preview_data

    @Property(bool, notify=previewBusyChanged)
    def previewBusy(self) -> bool:
        return self._preview_running

    @Slot(str, bool)
    def previewApply(self, profile: str, keep_wallpaper: bool = False) -> None:
        if self._busy:
            self.operationFinished.emit(False, self.tr("Another operation is still running"))
            return
        try:
            profile = validate_profile_name(profile)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return
        if self._preview_data.get("name") != profile:
            self._preview_saved = ""
        self._preview_serial += 1
        self._preview_data = {"name": profile, "keep_wallpaper": keep_wallpaper}
        self.previewChanged.emit()
        self.previewRequested.emit()
        request = (profile, keep_wallpaper, self._preview_serial)
        if self._preview_running:
            self._preview_queued = request
        else:
            self._launch_preview(request)

    def _launch_preview(self, request) -> None:
        profile, keep, serial = request
        payload = {}
        self._preview_running = True
        self.previewBusyChanged.emit()
        def work():
            result = self._core.preview(profile, keep)
            payload["report"] = result.data or {}
            return result.success, result.message
        def done(ok, message):
            self._preview_running = False
            if self._preview_queued is not None:
                request = self._preview_queued
                self._preview_queued = None
                self._launch_preview(request)
                return
            self.previewBusyChanged.emit()
            if serial != self._preview_serial:
                return
            self._preview_data = ({**payload["report"], "saved_as": self._preview_saved,
                                   "stale": self._preview_data.get("stale", False)}
                                  if ok else {"name": profile, "keep_wallpaper": keep, "error": message})
            self.previewChanged.emit()
        _run_async(self, work, done)

    @Slot()
    def cancelPreview(self) -> None:
        self._preview_serial += 1
        self._preview_queued = None
        self._preview_data = {}
        self.previewChanged.emit()

    @Slot(str)
    def savePreviewCurrent(self, name: str) -> None:
        if self._preview_running or self._busy or not self._preview_data.get("token"):
            return
        report = dict(self._preview_data)
        self._preview_data.pop("token", None)
        self.previewChanged.emit()
        def work():
            result = self._core.save_before_apply(name)
            return result.success, result.message
        def done(ok, message):
            self._on_operation_finished(ok, message or self.tr("Current appearance saved"))
            if ok:
                self._preview_saved = name
            self.previewApply(report["name"], report.get("keep_wallpaper", False))
        self._set_busy(True)
        self._operation = "save"
        self.operationStarted.emit("save")
        _run_async(self, work, done)

    @Slot(str)
    def confirmPreview(self, token: str) -> None:
        if self._preview_running or self._busy or not token or token != self._preview_data.get("token"):
            return
        report = dict(self._preview_data)
        payload = {}
        def work():
            result = self._core.apply(report["name"], components=report.get("components"),
                keep_wallpaper=report.get("keep_wallpaper", False), skip_dependencies=True,
                preview_token=token)
            payload["stale"] = not result.success and "PREVIEW_STALE" in result.message
            if result.success:
                self._pending_actions = (report["name"], list((result.data or {}).get("actions") or []))
            return result.success, result.message
        def done(ok, message):
            if payload.get("stale"):
                self._set_busy(False)
                self.operationFinished.emit(False, self.tr("Theme or live settings changed. Review the updated changes."))
                self.previewApply(report["name"], report.get("keep_wallpaper", False))
                self._preview_data["stale"] = True
                self.previewChanged.emit()
            else:
                self._on_operation_finished(ok, message)
        self._set_busy(True)
        self._operation = "apply"
        self.operationStarted.emit("apply")
        _run_async(self, work, done)

    @Slot(str, bool, bool, bool)
    def applyProfile(self, profile: str, keep_wallpaper: bool = False,
                     install_missing: bool = False, ask_first: bool = False) -> None:
        """Применить тему (асинхронно).

        Theme packages apply everything they define (component settings do
        not apply to them). With ``ask_first`` (the window's Apply button) what
        the theme lacks is offered *before* anything changes:
        ``dependenciesBeforeApply``; the answer comes back as
        ``installDependencies`` or ``applyProfileNow``. Legacy snapshots keep
        the component filter.
        """
        try:
            profile = validate_profile_name(profile)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return
        if self._is_package(profile):
            def _work() -> tuple[bool, str]:
                if ask_first and not install_missing:
                    pending = self._core.pending_dependencies(profile)
                    if pending.success and pending.data:
                        self._pending_before = (profile, list(pending.data), keep_wallpaper)
                        return True, ""
                result = self._core.apply(
                    profile, keep_wallpaper=keep_wallpaper, install_missing=install_missing
                )
                self._pending_missing = (profile, _needed(result.data))
                report = result.data if isinstance(result.data, dict) else {}
                self._pending_actions = (profile, list(report.get("actions") or []))
                return result.success, result.message

            self._start_operation("apply", _work)
            return
        components, error = self._settings_components()
        if error:
            self.operationFinished.emit(False, error)
            return

        def _work() -> tuple[bool, str]:
            result = self._core.apply(profile, components)
            return result.success, result.message

        self._start_operation("apply", _work)

    @Slot(str, bool)
    def applyProfileNow(self, profile: str, keep_wallpaper: bool = False) -> None:
        """Apply without what is missing: the user already answered."""
        try:
            profile = validate_profile_name(profile)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return

        def _work() -> tuple[bool, str]:
            result = self._core.apply(profile, keep_wallpaper=keep_wallpaper)
            report = result.data if isinstance(result.data, dict) else {}
            self._pending_actions = (profile, list(report.get("actions") or []))
            return result.success, result.message

        self._start_operation("apply", _work)

    @Slot(str, bool)
    def applyProfileWithoutDependencies(self, profile: str, keep_wallpaper: bool = False) -> None:
        """Explicit apply choice after the dependency dialog; never downloads."""
        try:
            profile = validate_profile_name(profile)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return
        def work():
            result = self._core.apply(profile, keep_wallpaper=keep_wallpaper, skip_dependencies=True)
            report = result.data if isinstance(result.data, dict) else {}
            self._pending_actions = (profile, list(report.get("actions") or []))
            return result.success, result.message
        self._start_operation("apply", work)

    @Slot()
    def cancelOperation(self) -> None:
        """Stop a dependency download between two items (each has its own limit)."""
        self._cancelled = True

    @Slot(str, str)
    def newTheme(self, name: str, base: str) -> None:
        """Start a new theme from a base (neutral template or a theme copy)."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return

        def _work() -> tuple[bool, str]:
            result = self._core.new_theme(name, base)
            return result.success, result.message

        self._start_operation("new", _work)

    @Slot(str, str, bool, bool)
    def saveProfileWithLookAndFeel(self, name: str, description: str,
                                   include_wallpaper: bool, make_look_and_feel: bool) -> None:
        """Save, first creating the theme's own Look & Feel package if asked."""
        if not make_look_and_feel:
            self.saveProfile(name, description, include_wallpaper)
            return
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return

        def _work() -> tuple[bool, str]:
            laf = self._core.look_and_feel(name)
            if not laf.success:
                return False, laf.message
            result = self._core.save(name, description=description.strip(), include_wallpaper=include_wallpaper)
            return result.success, (result.message if not result.success else f"Saved profile: {name}")

        self._start_operation("save", _work)

    @Slot(str, str, bool)
    def saveProfile(self, name: str, description: str = "",
                    include_wallpaper: bool = True) -> None:
        """Сохранить текущее оформление как тему-пакет (асинхронно).

        A theme stores theme values + local assets (+ the wallpaper unless
        ``include_wallpaper`` is false); the component settings do not apply.
        """
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return

        def _work() -> tuple[bool, str]:
            result = self._core.save(
                name, description=description.strip(), include_wallpaper=include_wallpaper
            )
            if not result.success:
                return result.success, result.message
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

    @staticmethod
    def _local_path(url_or_path: str) -> str:
        """QML FileDialog gives ``file:///…`` URLs; the CLI wants paths."""
        text = (url_or_path or "").strip()
        if text.startswith("file:"):
            return QUrl(text).toLocalFile()
        return text

    @Slot(str, str, bool)
    def exportProfile(self, name: str, file_url: str, include_wallpaper: bool) -> None:
        """Export a theme to a .zip (asynchronously)."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return
        path = self._local_path(file_url)

        def _work() -> tuple[bool, str]:
            result = self._core.export_theme(name, path, include_wallpaper)
            return result.success, result.message

        self._start_operation("export", _work)

    @Slot(str, str, bool)
    def importTheme(self, file_url: str, name: str, force: bool) -> None:
        """Import a theme (.zip); missing parts go to ``dependenciesMissing``."""
        path = self._local_path(file_url)

        def _work() -> tuple[bool, str]:
            result = self._core.import_theme(path, name, force)
            if result.success:
                report = result.data or {}
                self._pending_missing = (str(report.get("name", "")), _needed(report))
            return result.success, result.message

        self._start_operation("import", _work)

    @Slot(str, list, bool, bool)
    def installDependencies(self, profile: str, indexes: list, reapply: bool, keep_wallpaper: bool = False) -> None:
        """Download what was chosen, one item at a time so one that fails or
        stalls costs only itself, then (optionally) apply the theme. System
        packages go last, in one terminal the user types in."""
        try:
            profile = validate_profile_name(profile)
        except ValueError as exc:
            self.operationFinished.emit(False, str(exc))
            return
        chosen = [int(i) for i in indexes]
        self._cancelled = False

        def _work() -> tuple[bool, str]:
            listed = self._core.dependencies(profile)
            known = {d.get("index"): d for d in (listed.data or [])} if listed.success else {}
            root = [i for i in chosen if known.get(i, {}).get("source") in ("package", "aur")]
            store = [i for i in chosen if i not in root]
            lines: list[str] = []
            ok = True
            total = len(store) + (1 if root else 0)
            for step, index in enumerate(store, 1):
                if self._cancelled:
                    break
                dep = known.get(index, {})
                self.dependencyProgress.emit(f"{dep.get('title') or dep.get('name') or index} ({step}/{total})")
                result = self._core.install_dependencies(profile, [index], timeout=300)
                ok &= result.success
                lines.append(result.message or f"✗ {index}")
            if root and not self._cancelled:
                self.dependencyProgress.emit(f"{self.tr('System packages: see the terminal')} ({total}/{total})")
                result = self._core.install_dependencies(profile, root)
                ok &= result.success
                lines.append(result.message)
            if reapply and ok and not self._cancelled:
                applied = self._core.apply(profile, keep_wallpaper=keep_wallpaper)
                ok &= applied.success
                if applied.message:
                    lines.append(applied.message.splitlines()[0])
                report = applied.data if isinstance(applied.data, dict) else {}
                self._pending_actions = (profile, list(report.get("actions") or []))
                self._operation = "apply"
            elif self._cancelled:
                lines.append(self.tr("Stopped: the theme was not applied"))
            return ok, "\n".join(lines)

        self._start_operation("install", _work)

    @Slot(list)
    def runActions(self, ids: list) -> None:
        """Do the follow-up actions the user ticked (restart programs…) — `kolira action ID` each, in the background."""
        chosen = [str(i) for i in ids]

        def _work() -> tuple[bool, str]:
            lines, ok = [], True
            for action_id in chosen:
                result = self._core.run_action(action_id)
                ok &= result.success
                lines.append(("✓ " if result.success else "✗ ") + result.message)
            return ok, "\n".join(lines)

        self._start_operation("actions", _work)

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
            self.operationFinished.emit(False, self.tr("Another operation is still running"))
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

    @Slot(str, result=str)
    def componentName(self, component_id):
        return next((c["name"] for c in self._components_model if c["id"] == component_id), component_id)

    def _status_text(self, status: AppStatus) -> str:
        return {
            AppStatus.AVAILABLE: self.tr("Installed"),
            AppStatus.NOT_INSTALLED: self.tr("Not installed"),
            AppStatus.PARTIAL: self.tr("Partial"),
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
            "description": QCoreApplication.translate("TrackedApps", c.description) if c.description else "",
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
            "description": QCoreApplication.translate("TrackedApps", c.description) if c.description else "",
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

    @Slot(QUrl)
    def setWallpaperDirectoryUrl(self, url: QUrl) -> None:
        if url.isLocalFile():
            self.wallpaperDirectory = url.toLocalFile()

    # ── Automation: what applying a theme may do by itself (per user) ──
    automationChanged = Signal()

    def _automation(self) -> dict:
        getter = getattr(self._core, "automation", None)
        return getter() if callable(getter) else {}

    @Property(list, notify=automationChanged)
    def automationItems(self) -> list:
        """[{key, what, note, policy}] from `kolira automation --json`."""
        return list(self._automation().get("items") or [])

    @Property(bool, notify=automationChanged)
    def automationConfigured(self) -> bool:
        return bool(self._automation().get("configured", True))

    @Slot(str, str)
    def setAutomation(self, key: str, policy: str) -> None:
        setter = getattr(self._core, "set_automation", None)
        if callable(setter):
            setter({key: policy})
        self.automationChanged.emit()

    @Slot("QVariantMap")
    def confirmAutomation(self, choices: dict) -> None:
        """First run: store every choice shown (marks automation configured)."""
        setter = getattr(self._core, "set_automation", None)
        if callable(setter):
            current = {i["key"]: i["policy"] for i in self.automationItems}
            current.update({str(k): str(v) for k, v in dict(choices).items()})
            setter(current)
        self.automationChanged.emit()

    @Property(bool, notify=settingsChanged)
    def networkEnabled(self) -> bool:
        """Online features: palette gallery catalogue, editor theme stores."""
        return self._settings.network_enabled

    @networkEnabled.setter
    def networkEnabled(self, value: bool) -> None:
        if self._settings.network_enabled != value:
            self._settings = AppSettings(
                **{**self._settings.to_dict(), "network_enabled": bool(value)}
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
        self._bundle = ThemeBundleController(self)
        self._palette = PaletteController(self)
        self._history_schedule = HistoryScheduleController(self)
        self._history_timer = QTimer(self)
        self._history_timer.setInterval(20000)
        self._history_timer.timeout.connect(self._history_schedule.refreshHistory)
        self._history_timer.start()
        self._history_schedule.restored.connect(self._profile.refresh)
        self._history_schedule.restored.connect(_reload_app_look)
        self._dependencies = DependencyController(self)
        self._dependencies.applyRequested.connect(self._profile.previewApply)
        self._dependencies.policySaved.connect(self._settings.automationChanged.emit)
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

    @Property(QObject, constant=True)
    def bundle(self):
        return self._bundle

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
            QTimer.singleShot(0, lambda: self._profile.applyProfileNow(profile))


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
    engine.rootContext().setContextProperty("themeBundle", main_ctrl._bundle)
    engine.rootContext().setContextProperty("paletteController", main_ctrl._palette)
    engine.rootContext().setContextProperty("dependencyController", main_ctrl._dependencies)
    engine.rootContext().setContextProperty("historySchedule", main_ctrl._history_schedule)
    
    return main_ctrl