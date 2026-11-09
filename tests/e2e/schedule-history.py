#!/usr/bin/env python3
"""Live scheduler/restore smoke test, ONLY inside a disposable Docker Plasma.

Mount checkout read-only at /src, run under dbus-run-session, set
KTM_PRIORITY_CONTAINER=1. Logs/result live in /tmp/ktm-priority-live. No host
D-Bus or display sockets may be mounted. Does not need the GUI window.
"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

if os.environ.get('KTM_PRIORITY_CONTAINER') != '1' or not Path('/.dockerenv').exists():
    raise SystemExit('Run only in a disposable Docker container')

out = Path('/tmp/ktm-priority-live')
out.mkdir(exist_ok=True)
if os.getuid() == 0:
    subprocess.run(['useradd', '-m', '-u', '12001', 'ktmtest'], check=True)
    os.chown(out, 12001, 12001)
    # Start a new D-Bus session as the ordinary user.
    os.execvp('runuser', ['runuser', '-u', 'ktmtest', '--', 'env', 'HOME=/home/ktmtest',
                          'KTM_PRIORITY_CONTAINER=1', 'dbus-run-session', '--', sys.executable, __file__])

home = Path.home()
runtime = out / 'runtime'
runtime.mkdir(mode=0o700, exist_ok=True)
os.environ.update(XDG_CONFIG_HOME=str(home / '.config'), XDG_DATA_HOME=str(home / '.local/share'),
                  XDG_CACHE_HOME=str(home / '.cache'), XDG_RUNTIME_DIR=str(runtime),
                  WAYLAND_DISPLAY='wayland-priority', XDG_SESSION_TYPE='wayland',
                  XDG_CURRENT_DESKTOP='KDE', KDE_SESSION_VERSION='6', KDE_FULL_SESSION='true',
                  QT_QPA_PLATFORM='wayland', LIBGL_ALWAYS_SOFTWARE='1', KWIN_COMPOSE='Q',
                  QT_QUICK_BACKEND='software', KTM_OFFLINE='1', KTM_COMMAND_TIMEOUT='15', LANG='C.UTF-8')
os.environ.pop('KTM_THEME_NO_APPLY', None)
os.environ['E2E_OUT'] = str(out)
cli = os.environ.get('KTM_THEME_PROFILE_BIN', '/src/gui/qt-app/src/core/rust_core/target/release/theme-profile')
shim = home / 'bin'
shim.mkdir(exist_ok=True)
(shim / 'systemctl').write_text('''#!/usr/bin/python
import os, subprocess, sys, time
from pathlib import Path
args=sys.argv[1:]
if 'plasma-plasmashell.service' not in args: sys.exit(0)
def alive():
 pids=subprocess.run(['pgrep','-x','plasmashell'],capture_output=True,text=True).stdout.split()
 for pid in pids:
  try:
   if Path('/proc/'+pid+'/stat').read_text().rsplit(') ',1)[1].split()[0]!='Z': return True
  except OSError: pass
 return False
if 'is-active' in args: sys.exit(0 if alive() else 3)
if 'stop' in args:
 subprocess.run(['pkill','-TERM','-x','plasmashell'])
 for _ in range(100):
  if not alive(): sys.exit(0)
  time.sleep(.1)
 sys.exit(1)
if 'start' in args and not alive():
 log=open(Path(os.environ['E2E_OUT'])/'plasmashell.log','ab')
 subprocess.Popen(['plasmashell'],stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
''')
(shim / 'systemctl').chmod(0o755)
os.environ['PATH'] = str(shim) + ':' + os.environ['PATH']


def rpc(method, **args):
    result = subprocess.run([cli, 'core', method], input=json.dumps(args), text=True,
                            capture_output=True, timeout=180)
    data = json.loads(result.stdout)
    assert result.returncode == 0 and data['ok'], (method, data, result.stderr)
    return data['data']


def wait_for(predicate, timeout=30):
    deadline = time.monotonic() + timeout
    while not predicate():
        if time.monotonic() >= deadline:
            raise AssertionError('Timed out waiting for Plasma/scheduler')
        time.sleep(.5)


kwinlog = (out / 'kwin.log').open('ab')
kwin = subprocess.Popen(['kwin_wayland', '--virtual', '--no-lockscreen', '--width', '1280',
                         '--height', '800', '--socket', 'wayland-priority'], stdout=kwinlog, stderr=kwinlog)
try:
    wait_for(lambda: (runtime / 'wayland-priority').is_socket())
    log = (out / 'plasmashell.log').open('ab')
    subprocess.Popen(['plasmashell'], stdout=log, stderr=log)
    def service(name):
        r = subprocess.run(['qdbus6', 'org.freedesktop.DBus', '/', 'org.freedesktop.DBus.NameHasOwner', name], capture_output=True, text=True, timeout=5)
        return r.returncode == 0 and r.stdout.strip() == 'true'
    wait_for(lambda: service('org.kde.KWin') and service('org.kde.plasmashell'))
    time.sleep(5)
    target = rpc('agent.prepare', name='scheduled-probe', values={'widget_style': 'Breeze', 'color_scheme': 'BreezeDark'})
    assert target['missing'] == [], target
    config = home / '.config/kdeglobals'
    # A personal setting must survive apply and restoration.
    with config.open('a') as f:
        f.write('\n[General]\nBrowserApplication=personal-marker\n')
    before = config.read_bytes()
    when = datetime.datetime.now().astimezone() + datetime.timedelta(minutes=2)
    rpc('schedule.put', rule={'id': 'live-probe', 'theme': 'scheduled-probe', 'time': when.strftime('%H:%M'),
                             'days': [when.isoweekday()], 'enabled': True, 'keep_wallpaper': True})
    enabled = rpc('schedule.enable', enabled=True)
    wait_for(lambda: rpc('schedule.list')['running'])
    def completed():
        state = rpc('schedule.list')
        (out / 'schedule.json').write_text(json.dumps(state, indent=2))
        last = state['rules'][0]['last']
        return bool(last and last['state'] not in ('claimed', 'running'))
    wait_for(completed, 160)
    last = rpc('schedule.list')['rules'][0]['last']
    assert last['state'] == 'succeeded', last
    assert rpc('profile.active') == 'scheduled-probe'
    assert b'BrowserApplication=personal-marker' in config.read_bytes()
    history = rpc('operation.list')
    (out / 'history.json').write_text(json.dumps(history, indent=2))
    operation = next(r for r in history['operations'] if r['id'] == last['operation_id'])
    assert operation['source'] == 'schedule' and operation['backup_status'] == 'available'
    restored = rpc('operation.restore', id=operation['id'], confirmed=True, source='mcp')
    assert restored['restored_operation'] == operation['id']
    assert config.read_bytes() == before, 'Original config bytes were not restored'
    assert service('org.kde.KWin') and service('org.kde.plasmashell')
    rpc('schedule.enable', enabled=False)
    result = {'ok': True, 'scheduled_operation': operation['id'], 'restore_operation': restored['operation_id'],
              'live_hooks_disabled': False, 'gui_window_running': False,
              'cli_sha256': hashlib.sha256(Path(cli).read_bytes()).hexdigest(),
              'kwin_version': subprocess.run(['kwin_wayland','--version'],capture_output=True,text=True).stdout.strip()}
    (out / 'result.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(result), flush=True)
finally:
    try:
        rpc('schedule.enable', enabled=False)
    except Exception:
        pass
    kwin.send_signal(signal.SIGTERM)
