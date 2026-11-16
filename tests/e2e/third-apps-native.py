#!/usr/bin/env python3
"""Native smoke checks of batch-three palette resources in disposable HOME.

Run after cargo build: python3 tests/e2e/third-apps-native.py
No user profiles, media, accounts or LibreOffice settings are accessed.
"""
import json
import os
from pathlib import Path
import pty
import select
import shutil
import socket
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
CLI = REPO / "gui/qt-app/src/core/rust_core/target/debug/kolira"
TARGETS = ["micro", "lsd", "glow", "chromium", "google-chrome", "gimp", "heroic",
           "prismlauncher", "vim", "mpv", "cliamp", "vscodium"]
FILES = [".config/micro/settings.json", ".config/micro/colorschemes/kolira.micro",
         ".config/lsd/config.yaml", ".config/lsd/themes/kolira.yaml",
         ".config/glow/glow.yml", ".config/glow/kolira.json",
         ".config/chromium/kolira-theme/manifest.json",
         ".config/google-chrome/kolira-theme/manifest.json",
         ".config/GIMP/3.0/gimprc", ".config/GIMP/3.0/themes/Kolira/gimp.css",
         ".config/heroic/config.json", ".local/share/PrismLauncher/prismlauncher.cfg",
         ".local/share/PrismLauncher/themes/kolira/theme.json", ".vim/kolira-palette.json",
         ".config/mpv/mpv.conf", ".config/cliamp/config.toml",
         ".config/cliamp/themes/kolira.toml", ".config/VSCodium/User/settings.json"]


def run(args, env, **kwargs):
    p = subprocess.run(args, env=env, capture_output=True, timeout=25, **kwargs)
    assert p.returncode == 0, (args, p.returncode, p.stderr[-3000:])
    return p.stdout


def run_tty(args, env):
    master, slave = pty.openpty()
    p = subprocess.Popen(args, env=env, stdin=subprocess.DEVNULL, stdout=slave, stderr=slave)
    os.close(slave)
    output = b""
    deadline = time.monotonic() + 15
    try:
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.1)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                output += chunk
            elif p.poll() is not None:
                break
        p.wait(timeout=3)
        assert p.returncode == 0, output
        return output
    finally:
        if p.poll() is None:
            p.terminate()
            p.wait(timeout=3)
        os.close(master)


def rpc(method, args, env):
    v = json.loads(run([str(CLI), "core", method], env, input=json.dumps(args).encode()))
    assert v["ok"], v
    return v["data"]


def tty_micro(env, home):
    master, slave = pty.openpty()
    import fcntl
    import struct
    import termios
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
    p = subprocess.Popen(["micro", "-debug", str(home / "sample.md")], env=env,
                         cwd=home, stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
    os.close(slave)
    screen = b""
    try:
        limit = time.monotonic() + 3
        while time.monotonic() < limit:
            if select.select([master], [], [], 0.2)[0]:
                try:
                    screen += os.read(master, 65536)
                except OSError:
                    break
        os.write(master, b"\x11")
        p.wait(timeout=10)
        assert p.returncode == 0, screen[-1000:]
        log = (home / "log.txt").read_text() if (home / "log.txt").exists() else ""
        assert "Error parsing" not in log and "Error loading" not in log, log
        assert json.loads((home / ".config/micro/settings.json").read_text())["colorscheme"] == "kolira"
        assert screen, "No Micro terminal output"
    finally:
        if p.poll() is None:
            p.terminate()
            p.wait(timeout=5)
        os.close(master)


def tty_cliamp(env, home, palette):
    (home / "audio-null.conf").write_text("pcm.!default { type null }\nctl.!default { type null }\n")
    (home / "tmp").mkdir(exist_ok=True)
    env = dict(env, TMPDIR=str(home / "tmp"), ALSA_CONFIG_PATH=str(home / "audio-null.conf"),
               PULSE_SERVER="unix:" + str(home / "run/no-pulse"),
               PIPEWIRE_REMOTE=str(home / "run/no-pipewire"))
    master, slave = pty.openpty()
    import fcntl
    import struct
    import termios
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 35, 120, 0, 0))
    p = subprocess.Popen(["cliamp", "--no-auto-play", "--low-power"], env=env, cwd=home,
                         stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
    os.close(slave)
    screen = b""
    try:
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.1)[0]:
                try:
                    screen += os.read(master, 65536)
                except OSError:
                    break
        os.write(master, b"q")
        p.wait(timeout=10)
        assert p.returncode == 0, screen[-3000:]
        channels = ";".join(str(int(palette["accent"][i:i+2], 16)) for i in (1,3,5)).encode()
        assert channels in screen, ("Cliamp did not render palette accent", screen[-4000:])
    finally:
        if p.poll() is None:
            os.killpg(p.pid, 15)
            p.wait(timeout=5)
        os.close(master)


def mpv_options(env, home, palette):
    ipc = home / "mpv.sock"
    p = subprocess.Popen(["mpv", "--idle=yes", "--no-terminal", "--vo=null", "--ao=null",
                          "--log-file=" + str(home / "mpv.log"), "--input-ipc-server=" + str(ipc)], env=env,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        for _ in range(100):
            if ipc.exists():
                break
            assert p.poll() is None, p.communicate()
            time.sleep(0.03)
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(3)
            client.connect(str(ipc))
            io = client.makefile("rwb")
            for i, (option, expected) in enumerate([("osd-color", palette["base"][5]),
                                                   ("osd-border-color", palette["base"][0]),
                                                   ("osd-shadow-color", palette["base"][0])]):
                io.write((json.dumps({"command": ["get_property", "options/" + option],
                                      "request_id": i}) + "\n").encode())
                io.flush()
                while True:
                    reply = json.loads(io.readline())
                    if reply.get("request_id") == i:
                        break
                assert reply["error"] == "success", reply
                assert reply["data"].lower() == ("#ff" + expected[1:]).lower(), (option, reply, expected, (home / ".config/mpv/mpv.conf").read_text(), (home / "mpv.log").read_text()[:8500])
    finally:
        p.terminate()
        p.communicate(timeout=5)


def main():
    native = {name: bool(shutil.which(name)) for name in ["micro", "lsd", "glow", "vim", "mpv", "cliamp"]}
    for source in sorted((REPO / "themes").iterdir()):
        if not (source / "theme.json").is_file():
            continue
        palette = json.loads((source / "theme.json").read_text())["palette"]
        with tempfile.TemporaryDirectory(prefix="kolira-native-third-") as tmp:
            home = Path(tmp)
            env = dict(os.environ, HOME=tmp, XDG_CONFIG_HOME=tmp + "/.config",
                       XDG_DATA_HOME=tmp + "/.local/share", XDG_CACHE_HOME=tmp + "/.cache",
                       XDG_STATE_HOME=tmp + "/.local/state", XDG_RUNTIME_DIR=tmp + "/run",
                       KTM_THEME_NO_APPLY="1", KTM_OFFLINE="1", TERM="xterm-256color",
                       COLORTERM="truecolor", CLICOLOR_FORCE="1")
            for k in ["NO_COLOR", "GLOW_STYLE", "MICRO_CONFIG_HOME", "GLOW_CONFIG_HOME",
                      "LSD_CONFIG_DIR", "GIMP3_DIRECTORY", "MPV_HOME", "GLAMOUR_STYLE", "LS_COLORS"]:
                env.pop(k, None)
            (home / "run").mkdir(mode=0o700)
            selected = home / ".config/kolira/themes/selected"
            selected.mkdir(parents=True)
            (selected / "theme.json").write_text(json.dumps({"schema": "kolira/theme@2", "name": "selected",
                                                             "palette": palette, "app_targets": TARGETS}))
            for rel in FILES:
                (selected / rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source / rel, selected / rel)
            (home / ".vimrc").write_text("set nocompatible\nset number\n")
            (home / "sample.md").write_text("# Palette title\n\n**Hello** [link](https://example.org).\n")
            preview = rpc("profile.preview", {"name": "selected"}, env)
            rpc("profile.apply", {"name": "selected", "preview_token": preview["token"], "skip_dependencies": True}, env)
            if native["lsd"]:
                p = subprocess.run(["lsd", "--color", "always", "--long", str(home / "sample.md")], env=env, capture_output=True, timeout=10)
                assert p.returncode == 0 and not p.stderr, p.stderr
                assert b"\x1b[" in p.stdout, p.stdout
            if native["glow"]:
                data = run_tty(["glow", str(home / "sample.md")], env)
                rgb = ";".join(str(int(palette["accent"][i:i+2], 16)) for i in (1, 3, 5)).encode()
                assert rgb in data, (source.name, "Glow does not render palette accent", data)
            if native["vim"]:
                result = home / "vim-result"
                command = "call writefile([get(g:, 'colors_name', ''), synIDattr(hlID('Normal'), 'fg', 'gui'), string(&number)], '" + str(result) + "')"
                run_tty(["vim", "-N", "-i", "NONE", "-n", "-c", command, "-c", "qa!"], env)
                assert result.read_text().splitlines() == ["kolira", palette["base"][5], "1"], result.read_text()
            if native["mpv"]:
                mpv_options(env, home, palette)
            if native["micro"]:
                tty_micro(env, home)
            if native["cliamp"]:
                tty_cliamp(env, home, palette)
            print(source.name + ": native " + ", ".join(k for k,v in native.items() if v) + " PASS", flush=True)
    print("Limits: this script covers CLI clients only; see separate GTK, Prism/Chromium and Heroic tests.")


if __name__ == "__main__":
    main()
