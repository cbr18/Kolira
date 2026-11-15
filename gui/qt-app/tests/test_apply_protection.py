"""GUI recovery notifications and failure diagnostics."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))


def test_old_interrupted_apply_does_not_raise_errors_on_refresh_or_restart(qtbot, monkeypatch):
    from controllers import ProfileController
    from core.protocol import CoreFactory, ProfileInfo
    class Core:
        def list_profiles(self):
            return [ProfileInfo(name='glass', is_active=True, components=(), created_at='', description='')]
        def get_active_profile(self):
            return 'glass'
        def get_profile_components(self, name):
            return []
        def pending_operations(self):
            return [{'id': 'apply-interrupted', 'backup': '/private/backup'}]
    monkeypatch.setattr(CoreFactory, '_profile_core', Core())
    # A fresh controller models reopening the app with the same old operation.
    for _ in range(2):
        ctrl = ProfileController()
        messages = []
        ctrl.operationFinished.connect(lambda ok, msg: messages.append((ok, msg)))
        for _ in range(2):
            with qtbot.waitSignal(ctrl.refreshFinished, timeout=5000):
                ctrl.refresh()
            qtbot.waitUntil(lambda: not ctrl._summary_running, timeout=5000)
        assert ctrl.activeProfile == 'glass'
        assert messages == []


def test_apply_failure_preserves_operation_and_recovery_diagnostics(monkeypatch):
    from core.cli_core import CliProfileCore
    def request(*args, **kwargs):
        raise ValueError('Apply failed: write error; operation apply-error; recovery_failed; backup /private/backup; recovery errors: denied')
    monkeypatch.setattr('core.cli_core.core_request', request)
    result = CliProfileCore().apply('glass')
    assert not result.success
    assert all(word in result.message for word in ('apply-error', 'recovery_failed', '/private/backup', 'denied'))
