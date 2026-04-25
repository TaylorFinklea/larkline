# Current State

> Updated: 2026-04-24

## Active Branch

`main`

## Recent Progress

### v0.12.0 — Atlassian (Jira + Confluence) deep-dive

Six commits, all on `main`:

| Phase | Commit | Summary |
|---|---|---|
| A | `0922b57` | Plugin scaffold, API-token auth, my-issues, `lark.base64` host API |
| B | `ddf1403` | OAuth 2.0 (3LO + PKCE) Rust subsystem (`lark atlassian login/token/cloudid/site/status/logout`) |
| C | `fd31246` | Plugin lib.lua dispatches to OAuth when API-token absent; `LARK_BINARY` injection |
| D | `17930db` | Jira sprint, triage, new-issue (form), transition (chain), comment (chain) |
| E | `e83b2d7` | Confluence recent, search (CQL), my-pages, new-page (storage format) |
| F | (this) | Roadmap + handoff docs + CHANGELOG + `docs/plugins/atlassian.md` |

Both auth paths supported. API-token path is fully usable today. OAuth path is gated on registering a real OAuth app at developer.atlassian.com (placeholder client id baked in `src/atlassian/oauth.rs:22`); users can self-host via `LARKLINE_ATLASSIAN_CLIENT_ID` env override.

### v0.11.0 — Bitwarden plus follow-up fixes

Three commits accumulated since v0.11.0 was specced:

- `2932db3` — original Bitwarden deep-dive (6 commands)
- `7b4ee48` — stale-session preflight (`bw --response` silently returns empty data on a stale BW_SESSION)
- `9b84d6b` — `bw --response` envelope unwrap (the plugin was reading from the wrong layer of the discriminated envelope)
- `50cfa79` — `lark.json.decode` null-safety (mlua's default mapped JSON null to a truthy userdata sentinel; plugins that did `if x and x ~= "" then` silently leaked it)

The v0.10.0 follow-up `fa0eeff` (input-loop fix for Ghostty / Kitty-protocol) is also unreleased.

## Current Version

`Cargo.toml` at `0.10.0`. Three releases queued:

- v0.11.0 — Bitwarden + follow-up fixes
- v0.12.0 — Atlassian (this milestone)

Per Taylor's preference, both can ship as a single `0.12.0` cut since the v0.10.0 tag is the last actually released version.

## Validation

- `cargo test` — 185 passed (+9 atlassian unit tests since v0.10.0)
- `cargo clippy -- -D warnings` — clean (fixed 3 rust 1.95 lints during Phase A)
- `cargo fmt -- --check` — clean

## New Files This Session

| File | Purpose |
|------|---------|
| `src/atlassian/mod.rs` | `lark atlassian` subcommand dispatcher + `cloudid`/`site`/`status`/`logout` |
| `src/atlassian/oauth.rs` | PKCE, authorize URL, code exchange, refresh, accessible-resources |
| `src/atlassian/cache.rs` | `~/.cache/larkline/atlassian-access.json` (0600), 60s skew window |
| `src/atlassian/keychain.rs` | macOS `security` CLI wrappers for refresh + cloudid + email + site_url |
| `src/atlassian/callback.rs` | Hand-rolled one-shot OAuth callback HTTP listener |
| `examples/plugins/atlassian/manifest.toml` | 10 commands across Jira + Confluence |
| `examples/plugins/atlassian/lib.lua` | Canonical helpers (atlassian_auth, atlassian_get/post, ADF, etc.) |
| `examples/plugins/atlassian/{my-issues,sprint,triage,new-issue,transition,comment}.lua` | Jira commands |
| `examples/plugins/atlassian/confluence-{recent,search,my-pages,new-page}.lua` | Confluence commands |
| `docs/plugins/atlassian.md` | User-facing setup + auth docs + troubleshooting |

## Pre-Release Gates

1. **OAuth client_id** — register the public OAuth 2.0 (3LO) app at https://developer.atlassian.com/console/myapps/ and replace `BAKED_CLIENT_ID` in `src/atlassian/oauth.rs:22` (or document the env override). API-token path is unaffected.
2. **End-to-end smoke test** — set `ATLASSIAN_EMAIL` + `ATLASSIAN_API_TOKEN` + `atlassian_host`, run `lark`, hit each of the 10 quickkeys.

## Next

See `.docs/ai/next-steps.md`.
