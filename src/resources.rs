//! Real Linux cgroup-v2 enforcement. Never silently emulates a requested limit.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
pub struct Group {
    path: PathBuf,
}
impl Group {
    pub async fn cleanup(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }
        std::fs::write(self.path.join("cgroup.kill"), "1")?;
        for _ in 0..200 {
            match std::fs::remove_dir(&self.path) {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) if e.raw_os_error() == Some(libc::EBUSY) => {}
                Err(e) => return Err(e.into()),
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        bail!("Worker cgroup cleanup timed out: {}", self.path.display())
    }

    pub fn create(
        root: Option<&Path>,
        memory_mb: Option<u64>,
        pids: Option<u64>,
    ) -> Result<Option<Self>> {
        if memory_mb.is_none() && pids.is_none() {
            return Ok(None);
        }
        if !cfg!(target_os = "linux") {
            bail!("Hard resource limits require Linux cgroup v2");
        }
        let root=root.context("Requested hard limits require --cgroup-root / DELTA_CGROUP_ROOT (delegated cgroup v2 directory)")?.canonicalize()?;
        #[cfg(target_os = "linux")]
        unsafe {
            use std::os::unix::ffi::OsStrExt;
            let p = std::ffi::CString::new(root.as_os_str().as_bytes())?;
            let mut stat: libc::statfs = std::mem::zeroed();
            if libc::statfs(p.as_ptr(), &mut stat) != 0 || stat.f_type != 0x63677270 {
                bail!("Resource root is not a cgroup-v2 filesystem");
            }
        }
        let enabled = std::fs::read_to_string(root.join("cgroup.subtree_control"))?;
        for (name, needed) in [("memory", memory_mb.is_some()), ("pids", pids.is_some())] {
            if needed && !enabled.split_whitespace().any(|s| s == name) {
                bail!(
                    "Controller {name} is not delegated/enabled at {}",
                    root.display()
                );
            }
        }
        let path = root.join(format!("delta-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path)?;
        let g = Self { path };
        if let Some(mb) = memory_mb {
            if mb < 32 {
                bail!("Memory limit must be at least 32 MiB");
            }
            let bytes = mb.checked_mul(1048576).context("Memory limit overflow")?;
            g.set("memory.max", bytes)?;
            g.set("memory.oom.group", 1)?;
            if g.path.join("memory.swap.max").exists() {
                g.set("memory.swap.max", 0)?;
            }
        }
        if let Some(p) = pids {
            if p < 1 {
                bail!("PIDs limit must be positive");
            }
            g.set("pids.max", p)?;
        }
        Ok(Some(g))
    }
    fn set(&self, name: &str, value: u64) -> Result<()> {
        std::fs::write(self.path.join(name), value.to_string())
            .with_context(|| format!("Cannot enforce {name}"))
    }
    #[allow(dead_code)]
    pub fn enrollment(&self) -> Result<File> {
        Ok(std::fs::OpenOptions::new()
            .write(true)
            .open(self.path.join("cgroup.procs"))?)
    }
    pub fn stats(&self) -> Value {
        let number = |name: &str| {
            std::fs::read_to_string(self.path.join(name))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
        };
        let map = |name: &str| {
            let s = std::fs::read_to_string(self.path.join(name)).unwrap_or_default();
            let mut out = serde_json::Map::new();
            for line in s.lines() {
                let mut p = line.split_whitespace();
                if let (Some(k), Some(v)) = (p.next(), p.next()) {
                    if let Ok(n) = v.parse::<u64>() {
                        out.insert(k.into(), json!(n));
                    }
                }
            }
            Value::Object(out)
        };
        json!({"enforcement":"linux-cgroup-v2","path":self.path,"scope":"entire worker cgroup including descendants; excludes supervisor","memory_current_bytes":number("memory.current"),"memory_peak_bytes":number("memory.peak"),"memory_limit_bytes":number("memory.max"),"swap_limit_bytes":number("memory.swap.max"),"pids_current":number("pids.current"),"pids_limit":number("pids.max"),"memory_events":map("memory.events"),"pids_events":map("pids.events"),"cpu":map("cpu.stat"),"unavailable_or_unlimited_values_are_null":true})
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        if !self.path.exists() {
            return;
        }
        let _ = std::fs::write(self.path.join("cgroup.kill"), "1");
        if std::fs::remove_dir(&self.path).is_err() {
            let p = self.path.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    if std::fs::remove_dir(&p).is_ok() || !p.exists() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fail_closed() {
        assert!(Group::create(None, Some(128), None).is_err());
        assert!(Group::create(Some(Path::new("/tmp")), Some(128), None).is_err());
        assert!(Group::create(None, None, None).unwrap().is_none());
    }
}
