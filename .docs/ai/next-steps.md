# Next Steps

> Updated: 2026-04-17

## v0.9.0 — Complete

All items complete:
- [x] Phase A: Extract widget helpers
- [x] Phase B: Extract power menu builder
- [x] Phase C: Extract plugin manager state builder
- [x] Phase D: Extract output/form helpers
- [x] Phase E: Split handle_action into handler modules
- [x] Phase F: Performance — prefetch cap, slow-plugin profiling, widget refresh skip

**To release:** Bump `Cargo.toml` version and tag.

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
- v0.9.0 Internal Quality:
  - Split 4027-line `app.rs` god-object into 8 focused modules (−29% to 2851 lines)
  - Prefetch concurrency cap (8 parallel) so slow startup on machines with many plugins
  - Slow-plugin profiling: plugins ≥500ms log a warn-level trace
  - Widget auto-refresh skipped when dashboard hidden
