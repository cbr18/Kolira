"""Asynchronous Qt adapter to the Rust dependency registry and JSONL jobs."""
import json
import sys

from core.core_messages import localize

from PySide6.QtCore import QObject, Property, QProcess, QTimer, Signal, Slot

from core.cli_core import dependency_cli_process


class DependencyController(QObject):
    changed = Signal()
    finished = Signal(bool, str)
    applyRequested = Signal(str, bool)
    retryApply = Signal(str, bool)
    policySaved = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._rows = []
        self._process = None
        self._buffer = b""
        self._errors = b""
        self._message = ""
        self._operation = ""
        self._action = ""
        self._result = None
        self._cancel_requested = False
        self._after_install = None
        self._log = []
        self._policy_process = None
        self._pending_choice_action = None

    @Property(list, notify=changed)
    def rows(self):
        return self._rows

    @Property(bool, notify=changed)
    def busy(self):
        return self._process is not None or self._policy_process is not None

    @Property(str, notify=changed)
    def message(self):
        return self._message

    @Property(list, notify=changed)
    def log(self):
        return self._log

    @Property(bool, notify=changed)
    def canCancel(self):
        return self.busy and self._action in ("install", "remove", "reinstall") and not self._cancel_requested

    def _start(self, args, action):
        if self.busy:
            return False
        self._buffer = b""
        self._errors = b""
        self._operation = ""
        self._result = None
        self._cancel_requested = False
        self._action = action
        process = dependency_cli_process(self, args)
        self._process = process
        process.readyReadStandardOutput.connect(self._read)
        process.readyReadStandardError.connect(self._read_error)
        process.finished.connect(self._done)
        process.errorOccurred.connect(self._error)
        process.start()
        if action == "list":
            timer = QTimer(process)
            timer.setSingleShot(True)
            timer.setInterval(30000)
            timer.timeout.connect(process.kill)
            timer.start()
        self.changed.emit()
        return True

    @Slot()
    def refresh(self):
        self._start(["register-app", f"--python={sys.executable}", "--json"], "list")

    def _read_error(self):
        if self._process:
            self._errors = (self._errors + bytes(self._process.readAllStandardError()))[-65536:]

    def _read(self):
        if not self._process:
            return
        self._buffer += bytes(self._process.readAllStandardOutput())
        if self._action == "list":
            return
        while b"\n" in self._buffer:
            line, self._buffer = self._buffer.split(b"\n", 1)
            try:
                event = json.loads(line)
            except (ValueError, UnicodeError):
                continue
            kind = event.get("event")
            if kind == "started":
                self._operation = event.get("operation", "")
                if self._cancel_requested:
                    self._send_cancel()
            elif kind == "progress":
                stage = {
                    "starting": self.tr("Starting"), "downloading": self.tr("Downloading"),
                    "installing": self.tr("Installing"), "checking": self.tr("Checking"),
                    "removing": self.tr("Removing"), "waiting": self.tr("Waiting for the terminal"),
                }.get(event.get("stage"), self.tr("Working"))
                self._message = f"{stage}: {localize(event.get('message', ''))}"
            elif kind == "item":
                event = {**event, "message": localize(str(event.get("message", "")))}
                self._log = [*self._log, event]
                self._message = str(event.get("message", ""))
            elif kind == "finished":
                self._result = event
            self.changed.emit()

    def _error(self, error):
        if error == QProcess.ProcessError.FailedToStart:
            self._complete(False, self._process.errorString())

    def _done(self, code, status):
        self._read()
        self._read_error()
        success = code == 0 and status == QProcess.ExitStatus.NormalExit
        message = self._errors.decode(errors="replace").strip()
        if not success and not message:
            message = self._process.errorString()
        if self._action == "list" and success:
            try:
                rows = json.loads(self._buffer)
                if not isinstance(rows, list):
                    raise ValueError("expected dependency list")
                self._rows = rows
            except (ValueError, UnicodeError) as exc:
                success, message = False, str(exc)
        elif self._action != "list":
            success = success and bool(self._result and self._result.get("ok"))
            if not message:
                message = self.tr("Completed") if success else (
                    self.tr("Cancelled; the theme was not applied") if self._cancel_requested
                    else self.tr("Some operations failed. Review the results and retry."))
        self._complete(success, message)

    def _complete(self, success, message):
        action = self._action
        process, self._process = self._process, None
        if process is not None:
            process.deleteLater()
        if action != "list" or not success:
            self._message = localize(message)
        self.changed.emit()
        self.finished.emit(success, message)
        if action == "list":
            return
        after, self._after_install = self._after_install, None
        if after:
            if success:
                self.applyRequested.emit(*after)
            elif not self._cancel_requested:
                self.retryApply.emit(*after)
        # Keep the result visible while updating factual availability.
        self.refresh()

    @Slot(list, str, bool)
    def operate(self, ids, action, confirmed=False):
        if action not in ("install", "remove", "reinstall") or not ids or self.busy:
            return
        args = [action, *[str(i) for i in ids], "--events=jsonl"]
        if confirmed:
            args.append("--confirm-shared")
        self._log = []
        self._message = self.tr("Starting…")
        self._start(args, action)

    @Slot(str, list, bool, bool)
    def installThemeDependencies(self, theme, resource_ids, apply_after, keep_wallpaper):
        """The dialog passes stable resource ids supplied by Rust needed()."""
        if self._policy_process is not None:
            self._pending_choice_action = ("install", (theme, resource_ids, apply_after, keep_wallpaper))
            return
        if self.busy:
            return
        if apply_after:
            self._after_install = (theme, keep_wallpaper)
        self.operate(resource_ids, "install", False)

    @Slot(str, bool)
    def skipAndApply(self, theme, keep_wallpaper):
        if self._policy_process is not None:
            self._pending_choice_action = ("apply", (theme, keep_wallpaper))
        elif not self.busy:
            self.applyRequested.emit(theme, keep_wallpaper)

    def _send_cancel(self):
        if not self._operation:
            return
        process = dependency_cli_process(self, ["cancel", self._operation])
        process.finished.connect(lambda *args: process.deleteLater())
        process.start()

    @Slot()
    def cancel(self):
        if self.canCancel:
            self._cancel_requested = True
            self._message = self.tr("Stopping… Package transactions may need to finish in the terminal.")
            self._send_cancel()
            self.changed.emit()

    @Slot(str, list, list, bool)
    def rememberChoice(self, theme, selected_ids, groups, install):
        """Rust validates the complete class selection before saving policies."""
        if self.busy or not groups:
            return
        args = ["remember", f"--theme={theme}",
                "--policy=always" if install else "--policy=never",
                *[f"--selected={i}" for i in selected_ids], *[f"--group={g}" for g in groups]]
        process = dependency_cli_process(self, args)
        self._policy_process = process
        def done(code, status):
            if self._policy_process is not process:
                return
            self._policy_process = None
            if code:
                self._message = bytes(process.readAllStandardError()).decode(errors="replace") or process.errorString()
                self.changed.emit()
                self._pending_choice_action = None
            else:
                self.policySaved.emit()
                pending, self._pending_choice_action = self._pending_choice_action, None
                if pending:
                    action, args = pending
                    if action == "install":
                        self.installThemeDependencies(*args)
                    else:
                        self.skipAndApply(*args)
            self.changed.emit()
            process.deleteLater()
        process.finished.connect(done)
        process.errorOccurred.connect(lambda error: done(1, QProcess.ExitStatus.CrashExit) if error == QProcess.ProcessError.FailedToStart else None)
        timer = QTimer(process)
        timer.setSingleShot(True)
        timer.setInterval(15000)
        timer.timeout.connect(process.kill)
        timer.start()
        process.start()
        self.changed.emit()
