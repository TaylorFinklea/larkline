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
| v0.2.0 | Published release: Homebrew formula, 35 plugins, CHANGELOG |

## Completed (post-polish)

- [x] UX polish: flat list, quickkey priority, sidebar ratio, icons, vim mode transitions
- [x] Secrets: .env + env var + macOS Keychain fallback, `lark secret set/list/delete` CLI
- [x] Standard plugin library: 5 new plugins filling Raycast gaps
- [x] Home Assistant plugin: devices, toggle, scenes, automations
- [x] Plugin Manager: full-screen tree view, enable/disable, settings, secret status
- [x] Publishing: v0.2.0 release, Homebrew tap updated

## Next Up

### Neovim Plugin (`lark.nvim`)

Neovim integration that opens Lark as a floating terminal inside Neovim, with context awareness.

**Core idea:**
- Open Lark in a floating terminal window (`:Lark` command or keymap)
- Set `LARK_CWD` to the buffer's project root (git root) so plugins like Git, File Search, and Ports use the correct context
- Pass the current file path as `LARK_FILE` for file-aware plugins
- On action completion (e.g. open file), send the result back to Neovim (open buffer, run command, etc.)

**Implementation approach:**
- Lua plugin for Neovim (`lua/lark/init.lua`)
- Uses `vim.fn.termopen()` or `vim.api.nvim_open_term()` in a floating window
- Passes environment variables for context (`LARK_CWD`, `LARK_FILE`, `LARK_FILETYPE`)
- Optional: `lark invoke` JSON output piped back to Neovim for action dispatch
- Installable via lazy.nvim: `{ "tfinklea/lark.nvim" }`

**Stretch goals:**
- Telescope-style picker that uses Lark's plugin results as a source
- `:LarkSearch <query>` that pre-fills the search field
- File Search results open directly in Neovim buffers
- Git plugin actions (checkout branch, etc.) run in Neovim's terminal

### Dashboard Widgets (Priority: High)

Auto-refreshing status widgets at the top of the unified list — like Raycast menu bar items but terminal-native. Commands you use frequently (Git Status, GitHub PRs, Workflow Runs) show live summaries without needing to open them.

**Core idea:**
- Plugins opt in via a new `widget` field in manifest or command config
- Widget defines: what data to show (a compact summary), refresh interval (cron-like)
- Widgets render as a compact row/section above the command list
- Clicking/entering a widget opens the full command output
- The prefetch cache already powers background execution — widgets extend this with scheduled re-execution and a compact display format

**Design questions to resolve:**
- Widget display format: single-line summary? Multi-line card? Configurable?
- Plugin API: `on_widget()` callback returning a compact summary vs. deriving from existing `on_run()` output (e.g., first N items, item count, status text)
- Scheduling: per-widget cron string in manifest, or a global "widget refresh" interval?
- Configuration: which widgets are active should be user-configurable (Plugin Manager?)
- Layout: horizontal row of widgets at top? Vertical section? Collapsible?

**Example manifest:**
```toml
[[commands]]
name = "Status"
entry = "status.lua"
prefetch = true
widget = true
widget_refresh = "30s"
widget_summary = "count"  # or "first", "custom"
```

**Inspiration:**
- Raycast menu bar extras (weather, media, batteries — compact, auto-refresh)
- LazyVim dashboard (recent files, project info at launch)
- tmux status bar (compact, always-visible info)
- Datadog/Grafana dashboards (widgets with configurable data sources)

### Plugin Deep-Dive

Iterate on each plugin individually to bring quality up to Raycast standards:
- Better error handling and edge cases
- Richer actions per item
- Loading states and caching tuning
- Documentation and screenshots

### Publishing & Distribution

- Publish to crates.io (`cargo install larkline`)
- AUR package for Arch Linux
- Nix flake
- Automated release pipeline (done: scripts/release.sh + CI tap update)

## Constraints

- Terminal only. No GUI dependencies.
- Sub-100ms startup.
- The `Plugin` trait is the only interface between backends and the engine.
- TUI reads state, never owns it. State transitions happen in `app.rs`.

## Non-Goals

- GUI/Electron wrapper
- Plugin marketplace / remote registry (local-first)
- Mouse-only workflows
