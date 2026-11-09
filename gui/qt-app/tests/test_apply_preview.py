"""Async preview review/confirmation contracts; a stale result never applies twice."""
import sys
from pathlib import Path
from threading import Event
from types import SimpleNamespace
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))
from core.protocol import Result


def make_controller(monkeypatch, **operations):
    from core.protocol import CoreFactory
    from controllers import ProfileController
    core = SimpleNamespace(list_profiles=lambda: [], get_active_profile=lambda: '',
                           get_profile_components=lambda _: [], pending_operations=lambda: [], **operations)
    monkeypatch.setattr(CoreFactory, '_profile_core', core)
    return ProfileController()


def report(name, token='fresh', keep=False):
    return Result.ok(data=dict(name=name, token=token, keep_wallpaper=keep, components=None,
                               files=[], file_count=0, dependencies=[], actions=[]))


def test_preview_is_async_and_keeps_only_the_latest_request(qtbot, monkeypatch):
    gate = Event()
    calls = []
    def preview(name, keep):
        calls.append(name)
        if name == 'slow': gate.wait(5)
        return report(name, keep=keep)
    ctrl = make_controller(monkeypatch, preview=preview)
    ctrl.previewApply('slow', False)
    assert ctrl.previewBusy
    ctrl.previewApply('newest', True)
    gate.set()
    qtbot.waitUntil(lambda: not ctrl.previewBusy)
    assert ctrl.applyPreview['name'] == 'newest'
    assert ctrl.applyPreview['keep_wallpaper'] is True
    assert calls == ['slow', 'newest']


def test_cancelling_ignores_a_late_preview_and_never_applies(qtbot, monkeypatch):
    gate = Event()
    applied = []
    def preview(name, keep):
        gate.wait(5)
        return report(name)
    ctrl = make_controller(monkeypatch, preview=preview, apply=lambda *a, **kw: applied.append(kw))
    ctrl.previewApply('theme', False)
    ctrl.cancelPreview()
    gate.set()
    qtbot.waitUntil(lambda: not ctrl.previewBusy)
    assert ctrl.applyPreview == {} and applied == []


def test_stale_confirmation_reopens_review_without_automatic_retry(qtbot, monkeypatch):
    applied = []
    previews = []
    def preview(name, keep):
        previews.append(name)
        return report(name, token=f'token-{len(previews)}')
    def apply(name, **kwargs):
        applied.append((name, kwargs))
        return Result.err('PREVIEW_STALE: live file changed')
    ctrl = make_controller(monkeypatch, preview=preview, apply=apply)
    ctrl.previewApply('theme', True)
    qtbot.waitUntil(lambda: not ctrl.previewBusy)
    ctrl.confirmPreview('wrong-token')
    assert applied == []
    ctrl.confirmPreview('token-1')
    qtbot.waitUntil(lambda: not ctrl.busy and not ctrl.previewBusy and len(previews) == 2)
    assert len(applied) == 1
    assert applied[0][1]['preview_token'] == 'token-1'
    assert ctrl.applyPreview['token'] == 'token-2'


def test_save_current_invalidates_token_and_recalculates_preview(qtbot, monkeypatch):
    saved = []
    previews = []
    def preview(name, keep):
        previews.append(name)
        return report(name, token=f'token-{len(previews)}')
    def save(name):
        saved.append(name)
        return Result.ok()
    ctrl = make_controller(monkeypatch, preview=preview, save_before_apply=save)
    ctrl.previewApply('target', False)
    qtbot.waitUntil(lambda: not ctrl.previewBusy)
    ctrl.savePreviewCurrent('my-current-look')
    assert 'token' not in ctrl.applyPreview
    qtbot.waitUntil(lambda: not ctrl.busy and not ctrl.previewBusy and len(previews) == 2)
    assert saved == ['my-current-look']
    assert ctrl.applyPreview['name'] == 'target'
    assert ctrl.applyPreview['saved_as'] == 'my-current-look'
