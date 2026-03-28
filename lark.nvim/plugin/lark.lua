-- Auto-setup: if the user loads via lazy.nvim with config = true,
-- setup() is called automatically. This file provides a fallback
-- for users who add the plugin path manually without calling setup().

if vim.g.loaded_lark then
  return
end
vim.g.loaded_lark = true

-- Default keymap: Ctrl+L opens Lark (matches the shell integration binding).
-- Users can override this in their config.
vim.keymap.set("n", "<C-l>", function()
  require("lark").toggle()
end, { desc = "Toggle Lark", silent = true })

-- Also map in terminal mode so Ctrl+L doesn't conflict inside Lark itself.
vim.keymap.set("t", "<C-l>", function()
  require("lark").toggle()
end, { desc = "Toggle Lark", silent = true })
