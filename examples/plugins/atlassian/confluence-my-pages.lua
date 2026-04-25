-- Atlassian: My Pages — Confluence pages you authored, newest first.
-- Uses CQL `creator = currentUser() ORDER BY lastmodified DESC`.
-- Shared helpers copied from lib.lua.

local function not_signed_in_error(title)
    return { title = title or "Atlassian", items = {
        { label = "Not signed in to Atlassian",
          detail = "Run `lark atlassian login` for OAuth, or set ATLASSIAN_EMAIL + ATLASSIAN_API_TOKEN + atlassian_host for API-token auth",
          icon = "🔒",
          actions = { { label = "Run `lark atlassian login`", kind = "shell", args = { "lark atlassian login" } } },
        },
    } }
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
             conf_base = "https://api.atlassian.com/ex/confluence/" .. cid .. "/wiki",
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
    local msg = (status == 401) and "401 Unauthorized — token revoked or expired"
        or (status == 403) and "403 Forbidden"
        or (status >= 500) and string.format("%d — Atlassian is having issues", status)
        or "HTTP " .. tostring(status)
    local detail = body and body:gsub("\n", " "):sub(1, 160) or ""
    return { title = title, items = { { label = "Atlassian API error",
        detail = msg .. (detail ~= "" and " · " .. detail or ""), icon = "!" } } }
end
local function atlassian_get(auth, base, path, params, title)
    local url = base .. path .. build_query(params)
    local resp = lark.http.get(url, { headers = { Authorization = auth.header, Accept = "application/json" }, timeout = 15 })
    if not resp or resp.status == nil then
        return nil, { title = title, items = { { label = "No response from Atlassian", icon = "!" } } }
    end
    if resp.status < 200 or resp.status >= 300 then return nil, http_error_output(title, resp.status, resp.body) end
    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok then return nil, { title = title, items = { { label = "Invalid JSON", icon = "!" } } } end
    return data, nil
end

local function result_to_row(auth, r)
    local site = auth.site_url or auth.conf_base:gsub("/wiki$", "")
    local content = r.content or {}
    local url
    if r.url then
        url = site .. (r.url:sub(1, 1) == "/" and r.url or ("/wiki/" .. r.url))
    elseif content._links and content._links.webui then
        url = site .. "/wiki" .. content._links.webui
    else
        url = site
    end
    local title = (r.title or content.title or ""):gsub("@@@hl@@@", ""):gsub("@@@endhl@@@", "")
    local space = (r.resultGlobalContainer and r.resultGlobalContainer.title)
        or (content.space and content.space.name) or "?"
    local last = r.lastModified and r.lastModified:sub(1, 10) or nil
    local detail_parts = { space }
    if last then detail_parts[#detail_parts + 1] = last end
    return {
        label = title,
        detail = table.concat(detail_parts, "  ·  "),
        icon = (content.type == "blogpost") and "📰" or "📄",
        copy_text = title,
        actions = {
            { label = "Open in browser", kind = "open", args = { url } },
            { label = "Copy URL", kind = "clipboard", args = { url } },
            { label = "Copy title", kind = "clipboard", args = { title } },
        },
    }
end

lark.register({
    on_run = function()
        local auth, err = atlassian_auth("My Pages")
        if err then return err end

        local cql = "creator = currentUser() AND type = page ORDER BY lastmodified DESC"
        local data, rerr = atlassian_get(auth, auth.conf_base, "/rest/api/search",
            { cql = cql, limit = "50" }, "My Pages")
        if rerr then return rerr end

        local items = {}
        for _, r in ipairs(data.results or {}) do
            items[#items + 1] = result_to_row(auth, r)
        end
        if #items == 0 then
            items[#items + 1] = { label = "You haven't authored any pages yet", icon = "📭" }
        end
        return {
            title = string.format("My Pages — %d", #items),
            items = items,
        }
    end,
})
