# Current State

> Updated: 2026-04-14

## Active Branch

`main`

## Recent Progress (this session)

- **v0.7.0 complete:** three new plugins fully implemented
  - Notes/Obsidian: 4 commands — search (full-text grep), recent (mtime-sorted), browse (folder nav), settings (vault path)
  - Tailscale: 3 commands — devices (peer listing + SSH/ping), exit nodes (select/disable), network (tailnet overview)
  - Linear: 3 commands — my issues (assigned, GraphQL), current cycle (progress/issues), triage (triage queue)
- **Bug fix:** h/Left arrow now navigates between widget cards (was missing symmetric handler)

## Current Version

v0.5.0 (released on GitHub, Homebrew tap auto-updated)
v0.6.0 ready to release (all deep-dives + UX complete)
v0.7.0 ready to release (3 new plugins: Notes, Tailscale, Linear)

## Validation

- `cargo test` — passing
- `cargo clippy -- -D warnings` — clean

## Plugin Command Counts

| Plugin | Commands | Widgets |
|--------|----------|---------|
| Git | 8 | 3 |
| Docker | 6 | 1 |
| GitHub | 5 | 5 |
| SSH | 4 | 2 |
| Weather | 3 | 1 |
| Kubernetes | 6 | 2 |
| Notes | 4 | 1 |
| Tailscale | 3 | 1 |
| Linear | 3 | 2 |
