"""Declarative schema for the Theme Inspector.

One registry drives reading, writing, diffing and validation. Keep it free of
I/O: everything here is static metadata.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional


@dataclass(frozen=True, slots=True)
class SectionDef:
    id: str
    title: str
    icon: str
    description: str = ""


@dataclass(frozen=True, slots=True)
class FieldDef:
    """Один параметр темы.

    ``source``:
      - ``ini``    — INI-файл (KConfig, GTK, Kvantum), читаем секцию/ключ;
      - ``kv``     — строки ``key value`` (kitty.conf);
      - ``toml``   — плоский ключ верхнего уровня (alacritty);
      - ``json``   — путь в JSON/JSONC (zed, vscode, obsidian);
      - ``userjs`` — ``user_pref("key", value);`` (Zen);
      - ``kvantum``— файл активной Kvantum-темы (путь вычисляется).

    ``apply``:
      - ``config``    — запись через kwriteconfig6 (KDE) или generic INI;
      - ``ini``/``kv``/``toml``/``json``/``userjs`` — точечная запись файла;
      - ``cmd_config``— записать конфиг, затем выполнить ``command``.
    """

    id: str
    section: str
    label: str
    type: str
    source: str
    file: str = ""
    group: str = ""
    key: str = ""
    path: tuple[str, ...] = ()
    choices: str = ""
    apply: str = "config"
    command: tuple[str, ...] = ()
    hook: str = ""
    badge: str = "live"
    summary: bool = False
    advanced: bool = False
    profile_tracked: bool = True
    description: str = ""
    keywords: tuple[str, ...] = ()
    min: Optional[float] = None
    max: Optional[float] = None
    step: Optional[float] = None
    suffix: str = ""
    confirm: bool = False


def _load_registry() -> tuple[tuple[SectionDef, ...], tuple[FieldDef, ...]]:
    """Read the shared registry (``theme_fields.json``).

    The same file is compiled into the Rust core (``rust_core/src/field.rs``),
    so the Python inspector and the Rust theme engine read and write exactly
    the same keys. Edit fields there, not here.
    """
    import json
    from pathlib import Path

    data = json.loads((Path(__file__).with_name("theme_fields.json")).read_text(encoding="utf-8"))

    def tuples(item: dict) -> dict:
        return {k: tuple(v) if isinstance(v, list) else v for k, v in item.items()}

    sections = tuple(SectionDef(**item) for item in data["sections"])
    fields = tuple(FieldDef(**tuples(item)) for item in data["fields"])
    return sections, fields


SECTIONS, FIELDS = _load_registry()


FIELD_BY_ID: dict[str, FieldDef] = {f.id: f for f in FIELDS}

__all__ = ["SECTIONS", "FIELDS", "FIELD_BY_ID", "FieldDef", "SectionDef"]
