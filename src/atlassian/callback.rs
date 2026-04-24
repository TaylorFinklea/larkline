//! Minimal one-shot HTTP listener for the OAuth redirect URI.
//!
//! The Atlassian authorize flow sends the user's browser to
//! `http://127.0.0.1:<port>/callback?code=...&state=...`. All we need to do is
//! accept one TCP connection, pull the query string off the request line,
//! respond with a small static HTML page, and return the `code` + `state` back
//! to the caller.
//!
//! Rolling this by hand is cheaper than pulling in `tiny_http` or wiring up
//! `hyper::server`: the surface is a single GET, no routing, no keep-alive,
//! no body parsing. Prior art: `gh auth login`, `flyctl auth login`, `aws sso login`.

use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const SUCCESS_HTML: &str = concat!(
    "<!DOCTYPE html><html><head><meta charset=\"utf-8\">",
    "<title>larkline</title>",
    "<style>body{font-family:system-ui,-apple-system,sans-serif;",
    "max-width:480px;margin:80px auto;padding:0 24px;color:#2f3e46;}",
    "h1{font-size:22px;margin:0 0 12px;}p{color:#627279;}</style></head>",
    "<body><h1>🔐 Signed in to Atlassian</h1>",
    "<p>You can close this tab and return to your terminal.</p>",
    "</body></html>"
);

const ERROR_HTML: &str = concat!(
    "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>larkline</title></head>",
    "<body><h1>Authorization failed</h1>",
    "<p>Close this tab and check the terminal for details.</p></body></html>"
);

/// Bind `127.0.0.1:0` and return the listener plus the ephemeral port the OS
/// assigned. Used by the login flow to build the `redirect_uri` before opening
/// the browser.
pub async fn bind_local() -> Result<(TcpListener, u16)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("binding local OAuth callback port")?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

/// Block waiting for the authorize redirect. Returns the `code` query parameter
/// after verifying that `state` matches what we sent.
pub async fn wait_for_code(listener: TcpListener, expected_state: &str) -> Result<String> {
    let (mut stream, _peer) = listener.accept().await.context("accepting callback")?;

    // Read just enough bytes to see the end of the request headers. We never read
    // the body — OAuth callbacks are always GETs.
    let mut buf = vec![0u8; 8192];
    let mut read = 0;
    loop {
        if read == buf.len() {
            bail!("OAuth callback request too large");
        }
        let n = stream.read(&mut buf[read..]).await?;
        if n == 0 {
            break;
        }
        read += n;
        if buf[..read].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let req = std::str::from_utf8(&buf[..read]).context("callback request was not UTF-8")?;
    let first_line = req.lines().next().unwrap_or("");

    // Parse "GET /callback?code=...&state=... HTTP/1.1"
    let path = first_line.split_whitespace().nth(1).unwrap_or("");
    let query = path.split_once('?').map_or("", |(_, q)| q);
    let params = parse_query(query);

    let write_response = |status: &str, html: &str| {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{html}",
            html.len()
        )
    };

    // OAuth authorization servers can send back `error=...&error_description=...`
    // instead of `code` when the user cancels or consent fails.
    if let Some(err) = params.get("error") {
        let _ = stream
            .write_all(write_response("400 Bad Request", ERROR_HTML).as_bytes())
            .await;
        let desc = params.get("error_description").cloned().unwrap_or_default();
        bail!(
            "Atlassian returned OAuth error: {err}{}",
            if desc.is_empty() {
                String::new()
            } else {
                format!(" — {desc}")
            }
        );
    }

    let returned_state = params.get("state").map(String::as_str).unwrap_or_default();
    if returned_state != expected_state {
        let _ = stream
            .write_all(write_response("400 Bad Request", ERROR_HTML).as_bytes())
            .await;
        bail!("state mismatch in OAuth callback — possible CSRF");
    }

    let code = params
        .get("code")
        .cloned()
        .context("OAuth callback missing `code` parameter")?;

    let _ = stream
        .write_all(write_response("200 OK", SUCCESS_HTML).as_bytes())
        .await;
    let _ = stream.shutdown().await;

    Ok(code)
}

/// Parse `k1=v1&k2=v2` with percent-decoding. Duplicates keep the last value.
fn parse_query(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        out.insert(percent_decode(k), percent_decode(v));
    }
    out
}

/// Standard x-www-form-urlencoded percent-decode. Converts `+` to space and
/// `%XX` hex pairs to their byte value. Malformed `%XX` sequences pass through
/// literally — fail-soft is fine here because the caller validates `state` and
/// the auth server sends well-formed URLs in practice.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let Ok(byte) =
                    u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
                {
                    out.push(byte);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_query_handles_code_and_state() {
        let q = parse_query("code=abc&state=xyz");
        assert_eq!(q.get("code").map(String::as_str), Some("abc"));
        assert_eq!(q.get("state").map(String::as_str), Some("xyz"));
    }

    #[test]
    fn parse_query_percent_decodes() {
        let q = parse_query("code=a%20b&state=x%3Dy");
        assert_eq!(q.get("code").map(String::as_str), Some("a b"));
        assert_eq!(q.get("state").map(String::as_str), Some("x=y"));
    }

    #[test]
    fn parse_query_handles_error_response() {
        let q = parse_query("error=access_denied&error_description=User%20declined");
        assert_eq!(q.get("error").map(String::as_str), Some("access_denied"));
        assert_eq!(
            q.get("error_description").map(String::as_str),
            Some("User declined")
        );
    }
}
