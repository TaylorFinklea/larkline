# Current State

> Loop-state only — ≤20 lines, fragments. History → `git log` + `decisions.md`;
> pending work → beads (`bd ready`, epic `larkline-mkv`); multi-session detail → `phases/`.

**Branch:** `v1.0-prep` (all v1.0 feature phases code-complete). Working now:
**v1.0 hardening program** — ADR-012, spec `phases/hardening-v1.0-spec.md`,
beads epic `larkline-mkv` (31 items, 6 weekly milestones).

## Plan

<!-- The hardening backlog lives in beads, not here. Pull with `bd ready`. -->
Next iteration: `bd ready` → top P0 is `larkline-mkv.1` (frame/startup
instrumentation — lands FIRST so perf fixes are measured). Sequencing gate:
`.8` stable IDs → `.9` exec IDs → `.12/.13` async dispatch (deps wired in bd).

## Blockers

- Full test suite NOT hermetic on `v1.0-prep`: `tests/cli_action_test.rs`
  depends on the user's installed plugins (2 failures reproduced) — fixed in
  `larkline-mkv.28`. `cargo test --bin lark` + clippy `--all-targets` clean.
- Taylor-gated QA (real-provider AI, agent cancel, Mail mutations, k8s relink)
  rolled into `larkline-mkv.32` (v1.0 go/no-go).

## Open questions

- (resolved 2026-07-15) Backlog home = beads `backlog-larkline` Dolt remote
  (bootstrapped this session); hardening-first per Taylor.
