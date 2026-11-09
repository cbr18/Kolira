#!/usr/bin/env python3
"""Real provider lifecycle, isolated HOME/XDG. Package mode belongs in a container.

Usage: python dependency-cycle.py CLI zed
       python dependency-cycle.py CLI kde STORE_ID COLOUR_SCHEME_NAME
       python dependency-cycle.py CLI package PACKAGE BINARY
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

cli = str(Path(sys.argv[1]).resolve())
mode = sys.argv[2]
work = Path(tempfile.mkdtemp(prefix="ktm-real-dependencies-"))
env = dict(os.environ)
for key, rel in [("HOME", "home"), ("XDG_CONFIG_HOME", "config"),
                 ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache")]:
    path = work / rel
    path.mkdir()
    env[key] = str(path)
for key in ("KTM_THEME_NO_APPLY", "KTM_OFFLINE", "KTM_DEP_OPERATION", "KTM_DEP_EVENTS"):
    env.pop(key, None)
root = work / "config/kde-theme-profile"


def run(*args, ok=True):
    result = subprocess.run([cli, *args], env=env, capture_output=True, text=True, timeout=600)
    with (work / "commands.log").open("a") as log:
        log.write(f"{args}\n{result.stdout}\n{result.stderr}\nexit={result.returncode}\n")
    if ok:
        assert result.returncode == 0, (args, result.stdout, result.stderr)
    return result


def rows():
    return json.loads(run("dependencies", "list").stdout)


if mode == "zed":
    report = json.loads(run("apps", "store-install", "zed", "everforest", "--json").stdout)
    assert report["themes"]
    dependency = dict(kind="zed", name=report["themes"][0], source="zed", id="everforest", exact=True)
elif mode == "kde":
    dependency = dict(kind="color-scheme", name=sys.argv[4], source="kde-store", id=sys.argv[3], exact=True)
elif mode == "package":
    assert os.environ.get("KTM_DEPENDENCY_CONTAINER") == "1", "Package lifecycle requires the test container"
    dependency = dict(kind="command", name=sys.argv[4], source="package", id=sys.argv[3], exact=True)
    # Terminal transport only: pacman/sudo below are real. The adapter supplies
    # answers in a disposable container and preserves their output for review.
    bin_dir = work / "bin"
    bin_dir.mkdir()
    terminal = bin_dir / "konsole"
    terminal.write_text(f"#!{sys.executable}\n" + """import os, subprocess, sys
from pathlib import Path
args=sys.argv[1:]
args=args[args.index('-e')+1:]
with (Path(os.environ['HOME'])/'package-terminal.log').open('a') as log:
    sys.exit(subprocess.run(args,input='\\n'*12,text=True,stdout=log,stderr=log).returncode)
""")
    terminal.chmod(0o755)
    env["PATH"] = f"{bin_dir}:" + env["PATH"]
else:
    raise SystemExit("unknown provider")

for name in ("first", "second"):
    path = root / "themes" / name
    path.mkdir(parents=True)
    (path / "theme.json").write_text(json.dumps({"schema": "theme-profile/theme@2", "name": name, "dependencies": [dependency]}))

row = next(r for r in rows() if r["dependency"]["id"] == dependency["id"])
resource_id = row["id"]
assert row["themes"] == ["first", "second"]
run("dependencies", "install", resource_id)
row = next(r for r in rows() if r["id"] == resource_id)
assert row["installed"] and row["can_remove"], row
assert run("dependencies", "remove", resource_id, ok=False).returncode != 0
run("dependencies", "remove", resource_id, "--confirm-shared")
assert not next(r for r in rows() if r["id"] == resource_id)["installed"]
run("dependencies", "install", resource_id)
run("dependencies", "reinstall", resource_id, "--confirm-shared")
matches = [r for r in rows() if r["id"] == resource_id]
assert len(matches) == 1 and matches[0]["installed"] and matches[0]["can_remove"]
run("dependencies", "remove", resource_id, "--confirm-shared")
assert not next(r for r in rows() if r["id"] == resource_id)["installed"]
(work / "result.json").write_text(json.dumps({"provider": mode, "dependency": dependency, "resource": resource_id, "passed": True}, indent=2))
print(f"PASS {mode}: install/remove/reinstall; shared resource; no duplicates. Evidence: {work}")
