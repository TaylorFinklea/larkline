-- Shared helpers for the Atlassian (Jira + Confluence) plugin.
-- The Lark sandbox has no require/dofile; this file is the canonical source.
-- Each command file copies the helpers it uses inline.
--
-- SYNC INSTRUCTIONS:
--   All command files use: atlassian_auth(), atlassian_get(), atlassian_post(),
--   issue_icon(), status_badge(), adf_to_plaintext(), adf_from_plaintext(),
--   issue_browser_url(), url_encode(), build_query().

-- Resolve auth on each invocation. Returns (auth_table, err_output).
--   auth = {
--     mode       = "token" | "oauth",
--     header     = "Basic ..." | "Bearer ...",
--     jira_base  = "https://acme.atlassian.net" or "https://api.atlassian.com/ex/jira/<cloudid>",
--     conf_base  = "https://acme.atlassian.net/wiki" or "https://api.atlassian.com/ex/confluence/<cloudid>/wiki",
--     site_url   = "https://acme.atlassian.net",          -- for browser links (issue URLs, page URLs)
--   }
-- Phase A only implements the token branch. The OAuth branch returns a "run
-- `lark atlassian login`" error with a chain action; Phase C replaces it.
local function atlassian_auth()
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

    -- OAuth stub — Phase C replaces with real `lark atlassian token` dispatch.
    return nil, {
        title = "Atlassian",
        items = {
            {
                label = "Not signed in to Atlassian",
                detail = "Set ATLASSIAN_EMAIL + ATLASSIAN_API_TOKEN + atlassian_host, or run `lark atlassian login` (v0.12.0-B)",
                icon = "🔒",
                actions = {
                    { label = "Open sign-in docs", kind = "open", args = { "https://id.atlassian.com/manage-profile/security/api-tokens" } },
                },
            },
        },
    }
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
    local msg
    if status == 401 then msg = "401 Unauthorized — token revoked or expired"
    elseif status == 403 then msg = "403 Forbidden — account lacks permission"
    elseif status == 404 then msg = "404 Not Found — check host / project / key"
    elseif status >= 500 then msg = string.format("%d — Atlassian is having issues", status)
    else msg = "HTTP " .. tostring(status) end
    local detail = body and (body:gsub("\n", " "):sub(1, 160)) or ""
    return {
        title = title,
        items = { { label = "Atlassian API error", detail = msg .. (detail ~= "" and " · " .. detail or ""), icon = "!" } },
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
        return nil, { title = title, items = { { label = "No response from Atlassian", detail = url, icon = "!" } } }
    end
    if resp.status < 200 or resp.status >= 300 then
        return nil, http_error_output(title, resp.status, resp.body)
    end
    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok then
        return nil, { title = title, items = { { label = "Invalid JSON from Atlassian", detail = resp.body:sub(1, 120), icon = "!" } } }
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
        return nil, { title = title, items = { { label = "No response from Atlassian", detail = url, icon = "!" } } }
    end
    if resp.status < 200 or resp.status >= 300 then
        return nil, http_error_output(title, resp.status, resp.body)
    end
    if resp.body == "" then return {}, nil end
    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok then
        return nil, { title = title, items = { { label = "Invalid JSON from Atlassian", detail = resp.body:sub(1, 120), icon = "!" } } }
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
