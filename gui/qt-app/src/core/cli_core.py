"""Single transport to the Rust core for profiles, settings and application data.

Python adapts Qt values and report rows; Rust owns parsing and mutations.
"""

import json
import os
import subprocess
import shutil
from pathlib import Path
from typing import Optional

from .paths import (
    active_profile_file,
    cache_home,
    live_path,
    profiles_dir,
    settings_file,
    validate_profile_name,
)
from .protocol import (
    ProfileCore, WallpaperCore, SettingsCore,
    ProfileInfo, Result, AppSettings
)
from .tracked_apps import (
    CliTrackedAppsCore
)


def run_cli(arguments, *, stdin=None, timeout=120, binary=None):
    try:
        proc = subprocess.run([binary or resolve_cli_bin(), *arguments], input=stdin,
                              capture_output=True, text=True, timeout=timeout)
    except (OSError, subprocess.SubprocessError) as exc:
        return Result.err(str(exc))
    if proc.returncode:
        return Result.err(proc.stderr.strip() or proc.stdout.strip() or "kolira failed")
    return Result.ok(proc.stdout.strip())


def _json_gui_value(value):
    # Qt values cross the adapter boundary as ordinary JSON, never Qt types.
    from PySide6.QtCore import QUrl
    from PySide6.QtGui import QColor
    from PySide6.QtQml import QJSValue
    if isinstance(value, QUrl):
        return value.toLocalFile() if value.isLocalFile() else value.toString()
    if isinstance(value, QColor):
        return value.name(QColor.HexRgb)
    if isinstance(value, QJSValue):
        return value.toVariant()
    raise TypeError(f"Unsupported core argument: {type(value).__name__}")


def core_request(method: str, *, _binary=None, _timeout=180, **arguments):
    """One JSON transport to the Rust core; no local business-logic fallback."""
    try:
        proc = subprocess.run([_binary or resolve_cli_bin(), "core", method],
            input=json.dumps(arguments, ensure_ascii=False, default=_json_gui_value), capture_output=True,
            text=True, timeout=_timeout, check=False)
        response = json.loads(proc.stdout)
    except (OSError, subprocess.SubprocessError, ValueError, TypeError) as exc:
        detail = getattr(locals().get("proc"), "stderr", "") or str(exc)
        raise RuntimeError(f"Rust core {method}: {detail.strip()}") from exc
    if not isinstance(response, dict):
        raise RuntimeError("Invalid Rust core response (expected an object)")
    if response.get("api") != 1:
        raise RuntimeError("Incompatible Rust core API; update the CLI and GUI together")
    if not response.get("ok"):
        raise ValueError(response.get("error") or proc.stderr or f"Rust core {method} failed")
    return response.get("data")


# ──────────────────────────────────────────────────────────────
# Profile Core
# ──────────────────────────────────────────────────────────────

def resolve_cli_bin(name: str = "kolira") -> str:
    """Path of a Rust CLI binary: $KTM_THEME_PROFILE_BIN (kolira only;
    tests point it at the freshly built one) → PATH → ~/.local/bin."""
    override = os.environ.get("KTM_THEME_PROFILE_BIN")
    if name == "kolira" and override:
        return override
    resolved = shutil.which(name)
    if resolved:
        return resolved
    fallback = Path.home() / ".local/bin" / name
    return str(fallback) if fallback.exists() else name


def dependency_cli_process(parent, arguments):
    """Unstarted Qt process for the dependency protocol of the single Rust core.

    The controller attaches signals before start(); executable resolution and
    invocation stay in this adapter, alongside the other CLI operations.
    """
    from PySide6.QtCore import QProcess

    process = QProcess(parent)
    process.setProgram(resolve_cli_bin())
    process.setArguments(["dependencies", *arguments])
    return process


def core_cli_process(parent, method, arguments):
    """Create an asynchronous JSON request; execution remains in Rust."""
    from PySide6.QtCore import QProcess
    process = QProcess(parent)
    process.setProgram(resolve_cli_bin())
    process.setArguments(["core", method])
    payload = json.dumps(arguments, default=_json_gui_value).encode()
    def send():
        process.write(payload)
        process.closeWriteChannel()
    process.started.connect(send)
    return process


class CliProfileCore(ProfileCore):
    """Обёртка над Rust CLI `kolira`."""

    def __init__(self, kolira_profile_bin: str = "kolira"):
        # Resolve binary path: check PATH, then fallback to ~/.local/bin/
        self.bin = resolve_cli_bin(kolira_profile_bin)
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
        try:
            return [ProfileInfo(**{**row, "components": tuple(row["components"])})
                    for row in core_request("profile.list", _binary=self.bin)]
        except (ValueError, RuntimeError) as exc:
            raise RuntimeError(f"Could not load profiles: {exc}") from exc

    def get_active_profile(self) -> str:
        return core_request("profile.active", _binary=self.bin)

    def pending_operations(self) -> list[dict]:
        return [record for record in core_request("operation.list", _binary=self.bin)["operations"]
                if record["interrupted"]]

    def clear_stale_active(self, name: str) -> Result:
        try:
            return Result.ok(data={"cleared": core_request("profile.clear_stale", _binary=self.bin, name=name)})
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    def _get_profile_info(self, name: str, is_active: bool) -> ProfileInfo:
        data = core_request("profile.info", _binary=self.bin, name=name)
        return ProfileInfo(**{**data, "is_active": is_active, "components": tuple(data["components"])})

    def preview(self, profile: str, keep_wallpaper: bool = False, *, use_settings: bool = True) -> Result:
        try:
            report = core_request("profile.preview", _binary=self.bin,
                name=profile, keep_wallpaper=keep_wallpaper, use_settings=use_settings)
            from PySide6.QtCore import QCoreApplication
            for action in report.get("actions", []):
                for key in ("title", "reason", "warning"):
                    if action.get(key):
                        action[key] = QCoreApplication.translate("Core", action[key])
            return Result.ok(data=report)
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    def save_before_apply(self, name: str) -> Result:
        try:
            return Result.ok(data=core_request("profile.save_before_apply", _binary=self.bin, name=name))
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    def _component_arg(self, components: Optional[list[str]]) -> Optional[str]:
        """Validate a custom component selection.

        ``None`` means "all components"; an explicitly empty list is an error.
        Treating it as "all" was dangerous: deselecting every checkbox in the
        GUI applied the complete profile instead of nothing.
        """
        return core_request("profile.selection", _binary=self.bin, components=components)

    def apply(self, profile: str, components: Optional[list[str]] = None,
              keep_wallpaper: bool = False, install_missing: bool = False,
              skip_dependencies: bool = False, preview_token: Optional[str] = None, source: str = "gui") -> Result:
        extra = {"preview_token": preview_token} if preview_token is not None else {}
        try:
            report = core_request("profile.apply", _binary=self.bin, _timeout=3600, name=profile, components=components,
                keep_wallpaper=keep_wallpaper, install_missing=install_missing,
                skip_dependencies=skip_dependencies, source=source, **extra)
        except (ValueError, RuntimeError) as exc:
            result = Result.err(str(exc))
            self._bring_shell_back(result)
            return result
        missing = report.get("missing") or []
        message = f"Applied profile: {profile}"
        if missing:
            message += "\nMissing: " + "; ".join(
                str(item.get("name", "")) + (f" ({item['source']})" if item.get("source") else "")
                for item in missing)
        return Result.ok(message, data=report)

    @staticmethod
    def _bring_shell_back(result: Result) -> None:
        if result.success or not ("Timeout" in result.message or "timed out" in result.message):
            return
        try:
            core_request("desktop.recover", _timeout=120)
        except (ValueError, RuntimeError):
            pass

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
        """`kolira new NAME --from=BASE` (replaces the live look)."""
        try:
            name = validate_profile_name(name)
            if base not in ("neutral-light", "neutral-dark"):
                base = validate_profile_name(base)
        except ValueError as exc:
            return Result.err(str(exc))
        return self._run(["new", name, f"--from={base}"], timeout=300)

    def look_and_feel(self, name: str) -> Result:
        """`kolira look-and-feel NAME`: the live look as a Global Theme."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        return self._run(["look-and-feel", name], timeout=60)

    def export_theme(self, name: str, path: str, include_wallpaper: bool = True) -> Result:
        """`kolira export NAME --out=PATH [--no-wallpaper]`."""
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
        """`kolira import PATH [--name=] [--force] --json`.

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
            return Result.err("kolira import returned invalid JSON")
        return Result.ok(f"Imported theme: {report.get('name', '')}", data=report)

    def dependencies(self, name: str) -> Result:
        """`kolira deps NAME --json`: what the theme needs that is not installed."""
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
            return Result.err("kolira deps returned invalid JSON")

    def install_dependencies(self, name: str, indexes: Optional[list[int]] = None, timeout: int = 3600) -> Result:
        """`kolira deps NAME --install=… --json` (downloads from the
        official sources; packages are never installed — see their command)."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        spec = "all" if indexes is None else ",".join(str(int(i)) for i in indexes)
        if not spec:
            return Result.err("Nothing selected")
        result = self._run(["deps", name, f"--install={spec}", "--json"], timeout=timeout)
        try:
            items = json.loads(result.message or "[]")
        except ValueError:
            return result if not result.success else Result.err("invalid JSON from kolira deps")
        failed = [i for i in items if not i.get("ok")]
        lines = [("✓ " if i.get("ok") else "✗ ") + str(i.get("message", "")) for i in items]
        message = "\n".join(lines) or "Nothing to install"
        return Result(not failed, message, data=items)

    def actions(self, name: str) -> Result:
        """`kolira actions NAME --json`: follow-up steps for a theme."""
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        result = self._run(["actions", name, "--json"], timeout=30)
        try:
            return Result(result.success, "", data=json.loads(result.message or "[]"))
        except ValueError:
            return Result.err(result.message or "invalid JSON from kolira actions")

    def run_action(self, action_id: str) -> Result:
        """`kolira action ID`: one follow-up step the user agreed to."""
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
            return Result.err("kolira check returned invalid JSON")
        return Result.ok(data={"missing": missing})

    def delete(self, name: str) -> Result:
        try:
            name = validate_profile_name(name)
        except ValueError as exc:
            return Result.err(str(exc))
        return self._run(["delete", name])

    def clone(self, source: str, target: str) -> Result:
        """Дублировать профиль: `kolira clone SOURCE TARGET`."""
        try:
            source = validate_profile_name(source)
            target = validate_profile_name(target)
        except ValueError as exc:
            return Result.err(str(exc))
        return self._run(["clone", source, target], timeout=60)

    def set_description(self, name: str, description: str) -> Result:
        """Описание профиля — через `kolira describe NAME --set=...`."""
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
        if self._wallpaper_dir is None:
            self._wallpaper_dir = Path(core_request("wallpaper.directory"))
        return self._wallpaper_dir

    @property
    def wallpaper_dir(self) -> Path:
        return self._resolve_wallpaper_dir()

    @property
    def PLASMA_CONFIG(self):
        return live_path(".config/plasma-org.kde.plasma.desktop-appletsrc")

    def _plasma_wallpaper(self) -> Optional[str]:
        return core_request("wallpaper.current", plasma=str(self.PLASMA_CONFIG),
                            directory=str(self.wallpaper_dir), state=str(self.cache_index))

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
        return self._plasma_wallpaper()

    def list_wallpapers(self) -> list[str]:
        return core_request("wallpaper.list", directory=str(self.wallpaper_dir))

    def set_wallpaper(self, path: str) -> Result:
        try:
            result = core_request("wallpaper.set", path=path)
            return Result.ok(result["message"])
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))


# ──────────────────────────────────────────────────────────────
# Settings Core
# ──────────────────────────────────────────────────────────────

SETTINGS_FILE = settings_file()


class CliSettingsCore(SettingsCore):
    """Настройки приложения в JSON."""

    def load(self) -> AppSettings:
        return AppSettings.from_dict(core_request("settings.get"))

    def save(self, settings: AppSettings) -> Result:
        try:
            core_request("settings.set", settings=settings.to_dict())
            return Result.ok("Settings saved")
        except (ValueError, RuntimeError) as exc:
            return Result.err(str(exc))

    def get_defaults(self) -> AppSettings:
        return AppSettings.from_dict(core_request("settings.defaults"))

    # ── automation choices (owned by the Rust core: `kolira automation`)

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
        """`kolira automation set KEY=POLICY …` (also marks it configured)."""
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
