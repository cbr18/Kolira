#!/usr/bin/env python3
"""Probe native BrowserThemeColor apply/return in a disposable Docker container.

This is a research regression, not an installed integration. It deliberately
returns failure when Chromium loses a GTK/Qt baseline; do not weaken that check.
Writes only the container policy fixture, never Chromium Preferences. See
tasks/2026_11_16_chromium-policy-sync.md for invocation and known failures.
"""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time

from playwright.sync_api import sync_playwright

BASELINES = ("default", "extension", "gtk", "qt", "user-dark", "user-light")
REPO = Path("/src")
BINARY = "/usr/lib/chromium/chromium"
POLICY = Path("/etc/chromium/policies/managed/kolira-appearance.json")


def write_policy(value):
    if value is None:
        POLICY.unlink(missing_ok=True)
        return
    assert set(value) == {"BrowserThemeColor"}
    assert re.fullmatch(r"#[0-9a-fA-F]{6}", value["BrowserThemeColor"])
    with tempfile.NamedTemporaryFile(dir=POLICY.parent, delete=False) as tmp:
        tmp.write(json.dumps(value).encode())
        tmp.flush()
        os.fchmod(tmp.fileno(), 0o644)
        os.fsync(tmp.fileno())
    os.replace(tmp.name, POLICY)


def prefs(page):
    page.goto("chrome://prefs-internals/?kolira_check=" + str(time.monotonic_ns()))
    page.wait_for_function("() => document.body.innerText.trim().startsWith('{')")
    return json.loads(page.locator("body").inner_text())


def colors(page):
    page.goto("chrome://new-tab-page")
    page.wait_for_function("""() => document.body &&
        getComputedStyle(document.body).backgroundColor !== 'rgba(0, 0, 0, 0)'""")
    return page.evaluate("""() => {
        const s = getComputedStyle(document.body);
        return {background: s.backgroundColor, color: s.color, scheme: s.colorScheme};
    }""")


def refresh(home):
    result = subprocess.run(
        [BINARY, "--no-sandbox", "--ozone-platform=x11", "--user-data-dir=" + str(home),
         "--profile-directory=Default", "--refresh-platform-policy", "--no-startup-window"],
        capture_output=True, timeout=15, user=1000, group=1000,
    )
    assert result.returncode == 0, result.stderr[-2000:]


def theme_values(data):
    result = {}

    def walk(value, path=""):
        if not isinstance(value, dict):
            return
        if "metadata" in value and "value" in value:
            if "theme" in path or "color_scheme" in path:
                result[path] = value
        else:
            for key, child in value.items():
                walk(child, path + "." + key if path else key)

    walk(data)
    return result


def values(data):
    return {key: item["value"] for key, item in data.items()}


def select_baseline(page, baseline):
    if baseline in ("gtk", "qt"):
        page.goto("chrome://settings/appearance")
        page.evaluate("name => chrome.send(name)", "useGtkTheme" if baseline == "gtk" else "useQtTheme")
        time.sleep(0.3)
    if baseline.startswith("user-"):
        page.goto("chrome://settings/appearance")
        value = 2 if baseline == "user-dark" else 1
        # Use the actual Appearance control; these prefs are not exported by
        # settingsPrivate on current Chromium. Playwright pierces shadow roots.
        page.locator("#colorSchemeModeSelect").select_option(str(value))
        time.sleep(0.3)



def check_baseline(api, baseline):
    with tempfile.TemporaryDirectory(prefix="kolira-policy-") as tmp:
        root = Path(tmp)
        root.chmod(0o777)  # Browser runs as unprivileged UID 1000, writer as container root.
        home, extension = root / "browser", root / "foreign-theme"
        extension.mkdir()
        (extension / "manifest.json").write_text(json.dumps({
            "manifest_version": 3, "name": "Personal baseline test", "version": "1.0",
            "theme": {"colors": {"frame": [78, 35, 149], "frame_inactive": [80, 50, 120]}},
        }))
        args = [BINARY, "--ozone-platform=x11", "--no-sandbox", "--disable-gpu",
                "--no-first-run", "--disable-default-apps", "--no-default-browser-check",
                "--remote-debugging-port=0", "--user-data-dir=" + str(home)]
        if baseline == "extension":
            args.append("--load-extension=" + str(extension))
        args += ["--profile-directory=Default", "about:blank"]
        with (root / "browser.log").open("wb") as log:
            process = subprocess.Popen(
                args, stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
                user=1000, group=1000, env=dict(os.environ, HOME=str(root)),
            )
            try:
                portfile = home / "DevToolsActivePort"
                deadline = time.monotonic() + 15
                while not portfile.exists() and time.monotonic() < deadline:
                    assert process.poll() is None, (root / "browser.log").read_text()[-2500:]
                    time.sleep(0.1)
                port = int(portfile.read_text().splitlines()[0])
                browser = api.chromium.connect_over_cdp("http://127.0.0.1:" + str(port))
                page = browser.contexts[0].pages[0]
                time.sleep(6)  # Let Chromium finish first-run preference migrations.
                select_baseline(page, baseline)
                before = theme_values(prefs(page))
                before_colors = colors(page)
                if baseline == "extension":
                    assert before["extensions.theme.id"]["value"], "Baseline extension did not load"
                if baseline in ("gtk", "qt"):
                    assert before["extensions.theme.system_theme"]["value"] > 0, "System theme did not activate"
                if baseline.startswith("user-"):
                    expected_mode = 2 if baseline == "user-dark" else 1
                    assert any(before[key]["value"] == expected_mode for key in
                               ("browser.theme.color_scheme", "browser.theme.color_scheme2"))
                print("BASELINE", baseline, "PASS", flush=True)
                for source in sorted((REPO / "themes").iterdir()):
                    if not (source / "theme.json").is_file():
                        continue
                    palette = json.loads((source / "theme.json").read_text())["palette"]
                    write_policy({"BrowserThemeColor": palette["accent"]})
                    refresh(home)
                    deadline = time.monotonic() + 12
                    while True:
                        now = theme_values(prefs(page))
                        managed = now["autogenerated.theme.policy.color"]
                        if any("managed" in item for item in managed["metadata"]):
                            break
                        assert time.monotonic() < deadline, "Colour policy did not apply"
                        time.sleep(0.25)
                    expected = int(palette["accent"][1:], 16) | 0xff000000
                    if expected >= 1 << 31:
                        expected -= 1 << 32
                    assert managed["value"] == expected, (source.name, managed, expected)
                    assert colors(page) != before_colors, "Rendered native colours did not change"
                    print("POLICY", baseline, source.name, "PASS", flush=True)
                    write_policy(None)
                    refresh(home)
                    deadline = time.monotonic() + 12
                    while True:
                        restored = theme_values(prefs(page))
                        unmanaged = not any("managed" in item for item in
                                            restored["autogenerated.theme.policy.color"]["metadata"])
                        if values(restored) == values(before) and unmanaged:
                            break
                        if time.monotonic() >= deadline:
                            changed = {key: (old, values(restored).get(key))
                                       for key, old in values(before).items()
                                       if values(restored).get(key) != old}
                            raise AssertionError((baseline, source.name, "baseline lost", changed))
                        time.sleep(0.25)
                    assert colors(page) == before_colors, (baseline, source.name, "native colours not restored")
                    print("RETURN", baseline, source.name, "PASS", flush=True)
                browser.close()
            finally:
                write_policy(None)
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baselines", nargs="+", choices=BASELINES, default=list(BASELINES))
    args = parser.parse_args()
    assert Path("/.dockerenv").is_file() and os.environ.get("KOLIRA_POLICY_NATIVE_CONTAINER") == "1", "Disposable Docker only"
    assert os.geteuid() == 0, "Container root writer required"
    os.umask(0o022)
    assert not POLICY.exists() and not POLICY.is_symlink(), "Policy slot must initially be absent"
    POLICY.parent.mkdir(parents=True, exist_ok=True)
    print(subprocess.check_output([BINARY, "--version"], text=True).strip(), flush=True)
    failures = []
    with sync_playwright() as api:
        for baseline in args.baselines:
            try:
                check_baseline(api, baseline)
            except Exception as error:
                failures.append(baseline)
                print("FAIL", baseline, str(error), flush=True)
    print("FAILED BASELINES:", ", ".join(failures) or "none", flush=True)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
