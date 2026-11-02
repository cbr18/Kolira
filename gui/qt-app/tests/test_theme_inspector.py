"""Theme Inspector tests: schema, readers/writers, undo, controller, QML wiring."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = PROJECT_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from core.theme_schema import FIELDS, FIELD_BY_ID, SECTIONS  # noqa: E402
from core.theme_core import (  # noqa: E402
    AppThemeCore,
    _json_patch,
    _json_read,
    _parse_ini,
    _set_ini_text,
)


KDE_GLOBALS = """[General]
ColorScheme=Breeze1
AccentColor=128,142,71
accentColorFromWallpaper=true
font=Noto Sans,10,-1,5,50,0,0,0,0,0,0,0,0,0,0,1

[Colors:Window]
BackgroundNormal=32,36,31
ForegroundNormal=251,254,243
ForegroundInactive=161,170,167
ForegroundLink=195,216,108

[KDE]
LookAndFeelPackage=glass
contrast=4
"""

KWINRC = """[org.kde.kdecoration2]
library=org.kde.klassy
theme=Klassy
BorderSize=Normal

[Plugins]
better_blur_dxEnabled=true
blurEnabled=false

[Effect-better-blur-dx]
BlurStrength=11
Contrast=80
BlurDecorations=true
"""

KLASSY = """[TitleBarOpacity]
ActiveTitleBarOpacity=50
InactiveTitleBarOpacity=48

[Windeco]
WindowCornerRadius=10

[ShadowStyle]
ShadowSizeActive=ShadowSmall
"""

KITTY = """background_opacity 0.5
background_blur 30
"""

ALACRITTY = """[window]
opacity = 0.5
blur = true

[colors.primary]
background = "#1e1b2e"
"""

ZED = """// Zed settings
{
  "icon_theme": {
    "mode": "light",
    "light": "Zed (Default)",
    "dark": "Zed (Default)",
  },
  "theme": {
    "mode": "dark",
    "light": "Ayu Light",
    "dark": "Transparent New Darcula Darker",
  },
}
"""


@pytest.fixture()
def home(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / ".config"))
    monkeypatch.setenv("KTM_THEME_NO_APPLY", "1")
    monkeypatch.chdir(tmp_path)

    def put(relative: str, text: str) -> Path:
        path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    put(".config/kdeglobals", KDE_GLOBALS)
    put(".config/kwinrc", KWINRC)
    put(".config/klassy/klassyrc", KLASSY)
    put(".config/plasmarc", "[Theme]\nname=default\n")
    put(".config/kcminputrc", "[Mouse]\ncursorSize=32\n")
    put(".config/gtk-3.0/settings.ini",
        "[Settings]\ngtk-theme-name=Fluent-Dark\ngtk-application-prefer-dark-theme=true\n")
    put(".config/Kvantum/kvantum.kvconfig", "[General]\ntheme=OrchisDark\n")
    put(".config/Kvantum/Orchis/OrchisDark.kvconfig",
        "[%General]\ntranslucent_windows=true\nblurring=false\npopup_blurring=true\n")
    put(".config/kitty/kitty.conf", KITTY)
    put(".config/alacritty/alacritty.toml", ALACRITTY)
    put(".config/btop/btop.conf", 'color_theme = "noctalia"\nrounded_corners = true\n')
    put(".config/zed/settings.json", ZED)
    put(".config/Code/User/settings.json",
        '{\n  "workbench.colorTheme": "Default Dark Modern"\n}\n')
    put(".config/Cursor/User/settings.json",
        '{\n  "workbench.colorTheme": "Cursor Dark Midnight"\n}\n')
    put(".zen/test (release)/user.js",
        'user_pref("browser.tabs.allow_transparent_browser", true);\n'
        'user_pref("mod.sameerasw.zen_bg_opacity", "0.55");\n')
    put("obsidian/vault/.obsidian/appearance.json", "{}")
    (tmp_path / ".local/share/plasma/look-and-feel/glass/contents").mkdir(parents=True)

    profile = tmp_path / ".config/theme-profile/profiles/glass/.config"
    profile.mkdir(parents=True, exist_ok=True)
    (profile / "kdeglobals").write_text(KDE_GLOBALS.replace("128,142,71", "10,20,30"))
    (tmp_path / ".config/theme-profile/active-profile").write_text("glass\n")
    return tmp_path


def test_schema_is_consistent() -> None:
    section_ids = {section.id for section in SECTIONS}
    assert section_ids == {"colors", "typography", "windows", "assets", "apps"}
    assert len(FIELDS) >= 50

    ids = [field.id for field in FIELDS]
    assert len(ids) == len(set(ids)), "field ids must be unique"
    for field in FIELDS:
        assert field.section in section_ids, field.id
        assert field.label, field.id
        assert field.type in {"bool", "int", "float", "color", "enum", "font", "str"}, field.id
        assert field.source in {"ini", "kv", "toml", "json", "userjs", "kvantum"}, field.id
        assert field.apply in {
            "config", "ini", "kv", "toml", "json", "userjs", "cmd_config"
        }, field.id
        if field.type == "enum":
            assert field.choices, f"{field.id} has no choices provider"
        if field.type in {"int", "float"}:
            assert field.min is not None and field.max is not None, field.id


def test_reads_live_values(home: Path) -> None:
    core = AppThemeCore()
    assert core.get("accent_color")["value"] == "128,142,71"
    assert core.get("color_scheme")["value"] == "Breeze1"
    assert core.get("look_and_feel")["value"] == "glass"
    assert core.get("kitty_opacity")["value"] == pytest.approx(0.5)
    assert core.get("alacritty_opacity")["value"] == pytest.approx(0.5)
    assert core.get("alacritty_blur")["value"] is True
    assert core.get("btop_theme")["value"] == "noctalia"
    assert core.get("zed_mode")["value"] == "dark"
    assert core.get("vscode_theme")["value"] == "Default Dark Modern"
    assert core.get("cursor_ide_theme")["value"] == "Cursor Dark Midnight"
    assert core.get("zen_opacity")["value"] == pytest.approx(0.55)
    assert core.get("obsidian_theme")["value"] is None
    assert core.get("cursor_theme")["display"] == "System default"
    assert core.get("kvantum_theme")["value"] == "OrchisDark"
    assert core.get("kvantum_blurring")["value"] is False


def test_profile_diff_and_scope(home: Path) -> None:
    core = AppThemeCore()
    live = core.get("accent_color", "live")
    assert live["differs"] is True
    assert live["profileValue"] == "10,20,30"

    snapshot = core.get("accent_color", "profile:glass")
    assert snapshot["value"] == "10,20,30"
    assert snapshot["profileValue"] == "128,142,71"

    sections = core.sections("profile:glass")
    assert [section.id for section in sections] == [
        "colors", "typography", "windows", "assets", "apps"
    ]


def test_write_kde_and_undo(home: Path) -> None:
    core = AppThemeCore()
    result = core.set_value("accent_color", "#ff0000")
    assert result.success, result.message
    assert core.get("accent_color")["value"] == "255,0,0"
    assert core.session_changes() == 1

    result = core.undo()
    assert result.success, result.message
    assert core.get("accent_color")["value"] == "128,142,71"
    assert core.session_changes() == 0


def test_undo_depth_tracks_applied_changes(home: Path) -> None:
    core = AppThemeCore()
    assert core.undo_depth() == 0
    assert core.set_value("accent_color", "#ff0000").success
    assert core.undo_depth() == 0
    assert core.apply().success
    assert core.session_changes() == 0
    assert core.undo_depth() == 1
    assert core.undo().success
    assert core.undo_depth() == 0
    assert core.get("accent_color")["value"] == "128,142,71"


def test_write_jsonc_preserves_comments_and_trailing_commas(home: Path) -> None:
    core = AppThemeCore()
    result = core.set_value("zed_mode", "light")
    assert result.success, result.message

    text = (home / ".config/zed/settings.json").read_text()
    assert "// Zed settings" in text
    assert '"dark": "Transparent New Darcula Darker",' in text
    assert core.get("zed_mode")["value"] == "light"
    # icon_theme.mode must not be touched.
    assert '"icon_theme"' in text
    assert core.get("vscode_theme")["value"] == "Default Dark Modern"


def test_json_patch_skips_comments_and_nested_paths() -> None:
    text = '{\n  // comment\n  "a": {"b": "x",},\n  "c": true,\n}\n'
    assert _json_read(text, ("a", "b")) == "x"
    patched = _json_patch(text, ("a", "b"), "y")
    assert '"b": "y"' in patched
    assert "// comment" in patched
    assert _json_read(patched, ("a", "b")) == "y"


def test_write_toml_section(home: Path) -> None:
    core = AppThemeCore()
    assert core.set_value("alacritty_opacity", 0.65).success
    assert core.apply().success
    text = (home / ".config/alacritty/alacritty.toml").read_text()
    assert "opacity = 0.65" in text
    assert "[window]" in text
    # The colors table and keybindings are untouched.
    assert "[colors.primary]" in text
    assert core.get("alacritty_opacity")["value"] == pytest.approx(0.65)


def test_write_obsidian_inserts_key(home: Path) -> None:
    core = AppThemeCore()
    assert core.set_value("obsidian_theme", "moonstone").success
    assert core.apply().success
    text = (home / "obsidian/vault/.obsidian/appearance.json").read_text()
    assert '"theme": "moonstone"' in text
    assert core.get("obsidian_theme")["value"] == "moonstone"


def test_write_kitty_and_font(home: Path) -> None:
    core = AppThemeCore()
    assert core.set_value("kitty_opacity", 0.7).success
    assert core.apply().success
    assert "background_opacity 0.7" in (home / ".config/kitty/kitty.conf").read_text()
    assert core.get("kitty_opacity")["value"] == pytest.approx(0.7)

    assert core.set_value("ui_font", {"family": "DejaVu Sans", "size": 9}).success
    assert core.apply().success
    assert "font=DejaVu Sans,9," in (home / ".config/kdeglobals").read_text()
    assert core.get("ui_font")["display"].startswith("DejaVu Sans 9")


def test_write_kvantum_theme_file(home: Path) -> None:
    core = AppThemeCore()
    assert core.set_value("kvantum_blurring", True).success
    assert core.apply().success
    text = (home / ".config/Kvantum/Orchis/OrchisDark.kvconfig").read_text()
    assert "blurring=true" in text
    assert core.get("kvantum_blurring")["value"] is True


def test_revert_session(home: Path) -> None:
    core = AppThemeCore()
    original = (home / ".config/kdeglobals").read_text()
    assert core.set_value("accent_color", "#010203").success
    assert core.set_value("kitty_opacity", 0.9).success
    assert core.session_changes() == 2

    assert core.apply().success
    assert core.session_changes() == 0
    # После apply значение перечитано с диска: сырой формат kdeglobals.
    assert core.get("accent_color")["value"] == "1,2,3"
    assert core.get("kitty_opacity")["value"] == pytest.approx(0.9)

    result = core.revert()
    assert result.success, result.message
    assert core.session_changes() == 0
    assert core.get("accent_color")["value"] == "128,142,71"
    assert core.get("kitty_opacity")["value"] == pytest.approx(0.5)
    # The snapshot directory keeps the originals.
    assert (home / ".config/kdeglobals").read_text() == original
    backups = list((home / ".config/theme-profile/backups").glob("inspector-*"))
    assert backups, "revert must keep a backup of the session"


def test_revert_discards_staged_and_restores_applied_changes(home: Path) -> None:
    core = AppThemeCore()
    assert core.set_value("accent_color", "#010203").success
    assert core.apply().success
    assert core.set_value("kitty_opacity", 0.9).success

    result = core.revert()
    assert result.success, result.message
    assert core.session_changes() == 0
    assert core.undo_depth() == 0
    assert core.get("accent_color")["value"] == "128,142,71"
    assert core.get("kitty_opacity")["value"] == pytest.approx(0.5)


def test_validation_flags_broken_values(home: Path) -> None:
    core = AppThemeCore()
    kde = home / ".config/kdeglobals"
    kde.write_text(kde.read_text().replace(
        "LookAndFeelPackage=glass", "LookAndFeelPackage=Usage: kreadconfig6 --help"
    ))
    kv = home / ".config/Kvantum/kvantum.kvconfig"
    kv.write_text("[General]\ntheme=NoSuchTheme\n")

    issues = core.validate()
    by_field = {issue.field_id: issue for issue in issues}
    assert by_field["look_and_feel"].severity == "error"
    assert by_field["kvantum_theme"].severity == "error"
    assert (
        "command output" in by_field["look_and_feel"].message
        or "not installed" in by_field["look_and_feel"].message
    )


def test_blur_conflict_is_reported(home: Path) -> None:
    core = AppThemeCore()
    kwinrc = home / ".config/kwinrc"
    kwinrc.write_text(kwinrc.read_text().replace("blurEnabled=false", "blurEnabled=true"))
    conflicts = [issue for issue in core.validate() if "both enabled" in issue.message]
    assert conflicts and conflicts[0].field_id == "blur_enabled"


def test_search_uses_labels_keys_and_russian_keywords(home: Path) -> None:
    core = AppThemeCore()
    assert {item["id"] for item in core.search("kitty")} >= {"kitty_opacity", "kitty_blur"}
    assert any(item["id"] == "ui_font" for item in core.search("шрифт"))
    assert core.search("") == []


def test_revert_removes_a_file_created_in_the_session(home: Path) -> None:
    core = AppThemeCore()
    kitty = home / ".config/kitty/kitty.conf"
    kitty.unlink()
    assert core.set_value("kitty_opacity", 0.9).success
    assert core.apply().success
    assert kitty.is_file()
    assert core.revert().success
    assert not kitty.exists(), "revert must remove files created during the session"


def test_atomic_write_preserves_private_permissions(home: Path) -> None:
    core = AppThemeCore()
    kdeglobals = home / ".config/kdeglobals"
    kdeglobals.chmod(0o600)
    assert core.set_value("accent_color", "#010203").success
    assert core.apply().success
    assert kdeglobals.stat().st_mode & 0o777 == 0o600


def test_controller_rejects_invalid_profile_scope(home: Path) -> None:
    from core.protocol import CoreFactory
    from controllers.theme_inspector import ThemeInspectorController

    CoreFactory.reset()
    controller = ThemeInspectorController()
    messages: list[tuple[str, bool]] = []
    controller.toast.connect(lambda text, ok: messages.append((text, ok)))
    controller.setScope("profile:../etc")
    assert controller.scope == "live"
    assert messages and messages[-1][1] is False
    assert controller.profileSummary("../etc") == {}


def test_controller_read_only_undo_does_not_touch_live_files(home: Path) -> None:
    from core.protocol import CoreFactory
    from controllers.theme_inspector import ThemeInspectorController

    CoreFactory.reset()
    controller = ThemeInspectorController()
    controller.setScope("profile:glass")
    messages: list[tuple[str, bool]] = []
    controller.toast.connect(lambda text, ok: messages.append((text, ok)))
    before = (home / ".config/kdeglobals").read_text()
    controller.undo()
    controller.revertAll()
    assert len(messages) == 2
    assert all(not ok for _, ok in messages)
    assert (home / ".config/kdeglobals").read_text() == before


def test_controller_profile_scope_is_read_only(home: Path) -> None:
    from core.protocol import CoreFactory
    from controllers.theme_inspector import ThemeInspectorController

    CoreFactory.reset()
    controller = ThemeInspectorController()
    messages: list[tuple[str, bool]] = []
    controller.toast.connect(lambda text, ok: messages.append((text, ok)))

    assert controller.scope == "live"
    assert controller.readOnly is False
    assert controller.activeProfile == "glass"
    assert controller.modifiedCount >= 1
    assert controller.sections

    controller.setScope("profile:glass")
    assert controller.readOnly is True
    assert controller.modifiedCount >= 1

    controller.setBool("accent_from_wallpaper", False)
    assert messages and messages[-1][1] is False
    assert "read-only" in messages[-1][0].lower()


def test_controller_profile_summary(home: Path) -> None:
    """The profile card shows a theme fingerprint instead of identical chips."""
    from core.protocol import CoreFactory
    from controllers.theme_inspector import ThemeInspectorController

    CoreFactory.reset()
    controller = ThemeInspectorController()
    summary = controller.profileSummary("glass")

    assert summary["accent"] == "#0a141e"
    assert summary["scheme"] == "Breeze1"
    assert summary["lookAndFeel"] == "glass"
    assert summary["swatches"][0] == "#20241f"
    assert "#0a141e" in summary["swatches"]
    assert summary["fileCount"] >= 1
    assert summary["sizeBytes"] > 0

    assert controller.profileSummary("") == {}
    # Unknown profiles must not raise; the card falls back to its basic state.
    unknown = controller.profileSummary("no-such-profile")
    assert unknown["sizeBytes"] == 0
    assert unknown["accent"] == ""


def test_qml_wiring() -> None:
    qml = PROJECT_ROOT / "qml"
    page = (qml / "pages" / "ThemePage.qml").read_text()
    components_page = (qml / "pages" / "ThemeComponentsPage.qml").read_text()
    apps_page = (qml / "pages" / "ThemeApplicationsPage.qml").read_text()
    section_page = (qml / "pages" / "SectionPage.qml").read_text()
    row = (qml / "pages" / "ThemeFieldRow.qml").read_text()
    main = (qml / "Main.qml").read_text()

    # Новый главный экран — ThemeBundle, а не сырой инспектор полей.
    assert "themeBundle.groups" in page
    assert "themeBundle.applications" in page
    assert "themeBundle.applyChanges()" in page
    assert "ThemeComponentsPage.qml" in page
    assert "ThemeApplicationsPage.qml" in page

    # Группы открывают выбор установленных тем; fallback-поля живут в SectionPage.
    assert "themeBundle.setComponent" in components_page
    assert "themeBundle.openSystemSettings" in components_page
    assert "themeBundle.launchApp" in apps_page
    assert "themeBundle.openAppThemeSettings" in apps_page
    assert "ThemeSectionFields" in section_page
    assert "themeInspector.applyChanges" in section_page
    assert "ColorDialog" in row
    assert "themeInspector.setBool" in row
    assert "ThemePage.qml" in main
    assert "themeBundle" in main
    assert "onInspectClicked" in main


def test_profile_card_has_inspect_action() -> None:
    text = (PROJECT_ROOT / "qml" / "Components" / "ProfileCardDelegate.qml").read_text()
    assert "signal inspectClicked(string profileName)" in text
    assert "card.inspectClicked" in text
    # Actions are labelled: icon-only buttons were invisible without an icon
    # theme, and the palette icon was not recognizable as "Inspect".
    assert 'text: qsTr("Inspect")' in text
    assert 'qsTr("Apply")' in text
    assert "themeInspector.profileSummary" in text
    # Component chips were identical for every profile and are gone.
    assert "Kirigami.Chip" not in text


def test_ini_helpers_preserve_other_keys() -> None:
    text = "[General]\nA=1\nB=2\n\n[Other]\nC=3\n"
    updated = _set_ini_text(text, "General", "B", "20")
    assert _parse_ini(updated)["General"]["B"] == "20"
    assert _parse_ini(updated)["Other"]["C"] == "3"
    created = _set_ini_text(text, "New", "D", "4")
    assert _parse_ini(created)["New"]["D"] == "4"


def test_heavy_fields_ask_for_confirmation() -> None:
    for field_id in ("look_and_feel", "plasma_theme", "icon_theme"):
        assert FIELD_BY_ID[field_id].confirm is True, field_id


def test_controller_apply_fix_and_export(home: Path) -> None:
    from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer

    from core.protocol import CoreFactory
    from controllers.theme_inspector import ThemeInspectorController

    app = QCoreApplication.instance() or QCoreApplication([])  # noqa: F841
    CoreFactory.reset()

    kde = home / ".config/kdeglobals"
    kde.write_text(kde.read_text().replace(
        "LookAndFeelPackage=glass", "LookAndFeelPackage=Usage: kreadconfig6 --help"
    ))

    controller = ThemeInspectorController()
    assert controller.issueCount >= 1

    messages: list[tuple[str, bool]] = []
    loop = QEventLoop()
    controller.toast.connect(lambda text, ok: (messages.append((text, ok)), loop.quit()))
    QTimer.singleShot(5000, loop.quit)
    controller.applyFix("look_and_feel")
    loop.exec()
    assert messages, "applyFix must finish and report through toast()"
    assert messages[-1][1] is True, messages[-1]
    assert "LookAndFeelPackage=glass" in kde.read_text()

    target = home / "snapshot.json"
    controller.exportJson(str(target))
    data = json.loads(target.read_text())
    assert data["scope"] == "live"
    assert data["activeProfile"] == "glass"
    assert data["sections"]["colors"]["accent_color"]["display"] == "#808e47"


def test_controller_export_accepts_file_url(home: Path) -> None:
    from PySide6.QtCore import QCoreApplication

    from core.protocol import CoreFactory
    from controllers.theme_inspector import ThemeInspectorController

    app = QCoreApplication.instance() or QCoreApplication([])  # noqa: F841
    CoreFactory.reset()
    controller = ThemeInspectorController()

    target = home / "snapshot-url.json"
    controller.exportJson("file://" + str(target))
    assert target.is_file()
