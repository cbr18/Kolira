#!/usr/bin/env python3
"""Check a cached colour extension with real Cursor in a disposable HOME."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    binary = Path(__file__).resolve().parents[2] / 'gui/qt-app/src/core/rust_core/target/debug/kolira'
    cursor = shutil.which('cursor')
    if not cursor:
        raise RuntimeError('Cursor is required for this native integration check')
    with tempfile.TemporaryDirectory(prefix='kolira-editor-') as folder:
        home = Path(folder)
        env = dict(os.environ, HOME=folder, XDG_CONFIG_HOME=str(home / '.config'),
                   XDG_DATA_HOME=str(home / '.local/share'),
                   XDG_CACHE_HOME=str(home / '.cache'),
                   KTM_THEME_NO_APPLY='1', KTM_OFFLINE='1')
        root = home / '.config/kolira/themes/probe'
        relative = 'assets/editor-themes/cursor/example.night'
        cache = root / relative
        (cache / 'themes').mkdir(parents=True)
        (home / '.cursor/extensions').mkdir(parents=True)
        manifest = dict(
            publisher='example', name='night', version='1.0.0',
            engines={'vscode': '^1.70.0'}, categories=['Themes'],
            contributes={'themes': [dict(label='Night', uiTheme='vs-dark',
                                         path='themes/night.json')]})
        (cache / 'package.json').write_text(json.dumps(manifest))
        (cache / 'themes/night.json').write_text(json.dumps(dict(
            name='Night', type='dark',
            colors={'editor.background': '#202020', 'editor.foreground': '#ffffff'})))
        (root / 'theme.json').write_text(json.dumps(dict(
            schema='kolira/theme@2', name='probe', values={'cursor_ide_theme': 'Night'},
            editor_themes=[dict(editor='cursor', store='local', id='example.night',
                                local_path=relative, native_watch=True)])))

        def rpc(method, value):
            out = subprocess.run([str(binary), 'core', method], input=json.dumps(value),
                                 capture_output=True, text=True, env=env, check=True)
            result = json.loads(out.stdout)
            assert result['ok'], result
            return result['data']

        preview = rpc('profile.preview', {'name': 'probe'})
        rpc('profile.apply', dict(name='probe', preview_token=preview['token'],
                                  skip_dependencies=True))
        out = subprocess.run([
            cursor, '--list-extensions', '--show-versions', '--extensions-dir',
            str(home / '.cursor/extensions'), '--user-data-dir', str(home / '.config/Cursor')],
            capture_output=True, text=True, env=env, timeout=45)
        print('exit', out.returncode)
        print(out.stdout)
        print(out.stderr[-2000:])
        assert out.returncode == 0
        assert 'kolira-cache.example-night@1.0.0' in out.stdout
        print('Native Cursor recognized the isolated cached colour extension')


if __name__ == '__main__':
    main()
