"""Extract QML/Python strings and compile the merged Russian catalogue.

Run from any directory: python3 gui/qt-app/tools/update_translations.py.
LUPDATE, PYLUPDATE6 and LRELEASE override Qt tool executable paths.
"""
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys


def main() -> None:
    app = Path(__file__).resolve().parents[1]
    qml_ts = str(app / "i18n/qml_ru.ts")
    py_ts = str(app / "i18n/py_ru.ts")
    qm = str(app / "i18n/kolira_ru.qm")
    commands = [
        [os.environ.get("LUPDATE", "lupdate"), "qml", "-ts", qml_ts],
        [sys.executable, "tools/gen_i18n_strings.py"],
        [os.environ.get("PYLUPDATE6", "pylupdate6"), "src", "--ts", py_ts],
        [sys.executable, "tools/gen_i18n_strings.py", "--fill"],
        [os.environ.get("LRELEASE", "lrelease"), qml_ts, py_ts, "-qm", qm],
    ]
    for command in commands:
        subprocess.run(command, cwd=app, check=True)


if __name__ == "__main__":
    main()
