# Next Steps

> Updated: 2026-04-15

## v0.8.0 — Complete

All items complete:
- [x] Action chaining (on_action callback)
- [x] Mini app layout data model + manifest
- [x] Split-pane rendering
- [x] Mini app input + pane updates
- [x] User splits, resize, close
- [x] Clipboard history + Docker Dashboard mini app

**To release:** Bump `Cargo.toml` version to 0.8.0, tag, push.

## Backlog (can be done by smaller models in parallel)

See `.docs/ai/roadmap.md` → Backlog section for full list and guardrails.

## Release Notes (unpushed since v0.5.0)

- Widget picker overlay (A key)
- Widget discoverability status bar hints
- AI Projects plugin (cross-project handoff dashboard)
- Git Sync command (repos needing push/pull)
- v0.6.0 UX: widget drill-in, upgrade menu, picker search, error display
- Plugin deep-dives: GitHub (5 cmd), SSH (4 cmd), Weather (3 cmd), Kubernetes (6 cmd)
- v0.7.0 New plugins: Notes/Obsidian (4 cmd), Tailscale (3 cmd), Linear (3 cmd)
- Fix: h/Left arrow widget navigation, UTF-8 safe truncation
- v0.8.0 Mini App Mode:
  - Action chaining via `on_action` Lua callback
  - Full-screen split-pane UI (neovim-style)
  - User-initiated split/close/resize
  - `lark.clipboard_read()` host API
  - Clipboard history plugin (no Maccy dependency)
  - Docker Dashboard mini app reference plugin
