"""Observe instruction discovery with installed Codex/Claude against a local sink.

Standalone optional integration check; needs local CLI clients and loopback HTTP.
No real model or external API is used: every request receives a deliberate 400.
Only request bodies are inspected; credentials are removed from the isolated env.
The client exit code 1 is expected. Run with Python from any cwd.
"""
import http.server
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import time

root=Path(__file__).resolve().parents[2]
cli=root/'gui/qt-app/src/core/rust_core/target/release/theme-profile'
work=Path(tempfile.mkdtemp(prefix='ktm-discovery-'))
env=os.environ.copy()
for key in list(env):
    if key.startswith(('OPENAI_','CODEX_','ANTHROPIC_','CLAUDE_')):
        env.pop(key,None)
env.update(HOME=str(work/'home'),XDG_CONFIG_HOME=str(work/'config'),XDG_DATA_HOME=str(work/'data'),XDG_CACHE_HOME=str(work/'cache'),CODEX_HOME=str(work/'codex'),KTM_THEME_NO_APPLY='1',KTM_OFFLINE='1')
for name in ['home','config','data','cache','codex']:(work/name).mkdir()
subprocess.run([str(cli),'core','agent.install'],input='{}',text=True,env=env,capture_output=True,check=True)
themes=work/'config/kde-theme-profile/themes'
(themes/'nested').mkdir()
captures=[]
class Sink(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def do_POST(self):
        data=self.rfile.read(int(self.headers.get('Content-Length','0')))
        captures.append(data.decode())
        self.send_response(400);self.send_header('Content-Type','application/json');self.end_headers()
        self.wfile.write(b'{"error":{"message":"Instruction capture complete; no model is used","type":"invalid_request_error"}}')
server=http.server.HTTPServer(('127.0.0.1',0),Sink)
threading.Thread(target=server.serve_forever,daemon=True).start()
base=f'http://127.0.0.1:{server.server_port}/v1'
results=[]
for cwd,markers in [(themes,False),(themes/'nested',False),(themes/'nested',True)]:
    captures.clear()
    command=['codex','exec','--skip-git-repo-check','--sandbox','read-only','--ephemeral','--json','-C',str(cwd),
             '-c','model_provider="ktm_capture"','-c','model="ktm-capture"',
             '-c','model_providers.ktm_capture.name="Instruction capture"',
             '-c',f'model_providers.ktm_capture.base_url="{base}"',
             '-c','model_providers.ktm_capture.wire_api="responses"',
             '-c','model_providers.ktm_capture.requires_openai_auth=false',
             '-c','model_providers.ktm_capture.request_max_retries=0',
             '-c','model_providers.ktm_capture.stream_max_retries=0',
             'Return READY. Do not use tools.']
    if markers:
        command[-1:-1]=['-c','project_root_markers=[".git","AGENTS.md"]']
    try:
        proc=subprocess.run(command,env=env,text=True,capture_output=True,timeout=45)
        (work/(cwd.name+'-stderr.txt')).write_text(proc.stderr)
        content='\n'.join(captures)
        found='KDE Theme Manager — theme workspace instructions (v1)' in content
        results.append({'cwd':'themes' if cwd==themes else 'themes/nested','root_markers_configured':markers,'captured_requests':len(captures),'instructions_found':found,'client_exit':proc.returncode})
    except subprocess.TimeoutExpired:
        results.append({'cwd':str(cwd),'instructions_found':False,'timeout':True})
claude_results=[]
env.update(ANTHROPIC_BASE_URL=f'http://127.0.0.1:{server.server_port}', ANTHROPIC_API_KEY='test-only',
           CLAUDE_CONFIG_DIR=str(work/'claude'), CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC='1', DISABLE_AUTOUPDATER='1',
           CLAUDE_CODE_SKIP_PROJECT_ONBOARDING='1',DISABLE_TELEMETRY='1',DISABLE_ERROR_REPORTING='1')
for cwd,additional in [(themes,False),(themes/'nested',False),(themes/'nested',True)]:
    captures.clear()
    try:
        command=['claude','-p','--model','claude-sonnet-4-5','--tools','','--max-turns','1','Return READY. Do not use tools.']
        if additional:
            command[-1:-1]=['--add-dir',str(themes),'--']
            env['CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD']='1'
        proc=subprocess.run(command,env=env,cwd=cwd,text=True,capture_output=True,timeout=30)
        content='\n'.join(captures)
        (work/('claude-'+cwd.name+'-requests.txt')).write_text(content)
        claude_results.append({'cwd':'themes' if cwd==themes else 'themes/nested','additional_root_configured':additional,'captured_requests':len(captures),'instructions_found':'KDE Theme Manager — theme workspace instructions (v1)' in content,'client_exit':proc.returncode})
        (work/('claude-'+cwd.name+'-stderr.txt')).write_text(proc.stderr+'\n'+proc.stdout)
    except subprocess.TimeoutExpired:
        claude_results.append({'cwd':str(cwd),'instructions_found':False,'timeout':True})
print(json.dumps({'claude_results':claude_results,'codex':subprocess.run(['codex','--version'],env=env,text=True,capture_output=True).stdout.strip(),'results':results,'artifacts':str(work)},ensure_ascii=False))
server.shutdown()

# Root loading and the documented Codex root-marker launch must work. Claude
# nested/add-dir cases are observations, not a promise of automatic discovery.
assert all(r.get("instructions_found") for r in results if r.get("cwd") == "themes" or r.get("root_markers_configured")), results
assert claude_results[0].get("instructions_found"), claude_results
