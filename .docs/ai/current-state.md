# Current State

> Updated: 2026-04-15

## Active Branch

`main`

## Recent Progress (this session)

- **v0.8.0 complete:** Mini App Mode — 6 phases (A-F)
  - Phase A: Action chaining via `on_action` Lua callback, `ActionKind::Chain`/`UpdatePane`
  - Phase B: `MiniAppLayout` recursive tree, `PaneContent`, `Mode::MiniApp`, manifest `mini_app` field
  - Phase C: Recursive split-pane rendering in ratatui, `render_pane()` with focused border accent
  - Phase D: Dedicated `handle_mini_app()` input handler, Tab/Ctrl+h/l focus cycling, per-pane j/k/Enter
  - Phase E: User-initiated split/close/resize with tree mutation helpers
  - Phase F: `lark.clipboard_read()` host API, clipboard history plugin rewrite, Docker Dashboard mini app

## Current Version

v0.5.0 (released)
v0.6.0 ready (plugin deep-dives + UX)
v0.7.0 ready (Notes, Tailscale, Linear plugins)
v0.8.0 ready (mini app mode)

## Validation

- `cargo test` — 160 tests passing
- `cargo clippy -- -D warnings` — clean

## Key Files Added/Modified (v0.8.0)

| File | What |
|------|------|
| `src/mini_app.rs` | NEW — layout tree helpers, split/close/resize mutations |
| `src/plugin/traits.rs` | `MiniAppLayout`, `PaneContent`, `ActionKind::Chain/UpdatePane`, `Plugin::execute_action()` |
| `src/plugin/lua.rs` | `on_action` callback, `execute_action_inner()`, `lark.clipboard_read()` |
| `src/plugin/engine.rs` | `EngineEvent::ActionResult`, `execute_action()` |
| `src/app.rs` | `Mode::MiniApp`, `MiniAppState`, `PaneState`, all mini app action handling |
| `src/action.rs` | 9 new action variants |
| `src/input.rs` | `handle_mini_app()` with split/resize/focus keybindings |
| `src/tui/ui.rs` | `render_mini_app()`, `render_layout_node()`, `render_pane()` |
| `examples/plugins/docker/dashboard.lua` | NEW — reference mini app plugin |
| `examples/plugins/clipboard/history.lua` | Rewritten to use `lark.clipboard_read()` + `lark.store` |
