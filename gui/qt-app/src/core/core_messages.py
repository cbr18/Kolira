"""Russian (and any other) wording for the Rust core's report lines.

The CLI speaks English (scripts parse it); the GUI shows its messages to
people. Known message shapes are matched here and rebuilt from translatable
templates (context ``CoreMessages``); anything else passes through unchanged.
"""

from __future__ import annotations

import re
from typing import Callable

from PySide6.QtCore import QCoreApplication


def _t(text: str) -> str:
    return QCoreApplication.translate("CoreMessages", text)


def _fill(template: str, *args: str) -> str:
    out = template
    for i, value in enumerate(args, start=1):
        out = out.replace(f"%{i}", value)
    return out


_PATTERNS: list[tuple[re.Pattern[str], Callable[[re.Match[str]], str]]] = [
    (re.compile(r"^installed (.+?) into (.+?): (.+)$"),
     lambda m: _fill(_t("installed %1 into %2: %3"), *m.groups())),
    (re.compile(r"^installed (.+?) \((.+?)\) with kpackagetool6$"),
     lambda m: _fill(_t("installed %1 (%2)"), *m.groups())),
    (re.compile(r"^installed (.+?) for (.+?) — themes: (.+)$"),
     lambda m: _fill(_t("installed %1 for %2 — themes: %3"), *m.groups())),
    (re.compile(r"^installed (\S+) for (\S+)$"),
     lambda m: _fill(_t("installed %1 for %2"), *m.groups())),
    (re.compile(r"^(.+?) — opened (\S+): (.+)$"),
     lambda m: _fill(_t("%1 — opened %2 with: %3"), *m.groups())),
    (re.compile(r"^AUR packages need an AUR helper \(yay or paru\): install one first$"),
     lambda m: _t("AUR packages need an AUR helper (yay or paru): install one first")),
    (re.compile(r"^no terminal found \(.*\) — run: (.+)$"),
     lambda m: _fill(_t("no terminal found — run: %1"), *m.groups())),
    (re.compile(r"^(.+?) restarted \((.+?)\)$"),
     lambda m: _fill(_t("%1 restarted"), m.group(1))),
    (re.compile(r"^(.+?) is not running$"),
     lambda m: _fill(_t("%1 is not running"), m.group(1))),
    (re.compile(r"^it did not quit within (\d+) s — close it yourself$"),
     lambda m: _fill(_t("it did not quit within %1 s — close it yourself"), m.group(1))),
    (re.compile(r"^(.+?), but \"(.+?)\" is still not found — check (.+)$"),
     lambda m: _fill(_t("%1, but \"%2\" is still not found — check %3"), *m.groups())),
    (re.compile(r"^Applied profile: (.+)$"),
     lambda m: _fill(_t("Applied profile: %1"), m.group(1))),
    (re.compile(r"^All dependencies are installed\.$"),
     lambda m: _t("All dependencies are installed.")),
    (re.compile(r"^Nothing to do\.$"),
     lambda m: _t("Nothing to do.")),
]

_PREFIX = re.compile(r"^([✓✗] )?(.*)$", re.S)


def localize_line(line: str) -> str:
    match = _PREFIX.match(line)
    prefix, body = (match.group(1) or "", match.group(2)) if match else ("", line)
    for pattern, build in _PATTERNS:
        m = pattern.match(body)
        if m:
            return prefix + build(m)
    return line


def localize(message: str) -> str:
    """Translate every known line of a (multi-line) core message."""
    return "\n".join(localize_line(line) for line in message.split("\n"))
