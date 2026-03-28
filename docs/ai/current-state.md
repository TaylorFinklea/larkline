# Current State

> Updated: 2026-03-27

## Active Branch

`main` (clean)

## Recent Progress

- Phase 29: Flat list with inline plugin name badges
- Descriptions hidden by default, quickkey exact-match priority, cursor reset on search
- Normal mode on Back/Enter from ViewOutput
- Brew Update command added (multi-command plugin)
- AI handoff workflow: `docs/ai/` shared state + session protocol
- Fixed empty `icon_nerd = ""` replacing emoji with blank (3 sites)
- Filled in Nerd Font icons for all 16 plugins that had gaps
- Configurable `sidebar_ratio` (default 50% browse, 28% ViewOutput)

## Changed Files (this session)

- `src/app.rs` — flat list, quickkey priority, cursor reset, vim mode, icon guard, sidebar_ratio
- `src/tui/ui.rs` — removed GroupHeader render, dynamic sidebar width
- `src/config.rs` — show_descriptions default, sidebar_ratio field + template
- `src/main.rs` — icon_nerd empty-string guard (2 sites)
- `examples/plugins/brew/` — multi-command manifest + update.lua
- `examples/plugins/*/manifest.toml` — 16 Nerd Font icons filled in
- `docs/ai/` — new handoff workflow files
- `CLAUDE.md`, `AGENTS.md` — handoff protocol added

## Blockers

None.

## Open Questions

None currently.

## Validation

- `cargo test` — all tests passing
- `cargo clippy -- -D warnings` — clean
