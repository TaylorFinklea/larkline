-- Caffeinate plugin — canonical helpers.
--
-- We own the keep-awake session entirely using the built-in macOS `caffeinate`
-- command (no third-party CLI): a detached `caffeinate -d -i [-t SECS]` whose
-- PID + end time we track in lark.store (keyed by the "Caffeinate"
-- plugin_group). `caffeinate -t SECS` self-terminates, so liveness is checked
-- via `ps` (matched on the command name to survive PID recycling).
--
-- The mlua sandbox has no require(), so status.lua / start.lua / extend.lua
-- each inline a copy of these helpers under a "SHARED:" marker. Edit here
-- first, then sync the copies.
--
-- Store keys (all strings): `pid`, `ending_at` (unix seconds, "0" = indefinite).

local M = {}

-- Current unix time. The sandbox has no os.time(); shell out to `date`.
function M.now()
    return tonumber((lark.exec("date", { "+%s" }) or ""):match("%d+")) or 0
end

-- True only if `pid` is a live `caffeinate` process (the command-name check
-- guards against the OS recycling a dead PID onto an unrelated process).
function M.pid_alive(pid)
    if not pid or tostring(pid) == "" then
        return false
    end
    local comm = lark.exec("ps", { "-p", tostring(pid), "-o", "comm=" })
    return comm ~= nil and comm:find("caffeinate", 1, true) ~= nil
end

-- Kill the tracked session (if it's still ours) and clear the stored state.
function M.stop()
    local pid = lark.store.get("pid")
    if M.pid_alive(pid) then
        lark.exec("kill", { tostring(pid) })
    end
    lark.store.delete("pid")
    lark.store.delete("ending_at")
end

-- Start a detached caffeinate for `secs` seconds (<= 0 = indefinite), replacing
-- any running session. Returns the new pid string, or nil on failure.
function M.start(secs)
    M.stop()
    -- Cap finite sessions at 7 days so a date-failure miscalc (now() == 0 makes
    -- extend compute a ~55-year remaining) or a user typo can't keep the Mac
    -- awake for years. A nil / <= 0 duration stays a true indefinite session.
    if secs and secs > 604800 then secs = 604800 end
    local cmd
    if secs and secs > 0 then
        cmd = "nohup caffeinate -d -i -t " .. tostring(secs) .. " >/dev/null 2>&1 & echo $!"
    else
        cmd = "nohup caffeinate -d -i >/dev/null 2>&1 & echo $!"
    end
    local pid = (lark.exec("sh", { "-c", cmd }) or ""):match("%d+")
    if pid then
        lark.store.set("pid", pid)
        lark.store.set("ending_at", tostring((secs and secs > 0) and (M.now() + secs) or 0))
    end
    return pid
end

-- Format a seconds count as a compact chip: "1h 5m", "28m", "45s".
function M.fmt_remaining(secs)
    if secs <= 0 then
        return "0s"
    end
    local h = math.floor(secs / 3600)
    local m = math.floor((secs % 3600) / 60)
    if h > 0 then
        return h .. "h " .. m .. "m"
    end
    if m > 0 then
        return m .. "m"
    end
    return secs .. "s"
end

return M
