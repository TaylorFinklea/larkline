-- Calendar: Tomorrow -- structured events for tomorrow.
--
-- Same backend as schedule.lua but scoped to tomorrow's date range. Single
-- day, no day headers. Falls back to icalbuddy when helper isn't on PATH.
-- Canonical helpers in lib.lua; inlined here with SHARED markers.

-- SHARED: has_helper / helper_call (canonical in lib.lua)
local HELPER = "larkline-macos-helper"
local function has_helper()
    local r = lark.exec("which", { HELPER })
    return r ~= nil and r:match("%S") ~= nil
end
local function helper_call(command, args)
    local req = { id = "1", command = command, args = args }
    local req_json, err = lark.json.encode(req)
    if not req_json then return nil, "encode error: " .. tostring(err) end
    local r = lark.exec_io(HELPER, nil, { stdin = req_json .. "\n" })
    if r.exit_code ~= 0 then
        return nil, "helper exit " .. r.exit_code .. ": " .. (r.stderr ~= "" and r.stderr or "no stderr")
    end
    for line in (r.stdout .. "\n"):gmatch("([^\n]*)\n") do
        if line ~= "" then
            local ok, parsed = pcall(lark.json.decode, line)
            if ok and parsed and parsed.kind ~= "hello" then
                if parsed.ok then return parsed.data, nil end
                return nil, parsed.error or "unknown helper error"
            end
        end
    end
    return nil, "no response from helper"
end

-- SHARED: format_time / format_date / iso_for_offset
local function format_time(iso, all_day)
    if all_day then return "all day" end
    local h, m = iso:match("T(%d%d):(%d%d):")
    if h then return h .. ":" .. m end
    return iso:sub(1, 16)
end
local function format_date(iso) return iso:sub(1, 10) end
local function iso_for_offset(days)
    local sign = days >= 0 and "+" or ""
    local d = lark.exec("date", { "-v" .. sign .. days .. "d", "+%Y-%m-%dT00:00:00%z" })
    return d and d:match("[^\n]+") or nil
end

-- SHARED: icon_for_event / format_preview / format_event_row
local function icon_for_event(event)
    if event.meetingURL then return "📹" end
    if event.allDay then return "📌" end
    return "🗓 "
end
local function format_preview(event)
    local lines = {}
    table.insert(lines, "## " .. (event.title or "(no title)"))
    table.insert(lines, "")
    if event.allDay then
        table.insert(lines, "**Time:** all day · " .. format_date(event.start_iso))
    else
        table.insert(lines, "**Time:** " .. event.start_iso:sub(1, 16) .. " → " .. event.end_iso:sub(1, 16))
    end
    if event.location and event.location ~= "" then
        table.insert(lines, "**Location:** " .. event.location)
    end
    if event.calendarTitle then
        table.insert(lines, "**Calendar:** " .. event.calendarTitle ..
            (event.calendarSource and " (" .. event.calendarSource .. ")" or ""))
    end
    if event.meetingURL then
        table.insert(lines, "**Meeting:** " .. event.meetingURL)
    end
    if event.attendees and #event.attendees > 0 then
        table.insert(lines, "")
        table.insert(lines, "### Attendees")
        for _, a in ipairs(event.attendees) do
            local who = a.name or a.email or "?"
            local marker = a.isCurrentUser and " *(you)*" or ""
            local status = (a.status and a.status ~= "needs_action") and " — " .. a.status or ""
            table.insert(lines, "- " .. who .. marker .. status)
        end
    end
    if event.notes and event.notes ~= "" then
        table.insert(lines, "")
        table.insert(lines, "### Notes")
        table.insert(lines, event.notes)
    end
    return table.concat(lines, "\n")
end
local function format_event_row(event)
    local time_str = format_time(event.start_iso, event.allDay)
    local label = time_str .. "  " .. (event.title or "(no title)")
    local detail = (event.location ~= nil and event.location ~= "") and event.location
        or (event.calendarTitle or nil)
    local actions = {}
    if event.meetingURL then
        table.insert(actions, { label = "Join meeting", kind = "open", args = { event.meetingURL } })
    end
    table.insert(actions, {
        label = "Open in Calendar.app", kind = "open",
        args = { "ical://event/" .. event.id },
    })
    if event.meetingURL then
        table.insert(actions, {
            label = "Copy meeting link", kind = "clipboard",
            args = { event.meetingURL },
        })
    end
    return {
        icon = icon_for_event(event),
        label = label,
        detail = detail,
        preview = format_preview(event),
        copy_text = event.meetingURL or event.title,
        actions = actions,
    }
end

-- SHARED: error_item / icalbuddy_fallback
local function error_item(message, help_url)
    return { icon = "!", label = message, help_url = help_url, actions = {} }
end
local function icalbuddy_fallback()
    local r = lark.exec("which", { "icalbuddy" })
    if not r or r:match("%S") == nil then
        return { error_item("Neither larkline-macos-helper nor icalbuddy installed",
                            "https://hasseg.org/icalBuddy/") }
    end
    -- icalbuddy has no "tomorrow" verb — passing one makes it print its usage
    -- text. Compute tomorrow's date and query that single day with the
    -- documented eventsFrom:/to: range; fall back to eventsToday+1
    -- (today..tomorrow) only if the date shell call fails.
    local d = lark.exec("date", { "-v+1d", "+%Y-%m-%d" })
    d = d and d:match("%d%d%d%d%-%d%d%-%d%d") or nil
    local args = { "-n", "-nc", "-b", "EVENT: ", "-iep", "title,datetime,location", "-npn", "-nrd" }
    if d then
        args[#args + 1] = "eventsFrom:" .. d
        args[#args + 1] = "to:" .. d
    else
        args[#args + 1] = "eventsToday+1"
    end
    local raw = lark.exec("icalbuddy", args)
    if not raw or raw:match("^%s*$") then
        return { { icon = "🎉", label = "Nothing on the calendar tomorrow", actions = {} } }
    end
    return { { icon = "📋", label = "Tomorrow (icalbuddy fallback)", preview = raw, actions = {} } }
end

lark.register({
    on_run = function()
        if not has_helper() then
            return { title = "Tomorrow", items = icalbuddy_fallback() }
        end

        local start_iso = iso_for_offset(1)
        local end_iso = iso_for_offset(2)
        if not start_iso or not end_iso then
            return { title = "Tomorrow", items = { error_item("date(1) shell command failed") } }
        end

        local data, err = helper_call("events_for_range", {
            start_iso = start_iso, end_iso = end_iso,
        })
        if err then
            local help = err:find("calendar access denied", 1, true)
                and "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars"
                or nil
            return { title = "Tomorrow", items = { error_item(err, help) } }
        end

        local events = data and data.events or {}
        if #events == 0 then
            return {
                title = "Tomorrow",
                items = { { icon = "🎉", label = "Nothing on the calendar tomorrow", actions = {} } },
            }
        end

        local items = {}
        for _, event in ipairs(events) do
            table.insert(items, format_event_row(event))
        end
        return { title = "Tomorrow", items = items }
    end,
})
