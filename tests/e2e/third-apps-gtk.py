#!/usr/bin/env python3
"""Parse every bundled GIMP colour resource using installed GTK3.

Requires python-gobject, GTK3 and a display (xvfb-run is sufficient).
"""
from pathlib import Path
import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gtk

repo = Path(__file__).resolve().parents[2]
Gtk.init([])
for source in sorted((repo / "themes").iterdir()):
    css = source / ".config/GIMP/3.0/themes/Kolira/gimp.css"
    if not css.exists():
        continue
    errors = []
    provider = Gtk.CssProvider()
    provider.connect("parsing-error", lambda _p, _s, error: errors.append(str(error)))
    provider.load_from_path(str(css))
    assert not errors, (source.name, errors)
    style = Gtk.StyleContext()
    style.add_provider(provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    for key in ["fg-color", "bg-color", "selected-color", "accent_bg_color", "accent_fg_color"]:
        found, color = style.lookup_color(key)
        assert found, (source.name, key)
    print(source.name + ": GIMP native GTK CSS PASS", flush=True)
