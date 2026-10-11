use crate::browser::Browser;
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;
#[derive(Debug)]
pub struct Step {
    pub line: usize,
    pub words: Vec<String>,
}
pub fn parse(text: &str) -> Result<Vec<Step>> {
    if text.len() > 128 * 1024 {
        bail!("Script too large");
    }
    let mut steps = Vec::new();
    let mut depth = 0;
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let w = shell_words::split(line).map_err(|e| anyhow!("Line {}: {e}", n + 1))?;
        let valid = match w.first().map(String::as_str).unwrap_or("") {
            "open" => w.len() == 2 || (w.len() >= 4 && w.len() <= 5 && w[2] == "--wait"),
            "click" | "double_click" | "right_click" | "check" | "uncheck" | "hover" | "press"
            | "pdf" | "log" | "sleep" | "stop" => w.len() == 2,
            "fill" | "select" | "drag" | "upload" | "serve" => w.len() == 3,
            "type" => w.len() == 3 || w.len() == 4,
            "wait" | "wait_for" | "scroll" => w.len() == 2 || w.len() == 3,
            "back" | "forward" | "reload" | "snapshot" => w.len() == 1,
            "screenshot" => {
                w.len() <= 4
                    && w[1..].iter().filter(|a| !a.starts_with("--")).count() <= 1
                    && w[1..]
                        .iter()
                        .filter(|a| a.starts_with("--"))
                        .all(|a| ["--full", "--annotate"].contains(&a.as_str()))
            }
            "if" => {
                depth += 1;
                w.len() == 3 && w[1] == "visible" && depth <= 8
            }
            "end" => {
                depth -= 1;
                depth >= 0 && w.len() == 1
            }
            "get" => match w.get(1).map(String::as_str) {
                Some("text") => w.len() == 5 && w[3] == "->",
                Some("attr") => w.len() == 6 && w[4] == "->",
                Some("url") | Some("title") => w.len() == 4 && w[2] == "->",
                _ => false,
            },
            "assert" => match w.get(1).map(String::as_str) {
                Some("text") => w.len() == 4 || (w.len() == 5 && w[3] == "contains"),
                Some("attr") => w.len() == 5,
                Some("url") | Some("title") => w.len() == 3 || (w.len() == 4 && w[2] == "contains"),
                Some("visible") | Some("hidden") => w.len() == 3,
                Some("count") | Some("js") => w.len() == 4,
                _ => false,
            },
            _ => false,
        };
        if !valid {
            bail!(
                "Line {}: unsupported command or invalid argument count",
                n + 1
            );
        }
        steps.push(Step {
            line: n + 1,
            words: w,
        });
        if steps.len() > 200 {
            bail!("Maximum 200 steps");
        }
    }
    if depth != 0 {
        bail!("Unclosed conditional block");
    }
    Ok(steps)
}
fn locator(s: &str) -> Value {
    if s.starts_with('@') {
        json!({"ref":s})
    } else {
        json!({"selector":s})
    }
}
struct Process {
    child: tokio::process::Child,
}
impl Drop for Process {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        let _ = self.child.start_kill();
    }
}
#[derive(Default)]
struct Context {
    data: serde_json::Map<String, Value>,
    artifacts: Vec<Value>,
    processes: HashMap<u16, Process>,
}
async fn serve(b: &Browser, ctx: &mut Context, name: &str, port: u16) -> Result<()> {
    if ctx.processes.contains_key(&port) {
        bail!("This run already owns port {port}");
    }
    let file = std::env::var_os("DELTA_SERVE_CONFIG").ok_or_else(|| {
        anyhow!("serve disabled; operator must provide DELTA_SERVE_CONFIG with named command arrays")
    })?;
    let config: Value = serde_json::from_slice(&std::fs::read(file)?)?;
    let entry = &config[name];
    let argv = entry["argv"]
        .as_array()
        .filter(|a| !a.is_empty() && a.iter().all(Value::is_string))
        .ok_or_else(|| anyhow!("Unknown named server command or invalid argv"))?;
    if entry["port"].as_u64() != Some(port as u64) {
        bail!("Port does not match operator configuration");
    }
    if tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok()
    {
        bail!("Port already occupied; refusing to claim an unrelated server");
    }
    let executable = std::path::PathBuf::from(argv[0].as_str().unwrap()).canonicalize()?;
    let cwd = entry["cwd"]
        .as_str()
        .ok_or_else(|| anyhow!("Server command needs an explicit cwd"))?;
    // The operator owns this launch specification. Agent supplies only a name, not code or arguments.
    let mut command = tokio::process::Command::new(executable);
    command
        .args(argv[1..].iter().map(|v| v.as_str().unwrap()))
        .current_dir(cwd)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let log = std::fs::File::create(b.artifacts.root.join(format!("server-{port}.log")))?;
    command.stdout(log.try_clone()?).stderr(log);
    #[cfg(unix)]
    command.process_group(0);
    let mut process = Process {
        child: command.spawn()?,
    };
    let t = std::time::Instant::now();
    loop {
        if let Some(status) = process.child.try_wait()? {
            bail!("Configured server exited: {status}");
        }
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            break;
        }
        if t.elapsed() > std::time::Duration::from_secs(15) {
            bail!("Server readiness timeout");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    ctx.processes.insert(port, process);
    Ok(())
}
pub async fn run(b: &mut Browser, text: &str) -> Result<Value> {
    let steps = parse(text)?;
    let start = std::time::Instant::now();
    let total = steps.len();
    let mut results = Vec::new();
    let mut passed = 0;
    let mut skipped = 0;
    let mut failed = 0;
    let mut conditions: Vec<bool> = Vec::new();
    let mut ctx = Context::default();
    for step in steps {
        let t = std::time::Instant::now();
        let op = step.words[0].as_str();
        let enabled = conditions.iter().all(|b| *b);
        if op == "end" {
            conditions.pop();
            results.push(json!({"line":step.line,"action":op,"passed":true,"duration_ms":0}));
            passed += 1;
            continue;
        }
        if !enabled {
            if op == "if" {
                conditions.push(false);
            }
            skipped += 1;
            results.push(json!({"line":step.line,"action":op,"skipped":true}));
            continue;
        }
        let result = if op == "if" {
            let v=b.on_element(&locator(&step.words[2]),"function(){const r=this.getBoundingClientRect();return !!(r.width&&r.height)&&getComputedStyle(this).visibility!=='hidden'}",vec![]).await;
            // Missing elements mean false; other engine errors must not silently pass.
            match v {
                Ok(v) => {
                    conditions.push(v == true);
                    Ok(())
                }
                Err(e) if e.to_string() == "Element not found" => {
                    conditions.push(false);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        } else {
            execute(b, &step.words, &mut ctx).await
        };
        let ok = result.is_ok();
        if ok {
            passed += 1;
        } else {
            failed += 1;
        }
        results.push(json!({"line":step.line,"action":op,"passed":ok,"duration_ms":t.elapsed().as_millis(),"error":result.err().map(|e|e.to_string())}));
        if !ok {
            skipped += total - results.len();
            break;
        }
    }
    // Dropping the context kills only the processes this test run launched, even on assertion failure.
    Ok(
        json!({"total":total,"passed":passed,"failed":failed,"skipped":skipped,"duration_ms":start.elapsed().as_millis(),"steps":results,"data":ctx.data,"artifacts":ctx.artifacts}),
    )
}
async fn execute(b: &mut Browser, w: &[String], ctx: &mut Context) -> Result<()> {
    let (tool, args) = match w[0].as_str() {
        "open" => {
            b.run("navigate", json!({"url":w[1]})).await?;
            if w.len() > 2 {
                b.run("wait_for",json!({"selector":w[3],"timeout_ms":w.get(4).map(|v|v.parse::<u64>()).transpose()?.unwrap_or(5000)})).await?;
            }
            return Ok(());
        }
        "click" | "double_click" | "right_click" | "check" | "uncheck" | "hover" => {
            (w[0].as_str(), locator(&w[1]))
        }
        "fill" | "type" | "select" => {
            let mut a = locator(&w[1]);
            a["value"] = json!(w[2]);
            if w[0] == "type" && w.len() == 4 {
                let delay = w[3].parse::<u64>()?.min(1000);
                for ch in w[2].chars() {
                    a["value"] = json!(ch.to_string());
                    b.run("type", a.clone()).await?;
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
                return Ok(());
            }
            (
                if w[0] == "select" {
                    "select_option"
                } else {
                    &w[0]
                },
                a,
            )
        }
        "wait" => (
            "wait_for",
            json!({"selector":w[1],"timeout_ms":w.get(2).map(|v|v.parse::<u64>()).transpose()?.unwrap_or(5000)}),
        ),
        "wait_for" => {
            if w[1] == "networkidle" {
                b.wait_network_idle(
                    w.get(2)
                        .map(|v| v.parse::<u64>())
                        .transpose()?
                        .unwrap_or(5000)
                        .min(30000),
                )
                .await?;
                return Ok(());
            }
            let script = match w[1].as_str() {
                "load" => "document.readyState==='complete'",
                "domcontentloaded" => "document.readyState!=='loading'",
                _ => bail!("Unknown wait condition"),
            };
            let limit = w
                .get(2)
                .map(|v| v.parse::<u64>())
                .transpose()?
                .unwrap_or(5000)
                .min(30000);
            let t = std::time::Instant::now();
            loop {
                if b.eval(script).await? == true {
                    return Ok(());
                }
                if t.elapsed().as_millis() > limit as u128 {
                    bail!("Lifecycle wait timeout");
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
        "press" => ("press_key", json!({"key":w[1]})),
        "scroll" => (
            "scroll",
            json!({"direction":w[1],"pixels":w.get(2).map(|v|v.parse::<u64>()).transpose()?.unwrap_or(500)}),
        ),
        "drag" => {
            let mut a = json!({});
            a[if w[1].starts_with('@') {
                "from_ref"
            } else {
                "from_selector"
            }] = json!(w[1]);
            a[if w[2].starts_with('@') {
                "to_ref"
            } else {
                "to_selector"
            }] = json!(w[2]);
            ("drag", a)
        }
        "upload" => {
            let mut a = locator(&w[1]);
            a["path"] = json!(w[2]);
            ("upload_file", a)
        }
        "back" => ("go_back", json!({})),
        "forward" => ("go_forward", json!({})),
        "reload" | "snapshot" => (w[0].as_str(), json!({})),
        "sleep" => {
            let n = w[1].parse::<u64>()?;
            if n > 30000 {
                bail!("Sleep limit 30 seconds");
            }
            tokio::time::sleep(std::time::Duration::from_millis(n)).await;
            return Ok(());
        }
        "log" => {
            ctx.data
                .insert(format!("log-{}", ctx.data.len()), json!(w[1]));
            return Ok(());
        }
        "screenshot" | "pdf" => {
            let pdf = w[0] == "pdf";
            let annotate = w.iter().any(|s| s == "--annotate");
            let name = w[1..]
                .iter()
                .find(|s| !s.starts_with("--"))
                .cloned()
                .unwrap_or_else(|| format!("shot-{}.png", ctx.artifacts.len() + 1));
            let r = b
                .run(
                    if pdf {
                        "save_pdf"
                    } else if annotate {
                        "annotate_screenshot"
                    } else {
                        "screenshot"
                    },
                    if pdf || annotate {
                        json!({})
                    } else {
                        json!({"full_page":w.iter().any(|s|s=="--full")})
                    },
                )
                .await?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(crate::browser::string(&r, "data")?)?;
            let path = b.artifacts.write(&name, &bytes)?;
            ctx.artifacts.push(
                json!({"path":path,"bytes":bytes.len(),"type":if pdf{"pdf"}else{"screenshot"}}),
            );
            return Ok(());
        }
        "serve" => {
            serve(b, ctx, &w[1], w[2].parse::<u16>()?).await?;
            return Ok(());
        }
        "stop" => {
            let port = w[1].parse::<u16>()?;
            if ctx.processes.remove(&port).is_none() {
                bail!("Run does not own server port {port}");
            }
            return Ok(());
        }
        "get" => {
            let args = if w[1] == "text" || w[1] == "attr" {
                let mut a = locator(&w[2]);
                if w[1] == "attr" {
                    a["attribute"] = json!(w[3]);
                }
                a
            } else {
                json!({})
            };
            let tool = match w[1].as_str() {
                "text" => "get_text",
                "attr" => "get_attribute",
                "url" => "get_url",
                _ => "get_title",
            };
            let v = b.run(tool, args).await?;
            ctx.data.insert(w.last().unwrap().clone(), v);
            return Ok(());
        }
        "assert" => {
            let kind = w[1].as_str();
            if kind == "visible" || kind == "hidden" {
                if w[2].starts_with('@') {
                    let mut a = locator(&w[2]);
                    let r = b.run("test_assert_visible", a.take()).await?;
                    if r["passed"] != (kind == "visible") {
                        bail!("Visibility assertion mismatch");
                    }
                } else {
                    b.run(
                        "wait_for",
                        json!({"selector":w[2],"state":kind,"timeout_ms":3000}),
                    )
                    .await?;
                }
                return Ok(());
            }
            let (actual, offset) = match kind {
                "text" => (b.run("get_text", locator(&w[2])).await?, 3),
                "attr" => {
                    let mut a = locator(&w[2]);
                    a["attribute"] = json!(w[3]);
                    (b.run("get_attribute", a).await?, 4)
                }
                "url" => (b.run("get_url", json!({})).await?, 2),
                "title" => (b.run("get_title", json!({})).await?, 2),
                "count" => (
                    b.eval(&format!(
                        "document.querySelectorAll({}).length",
                        json!(w[2])
                    ))
                    .await?,
                    3,
                ),
                "js" => (b.eval(&w[2]).await?, 3),
                _ => bail!("Unknown assertion"),
            };
            let contains = w[offset] == "contains" && w.len() > offset + 1;
            let expected = &w[offset + usize::from(contains)];
            let actual = if let Some(s) = actual.as_str() {
                s.to_owned()
            } else {
                actual.to_string()
            };
            if !(if contains {
                actual.contains(expected)
            } else {
                actual == *expected
            }) {
                bail!("{kind} assertion mismatch (values omitted)");
            }
            return Ok(());
        }
        _ => bail!("Unsupported step"),
    };
    b.run(tool, args).await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotes() {
        let p = parse("fill '#email' \"hello world\"\nassert text '.x' contains 'Done'").unwrap();
        assert_eq!(p[0].words[2], "hello world");
    }
    #[test]
    fn structural_validation() {
        assert!(parse("click").is_err());
        assert!(parse("if visible .x").is_err());
        assert!(parse("end").is_err());
        assert!(parse("if visible .x\nclick .x\nend").is_ok());
        assert!(parse("serve app 4173").is_ok());
    }
    #[test]
    fn extraction() {
        assert!(parse("get text .result -> result\nassert count button 2\nscreenshot result.png --full --annotate").is_ok());
        assert!(parse("get text .x wrong key").is_err());
    }
}
