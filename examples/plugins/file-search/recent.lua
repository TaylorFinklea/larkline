-- Recent Files — recently modified files in common working directories.

lark.register({
    on_run = function()
        local home = os.getenv("HOME") or "/"
        -- Search common working dirs for recently modified files (last 3 days).
        local raw = lark.exec("find", {
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
            items[#items + 1] = {
                label = name,
                detail = dir,
                icon = "📄",
                copy_text = path,
                actions = {
                    { label = "Open in Finder", kind = "shell", args = { "open", "-R", path } },
                    { label = "Open in $EDITOR", kind = "shell", args = { os.getenv("EDITOR") or "vim", path } },
                    { label = "Copy path", kind = "clipboard", args = { path } },
                },
            }
        end

        return { title = "Recent Files — " .. #items .. " files", items = items }
    end,
})
