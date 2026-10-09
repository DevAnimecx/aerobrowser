#!/usr/bin/env python3
import urllib.request,json,os,subprocess,tempfile,pathlib,tomllib
BIN=os.environ.get('AERO_BIN','target/debug/aero');BASE=os.environ.get('AERO_SERVER','http://127.0.0.1:8931');TOKEN=os.environ['AERO_TOKEN']
checks=[]
def check(n,v):assert v,n;checks.append(n)
def event(stream):
 fields={}
 while True:
  line=stream.readline().decode().strip()
  if not line and fields:return fields
  if ': ' in line:
   k,v=line.split(': ',1);fields[k]=v
headers={'Authorization':'Bearer '+TOKEN,'Accept':'text/event-stream'}
with urllib.request.urlopen(urllib.request.Request(BASE+'/sse',headers=headers),timeout=15) as stream:
 e=event(stream);check('legacy SSE endpoint event',e['event']=='endpoint')
 endpoint=e['data']
 def post(v):
  req=urllib.request.Request(BASE+endpoint,data=json.dumps(v).encode(),headers={'Authorization':'Bearer '+TOKEN,'Content-Type':'application/json'})
  with urllib.request.urlopen(req,timeout=15) as r:return r.status
 check('SSE initialize accepted',post({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}})==202)
 e=event(stream);check('SSE JSON-RPC response',json.loads(e['data'])['result']['serverInfo']['name']=='aerobrowser')
 post({'jsonrpc':'2.0','method':'notifications/initialized'})
 post({'jsonrpc':'2.0','id':2,'method':'tools/list'})
 e=event(stream);check('SSE 61 tools',len(json.loads(e['data'])['result']['tools'])==61)
with tempfile.TemporaryDirectory() as td:
 for agent in ['claude-code','claude-desktop','cursor','windsurf','cline','roo-code','vscode','opencode','codex','hermes']:
  path=pathlib.Path(td)/(agent+('.toml' if agent=='codex' else '.yaml' if agent=='hermes' else '.json'))
  if agent=='codex':path.write_text('model = "preserve-me"\n')
  elif agent=='hermes':path.write_text('model: preserve-me\n')
  else:path.write_text(json.dumps({'unrelated':'preserve-me'}))
  args=[BIN,'--engine','external','profile','install',agent,'--path',str(path)]
  p=subprocess.run(args,capture_output=True,text=True);assert p.returncode==0,(agent,p.stderr)
  r=json.loads(p.stdout);check(agent+' backup',pathlib.Path(r['backup']).exists())
  content=path.read_text();check(agent+' preserves unrelated settings','preserve-me' in content)
  if agent=='codex':check('Codex TOML schema',tomllib.loads(content)['mcp_servers']['aero']['args']==['mcp'])
  elif agent=='hermes':check('Hermes YAML schema','mcp_servers:' in content)
  else:check(agent+' valid JSON',isinstance(json.loads(content),dict))
  p=subprocess.run(args,capture_output=True,text=True);check(agent+' conflict refused',p.returncode!=0)
  p=subprocess.run(args+['--force'],capture_output=True,text=True);check(agent+' explicit replace',p.returncode==0)
pathlib.Path(os.environ.get('AERO_EVIDENCE_DIR','artifacts/v04')+'/transports-profiles.json').write_text(json.dumps({'passed':len(checks),'checks':checks},indent=2));print(json.dumps({'passed':len(checks),'checks':checks},indent=2))
