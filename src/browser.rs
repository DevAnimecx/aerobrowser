use crate::cdp::Cdp;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// One element as previously emitted by a snapshot.
#[derive(Clone)]
pub(crate) struct DeltaEntry {
    /// The ref the agent was given, used for tombstones.
    pub(crate) reference: String,
    /// Fingerprint of the fields that affect agent decisions.
    pub(crate) fingerprint: String,
}

/// Fingerprint of one previously emitted snapshot, kept per tab so that
/// `snapshot { delta: true }` can report only what changed since then.
///
/// Keyed by backend DOM node id, not by ref. Refs are generation-scoped and are
/// reissued on every snapshot by design, so keying on them would make every
/// element look simultaneously added and removed.
#[derive(Default, Clone)]
pub(crate) struct SnapshotDelta {
    pub(crate) entries: HashMap<i64, DeltaEntry>,
    pub(crate) document: Option<i64>,
    pub(crate) url: String,
}

pub struct Browser {
    pub(crate) cdp: Cdp,
    pub(crate) context: String,
    pub(crate) extra_contexts: Vec<String>,
    pub(crate) tabs: HashMap<String, String>,
    pub(crate) active: String,
    pub(crate) refs: HashMap<String, i64>,
    pub(crate) generation: u64,
    pub(crate) document: Option<i64>,
    /// Per-tab fingerprint of the last emitted snapshot, used by `delta:true`.
    pub(crate) snapshot_deltas: HashMap<String, SnapshotDelta>,
    started: Instant,
    pub(crate) artifacts: crate::artifacts::Artifacts,
    pub(crate) engine: Option<crate::engine::Engine>,
    pub(crate) engine_info: Value,
}
pub fn string<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str()
        .ok_or_else(|| anyhow!("Missing string parameter: {k}"))
}
pub(crate) fn url_allowed(url: &str) -> Result<()> {
    if url == "about:blank" {
        return Ok(());
    }
    let u = reqwest::Url::parse(url)?;
    if !matches!(u.scheme(), "http" | "https") {
        bail!("Only HTTP(S) and about:blank navigation supported");
    }
    Ok(())
}
impl Browser {
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let engine = crate::engine::Engine::launch(endpoint).await?;
        let actual = engine
            .as_ref()
            .map(|e| e.endpoint.as_str())
            .unwrap_or(endpoint);
        let engine_info=engine.as_ref().map(|e|e.info.clone()).unwrap_or(json!({"engine":"external-cdp","sandbox":"external browser responsibility; no Delta kernel policy"}));
        let mut cdp = Cdp::connect(actual).await?;
        let r = cdp
            .call(
                "Target.createBrowserContext",
                json!({"disposeOnDetach":true}),
                None,
            )
            .await?;
        let context = string(&r, "browserContextId")?.to_owned();
        let mut b = Self {
            cdp,
            context,
            extra_contexts: Vec::new(),
            tabs: HashMap::new(),
            active: String::new(),
            refs: HashMap::new(),
            generation: 0,
            document: None,
            snapshot_deltas: HashMap::new(),
            started: Instant::now(),
            artifacts: crate::artifacts::Artifacts::new()?,
            engine,
            engine_info,
        };
        b.new_tab("about:blank").await?;
        Ok(b)
    }
    pub(crate) async fn call(&mut self, m: &str, p: Value) -> Result<Value> {
        let sid = self
            .tabs
            .get(&self.active)
            .ok_or_else(|| anyhow!("No active tab; use new_tab"))?
            .clone();
        self.cdp.call(m, p, Some(&sid)).await
    }
    pub(crate) async fn new_tab(&mut self, url: &str) -> Result<Value> {
        url_allowed(url)?;
        if self.tabs.len() >= 10 {
            bail!("Tab quota exceeded (10)");
        }
        let r = self
            .cdp
            .call(
                "Target.createTarget",
                json!({"url":"about:blank","browserContextId":self.context}),
                None,
            )
            .await?;
        let id = string(&r, "targetId")?.to_owned();
        let r = self
            .cdp
            .call(
                "Target.attachToTarget",
                json!({"targetId":id,"flatten":true}),
                None,
            )
            .await?;
        self.tabs
            .insert(id.clone(), string(&r, "sessionId")?.to_owned());
        self.active = id.clone();
        self.invalidate();
        self.call("Page.enable", json!({})).await?;
        self.call("Network.enable", json!({})).await?;
        self.call("Runtime.enable", json!({})).await?;
        let _ = self.call("Performance.enable", json!({})).await;
        let _ = self.call("Page.addScriptToEvaluateOnNewDocument",json!({"source":"try {globalThis.__deltaLcp=null;new PerformanceObserver(l=>{globalThis.__deltaLcp=l.getEntries().at(-1)?.startTime??null}).observe({type:'largest-contentful-paint',buffered:true})}catch{}"})).await;
        if url != "about:blank" {
            self.navigate(url).await?;
        }
        Ok(json!({"tab_id":id,"url":url}))
    }
    pub(crate) fn invalidate(&mut self) {
        self.refs.clear();
        self.document = None;
        self.generation += 1;
    }
    pub async fn eval(&mut self, script: &str) -> Result<Value> {
        let v = self
            .call(
                "Runtime.evaluate",
                json!({"expression":script,"awaitPromise":true,"returnByValue":true}),
            )
            .await?;
        if !v["exceptionDetails"].is_null() {
            bail!("JavaScript error: {}", v["exceptionDetails"]);
        }
        Ok(v["result"]["value"].clone())
    }
    async fn navigate(&mut self, url: &str) -> Result<Value> {
        url_allowed(url)?;
        self.invalidate();
        if let Some(sid) = self.tabs.get(&self.active) {
            let mut e = self.cdp.events.lock().unwrap();
            e.inflight.remove(sid);
            e.activity.insert(sid.clone(), Instant::now());
        }
        let t = Instant::now();
        let r = self.call("Page.navigate", json!({"url":url})).await?;
        if let Some(e) = r["errorText"].as_str() {
            bail!("Navigation: {e}");
        }
        for _ in 0..300 {
            if let Ok(v) = self.eval("document.readyState").await {
                if v == "complete" || v == "interactive" {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            if t.elapsed() > Duration::from_secs(15) {
                bail!("Navigation readiness timeout");
            }
        }
        Ok(
            json!({"status":"ok","url":self.eval("location.href").await?,"title":self.eval("document.title").await?,"load_time_ms":t.elapsed().as_millis()}),
        )
    }
    pub(crate) async fn document_id(&mut self) -> Result<i64> {
        let v = self.call("DOM.getDocument", json!({"depth":0})).await?;
        v["root"]["backendNodeId"]
            .as_i64()
            .ok_or_else(|| anyhow!("Missing document ID"))
    }
    pub(crate) async fn element(&mut self, args: &Value) -> Result<String> {
        if args["ref"].is_string() && args["selector"].is_string() {
            bail!("Provide ref OR selector, not both");
        }
        let r = if let Some(s) = args["selector"].as_str() {
            self.call("Runtime.evaluate",json!({"expression":format!("document.querySelector({})",json!(s)),"objectGroup":"delta-action"})).await?
        } else {
            let key = string(args, "ref")?.trim_start_matches('@');
            let id = *self
                .refs
                .get(key)
                .ok_or_else(|| anyhow!("Unknown/stale ref; take a new snapshot"))?;
            if Some(self.document_id().await?) != self.document {
                self.invalidate();
                bail!("Document changed; take a new snapshot");
            }
            self.call(
                "DOM.resolveNode",
                json!({"backendNodeId":id,"objectGroup":"delta-action"}),
            )
            .await?
        };
        r["result"]["objectId"]
            .as_str()
            .or_else(|| r["object"]["objectId"].as_str())
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("Element not found"))
    }
    pub(crate) async fn on_element(
        &mut self,
        a: &Value,
        func: &str,
        values: Vec<Value>,
    ) -> Result<Value> {
        if func.contains("getBoundingClientRect") {
            self.refresh_layout().await?;
        }
        let id = self.element(a).await?;
        let args: Vec<_> = values.into_iter().map(|v| json!({"value":v})).collect();
        let result=self.call("Runtime.callFunctionOn",json!({"objectId":id,"functionDeclaration":func,"arguments":args,"returnByValue":true,"awaitPromise":true})).await;
        let _ = self
            .call(
                "Runtime.releaseObjectGroup",
                json!({"objectGroup":"delta-action"}),
            )
            .await;
        let r = result?;
        if !r["exceptionDetails"].is_null() {
            bail!("Element operation failed: {}", r["exceptionDetails"]);
        }
        Ok(r["result"]["value"].clone())
    }
    pub async fn run(&mut self, name: &str, a: Value) -> Result<Value> {
        match name {
            "navigate"=>{
                let r=self.navigate(string(&a,"url")?).await?;
                let timeout=a["timeout_ms"].as_u64().unwrap_or(15000).clamp(1,30000);
                match a["wait_until"].as_str().unwrap_or("domcontentloaded"){
                    "networkidle"=>self.wait_network_idle(timeout).await?,
                    "load"=>{let start=Instant::now();while self.eval("document.readyState").await?!="complete"{if start.elapsed().as_millis()>timeout as u128{bail!("Load timeout");}tokio::time::sleep(Duration::from_millis(50)).await;}},
                    "domcontentloaded"=>{},_=>bail!("Invalid wait_until"),
                }Ok(r)
            },
            "new_tab"=>self.new_tab(a["url"].as_str().unwrap_or("about:blank")).await,
            "snapshot"=>self.snapshot(&a).await,
            "evaluate"=>self.eval(string(&a,"script")?).await,
            "get_url"=>self.eval("location.href").await,
            "get_title"=>self.eval("document.title").await,
            "get_dom"=>if a["selector"].is_string(){self.on_element(&a,"function(){return this.outerHTML}",vec![]).await}else{self.eval("document.documentElement.outerHTML").await},
            "get_text"=>self.on_element(&a,"function(){return this.innerText ?? this.textContent}",vec![]).await,
            "get_attribute"=>self.on_element(&a,"function(k){return this.getAttribute(k)}",vec![json!(string(&a,"attribute")?)]).await,
            "get_bounding_box"=>self.on_element(&a,"function(){return this.getBoundingClientRect().toJSON()}",vec![]).await,
            "fill"|"select_option"=>self.on_element(&a,"function(v){if(!this.isConnected)throw Error('Detached element'); this.focus(); const p=this instanceof HTMLTextAreaElement?HTMLTextAreaElement.prototype:this instanceof HTMLSelectElement?HTMLSelectElement.prototype:HTMLInputElement.prototype; const s=Object.getOwnPropertyDescriptor(p,'value').set;s.call(this,v);this.dispatchEvent(new Event('input',{bubbles:true}));this.dispatchEvent(new Event('change',{bubbles:true}));return {status:'ok'}}",vec![json!(string(&a,"value")?)]).await,
            "type"=>{self.on_element(&a,"function(){this.focus()}",vec![]).await?;self.call("Input.insertText",json!({"text":string(&a,"value")?})).await},
            "check"|"uncheck"=>self.on_element(&a,"function(v){if(this.disabled)throw Error('Disabled');if(this.checked!==v)this.click();if(this.checked!==v)throw Error('State did not change');return {status:'ok'}}",vec![json!(name=="check")]).await,
            "click"|"hover"=>{
                let r=self.on_element(&a,"function(){if(!this.isConnected||this.disabled)throw Error('Detached or disabled');this.scrollIntoView({block:'center'});const r=this.getBoundingClientRect();if(!r.width||!r.height)throw Error('Not visible');return {x:r.x+r.width/2,y:r.y+r.height/2}}",vec![]).await?;
                self.call("Input.dispatchMouseEvent",json!({"type":"mouseMoved","x":r["x"],"y":r["y"]})).await?;
                if name=="click" {for typ in ["mousePressed","mouseReleased"] {self.call("Input.dispatchMouseEvent",json!({"type":typ,"x":r["x"],"y":r["y"],"button":"left","clickCount":1})).await?;}}
                Ok(json!({"status":"ok"}))
            }
            "press_key"=>{
                let input=string(&a,"key")?;let parts:Vec<_>=input.split('+').collect();let k=*parts.last().unwrap();let mut modifiers=0;
                for m in &parts[..parts.len()-1]{modifiers|=match *m {"Alt"=>1,"Control"|"Ctrl"=>2,"Meta"|"Command"=>4,"Shift"=>8,_=>bail!("Unsupported key modifier")};}
                let code=match k {"Enter"=>13,"Tab"=>9,"Escape"=>27,"Backspace"=>8,"Delete"=>46,"ArrowLeft"=>37,"ArrowUp"=>38,"ArrowRight"=>39,"ArrowDown"=>40,"Home"=>36,"End"=>35,"PageUp"=>33,"PageDown"=>34,_ if k.chars().count()==1=>k.to_ascii_uppercase().as_bytes()[0] as u32,_=>bail!("Unsupported key")};
                let text=if modifiers&7!=0{""}else if k=="Enter"{"\r"}else if k.chars().count()==1{k}else{""};
                let commands=if modifiers&(2|4)!=0&&k.eq_ignore_ascii_case("a"){vec!["selectAll"]}else{vec![]};
                for typ in ["keyDown","keyUp"] {self.call("Input.dispatchKeyEvent",json!({"type":typ,"key":k,"modifiers":modifiers,"windowsVirtualKeyCode":code,"text":if typ=="keyDown"{text}else{""},"commands":if typ=="keyDown"{commands.clone()}else{vec![]}})).await?;}
                Ok(json!({"status":"ok"}))
            }
            "scroll"=>{
                let p=a["pixels"].as_i64().unwrap_or(500).clamp(0,100000);
                let (x,y)=match string(&a,"direction")? {"up"=>(0,-p),"down"=>(0,p),"left"=>(-p,0),"right"=>(p,0),_=>bail!("Invalid direction")};
                self.eval(&format!("scrollBy({x},{y}); ({{x:scrollX,y:scrollY}})")).await
            }
            "wait_for"=>{
                let s=string(&a,"selector")?;let state=a["state"].as_str().unwrap_or("visible");
                if !["visible","hidden","attached"].contains(&state){bail!("Invalid wait state");}
                let script=format!("(()=>{{const e=document.querySelector({});const r=e?.getBoundingClientRect(); const v=!!e&&r.width>0&&r.height>0&&getComputedStyle(e).visibility!=='hidden';return {} }})()",json!(s),match state{"hidden"=>"!v","attached"=>"!!e",_=>"v"});
                let t=Instant::now();let limit=a["timeout_ms"].as_u64().unwrap_or(5000).clamp(1,30000);
                loop {
                    self.refresh_layout().await?;
                    if self.eval(&script).await? == true {
                        return Ok(json!({"status":"ok","elapsed_ms":t.elapsed().as_millis()}));
                    }
                    if t.elapsed().as_millis() >= limit as u128 {
                        bail!("Wait timed out: {s}");
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
            "screenshot"=>{
                if a["annotate"]==true {return Box::pin(self.extended("annotate_screenshot",json!({}))).await;}
                let format=a["format"].as_str().unwrap_or("png"); if !["png","jpeg"].contains(&format){bail!("Invalid format");}
                let mut p=json!({"format":format,"captureBeyondViewport":a["full_page"]==true});
                if format=="jpeg"{if let Some(q)=a["quality"].as_u64(){p["quality"]=json!(q.min(100));}}
                if let Some(selector)=a["selector"].as_str(){
                    self.refresh_layout().await?;
                    let r=self.on_element(&json!({"selector":selector}),"function(){const r=this.getBoundingClientRect();return {x:r.x+scrollX,y:r.y+scrollY,width:r.width,height:r.height,scale:1}}",vec![]).await?;
                    p["clip"]=r;p["captureBeyondViewport"]=json!(true);
                }else if a["full_page"]==true {let m=self.call("Page.getLayoutMetrics",json!({})).await?;let s=&m["cssContentSize"];p["clip"]=json!({"x":0,"y":0,"width":s["width"],"height":s["height"],"scale":1});}
                let r=self.call("Page.captureScreenshot",p).await?;
                let mut result=json!({"data":r["data"],"mimeType":format!("image/{format}")});
                if let Some(path)=a["path"].as_str(){use base64::Engine;let bytes=base64::engine::general_purpose::STANDARD.decode(string(&result,"data")?)?;result["path"]=json!(self.artifacts.write(path,&bytes)?);}
                Ok(result)
            }
            "save_pdf"=>{
                let mut p=json!({"printBackground":true,"landscape":a["landscape"]==true});
                match a["format"].as_str().unwrap_or("a4"){"a4"=>{p["paperWidth"]=json!(8.27);p["paperHeight"]=json!(11.69);},"letter"=>{p["paperWidth"]=json!(8.5);p["paperHeight"]=json!(11.);},_=>bail!("Invalid paper format")};
                let mut r=self.call("Page.printToPDF",p).await?;
                if let Some(path)=a["path"].as_str(){use base64::Engine;let bytes=base64::engine::general_purpose::STANDARD.decode(string(&r,"data")?)?;r["path"]=json!(self.artifacts.write(path,&bytes)?);r["size_bytes"]=json!(bytes.len());}Ok(r)
            },
            "set_viewport"=>{let w=a["width"].as_u64().filter(|v|*v>0&&*v<=7680).ok_or_else(||anyhow!("Invalid width"))?;let h=a["height"].as_u64().filter(|v|*v>0&&*v<=7680).ok_or_else(||anyhow!("Invalid height"))?;self.call("Emulation.setDeviceMetricsOverride",json!({"width":w,"height":h,"deviceScaleFactor":1,"mobile":false})).await},
            "reload"=>{self.invalidate();self.call("Page.reload",json!({"ignoreCache":a["ignore_cache"]==true})).await},
            "go_back"|"go_forward"=>{
                let h=self.call("Page.getNavigationHistory",json!({})).await?;let idx=h["currentIndex"].as_i64().unwrap_or(0)+if name=="go_back"{-1}else{1};
                let e=h["entries"].as_array().and_then(|a|if idx>=0 {a.get(idx as usize)}else{None}).ok_or_else(||anyhow!("No history entry"))?;
                self.invalidate();self.call("Page.navigateToHistoryEntry",json!({"entryId":e["id"]})).await
            }
            "list_tabs"=>{
                let r=self.cdp.call("Target.getTargets",json!({}),None).await?;
                let entries:Vec<_>=r["targetInfos"].as_array().unwrap_or(&vec![]).iter().filter(|t|t["targetId"].as_str().is_some_and(|id|self.tabs.contains_key(id))).map(|t|json!({"id":t["targetId"],"title":t["title"],"url":t["url"],"active":t["targetId"]==self.active})).collect();Ok(json!(entries))
            }
            "switch_tab"=>{let id=string(&a,"tab_id")?;if !self.tabs.contains_key(id){bail!("Tab not owned by session");}self.active=id.to_owned();self.invalidate();Ok(json!({"status":"ok"}))},
            "close_tab"=>{let id=string(&a,"tab_id")?;if !self.tabs.contains_key(id){bail!("Tab not owned by session");}self.cdp.call("Target.closeTarget",json!({"targetId":id}),None).await?;self.tabs.remove(id);if self.active==id{self.active=self.tabs.keys().next().cloned().unwrap_or_default();}self.invalidate();Ok(json!({"status":"ok"}))},
            "get_cookies"=>self.cdp.call("Storage.getCookies",json!({"browserContextId":self.context}),None).await,
            "set_cookie"=>{url_allowed(string(&a,"url")?)?;self.cdp.call("Storage.setCookies",json!({"browserContextId":self.context,"cookies":[{"name":string(&a,"name")?,"value":string(&a,"value")?,"url":string(&a,"url")?}]}),None).await},
            "server_status"=>Ok(json!({"engine":self.engine_info["engine"],"engine_details":self.engine_info,"artifacts":self.artifacts.root,"tabs":self.tabs.len(),"uptime_s":self.started.elapsed().as_secs(),"sandbox":self.engine_info["sandbox"],"isolation":if self.engine.is_some(){"dedicated managed worker per session"}else{"browser context, not process or crash isolation"},"tool_count":crate::schema::tools().len(),"server_rss_mb":rss_mb(std::process::id()),"worker_rss_mb":self.engine.as_ref().and_then(|e|e.pid()).and_then(rss_mb),"memory_scope":"individual process RSS; use resources for cgroup tree accounting","resources":self.engine.as_ref().map(|e|e.resource_stats()),"telemetry":self.cdp.events.lock().unwrap().stats(),"network_mocks_enabled":std::env::var("DELTA_ALLOW_NETWORK_MOCKS").is_ok_and(|v|v=="1")})),
            _=>Box::pin(self.extended(name,a)).await,
        }
    }
    pub(crate) async fn refresh_layout(&mut self) -> Result<()> {
        if self.engine_info["engine"] == "moli" {
            // Moli v1.1.12 JS geometry can remain stale until a paint after hidden changes.
            self.call(
                "Page.captureScreenshot",
                json!({"format":"png","clip":{"x":0,"y":0,"width":1,"height":1,"scale":1}}),
            )
            .await?;
        }
        Ok(())
    }
    pub async fn dispose(&mut self) -> Result<()> {
        if let Some(engine) = self.engine.as_mut() {
            return engine.shutdown().await;
        }
        let mut contexts = self.extra_contexts.clone();
        contexts.push(self.context.clone());
        let mut errors = Vec::new();
        for context in contexts {
            if let Err(e) = self
                .cdp
                .call(
                    "Target.disposeBrowserContext",
                    json!({"browserContextId":context}),
                    None,
                )
                .await
            {
                errors.push(e.to_string());
            }
        }
        if !errors.is_empty() {
            bail!("Context cleanup failed: {}", errors.join("; "));
        }
        Ok(())
    }
}

fn rss_mb(pid: u32) -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        let s = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        let line = s.lines().find(|s| s.starts_with("VmRSS:"))?;
        Some(line.split_whitespace().nth(1)?.parse::<f64>().ok()? / 1024.)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}
