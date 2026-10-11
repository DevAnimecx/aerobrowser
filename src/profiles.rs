use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::Path;

pub fn list() -> Value {
    json!([
        {"name":"scrape","implemented":true,"filesystem":"runtime read + private worker write","network":"all","exec":"engine only; threads allowed, subprocesses blocked on Linux"},
        {"name":"offline","implemented":true,"filesystem":"runtime read + private worker write","network":"outbound connect and datagram send denied on Linux; local CDP listener remains"},
        {"name":"untrusted","implemented":true,"same_as":"offline"},
        {"name":"full","implemented":true,"same_as":"scrape; does not grant host filesystem write"},
        {"name":"base","implemented":false,"reason":"Loopback-only kernel network broker not implemented; fails closed"},
        {"name":"web-dev","implemented":false,"reason":"Hostname allowlist network broker not implemented; fails closed"},
        {"name":"test","implemented":false,"reason":"Loopback-only kernel network broker not implemented; fails closed"}
    ])
}

/// Resolves the user's home directory across platforms.
///
/// `HOME` is normally unset on Windows, so `USERPROFILE` and `HOMEDRIVE`+`HOMEPATH`
/// are consulted as fallbacks. Returns `None` only when no home can be determined.
pub fn home_dir() -> Option<std::path::PathBuf> {
    for key in ["HOME", "USERPROFILE"] {
        if let Some(v) = std::env::var_os(key) {
            if !v.is_empty() {
                return Some(std::path::PathBuf::from(v));
            }
        }
    }
    let drive = std::env::var_os("HOMEDRIVE");
    let path = std::env::var_os("HOMEPATH");
    match (drive, path) {
        (Some(d), Some(p)) => {
            let joined = format!("{}{}", d.to_string_lossy(), p.to_string_lossy());
            if joined.is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(joined))
            }
        }
        _ => None,
    }
}

/// Detects the best available browser engine for this system.
/// Priority: Moli (managed, sandboxed) > external Chrome (CDP port).
/// Returns (engine_name, endpoint, installed: bool).
pub fn detect_engine() -> (String, String, bool) {
    let home = home_dir();

    // Check for installed Moli engine
    if let Some(home) = &home {
        let moli_path = home.join(".delta/engines/moli-v1.1.12/moli");
        if moli_path.is_file() {
            return ("moli".to_string(), "managed".to_string(), true);
        }
    }

    // Check for bundled Moli (from @bundled-moli feature)
    if cfg!(feature = "bundled-moli") && home.is_some() {
        return ("moli".to_string(), "managed-bundled".to_string(), true);
    }

    // Check for external Chrome on default CDP port
    if let Ok(stream) = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9222".parse().unwrap(),
        std::time::Duration::from_millis(200),
    ) {
        drop(stream);
        return (
            "external".to_string(),
            "http://127.0.0.1:9222".to_string(),
            true,
        );
    }

    // Default: recommend Moli
    ("moli".to_string(), "managed".to_string(), false)
}

/// Every agent adapter this build supports.
pub const AGENTS: &[&str] = &[
    "opencode",
    "claude-code",
    "claude-desktop",
    "cursor",
    "vscode",
    "codex",
    "windsurf",
    "cline",
    "roo-code",
    "hermes",
    "generic",
];

/// Windows markers for an agent that is present but never launched.
#[cfg(target_os = "windows")]
fn windows_installed_markers(agent: &str) -> Vec<std::path::PathBuf> {
    let program_files = std::env::var_os("ProgramFiles").map(std::path::PathBuf::from);
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
    let appdata = std::env::var_os("APPDATA").map(std::path::PathBuf::from);
    let mut out = Vec::new();
    let mut push = |base: Option<&std::path::PathBuf>, parts: &[&str]| {
        if let Some(base) = base {
            let mut path = base.clone();
            for part in parts {
                path = path.join(part);
            }
            out.push(path);
        }
    };
    match agent {
        "claude-desktop" => push(appdata.as_ref(), &["Claude"]),
        "cursor" => push(local_app_data.as_ref(), &["Programs", "cursor"]),
        "vscode" => push(program_files.as_ref(), &["Microsoft VS Code"]),
        "windsurf" => push(local_app_data.as_ref(), &["Programs", "Windsurf"]),
        _ => {}
    }
    out
}

#[cfg(not(target_os = "windows"))]
fn windows_installed_markers(_agent: &str) -> Vec<std::path::PathBuf> {
    Vec::new()
}

/// macOS markers for an agent that is present but never launched.
#[cfg(target_os = "macos")]
fn macos_installed_markers(agent: &str) -> Vec<std::path::PathBuf> {
    let applications = std::path::PathBuf::from("/Applications");
    let match_name = match agent {
        "claude-desktop" => Some("Claude.app"),
        "cursor" => Some("Cursor.app"),
        "vscode" => Some("Visual Studio Code.app"),
        "windsurf" => Some("Windsurf.app"),
        _ => None,
    };
    match_name
        .map(|name| vec![applications.join(name)])
        .unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
fn macos_installed_markers(_agent: &str) -> Vec<std::path::PathBuf> {
    Vec::new()
}

/// Linux markers for an agent that is present but never launched.
#[cfg(target_os = "linux")]
fn linux_installed_markers(agent: &str) -> Vec<std::path::PathBuf> {
    let names: &[&str] = match agent {
        "claude-code" => &["claude"],
        "cursor" => &["cursor"],
        "vscode" => &["code"],
        "windsurf" => &["windsurf"],
        _ => &[],
    };
    let mut out = Vec::new();
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            for name in names {
                out.push(dir.join(name));
            }
        }
    }
    out
}

#[cfg(not(target_os = "linux"))]
fn linux_installed_markers(_agent: &str) -> Vec<std::path::PathBuf> {
    Vec::new()
}

/// Where this build expects an agent's configuration to live.
pub fn default_config_path(agent: &str) -> std::path::PathBuf {
    let home = home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    match agent {
        "opencode" => home.join(".config").join("opencode").join("opencode.json"),
        "claude-code" => std::path::PathBuf::from(".mcp.json"),
        // Claude Desktop keeps its config in a platform-specific application
        // support directory, not the macOS path on every host.
        "claude-desktop" => {
            if cfg!(target_os = "macos") {
                home.join("Library")
                    .join("Application Support")
                    .join("Claude")
                    .join("claude_desktop_config.json")
            } else if cfg!(target_os = "windows") {
                let appdata = std::env::var_os("APPDATA")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| home.clone());
                appdata.join("Claude").join("claude_desktop_config.json")
            } else {
                let xdg = std::env::var_os("XDG_CONFIG_HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| home.join(".config"));
                xdg.join("Claude").join("claude_desktop_config.json")
            }
        }
        "cursor" => home.join(".cursor").join("mcp.json"),
        "vscode" => home.join(".vscode").join("mcp.json"),
        "windsurf" => home.join(".codeium").join("windsurf").join("mcp.json"),
        "cline" => home.join(".cline").join("mcp.json"),
        "roo-code" => home.join(".roo").join("mcp.json"),
        "hermes" => home.join(".hermes").join("config.yaml"),
        "codex" => home.join(".codex").join("config.toml"),
        "generic" => std::path::PathBuf::from("delta-mcp.json"),
        _ => std::path::PathBuf::from(format!("{agent}-mcp.json")),
    }
}

/// Whether an agent appears to be installed on this machine.
///
/// This looks for the agent's application or executable. It does not launch it,
/// and it is a heuristic: a marker file existing does not prove the client works
/// with DeltaBrowser.
pub fn agent_installed(agent: &str) -> bool {
    let config_present = default_config_path(agent).exists();
    let markers = {
        let mut all = windows_installed_markers(agent);
        all.extend(macos_installed_markers(agent));
        all.extend(linux_installed_markers(agent));
        all
    };
    markers.iter().any(|m| m.exists()) || config_present
}

/// Surfaces the agents that appear to be installed, with their config paths.
pub fn detect_agents() -> Vec<Value> {
    AGENTS
        .iter()
        .map(|agent| {
            let path = default_config_path(agent);
            json!({
                "agent": agent,
                "installed": agent_installed(agent),
                "config": path.to_string_lossy(),
                "configured": path.exists(),
            })
        })
        .collect()
}

/// Agents that appear to be installed and are not configured yet.
pub fn unconfigured_agents() -> Vec<&'static str> {
    AGENTS
        .iter()
        .copied()
        .filter(|agent| agent_installed(agent) && !default_config_path(agent).exists())
        .collect()
}

/// Returns a human-readable summary of engine detection.
pub fn engine_status() -> Value {
    let (engine, endpoint, installed) = detect_engine();
    json!({
        "detected_engine": engine,
        "endpoint": endpoint,
        "installed": installed,
        "note": if installed {
            format!("Using {} engine at {}", engine, endpoint)
        } else {
            if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
                format!("{engine} not installed; run `delta engine install moli` or `delta connect <agent> --install`")
            } else {
                format!("{engine} managed engines are Linux x86_64 only; run `delta connect <agent> --install` to adopt a locally installed browser")
            }
        }
    })
}

/// Builds the MCP server env config for a given engine choice.
/// For managed engines (moli), no CDP endpoint is needed.
/// For external engines, an endpoint URL is required.
pub fn server_config(exe: &str, engine: &str, endpoint: &str) -> Value {
    let mut env = json!({});
    if engine == "moli" {
        env["AERO_ENGINE"] = json!("moli");
        if !endpoint.is_empty() && endpoint != "managed" {
            env["AERO_CDP"] = json!(endpoint);
        }
    } else if engine == "moli-bundled" {
        env["AERO_ENGINE"] = json!("moli");
    } else {
        env["AERO_CDP"] = json!(endpoint);
        env["AERO_ENGINE"] = json!("external");
    }
    json!({
        "command": exe,
        "args": ["mcp"],
        "env": env
    })
}

/// Returns just the env portion for inline config templates.
pub fn server_config_env(engine: &str, endpoint: &str) -> Result<Value> {
    Ok(server_config("", engine, endpoint)["env"].clone())
}

pub fn template(agent: &str, exe: &str, engine: &str, endpoint: &str) -> Result<Value> {
    let server = server_config(exe, engine, endpoint);
    Ok(match agent {
        "claude-code" | "claude-desktop" | "cursor" | "windsurf" | "cline" | "roo-code" => {
            json!({"mcpServers":{"delta":server}})
        }
        "vscode" => json!({"servers":{"delta":{"type":"stdio","command":exe,"args":["mcp"],"env":server["env"]}}}),
        "opencode" => json!({"mcp":{"delta":{"type":"local","command":[exe,"mcp"],"environment":server["env"],"enabled":true}}}),
        "hermes" => json!({"mcp_servers":{"delta":{"command":exe,"args":["mcp"],"env":server["env"]}}}),
        "generic" => server,
        _ => bail!("Supported JSON config adapters: claude-code, claude-desktop, cursor, vscode, opencode, windsurf, cline, roo-code, hermes, generic. Client file location must be supplied explicitly."),
    })
}

pub fn install(
    agent: &str,
    path: &Path,
    engine: &str,
    endpoint: &str,
    force: bool,
) -> Result<Value> {
    let exe = std::env::current_exe()?.to_string_lossy().into_owned();
    let template = template(agent, &exe, engine, endpoint)?;
    if agent == "codex" {
        return install_codex(path, &exe, engine, endpoint, force);
    }

    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        bail!("Refusing symlink configuration path");
    }

    let mut doc: Value = if path.exists() {
        if agent == "hermes" {
            serde_yaml_ng::from_slice(&std::fs::read(path)?)?
        } else {
            serde_json::from_slice(&std::fs::read(path)?)?
        }
    } else {
        json!({})
    };

    if !doc.is_object() {
        bail!("Configuration root must be an object");
    }

    let mut backup = None;
    for (group, value) in template.as_object().unwrap() {
        if agent == "generic" {
            if path.exists() && !force {
                bail!("Generic export exists; use --force");
            }
            doc = template.clone();
            break;
        }
        if doc.get(group).is_none() {
            doc[group] = json!({});
        }
        if !doc[group].is_object() {
            bail!("Existing config group is not an object");
        }
        if doc[group].get("delta").is_some() && !force {
            bail!("Delta configuration exists; use --force to replace with a backup");
        }
        doc[group]["delta"] = value["delta"].clone();
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    if path.exists() {
        let p = path.with_extension(format!("backup-{}.json", uuid::Uuid::new_v4()));
        std::fs::copy(path, &p)?;
        backup = Some(p);
    }

    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(
        &temp,
        if agent == "hermes" {
            serde_yaml_ng::to_string(&doc)?.into_bytes()
        } else {
            serde_json::to_vec_pretty(&doc)?
        },
    )?;
    std::fs::rename(temp, path)?;

    let skill = path.with_extension("delta-skill.md");
    std::fs::write(&skill, include_str!("../docs/AGENT-SKILL.md"))?;

    Ok(json!({
        "config": path,
        "backup": backup,
        "skill": skill,
        "engine": engine,
        "endpoint": endpoint,
        "note": "Config generated/merged with engine auto-detection. Import the skill file alongside if your client does not discover it. Client runtime compatibility is not implied by writing a file."
    }))
}

/// Locates a locally installed Chromium-family browser when no CDP listener is up.
///
/// Managed Moli downloads are Linux-only, so on Windows and macOS a `--install`
/// run must fall back to a browser already present on the machine rather than
/// failing. Returns the absolute path to a browser executable, if any.
pub fn find_local_browser() -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    let home = home_dir();
    let program_files = std::env::var_os("ProgramFiles").map(std::path::PathBuf::from);
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(std::path::PathBuf::from);
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);

    if cfg!(target_os = "windows") {
        for base in [program_files.as_ref(), program_files_x86.as_ref()]
            .into_iter()
            .flatten()
        {
            candidates.push(
                base.join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
            candidates.push(
                base.join("Microsoft")
                    .join("Edge")
                    .join("Application")
                    .join("msedge.exe"),
            );
        }
        if let Some(base) = &local_app_data {
            candidates.push(
                base.join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
            candidates.push(
                base.join("Microsoft")
                    .join("Edge")
                    .join("Application")
                    .join("msedge.exe"),
            );
        }
    } else if cfg!(target_os = "macos") {
        candidates.push(std::path::PathBuf::from(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ));
        candidates.push(std::path::PathBuf::from(
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ));
        candidates.push(std::path::PathBuf::from(
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ));
        if let Some(h) = &home {
            candidates.push(h.join("Applications/Google Chrome.app/Contents/MacOS/Google Chrome"));
        }
    } else {
        for name in [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
            "microsoft-edge",
        ] {
            if let Some(paths) = std::env::var_os("PATH") {
                for dir in std::env::split_paths(&paths) {
                    candidates.push(dir.join(name));
                }
            }
        }
        if let Some(h) = &home {
            candidates.push(h.join(".cache/ms-playwright/chromium-*/chrome-linux/chrome"));
        }
    }

    candidates.into_iter().find(|p| p.is_file())
}

/// One-step agent connection: detects/installs engine, generates agent config.
pub async fn connect(
    agent: &str,
    path: &Path,
    install_if_missing: bool,
    force: bool,
) -> Result<Value> {
    let (detected, endpoint, installed) = detect_engine();
    let mut engine = detected.clone();
    let mut resolved_endpoint = endpoint.clone();
    let mut engine_info = String::new();
    let mut platform_note: Option<String> = None;

    if !installed && install_if_missing && cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        let install_result = crate::engine::install_moli().await?;
        engine = "moli".to_string();
        resolved_endpoint = "managed".to_string();
        engine_info = install_result["path"].as_str().unwrap_or("").to_string();
    } else if !installed && install_if_missing {
        // Managed Moli downloads are Linux x86_64 only. On every other platform,
        // --install adopts a browser already installed on the machine so the
        // documented one-step flow still completes.
        match find_local_browser() {
            Some(browser) => {
                engine = "external".to_string();
                resolved_endpoint = "http://127.0.0.1:9222".to_string();
                engine_info = browser.to_string_lossy().to_string();
                platform_note = Some(format!(
                    "Managed Moli engine downloads are Linux x86_64 only. Configured the external engine using {} found on this machine; launch it with --remote-debugging-port=9222.",
                    browser.display()
                ));
            }
            None => {
                bail!(
                    "No managed engine is available on this platform (managed Moli downloads are Linux x86_64 only) and no local Chromium-family browser was found.\n\
                     Start an existing browser with --remote-debugging-port=9222 and re-run, or pass --engine-path to a browser executable."
                );
            }
        }
    } else if !installed && detected == "moli" {
        bail!(
            "No browser engine is available.\n\
             Run `delta connect {} --install` to install a managed engine on Linux x86_64 or adopt a locally installed browser, or start an existing browser with --remote-debugging-port=9222.",
            agent
        );
    }

    let result = install(agent, path, &engine, &resolved_endpoint, force)?;
    let mut out = json!({
        "agent": agent,
        "engine": engine,
        "endpoint": resolved_endpoint,
        "installed": installed || !engine_info.is_empty() || install_if_missing,
        "config": result["config"],
        "skill": result["skill"],
        "next_steps": if engine == "external" {
            "Start your browser with --remote-debugging-port=9222, or run: delta start"
        } else if engine == "moli" {
            "The managed Moli engine will auto-launch when you first use a browser tool. No external browser needed."
        } else {
            "Start the Delta HTTP daemon: delta start"
        }
    });

    if let Some(note) = platform_note {
        out["platform_note"] = json!(note);
    }
    if let Some(backup) = result.get("backup") {
        out["backup"] = backup.clone();
    }
    if !engine_info.is_empty() {
        out["engine_path"] = json!(engine_info);
    }
    Ok(out)
}

fn install_codex(
    path: &Path,
    exe: &str,
    engine: &str,
    endpoint: &str,
    force: bool,
) -> Result<Value> {
    use toml_edit::{value, Array, DocumentMut, Item, Table};

    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        bail!("Refusing symlink configuration path");
    }

    let mut doc: DocumentMut = if path.exists() {
        std::fs::read_to_string(path)?.parse()?
    } else {
        DocumentMut::new()
    };

    if doc.get("mcp_servers").is_none() {
        doc["mcp_servers"] = Item::Table(Table::new());
    }

    let servers = doc["mcp_servers"]
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("mcp_servers must be a TOML table"))?;

    if servers.contains_key("delta") && !force {
        bail!("Delta configuration exists; use --force for backed-up replacement");
    }

    let mut table = Table::new();
    table["command"] = value(exe);
    let mut args = Array::new();
    args.push("mcp");
    table["args"] = value(args);

    let mut env = Table::new();
    if engine == "moli" {
        env["AERO_ENGINE"] = value("moli");
        if !endpoint.is_empty() && endpoint != "managed" {
            env["AERO_CDP"] = value(endpoint);
        }
    } else {
        env["AERO_CDP"] = value(endpoint);
        env["AERO_ENGINE"] = value("external");
    }
    table["env"] = Item::Table(env);

    servers["delta"] = Item::Table(table);

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let backup = if path.exists() {
        let p = path.with_extension(format!("backup-{}.toml", uuid::Uuid::new_v4()));
        std::fs::copy(path, &p)?;
        Some(p)
    } else {
        None
    };

    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temp, doc.to_string())?;
    std::fs::rename(temp, path)?;

    let skill = path.with_extension("delta-skill.md");
    std::fs::write(&skill, include_str!("../docs/AGENT-SKILL.md"))?;

    Ok(json!({
        "config": path,
        "backup": backup,
        "skill": skill,
        "format": "toml"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert!(
            template("vscode", "delta", "moli", "managed").unwrap()["servers"]["delta"].is_object()
        );
        assert!(
            template("opencode", "delta", "external", "http://localhost:9222").unwrap()["mcp"]
                ["delta"]["command"]
                .is_array()
        );
        assert!(template("unknown", "delta", "moli", "managed").is_err());
    }

    #[test]
    fn managed_engine_has_no_cdp_env() {
        let v = server_config("delta", "moli", "managed");
        assert_eq!(v["env"]["AERO_ENGINE"], json!("moli"));
        assert!(v["env"]["AERO_CDP"].is_null());
    }

    #[test]
    fn external_engine_has_cdp_env() {
        let v = server_config("delta", "external", "http://127.0.0.1:9222");
        assert_eq!(v["env"]["AERO_ENGINE"], json!("external"));
        assert_eq!(v["env"]["AERO_CDP"], json!("http://127.0.0.1:9222"));
    }

    #[test]
    fn detect_engine_returns_known_variant() {
        let (engine, _, _) = detect_engine();
        assert!(["moli", "external"].contains(&engine.as_str()));
    }

    #[test]
    fn every_adapted_agent_has_a_distinct_config_path() {
        // Two agents sharing a path would mean one overwrites the other's config.
        let mut seen = std::collections::HashSet::new();
        for agent in AGENTS {
            let path = default_config_path(agent);
            assert!(
                seen.insert(path.clone()),
                "{agent} collides on {}",
                path.display()
            );
        }
        assert_eq!(seen.len(), AGENTS.len());
    }

    #[test]
    fn unknown_agents_still_get_a_usable_path() {
        let path = default_config_path("some-future-agent");
        assert!(path.to_string_lossy().contains("some-future-agent"));
    }

    #[test]
    fn detection_reports_every_agent_with_both_states() {
        let detected = detect_agents();
        assert_eq!(detected.len(), AGENTS.len());
        for agent in &detected {
            assert!(agent["agent"].is_string());
            assert!(agent["installed"].is_boolean());
            assert!(agent["configured"].is_boolean());
            assert!(agent["config"].is_string());
        }
        // Nothing reported as configured should be absent from the pending list.
        for pending in unconfigured_agents() {
            assert!(!default_config_path(pending).exists());
            assert!(agent_installed(pending));
        }
    }

    #[test]
    fn home_dir_reports_the_source_variable() {
        let previous_home = std::env::var_os("HOME");
        let previous_profile = std::env::var_os("USERPROFILE");

        std::env::set_var("HOME", "/tmp/delta-primary");
        std::env::set_var("USERPROFILE", r"C:\Users\should-not-win");
        assert_eq!(
            home_dir().map(|p| p.to_string_lossy().to_string()),
            Some("/tmp/delta-primary".to_owned()),
            "HOME must take precedence over USERPROFILE"
        );

        if let Some(v) = previous_home {
            std::env::set_var("HOME", v);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(v) = previous_profile {
            std::env::set_var("USERPROFILE", v);
        }
    }

    #[test]
    fn home_dir_prefers_home_then_userprofile() {
        // Either variable is enough; both unset means no home.
        let previous_home = std::env::var_os("HOME");
        let previous_profile = std::env::var_os("USERPROFILE");

        std::env::set_var("HOME", "/tmp/delta-home");
        assert_eq!(
            home_dir().map(|p| p.to_string_lossy().to_string()),
            Some("/tmp/delta-home".to_owned())
        );

        std::env::remove_var("HOME");
        std::env::set_var("USERPROFILE", r"C:\Users\test");
        assert_eq!(
            home_dir().map(|p| p.to_string_lossy().to_string()),
            Some(r"C:\Users\test".to_owned())
        );

        std::env::remove_var("USERPROFILE");
        std::env::remove_var("HOMEDRIVE");
        std::env::remove_var("HOMEPATH");
        assert!(home_dir().is_none());

        if let Some(v) = previous_home {
            std::env::set_var("HOME", v);
        }
        if let Some(v) = previous_profile {
            std::env::set_var("USERPROFILE", v);
        }
    }
}
