-- Caffeinate: Status — state of OUR keep-awake session. We own it via the
-- built-in macOS `caffeinate` command (no third-party CLI): a detached
-- `caffeinate -d -i [-t SECS]` whose PID + end time live in lark.store.
-- Canonical helpers in lib.lua; inlined here (mlua sandbox has no require()).

-- SHARED: now / pid_alive / stop (canonical in lib.lua)
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

local function clear_state()
    lark.store.delete("pid")
    lark.store.delete("ending_at")
end

local function fmt_remaining(secs)
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

lark.register({
    on_run = function()
        local pid = lark.store.get("pid")

        if not pid_alive(pid) then
            -- Nothing running (or it ended / was killed) — heal stale state.
            clear_state()
            return {
                title = "Caffeinate — idle",
                status = "off",
                items = { {
                    label  = "Inactive",
                    detail = "Mac will sleep normally — use Start to keep it awake",
                    icon   = "💤",
                } },
            }
        end

        local ending_at = tonumber(lark.store.get("ending_at")) or 0
        local items = {}
        local chip

        if ending_at > 0 then
            local remaining = ending_at - now()
            if remaining < 0 then
                remaining = 0
            end
            chip = fmt_remaining(remaining)
            items[#items + 1] = {
                label  = "Active — " .. chip .. " remaining",
                detail = "Mac is staying awake (caffeinate -d -i)",
                icon   = "☕",
            }
        else
            chip = "on"
            items[#items + 1] = {
                label  = "Active — indefinite",
                detail = "Mac is staying awake until you stop it",
                icon   = "☕",
            }
        end

        items[#items + 1] = {
            label     = "PID " .. tostring(pid),
            icon      = "🔧",
            copy_text = tostring(pid),
        }
        items[#items + 1] = {
            label   = "Stop",
            icon    = "⏹",
            actions = {
                { label = "Stop caffeinate", kind = "shell", args = { "kill", tostring(pid) } },
            },
        }

        return { title = "Caffeinate — active", status = chip, items = items }
    end,
})
