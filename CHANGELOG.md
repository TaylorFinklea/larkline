# Changelog

## v0.13.0 — lark.nvim v3 (Telescope-native picker) (unreleased)

The headless contract for the new `lark.nvim` v3 Telescope integration. The nvim plugin now lives at <https://github.com/TaylorFinklea/lark.nvim>.

### New CLI subcommands

- `lark list --json` — emits the full plugin catalog as JSON to stdout. One entry per discovered command (multi-command plugins flatten to multiple entries with a `plugin_group` field). Stable wire format consumed by `lark.nvim`'s Telescope source.
- `lark action <plugin> --action-json '<JSON>' [--confirm]` — fires a single `ItemAction` against a plugin and prints a tagged outcome. Three outcome variants:
  - `side` { summary, stdout? } — clipboard / open / shell / nvim_edit side-effect.
  - `chained` { output: PluginOutput } — chain / update_pane returned a new view.
  - `needs_confirmation` { command, args, description } — confirm-required shell action; caller prompts and re-issues with `--confirm`.

Both new subcommands inherit the same SECRETS scope as the TUI, so `lark.env(KEY)` calls inside plugins resolve identically. (Drive-by fix: `lark invoke` now does this too.)

### Action dispatcher extraction

Action execution was lifted out of the TUI's `App::execute_item_action` into a CLI-friendly `crate::actions` module:

- `actions::execute(action, plugin) -> Result<ActionResult>` — TUI-free dispatcher. Consumed by `lark action`.
- `actions::side_effects::{open_url, copy_to_clipboard, nvim_open_file}` — the building blocks (moved from `app.rs` private helpers).

The TUI's behavior is unchanged in v0.13.0; consolidation onto the new dispatcher is deferred.

### lark.nvim v3 (separate repo)

`<https://github.com/TaylorFinklea/lark.nvim>` — declared as a peer-dependency on `nvim-telescope/telescope.nvim`. The nvim plugin gains:

- `:Telescope lark` opens a Telescope picker over the larkline plugin catalog.
- `<CR>` invokes the plugin and pushes a results picker on top.
- `<CR>` on a result row fires the primary action; `<C-a>` opens an action sub-picker.
- Chain actions push fresh pickers on the stack — `<Esc>` returns.
- Forms (Bitwarden Generate Password, Atlassian New Issue) and mini-apps (Docker Dashboard) automatically fall back to the legacy floating-terminal TUI.
- New default keymap: `<C-l>` = Telescope picker, `<C-l><C-l>` = floating-terminal TUI.

Requires `lark` v0.13.0+ on `$PATH`.

### Repo extraction

The `lark.nvim/` subdirectory was removed from this repo and extracted to `TaylorFinklea/lark.nvim` (fresh history). A pointer doc at `lark.nvim.md` documents the new install URL. v2 commit history for the floating-terminal wrapper is still retrievable here via `git log -- lark.nvim/`.

### Tests

194 tests pass (was 186 pre-v0.13.0). New: `tests/cli_list_test.rs` (1 test, JSON shape contract), `tests/cli_action_test.rs` (4 tests, all three outcome variants + unknown-plugin error path), `actions::tests` (2 unit tests).

## v0.12.0 — Atlassian (Jira + Confluence) deep-dive (unreleased)

Single plugin covering both Jira and Confluence Cloud, with two auth paths.

### Plugin (10 commands)

- **Jira (6):** My Issues, Active Sprint, Triage Queue, New Issue (form), Transition Issue (chain), Comment on Issue.
- **Confluence (4):** Recent Pages, Search (CQL), My Pages, New Page (storage format).
- ADF (Atlassian Document Format) reducer covers the common nodes (paragraph, heading, text, bulletList, codeBlock, link). Unsupported nodes render as `[unsupported: <type>]`.

### Auth — both paths supported

- **API token** (zero new infrastructure): set `ATLASSIAN_EMAIL` + `ATLASSIAN_API_TOKEN` via `lark secret set`, plus `atlassian_host` in plugin settings. Same pattern as the github / linear / homeassistant plugins.
- **OAuth 2.0 (3LO + PKCE)**: run `lark atlassian login` once. Refresh tokens persist in macOS Keychain; access tokens cache to `~/.cache/larkline/atlassian-access.json` (0600). Plugins call back via `lark atlassian token` (silently refreshes when needed).

When both are configured, the API-token path wins — useful for per-session overrides when OAuth refresh is misbehaving.

### New CLI subcommands

- `lark atlassian login` — browser-based authorization with a hand-rolled one-shot HTTP callback listener (no new HTTP-server dep).
- `lark atlassian token` / `cloudid` / `site` — read state for plugins.
- `lark atlassian status` — debug info (signed-in account, expiry).
- `lark atlassian logout` — clear all persisted auth state.

### New Lua host APIs

- `lark.base64.encode(s)` / `lark.base64.decode(s)` — backed by the `base64` crate. Used by the API-token auth path; useful for any plugin that needs HTTP Basic auth.
- `LARK_BINARY` injected into the secrets map so plugins can re-invoke the running binary via `lark.exec(lark.env("LARK_BINARY") or "lark", {...})`. Lets dev runs (target/debug, not on `$PATH`) dispatch their own subcommands.

### Dependencies

- `base64` (already transitive via reqwest, now a direct dep).
- `sha2` (new, ~20k LOC) — for PKCE S256 challenge.
- `rand` (new direct dep, was transitive) — 32-byte CSRF state + PKCE verifier.

### Pre-publish gate

`BAKED_CLIENT_ID` in `src/atlassian/oauth.rs` is currently a placeholder. Before shipping the OAuth path, register a public OAuth 2.0 (3LO) app at <https://developer.atlassian.com/console/myapps/>. Users can self-host via `LARKLINE_ATLASSIAN_CLIENT_ID` env override regardless. The API-token path has no such gate.

### Drive-by

- Fixed two pre-existing rust 1.95 clippy lints (sort_unstable_by → sort_unstable_by_key + Reverse in `app.rs`, redundant `if`-inside-match collapses in `markdown.rs`).
- `forbid(unsafe_code)` continues to hold — the OAuth subsystem and `LARK_BINARY` injection both stay clear of `std::env::set_var` (Rust 2024's unsafe API).

## v0.11.0 — Bitwarden deep-dive (unreleased)

Raycast-parity plugin for the official `bw` CLI. 6 commands: Search Vault, Favorites, Folders, Generate Password, Sync Vault, Lock Vault. Supports all four item types (login / note / card / identity) with type-aware copy actions. Custom fields with hidden-type redaction in the detail view. Session lookup via `BW_SESSION` env var.

Plus follow-up fixes:

- **bw `--response` envelope unwrapping** (`9b84d6b`): the plugin was reading `parsed.data.userEmail` / `.status` directly when those live one level deeper under `.template`. Symptom was "Account unknown" + "No items in vault" even on an unlocked vault. Fix is a single `unwrap_bw` helper in each command file.
- **`lark.json.decode` null-safety** (`50cfa79`): mlua's default `to_value` mapped JSON `null` to a truthy userdata sentinel, so idiomatic plugin guards like `if x and x ~= "" then` silently leaked the sentinel through and crashed `table.concat`. `lark.json.decode` now produces Lua `nil` for null. Affects every plugin, fixes a class of latent bugs.
- **Stale-session preflight** (`7b4ee48`): `bw --response` returns `success: true, data: []` for a stale `BW_SESSION` instead of raising an error. Added an explicit `bw status` preflight that checks `status == "unlocked"` AND `userEmail` is non-empty.

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
