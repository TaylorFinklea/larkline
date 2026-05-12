-- Mail plugin shared helpers.
--
-- Bridges to Apple Mail via osascript with JavaScript for Automation (JXA).
-- All Mail.app interaction happens through JXA scripts piped to osascript's
-- stdin. The lark.exec_io host fn (v1.0+) captures stdout + stderr + exit
-- code, so we can surface errors structurally.
--
-- Pattern matches mail-app-cli and apple-mail-mcp -- the proven 2025-era
-- approach for third-party Mail.app access (MailKit is extension-only).
--
-- mlua sandbox has no require(); plugins in this dir copy the bits they
-- need with -- SHARED: markers. This file is the canonical source.

local M = {}

-- SHARED: jxa_run -- run a JXA script and return the parsed JSON result.
-- Returns (data, err) -- err is non-nil on failure. The JXA script
-- should end with `JSON.stringify(value)` so its stdout is parseable.
function M.jxa_run(script)
    local r = lark.exec_io("osascript", { "-l", "JavaScript" }, { stdin = script })
    if r.exit_code ~= 0 then
        local stderr_trim = r.stderr ~= "" and r.stderr:match("^(.-)%s*$") or "(no stderr)"
        return nil, "osascript exit " .. r.exit_code .. ": " .. stderr_trim
    end
    local stdout = r.stdout or ""
    if stdout:match("^%s*$") then return {}, nil end
    local ok, data = pcall(lark.json.decode, stdout)
    if not ok then
        return nil, "JXA returned non-JSON: " .. tostring(data) ..
            " | head: " .. stdout:sub(1, 200)
    end
    return data, nil
end

-- SHARED: time_ago -- "2h ago", "3d ago", "just now" for relative dates.
function M.time_ago(iso)
    if not iso or iso == "" then return "" end
    local now_s = tonumber(lark.exec("date", { "+%s" }):match("%d+") or "0")
    -- Parse ISO to epoch via `date -j -f` (BSD date).
    local t = lark.exec("date", { "-j", "-f", "%Y-%m-%dT%H:%M:%S",
        iso:sub(1, 19), "+%s" })
    local then_s = tonumber((t or ""):match("%d+") or "0")
    if then_s == 0 then return "" end
    local delta = now_s - then_s
    if delta < 60 then return "just now" end
    if delta < 3600 then return math.floor(delta / 60) .. "m ago" end
    if delta < 86400 then return math.floor(delta / 3600) .. "h ago" end
    if delta < 86400 * 7 then return math.floor(delta / 86400) .. "d ago" end
    -- Older: show date.
    return iso:sub(1, 10)
end

-- SHARED: icon_for_message
function M.icon_for_message(msg)
    if msg.flagged then return "🚩" end
    if not msg.readStatus then return "📨" end
    return "📧"
end

-- SHARED: format_preview -- markdown body for the preview pane.
function M.format_preview(msg)
    local lines = {}
    table.insert(lines, "## " .. (msg.subject ~= "" and msg.subject or "(no subject)"))
    table.insert(lines, "")
    table.insert(lines, "**From:** " .. (msg.sender or "(unknown)"))
    table.insert(lines, "**Account:** " .. (msg.account or ""))
    table.insert(lines, "**Date:** " .. (msg.dateReceived or ""))
    if msg.readStatus then table.insert(lines, "**Status:** read") end
    if msg.flagged then table.insert(lines, "**Flagged:** yes") end
    table.insert(lines, "")
    if msg.body and msg.body ~= "" then
        table.insert(lines, "---")
        table.insert(lines, "")
        -- Strip leading whitespace lines + collapse excessive blanks.
        local body = msg.body:gsub("\r\n", "\n"):gsub("\n\n\n+", "\n\n")
        table.insert(lines, body)
    end
    return table.concat(lines, "\n")
end

-- SHARED: urlencode -- minimal percent-encoding for message: URL scheme.
function M.urlencode(s)
    return (s:gsub("([^A-Za-z0-9%-_.~])", function(c)
        return string.format("%%%02X", string.byte(c))
    end))
end

-- SHARED: format_message_row -- build OutputItem from a message JSON.
function M.format_message_row(msg)
    local sender_short = (msg.sender or ""):gsub(" <.->", "")
    if #sender_short > 30 then sender_short = sender_short:sub(1, 27) .. "..." end

    local label = (msg.subject ~= "" and msg.subject or "(no subject)")
    if #label > 70 then label = label:sub(1, 67) .. "..." end

    local detail = sender_short .. " · " .. M.time_ago(msg.dateReceived)

    local actions = {}
    if msg.id then
        local mail_url = "message:" .. M.urlencode("<" .. msg.id .. ">")
        table.insert(actions, {
            label = "Open in Mail.app",
            kind = "open",
            args = { mail_url },
        })
    end

    return {
        icon = M.icon_for_message(msg),
        label = label,
        detail = detail,
        preview = M.format_preview(msg),
        copy_text = msg.subject ~= "" and msg.subject or msg.sender,
        actions = actions,
    }
end

-- SHARED: error_item
function M.error_item(message, help_url)
    return { icon = "!", label = message, help_url = help_url, actions = {} }
end

return M
