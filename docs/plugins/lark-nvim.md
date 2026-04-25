# lark.nvim — Neovim integration

`lark.nvim` is the Neovim integration for larkline. As of larkline v0.13.0
it lives in its own repo:

→ **<https://github.com/TaylorFinklea/lark.nvim>**

## What it does

`:Telescope lark` opens a fuzzy-searchable picker over every larkline plugin.
Hit `<CR>` on a plugin → it invokes and pushes a results picker on top. Hit
`<CR>` on a result row → it fires the primary action (open in browser, copy,
open file in nvim, …). `<C-a>` opens a sub-picker over all actions for the
row.

Chain actions push fresh pickers on the stack; `<Esc>` returns to the
previous picker. Native Telescope nested-picker behaviour.

Forms (Bitwarden Generate Password, Atlassian New Issue) and mini-apps
(Docker Dashboard) automatically fall back to a floating-terminal TUI —
those flows can't fit in Telescope's single-picker model.

## Install

Using [lazy.nvim](https://github.com/folke/lazy.nvim):

```lua
{
  "TaylorFinklea/lark.nvim",
  dependencies = { "nvim-telescope/telescope.nvim" },  -- optional but recommended
  config = function()
    require("lark").setup({})
    pcall(require("telescope").load_extension, "lark")
  end,
}
```

Requires:

- `larkline` v0.13.0+ on `$PATH` (the Telescope source uses
  `lark list --json` and `lark action`, both new in v0.13.0).
- `nvim` 0.9+ (0.10+ uses async `vim.system`; 0.9.x falls back to sync
  `vim.fn.system`).

`telescope.nvim` is a peer dependency. Without it, lark.nvim still works in
floating-terminal-only mode (`:Lark`, `<C-l>`).

## Default keymaps

| Keys | Mode | Action |
|---|---|---|
| `<C-l>` | n / t | Telescope picker (auto-falls back to TUI if telescope is missing) |
| `<C-l><C-l>` | n / t | Open the floating-terminal TUI explicitly |

## Underlying CLI contract

The Telescope source talks to `lark` via three subcommands. Useful to know
when debugging:

- **`lark list --json`** → array of `ListEntry` objects describing every
  plugin / command. Stable JSON shape; see `src/main.rs::ListEntry`.
- **`lark invoke <plugin>`** → runs the plugin's `on_run`, prints the
  resulting `PluginOutput` JSON.
- **`lark action <plugin> --action-json '<JSON>' [--confirm]`** → fires
  one `ItemAction`. Prints a tagged `ActionWireOutcome`:
  - `{"outcome": "side", "summary": "...", "stdout"?: "..."}` —
    clipboard / open / shell / nvim_edit happened.
  - `{"outcome": "chained", "output": <PluginOutput>}` — chain returned
    a new view; lark.nvim pushes a fresh picker.
  - `{"outcome": "needs_confirmation", "command": "...", "args": [...], "description": "..."}`
    — shell action with `confirm: true` was invoked without `--confirm`.
    lark.nvim prompts via `vim.fn.confirm` then re-issues the call.

All three are stdout-clean (tracing → stderr only) so they're safe to call
from `vim.system` / `vim.fn.system`.

## Why a separate repo?

- Standard Neovim plugin convention — every plugin manager (lazy.nvim,
  packer, vim-plug) installs from a top-level repo with one line.
- `nvim-telescope/telescope.nvim` peer-dep is more naturally declared at
  the Neovim-plugin level, not buried in a subdirectory.
- Decouples lark.nvim release cadence from larkline's. lark.nvim users track
  `main`; larkline releases its CLI on its own schedule.

The pre-extraction history of the v2 wrapper is preserved in this repo —
`git log -- lark.nvim/` shows it.

## Troubleshooting

**`:Telescope lark` says "no plugins found"** — `lark list --json` returned
an empty array. Try `lark plugin sync` or `lark plugin list` to check that
plugins are installed in `~/.config/larkline/plugins/`.

**Picker opens but `<CR>` does nothing** — The selected row has no actions,
or telescope.nvim wasn't loaded (lark.nvim falls back to copying
`copy_text` to the system clipboard in this case; check `:messages`).

**Form / mini-app plugin opens floating terminal instead of Telescope** —
Expected. Forms and mini-apps are full-screen TUI flows; Telescope can't
render them. The notification line explains this when it happens.

**`<C-l><C-l>` opens TUI but `<C-l>` errors** — `telescope.nvim` isn't
installed. `<C-l>` auto-detects and falls back to the TUI in this case;
if you're seeing an actual error, check `:checkhealth lark` (when
implemented in v4) or open an issue.
