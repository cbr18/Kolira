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


def _move_children(source: Path, target: Path) -> None:
    try:
        entries = list(source.iterdir())
        target.mkdir(parents=True, exist_ok=True)
    except OSError:
        return
    for entry in entries:
        destination = target / entry.name
        try:
            if not destination.exists():
                entry.rename(destination)
            elif entry.is_dir() and destination.is_dir() and entry.name == "backups":
                _move_children(entry, destination)
                entry.rmdir()
        except OSError:
            pass


def _remove_if_empty(path: Path) -> None:
    try:
        path.rmdir()
    except OSError:
        pass


def _migrate_old_names(old_roots: list[Path], new_root: Path) -> None:
    """Twin of ``paths::migrate_old_names`` in the Rust core: same moves, same rules."""
    for old in old_roots:
        if old.is_dir() and old != new_root:
            _move_children(old, new_root)
            _remove_if_empty(old)
    old_themes, themes = new_root / "profiles", new_root / "themes"
    if old_themes.is_dir():
        _move_children(old_themes, themes)
        _remove_if_empty(old_themes)
    if themes.is_dir():
        for theme in themes.iterdir():
            inner = theme / ".config" / _OLD_DATA_DIR
            if inner.is_dir():
                _move_children(inner, theme / ".config" / _DATA_DIR)
                _remove_if_empty(inner)


def theme_root() -> Path:
    """Data folder; data under the pre-0.6.0 names is moved here first."""
    preferred = config_home() / _DATA_DIR
    in_dot_config = home_dir() / ".config" / _DATA_DIR
    # Tests read this path, some with the real HOME: they must not move real data.
    if "PYTEST_CURRENT_TEST" not in os.environ:
        _migrate_old_names([config_home() / _OLD_DATA_DIR, home_dir() / ".config" / _OLD_DATA_DIR], preferred)
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
