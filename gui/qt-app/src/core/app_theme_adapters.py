"""Theme adapters for individual applications.

An adapter exposes the small, application-specific part of the theme contract:
detect the app, read the active theme, enumerate installed themes and apply a
selection. The rest of the application works with the common adapter interface
and never edits app configs directly from QML.
"""

from __future__ import annotations

import json
import shutil
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Optional

from .fileutil import atomic_write_text
from .paths import home_dir, live_path, theme_root
from .theme_core import _ini_get, _json_patch, _json_read, _set_ini_text, _strip_jsonc


@dataclass(frozen=True, slots=True)
class AdapterTheme:
    id: str
    label: str
    source: str = ""

    def to_dict(self) -> dict:
        return {"id": self.id, "label": self.label, "source": self.source}


def _ok(data: Optional[dict] = None, message: str = "") -> dict:
    item = {"ok": True, "message": message}
    if data:
        item.update(data)
    return item


def _err(message: str, status: str = "unsupported") -> dict:
    return {"ok": False, "status": status, "message": message}


def _read_text(path: Path) -> Optional[str]:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None


def backup_file(path: Path) -> Optional[Path]:
    """Copy an existing app config into the theme-profile backup area."""
    if not path.is_file():
        return None
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    try:
        relative = path.relative_to(home_dir())
    except ValueError:
        relative = Path("_external") / path.name
    target = theme_root() / "backups" / f"app-theme-{stamp}" / relative
    try:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
    except OSError:
        return None
    return target


class AppThemeAdapter:
    """Base class for per-app theme providers."""

    id = ""
    name = ""
    icon = "application-x-executable"
    launch_command: tuple[str, ...] = ()
    config_paths: tuple[str, ...] = ()
    integration = "generic"
    capabilities = {
        "detect": True,
        "current_theme": False,
        "installed_themes": False,
        "apply_theme": False,
        "launch": False,
        "open_theme_settings": False,
        "open_config": False,
    }

    def detect(self) -> bool:
        if self.launch_command and shutil.which(self.launch_command[0]):
            return True
        return any(live_path(path).exists() for path in self.config_paths)

    def config_file(self) -> Optional[Path]:
        for relative in self.config_paths:
            candidate = live_path(relative)
            if candidate.is_file():
                return candidate
        return None

    def current_theme(self) -> dict:
        return _err(f"{self.name} does not expose the current theme")

    def installed_themes(self) -> dict:
        return _err(f"{self.name} themes cannot be enumerated")

    def apply_theme(self, theme_id: str) -> dict:
        return _err(f"{self.name} themes cannot be applied automatically")

    def launch(self) -> dict:
        if not self.launch_command:
            return _err(f"{self.name} has no launch command")
        try:
            subprocess.Popen(list(self.launch_command))
        except OSError as exc:
            return _err(str(exc), "launch_failed")
        return _ok(message=f"Launched {self.name}")

    def open_theme_settings(self) -> dict:
        return _err(f"{self.name} has no known theme settings command")

    def open_config(self) -> dict:
        target = self.config_file()
        if target is None:
            if not self.config_paths:
                return _err(f"{self.name} has no known config file")
            target = live_path(self.config_paths[0])
            if target.is_dir():
                pass
            elif target.parent.is_dir():
                target = target.parent
            else:
                return _err(f"{self.name} config folder not found")
        try:
            subprocess.Popen(["xdg-open", str(target)])
        except OSError as exc:
            return _err(str(exc), "open_failed")
        return _ok(message=f"Opened {target}")


class JsonSettingsAdapter(AppThemeAdapter):
    """Adapter for apps that store the selected theme in a JSON settings file."""

    settings_rel = ""

    def settings_path(self) -> Path:
        return live_path(self.settings_rel)

    def current_theme(self) -> dict:
        path = self.settings_path()
        if not path.is_file():
            return _err(f"Settings file not found: {self.settings_rel}", "not_installed")
        value = _json_read(_read_text(path) or "", self.theme_path())
        if value in (None, ""):
            return _ok({"value": "", "label": ""}, "No theme selected")
        return _ok({"value": str(value), "label": str(value)})

    def apply_theme(self, theme_id: str) -> dict:
        if not theme_id:
            return _err("Theme is empty")
        path = self.settings_path()
        text = _read_text(path) or "{}"
        try:
            new_text = _json_patch(text, self.theme_path(), theme_id)
        except (KeyError, ValueError) as exc:
            return _err(str(exc), "apply_failed")
        backup_file(path)
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            atomic_write_text(path, new_text)
        except OSError as exc:
            return _err(str(exc), "apply_failed")
        return _ok(message=f"{self.name} theme set to {theme_id}")

    def theme_path(self) -> tuple[str, ...]:
        raise NotImplementedError


class VSCodeAdapter(JsonSettingsAdapter):
    name = "VS Code"
    icon = "vscode"
    launch_command = ("code",)
    config_paths = (".config/Code/User/settings.json",)
    settings_rel = ".config/Code/User/settings.json"
    extensions_rel = ".vscode/extensions"

    def __init__(self, name: str = "VS Code", icon: str = "vscode",
                 settings_rel: Optional[str] = None,
                 extensions_rel: Optional[str] = None,
                 launch_command: Optional[tuple[str, ...]] = None) -> None:
        if name:
            self.name = name
            self.id = "cursor" if name.casefold() == "cursor" else "vscode"
        if icon:
            self.icon = icon
        if settings_rel:
            self.settings_rel = settings_rel
            self.config_paths = (settings_rel,)
        if extensions_rel:
            self.extensions_rel = extensions_rel
        if launch_command:
            self.launch_command = launch_command
        self.capabilities = dict(self.capabilities)
        self.capabilities.update({
            "current_theme": True,
            "installed_themes": True,
            "apply_theme": True,
            "launch": bool(self.launch_command),
            "open_theme_settings": bool(self.launch_command),
        })

    def theme_path(self) -> tuple[str, ...]:
        return ("workbench.colorTheme",)

    def extensions_root(self) -> Path:
        return home_dir() / self.extensions_rel

    def installed_themes(self) -> dict:
        root = self.extensions_root()
        builtins = (
            ("Cursor Dark Midnight", "Cursor Light Midnight")
            if self.id == "cursor"
            else (
                "Default Dark Modern", "Default Light Modern",
                "Visual Studio Dark", "Visual Studio Light",
                "Abyss", "Kimbie Dark", "Monokai", "Solarized Dark", "Solarized Light",
            )
        )
        themes = [AdapterTheme(name, name, "built-in") for name in builtins]
        try:
            packages = sorted(root.glob("*/package.json"))
        except OSError:
            packages = []
        for package in packages:
            try:
                data = json.loads(package.read_text(encoding="utf-8", errors="replace"))
            except (OSError, json.JSONDecodeError):
                continue
            extension_id = data.get("name") or package.parent.name
            for item in data.get("contributes", {}).get("themes", []):
                label = str(item.get("label") or Path(str(item.get("path", "theme"))).stem)
                if label:
                    themes.append(AdapterTheme(label, label, f"{extension_id}:{package.parent.name}"))
        unique: dict[str, AdapterTheme] = {theme.id: theme for theme in themes}
        return _ok({"themes": [theme.to_dict() for theme in unique.values()]})

    def open_theme_settings(self) -> dict:
        if not shutil.which(self.launch_command[0]):
            return _err("VS Code command not found", "launch_failed")
        try:
            subprocess.Popen([self.launch_command[0], "workbench.action.selectTheme"])
        except OSError as exc:
            return _err(str(exc), "launch_failed")
        return _ok(message="Opened the theme picker")


class CursorAdapter(VSCodeAdapter):
    def __init__(self) -> None:
        super().__init__(
            name="Cursor",
            icon="cursor",
            settings_rel=".config/Cursor/User/settings.json",
            extensions_rel=".cursor/extensions",
            launch_command=("cursor",),
        )


class ZedAdapter(JsonSettingsAdapter):
    id = "zed"
    name = "Zed"
    icon = "zed"
    launch_command = ("zed",)
    config_paths = (".config/zed/settings.json",)
    settings_rel = ".config/zed/settings.json"
    built_in_themes = (
        "Zed (Default)", "Ayu Dark", "Ayu Light", "One Dark", "One Light",
        "Solarized Dark", "Solarized Light",
    )
    capabilities = {
        **AppThemeAdapter.capabilities,
        "current_theme": True,
        "installed_themes": True,
        "apply_theme": True,
        "launch": True,
        "open_theme_settings": True,
    }

    def theme_path(self) -> tuple[str, ...]:
        return ("theme", "mode")

    def current_theme(self) -> dict:
        path = self.settings_path()
        if not path.is_file():
            return _err("Settings file not found: .config/zed/settings.json", "not_installed")
        text = _read_text(path) or "{}"
        mode = _json_read(text, ("theme", "mode")) or "system"
        value = _json_read(text, ("theme", str(mode)))
        if value in (None, "") and mode == "system":
            value = _json_read(text, ("theme", "dark")) or _json_read(text, ("theme", "light"))
        return _ok({
            "value": str(value or ""),
            "label": str(value or ""),
            "mode": str(mode),
        })

    def installed_themes(self) -> dict:
        themes = [AdapterTheme(name, name, "built-in") for name in self.built_in_themes]
        root = live_path(".config/zed/themes")
        try:
            files = sorted(root.glob("*.json"))
        except OSError:
            files = []
        for path in files:
            label = path.stem
            try:
                data = json.loads(_strip_jsonc(path.read_text(encoding="utf-8", errors="replace")))
                label = str(data.get("name") or label)
            except (OSError, json.JSONDecodeError):
                pass
            themes.append(AdapterTheme(label, label, str(path)))
        unique = {theme.id: theme for theme in themes}
        return _ok({"themes": [theme.to_dict() for theme in unique.values()]})

    def apply_theme(self, theme_id: str) -> dict:
        if not theme_id:
            return _err("Theme is empty")
        path = self.settings_path()
        text = _read_text(path) or "{}"
        try:
            mode_text = _json_patch(text, ("theme", "mode"), "dark")
            final_text = _json_patch(mode_text, ("theme", "dark"), theme_id)
        except (KeyError, ValueError) as exc:
            return _err(str(exc), "apply_failed")
        backup_file(path)
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            atomic_write_text(path, final_text)
        except OSError as exc:
            return _err(str(exc), "apply_failed")
        return _ok(message=f"Zed dark theme set to {theme_id}")

    def open_theme_settings(self) -> dict:
        if not shutil.which(self.launch_command[0]):
            return _err("Zed command not found", "launch_failed")
        try:
            subprocess.Popen([self.launch_command[0], "--new"])
        except OSError as exc:
            return _err(str(exc), "launch_failed")
        return _ok(message="Opened Zed")


class ObsidianAdapter(AppThemeAdapter):
    id = "obsidian"
    name = "Obsidian"
    icon = "obsidian"
    launch_command = ("obsidian",)
    capabilities = {
        **AppThemeAdapter.capabilities,
        "current_theme": True,
        "installed_themes": True,
        "apply_theme": True,
        "launch": True,
        "open_theme_settings": True,
    }

    def vaults(self) -> list[Path]:
        roots = (home_dir() / "obsidian", home_dir() / "Obsidian")
        found: list[Path] = []
        for root in roots:
            try:
                children = sorted(root.iterdir(), key=lambda p: p.name.casefold())
            except OSError:
                continue
            for child in children:
                if child.is_dir() and (child / ".obsidian").is_dir():
                    found.append(child)
        return found

    def settings_path(self, vault: Path) -> Path:
        return vault / ".obsidian" / "appearance.json"

    def detect(self) -> bool:
        return bool(self.vaults())

    def current_theme(self) -> dict:
        vaults = self.vaults()
        if not vaults:
            return _err("Obsidian vault not found", "not_installed")
        path = self.settings_path(vaults[0])
        value = _json_read(_read_text(path) or "{}", ("theme",))
        return _ok({"value": str(value or "obsidian"), "label": str(value or "obsidian")})

    def installed_themes(self) -> dict:
        vaults = self.vaults()
        if not vaults:
            return _err("Obsidian vault not found", "not_installed")
        themes = [AdapterTheme("obsidian", "Obsidian", "built-in")]
        root = vaults[0] / ".obsidian" / "themes"
        try:
            entries = sorted((p for p in root.iterdir() if p.is_dir()), key=lambda p: p.name.casefold())
        except OSError:
            entries = []
        for entry in entries:
            if (entry / "theme.css").is_file() or (entry / "manifest.json").is_file():
                label = entry.name
                manifest = entry / "manifest.json"
                if manifest.is_file():
                    try:
                        data = json.loads(manifest.read_text(encoding="utf-8", errors="replace"))
                        label = str(data.get("name") or label)
                    except (OSError, json.JSONDecodeError):
                        pass
                themes.append(AdapterTheme(entry.name, label, str(entry)))
        return _ok({"themes": [theme.to_dict() for theme in themes]})

    def apply_theme(self, theme_id: str) -> dict:
        if not theme_id:
            return _err("Theme is empty")
        vaults = self.vaults()
        if not vaults:
            return _err("Obsidian vault not found", "not_installed")
        path = self.settings_path(vaults[0])
        text = _read_text(path) or "{}"
        try:
            new_text = _json_patch(text, ("theme",), theme_id)
        except (KeyError, ValueError) as exc:
            return _err(str(exc), "apply_failed")
        backup_file(path)
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            atomic_write_text(path, new_text)
        except OSError as exc:
            return _err(str(exc), "apply_failed")
        return _ok(message=f"Obsidian theme set to {theme_id}")

    def open_theme_settings(self) -> dict:
        if not shutil.which(self.launch_command[0]):
            return _err("Obsidian command not found", "launch_failed")
        try:
            subprocess.Popen([self.launch_command[0], "obsidian://appearance"])
        except OSError as exc:
            return _err(str(exc), "launch_failed")
        return _ok(message="Opened Obsidian appearance settings")


class BtopAdapter(AppThemeAdapter):
    id = "btop"
    name = "btop"
    icon = "btop"
    launch_command = ("btop",)
    config_paths = (".config/btop/btop.conf",)
    capabilities = {
        **AppThemeAdapter.capabilities,
        "current_theme": True,
        "installed_themes": True,
        "apply_theme": True,
        "launch": True,
    }

    def current_theme(self) -> dict:
        path = live_path(".config/btop/btop.conf")
        if not path.is_file():
            return _err("btop config not found", "not_installed")
        value = _ini_get(_read_text(path) or "", "", "color_theme")
        if isinstance(value, str) and len(value) >= 2 and value.startswith('"') and value.endswith('"'):
            value = value[1:-1]
        return _ok({"value": str(value or ""), "label": str(value or "")})

    def installed_themes(self) -> dict:
        roots = (live_path(".config/btop/themes"), Path("/usr/share/btop/themes"))
        themes: list[AdapterTheme] = []
        for root in roots:
            try:
                files = sorted(root.glob("*.theme"), key=lambda p: p.name.casefold())
            except OSError:
                continue
            themes.extend(AdapterTheme(path.stem, path.stem, str(path)) for path in files)
        unique = {theme.id: theme for theme in themes}
        return _ok({"themes": [theme.to_dict() for theme in unique.values()]})

    def apply_theme(self, theme_id: str) -> dict:
        if not theme_id:
            return _err("Theme is empty")
        path = live_path(".config/btop/btop.conf")
        if not path.is_file():
            return _err("btop config not found", "not_installed")
        text = _read_text(path) or ""
        new_text = _set_ini_text(text, "", "color_theme", f'"{theme_id}"')
        backup_file(path)
        try:
            atomic_write_text(path, new_text)
        except OSError as exc:
            return _err(str(exc), "apply_failed")
        return _ok(message=f"btop theme set to {theme_id}")


class KittyAdapter(AppThemeAdapter):
    id = "kitty"
    name = "Kitty"
    icon = "kitty"
    launch_command = ("kitty",)
    config_paths = (".config/kitty/kitty.conf",)
    integration = "config"
    capabilities = {
        **AppThemeAdapter.capabilities,
        "launch": True,
        "open_config": True,
    }


class AlacrittyAdapter(AppThemeAdapter):
    id = "alacritty"
    name = "Alacritty"
    icon = "alacritty"
    launch_command = ("alacritty",)
    config_paths = (".config/alacritty/alacritty.toml",)
    integration = "config"
    capabilities = {
        **AppThemeAdapter.capabilities,
        "launch": True,
        "open_config": True,
    }


class DiscordAdapter(AppThemeAdapter):
    id = "discord"
    name = "Discord"
    icon = "discord"
    launch_command = ("discord",)
    config_paths = (".config/discord/settings.json",)
    integration = "config"
    capabilities = {
        **AppThemeAdapter.capabilities,
        "current_theme": True,
        "launch": True,
        "open_config": True,
        "open_theme_settings": True,
    }

    def current_theme(self) -> dict:
        path = self.config_file()
        if path is None:
            return _err("Discord settings.json not found", "not_installed")
        try:
            data = json.loads(path.read_text(encoding="utf-8", errors="replace"))
        except (OSError, json.JSONDecodeError) as exc:
            return _err(f"Could not parse Discord settings.json: {exc}", "parse_error")
        value = str(data.get("BACKGROUND_COLOR") or data.get("theme") or "")
        lowered = value.lower()
        if lowered in ("#ffffff", "#fff", "light", "white"):
            label = "Light"
        elif value:
            label = "Dark"
        else:
            label = ""
        return _ok({"value": value, "label": label})

    def open_theme_settings(self) -> dict:
        # Native Discord does not expose a stable external theme-apply API.
        # Opening the app is the safe fallback; appearance lives inside Discord.
        return self.launch()


class TelegramAdapter(AppThemeAdapter):
    id = "telegram"
    name = "Telegram Desktop"
    icon = "telegram"
    launch_command = ("telegram-desktop",)
    config_paths = (".config/telegram-desktop/themes",)
    integration = "config"
    capabilities = {
        **AppThemeAdapter.capabilities,
        "installed_themes": True,
        "launch": True,
        "open_config": True,
        "open_theme_settings": True,
    }

    def installed_themes(self) -> dict:
        roots = (
            live_path(".config/telegram-desktop/themes"),
            live_path(".config/noctalia/Assets/Templates"),
            live_path(".local/share/TelegramDesktop/tdata/theme"),
        )
        themes: list[AdapterTheme] = []
        for root in roots:
            try:
                files = sorted(root.glob("*.tdesktop-theme"), key=lambda p: p.name.casefold())
            except OSError:
                continue
            themes.extend(AdapterTheme(path.stem, path.stem, str(path)) for path in files)
        unique = {theme.id: theme for theme in themes}
        return _ok({"themes": [theme.to_dict() for theme in unique.values()]})

    def open_theme_settings(self) -> dict:
        # Telegram stores the active theme internally; open the app rather than
        # editing tdata/settingss from outside.
        return self.launch()


ADAPTERS: dict[str, AppThemeAdapter] = {
    "zed": ZedAdapter(),
    "vscode": VSCodeAdapter(),
    "cursor": CursorAdapter(),
    "obsidian": ObsidianAdapter(),
    "discord": DiscordAdapter(),
    "telegram": TelegramAdapter(),
    "btop": BtopAdapter(),
    "kitty": KittyAdapter(),
    "alacritty": AlacrittyAdapter(),
}


def adapter_for(app_id: str) -> Optional[AppThemeAdapter]:
    return ADAPTERS.get(app_id)


__all__ = [
    "ADAPTERS", "AdapterTheme", "AppThemeAdapter", "adapter_for",
    "backup_file",
]
