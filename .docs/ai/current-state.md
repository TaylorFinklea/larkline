# Current State

> Updated: 2026-04-29

## Active Branch

`v0.14.0-prep` (larkline) + `v0.14.0-prep` (lark.nvim) — both unpushed local
branches. Taylor merges + tags after a manual smoke test.

## Recent Progress

### v0.14.0 — lark.nvim v4 (Telescope picker previewers)

Six commits across the two repos. Wire-format change is a single optional
field on `OutputItem`; everything else is plumbing.

**larkline (`v0.14.0-prep`):**

| Phase | Commit | Summary |
|---|---|---|
| A | `f69dd13` | Add `OutputItem.preview: Option<String>` with serde tests |
| C | `fc60ec4` | GitHub plugin: `preview = pr.body` / `issue.body` (zero-cost) |
| D | `48ce43a` | Atlassian plugin: opt-in `preview_full` toggle, ADF + storage-format reducers, helpers in `lib.lua` |
| E | `f515a68` | Bitwarden plugin: `preview = render_detail_markdown(item)`, honors `redact_secrets` |
| F | (this) | Docs (`lark-nvim.md` Previewers section, ARCHITECTURE.md schema, roadmap, current-state, next-steps, CHANGELOG) |

**lark.nvim (`v0.14.0-prep`):**

| Phase | Commit | Summary |
|---|---|---|
| B | `06da46d` | `lua/lark/previewer.lua` (buffer previewer, markdown filetype) wired into `results.lua` |

### Architecture summary

The contract: each `OutputItem` may now carry a `preview: Option<String>`.
The TUI ignores it. The lark.nvim Telescope previewer reads it from
`entry.value.preview`, splits on `\n`, and writes the lines into the
preview buffer with `filetype = "markdown"`. Empty/missing → placeholder
text. Synchronous fill — no async, no race on fast j/k scrolling.

GitHub gets it for free (`search/issues` already returns `body`). Bitwarden
gets it cheap (`bw list items` returns full bodies; the existing detail
renderer is reused). Atlassian gates it on a `preview_full` toggle (default
off) so list latency stays stable for users who don't want the preview.

## Current Version

`Cargo.toml` at `0.13.0`. Tag `v0.14.0` on Taylor's signal — pipeline:

```sh
bash scripts/release.sh set 0.14.0
```

(Per the spec: do NOT bump Cargo.toml or tag from this branch — Taylor
gates on a manual smoke test against a locally-built `lark` binary.)

## Validation

- `cargo test` — 208 passed (was 204 before v0.14.0; +4 from the new serde
  tests `preview_serializes_when_set` + `preview_absent_deserializes`,
  duplicated across the unit + integration runs).
- `cargo clippy --tests --all-targets -- -D warnings` — clean.
- `cargo fmt -- --check` — clean.
- `luac -p` syntax check on every modified plugin Lua file — clean.
- `nvim --headless --clean -c "lua require('lark.previewer')"` — module
  loads. (Done with `set rtp+=.` from the lark.nvim repo.)

## Pre-Release Gates

1. **Smoke-test end-to-end against a real Telescope install.** Runbook in the
   Now / Next / Later section of `.docs/ai/roadmap.md`. The four built-in scenarios (GitHub PR body,
   Atlassian Jira issue with `preview_full` on, Bitwarden item with
   redaction, fallback to floating terminal) all need a manual pass.
2. **Confluence storage-format reducer quality.** Best-effort regex strip;
   prose-heavy pages render cleanly, macro-heavy pages may show residual
   placeholders. If quality is unacceptable for Taylor's content, defer
   Confluence preview to v0.14.x and ship Jira-only.
3. **OAuth client_id** (carried over from v0.12.0) — Atlassian OAuth still
   gated on registering the public OAuth 2.0 (3LO) app; API-token path
   works today.

## Next

See the **Now / Next / Later** section in `.docs/ai/roadmap.md`.
