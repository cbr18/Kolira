"""PaletteController / PaletteCore against the freshly built Rust CLI.

Runs in a throwaway HOME with KTM_OFFLINE=1 and KTM_THEME_NO_APPLY=1, so no
network and no live desktop changes.
"""

import os
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))

CLI = os.environ.get("KTM_THEME_PROFILE_BIN", "")
pytestmark = pytest.mark.skipif(
    not CLI or not Path(CLI).is_file(), reason="Rust CLI not built (cargo build --release)"
)


@pytest.fixture
def home(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    for var in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"):
        monkeypatch.delenv(var, raising=False)
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("KTM_OFFLINE", "1")
    monkeypatch.setenv("KTM_THEME_NO_APPLY", "1")
    (tmp_path / ".config/theme-profile").mkdir(parents=True)
    return tmp_path


def _settle() -> None:
    from PySide6.QtCore import QCoreApplication, QThreadPool

    QThreadPool.globalInstance().waitForDone(30000)
    for _ in range(5):
        QCoreApplication.processEvents()


def test_core_roundtrip(home: Path) -> None:
    from core.palette_core import PaletteCore

    core = PaletteCore()
    assert core.show() is None  # no palette yet
    items = core.gallery("nord").data["items"]
    assert [i["id"] for i in items] == ["nord"] and items[0]["origin"] == "bundled"
    assert core.import_gallery("nord").success
    assert core.set_slots({"accent": "#a7c080"}).success
    shown = core.show()
    assert shown["name"] == "Nord" and shown["accent"] == "#a7c080"
    assert shown["kdeScheme"] == "ThemeProfileNord" and shown["warnings"] == []
    assert not core.apply(["gtk4"]).success  # unknown target filtered → nothing selected
    assert core.apply(["kde"]).success
    assert (home / ".local/share/color-schemes/ThemeProfileNord.colors").is_file()


def test_from_colors_uses_imagecolors_shape(home: Path) -> None:
    from core.palette_core import PaletteCore

    core = PaletteCore()
    result = core.from_colors(
        [{"color": "#1f3326", "ratio": 0.6}, {"color": "#8fb573", "ratio": 0.3}],
        name="Wallpaper forest", source="imagecolors:/w.jpg",
    )
    assert result.success, result.message
    shown = core.show()
    assert shown["name"] == "Wallpaper forest" and shown["variant"] == "dark"
    assert shown["source"] == "imagecolors:/w.jpg"


def test_controller_slots(home: Path) -> None:
    from PySide6.QtCore import QCoreApplication

    QCoreApplication.instance() or QCoreApplication([])
    from controllers.palette import PaletteController

    ctrl = PaletteController()
    toasts: list[tuple[str, bool]] = []
    ctrl.toast.connect(lambda m, ok: toasts.append((m, ok)))
    assert ctrl.hasPalette is False

    ctrl.searchGallery("rose", False)
    _settle()
    assert {i["id"] for i in ctrl.galleryItems} == {"rose-pine", "rose-pine-dawn"}

    ctrl.importGallery("rose-pine-dawn")
    _settle()
    assert ctrl.hasPalette and ctrl.palette["variant"] == "light"

    ctrl.setSlot("base08", "#ff0000")
    _settle()
    assert ctrl.palette["base"][8] == "#ff0000"

    ctrl.importGallery("../etc")
    _settle()
    assert toasts and toasts[-1][1] is False  # invalid id reported, not crashed


def test_mcp_palette_and_store_tools(home: Path) -> None:
    import json

    sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "mcp"))
    from theme_profile_mcp import ThemeProfileMcpServer

    server = ThemeProfileMcpServer()

    def call(tool: str, /, **arguments):
        reply = server.handle({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                               "params": {"name": tool, "arguments": arguments}})["result"]
        return reply["isError"], json.loads(reply["content"][0]["text"])

    names = {t["name"] for t in server.handle({"jsonrpc": "2.0", "id": 0, "method": "tools/list"})["result"]["tools"]}
    assert {"palette_show", "palette_gallery", "palette_import", "palette_set", "palette_from_wallpaper",
            "palette_from_colors", "palette_apply", "app_store_search", "app_store_install"} <= names

    assert call("palette_show") == (False, {"palette": None})
    err, out = call("palette_gallery", query="gruvbox")
    assert not err and {i["id"] for i in out["items"]} == {"gruvbox-dark-medium", "gruvbox-light-medium"}

    err, out = call("palette_import", source="gallery:gruvbox-dark-medium")
    assert not err and out["palette"]["name"] == "Gruvbox dark, medium"
    err, out = call("palette_set", slots={"accent": "#b8bb26"})
    assert not err and out["palette"]["accent"] == "#b8bb26"
    err, out = call("palette_from_colors", colors=[{"color": "#1f3326", "ratio": 0.7}, {"color": "#8fb573", "ratio": 0.3}], name="Agent")
    assert not err and out["palette"]["name"] == "Agent"

    err, out = call("palette_apply", targets=["kde"])
    assert not err and any("ThemeProfileAgent" in line for line in out["report"])
    assert call("palette_apply", targets=["gtk4"])[0] is True
    assert call("palette_import", source="gallery:../etc")[0] is True
    # Offline (KTM_OFFLINE=1): the store reports the disabled network as a tool error.
    assert call("app_store_search", appId="btop", query="x")[0] is True
