# Current State

> Loop-state only — ≤20 lines, fragments. History → `git log` + `decisions.md`;
> pending work → `roadmap.md` Now/Backlog; multi-session detail → `phases/`.

**Branch:** `v1.0-prep` — 2 commits ahead of origin (ADR-011, glance-sync fix);
glance-strip + caffeinate-rewrite batch already pushed. Build green: 62 lib +
247 bin, `clippy --all-targets -D warnings`, `fmt`.

## Plan

<!-- The ONE active roadmap Now item, expanded into phase checkboxes. Empty = pull the top Now item. -->

(none active — next iteration pulls the top `roadmap.md` Now item)

## Blockers

- Taylor-gated QA only (API keys / live TUI): real-provider AI smoke
  (Phases 5/6/8), Mail mutating actions, agent-cancel path, Caffeinate
  Start/Extend. Tracked in `roadmap.md` → Backlog → "Taylor QA".

## Open questions

- Execution mode for remaining v1.0 work — bite-sized clear-context vs
  ultracode workflows (raised 2026-06-05).
