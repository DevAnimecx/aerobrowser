use crate::mcp::{error, Session};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
struct Request {
    value: Value,
    result: oneshot::Sender<Option<Value>>,
    cancel: CancellationToken,
    key: Option<String>,
}
pub struct Handle {
    tx: mpsc::Sender<Request>,
    pending: Arc<Mutex<HashMap<String, CancellationToken>>>,
    closed: Arc<AtomicBool>,
    stop: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.stop.cancel(); /* actor owns Session and performs bounded cleanup */
    }
}
impl Handle {
    pub fn new(endpoint: String, timeout: Duration) -> Arc<Self> {
        let (tx, mut rx) = mpsc::channel::<Request>(8);
        let pending = Arc::new(Mutex::new(HashMap::<String, CancellationToken>::new()));
        let map = pending.clone();
        let closed = Arc::new(AtomicBool::new(false));
        let flag = closed.clone();
        let stop = CancellationToken::new();
        let stopping = stop.clone();
        let task = tokio::spawn(async move {
            let mut session = Session::new(endpoint);
            loop {
                let req = tokio::select! {biased;_=stopping.cancelled()=>break,r=rx.recv()=>match r{Some(r)=>r,None=>break}};
                let id = req.value.get("id").cloned().unwrap_or(Value::Null);
                let started = std::time::Instant::now();
                let reply = if req.cancel.is_cancelled() {
                    Some(error(id, -32800, "Request cancelled before execution"))
                } else {
                    let mut invalidated = false;
                    let reply = tokio::select! {biased;
                      _=stopping.cancelled()=>{invalidated=true;Some(error(id.clone(),-32800,"Session closed"))},
                      _=req.cancel.cancelled()=>{invalidated=true;Some(error(id.clone(),-32800,"Request cancelled; browser context invalidated; action may have completed"))},
                      r=tokio::time::timeout(timeout,session.rpc(req.value))=>match r{Ok(r)=>r,Err(_)=>{invalidated=true;Some(error(id.clone(),-32002,"Request deadline exceeded; browser context invalidated; action may have completed"))}}
                    };
                    if invalidated {
                        session.abort_browser().await;
                    }
                    reply
                };
                if let Some(k) = req.key {
                    map.lock().unwrap().remove(&k);
                }
                let _ = req.result.send(reply);
                session.logs.push_back(
                    json!({"event":"rpc_complete","duration_ms":started.elapsed().as_millis()}),
                );
                while session.logs.len() > 1000 {
                    session.logs.pop_front();
                }
                if stopping.is_cancelled() {
                    break;
                }
            }
            flag.store(true, Ordering::Release);
            for token in map.lock().unwrap().values() {
                token.cancel();
            }
            map.lock().unwrap().clear();
            while let Ok(req) = rx.try_recv() {
                let id = req.value.get("id").cloned().unwrap_or(Value::Null);
                let _ = req.result.send(Some(error(id, -32800, "Session closed")));
            }
            session.abort_browser().await;
        });
        Arc::new(Self {
            tx,
            pending,
            closed,
            stop,
            task,
        })
    }
    pub fn submit(&self, v: Value) -> oneshot::Receiver<Option<Value>> {
        let (tx, rx) = oneshot::channel();
        let id = v.get("id").cloned().unwrap_or(Value::Null);
        if v["jsonrpc"] == "2.0"
            && v["method"] == "notifications/cancelled"
            && v.get("id").is_none()
        {
            let key = v["params"]["requestId"].to_string();
            if let Some(t) = self.pending.lock().unwrap().get(&key) {
                t.cancel();
            }
            let _ = tx.send(None);
            return rx;
        }
        if self.closed.load(Ordering::Acquire) || self.stop.is_cancelled() {
            let _ = tx.send(Some(error(id, -32000, "Session closed")));
            return rx;
        }
        let cancel = CancellationToken::new();
        let key = v.get("id").map(Value::to_string);
        if let Some(key) = &key {
            let mut map = self.pending.lock().unwrap();
            if map.contains_key(key) {
                let _ = tx.send(Some(error(id, -32600, "Duplicate in-flight request ID")));
                return rx;
            }
            map.insert(key.clone(), cancel.clone());
        }
        let req = Request {
            value: v,
            result: tx,
            cancel,
            key: key.clone(),
        };
        if let Err(e) = self.tx.try_send(req) {
            if let Some(k) = key {
                self.pending.lock().unwrap().remove(&k);
            }
            let req = e.into_inner();
            let _ = req.result.send(Some(error(
                id,
                -32001,
                "Session queue full (maximum 8 waiting requests)",
            )));
        }
        rx
    }
    pub async fn rpc(&self, v: Value) -> Option<Value> {
        self.submit(v)
            .await
            .unwrap_or_else(|_| Some(error(Value::Null, -32603, "Session actor stopped")))
    }
    pub fn active(&self) -> bool {
        !self.pending.lock().unwrap().is_empty()
    }
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
    pub async fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.stop.cancel();
        for token in self.pending.lock().unwrap().values() {
            token.cancel();
        }
        let t = std::time::Instant::now();
        while !self.task.is_finished() && t.elapsed() < Duration::from_secs(5) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn fifo() {
        let h = Handle::new("unreachable".into(), Duration::from_secs(1));
        let a=h.submit(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}));
        let b = h.submit(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        let c = h.submit(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
        assert!(a.await.unwrap().unwrap()["result"].is_object());
        assert!(b.await.unwrap().is_none());
        assert!(c.await.unwrap().unwrap()["result"]["tools"].is_array());
        h.close().await;
    }
}
