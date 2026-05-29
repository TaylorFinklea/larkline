-- Caffeinate: Extend — add N minutes to the current keep-awake session by
-- restarting the macOS `caffeinate` process with (remaining + N) minutes. If
-- nothing is running, starts a fresh session of N minutes; an indefinite
-- session stays indefinite. Canonical helpers in lib.lua; inlined here
-- (mlua sandbox has no require()).

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
                    title = "Extend Caffeinate",
                    items = { { label = "Enter a positive number of minutes", icon = "⚠" } },
                }
            end

            local add = tonumber(minutes) * 60
            local pid = lark.store.get("pid")
            local ending_at = tonumber(lark.store.get("ending_at")) or 0

            local new_secs
            if pid_alive(pid) and ending_at == 0 then
                -- Already indefinite — keep it that way.
                new_secs = 0
            elseif pid_alive(pid) then
                -- Running with a deadline: add to whatever's left.
                local remaining = ending_at - now()
                if remaining < 0 then
                    remaining = 0
                end
                new_secs = remaining + add
            else
                -- Nothing running — extend just starts a fresh session.
                new_secs = add
            end

            local newpid = start_session(new_secs)
            if not newpid then
                return {
                    title = "Extend Caffeinate",
                    level = "warn",
                    items = { { label = "Failed to extend caffeinate", icon = "⚠" } },
                }
            end

            local detail
            if new_secs == 0 then
                detail = "Session is now indefinite"
            else
                detail = "Now " .. tostring(math.floor(new_secs / 60)) .. " minutes total"
            end

            return {
                title = "Extend Caffeinate",
                items = { {
                    label  = "Extended by " .. minutes .. " minutes",
                    detail = detail,
                    icon   = "☕",
                } },
            }
        end

        return {
            title = "Extend Caffeinate",
            form = {
                fields = {
                    {
                        id            = "minutes",
                        label         = "Extend by (minutes)",
                        type          = { kind = "text" },
                        required      = true,
                        placeholder   = "e.g. 30, 60, 90",
                        default_value = "30",
                    },
                },
                submit_label = "Extend",
            },
        }
    end,
})
