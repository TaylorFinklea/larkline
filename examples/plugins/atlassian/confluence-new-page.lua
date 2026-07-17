-- Atlassian: New Page — create a Confluence page from a form.
-- Body is treated as the Confluence "storage" format. Plain text gets wrapped
-- in <p>...</p> automatically; users who want richer markup can paste raw
-- Confluence storage XML.
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
    return { title = title or "Atlassian", items = {
        { label = "Not signed in to Atlassian",
          detail = "Run `lark atlassian login` for OAuth, or set ATLASSIAN_EMAIL + ATLASSIAN_API_TOKEN + atlassian_host for API-token auth",
          icon = "🔒",
          help_url = "https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/",
          actions = { { label = "Run `lark atlassian login`", kind = "shell", args = { "lark", "atlassian", "login" } } },
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

local function http_error_output(title, status, body)
    local msg, help_url
    if status == 401 then
        msg = "401 Unauthorized — token revoked or expired"
        help_url = "https://id.atlassian.com/manage-profile/security/api-tokens"
    elseif status == 403 then
        msg = "403 Forbidden"
        help_url = "https://id.atlassian.com/manage-profile/security/api-tokens"
    elseif status == 400 then
        msg = "400 Bad Request — " .. (body or ""):sub(1, 200)
        help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/"
    elseif status == 404 then
        msg = "404 Not Found — check space key"
        help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/"
    elseif status >= 500 then
        msg = string.format("%d — Atlassian is having issues", status)
        help_url = "https://status.atlassian.com/"
    else
        msg = "HTTP " .. tostring(status)
        help_url = "https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/"
    end
    return { title = title, items = { error_item({
        label = "Atlassian API error",
        detail = msg,
        help_url = help_url,
    }) } }
end
local function atlassian_post(auth, base, path, body_table, title)
    local resp = lark.http.post(base .. path, lark.json.encode(body_table or {}), {
        headers = { Authorization = auth.header, Accept = "application/json", ["Content-Type"] = "application/json" },
        timeout = 20,
    })
    if not resp or resp.status == nil then
        return nil, { title = title, items = { error_item({
            label = "No response from Atlassian",
            help_url = "https://status.atlassian.com/",
        }) } }
    end
    if resp.status < 200 or resp.status >= 300 then return nil, http_error_output(title, resp.status, resp.body) end
    if resp.body == "" then return {}, nil end
    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok then return {}, nil end
    return data, nil
end

-- Wrap plain text in Confluence storage format. Detect raw storage XML by
-- presence of an opening tag at the start; otherwise XML-escape and wrap each
-- paragraph in <p>...</p>.
local function to_storage(body)
    body = body or ""
    if body:match("^%s*<") then return body end
    local function xml_escape(s)
        return (s:gsub("&", "&amp;"):gsub("<", "&lt;"):gsub(">", "&gt;"))
    end
    local paras = {}
    for para in (body .. "\n\n"):gmatch("(.-)\n\n") do
        local trimmed = para:gsub("^%s+", ""):gsub("%s+$", "")
        if trimmed ~= "" then
            paras[#paras + 1] = "<p>" .. xml_escape(trimmed):gsub("\n", "<br/>") .. "</p>"
        end
    end
    if #paras == 0 then paras[#paras + 1] = "<p>" .. xml_escape(body) .. "</p>" end
    return table.concat(paras, "")
end

lark.register({
    on_run = function()
        local auth, err = atlassian_auth("New Page")
        if err then return err end

        if lark.form_values then
            local space  = trim(lark.form_values.space_key or "")
            local title  = trim(lark.form_values.title or "")
            local parent = trim(lark.form_values.parent_id or "")
            local body   = lark.form_values.body or ""

            if space == "" or title == "" then
                return { title = "New Page", items = {
                    { label = "Space key and title are required", icon = "!" },
                } }
            end

            local req = {
                type = "page",
                title = title,
                space = { key = space },
                body = {
                    storage = {
                        value = to_storage(body),
                        representation = "storage",
                    },
                },
            }
            if parent ~= "" then req.ancestors = { { id = parent } } end

            local data, perr = atlassian_post(auth, auth.conf_base, "/rest/api/content", req, "New Page")
            if perr then return perr end

            local site = auth.site_url or auth.conf_base:gsub("/wiki$", "")
            local url = (data and data._links and data._links.webui)
                and (site .. "/wiki" .. data._links.webui)
                or (site .. "/wiki/spaces/" .. space)
            return {
                title = "Created: " .. title,
                items = {
                    {
                        label = title,
                        detail = "in " .. space,
                        icon = "✅",
                        copy_text = title,
                        actions = {
                            { label = "Open in browser", kind = "open", args = { url } },
                            { label = "Copy URL", kind = "clipboard", args = { url } },
                        },
                    },
                },
            }
        end

        return {
            title = "New Page",
            form = {
                fields = {
                    { id = "space_key", label = "Space key", type = { kind = "text" },
                      required = true, placeholder = "e.g. PROJ" },
                    { id = "title", label = "Title", type = { kind = "text" },
                      required = true, placeholder = "Page title" },
                    { id = "parent_id", label = "Parent page id (optional)", type = { kind = "text" },
                      required = false, placeholder = "Numeric content id, leave blank for top-level" },
                    { id = "body", label = "Body", type = { kind = "text" },
                      required = false, placeholder = "Plain text — paragraphs become <p>. Or paste raw Confluence storage XML." },
                },
                submit_label = "Create",
            },
        }
    end,
})
