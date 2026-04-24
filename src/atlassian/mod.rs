//! `lark atlassian` subcommand — OAuth 2.0 (3LO) auth flow for the Atlassian plugin.
//!
//! The Atlassian plugin supports two auth paths:
//!
//! 1. **API token** — user sets `ATLASSIAN_EMAIL` + `ATLASSIAN_API_TOKEN` + `atlassian_host`.
//!    No subsystem needed; `lib.lua` builds a Basic auth header directly.
//! 2. **OAuth 2.0 with PKCE** — user runs `lark atlassian login` once; refresh token
//!    stays in Keychain; short-lived access tokens are cached in `~/.cache/larkline/`.
//!    Plugins call `lark atlassian token` to get a fresh access token on each invocation.
//!
//! All state lives in Rust. Plugins never handle refresh.
//!
//! Subcommands:
//!
//! | Command | Purpose |
//! |---|---|
//! | `lark atlassian login [--email X]` | Interactive OAuth flow; opens browser. |
//! | `lark atlassian token` | Print a valid access token (refreshes silently). Exit 1 + empty stdout when not signed in. |
//! | `lark atlassian cloudid` | Print the cloud id from Keychain. |
//! | `lark atlassian status` | Human-readable state summary. |
//! | `lark atlassian logout` | Delete all persisted auth state. |

use anyhow::Result;

pub mod cache;
pub mod callback;
pub mod keychain;
pub mod oauth;

/// Dispatch `lark atlassian <sub>` invocations.
pub async fn handle_command(args: &[String]) -> Result<()> {
    let sub = args.first().map(String::as_str);
    match sub {
        Some("login") => oauth::login_command(&args[1..]).await,
        Some("token") => oauth::token_command().await,
        Some("cloudid") => cloudid_command(),
        Some("site") => site_command(),
        Some("status") => status_command(),
        Some("logout") => logout_command(),
        Some(other) => {
            eprintln!("lark atlassian: unknown subcommand `{other}`");
            print_usage();
            std::process::exit(2);
        }
        None => {
            print_usage();
            Ok(())
        }
    }
}

fn print_usage() {
    println!(
        "lark atlassian — OAuth for the Atlassian plugin\n\n\
         USAGE:\n  \
           lark atlassian login [--email <addr>]    Authorize in a browser\n  \
           lark atlassian token                     Print a valid access token (refreshes if needed)\n  \
           lark atlassian cloudid                   Print the active Atlassian cloud id\n  \
           lark atlassian site                      Print the human-facing site URL (for browser links)\n  \
           lark atlassian status                    Show signed-in account + token state\n  \
           lark atlassian logout                    Delete all persisted auth state\n\n\
         Alternative: set ATLASSIAN_EMAIL + ATLASSIAN_API_TOKEN + atlassian_host setting for API-token auth."
    );
}

fn cloudid_command() -> Result<()> {
    match keychain::get(keychain::ATLASSIAN_CLOUDID)? {
        Some(cid) if !cid.is_empty() => {
            println!("{cid}");
            Ok(())
        }
        _ => {
            // Token-plugin dispatch checks for empty stdout + non-zero exit; both are
            // "not signed in" signals. Keep behaviour identical to `token` for consistency.
            std::process::exit(1);
        }
    }
}

/// Print the human-facing site URL (e.g. `https://acme.atlassian.net`) so plugins
/// can build browser URLs without knowing the cloudid-proxy convention. Falls
/// back to the cache file when the Keychain entry is missing (covers caches
/// written by Phase B, which predate `ATLASSIAN_SITE_URL`).
fn site_command() -> Result<()> {
    if let Some(url) = keychain::get(keychain::ATLASSIAN_SITE_URL)? {
        if !url.is_empty() {
            println!("{url}");
            return Ok(());
        }
    }
    if let Ok(c) = cache::read() {
        if !c.site_url.is_empty() {
            println!("{}", c.site_url);
            return Ok(());
        }
    }
    std::process::exit(1);
}

fn logout_command() -> Result<()> {
    let had_refresh =
        keychain::get(keychain::ATLASSIAN_REFRESH_TOKEN)?.is_some_and(|v| !v.is_empty());
    for key in keychain::ALL_KEYS {
        keychain::delete(key);
    }
    let cache_path = cache::cache_file_path()?;
    let had_cache = cache_path.exists();
    if had_cache {
        std::fs::remove_file(&cache_path).ok();
    }
    if had_refresh || had_cache {
        println!("Signed out of Atlassian.");
    } else {
        println!("Not signed in — nothing to remove.");
    }
    Ok(())
}

fn status_command() -> Result<()> {
    let cid = keychain::get(keychain::ATLASSIAN_CLOUDID)?.unwrap_or_default();
    let email = keychain::get(keychain::ATLASSIAN_ACCOUNT_EMAIL)?.unwrap_or_default();
    let has_refresh =
        keychain::get(keychain::ATLASSIAN_REFRESH_TOKEN)?.is_some_and(|v| !v.is_empty());

    if !has_refresh || cid.is_empty() {
        println!("Not signed in. Run `lark atlassian login` to authorize.");
        return Ok(());
    }

    println!("Signed in to Atlassian");
    if !email.is_empty() {
        println!("  Account:  {email}");
    }
    println!("  Cloud id: {cid}");

    // Report on the access-token cache without forcing a refresh.
    if let Ok(c) = cache::read() {
        let now = chrono_now();
        let expires_in = c.expires_at - now;
        if expires_in > 0 {
            println!(
                "  Access token: valid for {}m {}s",
                expires_in / 60,
                expires_in % 60
            );
        } else {
            println!("  Access token: expired — will refresh on next `lark atlassian token`");
        }
    } else {
        println!("  Access token: no cache — will fetch on next `lark atlassian token`");
    }

    Ok(())
}

// Unix timestamp helper. Used by status for a rough "valid for Nm" display.
// Reuses the cache module's clock so behaviour is consistent across commands.
fn chrono_now() -> i64 {
    cache::now_unix()
}
