#!/usr/bin/env python3
"""Apply a theme the way the app's button does, in a real Wayland session."""
import os
import sys

sys.path.insert(0, os.path.expanduser("~/.local/share/kolira/src"))
from PySide6.QtCore import QTimer  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

app = QApplication(sys.argv)
from controllers import ProfileController  # noqa: E402
from theme_controller import ThemeController  # noqa: E402

ThemeController.instance().watch_desktop_look()
controller = ProfileController()
outcome = {}


def finished(success, message):
    outcome["success"] = success
    outcome["message"] = message
    QTimer.singleShot(1500, app.quit)


controller.operationFinished.connect(finished)
QTimer.singleShot(0, lambda: controller.applyProfile(sys.argv[1]))
QTimer.singleShot(150000, app.quit)
app.exec()
print(f"gui apply {sys.argv[1]}: {outcome}")
sys.exit(0 if outcome.get("success") else 1)
