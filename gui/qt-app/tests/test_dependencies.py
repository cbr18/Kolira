"""The dependency adapter must stream, cancel and recover without blocking Qt."""
import json
from pathlib import Path
import sys

import pytest
from PySide6.QtCore import QTimer

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))
from controllers.dependencies import DependencyController


@pytest.fixture
def cli(tmp_path, monkeypatch):
    executable = tmp_path / "kolira"
    executable.write_text(f"#!{sys.executable}\n" + r'''
import json, pathlib, sys, time
root=pathlib.Path(__file__).parent
sub=sys.argv[2]
with (root/'calls').open('a') as calls: calls.write(sub+'\n')
if sub=='register-app':
    print(json.dumps([{'id':'resource','dependency':{'name':'Demo','kind':'icons'},'installed':(root/'installed').exists()}]))
elif sub=='remember':
    time.sleep(.1)
    (root/'policy').write_text('always')
elif sub=='cancel':
    (root/'cancel').touch()
else:
    print(json.dumps({'event':'started','operation':'1'*24}),flush=True)
    for index in range(15):
        print(json.dumps({'event':'progress','stage':'downloading','message':str(index)}),flush=True)
        if (root/'cancel').exists():
            print(json.dumps({'event':'finished','ok':False,'status':'cancelled'}),flush=True)
            sys.exit(2)
        time.sleep(.02)
    ok=not (root/'fail').exists()
    if ok: (root/'installed').touch()
    print(json.dumps({'event':'item','id':'resource','ok':ok,'message':'Verified' if ok else 'Failed'}),flush=True)
    print(json.dumps({'event':'finished','ok':ok}),flush=True)
    sys.exit(0 if ok else 2)
''')
    executable.chmod(0o755)
    monkeypatch.setenv("KTM_THEME_PROFILE_BIN", str(executable))
    return tmp_path


def test_progress_keeps_qt_responsive_and_refreshes_state(cli, qtbot):
    controller = DependencyController()
    ticks = []
    timer = QTimer()
    timer.setInterval(10)
    timer.timeout.connect(lambda: ticks.append(True))
    timer.start()
    controller.operate(["resource"], "install", False)
    qtbot.waitUntil(lambda: not controller.busy, timeout=5000)
    timer.stop()
    assert len(ticks) >= 5
    assert controller.rows[0]["installed"]
    assert controller.log[0]["ok"]


def test_cancel_stops_current_job_without_applying_or_retry_dialog(cli, qtbot):
    controller = DependencyController()
    applied, retried = [], []
    controller.applyRequested.connect(lambda *args: applied.append(args))
    controller.retryApply.connect(lambda *args: retried.append(args))
    controller.installThemeDependencies("demo", ["resource"], True, False)
    qtbot.waitUntil(lambda: bool(controller._operation), timeout=3000)
    controller.cancel()
    qtbot.waitUntil(lambda: not controller.busy, timeout=5000)
    assert not applied and not retried
    assert not (cli / "installed").exists()


def test_partial_failure_offers_explicit_retry_and_does_not_apply(cli, qtbot):
    (cli / "fail").touch()
    controller = DependencyController()
    applied, retried = [], []
    controller.applyRequested.connect(lambda *args: applied.append(args))
    controller.retryApply.connect(lambda *args: retried.append(args))
    controller.installThemeDependencies("demo", ["resource"], True, True)
    qtbot.waitUntil(lambda: not controller.busy, timeout=5000)
    assert not applied and retried == [("demo", True)]
    assert not controller.log[0]["ok"]


def test_policy_is_saved_before_install_and_apply(cli, qtbot):
    controller = DependencyController()
    applied = []
    controller.applyRequested.connect(lambda *args: applied.append(args))
    controller.rememberChoice("demo", ["resource"], ["store"], True)
    controller.installThemeDependencies("demo", ["resource"], True, False)
    qtbot.waitUntil(lambda: not controller.busy, timeout=5000)
    assert (cli / "calls").read_text().splitlines() == ["remember", "install", "register-app"]
    assert (cli / "policy").read_text() == "always"
    assert applied == [("demo", False)]


def test_missing_cli_releases_busy_for_queries_and_policy_changes(tmp_path, monkeypatch, qtbot):
    monkeypatch.setenv("KTM_THEME_PROFILE_BIN", str(tmp_path / "missing"))
    controller = DependencyController()
    with qtbot.waitSignal(controller.finished, timeout=3000) as done:
        controller.refresh()
    assert not done.args[0] and not controller.busy and controller.message
    controller.rememberChoice("demo", ["resource"], ["store"], True)
    controller.installThemeDependencies("demo", ["resource"], True, False)
    qtbot.waitUntil(lambda: not controller.busy, timeout=3000)
    assert controller.message and controller._pending_choice_action is None
