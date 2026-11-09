#!/usr/bin/env python3
"""Render/check source UI with disposable HOME/XDG and synthetic view data.

Use the app's Python. No appearance mutations or network. --platform xcb/wayland
opens a temporary real window; all other inputs remain isolated.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile
import traceback
import faulthandler
faulthandler.enable()

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('output', type=Path)
parser.add_argument('--page', default='main', choices=['main','ThemePage','ThemeComponentsPage','ThemeApplicationsPage','SettingsPage','ApplicationsPage','DependenciesPage','PalettePage','AuditPage','SchedulePage','SectionPage'])
parser.add_argument('--dialog', default='')
parser.add_argument('--theme', default='light', choices=['light','dark','paper','glass'])
parser.add_argument('--locale', default='ru_RU.UTF-8')
parser.add_argument('--width', type=int, default=820)
parser.add_argument('--height', type=int, default=680)
parser.add_argument('--large-font', action='store_true')
parser.add_argument('--long-names', action='store_true')
parser.add_argument('--active-bars', action='store_true')
parser.add_argument('--platform', default='offscreen')
parser.add_argument('--check', action='store_true', help='exercise real navigation, feedback and dialog gutters')
args = parser.parse_args()
if args.dialog and not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', args.dialog): parser.error('invalid QML dialog identifier')
root = Path(__file__).resolve().parents[2]
output = args.output.resolve(); output.parent.mkdir(parents=True,exist_ok=True)
work = tempfile.TemporaryDirectory(prefix='ktm-ui-render-')
runtime = os.environ.get('XDG_RUNTIME_DIR')
for key, name in [('HOME','home'),('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache'),('XDG_RUNTIME_DIR','runtime')]:
    path=Path(work.name)/name; path.mkdir(mode=0o700); os.environ[key]=str(path)
if args.platform!='offscreen' and runtime: os.environ['XDG_RUNTIME_DIR']=runtime
os.environ.update(QT_QPA_PLATFORM=args.platform,QT_QUICK_BACKEND='software',KTM_THEME_NO_APPLY='1',KTM_OFFLINE='1',LANG=args.locale,LC_ALL=args.locale,LANGUAGE=args.locale,KTM_THEME_PROFILE_BIN=str(root/'gui/qt-app/src/core/rust_core/target/release/theme-profile'))
for key in ('KTM_SCREENSHOT','KTM_OPEN_THEME','KTM_THEME_PAGE'): os.environ.pop(key,None)
data=Path(os.environ['XDG_CONFIG_HOME'])/'kde-theme-profile';data.mkdir()
(data/'app-preferences.json').write_text(json.dumps({'automation_configured':True}))
for i,name in enumerate(['Paper','dracula']):
    target=('Long-profile-'+str(i)+'-'+'x'*49) if args.long_names else name
    shutil.copytree(root/'themes'/name,data/'themes'/target)
sys.path.insert(0,str(root/'gui/qt-app/src'))
spec=importlib.util.spec_from_file_location('ktm_entry',root/'gui/qt-app/src/__main__.py')
entry=importlib.util.module_from_spec(spec);spec.loader.exec_module(entry)
entry._acquire_single_instance=lambda *unused:entry.SingleInstance.ACQUIRED
original_palette=entry._setup_palette

def palette(app):
    original_palette(app)
    if args.large_font:
        font=app.font();font.setPointSizeF(font.pointSizeF()*1.4);app.setFont(font)

from PySide6.QtCore import qInstallMessageHandler
runtime_errors=[]
recording=True

def qt_message(kind,context,message):
    if recording:
        print(message,file=sys.stderr)
        if any(m in message for m in ['ReferenceError:','TypeError:','Cannot assign','Binding loop detected']):runtime_errors.append(message)
qInstallMessageHandler(qt_message)

def setup(app,window):
    from PySide6.QtCore import QObject,QPointF,QTimer,QMetaObject,Qt
    from PySide6.QtQml import QQmlEngine,QQmlExpression
    from PySide6.QtTest import QTest
    context=QQmlEngine.contextForObject(window);engine=context.engine()
    window.resize(args.width,args.height)
    theme=engine.rootContext().contextProperty('themeController');theme.currentTheme=args.theme;theme._apply_theme()
    bundle=engine.rootContext().contextProperty('themeBundle')
    def js(text,scope=window):
        expr=QQmlExpression(QQmlEngine.contextForObject(scope),scope,text);value,_=expr.evaluate()
        if expr.hasError():raise RuntimeError(expr.error().toString())
        return value
    def find(name, scope=window):
        # Search the visual tree in QML: wrapping every anonymous delegate in
        # Python can disturb ownership/lifetimes in some PySide versions.
        return js('(function search(item) { if (item.objectName === ' + json.dumps(name) + ') return item; '
                  'for (let child of item.children) { const found = search(child); if (found) return found; } '
                  'return null; })(contentItem)', scope)
    def check():
        global recording
        try:
            for route in ('button','keyboard','card'):
                js('pageStack.layers.clear(); openThemePage()');QTest.qWait(300)
                card=find('themeGroup_plasma');assert card is not None
                if route=='button':QMetaObject.invokeMethod(find('openGroup_plasma'),'clicked')
                elif route=='keyboard':card.forceActiveFocus();QTest.keyClick(window,Qt.Key_Return)
                else:QMetaObject.invokeMethod(card,'openGroup')
                QTest.qWait(300);page=js('pageStack.layers.currentItem');assert page.objectName()=='themeComponentsPage'
                groups=json.loads(json.dumps(bundle._groups));group=next(g for g in groups if g['id']=='plasma')
                group['components'][0]['currentDisplay']='Updated via '+route;bundle._groups=groups;bundle.groupsChanged.emit();QTest.qWait(20)
                actual=page.property('components');actual=actual.toVariant() if hasattr(actual,'toVariant') else actual
                assert actual[0]['currentDisplay']=='Updated via '+route
            js('pageStack.layers.clear();openThemeApplicationsPage()');QTest.qWait(300)
            bundle.toast.emit('Child page failure',False);QTest.qWait(30)
            feedback=window.findChild(QObject,'themeFeedbackDialog');assert feedback.property('visible') and feedback.property('message')=='Child page failure'
            js('feedbackDialog.close();pageStack.layers.clear();pageStack.layers.push(Qt.resolvedUrl("pages/ApplicationsPage.qml"))');QTest.qWait(300)
            editor=window.findChild(QObject,'appEditor');QMetaObject.invokeMethod(editor,'openNew');QTest.qWait(30)
            QMetaObject.invokeMethod(editor,'save');QTest.qWait(20);error=find('appEditorError',editor)
            assert editor.property('visible') and error and error.isVisible() and error.property('text'), (editor.property('visible'), editor.property('validationError'), repr(error), error.property('text') if error else '')
            QMetaObject.invokeMethod(editor,'close')
            pal=engine.rootContext().contextProperty('paletteController')
            pal._palette={'name':'Keyboard test','variant':'dark','base':['#112233']*16,'accent':'#ffaa00'}
            pal.paletteChanged.emit()
            js('pageStack.layers.clear();openPalettePage()');QTest.qWait(300)
            palpage=js('pageStack.layers.currentItem')
            for slot in ['base'+format(i,'02X') for i in range(16)]:
                swatch=find('paletteSwatch_'+slot);assert swatch and swatch.property('activeFocusOnTab'), slot
                window.requestActivate();QTest.qWait(30)
                swatch.forceActiveFocus();QTest.keyClick(window,Qt.Key_Space);QTest.qWait(30)
                assert palpage.property('editingSlot')==slot, slot
                js('colorDialog.close()',palpage)
                QTest.qWait(250)  # Let the exit transition release popup focus.
            accent=find('paletteAccent');assert accent
            window.requestActivate();QTest.qWait(30);accent.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Return);QTest.qWait(30)
            assert palpage.property('editingSlot')=='accent'
            js('colorDialog.close()',palpage);QTest.qWait(250)
            window.requestActivate();QTest.qWait(30);swatch.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Tab);QTest.qWait(30)
            assert not swatch.hasActiveFocus(), 'Tab must leave the swatch'
            QTest.keyClick(window,Qt.Key_Tab,Qt.ShiftModifier);QTest.qWait(30)
            assert swatch.hasActiveFocus(), 'Shift+Tab must return to the swatch'

            profile=engine.rootContext().contextProperty('profileController')
            profile._preview_data={'name':'test','token':'test','file_count':50,'files':[{'path':'.config/'+str(i),'operation':'modify'} for i in range(50)]}
            profile.previewChanged.emit();js('applyDialog.open()');QTest.qWait(60)
            popup=window.findChild(QObject,'applyPreviewDialog');listing=find('applyFiles',popup);bar=find('applyFilesVerticalBar',popup);assert listing and bar
            assert listing.mapToScene(QPointF(listing.width(),0)).x()<=bar.mapToScene(QPointF()).x()
            listing.setProperty('contentY',listing.property('contentHeight')-listing.height());QTest.qWait(20);assert listing.property('contentY')>0
            js('applyDialog.close();saveDialog.open()');QTest.qWait(30)
            assert js('saveDialog.leftPadding')>1 and js('saveDialog.rightPadding')>1
            js('saveDialog.close();diffDialog.open()');QTest.qWait(300)
            js('diffDialog.close()');QTest.qWait(250)
            for dialog in ('importFileDialog','exportFileDialog'):
                js(dialog+'.open()');QTest.qWait(250)
                assert js(dialog+'.visible'), dialog
                js(dialog+'.close()');QTest.qWait(250)
            if runtime_errors:raise RuntimeError(runtime_errors)
            recording=False;output.write_text('PASS navigation, feedback, validation, gutters\n');print('PASS UI interactions',flush=True);app.exit(0)
        except Exception:
            recording=False;traceback.print_exc();app.exit(2)
    if args.check:
        QTimer.singleShot(100,check);QTimer.singleShot(25000,lambda:app.exit(3));return
    if args.page=='ThemeComponentsPage':js('openThemeComponentsPageDefault()')
    elif args.page!='main':js('pageStack.layers.push(Qt.resolvedUrl("pages/'+args.page+'.qml"))')
    def prepare():
        try:
            from core.core_messages import source_label,state_label
            pal=engine.rootContext().contextProperty('paletteController')
            pal._palette={'name':'Example base16','variant':'dark','base':['#20241f','#2b3028','#4b5347','#889080','#b0b8a5','#e5edda','#edf5e5','#f5faef','#e66b6b','#e8a65c','#dcca65','#a2c66d','#72c4b6','#79add4','#bd91c7','#bf9476'],'accent':'#a2c66d','source':'wallpaper','kdeScheme':'ExampleBase16'};pal.paletteChanged.emit()
            bundle._core._theme._app_changes['audit']='example';bundle.groupsChanged.emit()
            history=engine.rootContext().contextProperty('historySchedule')
            history._history=[{'id':'a'*24,'profile':'Paper','previous_profile':'dracula','started':'2026-11-08T10:00:00Z','source':'gui','sourceText':source_label('gui'),'state':'succeeded','stateText':state_label('succeeded'),'backup':'/tmp/example-backup','backup_status':'available','can_restore':True,'error':'','report':{'files':20}}]
            history._schedule={'enabled':False,'running':False,'rules':[{'id':'evening','theme':'dracula','time':'20:00','days':[1,2,3,4,5],'enabled':True,'keep_wallpaper':True,'next':'2026-11-09T10:00:00Z','last':None}]};history.changed.emit()
            if args.dialog=='applyDialog':
                profile=engine.rootContext().contextProperty('profileController');profile._preview_data={'name':'Paper','token':'render-only','file_count':50,'files':[{'path':'.config/example/app-'+str(i)+'.json','operation':'modify','reasons':[],'diff':'- old\n+ new'} for i in range(50)]};profile.previewChanged.emit()
            if args.dialog:
                page=window if args.page=='main' else js('pageStack.layers.currentItem')
                js(args.dialog+('.openNew()' if args.dialog=='appEditor' else '.open()'),page)
            if args.active_bars:
                js('(function() { function visit(item) { if (item instanceof ScrollBar) item.active = true; '
                   'for (let child of item.children) visit(child); } visit(contentItem); visit(overlay); })()')
        except Exception:traceback.print_exc();app.exit(2)
    def grab():
        global recording
        recording=False
        if runtime_errors:print('FAIL runtime QML errors:',runtime_errors,file=sys.stderr);app.exit(2);return
        if not window.grabWindow().save(str(output)):app.exit(2);return
        labels = js('(function() { const labels = []; function visit(item) { if (!item.visible) return; '
                    'if (typeof item.text === "string" && item.text) { const p = item.mapToItem(contentItem, 0, 0); '
                    'labels.push({text:item.text,x:p.x,y:p.y,width:item.width,height:item.height}); } '
                    'for (let child of item.children) visit(child); } visit(contentItem); visit(overlay); return labels; })()')
        if hasattr(labels, 'toVariant'): labels = labels.toVariant()
        output.with_suffix('.json').write_text(json.dumps(labels,ensure_ascii=False,indent=2))
        print('PASS UI render:',output,flush=True);app.quit()
    QTimer.singleShot(700,prepare);QTimer.singleShot(1800,grab);QTimer.singleShot(12000,lambda:app.exit(3))
entry._setup_palette=palette;entry._maybe_setup_screenshot=setup
sys.argv[:]=['kde-theme-manager']
raise SystemExit(entry.main())
