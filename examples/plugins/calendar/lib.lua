-- Calendar plugin shared helpers.
--
-- Plugins in this directory (schedule.lua, today.lua, tomorrow.lua) share
-- the same helper-invocation primitives + event-formatting code. mlua's
-- sandbox has no require(), so each plugin file inlines its own copy of
-- the bits it needs with `-- SHARED:` markers; this file is the canonical
-- source-of-truth for diff'ing when one of them drifts.
--
-- Backend selection:
--   * macOS with larkline-macos-helper on PATH -> rich events (meetingURL
--     extraction, attendees, calendar source labels).
--   * Anywhere else (Linux, dev env without helper installed) -> falls
--     back to icalbuddy with a degraded-but-functional row format.

local M = {}

local HELPER = "larkline-macos-helper"

-- SHARED: has_helper / helper_call
function M.has_helper()
    local r = lark.exec("which", { HELPER })
    return r ~= nil and r:match("%S") ~= nil
end

-- Send a single JSON request to the helper and parse the response. Returns
-- (data, err) -- err is non-nil on failure. The helper emits an unsolicited
-- `hello` line on startup before processing input; we skip it.
function M.helper_call(command, args)
    local req = { id = "1", command = command, args = args }
    local req_json, err = lark.json.encode(req)
    if not req_json then
        return nil, "encode error: " .. tostring(err)
    end
    local r = lark.exec_io(HELPER, nil, { stdin = req_json .. "\n" })
    if r.exit_code ~= 0 then
        return nil, "helper exit " .. r.exit_code .. ": " .. (r.stderr ~= "" and r.stderr or "no stderr")
    end
    for line in (r.stdout .. "\n"):gmatch("([^\n]*)\n") do
        if line ~= "" then
            local ok, parsed = pcall(lark.json.decode, line)
            if ok and parsed and parsed.kind ~= "hello" then
                if parsed.ok then
                    return parsed.data, nil
                else
                    return nil, parsed.error or "unknown helper error"
                end
            end
        end
    end
    return nil, "no response from helper"
end

-- SHARED: format_time / format_date / date_label
-- ISO 8601 strings from the helper come in local-timezone form
-- (e.g. "2026-05-13T08:00:00-05:00"). Time-portion extraction is regex.
function M.format_time(iso, all_day)
    if all_day then return "all day" end
    local h, m = iso:match("T(%d%d):(%d%d):")
    if h then return h .. ":" .. m end
    return iso:sub(1, 16)
end

function M.format_date(iso)
    return iso:sub(1, 10)
end

-- "Today" / "Tomorrow" / "Fri Jun 5" for a given date string vs today.
function M.date_label(date_str, today_str, tomorrow_str)
    if date_str == today_str then return "Today" end
    if date_str == tomorrow_str then return "Tomorrow" end
    local y, mo, d = date_str:match("(%d+)-(%d+)-(%d+)")
    if not y then return date_str end
    local months = { "Jan", "Feb", "Mar", "Apr", "May", "Jun",
                     "Jul", "Aug", "Sep", "Oct", "Nov", "Dec" }
    local dow = lark.exec("date", { "-jf", "%Y-%m-%d", date_str, "+%a" })
    dow = dow and dow:match("%a+") or ""
    return dow .. " " .. (months[tonumber(mo)] or mo) .. " " .. tonumber(d)
end

-- SHARED: iso_for_offset
-- Returns ISO 8601 timestamp (local TZ midnight) for today + days. Used
-- to build start_iso / end_iso for events_for_range.
function M.iso_for_offset(days)
    local sign = days >= 0 and "+" or ""
    local d = lark.exec("date", { "-v" .. sign .. days .. "d", "+%Y-%m-%dT00:00:00%z" })
    return d and d:match("[^\n]+") or nil
end

function M.today_iso() return M.iso_for_offset(0) end
function M.tomorrow_iso() return M.iso_for_offset(1) end

-- SHARED: icon_for_event / format_preview / format_event_row
function M.icon_for_event(event)
    if event.meetingURL then return "📹" end
    if event.allDay then return "📌" end
    return "🗓 "
end

function M.format_preview(event)
    local lines = {}
    table.insert(lines, "## " .. (event.title or "(no title)"))
    table.insert(lines, "")
    if event.allDay then
        table.insert(lines, "**Time:** all day · " .. M.format_date(event.start_iso))
    else
        table.insert(lines, "**Time:** " ..
            event.start_iso:sub(1, 16) .. " → " .. event.end_iso:sub(1, 16))
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
            local status = (a.status and a.status ~= "needs_action")
                and " — " .. a.status or ""
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

-- Build the OutputItem for a single helper event.
function M.format_event_row(event)
    local time_str = M.format_time(event.start_iso, event.allDay)
    local label = time_str .. "  " .. (event.title or "(no title)")
    local detail = (event.location ~= nil and event.location ~= "") and event.location
        or (event.calendarTitle or nil)

    local actions = {}
    if event.meetingURL then
        table.insert(actions, {
            label = "Join meeting",
            kind = "open",
            args = { event.meetingURL },
        })
    end
    table.insert(actions, {
        label = "Open in Calendar.app",
        kind = "open",
        args = { "ical://event/" .. event.id },
    })
    if event.meetingURL then
        table.insert(actions, {
            label = "Copy meeting link",
            kind = "clipboard",
            args = { event.meetingURL },
        })
    end

    return {
        icon = M.icon_for_event(event),
        label = label,
        detail = detail,
        preview = M.format_preview(event),
        copy_text = event.meetingURL or event.title,
        actions = actions,
    }
end

-- SHARED: error_item -- surface helper failures as a single row.
function M.error_item(message, help_url)
    return {
        icon = "!",
        label = message,
        help_url = help_url,
        actions = {},
    }
end

-- SHARED: icalbuddy_fallback
-- Degraded path for Linux / dev environments where the macOS helper isn't
-- available. Renders icalbuddy output as a single text-block item (no
-- per-event actions, no meeting URLs). Better than failing.
function M.icalbuddy_fallback(range_label)
    local r = lark.exec("which", { "icalbuddy" })
    if not r or r:match("%S") == nil then
        return {
            M.error_item(
                "Neither larkline-macos-helper nor icalbuddy installed",
                "https://hasseg.org/icalBuddy/"
            ),
        }
    end
    local raw = lark.exec("icalbuddy", {
        "-n", "-nc", "-b", "EVENT: ",
        "-iep", "title,datetime,location",
        "-npn", "-nrd",
        range_label or "eventsToday+14",
    })
    if not raw or raw:match("^%s*$") then
        return {
            {
                icon = "🎉",
                label = "No upcoming events",
                actions = {},
            },
        }
    end
    return {
        {
            icon = "📋",
            label = "Calendar (icalbuddy fallback)",
            preview = raw,
            actions = {},
        },
    }
end

return M
