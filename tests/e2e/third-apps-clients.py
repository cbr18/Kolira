#!/usr/bin/env python3
"""Load all eight native Prism themes in disposable app profiles.

Prism uses Qt offscreen. No personal accounts are accessed.
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
    app = "prismlauncher"
    if not shutil.which(app):
        continue
    with tempfile.TemporaryDirectory(prefix="kolira-native-" + app + "-") as tmp:
        root = Path(tmp)
        (root / "run").mkdir(mode=0o700)
        env = dict(os.environ, HOME=tmp, XDG_CONFIG_HOME=tmp + "/.config",
                   XDG_DATA_HOME=tmp + "/.local/share", XDG_CACHE_HOME=tmp + "/.cache",
                   XDG_RUNTIME_DIR=tmp + "/run", QT_QPA_PLATFORM="offscreen",
                   QT_STYLE_OVERRIDE="Fusion")
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
        print(source.name + ": native " + app + " theme load PASS", flush=True)
