-- Kill Process: Processes — running processes sorted by CPU usage.

lark.register({
    on_run = function()
        local raw = lark.exec("ps", { "-axo", "pid,pcpu,pmem,comm" })

        if not raw or raw == "" then
            return {
                title = "Processes",
                items = { { label = "Failed to list processes", icon = "⚠" } },
            }
        end

        local entries = {}
        local first = true
        for line in raw:gmatch("[^\n]+") do
            -- Skip header line
            if first then
                first = false
                goto continue
            end

            line = line:gsub("^%s+", "")
            local pid, pcpu, pmem, comm = line:match("^(%S+)%s+(%S+)%s+(%S+)%s+(.+)$")
            if pid and comm then
                entries[#entries + 1] = {
                    pid     = pid,
                    pcpu    = pcpu,
                    pmem    = pmem,
                    comm    = comm:gsub("%s+$", ""),
                    cpu_num = tonumber(pcpu) or 0,
                }
            end

            ::continue::
        end

        -- Sort by CPU descending, keep top 50
        table.sort(entries, function(a, b) return a.cpu_num > b.cpu_num end)
        if #entries > 50 then
            local trimmed = {}
            for i = 1, 50 do trimmed[i] = entries[i] end
            entries = trimmed
        end

        local items = {}
        for _, e in ipairs(entries) do
            local icon = e.cpu_num > 10 and "●" or (e.cpu_num > 1 and "◉" or "○")
            items[#items + 1] = {
                label     = e.comm,
                detail    = "PID " .. e.pid .. " · CPU " .. e.pcpu .. "% · MEM " .. e.pmem .. "%",
                icon      = icon,
                copy_text = e.pid,
                actions   = {
                    { label = "Terminate (SIGTERM)",  kind = "shell", args = { "kill", "-15", e.pid }, confirm = true },
                    { label = "Force Kill (SIGKILL)", kind = "shell", args = { "kill", "-9",  e.pid }, confirm = true },
                    { label = "Copy PID",             kind = "clipboard", args = { e.pid } },
                },
            }
        end

        return { title = "Processes — " .. #items, items = items }
    end,
})
