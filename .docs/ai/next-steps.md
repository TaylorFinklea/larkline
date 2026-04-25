# Next Steps

> Updated: 2026-04-24

## Three releases queued on `main`

The last actually-shipped tag is `v0.10.0` (commit `627b8fc`). Three releases'
worth of work has accumulated since:

1. v0.10.0 follow-up — input-loop fix for Ghostty / Kitty-protocol terminals (`fa0eeff`)
2. v0.11.0 — Bitwarden plugin (4 commits: `2932db3` + `7b4ee48` + `9b84d6b` + `50cfa79`)
3. v0.12.0 — Atlassian plugin (6 commits: A `0922b57`, B `ddf1403`, C `fd31246`, D `17930db`, E `e83b2d7`, F this commit)

Taylor's preference: ship as one combined `v0.12.0` tag since the v0.11.0 work
isn't separately tagged either.

```sh
bash scripts/release.sh set 0.12.0
```

This bumps `Cargo.toml` + `Formula/larkline.rb`, commits, tags `v0.12.0`,
pushes. CI handles binaries + Homebrew bump + crates.io publish (the last
gated on the `CRATES_IO_TOKEN` GitHub secret).

## Pre-release smoke test

Before tagging:

```sh
mkdir -p ~/.config/larkline/plugins
ln -sfn "$(pwd)/examples/plugins/atlassian" ~/.config/larkline/plugins/atlassian

# API-token path
lark secret set ATLASSIAN_EMAIL
lark secret set ATLASSIAN_API_TOKEN
# In the TUI: open Plugin Manager (P), Atlassian, Settings, set atlassian_host

# Launch and try each quickkey
lark
# jmi, jsp, jtr, jnw, jtx, jcm, cre, csr, cmy, cnw
```

If anything looks off, fix and recommit before tagging. Bitwarden is the
existing reference for what "good" looks like.

## OAuth gate (don't ship blocked)

The OAuth path is implemented end-to-end but `BAKED_CLIENT_ID` in
`src/atlassian/oauth.rs:22` is a placeholder. Two options:

1. **Defer OAuth, ship API-token only:** the plugin works fine without OAuth;
   `lark atlassian login` will show a clear "OAuth client id not configured"
   error. Users self-host via `LARKLINE_ATLASSIAN_CLIENT_ID` env override.
2. **Register and bake:** create a public OAuth 2.0 (3LO) app at
   <https://developer.atlassian.com/console/myapps/> with the scopes documented
   in `docs/plugins/atlassian.md`, and replace `BAKED_CLIENT_ID`.

Option 1 is the path of least resistance for the personal-use case Taylor
mentioned. Option 2 makes lark shareable.

## v0.13.0 candidates (next theme)

- **Telescope integration for lark.nvim v3** — needs `lark list --json` headless
  subcommand first.
- **`lark atlassian switch`** — pick a different cloud site without full
  re-login (handy if you have multiple Atlassian orgs).
- **PagerDuty / 1Password CLI deep-dives** — only if Taylor switches to using them.
- **`cargo-dist`** — evaluate replacing `scripts/release.sh` + `release.yml`.

## Backlog

See `.docs/ai/roadmap.md` → Backlog section. New items added during v0.11.0:

- Bitwarden backlog (rbw, Send, org filtering, attachments, edit/delete actions, lock-after-clipboard, TOTP countdown).
