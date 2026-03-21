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

- **One commit per sub-phase or feature** — commit as each piece of work is completed, not one giant commit at the end
- Each commit must be self-contained and pass `cargo test && cargo clippy -- -D warnings && cargo fmt -- --check`
- Clear, descriptive commit messages; reference the phase/sub-phase (e.g., "Phase 9A: fix Esc in ViewOutput")

## Branch Workflow

- Always implement on a feature branch (use git worktrees for isolation)
- Merge back to `main` locally when complete — never push unless explicitly asked
- One commit per sub-phase before merging; do not squash sub-phases into one commit

## Current Status

Phases 0–9 complete (including 4.5, 6, 7, 8). Standard plugin library shipped.

- Phase 9 added: global item ranking, match highlighting, RunPlugin rows, Ctrl+D/U scroll in unified mode, vestigial state cleanup
- Phase 8 added: prefetch cache, unified launcher mode, nucleo item-level fuzzy filter, flash messages, `max_items_per_section`
- Phase 6/7 added: `--help`, scaffolder, ANSI rendering, shell confirmation, table output, streaming, Nerd Font icons

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
