-- System Info (Lua) — system metrics with table output.
-- Demonstrates lark.exec() and table columns.
-- Note: lark.exec() uses tokio::process::Command with explicit args (safe, no shell).

lark.register({
    on_run = function()
        local host = lark.exec("hostname"):match("^(.-)%s*$")

        -- Disk usage (root filesystem)
        local df = lark.exec("df", { "-h", "/" })
        local disk = "unavailable"
        for line in df:gmatch("[^\n]+") do
            if not line:match("^Filesystem") then
                local used, total, pct = line:match("(%S+)%s+(%S+)%s+%S+%s+(%S+)")
                if used and total and pct then
                    disk = used .. " used / " .. total .. " total (" .. pct .. " full)"
                end
                break
            end
        end

        -- Load average
        local load_avg = "unavailable"
        local uname = lark.exec("uname"):match("^(.-)%s*$")
        if uname == "Darwin" then
            local sysctl = lark.exec("sysctl", { "-n", "vm.loadavg" })
            load_avg = sysctl:match("{%s*(.-)%s*}") or sysctl:match("^(.-)%s*$")
        else
            local proc = lark.exec("cat", { "/proc/loadavg" })
            load_avg = proc:match("^(.-)%s") or proc:match("^(.-)%s*$")
        end

        -- Uptime
        local uptime_raw = lark.exec("uptime"):match("^(.-)%s*$")
        local uptime = uptime_raw:match("up%s+(.-),%s+%d+ user") or uptime_raw

        return {
            title = "System Info — " .. host,
            columns = {
                { header = "Metric", key = "label" },
                { header = "Value", key = "detail" },
            },
            items = {
                { label = "Hostname", detail = host },
                { label = "Disk (root)", detail = disk },
                { label = "Load Average", detail = load_avg },
                { label = "Uptime", detail = uptime },
            },
        }
    end,
})
