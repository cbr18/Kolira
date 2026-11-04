"""Palette page glue: every operation is the Rust CLI (`theme-profile palette …`).

Python only runs the binary and parses its output — the palette model,
generators, gallery and wallpaper extraction live in `rust_core`
(`palette.rs`, `gallery.rs`, `extract.rs`). See docs/PALETTE.md.
"""

from __future__ import annotations

import json
import subprocess
from typing import Any, Optional

from .cli_core import resolve_cli_bin
from .protocol import Result

#: Targets the CLI accepts for `palette apply` (kvantum switches widgetStyle).
TARGETS = ("kde", "kitty", "alacritty", "kvantum")
DEFAULT_TARGETS = ("kde", "kitty", "alacritty")


def _run(args: list[str], *, stdin: Optional[str] = None, timeout: int = 120) -> Result:
    try:
        proc = subprocess.run(
            [resolve_cli_bin(), *args],
            input=stdin, capture_output=True, text=True, timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return Result.err(str(exc))
    if proc.returncode != 0:
        return Result.err(proc.stderr.strip() or proc.stdout.strip() or "theme-profile failed")
    return Result.ok(proc.stdout.strip())


class PaletteCore:
    """Thin adapter over `theme-profile palette` / `wallpaper current`."""

    def show(self) -> Optional[dict[str, Any]]:
        """Live palette (with `warnings`, `kdeScheme`), None when there is none."""
        result = _run(["palette", "show", "--json"])
        if not result.success:
            return None
        try:
            data = json.loads(result.message)
        except ValueError:
            return None
        return data if isinstance(data, dict) else None

    def gallery(self, query: str = "", online: bool = False) -> Result:
        args = ["palette", "gallery"]
        if query.strip():
            args.append(query.strip())
        if online:
            args.append("--online")
        args.append("--json")
        result = _run(args, timeout=60)
        if not result.success:
            return result
        try:
            items = json.loads(result.message)
        except ValueError:
            return Result.err("theme-profile returned invalid JSON")
        return Result.ok(f"{len(items)} scheme(s)", data={"items": items})

    def import_gallery(self, scheme_id: str) -> Result:
        return _run(["palette", "import", f"gallery:{scheme_id}"])

    def import_file(self, path: str) -> Result:
        return _run(["palette", "import", path])

    def set_slots(self, slots: dict[str, str]) -> Result:
        if not slots:
            return Result.err("Nothing to set")
        return _run(["palette", "set", *(f"{k}={v}" for k, v in slots.items())])

    def from_colors(self, colors: list[dict[str, Any]], name: str = "", source: str = "") -> Result:
        """Kirigami.ImageColors swatches → palette (same rule as the CLI)."""
        payload: dict[str, Any] = {
            "colors": [
                {"color": str(c.get("color", "")), "ratio": float(c.get("ratio", 1.0))}
                for c in colors
            ]
        }
        if name:
            payload["name"] = name
        if source:
            payload["source"] = source
        return _run(["palette", "from-colors", "-"], stdin=json.dumps(payload))

    def from_wallpaper(self, image: str = "") -> Result:
        return _run(["palette", "from-wallpaper", *([image] if image else [])])

    def apply(self, targets: list[str], kvantum_base: str = "") -> Result:
        chosen = [t for t in targets if t in TARGETS]
        if not chosen:
            return Result.err("Select at least one target")
        args = ["palette", "apply", f"--targets={','.join(chosen)}"]
        if kvantum_base:
            args.append(f"--kvantum-base={kvantum_base}")
        return _run(args, timeout=180)

    def current_wallpaper(self) -> str:
        result = _run(["wallpaper", "current"], timeout=10)
        return result.message if result.success else ""
