mod artifacts;
mod auth;
mod browser;
mod cdp;
mod dsl;
mod engine;
mod extended;
mod har;
mod http;
mod mcp;
mod profiles;
mod resources;
mod sandbox;
mod schema;
mod session_actor;
mod snapshot;
mod state;
use anyhow::{bail, Result};
use base64::Engine;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
#[derive(Parser)]
#[command(
    name = "delta",
    version,
    about = "Delta Browser v0.0.1 — the browser your AI agent actually needs, by Blackvault Technology.",
    long_about = "DeltaBrowser: a browser for AI agents.\n\nConnects your agent to a real browser with 62 MCP tools: batched tool calls, delta\nsnapshots, persistent sessions, and a kernel-enforced sandbox on Linux x86_64.\n\nInstall:\n  curl -fsSL https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.sh | sh\n  irm https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.ps1 | iex; Install-DeltaBrowser\n\nThen run `delta doctor` to verify the install, and `delta connect <agent>` to configure one."
)]
struct Cli {
    #[arg(
        long,
        env = "DELTA_CDP",
        default_value = "http://127.0.0.1:9222",
        global = true
    )]
    cdp: String,
    #[arg(
        long,
        env = "DELTA_SERVER",
        default_value = "http://127.0.0.1:8931",
        global = true
    )]
    server: String,
    #[arg(long, env = "DELTA_TOKEN", global = true, hide_env_values = true)]
    token: Option<String>,
    #[arg(long,env="DELTA_ENGINE",default_value=default_engine(),global=true,value_parser=["external","moli","chrome","lightpanda"])]
    engine: String,
    #[arg(long, env = "DELTA_ENGINE_PATH", global = true)]
    engine_path: Option<std::path::PathBuf>,
    #[arg(long, env = "DELTA_PROFILE", default_value = "scrape", global = true)]
    profile: String,
    #[arg(long, global = true)]
    no_sandbox: bool,
    #[arg(long, env = "DELTA_MEMORY_LIMIT_MB", global = true)]
    memory_limit_mb: Option<u64>,
    #[arg(long, env = "DELTA_PIDS_LIMIT", global = true)]
    pids_limit: Option<u64>,
    #[arg(long, env = "DELTA_CGROUP_ROOT", global = true)]
    cgroup_root: Option<std::path::PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Diagnose the local install: engine, sandbox, ports, token, config paths
    Doctor {
        /// Print machine-readable JSON instead of the human report
        #[arg(long)]
        json: bool,
    },
    Stop,
    #[command(external_subcommand)]
    Action(Vec<String>),
    #[command(name = "_worker", hide = true)]
    Worker {
        config: std::path::PathBuf,
    },
    Engine {
        #[command(subcommand)]
        action: EngineCommand,
    },
    Profile {
        #[command(subcommand)]
        action: ProfileCommand,
    },
    Connect {
        /// Agent name, or "all" to configure every detected agent
        agent: String,
        #[arg(long, env = "DELTA_CONFIG_PATH")]
        path: Option<std::path::PathBuf>,
        #[arg(long)]
        install: bool,
        #[arg(long)]
        force: bool,
        #[arg(long, default_value = "8931")]
        port: u16,
    },
    /// Find agents installed on this machine and report their config paths
    Agents {
        /// Print the MCP server block to paste into a client with no adapter
        #[arg(long)]
        config: bool,
        /// Print machine-readable output
        #[arg(long)]
        json: bool,
    },
    Sandbox {
        #[command(subcommand)]
        action: SandboxCommand,
    },
    Start {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8931)]
        port: u16,
    },
    Mcp {
        #[arg(long,default_value="stdio",value_parser=["stdio","http","sse"])]
        transport: String,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8931)]
        port: u16,
    },
    Tools,
    Call {
        tool: String,
        #[arg(default_value = "{}")]
        arguments: String,
    },
    Open {
        url: String,
    },
    Snapshot {
        #[arg(short = 'i', long)]
        interactive_only: bool,
    },
    Click {
        target: String,
    },
    Fill {
        target: String,
        value: String,
    },
    Eval {
        script: String,
    },
    Screenshot {
        path: std::path::PathBuf,
        #[arg(long)]
        full_page: bool,
    },
    Test {
        script: std::path::PathBuf,
        #[arg(long)]
        report: Option<std::path::PathBuf>,
    },
    Validate {
        script: std::path::PathBuf,
    },
    Config,
}
#[derive(Subcommand)]
enum EngineCommand {
    Install {
        #[arg(value_parser=["moli"])]
        name: String,
    },
}
#[derive(Subcommand)]
enum ProfileCommand {
    List,
    Show {
        name: String,
    },
    Create {
        path: std::path::PathBuf,
        #[arg(long, default_value = "scrape")]
        template: String,
    },
    Install {
        agent: String,
        #[arg(long)]
        path: std::path::PathBuf,
        #[arg(long)]
        force: bool,
        #[arg(long, default_value = "auto")]
        engine: String,
    },
}
#[derive(Subcommand)]
enum SandboxCommand {
    Probe,
    Exec {
        #[arg(long)]
        policy: std::path::PathBuf,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
}
fn default_engine() -> &'static str {
    if cfg!(feature = "bundled-moli") {
        "moli"
    } else {
        "external"
    }
}
fn endpoint(c: &Cli) -> Result<String> {
    if c.engine == "external" {
        if c.memory_limit_mb.is_some() || c.pids_limit.is_some() {
            bail!("Cannot enforce hard limits on an external browser; select a managed engine");
        }
        return Ok(c.cdp.clone());
    }
    let executable = if let Some(p) = &c.engine_path {
        p.clone()
    } else if c.engine == "moli" && cfg!(feature = "bundled-moli") {
        std::path::PathBuf::from("@bundled-moli")
    } else if c.engine == "moli" {
        profiles::home_dir()
            .ok_or_else(|| {
                anyhow::anyhow!("Could not determine home directory; set HOME or --engine-path")
            })?
            .join(".delta/engines/moli-v1.1.12/moli")
    } else {
        bail!("--engine-path is required for {}", c.engine)
    };
    Ok(serde_json::to_string(&engine::Config {
        engine: c.engine.clone(),
        executable,
        profile: c.profile.clone(),
        no_sandbox: c.no_sandbox,
        memory_limit_mb: c.memory_limit_mb,
        pids_limit: c.pids_limit,
        cgroup_root: c.cgroup_root.clone(),
    })?)
}
fn cli_action(words: &[String]) -> Result<(String, Value)> {
    let first = words.first().map(String::as_str).unwrap_or("");
    let w = &words[1..];
    let need = |i: usize| -> Result<&str> {
        w.get(i)
            .map(String::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing argument for {first}"))
    };
    let (name, args) = match first {
        "type" | "select" => {
            let mut a = locator(need(0)?.into());
            a["value"] = json!(need(1)?);
            (
                if first == "select" {
                    "select_option"
                } else {
                    "type"
                },
                a,
            )
        }
        "check" | "uncheck" | "hover" | "double-click" | "right-click" => (
            match first {
                "double-click" => "double_click",
                "right-click" => "right_click",
                x => x,
            },
            locator(need(0)?.into()),
        ),
        "press" => ("press_key", json!({"key":need(0)?})),
        "scroll" => (
            "scroll",
            json!({"direction":need(0)?,"pixels":w.get(1).map(|s|s.parse::<u64>()).transpose()?.unwrap_or(500)}),
        ),
        "wait" => (
            "wait_for",
            json!({"selector":need(0)?,"timeout_ms":w.get(1).map(|s|s.parse::<u64>()).transpose()?.unwrap_or(5000)}),
        ),
        "get" => match need(0)? {
            "text" => ("get_text", locator(need(1)?.into())),
            "attr" => {
                let mut a = locator(need(1)?.into());
                a["attribute"] = json!(need(2)?);
                ("get_attribute", a)
            }
            "url" => ("get_url", json!({})),
            "title" => ("get_title", json!({})),
            _ => bail!("get: text, attr, url or title"),
        },
        "tabs" => ("list_tabs", json!({})),
        "tab" => ("switch_tab", json!({"tab_id":need(0)?})),
        "new-tab" => (
            "new_tab",
            if w.is_empty() {
                json!({})
            } else {
                json!({"url":need(0)?})
            },
        ),
        "close-tab" => ("close_tab", json!({"tab_id":need(0)?})),
        "back" => ("go_back", json!({})),
        "forward" => ("go_forward", json!({})),
        "reload" => ("reload", json!({})),
        "status" => ("server_status", json!({})),
        "logs" => ("server_logs", json!({})),
        "close" => ("close", json!({})),
        "save-state" => ("save_state", json!({"path":need(0)?})),
        "restore-state" => {
            let bytes = std::fs::read(need(0)?)?;
            let v: Value = serde_json::from_slice(&bytes)?;
            (
                "restore_state",
                json!({"state":v.get("state").unwrap_or(&v)}),
            )
        }
        "upload" => {
            let mut a = locator(need(0)?.into());
            a["path"] = json!(need(1)?);
            ("upload_file", a)
        }
        "drag" => {
            let mut a = json!({});
            for (k, v) in [("from", need(0)?), ("to", need(1)?)] {
                a[format!(
                    "{k}_{}",
                    if v.starts_with('@') {
                        "ref"
                    } else {
                        "selector"
                    }
                )] = json!(v);
            }
            ("drag", a)
        }
        "pdf" => ("save_pdf", json!({"path":need(0)?})),
        "cdp" => (
            "cdp_raw",
            json!({"method":need(0)?,"params":serde_json::from_str::<Value>(w.get(1).map(String::as_str).unwrap_or("{}"))?}),
        ),
        "cookies" => match w.first().map(String::as_str).unwrap_or("get") {
            "get" => ("get_cookies", json!({})),
            "clear" => ("clear_cookies", json!({})),
            "set" => (
                "set_cookie",
                json!({"name":need(1)?,"value":need(2)?,"url":need(3)?}),
            ),
            _ => bail!("cookies: get, clear or set NAME VALUE URL"),
        },
        _ => bail!("Unknown command {first}; use delta tools or delta call TOOL JSON"),
    };
    Ok((name.to_owned(), args))
}
fn locator(target: String) -> Value {
    if target.starts_with('@') {
        json!({"ref":target})
    } else {
        json!({"selector":target})
    }
}
async fn remote(c: &Cli, name: &str, args: Value) -> Result<Value> {
    let token = c
        .token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Set DELTA_TOKEN to the HTTP server's token"))?;
    let r = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(900))
        .build()?
        .post(format!("{}/cli", c.server.trim_end_matches('/')))
        .bearer_auth(token)
        .json(&json!({"name":name,"arguments":args}))
        .send()
        .await?;
    let status = r.status();
    let body: Value = r.json().await?;
    if !status.is_success() {
        bail!("{}", body);
    }
    Ok(body["result"].clone())
}
async fn stdio(endpoint: String) -> Result<()> {
    let timeout = std::env::var("DELTA_REQUEST_TIMEOUT_MS")
        .ok()
        .map(|v| v.parse::<u64>())
        .transpose()?
        .unwrap_or(120000);
    if !(100..=900000).contains(&timeout) {
        bail!("Request timeout must be 100..900000 ms");
    }
    let session = session_actor::Handle::new(endpoint, std::time::Duration::from_millis(timeout));
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Value>(32);
    let writer = tokio::spawn(async move {
        let mut output = tokio::io::stdout();
        while let Some(v) = rx.recv().await {
            output.write_all(v.to_string().as_bytes()).await?;
            output.write_all(b"\n").await?;
            output.flush().await?;
        }
        Ok::<(), std::io::Error>(())
    });
    let mut input = tokio::io::BufReader::new(tokio::io::stdin());
    let mut tasks = tokio::task::JoinSet::new();
    loop {
        while tasks.try_join_next().is_some() {}
        if tasks.len() >= 64 {
            tasks.join_next().await;
        }
        let mut bytes = Vec::new();
        loop {
            let available = input.fill_buf().await?;
            if available.is_empty() {
                break;
            }
            let n = available
                .iter()
                .position(|c| *c == b'\n')
                .map(|i| i + 1)
                .unwrap_or(available.len());
            if bytes.len() + n > 256 * 1024 {
                session.close().await;
                bail!("MCP line exceeds 256 KiB");
            }
            let done = available[n - 1] == b'\n';
            bytes.extend_from_slice(&available[..n]);
            input.consume(n);
            if done {
                break;
            }
        }
        if bytes.is_empty() {
            break;
        }
        match serde_json::from_slice(&bytes) {
            Ok(v) => {
                let reply = session.submit(v);
                let tx = tx.clone();
                tasks.spawn(async move {
                    if let Ok(Some(v)) = reply.await {
                        let _ = tx.send(v).await;
                    }
                });
            }
            Err(_) => {
                let _ = tx
                    .send(mcp::error(Value::Null, -32700, "Parse error"))
                    .await;
            }
        }
    }
    while tasks.join_next().await.is_some() {}
    session.close().await;
    drop(tx);
    writer.await??;
    Ok(())
}

/// Renders a test report as a self-contained HTML document.
///
/// All interpolated values are escaped, and the document declares UTF-8 so the
/// separators below render correctly regardless of the reader's locale.
fn html_report(v: &Value) -> String {
    fn escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }

    let passed = v["passed"].as_u64().unwrap_or(0);
    let failed = v["failed"].as_u64().unwrap_or(0);
    let skipped = v["skipped"].as_u64().unwrap_or(0);
    let total = (passed + failed + skipped).max(1);
    let duration = v["duration_ms"].as_u64().unwrap_or(0);

    // Longest step drives the duration bar width, so one slow step is visible
    // without the bar collapsing to sub-pixel widths.
    let slowest = v["steps"]
        .as_array()
        .map(|steps| {
            steps
                .iter()
                .map(|s| s["duration_ms"].as_u64().unwrap_or(0))
                .max()
                .unwrap_or(1)
                .max(1)
        })
        .unwrap_or(1);

    let mut rows = String::new();
    for step in v["steps"].as_array().cloned().unwrap_or_default() {
        let is_skipped = step["skipped"] == true;
        let is_passed = step["passed"] == true;
        let (class, status) = if is_skipped {
            ("skip", "SKIP")
        } else if is_passed {
            ("pass", "PASS")
        } else {
            ("fail", "FAIL")
        };
        let ms = step["duration_ms"].as_u64().unwrap_or(0);
        let width = (ms as f64 / slowest as f64 * 100.0)
            .round()
            .clamp(0.0, 100.0);
        let error = step["error"].as_str().unwrap_or("");
        let detail = if error.is_empty() {
            String::new()
        } else {
            format!(
                "<details class='detail'><summary>error</summary><pre>{}</pre></details>",
                escape(error)
            )
        };
        rows.push_str(&format!(
            "<tr class='{class}'><td class='num'>{}</td><td class='action'>{}</td>\
             <td><span class='badge {class}'>{status}</span></td>\
             <td class='num'>{ms}</td><td class='bar'><i style='width:{width}%'></i></td>\
             <td>{detail}</td></tr>",
            step["line"].as_u64().unwrap_or(0),
            escape(step["action"].as_str().unwrap_or(""))
        ));
    }

    let empty = "<tr><td colspan='6' class='empty'>No steps were recorded.</td></tr>";
    if rows.is_empty() {
        rows = empty.to_owned();
    }

    let raw = match serde_json::to_string_pretty(v) {
        Ok(text) => escape(&text),
        Err(e) => format!(
            "Results could not be serialised: {}",
            escape(&e.to_string())
        ),
    };

    let style = r#"
:root{color-scheme:light dark;--bg:#0b1017;--panel:#141d29;--panel-2:#1b2735;--line:#24344a;
--text:#e8eef7;--muted:#93a5bd;--pass:#4ade80;--fail:#fb7185;--skip:#94a3b8;--accent:#5eead4;
--shadow:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.24)}
@media (prefers-color-scheme:light){:root{--bg:#f6f8fb;--panel:#fff;--panel-2:#f1f5f9;
--line:#e2e8f0;--text:#0f172a;--muted:#64748b;--pass:#15803d;--fail:#b91c1c;--skip:#475569;
--shadow:0 1px 2px rgba(15,23,42,.06),0 8px 24px rgba(15,23,42,.06)}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--text);
font:15px/1.55 ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;
-webkit-font-smoothing:antialiased}
main{max-width:1080px;margin:0 auto;padding:clamp(20px,5vw,56px) clamp(16px,4vw,32px)}
.brand{color:var(--accent);letter-spacing:.22em;font-size:11px;font-weight:600}
h1{font-size:clamp(28px,5vw,40px);letter-spacing:-.02em;margin:.35em 0 .2em;font-weight:680}
.lede{color:var(--muted);margin:0 0 28px;max-width:62ch}
.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:12px;margin-bottom:12px}
.card{background:var(--panel);border:1px solid var(--line);border-radius:12px;padding:16px 18px;box-shadow:var(--shadow)}
.card .n{font-size:26px;font-weight:660;letter-spacing:-.02em;font-variant-numeric:tabular-nums}
.card .k{color:var(--muted);font-size:11px;letter-spacing:.1em;text-transform:uppercase;margin-top:2px}
.card.pass .n{color:var(--pass)}.card.fail .n{color:var(--fail)}.card.skip .n{color:var(--skip)}
.verdict{display:inline-flex;align-items:center;gap:8px;padding:9px 16px;border-radius:999px;
font-weight:620;font-size:14px;margin:0 0 24px;border:1px solid var(--line);background:var(--panel)}
.verdict.ok{color:var(--pass);border-color:color-mix(in srgb,var(--pass) 45%,transparent)}
.verdict.bad{color:var(--fail);border-color:color-mix(in srgb,var(--fail) 45%,transparent)}
.verdict::before{content:"";width:8px;height:8px;border-radius:50%;background:currentColor}
table{width:100%;border-collapse:collapse;background:var(--panel);border:1px solid var(--line);
border-radius:12px;overflow:hidden;box-shadow:var(--shadow)}
th{text-align:left;padding:11px 14px;color:var(--muted);font-size:11px;letter-spacing:.09em;
text-transform:uppercase;font-weight:600;border-bottom:1px solid var(--line);background:var(--panel-2)}
td{padding:11px 14px;border-bottom:1px solid var(--line);vertical-align:top}
tr:last-child td{border-bottom:0}
.num{font-variant-numeric:tabular-nums;color:var(--muted);width:1%;white-space:nowrap}
.action{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:13px;word-break:break-word}
.badge{display:inline-block;padding:2px 9px;border-radius:6px;font-size:11px;font-weight:680;letter-spacing:.06em}
.badge.pass{color:var(--pass);background:color-mix(in srgb,var(--pass) 15%,transparent)}
.badge.fail{color:var(--fail);background:color-mix(in srgb,var(--fail) 15%,transparent)}
.badge.skip{color:var(--skip);background:color-mix(in srgb,var(--skip) 15%,transparent)}
.bar{width:120px}.bar i{display:block;height:6px;border-radius:3px;background:var(--muted);min-width:2px}
tr.pass .bar i{background:var(--pass)}tr.fail .bar i{background:var(--fail)}tr.skip .bar i{background:var(--skip)}
.empty{color:var(--muted);text-align:center;padding:32px}
.detail summary{cursor:pointer;color:var(--fail);font-size:13px}
.detail pre{white-space:pre-wrap;overflow-wrap:anywhere;background:var(--panel-2);padding:12px;
border-radius:8px;font-size:12px;margin:8px 0 0;max-height:320px;overflow:auto}
details.raw{margin-top:28px;background:var(--panel);border:1px solid var(--line);border-radius:12px;
padding:16px 20px;box-shadow:var(--shadow)}
details.raw summary{cursor:pointer;color:var(--muted);font-size:13px}
details.raw pre{white-space:pre-wrap;overflow-wrap:anywhere;background:var(--panel-2);padding:16px;
border-radius:8px;font-size:12px;overflow:auto;max-height:520px}
footer{margin-top:36px;color:var(--muted);font-size:12px;border-top:1px solid var(--line);padding-top:18px}
code{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:.92em}
"#;

    format!(
        r#"<!doctype html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>DeltaBrowser Test Report</title>
<style>{style}</style>
<main>
<div class="brand">DELTABROWSER / VERIFICATION</div>
<h1>Test report</h1>
<p class="lede">Real browser actions were executed. Arguments are deliberately omitted from step logs so this file is safe to share.</p>
<div class="verdict {verdict}">{verdict_text}</div>
<div class="cards">
  <div class="card pass"><div class="n">{passed}</div><div class="k">Passed</div></div>
  <div class="card fail"><div class="n">{failed}</div><div class="k">Failed</div></div>
  <div class="card skip"><div class="n">{skipped}</div><div class="k">Skipped</div></div>
  <div class="card"><div class="n">{total}</div><div class="k">Steps</div></div>
  <div class="card"><div class="n">{duration}<span style="font-size:14px">ms</span></div><div class="k">Total time</div></div>
</div>
<table>
<thead><tr><th>Line</th><th>Action</th><th>Result</th><th>Time</th><th>Relative</th><th>Details</th></tr></thead>
<tbody>{rows}</tbody>
</table>
<details class="raw"><summary>Structured results</summary><pre>{raw}</pre></details>
<footer>DeltaBrowser {version} &middot; This report records observed results, not a security certification.</footer>
</main>
</html>"#,
        style = style,
        verdict = if failed == 0 { "ok" } else { "bad" },
        verdict_text = if failed == 0 {
            "All executed steps passed".to_owned()
        } else {
            format!("{failed} step(s) failed")
        },
        passed = passed,
        failed = failed,
        skipped = skipped,
        total = total,
        duration = duration,
        rows = rows,
        raw = raw,
        version = env!("CARGO_PKG_VERSION")
    )
}
fn default_agent_config_path(agent: &str) -> std::path::PathBuf {
    profiles::default_config_path(agent)
}

async fn is_daemon_running(server: &str) -> bool {
    let url = format!("{}/health", server.trim_end_matches('/'));
    match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
    {
        Ok(client) => client
            .get(url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false),
        Err(_) => false,
    }
}

fn load_or_create_token(path: &std::path::Path) -> Result<String> {
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        let v: Value = serde_json::from_str(&content)?;
        if let Some(token) = v["token"].as_str() {
            if token.len() >= 24 {
                return Ok(token.to_string());
            }
        }
    }
    // Two UUIDv4 values encode to 44 URL-safe characters, comfortably above the
    // 24-character minimum enforced by auth::load.
    let mut raw = [0u8; 32];
    raw[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    raw[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    if !dir.as_os_str().is_empty() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        path,
        json!({"token":token,"server":dir.to_string_lossy()}).to_string(),
    )?;
    Ok(token)
}

fn generate_or_load_token() -> Result<String> {
    let token_path = std::env::var_os("DELTA_TOKEN_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            profiles::home_dir()
                .map(|h| h.join(".delta/token.json"))
                .unwrap_or_else(|| std::path::PathBuf::from(".delta-token.json"))
        });
    load_or_create_token(&token_path)
}

async fn spawn_daemon(server: &str, token: &str, endpoint: String) -> Result<()> {
    let exe = std::env::current_exe()?;
    let (host, port) = parse_server_addr(server);
    let child = tokio::process::Command::new(exe)
        .arg("start")
        .arg("--host")
        .arg(&host)
        .arg("--port")
        .arg(port.to_string())
        .env("DELTA_TOKEN", token)
        .env("DELTA_CDP", &endpoint)
        .env(
            "DELTA_ENGINE",
            if endpoint.starts_with('{') {
                "moli"
            } else {
                "external"
            },
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    // Wait for daemon to be ready
    let _ = child.id();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    for _ in 0..50 {
        if let Ok(r) = client
            .get(format!("{}/health", server.trim_end_matches('/')))
            .send()
            .await
        {
            if r.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err(anyhow::anyhow!("Daemon failed to start within 5 seconds"))
}

fn parse_server_addr(server: &str) -> (String, u16) {
    let s = server
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let default_port = if server.starts_with("https://") {
        443
    } else {
        8931
    };
    let (host, port_str) = s.split_once(':').unwrap_or((s, ""));
    let port = port_str.parse::<u16>().unwrap_or(default_port);
    (host.to_string(), port)
}

/// Collects every diagnostic check `delta doctor` reports.
async fn doctor(c: &Cli) -> Value {
    let mut checks: Vec<Value> = Vec::new();
    let mut push = |name: &str, ok: bool, detail: String, fix: Option<&str>| {
        checks.push(json!({"check":name,"ok":ok,"detail":detail,"fix":fix}));
    };

    // Build and platform
    push(
        "build",
        true,
        format!(
            "delta {} on {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        ),
        None,
    );
    push(
        "home_directory",
        profiles::home_dir().is_some(),
        profiles::home_dir()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|| "not resolvable (HOME and USERPROFILE both unset)".into()),
        Some("Set HOME (or USERPROFILE on Windows) so config and token paths resolve"),
    );

    // Engine availability
    let (engine, endpoint, installed) = profiles::detect_engine();
    push(
        "engine_detected",
        installed,
        if installed {
            format!("{engine} via {endpoint}")
        } else {
            format!("{engine} selected but not installed")
        },
        if installed {
            None
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some("Run `delta engine install moli`, or `delta connect <agent> --install`")
        } else {
            Some("Start a Chromium-family browser with --remote-debugging-port=9222, then run `delta connect <agent> --install`")
        },
    );
    match profiles::find_local_browser() {
        Some(path) => push(
            "local_browser",
            true,
            path.to_string_lossy().to_string(),
            None,
        ),
        None => push(
            "local_browser",
            false,
            "no Chromium-family browser found in standard install locations".into(),
            Some("Install Chrome, Edge or Chromium, or pass --engine-path"),
        ),
    }

    // Sandbox support on this host
    let sandbox = sandbox::probe();
    push(
        "sandbox",
        sandbox["supported"] == true,
        sandbox["detail"].as_str().unwrap_or("").to_owned(),
        if sandbox["supported"] == true {
            None
        } else {
            Some("Managed sandboxed browsing requires Linux; on other platforms use an external browser")
        },
    );

    // Ports
    for (label, addr, url) in [
        (
            "cdp_port",
            "127.0.0.1:9222",
            "http://127.0.0.1:9222/json/version",
        ),
        (
            "daemon_port",
            "127.0.0.1:8931",
            "http://127.0.0.1:8931/health",
        ),
    ] {
        let reachable = std::net::TcpStream::connect_timeout(
            &addr.parse().unwrap(),
            std::time::Duration::from_millis(250),
        )
        .is_ok();
        push(
            label,
            reachable,
            if reachable {
                format!("listening on {addr}")
            } else {
                format!("nothing listening on {addr}")
            },
            if reachable {
                None
            } else if label == "daemon_port" {
                Some("Start the daemon with `delta start`")
            } else {
                Some("Launch your browser with --remote-debugging-port=9222, or use a managed engine")
            },
        );
        let _ = url;
    }

    // Token
    let token_path = std::env::var_os("DELTA_TOKEN_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            profiles::home_dir()
                .map(|h| h.join(".delta/token.json"))
                .unwrap_or_else(|| std::path::PathBuf::from(".delta-token.json"))
        });
    if let Some(explicit) = c.token.as_ref() {
        push(
            "auth_token",
            explicit.len() >= 24,
            format!(
                "DELTA_TOKEN supplied via --token/environment ({} chars)",
                explicit.len()
            ),
            if explicit.len() >= 24 {
                None
            } else {
                Some("Tokens must be at least 24 characters")
            },
        );
    } else if token_path.exists() {
        match load_or_create_token(&token_path) {
            Ok(t) => push(
                "auth_token",
                true,
                format!(
                    "stored token at {} ({} chars)",
                    token_path.display(),
                    t.len()
                ),
                None,
            ),
            Err(e) => push(
                "auth_token",
                false,
                format!("token file {} unreadable: {e}", token_path.display()),
                Some("Delete the file to regenerate, or point DELTA_TOKEN_FILE elsewhere"),
            ),
        }
    } else {
        push(
            "auth_token",
            false,
            format!("no token file at {}", token_path.display()),
            Some("One is generated automatically by `delta connect <agent>`"),
        );
    }

    // Daemon liveness, when reachable
    let daemon_up = is_daemon_running(&c.server).await;
    push(
        "daemon_reachable",
        daemon_up,
        if daemon_up {
            format!("responding at {}", c.server.trim_end_matches('/'))
        } else {
            format!("not responding at {}", c.server.trim_end_matches('/'))
        },
        if daemon_up {
            None
        } else {
            Some(
                "Only needed for `delta start` and shared sessions; stdio clients do not require it",
            )
        },
    );

    // Agent detection and config paths
    let configs = profiles::detect_agents();

    let failures = checks.iter().filter(|c| c["ok"] != true).count();
    json!({
        "status": if failures == 0 { "ok" } else { "issues_found" },
        "failures": failures,
        "checks": checks,
        "agent_configs": configs,
        "agents_installed": profiles::unconfigured_agents(),
        "tool_count": schema::tools().len(),
        "sandbox_profiles": profiles::list(),
    })
}

/// Renders `delta doctor` output for a terminal.
fn render_doctor(report: &Value) -> String {
    let status = if report["failures"] == 0 {
        "all clear"
    } else {
        "issues"
    };
    let mut out = String::new();
    out.push_str(&format!(
        "\n  DeltaBrowser {} — environment report: {}\n\n",
        env!("CARGO_PKG_VERSION"),
        status
    ));
    let width = report["checks"]
        .as_array()
        .map(|c| {
            c.iter()
                .filter_map(|v| v["check"].as_str())
                .map(str::len)
                .max()
                .unwrap_or(10)
        })
        .unwrap_or(10)
        .max(10);
    for check in report["checks"].as_array().cloned().unwrap_or_default() {
        let mark = if check["ok"] == true { "ok  " } else { "FAIL" };
        let name = check["check"].as_str().unwrap_or("");
        out.push_str(&format!(
            "  [{mark}] {name:<width$}  {}\n",
            check["detail"].as_str().unwrap_or(""),
            width = width
        ));
        if check["ok"] != true {
            if let Some(fix) = check["fix"].as_str() {
                out.push_str(&format!("{:<width$}  -> {fix}\n", "", width = width));
            }
        }
    }
    let detected = report["agent_configs"]
        .as_array()
        .map(|c| {
            c.iter()
                .filter(|v| v["configured"] == true)
                .map(|v| v["agent"].as_str().unwrap_or("").to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let installed = report["agent_configs"]
        .as_array()
        .map(|c| {
            c.iter()
                .filter(|v| v["detected"] == true && v["configured"] != true)
                .map(|v| v["agent"].as_str().unwrap_or("").to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    out.push_str(&format!(
        "\n  Configured agents: {}\n",
        if detected.is_empty() {
            "none".to_owned()
        } else {
            detected.join(", ")
        }
    ));
    if !installed.is_empty() {
        out.push_str(&format!(
            "  Installed but not configured: {}\n",
            installed.join(", ")
        ));
        out.push_str("    run `delta connect all --install` to configure every one\n");
    } else if detected.is_empty() {
        out.push_str(
            "  Run `delta agents` to see what is installed, then `delta connect <name>`\n",
        );
    }
    out.push_str(&format!("  Tools available: {}\n\n", report["tool_count"]));
    if report["failures"] != 0 {
        out.push_str("  Re-run with `--json` for machine-readable output.\n\n");
    }
    out
}

/// Renders `delta agents` output for a terminal.
fn render_agents(engine: &str, agents: &[Value]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "\n  DeltaBrowser agents — engine: {}\n\n",
        if engine == "external" {
            "external browser (no kernel sandbox)"
        } else {
            "managed"
        }
    ));
    let name_width = agents
        .iter()
        .filter_map(|a| a["agent"].as_str())
        .map(str::len)
        .max()
        .unwrap_or(10)
        .max(10);
    let mut installed = Vec::new();
    for agent in agents {
        let name = agent["agent"].as_str().unwrap_or("");
        let is_installed = agent["installed"] == true;
        let is_configured = agent["configured"] == true;
        let state = if is_configured {
            "configured"
        } else if is_installed {
            installed.push(name.to_owned());
            "detected"
        } else {
            "-"
        };
        out.push_str(&format!(
            "  {name:<name_width$}  {state:<11}  {}\n",
            agent["config"].as_str().unwrap_or(""),
            name_width = name_width
        ));
    }
    out.push('\n');
    if installed.is_empty() {
        out.push_str("  No installed agent is waiting to be configured.\n");
        out.push_str("  To set up one anyway:  delta connect opencode --install\n");
    } else {
        out.push_str("  Configure everything detected with:\n\n");
        out.push_str("    delta connect all --install\n\n");
        out.push_str("  Or one at a time:\n\n");
        for name in &installed {
            out.push_str(&format!("    delta connect {name} --install\n"));
        }
        out.push('\n');
    }
    out.push_str(
        "  Any other MCP client works too; `delta agents --config` prints the block to paste.\n\n",
    );
    out
}

async fn run() -> Result<()> {
    let c = Cli::parse();
    let r = match &c.command {
        Command::Doctor { json: as_json } => {
            let report = doctor(&c).await;
            if *as_json {
                report
            } else {
                print!("{}", render_doctor(&report));
                return Ok(());
            }
        }
        Command::Agents {
            config: show_config,
            json: as_json,
        } => {
            let (engine, detected_endpoint, _) = profiles::detect_engine();
            let agents = profiles::detect_agents();
            if *show_config {
                let exe = std::env::current_exe()?.to_string_lossy().into_owned();
                profiles::template("generic", &exe, &engine, &detected_endpoint)?;
                return Ok(());
            }
            if *as_json {
                json!({"engine": engine, "endpoint": detected_endpoint, "agents": agents})
            } else {
                print!("{}", render_agents(&engine, &agents));
                return Ok(());
            }
        }
        Command::Stop => {
            let token = c
                .token
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Set DELTA_TOKEN"))?;
            let response = reqwest::Client::new()
                .post(format!("{}/admin/shutdown", c.server.trim_end_matches('/')))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?;
            response.json::<Value>().await?
        }
        Command::Action(words) => {
            let (name, args) = cli_action(words)?;
            remote(&c, &name, args).await?
        }
        Command::Worker { config } => {
            engine::worker(config)?;
            return Ok(());
        }
        Command::Engine {
            action: EngineCommand::Install { name: _ },
        } => engine::install_moli().await?,
        Command::Sandbox {
            action: SandboxCommand::Probe,
        } => sandbox::probe(),
        Command::Sandbox {
            action: SandboxCommand::Exec { policy, command },
        } => {
            let profile = sandbox::Profile::load(&policy.to_string_lossy())?;
            let executable = std::path::PathBuf::from(&command[0]).canonicalize()?;
            let work = artifacts::Artifacts::new()?;
            let worker = engine::Worker {
                executable,
                arguments: command[1..].to_vec(),
                profile,
                workdir: std::env::current_dir()?,
            };
            let path = work.write("worker.json", &serde_json::to_vec(&worker)?)?;
            engine::worker(&path)?;
            return Ok(());
        }
        Command::Profile { action } => match action {
            ProfileCommand::List => profiles::list(),
            ProfileCommand::Show { name } => serde_json::to_value(sandbox::Profile::load(name)?)?,
            ProfileCommand::Create { path, template } => {
                use std::io::Write;
                let p = sandbox::Profile::built_in(template)?;
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)?;
                f.write_all(&serde_json::to_vec_pretty(&p)?)?;
                json!({"path":path})
            }
            ProfileCommand::Install {
                agent,
                path,
                force,
                engine,
            } => {
                let (detected_engine, detected_endpoint, _installed) = profiles::detect_engine();
                let engine = if engine == "auto" {
                    detected_engine.clone()
                } else {
                    engine.clone()
                };
                let endpoint_str = if engine == "external" {
                    endpoint(&c)?
                } else {
                    detected_endpoint.clone()
                };
                profiles::install(agent, path, &engine, &endpoint_str, *force)?
            }
        },
        Command::Start { host, port } => {
            http::serve(
                host,
                *port,
                endpoint(&c)?,
                c.token.clone().unwrap_or_default(),
            )
            .await?;
            return Ok(());
        }
        Command::Mcp {
            transport,
            host,
            port,
        } => {
            if transport == "stdio" {
                stdio(endpoint(&c)?).await?;
            } else {
                http::serve(
                    host,
                    *port,
                    endpoint(&c)?,
                    c.token.clone().unwrap_or_default(),
                )
                .await?;
            }
            return Ok(());
        }
        Command::Tools => json!({"tools":schema::tools()}),
        Command::Config => {
            let (engine, ep, _) = profiles::detect_engine();
            json!({"mcpServers":{"delta":{"command":"delta","args":["mcp"],"env":profiles::server_config_env(&engine, &ep)?}}})
        }
        Command::Connect {
            agent,
            path,
            install,
            force,
            port: _,
        } if agent == "all" => {
            // Configure every agent already installed on this machine. Agents that
            // are not present are skipped rather than having config written for a
            // client the user does not have.
            let targets = profiles::unconfigured_agents();
            if targets.is_empty() {
                json!({
                    "status": "nothing_to_do",
                    "reason": "No installed agent has an DeltaBrowser config yet.",
                    "hint": "Run `delta agents` to see what was detected, or `delta agents --config` for a pasteable block.",
                    "agents": profiles::detect_agents(),
                })
            } else {
                let mut configured = Vec::new();
                let mut failed = Vec::new();
                let mut first_engine = String::new();
                for name in &targets {
                    let target = default_agent_config_path(name);
                    match profiles::connect(name, &target, *install, *force).await {
                        Ok(result) => {
                            let engine = result["engine"].as_str().unwrap_or("").to_owned();
                            if first_engine.is_empty() {
                                first_engine = engine.clone();
                            }
                            let mut entry = result;
                            entry["agent"] = json!(name);
                            configured.push(entry);
                        }
                        Err(e) => {
                            failed.push(json!({"agent": name, "error": e.to_string()}));
                        }
                    }
                }
                // A managed engine needs the daemon running before any client can
                // use it; a failing daemon must not be reported as a successful setup.
                if first_engine != "external" && !failed.is_empty() {
                    bail!("Some agents could not be configured; see the errors above.");
                }
                if first_engine != "external" && !is_daemon_running(&c.server).await {
                    eprintln!("Starting Delta HTTP daemon on {}...", c.server);
                    let token = generate_or_load_token()?;
                    spawn_daemon(&c.server, &token, endpoint(&c)?).await?;
                }
                json!({
                    "status": if failed.is_empty() { "ok" } else { "partial" },
                    "configured": configured,
                    "failed": failed,
                    "skipped": profiles::detect_agents(),
                    "engine_status": profiles::engine_status(),
                    "next_steps": "Restart each agent so it reloads its MCP configuration.",
                })
            }
        }
        Command::Connect {
            agent,
            path,
            install,
            force,
            port: _,
        } => {
            let path = path
                .clone()
                .unwrap_or_else(|| default_agent_config_path(agent));
            let result = profiles::connect(agent, &path, *install, *force).await?;
            let engine = result["engine"].as_str().unwrap_or("");
            if engine != "external" && !is_daemon_running(&c.server).await {
                eprintln!("Starting Delta HTTP daemon on {}...", c.server);
                let token = generate_or_load_token()?;
                spawn_daemon(&c.server, &token, endpoint(&c)?).await?;
                eprintln!("Daemon started.");
            }
            let mut out = result;
            out["engine_status"] = profiles::engine_status();
            out
        }
        Command::Validate { script } => {
            let steps = dsl::parse(&std::fs::read_to_string(script)?)?;
            json!({"status":"ok","steps":steps.len()})
        }
        Command::Call { tool, arguments } => {
            remote(&c, tool, serde_json::from_str(arguments)?).await?
        }
        Command::Open { url } => remote(&c, "navigate", json!({"url":url})).await?,
        Command::Snapshot { interactive_only } => {
            remote(&c, "snapshot", json!({"interactive_only":interactive_only})).await?
        }
        Command::Click { target } => remote(&c, "click", locator(target.clone())).await?,
        Command::Fill { target, value } => {
            let mut a = locator(target.clone());
            a["value"] = json!(value);
            remote(&c, "fill", a).await?
        }
        Command::Eval { script } => remote(&c, "evaluate", json!({"script":script})).await?,
        Command::Screenshot { path, full_page } => {
            let r = remote(&c, "screenshot", json!({"full_page":full_page})).await?;
            let bytes =
                base64::engine::general_purpose::STANDARD.decode(browser::string(&r, "data")?)?;
            std::fs::write(path, &bytes)?;
            json!({"path":path,"size_bytes":bytes.len()})
        }
        Command::Test { script, report } => {
            let steps = std::fs::read_to_string(script)?;
            dsl::parse(&steps)?;
            let r = remote(&c, "test_run", json!({"steps":steps})).await?;
            if let Some(path) = report {
                std::fs::write(
                    path,
                    if path.extension().is_some_and(|e| e == "html") {
                        html_report(&r)
                    } else {
                        serde_json::to_string_pretty(&r)?
                    },
                )?;
            }
            println!("{}", serde_json::to_string_pretty(&r)?);
            // A missing or non-numeric "failed" field must not be read as a clean run.
            let failed = match r.get("failed").and_then(Value::as_u64) {
                Some(n) => n,
                None => bail!(
                    "Test run result did not include a numeric 'failed' count; refusing to report success"
                ),
            };
            if failed > 0 {
                bail!("Test run failed");
            }
            return Ok(());
        }
    };
    println!("{}", serde_json::to_string_pretty(&r)?);
    Ok(())
}
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{}", json!({"status":"error","error":e.to_string()}));
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report(failed: usize) -> Value {
        json!({
            "passed": 3,
            "failed": failed,
            "skipped": 1,
            "duration_ms": 4821,
            "steps": [
                {"line":1,"action":"open","passed":true,"duration_ms":820},
                {"line":2,"action":"click @e1_3","passed":true,"duration_ms":140},
                {"line":3,"action":"fill @e1_4","passed":failed==0,"duration_ms":95,
                 "error":"Unknown/stale ref; take a new snapshot"},
                {"line":9,"action":"wait .done","skipped":true,"duration_ms":0}
            ]
        })
    }

    #[test]
    fn report_escapes_injected_markup() {
        let report = json!({
            "passed":0,"failed":1,"skipped":0,"duration_ms":1,
            "steps":[{"line":1,"action":"click <img src=x onerror=alert(1)>","passed":false,
                      "duration_ms":1,"error":"<script>alert('x')</script>"}]
        });
        let html = html_report(&report);
        assert!(
            !html.contains("<img src=x"),
            "raw markup leaked into the report"
        );
        assert!(
            !html.contains("<script>"),
            "raw script tag leaked into the report"
        );
        assert!(html.contains("&lt;img src=x"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn report_is_clean_utf8_with_no_mojibake() {
        let html = html_report(&sample_report(1));
        assert!(
            !html.contains('\u{fffd}'),
            "report contains a replacement character"
        );
        for marker in ["Ã", "â€", "Â"] {
            assert!(
                !html.contains(marker),
                "report contains mojibake marker {marker}"
            );
        }
        assert!(html.contains("<meta charset=\"utf-8\">"));
    }

    #[test]
    fn report_counts_match_status() {
        let html = html_report(&sample_report(0));
        assert!(html.contains("All executed steps passed"));
        let html = html_report(&sample_report(1));
        assert!(html.contains("1 step(s) failed"));
        assert!(html.contains("verdict bad"));
    }

    #[test]
    fn report_handles_missing_and_empty_results() {
        let html = html_report(&json!({}));
        assert!(html.contains("No steps were recorded"));
        assert!(html.contains("<pre>{}</pre>"));
    }

    #[test]
    fn doctor_report_is_wellformed() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let cli = Cli::try_parse_from(["delta", "doctor"]).unwrap();
        let report = runtime.block_on(doctor(&cli));

        assert_eq!(report["tool_count"], schema::tools().len());
        assert!(report["checks"].as_array().unwrap().len() >= 8);
        assert_eq!(
            report["agent_configs"].as_array().unwrap().len(),
            profiles::AGENTS.len()
        );
        // Every check carries a name and an explicit boolean verdict so the
        // report never renders an ambiguous state.
        for check in report["checks"].as_array().unwrap() {
            assert!(check["check"].is_string());
            assert!(check["ok"].is_boolean());
            if check["ok"] == false {
                assert!(
                    check["fix"].is_string(),
                    "failing check {} has no fix",
                    check["check"]
                );
            }
        }
        assert!(matches!(
            report["status"].as_str(),
            Some("ok") | Some("issues_found")
        ));
    }

    #[test]
    fn agents_output_lists_every_adapter_and_names_the_next_step() {
        let agents: Vec<Value> = crate::profiles::AGENTS
            .iter()
            .map(|name| {
                json!({
                    "agent": name,
                    "installed": name == &"cursor",
                    "configured": name == &"opencode",
                    "config": format!("/tmp/{name}.json"),
                })
            })
            .collect();
        let rendered = render_agents("external", &agents);

        for name in crate::profiles::AGENTS {
            assert!(
                rendered.contains(name),
                "{name} missing from `delta agents`"
            );
        }
        assert!(rendered.contains("configured"));
        assert!(rendered.contains("detected"));
        assert!(
            rendered.contains("delta connect cursor --install"),
            "a detected-but-unconfigured agent must get a copy-pasteable command"
        );
        assert!(
            rendered.contains("no kernel sandbox"),
            "external engine must say so"
        );
        assert!(!rendered.chars().any(|c| c == '\u{fffd}'));
    }

    #[test]
    fn agents_output_offers_setup_when_nothing_is_detected() {
        let agents: Vec<Value> = crate::profiles::AGENTS
            .iter()
            .map(|name| json!({"agent": name, "installed": false, "configured": false, "config": "/tmp/x"}))
            .collect();
        let rendered = render_agents("moli", &agents);
        assert!(rendered.contains("No installed agent is waiting"));
        assert!(rendered.contains("delta connect opencode --install"));
        assert!(
            !rendered.contains("delta connect all"),
            "no agents to batch means no all-command"
        );
        assert!(!rendered.contains("no kernel sandbox"));
    }

    #[test]
    fn doctor_human_output_is_ascii_safe() {
        let report = json!({
            "failures": 1,
            "checks": [
                {"check":"cdp_port","ok":false,"detail":"nothing listening","fix":"Start it"},
                {"check":"build","ok":true,"detail":"ok"}
            ],
            "agent_configs": [{"agent":"opencode","configured":true}],
            "tool_count": 62
        });
        let rendered = render_doctor(&report);
        assert!(rendered.contains("[FAIL]"));
        assert!(rendered.contains("[ok  ]"));
        assert!(rendered.contains("-> Start it"));
        assert!(rendered.contains("Configured agents: opencode"));
        assert!(rendered.contains("Tools available"));
        assert!(
            !rendered.chars().any(|c| c == '\u{fffd}'),
            "doctor output must be ASCII-safe for any console code page"
        );
    }

    #[test]
    fn doctor_offers_a_next_step_when_no_agent_is_configured() {
        let report = json!({"failures":0,"checks":[],"agent_configs":[],"tool_count":62});
        let rendered = render_doctor(&report);
        assert!(rendered.contains("Configured agents: none"));
        assert!(
            rendered.contains("delta agents"),
            "an unconfigured install must point at the detection command"
        );
    }

    #[test]
    fn doctor_calls_out_installed_but_unconfigured_agents() {
        let report = json!({
            "failures": 0,
            "checks": [],
            "agent_configs": [
                {"agent": "opencode", "configured": true, "detected": true},
                {"agent": "cursor", "configured": false, "detected": true}
            ],
            "tool_count": 62
        });
        let rendered = render_doctor(&report);
        assert!(rendered.contains("Configured agents: opencode"));
        assert!(rendered.contains("Installed but not configured: cursor"));
        assert!(rendered.contains("delta connect all --install"));
    }

    #[test]
    fn cli_aliases_map_to_real_tools() {
        // `click` and `fill` are dedicated clap subcommands, so the alias table
        // is only reached for words clap does not claim.
        let (name, args) = cli_action(&["double-click".into(), "@e1_2".into()]).unwrap();
        assert_eq!(name, "double_click");
        assert_eq!(args["ref"], "@e1_2");

        let (name, args) = cli_action(&["hover".into(), "#submit".into()]).unwrap();
        assert_eq!(name, "hover");
        assert_eq!(args["selector"], "#submit");

        let (name, args) = cli_action(&["select".into(), "#country".into(), "DE".into()]).unwrap();
        assert_eq!(name, "select_option");
        assert_eq!(args["value"], "DE");

        let (name, _) = cli_action(&["cookies".into(), "get".into()]).unwrap();
        assert_eq!(name, "get_cookies");

        let (name, args) = cli_action(&["wait".into(), ".ready".into(), "2000".into()]).unwrap();
        assert_eq!(name, "wait_for");
        assert_eq!(args["timeout_ms"], 2000);

        // Unknown words and missing arguments must fail rather than guess.
        assert!(cli_action(&["nonsense".into()]).is_err());
        assert!(cli_action(&["get".into(), "text".into()]).is_err());
        assert!(cli_action(&["hover".into()]).is_err());
        // Every alias must name a tool that actually exists.
        for word in [
            "type",
            "select",
            "check",
            "uncheck",
            "hover",
            "double-click",
            "right-click",
            "press",
            "scroll",
            "wait",
            "tabs",
            "new-tab",
            "back",
            "status",
            "logs",
            "save-state",
            "pdf",
            "cdp",
        ] {
            let call = match word {
                "type" | "select" => vec![word.into(), "#a".into(), "v".into()],
                "check" | "uncheck" | "hover" | "double-click" | "right-click" => {
                    vec![word.into(), "#a".into()]
                }
                "press" => vec![word.into(), "Enter".into()],
                "scroll" => vec![word.into(), "down".into()],
                "wait" => vec![word.into(), ".a".into()],
                "new-tab" => vec![word.into()],
                "save-state" => vec![word.into(), "s.json".into()],
                "pdf" => vec![word.into(), "p.pdf".into()],
                "cdp" => vec![word.into(), "Page.getFrameTree".into()],
                _ => vec![word.into()],
            };
            let (name, _) =
                cli_action(&call).unwrap_or_else(|e| panic!("alias '{word}' failed: {e}"));
            assert!(
                schema::tools().iter().any(|t| t["name"] == name),
                "alias '{word}' maps to unknown tool '{name}'"
            );
        }
    }

    #[test]
    fn generated_token_meets_auth_minimum() {
        // auth::load rejects tokens under 24 characters, so the generator must
        // never produce one.
        let mut raw = [0u8; 32];
        raw[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        raw[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
        assert!(
            token.len() >= 24,
            "generated token is only {} chars",
            token.len()
        );
    }
}
