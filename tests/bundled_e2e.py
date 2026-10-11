#!/usr/bin/env python3
import json,os,subprocess,time,base64,tempfile,shutil
from pathlib import Path
BIN=os.environ.get('DELTA_BIN','package-v04/delta-bundled-linux-x86_64')
home=Path(tempfile.mkdtemp(prefix='delta-bundle-test-',dir='/home/user/.cache'))
env={**os.environ,'HOME':str(home)}
for k in ['DELTA_CDP','DELTA_ENGINE','DELTA_ENGINE_PATH','DELTA_PROFILE']:env.pop(k,None)
messages=[{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}},{'jsonrpc':'2.0','method':'notifications/initialized'},{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'test_run','arguments':{'steps':Path('examples/login.delta').read_text()}}},{'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'screenshot','arguments':{}}},{'jsonrpc':'2.0','id':4,'method':'tools/call','params':{'name':'server_status','arguments':{}}}]
results=[]
for run in range(2):
 t=time.monotonic()
 p=subprocess.run([BIN,'mcp'],input=''.join(json.dumps(m)+'\n' for m in messages),capture_output=True,text=True,env=env,timeout=60)
 elapsed=(time.monotonic()-t)*1000
 assert p.returncode==0,p.stderr
 rows=sorted((json.loads(l) for l in p.stdout.splitlines()),key=lambda r:r["id"]);assert len(rows)==4
 assert not rows[1]['result']['isError'],rows[1]
 report=json.loads(rows[1]['result']['content'][0]['text']);assert report['passed']==10 and report['failed']==0
 img=base64.b64decode(rows[2]['result']['content'][0]['data']);assert img.startswith(b'\x89PNG')
 status=json.loads(rows[3]['result']['content'][0]['text']);assert status['engine']=='moli' and 'Landlock' in status['sandbox']
 results.append({'run':run+1,'wall_ms':round(elapsed,2),'engine_startup_ms':status['engine_details']['startup_ms'],'login_steps_passed':10,'image_bytes':len(img),'engine':'moli','sandbox':status['sandbox']})
Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/bundled-results.json').write_text(json.dumps({'executable_bytes':Path(BIN).stat().st_size,'results':results,'note':'First run includes engine cache extraction if cache was absent. Not a cold-cache OS benchmark.'},indent=2));print(json.dumps(results,indent=2))

shutil.rmtree(home)
