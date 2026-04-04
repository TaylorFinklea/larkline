# Next Steps

> Updated: 2026-04-03

## v0.6.0 — Plugin Deep-Dives + UX Polish (phase work)

### Plugin Deep-Dives
- [x] Git: richer status, branches, log, stash (v0.4.0)
- [x] Docker: full Portainer-style — 6 commands with stats, logs, exec, compose, networks, system (v0.5.0)
- [ ] Kubernetes: log streaming, describe pod, context switching, namespace picker
- [ ] GitHub: review request count, workflow status icons, PR quick-actions
- [ ] SSH: connection status, recent connections, quick-connect
- [ ] Weather: forecast view, location settings, hourly/daily toggle

### UX (required for v0.6.0)
- [ ] Widget card Enter → drill into full command output
- [ ] Update checker → power menu action to run upgrade command
- [ ] Widget picker search/filter for large plugin lists
- [ ] Better plugin error display: user-friendly messages + retry hints

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
- These are committed but not yet released — next release will include them
