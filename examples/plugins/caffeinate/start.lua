-- Caffeinate: Start — keep the Mac awake for N minutes using the built-in
-- macOS `caffeinate` command. Launches a detached `caffeinate -d -i -t SECS`
-- and records its PID + end time in lark.store; Status reads that back.
-- Canonical helpers in lib.lua; inlined here (mlua sandbox has no require()).

-- SHARED: now / pid_alive / stop / start (canonical in lib.lua)
local function now()
    return tonumber((lark.exec("date", { "+%s" }) or ""):match("%d+")) or 0
end

local function pid_alive(pid)
    if not pid or tostring(pid) == "" then
        return false
    end
    local comm = lark.exec("ps", { "-p", tostring(pid), "-o", "comm=" })
    return comm ~= nil and comm:find("caffeinate", 1, true) ~= nil
end

local function stop_session()
    local pid = lark.store.get("pid")
    if pid_alive(pid) then
        lark.exec("kill", { tostring(pid) })
    end
    lark.store.delete("pid")
    lark.store.delete("ending_at")
end

local function start_session(secs)
    stop_session()
    -- Cap finite sessions at 7 days so a user typo (or a date-failure miscalc in
    -- extend) can't keep the Mac awake for years. <= 0 / nil stays indefinite.
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
        lark.store.set("ending_at", tostring((secs and secs > 0) and (now() + secs) or 0))
    end
    return pid
end

lark.register({
    on_run = function()
        if lark.form_values then
            local minutes = (lark.form_values.minutes or ""):match("^%s*(%d+)%s*$")
            if not minutes or tonumber(minutes) <= 0 then
                return {
                    title = "Start Caffeinate",
                    items = { { label = "Enter a positive number of minutes", icon = "⚠" } },
                }
            end

            local pid = start_session(tonumber(minutes) * 60)
            if not pid then
                return {
                    title = "Start Caffeinate",
                    level = "warn",
                    items = { { label = "Failed to start caffeinate", icon = "⚠" } },
                }
            end

            return {
                title = "Start Caffeinate",
                items = { {
                    label  = "Started for " .. minutes .. " minutes",
                    detail = "Mac will stay awake (PID " .. pid .. ")",
                    icon   = "☕",
                } },
            }
        end

        return {
            title = "Start Caffeinate",
            form = {
                fields = {
                    {
                        id            = "minutes",
                        label         = "Duration (minutes)",
                        type          = { kind = "text" },
                        required      = true,
                        placeholder   = "e.g. 30, 60, 90",
                        default_value = "30",
                    },
                },
                submit_label = "Start",
            },
        }
    end,
})
