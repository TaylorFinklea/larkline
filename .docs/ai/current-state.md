# Current State
> Loop-state only — history → git/decisions; backlog → beads; detail → phases.
**Branch:** `v1.0-prep`; v1.0 features code-complete; hardening epic `larkline-mkv` active.

## Plan
<!-- Pull exactly one item with `bd ready`; phases stay in beads. -->
- Week 1 hardening COMPLETE (`mkv.1`–`mkv.7`, `.9`, `.10`; `.4` already-done; `.8` → v1.1).
  `mkv.12` async shell/nvim dispatch landed (BgCommandEvent channel + navigate-away guard).
  Next via `bd ready`: `mkv.11` (streaming through Plugin trait; LAUNCH slice per Sol
  split — in-flight-registry half deferred v1.1), then `.13` (blocked on `.11`).
- Launch gate remaining: `.12/.13` async dispatch; `.9` execution identity + `.10` cache owner landed.
- `.8` full stable-ID migration deferred to v1.1; not a `.9` dependency.

## Blockers
- `cargo test`: 2 known `cli_action_test` failures from installed-plugin dependency;
  `mkv.28` owns hermeticity. Lib/bin + other integrations + strict clippy clean.
- `cargo fmt -- --check`: pre-existing drift in `config.rs` + `onboarding.rs`.
- Taylor-gated provider/Mail/k8s QA → `mkv.32` go/no-go.

## Open questions
- None.
