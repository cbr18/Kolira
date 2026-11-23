"""QML controller for the high-level theme bundle view."""

from __future__ import annotations

from typing import Optional

from PySide6.QtCore import Property, QObject, QUrl, Signal, Slot

from core import CoreFactory
from core.core_messages import localize, render_message
from core.paths import validate_profile_name
from core.theme_bundle import ThemeBundleCore

from .theme_inspector import _run


_BASE16 = "base16_palette"
_APP_TARGETS = "app_targets"


def _reload_app_look() -> None:
    """Show what the CLI just applied in this window too (late import: QML registers it)."""
    from theme_controller import reload_app_look

    reload_app_look()


class ThemeBundleController(QObject):
    """Theme → component groups, application adapters and profile bundles."""

    groupsChanged = Signal()
    applicationsChanged = Signal()
    profilesChanged = Signal()
    scopeChanged = Signal()
    profileDraftsChanged = Signal()
    profileEditsSaved = Signal()
    resourcePreviewsChanged = Signal()
    busyChanged = Signal()
    storeChanged = Signal()
    conflictsFound = Signal(list)
    liveApplyFinished = Signal(bool)
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
        self._profile_editor: dict = {}
        self._profile_drafts: dict[str, tuple[dict, dict]] = {}
        self._resource_previews = {}
        self._resource_requests = {}
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

    @Property("QVariantMap", notify=resourcePreviewsChanged)
    def resourcePreviews(self) -> dict:
        return self._resource_previews

    @Slot(str, list)
    def requestResourcePreviews(self, component: str, options: list) -> None:
        from core.cli_core import core_request
        scope = self._scope
        key = f"{scope}/{component}"
        names = tuple(dict.fromkeys(str(value) for value in options if value))
        if not names or self._resource_requests.get(key) == names:
            return
        self._resource_requests[key] = names
        payload = {}
        def work():
            previews = core_request("bundle.resource_previews", id=component, options=names,
                                    profile=scope.removeprefix("profile:") if scope != "live" else None)
            for name, asset in previews.items():
                url = QUrl.fromLocalFile(asset["path"])
                url.setQuery(f'rev={asset["revision"]}')
                payload[name] = url
            return True, ""
        def done(ok, _message):
            if self._resource_requests.get(key) != names:
                return
            self._resource_requests.pop(key, None)
            if ok and self._scope == scope:
                self._resource_previews = {**self._resource_previews, key: payload}
                self.resourcePreviewsChanged.emit()
        _run(self, work, done)

    @Property(str, notify=scopeChanged)
    def scope(self) -> str:
        return self._scope

    @Property(str, notify=profilesChanged)
    def activeProfile(self) -> str:
        try:
            return CoreFactory.profile_core().get_active_profile()
        except (RuntimeError, ValueError):
            return ""

    @Property(bool, notify=scopeChanged)
    def readOnly(self) -> bool:
        return self._scope != "live"

    @Property(bool, notify=scopeChanged)
    def canEditProfile(self) -> bool:
        return bool(self._profile_editor)

    @Property(int, notify=profileDraftsChanged)
    def profileDraftSize(self) -> int:
        draft = self._profile_drafts.get(self._scope.removeprefix("profile:")) if self.readOnly else None
        return len(draft[1]) if draft else 0

    def _profile_group_keys(self, group: dict) -> set[str]:
        keys = {row["id"] for row in group["components"]}
        if group["id"] == "palette":
            keys.add(_BASE16)
        elif group["id"] == "applications":
            keys.add(_APP_TARGETS)
        return keys

    @Property(list, notify=profileDraftsChanged)
    def profileDraftGroups(self) -> list:
        if not self.readOnly:
            return []
        draft = self._profile_drafts.get(self._scope.removeprefix("profile:"))
        changes = draft[1] if draft else {}
        return [group["id"] for group in self._groups if self._profile_group_keys(group).intersection(changes)]

    @Property("QVariantMap", notify=profileDraftsChanged)
    def profilePalette(self) -> dict:
        base = self._profile_editor.get("values", {}).get(_BASE16)
        draft = self._profile_drafts.get(self._scope.removeprefix("profile:"))
        return ((draft[1].get(_BASE16, base) if draft else base) or {}) if self.readOnly else {}

    @Property(list, notify=profileDraftsChanged)
    def profilePalettePresets(self) -> list:
        return self._profile_editor.get("palettePresets", [])

    @Property(bool, notify=profileDraftsChanged)
    def profilePaletteChanged(self) -> bool:
        draft = self._profile_drafts.get(self._scope.removeprefix("profile:"))
        return self.readOnly and bool(draft and _BASE16 in draft[1])

    @Property(list, notify=profileDraftsChanged)
    def profileAppTargets(self) -> list:
        return self._profile_editor.get("appTargets", [])

    @Property(list, notify=profileDraftsChanged)
    def profileSelectedAppTargets(self) -> list:
        if not self.profilePalette:
            return []
        base = self._profile_editor.get("values", {}).get(_APP_TARGETS, [])
        draft = self._profile_drafts.get(self._scope.removeprefix("profile:"))
        return draft[1].get(_APP_TARGETS, base) if draft else base

    @Property(list, notify=profileDraftsChanged)
    def profileDraft(self) -> list:
        draft = self._profile_drafts.get(self._scope.removeprefix("profile:")) if self.readOnly else None
        if not draft:
            return []
        editor, changes = draft
        rows = [{"label": row["label"],
                 "before": self._profile_value_label(row["id"], editor["values"][row["id"]]),
                 "after": self._profile_value_label(row["id"], changes[row["id"]])}
                for group in self._groups for row in group["components"] if row["id"] in changes]
        if _BASE16 in changes:
            before = editor["values"].get(_BASE16)
            after = changes[_BASE16]
            label = self.tr("Base16 palette")
            if before and after:
                entries = [(f"base{index:02X}", left, right)
                           for index, (left, right) in enumerate(zip(before["base"], after["base"]))]
                entries.append((self.tr("Accent color"), before["accent"], after["accent"]))
                rows.extend({"label": f"{label} · {slot}", "before": left, "after": right}
                            for slot, left, right in entries if left != right)
                if before["variant"] != after["variant"] or not any(left != right for _, left, right in entries):
                    rows.append({"label": label, "before": self._palette_label(before), "after": self._palette_label(after)})
            else:
                rows.append({"label": label, "before": self._palette_label(before), "after": self._palette_label(after)})
        before_targets = set(editor["values"].get(_APP_TARGETS, []))
        after_targets = set(self.profileSelectedAppTargets)
        for target in self.profileAppTargets:
            id_ = target["id"]
            if (id_ in before_targets) != (id_ in after_targets):
                rows.append({"label": self.tr("Base16 · %1").replace("%1", target["name"]),
                             "before": render_message("On" if id_ in before_targets else "Off"),
                             "after": render_message("On" if id_ in after_targets else "Off")})
        return rows

    def _palette_label(self, palette: Optional[dict]) -> Optional[str]:
        if palette is None:
            return None
        variant = render_message("Dark" if palette["variant"] == "dark" else "Light")
        return f"{palette['name']} · {variant}"

    def _profile_value_label(self, key: str, value: object) -> object:
        if isinstance(value, bool):
            return render_message("On" if value else "Off")
        if key in ("obsidian_mode", "obsidian_theme") and value == {"mixed": True}:
            return self.tr("Varies by vault")
        options = self._profile_editor.get("choices", {}).get(key, ())
        selected = next((option for option in options if option["value"] == value), None)
        return self._choice_label(key, selected) if selected else value

    def _choice_label(self, key: str, option: dict) -> str:
        value = option.get("value")
        if isinstance(value, dict) and value.get("disabled") is True:
            return self.tr("No decoration")
        if key == "obsidian_theme" and value == "":
            return self.tr("Default Obsidian theme")
        if key in ("zed_mode", "zen_mode", "obsidian_mode") and option["available"]:
            return render_message(option["label"])
        return option["label"]

    @Property(bool, notify=profileDraftsChanged)
    def hasProfileDrafts(self) -> bool:
        return bool(self._profile_drafts)

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
                self.toast.emit(localize(message), False)

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
            self.toast.emit(localize(message), success)
            self.refresh()

        self._set_store(busy=True)
        _run(self, work, done)

    # ── refresh ──────────────────────────────────────────────────────────

    @Slot()
    def refresh(self) -> None:
        self._groups = self._core.groups(self._scope)
        self._overlay_profile_edits()
        self._applications = self._core.applications()
        self._profiles = self._core.profiles()
        self.groupsChanged.emit()
        self.applicationsChanged.emit()
        self.profilesChanged.emit()

    def _overlay_profile_edits(self) -> None:
        if not self._profile_editor:
            return
        saved = self._profile_editor["values"]
        assets = self._profile_editor.get("bundled", {})
        editor_choices = self._profile_editor.get("choices", {})
        draft = self._profile_drafts.get(self._scope.removeprefix("profile:"))
        changes = draft[1] if draft else {}
        for group in self._groups:
            for row in group["components"]:
                key = row["id"]
                if key not in saved:
                    continue
                value = changes.get(key, saved[key])
                choices = row["choices"]
                if key in editor_choices:
                    choices = [{**option, "label": self._choice_label(key, option)} for option in editor_choices[key]]
                    missing = any(option["value"] == value and not option["available"] for option in choices)
                    row.update(choices=choices, supported=bool(choices), status="warning" if missing else "ok",
                               statusText=self.tr("Selected theme is not installed") if missing else "")
                elif row["kind"] == "choice":
                    installed = set(row["installedChoices"])
                    bundled = set(assets.get(key, ()))
                    choices = [option for option in choices if option["value"] in installed]
                    choices.extend({"value": name, "label": name} for name in sorted(bundled - installed))
                    missing = value is not None and value not in installed and value not in bundled
                    if missing:
                        choices.append({"value": value, "label": value})
                    row.update(choices=choices, supported=bool(choices), status="warning" if missing else "ok",
                               statusText=self.tr("Selected theme is not installed") if missing else "")
                choice_index = next((index for index, option in enumerate(choices) if option["value"] == value), -1)
                choice = choices[choice_index] if choice_index >= 0 else None
                display = choice["label"] if choice else self._profile_value_label(key, value)
                row.update(profileEditable=True, staged=key in changes, included=value is not None,
                           currentValue=value, currentChoiceIndex=choice_index,
                           currentDisplay="" if display is None else str(display))

    def _set_busy(self, value: bool) -> None:
        if self._busy != value:
            self._busy = value
            self.busyChanged.emit()

    # ── scope and profiles ───────────────────────────────────────────────

    @Slot()
    def openLive(self) -> None:
        if self._busy:
            return
        self._scope = "live"
        self._profile_bundle = {}
        self._profile_editor = {}
        self.scopeChanged.emit()
        self.profileDraftsChanged.emit()
        self.refresh()

    @Slot(str)
    def openProfile(self, name: str) -> None:
        if self._busy:
            return
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            self.toast.emit(str(exc), False)
            return
        result = self._core.profile_bundle(name)
        if not result.success:
            self.toast.emit(localize(result.message), False)
            return
        self._scope = f"profile:{name}"
        self._profile_bundle = result.data or {}
        draft = self._profile_drafts.get(name)
        self._profile_editor = draft[0] if draft else self._core.profile_editor(name).data or {}
        self.scopeChanged.emit()
        self.refresh()
        self.profileDraftsChanged.emit()

    @Slot()
    def migrateProfile(self) -> None:
        if self._busy or not self.readOnly or not self._profile_bundle.get("legacy"):
            return
        name = self._scope.removeprefix("profile:")
        def work() -> tuple[bool, str]:
            result = self._core.migrate_profile(name)
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            if success:
                if self._scope == f"profile:{name}":
                    self.openProfile(name)
                self.profileEditsSaved.emit()
                self.toast.emit(self.tr("Theme is ready to edit."), True)
            else:
                self.toast.emit(localize(message), False)

        self._set_busy(True)
        _run(self, work, done)

    @Slot(str)
    @Slot(str, str)
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
            self.toast.emit(localize(message), success)
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
            if success:
                _reload_app_look()
            self.toast.emit(localize(message), success)
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
        if self.readOnly:
            self._stage_profile_edit(component_id, value)
            return
        result = self._core.set_component(component_id, value)
        self.toast.emit(localize(result.message), result.success)
        self.refresh()

    @Slot(str)
    def excludeProfileComponent(self, component_id: str) -> None:
        if self.readOnly and not self._busy:
            self._stage_profile_edit(component_id, None)

    @Slot(str, str)
    def stageApplication(self, app_id: str, theme: str) -> None:
        self._stage_live(lambda: self._core.stage_application(app_id, theme))

    def _stage_live(self, stage) -> None:
        if self._busy or self.readOnly:
            return

        def work() -> tuple[bool, str]:
            result = stage()
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            self.refresh()
            self.toast.emit(localize(message), success)

        self._set_busy(True)
        _run(self, work, done)

    @Slot(int)
    def chooseProfilePalette(self, index: int) -> None:
        if not self.readOnly or self._busy or not 0 <= index < len(self.profilePalettePresets):
            return
        preset = dict(self.profilePalettePresets[index])
        previous = self.profilePalette or self._profile_editor["values"].get(_BASE16)
        if previous:
            # Keep the stable KDE and Konsole resource IDs of this theme.
            preset["name"] = previous["name"]
        self._stage_profile_edit(_BASE16, preset)

    @Slot(str, str)
    def setProfilePaletteColor(self, slot: str, color: str) -> None:
        if not self.readOnly or self._busy or not self.profilePalette:
            return
        palette = dict(self.profilePalette)
        if slot == "accent":
            palette[slot] = color
        else:
            slots = [f"base{index:02X}" for index in range(16)]
            if slot not in slots:
                return
            palette["base"] = list(palette["base"])
            palette["base"][slots.index(slot)] = color
        self._stage_profile_edit(_BASE16, palette)

    @Slot(str, bool)
    def setProfileAppTarget(self, app_id: str, enabled: bool) -> None:
        if self._busy or not self.profilePalette or not any(t["id"] == app_id for t in self.profileAppTargets):
            return
        selected = set(self.profileSelectedAppTargets)
        if enabled:
            selected.add(app_id)
        else:
            selected.discard(app_id)
        self._stage_profile_edit(_APP_TARGETS, sorted(selected))

    def _stage_profile_edit(self, component_id: str, value: object) -> None:
        if hasattr(value, "toVariant"):
            value = value.toVariant()
        base = self._profile_editor.get("values", {})
        if component_id not in base:
            return
        name = self._scope.removeprefix("profile:")
        _, changes = self._profile_drafts.get(name, (self._profile_editor, {}))
        original = base[component_id]
        if component_id == _APP_TARGETS:
            original = sorted(original)
        if value == original:
            changes.pop(component_id, None)
        else:
            changes[component_id] = value
        if changes:
            self._profile_drafts[name] = self._profile_editor, changes
        else:
            self._profile_drafts.pop(name, None)
        self._overlay_profile_edits()
        self.groupsChanged.emit()
        self.profileDraftsChanged.emit()

    @Slot()
    def discardProfileDraft(self) -> None:
        if self._busy or not self.readOnly:
            return
        name = self._scope.removeprefix("profile:")
        self._profile_drafts.pop(name, None)
        self._profile_editor = self._core.profile_editor(name).data or {}
        self.scopeChanged.emit()
        self.profileDraftsChanged.emit()
        self.refresh()

    @Slot(str)
    def discardProfileGroup(self, group_id: str) -> None:
        if self._busy or not self.readOnly:
            return
        name = self._scope.removeprefix("profile:")
        draft = self._profile_drafts.get(name)
        group = next((group for group in self._groups if group["id"] == group_id), None)
        if not draft or group is None:
            return
        editor, changes = draft
        fields = self._profile_group_keys(group)
        keep = {key: value for key, value in changes.items() if key not in fields}
        if keep:
            self._profile_drafts[name] = editor, keep
        else:
            self._profile_drafts.pop(name)
        self._overlay_profile_edits()
        self.groupsChanged.emit()
        self.profileDraftsChanged.emit()

    @Slot()
    def saveProfileDraft(self) -> None:
        if self._busy or not self.profileDraftSize:
            return
        name = self._scope.removeprefix("profile:")
        editor, changes = self._profile_drafts[name]
        pending = dict(changes)
        if _BASE16 in pending and pending[_BASE16] is None:
            pending.pop(_APP_TARGETS, None)
        payload = {}

        def work() -> tuple[bool, str]:
            result = self._core.save_profile_edits(name, editor["revision"], pending)
            payload["editor"] = result.data
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            if success:
                self._profile_drafts.pop(name, None)
                self._profile_editor = payload["editor"]
                self._overlay_profile_edits()
                self.groupsChanged.emit()
                self.profileDraftsChanged.emit()
                self.profileEditsSaved.emit()
                message = self.tr("Theme saved")
            elif message == "Theme changed on disk. Reopen it before saving.":
                message = self.tr("The theme changed on disk. You can review the draft or discard it to load the current theme.")
            else:
                message = localize(message)
            self.toast.emit(message, success)

        self._set_busy(True)
        _run(self, work, done)

    @Slot()
    def applyChanges(self) -> None:
        if self._busy:
            self.toast.emit(self.tr("Another operation is still running"), False)
            return

        payload = {}

        def work() -> tuple[bool, str]:
            result = self._core.apply(check_live=True)
            payload["conflicts"] = (result.data or {}).get("conflicts", [])
            return result.success, result.message

        def done(success: bool, message: str) -> None:
            self._set_busy(False)
            if success:
                _reload_app_look()
            self.refresh()
            if payload.get("conflicts"):
                self.conflictsFound.emit(payload["conflicts"])
            else:
                self.liveApplyFinished.emit(success)
                self.toast.emit(localize(message), success)

        self._set_busy(True)
        _run(self, work, done)

    @Slot()
    def undo(self) -> None:
        result = self._core.undo()
        self.toast.emit(localize(result.message), result.success)
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
            self.toast.emit(localize(message), success)
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
            self.toast.emit(localize(message), success)
            self.refresh()

        self._set_busy(True)
        _run(self, work, done)

    @Slot(QUrl)
    def stageWallpaper(self, url: QUrl) -> None:
        local = url.toLocalFile()
        if local:
            self._stage_live(lambda: self._core.stage_wallpaper(local))

    # ── external actions ─────────────────────────────────────────────────

    @Slot()
    @Slot(str)
    def openSystemSettings(self, module: str = "") -> None:
        result = self._core.open_system_settings(module)
        if not result.success:
            self.toast.emit(localize(result.message), False)

    @Slot(str)
    def launchApp(self, app_id: str) -> None:
        result = self._core.launch_app(app_id)
        self.toast.emit(localize(result.message), result.success)

    @Slot(str)
    def openAppConfig(self, app_id: str) -> None:
        result = self._core.open_app_config(app_id)
        self.toast.emit(localize(result.message), result.success)

    @Slot(str)
    def openAppThemeSettings(self, app_id: str) -> None:
        result = self._core.open_app_theme_settings(app_id)
        self.toast.emit(localize(result.message), result.success)



__all__ = ["ThemeBundleController"]
