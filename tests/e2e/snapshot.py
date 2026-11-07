#!/usr/bin/env python3
"""Content of every file under a HOME, as JSON (theme-profile's own data excluded)."""
import json
import os
import sys

root, out = sys.argv[1:3]
files = {}
for folder, dirs, names in os.walk(root):
    rel = os.path.relpath(folder, root)
    skip = (".config/theme-profile", ".cache", ".local/share/kde-theme-manager", ".cargo", ".rustup", "kde-theme-manager", ".shim", ".config/dconf", ".local/state", ".config/session", ".config/KDE")
    if rel.startswith(skip):
        dirs[:] = []
        continue
    for name in names:
        path = os.path.join(folder, name)
        if os.path.isfile(path):
            with open(path, "rb") as handle:
                files[os.path.normpath(os.path.join(rel, name))] = handle.read().decode("utf-8", "replace")
with open(out, "w", encoding="utf-8") as handle:
    json.dump(files, handle)
