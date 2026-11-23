"""High-level theme bundle core.

``AppThemeCore`` knows how to read and write individual curated fields. This
module adds the product-level view on top of it: installed KDE/Plasma assets,
desktop parameters, Kvantum/GTK choices, per-app adapters, profile bundle
metadata and user-created derived themes.
"""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Optional

from .app_theme_adapters import adapter_for
from .paths import (
    profile_dir,
    validate_profile_name,
)
from .protocol import ProfileCore, Result, ThemeCore, WallpaperCore
from .theme_core import AppThemeCore


BUNDLE_SCHEMA = "theme-bundle/1"


def QT_TRANSLATE_NOOP(context: str, text: str) -> str:  # noqa: N802
    """Mark a static string for pylupdate6; translated later by ``_tr``."""
    del context
    return text


def _tr(text: str) -> str:
    """Translate a user-visible bundle string (context ``ThemeBundle``).

    Without a loaded translator (tests, CLI use) the source text is returned.
    """
    from PySide6.QtCore import QCoreApplication

    return QCoreApplication.translate("ThemeBundle", text)


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
    saved_only: bool = False


@dataclass(frozen=True, slots=True)
class BundleGroupDef:
    id: str
    label: str
    icon: str
    description: str
    components: tuple[BundleComponentDef, ...]


GROUPS: tuple[BundleGroupDef, ...] = (
    BundleGroupDef(
        "plasma", QT_TRANSLATE_NOOP("ThemeBundle", "KDE & Plasma"), "preferences-desktop-theme",
        QT_TRANSLATE_NOOP("ThemeBundle", "Global theme, Plasma Style, decorations, icons, cursors and fonts"),
        (
            BundleComponentDef(
                "plasma_global", QT_TRANSLATE_NOOP("ThemeBundle", "Global theme"), "look_and_feel",
                icon="preferences-desktop-theme-global",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Look & Feel package used by Plasma"),
                external_module="kcm_lookandfeel",
                legacy_section="assets",
            ),
            BundleComponentDef(
                "plasma_style", QT_TRANSLATE_NOOP("ThemeBundle", "Plasma Style"), "plasma_theme",
                icon="preferences-desktop-theme",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Visual style of Plasma widgets and panels"),
                external_module="kcm_desktoptheme",
                legacy_section="assets",
            ),
            BundleComponentDef(
                "decoration_theme", QT_TRANSLATE_NOOP("ThemeBundle", "Window decoration"), "decoration_theme",
                icon="preferences-system-windows",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Theme used by the selected window decoration plugin"),
                external_module="kcm_kwindecoration",
                legacy_section="windows",
            ),
            BundleComponentDef(
                "icon_theme", QT_TRANSLATE_NOOP("ThemeBundle", "Icon theme"), "icon_theme",
                icon="preferences-desktop-icons",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Icon theme for Plasma and Qt applications"),
                external_module="kcm_icons",
                legacy_section="assets",
            ),
            BundleComponentDef(
                "cursor_theme", QT_TRANSLATE_NOOP("ThemeBundle", "Cursor theme"), "cursor_theme",
                icon="preferences-desktop-cursors",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Mouse cursor theme"),
                external_module="kcm_cursortheme",
                legacy_section="assets",
                profile_tracked=False,
            ),
            BundleComponentDef(
                "ui_font", QT_TRANSLATE_NOOP("ThemeBundle", "Fonts"), "ui_font",
                kind="font",
                icon="preferences-desktop-font",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "UI, menu, title and fixed fonts"),
                external_module="kcm_fonts",
                legacy_section="typography",
            ),
        ),
    ),
    BundleGroupDef(
        "desktop", QT_TRANSLATE_NOOP("ThemeBundle", "Desktop & Windows"), "preferences-system-windows",
        QT_TRANSLATE_NOOP("ThemeBundle", "Wallpaper, corners, blur, translucency and window effects"),
        (
            BundleComponentDef(
                "wallpaper", QT_TRANSLATE_NOOP("ThemeBundle", "Wallpaper"), "",
                kind="wallpaper",
                icon="preferences-desktop-wallpaper",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Wallpaper used by the current Plasma session"),
            ),
            BundleComponentDef(
                "corner_radius", QT_TRANSLATE_NOOP("ThemeBundle", "Corner radius"), "klassy_radius",
                kind="int",
                icon="rounded",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Window corner radius used by Klassy"),
                legacy_section="windows",
            ),
            BundleComponentDef(
                "blur_strength", QT_TRANSLATE_NOOP("ThemeBundle", "Blur strength"), "blur_strength",
                kind="int",
                icon="blur",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Better Blur DX strength"),
                external_module="kcm_kwin_effects",
                legacy_section="windows",
            ),
            BundleComponentDef(
                "translucency", QT_TRANSLATE_NOOP("ThemeBundle", "Inactive window opacity"), "translucency_inactive",
                kind="int",
                icon="translucency",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Opacity of inactive windows"),
                legacy_section="windows",
            ),
            BundleComponentDef(
                "animations", QT_TRANSLATE_NOOP("ThemeBundle", "Animation speed"), "animation_factor",
                kind="float",
                icon="preferences-desktop-animation",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Global animation duration multiplier"),
                external_module="kcm_animations",
                legacy_section="windows",
            ),
        ),
    ),
    BundleGroupDef(
        "qt", QT_TRANSLATE_NOOP("ThemeBundle", "Qt & Kvantum"), "preferences-desktop-theme",
        QT_TRANSLATE_NOOP("ThemeBundle", "Qt widget style and Kvantum theme"),
        (
            BundleComponentDef(
                "widget_style", QT_TRANSLATE_NOOP("ThemeBundle", "Qt widget style"), "widget_style",
                icon="preferences-desktop-theme-style",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Qt application widget style"),
                external_module="kcm_style",
                legacy_section="colors",
            ),
            BundleComponentDef(
                "kvantum_theme", QT_TRANSLATE_NOOP("ThemeBundle", "Kvantum theme"), "kvantum_theme",
                icon="kvantum",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "SVG widget theme used by the Kvantum Qt style"),
                legacy_section="assets",
            ),
            BundleComponentDef(
                "kvantum_translucent", QT_TRANSLATE_NOOP("ThemeBundle", "Kvantum translucent windows"), "kvantum_translucent",
                kind="bool",
                icon="translucency",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Let Kvantum paint translucent windows"),
                legacy_section="assets",
            ),
        ),
    ),
    BundleGroupDef(
        "palette", QT_TRANSLATE_NOOP("ThemeBundle", "Palette"), "preferences-desktop-color",
        QT_TRANSLATE_NOOP("ThemeBundle", "KDE color scheme and accent"),
        (
            BundleComponentDef(
                "color_scheme", QT_TRANSLATE_NOOP("ThemeBundle", "Color scheme"), "color_scheme",
                icon="preferences-desktop-color",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Installed or bundled KDE color scheme"),
                external_module="kcm_colors",
                legacy_section="colors",
            ),
            BundleComponentDef(
                "accent_color", QT_TRANSLATE_NOOP("ThemeBundle", "Accent color"), "accent_color",
                kind="color", icon="color-picker",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Custom accent used when the wallpaper accent is off"),
            ),
            BundleComponentDef(
                "accent_from_wallpaper", QT_TRANSLATE_NOOP("ThemeBundle", "Accent from wallpaper"), "accent_from_wallpaper",
                kind="bool", icon="preferences-desktop-wallpaper",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Let Plasma choose the accent from the wallpaper"),
            ),
        ),
    ),
    BundleGroupDef(
        "applications", QT_TRANSLATE_NOOP("ThemeBundle", "Applications"), "applications-other",
        QT_TRANSLATE_NOOP("ThemeBundle", "Terminal appearance and application themes"),
        (
            BundleComponentDef("kitty_opacity", QT_TRANSLATE_NOOP("ThemeBundle", "Kitty opacity"),
                               "kitty_opacity", kind="float", icon="utilities-terminal"),
            BundleComponentDef("kitty_blur", QT_TRANSLATE_NOOP("ThemeBundle", "Kitty blur"),
                               "kitty_blur", kind="int", icon="utilities-terminal"),
            BundleComponentDef("alacritty_opacity", QT_TRANSLATE_NOOP("ThemeBundle", "Alacritty opacity"),
                               "alacritty_opacity", kind="float", icon="utilities-terminal"),
            BundleComponentDef("alacritty_blur", QT_TRANSLATE_NOOP("ThemeBundle", "Alacritty blur"),
                               "alacritty_blur", kind="bool", icon="utilities-terminal"),
            BundleComponentDef("vscode_theme", QT_TRANSLATE_NOOP("ThemeBundle", "VS Code theme"),
                               "vscode_theme", icon="vscode", saved_only=True),
            BundleComponentDef("cursor_ide_theme", QT_TRANSLATE_NOOP("ThemeBundle", "Cursor editor theme"),
                               "cursor_ide_theme", icon="cursor", saved_only=True),
            BundleComponentDef("zed_mode", QT_TRANSLATE_NOOP("ThemeBundle", "Zed theme mode"),
                               "zed_mode", icon="zed", saved_only=True),
            BundleComponentDef("zed_light", QT_TRANSLATE_NOOP("ThemeBundle", "Zed light theme"),
                               "zed_light", icon="zed", saved_only=True),
            BundleComponentDef("zed_dark", QT_TRANSLATE_NOOP("ThemeBundle", "Zed dark theme"),
                               "zed_dark", icon="zed", saved_only=True),
            BundleComponentDef("zen_accent_color", QT_TRANSLATE_NOOP("ThemeBundle", "Zen accent color"),
                               "", kind="color", icon="color-picker", saved_only=True),
            BundleComponentDef("zen_mode", QT_TRANSLATE_NOOP("ThemeBundle", "Zen theme"),
                               "", icon="preferences-desktop-theme", saved_only=True),
            BundleComponentDef("obsidian_mode", QT_TRANSLATE_NOOP("ThemeBundle", "Obsidian color mode"),
                               "", icon="preferences-desktop-theme", saved_only=True),
            BundleComponentDef("obsidian_theme", QT_TRANSLATE_NOOP("ThemeBundle", "Obsidian theme"),
                               "", icon="obsidian", saved_only=True),
            BundleComponentDef("btop_theme", QT_TRANSLATE_NOOP("ThemeBundle", "btop theme"),
                               "btop_theme", icon="utilities-system-monitor", saved_only=True),
            BundleComponentDef("btop_rounded", QT_TRANSLATE_NOOP("ThemeBundle", "btop rounded corners"),
                               "btop_rounded", kind="bool", icon="utilities-system-monitor", saved_only=True),
        ),
    ),
    BundleGroupDef(
        "gtk", QT_TRANSLATE_NOOP("ThemeBundle", "GTK"), "gtk",
        QT_TRANSLATE_NOOP("ThemeBundle", "GTK 2, GTK 3 and GTK 4 application themes"),
        (
            BundleComponentDef(
                "gtk_theme", QT_TRANSLATE_NOOP("ThemeBundle", "GTK theme"), "gtk_theme",
                icon="gtk",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Theme used by GTK applications"),
                legacy_section="assets",
            ),
            BundleComponentDef(
                "gtk_dark", QT_TRANSLATE_NOOP("ThemeBundle", "Prefer dark GTK variant"), "gtk_dark",
                kind="bool",
                icon="dark-mode",
                description=QT_TRANSLATE_NOOP("ThemeBundle", "Ask GTK applications to use their dark variant"),
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










def _display_text(component, text):
    from .core_messages import render_message
    return render_message(text) if component.kind == "bool" or text == "System default" else text


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

    # ── component registry ───────────────────────────────────────────────

    def groups(self, scope: str = "live") -> list[dict]:
        with self._theme.read_batch():
            return [self._group_to_dict(group, scope) for group in GROUPS]

    def _group_to_dict(self, group: BundleGroupDef, scope: str) -> dict:
        return {
            "id": group.id,
            "label": _tr(group.label),
            "icon": group.icon,
            "description": _tr(group.description),
            "components": [
                self._component_to_dict(component, scope)
                for component in group.components
                if scope != "live" or not component.saved_only
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
        installed: list[str] = []

        if component.kind == "wallpaper":
            pending = getattr(self._theme, "_wallpaper_draft", None) if scope == "live" else None
            current = pending or self._current_wallpaper()
            current_display = Path(current).name if current else ""
            staged = bool(pending)
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
            installed = [str(item["value"]) for item in choices]
            if current and component.kind == "choice" and all(
                str(item["value"]) != current for item in choices
            ):
                choices.append({"value": current, "label": _tr(QT_TRANSLATE_NOOP("ThemeBundle", "Custom: %1")).replace("%1", current)})
            if component.kind == "choice":
                supported = bool(choices)
                if current and installed and current not in installed:
                    status = "warning"
                    status_text = _tr(QT_TRANSLATE_NOOP("ThemeBundle", "Selected theme is not installed"))

        return {
            "id": component.id,
            "label": _tr(component.label),
            "icon": component.icon,
            "kind": component.kind,
            "description": _tr(component.description),
            "currentValue": current,
            "currentDisplay": _display_text(component, current_display),
            "choices": choices,
            "installedChoices": installed,
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
        from .cli_core import core_request
        return core_request("wallpaper.current") or ""

    def applications(self) -> list[dict]:
        from .cli_core import core_request
        apps = core_request("app.all")
        for app in apps:
            app["changed"] = self._app_changes.get(app["id"], "")
            from .core_messages import render_message, localize
            template = app.get("statusTemplate")
            app["statusText"] = (render_message(template, app.get("statusArgs") or []) if template else localize(app["statusText"]))
        return apps

    # ── changes ──────────────────────────────────────────────────────────

    def session_changes(self) -> int:
        # The shared Inspector count already includes staged app selections.
        return self._theme.session_changes() + len(self._app_changes) - self._theme._app_draft_count

    def undo_depth(self) -> int:
        return self._theme.undo_depth()

    @property
    def _app_changes(self):
        return self._theme._app_changes

    def set_component(self, component_id: str, value: Any) -> Result:
        return self._theme._session_call("component", id=component_id, value=value)

    def stage_application(self, app_id: str, theme: str) -> Result:
        return self._theme._session_call("app", id=app_id, value=theme, stage=True)

    def stage_wallpaper(self, path: str) -> Result:
        return self._theme._session_call("wallpaper", value=path, stage=True)

    def apply(self, *, check_live: bool = False) -> Result:
        return self._theme.apply(check_live=check_live)

    def undo(self) -> Result:
        return self._theme.undo()

    def revert(self) -> Result:
        return self._theme.revert()

    # ── profile bundles ──────────────────────────────────────────────────

    def profiles(self) -> list[dict]:
        if self._profiles is None:
            return []
        profiles = self._profiles.list_profiles()
        return [
            {
                "name": profile.name,
                "active": profile.is_active,
                "description": profile.description,
                "createdAt": profile.created_at,
                "hasBundle": any(
                    (profile_dir(profile.name) / filename).is_file()
                    for filename in ("theme.json", "theme-bundle.json")
                ),
            }
            for profile in profiles
        ]

    def profile_bundle(self, name: str) -> Result:
        from .cli_core import core_request
        try:
            return Result.ok(data=core_request("bundle.metadata", name=name))
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    def profile_editor(self, name: str) -> Result:
        from .cli_core import core_request
        try:
            return Result.ok(data=core_request("bundle.editor", name=name))
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    def migrate_profile(self, name: str) -> Result:
        from .cli_core import core_request
        try:
            return Result.ok(data=core_request("profile.migrate", name=name))
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    def save_profile_edits(self, name: str, revision: str, changes: dict) -> Result:
        from .cli_core import core_request
        try:
            return Result.ok(data=core_request("bundle.save_edits", name=name,
                                               revision=revision, changes=changes))
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    def save_profile(self, name: str, description: str = "", wallpaper: str = "",
                     include_wallpaper: bool = True) -> Result:
        if self._profiles is None:
            return Result.err("Profile core is not available")
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))

        # The Rust CLI writes the theme package (theme.json + assets); the
        # former Python-side theme-bundle.json is no longer needed.
        try:
            saved = self._profiles.save(
                name, description=description.strip(), include_wallpaper=include_wallpaper
            )
        except TypeError:  # profile cores without the package-aware signature
            saved = self._profiles.save(name)
        if not saved.success:
            return saved
        view = self.profile_bundle(name)
        return Result.ok(saved.message or f"Saved profile: {name}",
                         data=view.data if view.success else None)


    def apply_profile(self, name: str, keep_wallpaper: bool = False,
                      install_missing: bool = False) -> Result:
        if self._profiles is None:
            return Result.err("Profile core is not available")
        return self._profiles.apply(name, keep_wallpaper=keep_wallpaper,
                                    install_missing=install_missing)

    def validate_profile_bundle(self, name: str) -> Result:
        from .cli_core import core_request
        try:
            data = core_request("bundle.validate", name=name)
            return Result(data["ok"], data["message"], data=data)
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    def create_profile(self, name, components=None, applications=None,
                       description="", include_wallpaper=True):
        return self._theme._session_call("create_profile", name=name,
            components=components or {}, applications=applications or {},
            description=description, include_wallpaper=include_wallpaper)

    def create_custom_theme(self, name: str) -> Result:
        from .cli_core import core_request
        try:
            data = core_request("resource.clone", name=name,
                                session=getattr(self._theme, "_session", None))
            return Result.ok(data["message"], data=data)
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    # ── editor theme stores (Rust CLI: `kolira apps …`) ───────────

    def _cli_json(self, args: list[str], timeout: int = 180) -> Result:
        from .cli_core import run_cli
        result = run_cli([*args, "--json"], timeout=timeout)
        if not result.success:
            return result
        try:
            return Result.ok(data={"value": json.loads(result.message)})
        except ValueError:
            return Result.err("kolira returned invalid JSON")

    def _store_adapter(self, app_id: str) -> Result:
        adapter = adapter_for(app_id)
        if adapter is None or not adapter.capabilities.get("theme_store"):
            return Result.err(f"No theme store for {app_id}")
        if not adapter.detect():
            return Result.err(f"{adapter.name} is not installed")
        return Result.ok()

    def store_search(self, app_id: str, query: str) -> Result:
        checked = self._store_adapter(app_id)
        if not checked.success:
            return checked
        result = self._cli_json(["apps", "store-search", app_id, query.strip()])
        if not result.success:
            return result
        items = list((result.data or {}).get("value") or [])
        return Result.ok(f"{len(items)} theme(s) found", data={"items": items})

    def store_install(self, app_id: str, extension_id: str) -> Result:
        checked = self._store_adapter(app_id)
        if not checked.success:
            return checked
        result = self._cli_json(["apps", "store-install", app_id, extension_id])
        if not result.success:
            return result
        report = dict((result.data or {}).get("value") or {})
        themes = list(report.get("themes") or [])
        return Result.ok(
            f"Installed {extension_id}: {len(themes)} theme(s)", data=report
        )

    # ── external actions ─────────────────────────────────────────────────

    def open_system_settings(self, module: str) -> Result:
        try:
            subprocess.Popen(["kcmshell6", module] if module else ["systemsettings"])
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
