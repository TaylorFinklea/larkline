# Current State

> Updated: 2026-03-27

## Active Branch

`main` (clean)

## Recent Progress

- Phase 29: Flat list — removed `GroupHeader` rows, plugin name shown inline as dimmed badge
- Descriptions hidden by default (`show_descriptions: false`)
- Quickkey exact-match pins command to top of search results (`u32::MAX` score)
- Cursor resets to position 0 on every search query change
- Back from ViewOutput and Enter on result both enter Normal mode (no more j/k typing into search)

## Changed Files (this session)

- `src/app.rs` — `rebuild_unified_list` flat emit, quickkey priority, cursor reset, vim mode transitions
- `src/tui/ui.rs` — removed GroupHeader render arm, badge format simplified
- `src/config.rs` — `show_descriptions` default false

## Blockers

None.

## Open Questions

- Sidebar width ratio when drilled in — Taylor wants ~2/7, current is 2/3. Need to decide if this is a config value or hardcoded.
- Arrow key parity with hjkl — is this just mapping Right→`l` and Left→`h`, or are there edge cases in different modes?

## Validation

- `cargo test` — 138 tests passing
- `cargo clippy -- -D warnings` — clean
- Manual testing confirmed: flat list renders, quickkey `ca` pins Calendar, Normal mode on Back/Enter
