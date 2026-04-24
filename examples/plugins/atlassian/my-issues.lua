-- Atlassian: My Issues — Jira issues assigned to you, not yet Done.
-- Shared helpers copied from lib.lua (sandbox has no require).

local function not_signed_in_error(title)
    return {
        title = title or "Atlassian",
        items = {
            {
                label = "Not signed in to Atlassian",
                detail = "Run `lark atlassian login` for OAuth, or set ATLASSIAN_EMAIL + ATLASSIAN_API_TOKEN + atlassian_host for API-token auth",
                icon = "🔒",
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
        return {
            mode = "token",
            header = "Basic " .. lark.base64.encode(email .. ":" .. token),
            jira_base = site,
            conf_base = site .. "/wiki",
            site_url = site,
        }, nil
    end

    -- OAuth path: shell out to the running binary for a fresh access token.
    local bin = lark_binary()
    local tok = trim(lark.exec(bin, { "atlassian", "token" }))
    if tok == "" then return nil, not_signed_in_error(title) end
    local cid = trim(lark.exec(bin, { "atlassian", "cloudid" }))
    if cid == "" then return nil, not_signed_in_error(title) end
    local site = trim(lark.exec(bin, { "atlassian", "site" }))
    if site == "" then site = "https://api.atlassian.com/ex/jira/" .. cid end

    return {
        mode = "oauth",
        header = "Bearer " .. tok,
        jira_base = "https://api.atlassian.com/ex/jira/" .. cid,
        conf_base = "https://api.atlassian.com/ex/confluence/" .. cid,
        site_url = site,
    }, nil
end

local function url_encode(s)
    if s == nil then return "" end
    return (tostring(s):gsub("([^%w%-_%.~])", function(c)
        return string.format("%%%02X", string.byte(c))
    end))
end

local function build_query(params)
    if not params then return "" end
    local parts = {}
    for k, v in pairs(params) do
        if v ~= nil then parts[#parts + 1] = url_encode(k) .. "=" .. url_encode(v) end
    end
    if #parts == 0 then return "" end
    return "?" .. table.concat(parts, "&")
end

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

local function atlassian_get(auth, base, path, params, title)
    local url = base .. path .. build_query(params)
    local resp = lark.http.get(url, {
        headers = { Authorization = auth.header, Accept = "application/json" },
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

local function issue_icon(type_name)
    if not type_name then return "•" end
    local t = type_name:lower()
    if t == "bug" then return "🐛" end
    if t == "story" then return "📘" end
    if t == "task" then return "✓" end
    if t == "epic" then return "🗂" end
    if t == "sub-task" or t == "subtask" then return "└" end
    return "•"
end

local function status_badge(status)
    if not status then return "?" end
    local cat = status.statusCategory and status.statusCategory.key or ""
    local name = status.name or "?"
    if cat == "done" then return "✔ " .. name end
    if cat == "indeterminate" then return "▶ " .. name end
    if cat == "new" then return "○ " .. name end
    return "● " .. name
end

local function issue_browser_url(auth, key)
    return (auth.site_url or auth.jira_base) .. "/browse/" .. key
end

local function issue_to_row(auth, issue)
    local f = issue.fields or {}
    local type_name = f.issuetype and f.issuetype.name or nil
    local priority = f.priority and f.priority.name or nil
    local updated = f.updated and f.updated:sub(1, 10) or nil
    local detail_parts = { status_badge(f.status) }
    if priority then detail_parts[#detail_parts + 1] = priority end
    if updated then detail_parts[#detail_parts + 1] = updated end
    local url = issue_browser_url(auth, issue.key)
    return {
        label = issue.key .. "  " .. (f.summary or ""),
        detail = table.concat(detail_parts, "  ·  "),
        icon = issue_icon(type_name),
        copy_text = issue.key,
        actions = {
            { label = "Open in browser", kind = "open", args = { url } },
            { label = "Copy key", kind = "clipboard", args = { issue.key } },
            { label = "Copy URL", kind = "clipboard", args = { url } },
            { label = "View details", kind = "chain", args = { "show_detail", issue.key } },
            { label = "Comment", kind = "chain", args = { "go_comment", issue.key } },
            { label = "Transition", kind = "chain", args = { "go_transition", issue.key } },
        },
    }
end

lark.register({
    on_run = function()
        local auth, err = atlassian_auth()
        if err then return err end

        local max = tonumber(lark.store.get("max_results")) or 50
        local jql = "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC"
        local data, rerr = atlassian_get(auth, auth.jira_base,
            "/rest/api/3/search",
            {
                jql = jql,
                fields = "summary,status,issuetype,priority,updated",
                maxResults = tostring(max),
            },
            "My Issues")
        if rerr then return rerr end

        local items = {}
        for _, issue in ipairs(data.issues or {}) do
            items[#items + 1] = issue_to_row(auth, issue)
        end
        if #items == 0 then
            items[#items + 1] = { label = "No open issues assigned to you", icon = "✅" }
        end
        return {
            title = string.format("My Issues — %d", #items),
            items = items,
        }
    end,

    on_action = function(callback_id, context)
        -- Detail view: GET /rest/api/3/issue/{key}?fields=...
        if callback_id == "show_detail" then
            local auth, err = atlassian_auth()
            if err then return err end
            local data, rerr = atlassian_get(auth, auth.jira_base,
                "/rest/api/3/issue/" .. context,
                { fields = "summary,status,issuetype,priority,assignee,reporter,created,updated,description" },
                "Issue " .. context)
            if rerr then return rerr end
            local f = data.fields or {}
            local lines = { "# " .. context .. " — " .. (f.summary or "") }
            lines[#lines + 1] = ""
            lines[#lines + 1] = "- **Status:** " .. status_badge(f.status)
            if f.issuetype then lines[#lines + 1] = "- **Type:** " .. f.issuetype.name end
            if f.priority then lines[#lines + 1] = "- **Priority:** " .. f.priority.name end
            if f.assignee then lines[#lines + 1] = "- **Assignee:** " .. (f.assignee.displayName or f.assignee.emailAddress or "?") end
            if f.reporter then lines[#lines + 1] = "- **Reporter:** " .. (f.reporter.displayName or "?") end
            if f.updated then lines[#lines + 1] = "- **Updated:** " .. f.updated:sub(1, 10) end
            -- Render ADF description inline — minimal reducer.
            if f.description then
                lines[#lines + 1] = ""
                lines[#lines + 1] = "## Description"
                lines[#lines + 1] = ""
                local function flatten(node, out)
                    out = out or {}
                    if type(node) ~= "table" then return out end
                    if node.type == "text" then out[#out + 1] = node.text or "" end
                    if node.content then for _, c in ipairs(node.content) do flatten(c, out) end end
                    if node.type == "paragraph" or node.type == "heading" or node.type == "listItem" then
                        out[#out + 1] = "\n"
                    end
                    return out
                end
                lines[#lines + 1] = table.concat(flatten(f.description), "")
            end
            local url = issue_browser_url(auth, context)
            return {
                title = context,
                raw_text = table.concat(lines, "\n"),
                output_format = "markdown",
                items = {
                    {
                        label = (f.summary or context),
                        detail = status_badge(f.status),
                        icon = issue_icon(f.issuetype and f.issuetype.name),
                        actions = {
                            { label = "Open in browser", kind = "open", args = { url } },
                            { label = "Copy key", kind = "clipboard", args = { context } },
                            { label = "Comment", kind = "chain", args = { "go_comment", context } },
                            { label = "Transition", kind = "chain", args = { "go_transition", context } },
                        },
                    },
                },
            }
        elseif callback_id == "go_comment" then
            return { title = "Comment", items = {
                { label = "Open Comment command to add a comment",
                  detail = "Press `jcm " .. context .. "` or run the Comment command",
                  icon = "💬",
                  actions = { { label = "Copy key to paste into Comment", kind = "clipboard", args = { context } } },
                },
            } }
        elseif callback_id == "go_transition" then
            return { title = "Transition", items = {
                { label = "Open Transition command to change status",
                  detail = "Press `jtx " .. context .. "` or run the Transition command",
                  icon = "🔀",
                  actions = { { label = "Copy key to paste into Transition", kind = "clipboard", args = { context } } },
                },
            } }
        end
        return { title = "Atlassian", items = { { label = "Unknown action: " .. callback_id, icon = "!" } } }
    end,
})
