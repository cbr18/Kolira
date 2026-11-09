"""Asynchronous adapters against real Rust in isolated HOME/XDG."""
import json
from pathlib import Path

from controllers.history_schedule import HistoryScheduleController


def theme(tmp_path):
    path = tmp_path / '.config/kde-theme-profile/themes/probe'
    path.mkdir(parents=True)
    (path / 'theme.json').write_text(json.dumps({'schema':'theme-profile/theme@2','name':'probe'}))


def test_queued_read_requests_and_rule_persistence(qtbot, tmp_path):
    theme(tmp_path)
    controller = HistoryScheduleController()
    controller.refreshHistory()
    controller.refreshSchedule()
    qtbot.waitUntil(lambda: not controller.busy and 'rules' in controller.schedule, timeout=5000)
    assert not controller.message
    rule = dict(id='morning', theme='probe', time='08:00', days=[1,2,3,4,5], enabled=True)
    controller.saveRule(rule)
    qtbot.waitUntil(lambda: not controller.busy, timeout=5000)
    assert controller.schedule['rules'][0]['id'] == 'morning'
    other = HistoryScheduleController()
    other.refreshSchedule()
    qtbot.waitUntil(lambda: not other.busy, timeout=5000)
    assert other.schedule['rules'][0]['time'] == '08:00'
    other.deleteRule('morning')
    qtbot.waitUntil(lambda: not other.busy, timeout=5000)
    assert other.schedule['rules'] == []


def test_missing_executable_finishes_request_with_visible_error(qtbot, monkeypatch, tmp_path):
    monkeypatch.setenv('KTM_THEME_PROFILE_BIN', str(tmp_path/'does-not-exist'))
    controller = HistoryScheduleController()
    controller.refreshHistory()
    qtbot.waitUntil(lambda: not controller.busy, timeout=5000)
    assert controller.message


def test_failed_apply_links_the_error_to_the_actual_history_record(qtbot, tmp_path):
    from core.cli_core import CliProfileCore
    theme(tmp_path)
    broken = tmp_path/'.config/kde-theme-profile/themes/probe/.config/alacritty/alacritty.toml'
    broken.parent.mkdir(parents=True)
    broken.write_text('[invalid toml')
    result = CliProfileCore().apply('probe', skip_dependencies=True)
    assert not result.success
    controller = HistoryScheduleController()
    record_id = controller.operationIdFromMessage(result.message)
    assert record_id
    controller.refreshHistory()
    qtbot.waitUntil(lambda: not controller.busy, timeout=5000)
    assert controller.history[0]['id'] == record_id
    assert controller.operationIdFromMessage('No transaction was started') == ''
