//! Atlassian OAuth 2.0 (3LO) with PKCE.
//!
//! <https://developer.atlassian.com/cloud/jira/platform/oauth-2-3lo-apps/>
//!
//! Public client — no secret baked into the binary. PKCE S256 proves that the
//! client holding the authorization code is the same one that started the flow.
//!
//! The client id is compiled in. Users who want to self-host an OAuth app can
//! override via `LARKLINE_ATLASSIAN_CLIENT_ID` at runtime.

use anyhow::{Context, Result, bail};
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{cache, callback, keychain};

/// Atlassian-assigned client id for the larkline OAuth app.
///
/// Replace before first publish — registration gate at
/// <https://developer.atlassian.com/console/myapps/>.
const BAKED_CLIENT_ID: &str = "REPLACE_WITH_REAL_CLIENT_ID";

/// Runtime override env var. Lets users register their own app instead of
/// trusting the baked client id.
const CLIENT_ID_OVERRIDE_ENV: &str = "LARKLINE_ATLASSIAN_CLIENT_ID";

/// Authorize URL (browser-facing).
const AUTHORIZE_URL: &str = "https://auth.atlassian.com/authorize";

/// Token exchange + refresh URL.
const TOKEN_URL: &str = "https://auth.atlassian.com/oauth/token";

/// Discovery endpoint that returns the array of Atlassian cloud sites the
/// signed-in user has access to.
const ACCESSIBLE_RESOURCES_URL: &str = "https://api.atlassian.com/oauth/token/accessible-resources";

/// Scope set requested at first consent. All upfront — rescoping would force
/// the user through the consent screen again.
const SCOPES: &[&str] = &[
    "read:jira-work",
    "read:jira-user",
    "write:jira-work",
    "read:confluence-content.all",
    "read:confluence-user",
    "write:confluence-content",
    "offline_access",
];

/// Resolve the OAuth client id at runtime.
fn client_id() -> String {
    std::env::var(CLIENT_ID_OVERRIDE_ENV).unwrap_or_else(|_| BAKED_CLIENT_ID.to_string())
}

/// URL-safe base64 encoding without padding, as required by PKCE (RFC 7636 §4.2).
fn base64url_no_pad(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Generate a PKCE verifier + S256 challenge pair.
pub fn pkce_pair() -> (String, String) {
    let verifier_bytes: [u8; 32] = rand::random();
    let verifier = base64url_no_pad(&verifier_bytes);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = base64url_no_pad(&hasher.finalize());
    (verifier, challenge)
}

/// Random 32-byte base64url token used for the CSRF `state` parameter.
fn random_state() -> String {
    let bytes: [u8; 32] = rand::random();
    base64url_no_pad(&bytes)
}

/// Build the browser-facing authorize URL.
pub fn authorize_url(redirect_uri: &str, state: &str, code_challenge: &str) -> String {
    let scope = urlencode(&SCOPES.join(" "));
    format!(
        "{AUTHORIZE_URL}?\
         audience=api.atlassian.com&\
         client_id={cid}&\
         scope={scope}&\
         redirect_uri={redirect}&\
         state={state}&\
         response_type=code&\
         prompt=consent&\
         code_challenge={challenge}&\
         code_challenge_method=S256",
        cid = urlencode(&client_id()),
        scope = scope,
        redirect = urlencode(redirect_uri),
        state = urlencode(state),
        challenge = urlencode(code_challenge),
    )
}

fn urlencode(s: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            // write! into a String never fails — the Result is safe to discard.
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    /// Atlassian rotates refresh tokens on every call — always present in v2.
    refresh_token: Option<String>,
    /// Seconds until access token expires (typically 3600).
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct AccessibleResource {
    id: String,
    name: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    #[serde(default)]
    email: String,
}

/// Exchange an authorization code for a token pair.
async fn exchange_code(
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<TokenResponse> {
    let client = http_client()?;
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", &client_id()),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .context("POST /oauth/token (authorization_code)")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("Atlassian token exchange failed: {status} — {body}");
    }
    serde_json::from_str(&body).context("parsing token exchange response")
}

/// Exchange a refresh token for a fresh access token. Rotates the refresh token.
async fn exchange_refresh(refresh_token: &str) -> Result<TokenResponse> {
    let client = http_client()?;
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id()),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .context("POST /oauth/token (refresh_token)")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        // 400 invalid_grant is the "refresh revoked — re-login" signal. Surface
        // as a distinct error so the plugin can flag it to the user.
        if body.contains("invalid_grant") {
            bail!("Atlassian refresh token revoked. Run `lark atlassian login` again.");
        }
        bail!("Atlassian refresh failed: {status} — {body}");
    }
    serde_json::from_str(&body).context("parsing refresh response")
}

/// List the Atlassian cloud sites the signed-in user can access.
async fn accessible_resources(access_token: &str) -> Result<Vec<AccessibleResource>> {
    let client = http_client()?;
    let resp = client
        .get(ACCESSIBLE_RESOURCES_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .context("GET /oauth/token/accessible-resources")?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        bail!("accessible-resources failed: {status} — {body}");
    }
    resp.json().await.context("parsing accessible-resources")
}

/// Pull the user's email via the Atlassian `/me` endpoint (scoped to `read:jira-user`).
async fn fetch_email(access_token: &str, cloudid: &str) -> Option<String> {
    let client = http_client().ok()?;
    let url = format!("https://api.atlassian.com/ex/jira/{cloudid}/rest/api/3/myself");
    let resp = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let info: UserInfo = resp.json().await.ok()?;
    if info.email.is_empty() {
        None
    } else {
        Some(info.email)
    }
}

/// Construct a reqwest client with a consistent user-agent.
fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!(
            "larkline/",
            env!("CARGO_PKG_VERSION"),
            " (atlassian-oauth)"
        ))
        .build()
        .context("building reqwest client")
}

/// Select one of several Atlassian cloud sites interactively on stdin.
fn prompt_pick_resource(resources: &[AccessibleResource]) -> Result<usize> {
    use std::io::{BufRead, Write};
    eprintln!("You have access to multiple Atlassian sites:");
    for (i, r) in resources.iter().enumerate() {
        eprintln!("  [{}] {}  ({})", i + 1, r.name, r.url);
    }
    eprint!("Pick a site [1]: ");
    std::io::stderr().flush().ok();

    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    let n: usize = trimmed.parse().context("invalid selection")?;
    if n == 0 || n > resources.len() {
        bail!("selection out of range");
    }
    Ok(n - 1)
}

/// Implementation of `lark atlassian login`.
pub async fn login_command(_args: &[String]) -> Result<()> {
    if client_id() == BAKED_CLIENT_ID {
        eprintln!(
            "⚠  larkline ships without a baked OAuth client id yet. Register one at\n   \
             https://developer.atlassian.com/console/myapps/ and set\n   \
             LARKLINE_ATLASSIAN_CLIENT_ID=<your-id>, or use API-token auth instead."
        );
        bail!("OAuth client id not configured");
    }

    let (listener, port) = callback::bind_local().await?;
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let (verifier, challenge) = pkce_pair();
    let state = random_state();
    let url = authorize_url(&redirect_uri, &state, &challenge);

    eprintln!("Opening browser to authorize larkline with Atlassian…");
    eprintln!("If the browser doesn't open, visit this URL:\n  {url}");
    open_browser(&url);

    let code = callback::wait_for_code(listener, &state).await?;
    let tokens = exchange_code(&code, &redirect_uri, &verifier).await?;
    let refresh = tokens.refresh_token.as_deref().context(
        "Atlassian did not return a refresh_token — check scopes include offline_access",
    )?;

    let resources = accessible_resources(&tokens.access_token).await?;
    if resources.is_empty() {
        bail!(
            "Atlassian returned no accessible sites. Check your account has access to a Jira or Confluence instance."
        );
    }
    let picked = if resources.len() == 1 {
        0
    } else {
        prompt_pick_resource(&resources)?
    };
    let resource = &resources[picked];

    let email = fetch_email(&tokens.access_token, &resource.id)
        .await
        .unwrap_or_default();

    keychain::put(keychain::ATLASSIAN_REFRESH_TOKEN, refresh)?;
    keychain::put(keychain::ATLASSIAN_CLOUDID, &resource.id)?;
    keychain::put(keychain::ATLASSIAN_SITE_URL, &resource.url)?;
    if !email.is_empty() {
        keychain::put(keychain::ATLASSIAN_ACCOUNT_EMAIL, &email)?;
    }

    cache::write(&cache::Cached {
        access_token: tokens.access_token,
        expires_at: cache::now_unix() + tokens.expires_in,
        cloudid: resource.id.clone(),
        email: email.clone(),
        site_url: resource.url.clone(),
    })?;

    eprintln!();
    if email.is_empty() {
        eprintln!("✔ Signed in to {} ({})", resource.name, resource.url);
    } else {
        eprintln!(
            "✔ Signed in as {email} to {} ({})",
            resource.name, resource.url
        );
    }
    Ok(())
}

/// Implementation of `lark atlassian token`. Prints a valid access token to
/// stdout or exits 1 when not signed in.
pub async fn token_command() -> Result<()> {
    if let Ok(cached) = cache::read() {
        if cached.is_fresh() {
            println!("{}", cached.access_token);
            return Ok(());
        }
    }

    let refresh = match keychain::get(keychain::ATLASSIAN_REFRESH_TOKEN)? {
        Some(r) if !r.is_empty() => r,
        _ => {
            // Signal "not signed in" via empty stdout + exit 1. The plugin
            // dispatcher treats this as a distinct state from runtime errors.
            std::process::exit(1);
        }
    };

    let tokens = exchange_refresh(&refresh).await?;
    if let Some(new_refresh) = tokens.refresh_token.as_deref() {
        if new_refresh != refresh {
            keychain::put(keychain::ATLASSIAN_REFRESH_TOKEN, new_refresh)?;
        }
    }

    let cloudid = keychain::get(keychain::ATLASSIAN_CLOUDID)?.unwrap_or_default();
    let email = keychain::get(keychain::ATLASSIAN_ACCOUNT_EMAIL)?.unwrap_or_default();
    let site_url = keychain::get(keychain::ATLASSIAN_SITE_URL)?.unwrap_or_default();
    let cached = cache::Cached {
        access_token: tokens.access_token,
        expires_at: cache::now_unix() + tokens.expires_in,
        cloudid,
        email,
        site_url,
    };
    cache::write(&cached)?;
    println!("{}", cached.access_token);
    Ok(())
}

/// Spawn the platform `open` utility to launch the authorize URL. Best-effort —
/// failure is non-fatal because we also print the URL for manual fallback.
fn open_browser(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "start"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_sha256_of_verifier_base64url_no_pad() {
        let (verifier, challenge) = pkce_pair();
        assert_eq!(verifier.len(), 43, "32-byte base64url-no-pad is 43 chars");
        assert_eq!(challenge.len(), 43);
        // Reproduce the derivation manually and confirm it matches.
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let expected = base64url_no_pad(&hasher.finalize());
        assert_eq!(challenge, expected);
    }

    #[test]
    fn authorize_url_contains_state_and_pkce_challenge_method_s256() {
        let url = authorize_url("http://127.0.0.1:1234/callback", "STATE_X", "CHAL_Y");
        assert!(url.starts_with(AUTHORIZE_URL), "URL base");
        assert!(url.contains("state=STATE_X"), "state present");
        assert!(url.contains("code_challenge=CHAL_Y"), "challenge present");
        assert!(url.contains("code_challenge_method=S256"), "S256 method");
        assert!(url.contains("response_type=code"), "code grant");
        assert!(
            url.contains("audience=api.atlassian.com"),
            "Atlassian audience"
        );
        assert!(
            url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A1234%2Fcallback"),
            "redirect uri percent-encoded"
        );
    }

    #[test]
    fn authorize_url_requests_all_expected_scopes() {
        let url = authorize_url("http://x/c", "s", "c");
        for scope in SCOPES {
            let encoded = urlencode(scope);
            assert!(
                url.contains(&encoded),
                "expected scope `{scope}` in authorize URL"
            );
        }
    }

    #[test]
    fn random_state_is_unique_per_call() {
        let a = random_state();
        let b = random_state();
        assert_ne!(a, b, "state tokens must be distinct");
        assert_eq!(a.len(), 43);
    }
}
