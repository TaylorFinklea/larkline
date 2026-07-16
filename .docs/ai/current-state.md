# Current State
> Loop-state only — history → git/decisions; backlog → beads; detail → phases.
**Branch:** `v1.0-prep`; v1.0 features code-complete; hardening epic `larkline-mkv` active.

## Plan
<!-- Pull exactly one item with `bd ready`; phases stay in beads. -->
- Week 1 hardening COMPLETE: `mkv.1`–`mkv.7`, `mkv.9`, `mkv.10` all closed
  (`mkv.4` already-done via mkv.10; `mkv.8` deferred v1.1). Next via `bd ready`:
  launch-gate `.12/.13` async dispatch, or `mkv.11` (subsumes the
  `prefetch_in_flight` guard into the command_id in-flight registry).
- Launch gate remaining: `.12/.13` async dispatch; `.9` execution identity + `.10` cache owner landed.
- `.8` full stable-ID migration deferred to v1.1; not a `.9` dependency.

## Blockers
- `cargo test`: 2 known `cli_action_test` failures from installed-plugin dependency;
  `mkv.28` owns hermeticity. Lib/bin + other integrations + strict clippy clean.
- `cargo fmt -- --check`: pre-existing drift in `config.rs` + `onboarding.rs`.
- Taylor-gated provider/Mail/k8s QA → `mkv.32` go/no-go.

## Open questions
- None.
