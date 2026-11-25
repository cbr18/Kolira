"""Quick Switch controller: theme feed, pre-flight and the apply transaction.

The overlay is its own process. Everything that changes the desktop goes
through the Rust core, exactly like the main GUI: `quick_switch.cards` for the
carousel, `quick_switch.prepare` for the confirmation token, `kolira apply`
for the transaction, `operation.list` for the real stages.

There is no progress channel in the core. The stage scale is built from the
durable operation record, and the moving bar inside a stage is honest about
being an animation, not a measurement.
"""

from __future__ import annotations

import json
import sys
import threading
from datetime import datetime
from typing import Optional

from PySide6.QtCore import (
    Property,
    QCoreApplication,
    QObject,
    QProcess,
    QTimer,
    Signal,
    Slot,
)

from core.cli_core import core_cli_process, core_request, resolve_cli_bin

# ── phases ────────────────────────────────────────────────────────────────────
LOADING = "loading"
SELECT = "select"
CLOSING = "closing"
PREPARING = "preparing"
APPLYING = "applying"
REVEALING = "revealing"
ATTENTION = "attention"
ERROR = "error"

# The record states the core actually writes, in the order they happen.
STAGE_BY_STATE = {
    "preparing": 0,
    "applying": 1,
    "succeeded": 2,
    "recovering": 1,
    "rolled_back": 2,
    "recovery_failed": 2,
    "failed_preparation": 2,
    "skipped": 2,
}


def appearance_is_busy() -> bool:
    """True when another process is applying a theme right now.

    The overlay must not open mid-transaction: both its pre-flight and its
    apply take the cross-process appearance lock, so opening while a theme is
    still being applied only ends in a confusing "could not apply" failure.
    The probe fails open (returns False) when the core cannot be reached — a
    broken check must never stop the overlay from opening.
    """
    try:
        data = core_request("operation.busy") or {}
    except (RuntimeError, ValueError):
        return False
    return bool(data.get("busy"))


def _detect_system_reduced_motion() -> bool:
    """KDE's animation factor: 0 means the user turned animations off."""
    import subprocess

    try:
        proc = subprocess.run(
            [
                "kreadconfig6",
                "--file",
                "kdeglobals",
                "--group",
                "KDE",
                "--key",
                "AnimationDurationFactor",
            ],
            capture_output=True,
            text=True,
            timeout=5,
        )
        return float(proc.stdout.strip()) <= 0.0
    except (OSError, ValueError, subprocess.SubprocessError):
        return False


class QuickSwitchController(QObject):
    """State machine behind the overlay."""

    themesChanged = Signal()
    indexChanged = Signal()
    phaseChanged = Signal()
    stageChanged = Signal()
    statusChanged = Signal()
    preparedChanged = Signal()
    closed = Signal()

    def __init__(self, parent: Optional[QObject] = None, preview_only: bool = False) -> None:
        super().__init__(parent)
        self._themes: list[dict] = []
        self._index = 0
        self._phase = LOADING
        self._stage = 0
        self._status = ""
        self._message = ""
        self._attention: list[str] = []
        self._active_name = ""
        self._settings: dict = {}
        self._system_reduced_motion = _detect_system_reduced_motion()
        self._prepared: dict[str, dict] = {}
        self._prepared_lock = threading.Lock()
        self._inflight: set[str] = set()
        self._pending_name = ""
        self._prepare_attempts = 0
        self._prepare_timer = QTimer(self)
        self._prepare_timer.setInterval(120)
        self._prepare_timer.setSingleShot(True)
        self._prepare_timer.timeout.connect(self._check_prepared)
        self._token = ""
        self._apply_process: Optional[QProcess] = None
        self._hung = False
        self._poll = QTimer(self)
        self._poll.setInterval(700)
        self._poll.timeout.connect(self._poll_operation)
        self._preview_only = preview_only

    # ── properties ────────────────────────────────────────────────────────────

    def _get_themes(self) -> list[dict]:
        return self._themes

    themes = Property("QVariantList", _get_themes, notify=themesChanged)

    def _get_index(self) -> int:
        return self._index

    index = Property(int, _get_index, notify=indexChanged)

    def _get_phase(self) -> str:
        return self._phase

    phase = Property(str, _get_phase, notify=phaseChanged)

    def _get_stage(self) -> int:
        return self._stage

    stage = Property(int, _get_stage, notify=stageChanged)

    def _get_status(self) -> str:
        return self._status

    status = Property(str, _get_status, notify=statusChanged)

    def _get_message(self) -> str:
        return self._message

    message = Property(str, _get_message, notify=statusChanged)

    def _get_attention(self) -> list[str]:
        return self._attention

    attention = Property("QVariantList", _get_attention, notify=statusChanged)

    def _get_current(self) -> dict:
        if 0 <= self._index < len(self._themes):
            return self._themes[self._index]
        return {}

    current = Property("QVariantMap", _get_current, notify=indexChanged)

    def _get_settings(self) -> dict:
        return self._settings

    settings = Property("QVariantMap", _get_settings, notify=themesChanged)

    def _get_preview_only(self) -> bool:
        return self._preview_only

    previewOnly = Property(bool, _get_preview_only, notify=themesChanged)

    def _get_busy(self) -> bool:
        return self._phase in (CLOSING, PREPARING, APPLYING, REVEALING)

    busy = Property(bool, _get_busy, notify=phaseChanged)

    def _get_reduced_motion(self) -> bool:
        """System setting unless the user overrode it in Quick Switch."""
        mode = str(self._settings.get("reduced_motion", "system"))
        if mode == "on":
            return True
        if mode == "off":
            return False
        return self._system_reduced_motion

    reducedMotion = Property(bool, _get_reduced_motion, notify=themesChanged)

    # ── helpers ───────────────────────────────────────────────────────────────

    def _log(self, event: str, **fields: object) -> None:
        """One stderr line.

        The overlay usually starts from KDE's launcher, which puts its stderr
        in the journal: `journalctl --user | grep quick-switch`. Python must
        not write files itself (all file writes belong to the Rust core).
        """
        detail = " ".join(f"{key}={value}" for key, value in fields.items())
        print(
            f"{datetime.now():%Y-%m-%d %H:%M:%S} quick-switch {event} {detail}",
            file=sys.stderr,
            flush=True,
        )

    def _set_phase(self, phase: str) -> None:
        if phase != self._phase:
            self._phase = phase
            self.phaseChanged.emit()

    def _set_status(self, status: str) -> None:
        if status != self._status:
            self._status = status
            self.statusChanged.emit()

    def _set_stage(self, stage: int) -> None:
        if stage != self._stage:
            self._stage = stage
            self.stageChanged.emit()

    def _name_at(self, index: int) -> str:
        if 0 <= index < len(self._themes):
            return self._themes[index].get("name", "")
        return ""

    # ── data ──────────────────────────────────────────────────────────────────

    @Slot()
    def initialize(self) -> None:
        """Read settings and the card feed; called once the window is up."""
        try:
            self._settings = core_request("quick_switch.get") or {}
        except (RuntimeError, ValueError):
            self._settings = {}
        try:
            data = core_request("quick_switch.cards") or {}
        except (RuntimeError, ValueError) as exc:
            self._fail(self.tr("Не удалось прочитать список тем: {error}").format(error=exc))
            return
        self._themes = list(data.get("themes") or [])
        self._active_name = data.get("active") or ""
        active = next(
            (
                position
                for position, theme in enumerate(self._themes)
                if theme.get("active")
            ),
            0,
        )
        self._index = min(active, max(len(self._themes) - 1, 0))
        self._status = ""
        self._message = ""
        self._attention = []
        self._stage = 0
        self._set_phase(SELECT)
        self.themesChanged.emit()
        self.indexChanged.emit()
        self._prefetch(self._name_at(self._index))
        self._warm_thumbnails()

    @Slot(int)
    def selectIndex(self, index: int) -> None:
        if self._phase != SELECT or index == self._index:
            return
        if not (0 <= index < len(self._themes)):
            return
        self._index = index
        self.indexChanged.emit()
        self._prefetch(self._name_at(index))

    @Slot(int)
    def step(self, delta: int) -> None:
        if not self._themes:
            return
        self.selectIndex((self._index + delta) % len(self._themes))

    def _warm_thumbnails(self) -> None:
        """Render and cache the small card wallpapers in the background.

        The first ever open pays for it; after that the overlay reads tiny
        JPEGs from `$XDG_CACHE_HOME/kolira/quick-switch/thumbs` and cards
        appear immediately.
        """
        pending = [
            theme["name"]
            for theme in self._themes
            if theme.get("preview") and not theme.get("thumbPath")
        ]
        if not pending:
            return

        def work() -> None:
            for name in pending:
                try:
                    thumb = core_request("quick_switch.thumbnail", name=name)
                except Exception:  # noqa: BLE001 - a background warm-up must never crash
                    continue
                path = (thumb or {}).get("path")
                if not path:
                    continue
                with self._prepared_lock:
                    for theme in self._themes:
                        if theme.get("name") == name:
                            theme["thumbPath"] = path
                self.themesChanged.emit()

        threading.Thread(target=work, daemon=True).start()

    def _prefetch(self, name: str) -> None:
        """Warm the pre-flight for a card while the user is still browsing.

        Prepares may run in parallel: they are read-only, and waiting for the
        previously started one would delay the theme the user just picked.
        Errors are not cached: a transient failure (a busy lock) must not
        poison the rest of the session.
        """
        if not name or name in self._prepared:
            return
        with self._prepared_lock:
            if name in self._inflight:
                return
            self._inflight.add(name)

        def work() -> None:
            try:
                prepared = core_request("quick_switch.prepare", name=name)
            except Exception as exc:  # noqa: BLE001 - reported to the user, never raised
                prepared = {"error": str(exc)}
            with self._prepared_lock:
                self._inflight.discard(name)
                if prepared.get("error"):
                    self._prepared.pop(name, None)
                else:
                    self._prepared[name] = prepared
            self._log("prepare", name=name, error=prepared.get("error", ""))
            self.preparedChanged.emit()

        threading.Thread(target=work, daemon=True).start()

    def _prepared_for(self, name: str) -> Optional[dict]:
        with self._prepared_lock:
            return self._prepared.get(name)

    # ── flow ──────────────────────────────────────────────────────────────────

    @Slot()
    def confirm(self) -> None:
        """Enter the closing animation; the transaction starts after it."""
        if self._phase != SELECT or self._preview_only:
            return
        name = self._name_at(self._index)
        if not name:
            return
        self._set_status("")
        self._set_stage(0)
        self._prefetch(name)
        self._set_phase(CLOSING)

    @Slot()
    def beginApply(self) -> None:
        """QML calls this when the closing animation has finished.

        The pre-flight usually finished while the user was browsing. If it did
        not, wait without blocking the GUI thread: joining the worker here
        would freeze the animation on a black screen.
        """
        if self._phase != CLOSING:
            return
        name = self._name_at(self._index)
        if not name:
            self._fail(self.tr("Не выбрана тема"))
            return
        self._pending_name = name
        self._prefetch(name)
        prepared = self._prepared_for(name)
        if prepared is None or prepared.get("error"):
            self._prepare_attempts = 0
            self._set_stage(0)
            self._set_status(self.tr("Подготовка…"))
            self._set_phase(PREPARING)
            self._prepare_timer.start()
            return
        self._apply_prepared(name, prepared)

    def _check_prepared(self) -> None:
        """Poll for the pre-flight result; never block the event loop."""
        if self._phase != PREPARING:
            return
        name = self._pending_name
        prepared = self._prepared_for(name)
        if prepared is None:
            self._prepare_attempts += 1
            if self._prepare_attempts > 60:
                self._fail(self.tr("Не удалось подготовить применение. Попробуйте снова."))
                return
            self._prepare_timer.start()
            return
        if prepared.get("error"):
            self._fail(self.tr("Не удалось подготовить применение. Попробуйте снова."))
            return
        self._apply_prepared(name, prepared)

    def _apply_prepared(self, name: str, prepared: dict) -> None:
        self._token = prepared.get("token") or ""
        blocking = self._blocking_lines(prepared)
        if blocking:
            self._attention = blocking
            self._set_status(self.tr("Тема требует внимания"))
            self._set_phase(ATTENTION)
            return
        self._set_stage(0)
        self._start_apply(name)

    @staticmethod
    def _blocking_lines(prepared: dict) -> list[str]:
        """What the overlay cannot decide or install for the user.

        `actions` (restart prompts) are deliberately not here: Quick Switch
        never restarts applications and does not ask about them (Q16).
        """
        lines: list[str] = []
        for item in prepared.get("missing") or []:
            lines.append(
                QCoreApplication.translate(
                    "QuickSwitchController", "Нет ресурса: {name} ({field})"
                ).format(
                    name=item.get("name", "?"), field=item.get("field", "?")
                )
            )
        for item in prepared.get("dependencies") or []:
            lines.append(
                QCoreApplication.translate(
                    "QuickSwitchController", "Зависимость: {name}"
                ).format(name=item.get("name", "?"))
            )
        return lines

    def _start_apply(self, name: str) -> None:
        self._set_phase(APPLYING)
        self._set_stage(1)
        self._set_status(self.tr("Применяю…"))
        self._log("apply.start", name=name, token=(self._token or "")[:8])
        process = QProcess(self)
        process.setProgram(resolve_cli_bin())
        process.setArguments(
            ["apply", name, "--json", f"--preview-token={self._token}"]
        )
        process.finished.connect(
            lambda code, status: self._apply_finished(code, status, process)
        )
        self._apply_process = process
        self._poll.start()
        process.start()

    def _apply_finished(self, code: int, status: QProcess.ExitStatus, process: QProcess) -> None:
        self._poll.stop()
        raw = bytes(process.readAllStandardOutput()).decode("utf-8", "replace").strip()
        errors = bytes(process.readAllStandardError()).decode("utf-8", "replace").strip()
        self._log(
            "apply.finished",
            code=code,
            status=status.name,
            program=process.program(),
            error=errors[:400],
            stdout=raw[:400],
        )
        if status == QProcess.CrashExit and code == 0:
            # The process could not be started at all (missing binary, ENOENT).
            self._fail(self.tr("Не удалось запустить kolira: {error}").format(error=process.errorString()))
            return
        if code != 0:
            self._fail(errors or raw or self.tr("Применение не удалось"))
            return
        try:
            report = json.loads(raw)
        except ValueError:
            self._fail(raw or self.tr("Ядро вернуло не JSON"))
            return
        self._set_stage(2)
        self._set_status(self.tr("Готово"))
        self._set_phase(REVEALING)

    def _poll_operation(self) -> None:
        """Real stage from the durable operation record."""
        name = self._name_at(self._index)
        if not name:
            return
        process = core_cli_process(self, "operation.list", {})
        process.finished.connect(
            lambda _code, _status, p=process, name=name: self._read_operation(p, name)
        )
        process.start()

    def _read_operation(self, process: QProcess, name: str) -> None:
        raw = bytes(process.readAllStandardOutput()).decode("utf-8", "replace")
        try:
            payload = json.loads(raw)
        except ValueError:
            return
        operations = (payload.get("data") or {}).get("operations") or []
        for record in operations:
            if record.get("profile") != name:
                continue
            state = record.get("state") or ""
            stage = STAGE_BY_STATE.get(state)
            if stage is not None:
                self._set_stage(stage)
            if state == "succeeded":
                self._set_status(self.tr("Готово"))
            return

    def _fail(self, message: str) -> None:
        self._attention = []
        self._message = message
        self._set_phase(ERROR)

    @Slot()
    def retry(self) -> None:
        """Back to the carousel after an error or a refused confirmation."""
        self._message = ""
        self._attention = []
        self._set_stage(0)
        self._set_status("")
        self._set_phase(SELECT)

    @Slot()
    def requestToggle(self) -> None:
        """A second launch (the hotkey again) closes the open overlay."""
        if self._phase in (PREPARING, APPLYING):
            # Never hide a running transaction behind a hotkey.
            self._log("toggle.ignored", phase=self._phase)
            return
        self.closed.emit()

    @Slot()
    def dismiss(self) -> None:
        """Close without touching the desktop."""
        if self._phase == APPLYING and not self._hung:
            # Esc must not abort a running transaction (Q9).
            return
        self.closed.emit()

    @Slot()
    def watchdog(self) -> None:
        """The core stopped answering; let the user out of the black screen."""
        if self._phase != APPLYING:
            return
        self._hung = True
        self._message = self.tr("Kolira не отвечает. Esc закроет оверлей.")
        self._set_status(self._message)

    @Slot()
    def finishReveal(self) -> None:
        """QML calls this when the reveal animation has finished."""
        self.closed.emit()

    @Slot()
    def openInKolira(self) -> None:
        """Hand over to the main GUI for anything the overlay cannot do."""
        QProcess.startDetached(resolve_cli_bin("kolira-gui"))
        self.closed.emit()