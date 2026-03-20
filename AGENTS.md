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

## Current Status

Phases 0–7 complete (including 4.5 and 6). Standard plugin library shipped.

- Phase 6 added: `--help`/`-h` flag, `lark init-plugin` scaffolder, README polish, `icon_set` in default config
- Phase 7 added: ANSI rendering, shell action confirmation, table output, streaming output, Nerd Font icon system, standard plugin library
- Phase 5 added: `LuaPlugin` backend (mlua, Lua 5.4), `PluginKind` enum in registry, `lark.*` host API, `reqwest` for async HTTP
- Phase 4.5 added: Vim-style Normal/Insert/Command modes, `h`/`l` navigation

Next steps:
- Phase 8: Platform (themes, command history, mouse support)

## Release Artifacts

- **`.github/workflows/release.yml`** — triggered by `v*` tags; builds `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu` tarballs
- **`Formula/lark.rb`** — Homebrew formula (copy to `github.com/tfinklea/homebrew-tap` after filling in SHA256 values)
- Do not modify `Formula/lark.rb` SHA256 placeholders manually during development — they are filled in post-release

## Key AppState Fields

| Field | Type | Purpose |
|---|---|---|
| `mode` | `Mode` | Browse / Search / ViewOutput |
| `output_mode` | `OutputMode` | List / RawText (toggled with `t`) |
| `is_loading` | `bool` | Plugin executing |
| `loading_started` | `Option<Instant>` | For elapsed time display |
| `plugin_output` | `Option<PluginOutput>` | Last execution result |
| `warnings` | `Vec<String>` | Status bar warnings (cleared on keypress) |

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
