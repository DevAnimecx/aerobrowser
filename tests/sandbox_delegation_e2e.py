#!/usr/bin/env python3
"""Real managed-browser checks. Local app uses SQLite, not intercepted/mock responses.
Requires the documented v0.3 validation server and delegated cgroup controls.
"""
import os,json,urllib.request,urllib.error,time,threading,uuid,tempfile,sqlite3,html,base64
from http.server import ThreadingHTTPServer,BaseHTTPRequestHandler
from pathlib import Path
BASE=os.environ.get('DELTA_SERVER','http://127.0.0.1:8931')
checks=[]; evidence={}
def check(name,value):
 assert value,name
 checks.append(name);print('PASS',name,flush=True)
def request(owner,path,value=None,method=None,sid=None):
 headers={'Authorization':'Bearer delta-v03-test-only-token-'+owner}
 if value is not None:headers['Content-Type']='application/json'
 if sid:headers['mcp-session-id']=sid
 req=urllib.request.Request(BASE+path,data=json.dumps(value).encode() if value is not None else None,headers=headers,method=method)
 try:r=urllib.request.urlopen(req,timeout=20)
 except urllib.error.HTTPError as e:r=e
 body=r.read();return r.status,dict(r.headers),json.loads(body) if body else None
class Client:
 def __init__(self,owner):
  self.owner=owner;self.n=1
  c,h,r=request(owner,'/mcp',{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}});assert c==200,(c,r)
  self.sid=h['mcp-session-id'];self.rpc('notifications/initialized',{},False)
 def rpc(self,method,params,with_id=True):
  self.n+=1;v={'jsonrpc':'2.0','method':method,'params':params}
  if with_id:v['id']=self.n
  return request(self.owner,'/mcp',v,sid=self.sid)
 def tool(self,name,args={},error=False):
  c,h,r=self.rpc('tools/call',{'name':name,'arguments':args})
  if error:return r
  assert c==200 and not r.get('error') and not r['result'].get('isError'),r
  return r['result']['structuredContent']['result']
 def close(self):request(self.owner,'/mcp',method='DELETE',sid=self.sid)
with tempfile.TemporaryDirectory() as td:
 db=Path(td)/'tasks.sqlite';conn=sqlite3.connect(db);conn.execute('create table tasks(title text not null)');conn.commit();conn.close()
 class App(BaseHTTPRequestHandler):
  def log_message(self,*args):pass
  def do_GET(self):
   if self.path=='/redirect':self.send_response(302);self.send_header('Location','/');self.end_headers();return
   conn=sqlite3.connect(db);rows=conn.execute('select title from tasks').fetchall();conn.close()
   page='''<!doctype html><title>Persistent task service</title><style>body{font:20px sans-serif;margin:40px}input,button{font:inherit}#source,#target{display:inline-block;width:150px;height:70px;background:#acf;margin:20px}</style><h1>Persistent task service</h1><form method="post" action="/tasks"><label>Task <input name="title" id="title" required></label><button>Add task</button></form><ul>'''+''.join('<li>'+html.escape(r[0])+'</li>' for r in rows)+'''</ul><div id="source">Pointer source</div><div id="target">Pointer target</div><script>window.pointerEvidence=[];for(const name of ['mousedown','mousemove','mouseup'])document.addEventListener(name,e=>pointerEvidence.push({type:e.type,trusted:e.isTrusted,buttons:e.buttons,x:e.clientX,y:e.clientY}));</script>'''
   b=page.encode();self.send_response(200);self.send_header('Content-Type','text/html');self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
  def do_POST(self):
   from urllib.parse import parse_qs
   values=parse_qs(self.rfile.read(int(self.headers['Content-Length'])).decode());title=values['title'][0]
   conn=sqlite3.connect(db);conn.execute('insert into tasks values(?)',(title,));conn.commit();conn.close()
   self.send_response(303);self.send_header('Location','/');self.end_headers()
 server=ThreadingHTTPServer(('127.0.0.1',0),App);threading.Thread(target=server.serve_forever,daemon=True).start();url='http://127.0.0.1:'+str(server.server_port)
 a,b=Client('alice'),Client('bob')
 try:
  check('unauthorized token denied',request('invalid','/whoami')[0]==401)
  check('non-admin cannot shut down server',request('alice','/admin/shutdown',{})[0]==403)
  check('principal identified',request('bob','/whoami')[2]['principal']=='bob')
  v={'jsonrpc':'2.0','id':99,'method':'ping'}
  check('cross-principal RPC denied',request('bob','/mcp',v,sid=a.sid)[0]==404)
  check('cross-principal deletion denied',request('bob','/mcp',method='DELETE',sid=a.sid)[0]==404)
  status=a.tool('server_status');evidence['resources']=status['resources']
  check('real cgroup memory enforcement',status['resources']['memory_limit_bytes']==256*1048576)
  check('real kernel accounting',status['resources']['memory_current_bytes']>0 and status['resources']['pids_current']>0)
  check('mocks disabled by default',status['network_mocks_enabled'] is False)
  check('mock request refused',a.tool('intercept_request',{'pattern':'*','action':'mock','mock_response':{'body':'fake'}},True)['result']['isError'])
  a.tool('navigate',{'url':url+'/redirect','wait_until':'networkidle'})
  title='real-write-'+uuid.uuid4().hex;a.tool('fill',{'selector':'#title','value':title});a.tool('click',{'selector':'button'});a.tool('wait_for',{'selector':'li'})
  check('real form persisted to SQLite',sqlite3.connect(db).execute('select title from tasks').fetchone()[0]==title)
  a.tool('reload');check('persisted record survives browser reload',title in a.tool('get_text',{'selector':'body'}))
  image=a.tool('screenshot');data=base64.b64decode(image['data']);check('real rendered PNG',data.startswith(b'\x89PNG'));Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/sqlite-app.png').write_bytes(data)
  pdf=a.tool('save_pdf');data=base64.b64decode(pdf['data']);check('real browser PDF',data.startswith(b'%PDF'));Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/sqlite-app.pdf').write_bytes(data)
  a.tool('drag',{'from_selector':'#source','to_selector':'#target'});events=a.tool('evaluate',{'script':'pointerEvidence'});evidence['pointer_events']=events
  check('native drag generated pointer events',any(e['type']=='mousedown' for e in events) and any(e['type']=='mouseup' for e in events))
  har=a.tool('save_har')['har'];Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/network.har').write_text(json.dumps(har,indent=2));entries=har['log']['entries']
  check('HAR contains completed observed requests',len(entries)>0)
  check('redirect chain preserved',any(e['response']['status']==302 for e in entries))
  check('no estimated HAR timing flags',all('_timingEstimate' not in e and e['_source']=='CDP Network events' for e in entries))
  check('unknown body bytes not fabricated',all(e['response']['bodySize']==-1 for e in entries))
  check('HAR has no mocked response',all(not e['_interception'] for e in entries))
  b.tool('navigate',{'url':url});bpid=b.tool('server_status')['engine_details']['pid']
  oldpid=a.tool('server_status')['engine_details']['pid'];result={};request_id=700
  def long_request():result['value']=request('alice','/mcp',{'jsonrpc':'2.0','id':request_id,'method':'tools/call','params':{'name':'wait_for','arguments':{'selector':'#never-exists','timeout_ms':30000}}},sid=a.sid)
  thread=threading.Thread(target=long_request);thread.start();time.sleep(.25);start=time.monotonic()
  a.rpc('notifications/cancelled',{'requestId':request_id},False);thread.join(8)
  check('in-flight cancellation returns promptly',not thread.is_alive() and time.monotonic()-start<3)
  check('cancelled request has explicit error',result['value'][2]['error']['code']==-32800)
  newpid=a.tool('server_status')['engine_details']['pid'];check('cancelled context replaced',newpid!=oldpid)
  check('peer worker survives cancellation',b.tool('server_status')['engine_details']['pid']==bpid)
  start=time.monotonic();r=a.tool('wait_for',{'selector':'#never-exists','timeout_ms':30000},True)
  check('server deadline enforced',r.get('error',{}).get('code')==-32002 and time.monotonic()-start<7)
  a.tool('server_status');group=a.tool('server_status')['resources']['path']
  oom=a.tool('evaluate',{'script':'globalThis.large=new Uint8Array(1024*1024*512);large.fill(1);large.length'},True)
  kernel=dict(line.split() for line in Path(group,'memory.events').read_text().splitlines());evidence['oom_kernel_events']=kernel;evidence['oom_rpc']=oom
  check('actual kernel OOM kill recorded',int(kernel['oom_kill'])>=1)
  check('peer worker survives OOM',b.tool('get_title')=='Persistent task service')
  a.tool('close');a.tool('navigate',{'url':'https://example.com/','wait_until':'networkidle'})
  live={'url':a.tool('get_url'),'title':a.tool('get_title'),'text':a.tool('get_text',{'selector':'body'}),'performance':a.tool('get_performance'),'memory':a.tool('get_memory_usage')};evidence['public_https']=live
  check('live public HTTPS page fetched',live['title']=='Example Domain' and live['url'].startswith('https://example.com'))
  live_har=a.tool('save_har')['har'];Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/public-https.har').write_text(json.dumps(live_har,indent=2))
  check('live HTTPS response captured',any(e['response']['status']==200 and e['request']['url'].startswith('https://example.com') for e in live_har['log']['entries']))
 finally:a.close();b.close();server.shutdown()
Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/real-e2e.json').write_text(json.dumps({'passed':len(checks),'checks':checks,'evidence':evidence},indent=2));print('TOTAL',len(checks))
