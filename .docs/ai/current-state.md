# Current State

> Updated: 2026-04-25

## Active Branch

`main`

## Recent Progress

### v0.13.0 — lark.nvim v3 (Telescope-native picker)

Six commits in larkline + three commits in the new `TaylorFinklea/lark.nvim` repo. Larkline ships the headless contract; the nvim plugin consumes it.

**larkline (`main`):**

| Phase | Commit | Summary |
|---|---|---|
| A | `cf7bbb9` | `lark list --json` for headless plugin enumeration |
| B | `03d6167` | Extract action dispatcher from TUI to `crate::actions` |
| C | `380175a` | `lark action <plugin> --action-json '<JSON>' [--confirm]` with `side` / `chained` / `needs_confirmation` outcomes |
| D | `e451c91` | Extract `lark.nvim/` subdir to TaylorFinklea/lark.nvim (fresh history) |
| H | (this) | Roadmap + handoff docs + CHANGELOG + `docs/plugins/lark-nvim.md` |

**TaylorFinklea/lark.nvim (`main`):**

| Phase | Commit | Summary |
|---|---|---|
| Init | `1946757` | Initial commit from extracted v2 state |
| E | `1ee519c` | Telescope plugin list picker via `lark list --json` |
| F | `…dispatch` | Results picker, action dispatch, chain stacking |
| G | `c6c2113` | Floating-terminal fallback for forms / mini-apps; new keymap |

### Architecture summary

The Rust side gained two new public-facing CLI subcommands and an extracted dispatcher module:

- `lark list --json` — emits `Vec<ListEntry>` JSON to stdout.
- `lark action <plugin> --action-json '<JSON>' [--confirm]` — fires a single `ItemAction` against a plugin, prints a tagged `ActionWireOutcome`.
- `crate::actions::{execute, ActionResult, side_effects}` — TUI-free dispatcher used by `lark action`. The TUI's `App::execute_item_action` is unchanged (Phase B intentionally non-converging — that's deferred).

The Lua side (lark.nvim) picks up the contract:

- `lua/lark/cli.lua` — wraps `vim.system` with stdout-as-JSON helpers.
- `lua/lark/picker.lua` — main Telescope picker.
- `lua/lark/results.lua` — secondary picker over `PluginOutput.items`. Push-on-stack chain semantics via Telescope's nested-picker default.
- `lua/lark/actions.lua` — outcome handler. `side` → vim.notify (or scratch float for long stdout); `chained` → push picker; `needs_confirmation` → vim.fn.confirm + re-issue with `--confirm`.
- `lua/lark/fallback.lua` — drop into the floating-terminal TUI when `output.form` or `output.layout` is set.

## Current Version

`Cargo.toml` at `0.12.0`. Tag `v0.13.0` on Taylor's signal — pipeline:

```sh
bash scripts/release.sh set 0.13.0
```

## Validation

- `cargo test` — 194 passed (was 186 pre-v0.13.0; +8 across the new actions module + cli_list_test + cli_action_test).
- `cargo clippy -- -D warnings` — clean.
- `cargo fmt -- --check` — clean.
- `nvim --headless` — both lark.nvim modules load cleanly under default config.

## Pre-Release Gates

1. **Smoke-test end-to-end against a real Telescope install.** Steps in `.docs/ai/next-steps.md`. The Rust side is fully test-covered; the Lua side has only headless module-load smoke tests.
2. **OAuth client_id** (carried over from v0.12.0) — Atlassian OAuth is still gated on registering the public OAuth 2.0 (3LO) app at developer.atlassian.com; Taylor can do this whenever or stick with the API-token path (which works today).

## Next

See `.docs/ai/next-steps.md`.
