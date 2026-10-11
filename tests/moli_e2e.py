#!/usr/bin/env python3
import os,json,subprocess,base64,re
from pathlib import Path
BIN=os.environ.get('DELTA_BIN','target/debug/delta')
MOLI=os.environ.get('MOLI_PATH',str(Path.home()/'.delta/engines/moli-v1.1.12/moli'))
class Client:
 def __init__(self,profile='scrape'):
  self.p=subprocess.Popen([BIN,'--engine','moli','--engine-path',MOLI,'--profile',profile,'mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  self.n=0
  self.rpc('initialize',{'protocolVersion':'2025-11-25','clientInfo':{'name':'test','version':'1'},'capabilities':{}})
  self.p.stdin.write(json.dumps({'jsonrpc':'2.0','method':'notifications/initialized'})+'\n');self.p.stdin.flush()
 def rpc(self,m,p):
  self.n+=1;self.p.stdin.write(json.dumps({'jsonrpc':'2.0','id':self.n,'method':m,'params':p})+'\n');self.p.stdin.flush()
  return json.loads(self.p.stdout.readline())['result']
 def tool(self,n,a={},allow_error=False):
  r=self.rpc('tools/call',{'name':n,'arguments':a})
  if allow_error:return r
  assert not r.get('isError'),r
  v=r['content'][0]
  if v['type']=='image':return v
  try:return json.loads(v['text'])
  except ValueError:return v['text']
 def close(self):
  self.p.stdin.close();self.p.wait(timeout=20)
checks=[]
def check(n,v):assert v,n;checks.append(n)
a,b=Client(),Client()
try:
 status=a.tool('server_status');check('managed Moli under kernel sandbox',status['engine']=='moli' and 'Landlock' in status['sandbox'])
 a.tool('navigate',{'url':'http://127.0.0.1:4173/login.html'})
 check('Moli title',a.tool('get_title')=='Delta Test App')
 tree=a.tool('snapshot',{'interactive_only':True});check('Moli AX snapshot','Email' in tree)
 ref=re.search(r'textbox "Email" \[ref=(.*?)\]',tree).group(1)
 a.tool('fill',{'ref':ref,'value':'moli@example.com'})
 a.tool('fill',{'selector':'#password','value':'dummy'})
 a.tool('click',{'selector':'button'})
 a.tool('wait_for',{'selector':'#result'})
 check('Moli login', 'moli@example.com' in a.tool('get_text',{'selector':'#result'}))
 image=a.tool('screenshot');check('Moli rendered PNG',base64.b64decode(image['data']).startswith(b'\x89PNG'))
 Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/moli-login.png').write_bytes(base64.b64decode(image['data']))
 a.tool('set_cookie',{'name':'session-a','value':'secret','url':'http://127.0.0.1:4173'})
 b.tool('navigate',{'url':'http://127.0.0.1:4173/login.html'})
 check('Moli workers separated',b.tool('server_status')['engine_details']['worker_directory']!=status['engine_details']['worker_directory'])
 check('Moli cookies separated',all(c['name']!='session-a' for c in b.tool('get_cookies')['cookies']))
 report=a.tool('test_run',{'steps':Path('examples/login.delta').read_text()});check('Moli complete login recipe',report['failed']==0)
 # Capability probe reports real engine errors rather than faking outputs.
 capabilities={}
 for name,args in [('save_pdf',{}),('get_memory_usage',{}),('get_computed_style',{'selector':'button','properties':['color']}),('intercept_request',{'pattern':'*/moli-mock','action':'mock','mock_response':{'body':'moli-mock','status':200}})]:
  r=a.tool(name,args,allow_error=True);capabilities[name]={'supported':not r.get('isError',False),'error':r['content'][0].get('text') if r.get('isError') else None}
 if capabilities['intercept_request']['supported']:
  check('Moli network mock',a.tool('evaluate',{'script':'fetch("/moli-mock").then(r=>r.text())'})=='moli-mock')
 a.tool('close');check('one worker shutdown leaves other usable',b.tool('get_title')=='Delta Test App')
 pid=a.tool('server_status')['engine_details']['pid']
 import signal
 os.kill(pid,signal.SIGKILL)
 check('worker crash becomes tool error',a.tool('get_title',allow_error=True)['isError'])
 check('peer worker survives crash',b.tool('get_title')=='Delta Test App')
finally:a.close();b.close()
c=Client('scrape')
try:
 check('profile tightens before launch',c.tool('server_profile_set',{'name':'offline'})['applies_on_next_launch'])
 check('offline worker starts',c.tool('server_status')['engine_details']['network']=='none')
 r=c.tool('navigate',{'url':'http://127.0.0.1:4173/login.html'},allow_error=True)
 check('offline Moli navigation blocked',r.get('isError') or c.tool('get_title')!='Delta Test App')
finally:c.close()
summary={'passed':len(checks),'checks':checks,'engine_version':'v1.1.12','sandbox':'Landlock ABI2 + seccomp truncate protection','capabilities':capabilities,'status':status}
Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/moli-results.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2))
