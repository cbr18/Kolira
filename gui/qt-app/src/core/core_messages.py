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
    return re.sub(r"%([1-9][0-9]*)", lambda m: str(args[int(m[1]) - 1])
                  if int(m[1]) <= len(args) else m[0], template)


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

# Known legacy reports are matched as complete templates. Structured app
# statuses use render_message directly; unknown diagnostics remain verbatim.
for pattern, template in (
    (r"^Settings file not found: (.+)$", "Settings file not found: %1"),
    (r"^(.+) does not expose the current theme$", "%1 does not expose the current theme"),
    (r"^(.+) themes cannot be enumerated$", "%1 themes cannot be enumerated"),
    (r"^(.+) themes cannot be applied automatically$", "%1 themes cannot be applied automatically"),
    (r"^(.+) theme set to (.+)$", "%1 theme set to %2"),
    (r"^Opened (.+)$", "Opened %1"), (r"^Launched (.+)$", "Launched %1"),
    (r"^Removed (.+)$", "Removed %1"), (r"^Reinstalled (.+)$", "Reinstalled %1"),
    (r"^Installed (.+) in (.+)$", "Installed %1 in %2"),
    (r"^Could not parse Discord settings.json: (.+)$", "Could not parse Discord settings.json: %1"),
):
    _PATTERNS.append((re.compile(pattern), lambda m, template=template: render_message(template, m.groups())))


def render_message(template: str, arguments=()) -> str:
    """Translate before a single substitution pass, preserving user placeholders."""
    return _fill(_t(template), *arguments)


def state_label(state: str) -> str:
    return _t({"preparing":"Preparing", "applying":"Applying", "recovering":"Recovering",
        "succeeded":"Completed", "cancelled_preparation":"Cancelled", "applied":"Applied",
        "committed":"Applied", "completed":"Completed", "rolled_back":"Rolled back",
        "recovery_failed":"Recovery failed", "failed_preparation":"Preparation failed",
        "failed":"Failed", "cancelled":"Cancelled", "skipped":"Skipped", "running":"Running",
        "pending":"Pending", "interrupted":"Interrupted", "restored":"Restored"}.get(state,state))


def source_label(source: str) -> str:
    tag, separator, detail = source.partition(":")
    label = _t({"gui":"Application", "cli":"Command line", "mcp":"MCP", "schedule":"Schedule",
        "wallpaper":"Wallpaper", "image":"Image", "manual":"Manual", "import":"Imported",
        "gallery":"Gallery", "file":"File", "colors":"Colours", "kirigami":"Plasma colours",
        "imagecolors":"Plasma colours", "bundled":"Built-in"}.get(tag,tag))
    return label + (": " + detail if separator else "")


_PREFIX = re.compile(r"^([✓✗] )?(.*)$", re.S)


def localize_line(line: str) -> str:
    match = _PREFIX.match(line)
    prefix, body = (match.group(1) or "", match.group(2)) if match else ("", line)
    for pattern, build in _PATTERNS:
        m = pattern.match(body)
        if m:
            return prefix + build(m)
    contrast = re.fullmatch(r"(text on window|text on view|selected text): contrast ([0-9.]+):1 < ([0-9.]+):1", body)
    if contrast:
        return prefix + render_message("%1: contrast %2:1 < %3:1", [_t(contrast[1]), contrast[2], contrast[3]])
    skipped = re.fullmatch(r"([^:]+): skipped, (.+) not found", body)
    if skipped:
        return prefix + render_message("%1: skipped, %2 not found", skipped.groups())
    line = _t(body)
    for suffix in (" (no running kitty to reload)", " — plasma-apply-colorscheme failed"):
        if line.endswith(suffix): line = line[:-len(suffix)] + _t(suffix)
    return prefix + line


def localize(message: str) -> str:
    """Translate every known line of a (multi-line) core message."""
    return "\n".join(localize_line(line) for line in message.split("\n"))


def localize_inspector(message: str) -> str:
    """Render Rust Inspector messages using the existing GUI translation context."""
    def translate(text):
        return QCoreApplication.translate("AppThemeCore", text)
    patterns = (
        (r"^(.+) staged — press Apply$", "%s staged — press Apply", True),
        (r"^Unstaged: (.+)$", "Unstaged: %s", True),
        (r"^Undone: (.+)$", "Undone: %s", True),
        (r"^Discarded ([0-9]+) staged change\(s\)$", "Discarded %s staged change(s)", False),
        (r"^Reverted ([0-9]+) file\(s\)$", "Reverted %s file(s)", False),
    )
    def part(text):
        applied = re.match(r"^Applied ([0-9]+) change\(s\)(.*)$", text)
        if applied:
            return (translate("Applied %s change(s)") % applied.group(1)) + applied.group(2)
        for pattern, template, label in patterns:
            match = re.match(pattern, text)
            if match:
                value = match.group(1)
                if label:
                    value = QCoreApplication.translate("ThemeFields", value)
                return translate(template) % value
        return translate(text)
    return "; ".join(part(text) for text in message.split("; "))
