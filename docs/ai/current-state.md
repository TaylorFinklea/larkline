# Current State

> Updated: 2026-04-03

## Active Branch

`main` (clean, all committed)

## Recent Progress (this session)

- **v0.4.0 released**: Widgets, plugin manager, git deep-dive, developer plugin
- **v0.5.0 released**: Update checker, Docker deep-dive (6 commands)
- CI fixes: cargo fmt alignment, clippy `unnecessary_filter_map` (Rust 1.94), `too_many_lines` allows
- TAP_TOKEN secret set up for automated Homebrew tap updates (was missing)
- **Background update checker** (`src/update.rs`): checks GitHub releases API daily, caches to `~/.local/share/larkline/update-check.json`, detects Homebrew vs Cargo install method, shows hint in status bar
- **Docker plugin deep-dive**: 6 commands covering full Portainer workflow — Containers (with CPU/mem stats, logs, exec shell, widget), Compose (stacks with logs/services/lifecycle), Images (pull/prune/history), Volumes, Networks (new), System (new — disk usage, prune all)
- **Widget picker overlay**: press `A` in Normal mode to open a centered popup with all widget-eligible commands, toggle on/off with Space, persists to plugin-manager.json
- **Widget discoverability**: status bar now shows `K widgets` when visible, `W show widgets` when hidden, full keybinding hints when focused (`h/l reorder`, `A add/remove`, `D disable`)

## Current Version

v0.5.0 (released on GitHub, Homebrew tap auto-updated)

## Key Files Changed

- `src/update.rs` — new module: version check, install detection, cache
- `src/app.rs` — WidgetPickerState, WidgetPickerEntry, picker action handlers, update checker background task
- `src/action.rs` — WidgetPickerOpen/Close/Toggle/Up/Down actions
- `src/input.rs` — handle_widget_picker, `A` keybinding, widget_picker intercept in dispatch chain
- `src/tui/ui.rs` — render_widget_picker popup, status bar widget hints (K/W/A contextual)
- `examples/plugins/docker/` — 6 files: containers, compose, images, volumes, networks, system

## Validation

- `cargo test` — 141 tests passing
- `cargo clippy -- -D warnings` — clean
- `cargo fmt -- --check` — clean
- CI passing on both macOS and Ubuntu
