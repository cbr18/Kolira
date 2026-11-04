"""Rust field engine (`theme-profile field`) and the Python inspector core
must read and write the same keys identically — both use theme_fields.json."""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))

CLI = os.environ.get("KTM_THEME_PROFILE_BIN", "")
pytestmark = pytest.mark.skipif(not CLI or not Path(CLI).is_file(), reason="Rust CLI not built")

FILES = {
    ".config/kdeglobals": "[General]\nColorScheme=Breeze\nAccentColor=61,174,233\n\n[KDE]\nwidgetStyle=Breeze\n",
    ".config/kwinrc": "[Effect-better-blur-dx]\nBlurStrength=5\nBlurMenus=false\n\n[Plugins]\nblurEnabled=false\n",
    ".config/klassy/klassyrc": "[Windeco]\nWindowCornerRadius=4\n",
    ".config/kitty/kitty.conf": "font_size 11\nbackground_opacity 0.5\n",
    ".config/alacritty/alacritty.toml": "[window]\nopacity = 0.5 # keep\n",
    ".config/zed/settings.json": '{\n  // mine\n  "theme": {"mode": "dark", "dark": "One Dark", "light": "One Light"},\n}\n',
    ".config/gtk-3.0/settings.ini": "[Settings]\ngtk-theme-name=Adwaita\ngtk-application-prefer-dark-theme=0\n",
    ".config/btop/btop.conf": 'color_theme = "Default"\nrounded_corners = True\n',
    ".zen/abc.Default (release)/user.js": 'user_pref("zen.view.window.scheme", 0);\n',
}

VALUES = {
    "color_scheme": "Nord",
    "accent_color": "#a7c080",
    "widget_style": "kvantum",
    "blur_strength": 12,
    "blur_menus": True,
    "klassy_radius": 10,
    "kitty_opacity": 0.9,
    "alacritty_opacity": 0.8,
    "zed_dark": "Ayu Dark",
    "gtk_theme": "Breeze",
    "gtk_dark": True,
    "btop_theme": "nord",
    "zen_window_scheme": 1,
}


def _home(root: Path) -> Path:
    for rel, text in FILES.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    return root


def _env(home: Path) -> dict:
    env = {k: v for k, v in os.environ.items() if not k.startswith("XDG_")}
    env.update(HOME=str(home), KTM_THEME_NO_APPLY="1")
    return env


def test_rust_and_python_write_identical_files(tmp_path: Path, monkeypatch) -> None:
    py_home = _home(tmp_path / "py")
    rs_home = _home(tmp_path / "rs")

    # Fields that exist in the registry (the test must not silently skip).
    from core.theme_schema import FIELD_BY_ID
    missing = [k for k in VALUES if k not in FIELD_BY_ID]
    assert not missing, missing

    args = [f"{k}={json.dumps(v) if not isinstance(v, str) else v}" for k, v in VALUES.items()]
    proc = subprocess.run([CLI, "field", "set", *args], env=_env(rs_home), capture_output=True, text=True)
    assert proc.returncode == 0, proc.stderr

    for key in [k for k in os.environ if k.startswith("XDG_")]:
        monkeypatch.delenv(key)
    monkeypatch.setenv("HOME", str(py_home))
    monkeypatch.setenv("KTM_THEME_NO_APPLY", "1")
    from core.theme_core import AppThemeCore
    core = AppThemeCore()
    for key, value in VALUES.items():
        assert core.set_value(key, value).success, key
    assert core.apply().success

    for rel in FILES:
        py = (py_home / rel).read_text(encoding="utf-8")
        rs = (rs_home / rel).read_text(encoding="utf-8")
        assert py == rs, f"{rel} differs:\nPython:\n{py}\nRust:\n{rs}"

    # And Rust reads back what it wrote.
    got = json.loads(subprocess.run([CLI, "field", "get", *VALUES, "--json"], env=_env(rs_home),
                                     capture_output=True, text=True, check=True).stdout)
    assert got["color_scheme"] == "Nord" and got["accent_color"] == "167,192,128"
    assert got["blur_menus"] is True and got["zen_window_scheme"] in (1, "1")
