# Larkline — Agent Instructions

Project-specific guidance for any AI coding agent (Claude Code, Codex, Copilot, etc.). Shared agent behavior (TaskCreate/TaskUpdate, OPENAI_API_KEY conventions, shell discipline) lives in `~/AGENTS.md`.

**Source of truth:** `docs/ARCHITECTURE.md` — all architectural decisions, the phased roadmap, and technology choices live there. Read it before starting any phase.

## Project Overview

Larkline is a Rust-based terminal command palette. Binary: `lark`. Crate: `larkline`. Rust edition: 2024, MSRV 1.85+.

## Architecture Invariants

These are non-negotiable design decisions. Do not violate them:

1. **The `Plugin` trait is sacred.** All plugin backends implement `Plugin`. Never bypass the trait with backend-specific logic in the engine or TUI layers.
2. **TUI reads state, never owns it.** The TUI layer renders `AppState`. It does not mutate state, make network calls, or execute plugins. State transitions happen in `app.rs`.
3. **Async via channels, not callbacks.** Plugin results flow from Tokio tasks to the render loop via `tokio::sync::mpsc`. No shared mutable state between the render thread and plugin tasks.
4. **Graceful degradation.** If a plugin crashes, times out, or returns invalid output — show an error in the output pane, never crash the app.
5. **No GUI dependencies.** Terminal only. No Electron, webviews, or GUI toolkit imports.

## Build & Development

```bash
cargo build
cargo build --release
cargo test
cargo clippy -- -D warnings
cargo fmt
cargo fmt -- --check   # CI check mode
cargo run

# CLI flags (no TUI launched)
cargo run -- --version
cargo run -- --help
cargo run -- --print-alias zsh   # also: bash, fish
cargo run -- init-plugin my-plugin          # Lua scaffold
cargo run -- init-plugin my-plugin --shell  # Shell scaffold
```

## Code Style

- Follow the `rust-best-practices` skill (Apollo GraphQL style)
- Use `thiserror` for library errors (plugin traits, config parsing)
- Use `anyhow` for application-level errors (main, app state)
- Prefer `&str` over `String` in function parameters where possible
- Use `tracing` for all logging (never `println!` or `eprintln!` — they corrupt the TUI)
- Structured logging: `tracing::info!(plugin_name = %name, "executing plugin")`

## Skills

All agents working on this codebase should leverage:
- **`rust-best-practices`** — Idiomatic Rust patterns (ownership, error handling, borrowing, testing)
- **`rust-async-patterns`** — Async Rust with Tokio (channels, async traits, concurrent execution)

## AI Handoff Workflow

Handoff state lives in `.docs/ai/` (the global default). See `~/CLAUDE.md` for the full session start/end workflow.

## Research Before Implementing

Before writing code for any phase:
1. Read `docs/ARCHITECTURE.md` for the phase's requirements and exit criteria
2. Check ratatui docs and examples for the relevant patterns
3. Check crate documentation for any new dependency being introduced
4. Present a brief implementation plan before writing code

## Critical Contracts

These components have consumers on both sides. Changes require extra care:

| Contract | Producers | Consumers | Test Coverage Required |
|---|---|---|---|
| `Plugin` trait | `ScriptPlugin`, future `LuaPlugin` | `PluginEngine`, TUI layer | `tests/plugin_trait_test.rs` |
| `PluginOutput` JSON schema | External script plugins | `ScriptPlugin` parser, TUI output pane | `tests/plugin_output_test.rs` |
| `manifest.toml` format | Plugin authors | `PluginRegistry` parser | `tests/config_test.rs` |
| `config.toml` format | Users | `ConfigManager` | `tests/config_test.rs` |

When modifying any contract, verify all producers and consumers still work. Run the full test suite.

## Commit Practices

- **One commit per sub-phase or feature** — commit as each piece of work is completed, not one giant commit at the end
- Each commit must be self-contained and pass `cargo test && cargo clippy -- -D warnings && cargo fmt -- --check`
- Clear, descriptive commit messages; reference the phase/sub-phase (e.g., "Phase 9A: fix Esc in ViewOutput")

## Branch Workflow

- Always implement on a feature branch (use git worktrees for isolation)
- Merge back to `main` locally when complete — never push unless explicitly asked
- One commit per sub-phase before merging; do not squash sub-phases into one commit

## Current Status

See `.docs/ai/current-state.md` for live status. See `.docs/ai/roadmap.md` for completed milestones and priorities.

## Release Artifacts

- **`.github/workflows/release.yml`** — triggered by `v*` tags; builds `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu` tarballs
- **`Formula/lark.rb`** — Homebrew formula (copy to `github.com/tfinklea/homebrew-tap` after filling in SHA256 values)
- Do not modify `Formula/lark.rb` SHA256 placeholders manually during development — they are filled in post-release

## Key AppState Fields

| Field | Type | Purpose |
|---|---|---|
| `mode` | `Mode` | Unified / ViewOutput |
| `vim_mode` | `VimMode` | Normal / Insert / Command |
| `unified_rows` | `Vec<UnifiedRow>` | Flat row list (Section, Item, More, RunPlugin) |
| `unified_selected` | `usize` | Index into `unified_rows` of highlighted row |
| `result_cache` | `HashMap<usize, CachedResult>` | Prefetch results keyed by plugin index |
| `query` | `String` | Active search query |
| `output_mode` | `OutputMode` | List / RawText / Table (toggled with `t`) |
| `is_loading` | `bool` | Plugin executing (UserSelected) |
| `plugin_output` | `Option<PluginOutput>` | Last execution result |
| `warnings` | `Vec<String>` | Status bar warnings (cleared on keypress) |
| `status_message` | `Option<(String, Instant)>` | Flash message (expires after 2s) |

## Plugin JSON Safety

When writing or reviewing shell-based plugins, **never interpolate shell variables directly into JSON strings**. Variables containing quotes, backslashes, or newlines will silently produce invalid JSON.

Always use `jq` to construct JSON values:

```bash
# WRONG
echo "{\"label\": \"$value\"}"

# RIGHT
jq -n --arg label "$label" --arg detail "$detail" \
  '{label: $label, detail: $detail, icon: "📦"}'
```

Any plugin that touches user-facing data (file paths, process names, git output, hostnames, command output) must use `jq`. This applies to all example plugins in `examples/plugins/` and test plugins.

## Subagent Guidance

When spawning subagents for this project:
- **Explore agents:** Use to investigate ratatui widget patterns, crate APIs, or existing Rust TUI projects for reference
- **Plan agents:** Use before starting a new phase to design the implementation approach
- **Code review agents:** Use after completing each phase to validate against ARCHITECTURE.md exit criteria
- **Do not** spawn agents for trivial single-file changes — handle those directly
