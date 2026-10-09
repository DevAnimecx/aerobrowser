use anyhow::{bail, Result};
use serde_json::Value;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
/// Session-private artifact names only. No caller-controlled directory traversal.
pub struct Artifacts {
    pub root: PathBuf,
}
impl Artifacts {
    pub fn new() -> Result<Self> {
        let base = std::env::var_os("AERO_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("aero-artifacts"));
        std::fs::create_dir_all(&base)?;
        let root = base.join(uuid::Uuid::new_v4().to_string());
        let d = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            d.mode(0o700);
        }
        d.create(&root)?;
        Ok(Self { root })
    }
    pub fn name(name: &str) -> Result<&str> {
        if name.is_empty()
            || name.len() > 128
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            || name == "."
            || name == ".."
        {
            bail!("Artifact path must be a plain filename, not an absolute path or directory");
        }
        Ok(name)
    }
    pub fn write(&self, name: &str, bytes: &[u8]) -> Result<PathBuf> {
        Self::name(name)?;
        if bytes.len() > 32 * 1024 * 1024 {
            bail!("Artifact exceeds 32 MiB");
        }
        let temp = self.root.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        let path = self.root.join(name);
        std::fs::rename(temp, &path)?;
        Ok(path)
    }
    pub fn read_json(&self, name: &str) -> Result<Value> {
        Self::name(name)?;
        let path = self.root.join(name);
        if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            bail!("Symlink artifacts forbidden");
        }
        let bytes = std::fs::read(path)?;
        if bytes.len() > 16 * 1024 * 1024 {
            bail!("State too large");
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub fn upload(path: &str) -> Result<PathBuf> {
        let root = std::env::var_os("AERO_UPLOAD_ROOT").ok_or_else(|| {
            anyhow::anyhow!(
                "Uploads disabled: set AERO_UPLOAD_ROOT to an explicitly authorized directory"
            )
        })?;
        let root = PathBuf::from(root).canonicalize()?;
        let relative = Path::new(path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            bail!("Upload must be relative to AERO_UPLOAD_ROOT");
        }
        let p = root.join(relative).canonicalize()?;
        if !p.starts_with(&root) || !p.is_file() {
            bail!("Upload outside authorized root");
        }
        Ok(p)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_paths() {
        for p in ["../secret", "/etc/passwd", "a/b", "..", ""] {
            assert!(Artifacts::name(p).is_err());
        }
        assert!(Artifacts::name("state.json").is_ok());
    }
}
