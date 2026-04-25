-- Atlassian: Comment on Issue — POST a comment to a Jira issue.
-- Two-field form: issue key + body. Body wrapped in ADF before POST.
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
             conf_base = "https://api.atlassian.com/ex/confluence/" .. cid,
             site_url = site }, nil
end

local function http_error_output(title, status, body)
    local msg = (status == 401) and "401 Unauthorized — token revoked or expired"
        or (status == 403) and "403 Forbidden — account lacks permission"
        or (status == 404) and "404 Not Found — issue key not found"
        or (status >= 500) and string.format("%d — Atlassian is having issues", status)
        or "HTTP " .. tostring(status)
    local detail = body and body:gsub("\n", " "):sub(1, 160) or ""
    return { title = title, items = { { label = "Atlassian API error",
        detail = msg .. (detail ~= "" and " · " .. detail or ""), icon = "!" } } }
end
local function atlassian_post(auth, base, path, body_table, title)
    local resp = lark.http.post(base .. path, lark.json.encode(body_table or {}), {
        headers = { Authorization = auth.header, Accept = "application/json", ["Content-Type"] = "application/json" },
        timeout = 20,
    })
    if not resp or resp.status == nil then
        return nil, { title = title, items = { { label = "No response from Atlassian", icon = "!" } } }
    end
    if resp.status < 200 or resp.status >= 300 then return nil, http_error_output(title, resp.status, resp.body) end
    return {}, nil
end

local function adf_from_plaintext(text)
    text = text or ""
    local paras = {}
    for para in (text .. "\n\n"):gmatch("(.-)\n\n") do
        local content = {}
        local first = true
        for line in (para .. "\n"):gmatch("([^\n]*)\n") do
            if not first then content[#content + 1] = { type = "hardBreak" } end
            if line ~= "" then content[#content + 1] = { type = "text", text = line } end
            first = false
        end
        if #content > 0 then paras[#paras + 1] = { type = "paragraph", content = content } end
    end
    if #paras == 0 then
        paras = { { type = "paragraph", content = { { type = "text", text = text } } } }
    end
    return { version = 1, type = "doc", content = paras }
end

lark.register({
    on_run = function()
        local auth, err = atlassian_auth("Comment on Issue")
        if err then return err end

        if lark.form_values then
            local key  = trim(lark.form_values.issue_key or "")
            local body = lark.form_values.body or ""
            if key == "" or trim(body) == "" then
                return { title = "Comment on Issue", items = {
                    { label = "Issue key and comment body are required", icon = "!" },
                } }
            end
            local _, perr = atlassian_post(auth, auth.jira_base,
                "/rest/api/3/issue/" .. key .. "/comment",
                { body = adf_from_plaintext(body) }, "Comment on " .. key)
            if perr then return perr end

            local url = (auth.site_url or auth.jira_base) .. "/browse/" .. key
            return {
                title = "Comment posted",
                items = {
                    {
                        label = "Comment added to " .. key,
                        detail = (#body <= 60) and body or (body:sub(1, 60) .. "…"),
                        icon = "💬",
                        actions = {
                            { label = "Open in browser", kind = "open", args = { url } },
                            { label = "Copy key", kind = "clipboard", args = { key } },
                        },
                    },
                },
            }
        end

        return {
            title = "Comment on Issue",
            form = {
                fields = {
                    { id = "issue_key", label = "Issue key", type = { kind = "text" },
                      required = true, placeholder = "e.g. PROJ-123" },
                    { id = "body", label = "Comment", type = { kind = "text" },
                      required = true, placeholder = "Your comment — paragraphs separated by blank lines" },
                },
                submit_label = "Post",
            },
        }
    end,
})
