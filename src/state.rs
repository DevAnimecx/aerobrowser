//! Staged browser-context restore. External website effects are not transactional.
use crate::browser::{string, url_allowed, Browser};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
pub fn validate(state: &Value) -> Result<()> {
    if state["version"] != 1 {
        bail!("Unsupported state version");
    }
    if serde_json::to_vec(state)?.len() > 16 * 1024 * 1024 {
        bail!("State exceeds 16 MiB");
    }
    let tabs = state["tabs"]
        .as_array()
        .ok_or_else(|| anyhow!("Invalid state tabs"))?;
    if tabs.is_empty() || tabs.len() > 10 {
        bail!("State must contain 1..10 tabs");
    }
    let mut origins = HashMap::new();
    let mut active = 0;
    for t in tabs {
        let url = string(t, "url")?;
        url_allowed(url)?;
        let origin = if url == "about:blank" {
            "null".into()
        } else {
            reqwest::Url::parse(url)?.origin().ascii_serialization()
        };
        if t["origin"] != origin {
            bail!("State origin does not match tab URL");
        }
        if t["active"] == true {
            active += 1;
        }
        for k in ["localStorage", "sessionStorage"] {
            let values = t[k]
                .as_object()
                .ok_or_else(|| anyhow!("Invalid {k} object"))?;
            if !values.values().all(Value::is_string) {
                bail!("Storage values must be strings");
            }
            if origin == "null" && !values.is_empty() {
                bail!("Opaque origins cannot restore Web Storage");
            }
        }
        if origin != "null" {
            if let Some(previous) = origins.insert(origin, t["localStorage"].clone()) {
                if previous != t["localStorage"] {
                    bail!("Conflicting localStorage for the same origin");
                }
            }
        }
        for k in ["x", "y"] {
            if t["scroll"][k].as_f64().is_none_or(|n| !n.is_finite()) {
                bail!("Invalid scroll coordinates");
            }
        }
    }
    if active > 1 {
        bail!("At most one restored tab may be active");
    }
    let cookies = state["cookies"]
        .as_array()
        .ok_or_else(|| anyhow!("Invalid cookies"))?;
    if cookies.len() > 3000 {
        bail!("Cookie quota exceeded");
    }
    for c in cookies {
        string(c, "name")?;
        string(c, "value")?;
        string(c, "domain")?;
        string(c, "path")?;
    }
    Ok(())
}
impl Browser {
    pub(crate) async fn restore_transaction(&mut self, state: Value) -> Result<Value> {
        validate(&state)?;
        // Old context stays live and untouched until all new tabs have passed validation.
        let created = self
            .cdp
            .call(
                "Target.createBrowserContext",
                json!({"disposeOnDetach":true}),
                None,
            )
            .await?;
        let next = string(&created, "browserContextId")?.to_owned();
        let old = std::mem::replace(&mut self.context, next.clone());
        self.extra_contexts.push(old.clone());
        let old_tabs = std::mem::take(&mut self.tabs);
        let old_active = std::mem::take(&mut self.active);
        self.invalidate();
        let prepared = self.populate_state(&state).await;
        if let Err(e) = prepared {
            self.context = old.clone();
            self.tabs = old_tabs;
            self.active = old_active;
            self.invalidate();
            self.extra_contexts.retain(|id| id != &old);
            self.extra_contexts.push(next.clone());
            match self.cdp.call("Target.disposeBrowserContext",json!({"browserContextId":next}),None).await {
    Ok(_)=>{self.extra_contexts.retain(|id|id!=&next);bail!("Restore rolled back; original browser context retained: {e}");},
    Err(cleanup)=>bail!("Restore rolled back locally; staged context cleanup pending until close: {e}; {cleanup}")
   }
        }
        let cleanup = self
            .cdp
            .call(
                "Target.disposeBrowserContext",
                json!({"browserContextId":old}),
                None,
            )
            .await;
        if cleanup.is_ok() {
            self.extra_contexts.retain(|id| id != &old);
        }
        self.invalidate();
        Ok(
            json!({"status":"ok","restored_tabs":self.tabs.len(),"storage_applied_after_load":false,"restore_mode":"staged-context; storage before page scripts","previous_context_cleanup_warning":cleanup.err().map(|e|e.to_string()),"omitted":state["omitted"],"note":"Browser-context commit only. Requests and external website effects during staging are not rolled back. Cancellation invalidates the entire session browser."}),
        )
    }
    async fn populate_state(&mut self, state: &Value) -> Result<()> {
        let cookies: Vec<Value> = state["cookies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let mut o = serde_json::Map::new();
                for k in [
                    "name",
                    "value",
                    "domain",
                    "path",
                    "secure",
                    "httpOnly",
                    "sameSite",
                    "expires",
                    "priority",
                    "sameParty",
                    "sourceScheme",
                    "sourcePort",
                    "partitionKey",
                ] {
                    if let Some(v) = c.get(k) {
                        o.insert(k.into(), v.clone());
                    }
                }
                Value::Object(o)
            })
            .collect();
        self.cdp
            .call(
                "Storage.setCookies",
                json!({"browserContextId":self.context,"cookies":cookies}),
                None,
            )
            .await?;
        let mut active = None;
        for t in state["tabs"].as_array().unwrap() {
            let tab = self.new_tab("about:blank").await?;
            let marker = format!("__delta_restore_{}", uuid::Uuid::new_v4().simple());
            if t["origin"] != "null" {
                let source=format!("(()=>{{if(window.top!==window||location.origin!=={})return;try{{localStorage.clear();sessionStorage.clear();for(const [k,v]of Object.entries({}))localStorage.setItem(k,v);for(const [k,v]of Object.entries({}))sessionStorage.setItem(k,v);globalThis[{}]={{ok:true}};}}catch(e){{globalThis[{}]={{ok:false,error:String(e)}};}}}})()",t["origin"],t["localStorage"],t["sessionStorage"],json!(marker),json!(marker));
                let r = self
                    .call(
                        "Page.addScriptToEvaluateOnNewDocument",
                        json!({"source":source}),
                    )
                    .await
                    .map_err(|e| {
                        anyhow!("Engine cannot guarantee pre-boot storage restore: {e}")
                    })?;
                let script = string(&r, "identifier")?.to_owned();
                let nav = self.run("navigate", json!({"url":t["url"]})).await;
                let remove = self
                    .call(
                        "Page.removeScriptToEvaluateOnNewDocument",
                        json!({"identifier":script}),
                    )
                    .await;
                nav?;
                remove?;
                if self.eval("location.origin").await? != t["origin"] {
                    bail!("Restore redirected to another origin; state not injected there");
                }
                let verified = self.eval(&format!("globalThis[{}]", json!(marker))).await?;
                if verified["ok"] != true {
                    bail!("Pre-boot storage initialization not confirmed: {verified}");
                }
                self.eval(&format!("delete globalThis[{}]", json!(marker)))
                    .await?;
            }
            self.eval(&format!(
                "scrollTo({}, {})",
                t["scroll"]["x"], t["scroll"]["y"]
            ))
            .await?;
            if t["active"] == true {
                active = tab["tab_id"].as_str().map(str::to_owned);
            }
        }
        if let Some(id) = active {
            self.active = id;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> Value {
        json!({"version":1,"cookies":[],"tabs":[{"url":"https://example.com/","origin":"https://example.com","active":true,"localStorage":{"x":"1"},"sessionStorage":{},"scroll":{"x":0,"y":0}}]})
    }
    #[test]
    fn valid() {
        assert!(validate(&state()).is_ok());
    }
    #[test]
    fn origin_mismatch() {
        let mut s = state();
        s["tabs"][0]["origin"] = json!("https://evil.example");
        assert!(validate(&s).is_err());
    }
    #[test]
    fn conflicts() {
        let mut s = state();
        let mut t = s["tabs"][0].clone();
        t["active"] = json!(false);
        t["localStorage"]["x"] = json!("2");
        s["tabs"].as_array_mut().unwrap().push(t);
        assert!(validate(&s).is_err());
    }
}
