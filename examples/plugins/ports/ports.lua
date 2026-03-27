-- Ports: Listening — all TCP ports currently accepting connections.

lark.register({
    on_run = function()
        local raw = lark.exec("lsof", { "-nP", "-iTCP", "-sTCP:LISTEN" })

        if not raw or raw == "" then
            return {
                title = "Listening Ports",
                items = { { label = "No listening ports found", icon = "📭" } },
            }
        end

        local seen = {}
        local port_entries = {}

        for line in raw:gmatch("[^\n]+") do
            -- Skip the header line
            if line:match("^COMMAND") then goto continue end

            local parts = {}
            for p in line:gmatch("%S+") do
                parts[#parts + 1] = p
            end

            -- lsof output: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
            -- NAME (last field) contains address:port, e.g. *:3000 or 127.0.0.1:5432
            if #parts >= 9 then
                local cmd  = parts[1]
                local pid  = parts[2]
                local addr = parts[#parts]
                local port = addr:match(":(%d+)$")

                if port and not seen[port] then
                    seen[port] = true
                    port_entries[#port_entries + 1] = {
                        port     = tonumber(port),
                        port_str = port,
                        cmd      = cmd,
                        pid      = pid,
                        addr     = addr,
                    }
                end
            end

            ::continue::
        end

        table.sort(port_entries, function(a, b) return a.port < b.port end)

        local items = {}
        for _, e in ipairs(port_entries) do
            items[#items + 1] = {
                label      = ":" .. e.port_str .. "  " .. e.cmd,
                detail     = "PID " .. e.pid .. " · " .. e.addr,
                icon       = "◉",
                copy_text  = e.port_str,
                actions    = {
                    { label = "Kill Process", kind = "shell",     args = { "kill", "-15", e.pid }, confirm = true },
                    { label = "Force Kill",   kind = "shell",     args = { "kill", "-9",  e.pid }, confirm = true },
                    { label = "Copy Port",    kind = "clipboard", args = { e.port_str } },
                },
            }
        end

        if #items == 0 then
            return { title = "Listening Ports", items = { { label = "No listening ports found", icon = "📭" } } }
        end

        return { title = "Listening Ports — " .. #items, items = items }
    end,
})
