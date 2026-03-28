# Changelog

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
