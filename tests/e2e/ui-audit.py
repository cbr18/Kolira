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
parser.add_argument('--page', default='main', choices=['main','ThemePage','ThemeComponentsPage','ThemeApplicationsPage','SettingsPage','ApplicationsPage','DependenciesPage','PalettePage','AuditPage','SchedulePage','SectionPage','WallpapersPage'])
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
os.environ.update(QT_QPA_PLATFORM=args.platform,QT_QUICK_BACKEND='software',KTM_THEME_NO_APPLY='1',KTM_OFFLINE='1',LANG=args.locale,LC_ALL=args.locale,LANGUAGE=args.locale)
os.environ.setdefault('KTM_THEME_PROFILE_BIN', str(root/'gui/qt-app/src/core/rust_core/target/release/kolira'))
for key in ('KTM_SCREENSHOT','KTM_OPEN_THEME','KTM_THEME_PAGE'): os.environ.pop(key,None)
data=Path(os.environ['XDG_CONFIG_HOME'])/'kolira';data.mkdir()
(data/'app-preferences.json').write_text(json.dumps({'automation_configured':True}))
for i,name in enumerate(['Paper','dracula']):
    target=('Long-profile-'+str(i)+'-'+'x'*49) if args.long_names else name
    shutil.copytree(root/'themes'/name,data/'themes'/target)
sys.path.insert(0,str(root/'gui/qt-app/src'))
spec=importlib.util.spec_from_file_location('kolira_entry',root/'gui/qt-app/src/__main__.py')
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
        if '/org/kde/kirigami/controls/private/PassiveNotificationsManager.qml:' in message and "Cannot read property 'hovered' of null" in message:
            return
        if any(m in message for m in ['ReferenceError:','TypeError:','Cannot assign','Binding loop detected']):runtime_errors.append(message)
qInstallMessageHandler(qt_message)

def setup(app,window):
    from PySide6.QtCore import QCoreApplication,QObject,QPointF,QTimer,QMetaObject,Qt,QUrl
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
            QTest.qWait(500)
            search=find('themeSearch')
            assert search and find('themeCard-Paper') and find('themeCard-dracula')
            search.setProperty('text','PAPER');QTest.qWait(30)
            assert find('themeCard-dracula') is None
            card=find('themeCard-Paper')
            card.setProperty('profileDescription','ДлинноеОписаниеБезПробелов' * 25)
            QTest.qWait(50)
            card_right=card.mapToScene(QPointF(card.width(),0)).x()
            for label in ('themeCardName','themeCardMetadata','themeCardDescription'):
                item=find(label)
                assert item.mapToScene(QPointF(item.width(),0)).x() <= card_right, label
            card.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Return);QTest.qWait(300)
            details=js('pageStack.layers.currentItem')
            assert details.objectName()=='savedThemeDetails'
            preview=find('themeWallpaperPreview',details)
            assert preview is not None
            for _ in range(50):
                if js('status === 1',preview):
                    break
                QTest.qWait(20)
            assert js('status === 1',preview), preview.property('source')
            assert not js('applyDialog.visible')
            apply_button=find('savedThemeApply');apply_button.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Space);QTest.qWait(300)
            assert js('applyDialog.visible')
            assert not (data/'active-profile').exists()
            js('applyDialog.close();pageStack.layers.pop()');QTest.qWait(300)
            assert find('themeSearch').property('text')=='PAPER'
            apply_button=find('themeCardApply-Paper')
            QMetaObject.invokeMethod(apply_button,'clicked');QTest.qWait(300)
            assert js('applyDialog.visible')
            assert js('pageStack.layers.depth') == 1
            js('applyDialog.close()');QTest.qWait(300)
            assert not (data/'active-profile').exists()
            js('applyDialog.dependenciesRequested({name:"Paper",keep_wallpaper:true,without_sync:true})')
            QTest.qWait(300)
            dependencies=js('pageStack.layers.currentItem')
            assert dependencies.objectName()=='dependenciesPage'
            assert dependencies.property('themeFilter')=='Paper'
            js('reviewRequested(reviewOptions)',dependencies);QTest.qWait(300)
            assert js('applyDialog.visible && applyDialog.report.keep_wallpaper && applyDialog.report.without_sync')
            js('applyDialog.close();pageStack.layers.pop()');QTest.qWait(300)
            legacy = data/'themes/LegacyEditor'
            (legacy/'.config').mkdir(parents=True)
            (legacy/'.config/kdeglobals').write_text('[General]\nAnimationDurationFactor=1.25\n')
            (legacy/'theme-bundle.json').write_text(json.dumps({
                'schema': 'theme-bundle/1', 'components': {'animations': 1.5},
            }))
            js('pageStack.layers.push(Qt.resolvedUrl("pages/ThemePage.qml"), {profileName:"LegacyEditor"})')
            QTest.qWait(200)
            assert bundle.scope == 'profile:LegacyEditor' and not bundle.canEditProfile
            convert = find('convertLegacyTheme')
            assert convert and convert.property('visible')
            assert not (legacy/'theme.json').exists()
            for _ in range(200):
                if convert.property('enabled'):
                    break
                QTest.qWait(20)
            assert convert.property('enabled')
            QMetaObject.invokeMethod(convert, 'clicked')
            for _ in range(200):
                if bundle.canEditProfile and not convert.property('visible') and not bundle.busy:
                    break
                QTest.qWait(20)
            assert bundle.canEditProfile and not convert.property('visible'), (
                bundle.busy, bundle.profileBundle, (legacy/'theme.json').exists(),
                js('feedbackDialog.visible ? feedbackDialog.message : ""'),
            )
            assert json.loads((legacy/'theme.json').read_text())['values']['animation_factor'] == 1.5
            assert not (data/'active-profile').exists()
            js('pageStack.layers.pop()');QTest.qWait(300)
            js('pageStack.layers.push(Qt.resolvedUrl("pages/ThemePage.qml"), {profileName:"Paper"})')
            QTest.qWait(300)
            assert bundle.scope == 'profile:Paper' and bundle.canEditProfile
            QMetaObject.invokeMethod(find('openGroup_desktop'),'clicked');QTest.qWait(300)
            slider=find('componentSlider_animations');assert slider and slider.property('enabled')
            previous=slider.property('value'); increase=previous < slider.property('to')
            expected=previous + (1 if increase else -1) * slider.property('stepSize')
            window.requestActivate();QTest.qWait(30);slider.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Right if increase else Qt.Key_Left);QTest.qWait(50)
            assert bundle.profileDraftSize == 1
            js('requestQuit()');QTest.qWait(100)
            assert js('quitDraftDialog.visible')
            js('quitDraftDialog.close()');QTest.qWait(200)
            QMetaObject.invokeMethod(find('saveThemeDraft'),'clicked')
            for _ in range(100):
                if bundle.profileDraftSize == 0 and not bundle.busy:
                    break
                QTest.qWait(20)
            saved_paper=json.loads((data/'themes/Paper/theme.json').read_text())
            assert abs(saved_paper['values']['animation_factor'] - expected) < 1e-8
            assert not (data/'active-profile').exists()
            js('pageStack.layers.pop()');QTest.qWait(300)
            QMetaObject.invokeMethod(find('openGroup_plasma'),'clicked');QTest.qWait(300)
            font=find('profileFontCombo_ui_font');assert font and font.property('enabled')
            font.setProperty('editText', 'Editor Font Probe')
            QMetaObject.invokeMethod(font, 'accepted');QTest.qWait(50)
            assert bundle.profileDraftSize == 1 and bundle.profileDraft[0]['after'] == 'Editor Font Probe'
            QMetaObject.invokeMethod(find('saveThemeDraft'),'clicked')
            for _ in range(100):
                if bundle.profileDraftSize == 0 and not bundle.busy:
                    break
                QTest.qWait(20)
            saved_paper=json.loads((data/'themes/Paper/theme.json').read_text())
            assert saved_paper['values']['ui_font'].startswith('Editor Font Probe,')
            assert not (data/'active-profile').exists()
            decoration=find('openResources_decoration_theme')
            assert decoration and decoration.property('enabled')
            QMetaObject.invokeMethod(decoration,'clicked');QTest.qWait(50)
            gallery=find('resourceGallery_decoration_theme'); assert gallery
            index=js('model.findIndex(choice => choice.value.disabled === true)',gallery)
            assert index >= 0
            gallery.setProperty('currentIndex',index)
            gallery.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Return);QTest.qWait(50)
            assert bundle.profileDraftSize == 1
            assert isinstance(bundle.profileDraft[0]['after'],str)
            QMetaObject.invokeMethod(find('saveThemeDraft'),'clicked')
            for _ in range(100):
                if bundle.profileDraftSize == 0 and not bundle.busy:
                    break
                QTest.qWait(20)
            saved_paper=json.loads((data/'themes/Paper/theme.json').read_text())
            assert saved_paper['values']['decoration_disabled'] is True
            assert 'decoration_library' not in saved_paper['values']
            assert not (data/'active-profile').exists()
            js('pageStack.layers.pop()');QTest.qWait(300)
            QMetaObject.invokeMethod(find('openGroup_palette'),'clicked');QTest.qWait(300)
            live_globals = data.parent/'kdeglobals'
            live_before = live_globals.read_bytes() if live_globals.exists() else None
            picker=find('componentColor_accent_color');assert picker and picker.property('enabled')
            window.requestActivate();QTest.qWait(30);picker.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Space);QTest.qWait(200)
            palette_page=js('pageStack.layers.currentItem')
            assert js('componentColorDialog.visible',palette_page)
            js('componentColorDialog.selectedColor = "#123abc"; componentColorDialog.accept()',palette_page)
            QTest.qWait(100)
            assert bundle.profileDraftSize == 1 and bundle.profileDraft[0]['after'] == '#123abc'
            wallpaper_accent=find('componentSwitch_accent_from_wallpaper')
            assert wallpaper_accent and wallpaper_accent.property('enabled')
            window.requestActivate();QTest.qWait(50)
            wallpaper_accent.forceActiveFocus();QTest.keyClick(window,Qt.Key_Space);QTest.qWait(50)
            assert bundle.profileDraftSize == 2, bundle.profileDraft
            QMetaObject.invokeMethod(find('saveThemeDraft'),'clicked')
            for _ in range(100):
                if bundle.profileDraftSize == 0 and not bundle.busy:
                    break
                QTest.qWait(20)
            saved_paper=json.loads((data/'themes/Paper/theme.json').read_text())
            assert saved_paper['values']['accent_color'] == '18,58,188'
            assert isinstance(saved_paper['values']['accent_from_wallpaper'], bool)
            assert (live_globals.read_bytes() if live_globals.exists() else None) == live_before
            assert not (data/'active-profile').exists()
            preset=find('profilePalettePreset');assert preset and preset.property('count') > 0
            swatch=find('paletteSwatch_base00');assert swatch and swatch.property('activeFocusOnTab')
            window.requestActivate();QTest.qWait(50);swatch.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Space);QTest.qWait(150)
            assert js('componentColorDialog.visible && paletteSlot === "base00"',palette_page)
            js('componentColorDialog.selectedColor = "#e6dcce"; componentColorDialog.accept()',palette_page)
            QTest.qWait(100)
            assert bundle.profileDraftSize == 1 and bundle.profileDraft[0]['after'] == '#e6dcce'
            bundle.openLive();assert not bundle.profilePalette and bundle.hasProfileDrafts
            bundle.openProfile('Paper');QTest.qWait(100)
            assert bundle.profilePalette['base'][0] == '#e6dcce'
            QMetaObject.invokeMethod(find('saveThemeDraft'),'clicked')
            for _ in range(100):
                if bundle.profileDraftSize == 0 and not bundle.busy:
                    break
                QTest.qWait(20)
            assert bundle.profileDraftSize == 0
            saved_paper=json.loads((data/'themes/Paper/theme.json').read_text())
            assert saved_paper['palette']['base'][0] == '#e6dcce'
            assert saved_paper['values']['window_bg'] == '230,220,206'
            assert saved_paper['values']['accent_color'] == '18,58,188'
            assert (live_globals.read_bytes() if live_globals.exists() else None) == live_before
            assert not (data/'active-profile').exists()
            QMetaObject.invokeMethod(find('excludeProfilePalette'),'clicked');QTest.qWait(80)
            assert bundle.profileDraftSize == 1 and not bundle.profilePalette
            bundle.discardProfileDraft();QTest.qWait(100)
            assert bundle.profilePalette['base'][0] == '#e6dcce'
            js('activated(0)',find('profilePalettePreset'));QTest.qWait(40)
            assert bundle.profileDraftSize == 1
            assert bundle.profilePalette['name'] == saved_paper['palette']['name']
            assert bundle.profilePalette['base'] == bundle.profilePalettePresets[0]['base']
            bundle.discardProfileDraft();QTest.qWait(100)
            js('pageStack.layers.pop()');QTest.qWait(300)
            QMetaObject.invokeMethod(find('openGroup_applications'),'clicked');QTest.qWait(300)
            target_button=find('profileAppTargetToggle');assert target_button and target_button.property('enabled')
            QMetaObject.invokeMethod(target_button,'click');QTest.qWait(100)
            search_app=find('profileAppTargetSearch');assert search_app and search_app.property('visible')
            search_app.setProperty('text','kitty');QTest.qWait(40)
            target=find('profileAppTarget_kitty');assert target and target.property('checked')
            assert find('profileAppTarget_foot') is None
            window.requestActivate();QTest.qWait(50);target.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Space);QTest.qWait(50)
            assert 'kitty' not in bundle.profileSelectedAppTargets
            assert bundle.profileDraftSize == 1
            assert any('Kitty' in row['label'] for row in bundle.profileDraft)
            opacity=find('componentSlider_kitty_opacity');assert opacity and opacity.property('enabled')
            previous=opacity.property('value');increase=previous < opacity.property('to')
            expected=previous + (1 if increase else -1) * opacity.property('stepSize')
            opacity.forceActiveFocus();QTest.keyClick(window,Qt.Key_Right if increase else Qt.Key_Left);QTest.qWait(50)
            assert bundle.profileDraftSize == 2
            save_target_changes=find('saveThemeDraft');assert save_target_changes and save_target_changes.property('enabled')
            save_target_changes.forceActiveFocus();QTest.keyClick(window,Qt.Key_Space)
            for _ in range(100):
                if bundle.profileDraftSize == 0 and not bundle.busy:
                    break
                QTest.qWait(20)
            assert bundle.profileDraftSize == 0
            saved_paper=json.loads((data/'themes/Paper/theme.json').read_text())
            assert 'kitty' not in saved_paper['app_targets']
            assert abs(saved_paper['values']['kitty_opacity'] - expected) < 1e-8
            assert not (data.parent/'kitty').exists()
            assert not (data/'active-profile').exists()
            bundle.setComponent('accent_color','#314159')
            select_editor=find('componentChoice_vscode_theme');assert select_editor and select_editor.property('enabled')
            index=js('model.findIndex(choice => choice.value === "Monokai")',select_editor)
            assert index >= 0
            js('activated(' + str(index) + ')',select_editor);QTest.qWait(50)
            assert bundle.profileDraftSize == 2
            assert set(bundle.profileDraftGroups) == {'palette','applications'}
            reset=find('discardProfileCategory');assert reset and reset.property('visible')
            window.requestActivate();QTest.qWait(50);reset.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Space);QTest.qWait(70)
            assert bundle.profileDraftSize == 1 and bundle.profileDraftGroups == ['palette']
            assert bundle.profileDraft[0]['after'] == '#314159'
            select_editor=find('componentChoice_vscode_theme')
            js('activated(' + str(index) + ')',select_editor);QTest.qWait(50)
            for field,value in (('zed_mode','system'),('zed_light','Ayu Light'),('zed_dark','Ayu Dark')):
                choice=find('componentChoice_' + field);assert choice and choice.property('enabled')
                index=js('model.findIndex(choice => choice.value === "' + value + '")',choice)
                assert index >= 0
                js('activated(' + str(index) + ')',choice);QTest.qWait(30)
            system_label=QCoreApplication.translate('CoreMessages','System')
            assert {system_label,'Ayu Light','Ayu Dark'}.issubset(row['after'] for row in bundle.profileDraft)
            zen_mode=find('componentChoice_zen_mode')
            assert zen_mode and zen_mode.property('enabled')
            assert js('model.map(choice => choice.value).join(",")',zen_mode) == 'light,dark'
            js('activated(1)',zen_mode)
            zen_accent=find('componentColor_zen_accent_color')
            assert zen_accent and zen_accent.property('enabled')
            QMetaObject.invokeMethod(zen_accent,'clicked');QTest.qWait(50)
            apps_page=js('pageStack.layers.currentItem')
            assert js('componentColorDialog.visible',apps_page)
            js('componentColorDialog.selectedColor = "#113355"; componentColorDialog.accept()',apps_page)
            assert any(row['after'] == '#113355' for row in bundle.profileDraft)
            obsidian_mode=find('componentChoice_obsidian_mode')
            assert obsidian_mode and obsidian_mode.property('enabled')
            js('activated(1)',obsidian_mode)
            obsidian_theme=find('componentChoice_obsidian_theme')
            assert obsidian_theme and obsidian_theme.property('enabled')
            assert js('model[0].value === ""',obsidian_theme)
            js('activated(0)',obsidian_theme)
            QMetaObject.invokeMethod(find('saveThemeDraft'),'clicked')
            for _ in range(100):
                if bundle.profileDraftSize == 0 and not bundle.busy:
                    break
                QTest.qWait(20)
            saved_paper=json.loads((data/'themes/Paper/theme.json').read_text())
            assert saved_paper['values']['vscode_theme'] == 'Monokai'
            assert saved_paper['values']['zed_mode'] == 'system'
            assert saved_paper['values']['zed_light'] == 'Ayu Light'
            assert saved_paper['values']['zed_dark'] == 'Ayu Dark'
            assert saved_paper['values']['zen_accent_color'] == '#113355'
            assert saved_paper['values']['zen_window_scheme'] == 0
            assert saved_paper['obsidian_overrides'] == {'mode': {'value': 'dark'}, 'theme': {'value': ''}}
            assert not (data.parent/'Code').exists()
            assert not (data.parent/'zed').exists()
            assert not (Path(os.environ['HOME'])/'.zen').exists()
            assert not (Path(os.environ['HOME'])/'obsidian').exists()
            for route in ('button','keyboard','card'):
                js('pageStack.layers.clear(); openThemePage()');QTest.qWait(300)
                card=find('themeGroup_plasma');assert card is not None
                if route=='button':QMetaObject.invokeMethod(find('openGroup_plasma'),'clicked')
                elif route=='keyboard':
                    window.requestActivate();QTest.qWait(50)
                    card.forceActiveFocus();QTest.keyClick(window,Qt.Key_Return)
                else:QMetaObject.invokeMethod(card,'openGroup')
                QTest.qWait(300);page=js('pageStack.layers.currentItem');assert page.objectName()=='themeComponentsPage', route
                groups=json.loads(json.dumps(bundle._groups));group=next(g for g in groups if g['id']=='plasma')
                group['components'][0]['currentDisplay']='Updated via '+route;bundle._groups=groups;bundle.groupsChanged.emit();QTest.qWait(20)
                actual=page.property('components');actual=actual.toVariant() if hasattr(actual,'toVariant') else actual
                assert actual[0]['currentDisplay']=='Updated via '+route
            js('pageStack.layers.clear();openThemePage()');QTest.qWait(300)
            commands=Path(work.name)/'commands'
            commands.mkdir()
            code=commands/'code'
            code.write_text('#!/bin/sh\nexit 0\n')
            code.chmod(0o755)
            os.environ['PATH']=str(commands)+os.pathsep+os.environ['PATH']
            code_config=data.parent/'Code/User/settings.json'
            code_config.parent.mkdir(parents=True)
            code_config.write_text('{"workbench.colorTheme":"Default Dark Modern","editor.fontSize":21}')
            bundle.refresh()
            QMetaObject.invokeMethod(find('openGroup_applications'),'clicked');QTest.qWait(300)
            QMetaObject.invokeMethod(find('openLiveApplicationThemes'),'clicked');QTest.qWait(300)
            assert js('typeof pageStack.layers.currentItem.handleThemeMessage === "function"')
            choice=find('liveAppTheme_vscode')
            assert choice and choice.property('enabled')
            index=js('model.findIndex(theme => theme.id === "Monokai")',choice)
            assert index >= 0
            js('activated(' + str(index) + ')',choice)
            inspector=engine.rootContext().contextProperty('themeInspector')
            for _ in range(120):
                if inspector.sessionChanges == 1 and not bundle.busy:
                    break
                QTest.qWait(25)
            assert inspector.sessionChanges == 1
            assert inspector.draft[0]['after'] == 'Monokai'
            assert json.loads(code_config.read_text())['workbench.colorTheme'] == 'Default Dark Modern'
            assert find('liveAppTheme_vscode').property('currentText') == 'Monokai'
            QMetaObject.invokeMethod(find('applyLiveAppDraft'),'clicked')
            for _ in range(120):
                if inspector.sessionChanges == 0 and not bundle.busy:
                    break
                QTest.qWait(25)
            assert inspector.sessionChanges == 0
            assert json.loads(code_config.read_text()) == {'workbench.colorTheme':'Monokai','editor.fontSize':21}
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
            accent=find('paletteAccent',palpage)
            assert accent and accent.isVisible() and accent.property('enabled')
            window.requestActivate();QTest.qWait(100);accent.forceActiveFocus()
            assert accent.hasActiveFocus(), window.activeFocusItem()
            QTest.keyClick(window,Qt.Key_Return);QTest.qWait(80)
            assert palpage.property('editingSlot')=='accent', (accent.hasActiveFocus(), window.isActive())
            js('colorDialog.close()',palpage);QTest.qWait(300)
            for slot in ['base'+format(i,'02X') for i in range(16)]:
                swatch=find('paletteSwatch_'+slot,palpage);assert swatch and swatch.property('activeFocusOnTab'), slot
                # Native ColorDialog uses a separate window. Sample one base
                # swatch there and test every key route in the offscreen suite.
                if args.platform != 'offscreen' and slot != 'base0F':
                    continue
                window.requestActivate();QTest.qWait(30)
                swatch.forceActiveFocus();QTest.keyClick(window,Qt.Key_Space);QTest.qWait(30)
                assert palpage.property('editingSlot')==slot, slot
                js('colorDialog.close()',palpage)
                QTest.qWait(250)  # Let the exit transition release popup focus.
            window.requestActivate();QTest.qWait(30);swatch.forceActiveFocus()
            QTest.keyClick(window,Qt.Key_Tab);QTest.qWait(30)
            assert not swatch.hasActiveFocus(), 'Tab must leave the swatch'
            QTest.keyClick(window,Qt.Key_Tab,Qt.ShiftModifier);QTest.qWait(30)
            assert swatch.hasActiveFocus(), 'Shift+Tab must return to the swatch'

            profile=engine.rootContext().contextProperty('profileController')
            profile._preview_data={'name':'test','token':'test','file_count':50,'files':[{'path':'.config/'+str(i),'operation':'modify'} for i in range(50)]}
            profile.previewChanged.emit();js('applyDialog.open()');QTest.qWait(60)
            popup=window.findChild(QObject,'applyPreviewDialog');listing=find('applyFiles',popup);bar=find('applyFilesVerticalBar',popup);assert listing and bar
            assert listing.property('count') == 0
            QMetaObject.invokeMethod(find('applyTechnicalDiff',popup),'click');QTest.qWait(30)
            assert listing.property('count') == 50
            assert listing.mapToScene(QPointF(listing.width(),0)).x()<=bar.mapToScene(QPointF()).x()
            listing.setProperty('contentY',listing.property('contentHeight')-listing.height());QTest.qWait(20);assert listing.property('contentY')>0
            js('applyDialog.close(); actionsDialog.actions = [{ id: "appearance_reload", kind: "manual", '
               'title: "It is recommended to restart these programs.", '
               'reason: "", warning: "", programs: ["Micro", "mpv"] }]; actionsDialog.open()')
            QTest.qWait(100)
            popup=window.findChild(QObject,'applyActionsDialog')
            micro=find('followupProgram-Micro',popup)
            mpv=find('followupProgram-mpv',popup)
            assert micro and mpv and micro.property('text')=='• Micro' and mpv.property('text')=='• mpv'
            assert micro.mapToScene(QPointF()).y() < mpv.mapToScene(QPointF()).y()
            js('actionsDialog.close();saveDialog.open()');QTest.qWait(30)
            assert js('saveDialog.leftPadding')>1 and js('saveDialog.rightPadding')>1
            js('saveDialog.close();diffDialog.open()');QTest.qWait(300)
            js('diffDialog.close()');QTest.qWait(250)
            for dialog in ('importFileDialog','exportFileDialog'):
                js(dialog+'.open()');QTest.qWait(250)
                assert js(dialog+'.visible'), dialog
                js(dialog+'.close()');QTest.qWait(250)
            inspector=engine.rootContext().contextProperty('themeInspector')
            pictures = Path(work.name)
            old_image, new_image = pictures / 'before.png', pictures / 'chosen.png'
            old_image.touch()
            new_image.touch()
            appletsrc = Path(os.environ['XDG_CONFIG_HOME']) / 'plasma-org.kde.plasma.desktop-appletsrc'
            old_layout = ('[Containments][1]\nplugin=org.kde.plasma.folder\nwallpaperplugin=org.kde.image\n\n'
                          f'[Containments][1][Wallpaper][org.kde.image][General]\nImage={old_image.as_uri()}\n')
            appletsrc.write_text(old_layout)
            js('pageStack.layers.clear();openThemePage()');QTest.qWait(300)
            QMetaObject.invokeMethod(find('openGroup_desktop'),'clicked');QTest.qWait(300)
            wallpaper_dialog = window.findChild(QObject, 'liveWallpaperDialog')
            assert wallpaper_dialog and wallpaper_dialog.setProperty('selectedFile', QUrl.fromLocalFile(str(new_image)))
            QMetaObject.invokeMethod(wallpaper_dialog, 'accepted')
            for _ in range(100):
                if bundle.sessionChanges and not bundle.busy:
                    break
                QTest.qWait(20)
            assert bundle.sessionChanges == 1 and inspector.draft[0]['after'] == new_image.name
            assert appletsrc.read_text() == old_layout
            bundle.applyChanges()
            for _ in range(100):
                if not bundle.sessionChanges and not bundle.busy:
                    break
                QTest.qWait(20)
            assert bundle.sessionChanges == 0 and new_image.as_uri() in appletsrc.read_text()
            inspector.setColor('window_bg','#112233')
            for _ in range(50):
                if inspector.sessionChanges > 0:
                    break
                QTest.qWait(20)
            assert inspector.sessionChanges > 0 and inspector.draft
            js('openSaveThemeDialog()');QTest.qWait(100)
            assert js('saveDialog.hasDraft && draftSource.count === 2 && draftSource.currentIndex === 0')
            js('draftSource.currentIndex = 1; nameField.text = "DraftUI"; saveDialog.accept()')
            saved = data/'themes/DraftUI/theme.json'
            for _ in range(100):
                if saved.is_file() and not profile.busy:
                    break
                QTest.qWait(20)
            assert json.loads(saved.read_text())['values']['window_bg'] == '17,34,51'
            kdeglobals=Path(os.environ['XDG_CONFIG_HOME'])/'kdeglobals'
            kdeglobals.write_text('[Colors:Window]\nBackgroundNormal=1,2,3\n')
            for _ in range(100):
                if not inspector.busy:
                    break
                QTest.qWait(20)
            inspector.refreshDraft()
            conflicts=window.findChild(QObject,'liveConflictDialog')
            for _ in range(200):
                if conflicts.property('visible'):
                    break
                QTest.qWait(25)
            assert js('liveConflictDialog.conflicts.length === 1 && liveConflictDialog.conflicts[0].current === "1,2,3"'), (
                js('JSON.stringify(liveConflictDialog.conflicts)'), inspector.busy, inspector.draft)
            QMetaObject.invokeMethod(find('keepLiveDraft',conflicts),'clicked')
            for _ in range(100):
                if not conflicts.property('visible'):
                    break
                QTest.qWait(25)
            assert not conflicts.property('visible') and inspector.sessionChanges > 0
            assert 'BackgroundNormal=1,2,3' in kdeglobals.read_text()
            history=engine.rootContext().contextProperty('historySchedule')
            js('openAuditPage()');QTest.qWait(300)
            audit=js('pageStack.layers.currentItem')
            find('historyTabs').setProperty('currentIndex',1)
            create=find('createRestorePoint')
            for _ in range(100):
                if create.property('enabled'):
                    break
                QTest.qWait(25)
            assert create.property('enabled')
            QMetaObject.invokeMethod(create,'clicked');QTest.qWait(80)
            popup=window.findChild(QObject,'restorePointNameDialog')
            name=find('restorePointName',popup)
            assert name
            name.setProperty('text','Before redesign test')
            QMetaObject.invokeMethod(name,'accepted')
            point=None
            for _ in range(120):
                point=next((row for row in history.points if row.get('name')=='Before redesign test'),None)
                if point and not history.busy:
                    break
                QTest.qWait(25)
            assert point, history.message
            restore=find('restorePoint-'+point['id'])
            assert restore
            QMetaObject.invokeMethod(restore,'clicked');QTest.qWait(100)
            assert js('pointConfirmation.visible && pointConfirmation.restore',audit)
            js('pointConfirmation.close()',audit)
            js('requestQuit()');QTest.qWait(100)
            assert js('quitDraftDialog.visible') and window.isVisible()
            js('quitDraftDialog.close();close()');QTest.qWait(100)
            assert js('quitDraftDialog.visible') and window.isVisible()
            js('quitDraftDialog.close()')
            assert inspector.sessionChanges > 0 and not (data/'active-profile').exists()
            if runtime_errors:raise RuntimeError(runtime_errors)
            recording=False;output.write_text('PASS navigation, feedback, validation, gutters\n');print('PASS UI interactions',flush=True);app.exit(0)
        except Exception:
            recording=False;traceback.print_exc();app.exit(2)
    if args.check:
        QTimer.singleShot(100,check);QTimer.singleShot(50000,lambda:app.exit(3));return
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
sys.argv[:]=['kolira']
raise SystemExit(entry.main())
