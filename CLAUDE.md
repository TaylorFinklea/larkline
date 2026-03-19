# Larkline — Claude Code Project Instructions

## Project Overview

Larkline is a Rust-based terminal command palette. Binary: `lark`. Crate: `larkline`.

**Source of truth:** `docs/ARCHITECTURE.md` — all architectural decisions, the phased roadmap, and technology choices live there. Read it before starting any phase.

**Roles:** Taylor is Product Owner/PM. Claude Code writes and maintains the codebase. Taylor makes product decisions. Be technical with Taylor — he's a strong DevOps/SRE architect with Python and TypeScript expertise.

## Build & Development

```bash
# Build
cargo build
cargo build --release

# Test
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
cargo fmt -- --check   # CI check mode

# Run
cargo run

# CLI flags (no TUI launched)
cargo run -- --version
cargo run -- --print-alias zsh   # also: bash, fish
```

**Rust edition:** 2024
**MSRV:** 1.85+

## Architecture Invariants

These are non-negotiable design decisions. Do not violate them:

1. **The `Plugin` trait is sacred.** All plugin backends implement `Plugin`. Never bypass the trait with backend-specific logic in the engine or TUI layers.
2. **TUI reads state, never owns it.** The TUI layer renders `AppState`. It does not mutate state, make network calls, or execute plugins. State transitions happen in `app.rs`.
3. **Async via channels, not callbacks.** Plugin results flow from Tokio tasks to the render loop via `tokio::sync::mpsc`. No shared mutable state between the render thread and plugin tasks.
4. **Graceful degradation.** If a plugin crashes, times out, or returns invalid output — show an error in the output pane, never crash the app.
5. **No GUI dependencies.** Terminal only. No Electron, webviews, or GUI toolkit imports.

## Code Style

- Follow the `rust-best-practices` skill (Apollo GraphQL style)
- Use `thiserror` for library errors (plugin traits, config parsing)
- Use `anyhow` for application-level errors (main, app state)
- Prefer `&str` over `String` in function parameters where possible
- Use `tracing` for all logging (never `println!` or `eprintln!` — they corrupt the TUI)
- Structured logging: `tracing::info!(plugin_name = %name, "executing plugin")`

## Dependencies

Core dependencies are locked in `docs/ARCHITECTURE.md` under Technology Stack. Do not add new dependencies without checking:
1. Is there already a dependency that does this?
2. Is the crate well-maintained (recent commits, reasonable download count)?
3. Does it pull in heavy transitive dependencies?

## Testing

- **Always test:** Plugin trait contracts, manifest parsing, JSON output parsing, config loading
- **Test files live in:** `tests/` (integration) or inline `#[cfg(test)]` modules (unit)
- Integration tests should use example plugins from `examples/plugins/`

## Completed Phases

- **Phase 0:** Project scaffold, Plugin trait, ScriptPlugin, PluginEngine
- **Phase 1:** TUI shell (ratatui), browse list, fuzzy search
- **Phase 2:** Favorites, configurable keybindings, direct-launch
- **Phase 3:** Default plugin pre-selection, default config generation, graceful config error handling
- **Phase 4 (Polish & UX):** Loading elapsed time, panic recovery in engine, Ctrl+D/U scroll, `t` output mode toggle, `R` plugin refresh, lazy entry validation, `--print-alias` shell integration

## Keybindings (defaults)

| Key | Mode | Action |
|---|---|---|
| `j`/`k`, `↑`/`↓` | Browse / ViewOutput | Navigate |
| `Enter` | Browse | Execute plugin |
| `/` or printable char | Browse | Enter search |
| `Esc` | Search | Clear search |
| `q`, `Ctrl+C` | Any | Quit |
| `R` (shift+r) | Browse | Refresh plugin list |
| `Ctrl+D` / `Ctrl+U` | ViewOutput | Scroll half-page |
| `t` | ViewOutput | Toggle list / raw text |
| `Enter` | ViewOutput | Run item action |
| `Esc` / `Backspace` | ViewOutput | Back |

All keys except search mode and `Ctrl+C` are configurable in `config.toml`.

## Release Process

1. Ensure `cargo test && cargo clippy -- -D warnings` pass
2. Bump `version` in `Cargo.toml`
3. `git tag v<VERSION> && git push origin v<VERSION>`
4. Release workflow (`.github/workflows/release.yml`) builds 3 tarballs and creates a GitHub Release
5. Download each tarball, run `shasum -a 256`, update SHA256 values in `Formula/lark.rb`
6. Copy updated `Formula/lark.rb` to `github.com/tfinklea/homebrew-tap`

## Plugin Development

Plugin directory: `~/.config/larkline/plugins/`
Each plugin is a directory with `manifest.toml` + an executable entry point.
Entry script existence is checked at **execution time**, not at scan time — missing entries show in the list but fail gracefully when run.
JSON output schema is defined in `docs/ARCHITECTURE.md` under JSON Schema Specification.

## OPENAI_API_KEY

Store `OPENAI_API_KEY` in the macOS Keychain instead of this repo.

```bash
security add-generic-password -U -a "$USER" -s OPENAI_API_KEY -w 'your-api-key-here'
```

Open a new shell after saving it so the variable is exported automatically.
