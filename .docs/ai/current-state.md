# Current State

> Updated: 2026-07-01

## Maintenance (out of milestone)

- 2026-07-01 `f5e3b13` — ccusage plugin fixed for ccusage 20.x multi-agent
  schema (route all 5 commands through `ccusage claude <sub>`; restores legacy
  Claude-only schema + scopes to Claude). Tracked: bead `larkline-bbz` (closed);
  gotcha saved via `bd remember` key `ccusage-multiagent-schema`.

## Next Milestone — v1.0 Agent Palette

Planned 2026-05-09. ~22 weeks (~5 months) horizon. Headline thesis: **a command
palette where the AI uses your plugins as tools**. Plan in
[`v1.0-plan.md`](./v1.0-plan.md); roadmap entry in [`roadmap.md`](./roadmap.md)
under "v1.0 — Agent Palette".

**Phase 1 (current):** tag v0.13/v0.14/v0.15 from existing -prep branches.
Smoke runbook lives at [`phases/v0.14-v0.15-smoke-runbook.md`](./phases/v0.14-v0.15-smoke-runbook.md).

## Active Branches

- `v0.14.0-prep` (larkline) + `v0.14.0-prep` (lark.nvim) — both pushed to
  remote; tracking upstream. Six v0.14.0 commits + one in lark.nvim.
- `v0.15.0-prep` (larkline) — pushed; branched off `v0.14.0-prep`. Eleven
  v0.15.0 commits on top + MIT relicense (`d874bf7`).

Taylor merges + tags after manual smoke tests, in order: v0.14.0 first, then
v0.15.0. After both ship, work continues toward v1.0 on a fresh `v1.0-prep`
branch off `main`.

## Recent Progress

### v0.15.0 — Plugin Error UX

| Phase | Commit | Summary |
|---|---|---|
| A | `c997975` | `OutputItem.retry_action: Option<ItemAction>` + `help_url: Option<String>` with 3 serde tests |
| B | `3c6c726` | `examples/plugins/_shared/errors.lua` canonical helpers + `tests/plugin_error_translator_test.rs` (7 mlua-driven tests). Engine `scan()` skips `_`/`.`-prefixed dirs. |
| C | `0e89164` | TUI: `OpenUrl` prefers `help_url`; `RerunCommand` dispatches `retry_action` first; status bar appends `[r] retry` / `[o] help` |
| D (×7) | `72df199`, `faf2806`, `bd92174`, `2886d96`, `d3ba296`, `1670d11`, `cf8237f` | Plugin migrations — GitHub, Atlassian, Linear, Kubernetes, Home Assistant, Bitwarden, Docker |
| E | `a8f2bc5` | Docs — `LUA_PLUGINS.md` + `ARCHITECTURE.md` + `CHANGELOG.md` |
| F | (this) | Roadmap + handoff + phase report |

Wire-format change is purely additive. Existing items continue to deserialize
unchanged. Status bar surfaces `[r] retry` / `[o] help` hints only when the
focused item carries the affordance — invisible on normal rows.

### v0.14.0 — lark.nvim v4 (Telescope picker previewers)

| Phase | Commit | Summary |
|---|---|---|
| A | `f69dd13` | Add `OutputItem.preview: Option<String>` with serde tests |
| C | `fc60ec4` | GitHub plugin: `preview = pr.body` / `issue.body` (zero-cost) |
| D | `48ce43a` | Atlassian plugin: opt-in `preview_full` toggle, ADF + storage-format reducers, helpers in `lib.lua` |
| E | `f515a68` | Bitwarden plugin: `preview = render_detail_markdown(item)`, honors `redact_secrets` |
| F | `a79b049` | Docs (`lark-nvim.md` Previewers section, `ARCHITECTURE.md` schema, roadmap, current-state, CHANGELOG) |

**lark.nvim v0.14.0-prep:**

| Phase | Commit | Summary |
|---|---|---|
| B | `06da46d` | `lua/lark/previewer.lua` (buffer previewer, markdown filetype) wired into `results.lua` |

## Architecture summary

### v0.15.0 contract

`OutputItem` gains two optional fields:
- `retry_action: Option<ItemAction>` — when set, the focused item's `r` key fires this action instead of the default whole-plugin rerun. Use for chain-context failures where rerun would lose state. Most plugins leave it unset.
- `help_url: Option<String>` — when set, the focused item's `o` key opens this URL instead of `url`. Convention: error rows carry `help_url` pointing at docs; normal rows leave it unset.

Status bar in `ViewOutput` mode appends `[r] retry` and/or `[o] help` hints automatically when the focused row has those affordances.

The Lua side: a canonical `examples/plugins/_shared/errors.lua` defines `error_item(opts)` and `from_exit(stderr, hints)` translators. Plugins copy them inline (sandbox has no `require`) using `-- SHARED:` markers.

### Cost / activation note

`from_exit` translates known stderr patterns (missing CLI, auth failure, rate limit, network down) into structured rows. It's included verbatim in all shell-based plugins (Docker, k8s, Bitwarden, HA) but is currently dormant: `lark.exec()` returns stdout only, so plugins never see real stderr. A future stderr-aware exec API will activate the translator everywhere — see roadmap's Now / Next / Later **Next** section.

## Current Version

`Cargo.toml` at `0.13.0`. Tag v0.14.0 then v0.15.0 on Taylor's signal:

```sh
bash scripts/release.sh set 0.14.0     # after v0.14.0 smoke
git -C ~/git/lark.nvim tag v0.14.0 && git -C ~/git/lark.nvim push --tags
bash scripts/release.sh set 0.15.0     # after v0.15.0 smoke
```

## Validation

- `cargo test` — 221 passed (was 208 before v0.15.0; +6 from 3 new serde tests duplicated across lib + integration runs, +7 from the new translator test binary).
- `cargo clippy --tests --all-targets -- -D warnings` — clean.
- `cargo fmt -- --check` — clean.
- `luac -p` clean across every modified plugin Lua file.

## Pre-Release Gates

### v0.14.0 smoke (5 scenarios)

GitHub PR body in preview pane; Atlassian Jira `preview_full=true`; Confluence quality check; Bitwarden redacted preview; fallback paths for forms / mini-apps. Runbook in roadmap's Now / Next / Later section.

### v0.15.0 smoke (per migrated plugin)

Trigger a known failure (revoke token, break network, kill daemon) in each migrated plugin and verify:
1. The error row's icon is `!`.
2. Status bar shows `[o] help` (not `[r] retry`, since v0.15.0 plugins don't set `retry_action` — the rerun fallback handles them).
3. Pressing `o` opens the right docs URL in the system browser.
4. Pressing `r` re-runs the plugin (existing behavior, not regressed).

The 7 plugins: GitHub (`gh`), Atlassian (Jira/Confluence), Bitwarden (`bw`), Linear, Docker, Kubernetes, Home Assistant.

### Carried over from earlier

- **OAuth client_id** (carried over from v0.12.0) — Atlassian OAuth still gated on registering the public OAuth 2.0 (3LO) app; API-token path works today.

## Next

See the **Now / Next / Later** section in `.docs/ai/roadmap.md`.
