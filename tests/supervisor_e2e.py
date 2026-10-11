#!/usr/bin/env python3
import pathlib,tempfile,subprocess,json,os,sys,socket,time
BIN=os.environ.get('DELTA_BIN','target/debug/delta')
def is_open(port):
 try:
  with socket.create_connection(('127.0.0.1',port),timeout=.2):return True
 except OSError:return False
with tempfile.TemporaryDirectory() as td:
 p=pathlib.Path(td)/'servers.json'
 p.write_text(json.dumps({'fixture':{'argv':[sys.executable,'-m','http.server','4175','--bind','127.0.0.1'],'cwd':str(pathlib.Path('examples').resolve()),'port':4175}}))
 assert not is_open(4175),'Port is occupied; test refuses to stop any existing server'
 scenarios=[('success','serve fixture 4175\nopen http://127.0.0.1:4175/login.html\nassert title "Delta Test App"\nstop 4175',0),('failure-cleanup','serve fixture 4175\nassert title "Wrong"',1),('no-arbitrary-command','serve "python -c danger" 4175',1)]
 results=[]
 for name,steps,expected in scenarios:
  requests=[{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}},{'jsonrpc':'2.0','method':'notifications/initialized'},{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'test_run','arguments':{'steps':steps}}}]
  env={**os.environ,'DELTA_SERVE_CONFIG':str(p),'DELTA_ENGINE':'external'}
  r=subprocess.run([BIN,'mcp'],input=''.join(json.dumps(x)+'\n' for x in requests),env=env,text=True,capture_output=True,timeout=30)
  assert r.returncode==0,r.stderr
  report=json.loads(json.loads(r.stdout.splitlines()[-1])['result']['content'][0]['text']);assert report['failed']==expected,report
  assert not is_open(4175), 'Owned server leaked'
  results.append({'scenario':name,'expected_failed_steps':expected,'report':report,'process_cleanup':True})
 pathlib.Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/supervisor-results.json').write_text(json.dumps(results,indent=2));print(json.dumps(results,indent=2))
