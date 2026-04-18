# Changelog

## v0.10.0 — Distribution + lark.nvim v2 (unreleased)

Rollup release — ships v0.6.0 through v0.10.0 accumulated on `main` since v0.5.0.

### Distribution
- **crates.io**: published as `larkline`. Install via `cargo install larkline`.
- **AUR**: `larkline-bin` package pulls the linux tarball from GitHub Releases.
- **Nix flake**: `nix profile install github:TaylorFinklea/larkline` builds from source.
- **Homebrew**: formula unchanged — `brew install TaylorFinklea/tap/larkline`.

### Correctness
- **Tracing to file**: TUI sessions write logs to `$XDG_STATE_HOME/larkline/lark.log` via a daily rolling, non-blocking appender. Writing to stderr corrupted the ratatui alternate screen whenever any log fired at the current level. CLI subcommands (`init-plugin`, `plugin sync`, `secret`) keep logging to stderr.
- **`lark plugin sync`** repairs dead symlinks and relinks stale cache paths. Dead symlinks are no longer silently ignored. `--force` overwrites user-customized plugins after an interactive `y/N` prompt (non-TTY defaults to skip).

### lark.nvim v2
- `$NVIM` socket detection: plugins can dispatch `ActionKind::NvimEdit` to open files in the parent Neovim instance via the `nvim` CLI remote-send flag.
- New Lua host API `lark.nvim_exec(cmd)` sends arbitrary ex commands back to the parent editor; returns `false` outside Neovim so plugins can feature-detect.
- Updated `file-search` and `notes` plugins to surface "Open in Neovim" actions when running under nvim.

### Earlier unreleased work (rollup)
- **v0.6.0**: Plugin deep-dives (Kubernetes 6 cmd, GitHub 5 cmd, SSH 4 cmd, Weather 3 cmd). Widget drill-in, upgrade menu action, picker search, friendlier error display.
- **v0.7.0**: New plugins — Obsidian/Notes (4 cmd), Tailscale (3 cmd), Linear (3 cmd).
- **v0.8.0**: Mini App Mode — full-screen split-pane UI, `on_action` chaining, user-initiated splits/resize/close, `lark.clipboard_read()`, clipboard history plugin, Docker Dashboard reference mini app.
- **v0.9.0**: `app.rs` split from 4027 to 2851 lines across 8 focused modules. Prefetch concurrency cap (8). Slow-plugin profiling at `info` level. Widget auto-refresh skipped when dashboard hidden.

## v0.3.0

### Plugin Manager
- **LazyVim-style Plugin Manager** — full-screen tree view (`P` key or Space → P)
- Enable/disable plugins and individual commands with Space toggle
- View settings values and secret status (✅ .env/keychain or ❌ NOT SET)
- Persists enable/disable state across restarts
- Disabled plugins filtered from the unified list

### Home Assistant (21 commands)
- 13 new entity commands: Switches, Covers, Fans, Cameras, Sensors, Binary Sensors, Batteries, Doors, Windows, Buttons, Helpers, Persons, Vacuums
- Lights: brightness presets (25/50/75/100%), color temperature (warm/cool/daylight)
- Climate: HVAC mode switching, temperature presets and +/- increment
- Media Players: play/pause, next/prev, volume presets, source selection
- Scripts: run scripts, open in HA editor
- ⭐ Favorite and 🚫 Hide actions on every entity
- Persistent filters: hidden states (default unavailable/unknown), hidden entities
- Favorites command with domain-appropriate quick actions
- Shell actions show flash message instead of curl output
- No more confirmation prompts for API calls

### New Plugins
- **Claude Usage** — daily/weekly/monthly/blocks/sessions with configurable time range
- **Codex Usage** — daily/monthly/sessions for OpenAI Codex tracking

### Calendar
- **My Schedule** — threaded ANSI timeline view of next 14 days with day-of-week headers, vertical threading, color-coded events, and location display

### UX Improvements
- **Two-step Esc everywhere** — Esc exits search mode (keeps filter), second Esc clears
- **--query flag** — `lark --query "git"` opens with search pre-filled
- **Space in form text fields** — now types a space instead of toggling
- **Auto-detect RawText output mode** — plugins returning raw_text scroll properly
- Shell actions with empty/JSON output show flash message instead of output pane

### lark.nvim
- Neovim integration: floating terminal via `:Lark`, `:LarkToggle`, `:LarkSearch`
- Context passing: `LARK_CWD` (git root), `LARK_FILE`, `LARK_FILETYPE`
- File Search respects `LARK_CWD` for project-scoped results

### Calculator
- Rewritten to use `qalc` (libqalculate) with `bc` fallback
- Supports unit conversions, currency, constants, trig

## v0.2.0

### New Plugins (13)

- **Calendar** — Today and Tomorrow commands via macOS `icalBuddy`
- **Running Apps** — switch between open macOS applications
- **Clipboard** — clipboard history via Maccy integration
- **Caffeinate** — keep your Mac awake (status, start, extend)
- **Kubernetes** — pods, services, and contexts
- **Ports** — TCP ports listening on this machine
- **Kill Process** — find and terminate running processes
- **Brew Update** — check for outdated Homebrew packages
- **File Search** — find files by name using `fd`/`find`
- **Quicklinks** — personal bookmark manager (URLs and file paths)
- **Encode/Decode** — Base64, URL encode/decode, JWT decode
- **Emoji** — search and copy emoji by name
- **Translate** — translate text via `translate-shell`

### Features

- **Unified search (Phase 8-9)** — prefetch cache, nucleo fuzzy filter, global item ranking, match highlighting
- **Multi-command plugins (Phase 10)** — `[[commands]]` in manifests, quickkeys, plugin groups
- **Clipboard + copy menu (Phase 11)** — `y` copies label, `Y` opens multi-field copy menu
- **Output search (Phase 12)** — `/` to filter output items, `o` to open URLs
- **Plugin store (Phase 13)** — persistent key-value storage for plugins (`lark.store` API)
- **Forms (Phase 14)** — text, select, toggle inputs; plugins can collect user input
- **Markdown rendering (Phase 15)** — rich output with syntax-highlighted code blocks
- **Navigation history (Phase 16)** — breadcrumb trail, `lark invoke` CLI, inter-plugin calls
- **Action palette (Phase 18)** — Cmd+K style searchable action overlay per item
- **Secrets (Phase 19)** — `.env` file loader, advisory `secrets` field in manifests
- **Power menu (Phase 20-21)** — Space key overlay, sidebar toggle, UX polish
- **NebularNews client (Phase 22)** — 6-command RSS intelligence feed plugin
- **Multi-repo Git (Phase 23)** — git dashboard across tracked repos, persistent sidebar toggle
- **Command history (Phase 24)** — sort by Alpha or Recent
- **Plugin settings (Phase 25)** — `[[settings]]` in manifests, form-based UI, persistent store
- **Theme presets (Phase 26)** — 6 built-in themes (catppuccin, nord, tokyo-night, dracula, gruvbox), Space+T switcher
- **Raycast-style flat list (Phase 29)** — removed group headers, inline plugin name badges
- **Quickkey priority** — exact quickkey match pins command to top of search results
- **Configurable sidebar** — `sidebar_ratio` config setting (default 50% browse, 28% drilled in)
- **macOS Keychain integration** — `lark secret set/list/delete` CLI, fallback after .env and env vars
- **Nerd Font icons** — all 34 plugins now have both emoji and Nerd Font icons

### Fixes

- Cursor resets to top on every search query change
- Normal mode on Back/Enter from ViewOutput (no more j/k typing into search)
- Empty `icon_nerd = ""` no longer replaces emoji with blank
- GitHub 403: User-Agent header and .env quote stripping
- Various Lua plugin fixes (null guards, deserialization compat)

## v0.1.0

Initial release. Plugin trait, ScriptPlugin, LuaPlugin, TUI shell with ratatui, fuzzy search, favorites, configurable keybindings, vim modes, ANSI rendering, shell actions, tables, streaming output, Nerd Font icons, `--help`, `init-plugin` scaffolder, Homebrew formula, 7 Lua + 2 shell example plugins.
