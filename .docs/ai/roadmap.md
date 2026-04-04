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
- [ ] Kubernetes: log streaming, describe pod, context switching, namespace picker
- [ ] GitHub: review request count, workflow status icons, PR quick-actions
- [ ] SSH: connection status, recent connections, quick-connect
- [ ] Weather: forecast view, location settings, hourly/daily toggle

**UX (required for release):**
- [ ] Widget card Enter → drill into full command output
- [ ] Update checker → power menu action (run upgrade command)
- [ ] Widget picker search/filter for large plugin lists
- [ ] Better plugin error display: user-friendly messages + retry hints

### v0.7.0 — New Plugins

Expand the plugin library with three new integrations:
- [ ] Obsidian/Notes: quick note search, recent notes, vault browser
- [ ] Tailscale/VPN: device status, exit nodes, network overview
- [ ] Linear/Jira: assigned issues, sprint board, quick status changes

### Future (unordered — pick theme per release)

- **Distribution:** crates.io publish, AUR package, Nix flake, `lark plugin sync` update-in-place
- **lark.nvim v2:** Telescope integration, action dispatch back to Neovim, bidirectional comms
- **Performance:** prefetch tuning, slow-plugin profiling, widget refresh optimization (skip when not visible)
- **app.rs refactor:** split 3600+ line god-object into submodules (state, execution, widgets) — phase work only

---

## Backlog (parallel work for smaller models)

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
