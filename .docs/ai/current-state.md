# Current State

> Updated: 2026-04-05

## Active Branch

`main`

## Recent Progress (this session)

- **v0.6.0 complete:** all plugin deep-dives + UX items done
  - GitHub deep-dive: 5 commands — my-prs (merge/squash/close), reviews (approve/request-changes), notifications (mark-read), issues (close), workflows
  - SSH deep-dive: 4 commands — hosts (nc reachability), connections (active), recent (shell history), keys (fingerprints/agent)
  - Weather deep-dive: 3 commands — current (weather icons, astronomy), forecast (3-day hourly), locations (saved via lark.store)
  - Kubernetes deep-dive: 6 commands — enhanced pods, deployments (scale/restart/rollout), namespaces (resource counts), logs (per-container), contexts
  - UX: widget drill-in, upgrade menu, picker search, error display

## Current Version

v0.5.0 (released on GitHub, Homebrew tap auto-updated)
v0.6.0 ready to release (all deep-dives + UX complete, committed on main)

## Validation

- `cargo test` — 141 tests passing
- `cargo clippy -- -D warnings` — clean
- `cargo fmt -- --check` — clean

## Plugin Command Counts

| Plugin | Commands | Widgets |
|--------|----------|---------|
| Git | 8 | 3 |
| Docker | 6 | 1 |
| GitHub | 5 | 5 |
| SSH | 4 | 2 |
| Weather | 3 | 1 |
| Kubernetes | 6 | 2 |
