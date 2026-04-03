//! Background update checker — detects new releases via the GitHub API
//! and caches the result so we only check once per day.

use std::path::PathBuf;

/// Result of an update check, persisted to disk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateCache {
    /// Latest version available (e.g. "0.5.0").
    pub latest_version: String,
    /// Unix timestamp of the last check.
    pub checked_at: u64,
}

/// How larkline was installed, detected from the binary path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstallMethod {
    Homebrew,
    Cargo,
    #[default]
    Unknown,
}

impl InstallMethod {
    /// Human-readable upgrade command.
    #[must_use]
    pub fn upgrade_hint(self) -> &'static str {
        match self {
            Self::Homebrew => "brew upgrade larkline",
            Self::Cargo => "cargo install larkline",
            Self::Unknown => "brew upgrade larkline  or  cargo install larkline",
        }
    }
}

/// Detect how larkline was installed by inspecting the current executable path.
#[must_use]
pub fn detect_install_method() -> InstallMethod {
    let Ok(exe) = std::env::current_exe() else {
        return InstallMethod::Unknown;
    };
    let path = exe.to_string_lossy();

    if path.contains("/Cellar/") || path.contains("/opt/homebrew/") || path.contains("/usr/local/")
    {
        InstallMethod::Homebrew
    } else if path.contains("/.cargo/bin/") {
        InstallMethod::Cargo
    } else {
        InstallMethod::Unknown
    }
}

/// Path to the update-check cache file.
fn cache_path() -> PathBuf {
    let base = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("larkline")
    } else {
        let home = std::env::var("HOME").map_or_else(|_| PathBuf::from("/tmp"), PathBuf::from);
        home.join(".local").join("share").join("larkline")
    };
    base.join("update-check.json")
}

/// Load cached update info. Returns `None` if missing or corrupt.
#[must_use]
pub fn load_cache() -> Option<UpdateCache> {
    let contents = std::fs::read_to_string(cache_path()).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Save update cache to disk.
fn save_cache(cache: &UpdateCache) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

/// Check if the cache is fresh (less than 24 hours old).
#[must_use]
pub fn cache_is_fresh(cache: &UpdateCache) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(cache.checked_at) < 86400 // 24 hours
}

/// Returns `Some("x.y.z")` if a newer version is available, using cached data.
/// Does not make network requests — call `check_for_update` for that.
#[must_use]
pub fn cached_update_available() -> Option<String> {
    let cache = load_cache()?;
    let current = env!("CARGO_PKG_VERSION");
    if is_newer(&cache.latest_version, current) {
        Some(cache.latest_version)
    } else {
        None
    }
}

/// Async: fetch the latest release tag from GitHub and update the cache.
/// Returns `Some(version)` if newer than current, `None` otherwise.
pub async fn check_for_update() -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("larkline/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let resp = client
        .get("https://api.github.com/repos/TaylorFinklea/larkline/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let body: serde_json::Value = resp.json().await.ok()?;
    let tag = body.get("tag_name")?.as_str()?;
    let latest = tag.strip_prefix('v').unwrap_or(tag);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    save_cache(&UpdateCache {
        latest_version: latest.to_string(),
        checked_at: now,
    });

    let current = env!("CARGO_PKG_VERSION");
    if is_newer(latest, current) {
        Some(latest.to_string())
    } else {
        None
    }
}

/// Compare two semver strings. Returns true if `latest` is strictly newer than `current`.
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> (u32, u32, u32) {
        let mut parts = s.split('.');
        let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };
    parse(latest) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_basic() {
        assert!(is_newer("0.5.0", "0.4.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.4.1", "0.4.0"));
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("0.3.0", "0.4.0"));
    }

    #[test]
    fn detect_install_method_returns_something() {
        // Just ensure it doesn't panic — actual value depends on env.
        let _ = detect_install_method();
    }

    #[test]
    fn cache_freshness() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let fresh = UpdateCache {
            latest_version: "1.0.0".into(),
            checked_at: now,
        };
        assert!(cache_is_fresh(&fresh));

        let stale = UpdateCache {
            latest_version: "1.0.0".into(),
            checked_at: now - 90000,
        };
        assert!(!cache_is_fresh(&stale));
    }
}
