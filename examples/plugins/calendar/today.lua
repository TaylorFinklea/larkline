-- Calendar: Today — events from Calendar.app for today via ical-buddy.

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
            current = { title = line:sub(7), time = "", location = "" }
        elseif current and line:match("^  ") then
            local prop = line:gsub("^%s+", "")
            if current.time == "" then
                current.time = prop
            elseif current.location == "" then
                current.location = prop
            end
        end
    end
    if current then events[#events + 1] = current end

    return events
end

lark.register({
    on_run = function()
        if not check_icalbuddy() then
            return {
                title = "Calendar — Today",
                items = { {
                    label  = "ical-buddy not installed",
                    icon   = "⚠",
                    detail = "Run: brew install ical-buddy",
                } },
            }
        end

        local raw = lark.exec("icalbuddy", {
            "-n", "-nc",
            "-b", "EVENT:",
            "-ab", "  ",
            "-iep", "title,datetime,location",
            "eventsToday",
        })

        if not raw or raw:match("^%s*$") then
            return {
                title = "Calendar — Today",
                items = { { label = "No events today", icon = "🎉" } },
            }
        end

        local events = parse_events(raw)

        if #events == 0 then
            return {
                title = "Calendar — Today",
                items = { { label = "No events today", icon = "🎉" } },
            }
        end

        local items = {}
        for _, ev in ipairs(events) do
            local detail = ev.time
            if ev.location ~= "" then
                detail = detail .. " · " .. ev.location
            end
            items[#items + 1] = {
                label     = ev.title,
                detail    = detail,
                icon      = "◷",
                copy_text = ev.title,
                actions   = {
                    { label = "Open Calendar", kind = "shell",     args = { "open", "-a", "Calendar" } },
                    { label = "Copy Title",    kind = "clipboard", args = { ev.title } },
                },
            }
        end

        return { title = "Today — " .. #items .. " event" .. (#items == 1 and "" or "s"), items = items }
    end,
})
