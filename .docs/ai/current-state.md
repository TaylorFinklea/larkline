# Current State

> Updated: 2026-04-05

## Active Branch

`main`

## Recent Progress (this session)

- **v0.6.0 UX polish (all 4 items complete):**
  - Widget card Enter → drill into full command output (`WidgetCardOpen` action, `open_plugin_in_view_output`)
  - Power menu "Upgrade lark" (U key) — shows confirmation with brew/cargo command when update available
  - Widget picker search/filter — type to filter entries, Backspace to clear, title shows match count
  - Better plugin error display — categorized icons (timeout/invalid/failed), word-wrapped message, recovery hints
- Previous session: roadmap restructure, handoff migration, AI Projects plugin, Git Sync command, backlog audit

## Current Version

v0.5.0 (released on GitHub, Homebrew tap auto-updated)

## Validation

- `cargo test` — 141 tests passing
- `cargo clippy -- -D warnings` — clean
- `cargo fmt -- --check` — clean
