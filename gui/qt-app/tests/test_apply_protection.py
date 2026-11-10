"""GUI recovery notifications and failure diagnostics."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))


def test_interrupted_apply_is_reported_once_without_changing_active_profile(qtbot, monkeypatch):
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
    ctrl = ProfileController()
    messages = []
    ctrl.operationFinished.connect(lambda ok, msg: messages.append((ok, msg)))
    ctrl.refresh()
    ctrl.refresh()
    qtbot.waitUntil(lambda: not ctrl.refreshing and not ctrl._summary_running, timeout=5000)
    assert ctrl.activeProfile == 'glass'
    assert len(messages) == 1 and messages[0][0] is False
    assert 'apply-interrupted' in messages[0][1] and '/private/backup' in messages[0][1]


def test_apply_failure_preserves_operation_and_recovery_diagnostics(monkeypatch):
    from core.cli_core import CliProfileCore
    def request(*args, **kwargs):
        raise ValueError('Apply failed: write error; operation apply-error; recovery_failed; backup /private/backup; recovery errors: denied')
    monkeypatch.setattr('core.cli_core.core_request', request)
    result = CliProfileCore().apply('glass')
    assert not result.success
    assert all(word in result.message for word in ('apply-error', 'recovery_failed', '/private/backup', 'denied'))
