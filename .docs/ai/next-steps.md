# Next Steps

> Updated: 2026-04-29

## v0.14.0 — Ready to Cut (after smoke test)

Five v0.14.0 commits in larkline (`v0.14.0-prep` branch) + one in lark.nvim
(`v0.14.0-prep` branch). The wire-format change is purely additive:
`OutputItem.preview: Option<String>`. Telescope's right-hand pane renders
the focused row's preview while scrolling.

**Pre-tag smoke test** (below) is the only remaining gate. Once green:

```sh
# In larkline:
bash scripts/release.sh set 0.14.0
# In lark.nvim:
git tag v0.14.0 && git push --tags
```

The script bumps `Cargo.toml` + `Formula/larkline.rb`, commits, tags
`v0.14.0`, pushes. CI handles the rest.

## Pre-release smoke test

Manual end-to-end against a real nvim + telescope environment. Need:
- a recent `lark` build with `preview` on the wire
- `lark.nvim` `v0.14.0-prep` branch
- a logged-in GitHub `gh` CLI (for the GitHub previews)
- optionally: Atlassian + Bitwarden configured

```sh
# 1. Build local lark:
cd ~/git/larkline && cargo build --release

# 2. Point lark.nvim at it:
LARK_BINARY=$(pwd)/target/release/lark nvim

# 3. Inside nvim:
#    :Telescope lark
#    Pick "My PRs" — preview pane should show PR body. j/k should refresh
#    smoothly with no flicker.
#    Try the action sub-picker via <C-a> — preview pane should still work.
#
# 4. Toggle Atlassian preview_full on (TUI: Plugin Manager → atlassian →
#    Settings → preview_full → toggle). Re-run :Telescope lark → "My
#    Issues". Preview pane should show description text. Toggle off
#    again — preview falls back to "(no preview available)".
#
# 5. Bitwarden: bw unlock --raw | export BW_SESSION=...
#    :Telescope lark → "Vault" → preview shows item details with
#    password redacted (••••••••).
#
# 6. Form fallback: :Telescope lark → "Generate Password" → should
#    drop into the floating-terminal TUI (previewer never runs in this
#    path).
#
# 7. Mini-app fallback: :Telescope lark → "Docker Dashboard" → same
#    floating-terminal fallback.
```

**Confluence quality check:** if Confluence pages are macro-heavy and the
preview pane shows ugly residual placeholders, decide whether to:
(a) ship as-is and add a "best effort" note in user docs (already there);
(b) defer Confluence preview to v0.14.x by gating just the Confluence
files behind a `preview_full_confluence` setting.

If anything looks off, fix and re-commit on the prep branches before
tagging.

## Open follow-ups (not blocking v0.14.0)

- **Register Atlassian OAuth app** (carried from v0.12.0) — Taylor's call.
- **Lazy preview fetching (Approach B)** — fetch `preview` on demand via a
  `preview_action` callback. Worth revisiting once we have data on whether
  the `preview_full=true` Atlassian latency hurts in practice.
- **Treesitter highlighting** beyond markdown filetype default.
- **Attachments / images** in previews (binary content is silently skipped today).
- **Streaming previews** (live-update while reading).

## v0.15.0+ candidates

- **`cargo-dist`** evaluation — replace `scripts/release.sh` + `release.yml`.
- **PagerDuty / 1Password CLI** plugin deep-dives (deferred while Taylor isn't using them).
- **Atlassian `switch`** for multi-cloud orgs.

## Backlog

See `.docs/ai/roadmap.md` → Backlog section. Open items unchanged from
post-v0.13.0 burn:

- "Plugin error output: convert raw stderr to user-friendly messages" — UX-wording pass.
- Bitwarden backlog (rbw, Send, attachments, edit/delete, lock-after-clipboard, TOTP countdown).
- Atlassian polish (multi-cloud `lark atlassian switch`, OAuth client_id registration).
