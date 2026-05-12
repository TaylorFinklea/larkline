-- Mail: Inbox -- recent messages across all accounts with full action set.
--
-- Same data load as Inbox but each row carries an extended action set
-- for keyboard-driven cleanup:
--   * Open in Mail.app (default)
--   * Toggle read (Mark read / Mark unread)
--   * Toggle flag
--   * Archive
--   * Delete (confirm-gated)
--   * Reply / Forward (open composer window)
--
-- All mutations dispatch via osascript -l JavaScript -e '...' shell actions
-- scoped to the specific (account, mailbox, messageId) so they don't scan
-- all of Mail. JXA's whose({messageId: ...}) filter is the lookup.
--
-- Canonical helpers in lib.lua; inlined here with SHARED markers.

-- SHARED: jxa_run / time_ago / icon_for_message / format_preview / urlencode (from inbox.lua / lib.lua)
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

local function error_item(message, help_url)
    return { icon = "!", label = message, help_url = help_url, actions = {} }
end

-- SHARED: js_str -- encode a Lua string as a JS string literal via JSON.
local function js_str(s)
    local enc, _ = lark.json.encode(s)
    return enc or ('"' .. tostring(s):gsub('"', '\\"') .. '"')
end

-- SHARED: mutation_script -- build a JXA one-liner that looks up the
-- message by (account, mailbox, messageId) and runs `op` against it.
local function mutation_script(account, mbx, id, op)
    return string.format(
        'const m=Application("Mail").accounts.byName(%s).mailboxes.byName(%s).messages.whose({messageId:%s})()[0];if(m){%s}',
        js_str(account), js_str(mbx), js_str(id), op
    )
end

local function mutation_action(label, account, mbx, id, op, opts)
    opts = opts or {}
    return {
        label = label,
        kind = "shell",
        args = { "osascript", "-l", "JavaScript", "-e", mutation_script(account, mbx, id, op) },
        confirm = opts.confirm or false,
    }
end

-- Build OutputItem for a triage row -- same as inbox but with the
-- mutating action set appended after "Open in Mail.app".
local function format_triage_row(msg)
    local sender_short = (msg.sender or ""):gsub(" <.->", "")
    if #sender_short > 30 then sender_short = sender_short:sub(1, 27) .. "..." end
    local label = (msg.subject ~= "" and msg.subject or "(no subject)")
    if #label > 70 then label = label:sub(1, 67) .. "..." end
    local detail = sender_short .. " · " .. time_ago(msg.dateReceived)

    local mbx = msg.mailboxName or "INBOX"
    local id = msg.id or ""
    local acc = msg.account or ""

    local actions = {}
    if id ~= "" then
        local mail_url = "message:" .. urlencode("<" .. id .. ">")
        table.insert(actions, { label = "Open in Mail.app", kind = "open", args = { mail_url } })
    end
    if id ~= "" and acc ~= "" then
        if msg.readStatus then
            table.insert(actions, mutation_action("Mark unread", acc, mbx, id, "m.readStatus = false"))
        else
            table.insert(actions, mutation_action("Mark read",   acc, mbx, id, "m.readStatus = true"))
        end
        local flag_label = msg.flagged and "Unflag" or "Flag"
        local flag_op = msg.flagged and "m.flaggedStatus = false" or "m.flaggedStatus = true"
        table.insert(actions, mutation_action(flag_label, acc, mbx, id, flag_op))
        table.insert(actions, mutation_action("Archive",
            acc, mbx, id, 'Application("Mail").archive(m)'))
        table.insert(actions, mutation_action("Delete",
            acc, mbx, id, 'Application("Mail").delete(m)', { confirm = true }))
        table.insert(actions, mutation_action("Reply",
            acc, mbx, id, 'Application("Mail").reply(m)'))
        table.insert(actions, mutation_action("Forward",
            acc, mbx, id, 'Application("Mail").forward(m)'))
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

-- JXA: same as inbox.lua, plus emit mailboxName so mutations can scope correctly.
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
        mailboxName: "INBOX",
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
            table.insert(items, format_triage_row(msg))
        end
        return { title = "Inbox", items = items }
    end,
})
