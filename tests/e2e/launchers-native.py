#!/usr/bin/env python3
"""Real Rofi/Fuzzel parsers and GTK4 CSS, disposable Docker HOME per theme.

Run in Dockerfile.launchers with /src mounted read-only and DISPLAY from Xvfb.
No user's config, sockets or desktop session are mounted.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

assert Path('/.dockerenv').is_file() and os.environ.get('KOLIRA_LAUNCHERS_CONTAINER') == '1'
REPO = Path('/src')
CLI = REPO / 'gui/qt-app/src/core/rust_core/target/debug/kolira'

import gi

gi.require_version('Gtk', '4.0')
from gi.repository import Gtk

Gtk.init()
for theme in sorted((REPO / 'themes').iterdir()):
    if not (theme / 'theme.json').is_file():
        continue
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        env = dict(os.environ, HOME=tmp, XDG_CONFIG_HOME=tmp+'/.config',
                   XDG_DATA_HOME=tmp+'/.local/share', XDG_CACHE_HOME=tmp+'/.cache',
                   KTM_THEME_NO_APPLY='1', KTM_OFFLINE='1')
        for key in ('STARSHIP_CONFIG','WEZTERM_CONFIG_FILE','WEZTERM_CONFIG_DIR','CODEX_HOME','CLAUDE_CONFIG_DIR','CONFIG_DIR','WALLPAPER_DIR','TMUX'):
            env.pop(key, None)
        def run(args, **kwargs):
            out = subprocess.run([str(a) for a in args], env=env, cwd=root,
                                 capture_output=True, timeout=60, **kwargs)
            assert out.returncode == 0, (args, out.stdout[-2000:], out.stderr[-4000:])
            return out
        def write(rel, text):
            path=root/rel; path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text)
        default=run(['rofi','-no-config','-dump-theme']).stdout.decode()
        write('.config/rofi/themes/personal.rasi', default+'\nwindow { width: 777px; padding: 13px; }\n')
        write('.config/rofi/config.rasi', '@theme "personal"\nconfiguration { modi: "drun"; }\n')
        write('.config/fuzzel/fuzzel.ini', '[main]\nfont=monospace:size=13\n[colors]\nbackground=11223380\n')
        write('.config/walker/config.toml', 'theme="personal"\n')
        write('.config/walker/themes/personal/style.css', '.box-wrapper { padding: 23px; }\n')
        paths=['.config/rofi/config.rasi','.config/fuzzel/fuzzel.ini','.config/walker/themes/personal/style.css']
        before={r:(root/r).read_bytes() for r in paths}
        run([CLI,'import',theme])
        preview_env=dict(env,KTM_THEME_NO_APPLY='0')
        preview=subprocess.run([str(CLI),'core','profile.preview'],env=preview_env,cwd=root,capture_output=True,timeout=60,input=json.dumps({'name':theme.name}).encode())
        assert preview.returncode==0 and json.loads(preview.stdout)['ok'],(preview.stdout[-3000:],preview.stderr[-2000:])
        run([CLI,'apply',theme.name,'--without-dependencies'])
        package=json.loads((theme/'theme.json').read_text())
        for target in ('rofi','fuzzel','walker'):
            assert target in package['app_targets']
        dump=run(['rofi','-config',root/paths[0],'-dump-theme']).stdout.decode()
        assert re.search(r'width:\s*777px', dump), dump
        assert re.search(r'padding:\s*13px', dump), dump
        # The native parser must accept both package data and the merged host.
        run(['rofi','-no-config','-rasi-validate',theme/'.config/rofi/themes/kolira.rasi'])
        run(['fuzzel','--check-config','--config',root/paths[1]])
        text=(root/paths[1]).read_text()
        assert re.search(r'background\s*=\s*[0-9a-fA-F]{6}80',text), text
        provider=Gtk.CssProvider()
        errors=[]
        provider.connect('parsing-error',lambda _p,_s,e:errors.append(str(e)))
        provider.load_from_path(str(root/paths[2]))
        assert not errors, errors
        # Exercise a foreground colour through GTK, not only a text matcher.
        Gtk.StyleContext.add_provider_for_display(__import__('gi.repository',fromlist=['Gdk']).Gdk.Display.get_default(),provider,800)
        window=Gtk.Window();box=Gtk.Box();box.add_css_class('box-wrapper');window.set_child(box)
        colour=box.get_color()
        wanted=package['palette']['base'][5]
        actual='#'+''.join(f'{round(c*255):02x}' for c in (colour.red,colour.green,colour.blue))
        assert actual.lower()==wanted.lower(),(theme.name,actual,wanted)
        window.destroy()
        # Add unrelated late edits, then return through the real transaction.
        for rel in paths:
            with (root/rel).open('a') as f:
                f.write('# late personal\n' if rel.endswith('.ini') else '/* late personal */\n')
        write('.config/kolira/themes/neutral/theme.json',json.dumps({'schema':'kolira/theme@2','name':'neutral'}))
        run([CLI,'apply','neutral','--without-dependencies'])
        for rel in paths:
            current=(root/rel).read_text()
            assert 'BEGIN kolira' not in current, (rel,current)
            assert 'late personal' in current
        run(['rofi','-config',root/paths[0],'-dump-theme'])
        run(['fuzzel','--check-config','--config',root/paths[1]])
        assert (root/paths[0]).read_text().startswith(before[paths[0]].decode())
        print(f'{theme.name}: native Rofi layout, Fuzzel RGBA, GTK4 colours, late-edit return PASS',flush=True)
