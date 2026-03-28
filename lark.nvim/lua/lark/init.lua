-- lark.nvim — Lark command palette inside Neovim
--
-- Opens Lark in a floating terminal with project context.
-- Install: { "tfinklea/lark.nvim", config = true }

local M = {}

--- Default configuration.
M.config = {
  -- Path to the lark binary. Set to full path if not in $PATH.
  binary = "lark",
  -- Floating window dimensions (percentage of editor size).
  width = 0.8,
  height = 0.8,
  -- Border style: "rounded", "single", "double", "none".
  border = "rounded",
  -- Automatically detect project root via git. Falls back to cwd.
  detect_root = true,
}

--- Resolve the project root (git root or cwd).
local function find_root()
  -- Try git root first.
  local git_root = vim.fn.systemlist("git rev-parse --show-toplevel 2>/dev/null")[1]
  if vim.v.shell_error == 0 and git_root and git_root ~= "" then
    return git_root
  end
  return vim.fn.getcwd()
end

--- Build environment variables for Lark context.
local function build_env()
  local env = {}

  -- Project root for git plugins, file search, etc.
  if M.config.detect_root then
    env.LARK_CWD = find_root()
  else
    env.LARK_CWD = vim.fn.getcwd()
  end

  -- Current buffer context.
  local bufname = vim.api.nvim_buf_get_name(0)
  if bufname and bufname ~= "" then
    env.LARK_FILE = bufname
    env.LARK_FILETYPE = vim.bo.filetype or ""
  end

  return env
end

--- Calculate floating window dimensions.
local function calc_win_opts()
  local width = math.floor(vim.o.columns * M.config.width)
  local height = math.floor(vim.o.lines * M.config.height)
  local row = math.floor((vim.o.lines - height) / 2)
  local col = math.floor((vim.o.columns - width) / 2)

  return {
    relative = "editor",
    width = width,
    height = height,
    row = row,
    col = col,
    style = "minimal",
    border = M.config.border,
    title = " Lark ",
    title_pos = "center",
  }
end

--- State for the floating terminal.
local state = {
  buf = nil,
  win = nil,
}

--- Close the floating window if open.
local function close()
  if state.win and vim.api.nvim_win_is_valid(state.win) then
    vim.api.nvim_win_close(state.win, true)
  end
  state.win = nil
end

--- Open Lark in a floating terminal.
function M.open()
  -- Close existing window if open.
  close()

  -- Create a new scratch buffer.
  state.buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_set_option_value("bufhidden", "wipe", { buf = state.buf })

  -- Open floating window.
  local opts = calc_win_opts()
  state.win = vim.api.nvim_open_win(state.buf, true, opts)

  -- Build the command with environment variables.
  local env = build_env()
  local env_prefix = ""
  for k, v in pairs(env) do
    env_prefix = env_prefix .. k .. "=" .. vim.fn.shellescape(v) .. " "
  end

  local cmd = env_prefix .. M.config.binary

  -- Open terminal in the floating buffer.
  vim.fn.termopen(cmd, {
    on_exit = function(_, exit_code, _)
      -- Auto-close the floating window when lark exits.
      vim.schedule(function()
        close()
        if exit_code ~= 0 then
          vim.notify("Lark exited with code " .. exit_code, vim.log.levels.WARN)
        end
      end)
    end,
  })

  -- Enter terminal insert mode immediately.
  vim.cmd("startinsert")
end

--- Toggle the Lark floating window.
function M.toggle()
  if state.win and vim.api.nvim_win_is_valid(state.win) then
    close()
  else
    M.open()
  end
end

--- Open Lark with a pre-filled search query.
---@param query string
function M.search(query)
  -- TODO: Once lark supports --query flag, pass it directly.
  -- For now, just open lark.
  M.open()
end

--- Setup function for lazy.nvim.
---@param opts table|nil
function M.setup(opts)
  M.config = vim.tbl_deep_extend("force", M.config, opts or {})

  -- Register user commands.
  vim.api.nvim_create_user_command("Lark", function()
    M.open()
  end, { desc = "Open Lark command palette" })

  vim.api.nvim_create_user_command("LarkToggle", function()
    M.toggle()
  end, { desc = "Toggle Lark command palette" })

  vim.api.nvim_create_user_command("LarkSearch", function(args)
    M.search(args.args)
  end, { nargs = "?", desc = "Open Lark with search query" })
end

return M
