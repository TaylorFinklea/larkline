-- Recent Files — recently modified files in the project or common directories.

lark.register({
    on_run = function()
        local home = lark.env("HOME") or "/"
        -- Use LARK_CWD (set by lark.nvim) if available, otherwise common dirs.
        local lark_cwd = lark.env("LARK_CWD")

        local raw
        if lark_cwd and lark_cwd ~= "" then
            raw = lark.exec("find", {
                lark_cwd,
                "-maxdepth", "4",
                "-type", "f",
                "-mtime", "-3",
                "-not", "-path", "*/.*",
                "-not", "-path", "*/node_modules/*",
                "-not", "-path", "*/target/*",
                "-not", "-name", "*.pyc",
            })
        else
            raw = lark.exec("find", {
                home .. "/git",
                home .. "/projects",
                home .. "/Documents",
                home .. "/Desktop",
                "-maxdepth", "4",
                "-type", "f",
                "-mtime", "-3",
                "-not", "-path", "*/.*",
                "-not", "-path", "*/node_modules/*",
                "-not", "-path", "*/target/*",
                "-not", "-name", "*.pyc",
            })
        end

        if not raw or raw == "" then
            return {
                title = "Recent Files",
                items = { { label = "No recently modified files found", icon = "📭" } },
            }
        end

        local paths = {}
        for path in raw:gmatch("[^\n]+") do
            paths[#paths + 1] = path
        end

        local items = {}
        for i = 1, math.min(#paths, 50) do
            local path = paths[i]
            local name = path:match("([^/]+)$") or path
            local dir = path:match("^(.*)/[^/]+$") or ""
            if home ~= "" and dir:sub(1, #home) == home then
                dir = "~" .. dir:sub(#home + 1)
            end
            local actions = {}
            if lark.env("NVIM") then
                actions[#actions + 1] = { label = "Open in Neovim", kind = "nvim_edit", args = { path } }
                actions[#actions + 1] = { label = "Open (vsplit)", kind = "nvim_edit", args = { path, "vsplit" } }
            end
            actions[#actions + 1] = { label = "Open in Finder", kind = "shell", args = { "open", "-R", path } }
            actions[#actions + 1] = { label = "Open in $EDITOR", kind = "shell", args = { lark.env("EDITOR") or "vim", path } }
            actions[#actions + 1] = { label = "Copy path", kind = "clipboard", args = { path } }

            items[#items + 1] = {
                label = name,
                detail = dir,
                icon = "📄",
                copy_text = path,
                actions = actions,
            }
        end

        return { title = "Recent Files — " .. #items .. " files", items = items }
    end,
})
