//! Access-token cache file at `$XDG_CACHE_HOME/larkline/atlassian-access.json`.
//!
//! Refresh tokens are sensitive and long-lived — they stay in Keychain.
//! Access tokens are short-lived (Atlassian defaults to 1 hour) and reconstructible
//! from a refresh call — they live in a plaintext-but-0600 cache file so we don't
//! pay a Keychain round-trip on every plugin invocation.
//!
//! Format:
//! ```json
//! {
//!   "access_token": "eyJ...",
//!   "expires_at":   1713866722,
//!   "cloudid":      "abc-123",
//!   "email":        "taylor@example.com"
//! }
//! ```
//!
//! `expires_at` is a Unix timestamp in seconds, not an ISO-8601 string — simpler
//! comparison, no date-parsing crate needed.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Consider a token stale this many seconds before its real expiry. Buys us a
/// round-trip buffer when the network is slow and protects against minor clock
/// skew between the local machine and Atlassian's auth server.
const EXPIRY_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cached {
    pub access_token: String,
    /// Unix timestamp (seconds since epoch) when the token becomes unusable.
    pub expires_at: i64,
    pub cloudid: String,
    pub email: String,
    /// Human-facing site URL (e.g. `https://acme.atlassian.net`). Missing on
    /// caches written by Phase B — `Option` keeps old JSON forward-compatible.
    #[serde(default)]
    pub site_url: String,
}

impl Cached {
    /// Return true when the cache is usable without a refresh call.
    pub fn is_fresh(&self) -> bool {
        let now = now_unix();
        self.expires_at.saturating_sub(EXPIRY_SKEW_SECONDS) > now
    }
}

/// Resolve the cache file path, creating the parent directory if needed.
pub fn cache_file_path() -> Result<PathBuf> {
    let base = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".cache")
    };
    let dir = base.join("larkline");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir.join("atlassian-access.json"))
}

/// Read the cache from disk. Returns `Err` when the file is missing or malformed.
pub fn read() -> Result<Cached> {
    let path = cache_file_path()?;
    let raw =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).context("parsing atlassian access-token cache")
}

/// Write the cache to disk with mode 0600 (Unix).
pub fn write(cached: &Cached) -> Result<()> {
    let path = cache_file_path()?;
    let raw = serde_json::to_string_pretty(cached)?;
    write_private(&path, &raw)
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, content: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("opening {} for write", path.display()))?;
    // `.mode(0o600)` only applies when O_CREAT actually creates the file. If
    // the cache file already exists with looser permissions (pre-created by
    // another process, a prior build, or a restored backup), re-assert 0600
    // so the live access token it holds isn't left world/group-readable.
    f.set_permissions(std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("tightening permissions on {}", path.display()))?;
    f.write_all(content.as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, content: &str) -> Result<()> {
    std::fs::write(path, content)?;
    Ok(())
}

/// Unix time in seconds; 0 on impossible pre-epoch clock states. Clamps to
/// `i64::MAX` on the theoretical year-2554 overflow (harmless — still "future").
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_fresh_honors_60s_skew() {
        let now = now_unix();
        // Expires in 30s — under the 60s skew window; must be stale.
        let stale = Cached {
            access_token: "a".into(),
            expires_at: now + 30,
            cloudid: "c".into(),
            email: "e@e.com".into(),
            site_url: "https://example.atlassian.net".into(),
        };
        assert!(!stale.is_fresh(), "30s-to-expiry must not be fresh");

        // Expires in 120s — comfortably ahead; must be fresh.
        let fresh = Cached {
            access_token: "a".into(),
            expires_at: now + 120,
            cloudid: "c".into(),
            email: "e@e.com".into(),
            site_url: "https://example.atlassian.net".into(),
        };
        assert!(fresh.is_fresh(), "120s-to-expiry must be fresh");

        // Already expired.
        let expired = Cached {
            access_token: "a".into(),
            expires_at: now - 10,
            cloudid: "c".into(),
            email: "e@e.com".into(),
            site_url: "https://example.atlassian.net".into(),
        };
        assert!(!expired.is_fresh(), "expired cache must not be fresh");
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_0600_perms() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().expect("tempdir");
        // Override XDG_CACHE_HOME so we write inside the temp dir.
        // SAFETY: test runs single-threaded via tokio test harness is not used here;
        // cargo test's default parallelism can race on env vars, so we mutate the
        // path directly rather than through the helper.
        let path = tmp.path().join("atlassian-access.json");
        let cached = Cached {
            access_token: "secret".into(),
            expires_at: now_unix() + 3600,
            cloudid: "c".into(),
            email: "e@e.com".into(),
            site_url: "https://example.atlassian.net".into(),
        };
        let raw = serde_json::to_string_pretty(&cached).unwrap();
        super::write_private(&path, &raw).expect("write");
        let meta = std::fs::metadata(&path).expect("metadata");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "cache file must be 0600, got {mode:o}");
    }

    #[cfg(unix)]
    #[test]
    fn write_tightens_preexisting_loose_perms() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("atlassian-access.json");
        // Pre-create the file world-readable (the hijack/leak scenario).
        std::fs::write(&path, "stale").expect("pre-create");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");

        let cached = Cached {
            access_token: "secret".into(),
            expires_at: now_unix() + 3600,
            cloudid: "c".into(),
            email: "e@e.com".into(),
            site_url: "https://example.atlassian.net".into(),
        };
        let raw = serde_json::to_string_pretty(&cached).unwrap();
        super::write_private(&path, &raw).expect("write");
        let mode = std::fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "pre-existing file must be tightened to 0600, got {mode:o}");
    }
}
