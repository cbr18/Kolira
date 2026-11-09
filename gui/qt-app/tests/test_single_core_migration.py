"""Boundary contracts: Python renders; the Rust process owns data and writes."""
import ast
import json
import subprocess
import sys
from pathlib import Path

import pytest
from PySide6.QtCore import QUrl
from PySide6.QtGui import QColor

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from core.cli_core import core_request
from core.theme_core import AppThemeCore


def test_qt_values_cross_json_boundary(tmp_path):
    path = tmp_path / 'report.json'
    color = QColor('#123456')
    assert core_request('report.export', file=QUrl.fromLocalFile(str(path)),
                        data={'color': color}) is not None
    assert json.loads(path.read_text()) == {'color': '#123456'}


def test_incompatible_core_has_clear_error_and_no_fallback(monkeypatch):
    calls = []
    def run(args, **kwargs):
        calls.append(args)
        return subprocess.CompletedProcess(args, 0, '{"api":2,"ok":true,"data":{}}', '')
    monkeypatch.setattr('core.cli_core.subprocess.run', run)
    with pytest.raises(RuntimeError, match='update the CLI and GUI together'):
        core_request('settings.get')
    assert len(calls) == 1


def test_late_reply_does_not_replace_newer_staging(monkeypatch):
    view = AppThemeCore()
    revision = view._revision + 2
    state = dict(session=view._session, revision=revision, apps={},
                 staged={'window_bg': '#112233'}, configs=[], undoDepth=1,
                 ok=True, message='')
    replies = iter([state, {**state, 'revision': revision - 1, 'staged': {}, 'undoDepth': 0}])
    with monkeypatch.context() as patch:
        patch.setattr('core.cli_core.core_request', lambda *a, **kw: next(replies))
        view._session_call('state')
        view._session_call('state')
    assert view._staged == {'window_bg': '#112233'}
    assert view._undo_count == 1
    view.close()


def test_gui_component_aliases_match_rust_contract():
    from core.theme_bundle import GROUPS
    gui = {c.id: {'field': c.field_id, 'choice': c.kind == 'choice'}
           for group in GROUPS for c in group.components}
    assert core_request('bundle.components') == gui


def test_app_metadata_has_one_rust_source():
    from core.app_theme_adapters import ADAPTERS
    for adapter in ADAPTERS.values():
        metadata = core_request('app.metadata', id=adapter.id)
        assert adapter.name == metadata['name']


def test_production_python_has_no_file_mutators():
    source = Path(__file__).resolve().parents[1] / 'src'
    forbidden = {'write_text', 'write_bytes', 'unlink', 'mkdir', 'rmtree',
                 'copyfile', 'copytree', 'move', 'rename', 'replace'}
    offenders = []
    for path in source.rglob('*.py'):
        tree = ast.parse(path.read_text())
        for node in ast.walk(tree):
            if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Attribute):
                continue
            # str.replace and dataclasses.replace are presentation utilities.
            if node.func.attr in forbidden - {'replace'}:
                offenders.append(f'{path.relative_to(source)}:{node.lineno}:{node.func.attr}')
    assert not offenders
