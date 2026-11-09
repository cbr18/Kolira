"""Python view/transport adapter for the Rust Theme Inspector.

Rust owns parsing, validation, staging, apply and persistent Undo/Revert.
This module maps core results to GUI dataclasses and translated labels.
Pure format helpers below are compatibility transports to the same Rust code.
"""

from __future__ import annotations

from contextlib import contextmanager
import re
from pathlib import Path
from typing import Any, Optional


def field_text(text: str) -> str:
    """A field label/description from theme_fields.json, translated (the
    strings reach the catalogue through i18n_strings.py)."""
    if not text:
        return text
    from PySide6.QtCore import QCoreApplication

    return QCoreApplication.translate("ThemeFields", text)


def tr(text: str) -> str:
    """Compatibility translator for the generated AppThemeCore catalogue."""
    from PySide6.QtCore import QCoreApplication

    return QCoreApplication.translate("AppThemeCore", text)

from .paths import (
    home_dir,
    profile_dir,
    profiles_dir,
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


# ─────────────────────────────────────────────────────────────────────────────
# Small helpers
# ─────────────────────────────────────────────────────────────────────────────

def _config_call(operation, **arguments):
    from .cli_core import core_request
    return core_request("config." + operation, **arguments)


def _home() -> Path:
    return home_dir()


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

def _parse_ini(text: str) -> dict:
    return _config_call("ini_parse", text=text)


def _ini_get(text: str, group: str, key: str):
    return _config_call("ini_get", text=text, group=group, key=key)


def _set_ini_text(text: str, group: str, key: str, value: str) -> str:
    return _config_call("ini_set", text=text, group=group, key=key, value=value)


# ─────────────────────────────────────────────────────────────────────────────
# JSON / JSONC: read and patch values without re-serializing the file
# ─────────────────────────────────────────────────────────────────────────────

def _json_positions(text: str) -> dict:
    return {tuple(path): (len(text.encode()[:start].decode()), len(text.encode()[:end].decode()))
            for path, start, end in _config_call("positions", text=text)}


def _json_read(text, path):
    return _config_call("json_get", text=text or "", path=list(path))


def _coerce_json_value(old, raw):
    return _config_call("coerce", old=old, value=raw)


def _ini_read(text, group, key):
    return _ini_get(text, group, key)


def _json_patch(text, path, value):
    try:
        return _config_call("json_set", text=text, path=list(path), value=value)
    except ValueError as exc:
        raise KeyError(str(exc)) from exc


# ─────────────────────────────────────────────────────────────────────────────
# TOML / kitty / user.js
# ─────────────────────────────────────────────────────────────────────────────


def _toml_read(text, key, path=()):
    return _config_call("toml_get", text=text or "", key=key, path=list(path))


def _patch_toml(text, key, value, section=""):
    return _config_call("toml_set", text=text, key=key, value=value, group=section)


def _toml_set(text, key, raw):
    return tuple(_config_call("generic_toml", text=text, key=key, value=raw))


def _kv_read(text, key):
    return _config_call("kv_get", text=text or "", key=key)


def _patch_kv(text, key, value):
    return _config_call("kv_set", text=text, key=key, value=value)


def _userjs_read(text, key):
    return _config_call("userjs_get", text=text or "", key=key)


def _patch_userjs(text, key, value):
    return _config_call("userjs_set", text=text, key=key, value=value)


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


# ─────────────────────────────────────────────────────────────────────────────
# Core
# ─────────────────────────────────────────────────────────────────────────────


class AppThemeCore(ThemeCore):
    """Schema-driven theme inspector (Python)."""

    def __init__(self) -> None:
        self._choices_cache: dict[str, list[Choice]] = {}
        self._families: Optional[list[str]] = None
        self._read_cache = None
        self._closed = False
        self._revision = 0
        self._session = None
        self._undo_count = 0
        self._app_changes = {}
        # Read-only mirrors of the Rust session, used to render staged values.
        self._staged: dict[str, Any] = {}
        self._staged_configs: dict[tuple[str, str], str] = {}
        self._session_call("state")

    # ── paths ─────────────────────────────────────────────────────────────

    @property
    def profiles_dir(self) -> Path:
        return profiles_dir()

    def _active_profile(self) -> str:
        from .cli_core import core_request
        if self._read_cache is not None:
            if "active" not in self._read_cache:
                self._read_cache["active"] = core_request("profile.active")
            return self._read_cache["active"]
        return core_request("profile.active")

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

    def _kvantum_file(self, theme: str, base: Optional[Path] = None):
        from .cli_core import core_request
        value = core_request("field.kvantum_file", theme=theme,
                             base=str(base) if base is not None and base != _home() else None)
        return Path(value) if value else None

    def _file_for(self, field: FieldDef, base: Path):
        from .cli_core import core_request
        value = core_request("field.file", id=field.id,
                             base=str(base) if base != _home() else None)
        return Path(value) if value else None

    def _target_path(self, field: FieldDef):
        return self._file_for(field, _home())

    def _read_raw(self, field: FieldDef, base: Path):
        from .cli_core import core_request
        key = str(base) if base != _home() else None
        if self._read_cache is not None:
            if key not in self._read_cache:
                self._read_cache[key] = core_request("field.reads", base=key)
            return self._read_cache[key].get(field.id)
        return core_request("field.read", id=field.id, base=key)

    @contextmanager
    def read_batch(self):
        previous = self._read_cache
        if previous is None:
            self._read_cache = {}
        try:
            yield
        finally:
            self._read_cache = previous

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

        resolved = self._file_for(field, base) if field.source == "kvantum" else None
        file_label = str(resolved) if field.source == "kvantum" and resolved else field.file

        return ThemeFieldValue(
            id=field.id,
            section=field.section,
            label=field_text(field.label),
            type=field.type,
            raw=value,
            display=self._display(field, value),
            is_default=value is None,
            differs_from_profile=differs,
            profile_raw=self._coerce(field, profile_raw),
            profile_display=self._display(field, self._coerce(field, profile_raw)),
            badge=field.badge,
            description=field_text(field.description),
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

    def sections(self, scope: str = "live"):
        with self.read_batch():
            return self._view_sections(scope)

    def _view_sections(self, scope: str = "live") -> list[ThemeSection]:
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

    def fields(self, section: str, scope: str = "live"):
        with self.read_batch():
            return self._view_fields(section, scope)

    def _view_fields(self, section: str, scope: str = "live") -> list[dict]:
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

    def choices(self, field_id: str) -> list[Choice]:
        from .cli_core import core_request
        if field_id not in FIELD_BY_ID:
            return []
        key = FIELD_BY_ID[field_id].choices
        if not key:
            return []
        if key not in self._choices_cache:
            self._choices_cache[key] = [Choice(**v) for v in core_request("field.choices", id=field_id)]
        return self._choices_cache[key]

    def font_families(self) -> list[str]:
        from .cli_core import core_request
        if self._families is None:
            self._families = core_request("field.fonts")
        return self._families

    def validate(self) -> list[ThemeIssue]:
        from .cli_core import core_request
        return [ThemeIssue(**value) for value in core_request("field.validate")]

    # ── search ────────────────────────────────────────────────────────────

    def search(self, query: str, scope: str = "live"):
        with self.read_batch():
            return self._view_search(query, scope)

    def _view_search(self, query: str, scope: str = "live") -> list[dict]:
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

    # ── Rust-owned staging and mutations ────────────────────────────────

    def _session_call(self, operation: str, **arguments) -> Result:
        from .cli_core import core_request
        if self._closed:
            return Result.err("Inspector session is closed")
        try:
            response = core_request("inspector." + operation,
                                    session=self._session, **arguments)
        except (RuntimeError, ValueError) as exc:
            return Result.err(str(exc))
        if response["revision"] >= self._revision:
            self._revision = response["revision"]
            self._session = response["session"]
            self._app_changes = response["apps"]
            self._staged = response["staged"]
            self._staged_configs = {(r, k): v for r, k, v in response["configs"]}
            self._undo_count = response["undoDepth"]
            self._choices_cache.clear()
        from .core_messages import localize_inspector
        return Result(response["ok"], localize_inspector(response["message"]), data=response.get("data"))

    def close(self) -> None:
        from .cli_core import core_request
        if self._session is not None:
            core_request("inspector.close", session=self._session)
        self._closed = True
        self._session = None

    def set_value(self, field_id: str, value: object) -> Result:
        return self._session_call("stage", id=field_id, value=value)

    def set_config_value(self, rel_path: str, key: str, value: str) -> Result:
        return self._session_call("config", path=rel_path, key=key, value=value)

    def apply(self) -> Result:
        return self._session_call("apply")

    def undo(self) -> Result:
        return self._session_call("undo")

    def revert(self) -> Result:
        return self._session_call("revert")

    def reset(self, field_id: str, to: str = "default") -> Result:
        return self._session_call("reset", id=field_id, target=to)

    def session_changes(self) -> int:
        return len(self._staged) + len(self._staged_configs)

    def undo_depth(self) -> int:
        return self._undo_count


__all__ = ["AppThemeCore"]


# ──────────────────────────────────────────────────────────────
# Read-only config browsing (этап C1)
#
# Парсит файл конфигурации в плоский список {key, value} для показа в
# инспекторе. Только чтение: ничего не пишет и не пересериализует.
# Редактирование (C2/C3) переиспользует эти же форматы.
# ──────────────────────────────────────────────────────────────


def _strip_jsonc(text):
    return _config_call("strip_jsonc", text=text)


def _strip_trailing_commas(text):
    return _config_call("strip_trailing_commas", text=text)


def read_config_params(path: Path, *, limit: int = 300):
    return _config_call("params", file=str(path), limit=limit)
