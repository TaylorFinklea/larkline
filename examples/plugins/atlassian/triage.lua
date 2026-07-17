-- Atlassian: Triage Queue — unassigned To-Do issues in the default project.
-- Set the `default_project_key` plugin setting to a project key (e.g. "PROJ").
-- Shared helpers copied from lib.lua.

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
                    { label = "Run `lark atlassian login`", kind = "shell", args = { "lark", "atlassian", "login" } },
                },
            },
        },
    }
end

local function lark_binary()
    local b = lark.env("LARK_BINARY")
    if b and b ~= "" then return b end
    return "lark"
end
local function trim(s) return (s or ""):gsub("%s+$", "") end

local function atlassian_auth(title)
    local email = lark.env("ATLASSIAN_EMAIL")
    local token = lark.env("ATLASSIAN_API_TOKEN")
    local host  = lark.store.get("atlassian_host")
    host = (type(host) == "string" and host ~= "") and host or nil
    if email and email ~= "" and token and token ~= "" and host then
        local site = "https://" .. host:gsub("^https?://", ""):gsub("/$", "")
        return { mode = "token", header = "Basic " .. lark.base64.encode(email .. ":" .. token),
                 jira_base = site, conf_base = site .. "/wiki", site_url = site }, nil
    end
    local bin = lark_binary()
    local tok = trim(lark.exec(bin, { "atlassian", "token" }))
    if tok == "" then return nil, not_signed_in_error(title) end
    local cid = trim(lark.exec(bin, { "atlassian", "cloudid" }))
    if cid == "" then return nil, not_signed_in_error(title) end
    local site = trim(lark.exec(bin, { "atlassian", "site" }))
    if site == "" then site = "https://api.atlassian.com/ex/jira/" .. cid end
    return { mode = "oauth", header = "Bearer " .. tok,
             jira_base = "https://api.atlassian.com/ex/jira/" .. cid,
             conf_base = "https://api.atlassian.com/ex/confluence/" .. cid,
             site_url = site }, nil
end

local function url_encode(s)
    if s == nil then return "" end
    return (tostring(s):gsub("([^%w%-_%.~])", function(c) return string.format("%%%02X", string.byte(c)) end))
end
local function build_query(params)
    if not params then return "" end
    local parts = {}
    for k, v in pairs(params) do
        if v ~= nil then parts[#parts + 1] = url_encode(k) .. "=" .. url_encode(v) end
    end
    return #parts == 0 and "" or "?" .. table.concat(parts, "&")
end
local function http_error_output(title, status, body)
    local msg, help_url
    if status == 401 then
        msg = "401 Unauthorized — token revoked or expired"
        help_url = "https://id.atlassian.com/manage-profile/security/api-tokens"
    elseif status == 403 then
        msg = "403 Forbidden — account lacks permission"
        help_url = "https://id.atlassian.com/manage-profile/security/api-tokens"
    elseif status == 404 then
        msg = "404 Not Found — check project key"
        help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/"
    elseif status >= 500 then
        msg = string.format("%d — Atlassian is having issues", status)
        help_url = "https://status.atlassian.com/"
    else
        msg = "HTTP " .. tostring(status)
        help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/"
    end
    local detail = body and body:gsub("\n", " "):sub(1, 160) or ""
    return { title = title, items = { error_item({
        label = "Atlassian API error",
        detail = msg .. (detail ~= "" and " · " .. detail or ""),
        help_url = help_url,
    }) } }
end
local function atlassian_get(auth, base, path, params, title)
    local url = base .. path .. build_query(params)
    local resp = lark.http.get(url, { headers = { Authorization = auth.header, Accept = "application/json" }, timeout = 15 })
    if not resp or resp.status == nil then
        return nil, { title = title, items = { error_item({
            label = "No response from Atlassian",
            detail = url,
            help_url = "https://status.atlassian.com/",
        }) } }
    end
    if resp.status < 200 or resp.status >= 300 then return nil, http_error_output(title, resp.status, resp.body) end
    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok then return nil, { title = title, items = { error_item({
        label = "Invalid JSON",
        help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/",
    }) } } end
    return data, nil
end

local function issue_icon(name)
    if not name then return "•" end
    local t = name:lower()
    if t == "bug" then return "🐛" end
    if t == "story" then return "📘" end
    if t == "task" then return "✓" end
    if t == "epic" then return "🗂" end
    return "•"
end
local function status_badge(s)
    if not s then return "?" end
    local cat = s.statusCategory and s.statusCategory.key or ""
    local n = s.name or "?"
    if cat == "done" then return "✔ " .. n end
    if cat == "indeterminate" then return "▶ " .. n end
    if cat == "new" then return "○ " .. n end
    return "● " .. n
end

-- SHARED: copies of helpers from lib.lua (sandbox has no require).
local PREVIEW_CAP = 5 * 1024
local function preview_truncate(s)
    if type(s) ~= "string" then return s end
    if #s <= PREVIEW_CAP then return s end
    return s:sub(1, PREVIEW_CAP) .. "\n\n…(truncated)"
end

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

local function preview_enabled()
    local v = lark.store.get("preview_full")
    if type(v) ~= "string" then return false end
    return v == "true" or v == "1"
end

lark.register({
    on_run = function()
        local auth, err = atlassian_auth("Triage Queue")
        if err then return err end

        local proj = lark.store.get("default_project_key")
        proj = (type(proj) == "string" and proj ~= "") and proj or nil
        if not proj then
            return { title = "Triage Queue", items = {
                { label = "Set the default_project_key setting", detail = "Open the plugin settings and enter your Jira project key (e.g. PROJ)", icon = "⚙" },
            } }
        end

        local jql = string.format(
            'project = "%s" AND statusCategory = "To Do" AND assignee IS EMPTY ORDER BY created DESC',
            proj:gsub('"', '\\"'))
        local fields = "summary,status,issuetype,priority,reporter,created"
        if preview_enabled() then fields = fields .. ",description" end
        local data, rerr = atlassian_get(auth, auth.jira_base, "/rest/api/3/search/jql",
            { jql = jql, fields = fields, maxResults = "100" },
            "Triage Queue")
        if rerr then return rerr end

        local items = {}
        for _, issue in ipairs(data.issues or {}) do
            local f = issue.fields or {}
            local url = (auth.site_url or auth.jira_base) .. "/browse/" .. issue.key
            local detail_parts = { status_badge(f.status) }
            if f.priority then detail_parts[#detail_parts + 1] = f.priority.name end
            if f.reporter then detail_parts[#detail_parts + 1] = "by " .. (f.reporter.displayName or "?") end
            local preview = nil
            if f.description then
                preview = adf_to_text(f.description)
                if preview == "" then preview = nil end
            end
            items[#items + 1] = {
                label = issue.key .. "  " .. (f.summary or ""),
                detail = table.concat(detail_parts, "  ·  "),
                icon = issue_icon(f.issuetype and f.issuetype.name),
                copy_text = issue.key,
                preview = preview,
                actions = {
                    { label = "Open in browser", kind = "open", args = { url } },
                    { label = "Copy key", kind = "clipboard", args = { issue.key } },
                },
            }
        end
        if #items == 0 then
            items[#items + 1] = { label = "Triage queue is clear", detail = proj, icon = "✅" }
        end
        return {
            title = string.format("Triage: %s — %d", proj, #items),
            items = items,
        }
    end,
})
