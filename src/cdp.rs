use anyhow::{anyhow, bail, Result};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Default)]
pub struct Events {
    pub network: EventBuffer,
    pub console: EventBuffer,
    pub rules: HashMap<String, Vec<Value>>,
    pub dropped: u64,
    pub inflight: HashMap<String, HashSet<String>>,
    pub activity: HashMap<String, std::time::Instant>,
}
struct Command {
    id: u64,
    message: Value,
    result: oneshot::Sender<std::result::Result<Value, String>>,
}
pub struct Cdp {
    tx: mpsc::Sender<Command>,
    next: u64,
    pub events: Arc<Mutex<Events>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Cdp {
    fn drop(&mut self) {
        self.task.abort();
    }
}
/// Serialized-byte retention budget, not a claim about allocator/RSS limits.
#[derive(Default)]
pub struct EventBuffer {
    entries: VecDeque<(Value, usize)>,
    bytes: usize,
    dropped: u64,
    oversized: u64,
}
impl EventBuffer {
    const BUDGET: usize = 4 * 1024 * 1024;
    const ITEM: usize = 256 * 1024;
    pub fn iter(&self) -> impl Iterator<Item = &Value> {
        self.entries.iter().map(|(v, _)| v)
    }
    pub fn push(&mut self, v: Value) -> u64 {
        let size = serde_json::to_vec(&v)
            .map(|b| b.len())
            .unwrap_or(usize::MAX);
        if size > Self::ITEM {
            self.dropped += 1;
            self.oversized += 1;
            return 1;
        }
        let mut dropped = 0;
        while self.entries.len() >= 1000 || self.bytes + size > Self::BUDGET {
            if let Some((_, n)) = self.entries.pop_front() {
                self.bytes -= n;
                dropped += 1;
            } else {
                break;
            }
        }
        self.bytes += size;
        self.entries.push_back((v, size));
        self.dropped += dropped;
        dropped
    }
    pub fn stats(&self) -> Value {
        json!({"retained_events":self.entries.len(),"retained_serialized_bytes":self.bytes,"max_serialized_bytes":Self::BUDGET,"max_event_bytes":Self::ITEM,"dropped_events":self.dropped,"oversized_events":self.oversized})
    }
}
impl Events {
    pub fn stats(&self) -> Value {
        json!({"network":self.network.stats(),"console":self.console.stats(),"scope":"per CDP connection; serialized retained data only, not supervisor RSS"})
    }
}
pub fn matches(pattern: &str, text: &str) -> bool {
    // Linear wildcard matcher; only '*' and '?' are special, no regex backtracking.
    let (p, t) = (pattern.as_bytes(), text.as_bytes());
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < t.len() {
        if i < p.len() && (p[i] == b'?' || p[i] == t[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == b'*' {
            star = Some(i);
            i += 1;
            mark = j;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == b'*' {
        i += 1;
    }
    i == p.len()
}
impl Cdp {
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let url = if endpoint.starts_with("ws") {
            endpoint.to_owned()
        } else {
            let v: Value = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?
                .get(format!("{}/json/version", endpoint.trim_end_matches('/')))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            v["webSocketDebuggerUrl"]
                .as_str()
                .ok_or_else(|| anyhow!("Missing browser WebSocket URL"))?
                .to_owned()
        };
        let (mut socket, _) = tokio::time::timeout(Duration::from_secs(10), connect_async(url))
            .await
            .map_err(|_| anyhow!("CDP connection timed out"))??;
        let (tx, mut rx) = mpsc::channel::<Command>(64);
        let events = Arc::new(Mutex::new(Events::default()));
        let shared = events.clone();
        let task = tokio::spawn(async move {
            let mut pending: HashMap<u64, oneshot::Sender<std::result::Result<Value, String>>> =
                HashMap::new();
            let mut automatic = 1_000_000_000u64;
            loop {
                tokio::select! {
                    cmd=rx.recv()=>{let Some(cmd)=cmd else{break};pending.retain(|_,r|!r.is_closed());if socket.send(Message::Text(cmd.message.to_string().into())).await.is_err(){let _=cmd.result.send(Err("CDP write failed".into()));break;}pending.insert(cmd.id,cmd.result);},
                    msg=socket.next()=>{
                        match msg {
                            Some(Ok(Message::Text(s)))=>{
                                let Ok(v)=serde_json::from_str::<Value>(&s)else{continue};
                                if let Some(id)=v["id"].as_u64(){if let Some(p)=pending.remove(&id){let _=p.send(if v["error"].is_null(){Ok(v["result"].clone())}else{Err(format!("CDP: {}",v["error"]))});}continue;}
                                let method=v["method"].as_str().unwrap_or("");
                                let session=v["sessionId"].as_str().unwrap_or("");let p=&v["params"];
                                let automatic_command={
                                    let mut e=shared.lock().unwrap();
                                    if let Some(id)=p["requestId"].as_str(){
                                        if method=="Network.requestWillBeSent"{e.inflight.entry(session.into()).or_default().insert(id.into());e.activity.insert(session.into(),std::time::Instant::now());}
                                        if method=="Network.loadingFinished"||method=="Network.loadingFailed"{e.inflight.entry(session.into()).or_default().remove(id);e.activity.insert(session.into(),std::time::Instant::now());}
                                    }

                                    if method.starts_with("Network.")&&["Network.requestWillBeSent","Network.responseReceived","Network.dataReceived","Network.loadingFinished","Network.loadingFailed"].contains(&method){
                                        let n=e.network.push(v.clone());e.dropped+=n;
                                    }
                                    if method=="Runtime.consoleAPICalled"||method=="Runtime.exceptionThrown"||method=="Log.entryAdded" {
                                        let n=e.console.push(v.clone());e.dropped+=n;
                                    }
                                    if method=="Fetch.requestPaused" {
                                        let rule=e.rules.get(session).and_then(|rules|rules.iter().rev().find(|r|matches(r["pattern"].as_str().unwrap_or(""),p["request"]["url"].as_str().unwrap_or(""))));
                                        let (method,params)=match rule.and_then(|r|r["action"].as_str()) {
                                            Some("block")=>("Fetch.failRequest",json!({"requestId":p["requestId"],"errorReason":"BlockedByClient"})),
                                            Some("mock")=>{
                                                let r=&rule.unwrap()["mock_response"];
                                                let headers:Vec<Value>=r["headers"].as_object().map(|o|o.iter().map(|(k,v)|json!({"name":k,"value":v.as_str().unwrap_or("")})).collect()).unwrap_or_default();
                                                ("Fetch.fulfillRequest",json!({"requestId":p["requestId"],"responseCode":r["status"].as_u64().unwrap_or(200),"responseHeaders":headers,"body":base64::engine::general_purpose::STANDARD.encode(r["body"].as_str().unwrap_or(""))}))
                                            }
                                            _=>("Fetch.continueRequest",json!({"requestId":p["requestId"]})),
                                        };
                                        if method=="Fetch.fulfillRequest"||method=="Fetch.failRequest" {
                                            let record=json!({"method":"Delta.interception","sessionId":session,"params":{"requestId":p["networkId"],"requested_action":if method=="Fetch.fulfillRequest"{"mock"}else{"block"},"execution":"requested; inspect response/error for outcome"}});
                                            let n=e.network.push(record);e.dropped+=n;
                                        }
                                        automatic+=1;Some(json!({"id":automatic,"method":method,"params":params,"sessionId":session}))
                                    }else{None}
                                };
                                if let Some(c)=automatic_command{if socket.send(Message::Text(c.to_string().into())).await.is_err(){break;}}
                            }
                            Some(Ok(Message::Ping(p)))=>{if socket.send(Message::Pong(p)).await.is_err(){break;}},
                            Some(Ok(Message::Close(_)))|None|Some(Err(_))=>break,
                            _=>{}
                        }
                    }
                }
            }
            for (_, p) in pending {
                let _ = p.send(Err("Browser disconnected".into()));
            }
        });
        Ok(Self {
            tx,
            next: 0,
            events,
            task,
        })
    }
    pub async fn call(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value> {
        self.next += 1;
        let id = self.next;
        let mut message = json!({"id":id,"method":method,"params":params});
        if let Some(s) = session {
            message["sessionId"] = json!(s);
        }
        let (tx, rx) = oneshot::channel();
        let command = Command {
            id,
            message,
            result: tx,
        };
        tokio::time::timeout(Duration::from_secs(30), async {
            self.tx
                .send(command)
                .await
                .map_err(|_| anyhow!("CDP reader stopped"))?;
            match rx.await {
                Ok(Ok(v)) => Ok(v),
                Ok(Err(e)) => bail!("{e}"),
                Err(_) => bail!("CDP reader stopped"),
            }
        })
        .await
        .map_err(|_| anyhow!("CDP command timed out; action may have completed"))?
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wildcards() {
        assert!(matches("*/api/?", "https://site/api/1"));
        assert!(!matches("*/api/?", "https://site/api/12"));
        assert!(matches("*", ""));
        assert!(!matches("a", ""));
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn oversized_is_omitted() {
        let mut b = EventBuffer::default();
        assert_eq!(b.push(json!("x".repeat(300000))), 1);
        assert_eq!(b.stats()["retained_events"], 0);
        assert_eq!(b.stats()["oversized_events"], 1);
    }
    #[test]
    fn bytes_and_count_bounded() {
        let mut b = EventBuffer::default();
        for _ in 0..80 {
            b.push(json!("x".repeat(100000)));
        }
        assert!(b.bytes <= EventBuffer::BUDGET);
        assert!(b.dropped > 0);
        let mut b = EventBuffer::default();
        for i in 0..1100 {
            b.push(json!(i));
        }
        assert_eq!(b.entries.len(), 1000);
        assert_eq!(b.dropped, 100);
    }
}
