"""Asynchronous Qt transport for Rust operation history and scheduling."""
import json
import re

from core.core_messages import localize, state_label, source_label

from PySide6.QtCore import QObject, Property, QProcess, Signal, Slot
from PySide6.QtGui import QDesktopServices
from PySide6.QtCore import QUrl

from core.cli_core import core_cli_process


class HistoryScheduleController(QObject):
    changed = Signal()
    finished = Signal(bool, str)
    restored = Signal()
    errorFound = Signal(str, str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._process = None
        self._history = []
        self._next_offset = None
        self._points = []
        self._last_profile_apply = ""
        self._schedule = {}
        self._message = ""
        self._pending = []
        self._reported_errors = set()

    @Property(list, notify=changed)
    def history(self):
        return self._history

    @Property(list, notify=changed)
    def points(self):
        return self._points

    @Property(bool, notify=changed)
    def moreHistory(self):
        return self._next_offset is not None

    @Property(str, notify=changed)
    def lastProfileApply(self):
        return self._last_profile_apply

    @Property('QVariantMap', notify=changed)
    def schedule(self):
        return self._schedule

    @Property(bool, notify=changed)
    def busy(self):
        return self._process is not None

    @Property(str, notify=changed)
    def message(self):
        return self._message

    def _request(self, method, args=None):
        args = args or {}
        if self.busy:
            request = (method, args)
            if method.endswith('.list') and request not in self._pending:
                self._pending.append(request)
            return
        process = core_cli_process(self, method, args)
        self._process = process
        process.finished.connect(lambda code, status: self._done(method, args, code == 0 and status == QProcess.NormalExit))
        process.errorOccurred.connect(lambda error: self._done(method, args, False) if error == QProcess.FailedToStart else None)
        process.start()
        self.changed.emit()

    def _done(self, method, args, ok):
        process = self._process
        if process is None:
            return
        error = bytes(process.readAllStandardError()).decode(errors='replace').strip()
        try:
            response = json.loads(bytes(process.readAllStandardOutput()))
            if response.get('api') != 1:
                raise ValueError(self.tr('Incompatible core API; update the application'))
            ok = ok and bool(response.get('ok'))
            error = response.get('error') or error
            data = response.get('data') or {}
            if ok:
                if method == 'operation.list':
                    self._last_profile_apply = data.get('last_profile_apply') or ""
                    rows = [{**row, "stateText": state_label(row.get("state", "")),
                        "sourceText": source_label(row.get("source", "")), "errorText": localize(row.get("error") or "")}
                        for row in data.get('operations', [])]
                    if args.get("offset"):
                        known = {row["id"] for row in self._history}
                        self._history = [*self._history, *(row for row in rows if row["id"] not in known)]
                    else:
                        self._history = rows
                    self._next_offset = data.get("next_offset")
                    for record in rows:
                        if record.get('source') == 'schedule' and record.get('error') and record['id'] not in self._reported_errors:
                            self._reported_errors.add(record['id'])
                            self.errorFound.emit(record['id'], record['error'])
                elif method == 'restore_point.list':
                    self._points = data.get('points', [])
                elif method.startswith('schedule.'):
                    self._schedule = data
                    for rule in self._schedule.get("rules", []):
                        if rule.get("last"):
                            last = rule["last"]
                            last["stateText"] = state_label(last.get("state", ""))
                            last["messageText"] = localize(last.get("message", ""))
                elif method == 'operation.recover' and data.get('recovery_errors'):
                    ok, error = False, '\n'.join(data['recovery_errors'])
        except (ValueError, TypeError) as exc:
            ok, error = False, error or str(exc)
        self._process = None
        process.deleteLater()
        if not method.endswith('.list') or not ok:
            self._message = '' if ok else localize(error or process.errorString())
        if not method.endswith('.list'):
            self.finished.emit(ok, self._message)
            if method.startswith('operation.') or method == 'restore_point.restore':
                if ok:
                    from theme_controller import reload_app_look
                    reload_app_look()
                    self.restored.emit()
                self._pending.append(('operation.list', {'limit': 40}))
            if method.startswith('restore_point.'):
                self._pending.append(('restore_point.list', {}))
        self.changed.emit()
        if self._pending:
            self._request(*self._pending.pop(0))

    @Slot()
    def refreshHistory(self):
        self._request('operation.list', {'limit': 40})

    @Slot()
    def loadMoreHistory(self):
        if self._next_offset is not None and not self.busy:
            self._request('operation.list', {'limit': 40, 'offset': self._next_offset})

    @Slot()
    def refreshPoints(self):
        self._request('restore_point.list')

    @Slot(str, str)
    def savePoint(self, point_id, name):
        method = 'restore_point.rename' if point_id else 'restore_point.create'
        self._request(method, {'id': point_id, 'name': name})

    @Slot(str, bool)
    def pinPoint(self, point_id, pinned):
        self._request('restore_point.pin', {'id': point_id, 'pinned': pinned})

    @Slot(str, bool)
    def confirmPoint(self, point_id, restore):
        method = 'restore_point.restore' if restore else 'restore_point.delete'
        self._request(method, {'id': point_id, 'confirmed': True, 'source':'gui'})

    @Slot(str, result=str)
    def operationIdFromMessage(self, message):
        """Extract the final Rust transaction context, after any cause's ID.

        Rust's diagnostic protocol stays English; the full unrecognised
        transaction message passes through the GUI localizer unchanged.
        """
        ids = re.findall(r'operation (apply-[A-Za-z0-9]+)', message)
        return ids[-1] if ids else ''

    @Slot()
    def refreshSchedule(self):
        self._request('schedule.list')

    @Slot(bool)
    def enableSchedule(self, enabled):
        self._request('schedule.enable', {'enabled': enabled})

    @Slot('QVariantMap')
    def saveRule(self, rule):
        self._request('schedule.put', {'rule': rule})

    @Slot(str)
    def deleteRule(self, rule_id):
        self._request('schedule.delete', {'id': rule_id})

    @Slot(str, bool)
    def restoreOperation(self, operation_id, interrupted):
        self._request('operation.recover' if interrupted else 'operation.restore',
                      {'id': operation_id, 'confirmed': True, 'source': 'gui'})

    @Slot(str)
    def openBackup(self, operation_id):
        row = next((row for row in self._history if row['id'] == operation_id), {})
        if row.get('backup_status') == 'available':
            QDesktopServices.openUrl(QUrl.fromLocalFile(row['backup']))
