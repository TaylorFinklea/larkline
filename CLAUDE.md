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

## Plugin Development

Plugin directory: `~/.config/larkline/plugins/`
Each plugin is a directory with `manifest.toml` + an executable entry point.
JSON output schema is defined in `docs/ARCHITECTURE.md` under JSON Schema Specification.

## OPENAI_API_KEY

Store `OPENAI_API_KEY` in the macOS Keychain instead of this repo.

```bash
security add-generic-password -U -a "$USER" -s OPENAI_API_KEY -w 'your-api-key-here'
```

Open a new shell after saving it so the variable is exported automatically.
