"""High-level theme bundle core.

``AppThemeCore`` knows how to read and write individual curated fields. This
module adds the product-level view on top of it: installed KDE/Plasma assets,
desktop parameters, Kvantum/GTK choices, per-app adapters, profile bundle
metadata and user-created derived themes.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Optional

from .app_theme_adapters import ADAPTERS, adapter_for, backup_file
from .fileutil import atomic_write_text
from .paths import (
    active_profile_file,
    data_home,
    home_dir,
    live_path,
    profile_dir,
    profiles_dir,
    validate_profile_name,
)
from .protocol import ProfileCore, Result, ThemeCore, WallpaperCore
from .theme_core import AppThemeCore, _read_text, _set_ini_text


BUNDLE_SCHEMA = "theme-bundle/1"


@dataclass(frozen=True, slots=True)
class BundleComponentDef:
    id: str
    label: str
    field_id: str
    kind: str = "choice"
    icon: str = ""
    description: str = ""
    external_module: str = ""
    legacy_section: str = ""
    profile_tracked: bool = True


@dataclass(frozen=True, slots=True)
class BundleGroupDef:
    id: str
    label: str
    icon: str
    description: str
    components: tuple[BundleComponentDef, ...]


GROUPS: tuple[BundleGroupDef, ...] = (
    BundleGroupDef(
        "plasma", "KDE & Plasma", "preferences-desktop-theme",
        "Global theme, Plasma Style, colors, decorations, icons, cursors and fonts",
        (
            BundleComponentDef(
                "plasma_global", "Global theme", "look_and_feel",
                icon="preferences-desktop-theme-global",
                description="Look & Feel package used by Plasma",
                external_module="kcm_lookandfeel",
                legacy_section="assets",
            ),
            BundleComponentDef(
                "plasma_style", "Plasma Style", "plasma_theme",
                icon="preferences-desktop-theme",
                description="Visual style of Plasma widgets and panels",
                external_module="kcm_desktoptheme",
                legacy_section="assets",
            ),
            BundleComponentDef(
                "color_scheme", "Color scheme", "color_scheme",
                icon="preferences-desktop-color",
                description="Installed KDE color scheme",
                external_module="kcm_colors",
                legacy_section="colors",
            ),
            BundleComponentDef(
                "decoration_theme", "Window decoration", "decoration_theme",
                icon="preferences-system-windows",
                description="Theme used by the selected window decoration plugin",
                external_module="kcm_kwindecoration",
                legacy_section="windows",
            ),
            BundleComponentDef(
                "icon_theme", "Icon theme", "icon_theme",
                icon="preferences-desktop-icons",
                description="Icon theme for Plasma and Qt applications",
                external_module="kcm_icons",
                legacy_section="assets",
            ),
            BundleComponentDef(
                "cursor_theme", "Cursor theme", "cursor_theme",
                icon="preferences-desktop-cursors",
                description="Mouse cursor theme",
                external_module="kcm_cursortheme",
                legacy_section="assets",
                profile_tracked=False,
            ),
            BundleComponentDef(
                "ui_font", "Fonts", "ui_font",
                kind="font",
                icon="preferences-desktop-font",
                description="UI, menu, title and fixed fonts",
                external_module="kcm_fonts",
                legacy_section="typography",
            ),
        ),
    ),
    BundleGroupDef(
        "desktop", "Desktop & Windows", "preferences-system-windows",
        "Wallpaper, corners, blur, translucency and window effects",
        (
            BundleComponentDef(
                "wallpaper", "Wallpaper", "",
                kind="wallpaper",
                icon="preferences-desktop-wallpaper",
                description="Wallpaper used by the current Plasma session",
            ),
            BundleComponentDef(
                "corner_radius", "Corner radius", "klassy_radius",
                kind="int",
                icon="rounded",
                description="Window corner radius used by Klassy",
                legacy_section="windows",
            ),
            BundleComponentDef(
                "blur_strength", "Blur strength", "blur_strength",
                kind="int",
                icon="blur",
                description="Better Blur DX strength",
                legacy_section="windows",
            ),
            BundleComponentDef(
                "translucency", "Inactive window opacity", "translucency_inactive",
                kind="int",
                icon="translucency",
                description="Opacity of inactive windows",
                legacy_section="windows",
            ),
            BundleComponentDef(
                "animations", "Animation speed", "animation_factor",
                kind="float",
                icon="preferences-desktop-animation",
                description="Global animation duration multiplier",
                legacy_section="windows",
            ),
        ),
    ),
    BundleGroupDef(
        "qt", "Qt & Kvantum", "preferences-desktop-theme",
        "Qt widget style and Kvantum theme",
        (
            BundleComponentDef(
                "widget_style", "Qt widget style", "widget_style",
                icon="preferences-desktop-theme-style",
                description="Qt application widget style",
                external_module="kcm_style",
                legacy_section="colors",
            ),
            BundleComponentDef(
                "kvantum_theme", "Kvantum theme", "kvantum_theme",
                icon="kvantum",
                description="SVG widget theme used by the Kvantum Qt style",
                external_module="kcm_style",
                legacy_section="assets",
            ),
            BundleComponentDef(
                "kvantum_translucent", "Kvantum translucent windows", "kvantum_translucent",
                kind="bool",
                icon="translucency",
                description="Let Kvantum paint translucent windows",
                legacy_section="assets",
            ),
        ),
    ),
    BundleGroupDef(
        "gtk", "GTK", "gtk",
        "GTK 2, GTK 3 and GTK 4 application themes",
        (
            BundleComponentDef(
                "gtk_theme", "GTK theme", "gtk_theme",
                icon="gtk",
                description="Theme used by GTK applications",
                legacy_section="assets",
            ),
            BundleComponentDef(
                "gtk_dark", "Prefer dark GTK variant", "gtk_dark",
                kind="bool",
                icon="dark-mode",
                description="Ask GTK applications to use their dark variant",
                legacy_section="assets",
            ),
        ),
    ),
)

COMPONENT_BY_ID = {
    component.id: component
    for group in GROUPS
    for component in group.components
}
GROUP_BY_ID = {group.id: group for group in GROUPS}


def _data_dirs() -> list[Path]:
    dirs = [data_home()]
    for raw in os.environ.get("XDG_DATA_DIRS", "/usr/local/share:/usr/share").split(":"):
        raw = raw.strip()
        if raw:
            dirs.append(Path(raw))
    return dirs


def _theme_dirs(*suffixes: str) -> list[Path]:
    return [directory / suffix for directory in _data_dirs() for suffix in suffixes]


def _existing_file(candidates: list[Path]) -> Optional[Path]:
    for path in candidates:
        if path.is_file():
            return path
    return None


def _line_preserving_set(text: str, group: str, key: str, value: str) -> str:
    try:
        return _set_ini_text(text, group, key, value)
    except Exception:
        pattern = re.compile(rf"(?m)^({re.escape(key)}\s*=).*$")
        return pattern.sub(rf"\1 {value}", text)


def _set_group_key(text: str, group: str, key: str, value: str) -> str:
    lines = text.splitlines()
    output: list[str] = []
    current = ""
    replaced = False
    inserted = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            if current == group and not inserted:
                output.append(f"{key}={value}")
                inserted = True
            current = stripped[1:-1]
            output.append(line)
            continue
        if current == group and stripped.startswith(f"{key}="):
            if not replaced:
                output.append(f"{key}={value}")
                replaced = True
            continue
        output.append(line)
    if current == group and not inserted:
        output.append(f"{key}={value}")
    elif group not in text:
        if output and output[-1].strip():
            output.append("")
        output.extend([f"[{group}]", f"{key}={value}"])
    return "\n".join(output).rstrip() + "\n"


def _json_safe_dump(data: dict) -> str:
    return json.dumps(data, indent=2, ensure_ascii=False, sort_keys=True) + "\n"


class ThemeBundleCore:
    """Build, save and apply coherent theme bundles."""

    def __init__(
        self,
        theme_core: Optional[ThemeCore] = None,
        profile_core: Optional[ProfileCore] = None,
        wallpaper_core: Optional[WallpaperCore] = None,
    ) -> None:
        self._theme: ThemeCore = theme_core or AppThemeCore()
        self._profiles = profile_core
        self._wallpapers = wallpaper_core
        self._app_changes: dict[str, str] = {}

    # ── component registry ───────────────────────────────────────────────

    def groups(self, scope: str = "live") -> list[dict]:
        return [self._group_to_dict(group, scope) for group in GROUPS]

    def _group_to_dict(self, group: BundleGroupDef, scope: str) -> dict:
        return {
            "id": group.id,
            "label": group.label,
            "icon": group.icon,
            "description": group.description,
            "components": [
                self._component_to_dict(component, scope)
                for component in group.components
            ],
        }

    def _component_to_dict(self, component: BundleComponentDef, scope: str) -> dict:
        value: dict[str, Any] = {}
        choices: list[dict] = []
        supported = True
        status = "ok"
        status_text = ""
        current = ""
        current_display = ""
        staged = False

        if component.kind == "wallpaper":
            current = self._current_wallpaper()
            current_display = Path(current).name if current else ""
            choices = []
        elif component.field_id:
            value = self._theme.get(component.field_id, scope) or {}
            current = "" if value.get("value") is None else str(value.get("value"))
            current_display = str(value.get("display") or current)
            staged = bool(value.get("staged"))
            raw_choices = self._theme.choices(component.field_id)
            choices = [
                {"value": choice.value, "label": choice.label or choice.value}
                for choice in raw_choices
            ]
            if current and component.kind == "choice" and all(
                str(item["value"]) != current for item in choices
            ):
                choices.append({"value": current, "label": f"Custom: {current}"})
            if component.kind == "choice":
                installed = {str(item["value"]) for item in choices}
                supported = bool(choices)
                if current and installed and current not in installed:
                    status = "warning"
                    status_text = "Selected theme is not installed"

        return {
            "id": component.id,
            "label": component.label,
            "icon": component.icon,
            "kind": component.kind,
            "description": component.description,
            "currentValue": current,
            "currentDisplay": current_display,
            "choices": choices,
            "minimum": value.get("min"),
            "maximum": value.get("max"),
            "step": value.get("step"),
            "suffix": value.get("suffix") or "",
            "supported": supported,
            "status": status,
            "statusText": status_text,
            "staged": staged,
            "profileTracked": component.profile_tracked,
            "externalModule": component.external_module,
            "legacySection": component.legacy_section,
        }

    def _current_wallpaper(self) -> str:
        if self._wallpapers is not None:
            try:
                return self._wallpapers.current() or ""
            except Exception:
                return ""
        # Same location the GUI wallpaper core refreshes.
        try:
            for line in _read_text(live_path(".cache/theme-profile-wallpaper")).splitlines():
                if line.strip():
                    return line.strip()
        except (OSError, AttributeError):
            pass
        return ""

    # ── applications ─────────────────────────────────────────────────────

    def applications(self) -> list[dict]:
        apps: list[dict] = []
        for app_id, adapter in ADAPTERS.items():
            installed = adapter.detect()
            capabilities = dict(adapter.capabilities)
            current: dict[str, Any] = {"value": "", "label": ""}
            themes: list[dict] = []
            status = "ok"
            status_text = ""
            config_file = adapter.config_file()
            if config_file is None and adapter.config_paths:
                config_file = live_path(adapter.config_paths[0])

            if not installed:
                status = "not_installed"
                status_text = "Application not detected"
            elif capabilities.get("current_theme"):
                current_result = adapter.current_theme()
                if current_result.get("ok"):
                    current = current_result
                else:
                    status = str(current_result.get("status") or "warning")
                    status_text = str(current_result.get("message") or "")
            if installed and capabilities.get("installed_themes"):
                themes_result = adapter.installed_themes()
                if themes_result.get("ok"):
                    themes = list(themes_result.get("themes") or [])
                elif status == "ok":
                    status = str(themes_result.get("status") or "warning")
                    status_text = str(themes_result.get("message") or "")

            apps.append({
                "id": app_id,
                "name": adapter.name,
                "icon": adapter.icon,
                "installed": installed,
                "integration": adapter.integration,
                "capabilities": capabilities,
                "configPath": str(config_file or ""),
                "configDisplay": str(config_file.name if config_file else ""),
                "currentValue": str(current.get("value") or ""),
                "currentDisplay": str(current.get("label") or current.get("value") or ""),
                "mode": str(current.get("mode") or ""),
                "themes": themes,
                "status": status,
                "statusText": status_text,
                "changed": self._app_changes.get(app_id, ""),
            })
        return apps

    # ── changes ──────────────────────────────────────────────────────────

    def session_changes(self) -> int:
        return self._theme.session_changes() + len(self._app_changes)

    def undo_depth(self) -> int:
        return self._theme.undo_depth()

    def set_component(self, component_id: str, value: Any) -> Result:
        if component_id in ADAPTERS:
            adapter = ADAPTERS[component_id]
            if not adapter.detect():
                return Result.err(f"{adapter.name} is not installed")
            result = adapter.apply_theme(str(value))
            if result.get("ok"):
                self._app_changes[component_id] = str(value)
                return Result.ok(str(result.get("message") or f"{adapter.name} theme updated"))
            return Result.err(str(result.get("message") or "Theme update failed"))

        component = COMPONENT_BY_ID.get(component_id)
        if component is None:
            return Result.err(f"Unknown component: {component_id}")
        if component.kind == "wallpaper":
            if self._wallpapers is None:
                return Result.err("Wallpaper core is not available")
            result = self._wallpapers.set_wallpaper(str(value))
            if result.success:
                self._app_changes[component_id] = str(value)
            return result
        if not component.field_id:
            return Result.err(f"Component {component_id} is not writable")
        return self._theme.set_value(component.field_id, value)

    def apply(self) -> Result:
        app_count = len(self._app_changes)
        self._app_changes.clear()
        result = self._theme.apply()
        if result.success and app_count:
            return Result.ok(f"{result.message} + {app_count} app theme(s)")
        if result.success and app_count == 0:
            return result
        if app_count:
            return Result.ok(f"Applied {app_count} app theme(s); core: {result.message}")
        return result

    def undo(self) -> Result:
        return self._theme.undo()

    def revert(self) -> Result:
        result = self._theme.revert()
        self._app_changes.clear()
        return result

    # ── profile bundles ──────────────────────────────────────────────────

    def profiles(self) -> list[dict]:
        if self._profiles is None:
            return []
        try:
            profiles = self._profiles.list_profiles()
        except Exception:
            return []
        return [
            {
                "name": profile.name,
                "active": profile.is_active,
                "description": profile.description,
                "createdAt": profile.created_at,
                "hasBundle": self._bundle_path(profile.name).is_file(),
            }
            for profile in profiles
        ]

    def _bundle_path(self, name: str) -> Path:
        return profile_dir(name) / "theme-bundle.json"

    def profile_bundle(self, name: str) -> Result:
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        path = self._bundle_path(name)
        if path.is_file():
            try:
                data = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError) as exc:
                return Result.err(f"Invalid bundle metadata: {exc}")
            return Result.ok(data=data)

        # Legacy profile: synthesize a v2 view from the stored configs.
        scope = f"profile:{name}"
        components: dict[str, Any] = {}
        for group in GROUPS:
            for component in group.components:
                if not component.field_id:
                    continue
                item = self._component_to_dict(component, scope)
                if item["currentDisplay"]:
                    components[component.id] = item["currentValue"]
        return Result.ok(data={
            "schema": BUNDLE_SCHEMA,
            "name": name,
            "legacy": True,
            "components": components,
            "applications": {},
            "wallpaper": "",
        })

    def save_profile(self, name: str, description: str = "", wallpaper: str = "") -> Result:
        if self._profiles is None:
            return Result.err("Profile core is not available")
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))

        saved = self._profiles.save(name)
        if not saved.success:
            return saved
        if description.strip():
            described = self._profiles.set_description(name, description.strip())
            if not described.success:
                return described

        bundle = self._bundle_data(name, wallpaper=wallpaper)
        target = self._bundle_path(name)
        backup_file(target)
        try:
            target.parent.mkdir(parents=True, exist_ok=True)
            atomic_write_text(target, _json_safe_dump(bundle))
        except OSError as exc:
            return Result.err(f"Failed to write theme bundle: {exc}")
        return Result.ok(f"Saved profile and theme bundle: {name}", data=bundle)

    def _bundle_data(self, name: str, wallpaper: str = "") -> dict:
        components: dict[str, Any] = {}
        for group in self.groups("live"):
            for component in group["components"]:
                if component["profileTracked"] and component["currentDisplay"]:
                    components[component["id"]] = component["currentValue"]
        apps: dict[str, str] = {}
        for app in self.applications():
            if app["installed"] and app["currentValue"]:
                apps[str(app["id"])] = str(app["currentValue"])
        return {
            "schema": BUNDLE_SCHEMA,
            "name": name,
            "description": "",
            "createdAt": datetime.now().isoformat(timespec="seconds"),
            "components": components,
            "applications": apps,
            "wallpaper": wallpaper or self._current_wallpaper(),
        }

    def apply_profile(self, name: str) -> Result:
        if self._profiles is None:
            return Result.err("Profile core is not available")
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        applied = self._profiles.apply(name)
        if not applied.success:
            return applied

        bundle_result = self.profile_bundle(name)
        if bundle_result.success and bundle_result.data:
            data = bundle_result.data
            messages = [applied.message]
            for component_id, value in dict(data.get("components") or {}).items():
                component = COMPONENT_BY_ID.get(component_id)
                if component and component.field_id and value not in (None, ""):
                    staged = self._theme.set_value(component.field_id, value)
                    if not staged.success:
                        messages.append(staged.message)
            theme_applied = self._theme.apply()
            messages.append(theme_applied.message)
            for app_id, theme in dict(data.get("applications") or {}).items():
                app_result = self.set_component(str(app_id), theme)
                messages.append(f"{app_id}: {app_result.message}")
            wallpaper = str(data.get("wallpaper") or "")
            if wallpaper and self._wallpapers is not None:
                wallpaper_result = self._wallpapers.set_wallpaper(wallpaper)
                messages.append(wallpaper_result.message)
            return Result.ok("; ".join(messages))
        return applied

    def validate_profile_bundle(self, name: str) -> Result:
        result = self.profile_bundle(name)
        if not result.success:
            return result
        data = result.data or {}
        missing: list[str] = []
        for component_id, value in dict(data.get("components") or {}).items():
            component = COMPONENT_BY_ID.get(component_id)
            if component and component.kind == "choice" and value:
                item = self._component_to_dict(component, "live")
                values = {str(choice["value"]) for choice in item["choices"]}
                if values and str(value) not in values:
                    missing.append(f"{component.label}: {value}")
        for app_id, theme in dict(data.get("applications") or {}).items():
            app = next((item for item in self.applications() if item["id"] == app_id), None)
            if app and app["themes"]:
                labels = {str(theme_item["id"]) for theme_item in app["themes"]}
                if str(theme) not in labels:
                    missing.append(f"{app['name']}: {theme}")
        if missing:
            return Result.err("Missing theme references: " + "; ".join(missing))
        return Result.ok("Theme bundle is complete")

    # ── derived user themes ──────────────────────────────────────────────

    def create_custom_theme(self, name: str) -> Result:
        """Create user-owned derived color and Kvantum themes."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))

        created: list[str] = []
        color_result = self._create_color_scheme(name)
        if color_result.success:
            created.extend(color_result.data.get("assets", []))
        elif "not based" not in color_result.message:
            return color_result

        kvantum_result = self._create_kvantum_theme(name)
        if kvantum_result.success:
            created.extend(kvantum_result.data.get("assets", []))
        elif "not based" not in kvantum_result.message:
            return kvantum_result

        if not created:
            return Result.err(
                "Nothing to create: pick an installed base color scheme or Kvantum theme first"
            )
        return Result.ok(f"Created custom theme assets: {', '.join(created)}", data={"assets": created})

    def _create_color_scheme(self, name: str) -> Result:
        current = self._theme.get("color_scheme", "live") or {}
        base = str(current.get("value") or "")
        if not base:
            return Result.err("No installed base color scheme")
        source = _existing_file([
            data_home() / "color-schemes" / f"{base}.colors",
            *[directory / "color-schemes" / f"{base}.colors" for directory in _data_dirs()[1:]],
        ])
        if source is None:
            return Result.err(f"Base color scheme not found: {base}")
        target = data_home() / "color-schemes" / f"{name}.colors"
        if target.exists():
            return Result.err(f"Color scheme already exists: {name}")
        try:
            text = source.read_text(encoding="utf-8", errors="replace")
            text = _set_group_key(text, "General", "Name", name)
            text = _set_group_key(text, "General", "ColorScheme", name)
            accent = self._theme.get("accent_color", "live") or {}
            if accent.get("raw"):
                raw = str(accent["raw"])
                if re.fullmatch(r"\d+,\d+,\d+", raw):
                    text = _set_group_key(text, "General", "AccentColor", raw)
            target.parent.mkdir(parents=True, exist_ok=True)
            atomic_write_text(target, text if text.endswith("\n") else text + "\n")
        except OSError as exc:
            return Result.err(f"Failed to create color scheme: {exc}")
        return Result.ok(data={"assets": [str(target)]})

    def _create_kvantum_theme(self, name: str) -> Result:
        current = self._theme.get("kvantum_theme", "live") or {}
        base = str(current.get("value") or "")
        if not base or not isinstance(self._theme, AppThemeCore):
            return Result.err("No installed base Kvantum theme")
        source = self._theme._kvantum_file(base)  # noqa: SLF001 - same core package
        if source is None:
            return Result.err(f"Base Kvantum theme not found: {base}")
        root = data_home() / "Kvantum" / name
        target = root / f"{name}.kvconfig"
        if target.exists():
            return Result.err(f"Kvantum theme already exists: {name}")
        try:
            text = source.read_text(encoding="utf-8", errors="replace")
            for field_id in ("kvantum_translucent", "kvantum_blurring", "kvantum_popup_blurring"):
                item = self._theme.get(field_id, "live") or {}
                value = item.get("value")
                if value is None:
                    continue
                field = item
                key = str(field.get("key") or "")
                group = str(field.get("group") or "%General")
                if key:
                    text = _set_ini_text(text, group, key, "true" if value else "false")
            root.mkdir(parents=True, exist_ok=True)
            atomic_write_text(target, text if text.endswith("\n") else text + "\n")
            for asset in source.parent.iterdir():
                if asset.is_file() and asset.suffix in (".svg", ".png") and asset.name != target.name:
                    shutil.copy2(asset, root / asset.name)
        except OSError as exc:
            return Result.err(f"Failed to create Kvantum theme: {exc}")
        return Result.ok(data={"assets": [str(target)]})

    # ── external actions ─────────────────────────────────────────────────

    def open_system_settings(self, module: str) -> Result:
        if not module:
            return Result.err("System Settings module is empty")
        try:
            subprocess.Popen(["kcmshell6", module])
        except OSError as exc:
            return Result.err(str(exc))
        return Result.ok("Opened System Settings")

    def launch_app(self, app_id: str) -> Result:
        adapter = adapter_for(app_id)
        if adapter is None:
            return Result.err(f"Unknown application: {app_id}")
        result = adapter.launch()
        if result.get("ok"):
            return Result.ok(str(result.get("message") or "Application launched"))
        return Result.err(str(result.get("message") or "Launch failed"))

    def open_app_config(self, app_id: str) -> Result:
        adapter = adapter_for(app_id)
        if adapter is None:
            return Result.err(f"Unknown application: {app_id}")
        result = adapter.open_config()
        if result.get("ok"):
            return Result.ok(str(result.get("message") or "Config opened"))
        return Result.err(str(result.get("message") or "Config cannot be opened"))

    def open_app_theme_settings(self, app_id: str) -> Result:
        adapter = adapter_for(app_id)
        if adapter is None:
            return Result.err(f"Unknown application: {app_id}")
        result = adapter.open_theme_settings()
        if result.get("ok"):
            return Result.ok(str(result.get("message") or "Theme settings opened"))
        return Result.err(str(result.get("message") or "Theme settings are not available"))


__all__ = [
    "BUNDLE_SCHEMA", "GROUPS", "GROUP_BY_ID", "COMPONENT_BY_ID", "ThemeBundleCore",
]
