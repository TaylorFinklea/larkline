# Next Steps

> Updated: 2026-04-26

## v0.13.0 — Ready to Cut (after smoke test)

Five v0.13.0 commits + four post-tag backlog-burn commits accumulated since `v0.12.0` was tagged. The Rust side is the headless contract for `lark.nvim` v3 (Telescope picker, separate repo).

**Pre-tag smoke test** (Phase 2 below) is the only remaining gate. Once green:

```sh
bash scripts/release.sh set 0.13.0
```

This bumps `Cargo.toml` + `Formula/larkline.rb`, commits, tags `v0.13.0`, pushes. CI handles the rest.

## Pre-release smoke test

Manual end-to-end test before tagging — the Lua side only has unit-level smoke tests, full picker behavior needs a real nvim+telescope environment:

```sh
# 1. Install lark.nvim via lazy.nvim:
{
  "TaylorFinklea/lark.nvim",
  dependencies = { "nvim-telescope/telescope.nvim" },
  config = function()
    require("lark").setup({})
    pcall(require("telescope").load_extension, "lark")
  end,
}

# 2. Build local lark with the new subcommands:
cd ~/git/larkline && cargo build --release

# 3. With LARK_BINARY pointing at target/release/lark, in nvim:
#    :Telescope lark
#    Pick a plugin (e.g. "Recent Pages" if Atlassian configured) — hit <CR>.
#    Try <C-a> on a row for the action sub-picker.
#    Try a chain action (e.g. show_detail on a Jira issue) — should push a picker.
#    Try a form-based plugin (e.g. "Generate Password") — should fall back to floating terminal.
#    Try a mini-app (e.g. "Docker Dashboard") — should fall back too.
```

If anything looks off, fix and recommit before tagging. The Rust contract has 5 integration tests covering the JSON shapes; the Lua side relies on user verification.

## Open follow-ups (not blocking v0.13.0)

- **Register Atlassian OAuth app** (carried from v0.12.0) — Taylor's call. API-token path works today; OAuth path errors with a clear "OAuth client id not configured" message until `LARKLINE_ATLASSIAN_CLIENT_ID` is set or the baked id is replaced.
- **Action-result previewers in Telescope** — Telescope can show preview text for the focused entry. Markdown bodies for Jira issues, Confluence pages would be a nice polish. Defer to v0.14.x.
- **Streaming output** — plugins that emit newline-delimited JSON (log tailers) currently fall back to floating terminal. Telescope can update finders dynamically; engineering cost is moderate. Defer.
- **Atlassian `switch` subcommand** — multi-cloud Atlassian users. Polish, defer.

## v0.14.0 candidates

- **Telescope previewers** for plugin output (Jira issue body, Confluence page).
- **PagerDuty / 1Password CLI** plugin deep-dives (deferred while Taylor isn't using them).
- **`cargo-dist`** evaluation — replace `scripts/release.sh` + `release.yml` with unified cross-platform tooling.

## Backlog

See `.docs/ai/roadmap.md` → Backlog section. The post-v0.13.0 burn (2026-04-26) closed:

- HA plugin dedup markers (22 files)
- Compose plugin dedup markers (5 helpers)
- `init-plugin` scaffolder integration tests (4 cases)
- Output schema smoke tests for 6 pure plugins
- Pre-existing `cargo clippy --tests` lints in `cli_list_test.rs` and `plugin/traits.rs`

What's left in the backlog:

- "Plugin error output: convert raw stderr to user-friendly messages" — needs a UX-wording pass, not a tech-debt pass. Defer.
- Bitwarden backlog (rbw, Send, attachments, edit/delete, lock-after-clipboard, TOTP countdown) — feature work, belongs in a future Bitwarden milestone.
- Atlassian polish (multi-cloud `lark atlassian switch`, OAuth client_id registration).
