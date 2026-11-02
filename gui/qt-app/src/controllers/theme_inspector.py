"""Controller for the Theme Inspector screen.

The inspector reads live KDE/app configuration through ``ThemeCore`` and exposes
QML-ready dicts. Writes are always applied to the *live* system and run on the
thread pool; profile snapshots stay read-only.
"""

from __future__ import annotations

import json
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Callable, Optional

from PySide6.QtCore import (
    Property,
    QObject,
    QRunnable,
    QThreadPool,
    QUrl,
    Signal,
    Slot,
)

from core import CoreFactory
from core.paths import (
    active_profile_file,
    live_path,
    profile_dir,
    profiles_dir as profiles_root,
    validate_profile_name,
)
from core.protocol import ThemeCore
from core.theme_core import _UNSUPPORTED_SUFFIXES, read_config_params
from core.theme_schema import FIELD_BY_ID, SECTIONS


# ──────────────────────────────────────────────────────────────
# Local async helper (kept private to avoid a package import cycle)
# ──────────────────────────────────────────────────────────────

class _Signals(QObject):
    finished = Signal(bool, str)


class _Worker(QRunnable):
    def __init__(self, func: Callable[[], tuple[bool, str]], signals: _Signals) -> None:
        super().__init__()
        self._func = func
        self._signals = signals

    def run(self) -> None:
        try:
            success, message = self._func()
        except Exception as exc:  # noqa: BLE001 - surface to the UI
            success, message = False, str(exc)
        self._signals.finished.emit(bool(success), message)


def _run(owner: QObject, func: Callable[[], tuple[bool, str]],
         done: Callable[[bool, str], None]) -> None:
    signals = _Signals(owner)
    signals.finished.connect(done)
    # The worker keeps a Python reference to ``signals``.  Calling
    # deleteLater() here deleted the C++ object while the worker still held the
    # wrapper, which produced a SIGBUS in QObject::~QObject.  Parent ownership
    # (passed to _Signals) keeps the object alive for the controller.
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
            name = active_profile_file().read_text(encoding="utf-8").strip()
            return validate_profile_name(name)
        except (OSError, ValueError):
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
        editable_suffixes = {".json", ".jsonc", ".toml", ".js"}
        blocked_suffixes = set(_UNSUPPORTED_SUFFIXES)
        for component in components:
            files: list[dict] = []
            for rel in component.config_paths:
                path = live_path(rel) if base is None else base / rel
                if not path.is_file():
                    continue
                parsed = read_config_params(path)
                suffix = path.suffix.lower()
                if parsed is None:
                    files.append({"relPath": rel, "params": [], "truncated": False,
                                  "note": "unsupported format", "editable": False})
                    continue
                editable = (
                    base is None  # только live-scope редактируется
                    and parsed["note"] != "parse error"
                    and (suffix in editable_suffixes or suffix not in blocked_suffixes)
                )
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
            self.toast.emit("Switch to the Live scope to edit", False)
            return

        def work() -> tuple[bool, str]:
            result = self._core.set_config_value(rel_path, key, value)
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self.toast.emit(message, success)
            if success:
                self.refresh()

        _run(self, work, done)

    @Slot(str, result=list)
    def fieldsFor(self, section: str) -> list:
        return self._core.fields(section, self._scope)

    @Slot(str, result=list)
    def search(self, query: str) -> list:
        return self._core.search(query, self._scope)

    @Slot(result=list)
    def fontFamilies(self) -> list:
        return self._core.font_families()

    @Slot(str, result="QVariantMap")
    def profileSummary(self, name: str) -> dict:
        """Fingerprint of a saved profile for its card in "Available Profiles".

        The profile cards used to repeat the same component chips for every
        profile. The component set is almost always identical, so instead show
        what actually distinguishes a theme: its palette, colour scheme,
        Kvantum/Plasma assets, corner radius, blur and terminal opacity.
        """
        if not name:
            return {}
        try:
            validate_profile_name(name)
            root = profile_dir(name)
        except ValueError:
            return {}

        scope = f"profile:{name}"

        def field(field_id: str) -> dict:
            try:
                return self._core.get(field_id, scope) or {}
            except Exception:  # noqa: BLE001 - a card must never break the UI
                return {}

        def display(field_id: str) -> str:
            return str(field(field_id).get("display") or "")

        accent = display("accent_color")
        swatches = [
            color
            for color in (
                display("window_bg"),
                display("view_bg"),
                accent,
                display("text_color"),
            )
            if color
        ]

        # A broken or unreadable profile should still render a card; the stats
        # are a nicety, not a requirement.
        size_bytes = 0
        file_count = 0
        try:
            for entry in root.rglob("*"):
                try:
                    if entry.is_file():
                        file_count += 1
                        size_bytes += entry.stat().st_size
                except OSError:
                    continue
        except OSError:
            pass

        return {
            "accent": accent,
            "swatches": swatches,
            "scheme": display("color_scheme"),
            "lookAndFeel": display("look_and_feel"),
            "plasmaTheme": display("plasma_theme"),
            "kvantum": display("kvantum_theme"),
            "radius": display("klassy_radius"),
            "blur": display("blur_strength"),
            "blurEnabled": bool(field("blur_enabled").get("value")),
            "kitty": display("kitty_opacity"),
            "iconTheme": display("icon_theme"),
            "gtkTheme": display("gtk_theme"),
            "decoration": display("decoration_theme"),
            "sizeBytes": size_bytes,
            "fileCount": file_count,
        }

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
            self.toast.emit("Profile snapshots are read-only", False)
            return False
        if self._busy:
            self.toast.emit("Another change is still being applied", False)
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
            self.toast.emit("Profile snapshots are read-only", False)
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
            self.toast.emit(f"No safe fix for {field_id}", False)
            return
        self._write(target, value)

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
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(
                json.dumps(data, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
        except OSError as exc:
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
