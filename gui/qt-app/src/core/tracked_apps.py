"""
Tracked Applications Registry — полный список отслеживаемых приложений
(общая с Rust-ядром схема компонент → конфиги).
"""

from dataclasses import dataclass
import json
from typing import Optional
from .protocol import Component, AppStatus, TrackedAppsCore, CustomComponent, Result


# ──────────────────────────────────────────────────────────────
# Определение всех отслеживаемых компонентов
# ──────────────────────────────────────────────────────────────

@dataclass(frozen=True, slots=True)
class TrackedAppDef:
    """Определение отслеживаемого приложения (статические метаданные)."""
    id: str
    name: str
    category: str
    config_paths: tuple[str, ...]
    detect_command: Optional[str] = None          # Команда для проверки установки
    flatpak_id: Optional[str] = None              # Flatpak ID если есть
    supports_live_reload: bool = False
    description: str = ""
    icon: str = ""
    docs_url: str = ""
    reload_command: str = ""                      # Пользовательская команда перезагрузки
    is_custom: bool = False                       # Зарегистрирован пользователем


def _request(method, **arguments):
    from .cli_core import core_request
    return core_request("registry." + method, **arguments)


def _definition(data):
    from dataclasses import fields
    names = {field.name for field in fields(TrackedAppDef)}
    values = {k: v for k, v in data.items() if k in names}
    values["config_paths"] = tuple(values["config_paths"])
    return TrackedAppDef(**values)


class _Definitions:
    """Lazy Rust data: importing GUI modules does not invoke the CLI."""
    def __iter__(self):
        return iter([_definition(value) for value in _request("definitions")])

    def __len__(self):
        return len(_request("definitions"))

    def __getitem__(self, index):
        return list(self)[index]


TRACKED_APPS = _Definitions()


def all_app_defs():
    return [_definition(value) for value in _request("definitions", custom=True)]


def load_custom_defs():
    return [app for app in all_app_defs() if app.is_custom]


def config_paths_for(app):
    paths = _request("paths", id=app.id)
    return tuple(app.config_paths if paths is None else paths)


def detect_app_status(app):
    from dataclasses import asdict
    return AppStatus(_request("detect", definition=asdict(app)))


def _component(data):
    return Component(**{**data, "config_paths": tuple(data["config_paths"]),
                        "status": AppStatus(data["status"])})


def build_component(app):
    return Component(id=app.id, name=app.name, category=app.category,
                     config_paths=config_paths_for(app), status=detect_app_status(app),
                     supports_live_reload=app.supports_live_reload, description=app.description,
                     icon=app.icon, docs_url=app.docs_url, is_custom=app.is_custom,
                     reload_command=app.reload_command)


def detect_all_apps():
    return [_component(value) for value in _request("components")]


def _check_command(command):
    return _request("command", command=command)


def _check_configs_exist(paths):
    return _request("configs_exist", paths=list(paths))


def zen_profile_dir():
    return _request("profile", id="zen") or ""


def firefox_profile_dir():
    return _request("profile", id="firefox") or ""


def zen_config_paths():
    return tuple(_request("paths", id="zen"))


def firefox_config_paths():
    return tuple(_request("paths", id="firefox"))


def konsole_config_paths():
    return tuple(_request("paths", id="konsole"))


def discord_config_paths():
    return tuple(_request("paths", id="discord"))


def omp_config_paths():
    return tuple(_request("paths", id="oh-my-posh"))


# ──────────────────────────────────────────────────────────────
# Реализация TrackedAppsCore
# ──────────────────────────────────────────────────────────────

class CliTrackedAppsCore(TrackedAppsCore):
    """Локальная реализация реестра отслеживаемых приложений."""

    def __init__(self):
        self._cache: Optional[list[Component]] = None

    def _run(self, args: list[str], timeout: int = 30) -> Result:
        from .cli_core import run_cli
        return run_cli(args[1:], binary=args[0], timeout=timeout)

    def detect_all(self) -> list[Component]:
        """Свежий детект всех приложений."""
        self._cache = detect_all_apps()
        return self._cache

    def refresh(self) -> list[Component]:
        """Принудительный рефреш (очистка кэша)."""
        self._cache = None
        return self.detect_all()

    def get_all_components(self) -> list[Component]:
        """Все компоненты (из кэша или свежий детект)."""
        if self._cache is None:
            return self.detect_all()
        return self._cache

    def get_categories(self) -> list[str]:
        """Уникальные категории в порядке появления."""
        seen = set()
        categories = []
        for comp in self.get_all_components():
            if comp.category not in seen:
                seen.add(comp.category)
                categories.append(comp.category)
        return categories

    def get_components_by_category(self, category: str) -> list[Component]:
        """Компоненты по категории."""
        return [c for c in self.get_all_components() if c.category == category]

    def remove_custom(self, comp_id: str) -> Result:
        """Удалить пользовательский компонент по id."""
        from .cli_core import resolve_cli_bin
        args = [resolve_cli_bin(), "components", "remove", comp_id]
        res = self._run(args, timeout=30)
        if res.success:
            return Result.ok()
        return Result.err(res.message or "failed to remove custom component")

    # ── Пользовательские компоненты через CLI (components.json) ─────────────

    def list_custom(self) -> list[CustomComponent]:
        """Все пользовательские компоненты из components.json."""
        return [CustomComponent(**{**value, "config_paths": tuple(value["config_paths"]),
                                   "reload_command": value.get("reload_command") or ""})
                for value in _request("custom")]

    @staticmethod
    def _custom_component_payload(comp: CustomComponent) -> str:
        """Serialize for `theme-profile components add|edit`.

        json.dumps, never f-strings: user-entered quotes or newlines in a
        name/description must reach the CLI as valid JSON.
        """
        return json.dumps(
            {
                "id": comp.id,
                "name": comp.name,
                "category": comp.category,
                "config_paths": list(comp.config_paths),
                "reload_command": comp.reload_command,
                "supports_live_reload": comp.supports_live_reload,
                "description": comp.description,
                "icon": comp.icon,
            }
        )

    def add_custom(self, comp: CustomComponent) -> Result:
        """Добавить пользовательский компонент (пишет в components.json)."""
        from .cli_core import resolve_cli_bin
        args = [resolve_cli_bin(), "components", "add", self._custom_component_payload(comp)]
        res = self._run(args, timeout=30)
        if res.success:
            return Result.ok()
        return Result.err(res.message or "failed to add custom component")

    def update_custom(self, comp: CustomComponent) -> Result:
        """Обновить пользовательский компонент."""
        from .cli_core import resolve_cli_bin
        args = [resolve_cli_bin(), "components", "edit", self._custom_component_payload(comp)]
        res = self._run(args, timeout=30)
        if res.success:
            return Result.ok()
        return Result.err(res.message or "failed to update custom component")


# Для обратной совместимости
__all__ = [
    "TRACKED_APPS",
    "TrackedAppDef",
    "detect_all_apps",
    "detect_app_status",
    "build_component",
    "CliTrackedAppsCore",
]
