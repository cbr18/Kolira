#!/usr/bin/env python3
"""Check actual Heroic customCSS injection for all palettes in disposable HOME.

Run with xvfb-run -a python3 tests/e2e/third-apps-heroic.py.
Requires installed Heroic and Python Playwright; no browser download is needed.
Uses Electron's local CDP socket, not personal browser/game profiles.
"""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

from playwright.sync_api import sync_playwright

REPO = Path(__file__).resolve().parents[2]
if not shutil.which("heroic"):
    raise SystemExit("SKIP: Heroic is not installed")
with sync_playwright() as api:
    for source in sorted((REPO / "themes").iterdir()):
        if not (source / "theme.json").is_file():
            continue
        palette = json.loads((source / "theme.json").read_text())["palette"]
        with tempfile.TemporaryDirectory(prefix="kolira-native-heroic-") as tmp:
            root = Path(tmp)
            data = root / ".config/heroic"
            data.mkdir(parents=True)
            shutil.copyfile(source / ".config/heroic/config.json", data / "config.json")
            (root / "run").mkdir(mode=0o700)
            env = dict(os.environ, HOME=tmp, XDG_CONFIG_HOME=tmp + "/.config",
                       XDG_DATA_HOME=tmp + "/.local/share", XDG_CACHE_HOME=tmp + "/.cache",
                       XDG_RUNTIME_DIR=tmp + "/run")
            with (root / "client.log").open("wb") as log:
                p = subprocess.Popen(["heroic", "--no-sandbox", "--disable-gpu", "--ozone-platform=x11",
                                      "--remote-debugging-port=0", "--remote-allow-origins=*",
                                      "--user-data-dir=" + str(data)], env=env, cwd=root,
                                     stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    portfile = data / "DevToolsActivePort"
                    for _ in range(200):
                        if portfile.is_file():
                            break
                        assert p.poll() is None, (root / "client.log").read_text()[-2500:]
                        time.sleep(0.1)
                    assert portfile.is_file(), (root / "client.log").read_text()[-2500:]
                    port = int(portfile.read_text().splitlines()[0])
                    browser = api.chromium.connect_over_cdp("http://127.0.0.1:" + str(port))
                    page = browser.contexts[0].pages[0]
                    page.wait_for_function("() => !!document.body", timeout=20000)
                    page.wait_for_function("([bg, accent]) => { const s=getComputedStyle(document.body); return s.getPropertyValue('--background').trim()===bg && s.getPropertyValue('--accent').trim()===accent; }", arg=[palette["base"][0], palette["accent"]], timeout=20000)
                    actual = page.evaluate("() => getComputedStyle(document.body).colorScheme")
                    assert actual == palette["variant"], (source.name, actual)
                    print(source.name + ": native Heroic customCSS injection PASS", flush=True)
                    browser.close()
                finally:
                    try:
                        os.killpg(p.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                    p.wait(timeout=10)
