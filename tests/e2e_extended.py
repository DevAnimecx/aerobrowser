import os
#!/usr/bin/env python3
import e2e as t
import json, base64
from pathlib import Path
c=t.Client()
try:
    c.tool('navigate',{'url':t.PAGE,'wait_until':'networkidle'})
    c.tool('evaluate',{'script':'''document.body.insertAdjacentHTML('beforeend', `<input id="upload" type="file"><input type="checkbox" id="agree"><select id="select"><option value="a">A</option><option value="b">B</option></select><div id="drag">Drag</div><div id="drop" style="width:100px;height:50px">Drop</div><button id="double">Double</button><button id="right">Right</button>`);document.querySelector('#drop').addEventListener('mouseup',e=>e.target.textContent='Dropped');document.querySelector('#double').ondblclick=e=>e.target.textContent='Doubled';document.querySelector('#right').oncontextmenu=e=>{e.preventDefault();e.target.textContent='Right clicked'};console.log('delta console proof');true;'''})
    c.tool('check',{'selector':'#agree'})
    t.check('check real checkbox',c.tool('evaluate',{'script':'document.querySelector("#agree").checked'}) is True)
    c.tool('uncheck',{'selector':'#agree'})
    t.check('uncheck real checkbox',c.tool('evaluate',{'script':'document.querySelector("#agree").checked'}) is False)
    c.tool('select_option',{'selector':'#select','value':'b'})
    t.check('native select',c.tool('evaluate',{'script':'document.querySelector("#select").value'})=='b')
    c.tool('fill',{'selector':'#email','value':'a'})
    c.tool('type',{'selector':'#email','value':'bc'})
    c.tool('press_key',{'key':'Backspace'})
    t.check('text insertion and keyboard',c.tool('evaluate',{'script':'document.querySelector("#email").value'})=='ab')
    c.tool('click',{'selector':'#email'})
    c.tool('press_key',{'key':'Control+A'})
    c.tool('type',{'selector':'#email','value':'replacement@example.com'})
    t.check('keyboard chord',c.tool('evaluate',{'script':'document.querySelector("#email").value'})=='replacement@example.com')
    subtree=c.tool('snapshot',{'selector':'#login','interactive_only':True,'max_depth':10,'cursor_interactive':True})
    t.check('scoped snapshot excludes other controls','Right' not in subtree and 'Email' in subtree)
    jpeg=c.tool('screenshot',{'selector':'#login','format':'jpeg','quality':70})
    t.check('element JPEG screenshot',base64.b64decode(jpeg['data']).startswith(b'\xff\xd8'))
    style=c.tool('get_computed_style',{'selector':'button','properties':['display','color']})
    t.check('computed style',bool(style['color']))
    c.tool('upload_file',{'selector':'#upload','path':'login.delta'})
    t.check('real file upload',c.tool('evaluate',{'script':'document.querySelector("#upload").files[0].name'})=='login.delta')
    t.check('upload traversal denied',c.tool('upload_file',{'selector':'#upload','path':'../Cargo.toml'},error=True)['isError'])
    c.tool('drag',{'from_selector':'#drag','to_selector':'#drop'})
    t.check('Native pointer drag release',c.tool('get_text',{'selector':'#drop'})=='Dropped')
    c.tool('double_click',{'selector':'#double'})
    c.tool('right_click',{'selector':'#right'})
    t.check('double click',c.tool('get_text',{'selector':'#double'})=='Doubled')
    t.check('right click',c.tool('get_text',{'selector':'#right'})=='Right clicked')
    image=c.tool('annotate_screenshot',{'path':'annotated.png'})
    t.check('annotation PNG',base64.b64decode(image['data']).startswith(b'\x89PNG'))
    t.check('annotation cleanup',c.tool('evaluate',{'script':'document.querySelectorAll("[id^=__delta_annotation_]").length'})==0)
    c.tool('evaluate',{'script':'localStorage.setItem("state-test","local");sessionStorage.setItem("state-test","session");true'})
    c.tool('set_cookie',{'name':'state-cookie','value':'yes','url':t.PAGE})
    saved=c.tool('save_state',{'path':'state.json'})
    c.tool('evaluate',{'script':'localStorage.clear();sessionStorage.clear();true'})
    c.tool('clear_cookies')
    restored=c.tool('restore_state',{'state':saved['state']})
    t.check('state tab count',restored['restored_tabs']==1)
    t.check('state localStorage',c.tool('evaluate',{'script':'localStorage.getItem("state-test")'})=='local')
    t.check('state sessionStorage',c.tool('evaluate',{'script':'sessionStorage.getItem("state-test")'})=='session')
    t.check('state cookies',any(x['name']=='state-cookie' for x in c.tool('get_cookies')['cookies']))
    t.check('state artifact traversal denied',c.tool('save_state',{'path':'../secret'},error=True)['isError'])
    t.check('assert attr',c.tool('test_assert_attr',{'selector':'#email','attribute':'type','expected':'email'})['passed'])
    t.check('assert text mismatch observable',not c.tool('test_assert_text',{'selector':'button','expected':'Wrong'})['passed'])
    t.check('assert URL',c.tool('test_assert_url',{'expected':'login.html','contains':True})['passed'])
    t.check('assert visible',c.tool('test_assert_visible',{'selector':'button'})['passed'])
    c.tool('intercept_request',{'pattern':'*/mock-api','action':'mock','mock_response':{'status':201,'headers':{'content-type':'application/json'},'body':'{"mocked":true}'}})
    t.check('real Fetch mock',c.tool('evaluate',{'script':'fetch("/mock-api").then(async r=>({status:r.status,data:await r.json()}))'})=={'status':201,'data':{'mocked':True}})
    c.tool('intercept_request',{'pattern':'*/blocked-api','action':'block'})
    t.check('real request blocking',c.tool('evaluate',{'script':'fetch("/blocked-api").then(()=>false,()=>true)'}) is True)
    c.tool('intercept_request',{'pattern':'*','action':'clear'})
    t.check('mock cleared',c.tool('evaluate',{'script':'fetch("/mock-api").then(r=>r.status)'})==404)
    c.tool('evaluate',{'script':'console.warn("captured-warning");true'})
    t.check('console events',any('captured-warning' in r['message'] for r in c.tool('get_console_logs')))
    t.check('network events',len(c.tool('get_network_log')['events'])>0)
    har=c.tool('save_har',{'path':'capture.har'})
    t.check('HAR entries',len(har['har']['log']['entries'])>0)
    t.check('memory measured',c.tool('get_memory_usage')['dom_nodes']>0)
    perf=c.tool('get_performance')
    t.check('performance measured not fabricated',perf['dom_ready'] is not None and perf['tti'] is None)
    c.tool('set_user_agent',{'ua':'DeltaBrowser-E2E'})
    t.check('user-agent override',c.tool('evaluate',{'script':'navigator.userAgent'})=='DeltaBrowser-E2E')
    c.tool('set_geolocation',{'latitude':22.7,'longitude':75.8})
    t.check('geolocation bounds',c.tool('set_geolocation',{'latitude':122,'longitude':75},error=True)['isError'])
    t.check('safe raw CDP',bool(c.tool('cdp_raw',{'method':'DOM.getDocument'})['root']))
    script='''open http://127.0.0.1:4173/login.html --wait "#email" 3000
if visible "#absent"
  click "#never"
end
if visible "#email"
  fill "#email" "recipe@example.com"
end
assert attr "#email" "type" "email"
assert count "button" 1
assert js "document.querySelector('#email').value" "recipe@example.com"
get title -> title
get attr "#email" "type" -> input_type
log "recipe complete"
screenshot extended-dsl.png --annotate
pdf extended-dsl.pdf
wait_for networkidle 3000
'''
    report=c.tool('test_run',{'steps':script})
    t.check('expanded DSL',report['failed']==0 and report['skipped']==1 and report['data']['input_type']=='email')
    t.check('DSL artifacts',len(report['artifacts'])==2)
    html=c.tool('test_report',{'path':'report.html','format':'html'})
    t.check('HTML test report',html['path'].endswith('report.html'))
    import shutil
    shutil.copy2(html['path'],os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/test-report.html')
    for artifact in report['artifacts']:
        shutil.copy2(artifact['path'],Path('artifacts')/('v02-'+Path(artifact['path']).name))
    t.check('session logs',len(c.tool('server_logs',{'lines':10}))==10)
    t.check('profile list',any(p['name']=='offline' and p['implemented'] for p in c.tool('server_profile_list')))
    t.check('cannot reprofile active worker',c.tool('server_profile_set',{'name':'offline'},error=True)['isError'])
    t.check('shutdown is session scoped',c.tool('server_shutdown')['scope']=='this session only')
    t.check('browser lazy restart',c.tool('get_url')=='about:blank')
    t.check('close',c.tool('close')['status']=='closed')
finally:c.close()
summary={'passed':len(t.checks),'failed':0,'checks':t.checks,'version':'0.2.0','engine':'external Chromium; native sandbox disabled only inside test environment'}
Path(os.environ.get('DELTA_EVIDENCE_DIR','artifacts/v04')+'/e2e-results.json').write_text(json.dumps(summary,indent=2))
print(json.dumps(summary,indent=2))
