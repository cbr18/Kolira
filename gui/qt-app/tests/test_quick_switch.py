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


def test_blocking_lines_cover_missing_and_dependencies_only() -> None:
    lines = qs.QuickSwitchController._blocking_lines(
        {
            "missing": [{"field": "icon_theme", "name": "Papirus"}],
            "dependencies": [{"name": "kvantum"}],
            "actions": [{"title": "It is recommended to restart these programs."}],
        }
    )
    # Restart prompts never block: Quick Switch does not restart applications.
    assert lines == ["Нет ресурса: Papirus (icon_theme)", "Зависимость: kvantum"]
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


def test_overlay_runs_the_closing_animation_and_reaches_apply(monkeypatch, tmp_path) -> None:
    """The whole Enter path: closing animation → transaction → reveal.

    Guards the QML id trap (`root.closeAnimation` is undefined, so the
    animation never starts and the overlay just sits there).
    """
    from PySide6.QtCore import QTimer, QUrl
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtQml import QQmlApplicationEngine, QQmlComponent

    def fake_request(method, **_kwargs):
        if method == "quick_switch.get":
            return dict(SETTINGS)
        if method == "quick_switch.cards":
            return dict(CARDS)
        if method == "quick_switch.prepare":
            return dict(PREPARED)
        raise AssertionError(f"unexpected core call: {method}")

    stub = tmp_path / "kolira"
    stub.write_text(
        "#!/bin/sh\n"
        'case "$1 $2" in\n'
        '  "core quick_switch.get") echo \'{"api":1,"ok":true,"data":%s}\' ;;\n'
        '  "core quick_switch.cards") echo \'{"api":1,"ok":true,"data":%s}\' ;;\n'
        '  "core quick_switch.prepare") echo \'{"api":1,"ok":true,"data":%s}\' ;;\n'
        '  "core operation.list") echo \'{"api":1,"ok":true,"data":{"busy":true,'
        '"operations":[{"profile":"b","state":"applying"}]}}\' ;;\n'
        "  apply*) "
        'echo \'{"operation_id":"stub","applied":[],"errors":[],"backup":"/tmp/b",'
        '"dependencies":[],"actions":[]}\' ;;\n'
        "  *) echo '{\"api\":1,\"ok\":false,\"error\":\"stub\"}'; exit 1 ;;\n"
        "esac\n"
        % (
            __import__("json").dumps(SETTINGS),
            __import__("json").dumps(CARDS),
            __import__("json").dumps(PREPARED),
        )
    )
    stub.chmod(0o755)

    monkeypatch.setattr(qs, "_detect_system_reduced_motion", lambda: False)
    monkeypatch.setattr(qs, "core_request", fake_request)
    monkeypatch.setattr(qs, "resolve_cli_bin", lambda *args, **kwargs: str(stub))
    monkeypatch.setenv("KTM_THEME_PROFILE_BIN", str(stub))

    app = QGuiApplication.instance() or QGuiApplication([])
    engine = QQmlApplicationEngine()
    engine.addImportPath("/usr/lib/qt6/qml")
    controller = qs.QuickSwitchController()
    engine.rootContext().setContextProperty("quickSwitch", controller)
    component = QQmlComponent(
        engine,
        QUrl.fromLocalFile(
            str(PROJECT_ROOT / "qml" / "quick-switch" / "HostSurface.qml")
        ),
    )
    surface = component.createWithInitialProperties(
        {"screenIndex": 0, "primary": True, "interactive": True}
    )
    assert surface is not None

    controller.initialize()
    seen: list[str] = []
    controller.phaseChanged.connect(lambda: seen.append(controller.phase))

    QTimer.singleShot(800, controller.confirm)
    QTimer.singleShot(6000, app.quit)
    app.exec()

    assert "closing" in seen, f"the closing animation never ran: {seen}"
    assert "applying" in seen, f"the transaction never started: {seen}"
    assert "revealing" in seen, f"the reveal never started: {seen}"


def test_apply_screen_shader_is_shipped() -> None:
    """Qt 6 needs a precompiled shader; the source alone is not enough."""
    shaders = PROJECT_ROOT / "qml" / "quick-switch" / "shaders"
    assert (shaders / "apply.frag").is_file()
    assert (shaders / "apply.qsb").is_file()
    assert (shaders / "apply.qsb").stat().st_size > 0


def test_appearance_is_busy_reflects_the_core_probe(monkeypatch) -> None:
    monkeypatch.setattr(qs, "core_request", lambda method, **_kw: {"busy": True})
    assert qs.appearance_is_busy() is True
    monkeypatch.setattr(qs, "core_request", lambda method, **_kw: {"busy": False})
    assert qs.appearance_is_busy() is False


def test_appearance_is_busy_fails_open_when_the_core_is_unreachable(monkeypatch) -> None:
    def boom(method, **_kwargs):
        raise RuntimeError("core is gone")

    monkeypatch.setattr(qs, "core_request", boom)
    # A broken probe must never stop the overlay from opening.
    assert qs.appearance_is_busy() is False


class _FakeProcess:
    """Stand-in for QProcess in _apply_finished (only the read methods are used)."""

    def __init__(self, out: bytes, err: bytes) -> None:
        self._out = out
        self._err = err

    def readAllStandardOutput(self) -> bytes:
        return self._out

    def readAllStandardError(self) -> bytes:
        return self._err

    def program(self) -> str:
        return "kolira"

    def errorString(self) -> str:
        return ""


def test_stale_preview_retries_once_then_reveals(controller, monkeypatch) -> None:
    """A PREVIEW_STALE token must re-prepare and retry, not fail the switch."""
    from PySide6.QtCore import QProcess

    controller.initialize()
    controller._pending_name = "b"
    controller._token = "old-token"
    monkeypatch.setattr(controller, "_start_apply", lambda name: None)

    stale = _FakeProcess(b"", b"PREVIEW_STALE: theme or live settings changed")
    controller._apply_finished(1, QProcess.NormalExit, stale)
    assert controller.phase == qs.PREPARING
    assert controller._apply_retries == 1

    # The retried apply (fresh token) succeeds and reveals the desktop.
    ok = _FakeProcess(
        __import__("json").dumps({"operation_id": "x", "applied": [], "errors": []}).encode(),
        b"",
    )
    controller._apply_finished(0, QProcess.NormalExit, ok)
    assert controller.phase == qs.REVEALING


def test_stale_preview_fails_after_the_single_retry(controller, monkeypatch) -> None:
    from PySide6.QtCore import QProcess

    controller.initialize()
    controller._pending_name = "b"
    controller._token = "old-token"
    controller._apply_retries = 1  # already retried once
    monkeypatch.setattr(controller, "_start_apply", lambda name: None)

    stale = _FakeProcess(b"", b"PREVIEW_STALE: still stale")
    controller._apply_finished(1, QProcess.NormalExit, stale)
    assert controller.phase == qs.ERROR
    assert "PREVIEW_STALE" in controller.message