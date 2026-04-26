# Current State

> Updated: 2026-04-26

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

- `cargo test` — 204 passed (was 194 after v0.13.0; +10 from the post-tag backlog burn — `init_plugin_test` (4) and `plugin_output_smoke_test` (6)).
- `cargo clippy -- -D warnings` — clean.
- `cargo clippy --tests --all-targets -- -D warnings` — clean (was failing before the burn on pre-existing test-only lints in `cli_list_test.rs` and `plugin/traits.rs`; both fixed).
- `cargo fmt -- --check` — clean.
- `nvim --headless` — both lark.nvim modules load cleanly under default config.

## Post-v0.13.0 Backlog Burn (2026-04-26)

While waiting on the v0.13.0 smoke test + tag, knocked through the open backlog items that didn't need design decisions:

- **HA plugin** — `-- SHARED:` markers above `get_config`/`ha_headers`/`friendly_name`/`curl_service` across all 22 command files. Canonical copy stays in `helpers.lua` (which also serves as the "Helpers" command's entry). Pattern matches `ccusage`/`github`.
- **Compose plugin** — same per-helper `-- SHARED:` markers above `trim`/`split_lines`/`shell_action`/`clipboard_action`/`compose_action`; canonical copy in `docker/lib.lua`.
- **`init-plugin` integration tests** — `tests/init_plugin_test.rs` exercises the binary path with `XDG_CONFIG_HOME` redirection. 4 cases: Lua scaffold, shell scaffold (executable bit), multi-command scaffold, and refuses-to-overwrite.
- **Output schema smoke tests** — `tests/plugin_output_smoke_test.rs` runs 6 pure plugins through the engine and asserts well-formed `PluginOutput` shape: `Emoji`, `Hello World (Lua)`, `Timezones`, `Quicklinks`, `Calculator`, `Base64 Encode`. Other ~34 plugins skipped (network/auth/state).
- **Drive-by clippy fixes** — `cargo clippy --tests` was previously broken under Rust 1.95 because test code wasn't in the lint scope. Fixed `cli_list_test.rs` (`struct_excessive_bools`) and three `_ => panic!` matches in `plugin/traits.rs`.

Confirmed-skipped during the burn:
- "Plugin error output: convert raw stderr to user-friendly messages" — audit found plugins already use structured error items, not raw stderr passthrough. Remaining work is a UX-wording pass, not tech debt.
- Bitwarden backlog (rbw, Send, attachments, edit/delete, lock-after-clipboard, TOTP countdown) — these are real features. Belong in a future Bitwarden-themed milestone.

## Pre-Release Gates

1. **Smoke-test end-to-end against a real Telescope install.** Steps in `.docs/ai/next-steps.md`. The Rust side is fully test-covered; the Lua side has only headless module-load smoke tests.
2. **OAuth client_id** (carried over from v0.12.0) — Atlassian OAuth is still gated on registering the public OAuth 2.0 (3LO) app at developer.atlassian.com; Taylor can do this whenever or stick with the API-token path (which works today).

## Next

See `.docs/ai/next-steps.md`.
