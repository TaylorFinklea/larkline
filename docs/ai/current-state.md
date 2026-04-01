# Current State

> Updated: 2026-03-31

## Active Branch

`main` (clean)

## Recent Progress (this session)

- Phase 29: Raycast-style flat list, quickkey priority, cursor reset, vim mode fixes
- Two-step Esc everywhere (main search + output search)
- Plugin Manager: LazyVim-style enable/disable with full-screen tree view
- Home Assistant: 21 commands with filters, favorites, hide, resp.body fix, os.getenv fix
- Dashboard Widgets: bordered card panes with auto-refresh, context-aware navigation
- Widget management: reorder (H/L), hide (D), toggle visibility (W), persist order
- Context-aware power menu: adapts like which-key based on focused element
- Calendar: My Schedule threaded ANSI timeline, auto-detect RawText mode
- Calculator: qalc/bc backend, space in form fields fix
- Claude Usage + Codex Usage plugins with time range settings
- lark.nvim: Neovim floating terminal with --query flag
- Secrets: macOS Keychain fallback + lark secret CLI
- Plugin distribution: lark plugin sync/list/remove
- Automated release pipeline: scripts/release.sh + CI tap update
- Homebrew formula renamed to larkline (avoid LarkSuite conflict)
- v0.3.0 + v0.3.1 released

## Validation

- `cargo test` — all tests passing
- `cargo clippy -- -D warnings` — clean
