#!/usr/bin/env python3
import subprocess,json,os,tempfile,pathlib
BIN=os.environ.get('AERO_BIN','target/debug/aero')
with tempfile.TemporaryDirectory(prefix='aero-kernel-test-') as td:
 d=pathlib.Path(td);(d/'read.txt').write_text('allowed');(d/'secret.txt').write_text('must remain unchanged');(d/'writes').mkdir()
 subprocess.run(['cc','tests/sandbox_probe.c','-pthread','-o',str(d/'probe')],check=True)
 results=[]
 for mode in ['all','none']:
  policy={'name':'test-'+mode,'read':[p for p in ['/usr','/lib','/lib64','/etc/ld.so.cache',str(d/'read.txt')] if pathlib.Path(p).exists()],'write':[str(d/'writes')],'network':mode,'allow_child_processes':False,'cpu_seconds':20,'max_open_files':128}
  (d/'policy.json').write_text(json.dumps(policy))
  args=[BIN,'sandbox','exec','--policy',str(d/'policy.json'),'--',str(d/'probe'),str(d/'read.txt'),str(d/'secret.txt'),str(d/'writes'/('output-'+mode)),str(int(mode=='all'))]
  p=subprocess.run(args,env={**os.environ,'AERO_TOKEN':'must-not-leak'},text=True,capture_output=True,timeout=30)
  assert p.returncode==0,(p.stdout,p.stderr)
  result=json.loads(p.stdout);assert result['failed']==0,result
  assert (d/'secret.txt').read_text()=='must remain unchanged'
  assert (d/'read.txt').read_text()=='allowed'
  results.append({'profile':mode,'result':result})
 pathlib.Path(os.environ.get('AERO_EVIDENCE_DIR','artifacts/v04')+'/sandbox-results.json').write_text(json.dumps({'results':results,'checks_per_profile':11,'platform':os.uname().sysname},indent=2))
 print(json.dumps(results,indent=2))
