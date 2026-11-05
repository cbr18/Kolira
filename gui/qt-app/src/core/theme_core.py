"""Python implementation of the Theme Inspector core.

Reads the curated fields from :mod:`core.theme_schema`, writes them back with
the right mechanism (KConfig, atomic file patch, or a native CLI), validates
the values and keeps an undo snapshot for the current session.

Design notes
------------
* Reads are cheap and side-effect free; writes only touch the ``live`` scope.
* KDE/KConfig files are written through ``kwriteconfig6`` (escaping + types).
* JSON/JSONC files are patched by value position, so comments, ordering and
  trailing commas survive — nothing is re-serialized.
* Every write is recorded in an in-memory undo stack and the first write of a
  session copies the affected file to ``~/.config/theme-profile/backups/``.
* External commands and D-Bus hooks are best-effort; a failed hook does not
  roll the config back, it is reported in the result message.
* ``KTM_THEME_NO_APPLY=1`` disables external commands/hooks (tests, dry runs).
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import tomllib
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable, Optional

from .fileutil import atomic_write_bytes, atomic_write_text, copy_preserving_mode


# Fields the Rust core mirrors into GTK 3 and GTK 4 settings.ini (field.rs
# GTK_MIRROR): their undo snapshot includes those files.
GTK_MIRROR_FIELDS = frozenset({"gtk_theme", "gtk_dark", "icon_theme", "cursor_theme"})
GTK_SETTINGS = (".config/gtk-3.0/settings.ini", ".config/gtk-4.0/settings.ini")


def tr(text: str) -> str:
    """Перевод пользовательских сообщений ядра.

    pylupdate6 извлекает вызовы ``tr(...)`` под контекстом *класса*, в котором
    они стоят (все — в ``AppThemeCore``), поэтому и искать перевод нужно под
    ним; с контекстом "ThemeCore" сообщения ядра никогда не переводились.
    """
    from PySide6.QtCore import QCoreApplication

    return QCoreApplication.translate("AppThemeCore", text)

from .paths import (
    active_profile_file,
    home_dir,
    live_path,
    profile_dir,
    profiles_dir,
    theme_root,
    validate_profile_name,
)
from .protocol import (
    Choice,
    Result,
    ThemeCore,
    ThemeFieldValue,
    ThemeIssue,
    ThemeSection,
)
from .theme_schema import FIELDS, FIELD_BY_ID, SECTIONS, FieldDef

# Files written through kwriteconfig6 (KConfig escaping/typing).
_KDE_FILES = {
    ".config/kdeglobals",
    ".config/kwinrc",
    ".config/kwinrulesrc",
    ".config/plasmarc",
    ".config/plasmashellrc",
    ".config/kcminputrc",
    ".config/klassy/klassyrc",
    ".config/klassy/windecopresetsrc",
}

_KVANTUM_BASES = (".config/Kvantum", "/usr/share/Kvantum")
_NO_APPLY = "KTM_THEME_NO_APPLY"

# Choices providers that point at installed assets: a value outside the list
# is a real problem (e.g. a removed color scheme).
_ASSET_CHOICES = {
    "color_schemes",
    "plasma_themes",
    "look_and_feel",
    "icon_themes",
    "cursor_themes",
    "gtk_themes",
    "kvantum_themes",
    "btop_themes",
}


# ─────────────────────────────────────────────────────────────────────────────
# Small helpers
# ─────────────────────────────────────────────────────────────────────────────

def _home() -> Path:
    return home_dir()


def _now_stamp() -> str:
    return datetime.now().strftime("%Y%m%d-%H%M%S")


def _read_text(path: Path) -> Optional[str]:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None


def _atomic_write(path: Path, text: str) -> None:
    atomic_write_text(path, text)


def _atomic_write_bytes(path: Path, data: bytes) -> None:
    atomic_write_bytes(path, data)


def _resolve_under(base: Path, relative: str) -> Path:
    """Resolve a profile-relative path for a live or snapshot base."""
    if base == _home():
        return live_path(relative)
    return base / relative


def _snapshot_relative(path: Path) -> Path:
    """Stable snapshot key even when XDG points outside ``$HOME``."""
    try:
        return path.relative_to(_home())
    except ValueError:
        return Path("_external") / Path(*path.parts[1:])


_PACKAGE_CACHE: dict[Path, tuple[float, dict]] = {}


def _package_values(profile: Path) -> Optional[dict]:
    """``values`` of a theme package (``<profile>/theme.json``), else None.

    Cached by mtime: profile cards read a dozen fields per profile.
    """
    manifest = profile / "theme.json"
    try:
        mtime = manifest.stat().st_mtime
    except OSError:
        return None
    cached = _PACKAGE_CACHE.get(manifest)
    if cached and cached[0] == mtime:
        return cached[1]
    try:
        values = dict(json.loads(manifest.read_text(encoding="utf-8")).get("values") or {})
    except (OSError, ValueError):
        return None
    _PACKAGE_CACHE[manifest] = (mtime, values)
    return values


def _dedupe(items: Iterable[str]) -> list[str]:
    seen: set[str] = set()
    out: list[str] = []
    for item in items:
        if item and item not in seen:
            seen.add(item)
            out.append(item)
    return out


def _as_bool(raw: Any) -> Optional[bool]:
    if isinstance(raw, bool):
        return raw
    if raw is None:
        return None
    value = str(raw).strip().lower()
    if value in ("true", "1", "yes", "on"):
        return True
    if value in ("false", "0", "no", "off"):
        return False
    return None


def _fmt_number(value: float) -> str:
    if float(value).is_integer():
        return str(int(value))
    return f"{value:g}"


# ─────────────────────────────────────────────────────────────────────────────
# INI
# ─────────────────────────────────────────────────────────────────────────────

def _parse_ini(text: str) -> dict[str, dict[str, str]]:
    data: dict[str, dict[str, str]] = {}
    group = ""
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or stripped.startswith(";"):
            continue
        if stripped.startswith("[") and stripped.endswith("]"):
            group = stripped[1:-1]
            data.setdefault(group, {})
            continue
        if "=" in stripped:
            key, value = stripped.split("=", 1)
            data.setdefault(group, {})[key.strip()] = value.strip()
    return data


def _ini_get(text: str, group: str, key: str) -> Optional[str]:
    return _parse_ini(text).get(group, {}).get(key)


def _set_ini_text(text: str, group: str, key: str, value: str) -> str:
    """Line-preserving INI update (creates group/key when missing)."""
    lines = text.splitlines() if text else []
    current = ""
    group_header: Optional[int] = None
    group_end = len(lines)
    key_line: Optional[int] = None

    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            name = stripped[1:-1]
            if current == group and group_header is not None and group_end == len(lines):
                group_end = index
            if name == group:
                group_header = index
                current = name
            else:
                current = name
            continue
        if current == group and "=" in stripped:
            if stripped.split("=", 1)[0].strip() == key:
                key_line = index

    if key_line is not None:
        indent = lines[key_line][: len(lines[key_line]) - len(lines[key_line].lstrip())]
        lines[key_line] = f"{indent}{key}={value}"
        return "\n".join(lines) + "\n"

    if group_header is not None:
        insert_at = group_end if group_end != len(lines) else len(lines)
        lines.insert(insert_at, f"{key}={value}")
        return "\n".join(lines) + "\n"

    if group:
        if lines and lines[-1].strip():
            lines.append("")
        lines.extend([f"[{group}]", f"{key}={value}"])
    elif lines:
        lines.append(f"{key}={value}")
    else:
        lines = [f"{key}={value}"]
    return "\n".join(lines) + "\n"


# ─────────────────────────────────────────────────────────────────────────────
# JSON / JSONC: read and patch values without re-serializing the file
# ─────────────────────────────────────────────────────────────────────────────

def _json_positions(text: str) -> dict[tuple[str, ...], tuple[int, int]]:
    """Map every scalar path to its ``(start, end)`` slice in ``text``.

    Comments and trailing commas are skipped, so JSONC settings keep their
    formatting when patched.
    """
    positions: dict[tuple[str, ...], tuple[int, int]] = {}
    length = len(text)

    def skip_ws(index: int) -> int:
        while index < length:
            char = text[index]
            if char in " \t\r\n":
                index += 1
            elif text.startswith("//", index):
                newline = text.find("\n", index)
                index = length if newline < 0 else newline + 1
            elif text.startswith("/*", index):
                end = text.find("*/", index + 2)
                index = length if end < 0 else end + 2
            else:
                break
        return index

    def parse_string(index: int) -> tuple[str, int]:
        index += 1
        start = index
        while index < length:
            if text[index] == "\\":
                index += 2
                continue
            if text[index] == '"':
                return text[start:index], index + 1
            index += 1
        return text[start:], length

    def parse_value(index: int, path: tuple[str, ...]) -> int:
        index = skip_ws(index)
        if index >= length:
            return length
        char = text[index]
        if char == "{":
            index += 1
            while True:
                index = skip_ws(index)
                if index >= length:
                    return length
                if text[index] == "}":
                    return index + 1
                if text[index] != '"':
                    return length
                key, index = parse_string(index)
                index = skip_ws(index)
                if index < length and text[index] == ":":
                    index += 1
                index = parse_value(index, path + (key,))
                index = skip_ws(index)
                if index < length and text[index] == ",":
                    index += 1
                elif index < length and text[index] == "}":
                    return index + 1
                elif index >= length:
                    return length
        elif char == "[":
            index += 1
            item = 0
            while True:
                index = skip_ws(index)
                if index >= length:
                    return length
                if text[index] == "]":
                    return index + 1
                index = parse_value(index, path + (str(item),))
                item += 1
                index = skip_ws(index)
                if index < length and text[index] == ",":
                    index += 1
                elif index < length and text[index] == "]":
                    return index + 1
                elif index >= length:
                    return length
        elif char == '"':
            start = index
            _, index = parse_string(index)
            positions[path] = (start, index)
            return index
        else:
            start = index
            while index < length and text[index] not in ",}] \t\r\n":
                index += 1
            positions[path] = (start, index)
            return index

    parse_value(skip_ws(0), ())
    return positions


def _json_read(text: Optional[str], path: tuple[str, ...]) -> Any:
    if not text:
        return None
    positions = _json_positions(text)
    span = positions.get(path)
    if span is None:
        return None
    fragment = text[span[0]:span[1]]
    try:
        return json.loads(fragment)
    except json.JSONDecodeError:
        return fragment.strip('"')


def _coerce_json_value(old: Any, raw: str) -> Any:
    """Сохранить тип старого JSON-значения: число остаётся числом и т.п."""
    if isinstance(old, bool):
        lowered = raw.strip().lower()
        if lowered in ("true", "1", "yes", "on"):
            return True
        if lowered in ("false", "0", "no", "off"):
            return False
        raise ValueError(f"expected true/false, got {raw!r}")
    if isinstance(old, int) and not isinstance(old, bool):
        return int(float(raw))
    if isinstance(old, float):
        return float(raw)
    if isinstance(old, list):
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError:
            return raw
        return parsed if isinstance(parsed, list) else raw
    if old is not None:
        return raw
    stripped = raw.strip()
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return raw


def _ini_read(text: str, group: str, key: str) -> Optional[str]:
    """Текущее строковое значение INI-ключа (lenient, mirror _set_ini_text)."""
    current = ""
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            current = stripped[1:-1].strip()
            continue
        if current != group or "=" not in stripped:
            continue
        k, _, v = stripped.partition("=")
        if k.strip() == key:
            return v.strip()
    return None


def _json_patch(text: str, path: tuple[str, ...], value: Any) -> str:
    span = _json_positions(text).get(path)
    literal = json.dumps(value, ensure_ascii=False)
    if span is not None:
        return text[: span[0]] + literal + text[span[1]:]

    # Missing top-level key: insert before the final closing brace.
    if len(path) == 1:
        close = text.rfind("}")
        if close >= 0:
            body = text[:close].rstrip()
            prefix = "" if body.endswith("{") or body.endswith(",") else ","
            return f'{body}{prefix}\n  "{path[0]}": {literal}\n{text[close:]}'
    raise KeyError(f"JSON path not found: {'.'.join(path)}")


# ─────────────────────────────────────────────────────────────────────────────
# TOML / kitty / user.js
# ─────────────────────────────────────────────────────────────────────────────

def _toml_literal(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return _fmt_number(value)
    if isinstance(value, (list, tuple)):
        return json.dumps(list(value), ensure_ascii=False)
    return json.dumps(str(value))


def _toml_read(text: Optional[str], key: str, path: tuple[str, ...] = ()) -> Any:
    if not text:
        return None
    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError:
        match = re.search(rf"^\s*{re.escape(key)}\s*=\s*(.+)$", text, re.M)
        return match.group(1).strip() if match else None
    node: Any = data
    for part in (*path, key):
        if not isinstance(node, dict):
            return None
        node = node.get(part)
        if node is None:
            return None
    return node


def _patch_toml(text: str, key: str, value: Any, section: str = "") -> str:
    literal = _toml_literal(value)
    pattern = re.compile(rf"^(\s*){re.escape(key)}\s*=.*$", re.M)
    if not section:
        if pattern.search(text):
            return pattern.sub(lambda m: f"{m.group(1)}{key} = {literal}", text, count=1)
        suffix = "" if text.endswith("\n") else "\n"
        return f"{text}{suffix}{key} = {literal}\n"

    header = re.search(rf"^\[{re.escape(section)}\]\s*$", text, re.M)
    if header is None:
        suffix = "" if text.endswith("\n") else "\n"
        return f"{text}{suffix}[{section}]\n{key} = {literal}\n"

    start = header.end()
    following = re.search(r"^\[", text[start:], re.M)
    end = start + following.start() if following else len(text)
    block = text[start:end]
    if pattern.search(block):
        block = pattern.sub(lambda m: f"{m.group(1)}{key} = {literal}", block, count=1)
    else:
        if block and not block.endswith("\n"):
            block += "\n"
        block += f"{key} = {literal}\n"
    return text[:start] + block + text[end:]


def _toml_set(text: str, key: str, raw: str) -> tuple[str, Any]:
    """Заменить flattened-ключ TOML; вернуть (новый текст, старое значение).

    Flattened-форма — из read_config_params: ``section/key`` для ``[section]``,
    ``a/b/c`` для вложенных таблиц. Вложенный ключ правится только при явном
    заголовке секции (``[a.b]``); inline-таблицы текстово не пересобираются.
    """
    if "/" in key:
        section, _, leaf = key.rpartition("/")
    else:
        section, leaf = "", key
    header = ""
    if section:
        candidates = (section, section.replace("/", "."))
        header = next(
            (h for h in candidates
             if re.search(rf"^\[{re.escape(h)}\]\s*$", text, re.M)),
            "",
        )
        if not header and "/" in section:
            raise ValueError(f"nested TOML table has no [section] header: {key}")
    path = tuple(section.split("/")) if section else ()
    old = _toml_read(text, leaf, path)
    return _patch_toml(text, leaf, _coerce_json_value(old, raw), header), old


def _kv_read(text: Optional[str], key: str) -> Optional[str]:
    if not text:
        return None
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        parts = stripped.split(None, 1)
        if parts[0] == key:
            return parts[1].strip() if len(parts) > 1 else ""
    return None


def _patch_kv(text: str, key: str, value: str) -> str:
    lines = text.splitlines()
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped and not stripped.startswith("#") and stripped.split(None, 1)[0] == key:
            indent = line[: len(line) - len(line.lstrip())]
            lines[index] = f"{indent}{key} {value}"
            return "\n".join(lines) + "\n"
    if lines and lines[-1].strip():
        lines.append(f"{key} {value}")
    else:
        lines.append(f"{key} {value}")
    return "\n".join(lines) + "\n"


_USERJS_RE = r'user_pref\(\s*"{key}"\s*,\s*(.*?)\s*\);'


def _userjs_read(text: Optional[str], key: str) -> Any:
    if not text:
        return None
    match = re.search(_USERJS_RE.format(key=re.escape(key)), text)
    if not match:
        return None
    raw = match.group(1).strip()
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return raw.strip('"')


def _patch_userjs(text: str, key: str, value: Any) -> str:
    literal = json.dumps(value, ensure_ascii=False) if isinstance(value, str) else _toml_literal(value)
    pattern = re.compile(_USERJS_RE.format(key=re.escape(key)))
    if pattern.search(text):
        return pattern.sub(f'user_pref("{key}", {literal});', text, count=1)
    suffix = "" if text.endswith("\n") else "\n"
    return f'{text}{suffix}user_pref("{key}", {literal});\n'


# ─────────────────────────────────────────────────────────────────────────────
# Colors / fonts
# ─────────────────────────────────────────────────────────────────────────────

def _color_parts(raw: Any) -> Optional[tuple[int, int, int]]:
    if raw is None:
        return None
    if isinstance(raw, (list, tuple)) and len(raw) >= 3:
        try:
            return tuple(int(v) for v in raw[:3])  # type: ignore[return-value]
        except (TypeError, ValueError):
            return None
    value = str(raw).strip()
    if value.startswith("#"):
        digits = value[1:]
        if len(digits) in (3, 4):
            digits = "".join(ch * 2 for ch in digits[:3])
        if len(digits) >= 6:
            try:
                return tuple(int(digits[i:i + 2], 16) for i in (0, 2, 4))  # type: ignore[return-value]
            except ValueError:
                return None
        return None
    parts = [part.strip() for part in value.split(",")]
    if len(parts) >= 3:
        try:
            return tuple(max(0, min(255, int(part))) for part in parts[:3])  # type: ignore[return-value]
        except ValueError:
            return None
    return None


def _color_hex(raw: Any) -> str:
    parts = _color_parts(raw)
    if not parts:
        return ""
    return "#{:02x}{:02x}{:02x}".format(*parts)


def _color_raw(value: Any) -> Optional[str]:
    parts = _color_parts(value)
    if not parts:
        return None
    return "{},{},{}".format(*parts)


def _font_parts(raw: Any, gtk: bool = False) -> tuple[str, float]:
    if raw is None:
        return "", 0.0
    text = str(raw).strip()
    if not text:
        return "", 0.0
    if gtk:
        match = re.match(r"^(.*?)[,\s]+([0-9]+(?:\.[0-9]+)?)\s*$", text)
        if match:
            return match.group(1).strip().rstrip(","), float(match.group(2))
        return text, 0.0
    parts = [part.strip() for part in text.split(",")]
    family = parts[0] if parts else ""
    try:
        size = float(parts[1]) if len(parts) > 1 else 0.0
    except ValueError:
        size = 0.0
    return family, size


def _font_display(raw: Any, gtk: bool = False) -> str:
    family, size = _font_parts(raw, gtk)
    if not family and not size:
        return ""
    if size:
        return f"{family} {_fmt_number(size)} pt"
    return family


def _font_raw(family: str, size: float, existing: Any, gtk: bool = False) -> str:
    family = family.strip()
    if gtk:
        return f"{family} {_fmt_number(size)}" if size else family
    parts = str(existing or "").split(",")
    if len(parts) >= 2:
        parts[0] = family
        parts[1] = _fmt_number(size)
        return ",".join(parts)
    return f"{family},{_fmt_number(size)},-1,5,50,0,0,0,0,0,0,0,0,0,0,1"


# ─────────────────────────────────────────────────────────────────────────────
# Core
# ─────────────────────────────────────────────────────────────────────────────

@dataclass
class _UndoEntry:
    field_id: str
    old_raw: Any
    file: Optional[Path]
    old_bytes: Optional[bytes]
    # Пустой field_id + заполненный label — generic-запись (этап C2,
    # конфиги приложений), не привязанная к curated-полю схемы.
    label: str = ""


class AppThemeCore(ThemeCore):
    """Schema-driven theme inspector (Python)."""

    def __init__(self) -> None:
        self._choices_cache: dict[str, list[Choice]] = {}
        self._families: Optional[list[str]] = None
        self._undo: list[_UndoEntry] = []
        self._snapshot_dir: Optional[Path] = None
        # First-seen bytes for every file touched in this session.  ``None``
        # means the file did not exist, which lets revert() remove files that
        # were created while the inspector was open.
        self._originals: dict[Path, Optional[bytes]] = {}
        # Staged (not yet applied) edits. set_value/set_config_value only
        # validate and record; the actual file writes, commands and hooks run
        # once on apply() — one pass instead of a reload per keystroke.
        self._staged: dict[str, Any] = {}
        self._staged_configs: dict[tuple[str, str], str] = {}
        self._stage_order: list[tuple[str, Any]] = []

    # ── paths ─────────────────────────────────────────────────────────────

    @property
    def profiles_dir(self) -> Path:
        return profiles_dir()

    def _active_profile(self) -> str:
        text = _read_text(active_profile_file())
        return text.strip() if text else ""

    def _bases(self, scope: str) -> tuple[Path, Path]:
        home = _home()
        active = self._active_profile()
        if active:
            try:
                compare = profile_dir(active)
            except ValueError:
                compare = self.profiles_dir
        else:
            compare = self.profiles_dir
        if scope.startswith("profile:"):
            name = scope.split(":", 1)[1].strip()
            return profile_dir(name), home
        return home, compare

    def _kvantum_file(self, theme: str, base: Optional[Path] = None) -> Optional[Path]:
        if not theme:
            return None
        profile_base = base is not None and base != _home()
        for name in _KVANTUM_BASES:
            if name.startswith("/"):
                if profile_base:
                    continue
                root = Path(name)
            elif profile_base:
                root = _resolve_under(base, name)
            else:
                root = live_path(name)
            direct = root / f"{theme}.kvconfig"
            if direct.is_file():
                return direct
            try:
                for entry in root.iterdir():
                    if entry.is_dir():
                        candidate = entry / f"{theme}.kvconfig"
                        if candidate.is_file():
                            return candidate
            except OSError:
                continue
        return None

    # ── reading ───────────────────────────────────────────────────────────

    def _file_for(self, field: FieldDef, base: Path) -> Optional[Path]:
        if field.source == "kvantum":
            theme = self._read_raw(FIELD_BY_ID["kvantum_theme"], base)
            return self._kvantum_file(str(theme), base) if theme else None
        if not field.file:
            return None
        path = _resolve_under(base, field.file)
        if field.source == "userjs" and not path.is_file():
            # Zen profile ids change between installs; use the first profile
            # that has a user.js instead of hard-coding the id.
            try:
                matches = sorted(_resolve_under(base, ".zen").glob("*/user.js"))
            except OSError:
                matches = []
            if matches:
                return matches[0]
        return path

    def _target_path(self, field: FieldDef) -> Optional[Path]:
        """Where a live write goes (resolves Kvantum and Zen profiles)."""
        return self._file_for(field, _home())

    def _read_raw(self, field: FieldDef, base: Path) -> Any:
        # A profile that is a theme package (theme.json) stores its values
        # directly; the Rust core wrote them with the same registry.
        if base != _home():
            package = _package_values(base)
            if package is not None:
                return package.get(field.id)
        path = self._file_for(field, base)
        if path is None:
            return None
        text = _read_text(path)
        if text is None:
            return None

        if field.source == "ini":
            value = _ini_get(text, field.group, field.key)
            if (
                value is not None
                and field.apply == "ini"
                and len(value) >= 2
                and value.startswith('"')
                and value.endswith('"')
            ):
                value = value[1:-1]
            return value
        if field.source == "kv":
            return _kv_read(text, field.key)
        if field.source == "toml":
            return _toml_read(text, field.key, field.path)
        if field.source == "json":
            return _json_read(text, field.path)
        if field.source == "userjs":
            return _userjs_read(text, field.key)
        if field.source == "kvantum":
            return _ini_get(text, field.group, field.key)
        return None

    def _coerce(self, field: FieldDef, raw: Any) -> Any:
        if raw is None:
            return None
        if field.type == "bool":
            return _as_bool(raw)
        if field.type == "int":
            try:
                return int(float(str(raw).strip()))
            except (TypeError, ValueError):
                return None
        if field.type == "float":
            try:
                return float(str(raw).strip())
            except (TypeError, ValueError):
                return None
        return str(raw)

    def _display(self, field: FieldDef, value: Any) -> str:
        if value is None:
            if field.id == "cursor_theme":
                return "System default"
            return ""
        if field.type == "bool":
            return "On" if value else "Off"
        if field.type == "color":
            return _color_hex(value)
        if field.type == "font":
            return _font_display(value, field.file.endswith(".ini") and "gtk" in field.file)
        if field.id in ("zen_window_scheme", "zen_sites_scheme"):
            return {"0": "Dark", "1": "Light", "2": "System", "3": "Browser theme"}.get(str(value), str(value))
        if field.type in ("int", "float"):
            return f"{_fmt_number(float(value))}{field.suffix}"
        return str(value)

    def _normalized(self, field: FieldDef, value: Any) -> Any:
        if field.type == "bool":
            return _as_bool(value)
        if field.type in ("int", "float") and value is not None:
            try:
                return float(value)
            except (TypeError, ValueError):
                return value
        if field.type == "color":
            return _color_hex(value)
        if field.type == "font":
            return _font_display(value, field.file.endswith(".ini") and "gtk" in field.file)
        return "" if value is None else str(value)

    def _build_value(self, field: FieldDef, scope: str) -> ThemeFieldValue:
        base, compare_base = self._bases(scope)
        raw = self._read_raw(field, base)

        if field.id == "plasma_theme" and not raw:
            raw = "default"
        if field.type == "bool" and raw is None:
            value: Any = None
        else:
            value = self._coerce(field, raw)

        # Застейдженная правка показывается как текущее значение (поле ещё
        # не записано на диск, но UI должен отражать намерение пользователя).
        staged = False
        if scope == "live" and field.id in self._staged:
            value = self._coerce(field, self._staged[field.id])
            staged = True

        profile_raw = None
        differs = False
        if field.profile_tracked:
            profile_raw = self._read_raw(field, compare_base)
            if field.id == "plasma_theme" and not profile_raw:
                profile_raw = "default"
            differs = self._normalized(field, value) != self._normalized(
                field, self._coerce(field, profile_raw)
            )

        choices: list[Choice] = []
        if field.choices:
            choices = list(self.choices(field.id))
            if value not in (None, "") and not any(c.value == value for c in choices):
                choices.append(Choice(str(value), f"Custom: {value}"))

        resolved = self._file_for(field, base)
        file_label = str(resolved) if field.source == "kvantum" and resolved else field.file

        return ThemeFieldValue(
            id=field.id,
            section=field.section,
            label=field.label,
            type=field.type,
            raw=value,
            display=self._display(field, value),
            is_default=value is None,
            differs_from_profile=differs,
            profile_raw=self._coerce(field, profile_raw),
            profile_display=self._display(field, self._coerce(field, profile_raw)),
            badge=field.badge,
            description=field.description,
            file=file_label,
            group=field.group,
            key=field.key,
            choices=tuple(choices),
            min=field.min,
            max=field.max,
            step=field.step,
            suffix=field.suffix,
            tracked_in_profile=field.profile_tracked,
            advanced=field.advanced,
            confirm=field.confirm,
            staged=staged,
        )

    def sections(self, scope: str = "live") -> list[ThemeSection]:
        return [
            ThemeSection(
                id=section.id,
                title=section.title,
                icon=section.icon,
                description=section.description,
                field_count=sum(1 for f in FIELDS if f.section == section.id),
                issue_count=0,
                summary=self._section_summary(section.id, scope),
            )
            for section in SECTIONS
        ]

    def _section_summary(self, section_id: str, scope: str) -> str:
        def val(field_id: str) -> str:
            field = FIELD_BY_ID[field_id]
            return self._display(field, self._coerce(field, self._read_raw(field, self._bases(scope)[0])))

        if section_id == "colors":
            parts = [val("color_scheme"), val("accent_color")]
        elif section_id == "typography":
            parts = [val("ui_font") or val("fixed_font") or "System default"]
        elif section_id == "windows":
            radius = val("klassy_radius").replace("px", "").strip()
            blur = val("blur_strength")
            parts = [
                val("decoration_theme"),
                f"r{radius}" if radius else "",
                f"blur {blur}" if blur else "",
            ]
        elif section_id == "assets":
            parts = [val("look_and_feel"), val("icon_theme"), val("kvantum_theme")]
        elif section_id == "apps":
            parts = [
                f"kitty {val('kitty_opacity')}" if val("kitty_opacity") else "",
                f"Zen {val('zen_window_scheme')}" if val("zen_window_scheme") else "",
                f"Zed {val('zed_mode')}" if val("zed_mode") else "",
            ]
        else:
            parts = []
        return " · ".join(part for part in parts if part and part != "—")

    def fields(self, section: str, scope: str = "live") -> list[dict]:
        return [
            self._build_value(field, scope).to_dict()
            for field in FIELDS
            if field.section == section
        ]

    def get(self, field_id: str, scope: str = "live") -> Optional[dict]:
        field = FIELD_BY_ID.get(field_id)
        if field is None:
            return None
        return self._build_value(field, scope).to_dict()

    # ── choices ───────────────────────────────────────────────────────────

    def _dir_choices(self, roots: Iterable[tuple[Path, str]]) -> list[Choice]:
        found: list[Choice] = []
        for root, marker in roots:
            try:
                entries = sorted(root.iterdir(), key=lambda p: p.name)
            except OSError:
                continue
            for entry in entries:
                if entry.is_dir() and (entry / marker).exists():
                    found.append(Choice(entry.name))
        return found

    def _file_choices(self, roots: Iterable[Path], suffix: str) -> list[Choice]:
        found: list[Choice] = []
        for root in roots:
            try:
                entries = sorted(root.iterdir(), key=lambda p: p.name)
            except OSError:
                continue
            for entry in entries:
                if entry.is_file() and entry.name.endswith(suffix):
                    found.append(Choice(entry.name[: -len(suffix)]))
        return found

    def _kvantum_choices(self) -> list[Choice]:
        found: list[Choice] = []
        for base in _KVANTUM_BASES:
            root = Path(base) if base.startswith("/") else live_path(base)
            try:
                entries = sorted(root.iterdir(), key=lambda p: p.name)
            except OSError:
                continue
            for entry in entries:
                if entry.is_file() and entry.suffix == ".kvconfig":
                    found.append(Choice(entry.stem))
                elif entry.is_dir():
                    try:
                        for inner in sorted(entry.glob("*.kvconfig")):
                            found.append(Choice(inner.stem))
                    except OSError:
                        continue
        return found

    def choices(self, field_id: str) -> list[Choice]:
        field = FIELD_BY_ID.get(field_id)
        if field is None or not field.choices:
            return []
        key = field.choices
        if key in self._choices_cache:
            return self._choices_cache[key]

        home = _home()
        system = Path("/usr/share")

        if key == "color_schemes":
            result = self._file_choices(
                (home / ".local/share/color-schemes", system / "color-schemes"), ".colors"
            )
        elif key == "plasma_themes":
            result = self._dir_choices((
                (home / ".local/share/plasma/desktoptheme", "metadata.json"),
                (system / "plasma/desktoptheme", "metadata.json"),
            ))
            result.append(Choice("default"))
        elif key == "look_and_feel":
            result = self._dir_choices((
                (home / ".local/share/plasma/look-and-feel", "contents"),
                (system / "plasma/look-and-feel", "contents"),
            ))
        elif key == "icon_themes":
            result = self._dir_choices((
                (home / ".local/share/icons", "index.theme"),
                (home / ".icons", "index.theme"),
                (system / "icons", "index.theme"),
            ))
        elif key == "cursor_themes":
            result = self._dir_choices((
                (home / ".local/share/icons", "cursors"),
                (home / ".icons", "cursors"),
                (system / "icons", "cursors"),
            ))
        elif key == "gtk_themes":
            result = self._dir_choices((
                (home / ".themes", "gtk-3.0"),
                (home / ".local/share/themes", "gtk-3.0"),
                (system / "themes", "gtk-3.0"),
            ))
        elif key == "kvantum_themes":
            result = self._kvantum_choices()
        elif key == "btop_themes":
            result = self._file_choices(
                (home / ".config/btop/themes", system / "btop/themes"), ".theme"
            )
        elif key == "widget_styles":
            current = self._read_raw(FIELD_BY_ID["widget_style"], home)
            result = [Choice(v) for v in _dedupe(["Breeze", "kvantum", "Fusion", str(current or "")])]
        elif key == "decoration_libraries":
            current = self._read_raw(FIELD_BY_ID["decoration_library"], home)
            result = [Choice(v) for v in _dedupe([
                "org.kde.breeze", "org.kde.klassy", "org.kde.kwin.aurorae", str(current or ""),
            ])]
        elif key == "decoration_themes":
            current = self._read_raw(FIELD_BY_ID["decoration_theme"], home)
            result = self._dir_choices((
                (home / ".local/share/aurorae/themes", "metadata.json"),
                (system / "aurorae/themes", "metadata.json"),
            ))
            result += [Choice(v) for v in _dedupe(["Klassy", "Breeze", str(current or "")])]
        elif key == "border_sizes":
            result = [Choice(v) for v in ("None", "Tiny", "Normal", "Large", "Huge")]
        elif key == "shadow_sizes":
            result = [Choice(v) for v in (
                "ShadowNone", "ShadowSmall", "ShadowMedium", "ShadowLarge", "ShadowVeryLarge",
            )]
        elif key == "zen_window_schemes":
            result = [
                Choice("0", "Dark"),
                Choice("1", "Light"),
                Choice("2", "System"),
            ]
        elif key == "zen_site_schemes":
            result = [
                Choice("0", "Dark"),
                Choice("1", "Light"),
                Choice("2", "System"),
                Choice("3", "Browser theme"),
            ]
        elif key == "zed_modes":
            result = [Choice(v) for v in ("system", "light", "dark")]
        else:
            result = []

        result = [Choice(c.value, c.label) for c in result]
        # Stable de-dupe (user themes first, then system ones).
        deduped: dict[str, Choice] = {}
        for choice in result:
            deduped.setdefault(choice.value, choice)
        final = list(deduped.values())
        self._choices_cache[key] = final
        return final

    def font_families(self) -> list[str]:
        if self._families is not None:
            return self._families
        families: list[str] = []
        if shutil.which("fc-list"):
            try:
                result = subprocess.run(
                    ["fc-list", ":", "family"],
                    capture_output=True, text=True, timeout=10, check=False,
                )
                for line in result.stdout.splitlines():
                    for family in line.split(","):
                        family = family.strip()
                        if family and family not in families:
                            families.append(family)
            except (OSError, subprocess.SubprocessError):
                pass
        if not families:
            families = ["Noto Sans", "DejaVu Sans", "Liberation Sans", "Monospace"]
        self._families = sorted(families, key=str.casefold)
        return self._families

    # ── validation ────────────────────────────────────────────────────────

    def validate(self) -> list[ThemeIssue]:
        issues: list[ThemeIssue] = []

        for field in FIELDS:
            value = self._coerce(field, self._read_raw(field, _home()))
            if value in (None, ""):
                continue

            if field.choices in _ASSET_CHOICES:
                installed = {choice.value for choice in self.choices(field.id)}
                if str(value) not in installed:
                    severity = "error" if field.id in ("kvantum_theme", "look_and_feel") else "warning"
                    issues.append(ThemeIssue(
                        severity, field.id,
                        f"{field.label}: \"{value}\" is not installed",
                        "Choose an installed value",
                    ))

            if field.id == "look_and_feel" and (
                "Usage:" in str(value) or "\n" in str(value) or len(str(value)) > 80
            ):
                issues.append(ThemeIssue(
                    "error", field.id,
                    "Look & Feel package contains command output instead of a package name",
                    "Choose an installed package (e.g. glass)",
                ))

            if field.type == "font":
                family, _ = _font_parts(value, "gtk" in field.file)
                if family and self.font_families():
                    known = {f.casefold() for f in self.font_families()}
                    if family.casefold() not in known:
                        issues.append(ThemeIssue(
                            "warning", field.id,
                            f"{field.label}: font \"{family}\" is not installed",
                            "Choose an installed font",
                        ))

        blur = _as_bool(self._read_raw(FIELD_BY_ID["blur_enabled"], _home()))
        legacy = _as_bool(self._read_raw(FIELD_BY_ID["legacy_blur"], _home()))
        if blur and legacy:
            issues.append(ThemeIssue(
                "warning", "blur_enabled",
                "Better Blur DX and the legacy Blur effect are both enabled",
                "Disable one of the blur providers",
            ))

        return issues

    # ── search ────────────────────────────────────────────────────────────

    def search(self, query: str, scope: str = "live") -> list[dict]:
        needle = (query or "").strip().casefold()
        if not needle:
            return []
        titles = {section.id: section.title for section in SECTIONS}
        results: list[dict] = []
        for field in FIELDS:
            item = self._build_value(field, scope).to_dict()
            haystack = " ".join([
                field.id, field.label, field.key, field.description,
                field.file, " ".join(field.keywords), str(item["display"]),
            ]).casefold()
            if needle in haystack:
                item["sectionTitle"] = titles.get(field.section, field.section)
                results.append(item)
        return results

    # ── writing ───────────────────────────────────────────────────────────

    def _snapshot(self, path: Optional[Path]) -> Optional[bytes]:
        """Record the first-seen state of ``path`` for undo/revert.

        The old implementation only tracked files that already existed, so a
        revert after creating a config file left the new file behind.  The
        mapping is per path (not per undo entry), because several fields can
        touch the same file.
        """
        if path is None:
            return None
        if path in self._originals:
            return self._originals[path]
        if not path.is_file():
            self._originals[path] = None
            return None
        data = path.read_bytes()
        self._originals[path] = data
        if self._snapshot_dir is None:
            self._snapshot_dir = theme_root() / "backups" / f"inspector-{_now_stamp()}"
            self._snapshot_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
            try:
                os.chmod(self._snapshot_dir, 0o700)
            except OSError:
                pass
        target = self._snapshot_dir / _snapshot_relative(path.resolve())
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        copy_preserving_mode(path, target)
        return data

    def _field_file(self, field: FieldDef) -> Optional[Path]:
        """The file a field writes (for undo snapshots): the active Kvantum
        theme's kvconfig for Kvantum fields, else the field's file."""
        if field.source == "kvantum":
            theme = self._read_raw(FIELD_BY_ID["kvantum_theme"], _home())
            return self._kvantum_file(str(theme), _home()) if theme else None
        return self._target_path(field)

    @staticmethod
    def _side_files(field: FieldDef) -> list[Path]:
        """Other files the core touches for this field (GTK mirrors)."""
        if field.id in GTK_MIRROR_FIELDS:
            return [live_path(rel) for rel in GTK_SETTINGS]
        return []

    def _cli_field_set(self, values: list[tuple[str, Any]]) -> tuple[set[str], list[str]]:
        """`theme-profile field set ID=VALUE …` — the Rust core writes the
        values, runs live commands and hooks. Returns (applied ids, errors)."""
        from .cli_core import resolve_cli_bin

        args = [resolve_cli_bin(), "field", "set"]
        args += [f"{field_id}={json.dumps(value)}" for field_id, value in values]
        try:
            result = subprocess.run(args, capture_output=True, text=True, timeout=120, check=False)
        except (OSError, subprocess.SubprocessError) as exc:
            return set(), [tr("theme-profile is not available: %s") % exc]
        applied = {line.split("=", 1)[1] for line in result.stdout.splitlines() if line.startswith("applied=")}
        errors = [line.split("=", 1)[1] for line in result.stderr.splitlines() if line.startswith("error=")]
        if result.returncode != 0 and not errors:
            errors.append((result.stderr or result.stdout or "theme-profile field set failed").strip())
        return applied, errors

    def set_value(self, field_id: str, value: object) -> Result:
        """Застейджить правку поля (без записи): применится по apply()."""
        field = FIELD_BY_ID.get(field_id)
        if field is None:
            return Result.err(tr("Unknown field: %s") % field_id)

        new_raw = self._normalize_input(field, value)
        if new_raw is None:
            return Result.err(tr("Invalid value for %s: %r") % (field.label, value))

        if field_id not in self._staged:
            self._stage_order.append(("field", field_id))
        self._staged[field_id] = new_raw
        return Result.ok(tr("%s staged — press Apply") % field.label)

    def apply(self) -> Result:
        """Применить все застейдженные правки одним проходом.

        Записи, команды и хуки выполняются здесь — один раз на батч, а не
        на каждое изменение. Ошибки отдельных полей не отменяют остальные.
        """
        if not self._staged and not self._staged_configs:
            return Result.err(tr("Nothing to apply"))

        messages: list[str] = []
        applied = 0

        # Writing, live commands and hooks are the Rust core's (`theme-profile
        # field set`, one call for the batch): a second implementation here
        # drifted (kitty socket name, GTK mirroring). Python only records the
        # files' first-seen state for undo/revert.
        pending: list[tuple[FieldDef, Any, Any, Optional[Path], Optional[bytes]]] = []
        for field_id, new_raw in list(self._staged.items()):
            field = FIELD_BY_ID.get(field_id)
            if field is None:
                continue
            try:
                path = self._field_file(field)
                old_raw = self._coerce(field, self._read_raw(field, _home()))
                old_bytes = self._snapshot(path)
                for extra in self._side_files(field):
                    self._snapshot(extra)
            except (OSError, KeyError, ValueError) as exc:
                messages.append(f"{field.label}: {exc}")
                continue
            pending.append((field, new_raw, old_raw, path, old_bytes))
        if pending:
            done, errors = self._cli_field_set([(f.id, raw) for f, raw, _, _, _ in pending])
            messages.extend(errors)
            for field, _, old_raw, path, old_bytes in pending:
                if field.id in done:
                    self._undo.append(_UndoEntry(field.id, old_raw, path, old_bytes))
                    applied += 1
        self._staged.clear()

        for (rel_path, key), value in list(self._staged_configs.items()):
            result = self._apply_config_value(rel_path, key, value)
            if result.success:
                applied += 1
            else:
                messages.append(f"{rel_path} · {key}: {result.message}")
        self._staged_configs.clear()
        self._stage_order.clear()
        self._choices_cache.clear()

        message = tr("Applied %s change(s)") % applied
        if messages:
            message += " — " + "; ".join(messages)
        return Result.ok(message)

    def _normalize_input(self, field: FieldDef, value: Any) -> Any:
        if field.id in ("zen_window_scheme", "zen_sites_scheme"):
            try:
                number = int(float(value))
            except (TypeError, ValueError):
                return None
            top = 3 if field.id == "zen_sites_scheme" else 2
            return number if 0 <= number <= top else None
        if field.type == "bool":
            return _as_bool(value)
        if field.type == "int":
            try:
                return int(float(value))
            except (TypeError, ValueError):
                return None
        if field.type == "float":
            try:
                return float(value)
            except (TypeError, ValueError):
                return None
        if field.type == "color":
            return _color_raw(value)
        if field.type == "font":
            if isinstance(value, dict):
                current = self._read_raw(field, _home())
                return _font_raw(
                    str(value.get("family", "")),
                    float(value.get("size", 10)),
                    current,
                    gtk="gtk" in field.file,
                )
            return str(value)
        return str(value)

    def reset(self, field_id: str, to: str = "default") -> Result:
        field = FIELD_BY_ID.get(field_id)
        if field is None:
            return Result.err(tr("Unknown field: %s") % field_id)
        if to == "profile":
            base, _ = self._bases("live")
            active = self._active_profile()
            if not active:
                return Result.err(tr("No active profile"))
            try:
                active_dir = profile_dir(active)
            except ValueError as exc:
                return Result.err(str(exc))
            value = self._read_raw(field, active_dir)
            if value is None:
                return Result.err(tr("%s is not stored in the profile") % field.label)
            return self.set_value(field_id, value)
        if to != "default":
            return Result.err(tr("Unknown reset target: %s") % to)
        return Result.err(tr("This field has no portable default; pick a value instead"))

    def set_config_value(self, rel_path: str, key: str, value: str) -> Result:
        """Застейджить правку ключа конфига приложения (применится по apply()).

        ``key`` — flattened-форма из read_config_params: INI "group/key"
        (или "key" без группы), JSON/TOML "a/b", user.js — точечный ключ.
        Тип значения сохраняется (число/bool/список не превращаются в
        строку). Запись и undo-вход появляются в apply().
        """
        try:
            self._plan_config_write(rel_path, key, value)
        except (KeyError, ValueError) as exc:
            return Result.err(str(exc))
        if (rel_path, key) not in self._staged_configs:
            self._stage_order.append(("config", (rel_path, key)))
        self._staged_configs[(rel_path, key)] = value
        return Result.ok(tr("%s staged — press Apply") % key)

    def _plan_config_write(self, rel_path: str, key: str, value: str) -> tuple[str, Any]:
        """Проверить и рассчитать запись ключа; вернуть (new_text, old_raw).

        Чистое вычисление без I/O — вызывается и при стейджинге (валидация),
        и в apply() (самa запись). Бросает KeyError/ValueError с
        человекочитаемой ошибкой.
        """
        path = live_path(rel_path)
        if not path.is_file():
            raise ValueError(f"File not found: {rel_path}")
        suffix = path.suffix.lower()
        if suffix in _UNSUPPORTED_SUFFIXES:
            raise ValueError(f"Editing {suffix} files is not supported")
        text = _read_text(path) or ""
        if suffix in (".json", ".jsonc"):
            node_path = tuple(part for part in key.split("/") if part)
            old_raw = _json_read(text, node_path)
            new_value = _coerce_json_value(old_raw, value)
            new_text = _json_patch(text, node_path, new_value)
        elif suffix == ".toml":
            new_text, old_raw = _toml_set(text, key, value)
        elif suffix == ".js":
            old_raw = _userjs_read(text, key)
            new_value = _coerce_json_value(old_raw, value)
            new_text = _patch_userjs(text, key, new_value)
        else:
            # Flattened INI: "group/key"; без "/" — ключ вне секций либо
            # line-based "key value" (kitty.conf) — пишем в том же формате,
            # в котором ключ найден.
            if "/" in key:
                group, _, leaf = key.partition("/")
            else:
                group, leaf = "", key
            old_raw = _ini_read(text, group, leaf)
            if old_raw is None and not group and _kv_read(text, leaf) is not None:
                new_text = _patch_kv(text, leaf, value)
            else:
                new_text = _set_ini_text(text, group, leaf, value)
        return new_text, old_raw

    def _apply_config_value(self, rel_path: str, key: str, value: str) -> Result:
        """Применить одну застейдженную правку ключа (запись + undo-вход)."""
        path = live_path(rel_path)
        try:
            new_text, old_raw = self._plan_config_write(rel_path, key, value)
        except (KeyError, ValueError) as exc:
            return Result.err(str(exc))
        old_bytes = self._snapshot(path)
        try:
            _atomic_write(path, new_text)
        except OSError as exc:
            return Result.err(tr("Failed to write %s: %s") % (rel_path, exc))
        self._undo.append(_UndoEntry("", old_raw, path, old_bytes,
                                   label=f"{rel_path} · {key}"))
        return Result.ok(tr("%s set") % key)

    def undo(self) -> Result:
        # Сначала снимаем застейдженные (еще не записанные) правки — они
        # просто выбывают из стейджа; затем откатываем применённые записи.
        if self._stage_order:
            kind, token = self._stage_order.pop()
            if kind == "field":
                self._staged.pop(str(token), None)
                label = FIELD_BY_ID[str(token)].label
            else:
                self._staged_configs.pop(token, None)
                rel_path, key = token
                label = f"{rel_path} · {key}"
            self._choices_cache.clear()
            return Result.ok(tr("Unstaged: %s") % label)
        if not self._undo:
            return Result.err(tr("Nothing to undo"))
        entry = self._undo.pop()
        field = FIELD_BY_ID.get(entry.field_id)
        if entry.file is not None:
            if entry.old_bytes is None:
                try:
                    entry.file.unlink()
                except OSError:
                    pass
            else:
                try:
                    _atomic_write_bytes(entry.file, entry.old_bytes)
                except OSError as exc:
                    return Result.err(tr("Undo failed: %s") % exc)
        if field is None:
            # Generic-запись (этап C2): хуков и команд нет, только файл.
            self._choices_cache.clear()
            return Result.ok(tr("Undone: %s") % (entry.label or entry.field_id))
        if entry.old_raw is not None:
            _, errors = self._cli_field_set([(field.id, entry.old_raw)])
            if errors:
                self._choices_cache.clear()
                return Result.err(tr("Undo failed: %s") % "; ".join(errors))
        self._choices_cache.clear()
        return Result.ok(tr("Undone: %s") % field.label)

    def revert(self) -> Result:
        # Сбрасываем стейдж и откатываем применённые записи за одно действие.
        discarded = len(self._staged) + len(self._staged_configs)
        self._staged.clear()
        self._staged_configs.clear()
        self._stage_order.clear()
        self._choices_cache.clear()
        if not self._originals:
            if discarded:
                return Result.ok(tr("Discarded %s staged change(s)") % discarded)
            return Result.ok(tr("No changes in this session"))
        touched = [FIELD_BY_ID[entry.field_id] for entry in self._undo
                   if entry.field_id in FIELD_BY_ID]
        restored = 0
        for path, original in list(self._originals.items()):
            try:
                if original is None:
                    if path.is_file() or path.is_symlink():
                        path.unlink()
                        restored += 1
                else:
                    _atomic_write_bytes(path, original)
                    restored += 1
            except OSError:
                continue
        self._undo.clear()
        self._originals.clear()
        # Keep the snapshot directory: it is the only on-disk record of what
        # a session changed before the user reverted it.
        self._snapshot_dir = None
        self._choices_cache.clear()

        # Restoring the files is not enough for components with a live command
        # (accent, look-and-feel, Kvantum, cursor). Re-apply their (now old)
        # values and refresh the hooks once.
        seen: set[str] = set()
        reapply: list[tuple[str, Any]] = []
        for field in reversed(touched):
            if field.id in seen or not (field.command or field.hook):
                continue
            seen.add(field.id)
            value = self._coerce(field, self._read_raw(field, _home()))
            if value is not None:
                reapply.append((field.id, value))
        messages = []
        if reapply:
            _, errors = self._cli_field_set(reapply)
            messages.extend(errors)
        if discarded:
            messages.append(tr("Discarded %s staged change(s)") % discarded)
        messages.append(tr("Reverted %s file(s)") % restored)
        return Result.ok("; ".join(messages))

    def session_changes(self) -> int:
        """Число неприменённых (застейдженных) правок."""
        return len(self._staged) + len(self._staged_configs)

    def undo_depth(self) -> int:
        """Число применённых записей, доступных для пошаговой отмены."""
        return len(self._undo)


def _stringify(field: FieldDef, value: Any) -> str:
    if field.type == "bool":
        return "true" if value else "false"
    if field.type in ("int", "float"):
        return _fmt_number(float(value))
    return str(value)


__all__ = ["AppThemeCore"]


# ──────────────────────────────────────────────────────────────
# Read-only config browsing (этап C1)
#
# Парсит файл конфигурации в плоский список {key, value} для показа в
# инспекторе. Только чтение: ничего не пишет и не пересериализует.
# Редактирование (C2/C3) переиспользует эти же форматы.
# ──────────────────────────────────────────────────────────────

_UNSUPPORTED_SUFFIXES = {
    ".css", ".scss", ".png", ".jpg", ".jpeg", ".webp", ".bmp", ".gif",
    ".svg", ".pdf", ".zip", ".ttf", ".otf",
}

_USER_PREF = re.compile(r'user_pref\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*(.*?)\)\s*;', re.S)


def _display_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (dict, list)):
        return json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    return str(value)


def _flatten(value: Any, prefix: str, out: list[dict], limit: int) -> None:
    if isinstance(value, dict):
        for key in sorted(value.keys(), key=str):
            child = f"{prefix}{key}"
            if isinstance(value[key], dict):
                _flatten(value[key], child + "/", out, limit)
            else:
                if len(out) >= limit:
                    return
                out.append({"key": child, "value": _display_value(value[key])})
    else:
        if len(out) < limit:
            out.append({"key": prefix.rstrip("/."), "value": _display_value(value)})


def _strip_jsonc(text: str) -> str:
    """Убрать // и /* */ комментарии, не трогая строковые литералы."""
    out: list[str] = []
    i, n = 0, len(text)
    in_string = False
    quote = ""
    while i < n:
        ch = text[i]
        if in_string:
            out.append(ch)
            if ch == "\\" and i + 1 < n:
                out.append(text[i + 1])
                i += 2
                continue
            if ch == quote:
                in_string = False
            i += 1
            continue
        if ch in "\"'":
            in_string = True
            quote = ch
            out.append(ch)
            i += 1
            continue
        if ch == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                i += 1
            continue
        if ch == "/" and i + 1 < n and text[i + 1] == "*":
            i += 2
            while i + 1 < n and not (text[i] == "*" and text[i + 1] == "/"):
                i += 1
            i += 2
            continue
        out.append(ch)
        i += 1
    return "".join(out)


def _strip_trailing_commas(text: str) -> str:
    return re.sub(r",(\s*[}\]])", r"\1", text)


def _parse_ini_like(text: str, out: list[dict], limit: int) -> bool:
    """Lenient INI (KDE rc-файлы): [group] + key=value; без секций — key=value."""
    group = ""
    before = len(out)
    for line in text.splitlines():
        if len(out) >= limit:
            break
        stripped = line.strip()
        if not stripped or stripped[0] in "#;!":
            continue
        if stripped.startswith("[") and "]" in stripped:
            group = stripped[1:stripped.index("]")].strip()
            continue
        if "=" in stripped:
            key, _, value = stripped.partition("=")
        else:
            continue
        key = key.strip()
        if not key:
            continue
        full = f"{group}/{key}" if group else key
        out.append({"key": full, "value": value.strip()})
    return len(out) > before


def read_config_params(path: Path, *, limit: int = 300) -> Optional[dict]:
    """Прочитать конфиг в плоские {key, value} строки для показа.

    Возвращает ``{"params": [...], "truncated": bool, "note": str}`` или
    None, если формат не поддерживается или файл недоступен.
    """
    try:
        if path.suffix.lower() in _UNSUPPORTED_SUFFIXES:
            return None
        if path.stat().st_size > 512 * 1024:
            return {"params": [], "truncated": False, "note": "file too large"}
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None

    suffix = path.suffix.lower()
    out: list[dict] = []
    try:
        if suffix == ".toml":
            _flatten(tomllib.loads(text), "", out, limit)
        elif suffix in (".json", ".jsonc"):
            data = json.loads(_strip_trailing_commas(_strip_jsonc(text)))
            _flatten(data, "", out, limit)
        elif suffix == ".js":
            for match in _USER_PREF.finditer(text):
                out.append({"key": match.group(1), "value": match.group(2).strip()})
                if len(out) >= limit:
                    break
        elif not _parse_ini_like(text, out, limit):
            # line-based fallback: "key value" (kitty.conf и подобные)
            for line in text.splitlines():
                if len(out) >= limit:
                    break
                stripped = line.strip()
                if not stripped or stripped[0] in "#;":
                    continue
                parts = stripped.split(None, 1)
                out.append({
                    "key": parts[0],
                    "value": parts[1].strip() if len(parts) > 1 else "",
                })
    except Exception:
        return {"params": out, "truncated": len(out) >= limit, "note": "parse error"}
    if not out:
        return None
    return {"params": out, "truncated": len(out) >= limit, "note": ""}
