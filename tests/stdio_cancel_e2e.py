import os,json,subprocess,time,select
from pathlib import Path
binary=os.environ.get('DELTA_BIN','target/release/delta');engine=os.environ['MOLI_PATH']
p=subprocess.Popen([binary,'--engine','moli','--engine-path',engine,'mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
def send(v):p.stdin.write(json.dumps(v)+'\n');p.stdin.flush()
def read():
 assert select.select([p.stdout],[],[],8)[0],'stdio reply timed out'
 return json.loads(p.stdout.readline())
send({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}});assert read()['result']['serverInfo']['version']==os.environ.get('DELTA_EXPECT_VERSION','0.0.1')
send({'jsonrpc':'2.0','method':'notifications/initialized'})
send({'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'wait_for','arguments':{'selector':'#absent','timeout_ms':30000}}})
time.sleep(.4)
send({'jsonrpc':'2.0','id':3,'method':'ping'})
send({'jsonrpc':'2.0','method':'notifications/cancelled','params':{'requestId':2}})
rows=[read(),read()];byid={v['id']:v for v in rows};assert byid[2]['error']['code']==-32800;assert byid[3]['result']=={}
p.stdin.close();p.wait(timeout=8);assert p.returncode==0,p.stderr.read()
Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/stdio-cancellation.json').write_text(json.dumps({'passed':True,'responses':rows,'exit_code':p.returncode},indent=2));print('stdio cancellation, queued ping, clean EOF passed')
