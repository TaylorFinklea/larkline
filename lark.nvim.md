# lark.nvim

Neovim integration for larkline lives in its own repo:

→ **<https://github.com/TaylorFinklea/lark.nvim>**

The plugin was extracted from this monorepo on 2026-04-25 to follow standard
Neovim plugin conventions (one repo per plugin, easy lazy.nvim install).

## Install

Using [lazy.nvim](https://github.com/folke/lazy.nvim):

```lua
{
  "TaylorFinklea/lark.nvim",
  config = function()
    require("lark").setup({})
  end,
  keys = {
    { "<C-l>", "<cmd>Lark<cr>", desc = "Open lark", mode = { "n", "t" } },
  },
}
```

## Features

- `:Lark` opens larkline in a floating terminal with project context
  (`LARK_CWD`, `LARK_FILE`, `LARK_FILETYPE`).
- `nvim_edit` actions in larkline plugins open files back in the parent
  Neovim via `$NVIM` socket awareness.
- `lark.nvim_exec(":cmd")` lets larkline plugins send arbitrary ex commands
  back to the parent editor.

## What's coming in v3

`:Telescope lark` opens a Telescope picker over the larkline plugin catalog —
hit Enter on a result, fire its primary action, never leave nvim.

Built on three larkline CLI subcommands shipped in v0.13.0:

- `lark list --json` — enumerate the plugin catalog
- `lark invoke <plugin>` — run a plugin's `on_run`, get JSON output
- `lark action <plugin> --action-json '<…>'` — fire a single action

Forms (Bitwarden Generate Password, Atlassian New Issue) and mini-apps (Docker
Dashboard) drop into the existing floating terminal as fallback.

## Earlier history

The pre-extraction `lark.nvim/` directory's commit history lives in this
repo's git log — `git log -- lark.nvim/` for context if you need it.
