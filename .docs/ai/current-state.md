# Current State
> Loop-state only — history → git/decisions; backlog → beads; detail → phases.
**Branch:** `v1.0-prep`; v1.0 features code-complete; hardening epic `larkline-mkv` active.

## Plan
<!-- Pull exactly one item with `bd ready`; phases stay in beads. -->
- Week 1 + launch-gate async chain COMPLETE: `.9`–`.13` all closed (`.11` LAUNCH slice;
  registry half → `mkv.35`, blocked on `.8`/v1.1). Remaining P1s via `bd ready`:
  `.26` disabled-plugin parity, `.27` plugin sync pinning, `.31` plugin fix-it batch,
  then `.32` Taylor-gated launch rehearsal (go/no-go).
- Launch gate remaining: `.12/.13` async dispatch; `.9` execution identity + `.10` cache owner landed.
- `.8` full stable-ID migration deferred to v1.1; not a `.9` dependency.

## Blockers
- `cargo test`: 2 known `cli_action_test` failures from installed-plugin dependency;
  `mkv.28` owns hermeticity. Lib/bin + other integrations + strict clippy clean.
- `cargo fmt -- --check`: pre-existing drift in `config.rs` + `onboarding.rs`.
- Taylor-gated provider/Mail/k8s QA → `mkv.32` go/no-go.

## Open questions
- None.
