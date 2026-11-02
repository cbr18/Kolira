"""Tests for the v2 theme bundle core, app adapters, controller-facing data and MCP tools."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = PROJECT_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

REPO_ROOT = PROJECT_ROOT.parents[1]
MCP_DIR = REPO_ROOT / "mcp"
if str(MCP_DIR) not in sys.path:
    sys.path.insert(0, str(MCP_DIR))

from core import CoreFactory  # noqa: E402
from core.app_theme_adapters import (  # noqa: E402
    ADAPTERS,
    BtopAdapter,
    KittyAdapter,
    ObsidianAdapter,
    VSCodeAdapter,
    ZedAdapter,
)
from core.paths import live_path, profile_dir, theme_root  # noqa: E402
from core.protocol import ProfileInfo, Result  # noqa: E402
from core.theme_bundle import ThemeBundleCore  # noqa: E402
from core.theme_core import AppThemeCore  # noqa: E402
from theme_profile_mcp import ThemeProfileMcpServer  # noqa: E402


KDE_GLOBALS = """[General]
ColorScheme=Breeze1
AccentColor=128,142,71
font=Noto Sans,10,-1,5,50,0,0,0,0,0,0,0,0,0,0,1

[Colors:Window]
BackgroundNormal=32,36,31
ForegroundNormal=251,254,243

[KDE]
LookAndFeelPackage=glass
"""

KVANTUM = """[General]
theme=OrchisDark
"""

KVANTUM_THEME = """[%General]
translucent_windows=true
blurring=false
popup_blurring=true
"""


class FakeWallpaperCore:
    def __init__(self) -> None:
        self.current_value = str(Path("/wallpapers/night.jpg"))
        self.applied: list[str] = []

    def pick(self):
        return None

    def next(self) -> Result:
        return Result.ok("next")

    def prev(self) -> Result:
        return Result.ok("prev")

    def current(self):
        return self.current_value

    def list_wallpapers(self):
        return [self.current_value]

    def set_wallpaper(self, path: str) -> Result:
        self.applied.append(path)
        self.current_value = path
        return Result.ok(f"Wallpaper set to {path}")


class FakeProfileCore:
    def __init__(self, home: Path) -> None:
        self.home = home
        self.descriptions: dict[str, str] = {}
        self.saved: list[str] = []
        self.applied: list[str] = []

    def list_profiles(self):
        names = sorted(p.name for p in profile_dir("x").parent.iterdir() if p.is_dir()) if profile_dir("x").parent.exists() else []
        active = self.get_active_profile()
        return [
            ProfileInfo(
                name=name,
                is_active=name == active,
                components=("kdeglobals",),
                created_at="",
                description=self.descriptions.get(name, ""),
            )
            for name in names
        ]

    def get_active_profile(self) -> str:
        try:
            return (theme_root() / "active-profile").read_text(encoding="utf-8").strip()
        except OSError:
            return ""

    def apply(self, profile: str, components=None) -> Result:
        self.applied.append(profile)
        try:
            (theme_root() / "active-profile").write_text(profile + "\n", encoding="utf-8")
        except OSError as exc:
            return Result.err(str(exc))
        return Result.ok(f"Applied profile: {profile}")

    def save(self, name: str, components=None) -> Result:
        target = profile_dir(name) / ".config"
        target.mkdir(parents=True, exist_ok=True)
        for relative in (".config/kdeglobals", ".config/plasmarc"):
            source = live_path(relative)
            if source.is_file():
                (target / Path(relative).name).write_text(
                    source.read_text(encoding="utf-8"), encoding="utf-8"
                )
        self.saved.append(name)
        return Result.ok(f"Saved profile: {name}")

    def delete(self, name: str) -> Result:
        return Result.ok("delete is not used in these tests")

    def clone(self, source: str, target: str) -> Result:
        return Result.ok("clone is not used in these tests")

    def set_description(self, name: str, description: str) -> Result:
        self.descriptions[name] = description
        return Result.ok("Description updated")

    def diff(self, a: str, b: str) -> str:
        return ""

    def get_profile_components(self, profile: str):
        return ("kdeglobals",)


@pytest.fixture()
def bundle_home(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))
    monkeypatch.setenv("XDG_DATA_HOME", str(tmp_path / ".local" / "share"))
    monkeypatch.setenv("XDG_DATA_DIRS", str(tmp_path / "system-share"))
    monkeypatch.setenv("KTM_THEME_NO_APPLY", "1")
    monkeypatch.chdir(tmp_path)

    def put(relative: str, text: str) -> Path:
        path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    put(".config/kdeglobals", KDE_GLOBALS)
    put(".config/kwinrc", """[org.kde.kdecoration2]\nlibrary=org.kde.klassy\ntheme=Klassy\n\n[Effect-better-blur-dx]\nBlurStrength=11\n\n[Effect-translucency]\nInactive=42\n""")
    put(".config/klassy/klassyrc", "[Windeco]\nWindowCornerRadius=10\n")
    put(".config/plasmarc", "[Theme]\nname=Breeze\n")
    put(".config/kcminputrc", "[Mouse]\ncursorTheme=Breeze_Snow\ncursorSize=32\n")
    put(".config/gtk-3.0/settings.ini", "[Settings]\ngtk-theme-name=Fluent-Dark\ngtk-application-prefer-dark-theme=true\n")
    put(".config/Kvantum/kvantum.kvconfig", KVANTUM)
    put(".config/Kvantum/Orchis/OrchisDark.kvconfig", KVANTUM_THEME)

    # Installed assets visible to the theme registry.
    put(".local/share/color-schemes/Breeze1.colors", "[General]\nName=Breeze1\nColorScheme=Breeze1\nAccentColor=128,142,71\n\n[Colors:Window]\nBackgroundNormal=32,36,31\n")
    put(".local/share/color-schemes/Mine.colors", "[General]\nName=Mine\nColorScheme=Mine\n\n[Colors:Window]\nBackgroundNormal=1,2,3\n")
    put(".local/share/plasma/desktoptheme/Breeze/metadata.json", "{}")
    put(".local/share/plasma/desktoptheme/Orchis/metadata.json", "{}")
    put(".local/share/plasma/look-and-feel/glass/contents/defaults", "[kdeglobals][General]\nColorScheme=Breeze1\n")
    put(".local/share/icons/YAMIS/index.theme", "[Icon Theme]\nName=YAMIS\n")
    put(".local/share/icons/Breeze_Snow/cursors", "")
    put(".themes/Fluent-Dark/gtk-3.0/gtk.css", "/* gtk3 */")
    put("system-share/themes/Adwaita/gtk-3.0/gtk.css", "/* gtk3 */")
    put("system-share/themes/Adwaita/gtk-4.0/gtk.css", "/* gtk4 */")

    # Application adapters.
    put(".config/zed/settings.json", """{\n  // comment\n  "theme": {\n    "mode": "dark",\n    "light": "Ayu Light",\n    "dark": "Transparent New Darcula Darker",\n  },\n}\n""")
    put(".config/zed/themes/Local Theme.json", '{"name":"Local Theme","theme":"dark"}')
    put(".vscode/extensions/theme-one/package.json", json.dumps({
        "name": "theme-one",
        "contributes": {"themes": [{"label": "One Dark Pro", "path": "./themes/one.json"}]},
    }))
    put(".config/Code/User/settings.json", '{\n  "workbench.colorTheme": "Default Dark Modern"\n}\n')
    put(".cursor/extensions/theme-cursor/package.json", json.dumps({
        "name": "theme-cursor",
        "contributes": {"themes": [{"label": "Cursor Midnight", "path": "./themes/midnight.json"}]},
    }))
    put(".config/Cursor/User/settings.json", '{\n  "workbench.colorTheme": "Cursor Dark Midnight"\n}\n')
    put("obsidian/vault/.obsidian/appearance.json", '{"theme":"obsidian"}\n')
    put("obsidian/vault/.obsidian/themes/Fancy/manifest.json", '{"name":"Fancy"}\n')
    put("obsidian/vault/.obsidian/themes/Fancy/theme.css", "/* fancy */\n")
    put(".config/btop/btop.conf", 'color_theme = "noctalia"\nrounded_corners = true\n')
    put(".config/btop/themes/noctalia.theme", "# noctalia\n")
    put(".config/btop/themes/nord.theme", "# nord\n")
    put(".config/kitty/kitty.conf", "background_opacity 0.5\ninclude themes/old.conf\n")
    put(".config/kitty/themes/old.conf", "# old\n")
    put(".config/kitty/themes/night.conf", "# night\n")

    theme = AppThemeCore()
    profiles = FakeProfileCore(tmp_path)
    wallpapers = FakeWallpaperCore()
    core = ThemeBundleCore(theme, profiles, wallpapers)
    return {
        "home": tmp_path,
        "theme": theme,
        "profiles": profiles,
        "wallpapers": wallpapers,
        "core": core,
    }


def component(groups: list[dict], component_id: str) -> dict:
    for group in groups:
        for item in group["components"]:
            if item["id"] == component_id:
                return item
    raise AssertionError(f"component not found: {component_id}")


def test_component_registry_uses_installed_theme_names(bundle_home):
    groups = bundle_home["core"].groups("live")

    assert [group["id"] for group in groups] == ["plasma", "desktop", "qt", "gtk"]
    color = component(groups, "color_scheme")
    assert color["currentValue"] == "Breeze1"
    assert {choice["value"] for choice in color["choices"]} >= {"Breeze1", "Mine"}

    plasma = component(groups, "plasma_style")
    assert plasma["currentValue"] == "Breeze"
    assert {choice["value"] for choice in plasma["choices"]} >= {"Breeze", "Orchis"}

    icons = component(groups, "icon_theme")
    assert "YAMIS" in {choice["value"] for choice in icons["choices"]}

    gtk = component(groups, "gtk_theme")
    assert gtk["currentValue"] == "Fluent-Dark"
    assert {choice["value"] for choice in gtk["choices"]} >= {"Fluent-Dark", "Adwaita"}

    kvantum = component(groups, "kvantum_theme")
    assert kvantum["currentValue"] == "OrchisDark"


def test_component_set_and_apply_stages_core_values(bundle_home):
    core = bundle_home["core"]
    assert core.set_component("color_scheme", "Mine").success
    assert core.set_component("corner_radius", 14).success
    assert core.session_changes() == 2
    staged = component(core.groups("live"), "color_scheme")
    assert staged["currentValue"] == "Mine"
    assert staged["staged"] is True

    assert core.apply().success
    assert core.session_changes() == 0
    assert core.undo_depth() == 2
    text = live_path(".config/kdeglobals").read_text(encoding="utf-8")
    assert "ColorScheme=Mine" in text
    assert "WindowCornerRadius=14" in live_path(".config/klassy/klassyrc").read_text(encoding="utf-8")


def test_application_adapters_detect_and_apply_with_backups(bundle_home):
    home = bundle_home["home"]
    adapters = ADAPTERS

    zed = adapters["zed"]
    assert zed.detect()
    assert zed.current_theme()["value"] == "Transparent New Darcula Darker"
    zed_themes = {theme["id"] for theme in zed.installed_themes()["themes"]}
    assert {"Ayu Dark", "Local Theme"} <= zed_themes
    assert zed.apply_theme("Ayu Dark")["ok"]
    assert '"dark": "Ayu Dark"' in (home / ".config/zed/settings.json").read_text(encoding="utf-8")

    vscode = VSCodeAdapter()
    assert vscode.detect()
    vscode_themes = {theme["id"] for theme in vscode.installed_themes()["themes"]}
    assert "One Dark Pro" in vscode_themes
    assert vscode.apply_theme("One Dark Pro")["ok"]
    assert '"workbench.colorTheme": "One Dark Pro"' in (home / ".config/Code/User/settings.json").read_text(encoding="utf-8")

    cursor = adapters["cursor"]
    assert cursor.detect()
    assert "Cursor Midnight" in {theme["id"] for theme in cursor.installed_themes()["themes"]}
    assert cursor.apply_theme("Cursor Midnight")["ok"]

    obsidian = ObsidianAdapter()
    assert obsidian.detect()
    assert obsidian.current_theme()["value"] == "obsidian"
    assert "Fancy" in {theme["id"] for theme in obsidian.installed_themes()["themes"]}
    assert obsidian.apply_theme("Fancy")["ok"]
    appearance = json.loads(
        (home / "obsidian/vault/.obsidian/appearance.json").read_text(encoding="utf-8")
    )
    assert appearance["theme"] == "Fancy"

    btop = BtopAdapter()
    assert btop.detect()
    assert btop.current_theme()["value"] == "noctalia"
    assert "nord" in {theme["id"] for theme in btop.installed_themes()["themes"]}
    assert btop.apply_theme("nord")["ok"]

    kitty = KittyAdapter()
    assert kitty.detect()
    assert kitty.current_theme()["value"] == "old"
    assert "night" in {theme["id"] for theme in kitty.installed_themes()["themes"]}
    assert kitty.apply_theme("night")["ok"]
    kitty_text = (home / ".config/kitty/kitty.conf").read_text(encoding="utf-8")
    assert "include themes/night.conf" in kitty_text
    assert kitty_text.count("include themes/") == 1

    backups = list((theme_root() / "backups").glob("app-theme-*/**/*"))
    assert backups, "adapter writes must create backup copies"


def test_theme_bundle_core_tracks_adapter_changes(bundle_home):
    core = bundle_home["core"]
    result = core.set_component("vscode", "One Dark Pro")
    assert result.success
    assert core.session_changes() == 1
    apps = {app["id"]: app for app in core.applications()}
    assert apps["vscode"]["currentValue"] == "One Dark Pro"
    assert apps["vscode"]["changed"] == "One Dark Pro"
    assert core.apply().success
    assert core.session_changes() == 0


def test_save_profile_writes_v2_bundle_metadata(bundle_home):
    core = bundle_home["core"]
    core.set_component("color_scheme", "Mine")
    core.apply()
    core.set_component("zed", "Ayu Dark")
    core.set_component("wallpaper", "/wallpapers/custom.jpg")

    result = core.save_profile("night", "Dark test bundle")
    assert result.success, result.message
    path = profile_dir("night") / "theme-bundle.json"
    assert path.is_file()
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["schema"] == "theme-bundle/1"
    assert data["name"] == "night"
    assert data["components"]["color_scheme"] == "Mine"
    assert data["components"]["kvantum_theme"] == "OrchisDark"
    assert data["applications"]["zed"] == "Ayu Dark"
    assert data["wallpaper"] == "/wallpapers/custom.jpg"
    assert bundle_home["profiles"].descriptions["night"] == "Dark test bundle"


def test_profile_bundle_falls_back_to_legacy_profile_values(bundle_home):
    core = bundle_home["core"]
    bundle_home["profiles"].save("legacy")
    result = core.profile_bundle("legacy")
    assert result.success, result.message
    assert result.data["legacy"] is True
    assert result.data["components"]["color_scheme"] == "Breeze1"
    assert result.data["components"]["plasma_style"] == "Breeze"


def test_apply_profile_uses_legacy_core_and_bundle_references(bundle_home):
    core = bundle_home["core"]
    core.save_profile("night")
    result = core.apply_profile("night")
    assert result.success, result.message
    assert bundle_home["profiles"].applied == ["night"]
    assert bundle_home["wallpapers"].applied
    assert "Applied profile: night" in result.message
    assert "Applied 12 change(s)" in result.message


def test_validate_profile_reports_missing_references(bundle_home):
    core = bundle_home["core"]
    core.save_profile("night")
    data = json.loads((profile_dir("night") / "theme-bundle.json").read_text(encoding="utf-8"))
    data["components"]["color_scheme"] = "RemovedTheme"
    data["applications"]["zed"] = "Removed Zed Theme"
    (profile_dir("night") / "theme-bundle.json").write_text(
        json.dumps(data, ensure_ascii=False), encoding="utf-8"
    )
    result = core.validate_profile_bundle("night")
    assert not result.success
    assert "RemovedTheme" in result.message
    assert "Removed Zed Theme" in result.message


def test_create_custom_theme_creates_user_owned_assets(bundle_home):
    core = bundle_home["core"]
    result = core.create_custom_theme("MyNight")
    assert result.success, result.message

    color = bundle_home["home"] / ".local/share/color-schemes/MyNight.colors"
    kvantum = bundle_home["home"] / ".local/share/Kvantum/MyNight/MyNight.kvconfig"
    assert color.is_file()
    assert kvantum.is_file()
    assert "Name=MyNight" in color.read_text(encoding="utf-8")
    assert "AccentColor=128,142,71" in color.read_text(encoding="utf-8")
    assert "translucent_windows=true" in kvantum.read_text(encoding="utf-8")

    # System/base assets are not overwritten.
    assert "Name=Breeze1" in (bundle_home["home"] / ".local/share/color-schemes/Breeze1.colors").read_text(encoding="utf-8")
    assert "theme=OrchisDark" in live_path(".config/Kvantum/kvantum.kvconfig").read_text(encoding="utf-8")

    duplicate = core.create_custom_theme("MyNight")
    assert not duplicate.success


def test_groups_for_profile_scope_are_read_only_values(bundle_home):
    core = bundle_home["core"]
    bundle_home["profiles"].save("glass")
    profile_config = profile_dir("glass") / ".config" / "kdeglobals"
    profile_config.write_text(KDE_GLOBALS.replace("ColorScheme=Breeze1", "ColorScheme=Mine"), encoding="utf-8")

    groups = core.groups("profile:glass")
    assert component(groups, "color_scheme")["currentValue"] == "Mine"
    assert component(groups, "color_scheme")["choices"], "profile view still lists installed live choices"


def test_mcp_server_tools_list_and_call(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, bundle_home):
    monkeypatch.setattr(CoreFactory, "_theme_core", bundle_home["theme"])
    monkeypatch.setattr(CoreFactory, "_profile_core", bundle_home["profiles"])
    monkeypatch.setattr(CoreFactory, "_wallpaper_core", bundle_home["wallpapers"])
    server = ThemeProfileMcpServer()

    init = server.handle({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
    assert init["result"]["serverInfo"]["name"] == "theme-profile-kit"

    tools = server.handle({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
    names = {tool["name"] for tool in tools["result"]["tools"]}
    assert {
        "list_components", "list_installed_themes", "list_profiles",
        "get_profile_bundle", "validate_profile", "set_component",
        "apply_changes", "create_custom_theme", "create_profile", "apply_profile",
    } <= names

    listed = server.handle({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {
            "name": "list_installed_themes",
            "arguments": {"componentId": "color_scheme"},
        },
    })
    payload = json.loads(listed["result"]["content"][0]["text"])
    assert "Mine" in {choice["value"] for choice in payload["choices"]}

    created = server.handle({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {
            "name": "create_profile",
            "arguments": {
                "name": "mcpnight",
                "description": "Created through MCP",
                "components": {"color_scheme": "Mine"},
                "applications": {"zed": "Ayu Dark"},
            },
        },
    })
    assert created["result"]["isError"] is False
    payload = json.loads(created["result"]["content"][0]["text"])
    assert payload["ok"] is True
    assert payload["profile"]["components"]["color_scheme"] == "Mine"
    assert (profile_dir("mcpnight") / "theme-bundle.json").is_file()

    validate = server.handle({
        "jsonrpc": "2.0", "id": 5, "method": "tools/call",
        "params": {"name": "validate_profile", "arguments": {"name": "mcpnight"}},
    })
    payload = json.loads(validate["result"]["content"][0]["text"])
    assert payload["valid"] is True, payload

    bad = server.handle({
        "jsonrpc": "2.0", "id": 6, "method": "tools/call",
        "params": {"name": "unknown_tool", "arguments": {}},
    })
    assert bad["result"]["isError"] is True
