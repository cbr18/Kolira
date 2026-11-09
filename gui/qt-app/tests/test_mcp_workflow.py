"""Real Rust/MCP round trips from a theme workspace, with isolated HOME/XDG."""
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'mcp'))
from kolira_profile_mcp import KoliraMcpServer


def call(server, tool, **arguments):
    response = server.handle({'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call',
                              'params': {'name': tool, 'arguments': arguments}})
    payload = json.loads(response['result']['content'][0]['text'])
    return response['result']['isError'], payload


def test_offline_theme_preview_apply_history_and_restore(tmp_path):
    config = tmp_path / '.config'
    config.mkdir()
    live = config / 'kdeglobals'
    live.write_text('[KDE]\nwidgetStyle=Old\n[General]\nBrowserApplication=personal\n')
    before = live.read_bytes()
    server = KoliraMcpServer()
    error, capabilities = call(server, 'get_capabilities')
    assert not error and capabilities['api'] == 1
    error, schema = call(server, 'get_theme_schema')
    assert not error and any(f['id'] == 'widget_style' for f in schema['fields'])
    error, prepared = call(server, 'prepare_theme', name='offline', values={'widget_style': 'Breeze'})
    assert not error, prepared
    assert prepared['live_appearance_changed'] is False
    assert live.read_bytes() == before
    error, preview = call(server, 'preview_profile', name='offline')
    assert not error, preview
    assert live.read_bytes() == before
    error, applied = call(server, 'apply_profile', name='offline', previewToken=preview['previewToken'])
    assert not error, applied
    error, history = call(server, 'list_operations')
    assert not error
    record = history['operations'][0]
    assert record['source'] == 'mcp'
    assert record['id'] == applied['operation_id']
    error, result = call(server, 'restore_operation', id=record['id'], confirmed=False)
    assert error
    error, result = call(server, 'restore_operation', id=record['id'], confirmed=True)
    assert not error, result
    assert result['restored_operation'] == record['id']
    assert live.read_bytes() == before


@pytest.mark.parametrize('arguments', [
    {'name': '../escape', 'values': {}},
    {'name': 'bad', 'values': {'unknown': 'x'}},
    {'name': 'bad', 'values': [], 'use_palette': False},
    {'name': 'bad', 'values': {}, 'use_palette': 'false'},
])
def test_invalid_offline_requests_have_structured_errors_and_create_no_theme(tmp_path, arguments):
    server = KoliraMcpServer()
    error, result = call(server, 'prepare_theme', **arguments)
    assert error and result['error']['code'] == 'invalid_arguments'
    assert result['error']['next_action']
    assert not (tmp_path / '.config/kolira/themes/bad').exists()


def test_installed_mcp_stdio_from_nested_theme_cwd_survives_invalid_requests(tmp_path):
    installed = tmp_path / 'installed'
    installed.mkdir()
    script = installed / 'kolira_profile_mcp.py'
    script.write_bytes((ROOT / 'mcp/kolira_profile_mcp.py').read_bytes())
    (installed / 'src').symlink_to(ROOT / 'gui/qt-app/src', target_is_directory=True)
    cwd = tmp_path / '.config/kolira/themes/nested'
    cwd.mkdir(parents=True)
    requests = [
        [],
        {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize'},
        {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'},
        {'jsonrpc': '2.0', 'id': 3, 'method': 'tools/call', 'params': {'name': 'prepare_theme', 'arguments': {'name': 'from-cwd', 'values': {'widget_style': 'Breeze'}}}},
        {'jsonrpc': '2.0', 'id': 4, 'method': 'tools/call', 'params': {'name': 'validate_profile', 'arguments': {'name': 'from-cwd'}}},
    ]
    proc = subprocess.run([sys.executable, str(script)], cwd=cwd, input='\n'.join(json.dumps(v) for v in requests)+'\n',
                          text=True, capture_output=True, timeout=30, env=os.environ.copy())
    assert proc.returncode == 0, proc.stderr
    responses = [json.loads(line) for line in proc.stdout.splitlines()]
    assert len(responses) == len(requests)
    assert responses[0]['error']['code'] == -32600
    assert responses[1]['result']['serverInfo']['version'] == '0.6.0'
    assert any(t['name'] == 'prepare_theme' for t in responses[2]['result']['tools'])
    assert not responses[3]['result']['isError'], responses[3]
    assert not responses[4]['result']['isError'], responses[4]
    assert (cwd.parent / 'from-cwd/theme.json').is_file()


def test_missing_cli_has_actionable_error(tmp_path, monkeypatch):
    server = KoliraMcpServer()
    monkeypatch.setenv('KTM_THEME_PROFILE_BIN', str(tmp_path / 'missing-binary'))
    error, result = call(server, 'get_capabilities')
    assert error and result['error']['code'] == 'core_unavailable'
    assert result['error']['message'] and result['error']['next_action']
