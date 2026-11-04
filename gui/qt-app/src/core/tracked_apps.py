"""
Tracked Applications Registry — полный список отслеживаемых приложений
(общая с Rust-ядром схема компонент → конфиги).
"""

from dataclasses import dataclass
import json
import shutil
import subprocess
from typing import Optional
from .paths import live_path, theme_root
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


# Порядок = порядок в UI
TRACKED_APPS: tuple[TrackedAppDef, ...] = (
    # === Базовые KDE/Plasma (всегда доступны) ===
    TrackedAppDef(
        id="kwin",
        name="KWin",
        category="KDE / Plasma",
        config_paths=(
            ".config/kwinrc",
            ".config/kwinrulesrc",
        ),
        description="KWin window manager: effects, rules, blur, glass",
        icon="preferences-system-windows",
    ),
    TrackedAppDef(
        id="plasma_shell",
        name="Plasma Shell",
        category="KDE / Plasma",
        config_paths=(
            ".config/plasmarc",
            ".config/plasmashellrc",
            ".config/plasma-org.kde.plasma.desktop-appletsrc",
        ),
        description="Plasma panels, widgets, desktop applets",
        icon="plasma",
    ),
    TrackedAppDef(
        id="kdeglobals",
        name="KDE Globals",
        category="KDE / Plasma",
        # kcminputrc: only the cursor theme/size (appearance.rs).
        config_paths=(".config/kdeglobals", ".config/kcminputrc"),
        description="Colors, accent, Look & Feel, color scheme, widget style",
        icon="preferences-desktop-theme",
    ),

    # === GTK ===
    TrackedAppDef(
        id="gtk3",
        name="GTK3",
        category="GTK / Qt",
        config_paths=(
            ".config/gtk-3.0/settings.ini",
            ".config/gtk-3.0/gtk.css",
            ".config/gtk-3.0/colors.css",
        ),
        description="GTK3 theme, custom CSS, colors",
        icon="preferences-desktop-theme-gtk",
    ),
    TrackedAppDef(
        id="gtk4",
        name="GTK4",
        category="GTK / Qt",
        config_paths=(
            ".config/gtk-4.0/settings.ini",
            ".config/gtk-4.0/gtk.css",
            ".config/gtk-4.0/colors.css",
        ),
        description="GTK4 theme, custom CSS, colors",
        icon="preferences-desktop-theme-gtk",
    ),

    # === Qt ===
    TrackedAppDef(
        id="qt5ct",
        name="Qt5/Qt6 Style",
        category="GTK / Qt",
        # QtProject.conf is file-dialog state, not appearance (see the Rust registry).
        config_paths=(".config/Trolltech.conf",),
        description="Qt5/Qt6 widget style, font, icon theme",
        icon="qtlogo",
    ),

    # === Window Decorations ===
    TrackedAppDef(
        id="breeze",
        name="Breeze style",
        category="Window Decorations",
        config_paths=(".config/breezerc",),
        description="Breeze widget style: menu transparency and other style options",
        icon="preferences-desktop-theme",
    ),
    TrackedAppDef(
        id="klassy",
        name="Klassy",
        category="Window Decorations",
        config_paths=(
            ".config/klassy/klassyrc",
            ".config/klassy/windecopresetsrc",
        ),
        detect_command="klassy",
        description="KWin decoration: titlebar opacity, corner radius, buttons",
        icon="preferences-system-windows",
        docs_url="https://github.com/paul-g/klassy",
    ),
    TrackedAppDef(
        id="kvantum",
        name="Kvantum",
        category="Window Decorations",
        config_paths=(
            ".config/Kvantum/kvantum.kvconfig",
            ".config/Kvantum/Sandstone",
        ),
        detect_command="kvantummanager",
        description="SVG-based theme engine for Qt/KDE",
        icon="preferences-system-windows",
        docs_url="https://github.com/tsujan/Kvantum",
    ),

    # === Terminals ===
    TrackedAppDef(
        id="kitty",
        name="Kitty",
        category="Terminals",
        # kitty.conf is an include host; the look lives in theme-profile.conf.
        config_paths=(".config/kitty/kitty.conf", ".config/kitty/theme-profile.conf"),
        detect_command="kitty",
        supports_live_reload=True,
        description="GPU-accelerated terminal with live config reload",
        icon="utilities-terminal",
        docs_url="https://sw.kovidgoyal.net/kitty/",
    ),
    TrackedAppDef(
        id="alacritty",
        name="Alacritty",
        category="Terminals",
        config_paths=(
            ".config/alacritty/alacritty.toml",
            ".config/alacritty/theme-profile.toml",
            ".config/alacritty/themes/noctalia.toml",
        ),
        detect_command="alacritty",
        description="GPU-accelerated terminal, TOML config",
        icon="utilities-terminal",
        docs_url="https://github.com/alacritty/alacritty",
    ),

    # === Launchers / Menu ===
    TrackedAppDef(
        id="wofi",
        name="Wofi",
        category="Launchers / Menu",
        config_paths=(
            ".config/wofi/style.css",
            ".config/wofi/wallpaper.css",
        ),
        detect_command="wofi",
        description="Application launcher / menu (GTK)",
        icon="system-run",
        docs_url="https://hg.sr.ht/~scoopta/wofi",
    ),

    # === Monitors ===
    TrackedAppDef(
        id="btop",
        name="btop",
        category="Monitors",
        config_paths=(
            ".config/btop/btop.conf",
            ".config/btop/themes/noctalia.theme",
            ".config/btop/themes/paper.theme",
        ),
        detect_command="btop",
        description="Resource monitor with themes",
        icon="utilities-system-monitor",
        docs_url="https://github.com/aristocratos/btop",
    ),

    # === Sysinfo ===
    TrackedAppDef(
        id="fastfetch",
        name="Fastfetch",
        category="Sysinfo",
        config_paths=(
            ".config/fastfetch/config.jsonc",
            ".config/fastfetch/logo",
            ".config/fastfetch/cat-source.jpg",
        ),
        detect_command="fastfetch",
        description="System info tool with custom logo/config",
        icon="preferences-desktop-theme",
        docs_url="https://github.com/fastfetch-cli/fastfetch",
    ),

    # === Editors ===
    TrackedAppDef(
        id="zed",
        name="Zed",
        category="Editors",
        config_paths=(
            ".config/zed/settings.json",
            ".config/zed/themes",
        ),
        detect_command="zeditor|zed",
        description="Code editor with native themes",
        icon="text-editor",
        docs_url="https://zed.dev/",
    ),
    TrackedAppDef(
        id="vscode",
        name="VS Code",
        category="Editors",
        config_paths=(".config/Code/User/settings.json",),
        detect_command="code|code-oss|codium|vscodium",
        description="VS Code settings (theme, font, etc.)",
        icon="visual-studio-code",
        docs_url="https://code.visualstudio.com/",
    ),
    TrackedAppDef(
        id="cursor",
        name="Cursor",
        category="Editors",
        config_paths=(".config/Cursor/User/settings.json",),
        detect_command="cursor",
        description="Cursor editor settings",
        icon="text-editor",
        docs_url="https://cursor.sh/",
    ),

    # === Browser ===
    TrackedAppDef(
        id="zen",
        name="Zen Browser",
        category="Browser",
        config_paths=(
            # Legacy fallback; the live paths come from zen_config_paths()
            # (auto-detected profile directory).
            ".zen/gp0y8wrv.Default (release)/user.js",
        ),
        flatpak_id="app.zen_browser.zen",
        description="Firefox-based browser with user.js + userChrome.css theming",
        icon="web-browser",
        docs_url="https://zen-browser.app/",
    ),

    # === Notes ===
    TrackedAppDef(
        id="obsidian",
        name="Obsidian",
        category="Notes",
        # obsidian.json (vault list) / Preferences (spellcheck) are not appearance.
        config_paths=("obsidian/vault/.obsidian/appearance.json",),
        detect_command="obsidian",
        flatpak_id="md.obsidian.Obsidian",
        description="Markdown notes with theme/appearance settings",
        icon="x-office-note",
        docs_url="https://obsidian.md/",
    ),

    # === System Profiles ===
    TrackedAppDef(
        id="color_schemes",
        name="Color Schemes",
        category="System Profiles",
        config_paths=(".local/share/color-schemes",),
        description="KDE color scheme files",
        icon="preferences-desktop-color",
    ),
    TrackedAppDef(
        id="palette",
        name="Palette",
        category="System Profiles",
        config_paths=(".config/theme-profile/palette.json",),
        description="base16 palette the generated theme was built from",
        icon="color-management",
    ),
    TrackedAppDef(
        id="look_and_feel",
        name="Look & Feel",
        category="System Profiles",
        config_paths=(
            ".local/share/plasma/look-and-feel/glass",
            ".local/share/plasma/look-and-feel/paper",
        ),
        description="Plasma Look & Feel packages (glass/paper)",
        icon="preferences-desktop-theme",
    ),
)


# ──────────────────────────────────────────────────────────────
# Пользовательские компоненты (components.json)
# ──────────────────────────────────────────────────────────────

def _is_valid_component_id(comp_id: str) -> bool:
    from .paths import is_valid_profile_name

    return is_valid_profile_name(comp_id)


def _is_valid_rel_path(p: str) -> bool:
    return bool(p) and not p.startswith("/") and ".." not in p


# Зеркало RELOAD_COMMAND_FORBIDDEN в Rust (profile.rs) и контроллеров.
_RELOAD_FORBIDDEN = frozenset("|&;<>$`\"'\n\r")


def load_custom_defs() -> list[TrackedAppDef]:
    """Прочитать components.json и вернуть валидные определения.

    Зеркало Rust `custom_components::load_custom_components`: одна битая
    запись обнуляет весь пользовательский список (встроенные не трогаются).
    При совпадении id со встроенным компонентом встроенный выигрывает
    (см. all_app_defs — зеркало Rust merged_component_defs).
    """
    path = theme_root() / "components.json"
    if not path.is_file():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        entries = data.get("components", [])
        if not isinstance(entries, list):
            return []
    except Exception:
        return []
    out: list[TrackedAppDef] = []
    try:
        for entry in entries:
            comp_id = str(entry.get("id", ""))
            paths = [str(p) for p in entry.get("config_paths", [])]
            if not _is_valid_component_id(comp_id) or not paths:
                return []
            if any(not _is_valid_rel_path(p) for p in paths):
                return []
            command = str(entry.get("reload_command", "") or "")
            if command.strip():
                bad = next(
                    (ch for ch in command if ch in _RELOAD_FORBIDDEN), None
                )
                if bad is not None or shutil.which(command.split()[0]) is None:
                    return []
            out.append(
                TrackedAppDef(
                    id=comp_id,
                    name=str(entry.get("name", "")) or comp_id,
                    category=str(entry.get("category", "")) or "Custom",
                    config_paths=tuple(paths),
                    detect_command=None,
                    flatpak_id=None,
                    supports_live_reload=bool(entry.get("supports_live_reload", False))
                    or bool(command.strip()),
                    description=str(entry.get("description", "")),
                    icon=str(entry.get("icon", "")) or "application-x-executable",
                    docs_url="",
                    reload_command=command,
                    is_custom=True,
                )
            )
    except Exception:
        return []
    return out


def all_app_defs() -> list[TrackedAppDef]:
    """Встроенный реестр + пользовательские; встроенные выигрывают по id."""
    built_ids = {a.id for a in TRACKED_APPS}
    custom = [d for d in load_custom_defs() if d.id not in built_ids]
    return [*TRACKED_APPS, *custom]


# ──────────────────────────────────────────────────────────────
# Детекция статусов
# ──────────────────────────────────────────────────────────────

def _check_command(cmd: str) -> bool:
    """Проверить наличие команды в PATH; `a|b` — любая из альтернатив
    (Zed — `zeditor` на Arch/CachyOS, VS Code — `code`/`codium`…)."""
    return any(
        shutil.which(alt.split()[0]) is not None
        for alt in cmd.split("|")
        if alt.strip()
    )


def _check_flatpak(flatpak_id: str) -> bool:
    """Проверить установлен ли Flatpak."""
    try:
        result = subprocess.run(
            ["flatpak", "list", "--app", "--columns=application"],
            capture_output=True, text=True, timeout=5
        )
        return flatpak_id in result.stdout
    except Exception:
        return False


def _check_configs_exist(paths: tuple[str, ...]) -> bool:
    """Проверить наличие хотя бы одного конфига (с учётом XDG)."""
    return any(live_path(p).exists() for p in paths)


# ──────────────────────────────────────────────────────────────
# Zen Browser auto-detection (shared rules with the Rust core)
# ──────────────────────────────────────────────────────────────

def zen_profile_dir() -> str:
    """Имя профиля Zen внутри `~/.zen` (первый по byte-order с user.js).

    Раньше путь был хардкоднут (`.zen/gp0y8wrv.Default (release)/`), что
    ломалось после переименования профиля. Правила зеркалят Rust:
    `~/.zen/*Default*/user.js` → первый в byte-order → иначе legacy-имя.
    """
    import os

    zen_root = os.path.join(os.path.expanduser("~"), ".zen")
    candidates = []
    try:
        for entry in os.scandir(zen_root):
            if entry.is_dir(follow_symlinks=False) and "Default" in entry.name:
                if os.path.isfile(os.path.join(entry.path, "user.js")):
                    candidates.append(entry.name)
    except OSError:
        pass
    if candidates:
        return sorted(candidates)[0]
    return "gp0y8wrv.Default (release)"


def zen_config_paths() -> tuple[str, ...]:
    """Относительные пути конфигов Zen для обнаруженного профиля."""
    base = f".zen/{zen_profile_dir()}"
    return (
        f"{base}/user.js",
        f"{base}/chrome/userChrome.css",
        f"{base}/chrome/userContent.css",
        f"{base}/chrome/zen-themes.css",
    )


def config_paths_for(app: TrackedAppDef) -> tuple[str, ...]:
    """Актуальные пути приложения (zen — авто-детект, остальные — static)."""
    if app.id == "zen":
        return zen_config_paths()
    return app.config_paths


def detect_app_status(app: TrackedAppDef) -> AppStatus:
    """Определить статус приложения."""
    # 1. Проверка команды установки
    if app.detect_command and not _check_command(app.detect_command):
        # 2. Проверка Flatpak
        if app.flatpak_id and not _check_flatpak(app.flatpak_id):
            return AppStatus.NOT_INSTALLED
        elif not app.flatpak_id:
            return AppStatus.NOT_INSTALLED

    # 3. Проверка конфигов
    if _check_configs_exist(config_paths_for(app)):
        return AppStatus.AVAILABLE
    else:
        # Установлено, но конфигов ещё нет (первый запуск)
        return AppStatus.PARTIAL


def build_component(app: TrackedAppDef) -> Component:
    """Построить Component из определения + актуальный статус."""
    status = detect_app_status(app)
    return Component(
        id=app.id,
        name=app.name,
        category=app.category,
        config_paths=config_paths_for(app),
        status=status,
        supports_live_reload=app.supports_live_reload,
        description=app.description,
        icon=app.icon,
        docs_url=app.docs_url,
        is_custom=app.is_custom,
        reload_command=app.reload_command,
    )


def detect_all_apps() -> list[Component]:
    """Получить список всех компонентов с актуальными статусами."""
    return [build_component(app) for app in all_app_defs()]


# ──────────────────────────────────────────────────────────────
# Реализация TrackedAppsCore
# ──────────────────────────────────────────────────────────────

class CliTrackedAppsCore(TrackedAppsCore):
    """Локальная реализация реестра отслеживаемых приложений."""

    def __init__(self):
        self._cache: Optional[list[Component]] = None

    def _run(self, args: list[str], timeout: int = 30) -> Result:
        import subprocess, os
        try:
            proc = subprocess.run(args, capture_output=True, text=True, timeout=timeout, env={**os.environ})
            return Result(
                proc.returncode == 0,
                proc.stdout.strip() or proc.stderr.strip()
            )
        except Exception as e:
            return Result.err(str(e))

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
        args = ["theme-profile", "components", "remove", comp_id]
        res = self._run(args, timeout=30)
        if res.success:
            return Result.ok()
        return Result.err(res.message or "failed to remove custom component")

    # ── Пользовательские компоненты через CLI (components.json) ─────────────

    @staticmethod
    def _parse_custom_component_line(line: str) -> Optional[CustomComponent]:
        """Parse a single id line from 'theme-profile components list' output."""
        parts = line.strip().split()
        if not parts:
            return None
        return CustomComponent(id=parts[0], name="", category="Custom")

    def list_custom(self) -> list[CustomComponent]:
        """Все пользовательские компоненты из components.json."""
        args = ["theme-profile", "components", "list"]
        res = self._run(args, timeout=30)
        if not res.success:
            return []
        comps: list[CustomComponent] = []
        for line in res.message.strip().splitlines():
            c = self._parse_custom_component_line(line)
            if c:
                comps.append(c)
        return comps

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
        args = ["theme-profile", "components", "add", self._custom_component_payload(comp)]
        res = self._run(args, timeout=30)
        if res.success:
            return Result.ok()
        return Result.err(res.message or "failed to add custom component")

    def update_custom(self, comp: CustomComponent) -> Result:
        """Обновить пользовательский компонент."""
        args = ["theme-profile", "components", "edit", self._custom_component_payload(comp)]
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