#!/usr/bin/env python3
"""Exercise recovery points and the GUI in a disposable Plasma session.

Mount the workspace read-only at /src and run as an ordinary container user
under dbus-run-session. Use the KDE packages from tests/e2e/Dockerfile.
"""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

if not Path("/.dockerenv").exists() or os.getuid() == 0:
    raise SystemExit("Run this as an ordinary user inside a disposable Docker container")

if sys.argv[1:2] == ["observe-palette"]:
    from PySide6.QtCore import QTimer

    app_dir = Path("/src/gui/qt-app/src")
    sys.path.insert(0, str(app_dir))
    spec = importlib.util.spec_from_file_location("kolira_entry", app_dir / "__main__.py")
    entry = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(entry)
    output = Path(sys.argv[2])

    def observe(app, window):
        from theme_controller import ThemeController

        controller = ThemeController.instance()

        def record():
            if window.isVisible() and "dark" in controller.colors:
                with output.open("a") as stream:
                    print("dark" if controller.colors["dark"] else "light", file=stream)

        controller.colorsChanged.connect(record)
        QTimer.singleShot(200, record)

    entry._maybe_setup_screenshot = observe
    sys.argv[:] = ["kolira"]
    raise SystemExit(entry.main())

work = Path(tempfile.mkdtemp(prefix="kolira-close-"))
for key, name in [("HOME","home"),("XDG_CONFIG_HOME","config"),("XDG_DATA_HOME","data"),("XDG_CACHE_HOME","cache"),("XDG_RUNTIME_DIR","runtime")]:
    directory = work / name
    directory.mkdir(mode=0o700)
    os.environ[key] = str(directory)
os.environ.update(WAYLAND_DISPLAY="wayland-kolira-close", QT_QPA_PLATFORM="wayland", XDG_CURRENT_DESKTOP="KDE",
                  XDG_SESSION_TYPE="wayland", KDE_SESSION_VERSION="6", LIBGL_ALWAYS_SOFTWARE="1",
                  QT_QUICK_BACKEND="software", KWIN_COMPOSE="Q", KTM_OFFLINE="1", LANG="C.UTF-8",
                  KOLIRA_TEST_ROOT=str(work))
cli = os.environ.get("KTM_THEME_PROFILE_BIN", "/src/gui/qt-app/src/core/rust_core/target/debug/kolira")
config = Path(os.environ["XDG_CONFIG_HOME"])
applets = config / "plasma-org.kde.plasma.desktop-appletsrc"
globals_file = config / "kdeglobals"
probe = ["qdbus6","org.kde.plasmashell","/PlasmaShell","org.kde.PlasmaShell.evaluateScript"]
owner = ["qdbus6","org.freedesktop.DBus","/","org.freedesktop.DBus.NameHasOwner","org.kde.plasmashell"]
shim = work / "bin"
shim.mkdir()
cmd = shim / "systemctl"
cmd.write_text("""#!/usr/bin/python3
import os, subprocess, sys, time
from pathlib import Path
root = Path(os.environ["KOLIRA_TEST_ROOT"])
owner = ["qdbus6","org.freedesktop.DBus","/","org.freedesktop.DBus.NameHasOwner","org.kde.plasmashell"]
def alive():
    result = subprocess.run(owner,text=True,capture_output=True,timeout=5)
    return result.stdout.strip()=="true"
if "plasma-plasmashell.service" not in sys.argv: sys.exit(0)
if "is-active" in sys.argv: sys.exit(0 if alive() else 3)
if "stop" in sys.argv:
    subprocess.run(["pkill","-TERM","-x","plasmashell"])
    for _ in range(80):
        if not alive(): sys.exit(0)
        time.sleep(.1)
    sys.exit(1)
if "start" in sys.argv and not alive():
    with (root/"plasma.log").open("a") as log:
        subprocess.Popen(["plasmashell"],stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
    for _ in range(100):
        if alive(): sys.exit(0)
        time.sleep(.1)
    sys.exit(1)
""")
cmd.chmod(0o700)
os.environ["PATH"] = str(shim) + ":" + os.environ["PATH"]
subprocess.run(["dbus-update-activation-environment","HOME","XDG_CONFIG_HOME","XDG_DATA_HOME",
                "XDG_CACHE_HOME","XDG_RUNTIME_DIR","WAYLAND_DISPLAY","QT_QPA_PLATFORM",
                "XDG_CURRENT_DESKTOP","KDE_SESSION_VERSION","QT_QUICK_BACKEND",
                "LIBGL_ALWAYS_SOFTWARE"], check=True)


def run(argv, **kwargs):
    return subprocess.run(argv, text=True, capture_output=True, timeout=100, **kwargs)

def rpc(method, **args):
    result = run([cli,"core",method], input=json.dumps(args))
    value = json.loads(result.stdout)
    assert value["ok"], (method, value, result.stderr)
    return value["data"]

def wait(predicate,message,timeout=35):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate(): return
        time.sleep(.3)
    raise AssertionError(message)

def desktops():
    return run([*probe,"print(desktops().length)"]).stdout.strip() == "1"

active = []
try:
    with (work/"kwin.log").open("w") as log:
        active.append(subprocess.Popen(["kwin_wayland","--virtual","--no-lockscreen","--width","1100",
            "--height","750","--socket",os.environ["WAYLAND_DISPLAY"]],stdout=log,stderr=log))
    wait((Path(os.environ["XDG_RUNTIME_DIR"])/os.environ["WAYLAND_DISPLAY"]).is_socket,"KWin did not start")
    with (work/"plasma.log").open("w") as log:
        active.append(subprocess.Popen(["plasmashell"],stdout=log,stderr=log))
    wait(lambda: applets.is_file() and desktops(), "Plasma did not expose its desktop")
    first, second = work/"before.png", work/"after #%.png"
    for image, color in [(first,"navy"),(second,"gold")]:
        run(["magick","-size","128x128","xc:"+color,str(image)],check=True)
    run(["plasma-apply-wallpaperimage",str(first)],check=True)
    wait(lambda: "before.png" in applets.read_text(), "starting wallpaper not persisted")
    session = rpc("inspector.state")["session"]
    staged = rpc("inspector.wallpaper",session=session,value=str(second),stage=True)
    assert staged["ok"] and "after%" not in applets.read_text(), staged
    applied = rpc("inspector.apply",session=session,check_live=True,source="gui")
    assert applied["ok"] and "after%20%23%25.png" in applets.read_text(), applied
    point = rpc("operation.list")["operations"][0]["id"]
    assert any(item["id"] == point for item in rpc("restore_point.list")["points"])
    rpc("restore_point.restore",id=point,confirmed=True,source="gui")
    assert "before.png" in applets.read_text() and desktops()
    print("PASS native wallpaper draft, Apply and automatic restore point", flush=True)
    manual = rpc("restore_point.create", name="Original desktop")
    for name, scheme in [("NativeDark","BreezeDark"),("NativeLight","BreezeLight")]:
        assert not rpc("agent.prepare",name=name,values={"color_scheme":scheme})["missing"]
    observed = work / "palette.txt"
    app = [sys.executable, __file__, "observe-palette", str(observed)]
    app_env = {**os.environ, "KTM_THEME_PROFILE_BIN": cli}
    with (work / "kolira.log").open("w") as log:
        active.append(subprocess.Popen(app, stdout=log, stderr=log, env=app_env))
    wait(lambda: observed.is_file() and observed.stat().st_size, "Kolira did not open")
    duplicate = run(app, env=app_env)
    assert duplicate.returncode == 0 and "already running" in duplicate.stderr, duplicate.stderr
    print("PASS second Kolira launch hands over to the first", flush=True)
    for name, scheme in [("NativeDark","BreezeDark"),("NativeLight","BreezeLight"),("NativeDark","BreezeDark")]:
        for attempt in range(4):
            report = rpc("profile.prepare_apply",name=name,keep_wallpaper=True)
            try:
                rpc("profile.apply",name=name,keep_wallpaper=True,preview_token=report["token"],source="gui")
                break
            except AssertionError as error:
                if "PREVIEW_STALE" not in str(error) or attempt == 3:
                    raise
                # A newly started Plasma can persist its layout after the first review.
                time.sleep(1.5)
        wait(desktops,"Plasma did not reload")
        assert "ColorScheme="+scheme in globals_file.read_text()
        expected = "dark" if name == "NativeDark" else "light"
        wait(lambda: observed.read_text().splitlines()[-1] == expected,
             "The open Kolira window did not pick up " + scheme)
        matches = rpc("profile.live_matches", names=["NativeDark", "NativeLight"])
        assert matches["matches"] == [name], matches
    rpc("restore_point.restore",id=manual["id"],confirmed=True,source="gui")
    wait(desktops,"Manual restore did not reload Plasma")
    assert "before.png" in applets.read_text()
    print("PASS native dark → light → dark, open Kolira palette, manual restore and Plasma restart", flush=True)
    active[-1].terminate()
    active[-1].wait(timeout=5)

    gui = subprocess.run([sys.executable,"/src/tests/e2e/ui-audit.py",str(work/"interaction.txt"),
        "--check","--theme","dark","--width","820","--height","680","--platform","wayland"],
        env={**os.environ,"KTM_THEME_PROFILE_BIN":cli},capture_output=True,text=True,timeout=75)
    print(gui.stdout,flush=True)
    if gui.returncode:
        print(gui.stderr[-7500:],flush=True)
        raise AssertionError("Native GUI workflow failed")
    print("PASS GUI using native Wayland and Kirigami",flush=True)
except BaseException:
    for log in ["kwin.log","plasma.log","kolira.log"]:
        if (work/log).is_file():
            print(log, (work/log).read_text(errors="replace")[-4000:],flush=True)
    raise
finally:
    for p in reversed(active):
        if p.poll() is None:
            p.terminate()
            try: p.wait(timeout=5)
            except subprocess.TimeoutExpired: p.kill(); p.wait(timeout=5)
