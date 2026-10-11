#!/usr/bin/env python3
"""Requires running CDP browser, delta HTTP server and examples fixture server.
Uses only Python standard library. DELTA_TOKEN must match server.
"""
import base64, json, os, subprocess, urllib.request, urllib.error
from pathlib import Path
BASE=os.environ.get('DELTA_SERVER','http://127.0.0.1:8931')
TOKEN=os.environ['DELTA_TOKEN']
BIN=os.environ.get('DELTA_BIN','target/debug/delta')
PAGE='http://127.0.0.1:4173/login.html'
checks=[]
def check(name, condition):
    assert condition, name
    checks.append(name)
def request(path, value=None, headers=None, method=None):
    h={'Content-Type':'application/json','Accept':'application/json, text/event-stream','Authorization':'Bearer '+TOKEN}
    h.update(headers or {})
    req=urllib.request.Request(BASE+path, data=None if value is None else json.dumps(value).encode(),headers=h,method=method)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            text=r.read();return r.status,dict(r.headers),json.loads(text) if text else None
    except urllib.error.HTTPError as e:return e.code,dict(e.headers),None
class Client:
    def __init__(self):
        self.seq=0
        code,h,r=request('/mcp',{'jsonrpc':'2.0','id':0,'method':'initialize','params':{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'delta-e2e','version':'1'}}})
        assert code==200,r
        self.h={'mcp-session-id':h['mcp-session-id'],'mcp-protocol-version':'2025-11-25'}
        code,_,_=request('/mcp',{'jsonrpc':'2.0','method':'notifications/initialized'},self.h)
        assert code==202
    def rpc(self,method,params={}):
        self.seq+=1
        c,h,r=request('/mcp',{'jsonrpc':'2.0','id':self.seq,'method':method,'params':params},self.h)
        assert c==200,(c,r)
        assert 'error' not in r,r
        return r['result']
    def tool(self,name,args={},error=False):
        r=self.rpc('tools/call',{'name':name,'arguments':args})
        if error:assert r.get('isError'),r;return r
        assert not r.get('isError'),r
        if r['content'][0]['type']=='image':return r['content'][0]
        text=r['content'][0]['text']
        try:return json.loads(text)
        except ValueError:return text
    def close(self):return request('/mcp',headers=self.h,method='DELETE')[0]

check('HTTP auth rejects missing token',request('/mcp',{}, {'Authorization':''})[0]==401)
check('HTTP rejects browser origins',request('/mcp',{}, {'Origin':'https://evil.example'})[0]==401)
a,b=Client(),Client()
try:
    check('62 discoverable tools',len(a.rpc('tools/list')['tools'])==62)
    check('resource discovery',len(a.rpc('resources/list')['resources'])==5)
    check('prompt retrieval',bool(a.rpc('prompts/get',{'name':'test-script'})['messages']))
    a.tool('navigate',{'url':PAGE})
    check('title',a.tool('get_title')=='Delta Test App')
    text=a.tool('snapshot',{'interactive_only':True})
    check('accessibility snapshot', 'textbox "Email"' in text)
    import re
    ref=re.search(r'textbox "Email" \[ref=(.*?)\]',text).group(1)
    a.tool('fill',{'ref':'@'+ref,'value':'agent@example.com'})
    check('ref fill',a.tool('evaluate',{'script':'document.querySelector("#email").value'})=='agent@example.com')
    a.tool('snapshot')
    check('stale ref rejected',a.tool('click',{'ref':ref},error=True)['isError'])
    a.tool('fill',{'selector':'#password','value':'dummy'})
    a.tool('click',{'selector':'button'})
    a.tool('wait_for',{'selector':'#result'})
    check('real login result','agent@example.com' in a.tool('get_text',{'selector':'#result'}))
    check('attribute',a.tool('get_attribute',{'selector':'#email','attribute':'type'})=='email')
    check('geometry',a.tool('get_bounding_box',{'selector':'button'})['width']>0)
    a.tool('hover',{'selector':'button'})
    a.tool('scroll',{'direction':'down','pixels':20})
    a.tool('set_viewport',{'width':900,'height':700})
    image=a.tool('screenshot',{'full_page':True})
    check('real PNG',base64.b64decode(image['data']).startswith(b'\x89PNG'))
    check('real PDF',base64.b64decode(a.tool('save_pdf')['data']).startswith(b'%PDF'))
    check('DOM', '<form' in a.tool('get_dom'))
    a.tool('set_cookie',{'name':'private','value':'session-a','url':PAGE})
    a.tool('evaluate',{'script':'localStorage.setItem("private","a")'})
    b.tool('navigate',{'url':PAGE})
    check('cookie isolation',not any(c['name']=='private' for c in b.tool('get_cookies')['cookies']))
    check('localStorage isolation',b.tool('evaluate',{'script':'localStorage.getItem("private")'}) is None)
    aid=a.tool('list_tabs')[0]['id']
    check('tab isolation',all(t['id']!=aid for t in b.tool('list_tabs')))
    check('foreign tab switch denied',b.tool('switch_tab',{'tab_id':aid},error=True)['isError'])
    tab=a.tool('new_tab')['tab_id'];a.tool('switch_tab',{'tab_id':aid});a.tool('close_tab',{'tab_id':tab})
    check('tab lifecycle',len(a.tool('list_tabs'))==1)
    a.tool('go_back');a.tool('go_forward');a.tool('reload')
    check('file URL rejected',a.tool('navigate',{'url':'file:///etc/passwd'},error=True)['isError'])
    check('raw CDP not exposed',a.tool('cdp_raw',{'method':'Browser.close'},error=True)['isError'])
    check('schema type validation',a.tool('fill',{'value':123},error=True)['isError'])
    check('wait timeout',a.tool('wait_for',{'selector':'#missing','timeout_ms':10},error=True)['isError'])
    steps=Path('examples/login.delta').read_text()
    report=a.tool('test_run',{'steps':steps})
    check('DSL 10-step login',report['passed']==10 and report['failed']==0)
    Path('artifacts').mkdir(exist_ok=True)
    Path('artifacts/e2e-dsl.json').write_text(json.dumps(report,indent=2))
    bad=a.tool('test_run',{'steps':'assert title "Wrong"\nclick "button"'})
    check('DSL fail-fast and skipped steps',bad['failed']==1 and bad['skipped']==1)
    check('DSL no arbitrary processes',a.tool('test_run',{'steps':'serve "rm -rf /"'},error=True)['isError'])
    check('honest status','no Delta kernel policy' in a.tool('server_status')['sandbox'])
finally:
    check('session deletion',a.close()==204 and b.close()==204)

messages=[{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}},{'jsonrpc':'2.0','method':'notifications/initialized'},{'jsonrpc':'2.0','id':2,'method':'tools/list'},{'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'navigate','arguments':{'url':PAGE}}}]
p=subprocess.run([BIN,'mcp'],input=''.join(json.dumps(m)+'\n' for m in messages),text=True,capture_output=True,timeout=60)
rows=[json.loads(l) for l in p.stdout.splitlines()]
check('stdio MCP discovery and real navigation',p.returncode==0 and len(rows)==3 and not rows[2]['result']['isError'])
summary={'passed':len(checks),'failed':0,'checks':checks,'note':'Local Chromium with --no-sandbox for this sandboxed test environment; not a security audit.'}
Path('artifacts/e2e-results.json').write_text(json.dumps(summary,indent=2))
print(json.dumps(summary,indent=2))
