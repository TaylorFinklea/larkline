# Larkline

> The line to all your tools — a keyboard-driven terminal command palette.

`lark` is a fast, extensible TUI launcher. Press a key, fuzzy-search your plugins, hit Enter, and act on the results — all without leaving the terminal.

## Install

### Homebrew (macOS & Linux)

```sh
brew tap tfinklea/tap
brew install lark
```

### GitHub Releases

Download the pre-built binary for your platform from the [Releases page](https://github.com/tfinklea/larkline/releases), extract, and place `lark` on your `$PATH`.

### From Source

```sh
cargo install larkline
```

## Quick Start

```sh
lark
```

| Key | Action |
|---|---|
| `j` / `k` or `↑` / `↓` | Navigate |
| `/` or any char | Fuzzy search |
| `Enter` | Run selected plugin |
| `Ctrl+D` / `Ctrl+U` | Scroll output half-page |
| `t` | Toggle list / raw text view |
| `R` | Refresh plugin list |
| `Esc` | Back |
| `q` | Quit |

## Shell Integration

Bind `Ctrl+L` to launch `lark` from your shell:

```sh
# zsh
lark --print-alias zsh >> ~/.zshrc && source ~/.zshrc

# bash
lark --print-alias bash >> ~/.bashrc && source ~/.bashrc

# fish
lark --print-alias fish >> ~/.config/fish/config.fish
```

## Configuration

Config is auto-generated on first run at `~/.config/larkline/config.toml`. All fields are optional and commented out by default — uncomment what you need.

## Plugins

Plugins live in `~/.config/larkline/plugins/`. Each plugin is a directory containing a `manifest.toml` and an entry script (Lua or shell). Two backends are supported:

- **Lua** (recommended) — sandboxed Lua 5.4 with a built-in `lark.*` API for exec, HTTP, JSON, and more. See [`docs/LUA_PLUGINS.md`](docs/LUA_PLUGINS.md).
- **Shell** — any executable that prints JSON to stdout.

Plugin output supports plain text, structured JSON items, and **table output** via `columns` metadata. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full schema.

### Scaffold a new plugin

```sh
lark init-plugin my-plugin          # Lua plugin (recommended)
lark init-plugin my-plugin --shell  # Shell plugin
```

This creates `~/.config/larkline/plugins/my-plugin/` with a working `manifest.toml` and entry script.

### Standard plugins

The following plugins ship in [`examples/plugins/`](examples/plugins/):

| Plugin | Type | Description |
|---|---|---|
| `git-branches` | Lua | Recent local git branches with last-commit info |
| `hello-world` | Shell | Minimal example — returns a greeting |
| `hello-world-lua` | Lua | Minimal Lua example |
| `ip-addresses` | Lua | Local + public IPv4 addresses |
| `shell-snippets` | Lua | Run saved shell commands with confirmation |
| `system-info` | Shell | CPU, memory, and disk usage |
| `system-info-lua` | Lua | Same, via the `lark.*` API |
| `top-processes` | Lua | Top 20 processes by CPU — table output |
| `weather` | Lua | Current conditions from wttr.in |

### Nerd Font icons

Plugins can specify `icon_nerd` in their manifest for Nerd Font glyphs. Set `icon_set = "emoji"` in `config.toml` to fall back to standard emoji if you don't have a Nerd Font installed.

### JSON safety (shell plugins)

Never interpolate shell variables directly into JSON strings. Use `jq`:

```bash
jq -n --arg label "$label" --arg detail "$detail" \
  '{label: $label, detail: $detail, icon: "📦"}'
```

## Documentation

- [Architecture & JSON schema](docs/ARCHITECTURE.md)
- [Lua plugin guide](docs/LUA_PLUGINS.md)

## License

GPL-3.0-only — see [LICENSE](LICENSE).
