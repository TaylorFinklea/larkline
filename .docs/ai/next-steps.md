# Next Steps

> Updated: 2026-04-14

## v0.7.0 — Complete

All items complete:
- [x] Obsidian/Notes: search, recent, browse, settings (4 commands)
- [x] Tailscale/VPN: devices, exit nodes, network (3 commands)
- [x] Linear: my issues, current cycle, triage (3 commands, GraphQL API)

**To release:** Bump `Cargo.toml` version to 0.7.0, tag, push.

## Backlog (can be done by smaller models in parallel)

See `.docs/ai/roadmap.md` → Backlog section for full list and guardrails.

**Quick wins to start with:**
- [ ] HA plugin dedup: shared Lua module for 21 files of duplicated get_config/headers/filters
- [ ] Audit all plugins for missing icons
- [ ] Audit shell plugins for jq safety
- [ ] Manifest validation tests for all 42 plugins

## Release Notes (unpushed since v0.5.0)

- Widget picker overlay (A key)
- Widget discoverability status bar hints
- AI Projects plugin (cross-project handoff dashboard)
- Git Sync command (repos needing push/pull)
- v0.6.0 UX: widget drill-in, upgrade menu, picker search, error display
- Plugin deep-dives: GitHub (5 cmd), SSH (4 cmd), Weather (3 cmd), Kubernetes (6 cmd)
- v0.7.0 New plugins: Notes/Obsidian (4 cmd), Tailscale (3 cmd), Linear (3 cmd)
- Fix: h/Left arrow widget navigation
