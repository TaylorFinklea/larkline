-- Atlassian: Search Confluence — full-text search via CQL.
-- Step 1: form prompts for a query string.
-- Step 2: POST to /rest/api/search with `cql=text~"<q>"`.
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
        or (status == 400) and "400 Bad Request — check CQL syntax"
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

-- SHARED: copies of helpers from lib.lua (sandbox has no require).
local PREVIEW_CAP = 5 * 1024
local function preview_truncate(s)
    if type(s) ~= "string" then return s end
    if #s <= PREVIEW_CAP then return s end
    return s:sub(1, PREVIEW_CAP) .. "\n\n…(truncated)"
end

local function storage_to_text(html)
    if type(html) ~= "string" or html == "" then return "" end
    local s = html:gsub("<!%[CDATA%[(.-)%]%]>", "%1")
    s = s:gsub("</p>", "\n"):gsub("</li>", "\n"):gsub("<br%s*/?>", "\n")
    s = s:gsub("</h%d>", "\n\n")
    s = s:gsub("<[^>]+>", "")
    s = s:gsub("&amp;", "&")
        :gsub("&lt;", "<")
        :gsub("&gt;", ">")
        :gsub("&quot;", '"')
        :gsub("&#39;", "'")
        :gsub("&nbsp;", " ")
    s = s:gsub("\n\n\n+", "\n\n")
    s = s:gsub("^%s+", "")
    return preview_truncate(s)
end

local function preview_enabled()
    local v = lark.store.get("preview_full")
    if type(v) ~= "string" then return false end
    return v == "true" or v == "1"
end

-- /rest/api/search returns a flat 'results' list; each entry has either
-- `content` (page/blogpost) or `space` (space-only result). We keep just
-- content matches so the row layout stays uniform.
local function search_result_to_row(auth, result)
    local content = result.content or {}
    local site = auth.site_url or auth.conf_base:gsub("/wiki$", "")
    local url
    if result.url then
        url = site .. (result.url:sub(1, 1) == "/" and result.url or ("/wiki/" .. result.url))
    elseif content._links and content._links.webui then
        url = site .. "/wiki" .. content._links.webui
    else
        url = site
    end
    local title = (result.title or content.title or ""):gsub("@@@hl@@@", ""):gsub("@@@endhl@@@", "")
    local excerpt = (result.excerpt or ""):gsub("@@@hl@@@", ""):gsub("@@@endhl@@@", ""):gsub("\n", " ")
    if #excerpt > 100 then excerpt = excerpt:sub(1, 100) .. "…" end
    -- Preview for Telescope (lark.nvim v0.14.0). When preview_full is on, the
    -- expand query asks for content.body.storage on each result.
    local preview = nil
    if content.body and content.body.storage and content.body.storage.value then
        preview = storage_to_text(content.body.storage.value)
        if preview == "" then preview = nil end
    end
    return {
        label = title,
        detail = excerpt,
        icon = (content.type == "blogpost") and "📰" or "📄",
        copy_text = title,
        preview = preview,
        actions = {
            { label = "Open in browser", kind = "open", args = { url } },
            { label = "Copy URL", kind = "clipboard", args = { url } },
        },
    }
end

lark.register({
    on_run = function()
        local auth, err = atlassian_auth("Search Confluence")
        if err then return err end

        local q = lark.form_values and trim(lark.form_values.q or "") or ""
        if q == "" then
            return {
                title = "Search Confluence",
                form = {
                    fields = {
                        { id = "q", label = "Query", type = { kind = "text" },
                          required = true, placeholder = "Free text — searches page bodies + titles" },
                    },
                    submit_label = "Search",
                },
            }
        end

        -- Escape backslashes + quotes for safe inclusion in the CQL string literal.
        local cql = string.format('text ~ "%s"', q:gsub('\\', '\\\\'):gsub('"', '\\"'))
        local search_params = { cql = cql, limit = "25" }
        if preview_enabled() then search_params.expand = "content.body.storage" end
        local data, rerr = atlassian_get(auth, auth.conf_base, "/rest/api/search",
            search_params, "Search Confluence")
        if rerr then return rerr end

        local items = {}
        for _, r in ipairs(data.results or {}) do
            if r.content then items[#items + 1] = search_result_to_row(auth, r) end
        end
        if #items == 0 then
            items[#items + 1] = { label = "No matches", detail = q, icon = "📭" }
        end
        return {
            title = string.format("Search: %q — %d", q, #items),
            items = items,
        }
    end,
})
