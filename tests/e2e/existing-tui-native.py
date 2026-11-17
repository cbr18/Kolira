#!/usr/bin/env python3
"""Existing btop/Yazi/lazygit/Zellij adapters, native parsers and PTY colours.

Each palette gets its own HOME/config/cache/runtime and Git repository. No user
session, repository, shell init, accounts or existing TUI server is accessed.
"""
from pathlib import Path
import fcntl
import json
import os
import pty
import select
import shutil
import struct
import subprocess
import tempfile
import termios
import time

REPO=Path(__file__).resolve().parents[2]
CLI=REPO/'gui/qt-app/src/core/rust_core/target/debug/kolira'

def capture(args,env,root):
    master,slave=pty.openpty()
    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,140,0,0))
    def terminal_session():
        os.setsid()
        fcntl.ioctl(0,termios.TIOCSCTTY,0)
    process=subprocess.Popen(args,env=env,cwd=root,stdin=slave,stdout=slave,stderr=slave,preexec_fn=terminal_session)
    os.close(slave);data=b''
    try:
        deadline=time.monotonic()+3
        while time.monotonic()<deadline and process.poll() is None:
            if select.select([master],[],[],0.1)[0]:
                try:
                    chunk=os.read(master,262144);data+=chunk
                    if b'\x1b[18t' in chunk:os.write(master,b'\x1b[8;40;140t')
                    if b'\x1b]11;?' in chunk:os.write(master,b'\x1b]11;rgb:0000/0000/0000\x07')
                    if b'\x1b[6n' in chunk:os.write(master,b'\x1b[1;1R')
                except OSError:break
        if process.poll() is None:os.write(master,b'q')
        try:process.wait(timeout=8)
        except subprocess.TimeoutExpired:process.terminate();process.wait(timeout=5)
        assert data,(args,'no native terminal output')
        assert process.returncode==0,(args,process.returncode,data[-2000:])
        return data
    finally:
        if process.poll() is None:process.kill();process.wait()
        os.close(master)

def rgb_seen(data,hexvalue):
    components=[int(hexvalue[i:i+2],16) for i in (1,3,5)]
    return any(pattern.encode() in data for pattern in [';2;'+';'.join(map(str,components)), ':2::'+':'.join(map(str,components))])

for theme in sorted((REPO/'themes').iterdir()):
    if not (theme/'theme.json').is_file():continue
    with tempfile.TemporaryDirectory(prefix='kolira-tui-') as tmp:
        root=Path(tmp);(root/'run').mkdir(mode=0o700)
        env=dict(os.environ,HOME=tmp,XDG_CONFIG_HOME=tmp+'/.config',XDG_DATA_HOME=tmp+'/.local/share',XDG_CACHE_HOME=tmp+'/.cache',XDG_STATE_HOME=tmp+'/.local/state',XDG_RUNTIME_DIR=tmp+'/run',TMPDIR=tmp,TERM='xterm-256color',COLORTERM='truecolor',TCELL_TRUECOLOR='1',CLICOLOR_FORCE='1',SHELL='/bin/sh',KTM_OFFLINE='1',KTM_THEME_NO_APPLY='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/nonexistent')
        for key in ['NO_COLOR','LG_CONFIG_FILE','YAZI_CONFIG_HOME','CONFIG_DIR','TMUX','ZELLIJ','ZELLIJ_SESSION_NAME','CODEX_HOME','CLAUDE_CONFIG_DIR','STARSHIP_CONFIG','WEZTERM_CONFIG_FILE','WEZTERM_CONFIG_DIR']:
            env.pop(key,None)
        def run(args,**kw):
            out=subprocess.run([str(x) for x in args],env=env,cwd=root,capture_output=True,timeout=60,**kw)
            assert out.returncode==0,(args,out.stderr[-2000:],out.stdout[-1000:])
            return out
        def write(rel,text):
            path=root/rel;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text)
        write('sample.md','# native palette audit\n')
        run(['git','init','-q'])
        write('.config/btop/btop.conf','color_theme = "Personal"\ntruecolor = true\n')
        write('.config/btop/themes/Personal.theme','theme[main_fg]="#112233"\n')
        write('.config/yazi/theme.toml','[mgr]\ncwd = { fg = "#123456" }\n')
        write('.config/lazygit/config.yml','disableStartupPopups: true\ngit:\n  autoFetch: false\ngui:\n  showIcons: false\n  theme:\n    activeBorderColor: ["#112233"]\n')
        write('.config/zellij/config.kdl','theme "default"\n')
        run([CLI,'import',theme])
        run([CLI,'apply',theme.name,'--without-dependencies'])
        palette=json.loads((theme/'theme.json').read_text())['palette'];checked=[]
        if shutil.which('btop'):
            data=capture(['btop'],env,root)
            assert rgb_seen(data,palette['base'][5]),(theme.name,'btop did not render palette text')
            checked.append('btop RGB')
        if shutil.which('yazi'):
            data=capture(['yazi',str(root)],env,root)
            assert rgb_seen(data,palette['base'][13]),(theme.name,'Yazi did not render cwd palette colour')
            checked.append('Yazi RGB')
        if shutil.which('lazygit'):
            data=capture(['lazygit','--use-config-dir',tmp+'/.config/lazygit'],env,root)
            assert rgb_seen(data,palette['accent']),(theme.name,'lazygit did not render accent',data[-2500:])
            checked.append('lazygit RGB')
        if shutil.which('zellij'):
            run(['zellij','setup','--check'])
            checked.append('Zellij parser')
        write('.config/kolira/themes/neutral/theme.json',json.dumps({'schema':'kolira/theme@2','name':'neutral'}))
        run([CLI,'apply','neutral','--without-dependencies'])
        assert '123456' in (root/'.config/yazi/theme.toml').read_text()
        assert 'showIcons: false' in (root/'.config/lazygit/config.yml').read_text()
        assert 'theme "default"' in (root/'.config/zellij/config.kdl').read_text()
        assert not (root/'.config/zellij/themes/kolira.kdl').exists()
        if shutil.which('btop'):
            assert 'Personal' in (root/'.config/btop/btop.conf').read_text(), (theme.name,'btop selector did not return')
            assert rgb_seen(capture(['btop'],env,root),'#112233'), (theme.name,'btop original RGB did not return')
        if shutil.which('yazi'):
            assert rgb_seen(capture(['yazi',str(root)],env,root),'#123456'), (theme.name,'Yazi original RGB did not return')
        if shutil.which('lazygit'):
            assert rgb_seen(capture(['lazygit','--use-config-dir',tmp+'/.config/lazygit'],env,root),'#112233'), (theme.name,'lazygit original RGB did not return')
        print(theme.name+': '+', '.join(checked)+'; return PASS',flush=True)
