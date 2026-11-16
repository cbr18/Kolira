#!/usr/bin/env python3
"""Load all eight native Prism / Chromium themes in disposable app profiles.

Prism uses Qt offscreen; Chromium uses headless. Neither uses personal accounts.
Chromium's extension resource is copied too: the browser writes a Cached Theme.pak
next to its manifest, which must never contaminate repository snapshots.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
for source in sorted((REPO / "themes").iterdir()):
    if not (source / "theme.json").is_file():
        continue
    for app in ["prismlauncher", "chromium"]:
        if not shutil.which(app):
            continue
        with tempfile.TemporaryDirectory(prefix="kolira-native-" + app + "-") as tmp:
            root = Path(tmp)
            (root / "run").mkdir(mode=0o700)
            env = dict(os.environ, HOME=tmp, XDG_CONFIG_HOME=tmp + "/.config",
                       XDG_DATA_HOME=tmp + "/.local/share", XDG_CACHE_HOME=tmp + "/.cache",
                       XDG_RUNTIME_DIR=tmp + "/run", QT_QPA_PLATFORM="offscreen",
                       QT_STYLE_OVERRIDE="Fusion")
            if app == "prismlauncher":
                data = root / "prism"
                shutil.copytree(source / ".local/share/PrismLauncher", data)
                p = subprocess.Popen([app, "--dir", str(data), "--alive"], env=env, cwd=root,
                                     stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
                try:
                    time.sleep(2)
                    assert p.poll() is None, p.communicate()[0]
                finally:
                    if p.poll() is None:
                        p.terminate()
                    log = p.communicate(timeout=10)[0].decode(errors="replace")
                assert 'applying theme "Kolira"' in log, log
                assert "No theme qss present" in log, log
            else:
                ext = root / "extension"
                shutil.copytree(source / ".config/chromium/kolira-theme", ext)
                profile = root / "browser"
                p = subprocess.run([app, "--headless=new", "--no-sandbox", "--disable-gpu",
                                    "--no-first-run", "--disable-background-networking",
                                    "--no-default-browser-check", "--disable-default-apps",
                                    "--user-data-dir=" + str(profile), "--load-extension=" + str(ext),
                                    "--dump-dom", "about:blank"], env=env, cwd=root,
                                   capture_output=True, timeout=25)
                assert p.returncode == 0, p.stderr
                v = json.loads((profile / "Default/Preferences").read_text())
                theme = v["extensions"]["theme"]
                assert theme["id"] and theme["pack"] == str(ext), theme
                assert theme["id"] in v["extensions"]["settings"]
                assert (ext / "Cached Theme.pak").is_file(), "Browser did not compile native theme"
            print(source.name + ": native " + app + " theme load PASS", flush=True)
print("Limits: native load only; game/account workflows and personal browser selection are not exercised.")
