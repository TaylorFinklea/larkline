# Larkline Roadmap

## Vision

Terminal-native Raycast: a keyboard-driven command palette for personal productivity plugins. Launch, search, act, dismiss.

## Now / Next / Later

Active items. Trim as completed.

### Now — v1.0 Agent Palette (planned 2026-05-09; ~5-month horizon)

**v1.0** is the public-launch milestone. Theme: **a command palette where the AI uses your plugins as tools** — agentic AI calls Larkline plugins via a tool registry built from manifests. Full plan + phase table + risks live in [`v1.0-plan.md`](./v1.0-plan.md).

v1.0 scope (12 phases, ~22 weeks):
- **Tag backlog**: v0.13/v0.14/v0.15 ship under existing -prep branches; smoke gate in [`phases/v0.14-v0.15-smoke-runbook.md`](./phases/v0.14-v0.15-smoke-runbook.md).
- **macOS Swift helper** (`larkline-macos-helper`) — single binary, EventKit + MailKit, JSON-line protocol.
- **Calendar v2** — structured items, Teams/Zoom/Meet URL extraction, primary `[Enter] Join` action + 4 secondary actions.
- **Mail plugin** — read/search, triage (archive/flag/delete), compose via `$EDITOR` handoff, multi-account/mailbox switch.
- **AI Provider trait** — Anthropic Claude, OpenAI Responses, Codex CLI subprocess (free fallback), OpenRouter.
- **AI single-shot** + **AI tool-use agent** (the headline feature) with three-layer safety: per-plugin manifest opt-in, per-action `destructive` flag, dry-run plan preview.
- **Web search shortcuts**, **onboarding wizard**, **theme polish**, **beta + Medium launch prep**.

Phase 1 (current): tag v0.13/v0.14/v0.15 from their existing -prep branches.

### Next (after v1.0 ships)

- **v1.1** — promote AI single-shot to chat mini-app; explore mail compose mini-app editor.
- **Stderr-aware `lark.exec`** — activates the dormant v0.15.0 `from_exit` translator across shell plugins (Docker, k8s, Bitwarden, HA).
- **lark.nvim retry/help keymaps** — wire `<C-r>` / `<C-?>` in Telescope results picker.
- **lark.nvim loading UX flicker** — when invoking a slow plugin from the catalog picker, the inner results picker opens empty, then re-renders when `lark invoke` returns. Should show a loading spinner / placeholder row while invoking. Surfaced during v0.14.0 smoke (Taylor 2026-05-10). Fix lives in `lua/lark/results.lua`'s `invoke_and_show` — wrap in async (`vim.system` or `plenary.job`), show a transient placeholder picker until the JSON arrives.
- Lazy preview fetching (Approach B); treesitter beyond markdown; attachments/streaming previews.
- Register Atlassian OAuth app (carried from v0.12.0).

### Later (post-v1.0)

- `cargo-dist` evaluation — replace `scripts/release.sh` + `release.yml`.
- PagerDuty / 1Password CLI plugin deep-dives.
- Atlassian `switch` for multi-cloud orgs.

## Completed Milestones

| Phase | Summary |
|-------|---------|
| 0-4.5 | Core scaffold, Plugin trait, TUI, fuzzy search, favorites, keybindings, vim modes |
| 5 | Embedded Lua (mlua 5.4), sandboxed VM, `lark.*` host API |
| 6 | Distribution: `--help`, `init-plugin` scaffolder, Homebrew formula |
| 7 | Enhanced output: ANSI, shell actions, tables, streaming, Nerd Font icons |
| 8 | Unified search: prefetch cache, nucleo filter, flash messages |
| 9 | Global item ranking, match highlighting, RunPlugin rows |
| 10-16 | Forms, action palette, multi-command plugins, output search, markdown rendering |
| 17-22 | Plugin store, sidebar toggle, copy menu, output filter |
| 23-24 | Multi-repo Git, command history, recent section |
| 25-26 | Plugin settings UI, theme presets + TUI switcher |
| 27-28 | DevOps plugins (k8s, ports, kill-process), macOS plugins (calendar, apps, clipboard) |
| 29 | Raycast-style flat list: inline plugin name badges |
| 30 | UX polish: quickkey priority, sidebar ratio, Nerd Font icons, vim mode fixes |
| 31 | Secrets: macOS Keychain fallback + `lark secret` CLI |
| 32 | Standard plugins: file search, quicklinks, encode/decode, emoji, translate, Home Assistant |
| 33 | Plugin Manager: LazyVim-style enable/disable, settings, secret status |
| v0.2.0 | Published release: Homebrew formula, 35 plugins |
| v0.3.0 | Calendar, ccusage/codex-usage, lark.nvim, plugin distribution |
| v0.3.1 | Homebrew formula rename (larkline), brew upgrade fix |
| v0.4.0 | Dashboard widgets, widget management, git deep-dive, developer plugin, CI fmt fixes |
| v0.5.0 | Background update checker, Docker deep-dive (6 commands, Portainer-style) |

## Completed (post-v0.5.0)

- [x] Dashboard Widgets: bordered card panes, auto-refresh, reorder/disable/toggle
- [x] Widget picker: overlay to choose which widgets to show
- [x] Widget discoverability: contextual status bar hints
- [x] Background update checker: GitHub API, daily cache, install method detection
- [x] Docker deep-dive: containers (stats/logs/exec/widget), compose, images, volumes, networks, system
- [x] Git deep-dive: richer status, branches, log, stash
- [x] Developer plugin + Claude Code skill
- [x] Automated release pipeline: CI builds + auto-updates Homebrew tap
- [x] Context-aware power menu (adapts to focused element)

## Themed Releases

### v0.6.0 — Plugin Deep-Dives + UX Polish

Bring four plugins to Raycast quality and fix the top UX pain points.

**Plugin deep-dives:**
- [x] Kubernetes: 6 commands — pods (describe/exec/logs), deployments (scale/restart/rollout), services, namespaces (resource counts), logs (per-container), contexts
- [x] GitHub: 5 commands — my-prs (merge/squash/close), reviews (approve/request-changes), notifications (mark-read), issues (close), workflows
- [x] SSH: 4 commands — hosts (nc reachability), connections (active from ps), recent (shell history), keys (fingerprints/agent)
- [x] Weather: 3 commands — current (weather icons, astronomy), forecast (3-day hourly), locations (saved via lark.store)

**UX (required for release):**
- [x] Widget card Enter → drill into full command output
- [x] Update checker → power menu action (run upgrade command)
- [x] Widget picker search/filter for large plugin lists
- [x] Better plugin error display: user-friendly messages + retry hints

### v0.7.0 — New Plugins

Expand the plugin library with three new integrations:
- [x] Obsidian/Notes: quick note search, recent notes, vault browser (4 commands)
- [x] Tailscale/VPN: device status, exit nodes, network overview (3 commands)
- [x] Linear: assigned issues, current cycle, triage queue (3 commands, GraphQL API)

### v0.8.0 — Mini App Mode

Full-screen split-pane UI controlled by plugins — neovim-style.
- [x] Action chaining: `on_action` callback, `ActionKind::Chain` + `UpdatePane`
- [x] Layout data model: `MiniAppLayout` recursive tree, `PaneContent`, manifest `mini_app` field
- [x] Split-pane rendering: recursive `render_layout_node()`, focused pane accent border
- [x] Mini app input: Tab/Ctrl+h/l pane focus, j/k per-pane nav, Enter per-pane actions
- [x] User splits: Ctrl+\/- split, Ctrl+x close, +/_ resize, tree mutation helpers
- [x] `lark.clipboard_read()` host API, clipboard history plugin (no Maccy dependency)
- [x] Docker Dashboard mini app reference plugin (two-pane: container list + detail)

### v0.9.0 — Internal Quality

Non-user-facing cleanup: split the `app.rs` god-object and tune the prefetch/widget refresh paths.

- [x] Phase A: Extract widget state helpers into `src/widgets.rs` (ensure_widget_order, rebuild_widget_indices, sync_preview_index, widget picker helpers)
- [x] Phase B: Extract `build_power_menu_categories()` into `src/power_menu.rs`
- [x] Phase C: Extract `build_plugin_manager_state*()` into `src/plugin_manager_state.rs`
- [x] Phase D: Extract output/form helpers into `src/app_output.rs` (visible_output_count, selected_output_item, rebuild_output_filter, reset_output_search, output_mode_for, check_form_init, initialize_form)
- [x] Phase E: Split `handle_action()` by mode — delegate to per-mode handlers (Unified, ViewOutput, MiniApp, PluginManager, etc.)
- [x] Phase F: Performance — prefetch tuning, slow-plugin profiling, skip widget refresh when dashboard is not visible

### v0.10.0 — Distribution + lark.nvim v2 (bundled) ✅ SHIPPED

Tag `v0.10.0` cut on 2026-04-19 (commit `627b8fc`).

- [x] Phase A: Tracing-to-file (`tracing-appender` rolling daily, `$XDG_STATE_HOME/larkline/lark.log`)
- [x] Phase B: `lark plugin sync` — repair dead symlinks; `--force` overwrites user-modified plugins with interactive confirmation
- [x] Phase C: crates.io publish wiring (Cargo.toml `exclude`, CI dry-run, tag-gated publish job with auto-issue on failure)
- [x] Phase D: Install docs in README (Homebrew, Cargo, AUR, Nix, GitHub Releases) + CHANGELOG v0.6–v0.10 rollup
- [x] Phase E: AUR `larkline-bin` PKGBUILD + publish workflow docs
- [x] Phase F: Nix flake (`rustPlatform.buildRustPackage`, package + app + devshell)
- [x] Phase G: lark.nvim v2 — `$NVIM` socket awareness; new `ActionKind::NvimEdit` + Lua host fn for arbitrary ex commands; `file-search` and `notes` plugins updated
- [x] Phase H: `scripts/release.sh set <version>` mode; handoff docs
- [x] Post-tag fix: `fix(tui): drain all events per frame and accept key repeat` (commit `fa0eeff`) — fixed double-press bug on Ghostty/Kitty-protocol terminals

### v0.11.0 — Bitwarden Deep-Dive

Raycast-parity Bitwarden plugin using the official `bw` CLI. Requires `BW_SESSION` env var (user runs `bw unlock --raw` once and exports).

- [x] Phase A: `lib.lua` + session discovery (`bw_session`, `run_bw` with `--response` JSON, type/icon helpers, redact helpers, detail renderer)
- [x] Phase B: `items.lua` — search all items, type-specific copy actions (password/username/TOTP/URL for logins; number/CVV/holder/exp for cards; email/phone/SSN/passport for identities; custom fields for all)
- [x] Phase C: `folders.lua` (browse by folder with drill-in via `on_action`) + `favorites.lua` (`--favorite` filter)
- [x] Phase D: `generate.lua` — password + passphrase modes, length/symbols/numbers/ambiguous/minnumber/minspecial/capitalize/include-number options, regenerate action chain
- [x] Phase E: `sync.lua` (status + sync action) + `lock.lua` (confirm-before-lock action)
- [x] Phase F: Item detail renderer with card (number/CVV/exp/brand), identity (address/phone/SSN/etc), custom fields (field_type 1 = hidden, redacted)
- [x] Phase G: roadmap + handoff docs + `rbw` backlog

### v0.12.0 — Atlassian (Jira + Confluence) deep-dive

Single plugin covering both Atlassian products with both auth paths supported:

- [x] Phase A: API-token path (`ATLASSIAN_EMAIL` + `ATLASSIAN_API_TOKEN` + `atlassian_host` setting), `lark.base64` Lua host API, plugin scaffold + my-issues
- [x] Phase B: OAuth 2.0 (3LO + PKCE) subsystem — `lark atlassian login/token/cloudid/site/status/logout` subcommands. Refresh tokens in macOS Keychain, access tokens in `~/.cache/larkline/atlassian-access.json` (mode 0600). Hand-rolled HTTP callback listener (no new dep).
- [x] Phase C: Plugin auth dispatcher in `lib.lua` — falls through to OAuth via `lark atlassian token` when API-token vars are absent. New `LARK_BINARY` host injection so dev runs (target/debug) can re-invoke themselves.
- [x] Phase D: Jira commands — sprint, triage, new-issue (form + ADF), transition (chain), comment (chain).
- [x] Phase E: Confluence commands — recent, search (CQL), my-pages, new-page (storage format).
- [x] Phase F: roadmap + handoff docs + CHANGELOG + `docs/plugins/atlassian.md`.

**Pre-publish gate (Taylor):** OAuth `BAKED_CLIENT_ID` in `src/atlassian/oauth.rs:22` is a placeholder. Register a public OAuth 2.0 (3LO) app at https://developer.atlassian.com/console/myapps/ and either bake the real id or document `LARKLINE_ATLASSIAN_CLIENT_ID` env override. API-token path is fully usable today; OAuth path is gated.

### v0.13.0 — lark.nvim v3 (Telescope-native picker)

Headless contract on the Rust side, Telescope source on the Lua side. Plus the `lark.nvim/` subdirectory was extracted to its own public repo at <https://github.com/TaylorFinklea/lark.nvim>.

- [x] Phase A: `lark list --json` headless plugin enumeration
- [x] Phase B: extract action dispatcher from TUI to `crate::actions` for CLI reuse
- [x] Phase C: `lark action <plugin> --action-json '<JSON>' [--confirm]` subcommand with three outcome variants (`side` / `chained` / `needs_confirmation`)
- [x] Phase D: extract `lark.nvim/` to standalone repo at TaylorFinklea/lark.nvim (fresh history)
- [x] Phase E: Telescope plugin list picker (`lua/lark/cli.lua`, `picker.lua`, `lua/telescope/_extensions/lark.lua`)
- [x] Phase F: results picker + action dispatch + chain stacking (`lua/lark/results.lua`, `actions.lua`)
- [x] Phase G: floating-terminal fallback for forms / mini-apps + new keymap (`<C-l>` Telescope, `<C-l><C-l>` TUI)
- [x] Phase H: roadmap + current-state + CHANGELOG + `docs/plugins/lark-nvim.md`

### v0.14.0 — lark.nvim v4 (Telescope picker previewers)

Render rich body content in Telescope's right-hand preview pane while
scrolling rows in a `lark` results picker. New optional `preview` field on
`OutputItem` (markdown by convention; plain text fine). Plugins populate it
inline; the previewer reads `entry.value.preview` and writes it to the
preview buffer synchronously. Implementation kept on a `v0.14.0-prep` branch;
Taylor tags after a manual smoke test.

- [x] Phase A: `OutputItem.preview: Option<String>` with serde tests
- [x] Phase B: `lark.nvim/lua/lark/previewer.lua` (buffer previewer, markdown filetype) wired into `results.lua`
- [x] Phase C: GitHub plugin (zero-cost — `search/issues` already returns the body)
- [x] Phase D: Atlassian plugin — opt-in `preview_full` toggle (default off). Jira (description via JQL `fields=description`) and Confluence (body.storage via `expand=`). Ships with a best-effort storage→text reducer for Confluence
- [x] Phase E: Bitwarden plugin (free plumb; honors `redact_secrets`)
- [x] Phase F: Docs (`lark-nvim.md` Previewers section, `ARCHITECTURE.md` schema, CHANGELOG, both repos' README + handoff)

**Deferred to v0.14.x or later:**
- Lazy preview fetching (Approach B): fetch `preview` on demand via a
  `preview_action` callback. Not worth the cross-repo round-trip latency yet.
- Treesitter highlighting beyond what `filetype = "markdown"` gives for free.
- Attachments / images in previews.
- Streaming previews (live-update while reading).
- "Open preview in main buffer" action.

### v0.15.0 — Plugin Error UX

Error rows now carry actionable affordances. Press `r` to retry the specific
operation, `o` to open troubleshooting docs — the status bar shows
`[r] retry` / `[o] help` hints automatically when set. Wire-format change is
purely additive: `OutputItem.retry_action` + `help_url`.

- [x] Phase A: `OutputItem.retry_action: Option<ItemAction>` + `help_url: Option<String>` with serde tests
- [x] Phase B: `examples/plugins/_shared/errors.lua` canonical reference + `tests/plugin_error_translator_test.rs` (7 mlua-driven tests covering missing CLI, auth, rate-limit, network, fallback)
- [x] Phase C: TUI dispatch — `Action::OpenUrl` prefers `help_url`; `Action::RerunCommand` dispatches `retry_action` first; status bar appends conditional `[r] retry` / `[o] help` hints in `ViewOutput` mode
- [x] Phase D: 7 plugin migrations, one commit each — GitHub, Atlassian, Bitwarden, Linear, Kubernetes, Home Assistant, Docker. All carry SHARED `error_item` (and `from_exit` for shell plugins). Every error row gets a `help_url` to status-aware docs pages.
- [x] Phase E: Documentation — `docs/LUA_PLUGINS.md` Item Fields + Error Handling sections, `docs/ARCHITECTURE.md` schema + JSON, `CHANGELOG.md`
- [x] Phase F: Roadmap + handoff + phase report

**Deferred / dormant:**
- `from_exit(stderr, hints)` is included verbatim in shell plugins (Docker, k8s, Bitwarden, HA) but cannot fire today: `lark.exec()` returns stdout only. Stderr-aware exec API is the activation key — listed in **Next**.
- `lark.nvim` Telescope keymaps for `<C-r>` retry / `<C-?>` help — cross-repo sub-step. Listed in **Next**.

### v1.0 — Agent Palette (planned 2026-05-09; revised after research pass)

The public-launch milestone. Headline thesis: **a command palette where the AI uses your plugins as tools**. Spotlight launches apps; Raycast fires actions; Larkline lets you speak to your stack — by keyboard or by asking an agent that knows every plugin you've installed.

Full plan in [`v1.0-plan.md`](./v1.0-plan.md). Twelve phases, ~21 weeks.

- [x] Phase 1: Tag v0.13/v0.14/v0.15 (✅ 2026-05-10; v0.14.0 + v0.15.0 on Homebrew tap)
- [x] Phase 2: macOS Swift helper — `larkline-macos-helper` binary, **EventKit only** (✅ 2026-05-12; 5 commits on `v1.0-prep`. Sub-phase 2.E dropped — `EKParticipant.participantStatus` is read-only on macOS. Report: [`phases/v1.0-phase-2-macos-helper-report.md`](./phases/v1.0-phase-2-macos-helper-report.md))
- [ ] Phase 3: Calendar v2 — structured items, regex-extracted Teams/Zoom/Meet URL (helper's three-tier fallback over event.url + location + notes), primary `[Enter] Join` action
- [ ] Phase 4: Mail plugin — **osascript-based** (proven mail-app-cli/apple-mail-mcp pattern); read/search, triage, compose via `$EDITOR` → Mail.app composer, multi-account/mailbox switch
- [ ] Phase 5: AI Provider trait + 4 backends — **Anthropic Messages, OpenAI Responses, OpenRouter, Ollama**. Codex CLI dropped (would force MCP scope; in-app agent vision doesn't need MCP)
- [ ] Phase 6: AI single-shot plugin (`ai/ask.lua`)
- [ ] Phase 7: In-process tool registry auto-generated from manifests; `agent_callable` + `destructive` schema additions
- [ ] Phase 8: AI tool-use plugin (`ai/agent.lua`) — agent loop with dry-run plan preview, audit log
- [ ] Phase 9: Web search shortcuts plugin + onboarding wizard
- [ ] Phase 10: QA pass + bug sweep + theme polish
- [ ] Phase 11: Beta + Medium draft + launch prep
- [ ] Phase 12: Tag v1.0 + Medium post + Show HN

**Wire-format additions:** manifest `agent_callable` (plugin + command level), command `destructive` boolean. Tool registry generates Anthropic + OpenAI tool schemas from these at engine startup.

**Safety:** three-layer (manifest opt-in, action destructive flag, dry-run plan preview). Curated set of agent-callable plugins ships in v1.0; users opt in additional plugins via settings.

**Reused infrastructure (research-confirmed):** `crate::actions::execute()` dispatcher, `EngineEvent::PartialOutput` streaming, `lark secret` Keychain integration, `FormField` settings UI — all already in place from v0.13/v0.7/v0.10. New Rust modules confined to `src/agent/` (provider/anthropic/openai/loop/planner/audit) — no engine-level changes.

### Future (post-v1.0)

- **v1.1:** promote AI single-shot to chat mini-app; explore mail compose mini-app editor; Bitwarden v2 (rbw, Send, organizations); lark.nvim v5 (lazy preview, treesitter).
- **Release tooling:** evaluate `cargo-dist`; SBOM / `cargo-auditable` for published binaries.
- **More plugin deep-dives:** PagerDuty, 1Password CLI.
- **Atlassian polish:** `lark atlassian switch` for multi-cloud orgs; register the OAuth app under Taylor's developer account so OAuth path works without `LARKLINE_ATLASSIAN_CLIENT_ID` override.

---

## Backlog

> Self-contained items any agent can pick up alongside any phase. Scoped, low-risk; first agent to start it executes it. Tier hints are advice, not gating.

### Guardrails

1. **No core Rust changes.** Must not touch `app.rs`, `input.rs`, `src/tui/`, `src/main.rs`, or engine code
2. **No new dependencies.** Must not add crates to `Cargo.toml`
3. **Tests must pass.** Must run `cargo test` + `cargo clippy -- -D warnings` before marking done
4. **Plugin-only changes are always safe.** Lua/shell files in `examples/plugins/` can be freely modified
5. **May add test files.** New or modified tests in `tests/` or inline `#[cfg(test)]` modules are fine

### Plugin Quality

- [x] HA plugin dedup: `-- SHARED:` markers above duplicated helpers in 22 command files (canonical copy in `helpers.lua`)
- [x] Compose plugin: `-- SHARED:` markers above 5 duplicated helpers (canonical copy in `docker/lib.lua`)
- [x] Audit all plugins for missing icons — every plugin must have one <!-- audited 2026-04-26: all 44 manifests have `icon` -->
- [x] Audit shell plugins for jq safety — no raw `$var` interpolation in JSON strings <!-- audited 2026-04-26: all 5 shell entries clean -->
- [x] Improve plugin error output: convert raw stderr to user-friendly messages where feasible in plugin code <!-- v0.15.0 — `error_item` + `help_url` across 7 plugins; `from_exit` stderr translator ready in shell plugins (dormant pending stderr-aware lark.exec) -->

### Test Coverage

- [x] Manifest validation tests for all 39 plugins (valid TOML, required fields present) <!-- tests/plugin_manifest_integration_test.rs -->
- [x] Output format smoke tests (valid JSON structure) for plugins with testable output <!-- tests/plugin_output_smoke_test.rs — 6 pure plugins -->
- [x] `init-plugin` scaffolder edge case tests <!-- tests/init_plugin_test.rs — Lua/shell/multi/refuses-overwrite -->

### Drive-by clippy fixes (post-v0.13.0 burn)

- [x] `tests/cli_list_test.rs:13` — add `#[allow(clippy::struct_excessive_bools)]` to mirror production `ListEntry`
- [x] `src/plugin/traits.rs:634/637/659` — replace `_ =>` with explicit variant patterns to satisfy `match_wildcard_for_single_variants` under `cargo clippy --tests`

### Documentation

- [ ] Plugin development guide improvements
- [ ] Example plugin READMEs
- [ ] Keybinding reference accuracy check vs actual defaults in code

### Mechanical (Haiku candidates)

- [x] AI Projects plugin: deduplicate inlined `discover_projects()` + helpers across 5 Lua files — extract canonical copy in lib.lua with instructions for manual paste (sandbox has no require)
- [x] Docker plugin: deduplicate Docker availability check across 6 command files (`containers.lua`, `compose.lua`, `images.lua`, `volumes.lua`, `networks.lua`, `system.lua`)
- [x] Fix `ip-addresses/run.sh`: raw `$var` interpolation in JSON on lines 13, 40 — use jq instead
- [x] Remove commented-out dead code in plugin Lua files (scan all `examples/plugins/` for orphan comments) <!-- no orphan comments found -->
- [x] Add `icon_nerd` field to any plugin manifest missing it (audit all 40 manifests) <!-- all 39 manifests already have icon_nerd -->
- [x] Fix `examples/plugins/system-info/run.sh` lines 36, 40, 45, 50: `$host`, `$mem_detail`, `$disk_info`, and `$load` are interpolated raw into a heredoc JSON — rewrite using `jq --arg` to prevent corruption if values contain quotes or special characters
- [x] Fix `src/tui/ui.rs:983-984`: label truncation uses byte-index `&item.label[..n]` which panics on multi-byte UTF-8 — replace with `.chars().take(n).collect::<String>()`
- [x] Fix `src/tui/ui.rs:417-418`: same byte-index truncation pattern (`&value[..40]`) in copy-menu preview — replace with `.chars().take(40).collect::<String>()`
- [x] ccusage plugin: `fmt_tokens()`, `fmt_cost()`, and `get_since()` are copy-pasted identically across `daily.lua`, `sessions.lua`, `monthly.lua`, `weekly.lua`, `blocks.lua` (lines 3-25 in each) — add a comment block at the top of each file with a `-- SHARED:` marker so a future lib.lua extraction is trivially diff-able
- [x] github plugin: `gh_headers()` is copy-pasted identically across `my-prs.lua:3`, `reviews.lua:3`, `issues.lua:3`, `notifications.lua:3` — add a `-- SHARED: gh_headers` comment marker in each file for the same future extraction

### Bitwarden Backlog (post-v0.11.0)

- [ ] **rbw support:** `rbw` is a Rust-native Bitwarden client that caches the session in a background agent, skipping the `BW_SESSION` export dance. Add a `use_rbw` toggle in `bitwarden/manifest.toml` settings and a conditional code path in `lib.lua`: when enabled, swap `bw get/list` for `rbw get/list` and skip `--session`. rbw emits plain text for passwords (not JSON), so the output parsing differs — either shell `rbw get` per item or provide a `rbw list --fields name,id,username,uri` summary for the list view.
- [ ] **Send support:** `bw send list`/`create`/`delete` — create and manage Bitwarden Sends (time-limited secure shares)
- [ ] **Organization + collection items:** `bw list items --organizationid <id>`; show collection filter when in an org
- [ ] **Attachments:** `bw get attachment <id>` to download attachments associated with items
- [ ] **Edit + delete item actions:** `bw edit item`, `bw delete item <id> --hard` — require confirmation
- [ ] **Lock-after-clipboard:** optional setting to auto-lock vault N seconds after a password copy (matches Raycast behaviour)
- [ ] **TOTP countdown in detail view:** render a live 30-second countdown next to the TOTP row (would need widget auto-refresh or streaming output)

### Refactors (Sonnet candidates)

- [x] Docker plugin: extract shared helpers (availability check, result parsing, action builders) into pattern matching AI Projects' approach
- [x] Git plugin: `status.lua`, `branches.lua`, `log.lua`, `stash.lua` all duplicate `repo_name()` and repo validation — extract shared pattern
- [x] AI Projects plugin: dashboard `on_action` drill-in sub-commands render file content via `shell:cat` — convert to structured parsed output (like the sub-command files already do)
- [x] Add integration test: verify all 40 plugin manifests parse correctly and have valid `entry` files pointing to existing Lua/shell scripts
- [x] weather plugin: `weather_icon()` (lines 3-14) and `get_location()` (lines 16-19) are duplicated in `current.lua` and `forecast.lua` — extract into `examples/plugins/weather/lib.lua` following the AI Projects pattern (comment-paste approach, since sandbox has no require)
- [x] github plugin: extract `gh_headers()` and the common GITHUB_TOKEN fetch+error-return pattern into `examples/plugins/github/lib.lua` — 4 files (`my-prs.lua`, `reviews.lua`, `issues.lua`, `notifications.lua`) repeat the same 7-line preamble
- [x] ccusage plugin: extract `fmt_tokens()`, `fmt_cost()`, `get_since()` into `examples/plugins/ccusage/lib.lua` — 5 files repeat these ~20 lines verbatim; reduces the surface area for divergence when cost display logic changes

### New Simple Plugins (follow existing patterns)

- [ ] Additional shell snippet sets
- [ ] System monitors: disk usage, battery, network stats
- [ ] Any plugin using only existing Lua/shell patterns — no engine changes needed

## Constraints

- Terminal only. No GUI dependencies.
- Sub-100ms startup.
- The `Plugin` trait is the only interface between backends and the engine.
- TUI reads state, never owns it. State transitions happen in `app.rs`.

## Non-Goals

- GUI/Electron wrapper
- Plugin marketplace / remote registry (local-first)
- Mouse-only workflows
