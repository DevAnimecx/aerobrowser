#!/usr/bin/env python3
"""Actual HTTP app reads Web Storage in its first inline startup script."""
import os,json,subprocess,threading,uuid,copy,base64,time
from pathlib import Path
from http.server import ThreadingHTTPServer,BaseHTTPRequestHandler
BIN=os.environ.get('DELTA_BIN','target/release/delta');MOLI=os.environ['MOLI_PATH']
checks=[];evidence={}
def check(n,v):assert v,n;checks.append(n);print('PASS',n,flush=True)
slow_entered=threading.Event()
class App(BaseHTTPRequestHandler):
 def log_message(self,*a):pass
 def do_GET(self):
  if self.path=='/slow':slow_entered.set();time.sleep(1)
  if self.path=='/redirect':
   self.send_response(302);self.send_header('Location',f'http://localhost:{self.server.server_port}/landing');self.end_headers();return
  b=b'''<!doctype html><script>window.boot={local:localStorage.getItem('saved'),session:sessionStorage.getItem('tab'),cookie:document.cookie};</script><title>Preboot storage service</title><h1>Preboot storage service</h1><p id="boot"></p><script>document.getElementById('boot').textContent=JSON.stringify(boot);</script>'''
  self.send_response(200);self.send_header('Content-Type','text/html');self.send_header('Content-Length',str(len(b)));self.end_headers();
  try:self.wfile.write(b)
  except (BrokenPipeError,ConnectionResetError):pass
server=ThreadingHTTPServer(('127.0.0.1',0),App);threading.Thread(target=server.serve_forever,daemon=True).start();url='http://127.0.0.1:'+str(server.server_port)
p=subprocess.Popen([BIN,'--engine','moli','--engine-path',MOLI,'mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
n=0
def rpc(m,args,notification=False):
 global n
 n+=1;rid=n;v={'jsonrpc':'2.0','method':m,'params':args}
 if not notification:v['id']=n
 p.stdin.write(json.dumps(v)+'\n');p.stdin.flush()
 if notification:return
 r=json.loads(p.stdout.readline());assert r['id']==rid,r;return r
def tool(name,args={},error=False):
 r=rpc('tools/call',{'name':name,'arguments':args});assert 'error' not in r,r
 if error:return r['result']
 assert not r['result'].get('isError'),r
 return r['result']['structuredContent']['result']
try:
 check('release identifies 0.5',rpc('initialize',{'protocolVersion':'2025-11-25'})['result']['serverInfo']['version']==os.environ.get('DELTA_EXPECT_VERSION','0.0.1'));rpc('notifications/initialized',{},True)
 tool('navigate',{'url':url+'/one'});nonce=uuid.uuid4().hex
 tool('evaluate',{'script':f'localStorage.setItem("saved",{json.dumps(nonce)});sessionStorage.setItem("tab","first");true'})
 tool('set_cookie',{'name':'saved_cookie','value':nonce,'url':url})
 tool('new_tab',{'url':url+'/two'});tool('evaluate',{'script':'sessionStorage.setItem("tab","second");true'})
 state=tool('save_state')['state'];check('two real tabs saved',len(state['tabs'])==2)
 before=tool('list_tabs');result=tool('restore_state',{'state':state});evidence['restore']=result
 check('preboot staged restore succeeded',result['storage_applied_after_load'] is False)
 tabs=tool('list_tabs');evidence['tabs']=tabs
 # tabs tool returns a target list; use tab_id keys from actual response.
 rows=tabs if isinstance(tabs,list) else tabs.get('tabs',[])
 boots=[]
 for t in rows:
  tool('switch_tab',{'tab_id':t.get('tab_id',t.get('id'))});boots.append(tool('evaluate',{'script':'boot'}))
 evidence['boot_observations']=boots
 check('first inline script sees restored localStorage',len(boots)==2 and all(b['local']==nonce for b in boots))
 check('per-tab sessionStorage restored before boot',{b['session'] for b in boots}=={'first','second'})
 check('cookies available during page startup',all(nonce in b['cookie'] for b in boots))
 tool('evaluate',{'script':'localStorage.setItem("saved","changed-after-restore");true'});tool('reload')
 check('bootstrap removed after initial load',tool('evaluate',{'script':'boot.local'})=='changed-after-restore')
 stable_tabs=tool('list_tabs');stable_url=tool('get_url')
 bad=copy.deepcopy(state);bad['tabs'][0]['origin']='https://wrong.example'
 check('origin mismatch rejected before mutation',tool('restore_state',{'state':bad},True)['isError'])
 check('validation error preserves existing tabs',tool('list_tabs')==stable_tabs)
 bad=copy.deepcopy(state);bad['tabs'][0]['localStorage']['saved']='conflict'
 check('conflicting origin storage refused',tool('restore_state',{'state':bad},True)['isError'])
 bad=copy.deepcopy(state);bad['tabs'][0]['url']=url+'/redirect'
 failure=tool('restore_state',{'state':bad},True);evidence['redirect_failure']=failure
 check('cross-origin redirect fails staged restore',failure['isError'])
 check('failed staged restore preserves original tabs',tool('list_tabs')==stable_tabs)
 check('failed restore preserves original storage',tool('evaluate',{'script':'localStorage.getItem("saved")'})=='changed-after-restore')
 check('failed restore preserves active URL',tool('get_url')==stable_url)
 tool('evaluate',{'script':'console.log("X".repeat(300000));true'})
 status=tool('server_status');evidence['oversized_telemetry']=status['telemetry']
 check('oversized real console event omitted',status['telemetry']['console']['oversized_events']>=1)
 tool('evaluate',{'script':'for(let i=0;i<80;i++)console.log(i+":"+"X".repeat(100000));true'})
 status=tool('server_status');evidence['flood_telemetry']=status['telemetry']
 check('retention byte budget enforced',status['telemetry']['console']['retained_serialized_bytes']<=4194304)
 check('real console pressure records dropped events',status['telemetry']['console']['dropped_events']>1)
 # Cancel while a staged restore is awaiting a real slow HTTP response.
 oldpid=tool('server_status')['engine_details']['pid'];slow=copy.deepcopy(state);slow['tabs'][0]['url']=url+'/slow';cancelled={}
 request_id=n+1
 def restoring():cancelled['reply']=rpc('tools/call',{'name':'restore_state','arguments':{'state':slow}})
 thread=threading.Thread(target=restoring);thread.start();assert slow_entered.wait(5),'Slow request not observed'
 rpc('notifications/cancelled',{'requestId':request_id},True);thread.join(8)
 check('staged restore can be cancelled in flight',not thread.is_alive() and cancelled['reply'].get('error',{}).get('code')==-32800)
 check('cancelled staged worker reaped',not Path('/proc',str(oldpid)).exists())
 check('session restarts after cancelled restore',tool('server_status')['engine_details']['pid']!=oldpid)
 tool('navigate',{'url':'https://example.com/'})
 check('public HTTPS remains functional',tool('get_title')=='Example Domain')
 image=tool('screenshot');Path('artifacts/v04/public-example.png').write_bytes(base64.b64decode(image['data']))
 pid=tool('server_status')['engine_details']['pid'];tool('close');check('close reaps worker before returning',not Path('/proc',str(pid)).exists())
finally:
 p.stdin.close();p.wait(timeout=10);server.shutdown()
Path('artifacts/v04/state-e2e.json').write_text(json.dumps({'passed':len(checks),'checks':checks,'evidence':evidence},indent=2));print('TOTAL',len(checks))
