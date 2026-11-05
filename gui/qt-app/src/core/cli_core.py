"""
CLI Core Implementation — адаптер над Rust CLI.

Профильные операции и навигация по обоям вызывают ``theme-profile`` и
``wallpaper-*`` (Rust-бинари из ``rust_core``); настройки приложения и
реестр отслеживаемых приложений реализованы локально, потому что
отдельных CLI-команд для них нет.
"""

import json
import os
import subprocess
import shutil
from pathlib import Path
from typing import Optional

from .fileutil import atomic_write_text
from .paths import (
    active_profile_file,
    cache_home,
    live_path,
    profile_dir,
    profiles_dir,
    settings_file,
    validate_profile_name,
)
from .protocol import (
    ProfileCore, WallpaperCore, TrackedAppsCore, SettingsCore,
    ProfileInfo, Result, AppSettings, Component, AppStatus, CoreFactory
)
from .tracked_apps import (
    CliTrackedAppsCore, detect_all_apps, config_paths_for, TRACKED_APPS
)


# ──────────────────────────────────────────────────────────────
# Profile Core
# ──────────────────────────────────────────────────────────────

def resolve_cli_bin(name: str = "theme-profile") -> str:
    """Path of a Rust CLI binary: $KTM_THEME_PROFILE_BIN (theme-profile only;
    tests point it at the freshly built one) → PATH → ~/.local/bin."""
    override = os.environ.get("KTM_THEME_PROFILE_BIN")
    if name == "theme-profile" and override:
        return override
    resolved = shutil.which(name)
    if resolved:
        return resolved
    fallback = Path.home() / ".local/bin" / name
    return str(fallback) if fallback.exists() else name


class CliProfileCore(ProfileCore):
    """Обёртка над Rust CLI `theme-profile`."""

    def __init__(self, theme_profile_bin: str = "theme-profile"):
        # Resolve binary path: check PATH, then fallback to ~/.local/bin/
        self.bin = resolve_cli_bin(theme_profile_bin)
        # Cached for compatibility with the existing test-suite; the public
        # properties keep the rest of the code free of direct attribute use.
        self._profiles_dir = profiles_dir()
        self._active_file = active_profile_file()

    @property
    def profiles_dir(self) -> Path:
        return self._profiles_dir

    @property
    def active_file(self) -> Path:
        return self._active_file

    def _run(self, args: list[str], timeout: int = 30) -> Result:
        try:
            proc = subprocess.run(
                [self.bin, *args],
                capture_output=True, text=True, timeout=timeout
            )
            return Result(
                proc.returncode == 0,
                proc.stdout.strip() or proc.stderr.strip()
            )
        except subprocess.TimeoutExpired:
            return Result.err(f"Timeout after {timeout}s")
        except Exception as e:
            return Result.err(str(e))

    def list_profiles(self) -> list[ProfileInfo]:
        res = self._run(["list"])
        if not res.success:
            return []
        names = [n.strip() for n in res.message.split("\n") if n.strip()]
        active = self.get_active_profile()
        profiles = []
        for name in names:
            info = self._get_profile_info(name, name == active)
            profiles.append(info)
        return profiles

    def get_active_profile(self) -> str:
        try:
            active = self.active_file.read_text().strip()
        except Exception:
            return ""
        try:
            return validate_profile_name(active)
        except ValueError:
            return ""

    def _get_profile_info(self, name: str, is_active: bool) -> ProfileInfo:
        """Получить метаданные профиля."""
        try:
            directory = profile_dir(name)
        except ValueError:
            return ProfileInfo(name=str(name), is_active=False, components=())
        created_at = ""
        description = ""
        components = ()
        wallpaper_path = ""

        # Theme package: theme.json describes the snapshot (description,
        # created, wallpaper); components still come from the snapshot files.
        package = directory / "theme.json"
        if package.is_file():
            try:
                data = json.loads(package.read_text(encoding="utf-8"))
                created_at = str(data.get("created") or "")
                description = str(data.get("description") or "")
                wallpaper_path = str(data.get("wallpaper") or "")
            except (OSError, ValueError):
                pass

        # Snapshot manifest.json — предпочтительнее manifest.txt
        manifest_json = directory / "manifest.json"
        if manifest_json.exists():
            try:
                content = manifest_json.read_text(encoding="utf-8")
                data = json.loads(content)
                # theme.json (when present) is authoritative.
                created_at = created_at or data.get("created_at", "")
                description = description or data.get("description", "")
            except Exception:
                pass

        # Fallback to legacy manifest.txt for created_at + description
        # (only for fields manifest.json did not provide).
        manifest = directory / "manifest.txt"
        if manifest.exists():
            try:
                content = manifest.read_text()
                for line in content.split("\n"):
                    if line.startswith("timestamp="):
                        if not created_at:
                            created_at = line.split("=", 1)[1]
                    elif line.startswith("["):
                        break
            except Exception:
                pass

        # Определяем компоненты по наличию директорий в профиле
        comps = []
        for app_def in TRACKED_APPS:
            for rel_path in config_paths_for(app_def):
                if (directory / rel_path).exists():
                    comps.append(app_def.id)
                    break

        return ProfileInfo(
            name=name,
            is_active=is_active,
            components=tuple(comps),
            created_at=created_at,
            description=description,
            wallpaper_path=wallpaper_path,
        )

    def _component_arg(self, components: Optional[list[str]]) -> Optional[str]:
        """Validate a custom component selection.

        ``None`` means "all components"; an explicitly empty list is an error.
        Treating it as "all" was dangerous: deselecting every checkbox in the
        GUI applied the complete profile instead of nothing.
        """
        if components is None:
            return None
        if not components:
            raise ValueError("Select at least one component")
        allowed = {app.id for app in TRACKED_APPS}
        cleaned: list[str] = []
        for component in components:
            if not isinstance(component, str) or not component:
                raise ValueError("Invalid component id")
            if "," in component or "=" in component or "\n" in component:
                raise ValueError(f"Invalid component id: {component!r}")
            if component not in allowed:
                raise ValueError(f"Unknown component: {component}")
            if component not in cleaned:
                cleaned.append(component)
        if not cleaned:
            raise ValueError("Select at least one component")
        return ",".join(cleaned)

    def apply(self, profile: str, components: Optional[list[str]] = None,
              keep_wallpaper: bool = False, install_missing: bool = False) -> Result:
        """Apply a theme.

        Theme packages (``theme.json``) apply everything they define;
        ``components`` must be None for them. ``data`` then holds the report:
        applied / skipped / installed / missing [{field, name, source}] /
        errors / backup.
        """
        try:
            profile = validate_profile_name(profile)
        except ValueError as exc:
            return Result.err(str(exc))
        if components is None and (profile_dir(profile) / "theme.json").is_file():
            args = ["apply", profile, "--json"]
            if keep_wallpaper:
                args.append("--keep-wallpaper")
            if install_missing:
                args.append("--install-missing")
            result = self._run(args, timeout=300 if install_missing else 60)
            if not result.success:
                return result
            try:
                report = json.loads(result.message or "{}")
            except ValueError:
                return Result.err("theme-profile apply returned invalid JSON")
            return Result.ok(_apply_summary(profile, report), data=report)
        try:
            selected = self._component_arg(components)
        except ValueError as exc:
            return Result.err(str(exc))
        args = ["apply", profile]
        if selected is not None:
            args.append(f"--components={selected}")
        return self._run(args, timeout=60)

    def save(self, name: str, components: Optional[list[str]] = None,
             description: str = "", include_wallpaper: bool = True) -> Result:
        """Save the live theme as a theme package (``theme.json`` + assets).

        ``components`` is accepted for compatibility and ignored: a theme
        stores its theme values, not whole config files.
        """
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        args = ["save", name]
        if description.strip():
            args.append(f"--description={description.strip()}")
        if not include_wallpaper:
            args.append("--no-wallpaper")
        return self._run(args, timeout=120)

    def new_theme(self, name: str, base: str) -> Result:
        """`theme-profile new NAME --from=BASE` (replaces the live look)."""
        try:
            name = validate_profile_name(name)
            if base not in ("neutral-light", "neutral-dark"):
                base = validate_profile_name(base)
        except ValueError as exc:
            return Result.err(str(exc))
        return self._run(["new", name, f"--from={base}"], timeout=300)

    def look_and_feel(self, name: str) -> Result:
        """`theme-profile look-and-feel NAME`: the live look as a Global Theme."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        return self._run(["look-and-feel", name], timeout=60)

    def export_theme(self, name: str, path: str, include_wallpaper: bool = True) -> Result:
        """`theme-profile export NAME --out=PATH [--no-wallpaper]`."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        if not path:
            return Result.err("No export file chosen")
        args = ["export", name, f"--out={path}"]
        if not include_wallpaper:
            args.append("--no-wallpaper")
        return self._run(args, timeout=120)

    def import_theme(self, path: str, name: str = "", force: bool = False) -> Result:
        """`theme-profile import PATH [--name=] [--force] --json`.

        data = {name, files, replaced, missing: [{field, name, source}]}.
        """
        if not path:
            return Result.err("No theme file chosen")
        args = ["import", path, "--json"]
        if name.strip():
            try:
                args.append(f"--name={validate_profile_name(name.strip())}")
            except ValueError as exc:
                return Result.err(str(exc))
        if force:
            args.append("--force")
        result = self._run(args, timeout=120)
        if not result.success:
            return result
        try:
            report = json.loads(result.message or "{}")
        except ValueError:
            return Result.err("theme-profile import returned invalid JSON")
        return Result.ok(f"Imported theme: {report.get('name', '')}", data=report)

    def dependencies(self, name: str) -> Result:
        """`theme-profile deps NAME --json`: what the theme needs that is not installed."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        result = self._run(["deps", name, "--json"], timeout=30)
        if not result.success:
            return result
        try:
            return Result.ok(data=json.loads(result.message or "[]"))
        except ValueError:
            return Result.err("theme-profile deps returned invalid JSON")

    def install_dependencies(self, name: str, indexes: Optional[list[int]] = None) -> Result:
        """`theme-profile deps NAME --install=… --json` (downloads from the
        official sources; packages are never installed — see their command)."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        spec = "all" if indexes is None else ",".join(str(int(i)) for i in indexes)
        if not spec:
            return Result.err("Nothing selected")
        result = self._run(["deps", name, f"--install={spec}", "--json"], timeout=900)
        try:
            items = json.loads(result.message or "[]")
        except ValueError:
            return result if not result.success else Result.err("invalid JSON from theme-profile deps")
        failed = [i for i in items if not i.get("ok")]
        lines = [("✓ " if i.get("ok") else "✗ ") + str(i.get("message", "")) for i in items]
        message = "\n".join(lines) or "Nothing to install"
        return Result(not failed, message, data=items)

    def actions(self, name: str) -> Result:
        """`theme-profile actions NAME --json`: follow-up steps for a theme."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        result = self._run(["actions", name, "--json"], timeout=30)
        try:
            return Result(result.success, "", data=json.loads(result.message or "[]"))
        except ValueError:
            return Result.err(result.message or "invalid JSON from theme-profile actions")

    def run_action(self, action_id: str) -> Result:
        """`theme-profile action ID`: one follow-up step the user agreed to."""
        if not action_id.replace("-", "").isalnum():
            return Result.err(f"invalid action {action_id!r}")
        return self._run(["action", action_id], timeout=300)

    def check(self, name: str) -> Result:
        """What applying ``name`` would miss: data["missing"] = [{field, name, source}]."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        result = self._run(["check", name, "--json"], timeout=30)
        if not result.success:
            return result
        try:
            missing = json.loads(result.message or "[]")
        except ValueError:
            return Result.err("theme-profile check returned invalid JSON")
        return Result.ok(data={"missing": missing})

    def delete(self, name: str) -> Result:
        # Нет в CLI — делаем локально (как в Rust-ядре)
        try:
            directory = profile_dir(name)
        except ValueError as exc:
            return Result.err(str(exc))
        if not directory.exists() and not directory.is_symlink():
            return Result.err(f"Profile not found: {name}")
        try:
            if directory.is_symlink() or not directory.is_dir():
                directory.unlink()
            else:
                shutil.rmtree(directory)
            # Do not leave a stale marker pointing at a deleted profile.
            try:
                if self.active_file.read_text().strip() == name:
                    self.active_file.unlink(missing_ok=True)
            except OSError:
                pass
            return Result.ok(f"Deleted profile: {name}")
        except Exception as e:
            return Result.err(str(e))

    def clone(self, source: str, target: str) -> Result:
        """Дублировать профиль: `theme-profile clone SOURCE TARGET`."""
        try:
            source = validate_profile_name(source)
            target = validate_profile_name(target)
        except ValueError as exc:
            return Result.err(str(exc))
        return self._run(["clone", source, target], timeout=60)

    def set_description(self, name: str, description: str) -> Result:
        """Описание профиля — через `theme-profile describe NAME --set=...`."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        return self._run(["describe", name, f"--set={description}"])

    def diff(self, a: str, b: str) -> str:
        try:
            a = validate_profile_name(a)
            b = validate_profile_name(b)
        except ValueError as exc:
            return f"Error: {exc}"
        res = self._run(["diff", a, b], timeout=30)
        return res.message if res.success else f"Error: {res.message}"

    def get_profile_components(self, profile: str) -> list[str]:
        try:
            validate_profile_name(profile)
        except ValueError:
            return []
        info = self._get_profile_info(profile, False)
        return list(info.components)


def _apply_summary(profile: str, report: dict) -> str:
    lines = [f"Applied profile: {profile}"]
    if report.get("backup"):
        lines.append(f"Pre-apply backup: {report['backup']}")
    for item in report.get("installed") or []:
        lines.append(f"Installed: {item}")
    if report.get("skipped"):
        lines.append("Skipped (app not set up here): " + ", ".join(report["skipped"]))
    for item in report.get("missing") or []:
        source = f" ({item.get('source')})" if item.get("source") else ""
        lines.append(f"Missing: {item.get('name')}{source}")
    for item in report.get("errors") or []:
        lines.append(f"Error: {item}")
    return "\n".join(lines)


# ──────────────────────────────────────────────────────────────
# Wallpaper Core
# ──────────────────────────────────────────────────────────────

# Extensions and ordering *must* match the Rust CLI wallpaper-{picker,next,prev}:
# the cached index is a position in that list, not a file name.
WALLPAPER_EXTENSIONS = (".jpg", ".jpeg", ".png", ".webp", ".bmp")


class CliWallpaperCore(WallpaperCore):
    """Обёртка над Rust CLI wallpaper-* бинарями."""

    def __init__(self):
        self._wallpaper_dir: Optional[Path] = None
        self.cache_index = cache_home() / "wallpaper_index"

    def _resolve_wallpaper_dir(self) -> Path:
        """Wallpaper directory: the explicit GUI setting, else the Rust CLI's
        answer (`theme-profile wallpaper dir`: $WALLPAPER_DIR → setting →
        XDG Pictures/Wallpaper), so GUI, theme-menu and hotkeys agree."""
        if self._wallpaper_dir is None:
            configured = ""
            try:
                configured = CoreFactory.settings_core().load().wallpaper_directory.strip()
            except Exception:
                configured = ""
            if configured:
                self._wallpaper_dir = Path(configured).expanduser()
            else:
                res = self._run([resolve_cli_bin(), "wallpaper", "dir"], timeout=5)
                self._wallpaper_dir = (
                    Path(res.message) if res.success and res.message
                    else Path.home() / "Pictures" / "Wallpaper"
                )
        return self._wallpaper_dir

    @property
    def wallpaper_dir(self) -> Path:
        return self._resolve_wallpaper_dir()

    # Plasma stores the desktop wallpaper per screen in the appletsrc; it is the
    # authoritative answer to "what is on the desktop right now".
    PLASMA_CONFIG = live_path(".config/plasma-org.kde.plasma.desktop-appletsrc")

    def _plasma_wallpaper(self) -> Optional[str]:
        """Путь обоев, которые Plasma показывает сейчас (None — не прочитать).

        One implementation for everyone: the Rust CLI (`theme-profile
        wallpaper current`) picks the primary screen's org.kde.image wallpaper.
        """
        res = self._run(
            [resolve_cli_bin(), "wallpaper", "current"],
            timeout=5,
            env={"PLASMA_APPLETSRC": str(self.PLASMA_CONFIG)},
        )
        return res.message if res.success and res.message else None

    def _run(
        self, args: list[str], timeout: int = 15, env: Optional[dict[str, str]] = None
    ) -> Result:
        environ = None
        if env:
            environ = {**os.environ, **env}
        try:
            proc = subprocess.run(
                args, capture_output=True, text=True, timeout=timeout, env=environ
            )
            return Result(
                proc.returncode == 0,
                proc.stdout.strip() or proc.stderr.strip()
            )
        except subprocess.TimeoutExpired:
            return Result.err(f"Timeout after {timeout}s")
        except Exception as e:
            return Result.err(str(e))

    def _script_env(self) -> dict[str, str]:
        """Force the scripts to use the directory the app is showing.

        The scripts resolve the directory themselves (same rules), but the
        app may hold an explicit setting not yet saved to disk; pinning it
        keeps the index written by a script pointing into the same list.
        """
        return {"WALLPAPER_DIR": str(self.wallpaper_dir)}

    def pick(self) -> Optional[str]:
        """Открыть wofi пикер — ждём завершения, возвращаем выбранный файл."""
        # wallpaper-picker записывает индекс в cache, затем применяет
        res = self._run(["wallpaper-picker"], timeout=60, env=self._script_env())
        if res.success:
            return self.current()
        return None

    def next(self) -> Result:
        return self._run(["wallpaper-next"], env=self._script_env())

    def prev(self) -> Result:
        return self._run(["wallpaper-prev"], env=self._script_env())

    def current(self) -> Optional[str]:
        """Текущие обои: реальное значение Plasma, иначе кэшированный индекс.

        The cached index only resolves correctly while the file list is
        unchanged, and it silently pointed at another wallpaper whenever the
        enumeration order changed. Plasma knows the actual answer.
        """
        real = self._plasma_wallpaper()
        if real:
            return real

        try:
            idx_text = self.cache_index.read_text().strip()
            if not idx_text.isdigit():
                return None
            idx = int(idx_text)
            files = self.list_wallpapers()
            if 0 <= idx < len(files):
                return files[idx]
        except Exception:
            pass
        return None

    def list_wallpapers(self) -> list[str]:
        """Файлы обоев в том же порядке, что и в Rust CLI.

        The cached index is a *position* in this list, so the ordering rules
        have to match the Rust CLI wallpaper-{picker,next,prev} exactly:
        symlinks are followed (Path.is_file(), like find -L), the same
        extensions are accepted, and the sort is by the path string, i.e.
        code-point order — identical to `LC_ALL=C sort` over the UTF-8 bytes,
        but *not* to a locale-aware sort.
        """
        try:
            entries = list(self.wallpaper_dir.iterdir())
        except OSError:
            return []

        files = [
            f for f in entries
            if f.is_file() and f.suffix.lower() in WALLPAPER_EXTENSIONS
        ]
        files.sort(key=str)
        return [str(f) for f in files]

    def set_wallpaper(self, path: str) -> Result:
        return self._run(["plasma-apply-wallpaperimage", path])


# ──────────────────────────────────────────────────────────────
# Settings Core
# ──────────────────────────────────────────────────────────────

SETTINGS_FILE = settings_file()


class CliSettingsCore(SettingsCore):
    """Настройки приложения в JSON."""

    def load(self) -> AppSettings:
        if SETTINGS_FILE.exists():
            try:
                data = json.loads(SETTINGS_FILE.read_text())
                return AppSettings.from_dict(data)
            except Exception:
                pass
        return self.get_defaults()

    def save(self, settings: AppSettings) -> Result:
        try:
            SETTINGS_FILE.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
            # Keep keys this model does not own (the Rust core's "automation"
            # choices): overwrite only the app's own settings.
            data: dict = {}
            if SETTINGS_FILE.exists():
                try:
                    loaded = json.loads(SETTINGS_FILE.read_text())
                    data = loaded if isinstance(loaded, dict) else {}
                except ValueError:
                    data = {}
            data.update(settings.to_dict())
            # Preferences can reveal local paths; never widen them to 0644.
            atomic_write_text(
                SETTINGS_FILE,
                json.dumps(data, indent=2) + "\n",
                mode=0o600,
            )
            return Result.ok("Settings saved")
        except Exception as e:
            return Result.err(str(e))

    def get_defaults(self) -> AppSettings:
        return AppSettings()

    # ── automation choices (owned by the Rust core: `theme-profile automation`)

    def automation(self) -> dict:
        """{configured: bool, items: [{key, what, note, policy}]}."""
        try:
            proc = subprocess.run([resolve_cli_bin(), "automation", "--json"],
                                  capture_output=True, text=True, timeout=15)
            data = json.loads(proc.stdout or "{}")
            return data if isinstance(data, dict) else {}
        except (OSError, subprocess.SubprocessError, ValueError):
            return {}

    def set_automation(self, choices: dict) -> Result:
        """`theme-profile automation set KEY=POLICY …` (also marks it configured)."""
        pairs = [f"{k}={v}" for k, v in choices.items()
                 if str(k).replace("_", "").isalnum() and v in ("ask", "always", "never")]
        if not pairs:
            return Result.err("nothing to set")
        try:
            proc = subprocess.run([resolve_cli_bin(), "automation", "set", *pairs],
                                  capture_output=True, text=True, timeout=15)
        except (OSError, subprocess.SubprocessError) as exc:
            return Result.err(str(exc))
        return Result(proc.returncode == 0, (proc.stderr or proc.stdout).strip())


# ──────────────────────────────────────────────────────────────
# Экспорт
# ──────────────────────────────────────────────────────────────

__all__ = [
    "CliProfileCore",
    "CliWallpaperCore",
    "CliTrackedAppsCore",
    "CliSettingsCore",
    "CLI_FACTORIES",
]

#: Ядро, выбираемое CoreFactory._build() — см. CoreFactory.set_backend()
CLI_FACTORIES = {
    "profile": CliProfileCore,
    "wallpaper": CliWallpaperCore,
    "tracked_apps": CliTrackedAppsCore,
    "settings": CliSettingsCore,
}
