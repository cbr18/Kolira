"""Profile reads must preserve the last good UI and leave the Qt event loop free."""
import threading

from PySide6.QtCore import QTimer
from controllers import ProfileController
from core.cli_core import CliProfileCore
from core.protocol import ProfileInfo, Result


def refresh(qtbot, controller):
    with qtbot.waitSignal(controller.refreshFinished, timeout=5000) as signal:
        controller.refresh()
    return signal.args


def test_failed_refresh_preserves_cached_profiles_and_active(qtbot, tmp_path, monkeypatch):
    directory = tmp_path / '.config/kolira/themes/probe'
    directory.mkdir(parents=True)
    (directory / 'theme.json').write_text('{"schema":"kolira/theme@2","name":"probe"}')
    marker = directory.parent.parent / 'active-profile'
    marker.write_text('probe\n')
    controller = ProfileController()
    monkeypatch.setattr(controller, '_refresh_summaries', lambda: None)
    assert refresh(qtbot, controller)[0]
    before = controller.profiles
    controller._core = CliProfileCore(str(tmp_path / 'missing-cli'))
    errors = []
    controller.operationFinished.connect(lambda ok, message: errors.append((ok, message)))
    assert refresh(qtbot, controller)[0] is False
    assert controller.profiles == before
    assert controller.activeProfile == 'probe'
    assert marker.read_text() == 'probe\n'
    assert errors and not errors[-1][0] and errors[-1][1]


def test_slow_refresh_does_not_block_qt_and_coalesces_requests(qtbot, monkeypatch):
    gate = threading.Event()
    started = threading.Event()
    class Core:
        calls = 0
        def list_profiles(self):
            self.calls += 1
            started.set()
            assert gate.wait(3)
            return [ProfileInfo('probe', True)]
        def get_active_profile(self): return 'probe'
        def pending_operations(self): return []
    controller = ProfileController()
    controller._core = core = Core()
    monkeypatch.setattr(controller, '_refresh_summaries', lambda: None)
    ticks = []
    try:
        controller.refresh()
        qtbot.waitUntil(started.is_set, timeout=2000)
        controller.refresh()
        controller.refresh()
        QTimer.singleShot(0, lambda: ticks.append(True))
        qtbot.waitUntil(lambda: bool(ticks), timeout=1000)
        assert controller.refreshing
    finally:
        gate.set()
    qtbot.waitUntil(lambda: not controller.refreshing, timeout=5000)
    assert core.calls == 2
    assert controller.activeProfile == 'probe'


def test_refused_stale_marker_clear_does_not_blank_active(qtbot, monkeypatch):
    class Core:
        reads = 0
        def list_profiles(self):
            self.reads += 1
            return [] if self.reads == 1 else [ProfileInfo('probe', True)]
        def get_active_profile(self): return 'probe'
        def clear_stale_active(self, name): return Result.ok(data={'cleared': False})
    controller = ProfileController()
    controller._core = Core()
    monkeypatch.setattr(controller, '_refresh_summaries', lambda: None)
    assert refresh(qtbot, controller)[0]
    assert controller.activeProfile == 'probe'
    assert controller.profiles[0]['name'] == 'probe'
