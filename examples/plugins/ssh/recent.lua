-- SSH: Recent — recently connected hosts from shell history.

lark.register({
    on_run = function()
        local home = lark.env("HOME") or "/tmp"

        local raw = lark.exec("cat", { home .. "/.zsh_history" })
        if not raw or raw == "" then
            raw = lark.exec("cat", { home .. "/.bash_history" })
        end

        if not raw or raw == "" then
            return {
                title = "Recent SSH",
                items = { { label = "No shell history found", icon = "📭" } },
            }
        end

        local seen = {}
        local hosts = {}
        for line in raw:gmatch("[^\n]+") do
            local cmd = line:match(";(.+)$") or line
            local target = cmd:match("^ssh%s+[^-]?(%S+)$") or cmd:match("^ssh%s+%S+@(%S+)$")
            if target and not seen[target] and not target:match("^%-") then
                seen[target] = true
                hosts[#hosts + 1] = target
            end
        end

        local reversed = {}
        for i = #hosts, 1, -1 do
            reversed[#reversed + 1] = hosts[i]
            if #reversed >= 20 then break end
        end

        if #reversed == 0 then
            return {
                title = "Recent SSH",
                items = { { label = "No recent SSH connections in history", icon = "📭" } },
            }
        end

        local items = {}
        for _, host in ipairs(reversed) do
            items[#items + 1] = {
                label = host,
                icon = "↩",
                copy_text = "ssh " .. host,
                actions = {
                    { label = "Connect", kind = "shell", args = { "open", "-a", "Terminal", "ssh://" .. host } },
                    { label = "Copy command", kind = "clipboard", args = { "ssh " .. host } },
                },
            }
        end

        return { title = "Recent SSH — " .. #reversed, items = items }
    end,
})
