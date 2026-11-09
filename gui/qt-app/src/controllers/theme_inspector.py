"""Controller for the Theme Inspector screen.

The inspector reads live KDE/app configuration through ``ThemeCore`` and exposes
QML-ready dicts. Writes are always applied to the *live* system and run on the
thread pool; profile snapshots stay read-only.
"""

from __future__ import annotations

import subprocess
import sys
from threading import Event
from datetime import datetime
from pathlib import Path
from typing import Callable, Optional

from PySide6.QtCore import (
    Property,
    QObject,
    QRunnable,
    QThreadPool,
    QTimer,
    QUrl,
    Signal,
    Slot,
)

from core import CoreFactory
from core.paths import (
    live_path,
    profile_dir,
    profiles_dir as profiles_root,
    validate_profile_name,
)
from core.protocol import ThemeCore
from core.theme_core import read_config_params
from core.theme_schema import FIELD_BY_ID, SECTIONS


# ──────────────────────────────────────────────────────────────
# Local async helper (kept private to avoid a package import cycle)
# ──────────────────────────────────────────────────────────────

# Keep helpers alive independently of the page/controller until the runnable
# has released its Qt wrapper. Owner destruction suppresses delivery, not work.
_active_jobs: set["_Signals"] = set()


class _Signals(QObject):
    finished = Signal(bool, str)

    def __init__(self, owner: QObject, done: Callable[[bool, str], None]) -> None:
        super().__init__()
        self.exited = Event()
        self._done = done
        self._result: tuple[bool, str] = (False, "")
        self._timer = QTimer(self)
        self._timer.setInterval(0)
        self._timer.timeout.connect(self._release)
        self.finished.connect(self._finished)
        owner.destroyed.connect(self._owner_destroyed)

    @Slot()
    def _owner_destroyed(self) -> None:
        self._done = None

    @Slot(bool, str)
    def _finished(self, success: bool, message: str) -> None:
        self._result = success, message
        self._timer.start()

    @Slot()
    def _release(self) -> None:
        if not self.exited.is_set():
            return
        self._timer.stop()
        done, self._done = self._done, None
        try:
            if done is not None:
                done(*self._result)
        finally:
            _active_jobs.discard(self)
            self.deleteLater()


class _Worker(QRunnable):
    def __init__(self, func: Callable[[], tuple[bool, str]], signals: _Signals) -> None:
        super().__init__()
        self._func = func
        self._signals = signals
        self._exited = signals.exited

    def run(self) -> None:
        try:
            try:
                success, message = self._func()
            except Exception as exc:  # noqa: BLE001 - surface to the UI
                success, message = False, str(exc)
            self._signals.finished.emit(bool(success), message)
        finally:
            # No runnable may retain the QObject wrapper when deleteLater runs.
            self._signals = None
            self._func = None
            self._exited.set()


def _run(owner: QObject, func: Callable[[], tuple[bool, str]],
         done: Callable[[bool, str], None]) -> None:
    signals = _Signals(owner, done)
    _active_jobs.add(signals)
    QThreadPool.globalInstance().start(_Worker(func, signals))


# ──────────────────────────────────────────────────────────────
# Controller
# ──────────────────────────────────────────────────────────────

class ThemeInspectorController(QObject):
    """Тема: обзор, разделы, валидация и live-правки."""

    sectionsChanged = Signal()
    fieldsChanged = Signal()
    issuesChanged = Signal()
    scopeChanged = Signal()
    busyChanged = Signal()
    appConfigsChanged = Signal()
    toast = Signal(str, bool)          # message, success

    def __init__(self, parent: Optional[QObject] = None) -> None:
        super().__init__(parent)
        self._core: ThemeCore = CoreFactory.theme_core()
        self._scope = "live"
        self._sections: list[dict] = []
        self._issues: list[dict] = []
        self._app_configs: list[dict] = []
        self._busy = False
        self._accent = ""
        self._modified = 0
        self.refresh()

    # ── properties ────────────────────────────────────────────

    @Property(list, notify=sectionsChanged)
    def sections(self) -> list:
        return self._sections

    @Property(list, notify=issuesChanged)
    def issues(self) -> list:
        return self._issues

    @Property(bool, notify=busyChanged)
    def busy(self) -> bool:
        return self._busy

    @Property(str, notify=scopeChanged)
    def scope(self) -> str:
        return self._scope

    @Property(list, notify=scopeChanged)
    def scopes(self) -> list:
        try:
            names = sorted(
                p.name for p in profiles_root().iterdir()
                if p.is_dir() and not p.is_symlink()
            )
        except OSError:
            names = []
        valid = []
        for name in names:
            try:
                validate_profile_name(name)
            except ValueError:
                continue
            valid.append(name)
        return ["live"] + [f"profile:{name}" for name in valid]

    @Property(list, notify=appConfigsChanged)
    def appConfigs(self) -> list:
        """Дерево «приложение → файл → параметры» (read-only, этап C1)."""
        return self._app_configs

    @Property(str, notify=sectionsChanged)
    def activeProfile(self) -> str:
        try:
            return CoreFactory.profile_core().get_active_profile()
        except (RuntimeError, ValueError):
            return ""

    @Property(str, notify=sectionsChanged)
    def accentColor(self) -> str:
        return self._accent or "#808080"

    @Property(int, notify=sectionsChanged)
    def modifiedCount(self) -> int:
        return self._modified

    @Property(int, notify=issuesChanged)
    def issueCount(self) -> int:
        return len(self._issues)

    @Property(int, notify=sectionsChanged)
    def sessionChanges(self) -> int:
        return self._core.session_changes()

    @Property(int, notify=sectionsChanged)
    def undoDepth(self) -> int:
        return self._core.undo_depth()

    @Property(bool, notify=scopeChanged)
    def readOnly(self) -> bool:
        return self._scope != "live"

    # ── data ──────────────────────────────────────────────────

    @Slot()
    def refresh(self) -> None:
        titles = {section.id: section.title for section in SECTIONS}
        sections: list[dict] = []
        modified = 0

        for section in self._core.sections(self._scope):
            fields = self._core.fields(section.id, self._scope)
            modified += sum(1 for item in fields if item["differs"])
            sections.append({
                "id": section.id,
                "title": section.title,
                "icon": section.icon,
                "description": section.description,
                "fieldCount": section.field_count,
                "summary": section.summary,
                "issueCount": 0,
            })

        issues: list[dict] = []
        if self._scope == "live":
            for issue in self._core.validate():
                item = issue.to_dict()
                field = FIELD_BY_ID.get(issue.field_id)
                item["section"] = field.section if field else ""
                item["sectionTitle"] = titles.get(item["section"], "")
                issues.append(item)

        per_section: dict[str, int] = {}
        for issue in issues:
            per_section[issue["section"]] = per_section.get(issue["section"], 0) + 1
        for section in sections:
            section["issueCount"] = per_section.get(section["id"], 0)

        accent = self._core.get("accent_color", self._scope)
        self._accent = accent["display"] if accent else ""

        self._sections = sections
        self._issues = issues
        self._modified = modified
        self._app_configs = self._build_app_configs()
        self.sectionsChanged.emit()
        self.fieldsChanged.emit()
        self.issuesChanged.emit()
        self.appConfigsChanged.emit()

    def _build_app_configs(self) -> list[dict]:
        """Дерево: отслеживаемое приложение → файлы → строки ключ=значение.

        Флажок ``editable`` — только для live-файлов с поддержанными
        суффиксами (INI/JSON/JSONC/TOML/user.js/line-based).
        """
        try:
            components = CoreFactory.tracked_apps_core().get_all_components()
        except Exception:
            return []
        if self._scope == "live":
            base: Optional[Path] = None
        else:
            try:
                base = profile_dir(self._scope.split(":", 1)[1])
            except (IndexError, ValueError):
                return []
        entries: list[dict] = []
        for component in components:
            files: list[dict] = []
            for rel in component.config_paths:
                path = live_path(rel) if base is None else base / rel
                from core.cli_core import core_request
                parsed = core_request("config.describe", file=str(path))
                if parsed is None:
                    continue
                editable = base is None and parsed["editable"]
                files.append({
                    "relPath": rel,
                    "params": parsed["params"],
                    "truncated": parsed.get("truncated", False),
                    "note": parsed.get("note", ""),
                    "editable": editable,
                })
            if files:
                entries.append({
                    "id": component.id,
                    "name": component.name,
                    "icon": component.icon,
                    "files": files,
                })
        return entries

    @Slot(str, str, str)
    def setAppConfigValue(self, rel_path: str, key: str, value: str) -> None:
        """Записать ключ конфига приложения (этапы C2/C3; только live-scope)."""
        if self._scope != "live":
            self.toast.emit(self.tr("Switch to the Live scope to edit"), False)
            return

        def work() -> tuple[bool, str]:
            result = self._core.set_config_value(rel_path, key, value)
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self.toast.emit(message, success)
            if success:
                self.refresh()

        _run(self, work, done)

    @Slot(str, int, result=list)
    def appConfigParams(self, rel_path: str, limit: int) -> list:
        """Перечитать параметры уже показанного файла с другим лимитом."""
        known = any(
            item.get("relPath") == rel_path
            for app in self._app_configs
            for item in app.get("files", [])
        )
        if not known:
            return []
        if self._scope == "live":
            path = live_path(rel_path)
        else:
            try:
                path = profile_dir(self._scope.split(":", 1)[1]) / rel_path
            except (IndexError, ValueError):
                return []
        parsed = read_config_params(path, limit=max(1, limit))
        return parsed["params"] if parsed else []

    @Slot(str, result=list)
    def fieldsFor(self, section: str) -> list:
        return self._core.fields(section, self._scope)

    @Slot(str, result=list)
    def search(self, query: str) -> list:
        return self._core.search(query, self._scope)

    @Slot(result=list)
    def fontFamilies(self) -> list:
        return self._core.font_families()

    @Slot(str, result=str)
    def sectionTitle(self, section: str) -> str:
        for item in SECTIONS:
            if item.id == section:
                return item.title
        return section

    # ── scope ─────────────────────────────────────────────────

    @Slot(str)
    def setScope(self, scope: str) -> None:
        if scope != "live":
            if not scope.startswith("profile:"):
                return
            try:
                validate_profile_name(scope.split(":", 1)[1])
            except ValueError:
                self.toast.emit("Invalid profile name", False)
                return
        if scope == self._scope:
            return
        self._scope = scope
        self.scopeChanged.emit()
        self.refresh()

    # ── writes ────────────────────────────────────────────────

    def _set_busy(self, value: bool) -> None:
        if self._busy != value:
            self._busy = value
            self.busyChanged.emit()

    def _write(self, field_id: str, value: object) -> None:
        if not self._can_edit_live():
            return

        def work() -> tuple[bool, str]:
            result = self._core.set_value(field_id, value)
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            if not success:
                self.toast.emit(message, False)
            # Успех без тоста: застейдженное значение видно в поле, а кнопка
            # Apply/Revert (sessionChanges) показывает, что ждёт применения.
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    @Slot()
    def applyChanges(self) -> None:
        """Применить все застейдженные правки одним проходом."""
        if not self._can_edit_live():
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

    @Slot(str, str)
    def setText(self, field_id: str, value: str) -> None:
        self._write(field_id, value)

    @Slot(str, bool)
    def setBool(self, field_id: str, value: bool) -> None:
        self._write(field_id, value)

    @Slot(str, int)
    def setInt(self, field_id: str, value: int) -> None:
        self._write(field_id, value)

    @Slot(str, float)
    def setFloat(self, field_id: str, value: float) -> None:
        self._write(field_id, value)

    @Slot(str, str)
    def setColor(self, field_id: str, value: str) -> None:
        self._write(field_id, value)

    @Slot(str, str, int)
    def setFont(self, field_id: str, family: str, size: int) -> None:
        self._write(field_id, {"family": family, "size": size})

    @Slot(str)
    def resetToProfile(self, field_id: str) -> None:
        if not self._can_edit_live():
            return

        def work() -> tuple[bool, str]:
            result = self._core.reset(field_id, "profile")
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            self.toast.emit(message, success)
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    def _can_edit_live(self) -> bool:
        if self._scope != "live":
            self.toast.emit(self.tr("Profile snapshots are read-only"), False)
            return False
        if self._busy:
            self.toast.emit(self.tr("Another change is still being applied"), False)
            return False
        return True

    @Slot()
    def undo(self) -> None:
        if not self._can_edit_live():
            return

        def work() -> tuple[bool, str]:
            result = self._core.undo()
            return result.success, result.message
        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            self.toast.emit(message, success)
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    @Slot()
    def revertAll(self) -> None:
        if not self._can_edit_live():
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
    def applyFix(self, field_id: str) -> None:
        """Pick a safe value for a validation issue and write it live."""
        if self._scope != "live":
            self.toast.emit(self.tr("Profile snapshots are read-only"), False)
            return

        if field_id == "blur_enabled":
            value: object = False
            target = "legacy_blur"
        else:
            field = FIELD_BY_ID.get(field_id)
            if field is None:
                return
            target = field_id
            value = None
            installed = [choice.value for choice in self._core.choices(field_id)]
            active = self.activeProfile
            profile = self._core.get(field_id, f"profile:{active}") if active else None
            profile_value = profile["value"] if profile else None
            if profile_value in installed:
                value = profile_value
            elif installed:
                value = installed[0]
            elif field.type == "font":
                value = {"family": "Noto Sans", "size": 10}
            elif field.type == "bool":
                value = False

        if value is None:
            self.toast.emit(self.tr("No safe fix for %1").replace("%1", field_id), False)
            return

        def work() -> tuple[bool, str]:
            staged = self._core.set_value(target, value)
            if not staged.success:
                return False, staged.message
            # Fix — осознанное действие: применяем сразу. Внимание: apply()
            # коммитит и прочие застейдженные правки (как кнопка Apply).
            applied = self._core.apply()
            return applied.success, applied.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            self.toast.emit(message, success)
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    @Slot(str)
    def exportJson(self, path: str) -> None:
        """Write the current scope to a JSON snapshot (raw + display values)."""
        if not path:
            return
        local = QUrl(path).toLocalFile() if path.startswith("file:") else path
        target = Path(local)
        data = {
            "generatedAt": datetime.now().isoformat(timespec="seconds"),
            "scope": self._scope,
            "activeProfile": self.activeProfile,
            "sections": {},
        }
        for section in self._sections:
            data["sections"][section["id"]] = {
                item["id"]: {
                    "value": item["value"],
                    "display": item["display"],
                }
                for item in self._core.fields(section["id"], self._scope)
            }
        try:
            from core.cli_core import core_request
            core_request("report.export", file=str(target), data=data)
        except (OSError, RuntimeError, ValueError) as exc:
            self.toast.emit(f"Export failed: {exc}", False)
            return
        self.toast.emit(f"Exported theme snapshot to {target}", True)

    # ── external tools ────────────────────────────────────────

    @Slot(str)
    def openSystemSettings(self, module: str) -> None:
        if not module:
            return
        try:
            subprocess.Popen(["kcmshell6", module])
        except OSError as exc:
            self.toast.emit(str(exc), False)

    @Slot(str)
    def openFile(self, relative: str) -> None:
        if not relative:
            return
        path = Path(relative)
        if not path.is_absolute():
            path = live_path(relative)
        target = path if path.is_dir() else path.parent
        # Audit trail: users report Dolphin "opening by itself" — log every
        # launch so the trigger can be traced from the journal/console.
        print(
            f"[themeInspector] openFile: {relative!r} -> xdg-open {target}",
            file=sys.stderr, flush=True,
        )
        try:
            subprocess.Popen(["xdg-open", str(target)])
        except OSError as exc:
            self.toast.emit(str(exc), False)


__all__ = ["ThemeInspectorController"]
