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
--
-- Perf note: the listing JXA deliberately does NOT call m.content() for
-- every message -- that's the slowest single op in Mail.app scripting
-- and pulling it for 30 messages routinely blew past the 30s plugin
-- timeout on real-world inboxes. Body is fetched on demand by the
-- "View body" chain action via a single-message lookup.

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
    table.insert(lines, "_Press Space → View body for full message text._")
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
    -- "View body" is a chain action: it pushes a fullscreen markdown
    -- render of the message body on top of the current view. Esc/Back
    -- pops back to the inbox. The body is fetched on demand in
    -- on_action via a single-message JXA lookup so the inbox listing
    -- itself stays fast (see perf note at the top of this file).
    --
    -- The engine joins args[1..] with spaces into a single context
    -- string before handing it to on_action, so we pack the lookup
    -- keys + display fields as one JSON-encoded arg.
    if id ~= "" and acc ~= "" then
        local ctx_json, _ = lark.json.encode({
            account = acc,
            mailbox = mbx,
            id = id,
            subject = msg.subject or "",
            sender = msg.sender or "",
        })
        table.insert(actions, {
            label = "View body",
            kind = "chain",
            args = { "view_body", ctx_json or "{}" },
        })
        -- "View images" runs whether or not the message has images; the
        -- chain handler reports "(no images)" if there are none. We can't
        -- cheaply check for attachments without fetching the source, and
        -- doing that here would re-introduce the per-row perf cost we just
        -- removed from the listing JXA.
        table.insert(actions, {
            label = "View images",
            kind = "chain",
            args = { "view_images", ctx_json or "{}" },
        })
    end
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

-- JXA listing: metadata only, no m.content() -- see perf note at top of file.
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

    -- Chain callbacks below drill into a single message:
    --   view_body   -> fullscreen markdown render of the body, using the
    --                  best available HTML renderer (pandoc/w3m/lynx)
    --   view_images -> fullscreen ANSI render of every image attachment
    --                  via chafa, splitting them out so markdown body
    --                  rendering stays untouched
    --
    -- Both pull the raw RFC822 source on demand via m.source() so the
    -- inbox listing JXA can stay metadata-only. Each chain context is the
    -- same JSON blob format_triage_row produced.
    on_action = function(callback_id, context)
        local ok, data = pcall(lark.json.decode, context or "{}")
        if not ok or type(data) ~= "table" then data = {} end
        local subject = (data.subject ~= nil and data.subject ~= "") and data.subject or "(no subject)"
        local sender = data.sender or ""
        local id = data.id or ""
        local acc = data.account or ""
        local mbx = data.mailbox or "INBOX"

        -- Fetch the RFC822 source for the single message keyed by
        -- (account, mailbox, messageId). Empty string on any failure;
        -- callbacks below handle the fallback path.
        local function fetch_source()
            if id == "" or acc == "" then return "" end
            local script = string.format(
                'const m=Application("Mail").accounts.byName(%s).mailboxes.byName(%s).messages.whose({messageId:%s})()[0];m?(m.source()||""):"";',
                js_str(acc), js_str(mbx), js_str(id)
            )
            local r = lark.exec_io("osascript", { "-l", "JavaScript" }, { stdin = script })
            if r.exit_code ~= 0 then return "" end
            return r.stdout or ""
        end

        -- which(cmd) -> true if the binary is on $PATH.
        local function which(cmd)
            local r = lark.exec_io("which", { cmd })
            return r.exit_code == 0
        end

        if callback_id == "view_body" then
            local source = fetch_source()
            local body_md, renderer = nil, "fallback"
            local helper = lark.plugin_dir .. "/mail_render.py"

            if source ~= "" then
                -- Step 1: extract HTML body via the Python MIME helper.
                local rh = lark.exec_io("python3",
                    { helper, "--extract-html" },
                    { stdin = source })
                local html = (rh.exit_code == 0) and (rh.stdout or "") or ""

                -- Step 2: convert HTML -> displayable text via the best
                -- available renderer. Order is deliberate:
                --   1. w3m is purpose-built for terminal HTML and
                --      gracefully un-nests the layout tables that
                --      marketing emails are made of. Cleanest by far.
                --   2. pandoc -t plain handles correspondence well but
                --      faithfully renders every nested layout table for
                --      marketing emails -- unreadable in a terminal.
                --   3. lynx -dump is the lynx-flavored fallback.
                -- In all cases we wrap the output in a markdown code
                -- fence so lark's themed renderer preserves spacing.
                if html ~= "" and which("w3m") then
                    local r2 = lark.exec_io("w3m",
                        { "-dump", "-T", "text/html", "-cols", "100" },
                        { stdin = html })
                    if r2.exit_code == 0 and r2.stdout and r2.stdout ~= "" then
                        body_md = "```\n" .. r2.stdout .. "\n```"
                        renderer = "w3m"
                    end
                end
                if not body_md and html ~= "" and which("pandoc") then
                    local r = lark.exec_io("pandoc",
                        { "-f", "html", "-t", "plain",
                          "--reference-links", "--wrap=preserve",
                          "--columns=100" },
                        { stdin = html })
                    if r.exit_code == 0 and r.stdout and r.stdout ~= "" then
                        body_md, renderer = "```\n" .. r.stdout .. "\n```", "pandoc(html)"
                    end
                end
                if not body_md and html ~= "" and which("lynx") then
                    local r3 = lark.exec_io("lynx",
                        { "-dump", "-stdin", "-force_html", "-width=100" },
                        { stdin = html })
                    if r3.exit_code == 0 and r3.stdout and r3.stdout ~= "" then
                        body_md = "```\n" .. r3.stdout .. "\n```"
                        renderer = "lynx"
                    end
                end

                -- Step 3: fall back to the text/plain alternative.
                if not body_md then
                    local r4 = lark.exec_io("python3",
                        { helper, "--extract-text" },
                        { stdin = source })
                    if r4.exit_code == 0 and r4.stdout and r4.stdout ~= "" then
                        body_md = r4.stdout
                        renderer = "text/plain"
                    end
                end
            end

            -- 4. Absolute fallback: Mail.app's m.content() (the noisy
            -- plain-text-stripped-of-HTML the prior version always used).
            if not body_md then
                local script = string.format(
                    'const m=Application("Mail").accounts.byName(%s).mailboxes.byName(%s).messages.whose({messageId:%s})()[0];m?(m.content()||""):"";',
                    js_str(acc), js_str(mbx), js_str(id)
                )
                local r = lark.exec_io("osascript", { "-l", "JavaScript" }, { stdin = script })
                if r.exit_code == 0 then
                    body_md = (r.stdout or ""):gsub("%s+$", "")
                    renderer = "mail.content()"
                end
            end

            -- Strip invisible padding characters that marketing emails use
            -- as inbox-preview "preheader" filler. These bytes survive the
            -- HTML->markdown conversion and clutter the body output.
            -- U+00AD SOFT HYPHEN (\xC2\xAD), U+034F COMBINING GRAPHEME
            -- JOINER (\xCD\x8F), U+200B..U+200F zero-width markers
            -- (\xE2\x80\x8B..\x8F), U+2060 WORD JOINER (\xE2\x81\xA0),
            -- U+2800 BRAILLE BLANK (\xE2\xA0\x80), U+FEFF BOM
            -- (\xEF\xBB\xBF).
            body_md = (body_md or "")
                :gsub("\r\n", "\n")
                :gsub("\xC2\xAD", "")
                :gsub("\xCD\x8F", "")
                :gsub("\xE2\x80[\x8B-\x8F]", "")
                :gsub("\xE2\x81\xA0", "")
                :gsub("\xE2\xA0\x80", "")
                :gsub("\xEF\xBB\xBF", "")
                :gsub("[ \t]+\n", "\n")
                :gsub("\n\n\n+", "\n\n")

            local md = "# " .. subject .. "\n\n"
            if sender ~= "" then md = md .. "**From:** " .. sender .. "\n\n" end
            md = md .. "---\n\n"
            if body_md ~= "" then
                md = md .. body_md
            else
                md = md .. "*(empty body)*"
            end
            md = md .. "\n\n---\n_rendered via " .. renderer .. "_"

            return {
                title = subject,
                raw_text = md,
                output_format = "markdown",
            }
        end

        if callback_id == "view_images" then
            local source = fetch_source()
            if source == "" then
                return {
                    title = subject,
                    raw_text = "# " .. subject .. "\n\n*(could not fetch message source)*",
                    output_format = "markdown",
                }
            end

            -- Save images to a per-message scratch dir under XDG_CACHE.
            local cache_root = lark.env("XDG_CACHE_HOME")
                or ((lark.env("HOME") or "/tmp") .. "/.cache")
            local tmpdir = cache_root .. "/larkline/mail-images/" .. id:gsub("[^%w]", "_")
            local helper = lark.plugin_dir .. "/mail_render.py"
            local r = lark.exec_io("python3",
                { helper, "--save-images", tmpdir },
                { stdin = source })
            if r.exit_code ~= 0 then
                return {
                    title = subject,
                    raw_text = "# " .. subject .. "\n\n*(image extraction failed: "
                        .. ((r.stderr or ""):gsub("\n", " ")) .. ")*",
                    output_format = "markdown",
                }
            end

            local ok_dec, images = pcall(lark.json.decode, r.stdout or "[]")
            if not ok_dec or type(images) ~= "table" or #images == 0 then
                return {
                    title = subject,
                    raw_text = "# " .. subject .. "\n\n*(no images in this message)*",
                    output_format = "markdown",
                }
            end

            -- Render each image to ANSI symbol blocks via chafa. ANSI
            -- output flows through ansi_to_tui when we return raw_text
            -- WITHOUT output_format = markdown (the markdown renderer
            -- eats escape sequences).
            if not which("chafa") then
                local lines = { "# " .. subject, "", "_chafa not installed -- run `brew install chafa` to render images inline._", "" }
                for _, img in ipairs(images) do
                    table.insert(lines, "- " .. img.filename .. " (" .. img.mime .. ", " .. tostring(img.size) .. " bytes) -> " .. img.path)
                end
                return {
                    title = subject,
                    raw_text = table.concat(lines, "\n"),
                    output_format = "markdown",
                }
            end

            local out = { "Images in: " .. subject, "" }
            for i, img in ipairs(images) do
                table.insert(out, string.format("[%d/%d] %s (%s)",
                    i, #images, img.filename, img.mime))
                local rr = lark.exec_io("chafa",
                    { "--format=symbols", "--size=80x24", "--animate=off", img.path })
                if rr.exit_code == 0 then
                    table.insert(out, rr.stdout or "")
                else
                    table.insert(out, "  (chafa failed: " .. ((rr.stderr or ""):gsub("\n", " ")) .. ")")
                end
                table.insert(out, "")
            end
            return {
                title = "Images: " .. subject,
                -- Plain raw_text (no output_format) so ANSI escapes from
                -- chafa render through ansi_to_tui.
                raw_text = table.concat(out, "\n"),
            }
        end

        return nil
    end,
})
