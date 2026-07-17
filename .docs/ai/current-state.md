# Current State
> Loop-state only — history → git/decisions; backlog → beads; detail → phases.
**Branch:** `v1.0-prep`; v1.0 features code-complete; hardening epic `larkline-mkv` active.

## Plan
<!-- Pull exactly one item with `bd ready`; phases stay in beads. -->
- Week 1 + launch-gate async chain COMPLETE (`.9`–`.13`; `.11` registry half → `mkv.35`).
  `.26` parity + `.27` pinned atomic sync (`v{version}` tag, `--unpinned` opt-out,
  `host_api` manifest gate) landed. Remaining P1s via `bd ready`: `.31` plugin
  fix-it batch, then `.32` Taylor-gated launch rehearsal (go/no-go).
- `.8` full stable-ID migration deferred to v1.1; not a `.9` dependency.
- NOTE for release: tag releases `v{version}` — `lark plugin sync` now pins to that tag.

## Blockers
- `cargo test`: 2 known `cli_action_test` failures from installed-plugin dependency;
  `mkv.28` owns hermeticity. Lib/bin + other integrations + strict clippy clean.
- `cargo fmt -- --check`: CLEAN repo-wide (drift in config.rs/onboarding.rs formatted away).
- Taylor-gated provider/Mail/k8s QA → `mkv.32` go/no-go.

## Open questions
- None.
