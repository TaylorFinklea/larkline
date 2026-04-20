# Current State

> Updated: 2026-04-20

## Active Branch

`main`

## Recent Progress

### v0.10.0 shipped (2026-04-19)

Tag `v0.10.0` cut with commit `627b8fc`. GitHub Actions kicked off the release pipeline (binaries, GitHub Release, Homebrew tap bump, crates.io publish — the last gated on `CRATES_IO_TOKEN` with auto-issue on failure).

### v0.10.0 follow-up fix (commit `fa0eeff`)

After ship, diagnosed and fixed a double-press-to-navigate bug on Ghostty. The TUI event loop processed at most one crossterm event per frame and filtered only `KeyEventKind::Press`. Terminals that emit non-Key events (resize, paste, focus) interleaved with key presses — or that report autorepeat as `KeyEventKind::Repeat` under the Kitty keyboard protocol — were making every other `j/k/h/l` feel like it disappeared. Fix: drain all queued events per frame with `event::poll(Duration::ZERO)` and accept both `Press` and `Repeat`. Committed locally; will ship with the next release (v0.11.0).

### v0.11.0 — Bitwarden plugin deep-dive

Shipped alongside v0.10.0's follow-up fix. Raycast-parity plugin using the official `bw` CLI, 6 commands:

- **Search Vault** — list all items with type-aware copy actions
- **Favorites** — `--favorite` filter
- **Folders** — browse by folder with `on_action` drill-in; includes a "No Folder" bucket for unfoldered items
- **Generate Password** — form with password + passphrase modes, full option coverage (length, symbols/numbers/ambiguous/minnumber/minspecial for passwords; words/separator/capitalize/include-number for passphrases) + regenerate chain
- **Sync Vault** — status (account/server/state/last-sync) + sync action
- **Lock Vault** — confirm-before-lock one-shot action

Item types covered: login (type 1, with TOTP chain action), secure note (2), card (3, number/CVV/holder/exp/brand), identity (4, email/phone/SSN/passport/license). Custom fields (hidden ones redacted in detail view). Session discovery via `BW_SESSION` env var with friendly lock-state error output.

Auth model: user runs `bw unlock --raw` once and exports `BW_SESSION`. Future rbw support in backlog.

## Current Version

`Cargo.toml` at `0.10.0`. v0.11.0 bump is pending — ready to cut whenever.

## Validation

- `cargo test shipped_plugin_manifests` — passes with new bitwarden plugin
- `cargo clippy -- -D warnings` — clean (last run post input-loop fix)
- `cargo fmt -- --check` — clean

## New Files This Session

| File | Purpose |
|------|---------|
| `examples/plugins/bitwarden/manifest.toml` | Plugin + 6 commands |
| `examples/plugins/bitwarden/lib.lua` | Canonical shared helpers (SYNC source) |
| `examples/plugins/bitwarden/items.lua` | Search vault + TOTP/detail drill-in |
| `examples/plugins/bitwarden/favorites.lua` | Favorite items view |
| `examples/plugins/bitwarden/folders.lua` | Folder browser with drill-in |
| `examples/plugins/bitwarden/generate.lua` | Password + passphrase generator |
| `examples/plugins/bitwarden/sync.lua` | Vault status + sync action |
| `examples/plugins/bitwarden/lock.lua` | Lock vault action |

## Next

See `.docs/ai/next-steps.md`.
