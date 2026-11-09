"""Regression scenarios from the November audit; no live settings or network."""
import sys
from pathlib import Path
from threading import Event
from types import SimpleNamespace

import pytest
from PySide6.QtCore import QObject, QUrl, QEvent, QCoreApplication

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))


def test_completed_jobs_release_helpers_and_deliver_on_gui_thread(qtbot):
    from PySide6.QtCore import QThread, QThreadPool
    from controllers.theme_inspector import _run, _active_jobs
    owner = QObject()
    results = []
    baseline = set(_active_jobs)
    for i in range(30):
        _run(owner, lambda: (True, "ok"),
             lambda ok, message: results.append((ok, QThread.currentThread())))
    qtbot.waitUntil(lambda: len(results) == 30)
    QThreadPool.globalInstance().waitForDone(5000)
    qtbot.waitUntil(lambda: not (_active_jobs - baseline))
    assert all(ok and thread == owner.thread() for ok, thread in results)


def test_destroyed_owner_does_not_receive_async_callback(qtbot):
    from controllers.theme_inspector import _run, _active_jobs
    owner = QObject()
    gate = Event()
    results = []
    baseline = len(_active_jobs)
    _run(owner, lambda: (gate.wait(5), "done"), lambda *args: results.append(args))
    owner.deleteLater()
    QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
    gate.set()
    qtbot.waitUntil(lambda: len(_active_jobs) == baseline)
    assert not results


def test_palette_search_ignores_old_success_and_failure(qtbot):
    from controllers.palette import PaletteController
    from core.protocol import Result
    gate = Event()
    completed = []
    def gallery(query, online):
        if query == "old":
            gate.wait(5)
            completed.append(query)
            return Result.err("obsolete failure")
        completed.append(query)
        return Result.ok(data={"items": [{"id": query}]})
    core = SimpleNamespace(show=lambda: {}, current_wallpaper=lambda: "", gallery=gallery)
    ctrl = PaletteController(core=core)
    errors = []
    ctrl.toast.connect(lambda *args: errors.append(args))
    ctrl.searchGallery("old", False)
    ctrl.searchGallery("new", False)
    qtbot.waitUntil(lambda: ctrl.galleryItems == [{"id": "new"}])
    gate.set()
    qtbot.waitUntil(lambda: "old" in completed)
    from controllers.theme_inspector import _active_jobs
    qtbot.waitUntil(lambda: not _active_jobs)
    assert ctrl.galleryItems == [{"id": "new"}]
    assert not errors


@pytest.mark.parametrize("name", ["pictures", "обои с пробелами"])
def test_wallpaper_directory_url_is_decoded_locally(name, qtbot, monkeypatch):
    from controllers import SettingsController
    import json
    from PySide6.QtQml import QQmlEngine, QQmlComponent
    from core import CoreFactory
    from core.protocol import AppSettings
    monkeypatch.setattr(CoreFactory, "settings_core", lambda: SimpleNamespace(get_defaults=AppSettings))
    target = SettingsController()
    engine = QQmlEngine()
    engine.rootContext().setContextProperty("settingsController", target)
    component = QQmlComponent(engine)
    url = QUrl.fromLocalFile("/tmp/" + name)
    code = 'import QtQuick; QtObject { property url selectedFolder: ' + json.dumps(url.toString()) + '; Component.onCompleted: settingsController.setWallpaperDirectoryUrl(selectedFolder) }'
    component.setData(code.encode(), QUrl())
    obj = component.create()
    assert obj is not None, [e.toString() for e in component.errors()]
    assert target.wallpaperDirectory == "/tmp/" + name
    target.setWallpaperDirectoryUrl(QUrl("https://example.com/remote"))
    assert target.wallpaperDirectory == "/tmp/" + name
    obj.deleteLater()


@pytest.fixture
def qml_page(qtbot):
    from PySide6.QtCore import Property, Signal, Slot
    from PySide6.QtQml import QQmlEngine, QQmlComponent
    from PySide6.QtQuick import QQuickWindow, QQuickItem

    class Context(QObject):
        changed = Signal()
        def __init__(self):
            super().__init__()
            self.group_data = []
            self.log_data = []
            self.row_data = []
            self.palette_data = {}
        toast = Signal(str, bool)
        currentTheme = Property(str, lambda self: "system", constant=True)
        palette = Property("QVariantMap", lambda self: self.palette_data, notify=changed)
        hasPalette = Property(bool, lambda self: bool(self.palette_data), notify=changed)
        wallpaperUrl = Property(str, lambda self: "", constant=True)
        defaultTargets = Property(list, lambda self: [], constant=True)
        galleryItems = Property(list, lambda self: [], constant=True)
        groups = Property(list, lambda self: self.group_data, notify=changed)
        log = Property(list, lambda self: self.log_data, notify=changed)
        rows = Property(list, lambda self: self.row_data, notify=changed)
        profiles = Property(list, lambda self: [], constant=True)
        isGlass = Property(bool, lambda self: False, constant=True)
        readOnly = Property(bool, lambda self: False, constant=True)
        busy = Property(bool, lambda self: False, constant=True)
        canCancel = Property(bool, lambda self: False, constant=True)
        message = Property(str, lambda self: "", constant=True)
        @Slot()
        def refresh(self): pass

    ctx = Context()
    engine = QQmlEngine()
    root = Path(__file__).resolve().parents[1] / "qml"
    engine.addImportPath(str(root))
    engine.addImportPath("/usr/lib/qt6/qml")
    for name in ("themeController", "themeBundle", "dependencyController", "profileController", "paletteController"):
        engine.rootContext().setContextProperty(name, ctx)
    window = QQuickWindow()
    window.resize(820, 680)
    pages = []
    def load(name, properties=None):
        component = QQmlComponent(engine, QUrl.fromLocalFile(str(root / "pages" / name)))
        qtbot.waitUntil(lambda: component.status() != QQmlComponent.Loading)
        if component.isError():
            errors = "\n".join(e.toString() for e in component.errors())
            if 'is not installed' in errors:
                pytest.skip("System Kirigami QML modules unavailable: " + errors)
            pytest.fail(errors)
        page = component.createWithInitialProperties(properties or {})
        assert page is not None, [e.toString() for e in component.errors()]
        page.setParentItem(window.contentItem())
        page.setWidth(820)
        page.setHeight(680)
        pages.append((component, page))
        window.show()
        qtbot.wait(20)
        return page
    yield ctx, load
    for _, page in pages:
        page.setParentItem(None)
        page.deleteLater()
    window.close()
    QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
    engine.deleteLater()


def test_component_page_updates_open_model_on_groups_changed(qml_page, qtbot):
    ctx, load = qml_page
    def groups(value):
        return [{"id": "kde", "components": [{"id": "test", "label": "Probe", "description": "",
            "kind": "choice", "choices": [], "currentValue": value, "currentDisplay": value,
            "icon": "", "status": "ok", "statusText": "", "staged": False, "supported": True,
            "externalModule": "", "legacySection": ""}]}]
    ctx.group_data = groups("before")
    page = load("ThemeComponentsPage.qml", {"groupId": "kde"})
    def current():
        value = page.property("components")
        return value.toVariant() if hasattr(value, "toVariant") else value
    assert current()[0]["currentDisplay"] == "before"
    ctx.group_data = groups("after")
    ctx.changed.emit()
    qtbot.waitUntil(lambda: current()[0]["currentDisplay"] == "after")
    ctx.group_data = []
    ctx.changed.emit()
    qtbot.waitUntil(lambda: current() == [])


def test_long_dependency_log_keeps_table_visible_and_scrolls(qml_page, qtbot):
    from PySide6.QtQuick import QQuickItem
    ctx, load = qml_page
    page = load("DependenciesPage.qml")
    table = page.findChild(QQuickItem, "dependenciesTable")
    log = page.findChild(QQuickItem, "dependenciesOperationLog")
    assert table is not None and log is not None
    before = table.height()
    ctx.log_data = [{"ok": True, "message": "Result " + str(i)} for i in range(100)]
    ctx.changed.emit()
    qtbot.waitUntil(lambda: log.width() > 0 and 0 < log.height() <= 120)
    assert before > 300 and table.height() > 250
    assert log.height() <= 120
    assert log.property("contentHeight") > log.height()


def test_actual_main_shows_wallpaper_error_without_spinner(qml_page, tmp_path):
    """Load the actual application in a separate isolated Qt process."""
    import os
    import subprocess
    _, load = qml_page
    load("ThemeComponentsPage.qml")  # skip where system Kirigami is unavailable
    root = Path(__file__).resolve().parents[1]
    config = tmp_path / "config"
    config.mkdir()
    (config / "plasma-org.kde.plasma.desktop-appletsrc").write_text(
        "[Containments][1]\nplugin=org.kde.plasma.folder\n"
        "[Containments][1][Wallpaper][org.kde.image][General]\n"
        "Image=file:///nonexistent/ktm-audit-wallpaper.png\n")
    script = r'''
import importlib.util, sys, os
from PySide6.QtCore import QTimer, QObject
from PySide6.QtQml import QQmlEngine, QQmlExpression
spec = importlib.util.spec_from_file_location("kolira_app", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
module.SINGLE_INSTANCE_SOCKET = "ktm-audit-" + str(os.getpid())
sys.argv = [sys.argv[1]]
def setup(app, window):
    def check():
        image = window.findChild(QObject, "wallpaperPreview")
        busy = window.findChild(QObject, "wallpaperPreviewBusy")
        error = window.findChild(QObject, "wallpaperPreviewError")
        ok = image is not None and busy is not None and error is not None
        if ok:
            expression = QQmlExpression(QQmlEngine.contextForObject(image), image, "status === 3")
            is_error, undefined = expression.evaluate()
            ok = not expression.hasError() and not undefined and is_error and not busy.property("running") and error.property("visible")
        print("wallpaper error state:", ok, flush=True)
        app.exit(0 if ok else 2)
    QTimer.singleShot(1500, check)
    QTimer.singleShot(5000, lambda: app.exit(3))
module._maybe_setup_screenshot = setup
raise SystemExit(module.main())
'''
    env = dict(os.environ, HOME=str(tmp_path), XDG_CONFIG_HOME=str(config),
        XDG_DATA_HOME=str(tmp_path / "data"), XDG_CACHE_HOME=str(tmp_path / "cache"),
        KTM_THEME_NO_APPLY="1", KTM_OFFLINE="1", QT_QPA_PLATFORM="offscreen")
    env.pop("KTM_SCREENSHOT", None)
    result = subprocess.run([sys.executable, "-c", script, str(root / "src/__main__.py")],
        env=env, capture_output=True, text=True, timeout=20)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "wallpaper error state: True" in result.stdout


def test_dependency_scrollbar_stays_in_visible_viewport(qml_page, qtbot):
    from PySide6.QtCore import QPointF
    from PySide6.QtQuick import QQuickItem
    ctx, load = qml_page
    ctx.row_data = [{"id": str(i), "installed": True, "application": False,
        "themes": ["long-profile-name-" * 3], "dependency": {
            "title": "Resource " + str(i), "name": str(i), "kind": "icons", "source": "kde-store"},
        "can_install": False, "can_remove": False} for i in range(40)]
    page = load("DependenciesPage.qml")
    page.setWidth(640); page.setHeight(480); qtbot.wait(30)
    table = page.findChild(QQuickItem, "dependenciesTable")
    bar = page.findChild(QQuickItem, "dependenciesVerticalBar")
    assert table.property("contentWidth") > table.width()
    before = bar.mapToScene(QPointF()).x()
    table.setProperty("contentX", table.property("contentWidth") - table.width()); qtbot.wait(20)
    assert bar.mapToScene(QPointF()).x() == before
    assert before >= table.mapToScene(QPointF(table.width(), 0)).x()
    assert before + bar.width() <= page.width()
    assert bar.property("size") < 1
    bar.setProperty("position", .5); qtbot.wait(20)
    assert bar.property("position") > 0
