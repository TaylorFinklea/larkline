# Next Steps

> Updated: 2026-04-05

## v0.6.0 — Plugin Deep-Dives (phase work — remaining)

### Plugin Deep-Dives
- [x] Git: richer status, branches, log, stash (v0.4.0)
- [x] Docker: full Portainer-style — 6 commands with stats, logs, exec, compose, networks, system (v0.5.0)
- [ ] Kubernetes: log streaming, describe pod, context switching, namespace picker
- [ ] GitHub: review request count, workflow status icons, PR quick-actions
- [ ] SSH: connection status, recent connections, quick-connect
- [ ] Weather: forecast view, location settings, hourly/daily toggle

### UX (all complete)
- [x] Widget card Enter → drill into full command output
- [x] Update checker → power menu action (U key) with confirmation dialog
- [x] Widget picker search/filter with match count
- [x] Better plugin error display: categorized icons, word wrap, recovery hints

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
