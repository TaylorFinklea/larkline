# Current State
> Loop-state only — history → git/decisions; backlog → beads; detail → phases.
**Branch:** `v1.0-prep`; v1.0 features code-complete; hardening epic `larkline-mkv` active.

## Plan
<!-- Pull exactly one item with `bd ready`; phases stay in beads. -->
- ALL pre-gate P1s COMPLETE (`.9`–`.13`, `.26`, `.27`, `.31`).
- `mkv.32` [?] awaiting human verify: agent pre-flight green (tests/clippy/fmt/release
  build/CLI smoke); manual QA runbook published to harness-deck
  (`~/.harness/reports/larkline/20260717-mkv32-rehearsal/` — Parts A/B/C + go/no-go
  approval block). Taylor works the runbook, answers in the dashboard; agent picks up
  `responses.json` next session. v1.0 tag gated on GO.
- `.8` full stable-ID migration deferred to v1.1; not a `.9` dependency.
- NOTE for release: tag releases `v{version}` — `lark plugin sync` now pins to that tag.

## Blockers
- `cargo test`: 2 known `cli_action_test` failures from installed-plugin dependency;
  `mkv.28` owns hermeticity. Lib/bin + other integrations + strict clippy clean.
- `cargo fmt -- --check`: CLEAN repo-wide (drift in config.rs/onboarding.rs formatted away).
- Taylor-gated QA → `mkv.32` go/no-go (runbook live in harness-deck; see Plan).
- Release process: push 18 commits + tag `v{version}` before fresh-install tests (C1).

## Open questions
- None.
