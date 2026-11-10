"""Local MCP server for building theme profiles programmatically.

The server speaks newline-delimited JSON-RPC on stdio and uses the existing
ThemeBundle core. It deliberately exposes explicit operations only: reading
installed themes, staging/applying values, saving profiles and validating
bundle references. Profile deletion and hidden live edits are not exposed.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import Any, Callable

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = PROJECT_ROOT / "gui" / "qt-app" / "src"
if not SRC_DIR.is_dir():
    SRC_DIR = Path(__file__).resolve().parent / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from core import CoreFactory  # noqa: E402
from core.cli_core import core_request  # noqa: E402
from core.palette_core import DEFAULT_TARGETS, TARGETS, PaletteCore  # noqa: E402
from core.theme_bundle import ThemeBundleCore  # noqa: E402
from core.paths import validate_profile_name  # noqa: E402


def _validate_arguments(value, schema, path="arguments"):
    """Validate transport types before coercions; business validation stays Rust."""
    kind = schema.get("type")
    checks = {"object": lambda v: isinstance(v, dict), "array": lambda v: isinstance(v, list),
              "string": lambda v: isinstance(v, str), "boolean": lambda v: isinstance(v, bool),
              "integer": lambda v: isinstance(v, int) and not isinstance(v, bool),
              "number": lambda v: isinstance(v, (int, float)) and not isinstance(v, bool)}
    if kind in checks and not checks[kind](value):
        raise ValueError(f"{path} must be {kind}")
    if "enum" in schema and value not in schema["enum"]:
        raise ValueError(f"{path}: unknown value")
    if isinstance(value, dict):
        for key in schema.get("required", []):
            if key not in value:
                raise ValueError(f"{path}.{key} is required")
        for key, item in value.items():
            child = schema.get("properties", {}).get(key, schema.get("additionalProperties", {}))
            if child is False:
                raise ValueError(f"Unknown argument: {path}.{key}")
            if isinstance(child, dict):
                _validate_arguments(item, child, f"{path}.{key}")
    elif isinstance(value, list):
        if len(value) < schema.get("minItems", 0):
            raise ValueError(f"{path} is empty")
        for index, item in enumerate(value):
            _validate_arguments(item, schema.get("items", {}), f"{path}[{index}]")
    elif kind in {"integer", "number"}:
        if value < schema.get("minimum", float("-inf")) or value > schema.get("maximum", float("inf")):
            raise ValueError(f"{path} is out of range")


def _scope(arguments: dict) -> str:
    scope = str(arguments.get("scope") or "live")
    if scope != "live" and not scope.startswith("profile:"):
        raise ValueError("scope must be 'live' or 'profile:<name>'")
    return scope


def _component_choices(core: ThemeBundleCore, component_id: str) -> dict:
    for group in core.groups("live"):
        for component in group["components"]:
            if component["id"] == component_id:
                return component
    if component_id in {app["id"] for app in core.applications()}:
        app = next(item for item in core.applications() if item["id"] == component_id)
        return {
            "id": app["id"],
            "label": app["name"],
            "currentValue": app["currentValue"],
            "currentDisplay": app["currentDisplay"],
            "choices": app["themes"],
            "installed": app["installed"],
            "capabilities": app["capabilities"],
        }
    raise ValueError(f"Unknown component: {component_id}")


class KoliraMcpServer:
    protocol_version = "2024-11-05"

    def __init__(self) -> None:
        profile_core = CoreFactory.profile_core()
        wallpaper_core = CoreFactory.wallpaper_core()
        self.profiles = profile_core
        self.core = ThemeBundleCore(
            theme_core=CoreFactory.theme_core(),
            profile_core=profile_core,
            wallpaper_core=wallpaper_core,
        )
        self._tools: dict[str, Callable[[dict], Any]] = {
            "get_capabilities": lambda a: core_request("agent.capabilities"),
            "get_theme_schema": lambda a: core_request("agent.schema"),
            "prepare_theme": lambda a: core_request("agent.prepare", **a),
            "list_operations": lambda a: core_request("operation.list"),
            "restore_operation": lambda a: core_request("operation.restore", source="mcp", **a),
            "recover_operation": self._recover_operation,
            "list_schedule": lambda a: core_request("schedule.list"),
            "save_schedule_rule": lambda a: core_request("schedule.put", **a),
            "delete_schedule_rule": lambda a: core_request("schedule.delete", **a),
            "enable_schedule": lambda a: core_request("schedule.enable", **a),
            "list_dependency_registry": lambda a: core_request("dependency.list"),
            "start_dependency_operation": lambda a: core_request("dependency.start", **a),
            "dependency_status": lambda a: self._dependency_request("operation", a),
            "cancel_dependency_operation": lambda a: self._dependency_request("cancel", a),
            "list_components": self._list_components,
            "list_installed_themes": self._list_installed_themes,
            "list_profiles": self._list_profiles,
            "get_profile_bundle": self._get_profile_bundle,
            "validate_profile": self._validate_profile,
            "set_component": self._set_component,
            "apply_changes": self._apply_changes,
            "create_custom_theme": self._create_custom_theme,
            "create_profile": self._create_profile,
            "preview_profile": self._preview_profile,
            "apply_profile": self._apply_profile,
            "export_theme": self._export_theme,
            "import_theme": self._import_theme,
            "list_dependencies": self._list_dependencies,
            "install_dependencies": self._install_dependencies,
            "get_automation": self._get_automation,
            "set_automation": self._set_automation,
            "list_actions": self._list_actions,
            "run_action": self._run_action,
            "palette_show": self._palette_show,
            "palette_gallery": self._palette_gallery,
            "palette_import": self._palette_import,
            "palette_set": self._palette_set,
            "palette_from_wallpaper": self._palette_from_wallpaper,
            "palette_from_colors": self._palette_from_colors,
            "palette_apply": self._palette_apply,
            "app_store_search": self._app_store_search,
            "app_store_install": self._app_store_install,
        }
        self.palette = PaletteCore()

    # ── MCP protocol ───────────────────────────────────────────────────

    def handle(self, message: dict) -> dict | None:
        if not isinstance(message, dict):
            return self._invalid_request(None, "Request must be an object")
        method = message.get("method")
        if not isinstance(method, str):
            return self._invalid_request(message.get("id"), "method must be a string")
        message_id = message.get("id")
        if method.startswith("notifications/"):
            return None
        if method == "initialize":
            return self._result(message_id, {
                "protocolVersion": self.protocol_version,
                "capabilities": {"tools": {}},
                "serverInfo": {
                    "name": "kolira",
                    "version": "0.6.0",
                },
            })
        if method == "ping":
            return self._result(message_id, {})
        if method == "tools/list":
            return self._result(message_id, {"tools": self._tool_schemas()})
        if method == "tools/call":
            params = message.get("params", {})
            if not isinstance(params, dict):
                return self._invalid_request(message_id, "params must be an object", -32602)
            name = params.get("name")
            arguments = params.get("arguments", {})
            if not isinstance(name, str) or not isinstance(arguments, dict):
                return self._invalid_request(message_id, "name must be a string and arguments an object", -32602)
            try:
                if name not in self._tools:
                    raise ValueError(f"Unknown tool: {name}")
                schema = next(t["inputSchema"] for t in self._tool_schemas() if t["name"] == name)
                _validate_arguments(arguments, schema)
                output = self._tools[name](dict(arguments))
                return self._result(message_id, {
                    "content": [{
                        "type": "text",
                        "text": json.dumps(output, ensure_ascii=False, indent=2),
                    }],
                    "isError": isinstance(output, dict) and output.get("ok") is False,
                })
            except Exception as exc:  # noqa: BLE001 - return as MCP tool error
                return self._result(message_id, {
                    "content": [{
                        "type": "text",
                        "text": json.dumps({"ok": False, "error": {"code": "invalid_arguments" if isinstance(exc, (ValueError, TypeError)) else "core_unavailable", "message": str(exc), "next_action": "Review get_capabilities/get_theme_schema and arguments; refresh preview before retrying apply."}}, ensure_ascii=False),
                    }],
                    "isError": True,
                })
        if message_id is None:
            return None
        return {
            "jsonrpc": "2.0",
            "id": message_id,
            "error": {"code": -32601, "message": f"Unknown method: {method}"},
        }

    @staticmethod
    def _invalid_request(message_id: Any, message: str, code: int = -32600) -> dict:
        return {"jsonrpc": "2.0", "id": message_id,
                "error": {"code": code, "message": message}}

    @staticmethod
    def _result(message_id: Any, result: Any) -> dict:
        return {"jsonrpc": "2.0", "id": message_id, "result": result}

    def _tool_schemas(self) -> list[dict]:
        tools = [
            {
                "name": "list_components",
                "description": "List theme bundle groups, component values and application adapters.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "scope": {"type": "string", "default": "live"},
                    },
                },
            },
            {
                "name": "list_installed_themes",
                "description": "List installed choices for one component or application.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "componentId": {"type": "string"},
                    },
                    "required": ["componentId"],
                },
            },
            {
                "name": "list_profiles",
                "description": "List saved profiles and whether they have v2 theme bundle metadata.",
                "inputSchema": {"type": "object", "properties": {}},
            },
            {
                "name": "get_profile_bundle",
                "description": "Read the v2 component/application references of a profile.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "required": ["name"],
                },
            },
            {
                "name": "validate_profile",
                "description": "Check whether everything a theme references is installed here; lists what is missing and where to get it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "required": ["name"],
                },
            },
            {
                "name": "set_component",
                "description": "Set one theme component. Core fields are staged; app adapters apply immediately with a backup.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "componentId": {"type": "string"},
                        "value": {},
                    },
                    "required": ["componentId", "value"],
                },
            },
            {
                "name": "apply_changes",
                "description": "Apply staged core fields in one batch.",
                "inputSchema": {"type": "object", "properties": {}},
            },
            {
                "name": "create_custom_theme",
                "description": "Create user-owned derived color and Kvantum themes without changing system base themes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "required": ["name"],
                },
            },
            {
                "name": "create_profile",
                "description": "Save the current appearance as a theme package (theme.json + local assets), optionally after applying supplied component values.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "description": {"type": "string"},
                        "components": {"type": "object"},
                        "applications": {"type": "object"},
                        "includeWallpaper": {
                            "type": "boolean",
                            "description": "Store the current wallpaper in the theme (default true). Without it, applying the theme leaves the wallpaper unchanged.",
                        },
                    },
                    "required": ["name"],
                },
            },
            {
                "name": "preview_profile",
                "description": "Read-only forecast of applying a saved theme: file diffs, resets, missing dependencies and restarts. Returns a previewToken for confirmation; does not install or apply anything.",
                "inputSchema": {"type": "object", "properties": {
                    "name": {"type": "string"}, "keepWallpaper": {"type": "boolean"}
                }, "required": ["name"]},
            },
            {
                "name": "apply_profile",
                "description": "Apply a saved theme: only what it defines changes (pre-apply backup first). Follows the user's automation choices (get_automation): 'always' steps are done and listed in installed, 'never' ones skipped. Returns what is left to ask the user about: dependencies (install_dependencies) and actions (run_action) — ask before doing any of them.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "previewToken": {"type": "string", "description": "Token from preview_profile. Rejects stale state and applies without implicit dependency installation; install dependencies separately, then preview again."},
                        "keepWallpaper": {"type": "boolean", "description": "Do not change the wallpaper even if the theme has one."},
                        "installMissing": {"type": "boolean", "description": "Install missing VS Code/Cursor/Zed themes from their official stores."},
                    },
                    "required": ["name", "previewToken"],
                },
            },
            {
                "name": "export_theme",
                "description": "Export a saved theme to a portable .zip (home folder written as $HOME, desktop icon positions left out, SHA-256 per file).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "path": {"type": "string", "description": "Output .zip path."},
                        "includeWallpaper": {"type": "boolean", "description": "Default true; leave out images you may not share."},
                    },
                    "required": ["name", "path"],
                },
            },
            {
                "name": "import_theme",
                "description": "Import a theme from a .zip export or an unpacked theme directory (does not apply it). Returns the theme name and what this machine lacks, with where to get it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "name": {"type": "string", "description": "Import under this name instead."},
                        "force": {"type": "boolean", "description": "Replace an existing theme of that name (backed up first)."},
                    },
                    "required": ["path"],
                },
            },
            {
                "name": "list_dependencies",
                "description": "What a saved theme needs that is not installed here (icons, cursors, GTK, Kvantum, Plasma style, widgets, decoration, editor themes) with the official source; packages carry the command to run.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "required": ["name"],
                },
            },
            {
                "name": "install_dependencies",
                "description": "Install a theme's missing dependencies from their official sources (KDE Store via kpackagetool6 or unpacking, VS Code Marketplace / Open VSX / Zed). System packages open ONE terminal on the user's screen with the install command — the user types their password there; the call waits until the terminal is closed and reports whether it worked. Never runs sudo. Ask the user first; search results (exact=false) need their confirmation.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "indexes": {"type": "array", "items": {"type": "integer"}, "description": "Indexes from list_dependencies; omit for all."},
                    },
                    "required": ["name"],
                },
            },
            {
                "name": "get_automation",
                "description": "The user's automation choices — what applying a theme may do by itself: store (downloads), packages (system packages in a terminal), restart_vesktop/discord/zen/firefox. Each ask | always | never, with what it does and a warning.",
                "inputSchema": {"type": "object", "properties": {}},
            },
            {
                "name": "set_automation",
                "description": "Change the user's automation choices. Only when the user explicitly asks for that change — never to make your own work easier.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "choices": {
                            "type": "object",
                            "description": "key → ask | always | never (keys from get_automation).",
                            "additionalProperties": {"type": "string", "enum": ["ask", "always", "never"]},
                        }
                    },
                    "required": ["choices"],
                },
            },
            {
                "name": "list_actions",
                "description": "Follow-up steps a theme leaves on this machine (restart a running Vesktop/Discord/Zen/Firefox, Telegram's one-time file choice), each with reason and warning. apply_profile already returns the ones the user wants to be asked about.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "required": ["name"],
                },
            },
            {
                "name": "run_action",
                "description": "Do one follow-up step (restart vesktop | discord | zen | firefox): the program quits normally and starts again. A call, screen share or download in it ends — tell the user the warning and run it only after they agree.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"id": {"type": "string", "enum": ["vesktop", "discord", "zen", "firefox"]}},
                    "required": ["id"],
                },
            },
            {
                "name": "palette_show",
                "description": "Live base16 palette (16 slots + accent, variant, source, contrast warnings, generated KDE scheme id) or null.",
                "inputSchema": {"type": "object", "properties": {}},
            },
            {
                "name": "palette_gallery",
                "description": "List base16 schemes: bundled + downloaded; online=true adds the tinted-theming catalogue (network).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "online": {"type": "boolean"},
                    },
                },
            },
            {
                "name": "palette_import",
                "description": "Make a scheme the live palette: 'gallery:<id>' or a path to a base16 YAML / palette JSON. Does not apply it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"source": {"type": "string"}},
                    "required": ["source"],
                },
            },
            {
                "name": "palette_set",
                "description": "Edit live palette colours: {\"slots\": {\"base08\": \"#e67e80\", \"accent\": \"#a7c080\"}}. Does not apply.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"slots": {"type": "object", "additionalProperties": {"type": "string"}}},
                    "required": ["slots"],
                },
            },
            {
                "name": "palette_from_wallpaper",
                "description": "Build the live palette from an image (default: current wallpaper) with the Rust k-means extractor. Does not apply.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"image": {"type": "string"}},
                },
            },
            {
                "name": "palette_from_colors",
                "description": "Build the live palette from weighted colours [{color:'#rrggbb', ratio:0.4}, …] (the shape of Kirigami.ImageColors.palette). Does not apply.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "colors": {"type": "array", "items": {"type": "object"}},
                        "name": {"type": "string"},
                    },
                    "required": ["colors"],
                },
            },
            {
                "name": "palette_apply",
                "description": "APPLY the live palette to the desktop (changes the live session): targets from kde, kitty, alacritty, kvantum (default kde,kitty,alacritty; kvantum switches the widget style; kvantumBase picks the installed base theme). Terminal configs are backed up first.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "targets": {"type": "array", "items": {"type": "string", "enum": list(TARGETS)}},
                        "kvantumBase": {"type": "string"},
                    },
                },
            },
            {
                "name": "app_store_search",
                "description": "Search an editor's theme store (network): zed (api.zed.dev) or vscode/cursor (Open VSX).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "appId": {"type": "string", "enum": ["zed", "vscode", "cursor"]},
                        "query": {"type": "string"},
                    },
                    "required": ["appId", "query"],
                },
            },
            {
                "name": "app_store_install",
                "description": "INSTALL a theme extension (external action): Zed themes are copied to ~/.config/zed/themes, VS Code/Cursor use their own --install-extension. Returns the theme ids now selectable with set_component.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "appId": {"type": "string", "enum": ["zed", "vscode", "cursor"]},
                        "extensionId": {"type": "string"},
                    },
                    "required": ["appId", "extensionId"],
                },
            },
        ]
        tools.extend(self._extra_schemas())
        readonly = {"get_capabilities", "get_theme_schema", "list_components", "list_installed_themes", "list_profiles", "get_profile_bundle", "validate_profile", "list_dependencies", "get_automation", "list_actions", "palette_show", "list_operations", "list_schedule", "dependency_status", "list_dependency_registry"}
        for tool in tools:
            tool["annotations"] = {"readOnlyHint": tool["name"] in readonly, "destructiveHint": tool["name"] in {"apply_profile", "apply_changes", "restore_operation", "recover_operation", "palette_apply"}, "openWorldHint": tool["name"] in {"install_dependencies", "app_store_install", "app_store_search", "palette_gallery"}}
        return tools

    @staticmethod
    def _extra_schemas():
        string = {"type": "string"}
        boolean = {"type": "boolean"}
        definitions = [
            ("get_capabilities", "Read core version, offline authoring capabilities and mutation boundaries.", {}, []),
            ("get_theme_schema", "Read theme schema, field ids/types, snapshot support and component mapping.", {}, []),
            ("prepare_theme", "Create a NEW theme package offline, atomically; never changes live appearance. values use field ids from get_theme_schema; optional saved palette generates KDE/Kitty/Alacritty. Existing installed resources may be copied; check licenses before publishing.", {"name": string, "values": {"type": "object"}, "description": string, "use_palette": boolean}, ["name", "values"]),
            ("list_operations", "Read last 10 completed changes plus interrupted operations, backup health and reports.", {}, []),
            ("restore_operation", "REPLACE current appearance with pre-operation backup, using a new safety transaction. Require explicit user approval and confirmed=true.", {"id":string, "confirmed":boolean}, ["id", "confirmed"]),
            ("recover_operation", "Recover interrupted operation. Default preserves files changed since interruption; force requires explicit user approval to overwrite conflicts.", {"id":string, "force":boolean}, ["id"]),
            ("list_schedule", "Read local-time rules, next events and last outcomes.", {}, []),
            ("save_schedule_rule", "Create/update persistent rule. Does not enable scheduler by itself. ISO weekdays: 1–7.", {"rule":{"type":"object","properties":{"id":string,"theme":string,"time":string,"days":{"type":"array","items":{"type":"integer","minimum":1,"maximum":7},"minItems":1},"enabled":boolean,"keep_wallpaper":boolean},"required":["id","theme","time","days","enabled"],"additionalProperties":False}}, ["rule"]),
            ("delete_schedule_rule", "Delete a persistent rule by id.", {"id":string}, ["id"]),
            ("enable_schedule", "Enable/disable background schedule and Plasma autostart. Enabling permits future live appearance changes; require user intent.", {"enabled":boolean}, ["enabled"]),
            ("list_dependency_registry", "Refresh dependency registry; returns stable ids, availability, ownership and supported operations. Does not install resources.", {}, []),
            ("start_dependency_operation", "START a verified asynchronous dependency job. May download/install/remove resources or open a visible package terminal. Require user approval; use ids from list_dependency_registry. Returns durable id; poll dependency_status and cancel_dependency_operation.", {"action":{"type":"string","enum":["install","remove","reinstall"]},"ids":{"type":"array","items":string,"minItems":1},"confirmed":boolean}, ["action","ids"]),
            ("dependency_status", "Read durable dependency job status, progress and verified results by operation id.", {"id":string}, ["id"]),
            ("cancel_dependency_operation", "Request safe cancellation; privileged package transaction is allowed to finish safely. Poll dependency_status for final result.", {"id":string}, ["id"]),
        ]
        return [{"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":False}} for name,description,properties,required in definitions]

    @staticmethod
    def _recover_operation(arguments):
        record = core_request("operation.recover", **arguments)
        return {"ok": not bool(record.get("recovery_errors")), **record}

    def _dependency_request(self, action, arguments):
        from core.cli_core import run_cli
        result = run_cli(["dependencies", action, arguments["id"]], timeout=30)
        if not result.success:
            raise ValueError(result.message)
        return json.loads(result.message) if action == "operation" else {"ok":True,"cancellation_requested":True}



    # ── tool implementations ───────────────────────────────────────────

    def _list_components(self, arguments: dict) -> dict:
        scope = _scope(arguments)
        return {
            "scope": scope,
            "groups": self.core.groups(scope),
            "applications": self.core.applications(),
            "sessionChanges": self.core.session_changes(),
        }

    def _list_installed_themes(self, arguments: dict) -> dict:
        component_id = str(arguments.get("componentId") or "")
        return _component_choices(self.core, component_id)

    def _list_profiles(self, arguments: dict) -> dict:
        return {"profiles": self.core.profiles()}

    def _get_profile_bundle(self, arguments: dict) -> dict:
        result = self.core.profile_bundle(str(arguments.get("name") or ""))
        if not result.success:
            raise ValueError(result.message)
        return {"profile": result.data}

    def _validate_profile(self, arguments: dict) -> dict:
        result = self.core.validate_profile_bundle(str(arguments.get("name") or ""))
        return {
            "valid": result.success,
            "message": result.message,
            "missing": (result.data or {}).get("missing", []),
        }

    def _set_component(self, arguments: dict) -> dict:
        component_id = str(arguments.get("componentId") or "")
        result = self.core.set_component(component_id, arguments.get("value"))
        return {"ok": result.success, "message": result.message, "sessionChanges": self.core.session_changes()}

    def _apply_changes(self, arguments: dict) -> dict:
        result = self.core.apply()
        return {"ok": result.success, "message": result.message, "sessionChanges": self.core.session_changes()}

    def _create_custom_theme(self, arguments: dict) -> dict:
        result = self.core.create_custom_theme(str(arguments.get("name") or ""))
        if not result.success:
            raise ValueError(result.message)
        return {"ok": True, "message": result.message, "assets": (result.data or {}).get("assets", [])}

    def _create_profile(self, arguments: dict) -> dict:
        name = str(arguments.get("name") or "")
        validate_profile_name(name)
        result = self.core.create_profile(name,
            components=dict(arguments.get("components") or {}),
            applications=dict(arguments.get("applications") or {}),
            description=str(arguments.get("description") or ""),
            include_wallpaper=bool(arguments.get("includeWallpaper", True)))
        if not result.success:
            raise ValueError(result.message)
        return {"ok": True, "profile": result.data, "messages": [result.message]}

    def _export_theme(self, arguments: dict) -> dict:
        result = self.profiles.export_theme(
            str(arguments.get("name") or ""),
            str(arguments.get("path") or ""),
            bool(arguments.get("includeWallpaper", True)),
        )
        if not result.success:
            raise ValueError(result.message)
        return {"ok": True, "message": result.message}

    def _import_theme(self, arguments: dict) -> dict:
        result = self.profiles.import_theme(
            str(arguments.get("path") or ""),
            str(arguments.get("name") or ""),
            bool(arguments.get("force", False)),
        )
        if not result.success:
            raise ValueError(result.message)
        return {"ok": True, **(result.data or {})}

    def _list_dependencies(self, arguments: dict) -> dict:
        result = self.profiles.dependencies(str(arguments.get("name") or ""))
        if not result.success:
            raise ValueError(result.message)
        return {"dependencies": result.data or []}

    def _install_dependencies(self, arguments: dict) -> dict:
        indexes = arguments.get("indexes")
        result = self.profiles.install_dependencies(
            str(arguments.get("name") or ""),
            None if indexes is None else [int(i) for i in indexes],
        )
        return {"ok": result.success, "message": result.message, "results": result.data or []}

    def _preview_profile(self, arguments: dict) -> dict:
        result = self.profiles.preview(str(arguments.get("name") or ""),
            bool(arguments.get("keepWallpaper", False)), use_settings=False)
        if not result.success:
            raise ValueError(result.message)
        return {"ok": True, "previewToken": (result.data or {}).get("token", ""), **(result.data or {})}

    def _apply_profile(self, arguments: dict) -> dict:
        if not str(arguments.get("previewToken") or "").strip():
            raise ValueError("previewToken is required; call preview_profile and review its result first")
        result = self.profiles.apply(str(arguments.get("name") or ""),
            keep_wallpaper=bool(arguments.get("keepWallpaper", False)),
            install_missing=bool(arguments.get("installMissing", False)),
            skip_dependencies="previewToken" in arguments,
            preview_token=arguments.get("previewToken"), source="mcp")
        return {"ok":result.success, "message":result.message, **(result.data or {})}

    # ── automation & follow-up actions (ask the user before acting) ────

    def _get_automation(self, arguments: dict) -> dict:
        from core.cli_core import CliSettingsCore

        return CliSettingsCore().automation()

    def _set_automation(self, arguments: dict) -> dict:
        from core.cli_core import CliSettingsCore

        choices = arguments.get("choices") or {}
        if not isinstance(choices, dict) or not choices:
            raise ValueError("choices must be an object key → ask|always|never")
        result = CliSettingsCore().set_automation({str(k): str(v) for k, v in choices.items()})
        if not result.success:
            raise ValueError(result.message)
        return CliSettingsCore().automation()

    def _list_actions(self, arguments: dict) -> dict:
        result = self.profiles.actions(str(arguments.get("name") or ""))
        if not result.success:
            raise ValueError(result.message)
        return {"actions": result.data or []}

    def _run_action(self, arguments: dict) -> dict:
        result = self.profiles.run_action(str(arguments.get("id") or ""))
        return {"ok": result.success, "message": result.message}


    # ── palette (Rust CLI via PaletteCore) ─────────────────────────────

    def _palette_state(self, result) -> dict:
        if not result.success:
            raise ValueError(result.message)
        return {"ok": True, "palette": self.palette.show()}

    def _palette_show(self, arguments: dict) -> dict:
        return {"palette": self.palette.show()}

    def _palette_gallery(self, arguments: dict) -> dict:
        result = self.palette.gallery(str(arguments.get("query") or ""), bool(arguments.get("online")))
        if not result.success:
            raise ValueError(result.message)
        return {"items": (result.data or {}).get("items", [])}

    def _palette_import(self, arguments: dict) -> dict:
        source = str(arguments.get("source") or "").strip()
        if not source:
            raise ValueError("source is required ('gallery:<id>' or a file path)")
        if source.startswith("gallery:"):
            return self._palette_state(self.palette.import_gallery(source[len("gallery:"):]))
        return self._palette_state(self.palette.import_file(source))

    def _palette_set(self, arguments: dict) -> dict:
        slots = {str(k): str(v) for k, v in dict(arguments.get("slots") or {}).items()}
        return self._palette_state(self.palette.set_slots(slots))

    def _palette_from_wallpaper(self, arguments: dict) -> dict:
        return self._palette_state(self.palette.from_wallpaper(str(arguments.get("image") or "")))

    def _palette_from_colors(self, arguments: dict) -> dict:
        colors = [dict(c) for c in list(arguments.get("colors") or []) if isinstance(c, dict)]
        return self._palette_state(
            self.palette.from_colors(colors, name=str(arguments.get("name") or ""), source="mcp")
        )

    def _palette_apply(self, arguments: dict) -> dict:
        targets = [str(t) for t in (arguments.get("targets") or DEFAULT_TARGETS)]
        unknown = [t for t in targets if t not in TARGETS]
        if unknown:
            raise ValueError(f"Unknown targets: {', '.join(unknown)} (known: {', '.join(TARGETS)})")
        result = self.palette.apply(targets, str(arguments.get("kvantumBase") or ""))
        if not result.success:
            raise ValueError(result.message)
        return {"ok": True, "report": result.message.splitlines()}

    # ── editor theme stores ────────────────────────────────────────────

    def _app_store_search(self, arguments: dict) -> dict:
        result = self.core.store_search(str(arguments.get("appId") or ""), str(arguments.get("query") or ""))
        if not result.success:
            raise ValueError(result.message)
        return {"items": (result.data or {}).get("items", [])}

    def _app_store_install(self, arguments: dict) -> dict:
        result = self.core.store_install(
            str(arguments.get("appId") or ""), str(arguments.get("extensionId") or "")
        )
        if not result.success:
            raise ValueError(result.message)
        return {"ok": True, "message": result.message, "install": result.data}


def main() -> int:
    if os.environ.get("KTM_THEME_NO_APPLY") == "1":
        print(
            "kolira MCP: KTM_THEME_NO_APPLY=1 is set; external apply commands are disabled",
            file=sys.stderr,
        )
    server = KoliraMcpServer()
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError as exc:
            response = {
                "jsonrpc": "2.0",
                "id": None,
                "error": {"code": -32700, "message": f"Parse error: {exc}"},
            }
        else:
            response = server.handle(message)
        if response is not None:
            sys.stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
            sys.stdout.flush()
    CoreFactory.close_session()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
