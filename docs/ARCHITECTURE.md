# Larkline Architecture

### _The line to all your tools._

> **Status:** Pre-implementation — v0.1 planning
> **Last updated:** 2026-03-18
> **Maintainer:** Taylor (Product Owner) + Claude Code (Implementation)

---

## Table of Contents

1. [Vision & Principles](#vision--principles)
2. [Project Identity](#project-identity)
3. [Technology Stack](#technology-stack)
4. [Architecture Overview](#architecture-overview)
5. [Directory Structure](#directory-structure)
6. [Plugin Interface Design](#plugin-interface-design)
7. [JSON Schema Specification](#json-schema-specification)
8. [Phased Roadmap](#phased-roadmap)
9. [Non-Goals](#non-goals)
10. [Resolved Questions](#resolved-questions)

---

## Vision & Principles

A lightweight, keyboard-driven TUI application that provides a unified interface for running personal productivity plugins — like Raycast, but terminal-native. Launch it, navigate with the keyboard, run a plugin, see the result, act on it or dismiss it.

### Core Principles

1. **Terminal-native.** No Electron, no web views, no GUI dependencies. Runs in any modern terminal emulator.
2. **Fast.** Sub-100ms startup. The user should never feel like they're waiting.
3. **Plugin-first.** The app is a shell; all useful behavior comes from plugins. The core provides navigation, rendering, and plugin lifecycle management.
4. **Keyboard-driven.** Vim-style navigation (hjkl), arrow keys, fuzzy search to filter. Mouse is a nice-to-have, not required.
5. **Designed for evolution.** v0.1 uses script-based plugins. v1.0 migrates to embedded Lua. The architecture must not make this migration painful.

### Target Use Cases

- Check GitHub PR status across repos
- Monitor Claude Code API/token usage
- Toggle Home Assistant devices (lights, switches)
- Quick-check RSS feed highlights
- Run common shell snippets with confirmation
- View system resource usage at a glance
- Check weather or calendar at a glance

---

## Project Identity

| Field | Value |
|---|---|
| **Crate name** | `larkline` |
| **Binary name** | `lark` |
| **Config directory** | `~/.config/larkline/` |
| **Plugin directory** | `~/.config/larkline/plugins/` |
| **Config file** | `~/.config/larkline/config.toml` |

---

## Technology Stack

Finalized after ecosystem research (March 2026):

| Component | Choice | Version | Rationale |
|---|---|---|---|
| Language | Rust | 2024 edition, MSRV 1.85+ | Performance, safety, single binary distribution |
| TUI framework | **Ratatui** + Crossterm | ratatui ~0.30, crossterm ~0.29 | Most active Rust TUI framework; modular workspace since 0.30; 30M+ crates.io downloads. Crossterm provides cross-platform terminal handling |
| Async runtime | **Tokio** | latest stable | Non-blocking plugin execution, HTTP calls, subprocess management. Communication between async tasks and TUI render loop via `tokio::sync::mpsc` channels |
| Fuzzy search | **nucleo** | latest stable | Powers the Helix editor; fastest fuzzy matcher in the Rust ecosystem; 252K+ downloads. Designed for exactly this use case (filtering lists in a TUI) |
| Serialization | **serde** + **toml** + **serde_json** | latest stable | Config parsing (TOML), plugin output parsing (JSON) |
| Clipboard | **arboard** | latest stable | By 1Password; cross-platform (macOS, Linux X11/Wayland, Windows); well-maintained |
| ANSI rendering | **ansi-to-tui** | latest stable | Converts ANSI escape sequences in plugin output to ratatui `Text` objects for rendering |
| Lua embedding (v1.0) | **mlua** | 0.11.6+ | Async/await support with Tokio; Lua 5.1-5.5, LuaJIT, and Luau support; actively maintained |
| Error handling | **anyhow** + **thiserror** | latest stable | `anyhow` for application errors, `thiserror` for library/trait errors |
| Logging | **tracing** + **tracing-subscriber** | latest stable | Structured logging; can write to file to avoid polluting TUI output |

### Binary Size Expectations

A release build with ratatui + tokio + serde typically produces a **4-8 MB** binary. Optimization options:

- `opt-level = "z"` + `lto = true` + `strip = true` in `[profile.release]` can reduce to ~3-5 MB
- `codegen-units = 1` for better optimization at the cost of compile time
- Consider `cargo-bloat` to identify size contributors if it exceeds expectations

---

## Architecture Overview

```
                        Larkline
 +-----------+   +-------------+   +--------------+
 |    TUI    |   |   Plugin    |   |   Config     |
 |   Layer   |<--|   Engine    |<--|   Manager    |
 |  (view)   |   |  (runner)   |   |  (settings)  |
 +-----+-----+   +------+------+   +--------------+
       |                |
 +-----+-----+   +------+------+
 |   Input   |   |   Plugin   |
 |  Handler  |   |  Registry  |
 |  (keymap) |   | (discover) |
 +-----------+   +------+------+
                        |
                 +------+------+
                 |   Plugins   |
                 |  (on disk)  |
                 +-------------+

Data flow:
  Input -> Action -> Engine executes plugin -> Output via mpsc channel -> TUI renders
```

### Component Responsibilities

**App State (`app.rs`)**
Central application state struct. Owns the plugin list, current selection, search query, active plugin output, and UI mode (browsing / viewing output / searching). The TUI layer reads this state to render; it never mutates state directly.

**TUI Layer (`tui/`)**
Renders the interface using Ratatui. Responsibilities:
- Search/filter bar at top
- Plugin list in center (scrollable, fuzzy-filtered)
- Output/detail pane (shows selected plugin's result)
- Status bar at bottom (keybinding hints, loading state)

Follows a **view-model pattern**: reads `AppState`, produces `Frame` draws. No business logic here.

**Input Handler (`input.rs`)**
Maps raw `crossterm::event::KeyEvent` to semantic `Action` variants. Supports:
- Configurable keymap (vim defaults + arrow key fallbacks)
- Mode-aware input (search mode captures text, browse mode navigates)
- Direct-launch keybindings for pinned plugins

**Plugin Engine (`plugin/engine.rs`)**
Manages plugin lifecycle: takes a `Plugin` trait object, calls `execute()`, captures output, enforces timeouts, sends results back to the TUI via `mpsc` channel. This is behind the `Plugin` trait so backends are swappable.

**Plugin Registry (`plugin/registry.rs`)**
Scans plugin directories, parses `manifest.toml` files, builds a `Vec<PluginMetadata>`. Handles hot-reload (re-scan on demand without restart).

**Config Manager (`config.rs`)**
Reads `~/.config/larkline/config.toml`. Provides defaults for everything. Validates on load. Handles XDG base directory resolution.

---

## Directory Structure

```
larkline/
  Cargo.toml
  Cargo.lock
  CLAUDE.md
  AGENTS.md
  README.md
  docs/
    ARCHITECTURE.md          # This file
  src/
    main.rs                  # Entry point: init tokio, load config, run app
    app.rs                   # AppState struct + state transition methods
    action.rs                # Action enum (Navigate, Select, Search, Back, Quit, etc.)
    input.rs                 # KeyEvent -> Action mapping
    config.rs                # Config loading + defaults
    tui/
      mod.rs                 # Terminal setup/teardown, main render loop
      ui.rs                  # Layout + widget rendering
      widgets/               # Custom widgets (plugin list, output pane, search bar)
        mod.rs
        plugin_list.rs
        output_pane.rs
        search_bar.rs
    plugin/
      mod.rs                 # Re-exports
      traits.rs              # Plugin trait + PluginMetadata + PluginOutput types
      engine.rs              # Plugin execution engine (timeout, channel comms)
      registry.rs            # Plugin discovery + manifest parsing
      script.rs              # ScriptPlugin backend (subprocess execution)
  tests/
    plugin_output_test.rs    # JSON schema parsing tests
    plugin_trait_test.rs     # Trait contract tests
    config_test.rs           # Config loading/defaults tests
  examples/
    plugins/                 # Example plugins shipped with the repo
      hello-world/
        manifest.toml
        run.sh
      system-info/
        manifest.toml
        run.sh
      github-prs/
        manifest.toml
        run.py
```

---

## Plugin Interface Design

### The Plugin Trait

This is the most architecturally important piece. Designed as a trait from day one so the v0.1 script backend and v1.0 Lua backend are interchangeable.

```rust
// Conceptual — final Rust types will be refined during implementation

#[async_trait]
pub trait Plugin: Send + Sync {
    /// Returns metadata about this plugin (name, description, icon, etc.)
    fn metadata(&self) -> &PluginMetadata;

    /// Execute the plugin's main action
    async fn execute(&self) -> Result<PluginOutput, PluginError>;

    /// List available sub-actions (optional — default returns empty)
    fn actions(&self) -> Vec<PluginAction> {
        vec![]
    }

    /// Execute a specific sub-action by ID
    async fn on_action(&self, action_id: &str) -> Result<PluginOutput, PluginError> {
        Err(PluginError::ActionNotSupported(action_id.to_string()))
    }
}

pub struct PluginMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub icon: String,            // Emoji
    pub category: Option<String>,
    pub keybinding: Option<String>,
    pub timeout: Duration,       // Default: 10s
}

pub struct PluginOutput {
    pub title: String,
    pub items: Vec<OutputItem>,
    pub raw_text: Option<String>, // Fallback for non-structured output
}

pub struct OutputItem {
    pub label: String,
    pub detail: Option<String>,
    pub icon: Option<String>,
    pub url: Option<String>,
    pub actions: Vec<ItemAction>,
    pub metadata: HashMap<String, String>, // Extensible key-value pairs
}

pub struct ItemAction {
    pub id: String,
    pub label: String,
    pub command: ActionCommand,
}

pub enum ActionCommand {
    OpenUrl(String),
    CopyToClipboard(String),
    RunShell { command: String, args: Vec<String>, confirm: bool },
}

pub enum PluginError {
    Timeout,
    ExecutionFailed(String),
    InvalidOutput(String),
    ActionNotSupported(String),
}
```

### v0.1: Script-Based Backend (`ScriptPlugin`)

A plugin is a directory under `~/.config/larkline/plugins/`:

```
~/.config/larkline/plugins/
  github-prs/
    manifest.toml
    run.sh          (or run.py, run.js — any executable)
  homeassistant/
    manifest.toml
    run.py
```

**`manifest.toml` format:**

```toml
[plugin]
name = "GitHub PRs"
description = "Check open PRs across your repos"
version = "0.1.0"
author = "taylor"
icon = "🔀"
entry = "run.sh"
timeout_seconds = 10

# Optional fields
category = "dev"
keybinding = "g p"

[plugin.env]
GITHUB_TOKEN = "${GITHUB_TOKEN}"
```

`ScriptPlugin` implementation:
1. Reads `manifest.toml` into `PluginMetadata`
2. On `execute()`: spawns the entry script as a subprocess via `tokio::process::Command`
3. Passes environment variables from `[plugin.env]` (resolving `${VAR}` from shell env)
4. Captures stdout, enforces timeout via `tokio::time::timeout`
5. Attempts JSON parse into `PluginOutput`; falls back to `raw_text` if not valid JSON

### v1.0: Lua Backend (`LuaPlugin`) — Design Now, Build Later

Lua plugins will be `.lua` files with access to a host API:

```lua
local lark = require("larkline")

lark.register({
  name = "GitHub PRs",
  icon = "🔀",
  on_run = function()
    local response = lark.http.get("https://api.github.com/...", {
      headers = { Authorization = "token " .. lark.env("GITHUB_TOKEN") }
    })
    local prs = lark.json.decode(response.body)
    return lark.output({
      title = "Open PRs",
      items = { ... }
    })
  end
})
```

**Architectural implication for v0.1:** The Plugin Engine must not assume plugins are subprocesses. The `Plugin` trait abstracts over "something that produces `PluginOutput` when executed." `ScriptPlugin` spawns a process; `LuaPlugin` calls into mlua. Same trait, different implementations.

---

## JSON Schema Specification

The contract between script plugins and the host. Plugins write this JSON to stdout.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "LarklinePluginOutput",
  "type": "object",
  "required": ["title"],
  "properties": {
    "title": {
      "type": "string",
      "description": "Heading displayed in the output pane"
    },
    "items": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["label"],
        "properties": {
          "label": {
            "type": "string",
            "description": "Primary text displayed in the item list"
          },
          "detail": {
            "type": "string",
            "description": "Secondary text (dimmed, below or beside label)"
          },
          "icon": {
            "type": "string",
            "description": "Emoji or single character displayed before the label"
          },
          "url": {
            "type": "string",
            "format": "uri",
            "description": "URL associated with this item (for open action)"
          },
          "actions": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["label", "command"],
              "properties": {
                "label": { "type": "string" },
                "command": {
                  "type": "string",
                  "enum": ["open", "clipboard", "shell"],
                  "description": "open = open URL in browser, clipboard = copy args[0], shell = run command"
                },
                "args": {
                  "type": "array",
                  "items": { "type": "string" }
                },
                "confirm": {
                  "type": "boolean",
                  "default": false,
                  "description": "If true, prompt user before executing (for shell commands)"
                }
              }
            }
          },
          "metadata": {
            "type": "object",
            "additionalProperties": { "type": "string" },
            "description": "Arbitrary key-value pairs for future use"
          }
        }
      }
    }
  }
}
```

**Fallback behavior:** If stdout is not valid JSON, display the raw text in the output pane as-is, with ANSI escape codes rendered via `ansi-to-tui`. This makes simple echo-based scripts work out of the box.

---

## Phased Roadmap

### v0.1 — Proof of Concept

#### Phase 0: Project Scaffolding
> **Goal:** Compilable project, basic TUI lifecycle, CI green.

- [ ] `cargo init --name larkline`
- [ ] Set up `Cargo.toml` with dependencies: ratatui, crossterm, tokio (full), serde, serde_json, toml, anyhow, thiserror, tracing, tracing-subscriber
- [ ] Create directory structure per [Directory Structure](#directory-structure)
- [ ] Implement minimal `main.rs`: init terminal, show "Larkline v0.1" text, quit on `q`
- [ ] Set up GitHub Actions CI: `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`
- [ ] Create example plugin directory with one `hello-world` plugin
- [ ] `[profile.release]` with `opt-level = "z"`, `lto = true`, `strip = true`

**Exit criteria:** `cargo run` opens a TUI, displays text, quits on `q`. CI passes.

#### Phase 1: Core TUI Shell
> **Goal:** Navigable plugin list with fuzzy search.

- [ ] Define `AppState` struct with: plugins list, selected index, search query, UI mode enum (Browse, Search, ViewOutput)
- [ ] Define `Action` enum: MoveUp, MoveDown, Select, Search(char), BackspaceSearch, ClearSearch, Back, Quit
- [ ] Implement input handler: KeyEvent -> Action mapping (j/k/arrows = navigate, typing = search, Enter = select, Esc = back, q = quit)
- [ ] Build TUI layout with ratatui:
  - Top: search/filter bar (shows query, highlighted when in search mode)
  - Center: plugin list (scrollable, shows icon + name + description)
  - Bottom: status bar (keybinding hints)
- [ ] Integrate `nucleo` for fuzzy filtering: as user types, filter the plugin list in real-time
- [ ] Highlight matched characters in plugin names
- [ ] Support Ctrl+C for graceful exit (restore terminal state)
- [ ] Hardcode 5-10 fake plugins for testing the UI

**Exit criteria:** Smooth navigation, real-time fuzzy filtering, clean terminal restore on exit.

#### Phase 2: Script Plugin Engine
> **Goal:** Discover, load, and execute real plugins. Display output.

- [ ] Define the `Plugin` trait, `PluginMetadata`, `PluginOutput`, and related types in `plugin/traits.rs`
- [ ] Implement `ScriptPlugin` in `plugin/script.rs`: reads manifest, spawns subprocess, captures output
- [ ] Implement `PluginRegistry` in `plugin/registry.rs`: scans plugin dirs, parses manifests
- [ ] Implement `PluginEngine` in `plugin/engine.rs`: async execution with timeout, sends results via `mpsc` channel
- [ ] Wire into TUI: when user selects a plugin, engine executes it, output appears in detail pane
- [ ] Parse JSON output into structured `PluginOutput` rendering (item list with labels, details, icons)
- [ ] Handle non-JSON output: display as raw text with ANSI rendering
- [ ] Implement loading spinner while plugin executes
- [ ] Implement error display for failed plugins (show stderr)
- [ ] Implement built-in actions: "Open URL" (via `open` command), "Copy to clipboard" (via arboard)
- [ ] Write tests: manifest parsing, JSON output parsing, timeout enforcement, trait contract

**Exit criteria:** Can write a real shell script plugin, run it from Larkline, see structured output, act on items.

---

### v0.5 — Usable Daily Driver

#### Phase 3: Configuration & Keybindings
> **Goal:** User-configurable behavior, persistent preferences.

- [ ] Implement config file loading from `~/.config/larkline/config.toml`
- [ ] Generate default config on first run with comments explaining each option
- [ ] Configurable fields:
  - `plugin_dirs` — list of plugin directory paths
  - `keybindings` — override default vim/arrow mappings
  - `theme` — color scheme (foreground, background, accent, dimmed)
  - `default_plugin` — auto-select on launch
  - `favorites` — pinned plugins shown at top of list
- [ ] Support direct-launch keybindings for pinned plugins (e.g., `g p` for GitHub PRs)
- [ ] Handle missing/malformed config gracefully with warnings in status bar

#### Phase 4: Polish & UX
> **Goal:** Feels good to use daily.

- [ ] Loading indicators for slow plugins (spinner + elapsed time)
- [ ] Plugin refresh command (re-scan directories, bound to `R`)
- [ ] Startup time optimization: lazy plugin loading (load manifests synchronously, defer validation)
- [ ] Shell integration helper: `lark --print-alias zsh` outputs a suggested alias/function
- [ ] Error recovery: if a plugin crashes, show error in output pane, don't crash the app
- [ ] Scroll support in output pane (j/k when viewing output, or Ctrl+D/Ctrl+U)
- [ ] Multiple output modes: list view (default), raw text view (toggle with `t`)
- [ ] Status bar shows: selected plugin name, item count, loading state, available keybindings

---

### v1.0 — Lua Plugin Engine

#### Phase 5: Embedded Lua
> **Goal:** Lua as a first-class plugin authoring language.

- [ ] Add `mlua` dependency with features: `lua54` (or `luajit`), `async`, `serialize`
- [ ] Design the Lua host API:
  - `lark.http.get(url, opts)` / `lark.http.post(url, body, opts)` — async HTTP via reqwest
  - `lark.json.encode(table)` / `lark.json.decode(string)` — JSON serialization
  - `lark.env(name)` — read environment variables
  - `lark.output({ title, items })` — return structured output
  - `lark.log(message)` — write to Larkline's log file
  - `lark.exec(command, args)` — run a command and return output
- [ ] Implement `LuaPlugin` backend behind the existing `Plugin` trait
- [ ] Support mixed plugin directories (scripts and Lua side by side)
- [ ] Lua plugins detected by: presence of `init.lua` (or `entry = "init.lua"` in manifest)
- [ ] Sandbox Lua environment: no direct filesystem access, no `os.execute` — only the `lark.*` API
- [ ] Write Lua plugin authoring guide in `docs/LUA_PLUGINS.md`
- [ ] Port example plugins to Lua as reference implementations

#### Phase 6: Distribution & Community
> **Goal:** Easy to install, example plugins that demonstrate real value.

- [ ] Publish to crates.io (`cargo install larkline`)
- [ ] Homebrew formula (tap or core)
- [ ] Ship example plugins:
  - `github-prs` — GitHub PR status across repos
  - `system-info` — CPU, memory, disk usage
  - `homeassistant` — toggle devices via REST API
  - `weather` — current weather from a free API
  - `clipboard-history` — recent clipboard entries
  - `shell-snippets` — run saved commands with confirmation
- [ ] `lark init-plugin <name>` CLI command to scaffold a new plugin
- [ ] User-facing documentation site or comprehensive README

---

### v1.5 — Rich Interactions

#### Phase 7: Enhanced Plugin Output
> **Goal:** Plugins can provide richer, more interactive output.

- [ ] Plugin output v2 schema additions:
  - `table` type: render tabular data with columns and alignment
  - `progress` type: render progress bars
  - `form` type: simple input fields that send data back to the plugin
- [ ] Plugin streaming: long-running plugins can send incremental output (newline-delimited JSON)
- [ ] Plugin-to-plugin communication: plugins can invoke other plugins by name
- [ ] Rich text in output: support basic markdown rendering (bold, italic, headers, links)
- [ ] Notification support: plugins can trigger system notifications on completion (via `notify-rust`)

---

### v2.0 — Platform

#### Phase 8: Ecosystem & Extensibility
> **Goal:** Larkline becomes a platform others build on.

- [ ] Plugin manifest v2: declare dependencies on other plugins, declare required host API version
- [ ] Theme system: user-installable themes (TOML files with full color palette + widget style overrides)
- [ ] Plugin settings UI: plugins can declare settings in their manifest, Larkline renders a config form
- [ ] Command history: remember recently used plugins and actions, offer "recent" section
- [ ] Plugin update mechanism: plugins in git repos can be checked for updates (`lark plugin update`)
- [ ] Mouse support: click to select plugins, scroll plugin list, click actions
- [ ] Multi-window: split the TUI into multiple panes showing different plugin outputs simultaneously
- [ ] Performance profiling: `lark --profile` outputs startup timing breakdown
- [ ] Plugin API stability guarantee: v2.0 Plugin trait and JSON schema are considered stable

---

## Non-Goals

These are explicitly out of scope:

- **Not a terminal multiplexer.** No pane/session/PTY management.
- **Not a file manager.** Plugins can interact with files, but the core doesn't browse them.
- **Not a shell replacement.** Invoke, use, dismiss.
- **No plugin marketplace/registry.** Plugins are local directories. Share via git repos.
- **No mouse-first UI.** Keyboard is primary. Mouse is additive.
- **No networked/daemon mode.** Single-user, single-machine.
- **No GUI.** Terminal only. No Electron, no webviews.

---

## Resolved Questions

These were open research questions. Answers are based on ecosystem research conducted March 2026.

### 1. Ratatui vs alternatives?

**Answer: Ratatui is still the clear winner.** No real competitor has emerged. It has 30M+ downloads, an active community, and moved to a modular workspace architecture in 0.30. The only alternative worth noting is `cursive`, which is older and less actively developed. Stick with **Ratatui 0.30 + Crossterm 0.29**.

### 2. Async TUI patterns?

**Answer: Tokio + `mpsc` channels.** The standard pattern:
- Main thread runs the ratatui render loop with `crossterm::event::poll()` for input
- Plugin execution happens on Tokio tasks (spawned via `tokio::spawn`)
- Results sent back to the render loop via `tokio::sync::mpsc::channel`
- Render loop checks the channel receiver on each tick (non-blocking `try_recv()`)

This keeps the UI responsive while plugins execute asynchronously.

### 3. Fuzzy search crate?

**Answer: `nucleo`.** It powers the Helix editor, is the fastest fuzzy matcher in the Rust ecosystem, and is designed for exactly this use case (filtering lists in a TUI). It supports multi-threaded matching for large lists, though for our plugin list sizes (<100 items) single-threaded is fine. 252K+ downloads.

Alternatives considered:
- `fuzzy-matcher`: simpler API but slower
- `skim`: full TUI framework (too heavy, we just need the matching algorithm)
- `sublime_fuzzy`: less maintained

### 4. Binary size?

**Answer: ~4-8 MB for a release build.** With `opt-level = "z"` + `lto = true` + `strip = true`, expect ~3-5 MB. This is well within acceptable range for a single-binary CLI tool. Use `cargo-bloat` to investigate if it exceeds 10 MB.

### 5. Plugin output rendering?

**Answer: ANSI escape codes via `ansi-to-tui`, not markdown.** The `ansi-to-tui` crate converts ANSI sequences directly to ratatui `Text` objects. This means plugin scripts can use standard terminal colors and formatting and Larkline will render them correctly. For v1.5, we may add markdown rendering, but ANSI is the pragmatic v0.1 choice since scripts already produce it naturally.

### 6. Cross-platform clipboard?

**Answer: `arboard` by 1Password.** Works on macOS (AppKit), Linux (X11 + Wayland via `wayland-data-control` feature), and Windows. Well-maintained, widely used. On Wayland, enable the `wayland-data-control` feature flag.

---

## Appendix: Config File Reference

```toml
# ~/.config/larkline/config.toml

[general]
# Directories to scan for plugins (first match wins for name conflicts)
plugin_dirs = ["~/.config/larkline/plugins"]

# Plugin to auto-select on launch (by name)
# default_plugin = "GitHub PRs"

[ui]
# Color theme
theme = "default"  # or "dark", "light", "nord", "catppuccin"

# Show plugin icons in the list
show_icons = true

# Number of items visible in the plugin list before scrolling
visible_items = 15

[keybindings]
# Override defaults. Format: "action" = "key"
# Available actions: move_up, move_down, select, back, quit, search, refresh
# quit = "q"
# move_up = "k"
# move_down = "j"
# select = "Enter"
# back = "Escape"
# refresh = "R"

[favorites]
# Plugins pinned to the top of the list (by name)
# pinned = ["GitHub PRs", "System Info"]

[logging]
# Log level: error, warn, info, debug, trace
level = "warn"

# Log file path (default: no file logging)
# file = "~/.config/larkline/larkline.log"
```
