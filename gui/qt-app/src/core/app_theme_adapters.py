"""GUI data adapters for Rust-owned application theme operations."""
from dataclasses import dataclass
from pathlib import Path


def _request(operation, **arguments):
    from .cli_core import core_request
    return core_request("app." + operation, **arguments)


@dataclass(frozen=True, slots=True)
class AdapterTheme:
    id: str
    label: str
    source: str = ""

    def to_dict(self):
        return {"id": self.id, "label": self.label, "source": self.source}


class AppThemeAdapter:
    id = ""

    def __getattr__(self, name):
        metadata = self.__dict__.get("_metadata")
        if metadata is None:
            metadata = _request("metadata", id=self.id)
            self._metadata = metadata
        if name not in metadata:
            raise AttributeError(name)
        value = metadata[name]
        return tuple(value) if isinstance(value, list) else value

    def executable(self):
        return _request("executable", id=self.id)

    def detect(self):
        return _request("detect", id=self.id)

    def has_config(self):
        return _request("has_config", id=self.id)

    def config_file(self):
        value = _request("config", id=self.id)
        return Path(value) if value else None

    def current_theme(self):
        return _request("current", id=self.id)

    def installed_themes(self):
        return _request("themes", id=self.id)

    def apply_theme(self, theme_id):
        return _request("apply", id=self.id, value=theme_id)

    def launch(self, *arguments):
        return _request("launch", id=self.id, arguments=list(arguments))

    def open_config(self):
        return _request("open_config", id=self.id)

    def open_theme_settings(self):
        return _request("open_theme_settings", id=self.id)


class VSCodeAdapter(AppThemeAdapter):
    id = "vscode"


class CursorAdapter(AppThemeAdapter):
    id = "cursor"


class ZedAdapter(AppThemeAdapter):
    id = "zed"


class ObsidianAdapter(AppThemeAdapter):
    id = "obsidian"

    def vaults(self):
        return [Path(path) for path in _request("vaults")]


class BtopAdapter(AppThemeAdapter):
    id = "btop"


class KittyAdapter(AppThemeAdapter):
    id = "kitty"


class AlacrittyAdapter(AppThemeAdapter):
    id = "alacritty"


class DiscordAdapter(AppThemeAdapter):
    id = "discord"


class TelegramAdapter(AppThemeAdapter):
    id = "telegram"


def backup_file(path):
    value = _request("backup", path=str(path))
    return Path(value) if value else None


ADAPTERS = {adapter.id: adapter for adapter in (
    ZedAdapter(), VSCodeAdapter(), CursorAdapter(), ObsidianAdapter(),
    DiscordAdapter(), TelegramAdapter(), BtopAdapter(), KittyAdapter(), AlacrittyAdapter(),
)}


def adapter_for(app_id):
    return ADAPTERS.get(app_id)


__all__ = ["ADAPTERS", "AdapterTheme", "AppThemeAdapter", "adapter_for", "backup_file"]
