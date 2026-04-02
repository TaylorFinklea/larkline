-- Developer: Installed Plugins — list all plugins with paths and status.

lark.register({
    on_run = function()
        local home = lark.env("HOME") or "/"
        local plugin_dir = home .. "/.config/larkline/plugins"

        local raw = lark.exec("ls", { "-la", plugin_dir })
        if not raw or raw == "" then
            return {
                title = "Installed Plugins",
                items = { { label = "No plugins directory found", icon = "📭" } },
            }
        end

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            local name = line:match("([^%s]+)$")
            local target = line:match("->%s+(.+)$")
            if name and name ~= "." and name ~= ".." and not line:match("^total") then
                local is_symlink = target ~= nil
                local detail = is_symlink and ("→ " .. target) or (plugin_dir .. "/" .. name)

                items[#items + 1] = {
                    label = name,
                    detail = detail,
                    icon = is_symlink and "🔗" or "📁",
                    copy_text = plugin_dir .. "/" .. name,
                    actions = {
                        { label = "Open in Editor", kind = "shell",
                            args = { "code", plugin_dir .. "/" .. name } },
                        { label = "Copy path", kind = "clipboard",
                            args = { plugin_dir .. "/" .. name } },
                    },
                }
            end
        end

        return { title = "Installed Plugins — " .. #items, items = items }
    end,
})
