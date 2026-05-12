-- Mail: Inbox -- recent messages across all accounts via osascript+JXA.
--
-- v1.0: read-only inbox view. Each row carries the message body in its
-- preview field (so Telescope previewer shows it without round-trips)
-- and a single "Open in Mail.app" action. Mutating actions (mark read,
-- archive, flag) land in the Triage command (Phase 4.D).
--
-- Performance: single JXA call returns the last 30 messages per account
-- with bodies inline (~3s warm-cache). Cold-cache first invocation can
-- take 10-15s while Mail.app indexes; subsequent calls are fast.
--
-- Canonical helpers live in lib.lua; inlined here with SHARED markers
-- since the mlua sandbox has no require().

-- SHARED: jxa_run (canonical in lib.lua)
local function jxa_run(script)
    local r = lark.exec_io("osascript", { "-l", "JavaScript" }, { stdin = script })
    if r.exit_code ~= 0 then
        local stderr_trim = r.stderr ~= "" and r.stderr:match("^(.-)%s*$") or "(no stderr)"
        return nil, "osascript exit " .. r.exit_code .. ": " .. stderr_trim
    end
    local stdout = r.stdout or ""
    if stdout:match("^%s*$") then return {}, nil end
    local ok, data = pcall(lark.json.decode, stdout)
    if not ok then
        return nil, "JXA returned non-JSON: " .. tostring(data) .. " | head: " .. stdout:sub(1, 200)
    end
    return data, nil
end

-- SHARED: time_ago
local function time_ago(iso)
    if not iso or iso == "" then return "" end
    local now_s = tonumber(lark.exec("date", { "+%s" }):match("%d+") or "0")
    local t = lark.exec("date", { "-j", "-f", "%Y-%m-%dT%H:%M:%S", iso:sub(1, 19), "+%s" })
    local then_s = tonumber((t or ""):match("%d+") or "0")
    if then_s == 0 then return "" end
    local delta = now_s - then_s
    if delta < 60 then return "just now" end
    if delta < 3600 then return math.floor(delta / 60) .. "m ago" end
    if delta < 86400 then return math.floor(delta / 3600) .. "h ago" end
    if delta < 86400 * 7 then return math.floor(delta / 86400) .. "d ago" end
    return iso:sub(1, 10)
end

-- SHARED: icon_for_message / format_preview / urlencode / format_message_row
local function icon_for_message(msg)
    if msg.flagged then return "🚩" end
    if not msg.readStatus then return "📨" end
    return "📧"
end
local function format_preview(msg)
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
        local body = msg.body:gsub("\r\n", "\n"):gsub("\n\n\n+", "\n\n")
        table.insert(lines, body)
    end
    return table.concat(lines, "\n")
end
local function urlencode(s)
    return (s:gsub("([^A-Za-z0-9%-_.~])", function(c)
        return string.format("%%%02X", string.byte(c))
    end))
end
local function format_message_row(msg)
    local sender_short = (msg.sender or ""):gsub(" <.->", "")
    if #sender_short > 30 then sender_short = sender_short:sub(1, 27) .. "..." end
    local label = (msg.subject ~= "" and msg.subject or "(no subject)")
    if #label > 70 then label = label:sub(1, 67) .. "..." end
    local detail = sender_short .. " · " .. time_ago(msg.dateReceived)
    local actions = {}
    if msg.id then
        local mail_url = "message:" .. urlencode("<" .. msg.id .. ">")
        table.insert(actions, { label = "Open in Mail.app", kind = "open", args = { mail_url } })
    end
    return {
        icon = icon_for_message(msg),
        label = label,
        detail = detail,
        preview = format_preview(msg),
        copy_text = msg.subject ~= "" and msg.subject or msg.sender,
        actions = actions,
    }
end

-- SHARED: error_item
local function error_item(message, help_url)
    return { icon = "!", label = message, help_url = help_url, actions = {} }
end

-- JXA: fetch recent INBOX messages across all enabled accounts.
-- Returns array of message objects sorted newest-first.
local JXA_INBOX = [[
ObjC.import('Foundation');
const Mail = Application("Mail");
const N = 30;
const result = [];
const accounts = Mail.accounts();
for (let a = 0; a < accounts.length; a++) {
  const acc = accounts[a];
  if (!acc.enabled()) continue;
  let inbox;
  try { inbox = acc.mailboxes.byName("INBOX"); } catch(e) { continue; }
  const total = inbox.messages().length;
  const start = Math.max(0, total - N);
  for (let i = total - 1; i >= start; i--) {
    const m = inbox.messages[i];
    try {
      result.push({
        id: m.messageId(),
        account: acc.name(),
        subject: m.subject() || "",
        sender: m.sender() || "",
        dateReceived: m.dateReceived().toISOString(),
        readStatus: m.readStatus(),
        flagged: m.flaggedStatus(),
        body: (m.content() || "").slice(0, 5000),
      });
    } catch(e) {}
  }
}
// Sort all messages newest-first across accounts.
result.sort((a, b) => b.dateReceived.localeCompare(a.dateReceived));
JSON.stringify(result);
]]

lark.register({
    on_run = function()
        local messages, err = jxa_run(JXA_INBOX)
        if err then
            local help = err:find("not authorized", 1, true)
                and "x-apple.systempreferences:com.apple.preference.security?Privacy_AppleEvents"
                or nil
            return { title = "Inbox", items = { error_item("Mail error: " .. err, help) } }
        end

        if not messages or #messages == 0 then
            return {
                title = "Inbox",
                items = { { icon = "🎉", label = "Inbox zero — nothing to read", actions = {} } },
            }
        end

        local items = {}
        for _, msg in ipairs(messages) do
            table.insert(items, format_message_row(msg))
        end
        return { title = "Inbox", items = items }
    end,
})
