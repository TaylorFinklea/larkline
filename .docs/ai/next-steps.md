# Next Steps

> Updated: 2026-04-05

## v0.6.0 — Ready to Release

All items complete:
- [x] GitHub deep-dive (5 commands)
- [x] SSH deep-dive (4 commands)
- [x] Weather deep-dive (3 commands)
- [x] Kubernetes deep-dive (6 commands)
- [x] Widget card Enter drill-in
- [x] Update checker → power menu upgrade action
- [x] Widget picker search/filter
- [x] Better plugin error display

**To release:** Bump `Cargo.toml` version to 0.6.0, tag, push.

## v0.7.0 — New Plugins

- [ ] Obsidian/Notes: quick note search, recent notes, vault browser
- [ ] Tailscale/VPN: device status, exit nodes, network overview
- [ ] Linear/Jira: assigned issues, sprint board, quick status changes

## Backlog (can be done by smaller models in parallel)

See `.docs/ai/roadmap.md` → Backlog section for full list and guardrails.

**Quick wins to start with:**
- [ ] HA plugin dedup: shared Lua module for 21 files of duplicated get_config/headers/filters
- [ ] Audit all plugins for missing icons
- [ ] Audit shell plugins for jq safety
- [ ] Manifest validation tests for all 39 plugins

## Release Notes (unpushed since v0.5.0)

- Widget picker overlay (A key)
- Widget discoverability status bar hints
- AI Projects plugin (cross-project handoff dashboard)
- Git Sync command (repos needing push/pull)
- v0.6.0 UX: widget drill-in, upgrade menu, picker search, error display
- Plugin deep-dives: GitHub (5 cmd), SSH (4 cmd), Weather (3 cmd), Kubernetes (6 cmd)
