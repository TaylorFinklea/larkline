# lark.nvim

Neovim integration for [larkline](https://github.com/TaylorFinklea/larkline) —
opens `lark` in a floating terminal with project context, and (new in lark
0.10) lets plugins open files back in the parent Neovim.

## Install

Using [lazy.nvim](https://github.com/folke/lazy.nvim):

```lua
{
  "TaylorFinklea/larkline",
  name = "lark.nvim",
  -- Points lazy at the lark.nvim/ subdirectory within the larkline repo.
  config = function()
    require("lark").setup({})
  end,
  keys = {
    { "<leader>l", "<cmd>Lark<cr>", desc = "Open lark" },
  },
}
```

## Commands

| Command | Action |
|---|---|
| `:Lark` | Open lark in a floating terminal |
| `:LarkToggle` | Toggle the floating terminal |
| `:LarkSearch [query]` | Open lark with a pre-filled search query |

## Context passed to lark

Whenever the floating terminal opens, these env vars are inherited by `lark`:

| Variable | Value |
|---|---|
| `LARK_CWD` | Git root of the current buffer, or `getcwd()` fallback |
| `LARK_FILE` | Absolute path of the current buffer |
| `LARK_FILETYPE` | Filetype of the current buffer |
| `NVIM` | msgpack-rpc socket path inherited from the parent (nvim ≥ 0.9) |

## Parent-editor integration (lark 0.10+)

When lark runs inside `:terminal`, Neovim exposes its RPC socket as `$NVIM`.
lark plugins can use this to dispatch edits back to the parent editor instead
of launching a secondary editor in a shell.

### From a Lua plugin

```lua
-- Emit an action that opens a file in the parent nvim:
{ label = "Open in Neovim", kind = "nvim_edit", args = { "/path/to/file" } }

-- Choose a split mode (edit / split / vsplit / tabedit):
{ label = "Open (vsplit)", kind = "nvim_edit", args = { "/path/to/file", "vsplit" } }

-- Run an arbitrary ex command from Lua:
lark.nvim_exec(":LspInfo")   -- returns true/false
```

When `$NVIM` is not set (lark running outside nvim), `nvim_edit` falls back
to the same behavior as `open` (system URL handler) with a status flash that
explains. `lark.nvim_exec` is a no-op that returns `false`, so plugins can
feature-detect:

```lua
if lark.env("NVIM") then
    -- surface nvim-specific actions
end
```

### Default overrides

Shipped plugins that emit `nvim_edit` today:

- `file-search` / `recent` — "Open in Neovim" and "Open (vsplit)"
- `notes` / `search` / `recent` — "Open in Neovim"

## Configuration

```lua
require("lark").setup({
  binary = "lark",        -- Override if lark isn't on $PATH
  width = 0.8,            -- Floating window width (fraction of editor)
  height = 0.8,           -- Floating window height (fraction)
  border = "rounded",     -- "rounded" | "single" | "double" | "none"
  detect_root = true,     -- Resolve LARK_CWD via `git rev-parse --show-toplevel`
})
```

## Roadmap

- Telescope picker that sources lark's unified list directly (needs a
  `lark list --json` subcommand first).
- Two-way streaming so `on_action` results update a Neovim buffer in
  real time.

Neither blocks the 0.10 release; `nvim_edit` + `nvim_exec` deliver the
"open a file in the parent editor" killer feature today.
