"""Shared pytest setup.

Point the GUI at the freshly built Rust CLI (gui/qt-app/src/core/rust_core/
target/release/kolira) when it exists, so tests exercise the code in
this checkout rather than whatever version is installed in ~/.local/bin.
"""

import os
from pathlib import Path

_RELEASE_CLI = (
    Path(__file__).resolve().parent.parent
    / "src" / "core" / "rust_core" / "target" / "release" / "kolira"
)

# Tests never reach the network (saving a theme would search the KDE Store
# for dependency sources).
os.environ["KTM_OFFLINE"] = "1"

if _RELEASE_CLI.is_file() and "KTM_THEME_PROFILE_BIN" not in os.environ:
    os.environ["KTM_THEME_PROFILE_BIN"] = str(_RELEASE_CLI)


import pytest


@pytest.fixture(autouse=True)
def isolated_core_home(tmp_path, monkeypatch):
    """Every Rust-backed test owns HOME and XDG; never write the host's data."""
    monkeypatch.setenv("HOME", str(tmp_path))
    for variable in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"):
        monkeypatch.delenv(variable, raising=False)
    monkeypatch.setenv("KTM_THEME_NO_APPLY", "1")
    monkeypatch.setenv("KTM_OFFLINE", "1")
    from core.protocol import CoreFactory
    CoreFactory.reset()
    yield
    CoreFactory.reset()
