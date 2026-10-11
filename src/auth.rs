use anyhow::{bail, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::Path};
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub id: String,
    pub token_sha256: String,
    #[serde(default)]
    pub admin: bool,
    #[serde(default = "quota")]
    pub max_sessions: usize,
}
fn quota() -> usize {
    8
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    principals: Vec<Principal>,
}
#[derive(Clone)]
pub struct Auth {
    principals: Vec<Principal>,
}
impl Auth {
    pub fn load(token: Option<String>, path: Option<&Path>) -> Result<Self> {
        let mut principals = if let Some(path) = path {
            let b = std::fs::read(path)?;
            if b.len() > 65536 {
                bail!("Auth file exceeds 64 KiB");
            }
            serde_json::from_slice::<File>(&b)?.principals
        } else {
            vec![]
        };
        if let Some(t) = token {
            if t.len() < 24 {
                bail!("DELTA_TOKEN must contain at least 24 characters");
            }
            principals.push(Principal {
                id: "operator".into(),
                token_sha256: format!("{:x}", Sha256::digest(t.as_bytes())),
                admin: true,
                max_sessions: 32,
            });
        }
        if principals.len() > 64 {
            bail!("At most 64 principals allowed");
        }
        if principals.is_empty() {
            bail!("Provide DELTA_TOKEN or DELTA_AUTH_FILE");
        }
        let mut ids = HashSet::new();
        let mut hashes = HashSet::new();
        for p in &principals {
            if p.id.is_empty()
                || p.id.len() > 64
                || !p
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
                || !ids.insert(p.id.clone())
            {
                bail!("Invalid/duplicate principal id");
            }
            if p.token_sha256.len() != 64
                || !p
                    .token_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || !hashes.insert(p.token_sha256.clone())
            {
                bail!("Invalid/duplicate SHA-256 token digest");
            }
            if p.max_sessions == 0 || p.max_sessions > 32 {
                bail!("max_sessions must be 1..32");
            }
        }
        Ok(Self { principals })
    }
    pub fn authenticate(&self, bearer: &str) -> Option<Principal> {
        if !(24..=4096).contains(&bearer.len()) {
            return None;
        }
        let hash = format!("{:x}", Sha256::digest(bearer.as_bytes()));
        let mut found = None;
        for p in &self.principals {
            let mismatch = hash
                .bytes()
                .zip(p.token_sha256.bytes())
                .fold(0, |v, (a, b)| v | (a ^ b));
            if mismatch == 0 {
                found = Some(p.clone());
            }
        }
        found
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn token_auth() {
        let a = Auth::load(Some("a-valid-test-token-long-enough".into()), None).unwrap();
        assert!(
            a.authenticate("a-valid-test-token-long-enough")
                .unwrap()
                .admin
        );
        assert!(a
            .authenticate("a-different-test-token-long-enough")
            .is_none());
        assert!(Auth::load(Some("short".into()), None).is_err());
    }
}
