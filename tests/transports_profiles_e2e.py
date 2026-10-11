#!/usr/bin/env python3
import urllib.request,json,os,subprocess,tempfile,pathlib,tomllib
BIN=os.environ.get('DELTA_BIN','target/debug/delta');BASE=os.environ.get('DELTA_SERVER','http://127.0.0.1:8931');TOKEN=os.environ['DELTA_TOKEN']
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
 e=event(stream);check('SSE JSON-RPC response',json.loads(e['data'])['result']['serverInfo']['name']=='deltabrowser')
 post({'jsonrpc':'2.0','method':'notifications/initialized'})
 post({'jsonrpc':'2.0','id':2,'method':'tools/list'})
 e=event(stream);check('SSE 62 tools',len(json.loads(e['data'])['result']['tools'])==62)
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
  if agent=='codex':check('Codex TOML schema',tomllib.loads(content)['mcp_servers']['delta']['args']==['mcp'])
  elif agent=='hermes':check('Hermes YAML schema','mcp_servers:' in content)
  else:check(agent+' valid JSON',isinstance(json.loads(content),dict))
  # Reinstalling refreshes Delta's own entry rather than conflicting, because
  # the documented one-command install runs on machines that already have a
  # delta entry. What still matters is that the refresh is non-destructive:
  # it succeeds, it backs up, and it leaves everything else alone.
  p=subprocess.run(args,capture_output=True,text=True);check(agent+' refresh succeeds',p.returncode==0)
  r=json.loads(p.stdout);check(agent+' refresh backs up',pathlib.Path(r['backup']).exists())
  check(agent+' refresh preserves unrelated settings','preserve-me' in path.read_text())
  p=subprocess.run(args+['--force'],capture_output=True,text=True);check(agent+' explicit replace',p.returncode==0)
 # generic replaces the whole config file rather than one key, so it is the one
 # path that must still refuse to clobber an existing file without --force.
 g=pathlib.Path(td)/'generic.json';g.write_text(json.dumps({'unrelated':'preserve-me'}))
 gargs=[BIN,'--engine','external','profile','install','generic','--path',str(g)]
 p=subprocess.run(gargs,capture_output=True,text=True);check('generic export refuses without --force',p.returncode!=0)
 p=subprocess.run(gargs+['--force'],capture_output=True,text=True);check('generic export with --force',p.returncode==0)
pathlib.Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/transports-profiles.json').write_text(json.dumps({'passed':len(checks),'checks':checks},indent=2));print(json.dumps({'passed':len(checks),'checks':checks},indent=2))
