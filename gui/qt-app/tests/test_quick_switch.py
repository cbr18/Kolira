"""Quick Switch: controller state machine, pre-flight and QML smoke test.

The overlay is a separate process, so the controller is tested directly and
the QML is loaded offscreen with the real controller behind it.
"""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = PROJECT_ROOT / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

import quick_switch_controller as qs  # noqa: E402

CARDS = {
    "active": "b",
    "themes": [
        {"name": "a", "accent": "#111111", "swatches": ["#111111"], "active": False,
         "preview": None, "hasPreview": False},
        {"name": "b", "accent": "#22ff88", "swatches": ["#22ff88"], "active": True,
         "preview": {"path": "/tmp/none.png", "revision": "1-1"}, "hasPreview": False},
        {"name": "c", "accent": "#3344ff", "swatches": ["#3344ff"], "active": False},
    ],
}

SETTINGS = {
    "enabled": True,
    "hotkey": "Meta+K",
    "card_shape": "rounded",
    "card_scale": 1.0,
    "dim": 0.72,
    "glow": True,
    "animation_speed": "standard",
    "apply_animation": True,
    "reduced_motion": "system",
}

PREPARED = {
    "name": "b",
    "token": "deadbeef",
    "fileCount": 12,
    "missing": [],
    "dependencies": [],
    "actions": [],
}


@pytest.fixture()
def controller(monkeypatch):
    """A controller whose core calls are answered from fixtures."""
    monkeypatch.setattr(qs, "_detect_system_reduced_motion", lambda: False)

    def fake_request(method, **_kwargs):
        if method == "quick_switch.get":
            return dict(SETTINGS)
        if method == "quick_switch.cards":
            return dict(CARDS)
        if method == "quick_switch.prepare":
            return dict(PREPARED)
        raise AssertionError(f"unexpected core call: {method}")

    monkeypatch.setattr(qs, "core_request", fake_request)
    instance = qs.QuickSwitchController()
    yield instance
    instance.deleteLater()


def test_initialize_selects_the_active_theme(controller) -> None:
    controller.initialize()
    assert controller.phase == "select"
    assert [theme["name"] for theme in controller.themes] == ["a", "b", "c"]
    assert controller.index == 1
    assert controller.current["name"] == "b"
    assert controller.settings["card_shape"] == "rounded"


def test_selection_never_touches_the_system(controller) -> None:
    controller.initialize()
    controller.step(1)
    assert controller.index == 2
    controller.step(1)
    assert controller.index == 0  # wraps
    assert controller.phase == "select"


def test_prepare_blockers_are_shown_instead_of_applying(controller, monkeypatch) -> None:
    def fake_request(method, **_kwargs):
        if method == "quick_switch.get":
            return dict(SETTINGS)
        if method == "quick_switch.cards":
            return dict(CARDS)
        if method == "quick_switch.prepare":
            return {**PREPARED, "dependencies": [{"name": "Papirus"}]}
        raise AssertionError(f"unexpected core call: {method}")

    monkeypatch.setattr(qs, "core_request", fake_request)
    controller.initialize()
    controller.confirm()
    assert controller.phase == "closing"
    controller.beginApply()
    assert controller.phase == "attention"
    assert any("Papirus" in line for line in controller.attention)


def test_blocking_lines_cover_missing_dependencies_and_actions() -> None:
    lines = qs.QuickSwitchController._blocking_lines(
        {
            "missing": [{"field": "icon_theme", "name": "Papirus"}],
            "dependencies": [{"name": "kvantum"}],
            "actions": [{"title": "Restart Zen"}],
        }
    )
    assert len(lines) == 3
    assert "Papirus" in lines[0]
    assert "kvantum" in lines[1]
    assert "Restart Zen" in lines[2]
    assert qs.QuickSwitchController._blocking_lines({}) == []


def test_escape_is_ignored_while_a_transaction_runs(controller) -> None:
    controller.initialize()
    closed = []
    controller.closed.connect(lambda: closed.append(True))
    controller._set_phase(qs.APPLYING)
    controller.dismiss()
    assert closed == []
    controller.watchdog()  # the core stopped answering
    assert "не отвечает" in controller.status
    controller.dismiss()
    assert closed == [True]


def test_preview_mode_never_starts_a_transaction(controller) -> None:
    controller._preview_only = True
    controller.initialize()
    controller.confirm()
    assert controller.phase == "select"


def test_reduced_motion_follows_the_settings(controller) -> None:
    controller.initialize()
    assert controller.reducedMotion is False
    controller._settings = {**SETTINGS, "reduced_motion": "on"}
    assert controller.reducedMotion is True
    controller._settings = {**SETTINGS, "reduced_motion": "off"}
    assert controller.reducedMotion is False


def test_cards_feed_shape_matches_the_core_contract(controller) -> None:
    controller.initialize()
    theme = controller.themes[1]
    assert theme["accent"] == "#22ff88"
    assert theme["swatches"] == ["#22ff88"]
    assert theme["preview"]["revision"] == "1-1"


def test_overlay_qml_loads_with_the_real_controller(controller) -> None:
    """The QML tree must compile; a broken binding is a silent failure."""
    from PySide6.QtCore import QUrl
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtQml import QQmlApplicationEngine

    app = QGuiApplication.instance() or QGuiApplication([])
    engine = QQmlApplicationEngine()
    engine.addImportPath("/usr/lib/qt6/qml")
    engine.rootContext().setContextProperty("quickSwitch", controller)
    controller.initialize()

    path = PROJECT_ROOT / "qml" / "quick-switch" / "QuickSwitchContent.qml"
    component_path = QUrl.fromLocalFile(str(path))
    engine.load(component_path)
    assert engine.rootObjects(), "QuickSwitchContent.qml failed to load"
    assert app is not None


def test_apply_screen_shader_is_shipped() -> None:
    """Qt 6 needs a precompiled shader; the source alone is not enough."""
    shaders = PROJECT_ROOT / "qml" / "quick-switch" / "shaders"
    assert (shaders / "apply.frag").is_file()
    assert (shaders / "apply.qsb").is_file()
    assert (shaders / "apply.qsb").stat().st_size > 0