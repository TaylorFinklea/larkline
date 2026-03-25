-- SSH — list hosts from ~/.ssh/config with connect actions.

lark.register({
    on_run = function()
        local home = lark.env("HOME") or "/tmp"
        local raw = lark.exec("cat", { home .. "/.ssh/config" })

        if not raw or raw == "" then
            return {
                title = "SSH",
                items = { { label = "No ~/.ssh/config found", icon = "!" } },
            }
        end

        local hosts = {}
        local current = nil

        for line in raw:gmatch("[^\n]+") do
            local host = line:match("^Host%s+(.+)")
            if host then
                -- Skip wildcard patterns.
                if not host:match("[*?]") then
                    current = { name = host:gsub("%s+$", ""), hostname = nil, user = nil }
                    hosts[#hosts + 1] = current
                else
                    current = nil
                end
            elseif current then
                local hostname = line:match("^%s+HostName%s+(.+)")
                if hostname then current.hostname = hostname:gsub("%s+$", "") end
                local user = line:match("^%s+User%s+(.+)")
                if user then current.user = user:gsub("%s+$", "") end
            end
        end

        if #hosts == 0 then
            return {
                title = "SSH",
                items = { { label = "No hosts found in config", icon = "📭" } },
            }
        end

        local items = {}
        for _, h in ipairs(hosts) do
            local detail_parts = {}
            if h.hostname then detail_parts[#detail_parts + 1] = h.hostname end
            if h.user then detail_parts[#detail_parts + 1] = "user: " .. h.user end
            local detail = #detail_parts > 0 and table.concat(detail_parts, "  ") or nil

            local ssh_cmd = "ssh " .. h.name

            items[#items + 1] = {
                label = h.name,
                detail = detail,
                icon = "🖥",
                copy_text = ssh_cmd,
                actions = {
                    {
                        label = "Connect",
                        kind = "shell",
                        args = { "open", "-a", "Terminal", "ssh://" .. h.name },
                    },
                    { label = "Copy command", kind = "clipboard", args = { ssh_cmd } },
                    { label = "Copy hostname", kind = "clipboard", args = { h.hostname or h.name } },
                },
            }
        end

        return { title = "SSH — " .. #hosts .. " hosts", items = items }
    end,
})
