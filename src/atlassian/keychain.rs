//! Thin wrappers over the macOS `security` CLI.
//!
//! Mirrors the pattern used by `lark secret set` in `main.rs` so behaviour
//! is identical: add-generic-password with `-U` (upsert), find-generic-password
//! with `-w` (print the value), delete-generic-password.
//!
//! Non-macOS platforms return `Err` consistently — the plugin-side `lib.lua`
//! checks keychain availability and surfaces a clear "macOS-only" error to the
//! user, who can still use the API-token auth path on Linux / Windows.

use anyhow::{Context, Result, bail};

/// Keychain service names for Atlassian OAuth state.
pub const ATLASSIAN_REFRESH_TOKEN: &str = "ATLASSIAN_REFRESH_TOKEN";
pub const ATLASSIAN_CLOUDID: &str = "ATLASSIAN_CLOUDID";
pub const ATLASSIAN_ACCOUNT_EMAIL: &str = "ATLASSIAN_ACCOUNT_EMAIL";
/// Human-facing URL of the Atlassian site (e.g. `https://acme.atlassian.net`).
/// Distinct from the `/ex/jira/<cloudid>` proxy URL used for API calls: this
/// is what plugins hand to the browser for "Open in browser" actions.
pub const ATLASSIAN_SITE_URL: &str = "ATLASSIAN_SITE_URL";

pub const ALL_KEYS: [&str; 4] = [
    ATLASSIAN_REFRESH_TOKEN,
    ATLASSIAN_CLOUDID,
    ATLASSIAN_ACCOUNT_EMAIL,
    ATLASSIAN_SITE_URL,
];

/// Store `value` at `service`, upserting if present. macOS-only.
pub fn put(service: &str, value: &str) -> Result<()> {
    ensure_macos()?;
    let user = std::env::var("USER").unwrap_or_default();
    let status = std::process::Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-a",
            &user,
            "-s",
            service,
            "-w",
            value,
        ])
        .status()
        .context("failed to run `security add-generic-password`")?;
    if !status.success() {
        bail!("security add-generic-password for {service} exited {status}");
    }
    Ok(())
}

/// Retrieve the value stored at `service`. macOS-only. Returns `Ok(None)` when
/// the entry does not exist (this is not treated as an error).
pub fn get(service: &str) -> Result<Option<String>> {
    if !cfg!(target_os = "macos") {
        return Ok(None);
    }
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", service, "-w"])
        .output()
        .context("failed to run `security find-generic-password`")?;
    if !output.status.success() {
        // Not found is the primary failure mode; don't treat as error.
        return Ok(None);
    }
    let value = String::from_utf8(output.stdout)
        .context("keychain value was not UTF-8")?
        .trim_end_matches(['\n', '\r'])
        .to_string();
    Ok(Some(value))
}

/// Delete the entry at `service`. macOS-only + idempotent; a missing entry is
/// treated as successfully deleted, matching the semantics the callers want.
pub fn delete(service: &str) {
    if !cfg!(target_os = "macos") {
        return;
    }
    // Intentionally discard exit status — missing entries exit non-zero but
    // that's exactly the "idempotent delete" behaviour we want.
    let _ = std::process::Command::new("security")
        .args(["delete-generic-password", "-s", service])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn ensure_macos() -> Result<()> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        bail!(
            "Atlassian OAuth requires macOS Keychain. On Linux/Windows, use API-token auth: \
             set ATLASSIAN_EMAIL + ATLASSIAN_API_TOKEN via `lark secret set` and set \
             atlassian_host in plugin settings."
        )
    }
}
