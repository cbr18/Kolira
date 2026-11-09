"""UI audit regressions for background data and localized templates."""
import sys
from pathlib import Path
from threading import Event
from types import SimpleNamespace
import pytest
from PySide6.QtCore import QCoreApplication, QTimer, QTranslator
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))


def test_summary_refresh_coalesces_and_rejects_stale_results(qtbot, monkeypatch):
    from controllers import ProfileController
    from core.protocol import CoreFactory
    from core import cli_core
    from controllers.theme_inspector import _active_jobs
    gate = Event(); requests = []
    def request(method, **arguments):
        assert method == "profile.summaries"
        names = arguments["names"]; requests.append(names)
        if names == ["old"]: gate.wait(5)
        return {name: {"scheme": name} for name in names}
    monkeypatch.setattr(cli_core, "core_request", request)
    monkeypatch.setattr(CoreFactory, "profile_core", lambda: SimpleNamespace())
    ctrl = ProfileController(); ctrl._profiles = [SimpleNamespace(name="old")]
    ctrl._refresh_summaries(); qtbot.waitUntil(lambda: requests == [["old"]])
    ticks = []; QTimer.singleShot(0, lambda: ticks.append(True)); qtbot.waitUntil(lambda: bool(ticks))
    ctrl._profiles = [SimpleNamespace(name="intermediate")]; ctrl._refresh_summaries()
    ctrl._profiles = [SimpleNamespace(name="new")]; ctrl._refresh_summaries()
    delivered = []; ctrl.summariesChanged.connect(lambda: delivered.append(dict(ctrl.summaries)))
    gate.set(); qtbot.waitUntil(lambda: ctrl.summaries == {"new": {"scheme": "new"}})
    assert requests == [["old"], ["new"]] and all("old" not in v for v in delivered)
    ctrl._profiles = []; ctrl._refresh_summaries(); assert ctrl.summaries == {}
    qtbot.waitUntil(lambda: not _active_jobs)


@pytest.fixture
def russian(qapp):
    translator = QTranslator()
    assert translator.load(str(Path(__file__).resolve().parents[1] / "i18n/kolira_ru.qm"))
    qapp.installTranslator(translator)
    yield
    qapp.removeTranslator(translator)


def test_messages_preserve_paths_and_placeholders(russian):
    from core.core_messages import render_message, localize, state_label, source_label
    assert render_message("Settings file not found: %1", ["/tmp/%2 настройки.json"]) == "Файл настроек не найден: /tmp/%2 настройки.json"
    assert localize("Application not detected") == "Приложение не найдено"
    assert localize("Removed Мои значки") == "Удалено: Мои значки"
    assert state_label("rolled_back") == "Выполнен откат" and source_label("schedule") == "Расписание"
    assert state_label("succeeded") == "Завершено"
    assert source_label("imagecolors:/tmp/мой файл.png") == "Цвета Plasma: /tmp/мой файл.png"
    assert localize("third-party diagnostic: %1 <raw>") == "third-party diagnostic: %1 <raw>"
    assert state_label("future_state") == "future_state"
    assert render_message("%1 restarted", ["%2"]) == "%2 перезапущен"


def test_russian_catalogs_are_complete(russian):
    import xml.etree.ElementTree as ET
    for path in (Path(__file__).resolve().parents[1] / "i18n").glob("*.ts"):
        for ctx in ET.parse(path).getroot().findall("context"):
            for message in ctx.findall("message"):
                tr = message.find("translation"); assert tr is not None
                if tr.get("type") in ("vanished", "obsolete"): continue
                assert tr.get("type") != "unfinished", (ctx.findtext("name"),message.findtext("source"))
                assert "".join(tr.itertext()).strip()
                if message.get("numerus") == "yes": assert len(tr.findall("numerusform")) == 3
    assert QCoreApplication.translate("ApplyPreviewDialog", "%n files will change", None, 1) == "Изменится 1 файл"
    assert QCoreApplication.translate("ApplyPreviewDialog", "%n files will change", None, 5) == "Изменятся 5 файлов"


def test_status_templates_translate_before_substitution(russian, monkeypatch):
    from core.theme_bundle import ThemeBundleCore
    from core import cli_core
    monkeypatch.setattr(cli_core, "core_request", lambda *a, **kw: [{"id":"zed", "statusText":"Settings file not found: /tmp/путь", "statusTemplate":"Settings file not found: %1", "statusArgs":["/tmp/путь"]}])
    core = object.__new__(ThemeBundleCore); core._theme = SimpleNamespace(_app_changes={})
    assert core.applications()[0]["statusText"] == "Файл настроек не найден: /tmp/путь"


def test_actual_navigation_errors_and_gutters(tmp_path):
    import os
    import subprocess
    root = Path(__file__).resolve().parents[3]
    result = subprocess.run([sys.executable, str(root/'tests/e2e/ui-audit.py'), str(tmp_path/'interaction.txt'), '--check', '--width', '640', '--height', '480'], env=dict(os.environ, QT_QPA_PLATFORM='offscreen'), text=True, capture_output=True, timeout=25)
    assert result.returncode == 0, result.stdout + result.stderr
