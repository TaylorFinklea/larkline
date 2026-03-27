-- Running Apps: Apps — list foreground apps and switch focus or quit.

lark.register({
    on_run = function()
        local raw = lark.exec("osascript", {
            "-e",
            "tell application \"System Events\" to get name of every process where background only is false",
        })

        if not raw or raw:match("^%s*$") then
            return {
                title = "Running Apps",
                items = { { label = "No apps found", icon = "📭" } },
            }
        end

        local apps = {}
        for name in raw:gmatch("([^,]+)") do
            local trimmed = name:match("^%s*(.-)%s*$")
            if trimmed and trimmed ~= "" then
                apps[#apps + 1] = trimmed
            end
        end

        table.sort(apps)

        local items = {}
        for _, name in ipairs(apps) do
            items[#items + 1] = {
                label     = name,
                icon      = "▶",
                copy_text = name,
                actions   = {
                    {
                        label   = "Focus",
                        kind    = "shell",
                        args    = { "osascript", "-e", "tell application \"" .. name .. "\" to activate" },
                        confirm = false,
                    },
                    {
                        label   = "Quit",
                        kind    = "shell",
                        args    = { "osascript", "-e", "tell application \"" .. name .. "\" to quit" },
                        confirm = true,
                    },
                    { label = "Copy Name", kind = "clipboard", args = { name } },
                },
            }
        end

        return { title = "Running Apps — " .. #items, items = items }
    end,
})
