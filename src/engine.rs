use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::process::Child;
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub engine: String,
    pub executable: PathBuf,
    pub profile: String,
    #[serde(default)]
    pub no_sandbox: bool,
    #[serde(default)]
    pub memory_limit_mb: Option<u64>,
    #[serde(default)]
    pub pids_limit: Option<u64>,
    #[serde(default)]
    pub cgroup_root: Option<PathBuf>,
}
#[derive(Serialize, Deserialize)]
pub struct Worker {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub profile: crate::sandbox::Profile,
    pub workdir: PathBuf,
}
pub struct Engine {
    child: Child,
    pub endpoint: String,
    pub info: Value,
    resources: Option<crate::resources::Group>,
}
impl Drop for Engine {
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
impl Engine {
    pub async fn shutdown(&mut self) -> Result<()> {
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        let _ = self.child.start_kill();
        tokio::time::timeout(Duration::from_secs(2), self.child.wait())
            .await
            .context("Worker reap timed out")??;
        if let Some(group) = &self.resources {
            group.cleanup().await?;
        }
        self.resources.take();
        Ok(())
    }

    pub fn resource_stats(&self) -> Value {
        self.resources.as_ref().map(|g| g.stats()).unwrap_or_else(
            || json!({"enforcement":"none","reason":"no hard resource limits requested"}),
        )
    }
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    pub async fn launch(endpoint: &str) -> Result<Option<Self>> {
        if !endpoint.starts_with('{') {
            return Ok(None);
        }
        let config: Config = serde_json::from_str(endpoint)?;
        let executable = if config.executable == std::path::Path::new("@bundled-moli") {
            materialize_bundled()?
        } else {
            config.executable.clone()
        };
        let exe = executable
            .canonicalize()
            .context("Engine executable missing; install it or supply --engine-path")?;
        let work = crate::artifacts::Artifacts::new()?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let address = format!("http://127.0.0.1:{port}");
        let mut arguments = match config.engine.as_str() {
            "moli" => vec![
                "serve".into(),
                "--host".into(),
                "127.0.0.1".into(),
                "--port".into(),
                port.to_string(),
                "--layout".into(),
                "--resource".into(),
                "--profile-dir".into(),
                work.root.to_string_lossy().into_owned(),
            ],
            "lightpanda" => vec![
                "serve".into(),
                "--host".into(),
                "127.0.0.1".into(),
                "--port".into(),
                port.to_string(),
            ],
            "chrome" => vec![
                "--headless".into(),
                format!("--remote-debugging-port={port}"),
                "--remote-debugging-address=127.0.0.1".into(),
                format!("--user-data-dir={}", work.root.display()),
                "--no-first-run".into(),
                "--disable-background-networking".into(),
                "about:blank".into(),
            ],
            _ => bail!("Unknown engine"),
        };
        let mut profile = crate::sandbox::Profile::load(&config.profile)?;
        for p in [
            "/usr",
            "/System/Library",
            "/Library/Fonts",
            "/lib",
            "/lib64",
            "/etc/ssl",
            "/etc/fonts",
            "/etc/resolv.conf",
            "/etc/hosts",
            "/etc/nsswitch.conf",
            "/etc/ld.so.cache",
            "/dev/null",
            "/dev/urandom",
            "/proc/self",
        ] {
            let p = PathBuf::from(p);
            if p.exists() {
                profile.read.push(p);
            }
        }
        profile.write.push(work.root.clone());
        if let Some(upload) = std::env::var_os("AERO_UPLOAD_ROOT") {
            profile.read.push(PathBuf::from(upload).canonicalize()?);
        }
        if config.engine == "chrome" {
            profile.allow_child_processes = true;
            if config.no_sandbox {
                arguments.push("--no-sandbox".into());
            }
        }
        let mut command = if config.no_sandbox {
            let mut c = tokio::process::Command::new(&exe);
            c.args(&arguments);
            c
        } else {
            let worker = Worker {
                executable: exe,
                arguments,
                profile: profile.clone(),
                workdir: work.root.clone(),
            };
            let p = work.write("worker.json", &serde_json::to_vec(&worker)?)?;
            let mut c = tokio::process::Command::new(std::env::current_exe()?);
            c.arg("_worker").arg(p);
            c
        };
        let log = std::fs::File::create(work.root.join("engine.log"))?;
        command
            .current_dir(&work.root)
            .env_clear()
            .env("HOME", &work.root)
            .env("TMPDIR", &work.root)
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let t = Instant::now();
        let resources = crate::resources::Group::create(
            config.cgroup_root.as_deref(),
            config.memory_limit_mb,
            config.pids_limit,
        )?;
        #[cfg(target_os = "linux")]
        if let Some(group) = &resources {
            use std::os::fd::AsRawFd;
            let enrollment = group.enrollment()?;
            unsafe {
                command.pre_exec(move || {
                    let value = b"0";
                    if libc::write(enrollment.as_raw_fd(), value.as_ptr().cast(), 1) != 1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        let child = command
            .spawn()
            .context("Worker spawn/cgroup enrollment failed; no unrestricted fallback")?;
        let mut result = Self {
            child,
            resources,
            endpoint: if config.engine == "lightpanda" {
                format!("ws://127.0.0.1:{port}")
            } else {
                address.clone()
            },
            info: json!({"engine":config.engine,"profile":config.profile,"sandbox":if config.no_sandbox{"explicitly disabled"}else if cfg!(target_os="linux"){ "Landlock + seccomp; see policy limitations"}else{"Seatbelt; unvalidated platform"},"network":profile.network,"worker_directory":work.root}),
        };
        result.info["pid"] = json!(result.child.id());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()?;
        loop {
            if let Some(exit) = result.child.try_wait()? {
                let log = std::fs::read_to_string(work.root.join("engine.log")).unwrap_or_default();
                bail!(
                    "Engine exited {exit}: {}",
                    log.chars().take(3000).collect::<String>()
                );
            }
            let ready = if config.engine == "lightpanda" {
                tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .is_ok()
            } else {
                client
                    .get(format!("{address}/json/version"))
                    .send()
                    .await
                    .is_ok_and(|r| r.status().is_success())
            };
            if ready {
                result.info["startup_ms"] = json!(t.elapsed().as_millis());
                return Ok(Some(result));
            }
            if t.elapsed() > Duration::from_secs(15) {
                bail!(
                    "Engine startup timed out; log: {}",
                    work.root.join("engine.log").display()
                );
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}
pub fn worker(path: &std::path::Path) -> Result<()> {
    let _worker: Worker = serde_json::from_slice(&std::fs::read(path)?)?;
    std::fs::remove_file(path)?;
    #[cfg(target_os = "linux")]
    {
        crate::sandbox::apply(&worker.profile, &worker.executable)?;
        use std::os::unix::process::CommandExt;
        let e = std::process::Command::new(&worker.executable)
            .args(&worker.arguments)
            .current_dir(&worker.workdir)
            .env_clear()
            .env("HOME", &worker.workdir)
            .env("TMPDIR", &worker.workdir)
            .env("PATH", "/usr/bin:/bin")
            .exec();
        Err(anyhow!("Engine exec: {e}"))
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;
        let mut policy = String::from(
            "(version 1)(deny default)(allow process-exec process-fork sysctl-read mach-lookup)",
        );
        for p in &worker.profile.read {
            policy.push_str(&format!(
                "(allow file-read* (subpath {}))",
                serde_json::to_string(&p.to_string_lossy())?
            ));
        }
        policy.push_str(&format!(
            "(allow file-read* (literal {}))",
            serde_json::to_string(&worker.executable.to_string_lossy())?
        ));
        for p in &worker.profile.write {
            policy.push_str(&format!(
                "(allow file-read* file-write* (subpath {}))",
                serde_json::to_string(&p.to_string_lossy())?
            ));
        }
        if worker.profile.network == "all" {
            policy.push_str("(allow network*)");
        } else {
            policy.push_str("(allow network-bind network-inbound)");
        }
        let e = std::process::Command::new("/usr/bin/sandbox-exec")
            .args(["-p", &policy])
            .arg(&worker.executable)
            .args(&worker.arguments)
            .exec();
        Err(anyhow!("Seatbelt exec: {e}"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        bail!("Unsupported sandbox platform")
    }
}
fn extract_moli(bytes: &[u8], dir: &std::path::Path) -> Result<Value> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let pin: Value = serde_json::from_str(include_str!("../docs/engine-pin.json"))?;
    let hash = format!("{:x}", Sha256::digest(bytes));
    if hash != pin["sha256"].as_str().unwrap() {
        bail!("Engine archive checksum mismatch");
    }
    std::fs::create_dir_all(dir)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    for entry in archive.entries()? {
        let mut e = entry?;
        let p = e.path()?.into_owned();
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if [
            "moli",
            "LICENSE-MIT",
            "LICENSE-APACHE",
            "license-metadata.json",
            "Chromium-BSD-3-Clause.txt",
            "Selenium-NOTICE.txt",
        ]
        .contains(&name)
            && e.header().entry_type().is_file()
        {
            let mut data = Vec::new();
            e.read_to_end(&mut data)?;
            if name == "moli"
                && format!("{:x}", Sha256::digest(&data))
                    != pin["executable_sha256"].as_str().unwrap()
            {
                bail!("Extracted engine checksum mismatch");
            }
            let temp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
            use std::io::Write;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(if name == "moli" { 0o700 } else { 0o600 });
            }
            let mut file = options.open(&temp)?;
            file.write_all(&data)?;
            file.sync_all()?;
            std::fs::rename(temp, dir.join(name))?;
        }
    }
    Ok(
        json!({"path":dir.join("moli"),"version":pin["version"],"archive_sha256":hash,"bytes":std::fs::metadata(dir.join("moli"))?.len()}),
    )
}
#[cfg(feature = "bundled-moli")]
fn materialize_bundled() -> Result<PathBuf> {
    use sha2::{Digest, Sha256};
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        bail!("Bundled engine is Linux x86_64 only");
    }
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME unset"))?;
    let dir = PathBuf::from(home).join(".cache/aerobrowser/moli-v1.1.12");
    let path = dir.join("moli");
    let pin: Value = serde_json::from_str(include_str!("../docs/engine-pin.json"))?;
    if let Ok(bytes) = std::fs::read(&path) {
        if format!("{:x}", Sha256::digest(bytes)) == pin["executable_sha256"].as_str().unwrap() {
            return Ok(path);
        }
    }
    let archive = include_bytes!(concat!(env!("OUT_DIR"), "/moli.tar.gz"));
    extract_moli(archive, &dir)?;
    Ok(path)
}
#[cfg(not(feature = "bundled-moli"))]
fn materialize_bundled() -> Result<PathBuf> {
    bail!("This build has no bundled engine")
}
pub async fn install_moli() -> Result<Value> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        bail!("Pinned installer supports Linux x86_64; provide a verified --engine-path on other targets");
    }
    let pin: Value = serde_json::from_str(include_str!("../docs/engine-pin.json"))?;
    let bytes = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()?
        .get(pin["url"].as_str().unwrap())
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME unset"))?;
    let dir = PathBuf::from(home).join(".aero/engines/moli-v1.1.12");
    extract_moli(&bytes, &dir)
}
