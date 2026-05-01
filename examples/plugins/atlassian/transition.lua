-- Atlassian: Transition Issue — change a Jira issue's workflow state.
-- Step 1: form takes the issue key.
-- Step 2: list available transitions; selecting one POSTs and confirms.
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
    local msg, help_url
    if status == 401 then
        msg = "401 Unauthorized — token revoked or expired"
        help_url = "https://id.atlassian.com/manage-profile/security/api-tokens"
    elseif status == 403 then
        msg = "403 Forbidden — account lacks permission"
        help_url = "https://id.atlassian.com/manage-profile/security/api-tokens"
    elseif status == 404 then
        msg = "404 Not Found — issue key not found"
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
local function atlassian_get(auth, base, path, title)
    local resp = lark.http.get(base .. path, {
        headers = { Authorization = auth.header, Accept = "application/json" }, timeout = 15 })
    if not resp or resp.status == nil then
        return nil, { title = title, items = { error_item({
            label = "No response from Atlassian",
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
    return {}, nil
end

lark.register({
    on_run = function()
        local auth, err = atlassian_auth("Transition Issue")
        if err then return err end

        local key = lark.form_values and trim(lark.form_values.issue_key or "") or ""
        if key == "" then
            return {
                title = "Transition Issue",
                form = {
                    fields = {
                        { id = "issue_key", label = "Issue key", type = { kind = "text" },
                          required = true, placeholder = "e.g. PROJ-123" },
                    },
                    submit_label = "List transitions",
                },
            }
        end

        local data, rerr = atlassian_get(auth, auth.jira_base,
            "/rest/api/3/issue/" .. key .. "/transitions", "Transitions for " .. key)
        if rerr then return rerr end

        local items = {}
        for _, t in ipairs(data.transitions or {}) do
            local target_cat = t.to and t.to.statusCategory and t.to.statusCategory.key or ""
            local target_icon = (target_cat == "done") and "✔"
                or (target_cat == "indeterminate") and "▶"
                or (target_cat == "new") and "○" or "●"
            items[#items + 1] = {
                label = target_icon .. " " .. (t.name or "?"),
                detail = "→ " .. (t.to and t.to.name or "?"),
                icon = "🔀",
                actions = {
                    { label = "Apply transition", kind = "chain",
                      args = { "do_transition", key .. "|" .. t.id } },
                },
            }
        end
        if #items == 0 then
            items[#items + 1] = { label = "No transitions available", detail = "Issue may already be in a final state", icon = "📭" }
        end
        return {
            title = "Transitions for " .. key,
            items = items,
        }
    end,

    on_action = function(callback_id, context)
        if callback_id ~= "do_transition" then
            return { title = "Transition", items = { { label = "Unknown action", icon = "!" } } }
        end
        local auth, err = atlassian_auth("Transition")
        if err then return err end

        local key, tid = context:match("^([^|]+)|(.+)$")
        if not key or not tid then
            return { title = "Transition", items = { { label = "Invalid context: " .. context, icon = "!" } } }
        end

        local _, perr = atlassian_post(auth, auth.jira_base,
            "/rest/api/3/issue/" .. key .. "/transitions",
            { transition = { id = tid } }, "Transition " .. key)
        if perr then return perr end

        local url = (auth.site_url or auth.jira_base) .. "/browse/" .. key
        return {
            title = "Transitioned " .. key,
            items = {
                {
                    label = key .. " transitioned",
                    detail = "Status updated successfully",
                    icon = "✅",
                    actions = {
                        { label = "Open in browser", kind = "open", args = { url } },
                        { label = "Copy key", kind = "clipboard", args = { key } },
                    },
                },
            },
        }
    end,
})
