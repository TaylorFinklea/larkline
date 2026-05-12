-- Mail: Search -- full-text search across all enabled INBOXes.
--
-- Form-driven: prompts for a query string, then runs Mail.app's JXA
-- whose-filter on subject + sender across all account inboxes. Body
-- search is intentionally excluded -- too slow on the Apple Event
-- bridge. For body search use Mail.app's native Cmd-F.
--
-- Returns up to 100 matches sorted newest-first. Each row carries the
-- same shape as Inbox: subject as label, sender + relative time as
-- detail, body in preview, "Open in Mail.app" primary action.

-- SHARED helpers (canonical in lib.lua) -----
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
    table.insert(lines, "")
    if msg.body and msg.body ~= "" then
        table.insert(lines, "---"); table.insert(lines, "")
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

local function error_item(message, help_url)
    return { icon = "!", label = message, help_url = help_url, actions = {} }
end

local function js_str(s)
    local enc, _ = lark.json.encode(s)
    return enc or '""'
end

-- Build JXA script that searches all INBOX messages.
local function build_search_jxa(query)
    return string.format([[
ObjC.import('Foundation');
const Mail = Application("Mail");
const q = %s;
const N = 100;
const result = [];
const accounts = Mail.accounts();
for (let a = 0; a < accounts.length; a++) {
  const acc = accounts[a];
  if (!acc.enabled()) continue;
  let inbox;
  try { inbox = acc.mailboxes.byName("INBOX"); } catch(e) { continue; }
  let matches;
  try {
    matches = inbox.messages.whose({
      _or: [
        { subject: { _contains: q } },
        { sender:  { _contains: q } }
      ]
    })();
  } catch(e) { continue; }
  for (let i = 0; i < matches.length && result.length < N; i++) {
    const m = matches[i];
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
result.sort((a, b) => b.dateReceived.localeCompare(a.dateReceived));
JSON.stringify(result);
]], js_str(query))
end

lark.register({
    on_run = function()
        if lark.form_values then
            local query = (lark.form_values.query or ""):match("^%s*(.-)%s*$")
            if query == "" then
                return {
                    title = "Mail Search",
                    items = { error_item("Query was empty — try again") },
                }
            end
            local messages, err = jxa_run(build_search_jxa(query))
            if err then
                return { title = "Mail Search", items = { error_item("Mail error: " .. err) } }
            end
            if not messages or #messages == 0 then
                return {
                    title = "Mail Search: " .. query,
                    items = { { icon = "🔍", label = "No matches", actions = {} } },
                }
            end
            local items = {}
            for _, msg in ipairs(messages) do
                table.insert(items, format_message_row(msg))
            end
            return { title = "Mail Search: " .. query, items = items }
        end

        return {
            title = "Mail Search",
            form = {
                fields = {
                    { id = "query", label = "Search query (subject + sender)",
                      type = { kind = "text" }, placeholder = "any text" },
                },
            },
        }
    end,
})
