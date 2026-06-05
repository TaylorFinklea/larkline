# Current State

> Loop-state only — ≤20 lines, fragments. History → `git log` + `decisions.md`;
> pending work → `roadmap.md` Now/Backlog; multi-session detail → `phases/`.

**Branch:** `v1.0-prep` — several commits ahead of origin (2026-06-05 session:
web-search plugin, from_exit activation, theme polish, bug bash). Build green:
251 bin tests, `clippy --bin lark -D warnings`, `fmt`. Binary reinstalled.

## Plan

<!-- The ONE active roadmap Now item, expanded into phase checkboxes. Empty = pull the top Now item. -->

(none active — next iteration pulls the top `roadmap.md` Now item; onboarding
wizard is next but BLOCKED on a UX decision — see Backlog)

## Blockers

- Onboarding wizard (9b) needs a UX call: `Mode::Onboarding` vs auto-launched plugin.
- Taylor-gated QA (API keys / live TUI): real-provider AI smoke (5/6/8), Mail
  mutating actions, agent-cancel, Caffeinate + web-search submit, **Docker/
  Bitwarden error-state + empty-state checks** (post bug-bash), **k8s re-link**
  (stale copy). All in `roadmap.md` → Backlog → Taylor-gated QA.

## Open questions

- (resolved 2026-06-05) Exec mode = ultracode workflows; see
  [[feedback_ultracode_for_hard_tasks]].
