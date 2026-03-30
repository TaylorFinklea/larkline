-- Calendar: My Schedule — threaded timeline view of upcoming events.

local function check_icalbuddy()
    local path = lark.exec("which", { "icalbuddy" })
    return path and path:match("%S") ~= nil
end

local function parse_events(raw)
    local events = {}
    local current = nil

    for line in (raw .. "\n"):gmatch("([^\n]*)\n") do
        if line:match("^EVENT:") then
            if current then events[#events + 1] = current end
            current = { title = line:sub(7), date = "", time = "", location = "" }
        elseif current and line:match("^%s") then
            local prop = line:gsub("^%s+", "")
            local d, t_start, t_end = prop:match("^(%d%d%d%d%-%d%d%-%d%d) at (%d+:%d+) %- (%d+:%d+)")
            if d then
                current.date = d
                current.time = t_start .. " – " .. t_end
            else
                local d_only = prop:match("^(%d%d%d%d%-%d%d%-%d%d)")
                if d_only then
                    current.date = d_only
                    current.time = "All day"
                elseif current.location == "" and prop ~= "" then
                    current.location = prop
                end
            end
        end
    end
    if current then events[#events + 1] = current end
    return events
end

local function date_label(date_str, today, tomorrow)
    if date_str == today then return "Today" end
    if date_str == tomorrow then return "Tomorrow" end
    local y, m, d = date_str:match("(%d+)-(%d+)-(%d+)")
    if not y then return date_str end
    local months = { "Jan", "Feb", "Mar", "Apr", "May", "Jun",
                     "Jul", "Aug", "Sep", "Oct", "Nov", "Dec" }
    local month_name = months[tonumber(m)] or m
    -- Get day of week.
    local dow = lark.exec("date", { "-jf", "%Y-%m-%d", date_str, "+%a" })
    dow = dow and dow:match("%a+") or ""
    return dow .. " " .. month_name .. " " .. tonumber(d)
end

lark.register({
    on_run = function()
        if not check_icalbuddy() then
            return {
                title = "My Schedule",
                raw_text = "  ⚠  ical-buddy not installed\n\n  Run: brew install ical-buddy",
            }
        end

        local raw = lark.exec("icalbuddy", {
            "-n", "-nc",
            "-b", "EVENT:",
            "-ab", "  ",
            "-iep", "title,datetime,location",
            "-df", "%Y-%m-%d",
            "-tf", "%H:%M",
            "-npn", "-nrd",
            "eventsToday+14",
        })

        if not raw or raw:match("^%s*$") then
            return {
                title = "My Schedule",
                raw_text = "  🎉  No upcoming events in the next 2 weeks\n\n  Enjoy the free time!",
            }
        end

        local events = parse_events(raw)
        if #events == 0 then
            return {
                title = "My Schedule",
                raw_text = "  🎉  No upcoming events\n",
            }
        end

        local today = (lark.exec("date", { "+%Y-%m-%d" }) or ""):match("%d%d%d%d%-%d%d%-%d%d") or ""
        local tomorrow = (lark.exec("date", { "-v+1d", "+%Y-%m-%d" }) or ""):match("%d%d%d%d%-%d%d%-%d%d") or ""

        -- Group events by date.
        local days = {}
        local day_order = {}
        for _, ev in ipairs(events) do
            local d = ev.date ~= "" and ev.date or "Unknown"
            if not days[d] then
                days[d] = {}
                day_order[#day_order + 1] = d
            end
            days[d][#days[d] + 1] = ev
        end

        -- Build threaded timeline with ANSI colors.
        local lines = {}
        local dim = "\027[2m"
        local reset = "\027[0m"
        local bold = "\027[1m"
        local cyan = "\027[36m"
        local yellow = "\027[33m"
        local green = "\027[32m"
        local magenta = "\027[35m"

        for i, d in ipairs(day_order) do
            local label = date_label(d, today, tomorrow)
            local is_today = (d == today)
            local header_color = is_today and green or cyan

            -- Spacing between days.
            if i > 1 then
                lines[#lines + 1] = dim .. "  │" .. reset
            end

            -- Day header.
            lines[#lines + 1] = header_color .. bold .. "  ── " .. label .. " " .. dim .. d .. reset

            local day_events = days[d]
            for _, ev in ipairs(day_events) do
                lines[#lines + 1] = dim .. "  │" .. reset

                local title = ev.title or "?"
                if ev.time == "All day" then
                    lines[#lines + 1] = yellow .. "  ◆ " .. reset
                        .. dim .. "all day   " .. reset
                        .. title
                else
                    local time_display = ev.time ~= "" and ev.time or "??:??"
                    lines[#lines + 1] = yellow .. "  ● " .. reset
                        .. cyan .. time_display .. reset
                        .. "  " .. title
                end

                if ev.location and ev.location ~= "" then
                    lines[#lines + 1] = dim .. "  │  " .. magenta .. "📍 " .. ev.location .. reset
                end
            end
        end

        -- Summary footer.
        local total = #events
        lines[#lines + 1] = dim .. "  │" .. reset
        lines[#lines + 1] = dim .. "  ╰── " .. total .. " event"
            .. (total == 1 and "" or "s") .. " · "
            .. #day_order .. " day" .. (#day_order == 1 and "" or "s") .. reset

        return {
            title = "My Schedule",
            raw_text = table.concat(lines, "\n"),
        }
    end,
})
