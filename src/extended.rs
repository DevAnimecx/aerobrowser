use crate::browser::{string, Browser};
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde_json::{json, Value};
impl Browser {
    pub(crate) async fn extended(&mut self, name: &str, a: Value) -> Result<Value> {
        match name {
            "get_computed_style"=>self.on_element(&a,"function(keys){const s=getComputedStyle(this);return Object.fromEntries((keys||Array.from(s)).map(k=>[k,s.getPropertyValue(k)]))}",vec![a.get("properties").cloned().unwrap_or(Value::Null)]).await,
            "upload_file"=>{
                let path=crate::artifacts::Artifacts::upload(string(&a,"path")?)?;
                let object=self.element(&a).await?;
                let result=self.call("DOM.setFileInputFiles",json!({"objectId":object,"files":[path]})).await;
                let _=self.call("Runtime.releaseObjectGroup",json!({"objectGroup":"delta-action"})).await;result
            }
            "drag"=>{
                let from=if let Some(r)=a["from_ref"].as_str(){json!({"ref":r})}else{json!({"selector":string(&a,"from_selector")?})};
                let to=if let Some(r)=a["to_ref"].as_str(){json!({"ref":r})}else{json!({"selector":string(&a,"to_selector")?})};
                let point="function(){const r=this.getBoundingClientRect();const x=r.x+r.width/2,y=r.y+r.height/2;if(!r.width||!r.height||x<0||y<0||x>=innerWidth||y>=innerHeight)throw Error('Native drag requires both elements visible in the viewport');return {x,y}}";
                let src=self.on_element(&from,point,vec![]).await?;let dst=self.on_element(&to,point,vec![]).await?;
                let (x,y)=(src["x"].as_f64().ok_or_else(||anyhow!("Source geometry unavailable"))?,src["y"].as_f64().ok_or_else(||anyhow!("Source geometry unavailable"))?);
                let (dx,dy)=(dst["x"].as_f64().ok_or_else(||anyhow!("Target geometry unavailable"))?,dst["y"].as_f64().ok_or_else(||anyhow!("Target geometry unavailable"))?);
                self.call("Input.dispatchMouseEvent",json!({"type":"mouseMoved","x":x,"y":y})).await?;
                self.call("Input.dispatchMouseEvent",json!({"type":"mousePressed","x":x,"y":y,"button":"left","buttons":1,"clickCount":1})).await?;
                let movement=async {for i in 1..=20{let t=i as f64/20.;self.call("Input.dispatchMouseEvent",json!({"type":"mouseMoved","x":x+(dx-x)*t,"y":y+(dy-y)*t,"button":"left","buttons":1})).await?;tokio::time::sleep(std::time::Duration::from_millis(16)).await;}Ok::<(),anyhow::Error>(())}.await;
                let release=self.call("Input.dispatchMouseEvent",json!({"type":"mouseReleased","x":dx,"y":dy,"button":"left","buttons":0,"clickCount":1})).await;
                movement?;release?;Ok(json!({"input_dispatched":true,"mode":"native-cdp-pointer","from":src,"to":dst,"note":"No synthetic DOM events. Application drop outcome must be asserted separately; HTML5 DnD support depends on engine."}))
            }
            "double_click"|"right_click"=>{
                let r=self.on_element(&a,"function(){this.scrollIntoView({block:'center'});const r=this.getBoundingClientRect();if(!r.width||!r.height)throw Error('Not visible');return {x:r.x+r.width/2,y:r.y+r.height/2}}",vec![]).await?;
                let count=if name=="double_click"{2}else{1};let button=if name=="right_click"{"right"}else{"left"};
                for click in 1..=count{for typ in ["mousePressed","mouseReleased"]{self.call("Input.dispatchMouseEvent",json!({"type":typ,"x":r["x"],"y":r["y"],"button":button,"clickCount":click})).await?;}}
                Ok(json!({"status":"ok"}))
            }
            "annotate_screenshot"=>{
                // A temporary, pointer-transparent overlay is always removed after capture.
                let snapshot=self.run("snapshot",json!({"interactive_only":true})).await?;
                let mut elements=Vec::new();
                for e in snapshot["elements"].as_array().unwrap(){let r=self.on_element(&json!({"ref":e["ref"]}),"function(){const r=this.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height}}",vec![]).await;if let Ok(r)=r{elements.push(json!({"ref":e["ref"],"label":e["name"],"rect":r}));}}
                let overlay=format!("(()=>{{const root=document.createElement('div');root.id='__delta_annotation_{}';root.style='position:fixed;inset:0;pointer-events:none;z-index:2147483647';for(const e of {}){{const n=document.createElement('div');const r=e.rect;n.textContent=e.ref;n.style=`position:absolute;left:${{r.x}}px;top:${{r.y}}px;width:${{r.width}}px;height:${{r.height}}px;border:2px solid #f43f5e;color:white;font:12px monospace;background:#f43f5e22;box-sizing:border-box`;const label=document.createElement('span');label.textContent=e.ref;label.style='background:#be123c;color:#fff';n.textContent='';n.append(label);root.append(n);}}document.documentElement.append(root);return root.id}})()",uuid::Uuid::new_v4().simple(),json!(elements));
                let id=self.eval(&overlay).await?;let capture=self.run("screenshot",json!({})).await;
                let _=self.eval(&format!("document.getElementById({})?.remove()",id)).await;
                let mut image=capture?;image["elements"]=json!(elements);
                if let Some(path)=a["path"].as_str(){let bytes=base64::engine::general_purpose::STANDARD.decode(string(&image,"data")?)?;image["path"]=json!(self.artifacts.write(path,&bytes)?);}
                Ok(image)
            }
            "save_state"=>{
                let active=self.active.clone();let mut ids:Vec<_>=self.tabs.keys().cloned().collect();ids.sort();let mut tabs=Vec::new();
                for id in ids{self.active=id.clone();let result=self.eval("(()=>({url:location.href,origin:location.origin,localStorage:location.protocol.startsWith('http')?Object.fromEntries(Object.entries(localStorage)):{},sessionStorage:location.protocol.startsWith('http')?Object.fromEntries(Object.entries(sessionStorage)):{},scroll:{x:scrollX,y:scrollY}}))()").await;match result{Ok(mut v)=>{v["active"]=json!(id==active);tabs.push(v);},Err(e)=>{self.active=active;return Err(e);}}}
                self.active=active;let cookies=self.cdp.call("Storage.getCookies",json!({"browserContextId":self.context}),None).await?;
                let state=json!({"version":1,"cookies":cookies["cookies"],"tabs":tabs,"omitted":["history","IndexedDB","serviceWorkers","in-flight JavaScript","CacheStorage","open connections"]});
                let path=self.artifacts.write(a["path"].as_str().unwrap_or("state.json"),&serde_json::to_vec_pretty(&state)?)?;
                Ok(json!({"path":path,"state":state}))
            }
            "restore_state"=>{
                let state=if a["state"].is_object(){a["state"].clone()}else{self.artifacts.read_json(string(&a,"path")?)?};
                self.restore_transaction(state).await
            }
            "intercept_request"=>{
                let action=string(&a,"action")?;if action=="mock"{if !std::env::var("AERO_ALLOW_NETWORK_MOCKS").is_ok_and(|v|v=="1"){bail!("Network mocks disabled. Only an operator can opt into testing with AERO_ALLOW_NETWORK_MOCKS=1");}let m=&a["mock_response"];if !m["body"].is_string()||m["status"].as_u64().is_some_and(|n|!(100..=599).contains(&n)){bail!("Mock needs a string body and optional status 100..599");}}
                let sid=self.tabs.get(&self.active).ok_or_else(||anyhow!("No active tab"))?.clone();
                {let mut e=self.cdp.events.lock().unwrap();let rules=e.rules.entry(sid.clone()).or_default();if action=="clear"{rules.clear();}else{if rules.len()>=50{bail!("Rule quota exceeded");}rules.push(a.clone());}}
                if action=="clear"{self.call("Fetch.disable",json!({})).await?;}else{self.call("Fetch.enable",json!({"patterns":[{"urlPattern":"*","requestStage":"Request"}]})).await?;}
                Ok(json!({"status":"ok","scope":"current tab","pattern":a["pattern"]}))
            }
            "get_network_log"=>Ok(json!({"events":self.network_events(a["filter"].as_str()),"dropped_events":self.cdp.events.lock().unwrap().dropped})),
            "get_console_logs"=>{
                let sid=self.tabs.get(&self.active);let e=self.cdp.events.lock().unwrap();
                let rows:Vec<_>=e.console.iter().filter(|v|v["sessionId"].as_str()==sid.map(String::as_str)).map(|v|{let p=&v["params"];let level=p["type"].as_str().or(p["entry"]["level"].as_str()).unwrap_or("error");let message=if let Some(args)=p["args"].as_array(){args.iter().map(|v|v.get("value").map(Value::to_string).unwrap_or_else(||v["description"].as_str().unwrap_or("").into())).collect::<Vec<_>>().join(" ")}else if p["entry"].is_object(){p["entry"]["text"].as_str().unwrap_or("").to_owned()}else{p["exceptionDetails"].to_string()};json!({"level":level,"message":message,"timestamp":p["timestamp"]})}).filter(|v|a["level"].as_str().is_none_or(|l|v["level"]==l)).collect();Ok(json!(rows))
            }
            "save_har"=>{
                let events=self.network_events(None);
                let dropped=self.cdp.events.lock().unwrap().dropped;
                let har=crate::har::export(events,dropped);
                let path=self.artifacts.write(a["path"].as_str().unwrap_or("network.har"),&serde_json::to_vec_pretty(&har)?)?;Ok(json!({"path":path,"har":har}))
            }
            "get_performance"=>self.eval("(()=>{const n=performance.getEntriesByType('navigation')[0];const f=performance.getEntriesByName('first-contentful-paint')[0];return {fcp:f?.startTime??null,lcp:globalThis.__deltaLcp??null,tti:null,dom_ready:n?.domContentLoadedEventEnd??null,load:n?.loadEventEnd??null,time_origin:performance.timeOrigin,notes:'milliseconds relative to time origin; unavailable metrics are null; no TTI heuristic'}})()").await,
            "get_memory_usage"=>{
                let h=self.call("Runtime.getHeapUsage",json!({})).await;
                let d=self.call("Memory.getDOMCounters",json!({})).await;
                let (nodes,listeners,documents,scope)=match d {Ok(v)=>(v["nodes"].clone(),v["jsEventListeners"].clone(),v["documents"].clone(),"engine DOM counters"),Err(_)=>{
                    let count=self.eval("(()=>{const w=document.createTreeWalker(document,NodeFilter.SHOW_ALL);let count=1;while(w.nextNode())count++;return count})()").await?;
                    (count,Value::Null,json!(1),"current document traversal only; excludes shadow roots and frames")
                }};
                Ok(json!({"js_heap_mb":h.as_ref().ok().and_then(|v|v["usedSize"].as_f64()).map(|n|n/1048576.),"dom_nodes":nodes,"listeners":listeners,"documents":documents,"scope":scope,"unavailable_metrics_are_null":true}))
            },
            "set_user_agent"=>self.call("Network.setUserAgentOverride",json!({"userAgent":string(&a,"ua")?})).await,
            "set_geolocation"=>{
                let lat=a["latitude"].as_f64().filter(|n|(-90. ..=90.).contains(n)).ok_or_else(||anyhow!("Invalid latitude"))?;
                let lon=a["longitude"].as_f64().filter(|n|(-180. ..=180.).contains(n)).ok_or_else(||anyhow!("Invalid longitude"))?;
                self.call("Emulation.setGeolocationOverride",json!({"latitude":lat,"longitude":lon,"accuracy":a["accuracy"].as_f64().unwrap_or(1.)})).await
            }
            "cdp_raw"=>{
                let method=string(&a,"method")?;
                // Always constrain to current target. Browser/Target/Storage/IO escape hatches are never exposed.
                if !["DOM.getDocument","DOM.getOuterHTML","DOM.describeNode","Runtime.evaluate","Performance.getMetrics","Page.getLayoutMetrics","Memory.getDOMCounters","Runtime.getHeapUsage","Network.getResponseBody"].contains(&method){bail!("CDP method not in target-scoped allowlist");}
                self.call(method,a.get("params").cloned().unwrap_or(json!({}))).await
            }
            "test_assert_text"|"test_assert_attr"|"test_assert_url"|"test_assert_visible"=>{
                let actual=match name {"test_assert_text"=>self.run("get_text",a.clone()).await?,"test_assert_attr"=>self.run("get_attribute",a.clone()).await?,"test_assert_url"=>self.run("get_url",json!({})).await?,_=>self.on_element(&a,"function(){const r=this.getBoundingClientRect();return !!(r.width&&r.height)&&getComputedStyle(this).visibility!=='hidden'}",vec![]).await?};
                let passed=if name=="test_assert_visible"{actual==true}else if a["contains"]==true{actual.as_str().unwrap_or("").contains(string(&a,"expected")?)}else{actual==a["expected"]};
                Ok(json!({"passed":passed,"actual":actual,"expected":a["expected"]}))
            }
            "clear_cookies"=>self.cdp.call("Storage.clearCookies",json!({"browserContextId":self.context}),None).await,
            "close"=>{self.dispose().await?;self.tabs.clear();self.invalidate();self.engine.take();Ok(json!({"status":"closed"}))},
            _=>bail!("Unsupported browser tool: {name}"),
        }
    }
    pub(crate) async fn wait_network_idle(&mut self, timeout: u64) -> Result<()> {
        let sid = self
            .tabs
            .get(&self.active)
            .ok_or_else(|| anyhow!("No active tab"))?
            .clone();
        let start = std::time::Instant::now();
        loop {
            let idle = {
                let e = self.cdp.events.lock().unwrap();
                e.inflight.get(&sid).is_none_or(|s| s.is_empty())
                    && e.activity
                        .get(&sid)
                        .map(|t| t.elapsed())
                        .unwrap_or(start.elapsed())
                        >= std::time::Duration::from_millis(500)
            };
            if idle {
                return Ok(());
            }
            if start.elapsed().as_millis() >= timeout as u128 {
                bail!("Network idle timed out");
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }
    fn network_events(&self, filter: Option<&str>) -> Vec<Value> {
        let sid = self.tabs.get(&self.active);
        self.cdp
            .events
            .lock()
            .unwrap()
            .network
            .iter()
            .filter(|v| v["sessionId"].as_str() == sid.map(String::as_str))
            .filter(|v| filter.is_none_or(|s| v.to_string().contains(s)))
            .cloned()
            .collect()
    }
}
