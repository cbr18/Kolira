"""Path and profile-name helpers shared by the CLI core.

The application supports the XDG base directories; data lives in
``$XDG_CONFIG_HOME/kde-theme-profile/themes`` and older folder names are moved
there on start.  Profile names are validated here as a
single, reusable rule; the same rule is implemented in the Rust CLI and the Python CLI core.
"""

from __future__ import annotations

import os
import re
from pathlib import Path

# A profile name is a single path component.  It must not start with a dot so
# that ``.``/``..`` can never be selected, and the allowed set excludes every
# path separator and shell metacharacter.
_PROFILE_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")


def home_dir() -> Path:
    return Path.home()


def config_home() -> Path:
    value = os.environ.get("XDG_CONFIG_HOME", "").strip()
    return Path(value).expanduser() if value else home_dir() / ".config"


def cache_home() -> Path:
    value = os.environ.get("XDG_CACHE_HOME", "").strip()
    return Path(value).expanduser() if value else home_dir() / ".cache"


def data_home() -> Path:
    value = os.environ.get("XDG_DATA_HOME", "").strip()
    return Path(value).expanduser() if value else home_dir() / ".local/share"


_DATA_DIR = "kde-theme-profile"
_OLD_DATA_DIR = "theme-profile"


def _migrate_old_names(old_roots=None, new_root=None) -> None:
    """Compatibility entry point; Rust owns the canonical migration."""
    from .cli_core import core_request
    core_request("paths.migrate")


def theme_root() -> Path:
    """Resolve the GUI's data location; Rust migrates data on operations."""
    preferred = config_home() / _DATA_DIR
    in_dot_config = home_dir() / ".config" / _DATA_DIR
    return preferred if preferred.exists() or not in_dot_config.exists() else in_dot_config


def profiles_dir() -> Path:
    return theme_root() / "themes"


def active_profile_file() -> Path:
    return theme_root() / "active-profile"


def settings_file() -> Path:
    return theme_root() / "app-preferences.json"


def live_path(relative: str) -> Path:
    """Resolve a profile-relative path against the live XDG environment.

    Profile snapshots keep the historical ``.config/...`` layout.  Live files
    must be resolved through the XDG variables instead.  Paths outside the
    known XDG prefixes (Zen profiles, ``obsidian/vault``) stay relative to the
    home directory, exactly like in the Rust CLI.
    """
    path = Path(relative)
    if path.is_absolute():
        return path
    parts = path.parts
    if parts[:1] == (".config",):
        return config_home() / Path(*parts[1:])
    if parts[:2] == (".local", "share"):
        return data_home() / Path(*parts[2:])
    if parts[:1] == (".cache",):
        return cache_home() / Path(*parts[1:])
    return home_dir() / path


def is_valid_profile_name(name: object) -> bool:
    if not isinstance(name, str):
        return False
    return bool(_PROFILE_NAME_RE.fullmatch(name))


def validate_profile_name(name: object) -> str:
    """Return a safe profile name or raise :class:`ValueError`."""
    if not isinstance(name, str) or not is_valid_profile_name(name):
        raise ValueError(
            "Invalid profile name: must match [A-Za-z0-9][A-Za-z0-9._-]{0,63}"
        )
    return name


def profile_dir(name: object) -> Path:
    """Absolute profile directory, guaranteed to stay under ``themes/``."""
    safe = validate_profile_name(name)
    root = profiles_dir().resolve()
    candidate = (root / safe).resolve()
    if candidate.parent != root:
        raise ValueError("Invalid profile name: path escapes profiles directory")
    return candidate
