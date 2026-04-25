# Larkline Roadmap

## Vision

Terminal-native Raycast: a keyboard-driven command palette for personal productivity plugins. Launch, search, act, dismiss.

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

### Future (unordered — pick theme per release)

- **lark.nvim v3:** Telescope integration (needs `lark list --json` first), action streaming lark → nvim buffer updates in real time
- **Release tooling:** evaluate `cargo-dist`; SBOM / `cargo-auditable` for published binaries
- **More plugin deep-dives:** PagerDuty, 1Password CLI (deferred — user doesn't use these)

---

## Backlog (parallel work for smaller models)

<!-- tier3_owner: claude -->

These items can be worked on by cheaper AI assistants alongside any phase. They are scoped, low-risk, and don't require deep architectural knowledge.

### Guardrails

1. **No core Rust changes.** Must not touch `app.rs`, `input.rs`, `src/tui/`, `src/main.rs`, or engine code
2. **No new dependencies.** Must not add crates to `Cargo.toml`
3. **Tests must pass.** Must run `cargo test` + `cargo clippy -- -D warnings` before marking done
4. **Plugin-only changes are always safe.** Lua/shell files in `examples/plugins/` can be freely modified
5. **May add test files.** New or modified tests in `tests/` or inline `#[cfg(test)]` modules are fine

### Plugin Quality

- [ ] HA plugin dedup: extract shared Lua module for duplicated `get_config`/`headers`/`filters` across 21 files
- [ ] Compose plugin: simplify inline action helper arg-building
- [ ] Audit all plugins for missing icons — every plugin must have one
- [ ] Audit shell plugins for jq safety — no raw `$var` interpolation in JSON strings
- [ ] Improve plugin error output: convert raw stderr to user-friendly messages where feasible in plugin code

### Test Coverage

- [ ] Manifest validation tests for all 39 plugins (valid TOML, required fields present)
- [ ] Output format smoke tests (valid JSON structure) for plugins with testable output
- [ ] `init-plugin` scaffolder edge case tests

### Documentation

- [ ] Plugin development guide improvements
- [ ] Example plugin READMEs
- [ ] Keybinding reference accuracy check vs actual defaults in code

### Haiku Tier (trivial — smallest models)

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

### Sonnet Tier (moderate — mid-tier models)

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
