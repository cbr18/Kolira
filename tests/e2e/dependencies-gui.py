#!/usr/bin/env python3
"""Render the dependency page without changing a live theme or deployed app.

Run with the app's Python: dependencies-gui.py OUTPUT.png [--large-font]
Uses offscreen Qt by default; set QT_QPA_PLATFORM to exercise a real display.
"""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
output = Path(sys.argv[1]).resolve()
large = "--large-font" in sys.argv
work = Path(tempfile.mkdtemp(prefix="ktm-dependency-gui-"))
display_runtime = os.environ.get("XDG_RUNTIME_DIR") if os.environ.get("QT_QPA_PLATFORM", "offscreen") != "offscreen" else None
for key, rel in [("HOME", "home"), ("XDG_CONFIG_HOME", "config"),
                 ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache"), ("XDG_RUNTIME_DIR", "runtime")]:
    path = work / rel
    path.mkdir(mode=0o700)
    os.environ[key] = str(path)
if display_runtime:
    os.environ["XDG_RUNTIME_DIR"] = display_runtime
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
os.environ.update(QT_QUICK_BACKEND="software", KTM_OFFLINE="1", KTM_THEME_NO_APPLY="1",
                  LANG="ru_RU.UTF-8", LC_ALL="ru_RU.UTF-8", LANGUAGE="ru_RU",
                  KTM_OPEN_THEME="1", KTM_THEME_PAGE="dependencies",
                  KTM_THEME_PROFILE_BIN=str(root / "gui/qt-app/src/core/rust_core/target/release/kolira"))
data = work / "config/kolira"
data.mkdir()
(data / "app-preferences.json").write_text(json.dumps({"automation_configured": True}))
dependencies = [
    dict(kind="icons", name="Example Icons", source="kde-store", id="example", exact=True),
    dict(kind="zed", name="Example Dark", source="zed", id="example", exact=True),
]
for name in ["first", "second"]:
    theme = data / "themes" / name
    theme.mkdir(parents=True)
    (theme / "theme.json").write_text(json.dumps({"schema": "kolira/theme@2", "name": name, "dependencies": dependencies}))
sys.path.insert(0, str(root / "gui/qt-app/src"))
spec = importlib.util.spec_from_file_location("kolira_entry", root / "gui/qt-app/src/__main__.py")
entry = importlib.util.module_from_spec(spec)
spec.loader.exec_module(entry)
entry._acquire_single_instance = lambda *args: entry.SingleInstance.ACQUIRED
palette = entry._setup_palette


def setup_palette(app):
    palette(app)
    if large:
        font = app.font()
        font.setPointSizeF(font.pointSizeF() * 1.4)
        app.setFont(font)


def screenshot(app, window):
    from PySide6.QtCore import QObject, QTimer
    window.resize(1100 if large else 820, 800 if large else 680)

    def grab():
        page = window.findChild(QObject, "dependenciesPage")
        assert page is not None, "dependency page was not loaded"
        assert page.findChild(QObject, "selectAllDependencies") is not None
        button = page.findChild(QObject, "deleteSelectedDependencies")
        assert button is not None and not button.property("enabled")
        assert window.grabWindow().save(str(output)), "screenshot failed"
        print(f"PASS dependency GUI: {output}; large font={large}")
        app.quit()

    QTimer.singleShot(2500, grab)
    QTimer.singleShot(15000, lambda: app.exit(2))


entry._setup_palette = setup_palette
entry._maybe_setup_screenshot = screenshot
sys.argv[:] = ["kolira"]
raise SystemExit(entry.main())
