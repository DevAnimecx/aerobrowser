use crate::{
    auth::{Auth, Principal},
    session_actor::Handle,
};
use axum::{
    extract::{DefaultBodyLimit, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
type Legacy =
    tokio::sync::mpsc::Sender<Result<axum::response::sse::Event, std::convert::Infallible>>;
struct Entry {
    owner: String,
    handle: Arc<Handle>,
    touched: Instant,
    protocol: String,
    legacy: Option<Legacy>,
}
#[derive(Default)]
struct Registry {
    sessions: HashMap<String, Entry>,
    cli: HashMap<String, Arc<Handle>>,
}
#[derive(Clone)]
struct App {
    endpoint: String,
    auth: Auth,
    registry: Arc<Mutex<Registry>>,
    shutdown: Arc<tokio::sync::Notify>,
    started: Instant,
    timeout: Duration,
}
fn principal(h: &HeaderMap, a: &App) -> Option<Principal> {
    if h.contains_key(header::ORIGIN) {
        return None;
    }
    let token = h
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    a.auth.authenticate(token)
}
fn response(code: StatusCode, v: Value) -> Response {
    (code, axum::Json(v)).into_response()
}
fn unauthorized() -> Response {
    response(
        StatusCode::UNAUTHORIZED,
        json!({"error":"Bearer authentication required; browser origins denied"}),
    )
}
fn protocol(v: &Value) -> String {
    let s = v["params"]["protocolVersion"].as_str().unwrap_or("");
    if ["2025-03-26", "2025-06-18", "2025-11-25"].contains(&s) {
        s
    } else {
        "2025-11-25"
    }
    .into()
}
fn quota(r: &Registry, p: &Principal) -> bool {
    r.sessions.len() + r.cli.len() < 32
        && r.sessions.values().filter(|e| e.owner == p.id).count()
            + usize::from(r.cli.contains_key(&p.id))
            < p.max_sessions
}
async fn post_mcp(
    State(a): State<App>,
    h: HeaderMap,
    axum::Json(v): axum::Json<Value>,
) -> Response {
    let Some(p) = principal(&h, &a) else {
        return unauthorized();
    };
    let init = v["method"] == "initialize";
    let id = if init {
        if h.contains_key("mcp-session-id") || v.get("id").is_none() || v["jsonrpc"] != "2.0" {
            return StatusCode::BAD_REQUEST.into_response();
        }
        uuid::Uuid::new_v4().to_string()
    } else {
        match h.get("mcp-session-id").and_then(|v| v.to_str().ok()) {
            Some(id) => id.into(),
            None => return StatusCode::BAD_REQUEST.into_response(),
        }
    };
    let handle = {
        let mut r = a.registry.lock().await;
        if init {
            if !quota(&r, &p) {
                return StatusCode::TOO_MANY_REQUESTS.into_response();
            }
            r.sessions.insert(
                id.clone(),
                Entry {
                    owner: p.id.clone(),
                    handle: Handle::new(a.endpoint.clone(), a.timeout),
                    touched: Instant::now(),
                    protocol: protocol(&v),
                    legacy: None,
                },
            );
        }
        let Some(e) = r.sessions.get_mut(&id).filter(|e| e.owner == p.id) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        if !init
            && h.get("mcp-protocol-version")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|s| s != e.protocol)
        {
            return StatusCode::BAD_REQUEST.into_response();
        }
        e.touched = Instant::now();
        e.handle.clone()
    };
    let mut out = match handle.rpc(v).await {
        Some(v) => response(StatusCode::OK, v),
        None => StatusCode::ACCEPTED.into_response(),
    };
    if init {
        out.headers_mut()
            .insert("mcp-session-id", id.parse().unwrap());
    }
    out
}
async fn delete_mcp(State(a): State<App>, h: HeaderMap) -> Response {
    let Some(p) = principal(&h, &a) else {
        return unauthorized();
    };
    let Some(id) = h.get("mcp-session-id").and_then(|v| v.to_str().ok()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let e = {
        let mut r = a.registry.lock().await;
        if r.sessions.get(id).is_none_or(|e| e.owner != p.id) {
            return StatusCode::NOT_FOUND.into_response();
        }
        r.sessions.remove(id).unwrap()
    };
    e.handle.close().await;
    StatusCode::NO_CONTENT.into_response()
}
async fn cli(State(a): State<App>, h: HeaderMap, axum::Json(v): axum::Json<Value>) -> Response {
    let Some(p) = principal(&h, &a) else {
        return unauthorized();
    };
    let Some(name) = v["name"].as_str() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let handle = {
        let mut r = a.registry.lock().await;
        if !r.cli.contains_key(&p.id) {
            if !quota(&r, &p) {
                return StatusCode::TOO_MANY_REQUESTS.into_response();
            }
            let handle = Handle::new(a.endpoint.clone(), a.timeout);
            drop(handle.submit(json!({"jsonrpc":"2.0","id":"delta-cli-init","method":"initialize","params":{"protocolVersion":"2025-11-25"}})));
            drop(handle.submit(json!({"jsonrpc":"2.0","method":"notifications/initialized"})));
            r.cli.insert(p.id.clone(), handle);
        }
        r.cli[&p.id].clone()
    };
    let r=handle.rpc(json!({"jsonrpc":"2.0","id":uuid::Uuid::new_v4().to_string(),"method":"tools/call","params":{"name":name,"arguments":v.get("arguments").cloned().unwrap_or(json!({}))}})).await.unwrap_or(Value::Null);
    // A dead or stopped actor, and any request the actor treats as a notification,
    // both surface as a null reply. Report those as unavailable rather than as an
    // empty success payload.
    if r.is_null() {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"status":"error","error":"session actor unavailable; restart with `delta start`"}),
        );
    }
    if r["error"].is_object() {
        return response(
            StatusCode::REQUEST_TIMEOUT,
            json!({"status":"error","error":r["error"]}),
        );
    }
    if r["result"]["isError"] == true {
        return response(
            StatusCode::BAD_REQUEST,
            json!({"status":"error","error":r["result"]["content"][0]["text"]}),
        );
    }
    // structuredContent preserves exact tool data instead of parsing presentation text.
    response(
        StatusCode::OK,
        json!({"status":"ok","result":r["result"]["structuredContent"]["result"]}),
    )
}
async fn legacy_sse(State(a): State<App>, h: HeaderMap) -> Response {
    use axum::response::sse::{Event, KeepAlive, Sse};
    let Some(p) = principal(&h, &a) else {
        return unauthorized();
    };
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    {
        let mut r = a.registry.lock().await;
        if !quota(&r, &p) {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        r.sessions.insert(
            id.clone(),
            Entry {
                owner: p.id,
                handle: Handle::new(a.endpoint.clone(), a.timeout),
                touched: Instant::now(),
                protocol: String::new(),
                legacy: Some(tx.clone()),
            },
        );
    }
    let _ = tx
        .send(Ok(Event::default()
            .event("endpoint")
            .data(format!("/messages?session_id={id}"))))
        .await;
    Sse::new(futures_util::stream::unfold(rx, |mut rx| async {
        rx.recv().await.map(|v| (v, rx))
    }))
    .keep_alive(KeepAlive::default())
    .into_response()
}
async fn legacy_message(
    State(a): State<App>,
    Query(q): Query<HashMap<String, String>>,
    h: HeaderMap,
    axum::Json(v): axum::Json<Value>,
) -> Response {
    let Some(p) = principal(&h, &a) else {
        return unauthorized();
    };
    let Some(id) = q.get("session_id") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let (handle, tx) = {
        let mut r = a.registry.lock().await;
        let Some(e) = r.sessions.get_mut(id).filter(|e| e.owner == p.id) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        e.touched = Instant::now();
        (e.handle.clone(), e.legacy.clone())
    };
    let Some(tx) = tx else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if tx.is_closed() {
        return StatusCode::GONE.into_response();
    }
    if let Some(reply) = handle.rpc(v).await {
        if tx
            .try_send(Ok(axum::response::sse::Event::default()
                .event("message")
                .data(reply.to_string())))
            .is_err()
        {
            handle.close().await;
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    }
    StatusCode::ACCEPTED.into_response()
}
async fn status(State(a): State<App>, h: HeaderMap) -> Response {
    let Some(p) = principal(&h, &a) else {
        return unauthorized();
    };
    let r = a.registry.lock().await;
    response(
        StatusCode::OK,
        json!({"principal":p.id,"admin":p.admin,"uptime_s":a.started.elapsed().as_secs(),"owned_mcp_sessions":r.sessions.values().filter(|e|e.owner==p.id).count(),"cli_session":r.cli.contains_key(&p.id),"max_sessions":p.max_sessions,"pending_session_queues":r.sessions.values().filter(|e|e.owner==p.id&&e.handle.active()).count(),"request_timeout_ms":a.timeout.as_millis()}),
    )
}
async fn shutdown(State(a): State<App>, h: HeaderMap) -> Response {
    let Some(p) = principal(&h, &a) else {
        return unauthorized();
    };
    if !p.admin {
        return StatusCode::FORBIDDEN.into_response();
    }
    a.shutdown.notify_one();
    response(StatusCode::ACCEPTED, json!({"status":"stopping"}))
}
pub async fn serve(host: &str, port: u16, endpoint: String, token: String) -> anyhow::Result<()> {
    let file = std::env::var_os("DELTA_AUTH_FILE").map(std::path::PathBuf::from);
    let auth = Auth::load(
        if token.is_empty() { None } else { Some(token) },
        file.as_deref(),
    )?;
    let timeout = std::env::var("DELTA_REQUEST_TIMEOUT_MS")
        .ok()
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(120000);
    if !(100..=900000).contains(&timeout) {
        anyhow::bail!("Request timeout must be 100..900000 ms");
    }
    let a = App {
        endpoint,
        auth,
        registry: Default::default(),
        shutdown: Arc::new(tokio::sync::Notify::new()),
        started: Instant::now(),
        timeout: Duration::from_millis(timeout),
    };
    let clean = a.clone();
    let sweeper = tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(30));
        loop {
            timer.tick().await;
            let mut r = clean.registry.lock().await;
            r.sessions.retain(|_, e| {
                (!e.handle.is_closed()
                    && e.touched.elapsed() < Duration::from_secs(3600)
                    && e.legacy.as_ref().is_none_or(|tx| !tx.is_closed()))
                    || e.handle.active()
            });
        }
    });
    let router=Router::new().route("/health",get(||async{axum::Json(json!({"service":"deltabrowser","status":"alive","version":env!("CARGO_PKG_VERSION"),"browser_readiness":"lazy; not asserted by this endpoint"}))})).route("/whoami",get(status)).route("/mcp",post(post_mcp).delete(delete_mcp)).route("/sse",get(legacy_sse)).route("/messages",post(legacy_message)).route("/cli",post(cli)).route("/admin/shutdown",post(shutdown)).layer(DefaultBodyLimit::max(256*1024)).with_state(a.clone());
    let listener = tokio::net::TcpListener::bind((host, port)).await?;
    eprintln!("Delta HTTP listening on {host}:{port}; principal-bound sessions enabled");
    let closing = a.clone();
    let result = axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=closing.shutdown.notified()=>{}}
            let handles = {
                let mut r = closing.registry.lock().await;
                for e in r.sessions.values_mut() {
                    e.legacy.take();
                }
                r.sessions
                    .values()
                    .map(|e| e.handle.clone())
                    .chain(r.cli.values().cloned())
                    .collect::<Vec<_>>()
            };
            for h in handles {
                h.close().await;
            }
        })
        .await;
    sweeper.abort();
    result?;
    Ok(())
}
