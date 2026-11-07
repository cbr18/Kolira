#!/usr/bin/env python3
"""Do two HOME snapshots mean the same? Exit 1 with the differences if not.

Config files compare by meaning: key order and blank lines are not differences.
An explicit switch for a third-party KWin effect is not one either: a theme pins
the state the effect already had, so it behaves the same. Nor is an explicit
accentColorFromWallpaper=false, KDE's default (written to notify KDE's service).
"""
import json
import re
import sys

PINNED = {"better_blur_dxEnabled", "glassEnabled", "kwin4_effect_shapecornersEnabled", "forceblurEnabled"}


def ini(text):
    sections = {}
    group = ""
    for line in text.split("\n"):
        line = line.strip()
        if not line:
            continue
        if line.startswith("["):
            group = line
            sections.setdefault(group, {})
        elif "=" in line:
            key, value = line.split("=", 1)
            sections.setdefault(group, {})[key.strip()] = re.sub(r",\s+", ", ", value.strip())
    return {name: keys for name, keys in sections.items() if keys}


def main():
    with open(sys.argv[1], encoding="utf-8") as handle:
        before = json.load(handle)
    with open(sys.argv[2], encoding="utf-8") as handle:
        after = json.load(handle)
    problems = []
    for name in sorted(set(before) | set(after)):
        if name not in before:
            problems.append(f"left behind: {name}")
        elif name not in after:
            problems.append(f"lost: {name}")
        elif before[name] == after[name]:
            continue
        elif name.endswith("fish_variables"):
            if sorted(before[name].split("\n")) != sorted(after[name].split("\n")):
                problems.append(f"changed: {name}")
        elif name.endswith(("rc", ".ini", "kdeglobals", ".profile")):
            old, new = ini(before[name]), ini(after[name])
            for group in sorted(set(old) | set(new)):
                keys = set(old.get(group, {})) | set(new.get(group, {}))
                for key in sorted(keys):
                    a, b = old.get(group, {}).get(key), new.get(group, {}).get(key)
                    implied = key == "accentColorFromWallpaper" and a is None and b == "false"
                    if a != b and not implied and not (group == "[Plugins]" and a is None and key in PINNED):
                        problems.append(f"changed: {name} {group} {key}: {a!r} -> {b!r}")
        else:
            problems.append(f"changed: {name}")
    for problem in problems:
        print(f"  {problem}"[:170])
    return 1 if problems else 0


sys.exit(main())
