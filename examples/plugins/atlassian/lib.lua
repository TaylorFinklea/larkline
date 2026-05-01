-- Shared helpers for the Atlassian (Jira + Confluence) plugin.
-- The Lark sandbox has no require/dofile; this file is the canonical source.
-- Each command file copies the helpers it uses inline.
--
-- SYNC INSTRUCTIONS:
--   All command files use: atlassian_auth(), atlassian_get(), atlassian_post(),
--   issue_icon(), status_badge(), adf_to_plaintext(), adf_from_plaintext(),
--   issue_browser_url(), url_encode(), build_query(), error_item().

-- SHARED: error_item — canonical copy in examples/plugins/_shared/errors.lua.
local function error_item(opts)
    return {
        label = opts.label,
        detail = opts.detail,
        icon = opts.icon or "!",
        retry_action = opts.retry_action,
        help_url = opts.help_url,
    }
end

-- Error payload for the "no auth configured" state. Used by atlassian_auth()
-- when neither API-token env vars nor an OAuth refresh token are available.
local function not_signed_in_error(title)
    return {
        title = title or "Atlassian",
        items = {
            {
                label = "Not signed in to Atlassian",
                detail = "Run `lark atlassian login` for OAuth, or set ATLASSIAN_EMAIL + ATLASSIAN_API_TOKEN + atlassian_host for API-token auth",
                icon = "🔒",
                help_url = "https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/",
                actions = {
                    { label = "Run `lark atlassian login`", kind = "shell",
                      args = { "lark atlassian login" } },
                    { label = "Open Atlassian API tokens page", kind = "open",
                      args = { "https://id.atlassian.com/manage-profile/security/api-tokens" } },
                },
            },
        },
    }
end

-- Path to the running `lark` binary. In brew / cargo-install it's on PATH;
-- under `cargo run` dev we pick it up from the host-injected LARK_BINARY value.
local function lark_binary()
    local b = lark.env("LARK_BINARY")
    if b and b ~= "" then return b end
    return "lark"
end

local function trim(s) return (s or ""):gsub("%s+$", "") end

-- Resolve auth on each invocation. Returns (auth_table, err_output).
--   auth = {
--     mode       = "token" | "oauth",
--     header     = "Basic ..." | "Bearer ...",
--     jira_base  = "https://acme.atlassian.net"       or "https://api.atlassian.com/ex/jira/<cloudid>",
--     conf_base  = "https://acme.atlassian.net/wiki"  or "https://api.atlassian.com/ex/confluence/<cloudid>",
--     site_url   = "https://acme.atlassian.net",      -- always the human-facing URL (for browser links)
--   }
-- API-token auth wins when present — some users override on a per-session
-- basis to bypass a stale OAuth refresh.
local function atlassian_auth(title)
    local email = lark.env("ATLASSIAN_EMAIL")
    local token = lark.env("ATLASSIAN_API_TOKEN")
    local host  = lark.store.get("atlassian_host")
    host = (type(host) == "string" and host ~= "") and host or nil

    if email and email ~= "" and token and token ~= "" and host then
        local site = "https://" .. host:gsub("^https?://", ""):gsub("/$", "")
        return {
            mode      = "token",
            header    = "Basic " .. lark.base64.encode(email .. ":" .. token),
            jira_base = site,
            conf_base = site .. "/wiki",
            site_url  = site,
        }, nil
    end

    -- OAuth path: call back into the running binary to get a fresh access token.
    local bin = lark_binary()
    local tok = trim(lark.exec(bin, { "atlassian", "token" }))
    if tok == "" then return nil, not_signed_in_error(title) end
    local cid = trim(lark.exec(bin, { "atlassian", "cloudid" }))
    if cid == "" then return nil, not_signed_in_error(title) end
    local site = trim(lark.exec(bin, { "atlassian", "site" }))
    if site == "" then site = "https://api.atlassian.com/ex/jira/" .. cid end

    return {
        mode      = "oauth",
        header    = "Bearer " .. tok,
        jira_base = "https://api.atlassian.com/ex/jira/" .. cid,
        conf_base = "https://api.atlassian.com/ex/confluence/" .. cid,
        site_url  = site,
    }, nil
end

-- Minimal URL percent-encoder. Handles the JQL/CQL special chars plus spaces.
local function url_encode(s)
    if s == nil then return "" end
    s = tostring(s)
    return (s:gsub("([^%w%-_%.~])", function(c)
        return string.format("%%%02X", string.byte(c))
    end))
end

-- Turn {k1 = "v1", k2 = "v2"} into "?k1=v1&k2=v2" (or "" if empty).
local function build_query(params)
    if not params then return "" end
    local parts = {}
    for k, v in pairs(params) do
        if v ~= nil then
            parts[#parts + 1] = url_encode(k) .. "=" .. url_encode(v)
        end
    end
    if #parts == 0 then return "" end
    return "?" .. table.concat(parts, "&")
end

-- Distinguish HTTP error categories so users see the real problem instead of
-- a generic "API error".
local function http_error_output(title, status, body)
    local msg, help_url
    if status == 401 then
        msg = "401 Unauthorized — token revoked or expired"
        help_url = "https://id.atlassian.com/manage-profile/security/api-tokens"
    elseif status == 403 then
        msg = "403 Forbidden — account lacks permission"
        help_url = "https://id.atlassian.com/manage-profile/security/api-tokens"
    elseif status == 404 then
        msg = "404 Not Found — check host / project / key"
        help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/"
    elseif status >= 500 then
        msg = string.format("%d — Atlassian is having issues", status)
        help_url = "https://status.atlassian.com/"
    else
        msg = "HTTP " .. tostring(status)
        help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/"
    end
    local detail = body and (body:gsub("\n", " "):sub(1, 160)) or ""
    return {
        title = title,
        items = { error_item({
            label = "Atlassian API error",
            detail = msg .. (detail ~= "" and " · " .. detail or ""),
            help_url = help_url,
        }) },
    }
end

-- GET request. Returns (decoded_json_table, nil) on success or (nil, err_output) on failure.
local function atlassian_get(auth, base, path, params, title)
    local url = base .. path .. build_query(params)
    local resp = lark.http.get(url, {
        headers = {
            Authorization = auth.header,
            Accept = "application/json",
        },
        timeout = 15,
    })
    if not resp or resp.status == nil then
        return nil, { title = title, items = { error_item({
            label = "No response from Atlassian",
            detail = url,
            help_url = "https://status.atlassian.com/",
        }) } }
    end
    if resp.status < 200 or resp.status >= 300 then
        return nil, http_error_output(title, resp.status, resp.body)
    end
    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok then
        return nil, { title = title, items = { error_item({
            label = "Invalid JSON from Atlassian",
            detail = resp.body:sub(1, 120),
            help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/",
        }) } }
    end
    return data, nil
end

-- POST with JSON body. Returns (decoded_json_table, nil) or (nil, err_output).
local function atlassian_post(auth, base, path, body_table, title)
    local url = base .. path
    local body = lark.json.encode(body_table or {})
    local resp = lark.http.post(url, body, {
        headers = {
            Authorization = auth.header,
            Accept = "application/json",
            ["Content-Type"] = "application/json",
        },
        timeout = 20,
    })
    if not resp or resp.status == nil then
        return nil, { title = title, items = { error_item({
            label = "No response from Atlassian",
            detail = url,
            help_url = "https://status.atlassian.com/",
        }) } }
    end
    if resp.status < 200 or resp.status >= 300 then
        return nil, http_error_output(title, resp.status, resp.body)
    end
    if resp.body == "" then return {}, nil end
    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok then
        return nil, { title = title, items = { error_item({
            label = "Invalid JSON from Atlassian",
            detail = resp.body:sub(1, 120),
            help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/",
        }) } }
    end
    return data, nil
end

-- Nerd Font glyph per Jira issue type name.
local function issue_icon(type_name)
    if not type_name then return "•" end
    local t = type_name:lower()
    if t == "bug" then return "🐛" end
    if t == "story" then return "📘" end
    if t == "task" then return "✓" end
    if t == "epic" then return "🗂" end
    if t == "sub-task" or t == "subtask" then return "└" end
    if t == "improvement" then return "↑" end
    if t == "new feature" then return "✨" end
    return "•"
end

-- Human-friendly status badge. Atlassian groups statuses into categories:
-- "new" (To Do), "indeterminate" (In Progress), "done" (Done).
local function status_badge(status)
    if not status then return "?" end
    local cat = status.statusCategory and status.statusCategory.key or ""
    local name = status.name or "?"
    if cat == "done" then return "✔ " .. name end
    if cat == "indeterminate" then return "▶ " .. name end
    if cat == "new" then return "○ " .. name end
    return "● " .. name
end

-- Recursively reduce Atlassian Document Format (ADF) to plain text. Handles
-- the node types we see in issue descriptions + comments. Unknown nodes render
-- "[unsupported: <type>]" so we never silently drop content.
local function adf_to_plaintext(node, out)
    out = out or {}
    if type(node) ~= "table" then return table.concat(out) end
    local nt = node.type
    if nt == "doc" or nt == "paragraph" or nt == "bulletList" or nt == "orderedList" or nt == "listItem" then
        if node.content then
            for _, c in ipairs(node.content) do adf_to_plaintext(c, out) end
        end
        if nt == "paragraph" then out[#out + 1] = "\n" end
        if nt == "listItem" then out[#out + 1] = "\n" end
    elseif nt == "heading" then
        local level = (node.attrs and node.attrs.level) or 1
        out[#out + 1] = string.rep("#", level) .. " "
        if node.content then
            for _, c in ipairs(node.content) do adf_to_plaintext(c, out) end
        end
        out[#out + 1] = "\n"
    elseif nt == "hardBreak" then
        out[#out + 1] = "\n"
    elseif nt == "text" then
        out[#out + 1] = node.text or ""
    elseif nt == "codeBlock" then
        out[#out + 1] = "\n```\n"
        if node.content then
            for _, c in ipairs(node.content) do adf_to_plaintext(c, out) end
        end
        out[#out + 1] = "\n```\n"
    elseif nt == "inlineCard" or nt == "mention" then
        out[#out + 1] = (node.attrs and (node.attrs.url or node.attrs.text or node.attrs.id)) or ""
    elseif nt == "rule" then
        out[#out + 1] = "\n---\n"
    elseif nt then
        out[#out + 1] = "[unsupported: " .. nt .. "]"
    end
    return table.concat(out)
end

-- SHARED: preview_truncate — cap preview strings at ~5KB to keep JSON payload
-- small. Truncation point: just before the cap; doesn't try to align to
-- paragraph boundaries (read-mode acceptable).
local PREVIEW_CAP = 5 * 1024
local function preview_truncate(s)
    if type(s) ~= "string" then return s end
    if #s <= PREVIEW_CAP then return s end
    return s:sub(1, PREVIEW_CAP) .. "\n\n…(truncated)"
end

-- SHARED: adf_to_text — minimal ADF→text reducer used by preview rendering.
-- Mirrors the inline flatten() helper used in my-issues.lua's detail view.
-- Returns "" for nil/non-table input. Output is line-broken plaintext suitable
-- for the lark.nvim Telescope preview pane (markdown filetype tolerates plain).
local function adf_to_text(node)
    if type(node) ~= "table" then return "" end
    local out = {}
    local function flatten(n)
        if type(n) ~= "table" then return end
        if n.type == "text" then out[#out + 1] = n.text or "" end
        if n.content then
            for _, c in ipairs(n.content) do flatten(c) end
        end
        if n.type == "paragraph" or n.type == "heading" or n.type == "listItem" then
            out[#out + 1] = "\n"
        end
    end
    flatten(node)
    return preview_truncate(table.concat(out, ""))
end

-- SHARED: storage_to_text — best-effort Confluence storage-format reducer for
-- preview rendering. Strips XML-ish tags (including <ac:*>/<ri:*> macros and
-- self-closing structural tags), decodes the most common HTML entities, and
-- collapses whitespace. Macro-heavy pages may show residual placeholders;
-- prose-heavy pages render cleanly enough for read-mode previews. Document
-- the "best effort" caveat in user docs.
local function storage_to_text(html)
    if type(html) ~= "string" or html == "" then return "" end
    -- Drop entire CDATA blocks unchanged (preserves code fences within macros).
    local s = html:gsub("<!%[CDATA%[(.-)%]%]>", "%1")
    -- Drop block-level tags but keep a newline so paragraphs break.
    s = s:gsub("</p>", "\n"):gsub("</li>", "\n"):gsub("<br%s*/?>", "\n")
    s = s:gsub("</h%d>", "\n\n")
    -- Strip remaining tags (XHTML, including macros).
    s = s:gsub("<[^>]+>", "")
    -- Decode the most common entities.
    s = s:gsub("&amp;", "&")
        :gsub("&lt;", "<")
        :gsub("&gt;", ">")
        :gsub("&quot;", '"')
        :gsub("&#39;", "'")
        :gsub("&nbsp;", " ")
    -- Collapse 3+ blank lines down to 2.
    s = s:gsub("\n\n\n+", "\n\n")
    -- Trim leading whitespace.
    s = s:gsub("^%s+", "")
    return preview_truncate(s)
end

-- SHARED: preview_enabled — read the `preview_full` plugin setting. The store
-- returns strings; treat "true"/"1" as truthy (matches the manifest toggle's
-- "true"/"false" string form).
local function preview_enabled()
    local v = lark.store.get("preview_full")
    if type(v) ~= "string" then return false end
    return v == "true" or v == "1"
end

-- Wrap plain text in a minimal ADF doc for POSTing to Jira (comments + create).
-- Splits on \n\n into paragraphs; inner \n becomes hardBreak.
local function adf_from_plaintext(text)
    text = text or ""
    local paras = {}
    for para in (text .. "\n\n"):gmatch("(.-)\n\n") do
        local content = {}
        local first_line = true
        for line in (para .. "\n"):gmatch("([^\n]*)\n") do
            if not first_line then content[#content + 1] = { type = "hardBreak" } end
            if line ~= "" then content[#content + 1] = { type = "text", text = line } end
            first_line = false
        end
        if #content > 0 then
            paras[#paras + 1] = { type = "paragraph", content = content }
        end
    end
    if #paras == 0 then
        paras = { { type = "paragraph", content = { { type = "text", text = text } } } }
    end
    return { version = 1, type = "doc", content = paras }
end

-- Browser-facing URL for a Jira issue key. Uses `site_url` from auth (works
-- uniformly for both token and OAuth modes).
local function issue_browser_url(auth, key)
    return (auth.site_url or auth.jira_base) .. "/browse/" .. key
end

-- Confluence page URL from a content object returned by the REST API.
local function page_browser_url(auth, page)
    local base = auth.site_url or (auth.conf_base:gsub("/wiki$", ""))
    if page._links and page._links.webui then
        return base .. "/wiki" .. page._links.webui
    end
    return base .. "/wiki/spaces/" .. (page.space and page.space.key or "~") .. "/pages/" .. (page.id or "")
end
