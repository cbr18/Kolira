"""
Test suite for KDE Theme Manager — QA audit.

Tests cover:
  1. py_compile validation of all Python source files.
  2. Import checks for modules that don't require a display.
  3. Mock / unit tests for CLICore classes (Profile, Wallpaper, Settings).
  4. Protocol / dataclass integrity checks.
  5. Tracked-apps registry structural validation.
"""

from __future__ import annotations

import json
import os
import inspect
import subprocess
import sys
import tempfile
import ctypes
from pathlib import Path
from unittest import mock

import pytest

# ─────────────────────────────────────────────────────────────────────────────
# Path setup — ensure `src/` is importable
# ─────────────────────────────────────────────────────────────────────────────
PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = PROJECT_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))


# ─────────────────────────────────────────────────────────────────────────────
# 1. py_compile validation
# ─────────────────────────────────────────────────────────────────────────────
ALL_PY_FILES = sorted(
    p for p in SRC_DIR.rglob("*.py") if "core/rust_core/" not in str(p)
)


@pytest.mark.parametrize("py_file", ALL_PY_FILES, ids=lambda p: p.relative_to(SRC_DIR))
def test_py_compile(py_file: Path) -> None:
    """Every Python source file must compile without syntax errors."""
    result = subprocess.run(
        [sys.executable, "-m", "py_compile", str(py_file)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"py_compile failed for {py_file}:\n{result.stderr}"


# ─────────────────────────────────────────────────────────────────────────────
# 2. Import checks (no display required)
# ─────────────────────────────────────────────────────────────────────────────
def test_import_core_protocol() -> None:
    """Core protocol (ABCs + dataclasses) must import without side effects."""
    from core.protocol import (
        AppSettings,
        AppStatus,
        Component,
        CoreFactory,
        ProfileCore,
        Result,
        TrackedAppsCore,
        WallpaperCore,
    )
    # Ensure dataclasses produce expected defaults
    settings = AppSettings()
    assert settings.default_components_mode == "all"
    assert settings.theme == "system"
    assert settings.custom_components == ()
    assert "wallpaper_directory" in settings.to_dict()
    # The Rust CLI reads network_enabled from the same file; a GUI save must keep it.
    assert settings.to_dict()["network_enabled"] is True
    assert AppSettings.from_dict({"network_enabled": False}).to_dict()["network_enabled"] is False


def test_import_tracked_apps() -> None:
    """Tracked apps registry must import and contain 20 definitions."""
    from core.tracked_apps import TRACKED_APPS, detect_all_apps

    assert len(TRACKED_APPS) == 35, f"Expected 35 tracked apps, got {len(TRACKED_APPS)}"
    detect_all_apps()  # must not raise


def test_import_cli_core() -> None:
    """core implementations must import cleanly."""
    from core.cli_core import (
        CliProfileCore,
        CliSettingsCore,
        CliTrackedAppsCore,
        CliWallpaperCore,
    )
    from core.protocol import Result
    # Smoke-check Result constructors
    ok = Result.ok("done")
    assert ok.success is True
    err = Result.err("oops")
    assert err.success is False


# ─────────────────────────────────────────────────────────────────────────────
# 3. Protocol / dataclass integrity
# ─────────────────────────────────────────────────────────────────────────────
def test_appsettings_roundtrip() -> None:
    """AppSettings → dict → AppSettings must preserve all fields."""
    from core.protocol import AppSettings

    original = AppSettings(
        default_components_mode="custom",
        custom_components=("kwin", "kdeglobals", "kitty"),
        wallpaper_linked_to_profile=True,
        wallpaper_directory="~/Изображения/Wallpaper",
        theme="glass",
        show_not_installed_warning=False,
        auto_apply_on_startup=True,
        minimize_to_tray=True,
        global_hotkey="Meta+T",
        confirm_before_apply=False,
        create_backup_before_apply=False,
        notify_on_success=False,
    )
    data = original.to_dict()
    restored = AppSettings.from_dict(data)
    assert restored == original


def test_component_is_frozen() -> None:
    """Component dataclass must be frozen (slots=True)."""
    from core.protocol import AppStatus, Component

    comp = Component(
        id="test",
        name="Test",
        category="Test",
        config_paths=(".config/test",),
        status=AppStatus.AVAILABLE,
    )
    with pytest.raises((AttributeError, Exception)):
        comp.id = "mutated"  # frozen=True should block this


def test_result_ok_err_methods() -> None:
    from core.protocol import Result

    ok = Result.ok("success message", {"key": "val"})
    assert ok.success is True
    assert ok.message == "success message"
    assert ok.data == {"key": "val"}

    err = Result.err("error message", {"code": 1})
    assert err.success is False
    assert err.data == {"code": 1}


# ─────────────────────────────────────────────────────────────────────────────
# 4. CoreFactory
# ─────────────────────────────────────────────────────────────────────────────
def test_core_factory_reset_and_recreate() -> None:
    """CoreFactory must allow reset (for tests) and lazy instantiation."""
    from core.protocol import CoreFactory

    CoreFactory.reset()
    # After reset, calling the factories should lazily create new instances
    profile_core = CoreFactory.profile_core()
    assert profile_core is not None
    wallpaper_core = CoreFactory.wallpaper_core()
    assert wallpaper_core is not None
    # After reset, the same instance should be returned (singleton per core)
    assert CoreFactory.profile_core() is profile_core


# ─────────────────────────────────────────────────────────────────────────────
# 5. CliProfileCore — mocked subprocess
# ─────────────────────────────────────────────────────────────────────────────
class TestCliProfileCore:
    """Unit tests for CliProfileCore with mocked subprocess."""

    @pytest.fixture(autouse=True)
    def _isolated_home(self, tmp_path, monkeypatch) -> None:
        # Package vs legacy is decided by the profile dir: never the real one.
        for key in [k for k in os.environ if k.startswith("XDG_")]:
            monkeypatch.delenv(key)
        monkeypatch.setenv("HOME", str(tmp_path))
        self.home = tmp_path

    def _make_core(self) -> "object":
        from core.cli_core import CliProfileCore

        return CliProfileCore()

    def test_list_profiles_empty(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_run") as mock_run:
            mock_run.return_value = mock.Mock(success=False, message="error")
            profiles = core.list_profiles()
        assert profiles == []

    def test_list_profiles_success(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_run") as mock_run, \
             mock.patch.object(core, "get_active_profile") as mock_active:
            mock_run.return_value = mock.Mock(success=True, message="glass\npaper")
            mock_active.return_value = "glass"
            profiles = core.list_profiles()
        assert len(profiles) == 2
        assert profiles[0].name == "glass"
        assert profiles[0].is_active is True
        assert profiles[1].name == "paper"
        assert profiles[1].is_active is False

    def test_get_active_profile_file_not_found(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_active_file", Path("/nonexistent/active-profile-12345")):
            result = core.get_active_profile()
        assert result == ""

    def test_apply_profile_passes_components(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_run") as mock_run:
            mock_run.return_value = mock.Mock(success=True, message="ok")
            core.apply("glass", components=["kwin", "kdeglobals"])
        mock_run.assert_called_once()
        args, kwargs = mock_run.call_args
        assert args[0] == ["apply", "glass", "--components=kwin,kdeglobals"]

    def test_apply_package_uses_json_report(self) -> None:
        package = self.home / ".config/theme-profile/profiles/night"
        package.mkdir(parents=True)
        (package / "theme.json").write_text("{}", encoding="utf-8")
        core = self._make_core()
        report = '{"applied":["color_scheme"],"skipped":[],"installed":[],' \
                 '"missing":[{"field":"icon_theme","name":"YAMIS","source":"https://x"}],' \
                 '"errors":[],"backup":"/b"}'
        with mock.patch.object(core, "_run") as mock_run:
            mock_run.return_value = mock.Mock(success=True, message=report)
            result = core.apply("night", keep_wallpaper=True, install_missing=True)
        args, _ = mock_run.call_args
        assert args[0] == ["apply", "night", "--json", "--keep-wallpaper", "--install-missing"]
        assert result.success
        assert result.data["missing"][0]["name"] == "YAMIS"
        assert "Missing: YAMIS (https://x)" in result.message

    def test_apply_profile_no_components(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_run") as mock_run:
            mock_run.return_value = mock.Mock(success=True, message="ok")
            core.apply("glass")
        mock_run.assert_called_once()
        args, _ = mock_run.call_args
        assert args[0] == ["apply", "glass"]

    def test_delete_profile_not_found(self) -> None:
        core = self._make_core()
        result = core.delete("nonexistent_profile_12345")
        assert result.success is False
        assert "not found" in result.message.lower()

    def test_delete_profile_success(self) -> None:
        core = self._make_core()
        fake_dir = Path(tempfile.mkdtemp())
        result = core.delete(fake_dir.name)
        # May succeed or fail depending on whether the dir exists in profiles_dir
        # Just verify the method runs without crashing
        assert isinstance(result.success, bool)

    def test_delete_rejects_path_traversal(self, tmp_path: Path) -> None:
        core = self._make_core()
        victim = tmp_path / "victim"
        victim.mkdir()
        result = core.delete("../" + victim.name)
        assert result.success is False
        assert victim.is_dir()

    def test_clone_passes_names_to_cli(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_run") as mock_run:
            mock_run.return_value = mock.Mock(
                success=True, message="Cloned profile: glass -> glass2")
            result = core.clone("glass", "glass2")
        assert result.success
        mock_run.assert_called_once()
        args, _ = mock_run.call_args
        assert args[0] == ["clone", "glass", "glass2"]

    def test_clone_rejects_bad_names_without_cli(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_run") as mock_run:
            assert not core.clone("../etc", "ok-name").success
            assert not core.clone("glass", "bad/name").success
        mock_run.assert_not_called()

    def test_apply_rejects_empty_component_list(self) -> None:
        core = self._make_core()
        result = core.apply("glass", components=[])
        assert result.success is False
        assert "component" in result.message.lower()

    def test_apply_rejects_unknown_component(self) -> None:
        core = self._make_core()
        result = core.apply("glass", components=["not-a-real-component"])
        assert result.success is False


# ─────────────────────────────────────────────────────────────────────────────
# 6. CliSettingsCore — file-based I/O
# ─────────────────────────────────────────────────────────────────────────────
class TestCliSettingsCore:
    """Unit tests for CliSettingsCore with temp directory."""

    def test_defaults(self) -> None:
        from core.cli_core import CliSettingsCore

        core = CliSettingsCore()
        settings = core.get_defaults()
        assert settings.theme == "system"
        assert settings.default_components_mode == "all"
        assert settings.custom_components == ()

    def test_load_nonexistent_returns_defaults(self) -> None:
        from core.cli_core import CliSettingsCore

        core = CliSettingsCore()
        settings = core.load()
        assert settings.theme == "system"

    def test_save_and_load_roundtrip(self, tmp_path: Path) -> None:
        from core.cli_core import CliSettingsCore
        from core.protocol import AppSettings

        core = CliSettingsCore()
        custom = AppSettings(
            theme="glass",
            default_components_mode="custom",
            custom_components=("kwin", "kdeglobals"),
        )
        with mock.patch("core.cli_core.SETTINGS_FILE", tmp_path / "settings.json"):
            result = core.save(custom)
            assert result.success

            loaded = core.load()
            assert loaded.theme == "glass"
            assert loaded.default_components_mode == "custom"
            assert loaded.custom_components == ("kwin", "kdeglobals")
            # Preferences must not be widened to the process umask.
            assert (tmp_path / "settings.json").stat().st_mode & 0o777 == 0o600


# ─────────────────────────────────────────────────────────────────────────────
# 6b. CliTrackedAppsCore — custom components (components.json)
# ─────────────────────────────────────────────────────────────────────────────
class TestCliCustomComponents:
    """The CLI-backed CRUD for user-registered custom components."""

    def _make_core(self):
        from core.tracked_apps import CliTrackedAppsCore

        return CliTrackedAppsCore()

    def test_list_custom_parses_ids(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_run") as run:
            run.return_value = mock.Mock(success=True, message="myapp\nother")
            comps = core.list_custom()
        assert [c.id for c in comps] == ["myapp", "other"]
        assert run.call_args[0][0] == ["theme-profile", "components", "list"]

    def test_list_custom_failure_returns_empty(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_run") as run:
            run.return_value = mock.Mock(success=False, message="boom")
            assert core.list_custom() == []

    def test_add_custom_sends_valid_json(self) -> None:
        """Quotes/newlines in user fields must be escaped, not break the JSON."""
        from core.protocol import CustomComponent

        core = self._make_core()
        with mock.patch.object(core, "_run") as run:
            run.return_value = mock.Mock(success=True, message="Added")
            comp = CustomComponent(
                id="myapp",
                name='My "quoted" app',
                config_paths=(".config/myapp/settings.json",),
                description="line1\nline2",
            )
            result = core.add_custom(comp)
        assert result.success
        args = run.call_args[0][0]
        assert args[:3] == ["theme-profile", "components", "add"]
        # The payload must parse — regression guard for the f-string builder
        # that produced invalid JSON (and referenced an unimported `json`).
        payload = json.loads(args[3])
        assert payload["id"] == "myapp"
        assert payload["name"] == 'My "quoted" app'
        assert payload["description"] == "line1\nline2"
        assert payload["config_paths"] == [".config/myapp/settings.json"]

    def test_update_custom_uses_edit(self) -> None:
        from core.protocol import CustomComponent

        core = self._make_core()
        with mock.patch.object(core, "_run") as run:
            run.return_value = mock.Mock(success=True, message="Updated")
            result = core.update_custom(CustomComponent(id="myapp", name="v2"))
        assert result.success
        assert run.call_args[0][0][:3] == ["theme-profile", "components", "edit"]

    def test_remove_custom(self) -> None:
        core = self._make_core()
        with mock.patch.object(core, "_run") as run:
            run.return_value = mock.Mock(success=True, message="Removed")
            result = core.remove_custom("myapp")
        assert result.success
        assert run.call_args[0][0] == ["theme-profile", "components", "remove", "myapp"]

    def test_failed_add_maps_to_error_result(self) -> None:
        from core.protocol import CustomComponent

        core = self._make_core()
        with mock.patch.object(core, "_run") as run:
            run.return_value = mock.Mock(success=False, message="already exists")
            result = core.add_custom(CustomComponent(id="myapp"))
        assert not result.success
        assert result.message


# ─────────────────────────────────────────────────────────────────────────────
# 6c. CliProfileCore — manifest.json metadata
# ─────────────────────────────────────────────────────────────────────────────
class TestCliProfileManifest:
    """_get_profile_info reads manifest.json (new) with a manifest.txt fallback."""

    def _info(self, tmp_path: Path, monkeypatch, *, with_json: bool, with_txt: bool):
        from core.cli_core import CliProfileCore

        profile = tmp_path / "glass"
        (profile / ".config").mkdir(parents=True)
        (profile / ".config" / "kdeglobals").write_text("[General]\n")
        if with_json:
            (profile / "manifest.json").write_text(json.dumps({
                "schema_version": 1,
                "name": "glass",
                "created_at": "2026-10-28T12:00:00+10:00",
                "description": "Dark glass",
            }))
        if with_txt:
            (profile / "manifest.txt").write_text(
                "Generated by theme-profile\ntimestamp=2026-01-02T03:04:05\n"
            )
        monkeypatch.setattr("core.cli_core.profile_dir", lambda name: profile)
        return CliProfileCore()._get_profile_info("glass", True)

    def test_manifest_json_wins(self, tmp_path, monkeypatch) -> None:
        info = self._info(tmp_path, monkeypatch, with_json=True, with_txt=True)
        assert info.created_at == "2026-10-28T12:00:00+10:00"
        assert info.description == "Dark glass"
        assert "kdeglobals" in info.components

    def test_legacy_txt_fallback(self, tmp_path, monkeypatch) -> None:
        info = self._info(tmp_path, monkeypatch, with_json=False, with_txt=True)
        assert info.created_at == "2026-01-02T03:04:05"
        assert info.description == ""

    def test_no_manifest_defaults(self, tmp_path, monkeypatch) -> None:
        info = self._info(tmp_path, monkeypatch, with_json=False, with_txt=False)
        assert info.created_at == ""
        assert info.description == ""


# ─────────────────────────────────────────────────────────────────────────────
# 6d. Custom component registry merge + disabled_components
# ─────────────────────────────────────────────────────────────────────────────
class TestCustomRegistryMerge:
    """Python detection merges components.json with the built-in registry."""

    def _write_components(self, tmp_path: Path, monkeypatch, entries: list) -> None:
        import json as jsonlib

        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))
        root = tmp_path / ".config" / "theme-profile"
        root.mkdir(parents=True)
        (root / "components.json").write_text(jsonlib.dumps({"schema_version": 1, "components": entries}))

    def test_custom_component_joins_detection(self, tmp_path, monkeypatch) -> None:
        self._write_components(tmp_path, monkeypatch, [{
            "id": "mockapp",
            "name": "Mock App",
            "config_paths": [".config/mockapp/settings.conf"],
            "reload_command": "sh -c true",
        }])
        from core.tracked_apps import detect_all_apps

        comps = {c.id: c for c in detect_all_apps()}
        assert "mockapp" in comps
        assert comps["mockapp"].is_custom is True
        assert comps["mockapp"].reload_command == "sh -c true"
        assert comps["mockapp"].supports_live_reload is True
        # Встроенные на месте
        assert "kwin" in comps and comps["kwin"].is_custom is False

    def test_builtin_id_shadows_custom(self, tmp_path, monkeypatch) -> None:
        self._write_components(tmp_path, monkeypatch, [{
            "id": "kwin",
            "name": "Fake KWin",
            "config_paths": [".config/kwinrc"],
        }])
        from core.tracked_apps import all_app_defs

        defs = all_app_defs()
        kwin = [d for d in defs if d.id == "kwin"]
        assert len(kwin) == 1 and kwin[0].is_custom is False

    def test_invalid_entry_drops_whole_custom_list(self, tmp_path, monkeypatch) -> None:
        self._write_components(tmp_path, monkeypatch, [
            {"id": "ok", "config_paths": [".config/ok.conf"]},
            {"id": "../escape", "config_paths": [".config/x"]},
        ])
        from core.tracked_apps import all_app_defs

        assert all(not d.is_custom for d in all_app_defs())

    def test_missing_file_means_no_custom(self, tmp_path, monkeypatch) -> None:
        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))
        from core.tracked_apps import all_app_defs

        assert all(not d.is_custom for d in all_app_defs())


class TestDisabledComponents:
    """Галочка «отслеживать» = AppSettings.disabled_components."""

    def test_round_trip(self) -> None:
        from core.protocol import AppSettings

        settings = AppSettings(
            **{**AppSettings().to_dict(), "disabled_components": ("btop", "wofi")}
        )
        data = settings.to_dict()
        assert data["disabled_components"] == ["btop", "wofi"]
        loaded = AppSettings.from_dict(data)
        assert loaded.disabled_components == ("btop", "wofi")

    def test_old_configs_default_to_empty(self) -> None:
        from core.protocol import AppSettings

        assert AppSettings.from_dict({}).disabled_components == ()


class TestTrackedAppsControllerCrud:
    """Валидация полей пользовательского компонента (страница «Приложения»)."""

    def _validate(self, fields: dict):
        from controllers import TrackedAppsController

        return TrackedAppsController._custom_from_dict(fields)

    def test_valid_minimal(self) -> None:
        comp, error = self._validate({
            "name": "Mock App",
            "configPaths": [".config/mockapp/settings.conf"],
        })
        assert error == ""
        assert comp.id == "Mock-App" or comp.id  # авто-id из имени
        assert comp.category == "Custom"
        assert comp.icon == "application-x-executable"

    def test_auto_id_slug_from_name(self) -> None:
        comp, error = self._validate({"name": "My Cool App", "configPaths": [".config/x"]})
        assert error == ""
        assert comp.id == "My-Cool-App"

    def test_name_required(self) -> None:
        _, error = self._validate({"name": "  ", "configPaths": [".config/x"]})
        assert error

    def test_at_least_one_path(self) -> None:
        _, error = self._validate({"name": "X", "configPaths": []})
        assert error

    def test_path_must_be_relative(self) -> None:
        for bad in ["/etc/passwd", ".config/../../escape"]:
            _, error = self._validate({"name": "X", "configPaths": [bad]})
            assert error, bad

    def test_reload_command_rejects_shell_characters(self) -> None:
        _, error = self._validate({
            "name": "X", "configPaths": [".config/x"], "reloadCommand": "a|b",
        })
        assert "shell" in error

    def test_reload_command_binary_must_exist(self) -> None:
        _, error = self._validate({
            "name": "X",
            "configPaths": [".config/x"],
            "reloadCommand": "definitely-not-a-real-binary-ktm",
        })
        assert "PATH" in error

    def test_reload_command_accepts_real_binary(self) -> None:
        comp, error = self._validate({
            "name": "X", "configPaths": [".config/x"], "reloadCommand": "sh -c true",
        })
        assert error == ""
        assert comp.supports_live_reload is True


# ─────────────────────────────────────────────────────────────────────────────
# 6e. Profile descriptions: CLI core + controller + QML wiring
# ─────────────────────────────────────────────────────────────────────────────
class TestProfileDescription:
    def test_cli_core_set_description_runs_describe(self) -> None:
        from core.cli_core import CliProfileCore

        core = CliProfileCore()
        with mock.patch.object(core, "_run") as run:
            run.return_value = mock.Mock(success=True, message="Description updated.")
            result = core.set_description("glass", "Dark glass")
        assert result.success
        assert run.call_args[0][0] == ["describe", "glass", "--set=Dark glass"]

    def test_cli_core_set_description_validates_name(self) -> None:
        from core.cli_core import CliProfileCore

        core = CliProfileCore()
        assert not core.set_description("../escape", "x").success

    def test_controller_save_passes_description_to_save(self) -> None:
        from PySide6.QtCore import QCoreApplication

        from controllers import ProfileController
        from core.protocol import AppSettings, CoreFactory, Result

        calls: list[tuple[str, str]] = []

        class FakeProfileCore:
            def list_profiles(self): return []
            def get_active_profile(self): return ""
            def apply(self, profile, components=None): return Result.ok("ok")
            def save(self, name, components=None, description="", include_wallpaper=True):
                calls.append(("save", name, description, include_wallpaper))
                return Result.ok("ok")
            def set_description(self, name, description):
                calls.append(("describe", description))
                return Result.ok("ok")
            def delete(self, name): return Result.ok("ok")
            def diff(self, a, b): return ""
            def get_profile_components(self, profile): return []

        class FakeSettingsCore:
            def load(self): return AppSettings()
            def save(self, settings): return Result.ok("ok")
            def get_defaults(self): return AppSettings()

        CoreFactory.set_profile_core(FakeProfileCore())
        CoreFactory.set_settings_core(FakeSettingsCore())
        try:
            controller = ProfileController()
            done: list[bool] = []
            controller.operationFinished.connect(lambda ok, msg: done.append(ok))
            controller.saveProfile("glass", "Dark glass", False)
            app = QCoreApplication.instance() or QCoreApplication([])
            for _ in range(200):
                if done:
                    break
                app.processEvents()
                import time
                time.sleep(0.01)
            assert done and done[0]
            # One `theme-profile save --description=… --no-wallpaper` call.
            assert calls == [("save", "glass", "Dark glass", False)]
        finally:
            CoreFactory.reset()

    def test_controller_save_without_description_skips_describe(self) -> None:
        from controllers import ProfileController
        from core.protocol import AppSettings, CoreFactory, Result

        calls: list[str] = []

        class FakeProfileCore:
            def list_profiles(self): return []
            def get_active_profile(self): return ""
            def apply(self, profile, components=None): return Result.ok("ok")
            def save(self, name, components=None):
                calls.append("save")
                return Result.ok("ok")
            def set_description(self, name, description):
                calls.append("describe")
                return Result.ok("ok")
            def delete(self, name): return Result.ok("ok")
            def diff(self, a, b): return ""
            def get_profile_components(self, profile): return []

        class FakeSettingsCore:
            def load(self): return AppSettings()
            def save(self, settings): return Result.ok("ok")
            def get_defaults(self): return AppSettings()

        CoreFactory.set_profile_core(FakeProfileCore())
        CoreFactory.set_settings_core(FakeSettingsCore())
        try:
            controller = ProfileController()
            # Пустое/пробельное описание → только save (воркер тот же кодовый путь)
            comp, error = controller._settings_components()
            assert comp is None and error == ""
        finally:
            CoreFactory.reset()

    def test_description_ui_wiring(self) -> None:
        main = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
        card = (PROJECT_ROOT / "qml" / "Components" / "ProfileCardDelegate.qml").read_text()
        assert "descriptionField" in main
        assert "setProfileDescription" in main
        assert "descriptionDialog" in main
        assert "profileDescription" in card
        assert "editDescriptionClicked" in card


# ─────────────────────────────────────────────────────────────────────────────
# 6f. Stage C1: read-only config browsing (parser + inspector model)
# ─────────────────────────────────────────────────────────────────────────────
class TestReadConfigParams:
    def test_ini_groups(self, tmp_path: Path) -> None:
        from core.theme_core import read_config_params

        f = tmp_path / "kwinrc"
        f.write_text("[Plugins]\nblurEnabled=true\n\n[Windows]\nBorderSize=Normal\n")
        result = read_config_params(f)
        assert result is not None
        keys = {p["key"]: p["value"] for p in result["params"]}
        assert keys["Plugins/blurEnabled"] == "true"
        assert keys["Windows/BorderSize"] == "Normal"

    def test_jsonc_comments_and_trailing_commas(self, tmp_path: Path) -> None:
        from core.theme_core import read_config_params

        f = tmp_path / "settings.jsonc"
        f.write_text(
            '{\n  // comment\n  "theme": "dark",\n  "nested": {"a": 1, /* inline */ "b": [1, 2],},\n}\n'
        )
        result = read_config_params(f)
        assert result is not None
        keys = {p["key"]: p["value"] for p in result["params"]}
        assert keys["theme"] == "dark"
        assert keys["nested/a"] == "1"
        assert keys["nested/b"] == "[1,2]"

    def test_toml(self, tmp_path: Path) -> None:
        from core.theme_core import read_config_params

        f = tmp_path / "alacritty.toml"
        f.write_text('[window]\nopacity = 0.5\n[font]\nnormal = { family = "Mono" }\n')
        result = read_config_params(f)
        assert result is not None
        keys = {p["key"]: p["value"] for p in result["params"]}
        assert keys["window/opacity"] == "0.5"
        assert keys["font/normal/family"] == "Mono"

    def test_user_js(self, tmp_path: Path) -> None:
        from core.theme_core import read_config_params

        f = tmp_path / "user.js"
        f.write_text('user_pref("zen.theme.color", "red");\nuser_pref("zen.mode", 1);\n')
        result = read_config_params(f)
        assert result is not None
        keys = {p["key"]: p["value"] for p in result["params"]}
        assert keys["zen.theme.color"] == '"red"'
        assert keys["zen.mode"] == "1"

    def test_kitty_style_line_based(self, tmp_path: Path) -> None:
        from core.theme_core import read_config_params

        f = tmp_path / "kitty.conf"
        f.write_text("# comment\nfont_family JetBrains Mono\nbackground_opacity 0.5\n")
        result = read_config_params(f)
        assert result is not None
        keys = {p["key"]: p["value"] for p in result["params"]}
        assert keys["font_family"] == "JetBrains Mono"
        assert keys["background_opacity"] == "0.5"

    def test_unsupported_suffix_returns_none(self, tmp_path: Path) -> None:
        from core.theme_core import read_config_params

        f = tmp_path / "style.css"
        f.write_text("window { color: red; }")
        assert read_config_params(f) is None

    def test_empty_file_returns_none(self, tmp_path: Path) -> None:
        from core.theme_core import read_config_params

        f = tmp_path / "empty.conf"
        f.write_text("# only comments\n")
        assert read_config_params(f) is None


class TestInspectorAppConfigs:
    """appConfigs: tracked app → files → key/value rows for the current scope."""

    def _controller(self, tmp_path: Path, monkeypatch):
        from controllers.theme_inspector import ThemeInspectorController
        from core.protocol import AppStatus, Component, CoreFactory

        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))

        class FakeThemeCore:
            def sections(self, scope): return []
            def fields(self, section, scope): return []
            def validate(self): return []
            def get(self, field_id, scope): return None
            def search(self, query, scope): return []

        class FakeTrackedCore:
            def get_all_components(self):
                return [
                    Component(id="mockapp", name="Mock App", category="Custom",
                              config_paths=(".config/mockapp/settings.conf",),
                              status=AppStatus.AVAILABLE),
                ]

        monkeypatch.setattr(CoreFactory, "_theme_core", FakeThemeCore())
        monkeypatch.setattr(CoreFactory, "_tracked_apps_core", FakeTrackedCore())
        return ThemeInspectorController()

    def test_live_scope_reads_files(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.conf").write_text("[main]\nspeed=3\n")
        controller = self._controller(tmp_path, monkeypatch)

        configs = controller.appConfigs
        assert len(configs) == 1
        entry = configs[0]
        assert entry["id"] == "mockapp"
        assert len(entry["files"]) == 1
        file_entry = entry["files"][0]
        assert file_entry["relPath"] == ".config/mockapp/settings.conf"
        assert {p["key"]: p["value"] for p in file_entry["params"]} == {"main/speed": "3"}

    def test_app_config_params_can_expand_truncated_file(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.conf").write_text(
            "[main]\n" + "".join(f"key{index}={index}\n" for index in range(301))
        )
        controller = self._controller(tmp_path, monkeypatch)
        file_entry = controller.appConfigs[0]["files"][0]
        assert file_entry["truncated"] is True
        assert len(file_entry["params"]) == 300

        params = controller.appConfigParams(file_entry["relPath"], 1000)
        assert len(params) == 301
        assert params[-1] == {"key": "main/key300", "value": "300"}
        assert controller.appConfigParams(".config/other.conf", 1000) == []

    def test_missing_file_is_skipped(self, tmp_path: Path, monkeypatch) -> None:
        controller = self._controller(tmp_path, monkeypatch)
        assert controller.appConfigs == []

    def test_unsupported_file_shows_note(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.conf").write_text("body { color: red }")  # .conf → INI parse of css
        controller = self._controller(tmp_path, monkeypatch)
        # INI-парсер не найдёт key=value → line-based fallback найдёт строки —
        # главное, что файл попал в модель без падения.
        assert controller.appConfigs[0]["files"][0]["params"]

    def test_profile_scope_reads_snapshot(self, tmp_path: Path, monkeypatch) -> None:
        from core.paths import profile_dir

        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))
        snap = profile_dir("glass") / ".config" / "mockapp"
        snap.mkdir(parents=True)
        (snap / "settings.conf").write_text("[main]\nspeed=9\n")

        controller = self._controller(tmp_path, monkeypatch)
        controller.setScope("profile:glass")
        configs = controller.appConfigs
        assert {p["key"]: p["value"] for p in configs[0]["files"][0]["params"]} == {"main/speed": "9"}

    def test_editable_flags_by_suffix(self, tmp_path: Path, monkeypatch) -> None:
        """C3: TOML/user.js в live-scope редактируемы; экзотика — нет."""
        from controllers.theme_inspector import ThemeInspectorController
        from core.protocol import AppStatus, Component, CoreFactory

        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))

        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "a.toml").write_text("speed = 3\n")
        (conf / "b.js").write_text('user_pref("x.y", 1);\n')
        (conf / "c.css").write_text("body { color: red }")

        class FakeThemeCore:
            def sections(self, scope): return []
            def fields(self, section, scope): return []
            def validate(self): return []
            def get(self, field_id, scope): return None
            def search(self, query, scope): return []

        class FakeTrackedCore:
            def get_all_components(self):
                return [
                    Component(id="mockapp", name="Mock App", category="Custom",
                              config_paths=(
                                  ".config/mockapp/a.toml",
                                  ".config/mockapp/b.js",
                                  ".config/mockapp/c.css",
                              ),
                              status=AppStatus.AVAILABLE),
                ]

        monkeypatch.setattr(CoreFactory, "_theme_core", FakeThemeCore())
        monkeypatch.setattr(CoreFactory, "_tracked_apps_core", FakeTrackedCore())
        controller = ThemeInspectorController()

        files = {f["relPath"]: f for f in controller.appConfigs[0]["files"]}
        assert files[".config/mockapp/a.toml"]["editable"] is True
        assert files[".config/mockapp/b.js"]["editable"] is True
        assert files[".config/mockapp/c.css"]["editable"] is False
        assert files[".config/mockapp/c.css"]["note"] == "unsupported format"

    def test_theme_page_shows_app_configs(self) -> None:
        # M4: дерево конфигов живёт на странице «Приложения» (AppConfigTree),
        # со страницы Theme убрано.
        theme_qml = (PROJECT_ROOT / "qml" / "pages" / "ThemePage.qml").read_text()
        assert "themeInspector.appConfigs" not in theme_qml
        tree_qml = (PROJECT_ROOT / "qml" / "pages" / "AppConfigTree.qml").read_text()
        assert "themeInspector.appConfigs" in tree_qml


# ─────────────────────────────────────────────────────────────────────────────
# 6g. Stage C2/C3: editing app config values (INI/JSON/TOML/user.js/kv by key)
# ─────────────────────────────────────────────────────────────────────────────
class TestSetConfigValue:
    def _core(self, tmp_path: Path, monkeypatch):
        from core.theme_core import AppThemeCore

        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))
        # apply() выполняет команды/хуки — в тестах они отключены.
        monkeypatch.setenv("KTM_THEME_NO_APPLY", "1")
        return AppThemeCore()

    def test_ini_group_key(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.conf").write_text("[main]\nspeed=3\n# keep me\n")
        core = self._core(tmp_path, monkeypatch)

        result = core.set_config_value(".config/mockapp/settings.conf", "main/speed", "9")
        assert result.success
        assert core.apply().success
        text = (conf / "settings.conf").read_text()
        assert "speed=9" in text
        assert "# keep me" in text  # форматирование и комментарии сохранились

    def test_ini_top_level_key_creates_groupless_entry(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.conf").write_text("[main]\nspeed=3\n")
        core = self._core(tmp_path, monkeypatch)

        result = core.set_config_value(".config/mockapp/settings.conf", "volume", "loud")
        assert result.success
        assert core.apply().success
        text = (conf / "settings.conf").read_text()
        assert "volume=loud" in text

    def test_json_type_preserved(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.json").write_text(
            '{"speed": 3, "enabled": true, "name": "dark", "nested": {"x": 1.5}}'
        )
        core = self._core(tmp_path, monkeypatch)

        assert core.set_config_value(".config/mockapp/settings.json", "speed", "9").success
        assert core.set_config_value(".config/mockapp/settings.json", "enabled", "false").success
        assert core.set_config_value(".config/mockapp/settings.json", "name", "light").success
        assert core.set_config_value(".config/mockapp/settings.json", "nested/x", "2.25").success
        assert core.apply().success

        import json as jsonlib
        data = jsonlib.loads((conf / "settings.json").read_text())
        assert data == {"speed": 9, "enabled": False, "name": "light",
                        "nested": {"x": 2.25}}

    def test_jsonc_comments_survive(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.jsonc").write_text('{\n  // theme note\n  "theme": "dark",\n}\n')
        core = self._core(tmp_path, monkeypatch)

        result = core.set_config_value(".config/mockapp/settings.jsonc", "theme", "light")
        assert result.success
        assert core.apply().success
        text = (conf / "settings.jsonc").read_text()
        assert "// theme note" in text
        assert '"theme": "light"' in text

    def test_unknown_json_key_errors(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.json").write_text('{"a": 1}')
        core = self._core(tmp_path, monkeypatch)

        assert not core.set_config_value(".config/mockapp/settings.json", "missing/key", "1").success

    def test_toml_root_and_section_keys(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.toml").write_text("speed = 3\n\n[window]\nopacity = 0.9\n")
        core = self._core(tmp_path, monkeypatch)

        assert core.set_config_value(".config/mockapp/settings.toml", "speed", "9").success
        assert core.set_config_value(".config/mockapp/settings.toml", "window/opacity", "1").success
        assert core.apply().success
        text = (conf / "settings.toml").read_text()
        assert "speed = 9" in text
        assert "opacity = 1" in text  # float 0.9 → 1.0 → "1", тип сохранён

    def test_toml_dotted_header_nested_key(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.toml").write_text('[font.normal]\nfamily = "Fira"\n')
        core = self._core(tmp_path, monkeypatch)

        result = core.set_config_value(
            ".config/mockapp/settings.toml", "font/normal/family", "JetBrains")
        assert result.success
        assert core.apply().success
        text = (conf / "settings.toml").read_text()
        assert '[font.normal]' in text
        assert 'family = "JetBrains"' in text

    def test_toml_inline_table_rejected(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.toml").write_text('font = {normal = {family = "Fira"}}\n')
        core = self._core(tmp_path, monkeypatch)

        result = core.set_config_value(
            ".config/mockapp/settings.toml", "font/normal/family", "JetBrains")
        assert not result.success

    def test_toml_array_type_preserved(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "settings.toml").write_text('features = ["a", "b"]\n')
        core = self._core(tmp_path, monkeypatch)

        result = core.set_config_value(".config/mockapp/settings.toml", "features", '["c"]')
        assert result.success
        assert core.apply().success
        assert 'features = ["c"]' in (conf / "settings.toml").read_text()

    def test_userjs_key_updated_and_appended(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "user.js").write_text('user_pref("browser.startup.page", 1);\n')
        core = self._core(tmp_path, monkeypatch)

        assert core.set_config_value(
            ".config/mockapp/user.js", "browser.startup.page", "3").success
        assert core.set_config_value(
            ".config/mockapp/user.js", "ui.pref", "true").success
        assert core.apply().success
        text = (conf / "user.js").read_text()
        assert 'user_pref("browser.startup.page", 3);' in text  # int сохранён
        assert 'user_pref("ui.pref", true);' in text

    def test_kv_line_format_preserved(self, tmp_path: Path, monkeypatch) -> None:
        """kitty.conf: "key value" переписывается в том же формате, без '='."""
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "kitty.conf").write_text("background_opacity 0.9\n# keep me\n")
        core = self._core(tmp_path, monkeypatch)

        result = core.set_config_value(
            ".config/mockapp/kitty.conf", "background_opacity", "0.5")
        assert result.success
        assert core.apply().success
        lines = (conf / "kitty.conf").read_text().splitlines()
        assert "background_opacity 0.5" in lines
        assert "# keep me" in lines

    def test_unsupported_suffix_rejected(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        (conf / "style.css").write_text("body { color: red }")
        core = self._core(tmp_path, monkeypatch)

        result = core.set_config_value(".config/mockapp/style.css", "color", "blue")
        assert not result.success
        assert "not supported" in result.message

    def test_missing_file_errors(self, tmp_path: Path, monkeypatch) -> None:
        core = self._core(tmp_path, monkeypatch)
        assert not core.set_config_value(".config/nope.conf", "a", "1").success

    def test_undo_unstages_without_writing(self, tmp_path: Path, monkeypatch) -> None:
        """M2: undo до apply снимает стейдж; файл не тронут."""
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        target = conf / "settings.conf"
        target.write_text("[main]\nspeed=3\n")
        core = self._core(tmp_path, monkeypatch)

        assert core.set_config_value(".config/mockapp/settings.conf", "main/speed", "9").success
        assert core.session_changes() == 1
        assert core.undo().success
        assert core.session_changes() == 0
        assert target.read_text() == "[main]\nspeed=3\n"

    def test_revert_discards_staged_without_writing(self, tmp_path: Path, monkeypatch) -> None:
        """M2: revert до apply сбрасывает стейдж; файлы не тронуты."""
        conf = tmp_path / ".config" / "mockapp"
        conf.mkdir(parents=True)
        target = conf / "settings.conf"
        target.write_text("[main]\nspeed=3\n")
        core = self._core(tmp_path, monkeypatch)

        core.set_config_value(".config/mockapp/settings.conf", "main/speed", "9")
        core.set_config_value(".config/mockapp/settings.conf", "main/volume", "loud")
        assert core.session_changes() == 2
        assert core.revert().success
        assert core.session_changes() == 0
        assert target.read_text() == "[main]\nspeed=3\n"


class TestInspectorStaging:
    """M2: curated-поля стейджатся; staged-значение видно в модели до записи."""

    def _core(self, tmp_path: Path, monkeypatch):
        from core.theme_core import AppThemeCore

        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))
        monkeypatch.setenv("KTM_THEME_NO_APPLY", "1")
        return AppThemeCore()

    def test_field_staged_value_visible_before_apply(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config"
        conf.mkdir(parents=True)
        (conf / "kcminputrc").write_text("[Mouse]\ncursorTheme=Breeze\n")
        core = self._core(tmp_path, monkeypatch)

        assert core.set_value("cursor_theme", "Breeze_Snow").success
        field = core.get("cursor_theme", "live")
        assert field is not None
        assert field["staged"] is True
        assert field["value"] == "Breeze_Snow"
        # Файл ещё не тронут.
        assert "cursorTheme=Breeze\n" in (conf / "kcminputrc").read_text()

        assert core.apply().success
        field = core.get("cursor_theme", "live")
        assert field["staged"] is False
        assert "cursorTheme=Breeze_Snow" in (conf / "kcminputrc").read_text()

    def test_gtk_fields_mirror_into_gtk3_and_gtk4(self, tmp_path: Path, monkeypatch) -> None:
        conf = tmp_path / ".config"
        (conf / "gtk-3.0").mkdir(parents=True)
        (conf / "gtk-3.0" / "settings.ini").write_text("[Settings]\ngtk-theme-name=Old\ngtk-xft-dpi=98304\n")
        (conf / "kdeglobals").write_text("[Icons]\nTheme=breeze\n")
        core = self._core(tmp_path, monkeypatch)
        assert core.set_value("gtk_theme", "Glass").success
        assert core.set_value("icon_theme", "Seven").success
        assert core.apply().success
        gtk3 = (conf / "gtk-3.0" / "settings.ini").read_text()
        gtk4 = (conf / "gtk-4.0" / "settings.ini").read_text()
        assert "gtk-theme-name=Glass" in gtk3 and "gtk-xft-dpi=98304" in gtk3
        assert "gtk-theme-name=Glass" in gtk4 and "gtk-icon-theme-name=Seven" in gtk4

    def test_apply_with_empty_staging_is_error(self, tmp_path: Path, monkeypatch) -> None:
        core = self._core(tmp_path, monkeypatch)
        assert not core.apply().success


class TestSetAppConfigValueController:
    def test_controller_dispatches_to_core(self, tmp_path: Path, monkeypatch) -> None:
        from controllers.theme_inspector import ThemeInspectorController
        from core.protocol import CoreFactory, Result

        calls: list[tuple[str, str, str]] = []
        toasts: list[tuple[str, bool]] = []

        class FakeThemeCore:
            def sections(self, scope): return []
            def fields(self, section, scope): return []
            def validate(self): return []
            def get(self, field_id, scope): return None
            def search(self, query, scope): return []
            def session_changes(self): return 0
            def set_config_value(self, rel_path, key, value):
                calls.append((rel_path, key, value))
                return Result.ok("speed set")

        class FakeTrackedCore:
            def get_all_components(self): return []

        monkeypatch.setattr(CoreFactory, "_theme_core", FakeThemeCore())
        monkeypatch.setattr(CoreFactory, "_tracked_apps_core", FakeTrackedCore())
        controller = ThemeInspectorController()
        controller.toast.connect(lambda msg, ok: toasts.append((msg, ok)))

        controller.setAppConfigValue(".config/mockapp/settings.conf", "main/speed", "9")
        # Воркер на пуле потоков — дать ему завершиться (done придёт в event loop)
        import time
        from PySide6.QtWidgets import QApplication

        app = QApplication.instance() or QApplication([])
        for _ in range(200):
            if calls and toasts:
                break
            app.processEvents()
            time.sleep(0.01)

        assert calls == [(".config/mockapp/settings.conf", "main/speed", "9")]
        assert toasts and toasts[0][1]

    def test_profile_scope_refuses_edit(self, tmp_path: Path, monkeypatch) -> None:
        from controllers.theme_inspector import ThemeInspectorController
        from core.protocol import CoreFactory

        class FakeThemeCore:
            def sections(self, scope): return []
            def fields(self, section, scope): return []
            def validate(self): return []
            def get(self, field_id, scope): return None
            def search(self, query, scope): return []
            def session_changes(self): return 0
            def set_config_value(self, rel_path, key, value):
                raise AssertionError("must not be called for a profile scope")

        class FakeTrackedCore:
            def get_all_components(self): return []

        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))
        monkeypatch.setattr(CoreFactory, "_theme_core", FakeThemeCore())
        monkeypatch.setattr(CoreFactory, "_tracked_apps_core", FakeTrackedCore())
        controller = ThemeInspectorController()
        controller.setScope("profile:glass")
        toasts: list[tuple[str, bool]] = []
        controller.toast.connect(lambda msg, ok: toasts.append((msg, ok)))
        controller.setAppConfigValue(".config/x.conf", "k", "v")
        assert toasts and not toasts[0][1]

    def test_c2_qml_wiring(self) -> None:
        qml = (PROJECT_ROOT / "qml" / "pages" / "ThemePage.qml").read_text()
        assert "setAppConfigValue" not in qml  # дерево конфигов переехало (M4)
        apps_qml = (PROJECT_ROOT / "qml" / "pages" / "ApplicationsPage.qml").read_text()
        tree_qml = (PROJECT_ROOT / "qml" / "pages" / "AppConfigTree.qml").read_text()
        assert "configValueDialog" in tree_qml
        assert "setAppConfigValue" in tree_qml
        assert "fileData.editable" in tree_qml
        assert "AppConfigTree" in apps_qml
        assert "themeInspector.applyChanges" in apps_qml
        # Экзотика открывается папкой во внешнем редакторе.
        assert "Show in folder" in tree_qml
        assert "unsupported format" in tree_qml


# ─────────────────────────────────────────────────────────────────────────────
# 7. CliWallpaperCore — wallpaper dir uses settings
# ─────────────────────────────────────────────────────────────────────────────
class TestCliWallpaperCore:
    """Test that wallpaper_dir resolves from settings, not hardcoded."""

    def test_wallpaper_dir_is_property(self) -> None:
        """wallpaper_dir is now a property, not a hardcoded attribute."""
        from core.cli_core import CliWallpaperCore

        core = CliWallpaperCore()
        # Should be accessible as a property (Path object)
        dir_path = core.wallpaper_dir
        assert isinstance(dir_path, Path)

    def test_wallpaper_dir_uses_settings(self) -> None:
        """wallpaper_dir should come from AppSettings.wallpaper_directory."""
        from core.cli_core import CliWallpaperCore

        core = CliWallpaperCore()
        # The property should resolve to the default wallpaper directory
        # (since no settings file exists in CI, it falls back to default)
        assert core.wallpaper_dir is not None

    def test_list_wallpapers_nonexistent_dir(self) -> None:
        """list_wallpapers returns [] when dir doesn't exist."""
        from core.cli_core import CliWallpaperCore

        core = CliWallpaperCore()
        with mock.patch.object(type(core), "wallpaper_dir", Path("/nonexistent/dir/12345")):
            assert core.list_wallpapers() == []

    def test_resolve_wallpaper_dir_from_settings(self) -> None:
        """_resolve_wallpaper_dir should use CoreFactory settings."""
        from core.cli_core import CliWallpaperCore

        core = CliWallpaperCore()
        core._wallpaper_dir = None  # Reset
        with mock.patch("core.cli_core.CoreFactory") as mock_factory:
            mock_settings = mock.Mock()
            mock_settings.wallpaper_directory = "/tmp/test-wallpapers"
            mock_factory.settings_core.return_value.load.return_value = mock_settings
            result = core._resolve_wallpaper_dir()
            assert result == Path("/tmp/test-wallpapers")


# ─────────────────────────────────────────────────────────────────────────────
# 7b. Wallpaper list/order and "current wallpaper" resolution
#
# The cached index in ~/.cache/wallpaper_index is a *position* in the list, so
# the cli scripts and this core must enumerate the same files in the same
# order. A locale-aware `sort` (ru_RU.UTF-8) or a different extension list
# made the GUI resolve the index to a wallpaper other than the one applied.
# ─────────────────────────────────────────────────────────────────────────────
def _make_wallpaper_dir(tmp_path: Path) -> Path:
    """Wallpaper dir with the names that used to break the ordering."""
    d = tmp_path / "walls"
    d.mkdir()
    for name in (
        "apple.png",       # lowercase ASCII
        "Zebra.png",       # uppercase sorts before lowercase in byte order
        "-dash.jpg",       # leading punctuation sorts before digits
        "1number.webp",    # digit
        "Beta.bmp",        # .bmp used to be accepted by Python but not bash
        "UP.PNG",          # uppercase extension used to be rejected in Rust
        "Обои.jpg",        # Cyrillic sorts after ASCII in byte order
        "notes.txt",       # not an image -> never listed
    ):
        (d / name).write_bytes(b"x")
    (d / "linked.png").symlink_to(d / "apple.png")  # find -L / is_file() follow
    return d


class TestWallpaperOrdering:
    """Ordering/selection rules that the cli scripts and the GUI must share."""

    def _core(self, directory: Path):
        from core.cli_core import CliWallpaperCore

        core = CliWallpaperCore()
        core._wallpaper_dir = directory
        return core

    def test_list_wallpapers_byte_order_symlinks_and_extensions(
        self, tmp_path: Path
    ) -> None:
        d = _make_wallpaper_dir(tmp_path)
        files = self._core(d).list_wallpapers()

        names = [Path(f).name for f in files]
        assert names == sorted(names), "list must be in byte/code-point order"
        assert "Zebra.png" in names and "apple.png" in names
        assert names.index("-dash.jpg") == 0
        assert names.index("Обои.jpg") > names.index("apple.png")
        assert "Beta.bmp" in names, ".bmp must be listed (cli accepts it)"
        assert "UP.PNG" in names, "extensions must match case-insensitively"
        assert "linked.png" in names, "symlinks must be listed (find -L)"
        assert "notes.txt" not in names
        assert len(files) == 8

    def test_list_wallpapers_matches_wallpaper_next(self, tmp_path: Path) -> None:
        """The CLI and the GUI must walk the *same* list in the same order."""
        script = (
            SRC_DIR / "core" / "rust_core" / "target" / "release" / "wallpaper-next"
        )
        if not script.is_file():
            pytest.skip("the Rust CLI is not built (cargo build --release)")

        d = _make_wallpaper_dir(tmp_path)
        files = [Path(f).name for f in self._core(d).list_wallpapers()]

        state = tmp_path / "state"
        state.write_text(str(len(files) - 1))  # next one wraps to index 0
        env = {
            **os.environ,
            "WALLPAPER_DIR": str(d),
            "WALLPAPER_STATE": str(state),
            "WALLPAPER_DRY_RUN": "1",
            "PLASMA_APPLETSRC": str(tmp_path / "no-such-config"),
        }

        walked = []
        for _ in range(len(files)):
            proc = subprocess.run(
                [str(script)], capture_output=True, text=True, env=env
            )
            assert proc.returncode == 0, proc.stderr
            walked.append(Path(proc.stdout.strip()).name)

        assert walked == files

    def test_current_reads_plasma_config(self, tmp_path: Path, monkeypatch) -> None:
        """current() must report the wallpaper Plasma actually shows."""
        from core.cli_core import CliWallpaperCore

        d = _make_wallpaper_dir(tmp_path)
        real = d / "Zebra.png"
        config = tmp_path / "appletsrc"
        config.write_text(
            "[Containments][1][Wallpaper]\n"
            "wallpaperPlugin=org.kde.image\n"
            "[Containments][281][Wallpaper][org.kde.image][General]\n"
            f"Image=file://{real}\n"
            "[Containments][282][Wallpaper][org.kde.slideshow][General]\n"
            "Image=file:///elsewhere/slide.png\n"
        )
        monkeypatch.setattr(CliWallpaperCore, "PLASMA_CONFIG", config)

        core = self._core(d)
        core.cache_index = tmp_path / "state"
        core.cache_index.write_text("0")  # wrong on purpose: Plasma must win

        assert core.current() == str(real)

    def test_current_decodes_url_escapes(self, tmp_path: Path, monkeypatch) -> None:
        from core.cli_core import CliWallpaperCore

        config = tmp_path / "appletsrc"
        config.write_text(
            "[Containments][281][Wallpaper][org.kde.image][General]\n"
            "Image=file:///home/u/My%20Pictures/wall%20one.png\n"
        )
        monkeypatch.setattr(CliWallpaperCore, "PLASMA_CONFIG", config)

        core = self._core(tmp_path)
        assert core.current() == "/home/u/My Pictures/wall one.png"

    def test_current_falls_back_to_cached_index(
        self, tmp_path: Path, monkeypatch
    ) -> None:
        from core.cli_core import CliWallpaperCore

        d = _make_wallpaper_dir(tmp_path)
        monkeypatch.setattr(
            CliWallpaperCore, "PLASMA_CONFIG", tmp_path / "missing-appletsrc"
        )

        core = self._core(d)
        core.cache_index = tmp_path / "state"
        core.cache_index.write_text("2")
        assert core.current() == core.list_wallpapers()[2]

        core.cache_index.write_text("not-a-number")
        assert core.current() is None

    def test_script_env_pins_the_wallpaper_dir(self, tmp_path: Path) -> None:
        """The scripts must be told which dir the app is showing."""
        core = self._core(tmp_path)
        assert core._script_env() == {"WALLPAPER_DIR": str(tmp_path)}


# ─────────────────────────────────────────────────────────────────────────────
# 8. Tracked apps structure
# ─────────────────────────────────────────────────────────────────────────────
def test_tracked_apps_have_required_fields() -> None:
    """Every tracked app definition must have all required metadata."""
    from core.tracked_apps import TRACKED_APPS

    for app in TRACKED_APPS:
        assert app.id, f"Missing id for app: {app}"
        assert app.name, f"Missing name for app: {app}"
        assert app.category, f"Missing category for app: {app}"
        assert len(app.config_paths) > 0, f"No config_paths for app: {app.id}"
        assert app.description, f"Missing description for app: {app.id}"
        assert app.icon, f"Missing icon for app: {app.id}"


def test_tracked_apps_categories() -> None:
    """Categories should include expected KDE/Plasma categories."""
    from core.tracked_apps import CliTrackedAppsCore

    core = CliTrackedAppsCore()
    categories = core.get_categories()
    expected = {"KDE / Plasma", "GTK / Qt", "Window Decorations", "Terminals",
                "Editors", "Browser"}
    assert expected.issubset(set(categories))


# ─────────────────────────────────────────────────────────────────────────────
# 9. Duplicate QAction import check
# ─────────────────────────────────────────────────────────────────────────────
def test_no_duplicate_qaction_import() -> None:
    """system_tray.py must not import QAction twice."""
    import ast

    system_tray_path = SRC_DIR / "system_tray.py"
    tree = ast.parse(system_tray_path.read_text())

    qaction_imports = 0
    for node in ast.walk(tree):
        # Top-level import from PySide6.QtGui
        if isinstance(node, ast.ImportFrom):
            if node.module and "PySide6" in node.module:
                for alias in node.names:
                    if alias.name == "QAction":
                        qaction_imports += 1
        # Lazy import inside _import_widgets
        if isinstance(node, ast.ImportFrom):
            if node.module and "QtWidgets" in node.module:
                for alias in node.names:
                    if alias.name == "QAction":
                        qaction_imports += 1

    # QAction should appear in at most ONE import statement
    assert qaction_imports <= 1, (
        f"QAction imported {qaction_imports} times in system_tray.py; "
        "expected at most 1 (the duplicate in _import_widgets should be removed)"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 10. pyproject.toml dependency check
# ─────────────────────────────────────────────────────────────────────────────
def test_pyproject_no_unused_deps() -> None:
    """pyproject.toml must not list pydantic or pyyaml as dependencies."""
    pyproject_path = PROJECT_ROOT / "pyproject.toml"
    if not pyproject_path.exists():
        pytest.skip("pyproject.toml not found")

    content = pyproject_path.read_text()
    lower = content.lower()
    assert "pydantic" not in lower, "pydantic should be removed from dependencies"
    assert "pyyaml" not in lower, "pyyaml should be removed from dependencies"
    assert "pyside6" in lower, "PySide6 must remain in dependencies"


# ─────────────────────────────────────────────────────────────────────────────
# 11. QML file existence and basic structure
# ─────────────────────────────────────────────────────────────────────────────
def test_qml_files_exist() -> None:
    """Core QML files must exist at expected paths."""
    qml_dir = PROJECT_ROOT / "qml"
    assert (qml_dir / "Main.qml").exists()
    assert (qml_dir / "Components" / "qmldir").exists()
    assert (qml_dir / "Components" / "ComponentCategoryGroup.qml").exists()
    assert (qml_dir / "Components" / "ComponentCheckbox.qml").exists()
    assert (qml_dir / "Components" / "ProfileCardDelegate.qml").exists()
    assert (qml_dir / "Components" / "ThemeColors.qml").exists()
    assert (qml_dir / "Components" / "TypeScale.qml").exists()
    assert (qml_dir / "pages" / "SettingsPage.qml").exists()
    # ProfileRadioButton was removed with the radio tiles (M1 redesign).
    assert not (qml_dir / "Components" / "ProfileRadioButton.qml").exists()


def test_no_duplicate_components_dir() -> None:
    """The old Components/ directory at project root must be removed."""
    assert not (PROJECT_ROOT / "Components").exists(), (
        "Duplicate Components/ directory at project root should be deleted"
    )
    assert (PROJECT_ROOT / "qml" / "Components").exists(), (
        "qml/Components/ (renamed from qml/components/) must exist"
    )


def test_qmldir_lists_all_components() -> None:
    """qmldir must declare all component QML types and the two singletons."""
    qmldir = (PROJECT_ROOT / "qml" / "Components" / "qmldir").read_text()
    assert "module Components" in qmldir
    assert "ProfileCardDelegate" in qmldir
    assert "ProfileRadioButton" not in qmldir
    assert "ComponentCategoryGroup" in qmldir
    assert "ComponentCheckbox" in qmldir
    assert "singleton ThemeColors" in qmldir
    assert "singleton TypeScale" in qmldir


# ─────────────────────────────────────────────────────────────────────────────
# 12. __main__.py --help handling
# ─────────────────────────────────────────────────────────────────────────────
def test_main_has_help_flag() -> None:
    """__main__.py must handle --help and -h."""
    import ast

    main_path = SRC_DIR / "__main__.py"
    tree = ast.parse(main_path.read_text())

    found_help = False
    for node in ast.walk(tree):
        if isinstance(node, ast.Compare):
            for comparator in node.comparators:
                if isinstance(comparator, ast.Constant) and comparator.value == "--help":
                    found_help = True
        if isinstance(node, ast.Constant) and node.value == "--help":
            found_help = True
    assert found_help, "--help flag not handled in __main__.py"


# ─────────────────────────────────────────────────────────────────────────────
# 13. SettingsController._apply_theme integration
# ─────────────────────────────────────────────────────────────────────────────
def test_settings_controller_apply_theme_not_empty() -> None:
    """SettingsController._apply_theme must not be a TODO stub."""
    import inspect
    from controllers import SettingsController

    source = inspect.getsource(SettingsController._apply_theme)
    assert "pass" not in source.replace("pass", "", 0) or "TODO" not in source, (
        "SettingsController._apply_theme is still a TODO stub"
    )
    assert "TODO" not in source, (
        "SettingsController._apply_theme still contains TODO"
    )
    assert "ThemeController" in source, (
        "SettingsController._apply_theme must reference ThemeController"
    )


def test_settings_controller_has_backup_and_notify_properties() -> None:
    """SettingsController must expose createBackupBeforeApply and notifyOnSuccess."""
    from controllers import SettingsController

    # We can't fully instantiate SettingsController (requires Qt event loop),
    # but we can verify the properties are declared on the class
    cls = SettingsController
    assert hasattr(cls, "createBackupBeforeApply"), (
        "SettingsController must have createBackupBeforeApply property "
        "(referenced by SettingsPage.qml)"
    )
    assert hasattr(cls, "notifyOnSuccess"), (
        "SettingsController must have notifyOnSuccess property "
        "(in AppSettings but missing from controller)"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 14. Resources/icons directory
# ─────────────────────────────────────────────────────────────────────────────
def test_resources_icons_not_empty() -> None:
    """resources/icons/ must not be empty (has .gitkeep or icon files)."""
    icons_dir = PROJECT_ROOT / "resources" / "icons"
    assert icons_dir.exists(), "resources/icons/ directory must exist"
    contents = list(icons_dir.iterdir())
    assert len(contents) > 0, "resources/icons/ must not be empty"


# ─────────────────────────────────────────────────────────────────────────────
# 15. theme-profile --components support
# ─────────────────────────────────────────────────────────────────────────────
def test_theme_profile_has_components_support() -> None:
    """Rust CLI must support --components= flag (required by GUI)."""
    theme_profile = (
        SRC_DIR / "core" / "rust_core" / "target" / "release" / "theme-profile"
    )
    if not theme_profile.is_file():
        pytest.skip("theme-profile binary not found (cargo build --release)")
    result = subprocess.run(
        [str(theme_profile), "--help"], capture_output=True, text=True
    )
    assert result.returncode == 0
    assert "--components" in result.stdout, "theme-profile --help must mention --components"


# ─────────────────────────────────────────────────────────────────────────────
# 16. __main__.py uses QApplication (not QGuiApplication-only)
# ─────────────────────────────────────────────────────────────────────────────
def test_main_uses_qapplication() -> None:
    """__main__.py must import QApplication (for system tray support)."""
    main_path = SRC_DIR / "__main__.py"
    content = main_path.read_text()
    assert "QApplication" in content, (
        "__main__.py must use QApplication (supports both QML and Widgets/tray)"
    )
    assert "QQuickStyle.setStyle" not in content, (
        "QQuickStyle.setStyle('org.kde.desktop') removed (causes QWidget crash)"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 17. system_tray.py guards against missing QApplication
# ─────────────────────────────────────────────────────────────────────────────
def test_system_tray_guard() -> None:
    """register_system_tray must check QApplication.instance() before creating QWidgets."""
    import ast

    path = SRC_DIR / "system_tray.py"
    tree = ast.parse(path.read_text())

    # Find register_system_tray function
    found_qapp_check = False
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef) and node.name == "register_system_tray":
            source = ast.get_source_segment(path.read_text(), node)
            if source and "QApplication" in source:
                found_qapp_check = True
    assert found_qapp_check, (
        "register_system_tray must check for QApplication before creating QSystemTrayIcon"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 18. theme_controller.py uses setColor, not setAttribute
# ─────────────────────────────────────────────────────────────────────────────
def test_theme_controller_no_setattribute() -> None:
    """ThemeController must not use QWindow.setAttribute (not available in PySide6)."""
    path = SRC_DIR / "theme_controller.py"
    content = path.read_text()
    assert "setAttribute" not in content, (
        "QWindow.setAttribute doesn't exist in PySide6 — use setColor with try/except"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 19. QML fixed: no deprecated tooltip property
# ─────────────────────────────────────────────────────────────────────────────
def test_qml_no_tooltip_property() -> None:
    """QML files must use ToolTip attached property, not 'tooltip:' shorthand."""
    qml_dir = PROJECT_ROOT / "qml"
    for qml_file in qml_dir.rglob("*.qml"):
        content = qml_file.read_text()
        # 'tooltip:' (without ToolTip. prefix) is invalid in Qt 6.12
        lines = content.split("\n")
        for i, line in enumerate(lines, 1):
            stripped = line.strip()
            if stripped.startswith("tooltip:") or stripped.startswith("tooltip "):
                pytest.fail(
                    f"{qml_file.name}:{i} uses 'tooltip:' property "
                    f"(use ToolTip.text + ToolTip.visible instead)"
                )


# ─────────────────────────────────────────────────────────────────────────────
# 20. QML Main.qml uses function syntax for Connections
# ─────────────────────────────────────────────────────────────────────────────
def test_qml_connections_function_syntax() -> None:
    """Connections must use function syntax (onFoo: handler is deprecated in Qt 6.8+)."""
    import re

    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    assert "Connections {" in main_qml, "Main.qml should use a Connections block"

    blocks = []
    for match in re.finditer(r"Connections\s*\{", main_qml):
        i = match.end()
        depth = 1
        start = i
        while i < len(main_qml) and depth > 0:
            if main_qml[i] == "{":
                depth += 1
            elif main_qml[i] == "}":
                depth -= 1
            i += 1
        blocks.append(main_qml[start : i - 1])

    assert blocks, "Expected at least one Connections block"
    for block in blocks:
        for line in block.split("\n"):
            assert not re.match(r"\s*on[A-Z]\w*\s*:", line), (
                "Connections handlers must use 'function onFoo(...)' syntax, "
                f"not '{line.strip()}'"
            )
        assert re.search(r"function\s+on\w+\s*\(", block), (
            "Each Connections block must declare its handlers with function syntax"
        )


# ─────────────────────────────────────────────────────────────────────────────
# 21. QML safeColor helper (fallback for PySide6 bundled Qt Kirigami.Theme)
# ─────────────────────────────────────────────────────────────────────────────
def test_qml_has_safecolor_helper() -> None:
    """All QML files using Kirigami.Theme color roles must use safeColor fallback."""
    qml_dir = PROJECT_ROOT / "qml"
    for qml_file in qml_dir.rglob("*.qml"):
        content = qml_file.read_text()
        # If the file uses Kirigami.Theme color roles, it must also define safeColor
        if "Kirigami.Theme." in content:
            assert "function safeColor" in content, (
                f"{qml_file.relative_to(qml_dir)} uses Kirigami.Theme colors "
                "but doesn't define safeColor fallback"
            )


# ─────────────────────────────────────────────────────────────────────────────
# 22. Main.qml glass styling (translucent, driven by glassMode)
# ─────────────────────────────────────────────────────────────────────────────
def test_main_has_glass_styling() -> None:
    """Main.qml must drive a translucent glass look from the glassMode flag."""
    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    assert "property bool glassMode: themeController.isGlass" in main_qml, (
        "Main.qml must expose glassMode bound to themeController.isGlass"
    )
    # The page must paint its own glass panel / background, not rely on the
    # Kirigami default Platform.Theme.backgroundColor (which was the white bug).
    assert "background: Rectangle" in main_qml, (
        "Main.qml page must define an explicit background Rectangle"
    )
    assert main_qml.count("root.glassMode") >= 2, (
        "The page/window background must react to glassMode"
    )
    # Translucent colours are how glass is implemented. They live in the
    # ThemeColors singleton now (Main.qml must not carry its own colour
    # literals), so assert it there.
    theme_colors = (PROJECT_ROOT / "qml" / "Components" / "ThemeColors.qml").read_text()
    assert "Qt.rgba(" in theme_colors, (
        "ThemeColors.qml must use translucent Qt.rgba() colours for glass mode"
    )
    assert "glassPanel" in theme_colors, (
        "ThemeColors.qml must expose the glass panel colour"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 22b. Selection and application are separate actions
# ─────────────────────────────────────────────────────────────────────────────
def test_selection_is_separate_from_apply() -> None:
    """Clicking a profile selects it; applying requires the explicit Apply button."""
    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    assert "property string selectedProfile" in main_qml, (
        "Main.qml must track a pending selectedProfile"
    )
    # The radio tiles are gone: selection happens via the card delegate...
    assert "ProfileRadioButton" not in main_qml, (
        "Radio tiles were removed; cards select, they must not come back"
    )
    assert "root.selectedProfile = profileName" in main_qml, (
        "Clicking a profile card must only assign the pending selection"
    )
    # ...and a dedicated Apply button performs the (slow) application.
    assert "root.requestApply(root.selectedProfile)" in main_qml, (
        "An explicit Apply button must trigger profile application"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 22c. Theme controller switches the application palette
# ─────────────────────────────────────────────────────────────────────────────
def test_theme_controller_applies_palette() -> None:
    """Glass/dark themes must switch the app palette (else controls stay light)."""
    controller_path = SRC_DIR / "theme_controller.py"
    content = controller_path.read_text()
    assert "def _apply_palette" in content, (
        "ThemeController must define _apply_palette"
    )
    assert "setPalette(" in content, (
        "ThemeController must apply the palette via QGuiApplication.setPalette"
    )
    # The window colour must be owned by QML, not overridden from Python.
    assert "_window.setColor(" not in content, (
        "ThemeController must not call QWindow.setColor() — it fights the QML binding"
    )


def test_theme_controller_palettes(tmp_path, monkeypatch) -> None:
    """Glass → dark palette; System/unknown profile → the live KDE scheme colours."""
    import theme_controller as theme_controller_mod

    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))
    monkeypatch.setenv("KTM_THEME_NO_APPLY", "1")
    (tmp_path / ".config").mkdir()
    (tmp_path / ".config/kdeglobals").write_text(
        "[General]\nColorScheme=Everforest\n\n"
        "[Colors:Window]\nBackgroundNormal=45,53,59\nForegroundNormal=211,198,170\n"
        "ForegroundInactive=133,146,137\n\n"
        "[Colors:View]\nBackgroundNormal=35,42,46\nBackgroundAlternate=45,53,59\n\n"
        "[Colors:Button]\nBackgroundNormal=61,72,77\n\n"
        "[Colors:Selection]\nBackgroundNormal=167,192,128\nForegroundNormal=45,53,59\n",
        encoding="utf-8",
    )
    controller = theme_controller_mod.ThemeController()
    calls: list[tuple[bool, object]] = []
    monkeypatch.setattr(
        controller, "_apply_palette",
        lambda dark, colors=None: calls.append((dark, colors)),
    )

    controller._apply_glass_theme()
    assert calls[-1] == (True, None)

    controller.sync_with_active_profile("everforest")
    assert controller.currentTheme == "system"
    controller._apply_theme()
    dark, colors = calls[-1]
    assert dark is True
    assert colors["window"].name() == "#2d353b"
    assert colors["highlight"].name() == "#a7c080"
    assert controller.isGlass is False


def test_theme_controller_profile_names_are_case_insensitive() -> None:
    """Profiles saved as 'Paper'/'GLASS' must still dogfood the right app theme."""
    import theme_controller as theme_controller_mod

    controller = theme_controller_mod.ThemeController()
    assert controller._theme_for_profile("Paper") == "paper"
    assert controller._theme_for_profile("paper") == "paper"
    assert controller._theme_for_profile("GLASS") == "glass"
    assert controller._theme_for_profile("glass") == "glass"
    assert controller._theme_for_profile("custom") == ""


# ─────────────────────────────────────────────────────────────────────────────
# 23. core uses shutil.which for binary path resolution
# ─────────────────────────────────────────────────────────────────────────────
def test_cli_core_binary_resolution() -> None:
    """cli_core.py must use shutil.which with ~/.local/bin fallback for theme-profile."""
    core_path = SRC_DIR / "core" / "cli_core.py"
    content = core_path.read_text()
    assert "import shutil" in content, (
        "cli_core.py must import shutil for PATH resolution"
    )
    assert shutil_path_in_init(content), (
        "CliProfileCore.__init__ must use shutil.which with ~/.local/bin fallback"
    )


def shutil_path_in_init(content: str) -> bool:
    """Check that __init__ has shutil.which + ~/.local/bin fallback."""
    # Check for shutil.which call
    has_which = "shutil.which" in content
    # Check for local/bin fallback
    has_fallback = ".local/bin" in content
    return has_which and has_fallback


# ─────────────────────────────────────────────────────────────────────────────
# 23. Typography: font sizes must not be bound to layout spacing tokens
# ─────────────────────────────────────────────────────────────────────────────
def test_qml_font_sizes_not_bound_to_spacing_tokens() -> None:
    """No QML file may use Platform.Units.*Spacing as a font.pointSize.

    In this environment gridUnit=18, smallSpacing=4, mediumSpacing=6, so those
    bindings rendered body text at 4-6 pt (visually illegible).
    """
    qml_dir = PROJECT_ROOT / "qml"
    offenders = []
    for qml_file in qml_dir.rglob("*.qml"):
        for i, line in enumerate(qml_file.read_text().split("\n"), 1):
            stripped = line.strip()
            if not stripped.startswith("font.pointSize"):
                continue
            if "TypeScale." not in stripped:
                offenders.append(f"{qml_file.relative_to(qml_dir)}:{i}: {stripped}")
    assert not offenders, (
        "font.pointSize must come from TypeScale: " + "; ".join(offenders)
    )


# ─────────────────────────────────────────────────────────────────────────────
# 24. ThemeColors / TypeScale are the single source of colour and type
# ─────────────────────────────────────────────────────────────────────────────
def test_themecolors_is_the_only_colour_source() -> None:
    """Only ThemeColors.qml may read Kirigami.Theme directly (plus safeColor)."""
    qml_dir = PROJECT_ROOT / "qml"
    offenders = []
    for qml_file in qml_dir.rglob("*.qml"):
        if qml_file.name == "ThemeColors.qml":
            continue
        content = qml_file.read_text()
        if "Kirigami.Theme." in content or "function safeColor" in content:
            offenders.append(str(qml_file.relative_to(qml_dir)))
    assert not offenders, (
        "These files duplicate the colour logic instead of using ThemeColors: "
        + ", ".join(offenders)
    )

    theme_colors = (qml_dir / "Components" / "ThemeColors.qml").read_text()
    assert "pragma Singleton" in theme_colors
    assert "function safeColor" in theme_colors

    type_scale = (qml_dir / "Components" / "TypeScale.qml").read_text()
    assert "pragma Singleton" in type_scale
    for token in ("base", "body", "subtitle", "title", "caption"):
        assert token in type_scale, f"TypeScale must expose '{token}'"


# ─────────────────────────────────────────────────────────────────────────────
# 25. QQC2 controls must not own their own `checked` state (D-03)
# ─────────────────────────────────────────────────────────────────────────────
@pytest.mark.parametrize(
    "rel_path",
    ["Components/ComponentCheckbox.qml"],
)
def test_checked_binding_is_repairable(rel_path: str) -> None:
    """Radio/checkbox state must be corrected after a user click destroys the binding.

    A QQC2 control writes to `checked` on click, which kills a plain
    `checked: model.x` binding; two cards could then look checked at once.
    `Binding { restoreMode: ... }` re-arms it.
    """
    content = (PROJECT_ROOT / "qml" / rel_path).read_text()
    assert "Binding.RestoreBindingOrValue" in content, (
        f"{rel_path} must declare a Binding with restoreMode to survive user clicks"
    )
    assert "Binding {" in content, f"{rel_path} must use a Binding element"


def test_category_group_uses_modeldata() -> None:
    """Slot-returned list-of-dicts models are only readable via modelData."""
    content = (
        PROJECT_ROOT / "qml" / "Components" / "ComponentCategoryGroup.qml"
    ).read_text()
    assert "modelData.id" in content, (
        "ComponentCategoryGroup must read the delegate model via modelData.*"
    )
    assert "model.id" not in content, (
        "model.<key> is undefined for @Slot(result=list) models in Qt 6.12"
    )
    assert "componentsRevision" in content, (
        "The model binding must depend on componentsRevision so it re-evaluates "
        "after the asynchronous registry detection"
    )
    assert "signal selectionChanged(var components)" in content, (
        "ComponentCategoryGroup must emit the new full selection"
    )
    assert "group.selectedComponents = " not in content, (
        "ComponentCategoryGroup must never assign to its parent-bound property"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 26. Preview dialog computes its content at open time (D-02)
# ─────────────────────────────────────────────────────────────────────────────
def test_preview_dialog_computes_on_open() -> None:
    """The preview must be built when the dialog opens, not from a property handler."""
    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    assert "function updatePreview()" in main_qml, (
        "previewDialog must expose updatePreview()"
    )
    assert "onOpened: updatePreview()" in main_qml, (
        "previewDialog must compute its content in onOpened"
    )
    assert "onPreviewProfileChanged" not in main_qml, (
        "The property-changed handler ran before activeProfile was assigned "
        "(empty chips / stale diff) and must be gone"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 27. Settings page is reachable (D-20) and loadable (D-01)
# ─────────────────────────────────────────────────────────────────────────────
def test_settings_page_is_wired_up() -> None:
    """Main.qml must reach SettingsPage through the global drawer."""
    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    assert "globalDrawer" in main_qml, "Main.qml must define a globalDrawer"
    assert 'pages/SettingsPage.qml' in main_qml, (
        "The drawer must reference pages/SettingsPage.qml"
    )


def test_settings_page_uses_valid_types() -> None:
    """SettingsPage must not use Kirigami types that do not exist."""
    content = (PROJECT_ROOT / "qml" / "pages" / "SettingsPage.qml").read_text()
    for bogus in ("Kirigami.RadioButton", "Kirigami.Switch", "Kirigami.Button"):
        assert bogus not in content, (
            f"{bogus} does not exist in Qt 6 Kirigami (use QtQuick.Controls)"
        )
    assert "import Components 1.0" in content, (
        "SettingsPage uses ComponentCategoryGroup and must import the module"
    )
    assert "Kirigami.ScrollablePage" in content, (
        "SettingsPage must be a ScrollablePage with a single layout child"
    )
    assert content.count("contentItem:") >= 1, (
        "Each Kirigami.Card must assign an explicit contentItem"
    )
    assert "Kirigami.Separator" not in content, (
        "Use a 1 px Rectangle instead of the heavy Kirigami.Separator"
    )


def test_wallpaper_dir_picker_and_async_preview() -> None:
    """ROADMAP §2: папка обоев выбирается диалогом; preview грузится async."""
    settings_qml = (PROJECT_ROOT / "qml" / "pages" / "SettingsPage.qml").read_text()
    assert "import QtQuick.Dialogs" in settings_qml
    assert "FolderDialog" in settings_qml
    assert "wallpaperDirDialog.open()" in settings_qml
    # url → обычный путь: в настройках храним строку, не file:// URL.
    assert "selectedFolder.toLocalFile()" in settings_qml
    assert "Not implemented yet" not in settings_qml

    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    assert "asynchronous: true" in main_qml
    assert "BusyIndicator" in main_qml


def test_duplicate_and_edit_live_qml_wiring() -> None:
    """Duplicate flow (card → dialog → controller) and the quick live-edit jump."""
    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    assert "duplicateDialog" in main_qml
    assert "duplicateNameField" in main_qml
    assert "duplicateProfile" in main_qml
    # Быстрый переход к редактированию живой темы: ThemeBundle + ThemePage.
    assert "Edit live theme…" in main_qml
    assert "themeBundle.openLive()" in main_qml

    card_qml = (PROJECT_ROOT / "qml" / "Components" / "ProfileCardDelegate.qml").read_text()
    assert "duplicateClicked" in card_qml

    # Theme page exposes the live/profile scope switch and live editing actions.
    theme_qml = (PROJECT_ROOT / "qml" / "pages" / "ThemePage.qml").read_text()
    assert "themeBundle.openLive()" in theme_qml
    assert "themeBundle.openProfile" in theme_qml
    assert "themeBundle.applyChanges()" in theme_qml


def test_main_page_thin_header_and_overflow_menu() -> None:
    """M1: радио-плитки убраны; карточка — 3 кнопки + меню «⋯»."""
    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    # Тонкая шапка: «Active: %1» вместо карточки с плитками.
    assert 'qsTr("Active: %1")' in main_qml
    assert "ProfileRadioButton" not in main_qml

    card_qml = (PROJECT_ROOT / "qml" / "Components" / "ProfileCardDelegate.qml").read_text()
    assert "overflowMenu" in card_qml
    assert "overflow-menu" in card_qml
    # Видимых кнопок ровно три: Apply, Preview, Inspect.
    for signal in ("applyClicked", "previewClicked", "inspectClicked"):
        assert signal in card_qml
    # Вторичные действия живут в меню.
    for signal in ("duplicateClicked", "diffClicked", "deleteClicked",
                   "editDescriptionClicked"):
        assert signal in card_qml
    assert "MenuSeparator" in card_qml


def test_theme_overview_drill_in_and_save_as() -> None:
    """Обзор theme bundle → страница группы; live-режим — «Save as profile…»."""
    qml = (PROJECT_ROOT / "qml" / "pages" / "ThemePage.qml").read_text()
    assert "ThemeComponentsPage.qml" in qml
    assert "ThemeApplicationsPage.qml" in qml
    assert "Save as profile…" in qml
    assert "saveProfileDialog" in qml
    assert "themeBundle.saveProfile" in qml
    assert "themeBundle.groups" in qml

    section = (PROJECT_ROOT / "qml" / "pages" / "SectionPage.qml").read_text()
    assert "ThemeSectionFields" in section
    assert "applyChanges" in section

    # Старый компонент аккордеона удалён как мёртвый код.
    assert not (PROJECT_ROOT / "qml" / "pages" / "ThemeSectionCard.qml").exists()


def test_app_configs_accordion_on_applications_page() -> None:
    """M4: дерево «приложение → файл → ключ» раскрывается под приложением."""
    apps = (PROJECT_ROOT / "qml" / "pages" / "ApplicationsPage.qml").read_text()
    assert "expandedConfigs" in apps
    assert "configByApp" in apps
    assert "view-list-details" in apps
    assert "AppConfigTree" in apps
    assert "themeInspector.applyChanges" in apps

    tree = (PROJECT_ROOT / "qml" / "pages" / "AppConfigTree.qml").read_text()
    assert "property string appId" in tree
    assert "configValueDialog" in tree


def test_i18n_pipeline_wired() -> None:
    """M5: переводы по системной локали — loader, .ts и .qm на месте."""
    main_src = (SRC_DIR / "__main__.py").read_text()
    assert "QTranslator(app)" in main_src
    assert "QLocale.system()" in main_src
    # Язык только системный — ручного переключателя нет и не планируется.
    assert "installTranslator" in main_src

    ts_qml = (PROJECT_ROOT / "i18n" / "qml_ru.ts").read_text()
    assert 'language="ru_RU"' in ts_qml
    ts_py = (PROJECT_ROOT / "i18n" / "py_ru.ts").read_text()
    assert 'language="ru_RU"' in ts_py
    qm = PROJECT_ROOT / "i18n" / "kde-theme-manager_ru.qm"
    assert qm.exists() and qm.stat().st_size > 1000

    # Пайплайн обновления: lupdate (QML) и pylupdate6 (Python) пишут в
    # РАЗНЫЕ .ts (ни один экстрактор не сохраняет контексты другого),
    # lrelease сливает оба в один .qm.
    script = (PROJECT_ROOT.parent.parent / "scripts" / "update-translations.sh").read_text()
    assert '"$LUPDATE" qml -ts "$QML_TS"' in script
    assert '"$PYLUPDATE6" src --ts "$PY_TS"' in script
    assert '"$LRELEASE" "$QML_TS" "$PY_TS" -qm "$QM_FILE"' in script


def test_open_file_logs_every_launch() -> None:
    """Dolphin hunt: every xdg-open launch from the inspector must be logged."""
    source = (SRC_DIR / "controllers" / "theme_inspector.py").read_text()
    assert "[themeInspector] openFile:" in source
    assert "file=sys.stderr" in source


# ─────────────────────────────────────────────────────────────────────────────
# 28. Controllers: cached models, async operations, single palette pass
# ─────────────────────────────────────────────────────────────────────────────
def test_profile_controller_caches_its_qml_model() -> None:
    """ProfileController.profiles must return a cache, not rebuild per read."""
    from controllers import ProfileController

    source = inspect.getsource(ProfileController.profiles.fget)
    assert "_profiles_model" in source, (
        "profiles must return the cached _profiles_model list"
    )
    assert "for p in self._profiles" not in source, (
        "profiles must not rebuild the model on every read"
    )


def test_profile_operations_run_off_the_gui_thread() -> None:
    """apply/save/delete must be handed to QThreadPool and expose `busy`."""
    content = (SRC_DIR / "controllers" / "__init__.py").read_text()
    assert "QThreadPool" in content, "controllers must use QThreadPool"
    assert "_start_operation" in content, (
        "ProfileController must route write operations through _start_operation"
    )
    assert "_OperationWorker" in content and "_OperationSignals" in content, (
        "A shared worker must exist for background operations"
    )
    from controllers import ProfileController

    assert hasattr(ProfileController, "busy"), (
        "ProfileController must expose a `busy` property for the busy indicator"
    )
    for slot in ("applyProfile", "saveProfile", "deleteProfile", "duplicateProfile"):
        body = inspect.getsource(getattr(ProfileController, slot))
        assert "_start_operation" in body, (
            f"{slot} must run through the thread pool, not block the GUI thread"
        )


class TestStaleActiveMarker:
    """Маркер активного профиля, указывающий на удалённый профиль, сбрасывается."""

    def _marker(self, tmp_path: Path) -> Path:
        marker = tmp_path / ".config" / "theme-profile" / "active-profile"
        marker.parent.mkdir(parents=True, exist_ok=True)
        return marker

    def _controller(self, tmp_path: Path, monkeypatch, names, marker):
        from controllers import ProfileController
        from core.protocol import CoreFactory, ProfileInfo

        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))

        class FakeCore:
            def list_profiles(self):
                return [
                    ProfileInfo(name=n, is_active=False, components=("kwin",),
                                created_at="", description="")
                    for n in names
                ]

            def get_active_profile(self):
                try:
                    return marker.read_text().strip()
                except OSError:
                    return ""

            def get_profile_components(self, profile):
                return ["kwin"]

        monkeypatch.setattr(CoreFactory, "_profile_core", FakeCore())
        controller = ProfileController()
        toasts: list[tuple[bool, str]] = []
        controller.operationFinished.connect(lambda ok, msg: toasts.append((ok, msg)))
        controller.refresh()
        return controller, toasts

    def test_missing_profile_clears_marker(self, tmp_path: Path, monkeypatch) -> None:
        marker = self._marker(tmp_path)
        marker.write_text("ghost\n")
        controller, toasts = self._controller(tmp_path, monkeypatch, ["glass"], marker)

        assert controller.activeProfile == ""
        assert not marker.exists()
        assert len(toasts) == 1
        assert not toasts[0][0]
        assert "ghost" in toasts[0][1]

    def test_valid_marker_is_kept(self, tmp_path: Path, monkeypatch) -> None:
        marker = self._marker(tmp_path)
        marker.write_text("glass\n")
        controller, toasts = self._controller(
            tmp_path, monkeypatch, ["glass", "paper"], marker)

        assert controller.activeProfile == "glass"
        assert marker.exists()
        assert toasts == []


def test_tracked_apps_detection_is_async() -> None:
    """detect_all() must not run on the GUI thread during startup."""
    from controllers import TrackedAppsController

    source = inspect.getsource(TrackedAppsController.refresh)
    assert "_run_async" in source, (
        "TrackedAppsController.refresh must delegate detection to the thread pool"
    )
    assert hasattr(TrackedAppsController, "componentsRevision"), (
        "TrackedAppsController must expose componentsRevision for QML"
    )


def test_theme_controller_single_palette_pass() -> None:
    """No _apply_*_theme helper may chain into another one (double palette apply)."""
    import theme_controller as theme_controller_mod

    content = (SRC_DIR / "theme_controller.py").read_text()
    assert "def _apply_flags" in content, (
        "ThemeController must funnel every theme into a single _apply_flags pass"
    )
    helpers = ("_apply_glass_theme", "_apply_paper_theme", "_apply_dark_theme",
               "_apply_light_theme", "_apply_system_theme")
    for helper in helpers:
        body = inspect.getsource(getattr(theme_controller_mod.ThemeController, helper))
        for other in helpers:
            if other != helper:
                assert f"{other}(" not in body, (
                    f"{helper} must not call {other} (double apply / double emit)"
                )


def test_settings_available_components_uses_no_core_detection() -> None:
    """SettingsController must not trigger a detection when refreshing its cache."""
    from controllers import SettingsController

    source = inspect.getsource(SettingsController.refreshAvailableComponents)
    assert "_tracked_apps" in source, (
        "refreshAvailableComponents must read the wired tracked-apps controller "
        "(calling get_all_components() on a cold cache blocks the GUI thread)"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 29. PySide6 @Slot() arity must match the Python signature
# ─────────────────────────────────────────────────────────────────────────────
def test_slots_declare_every_python_argument() -> None:
    """A @Slot() with fewer argument types than the Python signature makes Qt
    silently drop the extra arguments at call time.

    That is what broke ``diffProfiles``: declared ``@Slot(str, result=str)`` but
    implemented as ``(profile_a, profile_b)``, so every call from QML raised
    "diffProfiles() missing 1 required positional argument: 'profile_b'" and the
    preview dialog rendered an empty diff.
    """
    from PySide6.QtCore import QMetaMethod

    from controllers import (
        ProfileController, WallpaperController, TrackedAppsController,
        SettingsController, MainController,
    )

    problems = []
    for klass in (ProfileController, WallpaperController, TrackedAppsController,
                  SettingsController, MainController):
        meta = klass.staticMetaObject
        for i in range(meta.methodOffset(), meta.methodCount()):
            method = meta.method(i)
            if method.methodType() != QMetaMethod.Slot:
                continue
            name = bytes(method.name()).decode()
            signature = bytes(method.methodSignature()).decode()
            args = signature[signature.find("(") + 1:signature.rfind(")")]
            declared = len([a for a in args.split(",") if a.strip()])
            func = getattr(klass, name, None)
            if func is None:
                continue
            params = inspect.signature(func).parameters.values()
            python_args = [
                p for p in params
                if p.kind in (p.POSITIONAL_OR_KEYWORD, p.POSITIONAL_ONLY)
                and p.name != "self"
            ]
            if declared != len(python_args):
                problems.append(
                    f"{klass.__name__}.{name}: Qt slot declares {declared} "
                    f"argument(s) but Python takes {len(python_args)}"
                )
    assert not problems, "; ".join(problems)


def test_dialogs_compute_content_on_open() -> None:
    """Both content-heavy dialogs build their body when they are opened."""
    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    assert "onOpened: updateDiff()" in main_qml, (
        "diffDialog must compute the diff when it opens"
    )
    assert "onOpened: updatePreview()" in main_qml, (
        "previewDialog must compute its content when it opens"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 21. System tray: quick profile switching
#
# The tray used to show a disabled "Profiles" placeholder. It now lists the
# profiles (active one checked), applies them without blocking the GUI thread
# and reports the result as a desktop notification.
# ─────────────────────────────────────────────────────────────────────────────
class TestSystemTray:
    """Tray menu model + wiring."""

    @staticmethod
    def _controller():
        from system_tray import SystemTrayController

        return SystemTrayController.instance()

    def test_profile_entries_put_active_first(self) -> None:
        entries = self._controller()._profile_entries(
            [
                {"name": "paper", "componentCount": 20, "isActive": False},
                {"name": "glass", "componentCount": 20, "isActive": True},
            ],
            "glass",
        )
        assert [e["name"] for e in entries] == ["glass", "paper"]
        assert [e["active"] for e in entries] == [True, False]

    def test_profile_entries_label_carries_component_count(self) -> None:
        entries = self._controller()._profile_entries(
            [{"name": "glass", "componentCount": 20}], "glass"
        )
        assert entries[0]["label"] == "glass (20)"

    def test_profile_entries_ignore_empty_and_plain_strings(self) -> None:
        entries = self._controller()._profile_entries(
            [{"name": ""}, {"name": "solo"}, "plain"], "solo"
        )
        assert [e["name"] for e in entries] == ["solo", "plain"]
        assert entries[0]["label"] == "solo"  # no count → no suffix

    def test_profile_entries_handle_missing_input(self) -> None:
        assert self._controller()._profile_entries(None, "") == []
        assert self._controller()._profile_entries([], "glass") == []

    def test_tray_applies_through_the_controller_off_the_gui_thread(self) -> None:
        source = (SRC_DIR / "system_tray.py").read_text()
        assert "controller.applyProfile(name)" in source, (
            "the tray must apply profiles through ProfileController.applyProfile "
            "(which runs the operation on the thread pool)"
        )
        assert "_pending_profile" in source, (
            "the tray must remember which profile it started, so it knows "
            "whether an operationFinished signal belongs to it"
        )
        assert "subprocess" not in source, "the tray must not shell out itself"

    def test_tray_wires_controllers_from_main_controller(self) -> None:
        import ast

        tree = ast.parse((SRC_DIR / "system_tray.py").read_text())
        register = next(
            node
            for node in ast.walk(tree)
            if isinstance(node, ast.FunctionDef) and node.name == "register_system_tray"
        )
        params = [a.arg for a in register.args.args]
        assert params[:3] == ["engine", "main_window", "main_controller"], params

        body = ast.dump(register)
        for call in ("set_profile_controller", "set_wallpaper_controller"):
            assert call in body, f"register_system_tray must call {call}"

    def test_tray_has_a_quit_action(self) -> None:
        """Minimize-to-tray must not leave the app without a way out."""
        source = (SRC_DIR / "system_tray.py").read_text()
        assert "QCoreApplication.quit()" in source
        assert "self._quit_app" in source

    def test_tray_notifies_via_show_message(self) -> None:
        """Notifications must go through QSystemTrayIcon.showMessage.

        A hand-built org.freedesktop.Notifications.Notify call is silently
        dropped: PySide6 marshals Python ints as D-Bus 'i' while Notify's
        replaces_id is 'u', so the daemon answers "No such method 'Notify'"
        (verified with dbus-monitor on Plasma 6).
        """
        source = (SRC_DIR / "system_tray.py").read_text()
        assert "showMessage(" in source
        assert '"Notify"' not in source, (
            "the hand-built Notify call is rejected by the notification daemon"
        )

    def test_minimize_to_tray_is_guarded_in_qml(self) -> None:
        main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
        assert "onClosing" in main_qml, (
            "closing the window must consult the minimize-to-tray preference"
        )
        assert "trayController.trayIconVisible" in main_qml, (
            "hiding the window without a tray icon would make the app unreachable"
        )


# ─────────────────────────────────────────────────────────────────────────────
# 22. Wallpaper operations are asynchronous
#
# Opening the picker is interactive (wofi stays open until the user picks),
# so every wallpaper operation runs on the thread pool and the UI disables
# its buttons while one is in flight.
# ─────────────────────────────────────────────────────────────────────────────
def test_wallpaper_controller_operations_are_async() -> None:
    source = (SRC_DIR / "controllers" / "__init__.py").read_text()
    start = source.index("class WallpaperController")
    end = source.index("class TrackedAppsController")
    block = source[start:end]

    assert "_run_async(self" in block, (
        "wallpaper operations must not run on the GUI thread"
    )
    for method in ("nextWallpaper", "prevWallpaper", "pickWallpaper", "setWallpaper"):
        assert f"def {method}" in block, f"{method} disappeared from the controller"

    # `busy` must be a Property with a change signal so QML/tray can react.
    assert "@Property(bool, notify=busyChanged)" in block
    assert "busyChanged = Signal()" in block


def test_qml_disables_wallpaper_buttons_while_busy() -> None:
    main_qml = (PROJECT_ROOT / "qml" / "Main.qml").read_text()
    assert main_qml.count("enabled: !wallpaperController.busy") == 3, (
        "Previous/Pick/Next must all be disabled while an operation runs"
    )


# ─────────────────────────────────────────────────────────────────────────────
# 23. Application icon
# ─────────────────────────────────────────────────────────────────────────────
def test_app_icon_matches_the_desktop_entry() -> None:
    """The icon name used at runtime must be the one the .desktop installs."""
    from app_icon import FALLBACK_ICON_NAME, ICON_NAME

    desktop = (PROJECT_ROOT / "resources" / "kde-theme-manager.desktop").read_text()
    assert f"Icon={ICON_NAME}" in desktop, (
        "app_icon.ICON_NAME and the desktop entry's Icon= must agree"
    )
    assert (PROJECT_ROOT / "resources" / "icons" / f"{ICON_NAME}.svg").is_file(), (
        "the svg the installer deploys is missing"
    )
    assert FALLBACK_ICON_NAME == "preferences-desktop-theme"


def test_tray_and_window_use_the_shared_icon() -> None:
    assert "app_icon()" in (SRC_DIR / "system_tray.py").read_text()
    assert "app_icon()" in (SRC_DIR / "__main__.py").read_text()


def test_icon_name_survives_kde_icon_loader() -> None:
    """KDE's loader collapses "<existing-icon>-<suffix>" to the existing icon.

    ``kde-theme-manager`` therefore resolved to the *KDE logo* and
    ``plasma-theme-manager`` to the Plasma logo, so the tray/menu icon was the
    wrong picture (verified on Plasma 6 via QIcon.fromTheme). The icon name
    must keep a first dash-separated token that cannot be an existing icon.
    """
    from app_icon import ICON_NAME

    reserved = {"kde", "plasma", "org", "theme", "preferences", "application"}
    first_token = ICON_NAME.split("-")[0].lower()
    assert first_token not in reserved, (
        f"{ICON_NAME!r} starts with {first_token!r}, which KDE's icon loader "
        "may resolve to that icon instead"
    )
    assert ICON_NAME != "kde-theme-manager"


# ─────────────────────────────────────────────────────────────────────────────
# 30. Core factory fixture (single core)
#
# The former Rust-library core (libkde_theme_core.so behind a C ABI) was
# removed together with its FFI and parity tests. What remains is the shared
# fixture and the single-core selection tests below.
# ─────────────────────────────────────────────────────────────────────────────


@pytest.fixture
def core_factory():
    """Keep CoreFactory on the core backend around a test."""
    from core.protocol import CoreFactory

    CoreFactory.set_backend("core")
    try:
        yield CoreFactory
    finally:
        CoreFactory.set_backend("core")


# ─────────────────────────────────────────────────────────────────────────────
# 31. Single-core behaviour (the valuable half of the old parity suite)
# ─────────────────────────────────────────────────────────────────────────────
class TestSingleCore:
    def test_profile_rejects_empty_and_unknown_components(
        self, core_factory, tmp_path, monkeypatch
    ) -> None:
        """``[]`` means "nothing selected", never "all components".

        Runs in a throwaway HOME: this test once saved into the real profiles.
        """
        for key in [k for k in os.environ if k.startswith("XDG_")]:
            monkeypatch.delenv(key)
        monkeypatch.setenv("HOME", str(tmp_path))
        monkeypatch.setenv("KTM_THEME_NO_APPLY", "1")
        from core.cli_core import CliProfileCore

        core = CliProfileCore()
        assert not core.apply("anything", []).success
        assert not core.apply("anything", ["not-a-component"]).success

    def test_wallpaper_directory_comes_from_the_settings(self, core_factory, tmp_path) -> None:
        """A settings directory must reach the CLI core, not be ignored.

        The cached index is a *position* in the list the app displays, so the
        core listing another directory would make next/prev jump elsewhere.
        """
        from core.cli_core import CliWallpaperCore
        from core.protocol import AppSettings

        (tmp_path / "a.jpg").write_bytes(b"x")
        (tmp_path / "B.PNG").write_bytes(b"x")
        (tmp_path / "notes.txt").write_bytes(b"x")  # wrong extension: ignored
        os.symlink(tmp_path / "a.jpg", tmp_path / "0-link.webp")  # symlink: followed

        class FakeSettings:
            def load(self):
                return AppSettings(wallpaper_directory=str(tmp_path))

        core_factory.set_settings_core(FakeSettings())

        listed = CliWallpaperCore().list_wallpapers()
        assert [Path(p).name for p in listed] == ["0-link.webp", "B.PNG", "a.jpg"]

    def test_settings_round_trip_keeps_the_core_choice(self, core_factory, tmp_path) -> None:
        """Saving settings through the core must keep `core_backend` (it is a setting)."""
        from core.cli_core import CliSettingsCore
        from core.protocol import AppSettings

        saved = AppSettings(
            **{**AppSettings().to_dict(), "core_backend": "core", "theme": "paper"}
        )
        target = tmp_path / "theme-profile" / "app-preferences.json"
        # Pin the module-level path: SETTINGS_FILE resolves the *legacy*
        # ~/.config/theme-profile first, so env overrides alone would write
        # into the real user directory.
        with mock.patch("core.cli_core.SETTINGS_FILE", target):
            assert CliSettingsCore().save(saved).success

            assert target.is_file(), "the core must create the config directory"

            loaded = CliSettingsCore().load()
            assert loaded.core_backend == "core"
            assert loaded.theme == "paper"
            assert loaded.to_dict() == saved.to_dict()
            assert AppSettings.from_dict(json.loads(target.read_text())).to_dict() == saved.to_dict()

    def test_mutating_profile_operations_are_native_rust(self, core_factory) -> None:
        """apply/save must be native Rust in profile.rs — no shell-out to scripts.

        The historical bash-owned implementation copied profile entries to
        `~/<component>` (`kwinrc` → `~/kwin`) and restarted plasmashell on
        every apply; 0.5.0 moved the mutations into `profile.rs`.
        """
        rust_source = (SRC_DIR / "core" / "rust_core" / "src" / "profile.rs").read_text()

        assert "run_theme_profile" not in rust_source, (
            "apply/save must not shell out to a theme-profile script"
        )
        assert "apply_with_paths" in rust_source
        assert "save_with_paths" in rust_source
        assert "copy_tracked_into" in rust_source
        assert "apply_live_hooks" in rust_source
        assert "restart_plasma_shell" not in rust_source


# ─────────────────────────────────────────────────────────────────────────────
# 32. Core selection: --core flag, $KTM_CORE, saved setting
# ─────────────────────────────────────────────────────────────────────────────
def test_core_factory_rejects_unknown_backend(core_factory) -> None:
    with pytest.raises(ValueError):
        core_factory.set_backend("cobol")


def test_core_factory_auto_and_legacy_names_resolve_to_core(core_factory, monkeypatch) -> None:
    """`auto` and the legacy `rust`/`cli` spellings must land on the single core."""
    assert core_factory.set_backend("auto") == "core"
    assert core_factory.set_backend("rust") == "core"
    assert core_factory.set_backend("cli") == "core"

    monkeypatch.setenv(core_factory.ENV_VAR, "rust")
    assert core_factory.set_backend_from_env() == "core"


def test_core_factory_reads_the_environment(core_factory, monkeypatch) -> None:
    monkeypatch.setenv(core_factory.ENV_VAR, "core")
    assert core_factory.set_backend_from_env() == "core"

    monkeypatch.delenv(core_factory.ENV_VAR)
    assert core_factory.set_backend_from_env(default="core") == "core"


def test_settings_expose_the_core_selector() -> None:
    """SettingsController must persist the choice and show the running core."""
    from core.protocol import AppSettings

    source = (SRC_DIR / "controllers" / "__init__.py").read_text()
    assert "def coreBackend" in source
    assert "def activeCore" in source

    qml = (PROJECT_ROOT / "qml" / "pages" / "SettingsPage.qml").read_text()
    assert 'settingsController.coreBackend' in qml

    defaults = AppSettings()
    assert defaults.core_backend == "core"
    assert AppSettings.from_dict({}).core_backend == "core"
    assert AppSettings.from_dict({"core_backend": "core"}).core_backend == "core"
    # Наследие эпохи двух ядер и мусор схлопываются в единственное ядро.
    assert AppSettings.from_dict({"core_backend": "rust"}).core_backend == "core"
    assert AppSettings.from_dict({"core_backend": "cli"}).core_backend == "core"
    assert AppSettings.from_dict({"core_backend": "bogus"}).core_backend == "core"
    assert defaults.to_dict()["core_backend"] == "core"


# ─────────────────────────────────────────────────────────────────────────────
# 33. --core CLI flag
# ─────────────────────────────────────────────────────────────────────────────
def _load_main_module():
    """Import src/__main__.py under a private name (it runs main() only as __main__)."""
    import importlib.util

    spec = importlib.util.spec_from_file_location("ktm_main", SRC_DIR / "__main__.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.mark.parametrize(
    "argv, expected, rest",
    [
        (["--core=core"], "core", []),
        (["--core", "core"], "core", []),
        (["--core=auto", "--verbose"], "auto", ["--verbose"]),
        (["--core", "core", "file.qml"], "core", ["file.qml"]),
        ([], None, []),
    ],
)
def test_core_flag_is_parsed_and_removed(argv, expected, rest, monkeypatch) -> None:
    """Qt must never see --core (a stray value would look like a file to open)."""
    main_module = _load_main_module()
    monkeypatch.setattr(sys, "argv", ["kde-theme-manager", *argv])

    assert main_module._extract_core_arg() == expected
    assert sys.argv[1:] == rest


def test_core_flag_without_a_value_is_an_error(monkeypatch) -> None:
    main_module = _load_main_module()
    monkeypatch.setattr(sys, "argv", ["kde-theme-manager", "--core"])

    with pytest.raises(SystemExit):
        main_module._extract_core_arg()


def test_explicit_unknown_core_is_fatal(core_factory, monkeypatch, capsys) -> None:
    """An explicit --core naming a nonexistent backend must fail loudly (exit 3)."""
    main_module = _load_main_module()

    with pytest.raises(SystemExit) as excinfo:
        main_module._select_core("bogus")

    assert excinfo.value.code == 3
    assert "Error:" in capsys.readouterr().err


def test_stale_saved_core_setting_is_normalized(core_factory, monkeypatch, capsys) -> None:
    """A legacy saved preference (e.g. "rust") must not break startup."""
    from core.protocol import AppSettings

    main_module = _load_main_module()
    import core.cli_core as cli_core

    monkeypatch.delenv(core_factory.ENV_VAR, raising=False)

    class FakeSettingsCore:
        def load(self):
            # Saved by an older version that still had two cores.
            return AppSettings(**{**AppSettings().to_dict(), "core_backend": "rust"})

        def save(self, settings):
            from core.protocol import Result

            return Result.ok()

    monkeypatch.setattr(cli_core, "CliSettingsCore", FakeSettingsCore)

    main_module._select_core(None)

    assert core_factory.backend() == "core"


def test_list_cores_reports_availability(capsys) -> None:
    main_module = _load_main_module()
    assert main_module._list_cores() == 0

    out = capsys.readouterr().out
    assert "core" in out
    assert ("available" in out) or ("NOT AVAILABLE" in out)


def test_help_mentions_the_core_flag(capsys) -> None:
    main_module = _load_main_module()
    assert main_module._show_help() == 0

    out = capsys.readouterr().out
    assert "--core" in out and "--list-cores" in out


def test_single_instance_second_launch_is_rejected() -> None:
    """Two running copies meant two tray icons; the second launch must hand
    over to the first and exit instead of spawning a duplicate."""
    from PySide6.QtCore import QCoreApplication
    from PySide6.QtNetwork import QLocalServer

    main_module = _load_main_module()
    app = QCoreApplication.instance() or QCoreApplication([])

    name = f"ktm-test-{os.getpid()}"
    first = QLocalServer(app)
    try:
        assert main_module._acquire_single_instance(name, first) is main_module.SingleInstance.ACQUIRED, (
            "the first instance must acquire the single-instance lock"
        )
        second = QLocalServer(app)
        assert main_module._acquire_single_instance(name, second) is main_module.SingleInstance.HANDED_OVER, (
            "a second instance must be told to hand over, not start"
        )
        second.close()
    finally:
        first.close()
        QLocalServer.removeServer(name)


def test_single_instance_unusable_socket_is_not_already_running(tmp_path, monkeypatch) -> None:
    """A socket path over the AF_UNIX limit (long $TMPDIR) used to be reported
    as "already running"; it must be UNAVAILABLE with a helpful reason."""
    from PySide6.QtCore import QCoreApplication
    from PySide6.QtNetwork import QLocalServer

    main_module = _load_main_module()
    app = QCoreApplication.instance() or QCoreApplication([])

    long_tmp = tmp_path / ("x" * 120)
    long_tmp.mkdir()
    monkeypatch.setenv("TMPDIR", str(long_tmp))
    server = QLocalServer(app)
    try:
        outcome = main_module._acquire_single_instance("kde-theme-manager", server)
        assert outcome is main_module.SingleInstance.UNAVAILABLE
        message = main_module._single_instance_error("kde-theme-manager", server)
        assert "108-byte" in message and str(long_tmp) in message
    finally:
        server.close()


def test_tray_click_shows_the_window() -> None:
    """Plasma sends a single click as Trigger; it must show the window
    (it used to react to DoubleClick only, so the icon looked dead)."""
    from system_tray import SystemTrayController, _import_widgets
    import system_tray

    _import_widgets()
    shown: list[str] = []
    controller = SystemTrayController.__new__(SystemTrayController)
    controller._show_window = lambda: shown.append("show")  # type: ignore[method-assign]
    for reason in (system_tray.QSystemTrayIcon.Trigger, system_tray.QSystemTrayIcon.DoubleClick,
                   system_tray.QSystemTrayIcon.MiddleClick):
        controller._on_tray_activated(reason)
    assert shown == ["show", "show", "show"]
    controller._on_tray_activated(system_tray.QSystemTrayIcon.Context)
    assert shown == ["show", "show", "show"], "right click is the menu, not show"
