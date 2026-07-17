# Current State
> Loop-state only — history → git/decisions; backlog → beads; detail → phases.
**Branch:** `v1.0-prep`; v1.0 features code-complete; hardening epic `larkline-mkv` active.

## Plan
<!-- Pull exactly one item with `bd ready`; phases stay in beads. -->
- ALL pre-gate P1s COMPLETE: `.9`–`.13`, `.26`, `.27`, `.31` (6-commit fix-it batch:
  confirm:true all kinds; secrets env for shell actions + exec_io opts.env; Bitwarden/HA
  secrets out of argv+screen; HA favorite/hide disabled; docker/k8s hangers removed;
  Jira /search/jql; clipboard os crash). Next: `mkv.32` Taylor-gated launch rehearsal
  (go/no-go — needs Taylor at the keyboard for provider/Mail/k8s QA).
- `.8` full stable-ID migration deferred to v1.1; not a `.9` dependency.
- NOTE for release: tag releases `v{version}` — `lark plugin sync` now pins to that tag.

## Blockers
- `cargo test`: 2 known `cli_action_test` failures from installed-plugin dependency;
  `mkv.28` owns hermeticity. Lib/bin + other integrations + strict clippy clean.
- `cargo fmt -- --check`: CLEAN repo-wide (drift in config.rs/onboarding.rs formatted away).
- Taylor-gated provider/Mail/k8s QA → `mkv.32` go/no-go.

## Open questions
- None.
