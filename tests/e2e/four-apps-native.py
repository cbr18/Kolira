#!/usr/bin/env python3
"""Native palette parsers in disposable Docker. No host apps/configs accessed."""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

REPO = Path('/src')
assert Path('/.dockerenv').is_file() and os.environ.get('KOLIRA_FOUR_NATIVE_CONTAINER') == '1', 'Disposable Docker only'
CLI = REPO / 'gui/qt-app/src/core/rust_core/target/debug/kolira'
for source in sorted((REPO / 'themes').iterdir()):
    if not (source / 'theme.json').is_file():
        continue
    with tempfile.TemporaryDirectory(prefix='kolira-four-native-') as tmp:
        root = Path(tmp)
        env = dict(os.environ, HOME=tmp, XDG_CONFIG_HOME=tmp+'/.config',
                   XDG_DATA_HOME=tmp+'/.local/share', XDG_CACHE_HOME=tmp+'/.cache',
                   STARSHIP_CONFIG=tmp+'/.config/starship.toml', TERM='xterm-256color',
                   STARSHIP_SHELL='fish', FORCE_COLOR='1', CLICOLOR_FORCE='1',
                   KTM_OFFLINE='1', KTM_THEME_NO_APPLY='1', QT_QPA_PLATFORM='offscreen')
        for key in ('NO_COLOR', 'TMUX', 'WEZTERM_CONFIG_FILE', 'WEZTERM_CONFIG_DIR'):
            env.pop(key, None)
        def run(args, **kwargs):
            result = subprocess.run([str(a) for a in args], env=kwargs.pop("native_env", env), cwd=root, capture_output=True, timeout=45, **kwargs)
            assert result.returncode == 0, (args, result.stdout[-2000:], result.stderr[-4000:])
            return result
        def write(rel, text):
            p=root/rel;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
        write('.config/starship.toml', "format='$character'\n[character]\nsuccess_symbol='[ok](green)'\n")
        write('.config/foot/foot.ini', 'font=monospace:size=13\n[colors-dark]\nalpha=0.8\n')
        write('.config/wezterm/wezterm.lua', 'local wezterm = require("wezterm"); local config = wezterm.config_builder(); config.font_size=13; config.enable_tab_bar=false; config.colors={indexed={[255]="#123456"},tab_bar={active_tab={fg_color="#ffffff",bg_color="#000000",italic=true}}}; return config\n')
        write('.tmux.conf', 'set -g mouse on\n')
        run([CLI, 'import', source])
        preview = run([CLI, 'core', 'profile.preview'], input=json.dumps({'name': source.name}).encode(), native_env=dict(env, KTM_THEME_NO_APPLY='0'))
        assert json.loads(preview.stdout)['ok'], preview.stdout
        run([CLI, 'apply', source.name, '--without-dependencies'])
        p=json.loads((source/'theme.json').read_text())['palette']
        prompt=run(['starship', 'module', 'character', '--status', '0']).stdout.decode()
        rgb=tuple(int(p['base'][11][i:i+2],16) for i in (1,3,5))
        assert f'38;2;{rgb[0]};{rgb[1]};{rgb[2]}' in prompt, (source.name, prompt, rgb)
        run(['foot','--check-config','--config',root/'.config/foot/foot.ini'])
        wez = run(['wezterm','--config-file',root/'.config/wezterm/wezterm.lua','ls-fonts'])
        assert not re.search(rb'ERROR|configuration error|error loading', wez.stderr, re.IGNORECASE), wez.stderr
        # Lua compilation/evaluation against a tiny native API stub also checks
        # that the user's non-colour fields survived the fixed local wrapper.
        lua="""package.preload.wezterm=function() return {config_builder=function() return {} end, color={load_scheme=function(path) assert(io.open(path)):close(); return {background='native-test',indexed={},tab_bar={}} end}} end
local cfg=assert(loadfile(arg[1]))();assert(cfg.font_size==13 and cfg.enable_tab_bar==false and cfg.colors.background=='native-test' and cfg.colors.indexed[255]=='#123456' and cfg.colors.tab_bar.active_tab.italic==true)
"""
        write('check.lua',lua)
        run(['lua',root/'check.lua',root/'.config/wezterm/wezterm.lua'])
        assert 'run-shell' not in (root/'.config/tmux/kolira.conf').read_text()
        print(source.name+': native Starship RGB, Foot parser, WezTerm loader PASS',flush=True)
