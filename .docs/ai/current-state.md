# Current State

> Updated: 2026-04-04

## Active Branch

`main`

## Recent Progress (this session)

- **AI Projects plugin**: new multi-command Lua plugin (`examples/plugins/ai-projects/`)
  - Dashboard: auto-scans `~/git` for `.docs/ai/` or `docs/ai/`, shows recency dots, branch, next-step counts
  - Sub-commands: Current State, Next Steps, Roadmap, Decisions — each parses the markdown into structured items
  - Widget-enabled with 2-minute refresh
  - Shared `lib.lua` helper: project discovery, markdown parsing, recency calculation
  - 7 files: manifest.toml, lib.lua, dashboard.lua, current-state.lua, next-steps.lua, roadmap.lua, decisions.lua
- **Handoff migration**: moved `docs/ai/` → `.docs/ai/` to match global standard
- **Roadmap v3**: themed releases + backlog tiers for smaller AI models

## Current Version

v0.5.0 (released on GitHub, Homebrew tap auto-updated)

## Validation

- `cargo test` — 141 tests passing
- `cargo clippy -- -D warnings` — clean
- Lua syntax — all 6 plugin files pass `luac -p`
