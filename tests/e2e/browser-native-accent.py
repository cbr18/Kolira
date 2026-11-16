#!/usr/bin/env python3
"""Check native accent, stock chrome contrast and return for every bundled theme."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time

spec = importlib.util.spec_from_file_location('browser_driver', Path(__file__).with_name('browser-colour-contrast.py'))
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--kolira', required=True)
    parser.add_argument('--browser', required=True)
    parser.add_argument('--zen', action='store_true')
    parser.add_argument('--themes', default=str(Path(__file__).resolve().parents[2] / 'themes'))
    args = parser.parse_args()
    browser = shutil.which(args.browser)
    assert browser, args.browser
    cli = str(Path(args.kolira).resolve())
    packages = sorted(p for p in Path(args.themes).resolve().iterdir() if (p/'theme.json').is_file())
    with tempfile.TemporaryDirectory(prefix='kolira-native-accent-') as folder:
        home = Path(folder)
        env = dict(os.environ, HOME=folder, XDG_CONFIG_HOME=str(home/'.config'),
                   XDG_DATA_HOME=str(home/'.local/share'), XDG_CACHE_HOME=str(home/'.cache'),
                   XDG_STATE_HOME=str(home/'.local/state'), KTM_THEME_NO_APPLY='1', KTM_OFFLINE='1',
                   MOZ_HEADLESS='1', MOZ_ENABLE_WAYLAND='0', MOZ_REMOTE_ALLOW_SYSTEM_ACCESS='1',
                   DBUS_SESSION_BUS_ADDRESS='unix:path=/nonexistent')
        env.pop('CODEX_HOME', None)
        env.pop('CLAUDE_CONFIG_DIR', None)
        root = home / ('.zen' if args.zen else '.mozilla/firefox')
        profile = root / 'test.default'
        (profile/'chrome').mkdir(parents=True)
        (root/'profiles.ini').write_text('[Profile0]\nDefault=1\nPath=test.default\n')
        with socket.socket() as probe:
            probe.bind(('127.0.0.1', 0))
            port = probe.getsockname()[1]
        personal_css = '#nav-bar { display:none; width:1px; }\n'
        (profile/'chrome/userChrome.css').write_text(personal_css)
        (profile/'user.js').write_text(
            f'user_pref("marionette.port", {port});\n'
            'user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", false);\n'
            'user_pref("browser.shell.checkDefaultBrowser", false);\n'
            'user_pref("datareporting.policy.dataSubmissionEnabled", false);\n'
            'user_pref("browser.startup.homepage_override.mstone", "ignore");\n')
        themes = home/'.config/kolira/themes'
        for package in packages:
            shutil.copytree(package, themes/package.name)
        for name in ['baseline', 'neutral']:
            (themes/name).mkdir(parents=True)
            (themes/name/'theme.json').write_text(json.dumps({'schema':'kolira/theme@2','name':name}))
        baseline = None
        for name in ['baseline', *[p.name for p in packages], 'neutral']:
            value = {'name':name}
            preview = driver.rpc(cli, env, 'profile.preview', value)
            driver.rpc(cli, env, 'profile.apply', dict(value,preview_token=preview['token'],skip_dependencies=True))
            log = open(home/'browser.log', 'w')
            process = subprocess.Popen([browser,'--headless','--no-remote','--new-instance','--profile',str(profile),
                                        '--marionette','--remote-allow-system-access','--offline','about:blank'],
                                       env=env,stdout=log,stderr=log)
            client = None
            try:
                deadline = time.monotonic()+45
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise RuntimeError((home/'browser.log').read_text()[-2000:])
                    try:
                        client = driver.Marionette(port)
                        break
                    except (OSError, RuntimeError):
                        time.sleep(.2)
                assert client, 'Marionette timeout'
                client.send('WebDriver:NewSession', {'capabilities':{}})
                client.send('Marionette:SetContext', {'value':'chrome'})
                script = '''const swatch=document.createElement('div');
                  swatch.style.color='AccentColor';swatch.style.backgroundColor='AccentColorText';
                  document.documentElement.append(swatch);
                  const s=getComputedStyle(swatch),bar=document.getElementById('nav-bar'),b=getComputedStyle(bar);
                  const result={dark:matchMedia('(prefers-color-scheme: dark)').matches,accent:s.color,onAccent:s.backgroundColor,background:b.backgroundColor,foreground:b.color,
                    stylesheet:Services.prefs.getBoolPref('toolkit.legacyUserProfileCustomizations.stylesheets',false),
                    controls:['back-button','forward-button','reload-button','PanelUI-menu-button'].flatMap(id=>{
                      const button=document.getElementById(id),node=button&&button.querySelector('.toolbarbutton-icon');
                      if (!node || !node.getBoundingClientRect().width) return [];
                      const c=getComputedStyle(node);return [{id,color:c.color,fill:c.fill}];})};
                  result.ancestors=[];
                  for(let node=bar;node;node=node.parentElement) { result.ancestors.push(getComputedStyle(node).backgroundColor); }
                  swatch.style.backgroundColor='Canvas';result.canvas=getComputedStyle(swatch).backgroundColor;
                  swatch.remove();return result;'''
                result = client.send('WebDriver:ExecuteScript', {'script':script,'args':[],'newSandbox':True})
                result = result.get('value', result)
                assert not result['stylesheet'], (name, 'disabled personal skin was enabled')
                assert (profile/'chrome/userChrome.css').read_text() == personal_css
                assert not (profile/'chrome/kolira-colors.css').exists()
                if name in ['baseline','neutral']:
                    if name=='baseline': baseline = (result['accent'],result['onAccent'])
                    else: assert (result['accent'],result['onAccent'])==baseline, ('accent did not return',result,baseline)
                else:
                    palette=json.loads((themes/name/'theme.json').read_text())['palette']
                    assert result['dark'] == (palette['variant']=='dark'), (name,'native day/night',result)
                    assert driver.rgb(result['accent'])==driver.rgb(palette['accent']), (name,'native accent mismatch',result,palette['accent'])
                    a=driver.luminance(driver.rgb(result['accent']));t=driver.luminance(driver.rgb(result['onAccent']))
                    assert (max(a,t)+.05)/(min(a,t)+.05)>=4.5, (name,'on-accent contrast',result)
                background=driver.rgb(result['canvas'])
                for color in reversed(result['ancestors']):
                    background=driver.rgb(color,background)
                for control in result['controls']:
                    for key in ['color','fill']:
                        if control[key].startswith(('rgb','color(srgb ')):
                            fg=driver.luminance(driver.rgb(control[key],background));bg=driver.luminance(background)
                            assert (max(fg,bg)+.05)/(min(fg,bg)+.05)>=3, (name,'stock icon contrast',control,result)
                print(json.dumps({'browser':args.browser,'theme':name,'result':result}),flush=True)
            finally:
                if client: client.close()
                process.terminate()
                try: process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill();process.wait()
                log.close()


if __name__ == '__main__':
    main()
