# Current State

> Updated: 2026-04-03

## Active Branch

`main`

## Recent Progress (this session)

- **Handoff migration**: moved `docs/ai/` → `.docs/ai/` to match global Claude Code standard
  - Updated CLAUDE.md to defer handoff workflow to global instructions
  - Updated AGENTS.md references
  - Updated internal cross-references in handoff-template.md and next-steps.md
- **Roadmap v3 restructure**: split into themed releases (expensive model phases) + backlog (smaller model parallel work)
  - v0.6.0 = Plugin Deep-Dives (k8s, GitHub, SSH, Weather) + 4 required UX items
  - v0.7.0 = New Plugins (Obsidian, Tailscale, Linear)
  - Backlog has guardrails: no core Rust, no new deps, tests must pass

## Current Version

v0.5.0 (released on GitHub, Homebrew tap auto-updated)

## Validation

- `cargo test` — 141 tests passing
- `cargo clippy -- -D warnings` — clean
- `cargo fmt` — applied (pre-existing fmt nit in input.rs)
