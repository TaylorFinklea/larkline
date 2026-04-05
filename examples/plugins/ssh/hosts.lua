-- SSH: Hosts — list hosts from ~/.ssh/config with connect actions and ping status.

local function parse_ssh_config()
    local home = lark.env("HOME") or "/tmp"
    local raw = lark.exec("cat", { home .. "/.ssh/config" })
    if not raw or raw == "" then return nil end

    local hosts = {}
    local current = nil

    for line in raw:gmatch("[^\n]+") do
        local host = line:match("^Host%s+(.+)")
        if host then
            if not host:match("[*?]") then
                current = { name = host:gsub("%s+$", ""), hostname = nil, user = nil, port = nil, identity = nil }
                hosts[#hosts + 1] = current
            else
                current = nil
            end
        elseif current then
            local hostname = line:match("^%s+HostName%s+(.+)")
            if hostname then current.hostname = hostname:gsub("%s+$", "") end
            local user = line:match("^%s+User%s+(.+)")
            if user then current.user = user:gsub("%s+$", "") end
            local port = line:match("^%s+Port%s+(%d+)")
            if port then current.port = port end
            local ident = line:match("^%s+IdentityFile%s+(.+)")
            if ident then current.identity = ident:gsub("%s+$", "") end
        end
    end
    return hosts
end

local function check_reachable(hostname)
    local result = lark.exec("nc", { "-z", "-w", "1", hostname, "22" })
    return result ~= nil
end

lark.register({
    on_run = function()
        local hosts = parse_ssh_config()
        if not hosts then
            return {
                title = "SSH Hosts",
                items = { { label = "No ~/.ssh/config found", icon = "!" } },
            }
        end

        if #hosts == 0 then
            return {
                title = "SSH Hosts",
                items = { { label = "No hosts found in config", icon = "📭" } },
            }
        end

        local items = {}
        for _, h in ipairs(hosts) do
            local detail_parts = {}
            if h.hostname then detail_parts[#detail_parts + 1] = h.hostname end
            if h.user then detail_parts[#detail_parts + 1] = "user:" .. h.user end
            if h.port and h.port ~= "22" then detail_parts[#detail_parts + 1] = "port:" .. h.port end

            local target = h.hostname or h.name
            local reachable = check_reachable(target)
            local icon = reachable and "●" or "○"
            if reachable then
                detail_parts[#detail_parts + 1] = "reachable"
            else
                detail_parts[#detail_parts + 1] = "unreachable"
            end

            local ssh_cmd = "ssh " .. h.name

            items[#items + 1] = {
                label = h.name,
                detail = #detail_parts > 0 and table.concat(detail_parts, "  ") or nil,
                icon = icon,
                copy_text = ssh_cmd,
                actions = {
                    { label = "Connect in Terminal", kind = "shell", args = { "open", "-a", "Terminal", "ssh://" .. h.name } },
                    { label = "Copy ssh command", kind = "clipboard", args = { ssh_cmd } },
                    { label = "Copy hostname", kind = "clipboard", args = { h.hostname or h.name } },
                },
            }
        end

        return { title = "SSH Hosts — " .. #hosts, items = items }
    end,
})
