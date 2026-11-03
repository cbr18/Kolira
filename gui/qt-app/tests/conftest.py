"""Shared pytest setup.

Point the GUI at the freshly built Rust CLI (gui/qt-app/src/core/rust_core/
target/release/theme-profile) when it exists, so tests exercise the code in
this checkout rather than whatever version is installed in ~/.local/bin.
"""

import os
from pathlib import Path

_RELEASE_CLI = (
    Path(__file__).resolve().parent.parent
    / "src" / "core" / "rust_core" / "target" / "release" / "theme-profile"
)

if _RELEASE_CLI.is_file() and "KTM_THEME_PROFILE_BIN" not in os.environ:
    os.environ["KTM_THEME_PROFILE_BIN"] = str(_RELEASE_CLI)
