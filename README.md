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

Plugins live in `~/.config/larkline/plugins/`. Each plugin is a directory containing a `manifest.toml` and an executable entry script. See [`examples/plugins/`](examples/plugins/) for working examples.

Plugin output can be plain text or structured JSON — see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the schema.

## License

MIT — see [LICENSE](LICENSE).
