# Current State

> Updated: 2026-04-17

## Active Branch

`main`

## Recent Progress (this session)

- **v0.9.0 complete:** Internal Quality — 6 phases (A–F)
  - Phase A: Extracted widget helpers → `src/widgets.rs`
  - Phase B: Extracted `build_power_menu_categories` → `src/power_menu.rs`
  - Phase C: Extracted `build_plugin_manager_state*` → `src/plugin_manager_state.rs`
  - Phase D: Extracted output/form helpers → `src/app_output.rs`
  - Phase E: Split `handle_action` — widget/form/mini_app/plugin_manager handlers in dedicated modules
  - Phase F: Prefetch concurrency semaphore (cap 8), slow-plugin profiling, widget refresh skipped when dashboard hidden

## Current Version

v0.5.0 (released)
v0.6.0 ready (plugin deep-dives + UX)
v0.7.0 ready (Notes, Tailscale, Linear plugins)
v0.8.0 ready (mini app mode)
v0.9.0 ready (internal quality refactor + performance)

## Validation

- `cargo test` — 160 tests passing
- `cargo clippy -- -D warnings` — clean

## app.rs Reduction

| Phase | Before | After | Delta |
|-------|--------|-------|-------|
| Start | 4027 | — | — |
| A (widgets) | 4027 | 3954 | −73 |
| B (power menu) | 3954 | 3626 | −328 |
| C (plugin manager state) | 3626 | 3505 | −121 |
| D (output/form helpers) | 3505 | 3367 | −138 |
| E (handler split) | 3367 | 2851 | −516 |
| **Total** | **4027** | **2851** | **−1176 (−29%)** |

## New Modules (v0.9.0)

| File | Purpose |
|------|---------|
| `src/widgets.rs` | Widget state helpers (indices, ordering, preview sync) |
| `src/power_menu.rs` | `build_power_menu_categories()` — which-key overlay data |
| `src/plugin_manager_state.rs` | Plugin Manager overlay state builder |
| `src/app_output.rs` | Output pane + form helpers |
| `src/widget_actions.rs` | 12 widget action handlers |
| `src/form_actions.rs` | 11 form action handlers |
| `src/plugin_manager_actions.rs` | 6 plugin manager action handlers |
| `src/mini_app.rs` | (expanded) Added 9 mini app action handlers |

## Performance Changes

- `PluginEngine` now caps concurrent prefetch to 8 via `tokio::sync::Semaphore`. User-selected runs bypass.
- `log_execution_time()` logs `debug!` for fast plugins, `warn!` for those ≥500ms.
- Widget auto-refresh tick skips entirely when dashboard is hidden, widgets are empty, or mode is not `Unified`. Iterates `widget_indices` instead of all plugins.
