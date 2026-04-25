-- Atlassian: New Jira Issue — create-issue form, posts to /rest/api/3/issue.
-- Shared helpers copied from lib.lua.

local function not_signed_in_error(title)
    return { title = title or "Atlassian", items = {
        { label = "Not signed in to Atlassian",
          detail = "Run `lark atlassian login` for OAuth, or set ATLASSIAN_EMAIL + ATLASSIAN_API_TOKEN + atlassian_host for API-token auth",
          icon = "🔒",
          actions = {
              { label = "Run `lark atlassian login`", kind = "shell", args = { "lark atlassian login" } },
          },
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
        or (status == 404) and "404 Not Found — check project key / issue type"
        or (status == 400) and ("400 Bad Request — " .. (body or ""):sub(1, 200))
        or (status >= 500) and string.format("%d — Atlassian is having issues", status)
        or "HTTP " .. tostring(status)
    return { title = title, items = { { label = "Atlassian API error", detail = msg, icon = "!" } } }
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
    if resp.body == "" then return {}, nil end
    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok then return {}, nil end
    return data, nil
end

-- Wrap plain-text description in an ADF doc (Atlassian's required format).
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
        local auth, err = atlassian_auth("New Jira Issue")
        if err then return err end

        if lark.form_values then
            local project = trim(lark.form_values.project_key or "")
            local itype   = trim(lark.form_values.issue_type or "Task")
            local summary = trim(lark.form_values.summary or "")
            local desc    = lark.form_values.description or ""

            if project == "" or summary == "" then
                return { title = "New Jira Issue", items = {
                    { label = "Project key and summary are required", icon = "!" },
                } }
            end

            local body = {
                fields = {
                    project    = { key = project },
                    issuetype  = { name = itype },
                    summary    = summary,
                },
            }
            if desc ~= "" then body.fields.description = adf_from_plaintext(desc) end

            local data, perr = atlassian_post(auth, auth.jira_base, "/rest/api/3/issue", body, "New Jira Issue")
            if perr then return perr end

            local key = data and data.key or "?"
            local url = (auth.site_url or auth.jira_base) .. "/browse/" .. key
            return {
                title = "Created " .. key,
                items = {
                    {
                        label = key .. "  " .. summary,
                        detail = "Created in " .. project,
                        icon = "✅",
                        copy_text = key,
                        actions = {
                            { label = "Open in browser", kind = "open", args = { url } },
                            { label = "Copy key", kind = "clipboard", args = { key } },
                            { label = "Copy URL", kind = "clipboard", args = { url } },
                        },
                    },
                },
            }
        end

        local default_proj = lark.store.get("default_project_key")
        default_proj = (type(default_proj) == "string" and default_proj ~= "") and default_proj or ""
        return {
            title = "New Jira Issue",
            form = {
                fields = {
                    { id = "project_key", label = "Project key", type = { kind = "text" },
                      required = true, default_value = default_proj, placeholder = "e.g. PROJ" },
                    { id = "issue_type", label = "Issue type", type = { kind = "text" },
                      required = true, default_value = "Task", placeholder = "Task, Story, Bug, …" },
                    { id = "summary", label = "Summary", type = { kind = "text" },
                      required = true, placeholder = "One-line title for the issue" },
                    { id = "description", label = "Description", type = { kind = "text" },
                      required = false, placeholder = "Optional. Plain text — paragraphs separated by blank lines." },
                },
                submit_label = "Create",
            },
        }
    end,
})
