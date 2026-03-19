# Larkline — Agent Instructions

## Skills

All agents working on this codebase should leverage:
- **`rust-best-practices`** — Idiomatic Rust patterns (ownership, error handling, borrowing, testing)
- **`rust-async-patterns`** — Async Rust with Tokio (channels, async traits, concurrent execution)

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

- Commit frequently with clear, descriptive messages
- One logical change per commit
- Run `cargo clippy -- -D warnings` and `cargo fmt -- --check` before committing
- All tests must pass before committing

## Subagent Guidance

When spawning subagents for this project:
- **Explore agents:** Use to investigate ratatui widget patterns, crate APIs, or existing Rust TUI projects for reference
- **Plan agents:** Use before starting a new phase to design the implementation approach
- **Code review agents:** Use after completing each phase to validate against ARCHITECTURE.md exit criteria
- **Do not** spawn agents for trivial single-file changes — handle those directly
