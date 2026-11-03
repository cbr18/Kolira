"""
Core Protocol (Abstract Base Classes) — КОНТРАКТ между UI и Core.

Этот модуль НЕ МЕНЯЕТСЯ при замене CliCore на RustCore.
UI и Controllers зависят только от этих абстракций.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional
from pathlib import Path
import os
import sys


class AppStatus(Enum):
    """Статус доступности приложения/компонента."""
    AVAILABLE = "available"          # Установлен, конфиги найдены
    NOT_INSTALLED = "not_installed"  # Не установлен
    PARTIAL = "partial"              # Установлен, но часть конфигов отсутствует


@dataclass(frozen=True, slots=True)
class Component:
    """Один отслеживаемый компонент (приложение или группа настроек)."""
    id: str
    name: str
    category: str
    config_paths: tuple[str, ...]
    status: AppStatus
    supports_live_reload: bool = False
    description: str = ""
    # Доп. метаданные для UI
    icon: str = ""           # имя иконки (Kirigami/Theme)
    docs_url: str = ""       # ссылка на документацию
    is_custom: bool = False  # зарегистрирован пользователем (components.json)
    reload_command: str = "" # команда перезагрузки после apply (пользовательские)


@dataclass(slots=True)
class CustomComponent:
    """Пользовательский компонент в components.json."""
    id: str
    name: str = ""
    category: str = "Custom"
    config_paths: tuple[str, ...] = ()
    reload_command: str = ""
    supports_live_reload: bool = False
    description: str = ""
    icon: str = ""


@dataclass(frozen=True, slots=True)
class ProfileInfo:
    """Информация о профиле."""
    name: str
    is_active: bool
    components: tuple[str, ...] = field(default_factory=tuple)  # какие компоненты в профиле
    created_at: str = ""
    description: str = ""
    # Метаданные
    wallpaper_path: str = ""


@dataclass(frozen=True, slots=True)
class Result:
    """Результат операции."""
    success: bool
    message: str
    data: Optional[dict] = None

    @classmethod
    def ok(cls, message: str = "", data: Optional[dict] = None) -> "Result":
        return cls(True, message, data)

    @classmethod
    def err(cls, message: str, data: Optional[dict] = None) -> "Result":
        return cls(False, message, data)


# ──────────────────────────────────────────────────────────────
# Протоколы (Abstract Base Classes)
# ──────────────────────────────────────────────────────────────

class ProfileCore(ABC):
    """Управление профилями тем."""

    @abstractmethod
    def list_profiles(self) -> list[ProfileInfo]:
        """Список всех профилей."""

    @abstractmethod
    def get_active_profile(self) -> str:
        """Имя активного профиля."""

    @abstractmethod
    def apply(self, profile: str, components: Optional[list[str]] = None) -> Result:
        """
        Применить профиль.
        components=None → всё (по умолчанию).
        components=['kwin', 'gtk3'] → только указанные.
        """

    @abstractmethod
    def save(self, name: str, components: Optional[list[str]] = None) -> Result:
        """Сохранить текущее состояние как профиль."""

    @abstractmethod
    def delete(self, name: str) -> Result:
        """Удалить профиль."""

    @abstractmethod
    def clone(self, source: str, target: str) -> Result:
        """Дублировать профиль: копия снимка под новым именем."""

    @abstractmethod
    def set_description(self, name: str, description: str) -> Result:
        """Установить/сменить описание профиля (manifest.json)."""

    @abstractmethod
    def diff(self, a: str, b: str) -> str:
        """Сравнить два профиля (raw diff)."""

    @abstractmethod
    def get_profile_components(self, profile: str) -> list[str]:
        """Какие компоненты сохранены в профиле."""


class WallpaperCore(ABC):
    """Управление обоями."""

    @abstractmethod
    def pick(self) -> Optional[str]:
        """Открыть пикер, вернуть путь к выбранным обоям."""

    @abstractmethod
    def next(self) -> Result:
        """Следующие обои."""

    @abstractmethod
    def prev(self) -> Result:
        """Предыдущие обои."""

    @abstractmethod
    def current(self) -> Optional[str]:
        """Текущие обои (путь)."""

    @abstractmethod
    def list_wallpapers(self) -> list[str]:
        """Список всех обоев в директории."""

    @abstractmethod
    def set_wallpaper(self, path: str) -> Result:
        """Установить конкретные обои."""


class TrackedAppsCore(ABC):
    """Реестр отслеживаемых приложений."""

    @abstractmethod
    def detect_all(self) -> list[Component]:
        """Детект всех приложений, возврат списка Component с актуальным статусом."""

    @abstractmethod
    def get_categories(self) -> list[str]:
        """Список уникальных категорий."""

    @abstractmethod
    def get_components_by_category(self, category: str) -> list[Component]:
        """Компоненты по категории."""

    @abstractmethod
    def get_all_components(self) -> list[Component]:
        """Все компоненты (кэшированный или свежий)."""

    @abstractmethod
    def refresh(self) -> list[Component]:
        """Принудительный рефреш детекта."""

    # Custom components (components.json)
    @abstractmethod
    def list_custom(self) -> list[CustomComponent]:
        """Все пользовательские компоненты."""

    @abstractmethod
    def add_custom(self, comp: CustomComponent) -> Result:
        """Добавить пользовательский компонент."""

    @abstractmethod
    def update_custom(self, comp: CustomComponent) -> Result:
        """Обновить пользовательский компонент."""

    @abstractmethod
    def remove_custom(self, comp_id: str) -> Result:
        """Удалить пользовательский компонент."""


class SettingsCore(ABC):
    """Настройки самого приложения (user preferences)."""

    @abstractmethod
    def load(self) -> "AppSettings":
        """Загрузить настройки."""

    @abstractmethod
    def save(self, settings: "AppSettings") -> Result:
        """Сохранить настройки."""

    @abstractmethod
    def get_defaults(self) -> "AppSettings":
        """Дефолтные настройки."""


# ──────────────────────────────────────────────────────────────
# Theme Inspector (экран «Тема»)
# ──────────────────────────────────────────────────────────────

@dataclass(frozen=True, slots=True)
class Choice:
    """Одно значение enum-поля (например, установленная цветовая схема)."""
    value: str
    label: str = ""

    def __post_init__(self) -> None:
        if not self.label:
            object.__setattr__(self, "label", self.value)


@dataclass(frozen=True, slots=True)
class ThemeSection:
    """Раздел инспектора темы."""
    id: str
    title: str
    icon: str = ""
    description: str = ""
    field_count: int = 0
    issue_count: int = 0
    summary: str = ""


@dataclass(frozen=True, slots=True)
class ThemeFieldValue:
    """Прочитанное значение одного параметра темы."""
    id: str
    section: str
    label: str
    type: str
    raw: object
    display: str
    is_default: bool = False
    differs_from_profile: bool = False
    profile_raw: object = None
    profile_display: str = ""
    badge: str = "live"
    description: str = ""
    file: str = ""
    group: str = ""
    key: str = ""
    choices: tuple[Choice, ...] = field(default_factory=tuple)
    min: Optional[float] = None
    max: Optional[float] = None
    step: Optional[float] = None
    suffix: str = ""
    tracked_in_profile: bool = True
    advanced: bool = False
    confirm: bool = False
    staged: bool = False

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "section": self.section,
            "label": self.label,
            "type": self.type,
            "value": self.raw,
            "display": self.display,
            "isDefault": self.is_default,
            "differs": self.differs_from_profile,
            "staged": self.staged,
            "profileValue": self.profile_raw,
            "profileDisplay": self.profile_display,
            "badge": self.badge,
            "description": self.description,
            "file": self.file,
            "group": self.group,
            "key": self.key,
            "choices": [
                {"value": c.value, "label": c.label} for c in self.choices
            ],
            "min": self.min,
            "max": self.max,
            "step": self.step,
            "suffix": self.suffix,
            "trackedInProfile": self.tracked_in_profile,
            "advanced": self.advanced,
            "confirm": self.confirm,
        }


@dataclass(frozen=True, slots=True)
class ThemeIssue:
    """Проблема, найденная валидатором темы."""
    severity: str          # "error" | "warning" | "info"
    field_id: str
    message: str
    fix_hint: str = ""

    def to_dict(self) -> dict:
        return {
            "severity": self.severity,
            "fieldId": self.field_id,
            "message": self.message,
            "fixHint": self.fix_hint,
        }


class ThemeCore(ABC):
    """Инспектор темы: чтение, запись и валидация параметров оформления.

    ``scope`` — ``"live"`` (текущая система; запись разрешена) или
    ``"profile:<name>"`` (снимок профиля; только чтение).
    """

    @abstractmethod
    def sections(self, scope: str = "live") -> list[ThemeSection]:
        """Разделы инспектора с кратким summary."""

    @abstractmethod
    def fields(self, section: str, scope: str = "live") -> list[dict]:
        """Поля раздела (QML-ready dicts)."""

    @abstractmethod
    def get(self, field_id: str, scope: str = "live") -> Optional[dict]:
        """Одно поле по id."""

    @abstractmethod
    def choices(self, field_id: str) -> list[Choice]:
        """Доступные значения для enum-поля."""

    @abstractmethod
    def font_families(self) -> list[str]:
        """Семейства шрифтов, установленные в системе."""

    @abstractmethod
    def validate(self) -> list[ThemeIssue]:
        """Проверить значения на существование целей и конфликты."""

    @abstractmethod
    def search(self, query: str, scope: str = "live") -> list[dict]:
        """Поиск по label/id/ключу/значению."""

    @abstractmethod
    def set_value(self, field_id: str, value: object) -> Result:
        """Застейджить правку поля (валидация без записи; применится по apply)."""

    @abstractmethod
    def apply(self) -> Result:
        """Применить все застейдженные правки одним проходом."""

    @abstractmethod
    def reset(self, field_id: str, to: str = "default") -> Result:
        """Сбросить поле к дефолту или к значению активного профиля."""

    @abstractmethod
    def set_config_value(self, rel_path: str, key: str, value: str) -> Result:
        """Застейджить правку ключа конфига приложения (только live; этап C2).

        ``key`` — flattened-форма из read_config_params (INI "group/key",
        JSON "a/b"). Запись участвует в undo/revert сессии.
        """

    @abstractmethod
    def undo(self) -> Result:
        """Отменить последнее изменение сессии."""

    @abstractmethod
    def revert(self) -> Result:
        """Вернуть все файлы сессии к снимку."""

    @abstractmethod
    def session_changes(self) -> int:
        """Сколько неприменённых изменений в текущей сессии."""

    @abstractmethod
    def undo_depth(self) -> int:
        """Сколько применённых изменений можно отменить."""


@dataclass(frozen=True, slots=True)
class AppSettings:
    """Настройки приложения (user preferences)."""
    # Режим применения профиля
    default_components_mode: str = "all"        # "all" | "custom"
    custom_components: tuple[str, ...] = field(default_factory=tuple)

    # Обои
    wallpaper_linked_to_profile: bool = False
    # "" = auto: $WALLPAPER_DIR → XDG Pictures/Wallpaper (resolved by the Rust CLI)
    wallpaper_directory: str = ""

    # UI
    theme: str = "system"                       # "system" | "glass" | "paper" | "dark" | "light"
    show_not_installed_warning: bool = True
    # Online features (base16 gallery catalogue, app theme stores). Read by the
    # Rust CLI from app-preferences.json; $KTM_OFFLINE=1 forces offline.
    network_enabled: bool = True
    auto_apply_on_startup: bool = False
    minimize_to_tray: bool = False
    global_hotkey: str = ""                     # e.g. "Meta+T"

    # Поведение
    confirm_before_apply: bool = True
    create_backup_before_apply: bool = True
    notify_on_success: bool = True

    # Какое ядро использовать при старте: "core" | "auto" (одно и то же ядро)
    # (переопределяется флагом --core и переменной окружения KTM_CORE).
    core_backend: str = "core"

    # Компоненты, снятые галочкой на странице «Приложения» (id). Исключены из
    # save/apply, но не удалены: файлы и снимки профилей не трогаются.
    disabled_components: tuple[str, ...] = field(default_factory=tuple)

    def __post_init__(self) -> None:
        backend = str(self.core_backend or "core").strip().lower()
        if backend not in ("core", "auto"):
            backend = "core"
        if backend != self.core_backend:
            object.__setattr__(self, "core_backend", backend)

    def to_dict(self) -> dict:
        return {
            "default_components_mode": self.default_components_mode,
            "custom_components": list(self.custom_components),
            "wallpaper_linked_to_profile": self.wallpaper_linked_to_profile,
            "wallpaper_directory": self.wallpaper_directory,
            "theme": self.theme,
            "show_not_installed_warning": self.show_not_installed_warning,
            "network_enabled": self.network_enabled,
            "auto_apply_on_startup": self.auto_apply_on_startup,
            "minimize_to_tray": self.minimize_to_tray,
            "global_hotkey": self.global_hotkey,
            "confirm_before_apply": self.confirm_before_apply,
            "create_backup_before_apply": self.create_backup_before_apply,
            "notify_on_success": self.notify_on_success,
            "core_backend": self.core_backend,
            "disabled_components": list(self.disabled_components),
        }

    @classmethod
    def from_dict(cls, data: dict) -> "AppSettings":
        return cls(
            default_components_mode=data.get("default_components_mode", "all"),
            custom_components=tuple(data.get("custom_components", [])),
            wallpaper_linked_to_profile=data.get("wallpaper_linked_to_profile", False),
            wallpaper_directory=data.get("wallpaper_directory", ""),
            theme=data.get("theme", "system"),
            show_not_installed_warning=data.get("show_not_installed_warning", True),
            network_enabled=bool(data.get("network_enabled", True)),
            auto_apply_on_startup=data.get("auto_apply_on_startup", False),
            minimize_to_tray=data.get("minimize_to_tray", False),
            global_hotkey=data.get("global_hotkey", ""),
            confirm_before_apply=data.get("confirm_before_apply", True),
            create_backup_before_apply=data.get("create_backup_before_apply", True),
            notify_on_success=data.get("notify_on_success", True),
            core_backend=data.get("core_backend", "core"),
            disabled_components=tuple(data.get("disabled_components", [])),
        )


# ──────────────────────────────────────────────────────────────
# Фабрика для получения реализации Core
# ──────────────────────────────────────────────────────────────

class CoreFactory:
    """
    Фабрика для получения реализаций Core протоколов.
    Позволяет подменять реализацию (CliCore → RustCore) в одной точке.

    Ядро выбирается флагом ``--core`` / переменной ``KTM_CORE`` / настройкой
    ``core_backend``; см. :meth:`set_backend`.
    """

    #: Доступные бэкенды. Теперь только core (бывший cli).
    BACKENDS = ("core", "auto")
    #: Переменная окружения, переопределяющая настройку (но не флаг --core).
    ENV_VAR = "KTM_CORE"

    _profile_core: Optional[ProfileCore] = None
    _wallpaper_core: Optional[WallpaperCore] = None
    _tracked_apps_core: Optional[TrackedAppsCore] = None
    _settings_core: Optional[SettingsCore] = None
    _theme_core: Optional["ThemeCore"] = None

    # Пока ``_select_core`` не выбран явно, безопасный локальный бэкенд —
    # core — единое ядро (бывший cli).
    _backend: str = "core"

    # ── Выбор ядра ─────────────────────────────────────────────

    @classmethod
    def normalize_backend(cls, backend: str) -> str:
        """Проверить и нормализовать имя бэкенда."""
        name = (backend or "").strip().lower()
        # Написания эпохи двух ядер теперь именуют единственное ядро.
        if name in ("rust", "cli"):
            name = "core"
        if name not in cls.BACKENDS:
            raise ValueError(
                f"Unknown core backend {backend!r}; expected one of "
                f"{', '.join(cls.BACKENDS)}"
            )
        return name

    @classmethod
    def set_backend(cls, backend: str, strict: bool = True) -> str:
        """Выбрать ядро и сбросить кэш реализаций.

        ``strict`` оставлен для совместимости сигнатуры: неизвестный бэкенд
        и так падает в :meth:`normalize_backend` с ``ValueError``. Единственное
        ядро — ``core``; ``auto`` и наследие ``rust``/``cli`` нормализуются
        в него же. Возвращает фактически используемый бэкенд.
        """
        name = cls.normalize_backend(backend)

        if name == "auto":
            name = "core"

        cls._backend = name
        cls.reset()
        return cls._backend

    @classmethod
    def set_backend_from_env(cls, strict: bool = True, default: str = "core") -> str:
        """Выбрать ядро из ``KTM_CORE`` (если задана), иначе ``default``."""
        requested = os.environ.get(cls.ENV_VAR, "").strip() or default
        return cls.set_backend(requested, strict=strict)

    @classmethod
    def backend(cls) -> str:
        """Фактически используемое ядро."""
        return cls._backend

    @classmethod
    def backend_label(cls) -> str:
        """Человекочитаемое имя ядра для UI."""
        return "Core"

    @classmethod
    def _build(cls, kind: str):
        """Создать реализацию нужного ядра."""
        from .cli_core import CLI_FACTORIES
        return CLI_FACTORIES[kind]()

    # ── Ручная подмена (тесты) ─────────────────────────────────

    @classmethod
    def set_profile_core(cls, core: ProfileCore) -> None:
        cls._profile_core = core

    @classmethod
    def set_wallpaper_core(cls, core: WallpaperCore) -> None:
        cls._wallpaper_core = core

    @classmethod
    def set_tracked_apps_core(cls, core: TrackedAppsCore) -> None:
        cls._tracked_apps_core = core

    @classmethod
    def set_settings_core(cls, core: SettingsCore) -> None:
        cls._settings_core = core

    @classmethod
    def profile_core(cls) -> ProfileCore:
        if cls._profile_core is None:
            cls._profile_core = cls._build("profile")
        return cls._profile_core

    @classmethod
    def wallpaper_core(cls) -> WallpaperCore:
        if cls._wallpaper_core is None:
            cls._wallpaper_core = cls._build("wallpaper")
        return cls._wallpaper_core

    @classmethod
    def tracked_apps_core(cls) -> TrackedAppsCore:
        if cls._tracked_apps_core is None:
            cls._tracked_apps_core = cls._build("tracked_apps")
        return cls._tracked_apps_core

    @classmethod
    def settings_core(cls) -> SettingsCore:
        if cls._settings_core is None:
            cls._settings_core = cls._build("settings")
        return cls._settings_core

    @classmethod
    def theme_core(cls) -> "ThemeCore":
        """Инспектор темы.

        Не является выбираемым бэкендом ``--core``: это отдельный Python-
        модуль приложения (чтение/запись конфигов и D-Bus-хуки), где Rust
        сейчас не даёт выигрыша. Контракт задокументирован в TZ; при
        стабилизации схемы можно добавить FFI.
        """
        if cls._theme_core is None:
            from .theme_core import AppThemeCore

            cls._theme_core = AppThemeCore()
        return cls._theme_core

    @classmethod
    def reset(cls) -> None:
        """Сброс для тестов или переинициализации."""
        cls._profile_core = None
        cls._wallpaper_core = None
        cls._tracked_apps_core = None
        cls._settings_core = None
        cls._theme_core = None